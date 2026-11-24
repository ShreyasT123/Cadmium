use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
    SwashImage, Weight,
};

use crate::core::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Regular,
    Medium,
    Semibold,
    Bold,
}

impl From<FontWeight> for Weight {
    fn from(w: FontWeight) -> Self {
        match w {
            FontWeight::Regular => Weight::NORMAL,
            FontWeight::Medium => Weight::MEDIUM,
            FontWeight::Semibold => Weight::SEMIBOLD,
            FontWeight::Bold => Weight::BOLD,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub color: Color,
    pub weight: FontWeight,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            line_height: 18.0,
            color: Color::from_rgba8(29, 29, 31, 255),
            weight: FontWeight::Regular,
        }
    }
}

pub struct TextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl TextEngine {
    pub fn new() -> Self {
        let mut db = cosmic_text::fontdb::Database::new();

        #[cfg(target_os = "windows")]
        {
            for path in [
                r"C:\Windows\Fonts\segoeui.ttf",
                r"C:\Windows\Fonts\segoeuib.ttf",
                r"C:\Windows\Fonts\segoeuisb.ttf",
            ] {
                let _ = db.load_font_file(path);
            }
            if db.is_empty() {
                db.load_system_fonts();
            } else {
                db.set_sans_serif_family("Segoe UI");
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            db.load_system_fonts();
        }

        Self {
            font_system: FontSystem::new_with_locale_and_db("en-US".to_string(), db),
            swash_cache: SwashCache::new(),
        }
    }

    pub fn load_system_fonts(&mut self) {
        self.font_system.db_mut().load_system_fonts();
    }

    pub(crate) fn rasterize_uncached(&mut self, key: CacheKey) -> Option<SwashImage> {
        self.swash_cache
            .get_image_uncached(&mut self.font_system, key)
    }
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct ShapedGlyph {
    pub cache_key: CacheKey,
    pub x: i32,
    pub y: i32,
}

pub struct TextBlock {
    buffer: Buffer,
    text: String,
    style: TextStyle,
    last_max_width: Option<f32>,
}

impl TextBlock {
    pub fn new(engine: &mut TextEngine, text: impl Into<String>, style: TextStyle) -> Self {
        let text = text.into();
        let mut buffer = Buffer::new(
            &mut engine.font_system,
            Metrics::new(style.font_size, style.line_height),
        );
        let attrs = Attrs::new()
            .family(Family::SansSerif)
            .weight(style.weight.into());
        buffer.set_text(&text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut engine.font_system, false);

        Self {
            buffer,
            text,
            style,
            last_max_width: None,
        }
    }

    pub fn style(&self) -> TextStyle {
        self.style
    }

    pub fn set_color(&mut self, color: Color) {
        self.style.color = color;
    }

    pub fn set_text(&mut self, engine: &mut TextEngine, text: &str) {
        if self.text == text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);

        let attrs = Attrs::new()
            .family(Family::SansSerif)
            .weight(self.style.weight.into());
        self.buffer.set_text(text, &attrs, Shaping::Advanced, None);
        self.buffer
            .shape_until_scroll(&mut engine.font_system, false);
    }

    pub fn measure(&mut self, engine: &mut TextEngine, max_width: Option<f32>) -> (f32, f32) {
        if self.last_max_width != max_width {
            self.last_max_width = max_width;
            self.buffer.set_size(max_width, None);
            self.buffer
                .shape_until_scroll(&mut engine.font_system, false);
        }

        let mut width = 0.0f32;
        let mut lines = 0usize;
        for run in self.buffer.layout_runs() {
            width = width.max(run.line_w);
            lines += 1;
        }
        let height = (lines.max(1) as f32) * self.style.line_height;
        (width.ceil(), height.ceil())
    }

    pub(crate) fn collect_glyphs_into(
        &self,
        origin_x_px: f32,
        origin_y_px: f32,
        scale: f32,
        out: &mut Vec<ShapedGlyph>,
    ) {
        out.clear();
        for run in self.buffer.layout_runs() {
            let line_y_px = (run.line_y * scale).round() as i32;
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((origin_x_px, origin_y_px), scale);
                out.push(ShapedGlyph {
                    cache_key: physical.cache_key,
                    x: physical.x,
                    y: physical.y + line_y_px,
                });
            }
        }
    }

    pub(crate) fn collect_char_offsets(&self, out: &mut Vec<(usize, f32)>) {
        out.clear();
        out.push((0, 0.0));
        if let Some(run) = self.buffer.layout_runs().next() {
            for g in run.glyphs.iter() {
                out.push((g.end, g.x + g.w));
            }
        }
    }

    pub(crate) fn line_y(&self, line: usize) -> f32 {
        self.buffer
            .layout_runs()
            .nth(line)
            .map(|run| run.line_y)
            .unwrap_or(line as f32 * self.style.line_height)
    }

    pub(crate) fn x_offset_for_byte(&self, line: usize, byte_idx: usize) -> f32 {
        let Some(run) = self.buffer.layout_runs().nth(line) else {
            return 0.0;
        };
        let mut x = 0.0;
        for glyph in run.glyphs.iter() {
            if glyph.end > byte_idx {
                return glyph.x;
            }
            x = glyph.x + glyph.w;
        }
        x
    }

    pub(crate) fn byte_index_at_x(&self, line: usize, x: f32) -> usize {
        let Some(run) = self.buffer.layout_runs().nth(line) else {
            return 0;
        };
        let mut best = 0;
        let mut best_dist = x.abs();
        let mut end = 0;
        for glyph in run.glyphs.iter() {
            let dist = (glyph.x - x).abs();
            if dist < best_dist {
                best_dist = dist;
                best = end;
            }
            end = glyph.end;
            let dist = (glyph.x + glyph.w - x).abs();
            if dist < best_dist {
                best_dist = dist;
                best = end;
            }
        }
        best
    }
}

pub(crate) fn swash_is_color(content: SwashContent) -> bool {
    matches!(content, SwashContent::Color)
}
