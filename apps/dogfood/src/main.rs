use std::sync::Arc;

use cadmium::{
    AlignItems, App, Button, Color, Column, Container, CustomGpuPass, CustomGpuSurface,
    DecodedImage, Edges, FontWeight, Icon, Image, Inset, IntoElement, JustifyContent, Length,
    Material, ProgressBar, Row, ScrollView, Shadow, Slider, Spacer, Stack, Text, TextInput,
    ThemeMode, TrafficLights, WidgetWindow, WindowSpec, create_signal,
};

// ── Inline SVG Icons (FontAwesome-equivalent vector paths) ─────────────────────────
const ICO_CHEVRON_L: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12z"/></svg>"##;
const ICO_CHEVRON_R: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M8.59 16.59 10 18l6-6-6-6-1.41 1.41L13.17 12z"/></svg>"##;
const ICO_HOME: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/></svg>"##;
const ICO_BELL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M12 22c1.1 0 2-.9 2-2h-4a2 2 0 0 0 2 2zm6-6v-5c0-3.07-1.64-5.64-4.5-6.32V4c0-.83-.67-1.5-1.5-1.5s-1.5.67-1.5 1.5v.68C7.63 5.36 6 7.92 6 11v5l-2 2v1h16v-1l-2-2z"/></svg>"##;
const ICO_LIBRARY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6zm16-4H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-8 12.5v-9l6 4.5-6 4.5z"/></svg>"##;
const ICO_PLUS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/></svg>"##;
const ICO_PLAY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M8 5.14v14l11-7-11-7z"/></svg>"##;
const ICO_PAUSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg>"##;
const ICO_PREV: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M6 6h2v12H6zm3.5 6 8.5 6V6z"/></svg>"##;
const ICO_NEXT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z"/></svg>"##;
const ICO_SHUFFLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M10.59 9.17 5.41 4 4 5.41l5.17 5.17 1.42-1.41zM14.5 4l2.04 2.04L4 18.59 5.41 20 17.96 7.46 20 9.5V4h-5.5zm.33 9.41-1.41 1.41 3.13 3.13L14.5 20H20v-5.5l-2.04 2.04-3.13-3.13z"/></svg>"##;
const ICO_REPEAT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z"/></svg>"##;
const ICO_CHECK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/></svg>"##;
const ICO_VERIFIED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#34d399" d="m23 12-2.44-2.79.34-3.69-3.61-.82-1.89-3.2L12 2.96 8.6 1.5 6.71 4.69 3.1 5.5l.34 3.7L1 12l2.44 2.79-.34 3.7 3.61.82L8.6 22.5l3.4-1.47 3.4 1.46 1.89-3.19 3.61-.82-.34-3.69L23 12zm-12.91 4.72-3.8-3.81 1.48-1.48 2.32 2.33 5.85-5.87 1.48 1.48-7.33 7.35z"/></svg>"##;
const ICO_VOLUME: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02z"/></svg>"##;
const ICO_MORE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M6 10c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm12 0c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm-6 0c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z"/></svg>"##;

// ── Ambient Crimson/Ember Radial Glow Shader (`.bg-ambient-glow` in WGSL) ──────────
const AMBIENT_BG_WGSL: &str = r#"
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

fn spot(uv: vec2<f32>, center: vec2<f32>, radius: f32) -> f32 {
    let d = length((uv - center) * vec2<f32>(1.45, 1.0));
    return pow(clamp(1.0 - d / radius, 0.0, 1.0), 2.1);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    var col = vec3<f32>(0.043, 0.027, 0.035); // #0b0709
    col += vec3<f32>(0.48, 0.14, 0.06) * spot(uv, vec2<f32>(0.18, 0.06), 0.58) * 0.52;
    col += vec3<f32>(0.55, 0.12, 0.16) * spot(uv, vec2<f32>(0.50, 0.04), 0.50) * 0.42;
    col += vec3<f32>(0.62, 0.10, 0.28) * spot(uv, vec2<f32>(0.85, 0.10), 0.55) * 0.48;
    col += vec3<f32>(0.28, 0.07, 0.14) * spot(uv, vec2<f32>(0.50, 0.65), 0.55) * 0.42;
    col += vec3<f32>(0.36, 0.08, 0.16) * spot(uv, vec2<f32>(0.20, 0.85), 0.48) * 0.40;
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
"#;

struct AmbientGlowPass {
    pipeline: Option<wgpu::RenderPipeline>,
    format: Option<wgpu::TextureFormat>,
}

impl CustomGpuPass for AmbientGlowPass {
    fn render(
        &mut self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        _width: u32,
        _height: u32,
        _time_secs: f32,
    ) {
        if self.pipeline.is_none() || self.format != Some(format) {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Ambient Glow Shader"),
                source: wgpu::ShaderSource::Wgsl(AMBIENT_BG_WGSL.into()),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Ambient Layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
            self.pipeline = Some(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Ambient Pipeline"),
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
                }),
            );
            self.format = Some(format);
        }

        if let Some(pipe) = &self.pipeline {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Ambient Glow Pass"),
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
            pass.draw(0..3, 0..1);
        }
    }
}

fn gen_cover(seed: usize, a: [f32; 3], b: [f32; 3]) -> Arc<DecodedImage> {
    let (w, h) = (192u32, 192u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let p = seed as f32 * 1.1;
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let wave = ((u * 6.0 + v * 4.0 + p).sin() * 0.5 + 0.5) * 0.45
                + ((u * 10.0 - v * 7.0).cos() * 0.5 + 0.5) * 0.25;
            let t = (u * 0.5 + v * 0.5 + wave * 0.4).clamp(0.0, 1.0);
            rgba.push(((a[0] * (1.0 - t) + b[0] * t) * 255.0) as u8);
            rgba.push(((a[1] * (1.0 - t) + b[1] * t) * 255.0) as u8);
            rgba.push(((a[2] * (1.0 - t) + b[2] * t) * 255.0) as u8);
            rgba.push(255);
        }
    }
    Arc::new(DecodedImage {
        width: w,
        height: h,
        rgba,
    })
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,wgpu=warn")
        .init();

    let pink = Color::from_rgba8(219, 39, 119, 255);
    let pink_soft = Color::from_rgba8(236, 72, 153, 255);
    let white = Color::from_rgba8(245, 245, 247, 255);
    let muted = Color::from_rgba8(163, 163, 170, 255);
    let subtle_border = Color::from_rgba8(255, 255, 255, 22);

    let glass_panel_mat = Material::Classic {
        fill: Color::from_rgba8(24, 20, 22, 240),
        fill_end: None,
        gradient_dir: [0.0, 0.0],
        border_color: subtle_border,
        border_width: 1.0,
        shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 145), 32.0, 14.0)),
    };
    let liquid_dock_mat = Material::Classic {
        fill: Color::from_rgba8(20, 16, 18, 245),
        fill_end: None,
        gradient_dir: [0.0, 0.0],
        border_color: Color::from_rgba8(255, 255, 255, 36),
        border_width: 1.0,
        shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 190), 36.0, 14.0)),
    };

    let search = create_signal(String::new());
    let is_playing = create_signal(true);
    let progress = create_signal(0.35_f32);
    let volume = create_signal(0.70_f32);
    let selected_lib = create_signal(8_usize); // Max Richter selected
    let following = create_signal(true);

    let hero_art = gen_cover(9, [0.45, 0.14, 0.18], [0.08, 0.06, 0.08]);
    let rainbow_art = gen_cover(10, [0.78, 0.28, 0.14], [0.16, 0.14, 0.32]);
    let band_art = gen_cover(11, [0.22, 0.22, 0.24], [0.07, 0.07, 0.08]);

    let lib_items = vec![
        (
            "Fro 💜",
            "Playlist • Fraustiz",
            gen_cover(1, [0.35, 0.25, 0.72], [0.12, 0.10, 0.28]),
        ),
        (
            "Frock",
            "Playlist • Fraustiz",
            gen_cover(2, [0.15, 0.38, 0.75], [0.08, 0.14, 0.30]),
        ),
        (
            "Liked Songs",
            "Playlist • Fraustiz",
            gen_cover(3, [0.48, 0.18, 0.78], [0.85, 0.22, 0.52]),
        ),
        (
            "actually good jojo refs",
            "Playlist • kyliekeona",
            gen_cover(4, [0.75, 0.24, 0.35], [0.20, 0.12, 0.25]),
        ),
        (
            "Akira Senju",
            "Artist",
            gen_cover(5, [0.38, 0.30, 0.26], [0.14, 0.11, 0.12]),
        ),
        (
            "MiM",
            "Artist",
            gen_cover(6, [0.20, 0.42, 0.48], [0.10, 0.14, 0.18]),
        ),
        (
            "Signalis chill ost...",
            "Playlist • Wndrfl7",
            gen_cover(7, [0.68, 0.12, 0.16], [0.10, 0.06, 0.08]),
        ),
        (
            "aupinard",
            "Artist",
            gen_cover(8, [0.55, 0.38, 0.22], [0.18, 0.12, 0.10]),
        ),
        ("Max Richter", "Artist", hero_art.clone()),
    ];

    let popular_tracks = vec![
        (
            "1",
            "On the Nature of Daylight",
            true,
            "191,479,806",
            "6:11",
            gen_cover(21, [0.25, 0.32, 0.45], [0.08, 0.10, 0.15]),
        ),
        (
            "2",
            "Spring 1 - 2012",
            false,
            "184,182,057",
            "2:31",
            gen_cover(22, [0.55, 0.42, 0.25], [0.18, 0.14, 0.10]),
        ),
        (
            "3",
            "On the Nature of Daylight - Orchestral Ver...",
            false,
            "28,911,503",
            "6:36",
            gen_cover(23, [0.38, 0.22, 0.35], [0.12, 0.08, 0.12]),
        ),
        (
            "4",
            "Written on the Sky",
            false,
            "98,396,781",
            "1:40",
            gen_cover(24, [0.22, 0.28, 0.34], [0.09, 0.11, 0.14]),
        ),
    ];

    let app = App::new().add_window(WindowSpec {
        title: "Music Desktop Player".into(),
        width: 1360.0,
        height: 860.0,
        borderless: true,
        clear_color: Color::from_rgba8(11, 7, 9, 255),
        delegate: Box::new(
            WidgetWindow::new(move || {
                // ── 1. Top Navigation Bar ──────────────────────────────────────────
                let top_bar = Row::new()
                    .height(Length::Px(56.0))
                    .padding(Edges::xy(20.0, 0.0))
                    .align(AlignItems::Center)
                    .gap(12.0)
                    .drag_region()
                    .child(TrafficLights::new())
                    .child(Button::icon_only(ICO_CHEVRON_L, 14.0))
                    .child(Button::icon_only(ICO_CHEVRON_R, 14.0))
                    .child(Spacer::flex())
                    .child(Button::icon_only(ICO_HOME, 15.0))
                    .child(
                        TextInput::new(search)
                            .placeholder("What do you want to play?")
                            .width(Length::Px(380.0))
                            .material(Material::Classic {
                                fill: Color::from_rgba8(255, 255, 255, 18),
                                fill_end: None,
                                gradient_dir: [0.0, 0.0],
                                border_color: subtle_border,
                                border_width: 1.0,
                                shadow: None,
                            }),
                    )
                    .child(Spacer::flex())
                    .child(Button::icon_only(ICO_BELL, 15.0).ghost())
                    .child(
                        Image::from_decoded(hero_art.clone())
                            .width(Length::Px(30.0))
                            .height(Length::Px(30.0))
                            .radius(15.0),
                    );

                // ── 2. Left Sidebar: "Your Library" ────────────────────────────────
                let mut lib_list = ScrollView::new().gap(4.0);
                for (idx, (name, sub, art)) in lib_items.iter().enumerate() {
                    let is_sel = idx == selected_lib.get();
                    let row = Container::row()
                        .padding(Edges::all(6.0))
                        .gap(10.0)
                        .radius(12.0)
                        .align(AlignItems::Center)
                        .fill(if is_sel {
                            Color::from_rgba8(255, 255, 255, 26)
                        } else {
                            Color::TRANSPARENT
                        })
                        .hover_fill(Color::from_rgba8(255, 255, 255, 14))
                        .on_click(move || selected_lib.set(idx))
                        .child(
                            Image::from_decoded(art.clone())
                                .width(Length::Px(40.0))
                                .height(Length::Px(40.0))
                                .radius(if sub == &"Artist" { 20.0 } else { 8.0 }),
                        )
                        .child(
                            Column::new()
                                .flex_grow(1.0)
                                .gap(2.0)
                                .child(
                                    Text::new(*name)
                                        .size(12.5)
                                        .weight(FontWeight::Semibold)
                                        .color(white),
                                )
                                .child(Text::new(*sub).size(11.0).color(muted)),
                        );
                    lib_list = lib_list.child(row);
                }

                let left_sidebar = Container::column()
                    .width(Length::Px(284.0))
                    .padding(Edges::all(16.0))
                    .gap(12.0)
                    .radius(18.0)
                    .material(glass_panel_mat)
                    .child(
                        Row::new()
                            .align(AlignItems::Center)
                            .gap(8.0)
                            .child(Icon::svg(ICO_LIBRARY, 15.0).color(muted))
                            .child(
                                Text::new("Your Library")
                                    .size(15.0)
                                    .weight(FontWeight::Bold)
                                    .color(white),
                            )
                            .child(Spacer::flex())
                            .child(Button::icon_only(ICO_PLUS, 13.0).ghost()),
                    )
                    .child(
                        Row::new()
                            .gap(8.0)
                            .child(Button::new("Playlists").pill())
                            .child(Button::new("Albums").ghost().pill())
                            .child(Button::new("Artists").ghost().pill()),
                    )
                    .child(lib_list);

                // ── 3. Center Main Content: Artist Hero + Popular Tracks ───────────
                let hero_banner = Stack::new()
                    .height(Length::Px(252.0))
                    .child(
                        Image::from_decoded(hero_art.clone())
                            .width(Length::Percent(1.0))
                            .height(Length::Px(252.0))
                            .radius(18.0),
                    )
                    .child(
                        Container::new()
                            .absolute()
                            .inset(Inset::all(Length::Px(0.0)))
                            .gradient(
                                Color::from_rgba8(22, 18, 21, 15),
                                Color::from_rgba8(22, 18, 21, 240),
                            ),
                    )
                    .child(
                        Container::column()
                            .absolute()
                            .inset(Inset {
                                left: Length::Px(28.0),
                                right: Length::Auto,
                                top: Length::Auto,
                                bottom: Length::Px(20.0),
                            })
                            .gap(4.0)
                            .child(
                                Row::new()
                                    .align(AlignItems::Center)
                                    .gap(6.0)
                                    .child(Icon::svg(ICO_VERIFIED, 15.0))
                                    .child(
                                        Text::new("Verified by Spotify")
                                            .size(12.0)
                                            .weight(FontWeight::Medium)
                                            .color(white),
                                    ),
                            )
                            .child(
                                Text::new("Max Richter")
                                    .size(38.0)
                                    .weight(FontWeight::Bold)
                                    .color(white),
                            )
                            .child(
                                Text::new("4,692,971 monthly listeners")
                                    .size(12.5)
                                    .color(muted),
                            ),
                    );

                let controls_strip = Row::new()
                    .padding(Edges::xy(28.0, 12.0))
                    .align(AlignItems::Center)
                    .gap(14.0)
                    .child(
                        Button::icon_only(ICO_PLAY, 18.0)
                            .primary()
                            .size(Length::Px(48.0), Length::Px(48.0))
                            .on_click(move || is_playing.update(|p| *p = !*p)),
                    )
                    .child(
                        Image::from_decoded(rainbow_art.clone())
                            .width(Length::Px(36.0))
                            .height(Length::Px(36.0))
                            .radius(6.0),
                    )
                    .child(Button::icon_only(ICO_SHUFFLE, 16.0).ghost())
                    .child(
                        Button::new(if following.get() {
                            "Following"
                        } else {
                            "Follow"
                        })
                        .pill()
                        .on_click(move || following.update(|f| *f = !*f)),
                    )
                    .child(Button::icon_only(ICO_MORE, 16.0).ghost());

                let mut track_rows = ScrollView::new().gap(4.0);
                for (num, title, has_mv, plays, dur, cov) in &popular_tracks {
                    track_rows = track_rows.child(
                        Container::row()
                            .padding(Edges::xy(12.0, 8.0))
                            .align(AlignItems::Center)
                            .gap(14.0)
                            .radius(10.0)
                            .hover_fill(Color::from_rgba8(255, 255, 255, 16))
                            .on_click(move || is_playing.set(true))
                            .child(
                                Text::new(*num)
                                    .size(12.5)
                                    .weight(FontWeight::Semibold)
                                    .color(muted),
                            )
                            .child(
                                Image::from_decoded(cov.clone())
                                    .width(Length::Px(36.0))
                                    .height(Length::Px(36.0))
                                    .radius(6.0),
                            )
                            .child(
                                Column::new()
                                    .flex_grow(1.0)
                                    .gap(2.0)
                                    .child(
                                        Text::new(*title)
                                            .size(13.0)
                                            .weight(FontWeight::Semibold)
                                            .color(white),
                                    )
                                    .child(
                                        Text::new(if *has_mv { "Music video" } else { "" })
                                            .size(10.5)
                                            .color(muted),
                                    ),
                            )
                            .child(Text::new(*plays).size(11.5).color(muted))
                            .child(Text::new(*dur).size(11.5).color(muted)),
                    );
                }

                let center_main = Container::column()
                    .flex_grow(1.0)
                    .radius(18.0)
                    .material(glass_panel_mat)
                    .child(hero_banner)
                    .child(controls_strip)
                    .child(
                        Column::new()
                            .flex_grow(1.0)
                            .padding(Edges::xy(28.0, 4.0))
                            .gap(10.0)
                            .child(
                                Text::new("Popular")
                                    .size(18.0)
                                    .weight(FontWeight::Bold)
                                    .color(white),
                            )
                            .child(track_rows),
                    );

                // ── 4. Right Sidebar: Now Playing + About Artist ───────────────────
                let right_sidebar = Column::new()
                    .width(Length::Px(284.0))
                    .gap(12.0)
                    .child(
                        Container::column()
                            .padding(Edges::all(12.0))
                            .gap(10.0)
                            .radius(18.0)
                            .material(glass_panel_mat)
                            .child(
                                Image::from_decoded(rainbow_art.clone())
                                    .height(Length::Px(256.0))
                                    .radius(12.0),
                            )
                            .child(
                                Row::new()
                                    .align(AlignItems::Center)
                                    .child(
                                        Column::new()
                                            .flex_grow(1.0)
                                            .gap(2.0)
                                            .child(
                                                Text::new("Catch The Rainbow")
                                                    .size(14.0)
                                                    .weight(FontWeight::Bold)
                                                    .color(white),
                                            )
                                            .child(
                                                Text::new("Rainbow").size(12.0).color(pink_soft),
                                            ),
                                    )
                                    .child(
                                        Container::row()
                                            .width(Length::Px(24.0))
                                            .height(Length::Px(24.0))
                                            .radius(12.0)
                                            .align(AlignItems::Center)
                                            .justify(JustifyContent::Center)
                                            .fill(Color::from_rgba8(219, 39, 119, 75))
                                            .child(Icon::svg(ICO_CHECK, 12.0).color(pink_soft)),
                                    ),
                            ),
                    )
                    .child(
                        Container::column()
                            .flex_grow(1.0)
                            .padding(Edges::all(12.0))
                            .radius(18.0)
                            .material(glass_panel_mat)
                            .child(
                                Stack::new()
                                    .flex_grow(1.0)
                                    .child(
                                        Image::from_decoded(band_art.clone())
                                            .flex_grow(1.0)
                                            .radius(12.0),
                                    )
                                    .child(
                                        Container::new()
                                            .absolute()
                                            .inset(Inset {
                                                left: Length::Px(12.0),
                                                top: Length::Px(12.0),
                                                right: Length::Auto,
                                                bottom: Length::Auto,
                                            })
                                            .child(
                                                Text::new("About the artist")
                                                    .size(13.0)
                                                    .weight(FontWeight::Bold)
                                                    .color(white),
                                            ),
                                    ),
                            ),
                    );

                // ── 5. Floating Liquid Glass Player Dock ───────────────────────────
                let player_dock = Container::row()
                    .absolute()
                    .inset(Inset {
                        left: Length::Px(72.0),
                        right: Length::Px(72.0),
                        top: Length::Auto,
                        bottom: Length::Px(18.0),
                    })
                    .height(Length::Px(68.0))
                    .padding(Edges::xy(22.0, 10.0))
                    .align(AlignItems::Center)
                    .gap(14.0)
                    .radius(34.0)
                    .material(liquid_dock_mat)
                    .child(Text::new("2:13").size(11.5).color(muted))
                    .child(Button::icon_only(ICO_SHUFFLE, 14.0).ghost())
                    .child(Button::icon_only(ICO_PREV, 15.0).ghost())
                    .child(
                        Button::icon_only(ICO_PAUSE, 16.0)
                            .size(Length::Px(40.0), Length::Px(40.0))
                            .on_click(move || is_playing.update(|p| *p = !*p)),
                    )
                    .child(Button::icon_only(ICO_NEXT, 15.0).ghost())
                    .child(Button::icon_only(ICO_REPEAT, 14.0).ghost())
                    .child(
                        Container::row()
                            .padding(Edges::xy(10.0, 6.0))
                            .gap(10.0)
                            .radius(24.0)
                            .align(AlignItems::Center)
                            .fill(Color::from_rgba8(255, 255, 255, 14))
                            .border(Color::from_rgba8(255, 255, 255, 24), 1.0)
                            .child(
                                Image::from_decoded(rainbow_art.clone())
                                    .width(Length::Px(36.0))
                                    .height(Length::Px(36.0))
                                    .radius(8.0),
                            )
                            .child(
                                Column::new()
                                    .gap(1.0)
                                    .child(
                                        Text::new("Catch The Rainbow")
                                            .size(12.0)
                                            .weight(FontWeight::Bold)
                                            .color(white),
                                    )
                                    .child(Text::new("Rainbow").size(11.0).color(muted)),
                            ),
                    )
                    .child(
                        Row::new()
                            .flex_grow(1.0)
                            .align(AlignItems::Center)
                            .gap(10.0)
                            .child(ProgressBar::new(progress).height(6.0).color(pink))
                            .child(Text::new("6:36").size(11.5).color(muted)),
                    )
                    .child(Icon::svg(ICO_VOLUME, 15.0).color(muted))
                    .child(Slider::new(volume, 0.0, 1.0).width(84.0));

                // ── Root Stack: Ambient WGSL Backdrop + Workspace + Floating Dock ──
                Stack::new()
                    .flex_grow(1.0)
                    .child(
                        CustomGpuSurface::new(AmbientGlowPass {
                            pipeline: None,
                            format: None,
                        })
                        .width(Length::Percent(1.0))
                        .height(Length::Percent(1.0))
                        .radius(0.0)
                        .resolution_scale(0.25),
                    )
                    .child(
                        Container::column()
                            .absolute()
                            .inset(Inset::all(Length::Px(0.0)))
                            .child(top_bar)
                            .child(
                                Row::new()
                                    .flex_grow(1.0)
                                    .padding(Edges::xy(16.0, 8.0))
                                    .gap(12.0)
                                    .child(left_sidebar)
                                    .child(center_main)
                                    .child(right_sidebar),
                            ),
                    )
                    .child(player_dock)
                    .into_element()
            })
            .with_theme(create_signal(ThemeMode::Dark))
            .show_fps(true),
        ),
    });

    if let Err(err) = app.run() {
        eprintln!("Music player error: {err}");
    }
}
