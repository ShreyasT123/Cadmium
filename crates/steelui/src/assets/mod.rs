use std::path::Path;

use crate::core::SteelError;

pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn load_image_file(path: impl AsRef<Path>) -> Result<DecodedImage, SteelError> {
    let img = image::open(path)?.to_rgba8();
    let (width, height) = img.dimensions();
    Ok(DecodedImage {
        width,
        height,
        rgba: img.into_raw(),
    })
}

pub fn rasterize_svg(svg: &str, width: u32, height: u32) -> Result<DecodedImage, SteelError> {
    let width = width.max(1);
    let height = height.max(1);
    let opt = resvg::usvg::Options::default();
    let tree =
        resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| SteelError::Svg(e.to_string()))?;

    let svg_size = tree.size();
    let sx = width as f32 / svg_size.width().max(1.0);
    let sy = height as f32 / svg_size.height().max(1.0);
    let scale = sx.min(sy);
    let tx = (width as f32 - svg_size.width() * scale) * 0.5;
    let ty = (height as f32 - svg_size.height() * scale) * 0.5;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| SteelError::Svg("Failed to allocate SVG pixmap".into()))?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(tx, ty);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert premultiplied tiny-skia RGBA to straight RGBA8
    let mut rgba = pixmap.take();
    for px in rgba.as_chunks_mut::<4>().0 {
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
