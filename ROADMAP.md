# SteelUI & Magnum Roadmap

**Current Version:** 0.1.0  
**Status:** Active Development (Spiral Model)  
**Primary Graphics API:** `wgpu` (WGSL)  
**Core Rendering Engine:** Magnum  

This document outlines the phased development roadmap for SteelUI and its custom rendering engine, Magnum.

---

## Design Tenets

1. **GPU-Native & Custom:** No Chromium, Electron, WebViews, Skia wrappers, or native widget abstractions. The Magnum pipeline owns all GPU draws, batching, and shaders.
2. **Three First-Class Visual Modes:**
   - **Classic UI:** Clean native desktop styling, solid surfaces, 1px hairlines, and soft elevation shadows.
   - **Frosted Glass:** Classic macOS translucent vibrancy via multi-pass Gaussian / Dual-Kawase blur.
   - **Liquid Glass:** Advanced optical lens refraction, chromatic dispersion (prism RGB split), and directional specular highlights.
3. **Predictable Rust Ownership:** Generational arena-backed retained widget trees (`slotmap`) and copyable `Signal<T>` handles. Zero borrow-checker fights for application developers.
4. **Performance by Default:** Zero CPU/GPU redraws when idle. Sub-millisecond layout passes via Taffy, batched draw calls, and physical pixel snapping.

---

## Milestones

### Phase 1: Engine Foundation & Magnum Core (Completed)
- [x] Multi-window runtime with shared process-wide `GpuContext`.
- [x] Canonical offscreen render target (`Rgba8UnormSrgb`) with swapchain presentation blit.
- [x] Instanced SDF primitive shader (`sdf.wgsl`) with anti-aliasing, rounded corners, linear gradients, and drop shadows.
- [x] Multi-mip blur pyramid (`blur.wgsl`) for translucent backdrops.
- [x] 2-texture Optical Liquid Glass shader (`glass.wgsl`) with convex bezel refraction and chromatic aberration.
- [x] Glyph atlas text shaping and rendering (`cosmic-text` + `swash`).
- [x] PNG/JPEG/WebP texture loading and SVG rasterization (`resvg`).
- [x] Time-based spring physics engine (`Spring`).

### Phase 2: Reactive Runtime & Widget Arena (Completed)
- [x] Generational `WidgetArena` based on `slotmap::SlotMap`.
- [x] Fine-grained reactive `Signal<T>` and `create_effect` dependency tracking.
- [x] Three-stage dirty pipeline (`STATE` → `LAYOUT` → `PAINT`).
- [x] Thread-safe `UiDispatcher` for async background workers.
- [x] Taffy 0.15 layout wrapper with leaf measurement bridge and fractional DPI pixel snapping.
- [x] Platform abstractions: Clipboard (`arboard`) and Native File Dialogs (`rfd`).
- [x] Accessibility tree builder bridge (`accesskit`).
- [x] Working dogfood workspace application (`apps/dogfood`).
- [x] Interactive Liquid Glass showcase (`examples/liquid_glass`).

---

### Phase 3: Core Desktop Controls & Input Subsystems (Completed)
- [x] **Unified `Material` System in Magnum:**
  - Consolidate surface styling into `Material::Classic`, `Material::Frosted`, and `Material::Liquid`.
  - Support setting material polymorphically on any container, card, or button.
- [x] **Hardware Scissor Clipping & `ScrollView`:**
  - Viewport scissor rect clipping (`wgpu::RenderPass::set_scissor_rect`).
  - Mouse wheel delta listener (`WindowEvent::MouseWheel`) with inertia spring damping.
  - Dynamic scroll boundary calculation and customizable scrollbar indicators.
- [x] **Desktop `TextInput` Engine:**
  - Single-line text input with blinking caret pulse (sleeping when idle).
  - Keyboard navigation: Left/Right arrows, Home/End, word jumps (`Ctrl`/`Cmd` + Arrow).
  - Text selection range and highlight box rendering.
  - Backspace, Delete, and clipboard copy/cut/paste integration (`Ctrl+C`, `Ctrl+X`, `Ctrl+V`).
  - Placeholder text and disabled states.
- [x] **Core Interactive Controls:** `Tabs`, `Dropdown`, and `TrafficLights`.
- [ ] **Floating Overlays & Portals:**
  - `Portal` layer rendering floating elements outside parent clipping hierarchies.
  - Context menu and tooltip system.

### Next (Planned)
- [ ] Live `AccessKit` adapter wiring: publish incremental updates, route semantic actions, and synchronize focus/bounds.
- [ ] Dynamic vector paths via `lyon`: animated Bezier paths, splines, charts, and node wires.
- [ ] `CustomGpuSurface`: embed custom `wgpu` render passes inside rounded, clipped layouts.
- [ ] IME and multiline `TextArea`: composition strings, candidate placement, wrapped editing, and undo/redo.

---

### Phase 4: Advanced Graphics, Motion & Typography Polish
- [ ] **Dynamic Vector Geometry:**
  - Wire dynamic arbitrary Bezier paths and spline curves (`lyon` tessellation into vertex buffers).
  - Path animation support for interactive graphs, timeline curves, and custom icons.
- [ ] **Custom GPU Surfaces (`CustomGpuSurface`):**
  - Dedicated widget allowing applications to embed raw custom `wgpu` render passes (e.g. 3D viewports, video frames, shader scopes) inside rounded/clipped layouts.
- [ ] **Typography & IME Expansion:**
  - Rich text formatting with mixed font weights, inline links, and color spans.
  - Input Method Editor (IME) support for CJK composition strings and OS candidate popup placement.
- [ ] **Multiline Text Editor (`TextArea`):**
  - Multiline text editing with line wrapping, vertical cursor movement, and undo/redo stacks.

---

### Phase 5: Workspace Widgets & Large-Scale Dogfood App
- [ ] **Workspace Layout Components:**
  - Resizable split panes (`SplitPane`) with drag handles.
  - Tab bars and dockable workspace panels (`DockArea`).
  - Multi-window panel tear-off (dragging an internal panel out into an independent native window).
- [ ] **Viewport Virtualization:**
  - `VirtualizedList` and `TreeView` capable of smoothly scrolling 100,000+ items at 120–144 FPS with O(visible) layout and memory cost.
- [ ] **Expand Dogfood Application:**
  - Build out a full-featured creative workstation / code & asset inspector stress-testing docking, tabs, virtualization, and custom shader viewports.

---

### Phase 6: Optimization, Visual Regression & 1.0 Stabilization
- [ ] **Profiling & Allocations Audit:**
  - Zero heap allocations during steady-state frame rendering.
  - GPU draw-call batching optimizations across complex multi-window workloads.
- [ ] **Automated Headless Visual Regression Suite:**
  - Offscreen headless `wgpu` frame rendering compared against golden reference PNGs across Windows, Linux, and macOS.
- [ ] **Public API Stabilization:**
  - Finalize crate boundaries and documentation.
  - Release SteelUI 1.0.
