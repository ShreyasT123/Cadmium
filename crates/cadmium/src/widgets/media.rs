/// How an [`Image`] maps its source pixels onto its laid-out bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFit {
    /// Crop the source with [`cover_uv_rect`] so the bounds are filled while
    /// preserving the image's aspect ratio. This is the default.
    Cover,
    /// Use the complete image without cropping.
    Contain,
    /// Use the complete image and stretch it to the widget's bounds.
    Stretch,
}

/// The source pixels for an [`Image`].
///
/// Sources are decoded when the element is mounted: file paths are loaded with
/// [`load_image_file`], SVG strings are rasterized with [`rasterize_svg`], and
/// pre-decoded images are shared by pointer identity so multiple widgets can
/// reuse one GPU texture.
#[derive(Clone)]
pub enum ImageSource {
    /// An image loaded from a filesystem path when mounted.
    File(PathBuf),
    /// SVG markup rasterized at the given pixel dimensions when mounted.
    Svg {
        /// The SVG document text.
        data: String,
        /// Rasterization width in pixels.
        width: u32,
        /// Rasterization height in pixels.
        height: u32,
    },
    /// An already decoded image, shared via [`Arc`].
    Decoded(Arc<DecodedImage>),
}

/// A raster image with layout, clipping, tint, shadow, and interaction options.
///
/// # Defaults
///
/// [`Image::new`] uses default layout sizing, [`ImageFit::Cover`], square
/// corners, an opaque white tint, no shadow, and no tooltip/menu/click handler.
/// If the source cannot be loaded or rasterized, the node is still mounted but
/// has no pixels to paint.
///
/// # Examples
///
/// ```
/// use cadmium::prelude::*;
///
/// let avatar = Image::from_file("avatar.png")
///     .width(Length::Px(64.0))
///     .height(Length::Px(64.0))
///     .radius(32.0)
///     .fit(ImageFit::Cover);
/// let element: Element = avatar.into_element();
/// ```
pub struct Image {
    source: ImageSource,
    layout: LayoutStyle,
    radius: f32,
    fit: ImageFit,
    tint: Color,
    shadow: Option<Shadow>,
    tooltip: Option<String>,
    context_menu: Vec<MenuItem>,
    on_click: Option<Box<dyn FnMut()>>,
}

impl Image {
    /// Creates an image widget from an explicit [`ImageSource`].
    pub fn new(source: ImageSource) -> Self {
        Self {
            source,
            layout: LayoutStyle::default(),
            radius: 0.0,
            fit: ImageFit::Cover,
            tint: Color::from_rgba8(255, 255, 255, 255),
            shadow: None,
            tooltip: None,
            context_menu: Vec::new(),
            on_click: None,
        }
    }

    /// Creates an image from pixels that are already decoded.
    pub fn from_decoded(img: Arc<DecodedImage>) -> Self {
        Self::new(ImageSource::Decoded(img))
    }
    /// Sets how much of the parent's free space the image absorbs.
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.layout.flex_grow = g;
        self
    }
    /// Creates an image that loads `path` when mounted.
    pub fn from_file(path: impl Into<PathBuf>) -> Self {
        Self::new(ImageSource::File(path.into()))
    }

    /// Creates an image by rasterizing SVG `data` at `width` by `height` pixels.
    pub fn svg(data: impl Into<String>, width: u32, height: u32) -> Self {
        Self::new(ImageSource::Svg {
            data: data.into(),
            width,
            height,
        })
    }

    /// Sets the image's preferred width.
    pub fn width(mut self, w: Length) -> Self {
        self.layout.width = w;
        self
    }

    /// Sets the image's preferred height.
    pub fn height(mut self, h: Length) -> Self {
        self.layout.height = h;
        self
    }

    /// Sets the corner radius used to clip the painted image.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    /// Sets how the source is fitted into the widget bounds.
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    /// Sets a multiplicative tint applied to the image pixels.
    ///
    /// The default is opaque white, which leaves the source colors unchanged.
    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }

    /// Paints a drop shadow behind the image.
    pub fn shadow(mut self, color: Color, blur: f32, offset_y: f32) -> Self {
        self.shadow = Some(Shadow::new(color, blur, offset_y));
        self
    }

    /// Sets the tooltip shown when the pointer rests over the image.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Appends entries to the image's right-click context menu.
    pub fn context_menu<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.context_menu.extend(items);
        self
    }

    /// Registers a callback invoked when the image is pressed (pointer-down).
    pub fn on_click(mut self, f: impl FnMut() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl IntoElement for Image {
    fn into_element(self) -> Element {
        Element::Image(self)
    }
}

/// A square SVG icon rasterized on mount and tinted like text.
///
/// The icon's layout width and height are both set to `size`; the SVG is
/// rasterized at `round(size * 2.0)` pixels (at least one pixel) for crisp
/// rendering. Colors are selected with [`Icon::color`] or a theme-aware
/// [`TextTone`].
pub struct Icon {
    svg: String,
    size: f32,
    tone: TextTone,
    color: Color,
    tooltip: Option<String>,
}

impl Icon {
    /// Creates an icon from SVG markup with a square size in logical pixels.
    ///
    /// The initial tone is [`TextTone::Primary`] and no tooltip is shown.
    pub fn svg(data: impl Into<String>, size: f32) -> Self {
        Self {
            svg: data.into(),
            size,
            color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            tone: TextTone::Primary,
            tooltip: None,
        }
    }
    /// Sets an explicit tint color.
    ///
    /// Calling this also switches the icon to [`TextTone::Custom`] so the
    /// provided color is used instead of a themed color.
    pub fn color(mut self, color: Color) -> Self {
        self.tone = TextTone::Custom;
        self.color = color; // or `self.tint = color;` if the field is named `tint`
        self
    }
    /// Selects a theme-aware tint for the icon.
    pub fn tone(mut self, tone: TextTone) -> Self {
        self.tone = tone;
        self
    }

    /// Sets the tooltip shown when the pointer rests over the icon.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Icon {
    fn into_element(self) -> Element {
        Element::Icon(self)
    }
}

