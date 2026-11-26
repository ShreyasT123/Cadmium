/// Texture format used by the glyph atlas, uploaded images and custom GPU
/// surfaces: [`wgpu::TextureFormat::Rgba8UnormSrgb`].
///
/// Eight bits per channel provide sRGB-encoded color and linear alpha. The
/// glyph atlas stores antialiasing coverage in the alpha channel with white
/// RGB, and [`crate::assets::DecodedImage`] data is expected to be straight
/// (not premultiplied) RGBA8 in this format.
pub const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const INITIAL_ATLAS_SIZE: u32 = 256;
const MAX_ATLAS_SIZE: u32 = 2048;

use slotmap::{SlotMap, new_key_type};

new_key_type! {
    /// Opaque, generation-counted handle to a texture stored in a [`GpuContext`].
    ///
    /// Returned by [`GpuContext::upload_image`] and
    /// [`GpuContext::create_render_texture`]; pass it to [`ImageQuad::new`]
    /// or the [`ImageQuad::texture`] field to draw the texture. Handles never
    /// alias a previously released texture, but an [`ImageQuad`] whose
    /// handle has been released with [`GpuContext::release_texture`] is
    /// skipped at render time.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::renderer::{GpuContext, ImageQuad, TextureHandle};
    /// use cadmium::{DecodedImage, Rect};
    ///
    /// # fn make_quad(gpu: &mut GpuContext, image: &DecodedImage) -> TextureHandle {
    /// let handle: TextureHandle = gpu.upload_image(image);
    /// let quad = ImageQuad::new(handle, Rect::new(0.0, 0.0, 64.0, 64.0));
    /// # let _ = quad;
    /// # handle
    /// # }
    /// ```
    pub struct TextureHandle;
}

/// One instanced rounded-rectangle primitive for the SDF pipeline.
///
/// Each value becomes a single four-vertex instance. The fragment shader
/// evaluates a rounded-box signed distance field, so corners stay crisp at
/// any size, and can combine a gradient fill, an inset border and a soft
/// drop shadow in one draw. `rect`, radii, border and shadow values are in
/// physical pixels; scale them with the frame's scale factor before
/// recording them.
///
/// # Examples
///
/// ```
/// use cadmium::core::{Color, CornerRadii, Rect};
/// use cadmium::renderer::SdfQuad;
///
/// let pill = SdfQuad {
///     rect: Rect::new(8.0, 8.0, 120.0, 32.0),
///     radii: CornerRadii::all(16.0),
///     fill: Color::rgba(0.1, 0.5, 0.9, 1.0),
///     fill_end: Some(Color::rgba(0.2, 0.7, 1.0, 1.0)),
///     gradient_dir: [0.0, 1.0],
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SdfQuad {
    /// Destination rectangle in physical pixels; `x` and `y` are the top-left corner.
    pub rect: Rect,
    /// Per-corner radii in physical pixels, in top-left, top-right, bottom-right,
    /// bottom-left order. The shader clamps each radius to half the box size.
    pub radii: CornerRadii,
    /// Fill color at the start of the gradient. Alpha participates in blending.
    pub fill: Color,
    /// End color of the linear gradient, or [`None`] for a solid [`Self::fill`].
    ///
    /// When present, colors are interpolated across the quad using
    /// [`Self::gradient_dir`].
    pub fill_end: Option<Color>,
    /// Direction of the linear gradient, expressed relative to the quad;
    /// `[0.0, 1.0]` (the default) runs from top to bottom.
    pub gradient_dir: [f32; 2],
    /// Rotation about the quad's center, in radians. Positive values rotate
    /// clockwise in screen space because screen-space `y` grows downwards.
    pub rotation: f32,
    /// Color of the border drawn just inside the rounded edge; a transparent
    /// color disables the border.
    pub border_color: Color,
    /// Border thickness in physical pixels. Values at or below roughly
    /// `0.001` are treated as no border, and large values are clamped to half
    /// the box size.
    pub border_width: f32,
    /// Shadow color; an alpha at or below roughly `0.001` disables the shadow.
    pub shadow_color: Color,
    /// Shadow translation in physical pixels, as `[x, y]`.
    pub shadow_offset: [f32; 2],
    /// Shadow blur radius in physical pixels. The shader derives a Gaussian
    /// sigma from it and reserves extra quad padding so the shadow is not
    /// clipped near the edges.
    pub shadow_blur: f32,
}

impl Default for SdfQuad {
    fn default() -> Self {
        Self {
            rect: Rect::default(),
            radii: CornerRadii::ZERO,
            fill: Color::TRANSPARENT,
            fill_end: None,
            gradient_dir: [0.0, 1.0],
            rotation: 0.0,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            shadow_color: Color::TRANSPARENT,
            shadow_offset: [0.0, 0.0],
            shadow_blur: 0.0,
        }
    }
}

/// A textured, rounded rectangle drawn from a [`TextureHandle`].
///
/// One instance of the image pipeline. The shader samples the texture with
/// the linear sampler, multiplies the sample by [`Self::tint`], and clips it
/// to a rounded-box signed distance field. [`Self::rect`] is in physical
/// pixels.
///
/// # Examples
///
/// ```no_run
/// use cadmium::assets::load_image_file;
/// use cadmium::core::Rect;
/// use cadmium::renderer::{GpuContext, ImageQuad};
///
/// # fn quad(gpu: &mut GpuContext, path: &str) -> Result<ImageQuad, cadmium::CadmiumError> {
/// let image = load_image_file(path)?;
/// let handle = gpu.upload_image(&image);
/// let quad = ImageQuad::new(
///     handle,
///     Rect::new(0.0, 0.0, image.width as f32, image.height as f32),
/// );
/// # Ok(quad)
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ImageQuad {
    /// Destination rectangle in physical pixels; `x` and `y` are the top-left corner.
    pub rect: Rect,
    /// Normalized texture window as `[u0, v0, u1, v1]`. The default covers
    /// the whole texture; [`crate::assets::cover_uv_rect`] produces a
    /// crop that fills a box without stretching.
    pub uv_rect: [f32; 4],
    /// Per-corner radii in physical pixels; the shader clamps them to half
    /// the box size.
    pub radii: CornerRadii,
    /// Linear RGBA multiplier applied to the sampled texture. The default
    /// is opaque white, which leaves the texture unchanged.
    pub tint: Color,
    /// Texture to sample. Must refer to a texture still registered in the
    /// `GpuContext` at render time, otherwise the image batch is skipped.
    pub texture: TextureHandle,
}

impl ImageQuad {
    /// Creates a full-texture, untinted, square-cornered image quad.
    ///
    /// `texture` should come from [`GpuContext::upload_image`] or
    /// [`GpuContext::create_render_texture`], and `rect` is in physical
    /// pixels. Set [`Self::uv_rect`], [`Self::radii`] or [`Self::tint`]
    /// afterwards to customize the drawing.
    pub fn new(texture: TextureHandle, rect: Rect) -> Self {
        Self {
            rect,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            radii: CornerRadii::ZERO,
            tint: Color::rgba(1.0, 1.0, 1.0, 1.0),
            texture,
        }
    }
}

/// A translucent, blurred "glass" rectangle sampled from the frame backdrop.
///
/// Glass quads are recorded in order with the rest of the draw list but are
/// drawn in a separate pass: [`WindowRenderTarget::render`] renders the
/// preceding primitives to an offscreen target, builds a blur pyramid, and
/// lets the glass shader blend a sharp and blurred copy of that backdrop
/// under a rounded, tinted surface. `rect`, radii and border width are in
/// physical pixels.
///
/// The default values approximate frosted glass: a translucent white tint,
/// boosted saturation, a visible specular sheen and moderate frost.
///
/// # Examples
///
/// ```
/// use cadmium::core::{Color, CornerRadii, Rect};
/// use cadmium::renderer::GlassQuad;
///
/// let panel = GlassQuad {
///     rect: Rect::new(0.0, 0.0, 320.0, 200.0),
///     radii: CornerRadii::all(24.0),
///     tint: Color::from_rgba8(255, 255, 255, 48),
///     frost: 0.45,
///     saturation: 1.35,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct GlassQuad {
    /// Destination rectangle in physical pixels; `x` and `y` are the top-left corner.
    pub rect: Rect,
    /// Per-corner radii in physical pixels; the shader clamps them to half
    /// the box size.
    pub radii: CornerRadii,
    /// Color and opacity of the surface tint. The RGB channels are converted
    /// from linear space to sRGB and mixed with the backdrop in sRGB space;
    /// the alpha channel is the mix factor. The default is white at
    /// `40 / 255` opacity.
    pub tint: Color,
    /// Color of the border drawn inside the rounded edge. Transparent means
    /// no border is blended in.
    pub border_color: Color,
    /// Border thickness in physical pixels; values at or below roughly
    /// `0.001` disable the border.
    pub border_width: f32,
    /// Multiplier applied to backdrop saturation: `0.0` is grayscale, `1.0`
    /// preserves the backdrop color, and values above `1.0` boost it until
    /// channels clip. The default is `1.35`.
    pub saturation: f32,
    /// Strength of the reflection band and, when [`Self::conic_strength`] is
    /// zero, the opposing-corner bevel highlights. `0.0` disables them. The
    /// default is `0.90`.
    pub specular: f32,
    /// Mix between the sharp and blurred backdrop: `0.0` shows the sharp
    /// copy and `1.0` the blurred one. Frost is reduced near the beveled
    /// rim. Values below `0.30` also request an extra sharp-backdrop
    /// snapshot for effects that distort the backdrop. The default is `0.45`.
    pub frost: f32,
    /// Amplitude of a smooth organic distortion applied to backdrop
    /// sampling, in physical pixels before scaling. `0.0` (the default)
    /// disables it; any value above `0.001` requests an extra sharp-backdrop
    /// snapshot.
    pub liquid_ripple: f32,
    /// Angle in radians used to orient the conic rim arc. It only has an
    /// effect when [`Self::conic_strength`] is above `0.001`. The default is
    /// approximately `-1.309` radians.
    pub rim_angle: f32,
    /// Position of the conic sheen band, typically in `0.0..=1.0`. It sweeps
    /// the band across the surface and only has an effect when
    /// [`Self::conic_strength`] is above `0.001`. The default is `0.0`.
    pub sheen_progress: f32,
    /// Strength of the conic sheen and rim effect. At `0.0` (the default)
    /// the regular specular bevel is used instead; any value above `0.001`
    /// also requests an extra sharp-backdrop snapshot.
    pub conic_strength: f32,
}

impl Default for GlassQuad {
    fn default() -> Self {
        Self {
            rect: Rect::default(),
            radii: CornerRadii::ZERO,
            tint: Color::from_rgba8(255, 255, 255, 40),
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            saturation: 1.35,
            specular: 0.90,
            frost: 0.45,
            liquid_ripple: 0.0,
            rim_angle: -1.309,
            sheen_progress: 0.0,
            conic_strength: 0.0,
        }
    }
}

