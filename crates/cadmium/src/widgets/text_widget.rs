/// Selects a semantic text color from the active [`MacTheme`].
///
/// Tones let labels adapt to light/dark themes without hard-coding colors.
/// [`Text::tone`] and [`Icon::tone`] store a tone; the actual color is
/// resolved during painting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTone {
    /// Use an explicitly supplied color instead of a themed color.
    Custom,
    /// The primary foreground color for prominent text.
    Primary,
    /// A secondary color for supporting text.
    Secondary,
    /// A tertiary color for de-emphasized text.
    Tertiary,
    /// A color intended for text drawn on an accent-colored surface.
    OnAccent,
}

impl TextTone {
    /// Resolves this tone to a concrete color for `theme`.
    ///
    /// `fallback` is returned only for [`TextTone::Custom`].
    pub fn resolve(self, theme: &MacTheme, fallback: Color) -> Color {
        match self {
            Self::Custom => fallback,
            Self::Primary => theme.text_primary,
            Self::Secondary => theme.text_secondary,
            Self::Tertiary => theme.text_tertiary,
            Self::OnAccent => theme.text_on_accent,
        }
    }
}

enum TextSource {
    Static(String),
    Reactive(Box<dyn FnMut() -> String>),
}

/// A text label, either static or refreshed by a reactive closure.
///
/// Text is measured by the layout engine using the configured [`TextStyle`]
/// and painted with the color selected by its [`TextTone`].
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let title = Text::new("Welcome back")
///     .size(22.0)
///     .weight(FontWeight::Semibold)
///     .tone(TextTone::Primary);
/// let caption = Text::reactive(|| format!("Rows: {}", 42)).size(13.0);
///
/// let ui = Column::new().gap(4.0).child(title).child(caption);
/// let element: Element = ui.into_element();
/// ```
pub struct Text {
    source: TextSource,
    style: TextStyle,
    tone: TextTone,
    single_line: bool,
}

impl Text {
    /// Creates a label containing a fixed string.
    ///
    /// The label uses [`TextStyle::default`] and [`TextTone::Custom`], so its
    /// color comes from the style unless [`Text::tone`] is called.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            source: TextSource::Static(text.into()),
            style: TextStyle::default(),
            tone: TextTone::Custom,
            single_line: false,
        }
    }

    /// Creates a label whose string is recomputed by a reactive effect.
    ///
    /// The closure runs inside a reactive effect when the text widget is
    /// mounted. Whenever it returns a different string, the mounted text block
    /// is updated and the widget is marked dirty for layout and paint. The
    /// closure must be `'static` because it is owned by the mounted widget.
    pub fn reactive(f: impl FnMut() -> String + 'static) -> Self {
        Self {
            source: TextSource::Reactive(Box::new(f)),
            style: TextStyle::default(),
            tone: TextTone::Custom,
            single_line: false,
        }
    }

    /// Sets the font size and derives a line height of `round(size * 1.38)`.
    pub fn size(mut self, font_size: f32) -> Self {
        self.style.font_size = font_size;
        self.style.line_height = (font_size * 1.38).round();
        self
    }

    /// Sets the font weight used for shaping and painting.
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.style.weight = weight;
        self
    }

    /// Sets an explicit text color.
    ///
    /// This color is used directly only while the tone is
    /// [`TextTone::Custom`]; selecting another tone resolves the color from
    /// the theme instead.
    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self
    }

    /// Selects a theme-aware semantic color for the text.
    pub fn tone(mut self, tone: TextTone) -> Self {
        self.tone = tone;
        self
    }

    /// Clips the label to a single line instead of wrapping it.
    ///
    /// The measured width is capped at the available layout width, so a long
    /// title in a narrow tiled pane never spills into neighbouring columns.
    /// This clips overflow rather than drawing a literal `...` glyph.
    pub fn ellipsis(mut self) -> Self {
        self.single_line = true;
        self
    }
}

impl IntoElement for Text {
    fn into_element(self) -> Element {
        Element::Text(self)
    }
}

