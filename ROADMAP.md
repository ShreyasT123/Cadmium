# Cadmium & Magnum Roadmap to v1.0.0

**Current Version:** 0.1.0  
**Target:** v1.0.0 Industry-Ready Release  
**Graphics Backend:** `wgpu` 30 (Magnum Engine)  
**Primary Validation Apps:**
1. **Cadmium Music** (`apps/dogfood`) — Apple Music–inspired desktop player showcasing **Classic**, **Frosted Glass**, and **Liquid Glass** optical shaders, live audio/visualizer `CustomGpuSurface`, virtualized track libraries, and fluid spring motion.
2. **Cadmium Zedis** (`apps/zedis` / benchmark suite) — Recreation of [`vicanso/zedis`](https://github.com/vicanso/zedis) (GPUI-based Redis GUI) to directly stress-test and benchmark Cadmium against GPUI, Iced, Slint, and Dioxus Native on real workstation workloads.

---

## Architectural Competitive Edge

Modern Rust GUI benchmarks highlight two critical bottlenecks in existing frameworks:
- **Immediate & view-rebuilding models** (`egui`, `Iced` diffing, `GPUI` view notify) scale with **tree size** on localized edits unless heavily manual-memoized.
- **Cadmium's bet** is **Generational Retained Arena (`slotmap`) + Run-Once Fine-Grained Signals (`O(change)`) + Relayout Boundaries + Subtree Display-List Caching + Batched Instanced SDF/Glass**.

---

## v1.0.0 Industry Performance SLAs (Non-Negotiable Exit Gates)

Measured via automated Criterion + headless/GPU harnesses (see `BENCHMARKS.md`):

| Metric | Target (v1.0.0) | Competitive Reference |
|---|---|---|
| **Idle CPU / GPU usage** | `0.0%` CPU, `0` GPU submits | Matches/beats Iced & Slint |
| **Localized edit (10k-node tree)** | `< 0.25 ms` CPU (O(change)) | Faster than GPUI (~10ms) & Slint (~5ms) |
| **Row selection (1k / 10k list)** | `< 0.15 ms` | Faster than GPUI & Slint; parity w/ Iced |
| **100,000-row virtualized scroll** | `< 1.0 ms` CPU frame, locked 120–144 FPS | Smooth parity with GPUI Zedis |
| **Frame pacing (p95 / p99, complex UI)** | `< 4.0 ms` / `< 7.5 ms` @ 120Hz | Beats Tauri (~28/45ms) & GPUI (~12/18ms) |
| **Steady-state heap allocations** | `0` per idle frame; near-zero in steady scroll | Zero GC pauses |
| **Resident RAM (Music / Zedis app)** | `< 90 MB` RSS baseline (no large media) | < 25% of Electron, < 50% of Tauri |
| **Cross-platform parity** | Identical layout, typography & glass on Win/macOS/Linux (100%–250% DPI) | Automated golden-image diff CI |

---

## Milestones & Phased Plan

### Phase 3.5 — Engineering Hardening & Foundation Fixes *(Immediate)*
Fix all Critical/High defects from the 2026-11-25 audit before expanding surface area:
- [ ] **TextArea & TextEngine Fixes:**
  - Wire `TextArea` into `hit_test_node()`, `reset_caret_blink()`, and vertical `on_scroll` / caret-follow.
  - Fix line-relative vs. global byte offset conversion and soft-wrap line mapping in `TextArea`.
  - Enable `db.load_system_fonts()` on Windows for color emoji and CJK fallback.
- [ ] **Renderer & Lifecycle Fixes:**
  - Release `CustomGpuSurface` textures on `WidgetArena::clear()`; add generational `TextureHandle`.
  - Prevent `GlyphAtlas` infinite redraw loop at `MAX_ATLAS_SIZE` (multi-page or bounded reset).
  - Cull `DrawBatch::Glass` blur passes when occluded by `active_scissor`.
  - Uniformly DPI-scale `border_width` and pass `scale` in `GlobalsUniform` to `glass.wgsl`.
  - Delete dead `vector.wgsl`; allow Vulkan fallback on Windows (`PRIMARY`).
- [ ] **Reactivity, Motion, Platform & A11y Fixes:**
  - Remove duplicate `wake_cb` from `ReactiveRuntime::notify_write()`.
  - Lower `Spring::step()` thresholds to `0.001` / `0.005` for smooth `[0..1]` animations.
  - Persist `arboard::Clipboard` instance across calls (fixes X11/Wayland clipboard loss).
  - Scale `AccessKit` bounds to physical pixels (`bounds.scale(scale)`).
  - Make widget colors theme-reactive at paint time so `ThemeMode::Dark` works everywhere.
  - Green CI (`cargo fmt --all`, `clippy -D warnings`, resolve 1.85 vs 1.88 MSRV).

---

### Phase 4 — Visual Primitives, Typography & Cadmium Music (Dogfood v1)
**Goal:** Replace the placeholder dogfood app with **Cadmium Music**—an Apple Music–inspired desktop app built for LinkedIn/Medium showcases and daily dogfooding.

#### Core Library Deliverables
- [ ] **Media & Graphics Primitives:**
  - `Image` widget (async off-thread decode + LRU GPU texture cache, `Cover`/`Contain` scaling, rounded corners).
  - `Icon` widget (`resvg` DPI-aware SVG atlas with LRU eviction + tinting).
  - `Stack` / `Overlay` / `Portal` containers + `Absolute` positioning (`LayoutStyle::position`, `inset`).
  - Analytical rounded shader clip stack (for rounded `ScrollView` / album cards / `CustomGpuSurface`).
  - `FillTessellator` + path geometry cache + edge AA for `lyon` dynamic paths.
  - Premultiplied-alpha flag on `ImageQuad` for translucent `CustomGpuSurface` compositing.
- [ ] **Tier 1 Controls Completion:**
  - `ProgressBar` / `ScrubberBar` (interactive seek bar with hover thumb & time tooltip) & `ProgressIndicator` (indeterminate spinner / circular ring).
  - `Checkbox`, `RadioButton`, `Badge`, `Card`, `Link`, `Accordion` / `Disclosure`, `NumberInput` / `Stepper`, `Popover`, `Modal` / `Dialog`.
  - `Button` / `IconButton` variants (Ghost, Glass Pill, Accent, Secondary) + icon-leading buttons.
  - `ScrollView` horizontal support + inner-first scroll capture + parent-offset hit-testing.
  - Standalone `Tooltip` (rich content) & `ContextMenu` widgets (beyond node-level attributes).
- [ ] **Typography, IME & Input Polish:**
  - `Text` alignment (`Left`/`Center`/`Right`), ellipsis truncation, font family/italic/letter-spacing, embedded font loading (`load_font_bytes`), and rich-text spans.
  - Full IME support (`set_ime_allowed`, `set_ime_cursor_area`, `Ime::Preedit` underline, `Ime::Commit`).
  - `TextArea` undo/redo stack; double-click word select; `FocusManager` focus scopes (trap `Tab` in modals/menus).
  - `WindowEvent::Focused(false)` cleanup; OS `prefers_reduced_motion` & system theme detection.
- [ ] **Animation & Theme Tokens:**
  - Easing curves (`Tween`) + staggered transitions + `create_memo` derived signals.
  - Complete `MacTheme` tokens (radii, spacing, typography scales) + `serde` serialization.

#### Showcase App 1: **Cadmium Music** (`apps/dogfood`)
- [ ] **Translucent Frosted Sidebar:** Playlists, Listen Now, Browse, Radio, Library sections with hover states and active pill indicators.
- [ ] **Liquid Glass Floating Now-Playing Bar:** Persistent bottom/top floating `Material::Liquid` pill with album thumbnail, track title/artist, play/pause/next/prev controls, optical refraction & specular edge highlights scrolling over vibrant album art.
- [ ] **Hero Album / Listen Now View:** Cover artwork cards with dynamic ambient color glow, `ScrollView` with rounded clipping, and `CustomGpuSurface` live audio-spectrum / shader visualizer.
- [ ] **Virtualized 100k-Track Library Table:** Instant filtering via `TextInput`, smooth 120–144 FPS scrolling, column headers, row hover/selection, right-click `ContextMenu`, and Light/Dark runtime theme toggle.

---

### Phase 5 — Performance Architecture & Cadmium Zedis (Workstation Stress-Test)
**Goal:** Achieve **O(change) UI updates**, full workspace widgets, and validate parity/superiority against GPUI via **Cadmium Zedis** (`vicanso/zedis` recreation).

#### Core Library Deliverables
- [ ] **O(change) Scaling Engine (Beats GPUI on Large Trees):**
  - Relayout boundaries in `LayoutEngine` (`compute_subtree`) + zero-alloc rect sync into `WidgetArena`.
  - Subtree `DisplayList` slice caching for `dirty_paint == false` subtrees.
  - Keyed reactive control flow: `Show` (conditional) and `ForEach` (dynamic keyed list reconciliation without remounting).
  - `LayoutStyle`: `Grid`, `max_width`/`max_height`, `margin`, `aspect_ratio`, `overflow`.
- [ ] **Multi-Window & Device Resilience:**
  - Dynamic runtime window creation (`WindowCommand::OpenWindow`) + `DockArea` panel tear-off.
  - `wgpu` device-lost recovery (`GpuContext` rebuild) + Wayland `Suboptimal` guard.
- [ ] **Tier 2 Workspace & Rich Input Widgets:**
  - `Grid` (2D CSS-Grid layout container backed by `taffy`), `NavigationSidebar`, `RichText` (mixed-style/color inline spans).
  - `TreeView` (virtualized expandable key/file tree with indent guides & keyboard nav).
  - `Table` (virtualized multi-column resizable/sortable grid).
  - `DockArea` (multi-pane docking, tab groups, tear-off windows).
  - `CommandPalette` (`Cmd/Ctrl+K` fuzzy filter overlay), `MenuBar`, `Toolbar`, `Toast` / `Notification`, `SegmentedControl`, `SkeletonLoader`, `Avatar`, `DatePicker`, `ColorPicker`.
- [ ] **Full `AccessKit` Coverage:**
  - All Tier 1 & Tier 2 roles, `disabled`/`expanded`/`selected` states, `SetValue`/`Increment`/`Decrement`, and incremental `TreeUpdate` deltas.

#### Showcase App 2: **Cadmium Zedis** (`apps/zedis`)
- [ ] Recreate [`vicanso/zedis`](https://github.com/vicanso/zedis) core workstation UI on Cadmium:
  - Server/Cluster Connection Sidebar & Tabbed Sessions.
  - Virtualized Redis Key Tree & Keyspace Browser (100,000+ keys, prefix grouping, instant search).
  - Multi-format Value Inspector (String/JSON syntax-colored `TextArea`, Hash/List/Set/ZSet virtualized `Table`, TTL/Memory metrics).
  - `Cmd/Ctrl+K` `CommandPalette`, `SplitPane` layouts, live telemetry sparklines via `lyon` paths, and Light/Dark themes.
- [ ] Connect optional live Redis/mock benchmark driver to measure identical UI workloads side-by-side against GPUI Zedis.

---

### Phase 6 — Benchmarks, Visual Regression & `v1.0.0` Stabilization
- [ ] **Automated Benchmark Suite (`benches/`):**
  - Criterion + GPU harness implementing `BENCHMARKS.md` (1k/10k DOM-style benchmarks, 100k scroll, Zedis 50k-keyspace soak, idle 0% check, RSS tracking).
- [ ] **Headless Visual Regression CI:**
  - Offscreen `wgpu` (WARP/SwiftShader/Lavapipe + hardware) golden-image diff suite on Windows, Linux, and macOS.
  - WGSL validation tests (`naga`) in CI for all shaders.
- [ ] **Codebase Modularization & Readability (Break Up Single-File Modules):**
  - Split `widgets/mod.rs` (~251 KB / 7,000+ lines) into focused submodules: `arena.rs`, `window.rs`, `paint.rs`, `hit_test.rs`, `layout_sync.rs`, and per-widget files (`container.rs`, `button.rs`, `text_input.rs`, `text_area.rs`, `virtual_list.rs`, `controls.rs`, `overlays.rs`).
  - Split `renderer/mod.rs` (~85 KB / 2,500+ lines) into `gpu_context.rs`, `draw_list.rs`, `atlas.rs`, `glass.rs`, `pipelines.rs`, `render_target.rs`.
  - Split `window/mod.rs` (~31 KB) into `app.rs`, `context.rs`, `delegate.rs`, `events.rs`.
- [ ] **Developer-Friendly Public API & Ergonomics (`v1.0.0`):**
  - Add `cadmium::prelude` (`use cadmium::prelude::*;`) for one-line imports of core widgets, signals, layout, and theme tokens.
  - Add ergonomic app bootstrap (`cadmium::run(...)` / `App::builder()`) and unified builder traits across widgets (`.padding()`, `.margin()`, `.width()`, `.height()`, `.disabled()`, `.on_click()`, `.on_hover()`, `.tooltip()`).
  - Change internal subsystems in `lib.rs` from `pub mod` to `mod` with clean curated `pub use` + controlled `gpu` escape-hatch module.
  - Rename `SteelError` → `CadmiumError` and wrap `winit`/`cosmic-text`/`taffy`/`accesskit`/`arboard`/`rfd` completely.
  - Complete `LICENSE-APACHE` full text, commit `Cargo.lock`, add MSRV CI check, reconcile `README.md` / `CHANGELOG.md` / `docs/`, and publish `v1.0.0`.