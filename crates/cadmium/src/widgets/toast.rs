/// Severity level determining the accent indicator of a [`ToastItem`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    /// Informational notification (blue accent).
    Info,
    /// Success confirmation (green accent).
    Success,
    /// Warning alert (amber accent).
    Warning,
    /// Error alert (red accent).
    Error,
}

impl ToastLevel {
    fn accent_color(self) -> Color {
        match self {
            Self::Info => Color::from_rgba8(10, 132, 255, 255),
            Self::Success => Color::from_rgba8(48, 209, 88, 255),
            Self::Warning => Color::from_rgba8(255, 159, 10, 255),
            Self::Error => Color::from_rgba8(255, 69, 58, 255),
        }
    }
}

/// Data model for a single notification toast displayed by [`ToastHost`].
#[derive(Debug, Clone)]
pub struct ToastItem {
    /// Unique stable key for keyed [`ForEach`] reconciliation.
    pub id: u64,
    /// Short notification heading.
    pub title: String,
    /// Optional detailed message body.
    pub message: Option<String>,
    /// Visual severity level.
    pub level: ToastLevel,
}

impl ToastItem {
    /// Creates a new toast item with a unique ID, title, and severity level.
    pub fn new(id: u64, title: impl Into<String>, level: ToastLevel) -> Self {
        Self {
            id,
            title: title.into(),
            message: None,
            level,
        }
    }

    /// Attaches a secondary message body beneath the title.
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }
}

/// Floating bottom-right overlay container rendering active [`ToastItem`]s.
///
/// Uses [`ForEach`] so pushing or dismissing toasts updates only the affected
/// notification cards without re-laying out the entire application window.
pub struct ToastHost {
    toasts: Signal<Vec<ToastItem>>,
}

impl ToastHost {
    /// Creates a toast host subscribed to the active toast queue signal.
    pub fn new(toasts: Signal<Vec<ToastItem>>) -> Self {
        Self { toasts }
    }
}

impl IntoElement for ToastHost {
    fn into_element(self) -> Element {
        let toasts_sig = self.toasts;
        let list = ForEach::new(toasts_sig, |item| item.id, move |item| {
            let id = item.id;
            let accent = item.level.accent_color();
            let mut text_col = Column::new()
                .flex_grow(1.0)
                .gap(2.0)
                .child(
                    Text::new(item.title.clone())
                        .size(13.0)
                        .weight(FontWeight::Semibold)
                        .tone(TextTone::Primary),
                );
            if let Some(msg) = &item.message {
                text_col = text_col.child(Text::new(msg.clone()).size(11.5).tone(TextTone::Secondary));
            }

            Container::row()
                .width(Length::Px(320.0))
                .padding(Edges::all(12.0))
                .gap(10.0)
                .radius(12.0)
                .align(AlignItems::Center)
                .material(Material::Frosted {
                    tint: Color::from_rgba8(38, 38, 44, 215),
                    frost: 0.48,
                    specular: 0.85,
                    border_color: Color::from_rgba8(255, 255, 255, 36),
                    border_width: 1.0,
                    shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 140), 18.0, 6.0)),
                })
                .child(
                    Container::new()
                        .width(Length::Px(4.0))
                        .height(Length::Px(28.0))
                        .radius(2.0)
                        .fill(accent),
                )
                .child(text_col)
                .child(Button::new("×").ghost().on_click(move || {
                    toasts_sig.update(|list| list.retain(|t| t.id != id));
                }))
                .into_element()
        })
        .gap(8.0);

        Container::column()
            .absolute()
            .inset(Inset {
                left: Length::Auto,
                top: Length::Auto,
                right: Length::Px(18.0),
                bottom: Length::Px(18.0),
            })
            .child(list)
            .into_element()
    }
}
