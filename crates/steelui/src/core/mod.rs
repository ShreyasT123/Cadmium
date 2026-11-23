use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: srgb_to_linear(r as f32 / 255.0),
            g: srgb_to_linear(g as f32 / 255.0),
            b: srgb_to_linear(b as f32 / 255.0),
            a: a as f32 / 255.0,
        }
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn to_wgpu(self) -> wgpu::Color {
        wgpu::Color {
            r: self.r as f64,
            g: self.g as f64,
            b: self.b as f64,
            a: self.a as f64,
        }
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    pub fn scale(self, factor: f32) -> Self {
        Self {
            x: self.x * factor,
            y: self.y * factor,
            width: self.width * factor,
            height: self.height * factor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub fn scale(self, factor: f32) -> Self {
        Self {
            top_left: self.top_left * factor,
            top_right: self.top_right * factor,
            bottom_right: self.bottom_right * factor,
            bottom_left: self.bottom_left * factor,
        }
    }

    pub fn to_array(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }
}

#[derive(Debug, Error)]
pub enum SteelError {
    #[error("Failed to create GPU surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),

    #[error("No compatible GPU adapter found: {0}")]
    RequestAdapter(#[from] wgpu::RequestAdapterError),

    #[error("Failed to request GPU device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),

    #[error("OS window event loop error: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),

    #[error("Failed to create OS window: {0}")]
    CreateWindow(#[from] winit::error::OsError),

    #[error("Image decode error: {0}")]
    Image(#[from] image::ImageError),

    #[error("SVG error: {0}")]
    Svg(String),

    #[error("Platform integration error: {0}")]
    Platform(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_edges() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(110.0, 70.0));
        assert!(!r.contains(9.9, 20.0));
        assert!(!r.contains(50.0, 71.0));
    }

    #[test]
    fn srgb_conversion_preserves_black_and_white() {
        let black = Color::from_rgba8(0, 0, 0, 255);
        assert_eq!(black.r, 0.0);
        assert_eq!(black.a, 1.0);
        let white = Color::from_rgba8(255, 255, 255, 255);
        assert!((white.r - 1.0).abs() < 1e-5);
    }
}
