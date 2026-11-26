/// Circular avatar displaying either an image or fallback initials.
///
/// An avatar is a fixed-size builder that becomes one of two [`Element`]
/// shapes: an [`Image`] cropped to a circle with [`ImageFit::Cover`], or a
/// filled [`Container`] with centered [`Text`] initials. It has no reactive
/// binding; replace the builder in the tree to change the image, initials, or
/// size.
///
/// # Defaults
///
/// Both [`Avatar::initials`] and [`Avatar::image`] start at 36 logical pixels
/// square, giving an 18px corner radius so the result is circular. The initials
/// branch uses the light theme's accent fill and matching on-accent text, with
/// a semibold font size of `max(size * 0.38, 10.0)`. The image branch supplies
/// panel and primary-text colors for its stored styling, but those colors are
/// only painted by the initials branch. [`Avatar::size`] controls both
/// dimensions and the radius; [`Avatar::fill`] and [`Avatar::text_color`]
/// affect initials only.
///
/// # Examples
///
/// ```rust
/// use cadmium::prelude::*;
///
/// let monogram = Avatar::initials("AS")
///     .size(48.0)
///     .fill(Color::from_rgba8(88, 86, 214, 255));
/// let photo = Avatar::image(ImageSource::File("team/ada.png".into()))
///     .size(40.0);
///
/// let ui = Row::new().gap(8.0).child(monogram).child(photo);
/// let element: Element = ui.into_element();
/// ```
pub struct Avatar {
    size: f32,
    initials: Option<String>,
    image: Option<ImageSource>,
    fill: Color,
    text_color: Color,
}

impl Avatar {
    /// Creates a circular avatar showing `initials` on an accent fill.
    ///
    /// The initials are stored as an owned [`String`] and centered in a 36px
    /// circle by default.
    pub fn initials(initials: impl Into<String>) -> Self {
        let light = MacTheme::light();
        Self {
            size: 36.0,
            initials: Some(initials.into()),
            image: None,
            fill: light.accent,
            text_color: light.text_on_accent,
        }
    }

    /// Creates a circular avatar showing `source` cropped to fill the circle.
    ///
    /// The image is decoded when the resulting element is mounted; see
    /// [`ImageSource`] for the supported sources. The default diameter is 36px.
    pub fn image(source: ImageSource) -> Self {
        let light = MacTheme::light();
        Self {
            size: 36.0,
            initials: None,
            image: Some(source),
            fill: light.panel_bg,
            text_color: light.text_primary,
        }
    }

    /// Sets the avatar's diameter in logical pixels.
    ///
    /// Both width and height use this value, and the circle's corner radius is
    /// half of it. Initials scale with the size but never drop below 10px.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Overrides the initials avatar's background color.
    ///
    /// This has no visible effect on an avatar created with [`Avatar::image`],
    /// because the image branch paints the source instead of the fill color.
    pub fn fill(mut self, color: Color) -> Self {
        self.fill = color;
        self
    }

    /// Overrides the initials avatar's text color.
    ///
    /// This has no visible effect on an avatar created with [`Avatar::image`].
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }
}

/// Builds either a cover-cropped circular image or centered initials.
impl IntoElement for Avatar {
    /// Returns either the cover-cropped image or the initials circle.
    fn into_element(self) -> Element {
        let radius = self.size * 0.5;
        if let Some(src) = self.image {
            Image::new(src)
                .width(Length::Px(self.size))
                .height(Length::Px(self.size))
                .radius(radius)
                .fit(ImageFit::Cover)
                .into_element()
        } else {
            Container::row()
                .width(Length::Px(self.size))
                .height(Length::Px(self.size))
                .radius(radius)
                .fill(self.fill)
                .align(AlignItems::Center)
                .justify(JustifyContent::Center)
                .child(
                    Text::new(self.initials.unwrap_or_default())
                        .size((self.size * 0.38).max(10.0))
                        .weight(FontWeight::Semibold)
                        .color(self.text_color),
                )
                .into_element()
        }
    }
}
