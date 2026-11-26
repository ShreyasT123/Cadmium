/// Represents an actionable command item inside a [`CommandPalette`].
///
/// Holds a display label, an optional keyboard shortcut badge, and an
/// execution callback.
#[derive(Clone)]
pub struct CommandItem {
    /// The human-readable description of the command.
    pub label: String,
    /// Optional keyboard shortcut text badge (e.g. `"Ctrl+K"`).
    pub shortcut: Option<String>,
    /// The callback executed when the command is selected.
    pub on_execute: Arc<dyn Fn()>,
}

impl CommandItem {
    /// Creates a new command item with a label and execution closure.
    pub fn new(label: impl Into<String>, on_execute: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
            on_execute: Arc::new(on_execute),
        }
    }

    /// Attaches an optional keyboard shortcut badge.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// A fuzzy-searchable command palette modal overlay (`Cmd/Ctrl + K`).
///
/// Driven by an open signal and a query string signal, it displays a
/// frosted glass dialog containing a text input filter and a scrollable list
/// of matching [`CommandItem`]s. Clicking an item or pressing Enter executes
/// the command and closes the palette.
pub struct CommandPalette {
    is_open: Signal<bool>,
    query: Signal<String>,
    commands: Vec<CommandItem>,
}

impl CommandPalette {
    /// Creates a command palette controlled by visibility and query signals.
    pub fn new(is_open: Signal<bool>, query: Signal<String>, commands: Vec<CommandItem>) -> Self {
        Self {
            is_open,
            query,
            commands,
        }
    }
}

impl IntoElement for CommandPalette {
    fn into_element(self) -> Element {
        if !self.is_open.try_get().unwrap_or(false) {
            return Element::Container(Container::new().width(Length::Px(0.0)).height(Length::Px(0.0)));
        }

        let q_val = self.query.try_get().unwrap_or_default().to_lowercase();
        let filtered: Vec<_> = self.commands
            .into_iter()
            .filter(|c| q_val.is_empty() || c.label.to_lowercase().contains(&q_val))
            .collect();

        let mut list = ScrollView::new().gap(4.0);
        for cmd in filtered {
            let exec = cmd.on_execute.clone();
            let open_sig = self.is_open;
            let row = Container::row()
                .height(Length::Px(36.0))
                .padding(Edges::xy(12.0, 6.0))
                .align(AlignItems::Center)
                .radius(8.0)
                .hover_fill(Color::rgba(0.0, 122.0, 255.0, 0.35))
                .on_click(move || {
                    open_sig.set(false);
                    exec();
                })
                .child(Text::new(&cmd.label).size(13.0).weight(FontWeight::Medium))
                .child(Spacer::flex());

            if let Some(sc) = &cmd.shortcut {
                let row = row.child(Text::new(sc).size(11.0).color(Color::from_rgba8(142, 142, 147, 255)));
                list = list.child(row);
            } else {
                list = list.child(row);
            }
        }

        let palette_box = Container::column()
            .width(Length::Px(520.0))
            .height(Length::Px(340.0))
            .padding(Edges::all(14.0))
            .gap(12.0)
            .radius(16.0)
            .material(Material::Frosted {
                tint: Color::from_rgba8(36, 36, 40, 210),
                frost: 0.55,
                specular: 0.90,
                border_color: Color::from_rgba8(255, 255, 255, 45),
                border_width: 1.0,
                shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 180), 32.0, 12.0)),
            })
            .child(TextInput::new(self.query).placeholder("Type a command or search keys..."))
            .child(list);

        let open_sig_backdrop = self.is_open;
        let overlay = Container::column()
            .absolute()
            .inset(Inset::all(Length::Px(0.0)))
            .align(AlignItems::Center)
            .justify(JustifyContent::Center)
            .fill(Color::rgba(0.0, 0.0, 0.0, 0.45))
            .on_click(move || {
                open_sig_backdrop.set(false);
            })
            .child(palette_box);

        Element::Container(overlay)
    }
}