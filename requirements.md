---

# cadmium — Product Requirements Document

**Version:** 1.1  
**Status:** Architecture-aligned specification  
**Language:** Rust  
**Graphics API:** wgpu  
**Project type:** Cross-platform GPU-native GUI framework  

---

# 1. Project Vision

cadmium is a custom, GPU-accelerated GUI framework written in Rust and built on top of `wgpu`.

Its objective is to enable developers to build visually sophisticated, high-performance desktop applications—ranging from knowledge tools and IDEs to real-time communication and creative media software—with consistent appearance and behaviour across Windows, Linux, and macOS.

The design philosophy takes inspiration from macOS: refined typography, rounded geometry, subtle transparency, fluid animations, smooth transitions, carefully designed spacing, and sophisticated visual effects. However, cadmium must establish its own design language rather than directly reproducing Apple's proprietary interface.

**Fundamental principle:** The same cadmium application should look and behave consistently regardless of the underlying operating system.

cadmium must not rely on native OS widgets for its visual interface. It owns its rendering, widget tree, layout integration, styling, reactivity, animation, and event systems, while isolating specialised OS integrations behind internal abstractions.

---

# 2. Primary Objectives

1. Build a hybrid GPU-native 2D rendering engine using `wgpu` (instanced SDF primitives, vector paths, text, compositing, and custom GPU surfaces).
2. Implement a hybrid retained and fine-grained reactive UI programming model with predictable Rust ownership.
3. Develop a reusable, composable, and virtualizable widget system.
4. Provide a flexible layout abstraction backed internally by a proven constraint/Flex/Grid solver (`taffy`).
5. Architect for multi-window applications from day one.
6. Support advanced visual effects including backdrop blur, Gaussian blur, shadows, gradients, and offscreen layer compositing.
7. Provide a unified, responsive input, focus, IME, and event system.
8. Implement time-based, refresh-rate-independent animations and spring transitions.
9. Provide a developer-friendly Rust API that does not require direct `wgpu` knowledge for standard UI, while offering safe custom-shader/GPU surface escape hatches.
10. Maintain high performance (60–144+ FPS, idle-frame skipping, minimal allocations) verified by benchmarks.

---

# 3. Supported Platforms

Initial targets:
- Windows 10 and 11
- Linux (X11 and Wayland)
- macOS

Requirements:
- Identical widget appearance, typography, spacing, and animation behaviour across platforms.
- Platform-independent layout calculations with explicit **fractional DPI scaling and pixel-grid snapping** (supporting 100%, 125%, 150%, 200%+ scaling without hairline seams or blurry 1px borders).
- Support for multi-monitor setups with mixed DPI scaling, refresh rates, and surface colour formats (SDR/HDR).
- Support for custom client-side window chrome (custom titlebars, drag regions, resize borders) alongside standard decorations.
- Graceful GPU fallback where supported.
- Platform-specific code isolated behind internal abstractions.

---

# 4. Technology Stack & Dependency Policy

cadmium implements its core GUI systems internally while using established crates for specialised low-level or platform-specific capabilities:

| Subsystem | Technology | Role & Boundary |
|---|---|---|
| Programming language | Rust | Core framework |
| GPU rendering | `wgpu` | GPU abstraction (exposed only via controlled custom-surface APIs) |
| Window management | `winit` | Cross-platform window creation and OS event loop |
| Layout solver | `taffy` | Internal Flex/Grid layout calculation behind cadmium's layout API |
| Text shaping | `cosmic-text` | Font discovery, shaping, bidi, wrapping, and line layout |
| Font rasterization | `swash` | Glyph rasterization (colour + alpha masks) |
| Static SVG rendering | `resvg` | Internal SVG parsing/rasterization into cached GPU textures |
| Dynamic vector geometry | `lyon` (or GPU compute path pipeline) | Tessellation/rendering of live Bezier curves and arbitrary paths |
| Image decoding | `image` | Decoding PNG, JPEG, WebP, etc. |
| Accessibility | `accesskit` | Native OS screen-reader and accessibility tree bridge |
| Clipboard | `arboard` | Cross-platform clipboard integration |
| Native dialogs | `rfd` | Native OS file/folder open and save dialogs |
| Math | `glam` | Vectors, matrices, transforms |
| Serialization | `serde` | Theme, config, and diagnostic serialization |
| Logging & profiling | `tracing` | Structured diagnostics and frame telemetry |
| Error handling | `thiserror` | Ergonomic internal/public error types |

**Dependency isolation rule:** Types from `taffy`, `winit`, `cosmic-text`, `accesskit`, `arboard`, `rfd`, and `resvg` must remain internal implementation details and must not leak into cadmium's public widget or application APIs.

---

# 5. Programming Model & Reactivity Architecture

cadmium uses a **hybrid retained and fine-grained reactive programming model**.

## 5.1 Retained Tree & Ownership
- cadmium maintains a persistent retained widget tree stored in an **arena / slotmap architecture** with stable handle-based IDs.
- Stable node IDs enable predictable Rust ownership without `Rc`/`RefCell` reference cycles, clean parent-child traversal, and persistent mapping to Taffy layout nodes and `AccessKit` semantic nodes.
- Applications avoid full-tree reconstruction when local state changes; only affected components or dynamic child slots update.

## 5.2 Fine-Grained Reactive State
- State updates are driven by reactive primitives (signals, derived computations, and subscriptions) tied to specific widget properties or dynamic subtrees.
- Component communication supports both reactive shared state and typed event/message passing.
- Subscriptions automatically clean up when their owning widget node is removed from the retained tree.

## 5.3 Three-Stage Invalidation Pipeline
To eliminate redundant work, every widget tracks three independent dirty flags:
1. **State / Style Invalidation (`dirty_state`):** Re-evaluates reactive bindings, styles, or dynamic children for subscribed nodes only.
2. **Layout Invalidation (`dirty_layout`):** Propagates upward only to the nearest **relayout boundary** (e.g., fixed-size containers or scroll viewports), triggering `taffy` recalculation solely where geometry can change.
3. **Paint Invalidation (`dirty_paint`):** Re-records GPU primitives or dirties a cached compositing layer without running layout (e.g., hover states, opacity, colour transitions, scroll offsets).

## 5.4 Asynchronous Operations
- Background tasks (network calls, file I/O, decoding, media processing) execute off the UI thread.
- An async bridge and event-loop waker allow background workers to dispatch state updates safely onto the main UI thread without blocking rendering or animations.

---

# 6. Hybrid GPU Rendering Engine

The rendering engine is built on `wgpu` and uses a **hybrid GPU rendering architecture** designed for both standard UI density and creative graphics workloads.

## 6.1 Instanced SDF Subsystem (Primary UI Primitives)
Handles high-volume UI primitives via instanced GPU draw calls and Signed Distance Field (SDF) shaders:
- Rectangles and rounded rectangles (including per-corner radii).
- Circles and ellipses.
- Crisp anti-aliased borders and outlines.
- Solid colours, linear gradients, and radial gradients.
- Analytical drop shadows and inner shadows.
- Opacity, 2D affine transforms (rotation, translation, scaling), and transform hierarchies.

## 6.2 Vector Graphics & SVG Subsystem
Separate from SDF primitives, dedicated to arbitrary geometry:
- **Static SVGs & Icons:** Rasterized on demand at target DPI scales via `resvg` and packed into shared GPU texture atlases.
- **Dynamic Arbitrary Paths:** Lines, Bezier curves, arbitrary polygons, and animated splines (e.g., node wires, charts, timeline curves) handled via GPU tessellation/path rendering with anti-aliasing.

## 6.3 Text Rendering Subsystem
- GPU glyph atlas supporting grayscale/subpixel text masks and colour emoji textures.
- Batched quad submission decoupled from `cosmic-text` shaping caches.

## 6.4 Compositor, Render Graph & Offscreen Effects
- Nested clipping and **rounded clipping** (stencil/scissor/shader clip stacks).
- Offscreen render targets for layer compositing, group opacity, and subtree caching.
- Multi-pass **Gaussian blur** and **backdrop blur** (sampling behind translucent surfaces without corrupting batch integrity).
- Alpha blending and tone/colour-space correctness across SDR/HDR displays.

## 6.5 Custom GPU Surfaces
- Widgets can embed **custom `wgpu` rendering surfaces** (e.g., video viewports, 3D previews, shader canvases, audio waveforms).
- Custom surfaces render into cadmium-managed offscreen texture targets sized to the widget's laid-out bounds, then composite through cadmium's render graph so rounded corners, overlays, popovers, and tooltips layer cleanly on top.

## 6.6 Multi-Window Resource Sharing & Format Strategy
- GPU state is split between a process-wide **`GpuContext`** (Device, Queue, compiled pipelines, glyph/icon/image caches) and per-window **`WindowContext`** (Surface, swapchain configuration).
- To support windows on monitors with different `TextureFormat`s cleanly, shared pipelines and atlases are either keyed per format or composited into a canonical internal format before final swapchain presentation.
- **Partial Updates:** Because modern swapchains rotate backbuffers, partial updates are achieved via **idle-frame skipping** and **offscreen retained layer caching** rather than assuming previous swapchain frame contents persist.

---

# 7. Layout Engine

cadmium implements its own layout abstraction backed internally by **`taffy`**.

## 7.1 Layout Models
- Horizontal (`Row`) and Vertical (`Column`) Flexbox-style layouts.
- Stack and Overlay layouts (`Z-stacking`, anchored overlays).
- Grid layouts.
- Absolute and anchored positioning.
- Scrollable viewports and nested layouts.
- Resizable split-panes and docking regions.

## 7.2 Layout Properties
Every applicable widget supports:
- Fixed, percentage, intrinsic, and flexible sizing (`width`, `height`, `min_*`, `max_*`, `flex_grow`, `flex_shrink`).
- Padding, margin/external spacing, and gap.
- Alignment, justification, and aspect ratio.
- Overflow behaviour (visible, clip, scroll).

## 7.3 Integration Responsibilities
cadmium's layout layer owns:
- **Leaf Measurement Bridge:** Connecting `taffy` measurement callbacks to `cosmic-text` (wrapped text bounds), images, and custom viewports without borrow conflicts.
- **Relayout Boundaries & Dirty Subtrees:** Avoiding full-window layout recalculation when localized content changes.
- **Viewport Virtualization Contract:** Supporting lists, trees, and tables with 100,000+ items by computing visible scroll windows and mounting only visible item nodes (plus overscan) into `taffy`.
- **DPI Pixel Snapping:** Aligning final computed bounds cleanly to the physical pixel grid per window scale factor.

---

# 8. Widget System

All widgets are composable, themeable, keyboard-accessible, and built on cadmium's retained/reactive core.

## 8.1 Tier 1 — Core Layout & Interactive Widgets (v1.0 MVP)
**Layout & Structure:**
- `Container`, `Row`, `Column`, `Stack`, `Overlay`, `Portal`, `Spacer`, `Divider`, `ScrollView`, `SplitPane`.

**Display:**
- `Text`, `Image`, `Icon`, `Badge`, `Card`, `ProgressBar`, `ProgressIndicator`.

**Interactive:**
- `Button`, `IconButton`, `Checkbox`, `RadioButton`, `Switch`, `Slider`, `TextInput` (single-line), `Dropdown` / `Select`, `Tabs`, `Tooltip`, `Popover`, `ContextMenu`, `Modal` / `Dialog`.

## 8.2 Tier 2 — Rich & Complex Workspace Widgets (v1.0 Full / v1.x)
**Dense Data & Workspace:**
- `VirtualizedList`, `Grid`, `TreeView`, `Table`, `DockArea` (multi-pane docking and detachable tab panels), `NavigationSidebar`, `Toolbar`, `MenuBar`, `CommandPalette`.

**Rich Text & Advanced Inputs:**
- `TextArea` (multiline editing), `RichText`, `SegmentedControl`, `DatePicker`, `ColorPicker`.

**Feedback & Media:**
- `Avatar`, `SkeletonLoader`, `Notification`, `Toast`, `CustomGpuSurface`.

---

# 9. Typography & Text Input System

Typography is central to cadmium's visual identity.

Requirements:
- Custom font loading (embedded or filesystem) and system font discovery.
- Font family selection, weights, styles, sizes, letter spacing, and line height.
- Text alignment, wrapping, truncation/ellipsis, and clipping.
- Rich text runs (mixed styles/colours within a single text block).
- Font fallback, full Unicode, colour emoji, and bidirectional (Bidi) text support.
- **Text Editing & IME:**
  - Single-line and multiline cursor navigation, selection, copy/cut/paste (`arboard`), and undo/redo stacks.
  - **IME (Input Method Editor) support:** Pre-edit composition strings, underlines, and OS candidate window positioning via `winit`.
  - **Platform-aware editing conventions:** Respecting OS keybindings (e.g., `Cmd` vs. `Ctrl` word/line jumps, macOS vs. Windows/Linux selection rules).
- Text shaping (`cosmic-text`) and glyph rasterization (`swash`) remain strictly decoupled from GPU submission.

---

# 10. Styling and Theming

## 10.1 Styling Properties
- Background colour, foreground colour, gradients, and opacity.
- Border colour, width, and per-corner border radius.
- Drop shadows and inner shadows.
- Backdrop blur and element blur.
- Padding, spacing, typography styles, and transition definitions.

## 10.2 Theme System
- Built-in **Light** and **Dark** themes.
- Global design tokens (colour palettes, spacing scales, radii, typography scales, elevation).
- Theme inheritance, overrides, and **runtime theme switching** without reconstructing the widget tree.
- Component-specific styling variants without modifying widget logic.

---

# 11. Animation System

cadmium includes a central, time-based animation engine integrated with the render loop:
- Animated properties: position, size, opacity, colour, transforms, blur radius, and border radius.
- Spring physics animations and standard easing curves.
- Sequencing, staggering, cancellation, and seamless reversal mid-flight.
- Strictly **delta-time based** (identical real-world speed across 60 Hz, 120 Hz, 144 Hz, and variable-refresh displays).
- Active animations drive scheduled redraws; once all animations settle, the render loop returns to idle.
- Respects OS reduced-motion accessibility preferences.

---

# 12. Input, Focus, and Event System

Unified cross-platform event pipeline:
- **Inputs:** Mouse move/click/scroll, keyboard input and shortcuts, touch/trackpad gestures (pinch/pan), IME events, and OS drag-and-drop (files and internal payloads).
- **Events:** Pointer enter/leave/hover, pointer down/up/click/double-click, drag start/move/drop, focus/blur, scroll, and keyboard key-down/up.
- **Propagation & Routing:** Hit-testing respecting z-order, transforms, scroll offsets, and rounded clipping; capture/bubble phases with event consumption.
- **Focus Management:** Tab order, focus scopes (e.g., trapping focus inside modals), programmatic focus, and visible focus rings.

---

# 13. Window Management (Multi-Window from Day One)

cadmium separates process-wide GPU/app state from per-window state from initial implementation:
- **`GpuContext` (Singleton):** Shared device, queue, pipelines, texture/glyph/SVG caches.
- **`WindowContext` (Per-Window):** `winit` window handle, `wgpu` surface, surface configuration, DPI scale factor, window root widget subtree, focus/IME state, and `AccessKit` adapter.

Capabilities:
- Creating, closing, resizing, moving, maximizing, minimizing, and fullscreening multiple independent windows.
- Detached toolbars, floating panels, and multi-window docking tear-off.
- Transparent windows and configurable decorations / custom client-side titlebars.
- Display/monitor enumeration and per-window DPI change handling.
- System clipboard (`arboard`) and native file dialogs (`rfd`) behind cadmium platform traits.

---

# 14. Image and Media Support

- Decoding PNG, JPEG, WebP (`image`), and SVG (`resvg`).
- Asynchronous off-thread decoding and mipmap/texture upload to shared GPU caches.
- Texture caching with automatic eviction/lifetime management.
- Scaling modes (fit, fill, stretch), cropping, rounded clipping, and opacity.
- Modular interface designed to accommodate future video/frame-stream textures for media applications.

---

# 15. Performance Requirements

Targets:
- Smooth 60 FPS baseline; native support for 120 FPS / 144 FPS displays.
- **Zero idle rendering:** 0% GPU redraws when no inputs, async updates, or animations are active.
- Minimal per-frame heap allocations during layout and paint passes (buffer/arena reuse).
- GPU draw-call batching and minimal pipeline state changes.
- O(visible) rendering and layout cost for virtualized lists/trees.
- Built-in profiling hooks (`tracing`), frame-time overlays, and draw-call diagnostics.
- Verified by automated Criterion/GPU benchmarks rather than assumptions.

---

# 16. Accessibility

Accessibility is built into the retained widget architecture from Phase 3 onward (not retrofitted late):
- Every retained widget node maintains a stable ID mapped to an **`AccessKit`** semantic node.
- Standard roles, labels, values, checked/expanded/disabled states, and bounds synced incrementally on updates.
- Full keyboard navigation and visible focus indicators.
- Text scaling and OS reduced-motion preference support.
- High-contrast theme readiness.

---

# 17. Developer Experience

- Ergonomic, type-safe Rust builder/declarative API with clean ownership and minimal boilerplate.
- Clear compiler errors; zero `wgpu`, `taffy`, or `winit` boilerplate required to build standard applications.
- Clean escape hatch for custom widgets, custom WGSL shaders, and embedded `CustomGpuSurface` viewports.
- Built-in visual inspector/debug overlay (layout bounds, relayout flashes, FPS/draw-call counters).

---

# 18. Architecture and Modularity

cadmium is organized into decoupled internal subsystems (maintained as logical modules first, split into workspace crates only as compile-time or dependency boundaries dictate):

- `core` — App lifecycle, `GpuContext`, async waker bridge, IDs, geometry primitives.
- `reactive` — Signals, derived state, subscriptions, 3-stage dirty queue.
- `renderer` — SDF instancing, vector/SVG pipeline, compositor/render graph, blur passes, custom surface host.
- `text` — `cosmic-text` / `swash` shaping, measurement cache, IME state, glyph atlas bridge.
- `layout` — `taffy` wrapper, leaf measurement bridge, relayout boundaries, virtualization contract, pixel snapping.
- `style` — Design tokens, themes, styles, transitions.
- `animation` — Timelines, springs, easings, motion scheduler.
- `events` — Hit-testing, event propagation, focus tree, shortcuts, drag-and-drop.
- `widgets` — Retained widget implementations (Tier 1 & Tier 2).
- `window` — `WindowContext`, multi-window manager, surface lifecycle, custom titlebar chrome.
- `assets` — Async image/SVG/font loading and GPU texture caching.
- `platform` — Abstractions over `arboard`, `rfd`, OS preferences.
- `accessibility` — `AccessKit` semantic tree synchronization.
- `debug` — Telemetry, layout/paint visualizers, performance overlays.

---

# 19. Testing and Reliability

- Unit tests for reactive dependency tracking, dirty-flag propagation, and cycle safety.
- Unit tests for layout integration, text measurement, and virtualization math.
- Unit tests for hit-testing, focus traversal, and event bubbling.
- Headless / offscreen `wgpu` visual regression tests comparing rendered frames against golden images across platforms.
- Multi-window, DPI-scaling, and swapchain resize/occlusion stress tests.
- Automated performance benchmarks (layout throughput, 10k-node paint batching, scroll latency).

---

# 20. Non-Goals

- Wrapping or reproducing native OS widgets (Win32, Cocoa, GTK).
- Depending on HTML, CSS, a browser engine, Electron, or a JavaScript runtime.
- Building a full 3D game engine or physics engine.
- Copying Apple's proprietary icons, trademarks, or exact visual assets.

---

# 21. Dogfood Validation Strategy

Long-term architectural benchmarks include applications on the scale of **Notion, VS Code, Discord, and DaVinci Resolve**.

To validate cadmium before v1.0 stabilization, development will drive a **multi-pane Workspace & Creative Inspector dogfood application** designed to stress-test all core bets simultaneously:
- **Multi-pane docking, splitters, and tabs** (including tearing off a panel into a second OS window).
- **A virtualized file/node tree** (10,000+ rows, icons, keyboard navigation, selection).
- **An inspector & document panel** (nested Taffy layouts, text editing, sliders, dropdowns, scrolling, async background updates).
- **An embedded `CustomGpuSurface` viewport** (live custom WGSL shader/canvas clipped inside rounded UI with floating popovers/context menus rendered on top).

---

# 22. Development Roadmap

## Phase 1 — Multi-Window GPU Foundation
- Module architecture and `GpuContext` (shared) vs. `WindowContext` (per-window) separation.
- `winit` multi-window event loop, swapchain lifecycle, DPI awareness, and idle redraw control.
- Basic `wgpu` render loop + initial text atlas validation.
- **Milestone:** Multiple independent windows rendering crisp GPU frames and text across DPI scales.

## Phase 2 — Hybrid Rendering Engine
- Instanced SDF pipeline (rounded rects, borders, circles, gradients, analytical shadows).
- Compositor & render graph (nested rounded clipping, offscreen layers, Gaussian/backdrop blur).
- Static SVG atlas (`resvg`) + dynamic vector path pipeline + image texture cache.
- `CustomGpuSurface` offscreen target integration.
- **Milestone:** Complete hybrid visual scenes rendered with batching, blur, shadows, SVGs, and an embedded custom shader viewport.

## Phase 3 — Reactive Core, Taffy Layout & Tier 1 Widgets
- Arena-based retained widget tree + fine-grained reactive signal runtime + 3-stage dirty pipeline + async waker bridge.
- `taffy` integration with text measurement hook, relayout boundaries, pixel snapping, and virtualization contract.
- Hit-testing, event propagation, focus traversal, IME hooks, and `AccessKit` node mapping from day one.
- Tier 1 widgets (`Container`, `Row`, `Column`, `Stack`, `ScrollView`, `Button`, `Text`, `TextInput`, `Slider`, `Dropdown`, `Tabs`, `Modal`, etc.).
- **Milestone:** Functional, accessible, reactive multi-pane app built without direct `wgpu` code.

## Phase 4 — Design System, Typography Polish & Motion
- Light/Dark themes, global design tokens, runtime theme switching.
- Rich text, `TextArea` multiline editing, platform shortcuts, clipboard (`arboard`), file dialogs (`rfd`).
- Time-based animation engine (springs, easings, transitions, reduced-motion support).
- **Milestone:** cadmium's signature visual identity, fluid motion, and complete editing experience.

## Phase 5 — Advanced Workspace Widgets & Dogfood Application
- Tier 2 widgets (`VirtualizedList`, `TreeView`, `Table`, `SplitPane`, `DockArea`, `CommandPalette`, `MenuBar`, `ColorPicker`, `Toast`).
- Drag-and-drop and detached multi-window panel tear-off.
- Build and iterate on the **Workspace & Creative Inspector dogfood app**.
- **Milestone:** Real-world architectural stress-test completed across Windows, Linux, and macOS.

## Phase 6 — Optimization, Visual Regression & v1.0 Release
- CPU/GPU profiling, memory/allocation audits, 120–144 FPS tuning.
- Cross-platform visual regression suite and automated benchmarks.
- Documentation, examples, and public API stabilization.
- **Milestone:** cadmium 1.0.

---

# 23. Acceptance Criteria

cadmium reaches v1.0 readiness when:
1. Complete multi-window desktop applications can be built without touching `wgpu`, `taffy`, or `winit` directly (except when opting into `CustomGpuSurface`).
2. Visual appearance and layout behaviour are consistent across Windows, Linux, and macOS, including fractional DPI scaling.
3. The 3-stage reactive pipeline prevents full-tree rebuilds/relayouts on localized state changes and achieves zero GPU redraws when idle.
4. Virtualized lists/trees smoothly handle 100,000+ items at target display refresh rates (60–144 FPS).
5. Typography supports Unicode, emoji, wrapping, selection, clipboard, and IME composition reliably.
6. Accessibility trees (`AccessKit`) and full keyboard navigation work across Tier 1 and Tier 2 widgets.
7. Custom GPU surfaces composite cleanly beneath overlays, popovers, and rounded clips.
8. The dogfood workspace application runs stably across all target platforms with automated benchmarks and visual regression tests passing.

---

**cadmium — A unified, reactive, GPU-native interface system for every desktop platform.**
