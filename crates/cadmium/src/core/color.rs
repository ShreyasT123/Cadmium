/// A linear-RGBA color used for fills, borders, text, tints, and clear values.
///
/// The `r`, `g`, and `b` fields are linear-light components; `a` is straight
/// alpha and is not premultiplied into the color channels. Components are
/// normally in `0.0..=1.0`, but this is plain data and does not clamp them.
///
/// Use [`Self::from_rgba8`] for palette values specified as sRGB bytes, or
/// [`Self::rgba`] when the components are already linear. [`Self::to_array`]
/// supplies shader uniforms, while [`Self::to_wgpu`] produces `wgpu` clear
/// colors.
///
/// # Examples
///
/// ```rust
/// use cadmium::core::Color;
///
/// let linear = Color::rgba(0.5, 0.25, 0.0, 1.0);
/// assert_eq!(linear.to_array(), [0.5, 0.25, 0.0, 1.0]);
///
/// let from_bytes = Color::from_rgba8(255, 255, 255, 255);
/// assert_eq!(from_bytes.r, 1.0);
/// assert_eq!(from_bytes.a, 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Linear-light red component, typically `0.0` (no red) to `1.0` (full red).
    pub r: f32,
    /// Linear-light green component, typically `0.0` (no green) to `1.0` (full green).
    pub g: f32,
    /// Linear-light blue component, typically `0.0` (no blue) to `1.0` (full blue).
    pub b: f32,
    /// Straight alpha: `0.0` is fully transparent and `1.0` is fully opaque.
    ///
    /// RGB components are not premultiplied by this value.
    pub a: f32,
}

impl Color {
    /// Fully transparent black.
    ///
    /// Useful as a neutral default for fills, borders, and shadows when no
    /// color should be drawn.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::Color;
    ///
    /// assert_eq!(Color::TRANSPARENT.to_array(), [0.0, 0.0, 0.0, 0.0]);
    /// ```
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Creates a color from already-linear RGBA components.
    ///
    /// Unlike [`Self::from_rgba8`], no sRGB decoding is applied. Pass normalized
    /// `0.0..=1.0` values; the function is `const` and does not clamp its inputs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::Color;
    ///
    /// const ACCENT: Color = Color::rgba(0.0, 0.5, 1.0, 1.0);
    /// assert_eq!(ACCENT.b, 1.0);
    /// assert_eq!(ACCENT.a, 1.0);
    /// ```
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a color from 8-bit sRGB bytes and a straight alpha byte.
    ///
    /// Each RGB byte is divided by `255` and converted from sRGB to linear light
    /// using the standard piecewise transfer function. Alpha is divided by `255`
    /// and left linear, so the result is straight RGBA. Use this for colors
    /// expressed as CSS hex values or theme palette bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::Color;
    ///
    /// let black = Color::from_rgba8(0, 0, 0, 255);
    /// assert_eq!(black.r, 0.0);
    /// assert_eq!(black.a, 1.0);
    ///
    /// let blue = Color::from_rgba8(0, 0, 255, 128);
    /// assert_eq!(blue.b, 1.0);
    /// assert!((blue.a - 128.0 / 255.0).abs() < 1e-6);
    /// ```
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: srgb_to_linear(r as f32 / 255.0),
            g: srgb_to_linear(g as f32 / 255.0),
            b: srgb_to_linear(b as f32 / 255.0),
            a: a as f32 / 255.0,
        }
    }

    /// Returns the color as `[r, g, b, a]`.
    ///
    /// This is the component order expected by Cadmium's shader uniforms and
    /// text vertices. No color-space conversion occurs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::Color;
    ///
    /// let color = Color::rgba(0.25, 0.5, 0.75, 1.0);
    /// assert_eq!(color.to_array(), [0.25, 0.5, 0.75, 1.0]);
    /// ```
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Converts this color to a `wgpu::Color` for surface clear values.
    ///
    /// The `f32` components are widened to `f64`; no color-space conversion is
    /// performed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::Color;
    ///
    /// let clear = Color::rgba(0.5, 0.25, 0.0, 1.0).to_wgpu();
    /// assert_eq!(clear.r, 0.5);
    /// assert_eq!(clear.g, 0.25);
    /// assert_eq!(clear.a, 1.0);
    /// ```
    pub fn to_wgpu(self) -> wgpu::Color {
        wgpu::Color {
            r: self.r as f64,
            g: self.g as f64,
            b: self.b as f64,
            a: self.a as f64,
        }
    }
}

/// Applies the sRGB electro-optical transfer function to a normalized channel.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

