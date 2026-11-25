# cadmium — Technical Architecture & Design Document

**Corresponding PRD:** v1.1  
**Target Language:** Rust  
**Graphics Backend:** `wgpu`  
**Scope:** System architecture, ownership model, frame pipeline, rendering graph, layout/virtualization mechanics, input/IME, and multi-window design (conceptual design — no implementation code).

---

# 1. Architectural Overview & Module Dependency Graph

To prevent circular dependencies and keep compile times predictable, cadmium’s subsystems form a strict **Directed Acyclic Graph (DAG)**. Lower layers never import higher layers; communication upward happens strictly through queues, callbacks/traits, or dirty-flag notifications.

```text
                       +-----------------------+
                       |   app / window loop   |  (Orchestrates GpuContext + WindowContexts)
                       +-----------+-----------+
                                   |
          +------------------------+------------------------+
          |                        |                        |
          v                        v                        v
+-------------------+    +-------------------+    +-------------------+
|      widgets      |    |    accessibility  |    |      debug        |
| (Tier 1 & Tier 2) |    |   (AccessKit)     |    | (Overlay/Metrics) |
+---------+---------+    +---------+---------+    +-------------------+
          |                        |
    +-----+----------+-------------+-----+
    |                |                   |
    v                v                   v
+--------+     +-----------+      +------------+
| events |     |  layout   |      |   style    |
| (Focus,|     | (Taffy +  |      | (Tokens &  |
|  IME)  |     |  Virtual) |      |  Themes)   |
+---+----+     +-----+-----+      +-----+------+
    |                |                  |
    +-------+--------+--------+---------+
            |                 |
            v                 v
      +-----------+     +-----------+
      | reactive  |     |   text    |
      | (Signals) |     | (Shaping) |
      +-----+-----+     +-----+-----+
            |                 |
            +--------+--------+
                     |
                     v
            +-----------------+
            |    renderer     |  (Render Graph, SDF, Vector, Compositor, Blur)
            +--------+--------+
                     |
         +-----------+-----------+
         |                       |
         v                       v
  +-------------+         +-------------+
  |   assets    |         |  platform   |
  | (Img/SVG/   |         | (Clipboard, |
  |  Font Cache)|         |  Dialogs)   |
  +------+------+         +-------------+
         |
         v
  +-------------+
  |    core     |  (IDs, Geometry, Color, Error, Async Waker)
  +-------------+
```

### Process vs. Window Ownership Split
All runtime state is divided cleanly across two lifetimes:

| Scope | Owner | Responsibilities |
|---|---|---|
| **Process-Wide** | `GpuContext` & `AppState` | `wgpu` Instance/Adapter/Device/Queue, compiled shader pipelines, shared glyph/icon/image texture caches, `FontSystem`, `ReactiveRuntime`, global theme tokens, async executor bridge. |
| **Per-Window** | `WindowContext` | `winit` Window, `wgpu` Surface & swapchain config, canonical offscreen render target, scale factor, window root `WidgetId`, `TaffyTree`, hover/focus/drag/IME state, `AccessKit` adapter. |

---

# 2. Memory, Ownership & Reactive Architecture

Rust’s borrow checker rejects traditional OOP GUI trees (where parents own children, children hold parent pointers, and event closures capture `&mut` references to siblings). cadmium solves this by decoupling **storage ownership** from **logical relationships**.

## 2.1 Arena-Backed Retained Widget Tree
- Every widget instance lives inside a central **generational arena / slotmap** (`WidgetArena`) rather than nested `Box` pointers on the heap.
- Widgets reference each other exclusively through lightweight, copyable **`WidgetId` handles** (index + generation tag to catch stale references safely).
- **Node Record Contents:**
  - Hierarchy links: `parent`, `first_child`, `next_sibling`, `prev_sibling`.
  - Identity & Flags: `dirty_state`, `dirty_layout`, `dirty_paint`, `is_relayout_boundary`, `accessible_role`.
  - Cached Computed State: resolved `Style`, last laid-out physical `Rect`, scroll offset, clip metadata.
  - Behaviour Payload: trait object / enum implementing measure, layout sync, paint, and event handling.
  - Owned Reactive Handles: list of `SignalId` / `EffectId` subscriptions owned by this widget.

**Why generational arenas work here:** Event handlers and reactive updates hold `WidgetId`s rather than Rust references, allowing safe, localized mutation of any node while traversing the tree. When a widget is removed, its slot is freed and its owned reactive subscriptions are automatically unsubscribed in bulk.

## 2.2 Fine-Grained Reactive Runtime
- **Signals as Copy Handles:** User-facing state (`Signal<T>`) is a tiny handle pointing into a central `ReactiveStore` on the UI thread. Reading a signal inside a reactive binding automatically registers the current `EffectId` as a subscriber.
- **Deferred Invalidation (Never Mutate Mid-Pass):** Writing to a signal does *not* immediately mutate the widget tree or trigger `taffy`. Instead, it marks dependent `EffectId`s as dirty and pushes them onto an **Invalidation Queue** (and wakes the event loop if triggered via the async bridge).
- **Dynamic Children Reconciliation:** Control-flow containers (conditional visibility, dynamic lists) own keyed slots so that adding/removing one item updates only that child slot without reconstructing sibling subtrees.

## 2.3 Async & Multi-Threading Bridge
- Background work runs on a worker thread pool or async runtime (e.g., Tokio) completely independent of the UI loop.
- Workers receive a thread-safe **`UiDispatcher`** (wrapping an MPSC channel + `winit` `EventLoopProxy`).
- When a background task finishes (e.g., HTTP response, decoded image, file read), it sends a state-update closure/message through the dispatcher and wakes the event loop.
- The UI thread drains all pending async updates in a single batch at the start of the frame before running layout or paint.

---

# 3. Deterministic Frame Execution Pipeline

Every active frame executes in a strict, deterministic 7-phase sequence. If no inputs, async messages, dirty flags, or active animations exist at the end of a frame, the event loop sleeps indefinitely with **0% CPU/GPU usage**.

```text
[OS Events / Async Messages / Timers]
                 |
                 v
  1. INPUT & ASYNC DRAIN     --> Updates pointer/focus/IME state; applies async mutations
                 |
                 v
  2. REACTIVE FLUSH          --> Evaluates dirty Effects; sets dirty_layout / dirty_paint flags
                 |
                 v
  3. ANIMATION TICK (dt)     --> Advances springs/easings; writes properties; marks dirty flags
                 |
                 v
  4. INCREMENTAL LAYOUT      --> Bubbles dirty_layout to Relayout Boundaries; runs Taffy + pixel snap
                 |
                 v
  5. DISPLAY LIST RECORDING  --> Walks visible nodes with dirty_paint; builds hierarchical DisplayList
                 |
                 v
  6. RENDER GRAPH & GPU      --> Plans offscreen passes (custom surfaces, blur); batches & submits wgpu
                 |
                 v
  7. A11Y SYNC & PRESENT     --> Pushes AccessKit TreeUpdate for changed nodes; presents swapchain
```

### Invalidation Rules Between Phases
- **State Change Only (e.g., button hover colour, cursor blink):** Marks `dirty_paint` only. Phase 4 (Taffy) is **completely skipped**.
- **Geometry Change Inside Fixed Container (e.g., text wraps inside a fixed-width sidebar):** `dirty_layout` bubbles up and stops at the sidebar (**Relayout Boundary**). Taffy runs only on that subtree.
- **Scroll Event:** Updates the viewport's scroll offset and marks `dirty_paint` (and updates virtualized list window if thresholds are crossed). Sibling/parent layout is untouched.

---

# 4. Hybrid GPU Rendering Engine & Render Graph

cadmium avoids a monolithic "draw everything in one fixed shader" approach. Instead, widgets emit high-level commands into a per-window **`DisplayList`**, which the **Pass Planner (Render Graph)** compiles into optimal `wgpu` render passes.

## 4.1 Multi-Window Colour Format Strategy
Because different monitors can negotiate different `wgpu::Surface` formats (e.g., 8-bit sRGB vs. 10-bit/16-bit HDR), compiling every SDF, vector, text, and blur pipeline for every possible surface format is brittle.
- **Design:** Each `WindowContext` renders its entire UI into a **canonical intermediate offscreen texture** (`Rgba8UnormSrgb` for SDR or `Rgba16Float` for wide-gamut/HDR).
- At the end of the frame, a lightweight **Final Present Pass** blits/tonemaps that canonical texture onto the window’s actual `Surface` texture.
- **Benefits:**
  1. Shared GPU pipelines compile against **one canonical format** across all windows.
  2. **Backdrop blur** always has an accessible source texture to sample from.
  3. **Headless visual regression tests** can read back the canonical texture identically on every OS without opening visible swapchains.

## 4.2 The Four Rendering Subsystems

### A. Instanced SDF Pipeline (Primary UI Chrome)
Handles 90%+ of UI geometry in minimal draw calls:
- **Instance Buffer Layout (GPU-aligned):** Bounding rect, corner radii (4 corners), fill mode (solid / linear gradient / radial gradient), gradient start/end points & colour stops, border width & colour, shadow blur/spread/offset, opacity, transform index, and clip index.
- **Rendering:** Draws a single instanced unit quad per primitive. The fragment shader evaluates exact analytical Signed Distance Fields for rounded rectangles, circles, borders, and Gaussian-approximated box shadows with resolution-independent anti-aliasing.

### B. Text Pipeline
- `cosmic-text` maintains shaped glyph runs (`Buffer`) cached per text widget.
- Before rendering, a **Glyph Upload Pass** checks if any needed glyphs are missing from the shared `GlyphAtlas`, rasterizes them on the CPU via `swash`, and uploads only the new dirty sub-rectangles into GPU textures:
  - **Mask Page (`R8Unorm`):** Grayscale/alpha masks for standard text (tinted in shader).
  - **Colour Page (`Rgba8UnormSrgb`):** Full-colour emoji glyphs.
- Text quads are batched into a single instanced/indexed draw call per layer.

### C. Vector & SVG Pipeline
Split into two distinct paths based on workload:
- **Static SVGs & Icons:** Parsed and rasterized on demand via `resvg` at `(logical_size * window_scale_factor)` and packed into a shared **Icon Texture Atlas** (LRU-evicted when memory budget is hit). Rendered as simple textured quads.
- **Dynamic Live Paths (Splines, Charts, Node Wires):** Tessellated into triangle meshes (via `lyon`) with a geometry cache keyed by path version hash. Only re-tessellated when control points actually animate or change.

### D. Custom GPU Surface Host
Enables DaVinci Resolve / CAD / 3D / shader viewports inside cadmium layouts:
- During layout, a `CustomGpuSurface` widget receives its exact physical pixel dimensions.
- In Phase 6 (before main UI compositing), cadmium passes a dedicated `wgpu` offscreen texture target (`TextureView`) and command encoder context to the widget’s render hook.
- cadmium then composites that texture into the `DisplayList` like an image layer—automatically inheriting rounded clipping, opacity, and allowing dropdowns/tooltips/modals to render cleanly *above* the custom GPU viewport.

## 4.3 Clipping & Backdrop Blur Execution (Render Graph)
- **Clip Stack Optimization:**
  - *Axis-aligned rectangular clips:* Executed via fast hardware `set_scissor_rect` (zero shader overhead).
  - *Rounded or rotated clips:* Stored in a per-frame **Clip Storage Buffer**. Each SDF/text/vector instance carries a `clip_id` (or clip chain index) evaluated analytically in the fragment shader—eliminating expensive stencil passes for common rounded cards and scroll viewports. Deeply nested arbitrary clip masks fall back to offscreen alpha-masked layers.
- **Backdrop Blur Pass Sequence:**
  When the Pass Planner encounters a translucent blurred panel (e.g., sidebar, popover, modal backdrop):
  1. Flushes all prior draw calls into the window's canonical target texture.
  2. Copies the bounded region behind the panel into a half/quarter-res **Blur Ping-Pong Texture**.
  3. Runs separable multi-pass **Dual-Kawase Blur** shaders (fast, wide-radius, artifact-free at 120+ FPS).
  4. Draws the blurred panel using its SDF rounded shape as the mask, blending tinted glass colour + noise/grain (to prevent colour banding) on top.
  5. Resumes batching subsequent foreground draws.

---

# 5. Layout Engine & Virtualization Design

## 5.1 Wrapping `taffy` Without Leaking It
- `LayoutEngine` (per window) owns a `taffy::TaffyTree` and a bidirectional map `WidgetId <-> TaffyNodeId`.
- cadmium exposes its own ergonomic layout types (`Length`, `Size`, `Edges`, `Align`, `Justify`, `LayoutMode`). Changes to widget styles sync lazily into `taffy` only for nodes marked `dirty_layout`.

## 5.2 Avoiding Borrow Conflicts in Leaf Measurement
When `taffy` computes layout, it invokes a `measure_function` callback for leaf nodes (primarily `Text` and `Image`) whose intrinsic height depends on available width (e.g., text wrapping).
- **The Trap:** If `taffy` lives inside `WindowContext` and the callback needs to read `WidgetArena` and mutate `FontSystem`, Rust’s borrow checker rejects overlapping `&mut` borrows.
- **The Solution:** Pass a lightweight **`MeasureContext`** into `taffy` containing *only* immutable references to leaf metadata (`TextLayoutKey` / `ImageDimensions`) and mutable access to `FontSystem` + `TextMeasureCache`—completely disjoint from `WidgetArena` and `TaffyTree`.

## 5.3 Relayout Boundaries & Fractional DPI Pixel Snapping
- **Relayout Boundaries:** Any widget whose outer size does not depend on its children (the `Window` root, `ScrollView` viewport, or widgets with explicit fixed width & height) acts as a boundary. Inner layout dirtying stops at the boundary.
- **Pixel Snapping:** `taffy` computes layout in logical float coordinates. After `taffy` finishes, cadmium runs a **Snap Pass** multiplying coordinates by the window’s `scale_factor`, rounding edge positions (`x`, `y`, `x + w`, `y + h`) to integer physical pixels, and deriving width/height from the snapped edges. This guarantees two adjacent boxes never leave a 1-subpixel gap at 125% or 150% scaling.

## 5.4 Viewport Virtualization Contract
For Notion/VS Code/Discord scale (100k+ items), cadmium defines a `VirtualDataSource` protocol:
1. The virtual container (`VirtualizedList`, `TreeView`, `Table`) queries total item count and estimated/cached row heights without instantiating widgets.
2. Based on current `scroll_offset` and viewport height, it calculates the `[first_visible .. last_visible]` range plus a small overscan buffer (e.g., 5 items above/below).
3. Only widgets inside that active window are mounted in `WidgetArena` and `TaffyTree`.
4. Items scrolling out of view are either unmounted (freeing reactive subscriptions) or returned to a typed node pool for reuse.

---

# 6. Input, Hit-Testing, Focus & IME

## 6.1 Spatial Hit-Testing (CPU-Side)
Because the `DisplayList` records exact final bounds, transforms, scroll offsets, and clip regions during Phase 5, hit-testing does **not** touch the GPU:
- Walks the active window’s visual hierarchy in **reverse paint order** (topmost overlay/portal first).
- Rejects points outside active clip rects or rounded clip bounds.
- Applies inverse 2D transforms for rotated/scaled elements.
- Returns the hit path `[Root -> ... -> TargetWidgetId]`.

## 6.2 Event Propagation & Gestures
- **Capture Phase (Root → Target):** Allows parent containers (like `ScrollView`, `DockArea`, or global shortcut handlers) to intercept events first.
- **Bubble Phase (Target → Root):** Standard widget handling (`Button` click, `Slider` drag). Any handler can mark the event as `Handled` to stop propagation.
- **Pointer Capture:** When a user presses down on a `Slider` thumb or `SplitPane` divider, that `WidgetId` acquires pointer capture so drag events keep routing to it even if the cursor leaves its bounds or window edge.

## 6.3 Focus Management & Focus Scopes
- Maintains a single active focused `WidgetId` per window plus a stack of **Focus Scopes** (e.g., a `Modal` or `Dropdown` pushes a scope that traps `Tab` / `Shift+Tab` cycling inside itself until dismissed).
- Supports directional focus navigation and keyboard shortcut tables with platform-aware modifiers (`Primary` = `Cmd` on macOS, `Ctrl` on Windows/Linux).

## 6.4 IME (Input Method Editor) Integration
- When a `TextInput` or `TextArea` gains focus, `WindowContext` enables IME on `winit`.
- Whenever layout or cursor position changes inside the focused text widget, it reports its caret physical rect to `winit` (`set_ime_cursor_area`) so OS CJK/emoji candidate popups appear directly under the cursor.
- Pre-edit strings (`Ime::Preedit`) are rendered inline with a composition underline without committing them to the underlying text model until `Ime::Commit` arrives.

---

# 7. Multi-Window, Overlays & Platform Integration

## 7.1 Portals & Window Overflow
Floating UI (`Popover`, `ContextMenu`, `Tooltip`) uses a **`Portal`** layer:
- By default, portals render in the parent window’s top-most overlay slot (above all clipping boundaries of the originating widget).
- For detached toolbars, torn-off `DockArea` tabs, or context menus overflowing small windows, cadmium can promote a subtree into its own child `WindowContext` using the exact same `GpuContext` and reactive signals—meaning state stays live across windows automatically.

## 7.2 Custom Client-Side Window Chrome
For macOS-style unified toolbars and cross-platform custom titlebars:
- cadmium exposes semantic regions (`WindowDragRegion`, `WindowControlButtons`).
- Pointer down inside a `WindowDragRegion` triggers `winit`'s native OS window drag (`drag_window()`), preserving OS snapping (Aero Snap on Windows, tiling on Wayland, Mission Control on macOS).
- Edge hit-testing on borderless windows delegates resize edges to `winit` (`drag_resize_window()`).

## 7.3 Accessibility (`AccessKit`) Sync
- Every widget in `WidgetArena` has an immutable accessibility node ID derived from its `WidgetId`.
- During Phase 7, any widget created, destroyed, or changed in state/bounds pushes a delta entry into an incremental `AccessKit` `TreeUpdate`.
- Platform screen-reader actions (e.g., VoiceOver/NVDA "Click" or "SetFocus") route back through the cadmium event dispatcher as first-class semantic actions.

---

# 8. Error Handling, Recovery & Testing Strategy

## 8.1 GPU & Surface Resilience
- **Surface Lost / Outdated / Suboptimal:** `WindowContext` treats swapchain errors as recoverable state transitions—dropping the acquired surface texture first, re-querying `window.inner_size()`, reconfiguring the `Surface`, and scheduling a single redraw (skipping rendering altogether when minimized to `0x0`).
- **Device Lost:** Triggers a `GpuContext` rebuild hook to re-upload cached textures/atlases or surfaces a graceful fatal diagnostic if hardware was physically disconnected.

## 8.2 Verification Architecture
- **Pure Logic Unit Suites:** Reactive graph cycle detection, dirty propagation rules, virtualization math, and hit-test clipping run headlessly without a GPU or window.
- **Headless GPU Visual Regression Harness:** Creates an offscreen `wgpu::Device` (software adapter like SwiftShader/WARP in CI or hardware GPU locally), runs Phases 2–6 to render into the canonical texture, reads pixels back to CPU, and compares perceptual diffs against golden reference PNGs across Windows, Linux, and macOS.
- **Built-in Frame Profiler Overlay:** Renders live telemetry directly via the SDF + Text pipeline (phase timings for Reactive/Layout/Paint/GPU, draw-call count, glyph/icon atlas utilization, and relayout flash highlights).

---

# 9. Summary of Key Technical Contracts to Implement First

When beginning **Phase 1 & Phase 2**, lock these four internal contracts before writing widgets:
1. **Generational `WidgetId` + `Signal<T>` handle semantics** (ensuring zero borrow-checker friction).
2. **Canonical Offscreen Texture + Final Blit** per `WindowContext` (ensuring multi-window format safety, backdrop blur, and headless testing).
3. **Instanced SDF Buffer Layout + Analytical Clip Stack** (keeping UI draw calls tiny).
4. **Disjoint `MeasureContext`** between `TaffyTree` and `cosmic-text` (keeping layout clean and fast).
