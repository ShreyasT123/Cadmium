/// Horizontal segmented control bound to a `Signal<usize>` active index.
///
/// This builder wraps [`Tabs`] and converts straight to its [`Element`]. The
/// signal is the only source of selection state: pressing a segment writes
/// that segment's index to the signal, and the mounted tab control highlights
/// the matching segment whenever the signal changes, including changes made
/// elsewhere in the app.
///
/// # Defaults
///
/// [`SegmentedControl::new`] copies the supplied labels into owned strings and
/// inherits the [`Tabs`] defaults: a light gray classic material, automatic
/// width, and a 30px height, with the minimum width growing with the number of
/// items. [`SegmentedControl::material`] replaces only the background
/// [`Material`]; other tab settings come from the wrapped [`Tabs`] builder.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let view_index = create_signal(0usize);
/// let control = SegmentedControl::new(["List", "Grid"], view_index)
///     .material(Material::solid(Color::from_rgba8(240, 240, 242, 255)));
///
/// let element: Element = control.into_element();
/// view_index.set(1); // a mounted control now highlights "Grid"
/// ```
pub struct SegmentedControl {
    inner: Tabs,
}

impl SegmentedControl {
    /// Creates segments labelled by `items`, controlled by `signal`.
    ///
    /// `items` may be any iterable of strings; the labels are copied at
    /// construction. The signal holds the zero-based index of the active
    /// segment.
    pub fn new<I, S>(items: I, signal: Signal<usize>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let items: Vec<String> = items.into_iter().map(Into::into).collect();
        Self {
            inner: Tabs::new(signal, &items),
        }
    }

    /// Replaces the control's background [`Material`].
    ///
    /// This only changes the surface material of the wrapped [`Tabs`]; the
    /// labels, active index, and hit behavior remain the same.
    pub fn material(mut self, material: Material) -> Self {
        self.inner = self.inner.material(material);
        self
    }
}

/// Converts the builder into the wrapped [`Tabs`] element.
impl IntoElement for SegmentedControl {
    /// Returns the wrapped tab control as the element to mount.
    fn into_element(self) -> Element {
        self.inner.into_element()
    }
}
