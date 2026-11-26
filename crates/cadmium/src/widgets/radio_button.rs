/// Interactive radio button bound to a shared `Signal<T>` and a candidate `value`.
///
/// Several `RadioButton` builders that share one signal form a mutually
/// exclusive group: each button compares the signal's value with its own
/// candidate and renders an accent-filled circle with an inner dot only when
/// they are equal. Otherwise it renders a panel-colored circle with a hairline
/// border. An optional label or tooltip can accompany the circle.
///
/// # Reactive behavior
///
/// Pressing the row writes this button's candidate `value` into the shared
/// [`Signal`] with [`Signal::set`]. Selection is derived by
/// [`crate::reactive::create_memo`], so writes from any button or from other
/// app code update every mounted member of the group. `T` must implement
/// `PartialEq` for that comparison, `Clone` because the value is captured by
/// the click handler and the memo, and `'static` for reactive storage.
///
/// # Defaults
///
/// [`RadioButton::new`] starts with no label or tooltip. The circle is 18×18
/// logical pixels with a 9px radius; the selected dot is 7×7 with a 3.5px
/// radius. Labels, when added with [`RadioButton::label`], use 13px primary
/// text.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let plan = create_signal("Standard");
/// let choices = Container::column()
///     .child(RadioButton::new(plan, "Standard").label("Standard"))
///     .child(
///         RadioButton::new(plan, "Pro")
///             .label("Pro")
///             .tooltip("Adds advanced features"),
///     );
///
/// let element: Element = choices.into_element();
/// plan.set("Pro"); // a mounted group now highlights the Pro button
/// ```
pub struct RadioButton<T: PartialEq + Clone + 'static> {
    signal: Signal<T>,
    value: T,
    label: Option<String>,
    tooltip: Option<String>,
}

impl<T: PartialEq + Clone + 'static> RadioButton<T> {
    /// Creates a radio button for `value` controlled by `signal`.
    ///
    /// The button is selected whenever `signal` equals `value`; sharing the
    /// same signal with other radio buttons groups them together. No label or
    /// tooltip is set initially.
    pub fn new(signal: Signal<T>, value: T) -> Self {
        Self {
            signal,
            value,
            label: None,
            tooltip: None,
        }
    }

    /// Shows `label` beside the circle at 13px in the primary text tone.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the tooltip shown while the pointer rests over the whole row.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

/// Builds a reactive row whose selected circle tracks the shared value signal.
impl<T: PartialEq + Clone + 'static> IntoElement for RadioButton<T> {
    /// Returns a row whose selected state is memoized from the shared signal.
    fn into_element(self) -> Element {
        let sig = self.signal;
        let val_click = self.value.clone();
        let val_check = self.value;
        let is_selected = crate::reactive::create_memo(move || sig.get() == val_check);
        let light = MacTheme::light();

        let circle = Show::new(is_selected.signal(), move || {
            Container::row()
                .width(Length::Px(18.0))
                .height(Length::Px(18.0))
                .radius(9.0)
                .fill(light.accent)
                .align(AlignItems::Center)
                .justify(JustifyContent::Center)
                .child(
                    Container::new()
                        .width(Length::Px(7.0))
                        .height(Length::Px(7.0))
                        .radius(3.5)
                        .fill(light.text_on_accent),
                )
                .into_element()
        })
        .fallback(move || {
            Container::row()
                .width(Length::Px(18.0))
                .height(Length::Px(18.0))
                .radius(9.0)
                .material(Material::Classic {
                    fill: light.panel_bg,
                    fill_end: None,
                    gradient_dir: [0.0, 0.0],
                    border_color: light.hairline,
                    border_width: 1.2,
                    shadow: None,
                })
                .into_element()
        });

        let mut row = Container::row()
            .align(AlignItems::Center)
            .gap(8.0)
            .padding(Edges::xy(2.0, 2.0))
            .on_click(move || sig.set(val_click.clone()))
            .child(circle);
        if let Some(lbl) = self.label {
            row = row.child(Text::new(lbl).size(13.0).tone(TextTone::Primary));
        }
        if let Some(tt) = self.tooltip {
            row = row.tooltip(tt);
        }
        row.into_element()
    }
}
