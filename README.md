<div align="center">

# SteelUI

**A GPU-native, reactive desktop GUI framework for Rust.**  
*Crafted for fluid motion, refined typography, and authentic macOS-grade Liquid Glass aesthetics across Windows, Linux, and macOS.*

**Early experimental prototype (`v0.1.0`) — active Phase 3 development.**

[![CI](https://github.com/ShreyasT123/cadmium/actions/workflows/ci.yml/badge.svg)](https://github.com/ShreyasT123/cadmium/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![GPU](https://img.shields.io/badge/graphics-wgpu%2030-purple.svg)](https://wgpu.rs)

</div>

---

## Highlights

* **Powered by Magnum:** A custom GPU-native 2D rendering pipeline built directly on `wgpu` and WGSL. No Skia, no WebViews, no Chromium, and no native OS widget wrapping.
* **Optical Liquid Glass:** Custom multi-pass shaders combining monotonic convex bezel refraction, chromatic aberration (RGB dispersion), dual-layer backdrop diffusion (sharp bezel + Gaussian core), and directional 45° opposing-corner specular catchlights.
* **Retained Widget Arena & Fine-Grained Signals:** Generational arena-backed widget tree (`slotmap`) paired with fine-grained reactive `Signal<T>` primitives. Subscribed widgets update their paint/layout state with zero full-tree reconstruction.
* **3-Stage Invalidation Pipeline:** Separates updates into `STATE` → `LAYOUT` → `PAINT`. Changing a hover state or dragging a slider bypasses layout recalculation entirely, maintaining a solid 60–144+ FPS and **dropping to 0% CPU/GPU when idle**.
* **Flexbox & Grid Layout via Taffy:** Flexbox row/column containers, gaps, alignment, and fractional DPI pixel snapping backed by an internal wrapper over `taffy` 0.15.
* **Decoupled Text Engine:** High-quality text shaping, bidi, wrapping, and glyph rasterization via `cosmic-text` and `swash`, rendered via a 512×512 GPU glyph atlas (grows dynamically up to 2048×2048).
* **Thread-Safe Async Dispatcher:** Safely dispatch closures and state mutations from Tokio or background threads back onto the UI thread via `UiDispatcher` with automatic event-loop waking.
* **Multi-Window by Design:** Shared process-wide `GpuContext` (device, pipelines, glyph atlas, texture caches) composited into canonical `Rgba8UnormSrgb` intermediate targets before per-window swapchain presentation.

---

## Architecture Overview

```
                      +-----------------------+
                      |   App / Event Loop    |  (winit multi-window runner)
                      +-----------+-----------+
                                  |
         +------------------------+------------------------+
         |                                                 |
         v                                                 v
+-------------------+                             +-------------------+
|   Widget Arena    |  (Generational SlotMap)     |   GpuContext      |  (Shared wgpu device,
|   + Signal Graph  |  (Fine-grained reactivity)  |   Magnum Engine   |   pipelines & atlases)
+---------+---------+                             +---------+---------+
          |                                                 |
          |  1. Reactive Flush                              |
          |  2. Taffy Layout + Snapping                     |
          |  3. DisplayList Recording                       |
          v                                                 v
+---------------------------------------------------------------------+
|                           Magnum Engine                             |
|  - Pass 1: Canonical Offscreen Target (Rgba8UnormSrgb)              |
|      * Instanced Analytical SDFs (boxes, radii, borders, shadows)   |
|      * Batched GPU Text (cosmic-text + swash shelf atlas)           |
|      * Textured Quads (PNG, JPEG, WebP, SVG rasterization)          |
|      * 2-Texture Optical Liquid Glass (refraction + dispersion)     |
|  - Pass 2: 9-tap Gaussian Mip Diffusion (1/2 -> 1/4 -> 1/2)         |
|  - Pass 3: Final Swapchain Format Blit                              |
+---------------------------------------------------------------------+
```

---

## Quick Example

Construct reactive desktop interfaces using declarative builders and copyable signals without fighting the Rust borrow checker:

```rust
use cadmium::{
    create_signal, AlignItems, App, Button, Column, Container,
    Edges, FontWeight, Length, MacTheme, Row, Slider, Spacer,
    Switch, Text, WidgetWindow, WindowSpec,
};

fn main() {
    let app = App::new();
    let theme = MacTheme::light();

    // Fine-grained reactive state handles (Copy)
    let radius = create_signal(14.0_f32);
    let is_active = create_signal(true);

    let window = WidgetWindow::new(move || {
        Column::new()
            .fill(theme.window_bg)
            .child(
                Row::new()
                    .height(Length::Px(52.0))
                    .padding(Edges::xy(20.0, 0.0))
                    .align(AlignItems::Center)
                    .fill(theme.content_bg)
                    .border(theme.hairline, 1.0)
                    .drag_region()
                    .child(Text::new("My Application").size(15.0).weight(FontWeight::Semibold))
                    .child(Spacer::flex())
                    .child(Switch::new(is_active))
            )
            .child(
                Container::column()
                    .padding(Edges::all(24.0))
                    .gap(16.0)
                    .child(
                        Container::column()
                            .padding(Edges::all(20.0))
                            .fill(theme.panel_bg)
                            .border(theme.hairline, 1.0)
                            .bind_radius(radius)
                            .shadow(theme.shadow_soft, 12.0, 4.0)
                            .child(Text::new("Reactive Card").size(17.0).weight(FontWeight::Bold))
                            .child(Text::reactive(move || format!("Radius: {:.0} pt", radius.get())))
                    )
                    .child(Slider::new(radius, 0.0, 32.0).width(200.0))
                    .child(Button::new("Click Me").primary().on_click(move || {
                        is_active.update(|v| *v = !*v);
                    }))
            )
            .into_element()
    });

    app.add_window(WindowSpec {
        title: "SteelUI App".into(),
        width: 800.0,
        height: 500.0,
        borderless: false,
        clear_color: theme.window_bg,
        delegate: Box::new(window),
    }).run().unwrap();
}
```

---

## Interactive Examples

Run any of the included reference applications to explore SteelUI:

### 1. Liquid Glass Menu & Interactive Conic Pill
Features elastic spring drag-and-drop, SVG icon rendering, crystal-clear optical refraction, and a dynamic conic-bordered glass button:
```powershell
cargo run -p liquid_glass
```

### 2. SteelUI Studio (Dogfood App)
Demonstrates the retained `WidgetArena`, reactive `Signal` updates, background worker dispatching via `UiDispatcher`, clipboard integration, and native file dialogs:
```powershell
cargo run -p cadmium_dogfood
```

---

## Testing

Run the cross-module unit test suite:
```powershell
cargo test --workspace
```

---

## Contributing

We welcome community contributions! Please read [CONTRIBUTING.md](CONTRIBUTING.md) and our [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before opening a pull request or submitting an issue.

---

## License

SteelUI is dual-licensed under either:

* **MIT License** ([LICENSE-MIT](LICENSE-MIT))
* **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you shall be dual-licensed as above, without any additional terms or conditions.
