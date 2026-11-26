# Cadmium & Magnum Roadmap to v1.0.0

**Current Version:** 0.1.0  
**Status:** Active Development — Phase 5C (Responsiveness, Tiling & Public API Polish)  
**Graphics Backend:** `wgpu` 30 (Magnum Engine)  
**Validation Apps:**
1. **Cadmium Music** (`apps/dogfood`) — Apple Music–inspired showcase for Frosted Glass, Liquid Glass optical refraction, `CustomGpuSurface` Beats 01 stage, and responsive desktop ergonomics.
2. **Cadmium Zedis** (`apps/zedis`) — Recreation of `vicanso/zedis` (GPUI Redis GUI) stress-testing `DockArea`, `TreeView`, `Table`, `CommandPalette`, and O(change) benchmarks.

---

## Completed Foundations

### Phase 1–3.5: Magnum GPU Engine, Reactive Core & Hardening (Completed)
- [x] Multi-window `GpuContext` + per-window `WindowRenderTarget` + lazy `GlassOffscreen` blur pyramid.
- [x] Instanced SDF pipeline (`sdf.wgsl`), 9-tap Dual-Kawase blur (`blur.wgsl`), 2-texture Liquid Glass (`glass.wgsl`), text atlas (`text.wgsl`), textured quads (`textured.wgsl`), `lyon` paths (`path.wgsl`), and `CustomGpuSurface`.
- [x] Generational `WidgetArena` (`slotmap`), `Signal<T>`, `OwnedSignal<T>`, `create_memo`, `create_selector`, `UiDispatcher`, `ForEach`, `Show`, `Switcher` (animated slide+fade).
- [x] Taffy 0.15 layout wrapper with relayout boundaries (`> 32` coalescing), physical pixel snapping, and `100k+` `VirtualizedList` (`select_row_10k` ~`110 µs`, `scroll_step_100k` ~`14 µs`).
- [x] Live `accesskit_winit::Adapter`, `arboard` persistent clipboard, `rfd` file dialogs, and full `rustdoc` module modularization.

### Phase 4 & 5A–5B: Tier 1 & Tier 2 Widget Suite (Completed)
- [x] Core & Display: `Container`, `Row`, `Column`, `Stack`, `Card`, `Badge`, `Avatar`, `Link`, `Divider`, `Spacer`, `Image`, `Icon`, `Text`, `ProgressBar`, `CircularProgressIndicator`.
- [x] Interactive & Forms: `Button` (`Primary`/`Secondary`/`Ghost`/`Glass`/pill), `Switch`, `Slider`, `Checkbox`, `RadioButton`, `SegmentedControl`, `Tabs`, `Dropdown`, `Accordion`, `TextInput`, `TextArea`.
- [x] Workspace & Overlays: `ScrollView`, `SplitPane`, `VirtualizedList`, `TreeView`, `Table`, `DockArea`, `MenuBar`, `Toolbar`, `CommandPalette`, `Modal`, `ToastHost`, `GridView`, `TrafficLights`.
- [x] **Cadmium Music** (`apps/dogfood`) with Light/Dark themes, collapsible `Pins`/`Playlists` sidebar dropdowns, Liquid Glass capsule, and Beats 01 Live Stage.

---

## Active & Upcoming Phases to v1.0.0

### Phase 5C: Responsiveness, Window Tiling & Public API Polish *(Current)*
- [ ] **Window Tiling & OS Snap Compliance:**
  - `WindowSpec::min_width` / `min_height` (`with_min_inner_size`) so tiled/resized windows never crush below usable bounds.
  - Double-click on `drag_region` toggles OS Maximize / Restore.
  - Windows borderless shadow (`with_undecorated_shadow(true)`) + crisp resize handling during rapid Aero Snap / Wayland tiling bursts.
  - Reactive `ViewportMetrics` (`width`, `height`, `scale`, `breakpoint`) exposed by `WidgetWindow`.
- [ ] **Responsive Layout & Overflow Primitives:**
  - `LayoutStyle::wrap` (`FlexWrap::Wrap`) + **`Wrap`** flow layout widget.
  - `Container::clip()` (`overflow: Clip` scissor clipping for cards/pills/text rows).
  - `Text::max_lines(n)` & `.ellipsis()` truncation (`...`) so narrow tiled layouts never break column grids.
  - **`GridView::responsive(min_cell_width)`** & **`Breakpoint`** (`Compact`, `Medium`, `Expanded`) helper.
- [ ] **User-Facing API Ergonomics (`dx`):**
  - `impl IntoElement for &str` / `String` (`.child("Hello")`).
  - Short layout builders (`.w()`, `.h()`, `.w_full()`, `.h_full()`, `.p()`, `.px()`, `.py()`, `.fill_when(...)`).
  - High-level **`ShaderCanvas`** wrapper (8-line custom WGSL fragment widgets).
  - **`cadmium::run(...)`** single-window entry helper (minimal boilerplate for simple apps).
  - Reactive selection inside `TreeView`, `Table`, and `DockGroup` using `fill_when` / `create_memo`.
- [ ] **Responsive Cadmium Music (`apps/dogfood`):**
  - Auto-collapsing sidebar (`224px` → `64px` icon rail or drawer) below `760px`.
  - Reflowing `Best New Songs` (`3 → 2 → 1` cols), `Hero` cards, and `player_capsule` margins/controls so half-screen and quarter-screen tiling look native.

### Phase 5D: Dynamic Multi-Window, IME & `Cadmium Zedis`
- [ ] **Runtime Multi-Window (`WindowCommand::OpenWindow`):** Detached panels and `DockArea` tab tear-off.
- [ ] **IME & Input Completeness:** `set_ime_allowed`, `set_ime_cursor_area`, `Ime::Preedit` inline underline, `Ime::Commit`, focus-trapping scopes for `Modal`/`CommandPalette`, `TextArea` undo/redo, and OS `DroppedFile`.
- [ ] **Remaining Tier 2 Controls:** `RangeSlider`, `Combobox`, `Breadcrumbs`, `SkeletonLoader`.
- [ ] **Build `Cadmium Zedis` (`apps/zedis`):** Redis workstation benchmark app (`vicanso/zedis` parity) + side-by-side GPUI comparison.

### Phase 6: Subtree Paint Caching, Visual Regression & v1.0.0 Release
- [ ] **Subtree `DisplayList` Caching:** Cache recorded draw slices for `dirty_paint == false` subtrees for full `O(change)` render scaling.
- [ ] **`wgpu` Device-Lost Recovery & Headless Golden-Image CI:** Automated offscreen `wgpu` perceptual diff tests on Windows, Linux, and macOS.
- [ ] **Public API Lockdown (`v1.0.0`):**
  - Hide internal crates (`taffy`, `winit`, `cosmic-text`, `accesskit`, `arboard`, `rfd`, `resvg`) behind private `mod` boundaries; rename `SteelError` → `CadmiumError` cleanly everywhere.
  - Complete `LICENSE-APACHE` full text, reconcile `README.md`/`CHANGELOG.md`, verify `BENCHMARKS.md` SLAs, and tag `v1.0.0`.
