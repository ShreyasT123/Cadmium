use std::time::Duration;

use steelui::{
    AlignItems, App, Button, Clipboard, Color, Column, Container, CustomGpuPass, CustomGpuSurface,
    Dropdown, Edges, FileDialog, FontWeight, IntoElement, JustifyContent, Length, MacTheme,
    Material, MenuItem, PathBuilder, Row, Shadow, Signal, Slider, Spacer, SplitPane, Switch, Tabs,
    Text, TextArea, TextInput, TrafficLights, VirtualRow, VirtualizedList, WidgetWindow,
    WindowSpec, create_effect, create_signal,
};

const AURORA_WGSL: &str = r#"
struct Params {
    time: f32,
    speed: f32,
    radius_norm: f32,
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> params: Params;

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
    let t = params.time * params.speed;
    let uv = in.uv;
    let w1 = sin(uv.x * 5.0 + t * 1.1 + cos(uv.y * 3.5 - t * 0.6));
    let w2 = cos(uv.y * 4.2 - t * 0.9 + sin(uv.x * 6.0 + t * 0.5));
    let glow = clamp(0.5 + 0.35 * w1 + 0.25 * w2, 0.0, 1.0);

    let deep = vec3<f32>(0.03, 0.05, 0.14);
    let cyan = vec3<f32>(0.05, 0.52, 0.95);
    let violet = vec3<f32>(0.58, 0.22, 0.92);
    let col = mix(deep, mix(cyan, violet, uv.x * 0.8 + 0.2 * params.radius_norm), glow);
    return vec4<f32>(col, 1.0);
}
"#;

struct AuroraShaderPass {
    radius_signal: Signal<f32>,
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buf: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
    format: Option<wgpu::TextureFormat>,
}

impl AuroraShaderPass {
    fn new(radius_signal: Signal<f32>) -> Self {
        Self {
            radius_signal,
            pipeline: None,
            uniform_buf: None,
            bind_group: None,
            format: None,
        }
    }
}

impl CustomGpuPass for AuroraShaderPass {
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
                label: Some("Dogfood Aurora Shader"),
                source: wgpu::ShaderSource::Wgsl(AURORA_WGSL.into()),
            });

            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Aurora BGL"),
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
                label: Some("Aurora Params"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Aurora BG"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });

            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Aurora Pipeline Layout"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Aurora Pipeline"),
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

        let params: [f32; 4] = [
            time_secs,
            1.4,
            (self.radius_signal.get() / 28.0).clamp(0.0, 1.0),
            0.0,
        ];
        if let Some(buf) = &self.uniform_buf {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&params));
        }

        if let (Some(pipe), Some(bg)) = (&self.pipeline, &self.bind_group) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Aurora Custom RenderPass"),
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

fn sidebar_material(mode: usize, theme: &MacTheme) -> Material {
    match mode {
        1 => Material::frosted(Color::from_rgba8(245, 245, 247, 175)),
        2 => Material::liquid(Color::from_rgba8(245, 245, 247, 95)),
        _ => Material::Classic {
            fill: theme.panel_bg,
            fill_end: None,
            gradient_dir: [0.0, 1.0],
            border_color: theme.hairline,
            border_width: 1.0,
            shadow: Some(Shadow::new(theme.shadow_soft, 8.0, 2.0)),
        },
    }
}

fn card_material(mode: usize, theme: &MacTheme) -> Material {
    match mode {
        1 => Material::frosted(Color::from_rgba8(255, 255, 255, 170)),
        2 => Material::liquid(Color::from_rgba8(255, 255, 255, 58)),
        _ => Material::Classic {
            fill: theme.panel_bg,
            fill_end: None,
            gradient_dir: [0.0, 1.0],
            border_color: theme.hairline,
            border_width: 1.0,
            shadow: Some(Shadow::new(theme.shadow_soft, 12.0, 4.0)),
        },
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,wgpu=warn")
        .init();

    let app = App::new();
    let dispatcher = app.dispatcher();

    let selected_engine_mode = create_signal(0_usize);
    let engine_modes = ["Classic UI", "Frosted Glass", "Liquid Glass"];
    let search_text = create_signal("Magnum Engine".to_string());
    let active_tab = create_signal(0_usize);
    let radius = create_signal(14.0_f32);
    let animate_shader = create_signal(false);
    let async_jobs = create_signal(0_u32);
    let split_ratio = create_signal(0.24_f32);
    let selected_row = create_signal(0_usize);
    let status_msg =
        create_signal("Right-click cards for ContextMenu, or toggle Animate Shader".to_string());

    let theme = MacTheme::light();
    let sidebar_mat = create_signal(sidebar_material(selected_engine_mode.get(), &theme));
    let card_mat = create_signal(card_material(selected_engine_mode.get(), &theme));

    create_effect(move || {
        let mode = selected_engine_mode.get();
        sidebar_mat.set(sidebar_material(mode, &theme));
        card_mat.set(card_material(mode, &theme));
    });

    let window = WidgetWindow::new(move || {
        let search_field = TextInput::new(search_text)
            .placeholder("Search or filter...")
            .width(Length::Px(195.0))
            .tooltip("Type, click/drag to select, Ctrl+C / Ctrl+V")
            .material(Material::Classic {
                fill: Color::from_rgba8(255, 255, 255, 225),
                fill_end: None,
                gradient_dir: [0.0, 1.0],
                border_color: theme.hairline,
                border_width: 1.0,
                shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 20), 4.0, 1.0)),
            });

        let view_tabs =
            Tabs::new(active_tab, &["Workspace", "Inspector", "Feed"]).width(Length::Px(244.0));

        let mode_dropdown = Dropdown::new(selected_engine_mode, &engine_modes)
            .width(Length::Px(144.0))
            .tooltip("Classic skips offscreen textures for minimal RAM");

        let toolbar = Row::new()
            .height(Length::Px(52.0))
            .padding(Edges::xy(18.0, 0.0))
            .align(AlignItems::Center)
            .gap(12.0)
            .fill(theme.content_bg)
            .border(theme.hairline, 1.0)
            .drag_region()
            .child(TrafficLights::new())
            .child(
                Text::new("SteelUI Studio")
                    .size(14.5)
                    .weight(FontWeight::Semibold)
                    .color(theme.text_primary),
            )
            .child(Spacer::flex())
            .child(view_tabs)
            .child(search_field)
            .child(mode_dropdown)
            .child(
                Text::reactive(move || format!("Jobs: {}", async_jobs.get()))
                    .size(12.5)
                    .weight(FontWeight::Medium)
                    .color(theme.text_secondary),
            );

        let sidebar = Container::column()
            .flex_grow(1.0)
            .padding(Edges::all(16.0))
            .gap(10.0)
            .radius(14.0)
            .bind_material(sidebar_mat)
            .child(
                Text::new("SYSTEM CONTROLS")
                    .size(11.0)
                    .weight(FontWeight::Semibold)
                    .color(theme.text_tertiary),
            )
            .child(
                Button::new("Run Async Task")
                    .primary()
                    .tooltip("Runs a background thread and wakes UI via UiDispatcher")
                    .on_click({
                        let disp = dispatcher.clone();
                        move || {
                            status_msg.set("Async worker dispatched...".into());
                            let d = disp.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_millis(320));
                                d.dispatch(move || {
                                    async_jobs.update(|n| *n += 1);
                                    status_msg.set("Background task completed on UI thread".into());
                                });
                            });
                        }
                    }),
            )
            .child(
                Button::new("Copy Summary")
                    .tooltip("Copies active state to the OS clipboard")
                    .on_click(move || {
                        let text_val = search_text.get();
                        let summary = format!(
                            "Mode={} Query='{text_val}' tab={} radius={:.0} jobs={}",
                            engine_modes[selected_engine_mode.get()],
                            active_tab.get(),
                            radius.get(),
                            async_jobs.get()
                        );
                        match Clipboard::set_text(&summary) {
                            Ok(()) => status_msg.set("Copied summary to clipboard".into()),
                            Err(e) => status_msg.set(format!("Clipboard error: {e}")),
                        }
                    }),
            )
            .child(
                Button::new("Open Asset...")
                    .tooltip("Opens native OS file dialog")
                    .on_click(move || {
                        if let Some(path) = FileDialog::pick_file("Select Asset") {
                            status_msg.set(format!("File: {}", path.display()));
                        }
                    }),
            )
            .child(Spacer::flex())
            .child(
                Row::new()
                    .align(AlignItems::Center)
                    .child(
                        Text::new("Animate Shader")
                            .size(12.5)
                            .weight(FontWeight::Medium)
                            .color(theme.text_primary),
                    )
                    .child(Spacer::flex())
                    .child(
                        Switch::new(animate_shader).tooltip(
                            "Toggle continuous WGSL shader animation (0% idle GPU when off)",
                        ),
                    ),
            )
            .child(
                Text::reactive(move || format!("Corner Radius: {:.0} pt", radius.get()))
                    .size(12.0)
                    .weight(FontWeight::Medium)
                    .color(theme.text_secondary),
            )
            .child(
                Slider::new(radius, 0.0, 28.0)
                    .width(184.0)
                    .tooltip("Scrub to change card & shader corner radius"),
            );

        let preview_card = Container::column()
            .flex_grow(1.0)
            .height(Length::Px(118.0))
            .padding(Edges::all(18.0))
            .gap(6.0)
            .justify(JustifyContent::Center)
            .radius(14.0)
            .bind_radius(radius)
            .bind_material(card_mat)
            .tooltip("Right-click here to open ContextMenu")
            .context_menu([
                MenuItem::new("Classic UI (Direct)", move || selected_engine_mode.set(0)),
                MenuItem::new("Frosted Glass", move || selected_engine_mode.set(1)),
                MenuItem::new("Liquid Glass", move || selected_engine_mode.set(2)),
                MenuItem::separator(),
                MenuItem::new("Toggle Shader Animation", move || {
                    animate_shader.update(|v| *v = !*v);
                }),
                MenuItem::new("Reset Radius (14pt)", move || {
                    radius.set(14.0);
                    status_msg.set("Reset corner radius to 14pt".into());
                }),
            ])
            .child(
                Text::reactive(move || {
                    format!(
                        "Magnum Renderer  •  {}",
                        engine_modes[selected_engine_mode.get()]
                    )
                })
                .size(16.0)
                .weight(FontWeight::Bold)
                .color(theme.text_primary),
            )
            .child(
                Text::reactive(move || {
                    format!(
                        "Query: \"{}\"  •  Tab: {}  •  Radius: {:.0} pt",
                        search_text.get(),
                        active_tab.get(),
                        radius.get()
                    )
                })
                .size(13.0)
                .color(theme.text_secondary),
            );

        let shader_viewport = CustomGpuSurface::new(AuroraShaderPass::new(radius))
            .width(Length::Px(240.0))
            .height(Length::Px(118.0))
            .bind_radius(radius)
            .animate_when(animate_shader)
            .tooltip("CustomGpuSurface: embedded WGSL shader clipped to rounded corners")
            .context_menu([
                MenuItem::new("Toggle Animation", move || {
                    animate_shader.update(|v| *v = !*v);
                }),
                MenuItem::new("Reset Radius", move || radius.set(14.0)),
            ]);

        let top_cards = Row::new()
            .gap(12.0)
            .child(preview_card)
            .child(shader_viewport);

        let feed_list = VirtualizedList::new(100_000, 48.0, move |idx| VirtualRow {
            title: format!("Asset Node #{:06}", idx + 1),
            subtitle: if idx % 3 == 0 {
                "WGSL Compute Pipeline • Ready".into()
            } else {
                "SDF Retained Node • Cached".into()
            },
            badge: if idx % 2 == 0 {
                "Verified".into()
            } else {
                "Active".into()
            },
        })
        .gap(8.0)
        .selected(selected_row)
        .on_select(move |idx| {
            status_msg.set(format!("Selected Virtualized Row #{}", idx + 1));
        });

        let code_text = create_signal(
            "// SteelUI Shader Script\nfn main() {\n    let color = vec4(1.0);\n}".to_string(),
        );
        let code_editor = TextArea::new(code_text)
            .height(Length::Px(140.0))
            .placeholder("Shader script...");

        let main_area = Column::new()
            .flex_grow(1.0)
            .gap(12.0)
            .child(top_cards)
            .child(
                Text::new("SCROLLABLE COMPONENT FEED")
                    .size(11.0)
                    .weight(FontWeight::Semibold)
                    .color(theme.text_tertiary),
            )
            .child(feed_list)
            .child(code_editor)
            .child(
                Text::reactive(move || status_msg.get())
                    .size(12.5)
                    .color(theme.text_secondary),
            );

        let body = SplitPane::horizontal(split_ratio)
            .bounds(0.18, 0.45)
            .divider_color(theme.hairline)
            .first(Container::column().padding(Edges::all(12.0)).child(sidebar))
            .second(
                Container::column()
                    .padding(Edges::all(12.0))
                    .child(main_area),
            );

        Column::new()
            .fill(theme.window_bg)
            .child(toolbar)
            .child(body)
            .into_element()
    })
    .draw_paths(|ctx| {
        let width = ctx.width;
        let mut path = PathBuilder::new();
        path.move_to(0.0, 50.0);
        path.cubic_bezier_to(width * 0.2, 10.0, width * 0.45, 90.0, width * 0.7, 50.0);
        ctx.push_stroke_path(&path.build(), Color::from_rgba8(59, 130, 246, 255), 2.5);
    })
    .show_fps(true);

    let app = app.add_window(WindowSpec {
        title: "SteelUI Studio".into(),
        width: 1000.0,
        height: 600.0,
        borderless: true,
        clear_color: theme.window_bg,
        delegate: Box::new(window),
    });

    if let Err(err) = app.run() {
        eprintln!("SteelUI Studio exited with error: {err}");
    }
}
