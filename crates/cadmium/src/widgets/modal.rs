/// Centered modal dialog overlay controlled by a `Signal<bool>`.
///
/// While the bound signal is true, a [`Show`] branch mounts a translucent
/// full-parent backdrop with a [`Card`] centered inside it. The dialog appears
/// only for that signal state; closing is done by setting the signal to
/// `false`, which the title-bar ✕ button and (optionally) a backdrop click do
/// on the user's behalf. Content children live inside the card, below an
/// optional title row.
///
/// # Reactive behavior
///
/// Visibility tracks the signal, so code elsewhere can open or close a mounted
/// dialog by writing it. Pressing the ✕ button always sets the signal to
/// `false`; pressing the backdrop does so only while
/// [`Modal::close_on_backdrop`] is enabled. The ✕ button is present only when a
/// title was supplied.
///
/// # Defaults
///
/// [`Modal::new`] uses a 420px-wide card, no title, no children, and
/// `close_on_backdrop = true`. [`Modal::title`] adds a header with 16px
/// semibold primary text, a flexible spacer, and the ghost ✕ button;
/// [`Modal::width`] changes the card width; [`Modal::child`] and
/// [`Modal::children`] append content beneath that header.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let show_confirm = create_signal(false);
/// let dialog = Modal::new(show_confirm)
///     .title("Discard changes?")
///     .width(Length::Px(360.0))
///     .child(Text::new("Your edits will be lost.").size(13.0))
///     .child(
///         Row::new()
///             .gap(8.0)
///             .child(Button::new("Cancel").on_click(|| {}))
///             .child(Button::new("Discard").primary().on_click(|| {})),
///     );
///
/// let element: Element = dialog.into_element();
/// show_confirm.set(true); // a mounted instance now shows the dialog
/// ```
pub struct Modal {
    open: Signal<bool>,
    title: Option<String>,
    width: Length,
    children: Vec<Element>,
    close_on_backdrop: bool,
}

impl Modal {
    /// Creates a dialog shown while `open` is true.
    ///
    /// The builder starts without a title or content; the title bar, and with
    /// it the ✕ close button, is created only when [`Modal::title`] is called.
    pub fn new(open: Signal<bool>) -> Self {
        Self {
            open,
            title: None,
            width: Length::Px(420.0),
            children: Vec::new(),
            close_on_backdrop: true,
        }
    }

    /// Adds a title row with a ghost ✕ button that closes the dialog.
    ///
    /// The title is stored as an owned [`String`] and rendered at 16px in the
    /// primary text tone. Without a title, neither the row nor the ✕ button is
    /// shown.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the width of the dialog card.
    ///
    /// The default is 420 logical pixels; other layout values are resolved by
    /// the layout engine.
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Controls whether clicking the backdrop closes the dialog.
    ///
    /// The default is `true`. Setting this to `false` leaves the title-bar ✕
    /// button as the built-in way to close a titled dialog; application code
    /// can always set the bound signal to `false`.
    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// Appends one element to the dialog's content.
    ///
    /// The child is converted into an [`Element`] now and is placed inside the
    /// card after the optional title row.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    /// Appends every item from `iter` to the dialog's content.
    ///
    /// Items are converted into [`Element`]s and placed in iteration order,
    /// after children added with [`Modal::child`].
    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        for item in iter {
            self.children.push(item.into_element());
        }
        self
    }
}

/// Wraps the dialog card and backdrop in a [`Show`] driven by the open signal.
impl IntoElement for Modal {
    /// Returns the signal-driven backdrop and dialog card as one element.
    fn into_element(self) -> Element {
        let open = self.open;
        let title = self.title;
        let width = self.width;
        let close_on_backdrop = self.close_on_backdrop;
        let body_cell = std::rc::Rc::new(std::cell::RefCell::new(Some(self.children)));

        Show::new(open, move || {
            let children = body_cell.borrow_mut().take().unwrap_or_default();
            let mut card = Card::new().width(width).gap(12.0);
            if let Some(t) = &title {
                card = card.child(
                    Row::new()
                        .align(AlignItems::Center)
                        .child(
                            Text::new(t.clone())
                                .size(16.0)
                                .weight(FontWeight::Semibold)
                                .tone(TextTone::Primary),
                        )
                        .child(Spacer::flex())
                        .child(Button::new("✕").ghost().on_click(move || open.set(false))),
                );
            }
            card = card.children(children);

            let mut backdrop = Container::column()
                .absolute()
                .inset(Inset {
                    left: Length::Px(0.0),
                    right: Length::Px(0.0),
                    top: Length::Px(0.0),
                    bottom: Length::Px(0.0),
                })
                .align(AlignItems::Center)
                .justify(JustifyContent::Center)
                .fill(Color::from_rgba8(0, 0, 0, 110))
                .child(card);
            if close_on_backdrop {
                backdrop = backdrop.on_click(move || open.set(false));
            }
            backdrop.into_element()
        })
        .into_element()
    }
}
