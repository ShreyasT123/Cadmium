//! Image decoding, SVG rasterization, and texture-coordinate helpers.
//!
//! [`load_image_file`] decodes common raster formats through the `image` crate,
//! and [`rasterize_svg`] parses and renders SVG strings with `resvg`/`tiny-skia`.
//! Both produce a [`DecodedImage`] holding straight (non-premultiplied) RGBA8
//! pixels in row-major order, ready for [`crate::renderer::GpuContext::upload_image`]
//! and [`crate::renderer::ImageQuad`]. [`cover_uv_rect`] computes a crop for
//! "cover" scaling, as used by [`crate::widgets::Image`].
//!
//! # Examples
//!
//! ```rust
//! use cadmium::assets::{cover_uv_rect, rasterize_svg, DecodedImage};
//!
//! let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#;
//! let image: DecodedImage = rasterize_svg(svg, 32, 32).unwrap();
//! assert_eq!((image.width, image.height), (32, 32));
//! assert_eq!(image.rgba.len(), 32 * 32 * 4);
//!
//! let uv = cover_uv_rect(image.width, image.height, 64.0, 64.0);
//! assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
//! ```

use std::path::Path;

use crate::core::CadmiumError;

/// A decoded image in straight RGBA8 format, ready for GPU upload.
///
/// Produced by [`load_image_file`] and [`rasterize_svg`], and accepted by
/// [`crate::renderer::GpuContext::upload_image`]. Pixels are row-major, with four
/// bytes per pixel in `r, g, b, a` order; alpha is not premultiplied.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Row-major RGBA8 pixel data, `width * height * 4` bytes long.
    pub rgba: Vec<u8>,
}

/// Loads and decodes an image file into straight RGBA8 pixels.
///
/// The format is inferred by the `image` crate from the file contents and path;
/// common raster formats such as PNG, JPEG, and WebP are supported. The decoded
/// image is converted to RGBA8 and returned with its original pixel dimensions.
///
/// # Errors
///
/// Returns [`CadmiumError::Image`] if the file cannot be read, its format is not
/// recognized, or decoding fails.
///
/// # Examples
///
/// ```no_run
/// use cadmium::assets::load_image_file;
/// use cadmium::CadmiumError;
///
/// # fn icon() -> Result<cadmium::DecodedImage, CadmiumError> {
/// let image = load_image_file("assets/icon.png")?;
/// assert_eq!(image.rgba.len(), image.width as usize * image.height as usize * 4);
/// # Ok(image)
/// # }
/// ```
pub fn load_image_file(path: impl AsRef<Path>) -> Result<DecodedImage, CadmiumError> {
    let img = image::open(path)?.to_rgba8();
    let (width, height) = img.dimensions();
    Ok(DecodedImage {
        width,
        height,
        rgba: img.into_raw(),
    })
}

/// Rasterizes an SVG string into a straight RGBA8 image of the requested size.
///
/// The SVG is parsed with default `resvg::usvg` options. `width` and `height`
/// are the output dimensions in pixels and are clamped to at least one. The
/// artwork is uniformly scaled to fit inside the output box, preserving its
/// aspect ratio, and centered; it is never stretched, so letterboxing may occur.
/// Transparent areas remain transparent, and the premultiplied output of
/// `tiny-skia` is converted back to straight alpha for GPU upload.
///
/// # Errors
///
/// Returns [`CadmiumError::Svg`] if the SVG cannot be parsed or the pixmap cannot
/// be allocated (for example, because the requested dimensions are too large).
///
/// # Examples
///
/// ```rust
/// use cadmium::assets::rasterize_svg;
///
/// let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><circle cx="10" cy="5" r="5" fill="blue"/></svg>"#;
/// let image = rasterize_svg(svg, 64, 64).unwrap();
/// assert_eq!((image.width, image.height), (64, 64));
/// ```
pub fn rasterize_svg(svg: &str, width: u32, height: u32) -> Result<DecodedImage, CadmiumError> {
    let width = width.max(1);
    let height = height.max(1);
    let opt = resvg::usvg::Options::default();
    let tree =
        resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| CadmiumError::Svg(e.to_string()))?;

    let svg_size = tree.size();
    let sx = width as f32 / svg_size.width().max(1.0);
    let sy = height as f32 / svg_size.height().max(1.0);
    let scale = sx.min(sy);
    let tx = (width as f32 - svg_size.width() * scale) * 0.5;
    let ty = (height as f32 - svg_size.height() * scale) * 0.5;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| CadmiumError::Svg("Failed to allocate SVG pixmap".into()))?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(tx, ty);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert premultiplied tiny-skia RGBA to straight RGBA8
    let mut rgba = pixmap.take();
    #[allow(clippy::chunks_exact_to_as_chunks)] // MSRV 1.85: as_chunks_mut requires Rust 1.88
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3];
        if a > 0 && a < 255 {
            let af = a as f32 / 255.0;
            px[0] = (px[0] as f32 / af).min(255.0) as u8;
            px[1] = (px[1] as f32 / af).min(255.0) as u8;
            px[2] = (px[2] as f32 / af).min(255.0) as u8;
        }
    }

    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

/// Computes normalized `cover` UVs so an image fills a box without stretching.
///
/// Returns `[u0, v0, u1, v1]` in normalized texture coordinates. The image is
/// scaled to cover the destination and the overflowing axis is cropped equally
/// on both sides. Assign the result to [`crate::renderer::ImageQuad::uv_rect`]
/// to draw an image with "cover" fit.
///
/// `img_w` and `img_h` are source dimensions in pixels; `box_w` and `box_h` are
/// destination dimensions in the same unit. Values below one are clamped to one
/// before aspect ratios are computed.
///
/// # Examples
///
/// ```rust
/// use cadmium::assets::cover_uv_rect;
///
/// // A 2:1 image in a square box: crop the horizontal axis.
/// let uv = cover_uv_rect(200, 100, 100.0, 100.0);
/// assert!((uv[0] - 0.25).abs() < 1e-6);
/// assert!((uv[2] - 0.75).abs() < 1e-6);
/// assert_eq!((uv[1], uv[3]), (0.0, 1.0));
///
/// // Matching aspect ratios need no crop.
/// assert_eq!(cover_uv_rect(100, 50, 200.0, 100.0), [0.0, 0.0, 1.0, 1.0]);
/// ```
pub fn cover_uv_rect(img_w: u32, img_h: u32, box_w: f32, box_h: f32) -> [f32; 4] {
    let img_aspect = img_w as f32 / img_h.max(1) as f32;
    let box_aspect = box_w / box_h.max(1.0);
    if img_aspect > box_aspect {
        let keep = box_aspect / img_aspect;
        let pad = (1.0 - keep) * 0.5;
        [pad, 0.0, 1.0 - pad, 1.0]
    } else {
        let keep = img_aspect / box_aspect;
        let pad = (1.0 - keep) * 0.5;
        [0.0, pad, 1.0, 1.0 - pad]
    }
}
