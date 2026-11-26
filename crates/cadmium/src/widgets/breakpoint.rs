/// Responsive viewport size classification based on logical window width.
///
/// Use [`Breakpoint::from_width`] with a reactive viewport signal to build
/// layouts that adapt when a window is tiled or resized.
///
/// # Examples
///
/// ```
/// use cadmium::Breakpoint;
///
/// assert_eq!(Breakpoint::from_width(640.0), Breakpoint::Compact);
/// assert_eq!(Breakpoint::from_width(900.0), Breakpoint::Medium);
/// assert_eq!(Breakpoint::from_width(1280.0), Breakpoint::Expanded);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    /// Narrow or half-screen tiled windows (`< 760` logical px).
    Compact,
    /// Medium split or tablet-sized windows (`760..1120` logical px).
    Medium,
    /// Full desktop workstation windows (`>= 1120` logical px).
    Expanded,
}

impl Breakpoint {
    /// Classifies a logical window width into a [`Breakpoint`].
    pub fn from_width(width: f32) -> Self {
        if width < 760.0 {
            Self::Compact
        } else if width < 1120.0 {
            Self::Medium
        } else {
            Self::Expanded
        }
    }
}
