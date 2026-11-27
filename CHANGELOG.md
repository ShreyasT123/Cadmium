# Changelog

All notable changes to Cadmium and the Magnum rendering engine will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Modular Subsystem Architecture:** Split `cadmium` into dedicated, documented submodules (`accessibility`, `animation`, `assets`, `core`, `debug`, `events`, `layout`, `platform`, `reactive`, `renderer`, `style`, `text`, `widgets`, `window`) with `prelude` and generated `rustdoc`.
- **O(1) Reactive Engine Upgrades:** Added `OwnedSignal<T>`, `create_memo`, `create_selector` (`~110 µs` on 10k-row selection), `ForEach`, `Show`, and relayout boundary coalescing.
- **Animated Transitions & Opacity Stack:** Added `DrawList::push_opacity` / `pop_opacity` and spring slide-crossfade transitions to `Switcher`, plus dirty-region glass blur skipping and `CustomGpuSurface::redraw_on`.
- **Responsiveness & Window Tiling:** Added `Breakpoint`, reactive `WidgetWindow::with_viewport`, `Wrap` flow layout, `Container::wrap` / `clip`, `Text::ellipsis`, `WindowSpec` `min_width`/`min_height`, Windows borderless shadow, and double-click drag-region maximize.
- **Tier 1 & Tier 2 Widget Suite:** Added `Card`, `Badge`, `Avatar`, `Link`, `Checkbox`, `RadioButton`, `SegmentedControl`, `Accordion`, `Modal`, `Image`, `Icon`, `ProgressBar`, `CircularProgressIndicator`, `SplitPane`, `VirtualizedList` (`.flat()`, `.columns_mode()`, `.row_art()`), `TreeView`, `Table`, `DockArea`, `MenuBar`, `Toolbar`, `CommandPalette`, `ToastHost`, `GridView`, and `ShaderCanvas`.
- **Apple Music Showcase (`apps/dogfood`):** Redesigned as **Cadmium Music** using real PNG assets (`assets/bg.png`, `assets/albums/*`), Frosted sidebar with collapsible `Pins`/`Playlists` dropdowns, Liquid Glass player capsule, 100k-track virtualized table, Light/Dark switching, and a Beats 01 stage with real album-art color bleed.
- **Live `AccessKit` Bridge & Criterion Benchmarks:** Wired `accesskit_winit::Adapter` and added `crates/cadmium/benches/gui_bench.rs`.

### Planned (Before v1.0.0)
- **IME & Focus Scopes:** `WindowEvent::Ime` (`Preedit` / `Commit`), cursor area placement, modal/menu focus trapping, and `TextArea` undo/redo.
- **Runtime Multi-Window & `wgpu` Recovery:** `WindowCommand::OpenWindow` for detached panels and device-lost rebuilds.
- **Subtree `DisplayList` Caching & Visual Regression CI:** Offscreen golden-image tests across Windows, Linux, and macOS, plus final `pub mod` API lockdown.

---

## [0.1.0] - 2026-11-23

### Added
- Initial Magnum `wgpu` 30 rendering engine (`sdf.wgsl`, `blur.wgsl`, `glass.wgsl`, `text.wgsl`, `textured.wgsl`), generational `WidgetArena`, `Signal<T>`, Taffy 0.15 layout integration, `Spring` physics, `TextInput`, `ScrollView`, `Tabs`, `Dropdown`, `TrafficLights`, and `examples/liquid_glass`.
