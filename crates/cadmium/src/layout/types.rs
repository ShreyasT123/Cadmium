/// Opaque handle to a node in a [`LayoutEngine`]'s Taffy tree.
///
/// Returned by [`LayoutEngine::new_leaf`] and [`LayoutEngine::new_container`], and used
/// to identify a node in later calls such as [`LayoutEngine::set_style`],
/// [`LayoutEngine::mark_dirty`], and [`LayoutEngine::collect_logical_rects`].
///
/// A handle is only meaningful for the engine that created it; do not mix nodes from
/// different engines in one [`LayoutEngine::new_container`] or
/// [`LayoutEngine::set_children`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutNode(NodeId);

/// Direction of the main axis in a flex container.
///
/// The main axis is the axis distributed by [`JustifyContent`]; the other axis is the
/// cross axis controlled by [`AlignItems`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDir {
    /// Lay children out left to right; the main axis is horizontal.
    Row,
    /// Lay children out top to bottom; the main axis is vertical.
    Column,
}

/// Cross-axis alignment of children within a flex container.
///
/// Converted to `taffy::style::AlignItems` when a [`LayoutStyle`] becomes a Taffy style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    /// Align children to the start of the cross axis (top for [`FlexDir::Row`]).
    Start,
    /// Align children to the end of the cross axis (bottom for [`FlexDir::Row`]).
    End,
    /// Center children along the cross axis.
    Center,
    /// Stretch children along the cross axis when their cross-axis size is automatic.
    Stretch,
}

/// Converts Cadmium's alignment into its Taffy equivalent.
impl From<AlignItems> for taffy::style::AlignItems {
    fn from(a: AlignItems) -> Self {
        match a {
            AlignItems::Start => taffy::style::AlignItems::START,
            AlignItems::End => taffy::style::AlignItems::END,
            AlignItems::Center => taffy::style::AlignItems::CENTER,
            AlignItems::Stretch => taffy::style::AlignItems::STRETCH,
        }
    }
}

/// Distribution of free space along a flex container's main axis.
///
/// Converted to `taffy::style::JustifyContent` when a [`LayoutStyle`] becomes a Taffy style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    /// Pack children at the start of the main axis.
    Start,
    /// Pack children at the end of the main axis.
    End,
    /// Center the group of children along the main axis.
    Center,
    /// Push the first and last children to the edges and space the rest evenly between
    /// adjacent children.
    SpaceBetween,
}

/// Converts Cadmium's justification into its Taffy equivalent.
impl From<JustifyContent> for taffy::style::JustifyContent {
    fn from(j: JustifyContent) -> Self {
        match j {
            JustifyContent::Start => taffy::style::JustifyContent::START,
            JustifyContent::End => taffy::style::JustifyContent::END,
            JustifyContent::Center => taffy::style::JustifyContent::CENTER,
            JustifyContent::SpaceBetween => taffy::style::JustifyContent::SPACE_BETWEEN,
        }
    }
}

/// A size or offset along one axis, expressed as automatic, logical pixels, or a percentage.
///
/// Used for [`LayoutStyle::width`], [`LayoutStyle::height`], the min/max constraints, and
/// every side of an [`Inset`]. When converting to Taffy, [`Length::Auto`] becomes
/// `Dimension::auto()` (or `LengthPercentageAuto::auto()` for insets), [`Length::Px`]
/// becomes a fixed length, and [`Length::Percent`] becomes a percentage of the parent's
/// corresponding inner size.
///
/// # Examples
///
/// ```
/// use cadmium::layout::{Inset, Length};
///
/// let fixed = Length::Px(24.0);          // 24 logical pixels
/// let half_width = Length::Percent(50.0); // 50% of the parent's width
/// let inset = Inset {
///     left: fixed,
///     right: half_width,
///     ..Inset::AUTO
/// };
/// assert_eq!(inset.top, Length::Auto);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// Let Taffy derive the size from content (the [`LayoutEngine::compute_with_measure`]
    /// callback for leaf nodes) or from flex rules. For insets, leave the side unset.
    Auto,
    /// A fixed size in logical pixels. Device-pixel scaling is applied when the computed
    /// rectangle is drawn, not here.
    Px(f32),
    /// A percentage of the parent's corresponding inner size. For example, `50.0` means
    /// half of the parent's width or height. Values are forwarded to Taffy without clamping.
    Percent(f32),
}

/// Converts a [`Length`] to Taffy's `Dimension`, used for sizes and min/max constraints.
impl From<Length> for Dimension {
    fn from(len: Length) -> Self {
        match len {
            Length::Auto => Dimension::auto(),
            Length::Px(v) => Dimension::length(v),
            Length::Percent(p) => Dimension::percent(p),
        }
    }
}

/// Converts a [`Length`] to Taffy's `LengthPercentageAuto`, used for inset sides.
impl From<Length> for LengthPercentageAuto {
    fn from(len: Length) -> Self {
        match len {
            Length::Auto => LengthPercentageAuto::auto(),
            Length::Px(v) => LengthPercentageAuto::length(v),
            Length::Percent(p) => LengthPercentageAuto::percent(p),
        }
    }
}

/// Four logical-pixel edge values, in the order used for padding and margins.
///
/// Every field is converted to a fixed Taffy length; percentages are not supported here.
/// Use [`Edges::ZERO`] for no spacing, [`Edges::all`] for uniform spacing, or
/// [`Edges::xy`] for separate horizontal and vertical spacing.
///
/// # Examples
///
/// ```
/// use cadmium::layout::Edges;
///
/// let uniform = Edges::all(8.0);
/// let horizontal = Edges::xy(12.0, 4.0);
/// assert_eq!(uniform, Edges { left: 8.0, right: 8.0, top: 8.0, bottom: 8.0 });
/// assert_eq!(horizontal, Edges { left: 12.0, right: 12.0, top: 4.0, bottom: 4.0 });
/// assert_eq!(Edges::ZERO, Edges::all(0.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edges {
    /// Space on the leading (left) edge, in logical pixels.
    pub left: f32,
    /// Space on the trailing (right) edge, in logical pixels.
    pub right: f32,
    /// Space on the top edge, in logical pixels.
    pub top: f32,
    /// Space on the bottom edge, in logical pixels.
    pub bottom: f32,
}

impl Edges {
    /// No spacing on any edge; the default for [`LayoutStyle::padding`] and
    /// [`LayoutStyle::margin`].
    pub const ZERO: Self = Self::all(0.0);

    /// Creates edges with the same logical-pixel value on all four sides.
    pub const fn all(v: f32) -> Self {
        Self {
            left: v,
            right: v,
            top: v,
            bottom: v,
        }
    }

    /// Creates edges with `x` on the left and right and `y` on the top and bottom.
    pub const fn xy(x: f32, y: f32) -> Self {
        Self {
            left: x,
            right: x,
            top: y,
            bottom: y,
        }
    }
}

/// Whether a node participates in normal flex layout or is positioned out of flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionMode {
    /// Participate in the parent's flex flow. Any non-automatic [`Inset`] offsets the
    /// node from its normal in-flow position. This is the default in [`LayoutStyle`].
    Relative,
    /// Take the node out of flow and place it using [`LayoutStyle::inset`] relative to
    /// the containing node. Siblings do not reserve space for an absolutely positioned
    /// node.
    Absolute,
}

/// Per-side offsets for an absolutely or relatively positioned node.
///
/// Each side is a [`Length`], so it can be automatic, a fixed number of logical pixels,
/// or a percentage of the parent's corresponding axis. [`Inset::AUTO`] (also the
/// [`Default`] value) leaves all sides automatic, which is appropriate for in-flow nodes.
///
/// # Examples
///
/// ```
/// use cadmium::layout::{Inset, Length};
///
/// let pinned = Inset {
///     top: Length::Px(8.0),
///     right: Length::Px(8.0),
///     ..Inset::AUTO
/// };
/// assert_eq!(pinned.bottom, Length::Auto);
/// assert_eq!(Inset::all(Length::Percent(10.0)).left, Length::Percent(10.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inset {
    /// Distance from the parent's left edge, or automatic.
    pub left: Length,
    /// Distance from the parent's right edge, or automatic.
    pub right: Length,
    /// Distance from the parent's top edge, or automatic.
    pub top: Length,
    /// Distance from the parent's bottom edge, or automatic.
    pub bottom: Length,
}

impl Inset {
    /// All four sides automatic, leaving the node at its normal (in-flow) position.
    pub const AUTO: Self = Self {
        left: Length::Auto,
        right: Length::Auto,
        top: Length::Auto,
        bottom: Length::Auto,
    };

    /// Creates an inset with the same [`Length`] on all four sides.
    pub const fn all(v: Length) -> Self {
        Self {
            left: v,
            right: v,
            top: v,
            bottom: v,
        }
    }
}

impl Default for Inset {
    fn default() -> Self {
        Self::AUTO
    }
}

