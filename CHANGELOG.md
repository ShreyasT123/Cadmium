# Changelog

All notable changes to Cadmium and the Magnum rendering engine will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Modular Subsystem Architecture:** Split `cadmium` into dedicated, fully documented submodules (`accessibility`, `animation`, `assets`, `core`, `debug`, `events`, `layout`, `platform`, `reactive`, `renderer`, `style`, `text`, `widgets`, `window`) with `prelude` and generated `rustdoc`.
- **Reactive Engine Upgrades:** Added `OwnedSignal<T>`, `create_memo`, `create_selector` (`O(1)` 10k-row selection down to `~110 µs`), and relayout boundary coalescing.
- **Tier 1 & Tier 2 Widget Suite:** Added `Card`, `Badge`, `Avatar`, `Link`, `Checkbox`, `RadioButton`, `SegmentedControl`, `Accordion`, `Modal`, `Image`, `Icon`, `ProgressBar`, `CircularProgressIndicator`, `SplitPane`, `VirtualizedList`, `TreeView`, `Table`, `DockArea`, `MenuBar`, `Toolbar`, `CommandPalette`, `ToastHost`, `GridView`, `Switcher` (with animated transitions), `Show`, and `ForEach`.
- **Live `AccessKit` Bridge & Dynamic Vector Paths:** Wired `accesskit_winit::Adapter` for screen readers and `lyon` stroke path rendering.
- **Custom GPU Surfaces & Beats 01 Showcase:** `CustomGpuSurface` / `CustomGpuPass` and redesigned `apps/dogfood` (`Cadmium Music`) with Frosted sidebar, Liquid Glass player capsule, and Beats 01 live waveform stage.
- **Automated Criterion Benchmarks (`benches/gui_bench.rs`):** Added `create_1k_rows`, `select_row_10k`, `update_every_10th_10k`, and `scroll_step_100k_virtual`.

### Planned (Before v1.0.0)
- **Responsiveness & Tiling (`Phase 5C`):** `Wrap` layout, `Container::clip()`, `Text` ellipsis truncation, `ViewportMetrics` / `Breakpoint`, `min_inner_size`, and double-click titlebar maximize.
- **IME, Focus Scopes & Multi-Window Tear-Off (`Phase 5D`):** `Ime::Preedit`/`Commit`, candidate cursor area, modal focus trapping, `WindowCommand::OpenWindow`, and `Cadmium Zedis`.
- **Subtree `DisplayList` Caching & Visual Regression CI (`Phase 6`).**
---

## [0.1.0] - 2026-11-23

### Added
- **Unified `Material` System in Magnum:** First-class enum supporting `Material::Classic`, `Material::Frosted`, and `Material::Liquid` across containers and interactive elements.
- **Hardware Scissor Clipping & `ScrollView`:** Viewport clipping via `wgpu::RenderPass::set_scissor_rect`, mouse-wheel scroll delta listener, momentum spring physics, and fading scrollbar indicators.
- **Desktop `TextInput` Engine:** Single-line text input with blinking caret pulse, keyboard cursor navigation, text selection highlighting, clipboard copy/paste (`arboard`), and placeholder rendering.
- **Core Interactive Components:** `Tabs`, `Dropdown`, and `TrafficLights`.
- **Magnum GPU Rendering Engine:**
  - Custom 2D rendering pipeline written from scratch on `wgpu` 30 and WGSL.
  - Multi-window architecture with a process-wide `GpuContext` (device, pipelines, caches) and per-window canonical `Rgba8UnormSrgb` render targets blitted to swapchains.
  - Instanced Analytical Signed Distance Field (SDF) pipeline (`sdf.wgsl`) supporting rounded rectangles, independent corner radii, linear gradients, straight-alpha border compositing, and Gaussian drop shadows with exterior cutout masking.
  - 9-tap Gaussian & Dual-Kawase multi-mip diffusion pipeline (`blur.wgsl`) downsampling through an offscreen pyramid (`1/2 → 1/4 → 1/2`).
  - 2-Texture Optical Liquid Glass shader (`glass.wgsl`) featuring monotonic convex bezel refraction, chromatic dispersion (prism RGB split), smooth organic liquid turbulence, and 45° opposing-corner diagonal specular rims.
  - Text pipeline (`text.wgsl`) decoupled via `cosmic-text` shaping and `swash` CPU rasterization into a 2048×2048 shelf-packed GPU glyph atlas.
  - Textured quad pipeline (`textured.wgsl`) supporting PNG, JPEG, and WebP via `image`, and on-demand SVG rasterization via `resvg`.
  - Interleaved `DrawBatch` planner (`Sdf`, `Image`, `Text`, `Glass`) preserving exact painter's z-order with minimal GPU state changes.
- **Core Runtime & Reactive Architecture:**
  - Generational arena-backed retained widget tree (`slotmap::SlotMap`) avoiding `Rc<RefCell<...>>` reference cycles.
  - Fine-grained reactive `Signal<T>` primitives with automatic dependency tracking, cycle-safe deferred flushing, and manual disposal.
  - Three-stage invalidation pipeline (`STATE` → `LAYOUT` → `PAINT`) with idle-frame skipping (0% CPU/GPU when settled).
  - Thread-safe `UiDispatcher` allowing background and asynchronous threads to dispatch state mutations directly onto the UI thread via `winit::event_loop::EventLoopProxy`.
  - Spring animation physics (`Spring`) for delta-time-based elastic dragging, snap-backs, and UI expansion/collapse.
- **Layout & Platform Integrations:**
  - Flexbox row, column, and container layout integration wrapping `taffy` 0.15 behind a clean abstraction.
  - Subpixel-free fractional DPI pixel-snapping (`snap_rect_to_pixels`).
  - Cross-platform clipboard read/write via `arboard`.
  - Native file and folder open/save dialogs via `rfd`.
  - Semantic accessibility tree builder (`accesskit` 0.25).
  - Built-in `FrameProfiler` recording frame time, layout latency, paint duration, and FPS telemetry.
- **Applications & Examples:**
  - `apps/dogfood` (Cadmium Studio): Multi-pane workspace testing declarative widget trees, live reactive signals, background async tasks, clipboard integration, and file dialogs.
  - `examples/liquid_glass`: Interactive Liquid Glass menu demonstrating optical bezel refraction, draggable spring physics, SVG icons, and a dynamic conic-bordered glass button.
