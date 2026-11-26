/// A scrollable multi-column grid layout container.
///
/// Arranges children into rows of `columns` equal-width cells inside a
/// [`ScrollView`].
pub struct GridView {
    columns: usize,
    gap: f32,
    children: Vec<Element>,
}

impl GridView {
    /// Creates a grid view with the specified number of columns.
    pub fn new(columns: usize) -> Self {
        Self {
            columns: columns.max(1),
            gap: 12.0,
            children: Vec::new(),
        }
    }

    /// Sets the horizontal and vertical spacing between grid cells.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// Appends one child element to the grid.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    /// Appends multiple child elements to the grid.
    pub fn children<I, E>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: IntoElement,
    {
        self.children
            .extend(iter.into_iter().map(IntoElement::into_element));
        self
    }
}

impl IntoElement for GridView {
    fn into_element(self) -> Element {
        let mut scroll = ScrollView::new().gap(self.gap);
        let mut iter = self.children.into_iter().peekable();
        while iter.peek().is_some() {
            let mut row = Row::new().gap(self.gap);
            for _ in 0..self.columns {
                if let Some(child) = iter.next() {
                    row = row.child(Container::column().flex_grow(1.0).child(child));
                } else {
                    row = row.child(Spacer::flex());
                }
            }
            scroll = scroll.child(row);
        }
        scroll.into_element()
    }
}

const PROGRESS_RING_WGSL: &str = r#"
struct RingParams {
    time: f32,
    progress: f32, // < 0.0 means indeterminate spinner
    thickness: f32,
    _pad: f32,
    color: vec4<f32>,
    track_color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> params: RingParams;

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

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let p = (in.uv - vec2<f32>(0.5)) * 2.0;
    let r = length(p);
    let mid = 1.0 - params.thickness;
    let half_w = params.thickness * 0.5;
    let ring_dist = abs(r - mid) - half_w;
    let ring_alpha = 1.0 - smoothstep(-0.04, 0.04, ring_dist);
    if ring_alpha <= 0.001 {
        discard;
    }

    let angle = atan2(p.y, p.x); // [-pi, pi]
    var norm_a = fract((angle + 1.5707963) / 6.2831853);
    var arc_mask = 0.0;
    if params.progress < 0.0 {
        let head = fract(params.time * 1.15);
        let tail_len = 0.35 + 0.25 * sin(params.time * 3.0);
        let d = fract(head - norm_a);
        arc_mask = 1.0 - smoothstep(tail_len - 0.04, tail_len + 0.02, d);
    } else {
        arc_mask = 1.0 - smoothstep(params.progress - 0.02, params.progress + 0.01, norm_a);
    }

    let col = mix(params.track_color, params.color, arc_mask);
    return vec4<f32>(col.rgb, col.a * ring_alpha);
}
"#;

struct RingGpuPass {
    progress: Option<Signal<f32>>,
    color: Color,
    track_color: Color,
    thickness: f32,
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buf: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
    format: Option<wgpu::TextureFormat>,
}

impl CustomGpuPass for RingGpuPass {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        _width: u32,
        _height: u32,
        time_secs: f32,
    ) {
        if self.pipeline.is_none() || self.format != Some(format) {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Cadmium ProgressRing Shader"),
                source: wgpu::ShaderSource::Wgsl(PROGRESS_RING_WGSL.into()),
            });
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ProgressRing BGL"),
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
                label: Some("ProgressRing Params"),
                size: 48,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ProgressRing BG"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ProgressRing Layout"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ProgressRing Pipeline"),
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
            });
            self.pipeline = Some(pipeline);
            self.uniform_buf = Some(buf);
            self.bind_group = Some(bg);
            self.format = Some(format);
        }

        let prog = self
            .progress
            .and_then(|s| s.try_get())
            .map(|p| p.clamp(0.0, 1.0))
            .unwrap_or(-1.0);
        let c = self.color.to_array();
        let tc = self.track_color.to_array();
        let params: [f32; 12] = [
            time_secs,
            prog,
            self.thickness,
            0.0,
            c[0],
            c[1],
            c[2],
            c[3],
            tc[0],
            tc[1],
            tc[2],
            tc[3],
        ];
        if let Some(buf) = &self.uniform_buf {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&params));
        }
        if let (Some(pipe), Some(bg)) = (&self.pipeline, &self.bind_group) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ProgressRing Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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

/// GPU-native circular progress indicator or indeterminate loading spinner.
///
/// Renders an anti-aliased SDF arc inside a [`CustomGpuSurface`], animating at
/// full display refresh rate when indeterminate (or when `active` is true) and
/// using 0% GPU when settled or determinate.
pub struct CircularProgressIndicator {
    size: f32,
    thickness: f32,
    progress: Option<Signal<f32>>,
    active: Option<Signal<bool>>,
    color: Color,
    track_color: Color,
}

impl CircularProgressIndicator {
    /// Creates an indeterminate animated loading spinner of `size` logical pixels.
    pub fn spinner(size: f32, active: Signal<bool>) -> Self {
        Self {
            size: size.max(12.0),
            thickness: 0.22,
            progress: None,
            active: Some(active),
            color: Color::from_rgba8(10, 132, 255, 255),
            track_color: Color::from_rgba8(255, 255, 255, 28),
        }
    }

    /// Creates a determinate circular progress ring bound to a `0.0..=1.0` signal.
    pub fn determinate(size: f32, progress: Signal<f32>) -> Self {
        Self {
            size: size.max(12.0),
            thickness: 0.22,
            progress: Some(progress),
            active: None,
            color: Color::from_rgba8(10, 132, 255, 255),
            track_color: Color::from_rgba8(255, 255, 255, 28),
        }
    }

    /// Sets the active arc color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Sets the background ring track color.
    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = color;
        self
    }
}

impl IntoElement for CircularProgressIndicator {
    fn into_element(self) -> Element {
        let mut surf = CustomGpuSurface::new(RingGpuPass {
            progress: self.progress,
            color: self.color,
            track_color: self.track_color,
            thickness: self.thickness,
            pipeline: None,
            uniform_buf: None,
            bind_group: None,
            format: None,
        })
        .width(Length::Px(self.size))
        .height(Length::Px(self.size))
        .radius(0.0);

        if let Some(anim) = self.active {
            surf = surf.animate_when(anim);
        }
        surf.into_element()
    }
}
