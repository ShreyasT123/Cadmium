//! GPU rendering backend for Cadmium's "Xune" engine.
//!
//! This module contains the `wgpu`-based renderer used by every Cadmium
//! window. It is organized in three layers:
//!
//! - [`GpuContext`] owns the `wgpu` instance, adapter, device and queue, the
//!   [`crate::text::TextEngine`], the glyph atlas, uploaded textures, and
//!   lazily-created render pipelines. [`crate::window::App`] shares one
//!   context across all of its windows.
//! - [`DrawList`] records a frame's primitives without touching the GPU. It
//!   batches consecutive draws by kind (and by texture for images), tracks a
//!   stack of clip rectangles, and stores tessellated paths and glyph
//!   instances. [`crate::window::FrameContext`] exposes these recording
//!   operations to widgets during [`crate::window::WindowDelegate::render`].
//! - [`WindowRenderTarget`] owns per-window buffers and offscreen targets.
//!   [`WindowRenderTarget::render`] uploads a [`DrawList`], issues the `wgpu`
//!   draw calls into a surface view, and submits a command buffer.
//!
//! # Drawing model
//!
//! Coordinates for [`SdfQuad`], [`ImageQuad`] and [`GlassQuad`] are
//! **physical pixels** in surface space, with the origin at the top-left;
//! widths and corner radii should already be scaled by the frame's scale
//! factor (see [`crate::layout::snap_rect_to_pixels`]). In contrast,
//! [`DrawList::push_text`], [`DrawList::push_stroke_path`] and
//! [`DrawList::push_clip`] take **logical** coordinates and scale them by the
//! factor passed to [`DrawList::new`]. Colors are linear RGBA
//! ([`crate::core::Color`]); the pipelines target sRGB-formatted textures.
//!
//! Most primitives are instanced quads: signed-distance-field rounded boxes
//! with an optional gradient, border and shadow ([`SdfQuad`]); textured,
//! rounded, tinted quads ([`ImageQuad`]); glyph rectangles backed by a shared
//! glyph atlas at [`ATLAS_FORMAT`]; and translucent glass quads
//! ([`GlassQuad`]). Paths are tessellated with `lyon` into indexed
//! [`PathVertex`] triangles. [`PathBuilder`] is a convenience wrapper around
//! the [`LyonPath`] builder for producing those paths.
//!
//! Glass quads sample the pixels drawn behind them. When a frame contains
//! glass, [`WindowRenderTarget::render`] first draws into an offscreen
//! canonical target, builds a small Kawase blur pyramid, composites the
//! glass quads against that backdrop, and finally blits the result to the
//! surface. Frames without glass are drawn directly to the surface.
//!
//! # Textures
//!
//! [`TextureHandle`] is a slot-map key returned by
//! [`GpuContext::upload_image`] and [`GpuContext::create_render_texture`].
//! Put it in an [`ImageQuad`] to draw the texture and call
//! [`GpuContext::release_texture`] when it is no longer needed. The
//! [`crate::widgets::CustomGpuSurface`] widget uses this API to hand a
//! `wgpu::TextureView` to a [`crate::widgets::CustomGpuPass`] and composite
//! the result back into the UI.
//!
//! # Examples
//!
//! Record a scaled rounded panel without creating a GPU context:
//!
//! ```rust
//! use cadmium::core::{Color, CornerRadii, Rect};
//! use cadmium::renderer::{DrawList, SdfQuad};
//!
//! let mut frame = DrawList::new(2.0);
//! frame.push_sdf(SdfQuad {
//!     rect: Rect::new(0.0, 0.0, 240.0, 96.0), // physical pixels
//!     radii: CornerRadii::all(16.0),
//!     fill: Color::rgba(0.15, 0.18, 0.22, 1.0),
//!     ..Default::default()
//! });
//! ```

use std::collections::HashMap;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use cosmic_text::CacheKey;
use lyon::math::point;
/// The `lyon` path type accepted by [`DrawList::push_stroke_path`].
///
/// Re-exported so callers can build paths with the same `lyon` version the
/// renderer uses. [`PathBuilder`] provides a small convenience wrapper over
/// this type's builder.
pub use lyon::path::Path as LyonPath;
use lyon::tessellation::*;
use wgpu::util::DeviceExt;

use crate::assets::DecodedImage;
use crate::core::{CadmiumError, Color, CornerRadii, Rect};
use crate::text::{ShapedGlyph, TextBlock, TextEngine, swash_is_color};

include!("primitives.rs");
include!("geometry.rs");
include!("draw_list.rs");
include!("atlas.rs");
include!("context.rs");
include!("target.rs");
include!("pipelines.rs");
include!("tests.rs");
