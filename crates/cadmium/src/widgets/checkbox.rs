/// Interactive boolean checkbox bound to a `Signal<bool>`.
///
/// The checkbox builds a clickable row whose square indicator is produced by
/// [`Show`]: the checked branch paints an accent square with a bold check
/// mark, and the fallback paints a panel-colored square with a hairline
/// border. The row is the hit target, so pressing either the indicator or an
/// optional label toggles the bound signal.
///
/// # Reactive behavior
///
/// Clicking inverts the value with [`Signal::update`]. Because the indicator
/// comes from a `Show` bound to that same signal, changes made elsewhere in
/// the app also switch the rendered state when the tree updates.
///
/// # Defaults
///
/// [`Checkbox::new`] starts with no label or tooltip. The indicator is 18×18
/// logical pixels with a 5px radius; the label, when supplied by
/// [`Checkbox::label`], is 13px with the primary text tone.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let accepted = create_signal(false);
/// let consent = Checkbox::new(accepted)
///     .label("I agree to the terms")
///     .tooltip("Required to continue");
///
/// let element: Element = consent.into_element();
/// accepted.set(true); // a mounted instance now shows the checked state
/// ```
pub struct Checkbox {
    signal: Signal<bool>,
    label: Option<String>,
    tooltip: Option<String>,
}

impl Checkbox {
    /// Creates a checkbox controlled by `signal`.
    ///
    /// The visual state follows the signal when the element is mounted; no
    /// label or tooltip is shown until one is configured.
    pub fn new(signal: Signal<bool>) -> Self {
        Self {
            signal,
            label: None,
            tooltip: None,
        }
    }

    /// Shows `label` beside the indicator at 13px in the primary text tone.
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

/// Builds a [`Show`]-backed row whose indicator follows the bound signal.
impl IntoElement for Checkbox {
    /// Returns a row whose indicator is rebuilt from the bound boolean signal.
    fn into_element(self) -> Element {
        let sig = self.signal;
        let light = MacTheme::light();
        let box_view = Show::new(sig, move || {
            Container::row()
                .width(Length::Px(18.0))
                .height(Length::Px(18.0))
                .radius(5.0)
                .fill(light.accent)
                .align(AlignItems::Center)
                .justify(JustifyContent::Center)
                .child(
                    Text::new("✓")
                        .size(12.0)
                        .weight(FontWeight::Bold)
                        .color(light.text_on_accent),
                )
                .into_element()
        })
        .fallback(move || {
            Container::row()
                .width(Length::Px(18.0))
                .height(Length::Px(18.0))
                .radius(5.0)
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
            .on_click(move || sig.update(|v| *v = !*v))
            .child(box_view);
        if let Some(lbl) = self.label {
            row = row.child(Text::new(lbl).size(13.0).tone(TextTone::Primary));
        }
        if let Some(tt) = self.tooltip {
            row = row.tooltip(tt);
        }
        row.into_element()
    }
}
