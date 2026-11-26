/// A string shaped once and reusable for measurement and drawing.
///
/// Construct one with [`TextBlock::new`], query its size with
/// [`TextBlock::measure`], and pass it to
/// [`crate::window::FrameContext::push_text`] to draw. The block owns its text
/// and [`TextStyle`]; the [`TextEngine`] is only borrowed for shaping operations.
pub struct TextBlock {
    buffer: Buffer,
    text: String,
    style: TextStyle,
    last_max_width: Option<f32>,
    last_single_line: bool,
    single_line: bool,
}

impl TextBlock {
    /// Creates a block and shapes `text` with the given `style`.
    ///
    /// `engine` is borrowed mutably because shaping may select or load fonts.
    /// ASCII text uses basic shaping and other text uses advanced shaping.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::text::{TextBlock, TextEngine, TextStyle};
    ///
    /// let mut engine = TextEngine::new();
    /// let block = TextBlock::new(&mut engine, "Hello, world", TextStyle::default());
    /// # let _ = block;
    /// ```
    pub fn new(engine: &mut TextEngine, text: impl Into<String>, style: TextStyle) -> Self {
        let text = text.into();
        let mut buffer = Buffer::new(
            &mut engine.font_system,
            Metrics::new(style.font_size, style.line_height),
        );
        let attrs = Attrs::new()
            .family(Family::SansSerif)
            .weight(style.weight.into());
        let shaping = if text.is_ascii() {
            Shaping::Basic
        } else {
            Shaping::Advanced
        };
        buffer.set_text(&text, &attrs, shaping, None);
        buffer.shape_until_scroll(&mut engine.font_system, false);

        Self {
            buffer,
            text,
            style,
            last_max_width: None,
            last_single_line: false,
            single_line: false,
        }
    }

    /// Returns a copy of the style used to shape and draw this block.
    pub fn style(&self) -> TextStyle {
        self.style
    }

    /// Changes the color used when this block is drawn.
    ///
    /// This only updates [`TextStyle::color`]; the text is not re-shaped or
    /// re-measured. [`crate::renderer::DrawList::push_text_colored`] can override
    /// the color for a single draw.
    pub fn set_color(&mut self, color: Color) {
        self.style.color = color;
    }

    /// Sets whether this block is measured and painted as a single clipped line.
    ///
    /// Single-line blocks never wrap; their measured width is capped at the
    /// supplied maximum width and their height is exactly one line height.
    pub fn set_single_line_ellipsis(&mut self, enabled: bool) {
        self.single_line = enabled;
    }

    /// Returns `true` when this block is configured for single-line clipping.
    pub(crate) fn is_single_line(&self) -> bool {
        self.single_line
    }

    /// Replaces the text and re-shapes the buffer with the current style.
    ///
    /// `engine` is borrowed mutably for shaping. If `text` equals the current
    /// text, the method returns without doing any work.
    pub fn set_text(&mut self, engine: &mut TextEngine, text: &str) {
        if self.text == text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);

        let attrs = Attrs::new()
            .family(Family::SansSerif)
            .weight(self.style.weight.into());
        let shaping = if text.is_ascii() {
            Shaping::Basic
        } else {
            Shaping::Advanced
        };
        self.buffer.set_text(text, &attrs, shaping, None);
        self.buffer
            .shape_until_scroll(&mut engine.font_system, false);
    }

    /// Measures the block, returning `(width, height)` in logical pixels.
    ///
    /// `max_width` sets the wrapping width in logical pixels; `None` leaves the
    /// buffer unconstrained. When it differs from the previous call the text is
    /// re-shaped. Width is the widest laid-out line and height is the line count
    /// (at least one) multiplied by [`TextStyle::line_height`]. Both components
    /// are rounded up.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::text::{TextBlock, TextEngine, TextStyle};
    ///
    /// let mut engine = TextEngine::new();
    /// let mut block = TextBlock::new(&mut engine, "A short sentence", TextStyle::default());
    /// let (width, height) = block.measure(&mut engine, Some(120.0));
    /// assert!(width > 0.0);
    /// assert!(height >= 18.0);
    /// ```
    pub fn measure(&mut self, engine: &mut TextEngine, max_width: Option<f32>) -> (f32, f32) {
        // Single-line blocks are shaped without a wrap width; the caller's
        // `max_width` only caps the reported measurement and the paint clip.
        let shape_width = if self.single_line { None } else { max_width };
        if self.last_max_width != shape_width || self.last_single_line != self.single_line {
            self.last_max_width = shape_width;
            self.last_single_line = self.single_line;
            self.buffer.set_size(shape_width, None);
            self.buffer
                .shape_until_scroll(&mut engine.font_system, false);
        }

        if self.single_line {
            let width = self
                .buffer
                .layout_runs()
                .next()
                .map(|run| run.line_w)
                .unwrap_or(0.0);
            let width = max_width.map_or(width, |mw| width.min(mw));
            return (width.ceil(), self.style.line_height.ceil());
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

    /// Returns the logical `(top, x)` position of a global UTF-8 byte offset.
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

    /// Returns the UTF-8 byte offset where each visual line begins.
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

    /// Moves a byte offset up or down by `delta` visual lines, preserving horizontal position.
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

    /// Appends `(x, top, width)` rectangles for the selection range to `out`.
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

    /// Appends positioned glyph cache keys for `out` at a physical-pixel origin.
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

    /// Appends `(byte offset, x)` pairs for the first visual line to `out`.
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

/// Reports whether a swash glyph image contains color data rather than a mask.
pub(crate) fn swash_is_color(content: SwashContent) -> bool {
    matches!(content, SwashContent::Color)
}

/// Finds the x coordinate of a byte offset within one laid-out line.
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

/// Finds the nearest glyph-edge byte offset to an x coordinate within one line.
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
