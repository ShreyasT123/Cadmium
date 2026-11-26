/// Discrete font weight used when shaping a [`TextBlock`].
///
/// Each variant maps to the correspondingly named `cosmic_text::Weight` constant:
/// `Regular` to `NORMAL`, `Medium` to `MEDIUM`, `Semibold` to `SEMIBOLD`, and
/// `Bold` to `BOLD`.
///
/// # Examples
///
/// ```rust
/// use cadmium::text::{FontWeight, TextStyle};
///
/// let style = TextStyle {
///     weight: FontWeight::Bold,
///     ..Default::default()
/// };
/// assert_eq!(style.weight, FontWeight::Bold);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    /// Normal (unemboldened) weight; the default for [`TextStyle`].
    Regular,
    /// Medium weight, between regular and semibold.
    Medium,
    /// Semibold weight, often used for emphasized labels and headings.
    Semibold,
    /// Bold weight, often used for titles.
    Bold,
}

/// Converts a weight into the `cosmic-text` weight used for font matching.
impl From<FontWeight> for Weight {
    fn from(w: FontWeight) -> Self {
        match w {
            FontWeight::Regular => Weight::NORMAL,
            FontWeight::Medium => Weight::MEDIUM,
            FontWeight::Semibold => Weight::SEMIBOLD,
            FontWeight::Bold => Weight::BOLD,
        }
    }
}

/// Style applied to all the text in a [`TextBlock`].
///
/// `font_size` and `line_height` are logical pixels. The default is a 13.0 px
/// font on an 18.0 px line height, colored `Color::from_rgba8(29, 29, 31, 255)`
/// (near-black) with [`FontWeight::Regular`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Font size in logical pixels; defaults to `13.0`.
    pub font_size: f32,
    /// Height of one laid-out line in logical pixels; defaults to `18.0`.
    ///
    /// [`TextBlock::measure`] uses this value directly rather than deriving a
    /// height from `font_size`.
    pub line_height: f32,
    /// Linear-RGBA color used when the block is drawn; defaults to
    /// `Color::from_rgba8(29, 29, 31, 255)`.
    pub color: Color,
    /// Weight used to match a font face; defaults to [`FontWeight::Regular`].
    pub weight: FontWeight,
}

/// Returns Cadmium's default body text style: 13.0 px font, 18.0 px line height,
/// near-black color, and [`FontWeight::Regular`].
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            line_height: 18.0,
            color: Color::from_rgba8(29, 29, 31, 255),
            weight: FontWeight::Regular,
        }
    }
}

