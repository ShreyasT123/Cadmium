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

    pub(crate) fn caret_pos_for_byte(&self, global_byte: usize) -> (f32, f32) {
        let starts = self.line_start_offsets();
        let line_i = self.text.as_bytes()[..global_byte.min(self.text.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        let line_start = starts.get(line_i).copied().unwrap_or(0);
        let local_byte = global_byte.saturating_sub(line_start);

        let mut matching_runs = self
            .buffer
            .layout_runs()
            .filter(|r| r.line_i == line_i)
            .peekable();

        if matching_runs.peek().is_none() {
            return (line_i as f32 * self.style.line_height, 0.0);
        }

        let mut last_top = line_i as f32 * self.style.line_height;
        let mut last_x = 0.0;
        for run in matching_runs {
            last_top = run.line_top;
            let run_end = run.glyphs.last().map(|g| g.end).unwrap_or(0);
            if local_byte <= run_end || run.glyphs.is_empty() {
                return (run.line_top, x_in_run(run.glyphs, local_byte));
            }
            last_x = run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0);
        }
        (last_top, last_x)
    }

    pub(crate) fn line_start_offsets(&self) -> Vec<usize> {
        let mut starts = vec![0];
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                starts.push(i + 1);
            }
        }
        starts
    }

    /// Maps a local `(x, y)` click/drag inside `TextBlock` to a global UTF-8 byte index.
    pub(crate) fn global_byte_at_point(&self, x: f32, y: f32) -> usize {
        let starts = self.line_start_offsets();
        let mut runs = self.buffer.layout_runs().peekable();
        if runs.peek().is_none() {
            return 0;
        }

        let mut chosen_line_i = 0usize;
        let mut chosen_glyphs: &[cosmic_text::LayoutGlyph] = &[];
        let mut found = false;
        for run in runs {
            chosen_line_i = run.line_i;
            chosen_glyphs = run.glyphs;
            found = true;
            if y < run.line_top + self.style.line_height {
                break;
            }
        }
        if !found {
            return 0;
        }

        let line_start = starts.get(chosen_line_i).copied().unwrap_or(0);
        let local = byte_in_run(chosen_glyphs, x);
        (line_start + local).min(self.text.len())
    }

    pub(crate) fn move_vertical_byte(&self, global_byte: usize, delta: isize) -> usize {
        let starts = self.line_start_offsets();
        let line_i = self.text.as_bytes()[..global_byte.min(self.text.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        let line_start = starts.get(line_i).copied().unwrap_or(0);
        let local_byte = global_byte.saturating_sub(line_start);

        let runs: Vec<_> = self.buffer.layout_runs().collect();
        if runs.is_empty() {
            return global_byte;
        }

        let mut cur_vis = runs.len().saturating_sub(1);
        for (idx, run) in runs.iter().enumerate() {
            if run.line_i == line_i {
                let run_end = run.glyphs.last().map(|g| g.end).unwrap_or(0);
                cur_vis = idx;
                if local_byte <= run_end || run.glyphs.is_empty() {
                    break;
                }
            }
        }

        let desired_x = x_in_run(runs[cur_vis].glyphs, local_byte);
        let target_vis = if delta < 0 {
            cur_vis.saturating_sub(delta.unsigned_abs())
        } else {
            (cur_vis + delta as usize).min(runs.len() - 1)
        };

        let target_run = &runs[target_vis];
        let target_start = starts.get(target_run.line_i).copied().unwrap_or(0);
        (target_start + byte_in_run(target_run.glyphs, desired_x)).min(self.text.len())
    }

    pub(crate) fn collect_selection_rects(
        &self,
        start_byte: usize,
        end_byte: usize,
        out: &mut Vec<(f32, f32, f32)>,
    ) {
        out.clear();
        if start_byte >= end_byte {
            return;
        }
        let starts = self.line_start_offsets();
        for run in self.buffer.layout_runs() {
            let line_start = starts.get(run.line_i).copied().unwrap_or(0);
            let run_start = line_start + run.glyphs.first().map(|g| g.start).unwrap_or(0);
            let run_end = line_start + run.glyphs.last().map(|g| g.end).unwrap_or(0);

            if run.glyphs.is_empty() {
                if start_byte <= line_start && end_byte > line_start {
                    out.push((0.0, run.line_top, 6.0));
                }
                continue;
            }

            let s = start_byte.max(run_start);
            let e = end_byte.min(run_end);
            if s < e {
                let x1 = x_in_run(run.glyphs, s - line_start);
                let x2 = x_in_run(run.glyphs, e - line_start);
                out.push((x1.min(x2), run.line_top, (x2 - x1).abs().max(2.0)));
            } else if start_byte <= run_end && end_byte > run_end {
                // Highlight trailing newline past end of visual line
                let x = run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0);
                out.push((x, run.line_top, 5.0));
            }
        }
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
}

pub(crate) fn swash_is_color(content: SwashContent) -> bool {
    matches!(content, SwashContent::Color)
}

fn x_in_run(glyphs: &[cosmic_text::LayoutGlyph], local_byte: usize) -> f32 {
    let mut x = 0.0;
    for g in glyphs {
        if g.end > local_byte {
            return g.x;
        }
        x = g.x + g.w;
    }
    x
}

fn byte_in_run(glyphs: &[cosmic_text::LayoutGlyph], x: f32) -> usize {
    let mut best = 0;
    let mut best_dist = x.abs();
    for g in glyphs {
        let d0 = (g.x - x).abs();
        if d0 < best_dist {
            best_dist = d0;
            best = g.start;
        }
        let end = g.end;
        let d1 = (g.x + g.w - x).abs();
        if d1 < best_dist {
            best_dist = d1;
            best = end;
        }
    }
    best
}
