use crate::core::Color;
use crate::renderer::GlassOptics;

/// Parameters for a drop shadow behind a widget surface.
///
/// Used by every variant of [`Material`]. The blur radius and offset are in logical
/// pixels; the widget renderer scales both by the current device-pixel ratio before
/// submitting the shadow to [`crate::renderer`].
///
/// # Examples
///
/// ```
/// use cadmium::core::Color;
/// use cadmium::style::Shadow;
///
/// let shadow = Shadow::new(Color::from_rgba8(0, 0, 0, 60), 12.0, 4.0);
/// assert_eq!(shadow.blur, 12.0);
/// assert_eq!(shadow.offset, [0.0, 4.0]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Shadow color, including alpha. A fully transparent color disables the visible
    /// shadow even when the other fields are non-zero.
    pub color: Color,
    /// Gaussian blur radius in logical pixels; `0.0` gives a hard-edged shadow.
    pub blur: f32,
    /// Shadow displacement in logical pixels as `[x, y]`. Positive `y` moves the shadow
    /// downward, matching a light source above the surface.
    pub offset: [f32; 2],
}

impl Shadow {
    /// Creates a shadow with no horizontal offset.
    ///
    /// `color` is the shadow color, `blur` is the logical-pixel blur radius, and
    /// `offset_y` is the downward logical-pixel displacement.
    pub const fn new(color: Color, blur: f32, offset_y: f32) -> Self {
        Self {
            color,
            blur,
            offset: [0.0, offset_y],
        }
    }
}

/// The surface treatment painted behind a widget.
///
/// Each variant corresponds to a different rendering path:
///
/// - [`Material::Classic`] produces a signed-distance-field surface
///   ([`crate::renderer::SdfQuad`]) with a flat or gradient fill, a hairline border, and
///   a Gaussian drop shadow.
/// - [`Material::Frosted`] produces a [`crate::renderer::GlassQuad`] that blurs and tints
///   whatever is behind the widget, with an optional shadow pass underneath.
/// - [`Material::Liquid`] produces the same glass quad with a refracting lens: Snell's-law
///   bending across a convex bevel, wavelength-dependent chromatic dispersion, ripple
///   distortion, Fresnel/catchlight lighting and an optional animated conic sheen. The lens is
///   described by [`GlassOptics`].
///
/// The helper constructors [`Material::solid`], [`Material::frosted`], and
/// [`Material::liquid`] provide sensible presets; the struct variants allow every field
/// to be tuned. Border widths and shadow dimensions are logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    /// A crisp, analytically drawn surface: a solid color or a two-stop linear gradient,
    /// a thin border, and an optional Gaussian shadow. Best for ordinary controls and
    /// panels that should not sample the backdrop.
    Classic {
        /// Fill color at the start of the gradient, or the entire fill when `fill_end`
        /// is `None`.
        fill: Color,
        /// Optional second gradient stop. `None` gives a flat fill; `Some(color)` blends
        /// from `fill` to this color along `gradient_dir`.
        fill_end: Option<Color>,
        /// Gradient direction as an `[x, y]` vector in the surface's local coordinates.
        /// `[0.0, 1.0]` (the preset used by [`Material::solid`]) runs from top to bottom;
        /// only meaningful when `fill_end` is `Some`.
        gradient_dir: [f32; 2],
        /// Border color, including alpha. A transparent color draws no visible border.
        border_color: Color,
        /// Border thickness in logical pixels. The renderer scales it by the device-pixel
        /// ratio; `0.0` draws no border.
        border_width: f32,
        /// Optional drop shadow drawn behind the surface. `None` disables it.
        shadow: Option<Shadow>,
    },
    /// Translucent "frosted glass": a blurred, tinted copy of the backdrop with a soft
    /// edge sheen and a thin highlight border. An optional shadow pass is drawn before
    /// the glass so the surface appears to float above its surroundings.
    Frosted {
        /// Tint color. Its alpha controls how strongly the tint replaces the sampled
        /// backdrop: `0.0` leaves the backdrop untouched, `1.0` makes the surface opaque.
        tint: Color,
        /// Blend factor between the sharp and blurred backdrop, typically `0.0`–`1.0`.
        /// The shader reduces it near the bevel so the rim stays clearer than the center.
        frost: f32,
        /// Strength of the curved edge highlight; `0.0` removes the sheen.
        specular: f32,
        /// Color of the one-pixel glass edge highlight, including alpha.
        border_color: Color,
        /// Border thickness in logical pixels, scaled by the device-pixel ratio when drawn.
        border_width: f32,
        /// Optional drop shadow drawn behind the glass. `None` disables it.
        shadow: Option<Shadow>,
    },
    /// "Liquid glass": a convex optical bezel that refracts the backdrop with Snell's law, splits red,
    /// green, and blue by wavelength-dependent index of refraction, adds a slow organic ripple, and
    /// can sweep a conic sheen around the rim. An optional shadow pass is drawn first.
    Liquid {
        /// Tint color blended over the refracted backdrop; alpha controls tint strength.
        tint: Color,
        /// Blend factor between sharp and blurred backdrop in the flat center. The lens
        /// bevel reduces this near the rim to keep refracted detail crisp.
        frost: f32,
        /// Amplitude of the organic wave distortion applied to the backdrop, in logical
        /// pixels; `0.0` disables the ripple.
        ripple: f32,
        /// Strength of the bevel catchlights. Cross-fades with the conic sheen as
        /// `conic_strength` rises.
        specular: f32,
        /// Angle in radians of the dark arc around the bezel rim; `-1.309` is the preset
        /// used by [`Material::liquid`].
        rim_angle: f32,
        /// Progress of the conic sheen band across the surface. Animate this from `0.0`
        /// to `1.0` to sweep the highlight; it has no effect while `conic_strength` is zero.
        sheen_progress: f32,
        /// Strength of the conic sheen and refraction ring. At `0.0` the shader uses the
        /// bevel catchlights instead.
        conic_strength: f32,
        /// Lens optics: index of refraction, dispersion, bevel width, refraction depth,
        /// convexity and light direction. [`GlassOptics::LIQUID`] is the preset used by
        /// [`Material::liquid`]; [`GlassOptics::FLAT`] turns the lens off.
        optics: GlassOptics,
        /// Color of the glass edge highlight, including alpha.
        border_color: Color,
        /// Border thickness in logical pixels, scaled by the device-pixel ratio when drawn.
        border_width: f32,
        /// Optional drop shadow drawn behind the glass. `None` disables it.
        shadow: Option<Shadow>,
    },
}

impl Material {
    /// Creates a flat [`Material::Classic`] surface with no border or shadow.
    ///
    /// The gradient direction defaults to `[0.0, 1.0]` and `fill_end` is `None`, so the
    /// surface is a single solid `color`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::style::Material;
    ///
    /// let material = Material::solid(Color::from_rgba8(0, 122, 255, 255));
    /// match material {
    ///     Material::Classic { fill, fill_end, border_width, shadow, .. } => {
    ///         assert_eq!(fill, Color::from_rgba8(0, 122, 255, 255));
    ///         assert_eq!(fill_end, None);
    ///         assert_eq!(border_width, 0.0);
    ///         assert_eq!(shadow, None);
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn solid(color: Color) -> Self {
        Self::Classic {
            fill: color,
            fill_end: None,
            gradient_dir: [0.0, 1.0],
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            shadow: None,
        }
    }

    /// Creates a [`Material::Frosted`] glass surface with a balanced preset.
    ///
    /// Uses `frost: 0.45`, `specular: 0.85`, a one-logical-pixel translucent white border
    /// (alpha `45`), and a soft black shadow (alpha `48`, blur `16.0`, vertical offset
    /// `6.0`). `tint` is the color blended over the blurred backdrop.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::style::Material;
    ///
    /// let material = Material::frosted(Color::from_rgba8(255, 255, 255, 130));
    /// match material {
    ///     Material::Frosted { frost, border_width, shadow, .. } => {
    ///         assert_eq!(frost, 0.45);
    ///         assert_eq!(border_width, 1.0);
    ///         assert!(shadow.is_some());
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn frosted(tint: Color) -> Self {
        Self::Frosted {
            tint,
            frost: 0.45,
            specular: 0.85,
            border_color: Color::from_rgba8(255, 255, 255, 45),
            border_width: 1.0,
            shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 48), 16.0, 6.0)),
        }
    }

    /// Creates a [`Material::Liquid`] glass surface with subtle lens effects.
    ///
    /// Uses `frost: 0.12`, `ripple: 0.25`, `specular: 0.95`, `rim_angle: -1.309`,
    /// `sheen_progress: 0.0`, and `conic_strength: 0.0` (so the initial look is the
    /// bevel catchlights), the [`GlassOptics::LIQUID`] lens, plus a one-logical-pixel translucent
    /// white border (alpha `65`) and a soft black shadow (alpha `52`, blur `18.0`, vertical
    /// offset `8.0`).
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::style::Material;
    ///
    /// let material = Material::liquid(Color::from_rgba8(255, 255, 255, 40));
    /// match material {
    ///     Material::Liquid { frost, ripple, conic_strength, optics, .. } => {
    ///         assert_eq!(frost, 0.12);
    ///         assert_eq!(ripple, 0.25);
    ///         assert_eq!(conic_strength, 0.0);
    ///         assert!(optics.thickness > 0.0);
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn liquid(tint: Color) -> Self {
        Self::Liquid {
            tint,
            frost: 0.12,
            ripple: 0.25,
            specular: 0.95,
            rim_angle: -1.309,
            sheen_progress: 0.0,
            conic_strength: 0.0,
            optics: GlassOptics::LIQUID,
            border_color: Color::from_rgba8(255, 255, 255, 65),
            border_width: 1.0,
            shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 52), 18.0, 8.0)),
        }
    }

    /// Replaces the lens optics of a [`Material::Liquid`] surface.
    ///
    /// The other variants have no lens and are returned unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::renderer::GlassOptics;
    /// use cadmium::style::Material;
    ///
    /// let lens = GlassOptics { dispersion: 0.30, ..GlassOptics::LIQUID };
    /// let material = Material::liquid(Color::from_rgba8(255, 255, 255, 40)).with_optics(lens);
    /// match material {
    ///     Material::Liquid { optics, .. } => assert_eq!(optics.dispersion, 0.30),
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn with_optics(mut self, lens: GlassOptics) -> Self {
        if let Self::Liquid { optics, .. } = &mut self {
            *optics = lens;
        }
        self
    }
}





// crates/cadmium/src/renderer/primitives.rs
//
// DROP-IN SECTION. In your existing file, replace:
//   * `pub struct GlassQuad` and `impl Default for GlassQuad`
//   * the `RawGlassInstance` definition (it was not part of the paste, so its layout is reconstructed
//     from the vertex attributes in `create_glass_pipeline`: offsets 0..96, i.e. 96 bytes)
// Keep everything else (`SdfQuad`, `ImageQuad`, `TextureHandle`, `RawSdfInstance`, `DrawList`, ...).
//
// Then:
//   1. add `GlassOptics` to the `pub use primitives::{..}` list in `renderer/mod.rs`;
//   2. apply the three-line change to `DrawList::push_glass` shown at the bottom of this file;
//   3. replace `create_glass_pipeline` in pipelines.rs (the shader now reads locations 9 and 10).

/// Direction toward the key light, as an angle in radians in screen space (`y` grows downwards):
/// `-3π/4` puts the light in the upper-left, so catchlights land on the top-left and bottom-right rim.
const LIGHT_UPPER_LEFT: f32 = -2.356_194_5;

/// Optical description of a glass lens: how its beveled rim bends, splits and reflects light.
///
/// The glass shader builds a convex height field from the quad's rounded-box signed distance field:
/// the plateau in the middle is flat and the outer [`Self::bevel_width`] ring tilts up to a rim angle
/// set by [`Self::convexity`] following a monotone `x^1.5` profile. A vertical view ray is refracted
/// at that tilt with Snell's law, travels [`Self::thickness`] through the glass, and samples the
/// backdrop there. Red, green and blue use slightly different indices of refraction
/// ([`Self::dispersion`]) and are integrated over seven spectral taps, so the prism split is smooth
/// instead of banded.
///
/// The refraction depth is clamped inside the shader to the largest value that keeps the sample
/// mapping strictly monotone, so large `thickness` values saturate instead of folding the image over
/// itself. A lower `convexity` allows a larger displacement fraction of the bevel; a higher one gives
/// a steeper, tighter rim.
///
/// [`GlassOptics::FLAT`] disables refraction (plain frosted glass) and is what
/// [`GlassQuad::default`] uses; [`GlassOptics::LIQUID`] is the Liquid Glass lens preset.
///
/// # Examples
///
/// ```
/// use cadmium::renderer::GlassOptics;
///
/// let lens = GlassOptics { bevel_width: 18.0, dispersion: 0.30, ..GlassOptics::LIQUID };
/// assert_eq!(lens.ior, 1.5);
/// assert_eq!(GlassOptics::FLAT.thickness, 0.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassOptics {
    /// Index of refraction of the glass at the green wavelength. `1.0` is air (no bending); real
    /// glass is about `1.5`. Values below `1.0` are clamped to `1.0`.
    pub ior: f32,
    /// Spread of the index of refraction between the blue and red ends of the spectrum (`n_blue -
    /// n_red`), clamped to `0.0..=0.5`. `0.0` disables chromatic dispersion. Physical crown glass is
    /// about `0.01`; UI-visible prism fringes need roughly `0.10..=0.25`.
    pub dispersion: f32,
    /// Width in logical pixels of the beveled rim that refracts. The shader limits it to 48% of the
    /// smaller half-extent, so small widgets keep a flat plateau.
    pub bevel_width: f32,
    /// Distance in logical pixels from the glass surface to the backdrop plane it refracts. `0.0`
    /// disables refraction. Values beyond the monotone limit (roughly one to two bevel widths) are
    /// clamped in the shader.
    pub thickness: f32,
    /// Steepness of the rim in `0.0..=1.0`, mapped to a maximum surface tilt of `sin θ = 0.30..=0.95`.
    /// The default is `0.45`.
    pub convexity: f32,
    /// Direction toward the key light in radians, in screen space where `y` grows downwards. The
    /// default is `-3π/4` (upper-left). The opposite rim receives the counter catchlight.
    pub light_angle: f32,
    /// Gain of the Fresnel rim glow and of the edge hairline; `0.0` removes them. The default is `1.0`.
    pub fresnel: f32,
    /// Width in logical pixels of the crisp catchlight on the bevel. The default is `3.5`.
    pub edge_width: f32,
}

impl GlassOptics {
    /// No refraction, no dispersion: flat frosted glass with a rim catchlight.
    pub const FLAT: Self = Self {
        ior: 1.5,
        dispersion: 0.0,
        bevel_width: 26.0,
        thickness: 0.0,
        convexity: 0.45,
        light_angle: LIGHT_UPPER_LEFT,
        fresnel: 1.0,
        edge_width: 3.5,
    };

    /// Liquid Glass lens: `ior` 1.5, a 26 px bevel, 48 px of refraction depth (the shader clamps it to
    /// the monotone limit) and `0.16` of dispersion.
    pub const LIQUID: Self = Self {
        ior: 1.5,
        dispersion: 0.16,
        bevel_width: 26.0,
        thickness: 48.0,
        convexity: 0.45,
        light_angle: LIGHT_UPPER_LEFT,
        fresnel: 1.0,
        edge_width: 3.5,
    };
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
/// boosted saturation, a visible specular sheen and moderate frost, with no
/// refraction ([`GlassOptics::FLAT`]). Use [`GlassQuad::liquid`] or set
/// [`Self::optics`] to [`GlassOptics::LIQUID`] for a refracting lens.
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
    /// Multiplier applied to the backdrop's perceptual chroma (OKLab): `0.0` is grayscale, `1.0`
    /// preserves the backdrop color, and values above `1.0` boost it with lightness-preserving gamut
    /// compression instead of channel clipping. The default is `1.35`.
    pub saturation: f32,
    /// Strength of the reflection band and of the bevel catchlights. `0.0` disables them. The default
    /// is `0.90`.
    pub specular: f32,
    /// Mix between the sharp and blurred backdrop in the flat body of the glass: `0.0` shows the sharp
    /// copy and `1.0` the blurred one. Frost falls to 15% across the beveled rim so refracted detail
    /// stays crisp there. Values below `0.30` also request an extra sharp-backdrop snapshot. The
    /// default is `0.45`.
    pub frost: f32,
    /// Amplitude of a smooth organic distortion applied to backdrop sampling, in logical pixels.
    /// `0.0` (the default) disables it; any value above `0.001` requests an extra sharp-backdrop
    /// snapshot. Values above `2.0` are clamped by the shader.
    pub liquid_ripple: f32,
    /// Angle in radians used to orient the conic rim arc. It only has an
    /// effect when [`Self::conic_strength`] is above `0.001`. The default is
    /// approximately `-1.309` radians.
    pub rim_angle: f32,
    /// Position of the conic sheen band, typically in `0.0..=1.0`. It sweeps
    /// the band across the surface and only has an effect when
    /// [`Self::conic_strength`] is above `0.001`. The default is `0.0`.
    pub sheen_progress: f32,
    /// Strength of the conic sheen and rim effect in `0.0..=1.0`. It cross-fades with the regular
    /// bevel catchlights: at `0.0` (the default) only the catchlights are used, at `1.0` only the
    /// conic sheen. Any value above `0.001` also requests an extra sharp-backdrop snapshot.
    pub conic_strength: f32,
    /// Lens optics: refraction depth, bevel, IOR, dispersion and light direction. The default is
    /// [`GlassOptics::FLAT`]. A [`GlassOptics::thickness`] above `0.001` requests an extra
    /// sharp-backdrop snapshot.
    pub optics: GlassOptics,
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
            optics: GlassOptics::FLAT,
        }
    }
}

impl GlassQuad {
    /// A Liquid Glass preset: a refracting, dispersing lens with little frost, a slow ripple and a
    /// strong catchlight. Set [`Self::rect`] and [`Self::radii`] with struct update syntax.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::{CornerRadii, Rect};
    /// use cadmium::renderer::GlassQuad;
    ///
    /// let card = GlassQuad {
    ///     rect: Rect::new(0.0, 0.0, 320.0, 200.0),
    ///     radii: CornerRadii::all(32.0),
    ///     ..GlassQuad::liquid()
    /// };
    /// assert!(card.optics.thickness > 0.0);
    /// ```
    pub fn liquid() -> Self {
        Self {
            frost: 0.12,
            liquid_ripple: 0.25,
            specular: 0.95,
            optics: GlassOptics::LIQUID,
            ..Self::default()
        }
    }

    /// Whether this quad needs the extra full-resolution backdrop snapshot: low frost, ripple, the
    /// conic effect, or any refraction all sample a backdrop that must stay sharp.
    pub(crate) fn needs_sharp_backdrop(&self) -> bool {
        self.frost < 0.30
            || self.liquid_ripple > 0.001
            || self.conic_strength > 0.001
            || self.optics.thickness > 0.001
    }

    /// Packs [`Self::optics`] into the two `vec4` slots of [`RawGlassInstance`]:
    /// `glass_lens = (ior, bevel_width, thickness, dispersion)` and
    /// `glass_light = (light_angle, fresnel, convexity, edge_width)`. Pixel quantities stay logical;
    /// the shader multiplies them by the frame scale.
    pub(crate) fn pack_optics(&self) -> ([f32; 4], [f32; 4]) {
        let o = &self.optics;
        (
            [
                o.ior.max(1.0),
                o.bevel_width.max(1.0),
                o.thickness.max(0.0),
                o.dispersion.clamp(0.0, 0.5),
            ],
            [
                o.light_angle,
                o.fresnel.max(0.0),
                o.convexity.clamp(0.0, 1.0),
                o.edge_width.max(0.25),
            ],
        )
    }
}

/// GPU instance record for one glass quad: 128 bytes, eight 16-byte slots, no padding.
///
/// Field order and offsets must match `GlassIn` in `glass.wgsl` and the attributes in
/// `create_glass_pipeline`. Keep your existing field names if they differ from the shader's; only the
/// two trailing `vec4` slots are new.
///
/// | location | field          | offset | format      |
/// |----------|----------------|--------|-------------|
/// | 0        | `rect`         | 0      | `Float32x4` |
/// | 1        | `radii`        | 16     | `Float32x4` |
/// | 2        | `tint_color`   | 32     | `Float32x4` |
/// | 3        | `border_color` | 48     | `Float32x4` |
/// | 4..=7    | `border_width`, `saturation`, `specular`, `frost` | 64, 68, 72, 76 | `Float32` |
/// | 8        | `glass_fx`     | 80     | `Float32x4` (ripple, rim angle, sheen progress, conic strength) |
/// | 9        | `glass_lens`   | 96     | `Float32x4` (ior, bevel, thickness, dispersion) |
/// | 10       | `glass_light`  | 112    | `Float32x4` (light angle, fresnel, convexity, edge width) |
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RawGlassInstance {
    pub(crate) rect: [f32; 4],
    pub(crate) radii: [f32; 4],
    pub(crate) tint_color: [f32; 4],
    pub(crate) border_color: [f32; 4],
    pub(crate) border_width: f32,
    pub(crate) saturation: f32,
    pub(crate) specular: f32,
    pub(crate) frost: f32,
    pub(crate) glass_fx: [f32; 4],
    pub(crate) glass_lens: [f32; 4],
    pub(crate) glass_light: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<RawGlassInstance>() == 128);
const _: () = assert!(std::mem::size_of::<RawGlassInstance>() % 16 == 0);

// ---- DrawList::push_glass: the only change (your colour/border packing stays exactly as it is) ----
//
//     let (glass_lens, glass_light) = quad.pack_optics();
//     self.has_liquid_glass |= quad.needs_sharp_backdrop();   // replaces the old frost/ripple/conic test
//     self.glass_instances.push(RawGlassInstance {
//         /* ...your existing fields... */
//         glass_lens,
//         glass_light,
//     });




// crates/cadmium/src/renderer/target.rs
//
// DROP-IN REPLACEMENT for the `WindowRenderTarget` block you pasted (struct + impl). Your `use` lines,
// `GlassOffscreen` and the helpers in pipelines.rs are untouched. Changes versus the original:
//   1. `resize` re-uploads the real frame scale. It used to write `scale: 1.0` while `self.scale` kept
//      the old value, so after any resize on a HiDPI window the shader ran with scale 1 (bevel, ripple
//      and catchlight widths collapsed) until the scale changed again.
//   2. The blur pyramid iterates `BLUR_PAIRS` times (1/2 -> 1/4 -> 1/2), reusing the existing param bind
//      groups, so no `GlassOffscreen` change is needed. Set it to 1 for the old pass count.
//   3. Glass quads mark their own rect dirty, so a glass quad drawn over another one re-blurs and
//      re-snapshots the backdrop instead of reusing a stale copy. The dirty test also pads the glass
//      rect by the blur/ripple reach.

/// Dual-Kawase ping-pong iterations (1/2 -> 1/4 -> 1/2 resolution) run after the initial
/// full -> 1/2 downsample. With the offsets from `blur_pyramid_uniforms` each pair adds roughly
/// 18 px² of variance (physical pixels), so 3 pairs give a sigma of about 7 px with no tap ghosting.
const BLUR_PAIRS: usize = 3;

/// Physical-pixel margin added around a glass rect when testing whether earlier draws invalidated
/// the backdrop: covers the blur footprint and the ripple displacement outside the rect.
const GLASS_REBLUR_MARGIN: f32 = 32.0;

/// Per-window GPU buffers and render targets used to draw a frame.
///
/// One target is created for each window by [`crate::window::App`]. It owns
/// the viewport uniform buffer, instance and geometry buffers whose
/// capacities grow on demand, and the offscreen targets required for glass
/// compositing. The target is reused between frames; call [`Self::resize`]
/// when the surface dimensions change.
pub struct WindowRenderTarget {
    width: u32,
    height: u32,
    scale: f32,
    glass_offscreen: Option<GlassOffscreen>,

    globals_buffer: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,

    sdf_buffer: wgpu::Buffer,
    sdf_capacity: usize,

    image_buffer: wgpu::Buffer,
    image_capacity: usize,

    glyph_buffer: wgpu::Buffer,
    glyph_capacity: usize,

    glass_buffer: wgpu::Buffer,
    glass_capacity: usize,

    path_vbuf: wgpu::Buffer,
    path_vcapacity: usize,
    path_ibuf: wgpu::Buffer,
    path_icapacity: usize,
}

impl WindowRenderTarget {
    /// Creates a target for a `width` × `height` physical-pixel surface.
    ///
    /// Zero dimensions are clamped to one. Initial buffer capacities are
    /// 128 SDF instances, 32 images, 512 glyphs, 16 glass quads, 256 path
    /// vertices and 512 path indices; each grows to the next power of two
    /// when a frame needs more. Offscreen glass targets are not allocated
    /// until a frame actually contains glass.
    pub fn new(gpu: &GpuContext, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        let globals = GlobalsUniform {
            viewport_size: [width as f32, height as f32],
            scale: 1.0,
            _pad: 0.0,
        };
        let globals_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SteelUI Globals"),
                contents: bytemuck::bytes_of(&globals),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let globals_bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SteelUI Globals BG"),
            layout: &gpu.globals_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let sdf_capacity = 128;
        let sdf_buffer = create_vertex_buffer::<RawSdfInstance>(&gpu.device, sdf_capacity, "SDF");

        let image_capacity = 32;
        let image_buffer =
            create_vertex_buffer::<RawImageInstance>(&gpu.device, image_capacity, "Image");

        let glyph_capacity = 512;
        let glyph_buffer =
            create_vertex_buffer::<RawGlyphInstance>(&gpu.device, glyph_capacity, "Glyph");

        let glass_capacity = 16;
        let glass_buffer =
            create_vertex_buffer::<RawGlassInstance>(&gpu.device, glass_capacity, "Glass");

        let path_vcapacity = 256;
        let path_vbuf = create_vertex_buffer::<PathVertex>(&gpu.device, path_vcapacity, "Path");
        let path_icapacity = 512;
        let path_ibuf = create_index_buffer(&gpu.device, path_icapacity, "Path Indices");

        Self {
            width,
            height,
            scale: 1.0,
            glass_offscreen: None,
            globals_buffer,
            globals_bg,
            sdf_buffer,
            sdf_capacity,
            image_buffer,
            image_capacity,
            glyph_buffer,
            glyph_capacity,
            glass_buffer,
            glass_capacity,
            path_vbuf,
            path_vcapacity,
            path_ibuf,
            path_icapacity,
        }
    }

    /// Updates the physical dimensions used for rendering and the viewport uniform.
    ///
    /// Zero dimensions are clamped to one; unchanged dimensions are a no-op.
    /// Existing offscreen glass targets are discarded because their size no
    /// longer matches the surface, and will be recreated on the next frame
    /// that needs them. The uniform keeps the current frame scale, so shader
    /// effects measured in logical pixels survive a resize. Call this
    /// whenever the surface is reconfigured.
    pub fn resize(&mut self, gpu: &GpuContext, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        self.glass_offscreen = None;

        let globals = GlobalsUniform {
            viewport_size: [width as f32, height as f32],
            scale: self.scale,
            _pad: 0.0,
        };
        gpu.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
    }

    /// Uploads `draw_list`, renders one frame into `surface_view`, and submits it.
    ///
    /// Pipelines are created on demand for `surface_format`, instance and
    /// geometry buffers grow as needed, and the viewport uniform is updated
    /// when the draw list's scale differs from the target's current scale.
    /// The frame is cleared to `clear_color`. If no glass quads are present,
    /// all primitives are drawn directly to `surface_view`; otherwise the
    /// UI is drawn offscreen, the glass backdrop is blurred, and the result
    /// is blitted to `surface_view`. All commands are submitted to
    /// `gpu.queue` before this method returns.
    ///
    /// Glass quads are composited in draw order. The backdrop snapshot and
    /// blur pyramid are rebuilt only when a primitive or an earlier glass
    /// quad touched the area the next glass quad samples, so glass can
    /// refract and blur other glass.
    ///
    /// Returns `true` when glyph rasterization had to skip glyphs because
    /// the atlas is full at its maximum size. Schedule another frame in that
    /// case so the enlarged atlas can be used; [`crate::window::App`] does
    /// this automatically.
    ///
    /// `surface_format` must be the format the surface was configured with,
    /// `surface_view` must be a render-attachment view of that surface, and
    /// `draw_list` must have been recorded with the same [`GpuContext`].
    ///
    /// # Panics
    ///
    /// Panics if `wgpu` rejects the resources or format (for example, an
    /// incompatible `surface_view`), or if required pipelines are missing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::core::Color;
    /// use cadmium::renderer::{DrawList, GpuContext, WindowRenderTarget};
    ///
    /// # fn draw(
    /// #     gpu: &mut GpuContext,
    /// #     target: &mut WindowRenderTarget,
    /// #     view: &wgpu::TextureView,
    /// #     format: wgpu::TextureFormat,
    /// #     frame: &DrawList,
    /// # ) -> bool {
    /// let atlas_overflowed = target.render(
    ///     gpu,
    ///     view,
    ///     format,
    ///     Color::rgba(0.0, 0.0, 0.0, 1.0),
    ///     frame,
    /// );
    /// # atlas_overflowed
    /// # }
    /// ```
    pub fn render(
        &mut self,
        gpu: &mut GpuContext,
        surface_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        clear_color: Color,
        draw_list: &DrawList,
    ) -> bool {
        if (self.scale - draw_list.scale).abs() > f32::EPSILON {
            self.scale = draw_list.scale;
            let globals = GlobalsUniform {
                viewport_size: [self.width as f32, self.height as f32],
                scale: self.scale,
                _pad: 0.0,
            };
            gpu.queue
                .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        }

        gpu.ensure_base_pipelines(surface_format);
        if !draw_list.image_instances.is_empty() {
            gpu.ensure_image_pipeline(surface_format);
        }
        let needs_glass = !draw_list.glass_instances.is_empty();
        if needs_glass {
            gpu.ensure_glass_pipelines(surface_format);
        }

        if draw_list.sdf_instances.len() > self.sdf_capacity {
            let new_cap = draw_list.sdf_instances.len().next_power_of_two();
            self.sdf_buffer = create_vertex_buffer::<RawSdfInstance>(&gpu.device, new_cap, "SDF");
            self.sdf_capacity = new_cap;
        }
        if !draw_list.sdf_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.sdf_buffer,
                0,
                bytemuck::cast_slice(&draw_list.sdf_instances),
            );
        }

        if draw_list.image_instances.len() > self.image_capacity {
            let new_cap = draw_list.image_instances.len().next_power_of_two();
            self.image_buffer =
                create_vertex_buffer::<RawImageInstance>(&gpu.device, new_cap, "Image");
            self.image_capacity = new_cap;
        }
        if !draw_list.image_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.image_buffer,
                0,
                bytemuck::cast_slice(&draw_list.image_instances),
            );
        }

        if draw_list.glyph_instances.len() > self.glyph_capacity {
            let new_cap = draw_list.glyph_instances.len().next_power_of_two();
            self.glyph_buffer =
                create_vertex_buffer::<RawGlyphInstance>(&gpu.device, new_cap, "Glyph");
            self.glyph_capacity = new_cap;
        }
        if !draw_list.glyph_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.glyph_buffer,
                0,
                bytemuck::cast_slice(&draw_list.glyph_instances),
            );
        }

        if draw_list.glass_instances.len() > self.glass_capacity {
            let new_cap = draw_list.glass_instances.len().next_power_of_two();
            self.glass_buffer =
                create_vertex_buffer::<RawGlassInstance>(&gpu.device, new_cap, "Glass");
            self.glass_capacity = new_cap;
        }
        if !draw_list.glass_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.glass_buffer,
                0,
                bytemuck::cast_slice(&draw_list.glass_instances),
            );
        }

        if draw_list.path_vertices.len() > self.path_vcapacity {
            let new_cap = draw_list.path_vertices.len().next_power_of_two();
            self.path_vbuf = create_vertex_buffer::<PathVertex>(&gpu.device, new_cap, "Path");
            self.path_vcapacity = new_cap;
        }
        if !draw_list.path_vertices.is_empty() {
            gpu.queue.write_buffer(
                &self.path_vbuf,
                0,
                bytemuck::cast_slice(&draw_list.path_vertices),
            );
        }
        if draw_list.path_indices.len() > self.path_icapacity {
            let new_cap = draw_list.path_indices.len().next_power_of_two();
            self.path_ibuf = create_index_buffer(&gpu.device, new_cap, "Path Indices");
            self.path_icapacity = new_cap;
        }
        if !draw_list.path_indices.is_empty() {
            gpu.queue.write_buffer(
                &self.path_ibuf,
                0,
                bytemuck::cast_slice(&draw_list.path_indices),
            );
        }

        if !needs_glass {
            self.glass_offscreen = None;
            return self.render_direct(gpu, surface_view, surface_format, clear_color, draw_list);
        }

        if self
            .glass_offscreen
            .as_ref()
            .is_some_and(|g| g.format != surface_format)
        {
            self.glass_offscreen = None;
        }
        let width = self.width;
        let height = self.height;
        let bufs = BatchBuffers {
            width,
            height,
            globals_bg: &self.globals_bg,
            sdf_buffer: &self.sdf_buffer,
            image_buffer: &self.image_buffer,
            glyph_buffer: &self.glyph_buffer,
            path_vbuf: &self.path_vbuf,
            path_ibuf: &self.path_ibuf,
        };
        let go = self
            .glass_offscreen
            .get_or_insert_with(|| GlassOffscreen::new(gpu, width, height, surface_format));
        if draw_list.has_liquid_glass {
            go.ensure_liquid_backdrop(gpu, width, height);
        } else {
            go.liquid_backdrop = None;
        }

        let pipes = &gpu.pipelines[&surface_format];
        let blur_pipe = pipes.blur.as_ref().expect("blur pipeline");
        let glass_pipe = pipes.glass.as_ref().expect("glass pipeline");
        let blit_pipe = pipes.blit.as_ref().expect("blit pipeline");

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SteelUI Glass Frame Encoder"),
            });

        let mut is_first_pass = true;
        let mut active_scissor: Option<Rect> = None;
        let mut dirty_backdrop: Option<Rect> = None;
        let mut has_blur_pyramid = false;
        let mut i = 0;

        while i < draw_list.batches.len() {
            match &draw_list.batches[i] {
                DrawBatch::SetScissor(sc) => {
                    active_scissor = *sc;
                    i += 1;
                }
                DrawBatch::Glass(idx) => {
                    let Some(glass) = draw_list.glass_instances.get(*idx as usize) else {
                        i += 1;
                        continue;
                    };
                    let glass_rect = glass.rect;
                    if !scissor_intersects_rect(
                        active_scissor,
                        glass_rect,
                        draw_list.scale,
                        width,
                        height,
                    ) {
                        i += 1;
                        continue;
                    }

                    if is_first_pass {
                        let _ = begin_ui_pass(
                            &mut encoder,
                            &go.canonical_view,
                            wgpu::LoadOp::Clear(clear_color.to_wgpu()),
                        );
                        is_first_pass = false;
                    }

                    // The glass reads the backdrop a blur radius (and a ripple amplitude) beyond its own
                    // rect, so the staleness probe is padded.
                    let probe = [
                        glass_rect[0] - GLASS_REBLUR_MARGIN,
                        glass_rect[1] - GLASS_REBLUR_MARGIN,
                        glass_rect[2] + GLASS_REBLUR_MARGIN * 2.0,
                        glass_rect[3] + GLASS_REBLUR_MARGIN * 2.0,
                    ];
                    let need_reblur = !has_blur_pyramid
                        || dirty_backdrop.is_some_and(|d| phys_rects_intersect(d, probe));

                    if need_reblur {
                        if let Some((backdrop_tex, _)) = &go.liquid_backdrop {
                            encoder.copy_texture_to_texture(
                                wgpu::TexelCopyTextureInfo {
                                    texture: &go.canonical_tex,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                wgpu::TexelCopyTextureInfo {
                                    texture: backdrop_tex,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                wgpu::Extent3d {
                                    width,
                                    height,
                                    depth_or_array_layers: 1,
                                },
                            );
                        }

                        // full -> 1/2, then BLUR_PAIRS x (1/2 -> 1/4 -> 1/2). Pass 1 and pass 2 use the
                        // same param bind groups every iteration; the result always lands in mip1.
                        run_blur_pass(
                            &mut encoder,
                            blur_pipe,
                            &go.canonical_linear_bg,
                            &go.blur_param_bgs[0],
                            &go.mip1_view,
                        );
                        for _ in 0..BLUR_PAIRS {
                            run_blur_pass(
                                &mut encoder,
                                blur_pipe,
                                &go.mip1_bg,
                                &go.blur_param_bgs[1],
                                &go.mip2_view,
                            );
                            run_blur_pass(
                                &mut encoder,
                                blur_pipe,
                                &go.mip2_bg,
                                &go.blur_param_bgs[2],
                                &go.mip1_view,
                            );
                        }
                        has_blur_pyramid = true;
                        dirty_backdrop = None;
                    }

                    {
                        let mut pass =
                            begin_ui_pass(&mut encoder, &go.canonical_view, wgpu::LoadOp::Load);
                        if apply_scissor(&mut pass, active_scissor, draw_list.scale, width, height)
                        {
                            let sharp_bg = go
                                .liquid_backdrop
                                .as_ref()
                                .map(|(_, bg)| bg)
                                .unwrap_or(&go.mip1_bg);
                            pass.set_pipeline(glass_pipe);
                            pass.set_bind_group(0, &self.globals_bg, &[]);
                            pass.set_bind_group(1, sharp_bg, &[]);
                            pass.set_bind_group(2, &go.mip1_bg, &[]);
                            pass.set_vertex_buffer(0, self.glass_buffer.slice(..));
                            pass.draw(0..4, *idx..(*idx + 1));
                        }
                    }

                    // The glass just wrote into the canonical target; any later glass overlapping it must
                    // snapshot and blur the result, not the stale pre-glass copy.
                    dirty_backdrop = union_opt_rect(
                        dirty_backdrop,
                        Rect::new(
                            glass_rect[0] - 1.0,
                            glass_rect[1] - 1.0,
                            glass_rect[2] + 2.0,
                            glass_rect[3] + 2.0,
                        ),
                    );
                    i += 1;
                }
                DrawBatch::Sdf(_)
                | DrawBatch::Image { .. }
                | DrawBatch::Text(_)
                | DrawBatch::Path { .. } => {
                    let load = if is_first_pass {
                        is_first_pass = false;
                        wgpu::LoadOp::Clear(clear_color.to_wgpu())
                    } else {
                        wgpu::LoadOp::Load
                    };

                    let start_i = i;
                    {
                        let mut pass = begin_ui_pass(&mut encoder, &go.canonical_view, load);
                        i = record_standard_batches(
                            &mut pass,
                            pipes,
                            gpu,
                            &bufs,
                            draw_list,
                            i,
                            &mut active_scissor,
                        );
                    }
                    for b in &draw_list.batches[start_i..i] {
                        if let Some(r) = batch_phys_bounds(draw_list, b, width, height) {
                            dirty_backdrop = union_opt_rect(dirty_backdrop, r);
                        }
                    }
                }
            }
        }

        if is_first_pass {
            let _ = begin_ui_pass(
                &mut encoder,
                &go.canonical_view,
                wgpu::LoadOp::Clear(clear_color.to_wgpu()),
            );
        }

        {
            let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SteelUI Final Blit Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            blit_pass.set_pipeline(blit_pipe);
            blit_pass.set_bind_group(0, &go.canonical_blit_bg, &[]);
            blit_pass.draw(0..3, 0..1);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        draw_list.atlas_overflowed
    }

    fn render_direct(
        &self,
        gpu: &GpuContext,
        surface_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        clear_color: Color,
        draw_list: &DrawList,
    ) -> bool {
        let pipes = &gpu.pipelines[&surface_format];
        let bufs = BatchBuffers {
            width: self.width,
            height: self.height,
            globals_bg: &self.globals_bg,
            sdf_buffer: &self.sdf_buffer,
            image_buffer: &self.image_buffer,
            glyph_buffer: &self.glyph_buffer,
            path_vbuf: &self.path_vbuf,
            path_ibuf: &self.path_ibuf,
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SteelUI Direct Frame Encoder"),
            });

        {
            let mut pass = begin_ui_pass(
                &mut encoder,
                surface_view,
                wgpu::LoadOp::Clear(clear_color.to_wgpu()),
            );
            let mut active_scissor: Option<Rect> = None;
            let _ = record_standard_batches(
                &mut pass,
                pipes,
                gpu,
                &bufs,
                draw_list,
                0,
                &mut active_scissor,
            );
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        draw_list.atlas_overflowed
    }
}




use crate::core::Color;
use crate::renderer::GlassOptics;

/// Parameters for a drop shadow behind a widget surface.
///
/// Used by every variant of [`Material`]. The blur radius and offset are in logical
/// pixels; the widget renderer scales both by the current device-pixel ratio before
/// submitting the shadow to [`crate::renderer`].
///
/// # Examples
///
/// ```
/// use cadmium::core::Color;
/// use cadmium::style::Shadow;
///
/// let shadow = Shadow::new(Color::from_rgba8(0, 0, 0, 60), 12.0, 4.0);
/// assert_eq!(shadow.blur, 12.0);
/// assert_eq!(shadow.offset, [0.0, 4.0]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Shadow color, including alpha. A fully transparent color disables the visible
    /// shadow even when the other fields are non-zero.
    pub color: Color,
    /// Gaussian blur radius in logical pixels; `0.0` gives a hard-edged shadow.
    pub blur: f32,
    /// Shadow displacement in logical pixels as `[x, y]`. Positive `y` moves the shadow
    /// downward, matching a light source above the surface.
    pub offset: [f32; 2],
}

impl Shadow {
    /// Creates a shadow with no horizontal offset.
    ///
    /// `color` is the shadow color, `blur` is the logical-pixel blur radius, and
    /// `offset_y` is the downward logical-pixel displacement.
    pub const fn new(color: Color, blur: f32, offset_y: f32) -> Self {
        Self {
            color,
            blur,
            offset: [0.0, offset_y],
        }
    }
}

/// The surface treatment painted behind a widget.
///
/// Each variant corresponds to a different rendering path:
///
/// - [`Material::Classic`] produces a signed-distance-field surface
///   ([`crate::renderer::SdfQuad`]) with a flat or gradient fill, a hairline border, and
///   a Gaussian drop shadow.
/// - [`Material::Frosted`] produces a [`crate::renderer::GlassQuad`] that blurs and tints
///   whatever is behind the widget, with an optional shadow pass underneath.
/// - [`Material::Liquid`] produces the same glass quad with a refracting lens: Snell's-law
///   bending across a convex bevel, wavelength-dependent chromatic dispersion, ripple
///   distortion, Fresnel/catchlight lighting and an optional animated conic sheen. The lens is
///   described by [`GlassOptics`].
///
/// The helper constructors [`Material::solid`], [`Material::frosted`], and
/// [`Material::liquid`] provide sensible presets; the struct variants allow every field
/// to be tuned. Border widths and shadow dimensions are logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    /// A crisp, analytically drawn surface: a solid color or a two-stop linear gradient,
    /// a thin border, and an optional Gaussian shadow. Best for ordinary controls and
    /// panels that should not sample the backdrop.
    Classic {
        /// Fill color at the start of the gradient, or the entire fill when `fill_end`
        /// is `None`.
        fill: Color,
        /// Optional second gradient stop. `None` gives a flat fill; `Some(color)` blends
        /// from `fill` to this color along `gradient_dir`.
        fill_end: Option<Color>,
        /// Gradient direction as an `[x, y]` vector in the surface's local coordinates.
        /// `[0.0, 1.0]` (the preset used by [`Material::solid`]) runs from top to bottom;
        /// only meaningful when `fill_end` is `Some`.
        gradient_dir: [f32; 2],
        /// Border color, including alpha. A transparent color draws no visible border.
        border_color: Color,
        /// Border thickness in logical pixels. The renderer scales it by the device-pixel
        /// ratio; `0.0` draws no border.
        border_width: f32,
        /// Optional drop shadow drawn behind the surface. `None` disables it.
        shadow: Option<Shadow>,
    },
    /// Translucent "frosted glass": a blurred, tinted copy of the backdrop with a soft
    /// edge sheen and a thin highlight border. An optional shadow pass is drawn before
    /// the glass so the surface appears to float above its surroundings.
    Frosted {
        /// Tint color. Its alpha controls how strongly the tint replaces the sampled
        /// backdrop: `0.0` leaves the backdrop untouched, `1.0` makes the surface opaque.
        tint: Color,
        /// Blend factor between the sharp and blurred backdrop, typically `0.0`–`1.0`.
        /// The shader reduces it near the bevel so the rim stays clearer than the center.
        frost: f32,
        /// Strength of the curved edge highlight; `0.0` removes the sheen.
        specular: f32,
        /// Color of the one-pixel glass edge highlight, including alpha.
        border_color: Color,
        /// Border thickness in logical pixels, scaled by the device-pixel ratio when drawn.
        border_width: f32,
        /// Optional drop shadow drawn behind the glass. `None` disables it.
        shadow: Option<Shadow>,
    },
    /// "Liquid glass": a convex optical bezel that refracts the backdrop with Snell's law, splits red,
    /// green, and blue by wavelength-dependent index of refraction, adds a slow organic ripple, and
    /// can sweep a conic sheen around the rim. An optional shadow pass is drawn first.
    Liquid {
        /// Tint color blended over the refracted backdrop; alpha controls tint strength.
        tint: Color,
        /// Blend factor between sharp and blurred backdrop in the flat center. The lens
        /// bevel reduces this near the rim to keep refracted detail crisp.
        frost: f32,
        /// Amplitude of the organic wave distortion applied to the backdrop, in logical
        /// pixels; `0.0` disables the ripple.
        ripple: f32,
        /// Strength of the bevel catchlights. Cross-fades with the conic sheen as
        /// `conic_strength` rises.
        specular: f32,
        /// Angle in radians of the dark arc around the bezel rim; `-1.309` is the preset
        /// used by [`Material::liquid`].
        rim_angle: f32,
        /// Progress of the conic sheen band across the surface. Animate this from `0.0`
        /// to `1.0` to sweep the highlight; it has no effect while `conic_strength` is zero.
        sheen_progress: f32,
        /// Strength of the conic sheen and refraction ring. At `0.0` the shader uses the
        /// bevel catchlights instead.
        conic_strength: f32,
        /// Lens optics: index of refraction, dispersion, bevel width, refraction depth,
        /// convexity and light direction. [`GlassOptics::LIQUID`] is the preset used by
        /// [`Material::liquid`]; [`GlassOptics::FLAT`] turns the lens off.
        optics: GlassOptics,
        /// Color of the glass edge highlight, including alpha.
        border_color: Color,
        /// Border thickness in logical pixels, scaled by the device-pixel ratio when drawn.
        border_width: f32,
        /// Optional drop shadow drawn behind the glass. `None` disables it.
        shadow: Option<Shadow>,
    },
}

impl Material {
    /// Creates a flat [`Material::Classic`] surface with no border or shadow.
    ///
    /// The gradient direction defaults to `[0.0, 1.0]` and `fill_end` is `None`, so the
    /// surface is a single solid `color`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::style::Material;
    ///
    /// let material = Material::solid(Color::from_rgba8(0, 122, 255, 255));
    /// match material {
    ///     Material::Classic { fill, fill_end, border_width, shadow, .. } => {
    ///         assert_eq!(fill, Color::from_rgba8(0, 122, 255, 255));
    ///         assert_eq!(fill_end, None);
    ///         assert_eq!(border_width, 0.0);
    ///         assert_eq!(shadow, None);
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn solid(color: Color) -> Self {
        Self::Classic {
            fill: color,
            fill_end: None,
            gradient_dir: [0.0, 1.0],
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            shadow: None,
        }
    }

    /// Creates a [`Material::Frosted`] glass surface with a balanced preset.
    ///
    /// Uses `frost: 0.45`, `specular: 0.85`, a one-logical-pixel translucent white border
    /// (alpha `45`), and a soft black shadow (alpha `48`, blur `16.0`, vertical offset
    /// `6.0`). `tint` is the color blended over the blurred backdrop.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::style::Material;
    ///
    /// let material = Material::frosted(Color::from_rgba8(255, 255, 255, 130));
    /// match material {
    ///     Material::Frosted { frost, border_width, shadow, .. } => {
    ///         assert_eq!(frost, 0.45);
    ///         assert_eq!(border_width, 1.0);
    ///         assert!(shadow.is_some());
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn frosted(tint: Color) -> Self {
        Self::Frosted {
            tint,
            frost: 0.45,
            specular: 0.85,
            border_color: Color::from_rgba8(255, 255, 255, 45),
            border_width: 1.0,
            shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 48), 16.0, 6.0)),
        }
    }

    /// Creates a [`Material::Liquid`] glass surface with subtle lens effects.
    ///
    /// Uses `frost: 0.12`, `ripple: 0.25`, `specular: 0.95`, `rim_angle: -1.309`,
    /// `sheen_progress: 0.0`, and `conic_strength: 0.0` (so the initial look is the
    /// bevel catchlights), the [`GlassOptics::LIQUID`] lens, plus a one-logical-pixel translucent
    /// white border (alpha `65`) and a soft black shadow (alpha `52`, blur `18.0`, vertical
    /// offset `8.0`).
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::style::Material;
    ///
    /// let material = Material::liquid(Color::from_rgba8(255, 255, 255, 40));
    /// match material {
    ///     Material::Liquid { frost, ripple, conic_strength, optics, .. } => {
    ///         assert_eq!(frost, 0.12);
    ///         assert_eq!(ripple, 0.25);
    ///         assert_eq!(conic_strength, 0.0);
    ///         assert!(optics.thickness > 0.0);
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn liquid(tint: Color) -> Self {
        Self::Liquid {
            tint,
            frost: 0.12,
            ripple: 0.25,
            specular: 0.95,
            rim_angle: -1.309,
            sheen_progress: 0.0,
            conic_strength: 0.0,
            optics: GlassOptics::LIQUID,
            border_color: Color::from_rgba8(255, 255, 255, 65),
            border_width: 1.0,
            shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 52), 18.0, 8.0)),
        }
    }

    /// Replaces the lens optics of a [`Material::Liquid`] surface.
    ///
    /// The other variants have no lens and are returned unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::renderer::GlassOptics;
    /// use cadmium::style::Material;
    ///
    /// let lens = GlassOptics { dispersion: 0.30, ..GlassOptics::LIQUID };
    /// let material = Material::liquid(Color::from_rgba8(255, 255, 255, 40)).with_optics(lens);
    /// match material {
    ///     Material::Liquid { optics, .. } => assert_eq!(optics.dispersion, 0.30),
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn with_optics(mut self, lens: GlassOptics) -> Self {
        if let Self::Liquid { optics, .. } = &mut self {
            *optics = lens;
        }
        self
    }
}