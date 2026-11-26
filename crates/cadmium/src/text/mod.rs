//! Text shaping, measurement, and glyph caching powered by `cosmic-text` and `swash`.
//!
//! [`TextEngine`] owns the font database (`cosmic-text`'s `FontSystem`, backed by
//! `fontdb`) and a `SwashCache` used to rasterize glyph masks on demand. It is
//! created once by [`crate::renderer::GpuContext`] and can be borrowed while
//! drawing through [`crate::window::FrameContext::text_engine`].
//!
//! A [`TextBlock`] holds a shaped `cosmic-text` buffer for one string and one
//! [`TextStyle`]. ASCII text is shaped with `Shaping::Basic`; non-ASCII text uses
//! `Shaping::Advanced` so complex scripts, ligatures, and color glyphs are
//! handled correctly. Replacing the text re-shapes the buffer, while changing
//! only [`TextStyle::color`] with [`TextBlock::set_color`] does not.
//!
//! Sizes in [`TextStyle`] (`font_size`, `line_height`) and the values returned by
//! [`TextBlock::measure`] are in **logical pixels**; the renderer scales glyph
//! positions by the frame's device-pixel ratio. Colors are linear-RGBA
//! [`crate::core::Color`] values. Text blocks are usually created by widgets such
//! as [`crate::widgets::Text`] and recorded with
//! [`crate::window::FrameContext::push_text`].
//!
//! # Examples
//!
//! ```rust
//! use cadmium::core::Color;
//! use cadmium::text::{FontWeight, TextStyle};
//!
//! let style = TextStyle {
//!     font_size: 14.0,
//!     line_height: 20.0,
//!     color: Color::from_rgba8(240, 240, 245, 255),
//!     weight: FontWeight::Semibold,
//! };
//! assert_eq!(style.font_size, 14.0);
//! ```

use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
    SwashImage, Weight,
};

use crate::core::Color;

include!("style.rs");
include!("engine.rs");
include!("block.rs");
