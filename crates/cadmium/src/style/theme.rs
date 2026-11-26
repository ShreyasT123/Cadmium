/// Selects the light or dark built-in appearance.
///
/// Passed to [`MacTheme::from_mode`] to obtain the matching palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    /// Light appearance: bright backgrounds and dark foreground text.
    Light,
    /// Dark appearance: dark backgrounds and light foreground text.
    Dark,
}

/// A macOS-inspired color palette used by the built-in widgets.
///
/// The palette covers window and surface backgrounds, glass tints, hairlines and
/// shadows, three levels of foreground text, accent colors, and the traffic-light
/// buttons. Colors are linear-RGBA [`crate::core::Color`] values; the presets are built
/// with [`crate::core::Color::from_rgba8`] from sRGB bytes. Glass tints, hover colors,
/// and shadow colors are intentionally translucent, so their alpha channel is as
/// important as their RGB components.
///
/// Widget builders read these fields when no explicit color is supplied; applications
/// can start from [`MacTheme::light`] or [`MacTheme::dark`] and override individual
/// fields to customize the look.
///
/// # Examples
///
/// ```
/// use cadmium::style::{MacTheme, ThemeMode};
///
/// let dark = MacTheme::from_mode(ThemeMode::Dark);
/// assert_eq!(dark.mode, ThemeMode::Dark);
///
/// let custom = MacTheme { accent: dark.switch_on, ..MacTheme::light() };
/// assert_eq!(custom.mode, ThemeMode::Light);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct MacTheme {
    /// Which preset this palette was derived from.
    pub mode: ThemeMode,

    // Backgrounds & Surfaces (no inversion, neutral grayscale)
    /// Background behind the whole window, visible behind content and panels.
    pub window_bg: Color,
    /// Background of the main content area.
    pub content_bg: Color,
    /// Tint applied to sidebar glass surfaces; alpha controls tint strength.
    pub sidebar_glass_tint: Color,
    /// Tint applied to toolbar glass surfaces; alpha controls tint strength.
    pub toolbar_glass_tint: Color,
    /// Tint applied to popover glass surfaces; alpha controls tint strength.
    pub popover_glass_tint: Color,
    /// Opaque background of panels and cards that do not sample the backdrop.
    pub panel_bg: Color,
    /// Unfilled track of sliders, switches, and progress bars.
    pub control_track: Color,
    /// Knob or thumb of sliders and switches.
    pub control_thumb: Color,
    /// Translucent overlay applied to controls while the pointer hovers over them.
    pub control_hover: Color,

    // Hairlines & Shadows
    /// Low-contrast color for separator lines and control outlines.
    pub hairline: Color,
    /// Color of the window's outer border.
    pub window_border: Color,
    /// Shadow color for low-elevation surfaces such as controls and cards.
    pub shadow_soft: Color,
    /// Shadow color for elevated surfaces such as menus and popovers.
    pub shadow_elevated: Color,

    // Typography
    /// Color for primary labels and values.
    pub text_primary: Color,
    /// Color for secondary labels and supporting text.
    pub text_secondary: Color,
    /// Color for tertiary text such as placeholders and disabled labels.
    pub text_tertiary: Color,
    /// Foreground color for text drawn on top of an accent-colored surface.
    pub text_on_accent: Color,

    // Accent & Semantic
    /// Primary accent color for selection, focus, and tinted buttons.
    pub accent: Color,
    /// Translucent accent color for subtle highlights and selected backgrounds.
    pub accent_soft: Color,
    /// Color of an enabled (on) switch or toggle.
    pub switch_on: Color,
    /// Color of a disabled (off) switch or toggle.
    pub switch_off: Color,

    // Traffic Lights
    /// Color of the red close button.
    pub traffic_close: Color,
    /// Color of the yellow minimize button.
    pub traffic_minimize: Color,
    /// Color of the green zoom/maximize button.
    pub traffic_zoom: Color,
    /// Border drawn around the traffic-light buttons.
    pub traffic_border: Color,
    /// Color of the glyphs drawn inside traffic-light buttons.
    pub traffic_glyph: Color,
}

impl MacTheme {
    /// Returns the built-in palette for `mode`.
    ///
    /// Equivalent to [`MacTheme::light`] for [`ThemeMode::Light`] and
    /// [`MacTheme::dark`] for [`ThemeMode::Dark`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::style::{MacTheme, ThemeMode};
    ///
    /// let theme = MacTheme::from_mode(ThemeMode::Light);
    /// assert_eq!(theme.mode, ThemeMode::Light);
    /// ```
    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
        }
    }

    /// Returns the light macOS-inspired palette.
    ///
    /// Uses bright neutral backgrounds (for example `window_bg` = `245, 245, 247`),
    /// dark primary text (`29, 29, 31`), and the blue accent `0, 122, 255`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::style::{MacTheme, ThemeMode};
    ///
    /// let theme = MacTheme::light();
    /// assert_eq!(theme.mode, ThemeMode::Light);
    /// ```
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            window_bg: Color::from_rgba8(245, 245, 247, 255),
            content_bg: Color::from_rgba8(255, 255, 255, 255),
            sidebar_glass_tint: Color::from_rgba8(245, 245, 247, 150),
            toolbar_glass_tint: Color::from_rgba8(255, 255, 255, 195),
            popover_glass_tint: Color::from_rgba8(252, 252, 255, 130),
            panel_bg: Color::from_rgba8(255, 255, 255, 255),
            control_track: Color::from_rgba8(229, 229, 234, 255),
            control_thumb: Color::from_rgba8(255, 255, 255, 255),
            control_hover: Color::from_rgba8(0, 0, 0, 14),

            hairline: Color::from_rgba8(0, 0, 0, 22),
            window_border: Color::from_rgba8(0, 0, 0, 32),
            shadow_soft: Color::from_rgba8(0, 0, 0, 24),
            shadow_elevated: Color::from_rgba8(0, 0, 0, 52),

            text_primary: Color::from_rgba8(29, 29, 31, 255),
            text_secondary: Color::from_rgba8(110, 110, 115, 255),
            text_tertiary: Color::from_rgba8(134, 134, 139, 255),
            text_on_accent: Color::from_rgba8(255, 255, 255, 255),

            accent: Color::from_rgba8(0, 122, 255, 255),
            accent_soft: Color::from_rgba8(0, 122, 255, 28),
            switch_on: Color::from_rgba8(52, 199, 89, 255),
            switch_off: Color::from_rgba8(216, 216, 222, 255),

            traffic_close: Color::from_rgba8(255, 95, 87, 255),
            traffic_minimize: Color::from_rgba8(254, 188, 46, 255),
            traffic_zoom: Color::from_rgba8(40, 200, 64, 255),
            traffic_border: Color::from_rgba8(0, 0, 0, 35),
            traffic_glyph: Color::from_rgba8(77, 10, 8, 210),
        }
    }

    /// Returns the dark macOS-inspired palette.
    ///
    /// Uses dark neutral backgrounds (for example `window_bg` = `28, 28, 30`),
    /// light primary text (`245, 245, 247`), and the blue accent `10, 132, 255`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::style::{MacTheme, ThemeMode};
    ///
    /// let theme = MacTheme::dark();
    /// assert_eq!(theme.mode, ThemeMode::Dark);
    /// ```
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            window_bg: Color::from_rgba8(28, 28, 30, 255),
            content_bg: Color::from_rgba8(30, 30, 32, 255),
            sidebar_glass_tint: Color::from_rgba8(36, 36, 38, 160),
            toolbar_glass_tint: Color::from_rgba8(40, 40, 43, 195),
            popover_glass_tint: Color::from_rgba8(44, 44, 48, 135),
            panel_bg: Color::from_rgba8(44, 44, 46, 255),
            control_track: Color::from_rgba8(58, 58, 60, 255),
            control_thumb: Color::from_rgba8(255, 255, 255, 255),
            control_hover: Color::from_rgba8(255, 255, 255, 18),

            hairline: Color::from_rgba8(255, 255, 255, 26),
            window_border: Color::from_rgba8(255, 255, 255, 34),
            shadow_soft: Color::from_rgba8(0, 0, 0, 95),
            shadow_elevated: Color::from_rgba8(0, 0, 0, 150),

            text_primary: Color::from_rgba8(245, 245, 247, 255),
            text_secondary: Color::from_rgba8(152, 152, 157, 255),
            text_tertiary: Color::from_rgba8(118, 118, 124, 255),
            text_on_accent: Color::from_rgba8(255, 255, 255, 255),

            accent: Color::from_rgba8(10, 132, 255, 255),
            accent_soft: Color::from_rgba8(10, 132, 255, 42),
            switch_on: Color::from_rgba8(48, 209, 88, 255),
            switch_off: Color::from_rgba8(72, 72, 74, 255),

            traffic_close: Color::from_rgba8(255, 95, 87, 255),
            traffic_minimize: Color::from_rgba8(254, 188, 46, 255),
            traffic_zoom: Color::from_rgba8(40, 200, 64, 255),
            traffic_border: Color::from_rgba8(255, 255, 255, 26),
            traffic_glyph: Color::from_rgba8(40, 8, 6, 220),
        }
    }
}
