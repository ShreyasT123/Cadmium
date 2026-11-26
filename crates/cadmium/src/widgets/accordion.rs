/// Collapsible disclosure section with a clickable header and toggleable body.
///
/// An `Accordion` builds a column containing a rounded header row and a body
/// wrapped in [`Show`]. Pressing the header toggles the bound boolean signal;
/// the header arrow (`▾` when open, `▸` when closed) is produced by
/// [`Text::reactive`], so it follows the signal even when the change comes from
/// elsewhere in the app. The body uses the same signal to show its children,
/// which are laid out in a vertically stacked column below the header.
///
/// # Defaults
///
/// [`Accordion::new`] starts with no children and a collapsed state determined
/// by the signal's current value. The header uses 10px/8px horizontal and
/// vertical padding, an 8px corner radius, and 13.5px semibold primary text;
/// the body indents children by 22px on the left and spaces them by 6px.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let details_open = create_signal(false);
/// let section = Accordion::new("Advanced options", details_open)
///     .child(Text::new("Enable logging").size(13.0))
///     .child(Checkbox::new(create_signal(true)).label("Verbose output"));
///
/// let element: Element = section.into_element();
/// ```
pub struct Accordion {
    title: String,
    expanded: Signal<bool>,
    children: Vec<Element>,
}

impl Accordion {
    /// Creates a section titled `title` whose expansion follows `expanded`.
    ///
    /// The title is stored as an owned [`String`]. The initial appearance is
    /// determined by the signal when the resulting element is mounted.
    pub fn new(title: impl Into<String>, expanded: Signal<bool>) -> Self {
        Self {
            title: title.into(),
            expanded,
            children: Vec::new(),
        }
    }

    /// Appends one child to the collapsible body.
    ///
    /// Children are converted into [`Element`]s now and inserted after any
    /// children added previously.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    /// Appends every item from `iter` to the collapsible body.
    ///
    /// Items are converted into [`Element`]s and placed in iteration order,
    /// after children added with [`Accordion::child`].
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

/// Builds the header and a [`Show`]-controlled body for the expansion signal.
impl IntoElement for Accordion {
    /// Returns the header and signal-driven body as one column element.
    fn into_element(self) -> Element {
        let expanded = self.expanded;
        let title = self.title;
        let body_cell = std::rc::Rc::new(std::cell::RefCell::new(Some(self.children)));

        let header = Container::row()
            .align(AlignItems::Center)
            .padding(Edges::xy(10.0, 8.0))
            .radius(8.0)
            .on_click(move || expanded.update(|v| *v = !*v))
            .child(
                Text::reactive(move || {
                    let arrow = if expanded.get() { "▾" } else { "▸" };
                    format!("{arrow}  {title}")
                })
                .size(13.5)
                .weight(FontWeight::Semibold)
                .tone(TextTone::Primary),
            );

        let body = Show::new(expanded, move || {
            let children = body_cell.borrow_mut().take().unwrap_or_default();
            Container::column()
                .padding(Edges {
                    left: 22.0,
                    right: 8.0,
                    top: 4.0,
                    bottom: 8.0,
                })
                .gap(6.0)
                .children(children)
                .into_element()
        });

        Container::column()
            .gap(2.0)
            .child(header)
            .child(body)
            .into_element()
    }
}
