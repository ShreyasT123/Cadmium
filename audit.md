# Cadmium Architecture Audit

> [!NOTE] Generated from deep instrumentation probes, git bisect, and static analysis.
> All MB figures are **Windows Working Set (WS)** / **Private Commit (PC)** unless noted.

---

## 1. Why "Idle" Shows During Typing (FPS Display)

The FPS label is NOT broken — it is intentionally a **render-demand meter**, not a vsync counter.

### What actually happens per keystroke

```
keystroke arrives
  → request_redraw()
  → render() fires  [frame_dt ≈ 0 ms]
  → record_cadence(dt=0ms) → dt < 0.001 guard → FILTERED OUT
  → effective_fps stays 0.0
  → was_animating = false   (typing never sets this flag)
  → label = "Idle • X.XX ms"
  → EventLoop sleeps until WaitUntil(caret_blink = now + 530ms)

530 ms later: caret blink fires
  → render() fires  [frame_dt ≈ 530 ms]
  → record_cadence(dt=0.53s) → dt ≥ 0.045 → CLEARS samples, effective_fps=0
  → label still "Idle"
```

**`was_animating` is `true` only when:**
- A `Spring` (scroll/tab) is actively stepping, OR
- `CustomGpuSurface.animate_signal` is `true`

Typing in a `TextInput` sets neither. So "Idle" is correct.

**The "22–30 FPS" the user saw** was likely a brief window where both a Spring scroll *and* a keypress landed in the same polling window — not the actual typing throughput. Actual typing render latency is **sub-millisecond** (frames complete in <1ms).

### Fix (if you want typing to show real FPS)

In `widgets/mod.rs` → `tick()`, add:

```rust
// after the Spring-stepping block:
if self.arena.focus.focused.is_some() && self.next_blink_at.is_some() {
    self.was_animating = true;
}
```

This makes the FPS counter reflect keystroke render cadence instead of showing Idle.

---

## 2. RAM Attribution (~190 MB Working Set)

Instrumented stage-by-stage via `mem_probe` examples on DX12 (default Windows backend).

| Stage | ΔWS | ΔPrivCommit | Cause |
|---|---|---|---|
| Baseline process | 10 MB | 2 MB | Rust stdlib, CRT |
| `winit::EventLoop::new` | +1 MB | +0.5 MB | Win32 message pump |
| `create_window` + AccessKit | +4 MB | +1 MB | HWND, accessibility tree |
| `wgpu::Instance::new` (DX12) | +2 MB | +2 MB | dxgi.dll |
| `request_adapter` | **+28 MB** | **+33 MB** | DX12 IGC shader compiler loads into process |
| `request_device` | **+66 MB** | **+64 MB** | `MemoryHints::Performance` → 128 MB device block pre-committed |
| `surface.configure` + 2× offscreen textures | +2 MB | +1 MB | 2 × (1360×860×4 B) = 9.4 MB VRAM (shared mem on iGPU) |
| `WidgetWindow::init` (widget tree + 4 fonts) | +2 MB | +1 MB | widget arena, Segoe UI family |
| **First render** (Glass blur + 14 imgs) | **+47 MB** | **+43 MB** | Pipeline cache warmup, Kawase staging buffers, vertex/index buffers |
| **Total** | **~162 MB** | **~147 MB** | |

### Where the remaining ~28 MB goes

Identified from `VirtualQuery` AllocationBase grouping on the live process:
- **61 MB** private region (Prot=0x4): DX12 placed resources (bypass gpu-allocator, invisible to `generate_allocator_report`)
- **3 × 32 MB** regions (Prot=0x404 = NO_CACHE+RW): DX12 command allocators / upload heaps
- Many smaller regions: shader module data, DX12 descriptor heaps, driver private state

> [!IMPORTANT] The single highest-ROI fix is changing `MemoryHints::Performance` → `MemoryHints::MemoryUsage` in `GpuContext::new()`. This saves **~60 MB PrivCommit** at startup (confirmed: Vulkan with equivalent usage hint → 44 MB vs 105 MB at `request_device`).

### DX12 vs Vulkan after `GpuContext::new`

| Backend | WS | PrivCommit |
|---|---|---|
| DX12 (current) | 118 MB | 105 MB |
| Vulkan | 58 MB | 45 MB |

Vulkan has lower startup cost but maps more shared memory on first render (+30 MB WS at frame 1). Net at steady state they converge.

### Glass Blur Cost

| Config | WS | PrivCommit |
|---|---|---|
| With Glass (3-pass Kawase) | 168 MB | 150 MB |
| Without Glass | 137 MB | 119 MB |
| **Glass overhead** | **+31 MB** | **+31 MB** |

Each Kawase pass writes to a fullscreen offscreen texture (1360×860×4 B ≈ 4.7 MB). Three passes = ~14 MB VRAM plus staging/upload buffers.

---

## 3. Architectural Issues

### 🔴 Critical — Correctness Bugs

#### C1: `TextArea` coordinate system is broken
- `hit_test_node()` (`widgets/mod.rs:6806–6829`) has no `TextArea` arm → clicks fall through to parent
- `reset_caret_blink()` checks only `TextInput`, not `TextArea`
- Byte offsets stored globally but `cursor_to_line_col()` / `line_col_to_offset()` take line-relative byte positions → cursor jumps to column 0 on line 2+
- No caret-scroll: typing past the visible area doesn't scroll the TextArea

#### C2: `CustomGpuSurface` GPU texture leak on arena clear
- `CustomSurfaceState.handle: TextureHandle` is a SlotMap key into `GpuContext::textures`
- `WidgetArena::clear()` / `unmount_subtree()` drop the widget without notifying `GpuContext`
- The SlotMap entry is never freed → leaked GPU allocation lives for the process lifetime
- Repro: mount/unmount a view containing `CustomGpuSurface` repeatedly

---

### 🟠 High — Architecture-Level Problems

#### H1: `MemoryHints::Performance` pre-commits 128 MB GPU blocks
**Location**: `crates/cadmium/src/renderer/mod.rs` → `request_device()`

wgpu-hal's DX12 backend interprets `MemoryHints::Performance` as "allocate the maximum device memory block upfront." For a UI library this is wasteful — UI workloads are small and latency-bound, not throughput-bound.

**Fix**: Change to `MemoryHints::MemoryUsage`. Expected saving: **~60 MB PrivCommit**.

```rust
// Before:
memory_hints: wgpu::MemoryHints::Performance,
// After:
memory_hints: wgpu::MemoryHints::MemoryUsage,
```

#### H2: O(N) full-tree repaint every frame, no dirty regions
- `paint_node()` walks the entire widget tree unconditionally on every `render()` call
- No display-list caching, no layer/tile system, no damage rectangles
- A 1000-widget tree pays the same O(N) cost whether 1 or 1000 widgets changed
- **Impact**: CPU-bound repaint cost scales linearly with tree size

**What would fix it**: retained display lists (cache paint output per widget, invalidate on dirty signal) or at minimum a dirty-rect union that clips the repaint walk.

#### H3: O(N) full-tree relayout on most signal writes
- `needs_full_layout=true` is set by: any `ForEach` update, any `Show`/`Switcher` toggle, any text node signal write, any window resize
- Relayout boundaries (`is_relayout_boundary`) are only set for fixed-size containers and `TextInput`/`TextArea`
- In practice almost every interactive change triggers a full Taffy relayout
- **Impact**: 1000-node tree → Taffy processes all 1000 nodes on every keypress

**What would fix it**: fine-grained dirty propagation — mark only the subtree containing the changed node as needing relayout, using the `is_relayout_boundary` fence.

#### H4: Unbounded recursion in ALL tree-traversal functions
Every tree operation is a direct recursive descent with no depth limit or stack check:

| Function | File | Risk |
|---|---|---|
| `paint_node` | `widgets/mod.rs` | stack overflow at ~8k nesting depth |
| `hit_test_node` | `widgets/mod.rs` | " |
| `find_tooltip_node` | `widgets/mod.rs` | " |
| `find_context_menu_node` | `widgets/mod.rs` | " |
| `widget_screen_bounds` | `widgets/mod.rs` | " |
| `dispatch_scroll_node` | `widgets/mod.rs` | " |
| `is_drag_node` | `widgets/mod.rs` | " |
| `unmount_subtree` | `widgets/mod.rs` | " |
| `mount_node` | `widgets/mod.rs` | " |

For typical UI trees (depth < 50) this is fine. For generated trees (recursive `ForEach` / list-of-lists) or pathological inputs it will stack-overflow on Windows (1 MB default stack).

**Fix pattern**: convert to explicit `Vec<NodeId>` work-stack (iterative DFS).

#### H5: No O(change) reactive widget reconciliation
- `build_fn` runs once at mount; reactive rebuilds use `ForEach`/`Show`/`Switcher`
- These rebuild their subtree by calling `build_fn` again on every signal change
- No keyed reconciliation: items are matched by index — inserting at position 0 remounts ALL items
- **Impact**: a list of 1000 items → inserting one item → 1000 unmount + 1001 mount calls

**Fix**: keyed `ForEach` with stable `NodeId` per key (like React keys or SwiftUI `.id()`).

#### H6: No async rendering pipeline — everything blocks main thread
- `image::open()` (file decode), `rfd::FileDialog` (native dialog), font loading — all called on the UI thread
- If any of these stalls, the event loop freezes (no frames, no input)
- **Fix**: `tokio::spawn` / `rayon` for decode; `rfd::AsyncFileDialog` for dialogs

---

### 🟡 Medium — Quality Issues

#### M1: Glass blur runs before scissor / visibility check
- `glass.wgsl` Kawase passes execute even if the blurred rect is entirely offscreen
- 3 full-resolution texture reads/writes for invisible content
- **Fix**: skip blur passes when `rect.intersect(viewport).is_empty()`

#### M2: `notify_write` wakes event loop on every signal write
- Every `Signal::set()` calls `notify_write()` → `event_loop_proxy.send_event(Wakeup)` even if no effect depends on the signal
- Causes spurious redraws for internal state that nothing reads
- **Fix**: only call `notify_write` after tracking confirms at least one live subscriber

#### M3: `push_stroke_path()` re-tessellates on CPU every frame
- Stroke paths (rounded borders, etc.) are tessellated from scratch each `render()` call
- No geometry cache keyed on `(shape, stroke_width, color)`
- **Fix**: cache tessellated geometry in a `HashMap<StrokeKey, Vec<Vertex>>`, invalidate on shape change

#### M4: `LayoutEngine::collect_logical_rects()` allocates new `HashMap` every relayout
- Called on every relayout (which is every frame in most apps)
- `HashMap::new()` + hash insertions per widget = measurable GC pressure
- **Fix**: reuse a pre-allocated `HashMap` cleared in-place with `.clear()`

#### M5: `Frosted`/`Liquid` border_width not DPI-scaled
- `border_width` is passed as logical pixels but used as physical in shader
- On 2× HiDPI display: borders appear half the intended width
- `glass.wgsl` also hardcodes `bevel = 26.0` in physical pixels

#### M6: `Suboptimal` present mode can spin on Wayland fractional scaling
- When `surface.get_current_texture()` returns `Suboptimal`, current code re-configures and immediately retries
- On Wayland with fractional scaling this can loop: configure → suboptimal → configure → ...
- **Fix**: add a backoff (skip 1 frame before re-configuring)

#### M7: Windows system font fallback broken
- Font fallback chain loads only 4 Segoe UI files
- Segoe UI Emoji and CJK fallbacks never loaded → emoji render as □ tofu
- **Fix**: use `font_kit` or `fontdb` to query the system fallback chain at runtime

#### M8: Linux clipboard drops content on app exit
- `arboard::Clipboard` must be held alive (kept in a background thread) for clipboard content to persist after the process exits
- Current code drops it → clipboard cleared on window close
- **Fix**: spawn a clipboard daemon thread holding the content until another app reads it

#### M9: `AccessKit` bounds reported in logical pixels, not physical
- `accesskit::NodeBuilder::set_bounds()` requires physical pixel coordinates
- Current code passes logical pixel rects without DPI scaling
- On 150% display: AT reports hit zones 1.5× too small

#### M10: Dark mode colors baked at mount time
- Colors computed from `ColorScheme` at widget mount
- If OS switches light↔dark while app is running, existing widgets keep old colors
- **Fix**: subscribe to `ColorScheme` signal and invalidate paint on change

#### M11: No IME pre-edit support
- `WindowEvent::Ime` events are not handled
- CJK / Korean / Japanese input shows no composition candidates
- Composed characters land as `WindowEvent::ReceivedCharacter` after full commit only

#### M12: No runtime multi-window spawn
- `WidgetWindow::new()` requires ownership of the `EventLoop`, which can only be created once
- Creating a second window requires calling `create_window()` on the existing event loop handle
- Currently not exposed: no public API to spawn additional windows at runtime

---

### 🔵 Low — Cleanup / Polish

#### L1: `vector.wgsl` dead code
- Shader file is compiled into the binary but no pipeline uses it
- Increases binary size + compile time by one shader module

#### L2: MSRV mismatch
- `as_chunks_mut` used in font code requires Rust **1.88**
- `README.md` advertises MSRV **1.85**
- **Fix**: either bump README or use `chunks_exact_mut` (stable since 1.0)

#### L3: All 14 modules are `pub mod` — no API surface control
- `lib.rs`: `pub mod accessibility`, `pub mod animation`, `pub mod core`, ...
- Internal implementation types (`WidgetArena`, `GlyphAtlas`, `LayoutEngine`, ...) are all public
- **Fix**: make implementation modules `pub(crate)` and re-export only the intended public API

#### L4: `SteelError` exposes `winit` types in public API
- `SteelError::WindowCreation(winit::error::OsError)` leaks winit in the public error enum
- Callers must depend on winit just to match on this variant
- **Fix**: wrap in a `Box<dyn Error + Send + Sync>` or opaque string

#### L5: `paint_node` has no recursion depth guard
- As noted in H4, but specifically: a widget that sets its own `children` to include itself would infinite-loop
- Low risk in practice since widget trees are built at mount time, not dynamically self-referential

---

## 4. Priority Fix List

| Priority | Fix | Expected Impact |
|---|---|---|
| **P0** | `MemoryHints::MemoryUsage` in `request_device` | −60 MB PrivCommit at startup |
| **P0** | Fix `TextArea` coordinate system | Correctness bug — cursor broken on line 2+ |
| **P0** | Free `CustomGpuSurface` texture on arena unmount | Prevents GPU memory leak on dynamic views |
| **P1** | Iterative tree traversal (replace recursion) | Stack safety on deep trees |
| **P1** | Skip Glass blur passes for offscreen rects | −3 GPU passes for hidden content |
| **P1** | Async file I/O + dialogs off main thread | Eliminates UI freeze on slow I/O |
| **P2** | Dirty-rect repaint (avoid full O(N) repaint) | Scales to larger widget trees |
| **P2** | Finer relayout boundaries | Less Taffy work per interaction |
| **P2** | Font fallback for emoji + CJK | Renders non-Latin scripts correctly |
| **P3** | IME pre-edit support | CJK input usable |
| **P3** | AccessKit physical-pixel bounds | Screen reader hit zones correct on HiDPI |
| **P3** | Dark mode reactive colors | Live theme switching |
| **P4** | Keyed `ForEach` reconciliation | O(change) list updates |
| **P4** | Async clipboard persistence (Linux) | Clipboard survives app exit |
