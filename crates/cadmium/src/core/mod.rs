//! Foundational value types and the shared error type for Cadmium.
//!
//! [`Color`], [`Rect`], and [`CornerRadii`] are small `Copy` values shared by
//! [`crate::widgets`], [`crate::layout`], and [`crate::renderer`].
//! [`CadmiumError`] is the common error type returned by application, window,
//! renderer, asset, and platform APIs; [`crate::window`] uses it as the result
//! type for [`crate::window::App::run`].
//!
//! # Colors
//!
//! Colors are stored as **linear RGBA**: the `r`, `g`, and `b` fields are
//! linear-light components and `a` is straight (not premultiplied) alpha.
//! [`Color::from_rgba8`] decodes the familiar 8-bit sRGB palette values used by
//! CSS and design tools into this linear space; [`Color::rgba`] is for values
//! that are already linear. Cadmium's pipelines render to sRGB-formatted
//! textures, so keeping palette values linear avoids double gamma conversion.
//!
//! # Geometry
//!
//! [`Rect`] and [`CornerRadii`] use **logical pixels**, with the origin at the
//! top-left and `y` increasing downward. [`crate::layout`] computes rectangles
//! in this space, [`crate::widgets`] uses them for hit testing and styling, and
//! [`crate::renderer`] scales them by the window's device-pixel ratio before
//! drawing. Corner radii are ordered top-left, top-right, bottom-right,
//! bottom-left, both in the fields and in [`CornerRadii::to_array`].
//!
//! # Example
//!
//! ```rust
//! use cadmium::core::{CadmiumError, Color, CornerRadii, Rect};
//!
//! let accent = Color::from_rgba8(0, 122, 255, 255);
//! assert!(accent.g > 0.18 && accent.g < 0.21); // sRGB 122 decodes to ~0.195
//! assert_eq!(accent.a, 1.0);
//!
//! let card = Rect::new(10.0, 20.0, 100.0, 50.0);
//! assert!(card.contains(10.0, 20.0));
//! assert_eq!(card.scale(2.0), Rect::new(20.0, 40.0, 200.0, 100.0));
//!
//! let radii = CornerRadii::all(12.0);
//! assert_eq!(radii.to_array(), [12.0, 12.0, 12.0, 12.0]);
//!
//! let error = CadmiumError::Svg("bad path".into());
//! assert_eq!(error.to_string(), "SVG error: bad path");
//! ```

use thiserror::Error;

include!("color.rs");
include!("geometry.rs");
include!("error.rs");
include!("tests.rs");
