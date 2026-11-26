/// A general-purpose flex container and the building block for most layouts.
///
/// A `Container` owns its children as [`Element`]s, applies a [`LayoutStyle`]
/// to them, and paints its own [`Material`], border radius, hover fill, and
/// optional shadow. It can also act as a click target, a window drag region,
/// and an anchor for a tooltip and context menu.
///
/// # Defaults
///
/// [`Container::new`] starts with default layout settings, a transparent
/// solid material, zero corner radius, no tooltip/menu/hover fill/click
/// handler, and no children.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let panel = Container::new()
///     .padding(Edges::all(16.0))
///     .gap(8.0)
///     .fill(Color::from_rgba8(245, 245, 247, 255))
///     .radius(12.0)
///     .child(Text::new("Title").size(18.0))
///     .child(Button::new("Action").primary());
///
/// let element: Element = panel.into_element();
/// ```
pub struct Container {
    layout: LayoutStyle,
    material: Material,
    material_signal: Option<Signal<Material>>,
    radius: f32,
    radius_signal: Option<Signal<f32>>,
    is_drag_region: bool,
    tooltip: Option<String>,
    context_menu: Vec<MenuItem>,
    hover_fill: Option<Color>,
    on_click: Option<Box<dyn FnMut()>>,
    children: Vec<Element>,
}

impl Container {
    /// Creates an empty container with default layout and transparent fill.
    pub fn new() -> Self {
        Self {
            layout: LayoutStyle::default(),
            material: Material::solid(Color::TRANSPARENT),
            material_signal: None,
            radius: 0.0,
            radius_signal: None,
            is_drag_region: false,
            tooltip: None,
            context_menu: Vec::new(),
            hover_fill: None,
            on_click: None,
            children: Vec::new(),
        }
    }

    /// Creates an empty container whose children are stacked vertically.
    ///
    /// Equivalent to [`Container::new`] followed by
    /// [`Container::direction`] with [`FlexDir::Column`].
    pub fn column() -> Self {
        Self::new().direction(FlexDir::Column)
    }

    /// Creates an empty container whose children are laid out horizontally.
    ///
    /// Equivalent to [`Container::new`] followed by
    /// [`Container::direction`] with [`FlexDir::Row`].
    pub fn row() -> Self {
        Self::new().direction(FlexDir::Row)
    }

    /// Sets the flex direction used to arrange direct children.
    ///
    /// The default is [`FlexDir::Column`].
    pub fn direction(mut self, dir: FlexDir) -> Self {
        self.layout.direction = dir;
        self
    }

    /// Sets the container's preferred width.
    ///
    /// [`Length::Auto`] (the default) sizes the container from its children;
    /// fixed pixel and percentage lengths are resolved by the layout engine.
    pub fn width(mut self, w: Length) -> Self {
        self.layout.width = w;
        self
    }

    /// Sets the container's preferred height.
    ///
    /// [`Length::Auto`] (the default) sizes the container from its children.
    pub fn height(mut self, h: Length) -> Self {
        self.layout.height = h;
        self
    }

    /// Sets how much of the parent's free space this container absorbs.
    ///
    /// `0.0` is the default; larger values give the container a larger share
    /// relative to siblings with a nonzero grow factor.
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.layout.flex_grow = g;
        self
    }

    /// Sets inner spacing on each side of the container.
    pub fn padding(mut self, p: Edges) -> Self {
        self.layout.padding = p;
        self
    }

    /// Sets the logical-pixel gap inserted between adjacent children.
    pub fn gap(mut self, gap: f32) -> Self {
        self.layout.gap = gap;
        self
    }

    /// Sets cross-axis alignment of children.
    ///
    /// When unset, the layout engine uses its default alignment.
    pub fn align(mut self, align: AlignItems) -> Self {
        self.layout.align_items = Some(align);
        self
    }

    /// Sets main-axis distribution of children.
    ///
    /// When unset, the layout engine uses its default distribution.
    pub fn justify(mut self, justify: JustifyContent) -> Self {
        self.layout.justify_content = Some(justify);
        self
    }

    /// Allows children to wrap onto multiple lines when exceeding available width.
    pub fn wrap(mut self) -> Self {
        self.layout.wrap = true;
        self
    }

    /// Clips child drawing strictly to this container's bounds.
    pub fn clip(mut self) -> Self {
        self.layout.clip = true;
        self
    }

    /// Caps the resolved width of the container.
    pub fn max_width(mut self, w: Length) -> Self {
        self.layout.max_width = w;
        self
    }

    /// Caps the resolved height of the container.
    pub fn max_height(mut self, h: Length) -> Self {
        self.layout.max_height = h;
        self
    }

    /// Sets outer spacing on each side of the container.
    pub fn margin(mut self, m: Edges) -> Self {
        self.layout.margin = m;
        self
    }

    /// Takes the container out of the parent's flex flow.
    ///
    /// After this call, use [`Container::inset`] to position it relative to its
    /// containing node; siblings do not reserve space for it.
    pub fn absolute(mut self) -> Self {
        self.layout.position = PositionMode::Absolute;
        self
    }

    /// Sets per-side offsets from the parent's corresponding edges.
    ///
    /// The offsets are applied whether the container is in flow or absolute.
    pub fn inset(mut self, inset: Inset) -> Self {
        self.layout.inset = inset;
        self
    }

    /// Sets a fill color used while the pointer hovers over the container.
    ///
    /// The hover fill is applied on top of the configured material.
    pub fn hover_fill(mut self, color: Color) -> Self {
        self.hover_fill = Some(color);
        self
    }

    /// Registers a callback invoked when the container is pressed.
    ///
    /// The callback receives no arguments and runs on pointer-down for the
    /// hit-tested container. It must be `'static` since the mounted widget
    /// owns it.
    pub fn on_click(mut self, f: impl FnMut() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// Sets the painted [`Material`], replacing any previously configured fill.
    ///
    /// This is the general form of [`Container::fill`] and
    /// [`Container::gradient`]; it can also install frosted or liquid materials.
    /// Draw options such as [`Container::border`] and [`Container::shadow`]
    /// modify the material's corresponding fields afterwards.
    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    /// Binds the container's material to a reactive signal.
    ///
    /// The initial value is taken from the signal immediately if it is alive;
    /// afterwards an effect keeps the mounted node's material in sync and
    /// queues a paint update whenever the signal changes.
    pub fn bind_material(mut self, sig: Signal<Material>) -> Self {
        self.material = sig.try_get().unwrap_or(self.material);
        self.material_signal = Some(sig);
        self
    }

    /// Replaces the material with a solid color fill.
    ///
    /// Any gradient, border, or shadow previously set on the material is
    /// discarded.
    pub fn fill(mut self, color: Color) -> Self {
        self.material = Material::solid(color);
        self
    }

    /// Replaces the material with a vertical classic gradient.
    ///
    /// `start` is the top fill color, `end` the bottom fill color. The
    /// gradient has no border or shadow until [`Container::border`] or
    /// [`Container::shadow`] is applied.
    pub fn gradient(mut self, start: Color, end: Color) -> Self {
        self.material = Material::Classic {
            fill: start,
            fill_end: Some(end),
            gradient_dir: [0.0, 1.0],
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            shadow: None,
        };
        self
    }

    /// Sets the border color and width on the current material.
    ///
    /// Works for classic, frosted, and liquid materials; the value is in
    /// logical pixels and is scaled when painted. The material's other fields
    /// are preserved.
    pub fn border(mut self, color: Color, width: f32) -> Self {
        match &mut self.material {
            Material::Classic {
                border_color,
                border_width,
                ..
            }
            | Material::Frosted {
                border_color,
                border_width,
                ..
            }
            | Material::Liquid {
                border_color,
                border_width,
                ..
            } => {
                *border_color = color;
                *border_width = width;
            }
        }
        self
    }

    /// Sets the corner radius, in logical pixels, used to paint the container.
    ///
    /// The default is `0.0` (square corners).
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    /// Binds the corner radius to a reactive signal.
    ///
    /// The mounted node updates its radius and queues a paint refresh whenever
    /// the signal changes. The signal's current value is used at paint time;
    /// the builder's radius remains as the initial fallback when the signal is
    /// not alive.
    pub fn bind_radius(mut self, sig: Signal<f32>) -> Self {
        self.radius_signal = Some(sig);
        self
    }

    /// Sets a drop shadow on the current material.
    ///
    /// `blur` and `offset_y` are in logical pixels; a positive `offset_y`
    /// places the shadow below the container. This replaces any shadow
    /// already present on the material.
    pub fn shadow(mut self, color: Color, blur: f32, offset_y: f32) -> Self {
        let shad = Some(Shadow::new(color, blur, offset_y));
        match &mut self.material {
            Material::Classic { shadow, .. }
            | Material::Frosted { shadow, .. }
            | Material::Liquid { shadow, .. } => {
                *shadow = shad;
            }
        }
        self
    }

    /// Marks this container as a window drag region.
    ///
    /// The window delegate uses this when hit-testing custom title bars: a
    /// press over a drag region that did not hit an interactive widget is
    /// handled as a drag-region press instead of clearing keyboard focus.
    pub fn drag_region(mut self) -> Self {
        self.is_drag_region = true;
        self
    }

    /// Sets the tooltip shown when the pointer rests over the container.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Appends entries to the container's right-click context menu.
    ///
    /// Existing entries are kept; call this repeatedly to add more items or
    /// use [`MenuItem::separator`] to divide groups.
    pub fn context_menu<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.context_menu.extend(items);
        self
    }

    /// Appends one child to the container's element tree.
    ///
    /// Children are laid out in insertion order along the configured flex
    /// direction and are painted in the same order.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    /// Appends all elements yielded by `iter` as children.
    ///
    /// Each item is converted with [`IntoElement::into_element`] and appended
    /// after any children already added.
    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.children
            .extend(iter.into_iter().map(IntoElement::into_element));
        self
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Container {
    fn into_element(self) -> Element {
        Element::Container(self)
    }
}

/// Alias for [`Container`], used to make vertical compositions read clearly.
///
/// It is a type alias rather than a distinct widget, so all [`Container`]
/// builders apply; use [`Container::column`] or [`Container::new`] to request a
/// vertical flex direction.
pub type Column = Container;

/// Convenience entry point for a horizontally arranged [`Container`].
///
/// `Row` is a zero-sized builder marker; [`Row::new`] returns an actual
/// container configured with [`FlexDir::Row`].
pub struct Row;
impl Row {
    /// Creates an empty container whose children are laid out horizontally.
    ///
    /// ```
    /// use cadmium::prelude::*;
    ///
    /// let toolbar = Row::new()
    ///     .gap(6.0)
    ///     .child(Button::new("Open"))
    ///     .child(Button::new("Close"));
    /// let element: Element = toolbar.into_element();
    /// ```
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::row()
    }
}

/// Convenience entry point for an unconfigured [`Container`].
///
/// `Stack` is a zero-sized builder marker; [`Stack::new`] returns a plain
/// container with default layout settings, whose direction follows the layout
/// engine's default.
pub struct Stack;
impl Stack {
    /// Creates an empty container with default layout settings.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::new()
    }
}

/// A translucent panel with macOS-style frosted-glass styling.
///
/// `GlassPanel` wraps a [`Container`] and forwards its layout and child
/// builders, so it composes like any other container while painting with a
/// [`Material::frosted`] background by default. Its element is mounted as a
/// regular [`Element::Container`].
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let card = GlassPanel::new()
///     .width(Length::Px(280.0))
///     .padding(Edges::all(20.0))
///     .radius(18.0)
///     .child(Text::new("Frosted").tone(TextTone::Primary));
/// let element: Element = card.into_element();
/// ```
pub struct GlassPanel {
    inner: Container,
}

impl GlassPanel {
    /// Creates a panel with a 16-pixel radius and a light frosted tint.
    pub fn new() -> Self {
        Self {
            inner: Container::new()
                .radius(16.0)
                .material(Material::frosted(Color::from_rgba8(255, 255, 255, 130))),
        }
    }

    /// Sets the panel's preferred width; see [`Container::width`].
    pub fn width(mut self, w: Length) -> Self {
        self.inner = self.inner.width(w);
        self
    }

    /// Sets the panel's preferred height; see [`Container::height`].
    pub fn height(mut self, h: Length) -> Self {
        self.inner = self.inner.height(h);
        self
    }

    /// Sets how much of the parent's free space the panel absorbs.
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.inner = self.inner.flex_grow(g);
        self
    }

    /// Sets inner spacing on each side of the panel.
    pub fn padding(mut self, p: Edges) -> Self {
        self.inner = self.inner.padding(p);
        self
    }

    /// Sets the logical-pixel gap between direct children.
    pub fn gap(mut self, g: f32) -> Self {
        self.inner = self.inner.gap(g);
        self
    }

    /// Sets the corner radius used to clip and paint the panel.
    ///
    /// The default is `16.0`.
    pub fn radius(mut self, r: f32) -> Self {
        self.inner = self.inner.radius(r);
        self
    }

    /// Replaces the frosted tint color.
    ///
    /// The alpha component of `tint` controls how much of the background is
    /// allowed through.
    pub fn tint(mut self, tint: Color) -> Self {
        self.inner.material = Material::frosted(tint);
        self
    }

    /// Switches to a liquid-glass material with a faint white tint.
    ///
    /// This replaces the frosted material configured by [`GlassPanel::new`]
    /// or [`GlassPanel::tint`].
    pub fn liquid(mut self) -> Self {
        self.inner.material = Material::liquid(Color::from_rgba8(255, 255, 255, 40));
        self
    }

    /// Appends one child to the panel's inner container.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    /// Appends all elements yielded by `iter` to the panel's inner container.
    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.inner = self.inner.children(iter);
        self
    }
}

impl Default for GlassPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for GlassPanel {
    fn into_element(self) -> Element {
        Element::Container(self.inner)
    }
}

