//! # Cadmium
//!
//! A GPU-native, reactive desktop GUI framework for Rust with macOS-inspired
//! *Liquid Glass* aesthetics. Cadmium draws its own widgets with `wgpu` and
//! custom WGSL shaders: it does not wrap native controls, embed a web view, or
//! route rendering through Skia.
//!
//! ## What this crate provides
//!
//! - Declarative, builder-style widgets ([`Button`], [`Text`], [`Container`],
//!   [`ScrollView`], [`Tabs`], ...) that convert into a lightweight [`Element`]
//!   tree through [`IntoElement`].
//! - Fine-grained reactivity with copyable [`Signal`] handles, [`create_memo`],
//!   [`create_selector`], and [`create_effect`].
//! - Flexbox and grid layout through Taffy, with logical-pixel coordinates and
//!   DPI-aware pixel snapping (see [`crate::layout`]).
//! - A batched GPU renderer for analytic shapes, text, images, vector paths,
//!   and multi-pass glass materials (see [`crate::renderer`]).
//! - Multi-window applications driven by winit, with clipboard, native file
//!   dialogs, and AccessKit integration.
//!
//! ## Architecture
//!
//! An [`App`] owns the event loop and one or more windows. Each window is
//! driven by a [`WindowDelegate`]; most applications use [`WidgetWindow`],
//! which rebuilds a widget tree from a closure. The tree is retained in a
//! [`WidgetArena`], so changing a signal marks only the affected widgets dirty.
//! Each frame follows three stages:
//!
//! 1. **State** — reactive effects and memos are flushed.
//! 2. **Layout** — the [`crate::layout::LayoutEngine`] computes logical-pixel
//!    rectangles for dirty nodes.
//! 3. **Paint** — the renderer records GPU work into draw lists and presents
//!    it through the window surface.
//!
//! Geometry uses logical pixels throughout the public API. Colors are stored
//! as linear RGBA; [`Color::from_rgba8`] converts sRGB byte values for you.
//!
//! ## Installation
//!
//! Add Cadmium to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! cadmium = "0.1"
//! ```
//!
//! A working GPU backend and a display are required to run an application.
//!
//! ## Quick start
//!
//! ```no_run
//! use cadmium::{
//!     create_signal, AlignItems, App, Button, Column, Edges, FontWeight,
//!     MacTheme, Switch, Text, WidgetWindow, WindowSpec,
//! };
//!
//! fn main() -> Result<(), cadmium::CadmiumError> {
//!     let theme = MacTheme::light();
//!     let enabled = create_signal(true);
//!
//!     let delegate = WidgetWindow::new(move || {
//!         Column::new()
//!             .fill(theme.window_bg)
//!             .padding(Edges::all(24.0))
//!             .gap(12.0)
//!             .align(AlignItems::Center)
//!             .child(
//!                 Text::new("Hello, Cadmium")
//!                     .size(20.0)
//!                     .weight(FontWeight::Semibold),
//!             )
//!             .child(Switch::new(enabled))
//!             .child(Button::new("Continue").primary().on_click(move || {
//!                 enabled.update(|value| *value = !*value);
//!             }))
//!             .into_element()
//!     });
//!
//!     App::new()
//!         .add_window(WindowSpec {
//!             title: "Hello".into(),
//!             width: 420.0,
//!             height: 260.0,
//!             borderless: false,
//!             clear_color: theme.window_bg,
//!             delegate: Box::new(delegate),
//!         })
//!         .run()
//! }
//! ```
//!
//! For a single-window application, [`run`] provides a shorter form that
//! takes a title, size, and tree-building closure.
//!
//! ## Feature overview
//!
//! | Area | Highlights |
//! | --- | --- |
//! | Widgets | Containers, text, buttons, sliders, switches, tabs, dropdowns, scroll views, virtualized lists, and custom GPU passes |
//! | Styling | [`MacTheme`] palettes, [`Material::Classic`], [`Material::Frosted`], and [`Material::Liquid`] surfaces |
//! | Reactivity | [`Signal`], [`Memo`], [`Selector`], effects, and [`UiDispatcher`] for background-thread updates |
//! | Layout | Flexbox/grid, gaps, alignment, insets, and [`Length`] values |
//! | Text | Shaping, bidi, wrapping, and glyph atlas caching through [`crate::text`] |
//! | Platform | [`Clipboard`], [`FileDialog`], keyboard/mouse events, and [`SemanticTree`] |
//!
//! ## Important modules
//!
//! - [`crate::widgets`] — the widget catalog and element tree.
//! - [`crate::reactive`] — signals and fine-grained updates.
//! - [`crate::layout`] — layout primitives and the Taffy-backed engine.
//! - [`crate::style`] — themes, materials, and shadows.
//! - [`crate::renderer`] — GPU primitives and rendering resources.
//! - [`crate::window`] — application and window lifecycle.
//! - [`crate::prelude`] — frequently used re-exports.
//!
//! ## Related crates
//!
//! Cadmium builds on [`taffy`](https://docs.rs/taffy),
//! [`cosmic-text`](https://docs.rs/cosmic-text), and
//! [`wgpu`](https://docs.rs/wgpu). It is developed in the same workspace as
//! the `cadmium_music` and `liquid_glass` example applications.
//!
//! ## Additional resources
//!
//! - [Repository and examples](https://github.com/ShreyasT123/cadmium)
//! - [Project README](https://github.com/ShreyasT123/cadmium#readme)
//! - Licensed under the Apache License, Version 2.0 (see `LICENSE` in the repository root).

#![warn(missing_docs)]

pub mod accessibility;
pub mod animation;
pub mod assets;
pub mod core;
pub mod debug;
pub mod events;
pub mod layout;
pub mod platform;
pub mod prelude;
pub mod reactive;
pub mod renderer;
pub mod style;
pub mod text;
pub mod widgets;
pub mod window;

pub use crate::accessibility::{A11yAction, A11yRole, SemanticTree};
pub use crate::animation::Spring;
pub use crate::assets::{DecodedImage, cover_uv_rect, load_image_file, rasterize_svg};
pub use crate::core::{CadmiumError, Color, CornerRadii, Rect};
pub use crate::debug::{FrameMetrics, FrameProfiler};
pub use crate::events::{Key, Modifiers, WidgetId};
pub use crate::layout::{
    AlignItems, Edges, FlexDir, Inset, JustifyContent, LayoutEngine, LayoutNode, LayoutStyle,
    Length, PositionMode, snap_rect_to_pixels,
};
pub use crate::platform::{Clipboard, FileDialog};
pub use crate::reactive::{
    Memo, OwnedSignal, Selector, Signal, UiDispatcher, create_effect, create_memo, create_selector,
    create_signal, flush_effects,
};
pub use crate::renderer::{
    GlassQuad, GpuContext, ImageQuad, LyonPath, PathBuilder, PathVertex, SdfQuad, TextureHandle,
};
pub use crate::style::{MacTheme, Material, Shadow, ThemeMode};
pub use crate::text::{FontWeight, TextBlock, TextEngine, TextStyle};
pub use crate::widgets::{
    Accordion, Avatar, Badge, Breakpoint, Button, ButtonVariant, Card, Checkbox,
    CircularProgressIndicator, Column, CommandItem, CommandPalette, Container, CustomGpuPass,
    CustomGpuSurface, Divider, DockArea, DockGroup, DockNode, DockTab, Dropdown, Element, ForEach,
    GlassPanel, GridView, Icon, Image, ImageFit, ImageSource, IntoElement, Link, MenuAction,
    MenuBar, MenuGroup, MenuItem, Modal, ProgressBar, RadioButton, Row, ScrollView,
    SegmentedControl, ShaderCanvas, Show, Slider, Spacer, SplitDirection, SplitPane, Stack, Switch,
    Switcher, Table, TableColumn, TableRow, Tabs, Text, TextArea, TextInput, TextTone, ToastHost,
    ToastItem, ToastLevel, Toolbar, TrafficLights, TreeNode, TreeView, VirtualRow, VirtualizedList,
    WidgetArena, WidgetWindow, Wrap,
};
pub use crate::window::{
    App, CursorStyle, FrameContext, WindowCommand, WindowDelegate, WindowSpec, run,
};
