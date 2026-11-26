/// Clickable text hyperlink with accent styling.
///
/// A `Link` builds a padded [`Container`] row around a single [`Text`] label.
/// It is a click target and tooltip anchor in the element tree; it does not
/// store a URL or navigate on its own, so a [`Link::on_click`] callback must
/// supply the action. Like the rest of the builder API, it holds no [`Signal`]
/// and is rebuilt by its parent when the displayed text should change.
///
/// # Defaults
///
/// [`Link::new`] colors the label with the light theme's accent color at 13px
/// and pads the row by 2px horizontally and 1px vertically. There is no
/// callback or tooltip until one is configured. [`Link::color`] and
/// [`Link::size`] affect the text only; [`Link::on_click`] and
/// [`Link::tooltip`] are forwarded to the enclosing container.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let docs = Link::new("Documentation")
///     .size(14.0)
///     .tooltip("Open the documentation")
///     .on_click(|| println!("navigate to docs"));
///
/// let element: Element = docs.into_element();
/// ```
pub struct Link {
    label: String,
    color: Color,
    size: f32,
    inner: Container,
}

impl Link {
    /// Creates an accent-colored link displaying `label`.
    ///
    /// The label is stored as an owned [`String`]. No click handler is installed
    /// until [`Link::on_click`] is called.
    pub fn new(label: impl Into<String>) -> Self {
        let light = MacTheme::light();
        Self {
            label: label.into(),
            color: light.accent,
            size: 13.0,
            inner: Container::row().padding(Edges::xy(2.0, 1.0)),
        }
    }

    /// Overrides the label's text color.
    ///
    /// The default is the light theme's accent color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Overrides the label's font size in logical pixels.
    ///
    /// The default is 13px.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Registers a callback invoked when the link is pressed (pointer-down).
    ///
    /// The callback must be `'static` because the mounted container owns it;
    /// the link itself does not perform navigation.
    pub fn on_click(mut self, f: impl FnMut() + 'static) -> Self {
        self.inner = self.inner.on_click(f);
        self
    }

    /// Sets the tooltip shown while the pointer rests over the link.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.inner = self.inner.tooltip(text);
        self
    }
}

/// Builds the styled text and its clickable container into one [`Element`].
impl IntoElement for Link {
    /// Returns the clickable, labelled container as a single element.
    fn into_element(self) -> Element {
        self.inner
            .child(Text::new(self.label).size(self.size).color(self.color))
            .into_element()
    }
}
