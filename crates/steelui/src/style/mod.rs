#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub blur: f32,
    pub offset: [f32; 2],
}

impl Shadow {
    pub const fn new(color: Color, blur: f32, offset_y: f32) -> Self {
        Self {
            color,
            blur,
            offset: [0.0, offset_y],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    /// Classic UI: Solid or gradient surface, crisp 1px hairlines, analytical Gaussian drop shadows
    Classic {
        fill: Color,
        fill_end: Option<Color>,
        gradient_dir: [f32; 2],
        border_color: Color,
        border_width: f32,
        shadow: Option<Shadow>,
    },
    /// Frosted Glass: Dual-Kawase 9-tap Gaussian diffusion, soft edge sheen, ambient tint
    Frosted {
        tint: Color,
        frost: f32,
        specular: f32,
        border_color: Color,
        border_width: f32,
        shadow: Option<Shadow>,
    },
    /// Liquid Glass: Optical bezel lens refraction, chromatic dispersion (prism RGB split), dynamic sheen
    Liquid {
        tint: Color,
        frost: f32,
        ripple: f32,
        specular: f32,
        rim_angle: f32,
        sheen_progress: f32,
        conic_strength: f32,
        border_color: Color,
        border_width: f32,
        shadow: Option<Shadow>,
    },
}

impl Material {
    pub fn solid(color: Color) -> Self {
        Self::Classic {
            fill: color,
            fill_end: None,
            gradient_dir: [0.0, 1.0],
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            shadow: None,
        }
    }

    pub fn frosted(tint: Color) -> Self {
        Self::Frosted {
            tint,
            frost: 0.45,
            specular: 0.85,
            border_color: Color::from_rgba8(255, 255, 255, 45),
            border_width: 1.0,
            shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 48), 16.0, 6.0)),
        }
    }

    pub fn liquid(tint: Color) -> Self {
        Self::Liquid {
            tint,
            frost: 0.12,
            ripple: 0.25,
            specular: 0.95,
            rim_angle: -1.309,
            sheen_progress: 0.0,
            conic_strength: 0.0,
            border_color: Color::from_rgba8(255, 255, 255, 65),
            border_width: 1.0,
            shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 52), 18.0, 8.0)),
        }
    }
}

use crate::core::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy)]
pub struct MacTheme {
    pub mode: ThemeMode,

    // Backgrounds & Surfaces (no inversion, neutral grayscale)
    pub window_bg: Color,
    pub content_bg: Color,
    pub sidebar_glass_tint: Color,
    pub toolbar_glass_tint: Color,
    pub popover_glass_tint: Color,
    pub panel_bg: Color,
    pub control_track: Color,
    pub control_thumb: Color,
    pub control_hover: Color,

    // Hairlines & Shadows
    pub hairline: Color,
    pub window_border: Color,
    pub shadow_soft: Color,
    pub shadow_elevated: Color,

    // Typography
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_tertiary: Color,
    pub text_on_accent: Color,

    // Accent & Semantic
    pub accent: Color,
    pub accent_soft: Color,
    pub switch_on: Color,
    pub switch_off: Color,

    // Traffic Lights
    pub traffic_close: Color,
    pub traffic_minimize: Color,
    pub traffic_zoom: Color,
    pub traffic_border: Color,
    pub traffic_glyph: Color,
}

impl MacTheme {
    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
        }
    }

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
