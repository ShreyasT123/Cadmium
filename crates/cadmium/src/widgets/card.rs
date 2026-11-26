/// Elevated surface container with macOS-inspired card defaults (rounded corners,
/// padding, subtle hairline border, and soft drop shadow).
///
/// A `Card` is a thin builder around [`Container`]. It converts to the same
/// container [`Element`] used by the rest of the framework, so it takes part in
/// normal layout, painting, hit-testing, and can be passed anywhere an
/// [`IntoElement`] is accepted.
///
/// # Reactive behavior
///
/// A card does not store a [`Signal`]; its surface stays as configured until the
/// tree is rebuilt. A callback registered with [`Card::on_click`] runs on
/// pointer-down, like other container click handlers.
///
/// # Defaults
///
/// [`Card::new`] applies a 12px corner radius, 16px padding on every side, an
/// 8px gap between children, and a [`Material::Classic`] surface using the
/// light theme's panel fill, hairline border, and soft drop shadow.
/// [`Card::fill`] replaces that entire material with a solid color, losing the
/// default border and shadow; use [`Card::material`] to keep or customize them.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let card = Card::new()
///     .width(Length::Px(280.0))
///     .gap(12.0)
///     .child(Text::new("Storage").size(16.0).weight(FontWeight::Semibold))
///     .child(Text::new("12.4 GB of 50 GB used").size(13.0));
///
/// let element: Element = card.into_element();
/// ```
pub struct Card {
    inner: Container,
}

impl Card {
    /// Creates an empty card with the elevated surface described on [`Card`].
    pub fn new() -> Self {
        let light = MacTheme::light();
        Self {
            inner: Container::new()
                .radius(12.0)
                .padding(Edges::all(16.0))
                .gap(8.0)
                .material(Material::Classic {
                    fill: light.panel_bg,
                    fill_end: None,
                    gradient_dir: [0.0, 0.0],
                    border_color: light.hairline,
                    border_width: 1.0,
                    shadow: Some(Shadow::new(light.shadow_soft, 12.0, 4.0)),
                }),
        }
    }

    /// Sets the card's preferred width.
    ///
    /// [`Length::Auto`] sizes the card from its children; fixed pixel and
    /// percentage lengths are resolved by the layout engine.
    pub fn width(mut self, w: Length) -> Self {
        self.inner = self.inner.width(w);
        self
    }

    /// Sets the card's preferred height.
    ///
    /// [`Length::Auto`] sizes the card from its children unless a fixed length
    /// is given.
    pub fn height(mut self, h: Length) -> Self {
        self.inner = self.inner.height(h);
        self
    }

    /// Sets how much of the parent's free space the card absorbs.
    ///
    /// The default is `0.0`; larger factors give the card a larger share
    /// relative to siblings with a nonzero grow factor.
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.inner = self.inner.flex_grow(g);
        self
    }

    /// Overrides the default 16px inner padding on every side.
    pub fn padding(mut self, p: Edges) -> Self {
        self.inner = self.inner.padding(p);
        self
    }

    /// Overrides the default 8px spacing between adjacent children.
    pub fn gap(mut self, g: f32) -> Self {
        self.inner = self.inner.gap(g);
        self
    }

    /// Overrides the default 12px corner radius.
    pub fn radius(mut self, r: f32) -> Self {
        self.inner = self.inner.radius(r);
        self
    }

    /// Replaces the card's surface with a solid fill of `color`.
    ///
    /// Like [`Container::fill`], this discards the material configured by
    /// [`Card::new`], including its hairline border and drop shadow.
    pub fn fill(mut self, color: Color) -> Self {
        self.inner = self.inner.fill(color);
        self
    }

    /// Installs a complete surface [`Material`] on the card.
    ///
    /// Use this instead of [`Card::fill`] when the surface should keep a
    /// gradient, border, shadow, or a frosted/liquid rendering path.
    pub fn material(mut self, material: Material) -> Self {
        self.inner = self.inner.material(material);
        self
    }

    /// Registers a callback invoked when the card is pressed (pointer-down).
    ///
    /// The callback takes no arguments and must be `'static` because the
    /// mounted container owns it.
    pub fn on_click(mut self, f: impl FnMut() + 'static) -> Self {
        self.inner = self.inner.on_click(f);
        self
    }

    /// Appends a child to the card's content.
    ///
    /// Children are laid out in insertion order with the configured
    /// [`Card::gap`] between them.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    /// Appends every item from `iter` to the card's content.
    ///
    /// Each item is converted into an [`Element`] and placed in iteration
    /// order, after any children added by [`Card::child`].
    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.inner = self.inner.children(iter);
        self
    }
}

/// Produces the same elevated surface as [`Card::new`].
impl Default for Card {
    /// Delegates to [`Card::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// Converts the builder into its underlying [`Container`] element.
impl IntoElement for Card {
    /// Returns the configured inner container as the element to mount.
    fn into_element(self) -> Element {
        Element::Container(self.inner)
    }
}
