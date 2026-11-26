//! Visual styling primitives: surface [`Material`]s, [`Shadow`]s, and the macOS-inspired
//! [`MacTheme`] palette.
//!
//! A [`Material`] describes how a widget surface is painted. [`Material::Classic`] draws an
//! opaque or gradient surface with crisp hairlines and analytic Gaussian shadows;
//! [`Material::Frosted`] blurs the backdrop behind a translucent tint; and
//! [`Material::Liquid`] adds lens refraction, chromatic dispersion, and a moving sheen.
//! All three variants share the same [`Shadow`] description for their optional drop shadow.
//! [`MacTheme`] groups the colors used by the built-in widgets in [`crate::widgets`], and
//! [`ThemeMode`] selects between its light and dark presets.
//!
//! These types are plain `Copy` data: widget builders such as
//! [`crate::widgets::Container::material`] store them, and the widget renderer lowers them
//! to [`crate::renderer::SdfQuad`] and [`crate::renderer::GlassQuad`] instances for
//! [`crate::renderer::GpuContext`]. Dimensions such as `border_width` and
//! [`Shadow::blur`] are in **logical pixels** and are multiplied by the window's
//! device-pixel ratio during rendering. Colors are linear-RGBA [`crate::core::Color`]
//! values; use [`crate::core::Color::from_rgba8`] to convert sRGB bytes.
//!
//! # Example
//!
//! ```
//! use cadmium::core::Color;
//! use cadmium::style::{MacTheme, Material, Shadow, ThemeMode};
//! use cadmium::widgets::Container;
//!
//! let theme = MacTheme::from_mode(ThemeMode::Dark);
//! let material = Material::Classic {
//!     fill: theme.panel_bg,
//!     fill_end: None,
//!     gradient_dir: [0.0, 1.0],
//!     border_color: theme.hairline,
//!     border_width: 1.0,
//!     shadow: Some(Shadow::new(theme.shadow_soft, 12.0, 4.0)),
//! };
//! let panel = Container::new().material(material);
//! ```

include!("material.rs");
include!("theme.rs");
