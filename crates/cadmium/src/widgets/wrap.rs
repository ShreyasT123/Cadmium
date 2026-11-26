/// A flex-wrapping flow container that automatically moves overflowing children
/// onto new rows when horizontal space shrinks (e.g. during window tiling).
///
/// `Wrap` is a thin builder around a horizontal [`Container`] with
/// [`Container::wrap`] enabled. It is useful for card grids that should reflow
/// from three columns to two or one as the window narrows.
///
/// # Examples
///
/// ```
/// use cadmium::{IntoElement, Text, Wrap};
///
/// let flow = Wrap::new()
///     .gap(12.0)
///     .child(Text::new("Alpha"))
///     .child(Text::new("Beta"));
/// let _ = flow.into_element();
/// ```
pub struct Wrap {
    inner: Container,
}

impl Wrap {
    /// Creates a new wrapping horizontal container.
    pub fn new() -> Self {
        Self {
            inner: Container::row().wrap().gap(10.0),
        }
    }

    /// Sets horizontal and vertical spacing between wrapped items.
    pub fn gap(mut self, gap: f32) -> Self {
        self.inner = self.inner.gap(gap);
        self
    }

    /// Sets outer padding around the wrapped container.
    pub fn padding(mut self, p: Edges) -> Self {
        self.inner = self.inner.padding(p);
        self
    }

    /// Appends a child element.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    /// Appends multiple child elements.
    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.inner = self.inner.children(iter);
        self
    }
}

impl Default for Wrap {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Wrap {
    fn into_element(self) -> Element {
        self.inner.into_element()
    }
}
