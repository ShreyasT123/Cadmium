#[derive(Debug, Clone)]
enum DrawBatch {
    Sdf(Range<u32>),
    Image {
        tex: TextureHandle,
        range: Range<u32>,
    },
    Text(Range<u32>),
    Path {
        index_range: Range<u32>,
        vertex_offset: i32,
    },
    Glass(u32),
    SetScissor(Option<Rect>),
}

/// A frame's recorded draw commands, accumulated independently of the GPU.
///
/// A `DrawList` stores instanced quads, tessellated path geometry, glyph
/// instances, glass quads, and clip-state changes in submission order.
/// Consecutive SDF, text, and same-texture image draws are merged into
/// batches; each glass quad is its own batch because it samples the
/// backdrop produced so far. A list is normally created by
/// [`crate::window::FrameContext`] during
/// [`crate::window::WindowDelegate::render`], then submitted exactly once by
/// [`WindowRenderTarget::render`].
///
/// See the [module documentation](crate::renderer) for which primitives take
/// logical versus physical coordinates.
pub struct DrawList {
    scale: f32,
    sdf_instances: Vec<RawSdfInstance>,
    image_instances: Vec<RawImageInstance>,
    glyph_instances: Vec<RawGlyphInstance>,
    glass_instances: Vec<RawGlassInstance>,
    path_vertices: Vec<PathVertex>,
    path_indices: Vec<u32>,
    clip_stack: Vec<Rect>,
    opacity_stack: Vec<f32>,
    glyph_scratch: Vec<ShapedGlyph>,
    batches: Vec<DrawBatch>,
    atlas_overflowed: bool,
    has_liquid_glass: bool,
}

impl DrawList {
    /// Creates an empty draw list for a frame rendered at `scale`.
    ///
    /// The scale factor converts the logical coordinates used by text,
    /// strokes and clips into physical pixels. Use the same value as the
    /// window's scale factor so that quad geometry, which callers supply in
    /// physical pixels, lines up with scaled primitives. Use `1.0` when
    /// logical and physical pixels coincide; negative or zero scales are not
    /// meaningful.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::core::Rect;
    /// use cadmium::renderer::DrawList;
    ///
    /// let mut frame = DrawList::new(2.0);
    /// frame.push_clip(Rect::new(8.0, 8.0, 100.0, 40.0));
    /// // Record primitives, then restore the previous clip.
    /// frame.pop_clip();
    /// ```
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            sdf_instances: Vec::new(),
            image_instances: Vec::new(),
            glyph_instances: Vec::new(),
            glass_instances: Vec::new(),
            path_vertices: Vec::new(),
            path_indices: Vec::new(),
            clip_stack: Vec::new(),
            opacity_stack: Vec::new(),
            glyph_scratch: Vec::new(),
            batches: Vec::new(),
            atlas_overflowed: false,
            has_liquid_glass: false,
        }
    }

    /// Returns the opacity inherited from the current opacity stack.
    fn current_opacity(&self) -> f32 {
        self.opacity_stack.last().copied().unwrap_or(1.0)
    }

    /// Multiplies the opacity applied to every primitive pushed afterwards.
    ///
    /// Pair every call with [`Self::pop_opacity`]. Nested calls multiply.
    pub fn push_opacity(&mut self, alpha: f32) {
        let next = self.current_opacity() * alpha.clamp(0.0, 1.0);
        self.opacity_stack.push(next);
    }

    /// Restores the opacity in effect before the matching [`Self::push_opacity`].
    pub fn pop_opacity(&mut self) {
        self.opacity_stack.pop();
    }

    /// Records a rounded-box SDF quad.
    ///
    /// The quad's fields are copied into an instance buffer at render time.
    /// Consecutive SDF pushes are merged into a single draw call. No culling
    /// or validation happens here; fully transparent quads are discarded by
    /// the shader. See [`SdfQuad`] for the coordinate space.
    pub fn push_sdf(&mut self, mut quad: SdfQuad) {
        let op = self.current_opacity();
        if op < 0.999 {
            quad.fill.a *= op;
            if let Some(end) = &mut quad.fill_end {
                end.a *= op;
            }
            quad.border_color.a *= op;
            quad.shadow_color.a *= op;
        }
        let idx = self.sdf_instances.len() as u32;
        self.sdf_instances.push(RawSdfInstance::from(quad));
        match self.batches.last_mut() {
            Some(DrawBatch::Sdf(range)) if range.end == idx => range.end = idx + 1,
            _ => self.batches.push(DrawBatch::Sdf(idx..(idx + 1))),
        }
    }

    /// Records a textured quad.
    ///
    /// Consecutive images that reference the same [`TextureHandle`] are
    /// merged into one draw call. The handle is resolved when the list is
    /// rendered: if [`GpuContext::release_texture`] was called in the
    /// meantime, the corresponding batch is skipped rather than panicking.
    pub fn push_image(&mut self, mut img: ImageQuad) {
        let op = self.current_opacity();
        if op < 0.999 {
            img.tint.a *= op;
        }
        let idx = self.image_instances.len() as u32;
        let tex = img.texture;
        self.image_instances.push(RawImageInstance::from(img));
        match self.batches.last_mut() {
            Some(DrawBatch::Image { tex: prev, range }) if *prev == tex && range.end == idx => {
                range.end = idx + 1;
            }
            _ => self.batches.push(DrawBatch::Image {
                tex,
                range: idx..(idx + 1),
            }),
        }
    }

    /// Records a glass quad for the backdrop-aware glass pass.
    ///
    /// The quad is first intersected with the current clip stack (clips are
    /// logical, the quad rectangle physical): fully clipped quads are
    /// dropped. Every glass quad is drawn in its own batch so that it sees
    /// the backdrop produced by earlier batches.
    ///
    /// Effects such as [`GlassQuad::liquid_ripple`],
    /// [`GlassQuad::conic_strength`] above `0.001`, or
    /// [`GlassQuad::frost`] below `0.30` cause the renderer to allocate an
    /// extra sharp-backdrop snapshot for accurate distortion.
    pub fn push_glass(&mut self, mut glass: GlassQuad) {
        let op = self.current_opacity();
        if op < 0.999 {
            glass.tint.a *= op;
            glass.border_color.a *= op;
        }
        if let Some(clip) = self.clip_stack.last().copied() {
            let scaled_clip = Rect::new(
                clip.x * self.scale,
                clip.y * self.scale,
                clip.width * self.scale,
                clip.height * self.scale,
            );
            let ix = glass.rect.x.max(scaled_clip.x);
            let iy = glass.rect.y.max(scaled_clip.y);
            let ir = (glass.rect.x + glass.rect.width).min(scaled_clip.x + scaled_clip.width);
            let ib = (glass.rect.y + glass.rect.height).min(scaled_clip.y + scaled_clip.height);
            if ir <= ix || ib <= iy {
                return;
            }
        }
        if glass.liquid_ripple > 0.001 || glass.conic_strength > 0.001 || glass.frost < 0.30 {
            self.has_liquid_glass = true;
        }
        let idx = self.glass_instances.len() as u32;
        self.glass_instances.push(RawGlassInstance::from(glass));
        self.batches.push(DrawBatch::Glass(idx));
    }

    /// Pushes a clip rectangle in logical coordinates onto the clip stack.
    ///
    /// The new clip is the intersection of `rect` with the current clip, or
    /// `rect` itself when the stack is empty. A scissor change is recorded at
    /// this point in the batch order, so only draws recorded afterwards are
    /// affected. Clips are scaled by the frame's scale factor and rounded
    /// to physical pixels when applied. Pair every push with
    /// [`Self::pop_clip`].
    pub fn push_clip(&mut self, rect: Rect) {
        let clipped = match self.clip_stack.last().copied() {
            Some(parent) => intersect_rect(parent, rect),
            None => rect,
        };
        self.clip_stack.push(clipped);
        self.batches.push(DrawBatch::SetScissor(Some(clipped)));
    }

    /// Pops the most recent clip and restores the parent scissor rectangle.
    ///
    /// When the clip stack becomes empty, subsequent draws use the full
    /// render target. Extra pops on an empty stack do not panic; they simply
    /// record another full-target scissor.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        self.batches
            .push(DrawBatch::SetScissor(self.clip_stack.last().copied()));
    }

    /// Tessellates and records a stroked path.
    ///
    /// Path coordinates and `width` are logical; tessellated vertex
    /// positions are multiplied by the frame's scale factor to become
    /// physical pixels. `color` is used for every vertex. The stroke uses
    /// lyon's default stroke options with the given line width. If
    /// tessellation fails or produces no triangles, nothing is recorded.
    ///
    /// Use [`PathBuilder`] to construct `path` with the matching `lyon`
    /// version.
    pub fn push_stroke_path(&mut self, path: &LyonPath, mut color: Color, width: f32) {
        let op = self.current_opacity();
        if op < 0.999 {
            color.a *= op;
        }
        let v_start = self.path_vertices.len() as i32;
        let i_start = self.path_indices.len() as u32;

        let mut tessellator = StrokeTessellator::new();
        let options = StrokeOptions::default().with_line_width(width);
        let mut geometry: VertexBuffers<PathVertex, u32> = VertexBuffers::new();
        let color_arr = color.to_array();

        let res = tessellator.tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut geometry, |pos: StrokeVertex| PathVertex {
                position: [pos.position().x * self.scale, pos.position().y * self.scale],
                color: color_arr,
            }),
        );

        if res.is_ok() && !geometry.indices.is_empty() {
            self.path_vertices.extend(geometry.vertices);
            self.path_indices.extend(geometry.indices);
            let i_end = self.path_indices.len() as u32;
            self.batches.push(DrawBatch::Path {
                index_range: i_start..i_end,
                vertex_offset: v_start,
            });
        }
    }

    /// Records shaped text using the color stored in `block`'s style.
    ///
    /// `logical_x` and `logical_y` are the logical-space origin of the text
    /// block; glyph positions from `block` are offset from that origin and
    /// scaled by the frame's scale factor. Glyph masks are rasterized on
    /// demand into the GPU context's shared glyph atlas. This is equivalent
    /// to calling [`Self::push_text_colored`] with `None`.
    pub fn push_text(
        &mut self,
        gpu: &mut GpuContext,
        block: &TextBlock,
        logical_x: f32,
        logical_y: f32,
    ) {
        self.push_text_colored(gpu, block, logical_x, logical_y, None);
    }

    /// Records shaped text, optionally overriding the block's text color.
    ///
    /// `logical_x` and `logical_y` are the logical-space origin of the block.
    /// When `override_color` is `Some`, it replaces the color from
    /// [`TextBlock::style`] for all monochrome glyphs; color glyphs keep
    /// their own RGB and only take the override's alpha. Glyphs whose mask
    /// cannot be rasterized (for example while the atlas is growing) are
    /// skipped; a pending atlas growth makes
    /// [`WindowRenderTarget::render`] return `true` so another frame can be
    /// scheduled.
    pub fn push_text_colored(
        &mut self,
        gpu: &mut GpuContext,
        block: &TextBlock,
        logical_x: f32,
        logical_y: f32,
        override_color: Option<Color>,
    ) {
        let origin_x_px = (logical_x * self.scale).round();
        let origin_y_px = (logical_y * self.scale).round();
        block.collect_glyphs_into(
            origin_x_px,
            origin_y_px,
            self.scale,
            &mut self.glyph_scratch,
        );
        let mut style_color = override_color.unwrap_or_else(|| block.style().color);
        let op = self.current_opacity();
        if op < 0.999 {
            style_color.a *= op;
        }

        let start = self.glyph_instances.len() as u32;
        for g in &self.glyph_scratch {
            let Some(entry) =
                gpu.glyph_atlas
                    .get_or_insert(&gpu.queue, &mut gpu.text_engine, g.cache_key)
            else {
                if gpu.glyph_atlas.overflow_pending && gpu.glyph_atlas.size < MAX_ATLAS_SIZE {
                    self.atlas_overflowed = true;
                }
                continue;
            };

            if entry.width == 0 || entry.height == 0 {
                continue;
            }

            let gx = (g.x + entry.left) as f32;
            let gy = (g.y - entry.top) as f32;
            let color = if entry.is_color {
                Color::rgba(1.0, 1.0, 1.0, style_color.a)
            } else {
                style_color
            };

            self.glyph_instances.push(RawGlyphInstance {
                rect: [gx, gy, entry.width as f32, entry.height as f32],
                uv_rect: entry.uv_rect,
                color: color.to_array(),
            });
        }
        let end = self.glyph_instances.len() as u32;

        if end > start {
            match self.batches.last_mut() {
                Some(DrawBatch::Text(range)) if range.end == start => range.end = end,
                _ => self.batches.push(DrawBatch::Text(start..end)),
            }
        }
    }
}

fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);
    Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

