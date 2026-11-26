/// The visual style of a [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// A standard bordered button using the theme's panel background.
    Secondary,
    /// An emphasized button filled with the theme's accent color.
    Primary,
    /// A borderless button that paints a background only on hover.
    Ghost,
    /// A translucent button painted with the glass renderer.
    Glass,
}

/// A clickable button with optional SVG icon and macOS-style variants.
///
/// Buttons are focusable and invoke their callback on pointer-down. Their
/// label is measured during layout and can be combined with an icon; an
/// empty label with an icon is sized as a 30×30 square.
///
/// # Defaults
///
/// [`Button::new`] creates a secondary button with auto width, 30-pixel
/// height, a 14-pixel icon size, square-ish 7-pixel corners, no icon, no
/// tooltip, and no callback.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let save = Button::new("Save")
///     .primary()
///     .tooltip("Save changes")
///     .on_click(|| println!("save pressed"));
///
/// let delete = Button::new("Delete").ghost().on_click(|| {});
/// let ui = Row::new().gap(8.0).child(save).child(delete);
/// let element: Element = ui.into_element();
/// ```
pub struct Button {
    label: String,
    accent: bool,
    variant: ButtonVariant,
    tooltip: Option<String>,
    on_click: Option<Box<dyn FnMut()>>,
    svg_icon: Option<String>,
    icon_size: f32,
    width: Length,
    height: Length,
    pill: bool,
}

impl Button {
    /// Creates a secondary button with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            accent: false,
            variant: ButtonVariant::Secondary,
            tooltip: None,
            on_click: None,
            svg_icon: None,
            icon_size: 14.0,
            width: Length::Auto,
            height: Length::Px(30.0),
            pill: false,
        }
    }

    /// Creates a 30×30 icon-only button with no visible label.
    ///
    /// The SVG is rasterized at twice `icon_size` when mounted.
    pub fn icon_only(svg: impl Into<String>, icon_size: f32) -> Self {
        let mut b = Self::new("");
        b.svg_icon = Some(svg.into());
        b.icon_size = icon_size;
        b.width = Length::Px(30.0);
        b.height = Length::Px(30.0);
        b
    }

    /// Adds an SVG icon before the label, rasterized at twice `icon_size`.
    pub fn icon(mut self, svg: impl Into<String>, icon_size: f32) -> Self {
        self.svg_icon = Some(svg.into());
        self.icon_size = icon_size;
        self
    }

    /// Uses the accent-filled [`ButtonVariant::Primary`] appearance.
    pub fn primary(mut self) -> Self {
        self.accent = true;
        self.variant = ButtonVariant::Primary;
        self
    }

    /// Uses the borderless [`ButtonVariant::Ghost`] appearance.
    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    /// Uses the translucent [`ButtonVariant::Glass`] appearance.
    pub fn glass(mut self) -> Self {
        self.variant = ButtonVariant::Glass;
        self
    }

    /// Renders the button with fully rounded (pill-shaped) corners.
    pub fn pill(mut self) -> Self {
        self.pill = true;
        self
    }

    /// Overrides the button's width and height.
    ///
    /// The default is [`Length::Auto`] wide and 30 logical pixels high.
    pub fn size(mut self, w: Length, h: Length) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    /// Sets the tooltip shown when the pointer rests over the button.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Registers a callback invoked when the button is pressed (pointer-down).
    ///
    /// The callback must be `'static` because the mounted widget owns it.
    pub fn on_click(mut self, f: impl FnMut() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl IntoElement for Button {
    fn into_element(self) -> Element {
        Element::Button(self)
    }
}

/// A two-state toggle bound to a [`Signal`] of `bool`.
///
/// Pressing the switch inverts the signal value. Mounting installs an effect
/// that repaints the knob and track whenever the signal changes, so updates
/// from elsewhere in the app are reflected automatically.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let dark_mode = create_signal(false);
/// let toggle = Switch::new(dark_mode).tooltip("Use dark appearance");
/// let element: Element = toggle.into_element();
/// ```
pub struct Switch {
    signal: Signal<bool>,
    tooltip: Option<String>,
}

impl Switch {
    /// Creates a switch controlled by `signal`.
    pub fn new(signal: Signal<bool>) -> Self {
        Self {
            signal,
            tooltip: None,
        }
    }

    /// Sets the tooltip shown when the pointer rests over the switch.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Switch {
    fn into_element(self) -> Element {
        Element::Switch(self)
    }
}

/// A continuous value control bound to a [`Signal`] of `f32`.
///
/// The laid-out width is divided into the range `min..=max`; dragging maps the
/// pointer's horizontal position to a value and writes it to the signal.
/// Painting normalizes the signal back into that range to place the thumb.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let volume = create_signal(0.5f32);
/// let slider = Slider::new(volume, 0.0, 1.0)
///     .width(200.0)
///     .tooltip("Volume");
/// let element: Element = slider.into_element();
/// ```
pub struct Slider {
    signal: Signal<f32>,
    min: f32,
    max: f32,
    width: f32,
    tooltip: Option<String>,
}

impl Slider {
    /// Creates a slider for values between `min` and `max`.
    ///
    /// The initial width is 160 logical pixels and the height is fixed at 24.
    pub fn new(signal: Signal<f32>, min: f32, max: f32) -> Self {
        Self {
            signal,
            min,
            max,
            width: 160.0,
            tooltip: None,
        }
    }

    /// Sets the track width in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Sets the tooltip shown when the pointer rests over the slider.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Slider {
    fn into_element(self) -> Element {
        Element::Slider(self)
    }
}

/// A read-only progress track driven by a [`Signal`] of `f32`.
///
/// The signal is interpreted as a fraction and clamped to `0.0..=1.0` at paint
/// time; changes repaint the filled portion automatically. Unlike [`Slider`],
/// the widget does not write back to the signal.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let progress = create_signal(0.25f32);
/// let bar = ProgressBar::new(progress)
///     .width(Length::Px(220.0))
///     .height(6.0)
///     .color(Color::from_rgba8(10, 132, 255, 255));
/// let element: Element = bar.into_element();
/// ```
pub struct ProgressBar {
    signal: Signal<f32>,
    width: Length,
    height: f32,
    color: Option<Color>,
}

impl ProgressBar {
    /// Creates a progress bar reading `signal`.
    ///
    /// Defaults to auto width, a 4-pixel height, and the theme's accent color.
    pub fn new(signal: Signal<f32>) -> Self {
        Self {
            signal,
            width: Length::Auto,
            height: 4.0,
            color: None,
        }
    }

    /// Sets the width of the whole track.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }

    /// Sets the track height; values below 2 pixels are raised to 2.
    pub fn height(mut self, h: f32) -> Self {
        self.height = h.max(2.0);
        self
    }

    /// Overrides the fill color, falling back to the theme accent by default.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

impl IntoElement for ProgressBar {
    fn into_element(self) -> Element {
        Element::ProgressBar(self)
    }
}

/// A single-line text editor bound to a [`Signal`] of `String`.
///
/// Typing, deletion, and selection update the signal; external changes to the
/// signal are synchronized back into the mounted text block. The caret starts
/// at the end of the initial value, and the placeholder is painted while the
/// value is empty.
///
/// # Defaults
///
/// A 240-pixel width, 34-pixel height, nearly opaque white background, and
/// the placeholder `"Search or type..."`.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let query = create_signal(String::new());
/// let search = TextInput::new(query)
///     .placeholder("Search documents")
///     .width(Length::Px(320.0));
/// let element: Element = search.into_element();
/// ```
pub struct TextInput {
    signal: Signal<String>,
    placeholder: String,
    material: Material,
    width: Length,
    tooltip: Option<String>,
}

impl TextInput {
    /// Creates a text editor that reads and writes `signal`.
    pub fn new(signal: Signal<String>) -> Self {
        Self {
            signal,
            placeholder: "Search or type...".into(),
            material: Material::solid(Color::from_rgba8(255, 255, 255, 240)),
            width: Length::Px(240.0),
            tooltip: None,
        }
    }

    /// Sets the gray text shown when the editor's value is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Replaces the editor's background [`Material`].
    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    /// Sets the editor's width; the height is fixed at 34 logical pixels.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }

    /// Sets the tooltip shown when the pointer rests over the editor.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for TextInput {
    fn into_element(self) -> Element {
        Element::TextInput(self)
    }
}

/// A multiline text editor bound to a [`Signal`] of `String`.
///
/// Like [`TextInput`], edits are written back to the signal and outside
/// changes are synchronized into the mounted text block. The editor scrolls
/// vertically to keep the caret visible and paints its placeholder while the
/// value is empty.
///
/// # Defaults
///
/// Auto width, 160-pixel height, 10 pixels of inner padding, a lightly
/// bordered translucent fill, and the placeholder
/// `"Enter multiline text or code..."`.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let code = create_signal(String::from("fn main() {}"));
/// let editor = TextArea::new(code)
///     .placeholder("Write code here")
///     .height(Length::Px(240.0));
/// let element: Element = editor.into_element();
/// ```
pub struct TextArea {
    signal: Signal<String>,
    placeholder: String,
    material: Material,
    width: Length,
    height: Length,
}

impl TextArea {
    /// Creates a multiline editor that reads and writes `signal`.
    pub fn new(signal: Signal<String>) -> Self {
        Self {
            signal,
            placeholder: "Enter multiline text or code...".into(),
            material: Material::Classic {
                fill: Color::from_rgba8(255, 255, 255, 220),
                fill_end: None,
                gradient_dir: [0.0, 1.0],
                border_color: Color::from_rgba8(0, 0, 0, 30),
                border_width: 1.0,
                shadow: None,
            },
            width: Length::Auto,
            height: Length::Px(160.0),
        }
    }

    /// Sets the gray text shown when the editor's value is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Replaces the editor's background [`Material`].
    pub fn material(mut self, mat: Material) -> Self {
        self.material = mat;
        self
    }

    /// Sets the editor's preferred width.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }

    /// Sets the editor's preferred height.
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
}

impl IntoElement for TextArea {
    fn into_element(self) -> Element {
        Element::TextArea(self)
    }
}

