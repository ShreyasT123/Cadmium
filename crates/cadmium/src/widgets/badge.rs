/// Compact pill badge for status labels, counts, and tags.
///
/// A `Badge` is a builder for a small centered [`Container`] row holding one
/// [`Text`] label. It carries no [`Signal`] and no click callback, so it is used
/// for static annotations inside larger trees; the parent decides when the
/// badge is rebuilt with different content or colors.
///
/// # Defaults
///
/// [`Badge::new`] uses the light theme's accent-soft fill with accent text,
/// an 11px semibold label, a 10px corner radius, and 8px/3px horizontal and
/// vertical padding. [`Badge::fill`] and [`Badge::text_color`] change the two
/// colors independently, while [`Badge::size`] and [`Badge::radius`] tune the
/// label size and pill rounding.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let status = Badge::new("Draft")
///     .fill(Color::from_rgba8(230, 230, 235, 255))
///     .text_color(Color::from_rgba8(90, 90, 100, 255))
///     .size(12.0)
///     .radius(8.0);
///
/// let element: Element = status.into_element();
/// ```
pub struct Badge {
    label: String,
    fill: Color,
    text_color: Color,
    font_size: f32,
    radius: f32,
}

impl Badge {
    /// Creates a badge displaying `label` with the default accent styling.
    ///
    /// The label is converted to an owned [`String`] at construction time.
    pub fn new(label: impl Into<String>) -> Self {
        let light = MacTheme::light();
        Self {
            label: label.into(),
            fill: light.accent_soft,
            text_color: light.accent,
            font_size: 11.0,
            radius: 10.0,
        }
    }

    /// Sets the pill's background color.
    ///
    /// The default is the light theme's accent-soft color; the label color is
    /// unaffected.
    pub fn fill(mut self, color: Color) -> Self {
        self.fill = color;
        self
    }

    /// Sets the label's text color.
    ///
    /// The default is the light theme's accent color; the pill fill is
    /// unaffected.
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    /// Sets the label's font size in logical pixels.
    ///
    /// The label remains semibold; the default size is 11px.
    pub fn size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    /// Sets the pill's corner radius in logical pixels.
    ///
    /// The default radius of 10px gives short labels a rounded, pill-like
    /// outline; larger values round the surface further.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
}

/// Builds the padded, centered pill container and its label as one [`Element`].
impl IntoElement for Badge {
    /// Builds the pill surface and label as a single element.
    fn into_element(self) -> Element {
        Container::row()
            .padding(Edges::xy(8.0, 3.0))
            .align(AlignItems::Center)
            .justify(JustifyContent::Center)
            .radius(self.radius)
            .fill(self.fill)
            .child(
                Text::new(self.label)
                    .size(self.font_size)
                    .weight(FontWeight::Semibold)
                    .color(self.text_color),
            )
            .into_element()
    }
}
