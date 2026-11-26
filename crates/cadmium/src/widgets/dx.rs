impl IntoElement for &str {
    fn into_element(self) -> Element {
        Text::new(self).tone(TextTone::Primary).into_element()
    }
}

impl IntoElement for String {
    fn into_element(self) -> Element {
        Text::new(self).tone(TextTone::Primary).into_element()
    }
}

impl Container {
    /// Sets a fixed pixel width (`Length::Px(px)`).
    pub fn w(self, px: f32) -> Self {
        self.width(Length::Px(px))
    }

    /// Sets a fixed pixel height (`Length::Px(px)`).
    pub fn h(self, px: f32) -> Self {
        self.height(Length::Px(px))
    }

    /// Expands width to 100% of parent (`Length::Percent(1.0)`).
    pub fn w_full(self) -> Self {
        self.width(Length::Percent(1.0))
    }

    /// Expands height to 100% of parent (`Length::Percent(1.0)`).
    pub fn h_full(self) -> Self {
        self.height(Length::Percent(1.0))
    }

    /// Sets uniform padding on all sides (`Edges::all(px)`).
    pub fn p(self, px: f32) -> Self {
        self.padding(Edges::all(px))
    }

    /// Sets horizontal padding (`Edges::xy(px, 0.0)`).
    pub fn px(self, px: f32) -> Self {
        self.padding(Edges::xy(px, 0.0))
    }

    /// Sets vertical padding (`Edges::xy(0.0, px)`).
    pub fn py(self, px: f32) -> Self {
        self.padding(Edges::xy(0.0, px))
    }

    /// Reactively switches background fill based on a condition closure.
    ///
    /// Evaluates at `0.00 ms` layout cost (paint-only invalidation via [`crate::create_memo`]).
    pub fn fill_when(
        self,
        mut cond: impl FnMut() -> bool + 'static,
        active: Color,
        inactive: Color,
    ) -> Self {
        let memo = crate::reactive::create_memo(move || {
            Material::solid(if cond() { active } else { inactive })
        });
        self.bind_material(memo.signal())
    }
}

const SHADER_CANVAS_VS: &str = r#"
struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOut {
    let x = f32(i32(idx) / 2) * 4.0 - 1.0;
    let y = f32(i32(idx) % 2) * 4.0 - 1.0;
    var out: VertexOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}
"#;

struct SimpleShaderPass {
    wgsl_frag: String,
    uniforms_fn: Box<dyn FnMut(f32, u32, u32) -> [f32; 4]>,
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buf: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
    format: Option<wgpu::TextureFormat>,
}

impl CustomGpuPass for SimpleShaderPass {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        time_secs: f32,
    ) {
        if self.pipeline.is_none() || self.format != Some(format) {
            let full_wgsl = format!("{SHADER_CANVAS_VS}\n{}", self.wgsl_frag);
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Cadmium ShaderCanvas"),
                source: wgpu::ShaderSource::Wgsl(full_wgsl.into()),
            });
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ShaderCanvas BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ShaderCanvas Uniforms"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ShaderCanvas BG"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ShaderCanvas Layout"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            self.pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ShaderCanvas Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            }));
            self.uniform_buf = Some(buf);
            self.bind_group = Some(bg);
            self.format = Some(format);
        }

        let vals = (self.uniforms_fn)(time_secs, width, height);
        if let Some(buf) = &self.uniform_buf {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&vals));
        }
        if let (Some(pipe), Some(bg)) = (&self.pipeline, &self.bind_group) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ShaderCanvas Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// High-level WGSL fragment shader widget hiding all raw `wgpu` boilerplate.
///
/// The supplied WGSL snippet receives `@group(0) @binding(0) var<uniform> u: vec4<f32>;`
/// and `VertexOut` (`@location(0) uv: vec2<f32>`), and only needs to define `fs_main`.
pub struct ShaderCanvas {
    inner: CustomGpuSurface,
}

impl ShaderCanvas {
    /// Creates an embedded shader viewport from a WGSL fragment body and per-frame `[f32; 4]` uniform supplier.
    pub fn new(
        wgsl_fragment: impl Into<String>,
        uniforms: impl FnMut(f32, u32, u32) -> [f32; 4] + 'static,
    ) -> Self {
        Self {
            inner: CustomGpuSurface::new(SimpleShaderPass {
                wgsl_frag: wgsl_fragment.into(),
                uniforms_fn: Box::new(uniforms),
                pipeline: None,
                uniform_buf: None,
                bind_group: None,
                format: None,
            }),
        }
    }

    /// Sets the canvas width.
    pub fn width(mut self, w: Length) -> Self {
        self.inner = self.inner.width(w);
        self
    }

    /// Sets the canvas height.
    pub fn height(mut self, h: Length) -> Self {
        self.inner = self.inner.height(h);
        self
    }

    /// Sets flex grow factor.
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.inner = self.inner.flex_grow(g);
        self
    }

    /// Sets corner radius.
    pub fn radius(mut self, r: f32) -> Self {
        self.inner = self.inner.radius(r);
        self
    }

    /// Drives continuous animation while `sig` is `true` (0% GPU when `false`).
    pub fn animate_when(mut self, sig: Signal<bool>) -> Self {
        self.inner = self.inner.animate_when(sig);
        self
    }
}

impl IntoElement for ShaderCanvas {
    fn into_element(self) -> Element {
        self.inner.into_element()
    }
}
