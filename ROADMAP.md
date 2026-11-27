# Cadmium & Xune Roadmap to v1.0.0

**Current Version:** 0.1.0 (preparing `0.2.0` preview)  
**Target:** v1.0.0 Industry-Ready Release  
**Graphics Backend:** `wgpu` 30 (Xune Engine)  
**Primary Showcase & Dogfood App:** **Cadmium Music** (`apps/dogfood`)

---

## Completed Milestones

### Phase 1–3.5: Xune Engine, Reactive Core & Hardening (Completed)
- [x] Multi-window `GpuContext` + `WindowRenderTarget` + lazy `GlassOffscreen` blur pyramid with dirty-region blur skipping.
- [x] Instanced SDF (`sdf.wgsl`), 9-tap Dual-Kawase blur (`blur.wgsl`), 2-texture Liquid Glass (`glass.wgsl`), shelf glyph atlas (`text.wgsl`), textured quads (`textured.wgsl`), `lyon` paths (`path.wgsl`), and `CustomGpuSurface` (`animate_when` + `redraw_on`).
- [x] Generational `WidgetArena` (`slotmap`), `Signal<T>`, `OwnedSignal<T>`, `create_memo`, `create_selector`, `ForEach`, `Show`, and animated `Switcher` with `DrawList` opacity stack.
- [x] Taffy 0.15 layout wrapper with relayout boundaries (`> 32` coalescing), physical pixel snapping, and `100k+` `VirtualizedList` (`select_row_10k` `~110 µs`, `scroll_step_100k` `~14 µs`).
- [x] Live `accesskit_winit::Adapter`, persistent `arboard` clipboard, `rfd` file dialogs, Criterion benchmarks (`gui_bench.rs`), and full `rustdoc` modularization.

### Phase 4 & 5A–5B: Tier 1 & Tier 2 Widget Suite + Cadmium Music (Completed)
- [x] Core & Display: `Container`, `Row`, `Column`, `Stack`, `Card`, `Badge`, `Avatar`, `Link`, `Divider`, `Spacer`, `Image`, `Icon`, `Text`, `ProgressBar`, `CircularProgressIndicator`.
- [x] Interactive & Workspace: `Button`, `Switch`, `Slider`, `Checkbox`, `RadioButton`, `SegmentedControl`, `Tabs`, `Dropdown`, `Accordion`, `Modal`, `TextInput`, `TextArea`, `ScrollView`, `SplitPane`, `VirtualizedList` (`.columns_mode()` + `.row_art()`), `TreeView`, `Table`, `DockArea`, `MenuBar`, `Toolbar`, `CommandPalette`, `ToastHost`, `GridView`, `Wrap`, `ShaderCanvas`, `TrafficLights`.
- [x] **Cadmium Music** (`apps/dogfood`): Frosted sidebar with collapsible dropdowns, Liquid Glass player capsule, 100k-track table, real PNG album-art color bleed, Beats 01 equalizer stage, Light/Dark switching, and responsive `Breakpoint` layout.

---

## Remaining Phases to v1.0.0

### Phase 5C: Responsiveness, Tiling Polish & Public API Cleanup
- [ ] Complete `Wrap` / responsive testing across half-screen Aero Snap (`640×800`), macOS Tile, and Linux tiling WMs.
- [ ] Virtualize `TreeView` and `Table` row pools (`O(visible)`) to match `VirtualizedList` 100k+ scaling.
- [ ] Add `cadmium::run(...)` convenience entrypoint and polish `dx` builders.

### Phase 5D: IME, Focus Scopes & Runtime Multi-Window
- [ ] **IME Support:** `WindowEvent::Ime` (`Preedit` inline underline + `Commit`), `set_ime_allowed`, and `set_ime_cursor_area`.
- [ ] **Input & Focus:** Modal/menu focus-trapping scopes, `TextArea` undo/redo stack, double-click word selection, and OS `DroppedFile`.
- [ ] **Runtime Multi-Window:** `WindowCommand::OpenWindow` for secondary windows and `DockArea` panel tear-off.
- [ ] **Windows Emoji Optimization:** Lazy emoji fallback without `cosmic-text` 3× ASCII shaping overhead.
- [ ] *(Optional / Deferred)* Additional workstation showcase apps (`zedis`-style inspector).

### Phase 6: Subtree Caching, Visual Regression CI & v1.0.0 Release
- [ ] **Subtree `DisplayList` Caching:** Cache recorded draw slices for clean subtrees (`O(change)` render).
- [ ] **GPU Resilience & Visual Regression:** `wgpu` device-lost recovery + headless golden-image diff tests on Windows, Linux, and macOS in CI.
- [ ] **Public API Lockdown (`v1.0.0`):** Restrict internal subsystems in `lib.rs` from `pub mod` to `mod`, finalize `CadmiumError` & `WindowDelegate` isolation, include full `LICENSE-APACHE` text, verify `BENCHMARKS.md` SLAs, and release `v1.0.0`.
