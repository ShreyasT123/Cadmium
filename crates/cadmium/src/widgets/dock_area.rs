/// Represents a single tab inside a [`DockGroup`].
pub struct DockTab {
    /// Unique identifier for the tab.
    pub id: String,
    /// Display title shown in the pane tab strip.
    pub title: String,
    /// Whether a close button is rendered on this tab.
    pub closable: bool,
    /// Content element mounted inside the pane when this tab is active.
    pub content: Element,
}

impl DockTab {
    /// Creates a new dock tab with a stable ID, title, and content element.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            closable: true,
            content: content.into_element(),
        }
    }

    /// Configures whether the tab displays a close button.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

/// A tabbed pane group hosted inside a [`DockArea`].
///
/// Renders a compact IDE-style tab bar at the top of the pane and mounts all
/// tab contents inside a [`Switcher`] so hidden tabs retain their scroll and
/// cursor state.
pub struct DockGroup {
    /// Signal storing the currently active tab index.
    pub active_tab: Signal<usize>,
    /// Tabs belonging to this pane group.
    pub tabs: Vec<DockTab>,
    /// Optional callback invoked when a closable tab's close button is clicked.
    #[allow(clippy::type_complexity)]
    pub on_close_tab: Option<Arc<dyn Fn(usize, &str)>>,
}

impl DockGroup {
    /// Creates a new tabbed dock group bound to `active_tab`.
    pub fn new(active_tab: Signal<usize>, tabs: Vec<DockTab>) -> Self {
        Self {
            active_tab,
            tabs,
            on_close_tab: None,
        }
    }

    /// Registers a callback invoked when a user clicks a tab's close button.
    pub fn on_close(mut self, f: impl Fn(usize, &str) + 'static) -> Self {
        self.on_close_tab = Some(Arc::new(f));
        self
    }
}

impl IntoElement for DockGroup {
    fn into_element(self) -> Element {
        let active_sig = self.active_tab;
        let on_close = self.on_close_tab.clone();
        let active_idx = active_sig.try_get().unwrap_or(0);

        let mut tab_strip = Row::new()
            .height(Length::Px(34.0))
            .padding(Edges::xy(8.0, 4.0))
            .gap(4.0)
            .align(AlignItems::Center)
            .fill(Color::from_rgba8(28, 28, 32, 220))
            .border(Color::from_rgba8(255, 255, 255, 18), 1.0);

        let mut bodies: Vec<Element> = Vec::with_capacity(self.tabs.len());

        for (i, tab) in self.tabs.into_iter().enumerate() {
            let is_active = i == active_idx;
            let tab_id = tab.id.clone();

            let mut pill = Container::row()
                .height(Length::Px(26.0))
                .padding(Edges::xy(10.0, 0.0))
                .gap(6.0)
                .radius(6.0)
                .align(AlignItems::Center)
                .fill_when(
                    move || active_sig.try_get() == Some(i),
                    Color::from_rgba8(255, 255, 255, 28),
                    Color::TRANSPARENT,
                )
                .hover_fill(Color::from_rgba8(255, 255, 255, 16))
                .on_click(move || active_sig.set(i))
                .child(
                    Text::new(tab.title)
                        .size(12.0)
                        .weight(if is_active {
                            FontWeight::Semibold
                        } else {
                            FontWeight::Medium
                        })
                        .tone(if is_active {
                            TextTone::Primary
                        } else {
                            TextTone::Secondary
                        }),
                );

            if tab.closable {
                let close_cb = on_close.clone();
                pill = pill.child(
                    Container::row()
                        .width(Length::Px(16.0))
                        .height(Length::Px(16.0))
                        .radius(8.0)
                        .align(AlignItems::Center)
                        .justify(JustifyContent::Center)
                        .hover_fill(Color::from_rgba8(255, 255, 255, 30))
                        .on_click(move || {
                            if let Some(cb) = &close_cb {
                                cb(i, &tab_id);
                            }
                        })
                        .child(Text::new("×").size(11.0).tone(TextTone::Secondary)),
                );
            }

            tab_strip = tab_strip.child(pill);
            bodies.push(tab.content);
        }

        let body = if bodies.is_empty() {
            Container::column()
                .flex_grow(1.0)
                .align(AlignItems::Center)
                .justify(JustifyContent::Center)
                .child(Text::new("Empty Dock Pane").size(12.5).tone(TextTone::Tertiary))
                .into_element()
        } else {
            Switcher::new(active_sig, bodies).into_element()
        };

        Container::column()
            .flex_grow(1.0)
            .radius(10.0)
            .border(Color::from_rgba8(255, 255, 255, 20), 1.0)
            .child(tab_strip)
            .child(body)
            .into_element()
    }
}

/// Recursive layout node inside a [`DockArea`].
pub enum DockNode {
    /// A leaf tabbed pane group.
    Pane(DockGroup),
    /// A resizable split dividing two child dock subtrees.
    Split {
        /// Horizontal (side-by-side) or vertical (top-bottom) split direction.
        direction: SplitDirection,
        /// Reactive split ratio (`0.0..=1.0`) controlled by dragging the divider.
        ratio: Signal<f32>,
        /// Minimum allowed split ratio.
        min_ratio: f32,
        /// Maximum allowed split ratio.
        max_ratio: f32,
        /// Left or top dock subtree.
        first: Box<DockNode>,
        /// Right or bottom dock subtree.
        second: Box<DockNode>,
    },
}

impl DockNode {
    /// Creates a leaf pane node from a [`DockGroup`].
    pub fn pane(group: DockGroup) -> Self {
        Self::Pane(group)
    }

    /// Creates a horizontal (left/right) split node between two subtrees.
    pub fn split_horizontal(ratio: Signal<f32>, first: DockNode, second: DockNode) -> Self {
        Self::Split {
            direction: SplitDirection::Horizontal,
            ratio,
            min_ratio: 0.15,
            max_ratio: 0.85,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Creates a vertical (top/bottom) split node between two subtrees.
    pub fn split_vertical(ratio: Signal<f32>, first: DockNode, second: DockNode) -> Self {
        Self::Split {
            direction: SplitDirection::Vertical,
            ratio,
            min_ratio: 0.15,
            max_ratio: 0.85,
            first: Box::new(first),
            second: Box::new(second),
        }
    }
}

impl IntoElement for DockNode {
    fn into_element(self) -> Element {
        match self {
            DockNode::Pane(group) => group.into_element(),
            DockNode::Split {
                direction,
                ratio,
                min_ratio,
                max_ratio,
                first,
                second,
            } => SplitPane::new(direction, ratio)
                .bounds(min_ratio, max_ratio)
                .first((*first).into_element())
                .second((*second).into_element())
                .into_element(),
        }
    }
}

/// Multi-pane workspace docking container for IDEs and workstation apps.
///
/// Composes recursive [`DockNode`] splits and [`DockGroup`] tab strips using
/// Cadmium's [`SplitPane`] and [`Switcher`] primitives.
pub struct DockArea {
    root: DockNode,
    padding: Edges,
}

impl DockArea {
    /// Creates a new workspace dock area rooted at `root`.
    pub fn new(root: DockNode) -> Self {
        Self {
            root,
            padding: Edges::all(6.0),
        }
    }

    /// Sets outer padding around the dock workspace.
    pub fn padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }
}

impl IntoElement for DockArea {
    fn into_element(self) -> Element {
        Container::column()
            .flex_grow(1.0)
            .padding(self.padding)
            .child(self.root.into_element())
            .into_element()
    }
}
