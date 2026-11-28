/// Dual-Kawase ping-pong iterations (1/2 -> 1/4 -> 1/2 resolution) run after the initial
/// full -> 1/2 downsample. With the offsets from `blur_pyramid_uniforms` each pair adds roughly
/// 18 px² of variance (physical pixels), so 3 pairs give a sigma of about 7 px with no tap ghosting.
const BLUR_PAIRS: usize = 3;

/// Physical-pixel margin added around a glass rect when testing whether earlier draws invalidated
/// the backdrop: covers the blur footprint and the ripple displacement outside the rect.
const GLASS_REBLUR_MARGIN: f32 = 32.0;

/// Per-window GPU buffers and render targets used to draw a frame.
///
/// One target is created for each window by [`crate::window::App`]. It owns
/// the viewport uniform buffer, instance and geometry buffers whose
/// capacities grow on demand, and the offscreen targets required for glass
/// compositing. The target is reused between frames; call [`Self::resize`]
/// when the surface dimensions change.
pub struct WindowRenderTarget {
    width: u32,
    height: u32,
    scale: f32,
    glass_offscreen: Option<GlassOffscreen>,

    globals_buffer: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,

    sdf_buffer: wgpu::Buffer,
    sdf_capacity: usize,

    image_buffer: wgpu::Buffer,
    image_capacity: usize,

    glyph_buffer: wgpu::Buffer,
    glyph_capacity: usize,

    glass_buffer: wgpu::Buffer,
    glass_capacity: usize,

    path_vbuf: wgpu::Buffer,
    path_vcapacity: usize,
    path_ibuf: wgpu::Buffer,
    path_icapacity: usize,
}

impl WindowRenderTarget {
    /// Creates a target for a `width` × `height` physical-pixel surface.
    ///
    /// Zero dimensions are clamped to one. Initial buffer capacities are
    /// 128 SDF instances, 32 images, 512 glyphs, 16 glass quads, 256 path
    /// vertices and 512 path indices; each grows to the next power of two
    /// when a frame needs more. Offscreen glass targets are not allocated
    /// until a frame actually contains glass.
    pub fn new(gpu: &GpuContext, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        let globals = GlobalsUniform {
            viewport_size: [width as f32, height as f32],
            scale: 1.0,
            _pad: 0.0,
        };
        let globals_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SteelUI Globals"),
                contents: bytemuck::bytes_of(&globals),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let globals_bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SteelUI Globals BG"),
            layout: &gpu.globals_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let sdf_capacity = 128;
        let sdf_buffer = create_vertex_buffer::<RawSdfInstance>(&gpu.device, sdf_capacity, "SDF");

        let image_capacity = 32;
        let image_buffer =
            create_vertex_buffer::<RawImageInstance>(&gpu.device, image_capacity, "Image");

        let glyph_capacity = 512;
        let glyph_buffer =
            create_vertex_buffer::<RawGlyphInstance>(&gpu.device, glyph_capacity, "Glyph");

        let glass_capacity = 16;
        let glass_buffer =
            create_vertex_buffer::<RawGlassInstance>(&gpu.device, glass_capacity, "Glass");

        let path_vcapacity = 256;
        let path_vbuf = create_vertex_buffer::<PathVertex>(&gpu.device, path_vcapacity, "Path");
        let path_icapacity = 512;
        let path_ibuf = create_index_buffer(&gpu.device, path_icapacity, "Path Indices");

        Self {
            width,
            height,
            scale: 1.0,
            glass_offscreen: None,
            globals_buffer,
            globals_bg,
            sdf_buffer,
            sdf_capacity,
            image_buffer,
            image_capacity,
            glyph_buffer,
            glyph_capacity,
            glass_buffer,
            glass_capacity,
            path_vbuf,
            path_vcapacity,
            path_ibuf,
            path_icapacity,
        }
    }

    /// Updates the physical dimensions used for rendering and the viewport uniform.
    ///
    /// Zero dimensions are clamped to one; unchanged dimensions are a no-op.
    /// Existing offscreen glass targets are discarded because their size no
    /// longer matches the surface, and will be recreated on the next frame
    /// that needs them. The uniform keeps the current frame scale, so shader
    /// effects measured in logical pixels survive a resize. Call this
    /// whenever the surface is reconfigured.
    pub fn resize(&mut self, gpu: &GpuContext, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        self.glass_offscreen = None;

        let globals = GlobalsUniform {
            viewport_size: [width as f32, height as f32],
            scale: self.scale,
            _pad: 0.0,
        };
        gpu.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
    }

    /// Uploads `draw_list`, renders one frame into `surface_view`, and submits it.
    ///
    /// Pipelines are created on demand for `surface_format`, instance and
    /// geometry buffers grow as needed, and the viewport uniform is updated
    /// when the draw list's scale differs from the target's current scale.
    /// The frame is cleared to `clear_color`. If no glass quads are present,
    /// all primitives are drawn directly to `surface_view`; otherwise the
    /// UI is drawn offscreen, the glass backdrop is blurred, and the result
    /// is blitted to `surface_view`. All commands are submitted to
    /// `gpu.queue` before this method returns.
    ///
    /// Glass quads are composited in draw order. The backdrop snapshot and
    /// blur pyramid are rebuilt only when a primitive or an earlier glass
    /// quad touched the area the next glass quad samples, so glass can
    /// refract and blur other glass.
    ///
    /// Returns `true` when glyph rasterization had to skip glyphs because
    /// the atlas is full at its maximum size. Schedule another frame in that
    /// case so the enlarged atlas can be used; [`crate::window::App`] does
    /// this automatically.
    ///
    /// `surface_format` must be the format the surface was configured with,
    /// `surface_view` must be a render-attachment view of that surface, and
    /// `draw_list` must have been recorded with the same [`GpuContext`].
    ///
    /// # Panics
    ///
    /// Panics if `wgpu` rejects the resources or format (for example, an
    /// incompatible `surface_view`), or if required pipelines are missing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cadmium::core::Color;
    /// use cadmium::renderer::{DrawList, GpuContext, WindowRenderTarget};
    ///
    /// # fn draw(
    /// #     gpu: &mut GpuContext,
    /// #     target: &mut WindowRenderTarget,
    /// #     view: &wgpu::TextureView,
    /// #     format: wgpu::TextureFormat,
    /// #     frame: &DrawList,
    /// # ) -> bool {
    /// let atlas_overflowed = target.render(
    ///     gpu,
    ///     view,
    ///     format,
    ///     Color::rgba(0.0, 0.0, 0.0, 1.0),
    ///     frame,
    /// );
    /// # atlas_overflowed
    /// # }
    /// ```
    pub fn render(
        &mut self,
        gpu: &mut GpuContext,
        surface_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        clear_color: Color,
        draw_list: &DrawList,
    ) -> bool {
        if (self.scale - draw_list.scale).abs() > f32::EPSILON {
            self.scale = draw_list.scale;
            let globals = GlobalsUniform {
                viewport_size: [self.width as f32, self.height as f32],
                scale: self.scale,
                _pad: 0.0,
            };
            gpu.queue
                .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        }

        gpu.ensure_base_pipelines(surface_format);
        if !draw_list.image_instances.is_empty() {
            gpu.ensure_image_pipeline(surface_format);
        }
        let needs_glass = !draw_list.glass_instances.is_empty();
        if needs_glass {
            gpu.ensure_glass_pipelines(surface_format);
        }

        if draw_list.sdf_instances.len() > self.sdf_capacity {
            let new_cap = draw_list.sdf_instances.len().next_power_of_two();
            self.sdf_buffer = create_vertex_buffer::<RawSdfInstance>(&gpu.device, new_cap, "SDF");
            self.sdf_capacity = new_cap;
        }
        if !draw_list.sdf_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.sdf_buffer,
                0,
                bytemuck::cast_slice(&draw_list.sdf_instances),
            );
        }

        if draw_list.image_instances.len() > self.image_capacity {
            let new_cap = draw_list.image_instances.len().next_power_of_two();
            self.image_buffer =
                create_vertex_buffer::<RawImageInstance>(&gpu.device, new_cap, "Image");
            self.image_capacity = new_cap;
        }
        if !draw_list.image_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.image_buffer,
                0,
                bytemuck::cast_slice(&draw_list.image_instances),
            );
        }

        if draw_list.glyph_instances.len() > self.glyph_capacity {
            let new_cap = draw_list.glyph_instances.len().next_power_of_two();
            self.glyph_buffer =
                create_vertex_buffer::<RawGlyphInstance>(&gpu.device, new_cap, "Glyph");
            self.glyph_capacity = new_cap;
        }
        if !draw_list.glyph_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.glyph_buffer,
                0,
                bytemuck::cast_slice(&draw_list.glyph_instances),
            );
        }

        if draw_list.glass_instances.len() > self.glass_capacity {
            let new_cap = draw_list.glass_instances.len().next_power_of_two();
            self.glass_buffer =
                create_vertex_buffer::<RawGlassInstance>(&gpu.device, new_cap, "Glass");
            self.glass_capacity = new_cap;
        }
        if !draw_list.glass_instances.is_empty() {
            gpu.queue.write_buffer(
                &self.glass_buffer,
                0,
                bytemuck::cast_slice(&draw_list.glass_instances),
            );
        }

        if draw_list.path_vertices.len() > self.path_vcapacity {
            let new_cap = draw_list.path_vertices.len().next_power_of_two();
            self.path_vbuf = create_vertex_buffer::<PathVertex>(&gpu.device, new_cap, "Path");
            self.path_vcapacity = new_cap;
        }
        if !draw_list.path_vertices.is_empty() {
            gpu.queue.write_buffer(
                &self.path_vbuf,
                0,
                bytemuck::cast_slice(&draw_list.path_vertices),
            );
        }
        if draw_list.path_indices.len() > self.path_icapacity {
            let new_cap = draw_list.path_indices.len().next_power_of_two();
            self.path_ibuf = create_index_buffer(&gpu.device, new_cap, "Path Indices");
            self.path_icapacity = new_cap;
        }
        if !draw_list.path_indices.is_empty() {
            gpu.queue.write_buffer(
                &self.path_ibuf,
                0,
                bytemuck::cast_slice(&draw_list.path_indices),
            );
        }

        if !needs_glass {
            self.glass_offscreen = None;
            return self.render_direct(gpu, surface_view, surface_format, clear_color, draw_list);
        }

        if self
            .glass_offscreen
            .as_ref()
            .is_some_and(|g| g.format != surface_format)
        {
            self.glass_offscreen = None;
        }
        let width = self.width;
        let height = self.height;
        let bufs = BatchBuffers {
            width,
            height,
            globals_bg: &self.globals_bg,
            sdf_buffer: &self.sdf_buffer,
            image_buffer: &self.image_buffer,
            glyph_buffer: &self.glyph_buffer,
            path_vbuf: &self.path_vbuf,
            path_ibuf: &self.path_ibuf,
        };
        let go = self
            .glass_offscreen
            .get_or_insert_with(|| GlassOffscreen::new(gpu, width, height, surface_format));
        if draw_list.has_liquid_glass {
            go.ensure_liquid_backdrop(gpu, width, height);
        } else {
            go.liquid_backdrop = None;
        }

        let pipes = &gpu.pipelines[&surface_format];
        let blur_pipe = pipes.blur.as_ref().expect("blur pipeline");
        let glass_pipe = pipes.glass.as_ref().expect("glass pipeline");
        let blit_pipe = pipes.blit.as_ref().expect("blit pipeline");

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SteelUI Glass Frame Encoder"),
            });

        let mut is_first_pass = true;
        let mut active_scissor: Option<Rect> = None;
        let mut dirty_backdrop: Option<Rect> = None;
        let mut has_blur_pyramid = false;
        let mut i = 0;

        while i < draw_list.batches.len() {
            match &draw_list.batches[i] {
                DrawBatch::SetScissor(sc) => {
                    active_scissor = *sc;
                    i += 1;
                }
                DrawBatch::Glass(idx) => {
                    let Some(glass) = draw_list.glass_instances.get(*idx as usize) else {
                        i += 1;
                        continue;
                    };
                    let glass_rect = glass.rect;
                    if !scissor_intersects_rect(
                        active_scissor,
                        glass_rect,
                        draw_list.scale,
                        width,
                        height,
                    ) {
                        i += 1;
                        continue;
                    }

                    if is_first_pass {
                        let _ = begin_ui_pass(
                            &mut encoder,
                            &go.canonical_view,
                            wgpu::LoadOp::Clear(clear_color.to_wgpu()),
                        );
                        is_first_pass = false;
                    }

                    // The glass reads the backdrop a blur radius (and a ripple amplitude) beyond its own
                    // rect, so the staleness probe is padded.
                    let probe = [
                        glass_rect[0] - GLASS_REBLUR_MARGIN,
                        glass_rect[1] - GLASS_REBLUR_MARGIN,
                        glass_rect[2] + GLASS_REBLUR_MARGIN * 2.0,
                        glass_rect[3] + GLASS_REBLUR_MARGIN * 2.0,
                    ];
                    let need_reblur = !has_blur_pyramid
                        || dirty_backdrop.is_some_and(|d| phys_rects_intersect(d, probe));

                    if need_reblur {
                        if let Some((backdrop_tex, _)) = &go.liquid_backdrop {
                            encoder.copy_texture_to_texture(
                                wgpu::TexelCopyTextureInfo {
                                    texture: &go.canonical_tex,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                wgpu::TexelCopyTextureInfo {
                                    texture: backdrop_tex,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                wgpu::Extent3d {
                                    width,
                                    height,
                                    depth_or_array_layers: 1,
                                },
                            );
                        }

                        // full -> 1/2, then BLUR_PAIRS x (1/2 -> 1/4 -> 1/2). Pass 1 and pass 2 use the
                        // same param bind groups every iteration; the result always lands in mip1.
                        run_blur_pass(
                            &mut encoder,
                            blur_pipe,
                            &go.canonical_linear_bg,
                            &go.blur_param_bgs[0],
                            &go.mip1_view,
                        );
                        for _ in 0..BLUR_PAIRS {
                            run_blur_pass(
                                &mut encoder,
                                blur_pipe,
                                &go.mip1_bg,
                                &go.blur_param_bgs[1],
                                &go.mip2_view,
                            );
                            run_blur_pass(
                                &mut encoder,
                                blur_pipe,
                                &go.mip2_bg,
                                &go.blur_param_bgs[2],
                                &go.mip1_view,
                            );
                        }
                        has_blur_pyramid = true;
                        dirty_backdrop = None;
                    }

                    {
                        let mut pass =
                            begin_ui_pass(&mut encoder, &go.canonical_view, wgpu::LoadOp::Load);
                        if apply_scissor(&mut pass, active_scissor, draw_list.scale, width, height)
                        {
                            let sharp_bg = go
                                .liquid_backdrop
                                .as_ref()
                                .map(|(_, bg)| bg)
                                .unwrap_or(&go.mip1_bg);
                            pass.set_pipeline(glass_pipe);
                            pass.set_bind_group(0, &self.globals_bg, &[]);
                            pass.set_bind_group(1, sharp_bg, &[]);
                            pass.set_bind_group(2, &go.mip1_bg, &[]);
                            pass.set_vertex_buffer(0, self.glass_buffer.slice(..));
                            pass.draw(0..4, *idx..(*idx + 1));
                        }
                    }

                    // The glass just wrote into the canonical target; any later glass overlapping it must
                    // snapshot and blur the result, not the stale pre-glass copy.
                    dirty_backdrop = union_opt_rect(
                        dirty_backdrop,
                        Rect::new(
                            glass_rect[0] - 1.0,
                            glass_rect[1] - 1.0,
                            glass_rect[2] + 2.0,
                            glass_rect[3] + 2.0,
                        ),
                    );
                    i += 1;
                }
                DrawBatch::Sdf(_)
                | DrawBatch::Image { .. }
                | DrawBatch::Text(_)
                | DrawBatch::Path { .. } => {
                    let load = if is_first_pass {
                        is_first_pass = false;
                        wgpu::LoadOp::Clear(clear_color.to_wgpu())
                    } else {
                        wgpu::LoadOp::Load
                    };

                    let start_i = i;
                    {
                        let mut pass = begin_ui_pass(&mut encoder, &go.canonical_view, load);
                        i = record_standard_batches(
                            &mut pass,
                            pipes,
                            gpu,
                            &bufs,
                            draw_list,
                            i,
                            &mut active_scissor,
                        );
                    }
                    for b in &draw_list.batches[start_i..i] {
                        if let Some(r) = batch_phys_bounds(draw_list, b, width, height) {
                            dirty_backdrop = union_opt_rect(dirty_backdrop, r);
                        }
                    }
                }
            }
        }

        if is_first_pass {
            let _ = begin_ui_pass(
                &mut encoder,
                &go.canonical_view,
                wgpu::LoadOp::Clear(clear_color.to_wgpu()),
            );
        }

        {
            let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SteelUI Final Blit Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            blit_pass.set_pipeline(blit_pipe);
            blit_pass.set_bind_group(0, &go.canonical_blit_bg, &[]);
            blit_pass.draw(0..3, 0..1);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        draw_list.atlas_overflowed
    }

    fn render_direct(
        &self,
        gpu: &GpuContext,
        surface_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        clear_color: Color,
        draw_list: &DrawList,
    ) -> bool {
        let pipes = &gpu.pipelines[&surface_format];
        let bufs = BatchBuffers {
            width: self.width,
            height: self.height,
            globals_bg: &self.globals_bg,
            sdf_buffer: &self.sdf_buffer,
            image_buffer: &self.image_buffer,
            glyph_buffer: &self.glyph_buffer,
            path_vbuf: &self.path_vbuf,
            path_ibuf: &self.path_ibuf,
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SteelUI Direct Frame Encoder"),
            });

        {
            let mut pass = begin_ui_pass(
                &mut encoder,
                surface_view,
                wgpu::LoadOp::Clear(clear_color.to_wgpu()),
            );
            let mut active_scissor: Option<Rect> = None;
            let _ = record_standard_batches(
                &mut pass,
                pipes,
                gpu,
                &bufs,
                draw_list,
                0,
                &mut active_scissor,
            );
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        draw_list.atlas_overflowed
    }
}

