struct BatchBuffers<'a> {
    width: u32,
    height: u32,
    globals_bg: &'a wgpu::BindGroup,
    sdf_buffer: &'a wgpu::Buffer,
    image_buffer: &'a wgpu::Buffer,
    glyph_buffer: &'a wgpu::Buffer,
    path_vbuf: &'a wgpu::Buffer,
    path_ibuf: &'a wgpu::Buffer,
}

fn record_standard_batches<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    pipes: &'a FormatPipelines,
    gpu: &'a GpuContext,
    bufs: &BatchBuffers<'a>,
    draw_list: &DrawList,
    mut i: usize,
    active_scissor: &mut Option<Rect>,
) -> usize {
    let mut scissor_visible = apply_scissor(
        pass,
        *active_scissor,
        draw_list.scale,
        bufs.width,
        bufs.height,
    );
    while i < draw_list.batches.len() {
        match &draw_list.batches[i] {
            DrawBatch::SetScissor(sc) => {
                *active_scissor = *sc;
                scissor_visible = apply_scissor(
                    pass,
                    *active_scissor,
                    draw_list.scale,
                    bufs.width,
                    bufs.height,
                );
                i += 1;
            }
            DrawBatch::Sdf(range) => {
                if scissor_visible {
                    pass.set_pipeline(&pipes.sdf);
                    pass.set_bind_group(0, bufs.globals_bg, &[]);
                    pass.set_vertex_buffer(0, bufs.sdf_buffer.slice(..));
                    pass.draw(0..4, range.clone());
                }
                i += 1;
            }
            DrawBatch::Image { tex, range } => {
                if scissor_visible
                    && let Some(img_pipe) = &pipes.image
                    && let Some(uploaded) = gpu.textures.get(*tex)
                {
                    pass.set_pipeline(img_pipe);
                    pass.set_bind_group(0, bufs.globals_bg, &[]);
                    pass.set_bind_group(1, &uploaded.bind_group, &[]);
                    pass.set_vertex_buffer(0, bufs.image_buffer.slice(..));
                    pass.draw(0..4, range.clone());
                }
                i += 1;
            }
            DrawBatch::Text(range) => {
                if scissor_visible {
                    pass.set_pipeline(&pipes.text);
                    pass.set_bind_group(0, bufs.globals_bg, &[]);
                    pass.set_bind_group(1, &gpu.glyph_atlas.bind_group, &[]);
                    pass.set_vertex_buffer(0, bufs.glyph_buffer.slice(..));
                    pass.draw(0..4, range.clone());
                }
                i += 1;
            }
            DrawBatch::Path {
                index_range,
                vertex_offset,
            } => {
                if scissor_visible {
                    pass.set_pipeline(&pipes.path);
                    pass.set_bind_group(0, bufs.globals_bg, &[]);
                    pass.set_vertex_buffer(0, bufs.path_vbuf.slice(..));
                    pass.set_index_buffer(bufs.path_ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(index_range.clone(), *vertex_offset, 0..1);
                }
                i += 1;
            }
            DrawBatch::Glass(_) => break,
        }
    }
    i
}

fn apply_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    scissor: Option<Rect>,
    scale: f32,
    target_width: u32,
    target_height: u32,
) -> bool {
    match scissor {
        Some(r) => {
            let tw = target_width as f32;
            let th = target_height as f32;
            let x0 = (r.x * scale).round().clamp(0.0, tw) as u32;
            let y0 = (r.y * scale).round().clamp(0.0, th) as u32;
            let x1 = ((r.x + r.width) * scale).round().clamp(0.0, tw) as u32;
            let y1 = ((r.y + r.height) * scale).round().clamp(0.0, th) as u32;
            if x1 <= x0 || y1 <= y0 {
                return false;
            }
            pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
            true
        }
        None => {
            pass.set_scissor_rect(0, 0, target_width.max(1), target_height.max(1));
            true
        }
    }
}

fn blur_pyramid_uniforms(full: [u32; 2], mip1: [u32; 2], mip2: [u32; 2]) -> [BlurUniform; 3] {
    [
        BlurUniform {
            texel_size: [1.0 / full[0] as f32, 1.0 / full[1] as f32],
            offset: 0.60,
            _pad: 0.0,
        },
        BlurUniform {
            texel_size: [1.0 / mip1[0] as f32, 1.0 / mip1[1] as f32],
            offset: 0.50,
            _pad: 0.0,
        },
        BlurUniform {
            texel_size: [1.0 / mip2[0] as f32, 1.0 / mip2[1] as f32],
            offset: 0.35,
            _pad: 0.0,
        },
    ]
}

fn begin_ui_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SteelUI UI Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    })
}

fn run_blur_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    src_bg: &wgpu::BindGroup,
    params_bg: &wgpu::BindGroup,
    dst_view: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SteelUI Kawase Blur Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: dst_view,
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
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, src_bg, &[]);
    pass.set_bind_group(1, params_bg, &[]);
    pass.draw(0..3, 0..1);
}

fn create_vertex_buffer<T>(device: &wgpu::Device, capacity: usize, tag: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(tag),
        size: (capacity * std::mem::size_of::<T>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_canonical_target(
    gpu: &GpuContext,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::BindGroup,
    wgpu::BindGroup,
) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SteelUI Canonical Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let blit_bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SteelUI Blit BG"),
        layout: &gpu.tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.nearest_sampler),
            },
        ],
    });

    let linear_bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SteelUI Canonical Linear BG"),
        layout: &gpu.tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.linear_sampler),
            },
        ],
    });

    (texture, view, blit_bg, linear_bg)
}

fn create_blur_target(
    gpu: &GpuContext,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &gpu.tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.linear_sampler),
            },
        ],
    });

    (texture, view, bg)
}

fn create_backdrop_target(
    gpu: &GpuContext,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SteelUI Sharp Backdrop Copy"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SteelUI Sharp Backdrop BG"),
        layout: &gpu.tex_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.linear_sampler),
            },
        ],
    });

    (texture, bg)
}

fn create_path_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Path Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl)],
        immediate_size: 0,
    });

    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<PathVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Path Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_index_buffer(device: &wgpu::Device, capacity: usize, tag: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(tag),
        size: (capacity * std::mem::size_of::<u32>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_sdf_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI SDF Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawSdfInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 64,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 80,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 96,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 112,
                shader_location: 7,
                format: wgpu::VertexFormat::Float32x3,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI SDF Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_image_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    tex_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Image Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(tex_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawImageInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Image Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_text_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    atlas_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Text Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(atlas_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawGlyphInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Text Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_blur_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    tex_bgl: &wgpu::BindGroupLayout,
    blur_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Blur Pipeline Layout"),
        bind_group_layouts: &[Some(tex_bgl), Some(blur_bgl)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Blur Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_glass_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    globals_bgl: &wgpu::BindGroupLayout,
    tex_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Glass Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bgl), Some(tex_bgl), Some(tex_bgl)],
        immediate_size: 0,
    });

    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RawGlassInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 64,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 68,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 72,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 76,
                shader_location: 7,
                format: wgpu::VertexFormat::Float32,
            },
            wgpu::VertexAttribute {
                offset: 80,
                shader_location: 8,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Glass Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_blit_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    tex_bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SteelUI Blit Pipeline Layout"),
        bind_group_layouts: &[Some(tex_bgl)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SteelUI Blit Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_tex_resources(
    device: &wgpu::Device,
) -> (wgpu::BindGroupLayout, wgpu::Sampler, wgpu::Sampler) {
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("SteelUI Texture BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("SteelUI Nearest Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let linear = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("SteelUI Linear Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });

    (bgl, nearest, linear)
}

fn phys_rects_intersect(a: Rect, b: [f32; 4]) -> bool {
    a.x < b[0] + b[2] && a.x + a.width > b[0] && a.y < b[1] + b[3] && a.y + a.height > b[1]
}

fn union_opt_rect(cur: Option<Rect>, add: Rect) -> Option<Rect> {
    if add.width <= 0.0 || add.height <= 0.0 {
        return cur;
    }
    Some(match cur {
        Some(c) => {
            let x0 = c.x.min(add.x);
            let y0 = c.y.min(add.y);
            let x1 = (c.x + c.width).max(add.x + add.width);
            let y1 = (c.y + c.height).max(add.y + add.height);
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        }
        None => add,
    })
}

fn batch_phys_bounds(draw_list: &DrawList, batch: &DrawBatch, w: u32, h: u32) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    match batch {
        DrawBatch::Sdf(range) => {
            for idx in range.clone() {
                let inst = &draw_list.sdf_instances[idx as usize];
                let pad = if inst.shadow_color[3] > 0.001 {
                    inst.shadow_params[2] * 1.7
                        + inst.shadow_params[0].abs().max(inst.shadow_params[1].abs())
                        + 2.0
                } else {
                    1.0
                };
                acc = union_opt_rect(
                    acc,
                    Rect::new(
                        inst.rect[0] - pad,
                        inst.rect[1] - pad,
                        inst.rect[2] + pad * 2.0,
                        inst.rect[3] + pad * 2.0,
                    ),
                );
            }
        }
        DrawBatch::Image { range, .. } => {
            for idx in range.clone() {
                let r = draw_list.image_instances[idx as usize].rect;
                acc = union_opt_rect(acc, Rect::new(r[0], r[1], r[2], r[3]));
            }
        }
        DrawBatch::Text(range) => {
            for idx in range.clone() {
                let r = draw_list.glyph_instances[idx as usize].rect;
                acc = union_opt_rect(acc, Rect::new(r[0], r[1], r[2], r[3]));
            }
        }
        DrawBatch::Path { .. } => {
            acc = Some(Rect::new(0.0, 0.0, w as f32, h as f32));
        }
        DrawBatch::Glass(_) | DrawBatch::SetScissor(_) => {}
    }
    acc
}

fn scissor_intersects_rect(
    scissor: Option<Rect>,
    phys_rect: [f32; 4],
    scale: f32,
    target_width: u32,
    target_height: u32,
) -> bool {
    let tw = target_width as f32;
    let th = target_height as f32;
    let (sx0, sy0, sx1, sy1) = match scissor {
        Some(r) => (
            (r.x * scale).round().clamp(0.0, tw),
            (r.y * scale).round().clamp(0.0, th),
            ((r.x + r.width) * scale).round().clamp(0.0, tw),
            ((r.y + r.height) * scale).round().clamp(0.0, th),
        ),
        None => (0.0, 0.0, tw, th),
    };
    let rx0 = phys_rect[0].max(sx0);
    let ry0 = phys_rect[1].max(sy0);
    let rx1 = (phys_rect[0] + phys_rect[2]).min(sx1);
    let ry1 = (phys_rect[1] + phys_rect[3]).min(sy1);
    rx1 > rx0 && ry1 > ry0
}

