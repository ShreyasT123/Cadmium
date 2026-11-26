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
/// - [`Material::Liquid`] produces the same glass quad with lens-like edge refraction,
///   chromatic dispersion, ripple distortion, and an optional animated conic sheen.
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
    /// "Liquid glass": a convex optical bezel that refracts the backdrop, splits red,
    /// green, and blue samples for chromatic dispersion, adds a slow organic ripple, and
    /// can sweep a conic sheen around the rim. An optional shadow pass is drawn first.
    Liquid {
        /// Tint color blended over the refracted backdrop; alpha controls tint strength.
        tint: Color,
        /// Blend factor between sharp and blurred backdrop in the flat center. The lens
        /// bevel reduces this near the rim to keep refracted detail crisp.
        frost: f32,
        /// Amplitude of the organic wave distortion applied to the backdrop; `0.0`
        /// disables the ripple.
        ripple: f32,
        /// Strength of the crisp specular highlight on the bezel. Used when the conic
        /// sheen is disabled (see `conic_strength`).
        specular: f32,
        /// Angle in radians of the dark arc around the bezel rim; `-1.309` is the preset
        /// used by [`Material::liquid`].
        rim_angle: f32,
        /// Progress of the conic sheen band across the surface. Animate this from `0.0`
        /// to `1.0` to sweep the highlight; it has no effect while `conic_strength` is zero.
        sheen_progress: f32,
        /// Strength of the conic sheen and refraction ring. At `0.0` the shader uses the
        /// simpler specular-bevel path instead.
        conic_strength: f32,
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
    /// specular bevel), plus a one-logical-pixel translucent white border (alpha `65`)
    /// and a soft black shadow (alpha `52`, blur `18.0`, vertical offset `8.0`).
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Color;
    /// use cadmium::style::Material;
    ///
    /// let material = Material::liquid(Color::from_rgba8(255, 255, 255, 40));
    /// match material {
    ///     Material::Liquid { frost, ripple, conic_strength, .. } => {
    ///         assert_eq!(frost, 0.12);
    ///         assert_eq!(ripple, 0.25);
    ///         assert_eq!(conic_strength, 0.0);
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
            border_color: Color::from_rgba8(255, 255, 255, 65),
            border_width: 1.0,
            shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 52), 18.0, 8.0)),
        }
    }
}

use crate::core::Color;

