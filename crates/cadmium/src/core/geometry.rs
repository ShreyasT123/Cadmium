/// An axis-aligned rectangle in logical pixels.
///
/// `x` and `y` locate the top-left corner; `width` and `height` extend right and
/// down. [`crate::layout`] produces rectangles in this space, [`crate::widgets`]
/// uses [`Self::contains`] for hit testing, and [`crate::renderer`] multiplies
/// the values by the device-pixel ratio before drawing.
///
/// [`Self::contains`] treats the right and bottom edges as inside the rectangle.
///
/// # Examples
///
/// ```rust
/// use cadmium::core::Rect;
///
/// let rect = Rect {
///     x: 10.0,
///     y: 20.0,
///     width: 100.0,
///     height: 50.0,
/// };
/// assert_eq!(rect, Rect::new(10.0, 20.0, 100.0, 50.0));
/// assert!(rect.contains(110.0, 70.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge in logical pixels, measured from the top-left origin.
    pub x: f32,
    /// Top edge in logical pixels, measured from the top-left origin.
    pub y: f32,
    /// Width in logical pixels, extending to the right of `x`.
    pub width: f32,
    /// Height in logical pixels, extending below `y`.
    pub height: f32,
}

impl Rect {
    /// Creates a rectangle from a top-left position and a size in logical pixels.
    ///
    /// The function is `const`, so it can define static layouts and theme
    /// constants. No validation or rounding is performed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::Rect;
    ///
    /// const CARD: Rect = Rect::new(8.0, 8.0, 120.0, 40.0);
    /// assert_eq!(CARD.x, 8.0);
    /// assert_eq!(CARD.width, 120.0);
    /// ```
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Reports whether a logical-pixel point lies inside this rectangle.
    ///
    /// The left and top edges are included, and the right edge at `x + width`
    /// and bottom edge at `y + height` are also included.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::Rect;
    ///
    /// let hit = Rect::new(10.0, 20.0, 100.0, 50.0);
    /// assert!(hit.contains(10.0, 20.0));
    /// assert!(hit.contains(110.0, 70.0));
    /// assert!(!hit.contains(9.9, 20.0));
    /// assert!(!hit.contains(50.0, 70.1));
    /// ```
    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    /// Multiplies the position and size by `factor`.
    ///
    /// Because all four fields are scaled, this maps a logical-pixel rectangle
    /// into the same coordinate system at another scale, such as device pixels
    /// for a given DPI. The result is not rounded or clipped.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::Rect;
    ///
    /// let logical = Rect::new(10.0, 20.0, 100.0, 50.0);
    /// assert_eq!(logical.scale(2.0), Rect::new(20.0, 40.0, 200.0, 100.0));
    /// ```
    pub fn scale(self, factor: f32) -> Self {
        Self {
            x: self.x * factor,
            y: self.y * factor,
            width: self.width * factor,
            height: self.height * factor,
        }
    }
}

/// Per-corner rounding radii in logical pixels.
///
/// The fields are ordered clockwise from the top-left: top-left, top-right,
/// bottom-right, bottom-left. [`Self::to_array`] preserves that order for
/// shader uniforms, and [`Self::scale`] scales every corner uniformly. The
/// renderer clamps a radius to half the corresponding box side when it records
/// a rounded primitive.
///
/// # Examples
///
/// ```rust
/// use cadmium::core::CornerRadii;
///
/// let pill = CornerRadii::all(16.0);
/// assert_eq!(pill.to_array(), [16.0, 16.0, 16.0, 16.0]);
/// assert_eq!(CornerRadii::ZERO, CornerRadii::all(0.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CornerRadii {
    /// Rounding radius of the top-left corner in logical pixels.
    pub top_left: f32,
    /// Rounding radius of the top-right corner in logical pixels.
    pub top_right: f32,
    /// Rounding radius of the bottom-right corner in logical pixels.
    pub bottom_right: f32,
    /// Rounding radius of the bottom-left corner in logical pixels.
    pub bottom_left: f32,
}

impl CornerRadii {
    /// No rounding on any corner.
    ///
    /// This is the identity value for rounded-rectangle geometry and the default
    /// used by renderer primitives.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::CornerRadii;
    ///
    /// assert_eq!(CornerRadii::ZERO.to_array(), [0.0, 0.0, 0.0, 0.0]);
    /// ```
    pub const ZERO: Self = Self::all(0.0);

    /// Creates radii with the same value on all four corners.
    ///
    /// The function is `const` and does not clamp the radius; rendering clamps
    /// each corner to the size of its box.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::CornerRadii;
    ///
    /// const CARD: CornerRadii = CornerRadii::all(12.0);
    /// assert_eq!(CARD.top_left, 12.0);
    /// assert_eq!(CARD.bottom_right, 12.0);
    /// ```
    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    /// Multiplies every corner radius by `factor`.
    ///
    /// Use this to convert logical-pixel radii to device pixels alongside
    /// [`crate::core::Rect::scale`]. The result is not rounded or clamped.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::CornerRadii;
    ///
    /// let logical = CornerRadii::all(9.0);
    /// assert_eq!(
    ///     logical.scale(2.0).to_array(),
    ///     [18.0, 18.0, 18.0, 18.0],
    /// );
    /// ```
    pub fn scale(self, factor: f32) -> Self {
        Self {
            top_left: self.top_left * factor,
            top_right: self.top_right * factor,
            bottom_right: self.bottom_right * factor,
            bottom_left: self.bottom_left * factor,
        }
    }

    /// Returns the radii as `[top_left, top_right, bottom_right, bottom_left]`.
    ///
    /// This clockwise order matches the layout of the renderer's WGSL uniforms.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::core::CornerRadii;
    ///
    /// let radii = CornerRadii {
    ///     top_left: 1.0,
    ///     top_right: 2.0,
    ///     bottom_right: 3.0,
    ///     bottom_left: 4.0,
    /// };
    /// assert_eq!(radii.to_array(), [1.0, 2.0, 3.0, 4.0]);
    /// ```
    pub fn to_array(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }
}

