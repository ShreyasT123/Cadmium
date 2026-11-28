/// A vertically scrolling viewport over a column of child elements.
///
/// Mounting creates a viewport node with a single column content node. The
/// viewport grows to fill its parent; its children are measured at their
/// unconstrained height and moved with a spring-smoothed vertical scroll
/// offset when wheel events are dispatched to the scrollable node.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let feed = ScrollView::new()
///     .gap(6.0)
///     .children((0..20).map(|i| Text::new(format!("Row {i}")).size(13.0)));
/// let element: Element = feed.into_element();
/// ```
pub struct ScrollView {
    children: Vec<Element>,
    gap: f32,
}

impl ScrollView {
    /// Creates an empty scroll view with an 8-pixel gap between children.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            gap: 8.0,
        }
    }

    /// Sets the logical-pixel gap between stacked children.
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }

    /// Appends one child to the scrollable column.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    /// Appends all elements yielded by `iter` to the scrollable column.
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

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for ScrollView {
    fn into_element(self) -> Element {
        Element::ScrollView(self)
    }
}

/// The standard red/yellow/green macOS window controls.
///
/// The widget is a single hit-tested node: pressing its left third requests
/// window close, the middle third requests minimize, and the right third
/// requests zoom/maximize through [`crate::window::WindowCommand`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficLights;

impl TrafficLights {
    /// Creates the traffic-light control cluster.
    pub fn new() -> Self {
        Self
    }
}

impl IntoElement for TrafficLights {
    fn into_element(self) -> Element {
        Element::TrafficLights(self)
    }
}

/// A segmented tab selector bound to a [`Signal`] holding the active index.
///
/// Pressing a segment writes its index to the signal, clamped to the available
/// items. The selection pill follows the signal with a spring animation, so
/// changes made elsewhere in the app animate as well. Items are copied into
/// owned strings when the builder is created.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let tab = create_signal(0usize);
/// let tabs = Tabs::new(tab, &["General", "Appearance", "Privacy"])
///     .width(Length::Px(320.0))
///     .tooltip("Settings section");
/// let element: Element = tabs.into_element();
/// ```
pub struct Tabs {
    signal: Signal<usize>,
    items: Vec<String>,
    material: Material,
    width: Length,
    tooltip: Option<String>,
}

impl Tabs {
    /// Creates tabs for `items`, controlled by `signal`.
    ///
    /// Defaults to a light gray classic material, auto width, and a 30-pixel
    /// height; the minimum width grows with the number of items.
    pub fn new(signal: Signal<usize>, items: &[impl AsRef<str>]) -> Self {
        Self {
            signal,
            items: items.iter().map(|s| s.as_ref().to_string()).collect(),
            material: Material::Classic {
                fill: Color::from_rgba8(229, 229, 234, 255),
                fill_end: None,
                gradient_dir: [0.0, 1.0],
                border_color: Color::TRANSPARENT,
                border_width: 0.0,
                shadow: None,
            },
            width: Length::Auto,
            tooltip: None,
        }
    }

    /// Replaces the segmented control's background material.
    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    /// Sets the control's width.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }

    /// Sets the tooltip shown when the pointer rests over the control.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Tabs {
    fn into_element(self) -> Element {
        Element::Tabs(self)
    }
}

/// A drop-down selector bound to a [`Signal`] holding the selected index.
///
/// Pressing the control toggles a menu overlay; pressing a row writes that
/// row's index to the signal and closes the menu. Items are copied into owned
/// strings when the builder is created.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let choice = create_signal(0usize);
/// let dropdown = Dropdown::new(choice, &["Small", "Medium", "Large"])
///     .width(Length::Px(180.0));
/// let element: Element = dropdown.into_element();
/// ```
pub struct Dropdown {
    signal: Signal<usize>,
    items: Vec<String>,
    material: Material,
    width: Length,
    tooltip: Option<String>,
}

impl Dropdown {
    /// Creates a drop-down for `items`, controlled by `signal`.
    ///
    /// Defaults to a 160-pixel width, a translucent bordered material, and a
    /// soft shadow.
    pub fn new(signal: Signal<usize>, items: &[impl AsRef<str>]) -> Self {
        Self {
            signal,
            items: items.iter().map(|s| s.as_ref().to_string()).collect(),
            material: Material::Classic {
                fill: Color::from_rgba8(255, 255, 255, 220),
                fill_end: None,
                gradient_dir: [0.0, 1.0],
                border_color: Color::from_rgba8(0, 0, 0, 30),
                border_width: 1.0,
                shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 18), 4.0, 1.0)),
            },
            width: Length::Px(160.0),
            tooltip: None,
        }
    }

    /// Replaces the control's background material.
    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    /// Sets the closed control's width; its height is fixed at 32 pixels.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }

    /// Sets the tooltip shown when the pointer rests over the closed control.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Dropdown {
    fn into_element(self) -> Element {
        Element::Dropdown(self)
    }
}

type SurfaceDirtyBinder = Box<dyn FnOnce(DirtyQueue, WidgetId, Rc<Cell<bool>>) -> EffectId>;

/// A widget that renders a user-defined wgpu pass into its own texture.
///
/// The surface allocates a render target sized to its laid-out bounds, calls
/// [`CustomGpuPass::render`] whenever the target is new or its animation
/// signal is active, and composites the texture with the configured corner
/// radius. Textures are released when the surface is resized or unmounted.
///
/// # Defaults
///
/// A 12-pixel corner radius, full resolution scale, no animation signal, and
/// no tooltip or context menu.
pub struct CustomGpuSurface {
    layout: LayoutStyle,
    radius: f32,
    radius_signal: Option<Signal<f32>>,
    animate_signal: Option<Signal<bool>>,
    tooltip: Option<String>,
    context_menu: Vec<MenuItem>,
    resolution_scale: f32,
    dirty_binders: Vec<SurfaceDirtyBinder>,
    pass: Box<dyn CustomGpuPass>,
}
impl CustomGpuSurface {
    /// Creates a surface driven by `pass`.
    pub fn new(pass: impl CustomGpuPass) -> Self {
        Self {
            layout: LayoutStyle::default(),
            radius: 12.0,
            radius_signal: None,
            animate_signal: None,
            tooltip: None,
            context_menu: Vec::new(),
            resolution_scale: 1.0,
            dirty_binders: Vec::new(),
            pass: Box::new(pass),
        }
    }

    /// Sets the surface's preferred width.
    pub fn width(mut self, w: Length) -> Self {
        self.layout.width = w;
        self
    }

    /// Sets the surface's preferred height.
    pub fn height(mut self, h: Length) -> Self {
        self.layout.height = h;
        self
    }

    /// Sets how much of the parent's free space the surface absorbs.
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.layout.flex_grow = g;
        self
    }

    /// Sets the corner radius used to clip the composited texture.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    /// Binds the corner radius to a reactive signal.
    pub fn bind_radius(mut self, sig: Signal<f32>) -> Self {
        self.radius = sig.try_get().unwrap_or(self.radius);
        self.radius_signal = Some(sig);
        self
    }

    /// Re-renders the custom pass on every frame while `sig` is `true`.
    ///
    /// Without an animation signal the pass is rendered only once after
    /// mounting or a resize.
    pub fn animate_when(mut self, sig: Signal<bool>) -> Self {
        self.animate_signal = Some(sig);
        self
    }
/// Marks this surface dirty to re-render one frame whenever `sig` changes.
    pub fn redraw_on<T: 'static>(mut self, sig: Signal<T>) -> Self {
        self.dirty_binders.push(Box::new(move |q, id, flag| {
            create_effect(move || {
                let _ = sig.try_with(|_| ());
                flag.set(true);
                push_dirty(
                    &q,
                    id,
                    DirtyFlags {
                        state: false,
                        layout: false,
                        paint: true,
                    },
                );
            })
        }));
        self
    }
    /// Scales the backing texture relative to the widget's logical size.
    ///
    /// Values outside `0.1..=1.0` are clamped into that range; `1.0` renders
    /// at the full pixel size of the laid-out bounds.
    pub fn resolution_scale(mut self, s: f32) -> Self {
        self.resolution_scale = s.clamp(0.1, 1.0);
        self
    }

    /// Sets the tooltip shown when the pointer rests over the surface.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Appends entries to the surface's right-click context menu.
    pub fn context_menu<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.context_menu.extend(items);
        self
    }
    
}

impl IntoElement for CustomGpuSurface {
    fn into_element(self) -> Element {
        Element::CustomGpuSurface(self)
    }
}

/// The axis along which a [`SplitPane`] divides its two children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Place the panes side by side, separated by a vertical one-pixel divider.
    Horizontal,
    /// Stack the panes vertically, separated by a horizontal one-pixel divider.
    Vertical,
}

/// Two panes whose sizes are controlled by a shared fractional ratio signal.
///
/// The ratio describes the first pane's share of the available axis and is
/// updated by dragging the divider. Missing panes are filled with
/// [`Spacer::flex`], and the initial ratio is clamped to the configured bounds.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let ratio = create_signal(0.3f32);
/// let pane = SplitPane::horizontal(ratio)
///     .bounds(0.2, 0.8)
///     .first(Text::new("Sidebar"))
///     .second(ScrollView::new().child(Text::new("Content")));
/// let element: Element = pane.into_element();
/// ```
pub struct SplitPane {
    direction: SplitDirection,
    ratio: Signal<f32>,
    min_ratio: f32,
    max_ratio: f32,
    first: Option<Box<Element>>,
    second: Option<Box<Element>>,
    divider_color: Option<Color>,
}

impl SplitPane {
    /// Creates a side-by-side split pane controlled by `ratio`.
    pub fn horizontal(ratio: Signal<f32>) -> Self {
        Self::new(SplitDirection::Horizontal, ratio)
    }

    /// Creates a vertically stacked split pane controlled by `ratio`.
    pub fn vertical(ratio: Signal<f32>) -> Self {
        Self::new(SplitDirection::Vertical, ratio)
    }

    /// Creates a split pane along `direction` controlled by `ratio`.
    ///
    /// The default ratio bounds are `0.15..=0.85`; both panes default to a
    /// flex spacer, and the divider uses the theme hairline color.
    pub fn new(direction: SplitDirection, ratio: Signal<f32>) -> Self {
        Self {
            direction,
            ratio,
            min_ratio: 0.15,
            max_ratio: 0.85,
            first: None,
            second: None,
            divider_color: None,
        }
    }

    /// Constrains the permitted first-pane fraction.
    ///
    /// `min_ratio` is clamped to `0.05..=0.90` and `max_ratio` to at least
    /// `min_ratio` and at most `0.95`.
    pub fn bounds(mut self, min_ratio: f32, max_ratio: f32) -> Self {
        self.min_ratio = min_ratio.clamp(0.05, 0.90);
        self.max_ratio = max_ratio.clamp(self.min_ratio, 0.95);
        self
    }

    /// Overrides the color of the draggable divider.
    pub fn divider_color(mut self, color: Color) -> Self {
        self.divider_color = Some(color);
        self
    }

    /// Sets the first (top or left) pane's content.
    pub fn first(mut self, elem: impl IntoElement) -> Self {
        self.first = Some(Box::new(elem.into_element()));
        self
    }

    /// Sets the second (bottom or right) pane's content.
    pub fn second(mut self, elem: impl IntoElement) -> Self {
        self.second = Some(Box::new(elem.into_element()));
        self
    }
}

impl IntoElement for SplitPane {
    fn into_element(self) -> Element {
        Element::SplitPane(self)
    }
}

/// The text content of one row produced by a [`VirtualizedList`] builder.
///
/// Empty strings are allowed: they simply leave that part of the row unpainted.
#[derive(Debug, Clone)]
pub struct VirtualRow {
    /// Primary row label.
    pub title: String,
    /// Secondary label drawn beneath the title.
    pub subtitle: String,
    /// Optional trailing label drawn as a badge.
    pub badge: String,
}

/// A scrollable list that builds only the rows currently in view.
///
/// The list keeps a fixed-size pool of row text blocks and rebinds them as the
/// scroll position changes, so a large `item_count` does not create a widget
/// per item. Row heights are uniform; `item_height` values below 20 pixels are
/// raised to 20.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let selected = create_signal(0usize);
/// let list = VirtualizedList::new(10_000, 44.0, |i| VirtualRow {
///     title: format!("Item {i}"),
///     subtitle: format!("Row number {i}"),
///     badge: String::new(),
/// })
/// .selected(selected)
/// .gap(4.0);
/// let element: Element = list.into_element();
/// ```
pub struct VirtualizedList {
    item_count: usize,
    item_height: f32,
    gap: f32,
    flat: bool,
    columns_mode: bool,
    accent: Option<Color>,
    selected: Option<Signal<usize>>,
    on_select: Option<Box<dyn FnMut(usize)>>,
    row_builder: Box<dyn FnMut(usize) -> VirtualRow>,
    row_art: Option<Box<dyn FnMut(usize) -> Arc<DecodedImage>>>,
}

impl VirtualizedList {
    /// Creates a list of `item_count` rows with a uniform height.
    ///
    /// `row_builder` is called lazily for each visible index. It must be
    /// `'static` because the mounted list owns it. The default gap is 6 pixels
    /// and no row is selected.
    pub fn new(
        item_count: usize,
        item_height: f32,
        row_builder: impl FnMut(usize) -> VirtualRow + 'static,
    ) -> Self {
        Self {
            item_count,
            item_height: item_height.max(20.0),
            gap: 6.0,
            flat: false,
            columns_mode: false,
            accent: None,
            selected: None,
            on_select: None,
            row_builder: Box::new(row_builder),
            row_art: None,
        }
    }

    /// Sets the vertical gap between rows; negative values become zero.
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g.max(0.0);
        self
    }

    /// Binds the highlighted row to a signal holding its index.
    pub fn selected(mut self, sig: Signal<usize>) -> Self {
        self.selected = Some(sig);
        self
    }

    /// Renders rows as borderless Apple-Music-style list rows with subtle alternating
    /// tint instead of boxed cards.
    pub fn flat(mut self) -> Self {
        self.flat = true;
        self.gap = 0.0;
        self
    }

    /// Lays out rows as a horizontal single-line media table (`[Art] Title | Subtitle | Time`).
    pub fn columns_mode(mut self) -> Self {
        self.columns_mode = true;
        self.flat = true;
        self.gap = 0.0;
        self
    }

    /// Overrides the active selection highlight color (defaults to the theme accent).
    pub fn accent(mut self, color: Color) -> Self {
        self.accent = Some(color);
        self
    }

    /// Supplies an optional album/thumbnail image per row index (deduplicated on GPU).
    pub fn row_art(mut self, f: impl FnMut(usize) -> Arc<DecodedImage> + 'static) -> Self {
        self.row_art = Some(Box::new(f));
        self
    }

    /// Registers a callback invoked with the index of a pressed row.
    pub fn on_select(mut self, f: impl FnMut(usize) + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
}

impl IntoElement for VirtualizedList {
    fn into_element(self) -> Element {
        Element::VirtualizedList(self)
    }
}

/// A container that displays exactly one of its prebuilt children.
///
/// All children are mounted once; the child whose index matches the signal is
/// visible and the others are hidden from layout. Changing the signal requests
/// a layout pass that swaps which child is shown without rebuilding the tree.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let page = create_signal(0usize);
/// let pages = Switcher::new(
///     page,
///     vec![
///         Text::new("First page").into_element(),
///         Text::new("Second page").into_element(),
///     ],
/// );
/// let element: Element = pages.into_element();
/// ```
pub struct Switcher {
    signal: Signal<usize>,
    children: Vec<Element>,
    layout: LayoutStyle,
}

impl Switcher {
    /// Creates a switcher over `children`, controlled by `signal`.
    ///
    /// An out-of-range signal value leaves all children hidden because no
    /// index matches. The switcher grows to fill its parent by default; use
    /// [`Switcher::flex_grow`] to make it hug a fixed-size child instead.
    pub fn new(signal: Signal<usize>, children: Vec<Element>) -> Self {
        Self {
            signal,
            children,
            layout: LayoutStyle {
                flex_grow: 1.0,
                ..Default::default()
            },
        }
    }

    /// Sets how much of the parent's free space the switcher absorbs.
    ///
    /// Use `0.0` when switching between fixed-size children such as sidebars.
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.layout.flex_grow = g;
        self
    }

    /// Sets the switcher's preferred width.
    pub fn width(mut self, w: Length) -> Self {
        self.layout.width = w;
        self
    }

    /// Sets an explicit height constraint on the switcher container.
    pub fn height(mut self, h: Length) -> Self {
        self.layout.height = h;
        self
    }
}

impl IntoElement for Switcher {
    fn into_element(self) -> Element {
        Element::Switcher(self)
    }
}

/// A container whose child is chosen by a boolean signal.
///
/// The selected builder is invoked when the widget mounts and again whenever
/// the condition flips. The previously mounted child subtree is unmounted (and
/// its effects and GPU textures released) before the new one is built, so
/// builder closures should produce fresh elements.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let logged_in = create_signal(true);
/// let content = Show::new(logged_in, || Text::new("Welcome").into_element())
///     .fallback(|| Text::new("Please sign in").into_element());
/// let element: Element = content.into_element();
/// ```
pub struct Show {
    condition: Signal<bool>,
    when_true: Option<Box<dyn FnMut() -> Element>>,
    when_false: Option<Box<dyn FnMut() -> Element>>,
    layout: LayoutStyle,
}

impl Show {
    /// Creates a container that builds `when_true` while `condition` is true.
    ///
    /// When the condition is false and no [`Show::fallback`] is provided, the
    /// container has no children. Both builder closures must be `'static`
    /// because the mounted widget owns them.
    pub fn new(condition: Signal<bool>, when_true: impl FnMut() -> Element + 'static) -> Self {
        Self {
            condition,
            when_true: Some(Box::new(when_true)),
            when_false: None,
            layout: LayoutStyle::default(),
        }
    }

    /// Sets the builder used while the condition is false.
    pub fn fallback(mut self, when_false: impl FnMut() -> Element + 'static) -> Self {
        self.when_false = Some(Box::new(when_false));
        self
    }
}

impl IntoElement for Show {
    fn into_element(self) -> Element {
        Element::Show(self)
    }
}

type ForEachItem = (u64, Option<WidgetId>, Option<Element>);
type ForEachBuild = Box<dyn FnMut(&mut HashMap<u64, WidgetId>) -> Vec<ForEachItem>>;

/// A keyed, signal-driven container that reconciles its children in place.
///
/// On mount and whenever the item signal changes, `ForEach` computes a key for
/// each item. Children with a key that is already mounted are reused (and stay
/// in their original order); children with new keys are built and mounted;
/// children whose keys disappear are unmounted. This preserves their state
/// across reordering as long as keys are stable.
///
/// Keys must be unique among the current items: the key-to-child map keeps
/// only one node per key, so duplicate keys make later reconciliation of those
/// rows ambiguous.
pub struct ForEach {
    layout: LayoutStyle,
    build: ForEachBuild,
    bind_signal: Box<dyn FnOnce(DirtyQueue, WidgetId) -> EffectId>,
}

impl ForEach {
    /// Creates a list container driven by `items`.
    ///
    /// `key_fn` returns the stable identity used for reconciliation;
    /// `item_builder` produces the [`Element`] for a newly seen item. Both
    /// closures must be `'static`. Initial children are mounted immediately,
    /// and later signal changes request a full layout pass.
    ///
    /// ```
    /// use cadmium::prelude::*;
    ///
    /// let items = create_signal(vec![(1u64, "Alpha"), (2, "Beta")]);
    /// let list = ForEach::new(
    ///     items,
    ///     |(key, _)| *key,
    ///     |(_, label)| Text::new(*label).into_element(),
    /// )
    /// .direction(FlexDir::Column)
    /// .gap(6.0);
    /// let element: Element = list.into_element();
    /// ```
    pub fn new<T: 'static>(
        items: Signal<Vec<T>>,
        mut key_fn: impl FnMut(&T) -> u64 + 'static,
        mut item_builder: impl FnMut(&T) -> Element + 'static,
    ) -> Self {
        let build = Box::new(move |mounted_keys: &mut HashMap<u64, WidgetId>| {
            let mut plan = Vec::new();
            let _ = items.try_with(|list| {
                plan.reserve(list.len());
                for item in list.iter() {
                    let key = key_fn(item);
                    if let Some(id) = mounted_keys.remove(&key) {
                        plan.push((key, Some(id), None));
                    } else {
                        plan.push((key, None, Some(item_builder(item))));
                    }
                }
            });
            plan
        });

        let bind_signal = Box::new(move |q: DirtyQueue, id: WidgetId| {
            create_effect(move || {
                let _ = items.try_with(|_| ());
                push_dirty(
                    &q,
                    id,
                    DirtyFlags {
                        state: true,
                        layout: true,
                        paint: true,
                    },
                );
            })
        });

        Self {
            layout: LayoutStyle::default(),
            build,
            bind_signal,
        }
    }

    /// Sets the direction in which reconciled children are laid out.
    pub fn direction(mut self, dir: FlexDir) -> Self {
        self.layout.direction = dir;
        self
    }

    /// Sets the logical-pixel gap between reconciled children.
    pub fn gap(mut self, gap: f32) -> Self {
        self.layout.gap = gap;
        self
    }

    /// Sets how much of the parent's free space the container absorbs.
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.layout.flex_grow = g;
        self
    }
}

impl IntoElement for ForEach {
    fn into_element(self) -> Element {
        Element::ForEach(self)
    }
}

/// A one-pixel horizontal rule painted with a fixed color.
///
/// The divider uses auto width and can be combined with [`Spacer`] to control
/// how much horizontal space it occupies.
pub struct Divider {
    color: Color,
}

impl Divider {
    /// Creates a divider painted with `color`.
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl IntoElement for Divider {
    fn into_element(self) -> Element {
        Element::Divider(self)
    }
}

/// An empty layout node that can absorb free space.
///
/// A spacer paints nothing; its only effect is the flex grow factor assigned
/// by [`Spacer::flex`].
pub struct Spacer {
    grow: f32,
}

impl Spacer {
    /// Creates a spacer with a flex grow factor of `1.0`.
    pub fn flex() -> Self {
        Self { grow: 1.0 }
    }
}

impl IntoElement for Spacer {
    fn into_element(self) -> Element {
        Element::Spacer(self)
    }
}

