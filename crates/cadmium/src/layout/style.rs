/// The layout constraints attached to one node of the widget tree.
///
/// [`LayoutStyle`] is converted to a Taffy `Style` with `LayoutStyle::into()`: `hidden`
/// maps to `Display::None`, `position` and `direction` map to their Taffy counterparts,
/// and the size fields become Taffy `Size`/`Dimension` values. Padding and margin are
/// always fixed logical-pixel lengths, and [`Self::gap`] is applied to both axes.
///
/// [`LayoutStyle::default`] describes a visible, relatively positioned, column-oriented,
/// auto-sized node that shrinks freely but does not grow.
///
/// # Examples
///
/// ```
/// use cadmium::layout::{
///     AlignItems, Edges, FlexDir, LayoutStyle, Length, PositionMode,
/// };
///
/// let style = LayoutStyle {
///     direction: FlexDir::Row,
///     width: Length::Percent(100.0),
///     padding: Edges::xy(12.0, 8.0),
///     gap: 6.0,
///     align_items: Some(AlignItems::Center),
///     ..Default::default()
/// };
/// assert_eq!(style.position, PositionMode::Relative);
/// assert_eq!(style.flex_grow, 0.0);
/// assert_eq!(style.flex_shrink, 1.0);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct LayoutStyle {
    /// Whether to skip this node entirely. When `true`, the node is converted to
    /// `Display::None` and receives no layout rectangle. Defaults to `false`.
    pub hidden: bool,
    /// Positioning mode for this node. Defaults to [`PositionMode::Relative`].
    pub position: PositionMode,
    /// Offsets used by [`PositionMode::Absolute`] (or to nudge a relative node).
    /// Defaults to [`Inset::AUTO`].
    pub inset: Inset,
    /// Main-axis direction for children. Defaults to [`FlexDir::Column`].
    pub direction: FlexDir,
    /// Preferred width. Defaults to [`Length::Auto`], meaning content- or flex-derived.
    pub width: Length,
    /// Preferred height. Defaults to [`Length::Auto`], meaning content- or flex-derived.
    pub height: Length,
    /// Lower bound on width. Defaults to [`Length::Auto`], which imposes no minimum.
    pub min_width: Length,
    /// Lower bound on height. Defaults to [`Length::Auto`], which imposes no minimum.
    pub min_height: Length,
    /// Upper bound on width. Defaults to [`Length::Auto`], which imposes no maximum.
    pub max_width: Length,
    /// Upper bound on height. Defaults to [`Length::Auto`], which imposes no maximum.
    pub max_height: Length,
    /// Share of positive free space this node takes when its siblings grow.
    /// Defaults to `0.0` (never grow); use `1.0` to fill the remaining space.
    pub flex_grow: f32,
    /// Share of negative free space this node gives up when its siblings shrink.
    /// Defaults to `1.0`; set to `0.0` to keep the node's size when space is tight.
    pub flex_shrink: f32,
    /// Inner spacing along all four edges, in logical pixels. Defaults to [`Edges::ZERO`].
    pub padding: Edges,
    /// Outer spacing along all four edges, in logical pixels. Defaults to [`Edges::ZERO`].
    pub margin: Edges,
    /// Space between adjacent children, in logical pixels, applied to both flex axes.
    /// Defaults to `0.0`.
    pub gap: f32,
    /// Cross-axis alignment of children. `None` keeps Taffy's default alignment;
    /// `Some(..)` overrides it. Defaults to `None`.
    pub align_items: Option<AlignItems>,
    /// Main-axis distribution of children. `None` keeps Taffy's default distribution;
    /// `Some(..)` overrides it. Defaults to `None`.
    pub justify_content: Option<JustifyContent>,
    /// Whether flex children may wrap onto multiple lines when the available
    /// space is exceeded. Defaults to `false`.
    pub wrap: bool,
    /// Whether child painting and hit testing are clipped to this node's bounds.
    /// Defaults to `false`.
    pub clip: bool,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            hidden: false,
            position: PositionMode::Relative,
            inset: Inset::AUTO,
            direction: FlexDir::Column,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: Edges::ZERO,
            margin: Edges::ZERO,
            gap: 0.0,
            align_items: None,
            justify_content: None,
            wrap: false,
            clip: false,
        }
    }
}

/// Converts the Cadmium layout description into Taffy's `Style`.
impl From<LayoutStyle> for Style {
    fn from(s: LayoutStyle) -> Self {
        let base = Style::default();
        Style {
            display: if s.hidden {
                Display::None
            } else {
                Display::Flex
            },
            position: match s.position {
                PositionMode::Relative => Position::Relative,
                PositionMode::Absolute => Position::Absolute,
            },
            inset: taffy::Rect {
                left: s.inset.left.into(),
                right: s.inset.right.into(),
                top: s.inset.top.into(),
                bottom: s.inset.bottom.into(),
            },
            flex_direction: match s.direction {
                FlexDir::Row => FlexDirection::Row,
                FlexDir::Column => FlexDirection::Column,
            },
            size: Size {
                width: s.width.into(),
                height: s.height.into(),
            },
            min_size: Size {
                width: s.min_width.into(),
                height: s.min_height.into(),
            },
            max_size: Size {
                width: s.max_width.into(),
                height: s.max_height.into(),
            },
            flex_grow: s.flex_grow,
            flex_shrink: s.flex_shrink,
            padding: taffy::Rect {
                left: LengthPercentage::length(s.padding.left),
                right: LengthPercentage::length(s.padding.right),
                top: LengthPercentage::length(s.padding.top),
                bottom: LengthPercentage::length(s.padding.bottom),
            },
            margin: taffy::Rect {
                left: LengthPercentageAuto::length(s.margin.left),
                right: LengthPercentageAuto::length(s.margin.right),
                top: LengthPercentageAuto::length(s.margin.top),
                bottom: LengthPercentageAuto::length(s.margin.bottom),
            },
            gap: Size {
                width: LengthPercentage::length(s.gap),
                height: LengthPercentage::length(s.gap),
            },
            flex_wrap: if s.wrap {
                FlexWrap::Wrap
            } else {
                FlexWrap::NoWrap
            },
            overflow: Point {
                x: if s.clip {
                    Overflow::Clip
                } else {
                    Overflow::Visible
                },
                y: if s.clip {
                    Overflow::Clip
                } else {
                    Overflow::Visible
                },
            },
            align_items: s.align_items.map(Into::into).unwrap_or(base.align_items),
            justify_content: s
                .justify_content
                .map(Into::into)
                .unwrap_or(base.justify_content),
            ..base
        }
    }
}

