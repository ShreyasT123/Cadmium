/// Represents a clickable entry or separator inside a [`MenuBar`] dropdown.
#[derive(Clone)]
pub struct MenuAction {
    /// Display text (`None` denotes a horizontal divider).
    pub label: Option<String>,
    /// Optional shortcut hint displayed on the right side (e.g. `"Ctrl+S"`).
    pub shortcut: Option<String>,
    /// Callback executed when the action is clicked.
    pub on_trigger: Option<Arc<dyn Fn()>>,
}

impl MenuAction {
    /// Creates a clickable menu action with a label and callback.
    pub fn new(label: impl Into<String>, on_trigger: impl Fn() + 'static) -> Self {
        Self {
            label: Some(label.into()),
            shortcut: None,
            on_trigger: Some(Arc::new(on_trigger)),
        }
    }

    /// Creates a horizontal separator line between menu groups.
    pub fn separator() -> Self {
        Self {
            label: None,
            shortcut: None,
            on_trigger: None,
        }
    }

    /// Attaches an optional keyboard shortcut hint label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// A top-level menu category inside a [`MenuBar`] (e.g. `"File"`, `"Edit"`).
#[derive(Clone)]
pub struct MenuGroup {
    /// Menu button title shown in the menu bar.
    pub title: String,
    /// Entries displayed when this menu is opened.
    pub items: Vec<MenuAction>,
}

impl MenuGroup {
    /// Creates a menu group with a title and list of actions.
    pub fn new(title: impl Into<String>, items: Vec<MenuAction>) -> Self {
        Self {
            title: title.into(),
            items,
        }
    }
}

/// Desktop application menu bar with frosted glass dropdown overlays.
///
/// Renders top-level menu buttons along a horizontal bar and displays an
/// overlay menu for the currently opened index.
pub struct MenuBar {
    active_menu: Signal<Option<usize>>,
    menus: Vec<MenuGroup>,
}

impl MenuBar {
    /// Creates a menu bar driven by `active_menu` (`None` = all menus closed).
    pub fn new(active_menu: Signal<Option<usize>>, menus: Vec<MenuGroup>) -> Self {
        Self { active_menu, menus }
    }
}

impl IntoElement for MenuBar {
    fn into_element(self) -> Element {
        let active_sig = self.active_menu;
        let is_open = create_signal(active_sig.try_get().flatten().is_some());
        let active_idx = create_signal(active_sig.try_get().flatten().unwrap_or(0));

        let groups = Rc::new(self.menus);

        let eff = create_effect(move || {
            match active_sig.try_get().flatten() {
                Some(idx) => {
                    active_idx.set(idx);
                    is_open.set(true);
                }
                None => is_open.set(false),
            }
        });
        let _ = eff;

        let mut bar = Row::new()
            .height(Length::Px(30.0))
            .padding(Edges::xy(10.0, 0.0))
            .gap(4.0)
            .align(AlignItems::Center)
            .fill(Color::from_rgba8(24, 24, 28, 220))
            .border(Color::from_rgba8(255, 255, 255, 16), 1.0);

        for (i, group) in groups.iter().enumerate() {
            bar = bar.child(
                Container::row()
                    .height(Length::Px(24.0))
                    .padding(Edges::xy(10.0, 2.0))
                    .radius(5.0)
                    .align(AlignItems::Center)
                    .hover_fill(Color::from_rgba8(255, 255, 255, 24))
                    .on_click(move || {
                        if active_sig.try_get().flatten() == Some(i) {
                            active_sig.set(None);
                        } else {
                            active_sig.set(Some(i));
                        }
                    })
                    .child(Text::new(group.title.clone()).size(12.5).tone(TextTone::Primary)),
            );
        }

        let dropdown_groups = groups.clone();
        let dropdown_overlay = Show::new(is_open, move || {
            let mut menu_panels: Vec<Element> = Vec::with_capacity(dropdown_groups.len());

            for (i, group) in dropdown_groups.iter().enumerate() {
                let x_offset = 10.0 + (i as f32) * 64.0;
                let mut list = Column::new().gap(2.0);

                for item in &group.items {
                    if let Some(lbl) = item.label.clone() {
                        let cb = item.on_trigger.clone();
                        let mut row = Container::row()
                            .height(Length::Px(28.0))
                            .padding(Edges::xy(10.0, 4.0))
                            .radius(6.0)
                            .align(AlignItems::Center)
                            .hover_fill(Color::from_rgba8(0, 122, 255, 175))
                            .on_click(move || {
                                active_sig.set(None);
                                if let Some(f) = &cb {
                                    f();
                                }
                            })
                            .child(Text::new(lbl).size(12.5).tone(TextTone::Primary))
                            .child(Spacer::flex());
                        if let Some(sc) = item.shortcut.clone() {
                            row = row.child(Text::new(sc).size(11.0).tone(TextTone::Secondary));
                        }
                        list = list.child(row);
                    } else {
                        list = list.child(Divider::new(Color::from_rgba8(255, 255, 255, 24)));
                    }
                }

                menu_panels.push(
                    Container::column()
                        .absolute()
                        .inset(Inset {
                            left: Length::Px(x_offset),
                            top: Length::Px(32.0),
                            right: Length::Auto,
                            bottom: Length::Auto,
                        })
                        .width(Length::Px(210.0))
                        .padding(Edges::all(6.0))
                        .radius(10.0)
                        .material(Material::Frosted {
                            tint: Color::from_rgba8(38, 38, 42, 210),
                            frost: 0.50,
                            specular: 0.85,
                            border_color: Color::from_rgba8(255, 255, 255, 36),
                            border_width: 1.0,
                            shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 140), 20.0, 8.0)),
                        })
                        .child(list)
                        .into_element(),
                );
            }

            Container::new()
                .absolute()
                .inset(Inset::all(Length::Px(0.0)))
                .on_click(move || active_sig.set(None))
                .child(Switcher::new(active_idx, menu_panels))
                .into_element()
        });

        Stack::new()
            .child(bar)
            .child(dropdown_overlay)
            .into_element()
    }
}

/// A horizontal desktop toolbar container for action buttons, dividers, and inputs.
pub struct Toolbar {
    inner: Container,
}

impl Toolbar {
    /// Creates a standard 40px-tall horizontal desktop toolbar.
    pub fn new() -> Self {
        Self {
            inner: Container::row()
                .height(Length::Px(40.0))
                .padding(Edges::xy(12.0, 0.0))
                .gap(8.0)
                .align(AlignItems::Center)
                .fill(Color::from_rgba8(32, 32, 36, 220))
                .border(Color::from_rgba8(255, 255, 255, 18), 1.0),
        }
    }

    /// Appends an element (button, input, spacer, etc.) to the toolbar.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    /// Appends multiple elements to the toolbar.
    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.inner = self.inner.children(iter);
        self
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Toolbar {
    fn into_element(self) -> Element {
        self.inner.into_element()
    }
}
