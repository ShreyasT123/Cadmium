use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use cadmium::{
    AlignItems, App, Breakpoint, Button, Clipboard, Color, Column, Container, CustomGpuPass,
    CustomGpuSurface, DecodedImage, Edges, Element, FontWeight, Icon, Image, Inset, IntoElement,
    Length, MacTheme, Material, MenuItem, ProgressBar, Row, ScrollView, Shadow, Show, Signal,
    Slider, Spacer, Stack, Switcher, Text, TextInput, TextTone, ThemeMode, TrafficLights,
    VirtualRow, VirtualizedList, WidgetWindow, WindowSpec, Wrap, create_effect, create_memo,
    create_signal,
};

// ── Crisp SVG Vector Icons ───────────────────────────────────────────────────────
const ICO_SEARCH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M15.5 14h-.79l-.28-.27A6.47 6.47 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/></svg>"##;
const ICO_HOME: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/></svg>"##;
const ICO_GRID: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M3 3h8v8H3V3zm10 0h8v8h-8V3zM3 13h8v8H3v-8zm10 0h8v8h-8v-8z"/></svg>"##;
const ICO_RADIO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M12 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4zm-4.95-2.95a7 7 0 0 0 0 9.9l1.41-1.41a5 5 0 0 1 0-7.07L7.05 7.05zm9.9 0-1.41 1.42a5 5 0 0 1 0 7.07l1.41 1.41a7 7 0 0 0 0-9.9zM4.22 4.22a11 11 0 0 0 0 15.56l1.41-1.41a9 9 0 0 1 0-12.73L4.22 4.22zm15.56 0-1.41 1.42a9 9 0 0 1 0 12.73l1.41 1.41a11 11 0 0 0 0-15.56z"/></svg>"##;
const ICO_NOTE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>"##;
const ICO_PIN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M16 12V4h1V2H7v2h1v8l-2 2v2h5v6l1 1 1-1v-6h5v-2l-2-2z"/></svg>"##;
const ICO_PLAYLIST: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/></svg>"##;
const ICO_STAR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="m12 17.27 6.18 3.73-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/></svg>"##;
const ICO_WAVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M3 13h2v-2H3v2zm4 4h2V7H7v10zm4 4h2V3h-2v18zm4-4h2V7h-2v10zm4-6v2h2v-2h-2z"/></svg>"##;
const ICO_CHEVRON_DOWN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M7.41 8.59 12 13.17l4.59-4.58L18 10l-6 6-6-6z"/></svg>"##;
const ICO_CHEVRON_RIGHT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M8.59 16.59 13.17 12 8.59 7.41 10 6l6 6-6 6z"/></svg>"##;
const ICO_CHEVRON_LEFT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12z"/></svg>"##;
const ICO_SHUFFLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M10.59 9.17 5.41 4 4 5.41l5.17 5.17 1.42-1.41zM14.5 4l2.04 2.04L4 18.59 5.41 20 17.96 7.46 20 9.5V4h-5.5zm.33 9.41-1.41 1.41 3.13 3.13L14.5 20H20v-5.5l-2.04 2.04-3.13-3.13z"/></svg>"##;
const ICO_PREV: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M6 6h2v12H6zm3.5 6 8.5 6V6z"/></svg>"##;
const ICO_PLAY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M8 5.14v14l11-7-11-7z"/></svg>"##;
const ICO_CLOSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>"##;
const ICO_PAUSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg>"##;
const ICO_NEXT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z"/></svg>"##;
const ICO_REPEAT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z"/></svg>"##;
const ICO_MORE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M6 10c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm12 0c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm-6 0c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z"/></svg>"##;
const ICO_QUEUE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M3 6h18v2H3V6zm0 5h18v2H3v-2zm0 5h12v2H3v-2z"/></svg>"##;
const ICO_VOLUME: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02z"/></svg>"##;
const ICO_THEME: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M12 3a9 9 0 1 0 9 9c0-.46-.04-.92-.1-1.36a5.39 5.39 0 0 1-4.4 2.26 5.4 5.4 0 0 1-3.14-9.8C12.92 3.04 12.46 3 12 3z"/></svg>"##;

#[derive(Clone)]
struct Track {
    title: &'static str,
    artist: &'static str,
    album: &'static str,
    duration_secs: u32,
    explicit: bool,
    color_a: [f32; 3],
    color_b: [f32; 3],
    art: Arc<DecodedImage>,
}

fn gen_art(
    seed: usize,
    c1: [f32; 3],
    c2: [f32; 3],
    c3: [f32; 3],
    w: u32,
    h: u32,
) -> Arc<DecodedImage> {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let p = seed as f32 * 1.47;
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let dx = u - 0.52;
            let dy = v - 0.48;
            let dist = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);
            let swirl = (dist * 8.5 - angle * 2.0 + p).sin() * 0.5 + 0.5;
            let stripe = ((u * 7.0 + v * 3.0 + p).cos() * 0.5 + 0.5) * 0.45;
            let halo = (1.0 - (dist * 1.8).min(1.0)).powf(1.6) * 0.5;

            let t1 = (u * 0.65 + v * 0.35 + swirl * 0.32).clamp(0.0, 1.0);
            let t2 = (stripe + halo).clamp(0.0, 1.0);
            let r = ((c1[0] * (1.0 - t1) + c2[0] * t1) * (1.0 - t2 * 0.45) + c3[0] * t2 * 0.6)
                .clamp(0.0, 1.0);
            let g = ((c1[1] * (1.0 - t1) + c2[1] * t1) * (1.0 - t2 * 0.45) + c3[1] * t2 * 0.6)
                .clamp(0.0, 1.0);
            let b = ((c1[2] * (1.0 - t1) + c2[2] * t1) * (1.0 - t2 * 0.45) + c3[2] * t2 * 0.6)
                .clamp(0.0, 1.0);

            rgba.push((r * 255.0) as u8);
            rgba.push((g * 255.0) as u8);
            rgba.push((b * 255.0) as u8);
            rgba.push(255);
        }
    }
    Arc::new(DecodedImage {
        width: w,
        height: h,
        rgba,
    })
}

const BEATS_STAGE_WGSL: &str = r#"
struct Params {
    time: f32,
    playing: f32,
    show_wave: f32,
    center_x: f32,
    color_a: vec4<f32>,
    color_b: vec4<f32>,
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
    let uv = in.uv;
    let t = params.time * mix(0.15, 2.2, params.playing);

    // 1. Deep obsidian black (#0E0E11 in linear space for Rgba8UnormSrgb target)
    var col = vec3<f32>(0.0042, 0.0042, 0.0055);

    // Continuous album-art color bleed radiating across both the canvas and frosted sidebar
    let cx = params.center_x;
    let wave_center = vec2<f32>(cx, 0.21);
    let art_center  = vec2<f32>(cx, 0.48);

    let d_wave = length((uv - wave_center) * vec2<f32>(1.35, 3.0));
    let d_art  = length((uv - art_center)  * vec2<f32>(1.55, 1.9));
    let d_side = length((uv - vec2<f32>(0.08, 0.36)) * vec2<f32>(1.8, 1.1));

    let smudge_wave = exp(-d_wave * 2.5) * 0.062;
    let smudge_art  = exp(-d_art * 2.7) * 0.056;
    let smudge_b    = exp(-length((uv - vec2<f32>(cx - 0.07, 0.45)) * vec2<f32>(1.7, 2.1)) * 2.8) * 0.038;
    let sidebar_bleed = exp(-d_side * 2.2) * 0.042;

    col += params.color_a.rgb * (smudge_wave + smudge_art + sidebar_bleed)
         + params.color_b.rgb * (smudge_b + sidebar_bleed * 0.75);

    // 2. Single continuous unbroken symmetric audio waveform (no center gap)
    if params.show_wave > 0.5 {
        let bars = 190.0;
        let cell_x = fract(uv.x * bars);
        let bar_id = floor(uv.x * bars);
        let dx_wave = abs(uv.x - cx);

        let env = exp(-pow(dx_wave / 0.145, 2.0));
        let h1 = 0.5 + 0.5 * sin(bar_id * 1.31 + t * 2.4) * cos(bar_id * 0.67 - t * 1.7);
        let h2 = 0.5 + 0.5 * sin(bar_id * 2.73 - t * 3.1);
        let peak = pow(h1 * 0.6 + h2 * 0.4, 1.95);

        let spike = select(0.36, 1.0, (i32(bar_id) % 3) == 0);
        let amp = (0.004 + 0.054 * peak * spike * env) * mix(0.55, 1.0, params.playing);

        let dy_wave = abs(uv.y - wave_center.y);
        let tick_x = smoothstep(0.36, 0.47, cell_x) * (1.0 - smoothstep(0.53, 0.64, cell_x));
        let tick_y = 1.0 - smoothstep(amp - 0.002, amp + 0.002, dy_wave);
        let dot_mask = (1.0 - smoothstep(0.0015, 0.0032, dy_wave)) * tick_x;

        let edge_fade = smoothstep(0.28, 0.08, dx_wave);
        let bar_tint = mix(params.color_a.rgb, vec3<f32>(1.0, 0.88, 0.92), 0.36);
        let bar_alpha = max(tick_x * tick_y * 0.88, dot_mask * 0.52) * edge_fade;
        col = mix(col, bar_tint, bar_alpha);
    }

    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
"#;

struct BeatsStagePass {
    playing: Signal<bool>,
    active_nav: Signal<usize>,
    viewport: Signal<(f32, f32)>,
    is_compact: Signal<bool>,
    track_idx: Signal<usize>,
    color_pairs: Vec<([f32; 3], [f32; 3])>,
    pipeline: Option<wgpu::RenderPipeline>,
    buf: Option<wgpu::Buffer>,
    bg: Option<wgpu::BindGroup>,
    format: Option<wgpu::TextureFormat>,
}

impl CustomGpuPass for BeatsStagePass {
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        _w: u32,
        _h: u32,
        time_secs: f32,
    ) {
        if self.pipeline.is_none() || self.format != Some(format) {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("BeatsStage Shader"),
                source: wgpu::ShaderSource::Wgsl(BEATS_STAGE_WGSL.into()),
            });
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("BeatsStage BGL"),
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
                label: Some("BeatsStage Uniform"),
                size: 48,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("BeatsStage BG"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("BeatsStage Layout"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            self.pipeline = Some(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("BeatsStage Pipe"),
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
            self.buf = Some(buf);
            self.bg = Some(bg);
            self.format = Some(format);
        }

        let idx = self.track_idx.try_get().unwrap_or(0) % self.color_pairs.len().max(1);
        let (ca, cb) = self.color_pairs[idx];
        let win_w = self
            .viewport
            .try_get()
            .map(|v| v.0)
            .unwrap_or(1280.0)
            .max(1.0);
        let sb_w = if self.is_compact.try_get().unwrap_or(false) {
            60.0
        } else {
            224.0
        };
        let center_x = ((sb_w + (win_w - sb_w) * 0.5) / win_w).clamp(0.35, 0.75);
        let show_wave = if self.active_nav.try_get().unwrap_or(2) == 3 {
            1.0
        } else {
            0.0
        };

        let params: [f32; 12] = [
            time_secs,
            if self.playing.try_get().unwrap_or(false) {
                1.0
            } else {
                0.0
            },
            show_wave,
            center_x,
            ca[0],
            ca[1],
            ca[2],
            1.0,
            cb[0],
            cb[1],
            cb[2],
            1.0,
        ];
        if let Some(buf) = &self.buf {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&params));
        }
        if let (Some(pipe), Some(bg)) = (&self.pipeline, &self.bg) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BeatsStage Pass"),
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

fn format_mm_ss(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,wgpu=warn")
        .init();

    let tracks = vec![
        Track {
            title: "Cruel World",
            artist: "Holly Humberstone",
            album: "Cruel World",
            duration_secs: 218,
            explicit: false,
            color_a: [0.06, 0.14, 0.28],
            color_b: [0.22, 0.54, 0.88],
            art: gen_art(
                1,
                [0.06, 0.14, 0.28],
                [0.22, 0.54, 0.88],
                [0.95, 0.88, 0.72],
                160,
                160,
            ),
        },
        Track {
            title: "Snakeskin Boots",
            artist: "Miranda Lambert & Ella Langley",
            album: "Single",
            duration_secs: 194,
            explicit: false,
            color_a: [0.55, 0.22, 0.12],
            color_b: [0.88, 0.52, 0.28],
            art: gen_art(
                2,
                [0.55, 0.22, 0.12],
                [0.88, 0.52, 0.28],
                [0.96, 0.92, 0.84],
                160,
                160,
            ),
        },
        Track {
            title: "Us Again",
            artist: "Gavin Adcock & Treaty Oak",
            album: "Actin' Up",
            duration_secs: 206,
            explicit: false,
            color_a: [0.18, 0.18, 0.22],
            color_b: [0.58, 0.58, 0.62],
            art: gen_art(
                3,
                [0.18, 0.18, 0.22],
                [0.58, 0.58, 0.62],
                [0.92, 0.92, 0.94],
                160,
                160,
            ),
        },
        Track {
            title: "Psycho Killer (2024)",
            artist: "Talking Heads",
            album: "77",
            duration_secs: 261,
            explicit: false,
            color_a: [0.65, 0.10, 0.14],
            color_b: [0.18, 0.18, 0.20],
            art: gen_art(
                4,
                [0.65, 0.10, 0.14],
                [0.18, 0.18, 0.20],
                [0.96, 0.86, 0.82],
                160,
                160,
            ),
        },
        Track {
            title: "Draw You Out",
            artist: "Noah Kahan",
            album: "OST",
            duration_secs: 232,
            explicit: false,
            color_a: [0.12, 0.34, 0.26],
            color_b: [0.32, 0.66, 0.52],
            art: gen_art(
                5,
                [0.12, 0.34, 0.26],
                [0.32, 0.66, 0.52],
                [0.88, 0.95, 0.86],
                160,
                160,
            ),
        },
        Track {
            title: "Why Don't You Say So",
            artist: "Dexter and The Moonrocks",
            album: "Western Space",
            duration_secs: 185,
            explicit: false,
            color_a: [0.48, 0.28, 0.14],
            color_b: [0.82, 0.64, 0.36],
            art: gen_art(
                6,
                [0.48, 0.28, 0.14],
                [0.82, 0.64, 0.36],
                [0.98, 0.92, 0.82],
                160,
                160,
            ),
        },
        Track {
            title: "HELEN KELLER",
            artist: "Placebo",
            album: "Never Let Me Go",
            duration_secs: 210,
            explicit: false,
            color_a: [0.20, 0.08, 0.12],
            color_b: [0.92, 0.24, 0.32],
            art: gen_art(
                7,
                [0.20, 0.08, 0.12],
                [0.92, 0.24, 0.32],
                [0.96, 0.96, 0.98],
                160,
                160,
            ),
        },
        Track {
            title: "Man In The Box",
            artist: "HARDY",
            album: "Quit!!",
            duration_secs: 244,
            explicit: true,
            color_a: [0.26, 0.20, 0.14],
            color_b: [0.64, 0.52, 0.38],
            art: gen_art(
                8,
                [0.26, 0.20, 0.14],
                [0.64, 0.52, 0.38],
                [0.90, 0.86, 0.80],
                160,
                160,
            ),
        },
        Track {
            title: "HONEYMOON NIGHT",
            artist: "Teezo Touchdown",
            album: "How Do You Sleep",
            duration_secs: 178,
            explicit: true,
            color_a: [0.58, 0.12, 0.34],
            color_b: [0.92, 0.44, 0.68],
            art: gen_art(
                9,
                [0.58, 0.12, 0.34],
                [0.92, 0.44, 0.68],
                [0.98, 0.90, 0.94],
                160,
                160,
            ),
        },
        Track {
            title: "Close To Me (Solo)",
            artist: "Ellie Goulding & Diplo",
            album: "Brightest Blue",
            duration_secs: 189,
            explicit: false,
            color_a: [0.44, 0.32, 0.12],
            color_b: [0.88, 0.72, 0.36],
            art: gen_art(
                10,
                [0.44, 0.32, 0.12],
                [0.88, 0.72, 0.36],
                [0.98, 0.96, 0.88],
                160,
                160,
            ),
        },
        Track {
            title: "Moving at Light Speed",
            artist: "Thundercat",
            album: "It Is What It Is",
            duration_secs: 226,
            explicit: false,
            color_a: [0.16, 0.08, 0.24],
            color_b: [0.96, 0.48, 0.12],
            art: gen_art(
                11,
                [0.16, 0.08, 0.24],
                [0.96, 0.48, 0.12],
                [0.98, 0.84, 0.28],
                160,
                160,
            ),
        },
        Track {
            title: "The Myth",
            artist: "DOWN",
            album: "NOLA",
            duration_secs: 252,
            explicit: true,
            color_a: [0.12, 0.10, 0.10],
            color_b: [0.62, 0.28, 0.18],
            art: gen_art(
                12,
                [0.12, 0.10, 0.10],
                [0.62, 0.28, 0.18],
                [0.88, 0.84, 0.78],
                160,
                160,
            ),
        },
    ];
    let hero_arts = [
        gen_art(
            101,
            [0.06, 0.08, 0.14],
            [0.18, 0.52, 0.96],
            [0.96, 0.96, 0.98],
            320,
            190,
        ),
        gen_art(
            102,
            [0.96, 0.22, 0.08],
            [0.98, 0.80, 0.10],
            [0.12, 0.16, 0.28],
            320,
            190,
        ),
        gen_art(
            103,
            [0.22, 0.22, 0.24],
            [0.58, 0.58, 0.62],
            [0.88, 0.76, 0.62],
            320,
            190,
        ),
    ];
    let color_pairs: Vec<([f32; 3], [f32; 3])> =
        tracks.iter().map(|t| (t.color_a, t.color_b)).collect();

    let app = App::new();
    let dispatcher = app.dispatcher();

    let theme_mode = create_signal(ThemeMode::Dark);
    let active_nav = create_signal(2_usize); // 0=Search, 1=Home, 2=New, 3=Beats 01 Stage, 4=Songs(100k)
    let pins_open = create_signal(true);
    let playlists_open = create_signal(true);
    let current_track = create_signal(0_usize);
    let is_playing = create_signal(false);
    let is_paused = create_memo(move || !is_playing.get());
    let shuffle_on = create_signal(false);
    let repeat_on = create_signal(false);
    let progress = create_signal(0.32_f32);
    let volume = create_signal(0.75_f32);
    let queue_open = create_signal(false);
    let search_query = create_signal(String::new());
    let viewport = create_signal((1280.0_f32, 800.0_f32));
    let breakpoint = create_memo(move || Breakpoint::from_width(viewport.get().0));
    let sidebar_open = create_signal(false);
    let is_compact =
        create_memo(move || breakpoint.get() == Breakpoint::Compact && !sidebar_open.get());
    let stage_active = create_memo(move || active_nav.get() == 3);
    let bg_animating = create_memo(move || is_playing.get() && active_nav.get() == 3);

    let avatar_art = gen_art(
        77,
        [0.28, 0.20, 0.34],
        [0.68, 0.42, 0.52],
        [0.96, 0.88, 0.84],
        128,
        128,
    );

    // Zero-wakeup playback timer thread (parks on Condvar when paused)
    let play_gate = Arc::new((Mutex::new(false), Condvar::new()));
    let playing_atomic = Arc::new(AtomicBool::new(false));
    {
        let gate = play_gate.clone();
        let flag = playing_atomic.clone();
        let disp = dispatcher.clone();
        std::thread::spawn(move || {
            loop {
                let (lock, cvar) = &*gate;
                let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
                while !*guard {
                    guard = cvar.wait(guard).unwrap_or_else(|e| e.into_inner());
                }
                drop(guard);

                std::thread::sleep(Duration::from_millis(250));
                if flag.load(Ordering::Relaxed) {
                    disp.dispatch(move || {
                        if is_playing.try_get().unwrap_or(false) {
                            progress.update(|p| {
                                *p += 0.0025;
                                if *p >= 1.0 {
                                    *p = 0.0;
                                    current_track.update(|t| *t = (*t + 1) % 12);
                                }
                            });
                        }
                    });
                }
            }
        });
    }

    let toggle_play: Rc<dyn Fn()> = Rc::new({
        let gate = play_gate.clone();
        let flag = playing_atomic.clone();
        move || {
            let next = !is_playing.get();
            is_playing.set(next);
            flag.store(next, Ordering::Relaxed);
            let (lock, cvar) = &*gate;
            *lock.lock().unwrap_or_else(|e| e.into_inner()) = next;
            cvar.notify_all();
        }
    });

    let play_index: Rc<dyn Fn(usize)> = Rc::new({
        let gate = play_gate.clone();
        let flag = playing_atomic.clone();
        move |idx: usize| {
            current_track.set(idx % 12);
            progress.set(0.0);
            if !is_playing.get() {
                is_playing.set(true);
                flag.store(true, Ordering::Relaxed);
                let (lock, cvar) = &*gate;
                *lock.lock().unwrap_or_else(|e| e.into_inner()) = true;
                cvar.notify_all();
            }
        }
    });

    // Theme-reactive materials (zero harsh white borders in Dark Mode; translucent frosted continuity)
    let canvas_mat = create_signal(Material::solid(Color::TRANSPARENT));
    let sidebar_mat = create_signal(Material::Frosted {
        tint: Color::from_rgba8(18, 17, 22, 138),
        frost: 0.60,
        specular: 0.0,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        shadow: None,
    });
    let divider_mat = create_signal(Material::solid(Color::from_rgba8(255, 255, 255, 10)));
    let dock_mat = create_signal(Material::Liquid {
        tint: Color::from_rgba8(24, 24, 28, 175),
        frost: 0.42,
        ripple: 0.42,
        specular: 0.16,
        rim_angle: -1.309,
        sheen_progress: 0.0,
        conic_strength: 0.0,
        border_color: Color::from_rgba8(255, 255, 255, 14),
        border_width: 1.0,
        shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 150), 24.0, 10.0)),
    });
    let popover_mat = create_signal(Material::Frosted {
        tint: Color::from_rgba8(28, 28, 34, 215),
        frost: 0.55,
        specular: 0.12,
        border_color: Color::from_rgba8(255, 255, 255, 14),
        border_width: 1.0,
        shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 160), 22.0, 8.0)),
    });
    let search_stage_mat = create_signal(Material::solid(Color::from_rgba8(255, 255, 255, 20)));

    create_effect(move || {
        let mode = theme_mode.get();
        let on_stage = stage_active.get();
        match mode {
            ThemeMode::Light => {
                canvas_mat.set(if on_stage {
                    Material::solid(Color::TRANSPARENT)
                } else {
                    Material::solid(MacTheme::light().content_bg)
                });
                sidebar_mat.set(Material::Frosted {
                    tint: if on_stage {
                        Color::from_rgba8(238, 236, 242, 200)
                    } else {
                        Color::from_rgba8(245, 244, 247, 224)
                    },
                    frost: 0.58,
                    specular: 0.0,
                    border_color: Color::TRANSPARENT,
                    border_width: 0.0,
                    shadow: None,
                });
                divider_mat.set(Material::solid(Color::from_rgba8(0, 0, 0, 20)));
                dock_mat.set(Material::Liquid {
                    tint: Color::from_rgba8(255, 255, 255, 198),
                    frost: 0.36,
                    ripple: 0.42,
                    specular: 0.85,
                    rim_angle: -1.309,
                    sheen_progress: 0.0,
                    conic_strength: 0.0,
                    border_color: Color::from_rgba8(0, 0, 0, 22),
                    border_width: 1.0,
                    shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 46), 24.0, 8.0)),
                });
                popover_mat.set(Material::Frosted {
                    tint: Color::from_rgba8(252, 252, 254, 228),
                    frost: 0.52,
                    specular: 0.4,
                    border_color: Color::from_rgba8(0, 0, 0, 20),
                    border_width: 1.0,
                    shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 44), 20.0, 8.0)),
                });
                search_stage_mat.set(Material::solid(Color::from_rgba8(255, 255, 255, 242)));
            }
            ThemeMode::Dark => {
                // Opaque on browse pages so the sidebar still gets full-window bleed
                // without Stage waveforms peeking behind cards; Transparent on Stage (3).
                canvas_mat.set(if on_stage {
                    Material::solid(Color::TRANSPARENT)
                } else {
                    Material::solid(Color::from_rgba8(16, 16, 20, 255))
                });
                sidebar_mat.set(Material::Frosted {
                    tint: Color::from_rgba8(18, 17, 22, 138),
                    frost: 0.62,
                    specular: 0.0,
                    border_color: Color::TRANSPARENT,
                    border_width: 0.0,
                    shadow: None,
                });
                divider_mat.set(Material::solid(Color::from_rgba8(255, 255, 255, 10)));
                dock_mat.set(Material::Liquid {
                    tint: Color::from_rgba8(24, 24, 28, 178),
                    frost: 0.42,
                    ripple: 0.40,
                    specular: 0.16,
                    rim_angle: -1.309,
                    sheen_progress: 0.0,
                    conic_strength: 0.0,
                    border_color: Color::from_rgba8(255, 255, 255, 14),
                    border_width: 1.0,
                    shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 155), 24.0, 10.0)),
                });
                popover_mat.set(Material::Frosted {
                    tint: Color::from_rgba8(28, 28, 34, 218),
                    frost: 0.55,
                    specular: 0.12,
                    border_color: Color::from_rgba8(255, 255, 255, 14),
                    border_width: 1.0,
                    shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 160), 22.0, 8.0)),
                });
                search_stage_mat.set(Material::solid(Color::from_rgba8(255, 255, 255, 20)));
            }
        }
    });

    let tracks_ui = tracks.clone();
    let window = WidgetWindow::new(move || {
        let tracks = tracks_ui.clone();
        let active_pill_fill = Color::from_rgba8(120, 120, 128, 34);
        let hover_pill_fill = Color::from_rgba8(120, 120, 128, 20);
        let apple_pink = Color::from_rgba8(250, 45, 72, 255);
        let sidebar_mode = create_memo(move || if is_compact.get() { 1 } else { 0 });
        let show_secondary = create_memo(move || viewport.get().0 >= 1020.0);

        let nav_row =
            |label: &'static str, icon_svg: &'static str, nav_idx: usize, tint_accent: bool| {
                let mat = create_memo(move || {
                    Material::solid(if active_nav.get() == nav_idx {
                        active_pill_fill
                    } else {
                        Color::TRANSPARENT
                    })
                });
                Container::row()
                    .height(Length::Px(30.0))
                    .padding(Edges::xy(10.0, 0.0))
                    .gap(9.0)
                    .radius(8.0)
                    .align(AlignItems::Center)
                    .bind_material(mat.signal())
                    .hover_fill(hover_pill_fill)
                    .on_click(move || active_nav.set(nav_idx))
                    .child(if tint_accent {
                        Icon::svg(icon_svg, 15.0).color(apple_pink)
                    } else {
                        Icon::svg(icon_svg, 15.0).tone(TextTone::Primary)
                    })
                    .child(
                        Text::new(label)
                            .size(13.0)
                            .weight(FontWeight::Medium)
                            .tone(TextTone::Primary),
                    )
            };

        let svg_chevron = |sig: Signal<bool>| {
            let closed = create_memo(move || !sig.get());
            Stack::new()
                .width(Length::Px(12.0))
                .height(Length::Px(12.0))
                .child(Show::new(sig, || {
                    Icon::svg(ICO_CHEVRON_DOWN, 12.0)
                        .tone(TextTone::Tertiary)
                        .into_element()
                }))
                .child(Show::new(closed.signal(), || {
                    Icon::svg(ICO_CHEVRON_RIGHT, 12.0)
                        .tone(TextTone::Tertiary)
                        .into_element()
                }))
        };

        // ── 1. Frosted Left Sidebar (Full Height, Borderless, Bleeds Color) ──────
        let pins_dropdown = Show::new(pins_open, {
            let tr = tracks.clone();
            let pi = play_index.clone();
            move || {
                let mut col = Column::new().gap(2.0).padding(Edges::xy(8.0, 2.0));
                for (i, label) in [
                    (0, "Cruel World"),
                    (1, "The Romantic"),
                    (4, "Work focus"),
                    (10, "Commute"),
                ] {
                    let pi_cl = pi.clone();
                    col = col.child(
                        Container::row()
                            .height(Length::Px(28.0))
                            .padding(Edges::xy(8.0, 0.0))
                            .gap(8.0)
                            .radius(6.0)
                            .align(AlignItems::Center)
                            .hover_fill(hover_pill_fill)
                            .on_click(move || pi_cl(i))
                            .child(
                                Image::from_decoded(tr[i].art.clone())
                                    .width(Length::Px(18.0))
                                    .height(Length::Px(18.0))
                                    .radius(4.0),
                            )
                            .child(Text::new(label).size(12.5).tone(TextTone::Primary)),
                    );
                }
                col.into_element()
            }
        });

        let playlists_dropdown = Show::new(playlists_open, {
            let pi = play_index.clone();
            move || {
                let mut col = Column::new().gap(2.0);
                for (label, icon, track_i) in [
                    ("All Playlists", ICO_PLAYLIST, 0),
                    ("Favorite Songs", ICO_STAR, 3),
                    ("Liquid Focus", ICO_NOTE, 6),
                ] {
                    let pi_cl = pi.clone();
                    col = col.child(
                        Container::row()
                            .height(Length::Px(28.0))
                            .padding(Edges::xy(10.0, 0.0))
                            .gap(9.0)
                            .radius(7.0)
                            .align(AlignItems::Center)
                            .hover_fill(hover_pill_fill)
                            .on_click(move || pi_cl(track_i))
                            .child(Icon::svg(icon, 14.0).color(apple_pink))
                            .child(Text::new(label).size(12.5).tone(TextTone::Primary)),
                    );
                }
                col.into_element()
            }
        });

        let sidebar_scroll = ScrollView::new()
            .gap(2.0)
            .child(nav_row("Search", ICO_SEARCH, 0, false))
            .child(nav_row("Home", ICO_HOME, 1, false))
            .child(nav_row("New", ICO_GRID, 2, true))
            .child(nav_row("Now Playing • Beats 01", ICO_RADIO, 3, true))
            .child(Container::new().height(Length::Px(12.0)))
            .child(
                Container::row().padding(Edges::xy(10.0, 2.0)).child(
                    Text::new("Library")
                        .size(11.0)
                        .weight(FontWeight::Semibold)
                        .tone(TextTone::Tertiary),
                ),
            )
            .child(
                Container::row()
                    .height(Length::Px(28.0))
                    .padding(Edges::xy(8.0, 0.0))
                    .gap(6.0)
                    .radius(7.0)
                    .align(AlignItems::Center)
                    .hover_fill(hover_pill_fill)
                    .on_click(move || pins_open.update(|v| *v = !*v))
                    .child(svg_chevron(pins_open))
                    .child(Icon::svg(ICO_PIN, 13.0).color(apple_pink))
                    .child(
                        Text::new("Pins")
                            .size(13.0)
                            .weight(FontWeight::Medium)
                            .tone(TextTone::Primary),
                    ),
            )
            .child(pins_dropdown)
            .child(nav_row("Songs (100k)", ICO_NOTE, 4, true))
            .child(Container::new().height(Length::Px(12.0)))
            .child(
                Container::row()
                    .height(Length::Px(26.0))
                    .padding(Edges::xy(8.0, 2.0))
                    .gap(6.0)
                    .radius(6.0)
                    .align(AlignItems::Center)
                    .hover_fill(hover_pill_fill)
                    .on_click(move || playlists_open.update(|v| *v = !*v))
                    .child(svg_chevron(playlists_open))
                    .child(
                        Text::new("Playlists")
                            .size(11.0)
                            .weight(FontWeight::Semibold)
                            .tone(TextTone::Tertiary),
                    ),
            )
            .child(playlists_dropdown);

        let sidebar = Container::column()
            .width(Length::Px(224.0))
            .height(Length::Percent(1.0))
            .flex_grow(1.0)
            .padding(Edges::xy(10.0, 12.0))
            .gap(10.0)
            .bind_material(sidebar_mat)
            .child(
                Container::row()
                    .height(Length::Px(28.0))
                    .padding(Edges::xy(6.0, 0.0))
                    .align(AlignItems::Center)
                    .drag_region()
                    .child(TrafficLights::new()),
            )
            .child(sidebar_scroll)
            .child(
                Row::new()
                    .height(Length::Px(34.0))
                    .padding(Edges::xy(6.0, 0.0))
                    .gap(8.0)
                    .align(AlignItems::Center)
                    .child(
                        Image::from_decoded(avatar_art.clone())
                            .width(Length::Px(26.0))
                            .height(Length::Px(26.0))
                            .radius(13.0),
                    )
                    .child(
                        Text::new("Shreyas")
                            .size(12.5)
                            .weight(FontWeight::Medium)
                            .tone(TextTone::Primary),
                    )
                    .child(Spacer::flex())
                    .child(
                        Button::icon_only(ICO_THEME, 14.0)
                            .ghost()
                            .tooltip("Toggle Light / Dark Mode")
                            .on_click(move || {
                                theme_mode.update(|m| {
                                    *m = match *m {
                                        ThemeMode::Light => ThemeMode::Dark,
                                        ThemeMode::Dark => ThemeMode::Light,
                                    };
                                });
                            }),
                    ),
            );

        let compact_sidebar = Container::column()
            .width(Length::Px(60.0))
            .height(Length::Percent(1.0))
            .flex_grow(1.0)
            .padding(Edges::xy(6.0, 12.0))
            .gap(10.0)
            .bind_material(sidebar_mat)
            .child(
                Container::row()
                    .height(Length::Px(28.0))
                    .drag_region()
                    .child(TrafficLights::new()),
            )
            .child(
                Column::new()
                    .gap(6.0)
                    .child(
                        Button::icon_only(ICO_SEARCH, 16.0)
                            .ghost()
                            .tooltip("Search")
                            .on_click(move || active_nav.set(0)),
                    )
                    .child(
                        Button::icon_only(ICO_HOME, 16.0)
                            .ghost()
                            .tooltip("Home")
                            .on_click(move || active_nav.set(1)),
                    )
                    .child(
                        Button::icon_only(ICO_GRID, 16.0)
                            .ghost()
                            .tooltip("New")
                            .on_click(move || active_nav.set(2)),
                    )
                    .child(
                        Button::icon_only(ICO_RADIO, 16.0)
                            .ghost()
                            .tooltip("Beats 01 Stage")
                            .on_click(move || active_nav.set(3)),
                    )
                    .child(
                        Button::icon_only(ICO_NOTE, 16.0)
                            .ghost()
                            .tooltip("Songs (100k)")
                            .on_click(move || active_nav.set(4)),
                    ),
            )
            .child(Spacer::flex())
            .child(
                Button::icon_only(ICO_THEME, 14.0)
                    .ghost()
                    .tooltip("Toggle Light / Dark Mode")
                    .on_click(move || {
                        theme_mode.update(|m| {
                            *m = match *m {
                                ThemeMode::Light => ThemeMode::Dark,
                                ThemeMode::Dark => ThemeMode::Light,
                            };
                        });
                    }),
            );

        // ── 2. View 2: "New" Editorial Screen ───────────────────────────────────
        let hero_card = |eyebrow: &'static str,
                         title: &'static str,
                         sub: &'static str,
                         caption: &'static str,
                         art: Arc<DecodedImage>,
                         idx: usize| {
            let pi = play_index.clone();
            Container::column()
                .width(Length::Px(0.0))
                .flex_grow(1.0)
                .gap(4.0)
                .on_click(move || {
                    pi(idx);
                    active_nav.set(3);
                })
                .child(
                    Text::new(eyebrow)
                        .size(10.0)
                        .weight(FontWeight::Semibold)
                        .tone(TextTone::Tertiary),
                )
                .child(
                    Text::new(title)
                        .size(13.5)
                        .weight(FontWeight::Semibold)
                        .tone(TextTone::Primary),
                )
                .child(Text::new(sub).size(12.5).tone(TextTone::Secondary))
                .child(
                    Stack::new()
                        .height(Length::Px(172.0))
                        .child(
                            Image::from_decoded(art)
                                .width(Length::Percent(1.0))
                                .height(Length::Px(172.0))
                                .radius(12.0),
                        )
                        .child(
                            Container::new()
                                .absolute()
                                .inset(Inset::all(Length::Px(0.0)))
                                .radius(12.0)
                                .gradient(Color::TRANSPARENT, Color::from_rgba8(0, 0, 0, 175)),
                        )
                        .child(
                            Container::column()
                                .absolute()
                                .inset(Inset {
                                    left: Length::Px(12.0),
                                    right: Length::Px(12.0),
                                    top: Length::Auto,
                                    bottom: Length::Px(10.0),
                                })
                                .child(
                                    Text::new(caption)
                                        .size(11.5)
                                        .color(Color::from_rgba8(248, 248, 250, 240)),
                                ),
                        ),
                )
        };

        let hero_row = Row::new()
            .padding(Edges::xy(28.0, 0.0))
            .gap(16.0)
            .child(hero_card(
                "UPDATED PLAYLIST",
                "danceXL",
                "Apple Music Dance",
                "Cloonee and Chris Lake bring fresh energy.",
                hero_arts[0].clone(),
                10,
            ))
            .child(hero_card(
                "NEW RADIO SHOW",
                "5 on Fridays",
                "Apple Music 1",
                "Missed Friday's episode? Catch up on Vince.",
                hero_arts[1].clone(),
                8,
            ))
            .child(hero_card(
                "NEW ALBUM",
                "She's the Best",
                "Spatial Audio • Dolby Atmos",
                "Handcrafted acoustic resonance.",
                hero_arts[2].clone(),
                0,
            ));

        // Single Wrap across all 12 songs so 3->2->1 columns never leave orphan rows
        let mut song_grid = Wrap::new().padding(Edges::xy(28.0, 0.0)).gap(14.0);
        for (idx, tr) in tracks.iter().enumerate().take(12) {
            let pi = play_index.clone();
            let t_title = tr.title;
            let t_artist = tr.artist;
            let sel_mat = create_memo(move || {
                Material::solid(if current_track.get() == idx {
                    Color::from_rgba8(250, 45, 72, 22)
                } else {
                    Color::TRANSPARENT
                })
            });
            song_grid = song_grid.child(
                Container::row()
                    .width(Length::Px(280.0))
                    .flex_grow(1.0)
                    .height(Length::Px(54.0))
                    .padding(Edges::xy(6.0, 4.0))
                    .gap(10.0)
                    .radius(8.0)
                    .clip()
                    .align(AlignItems::Center)
                    .bind_material(sel_mat.signal())
                    .hover_fill(hover_pill_fill)
                    .context_menu([
                        MenuItem::new("Play Now", {
                            let pi2 = play_index.clone();
                            move || {
                                pi2(idx);
                                active_nav.set(3);
                            }
                        }),
                        MenuItem::new("Copy Song Info", move || {
                            let _ = Clipboard::set_text(&format!("{t_title} — {t_artist}"));
                        }),
                    ])
                    .on_click(move || {
                        pi(idx);
                        active_nav.set(3);
                    })
                    .child(
                        Image::from_decoded(tr.art.clone())
                            .width(Length::Px(44.0))
                            .height(Length::Px(44.0))
                            .radius(6.0),
                    )
                    .child(
                        Column::new()
                            .width(Length::Px(0.0))
                            .flex_grow(1.0)
                            .gap(2.0)
                            .child(
                                Text::new(if tr.explicit {
                                    format!("{} [E]", tr.title)
                                } else {
                                    tr.title.to_string()
                                })
                                .size(13.0)
                                .weight(FontWeight::Semibold)
                                .tone(TextTone::Primary)
                                .ellipsis(),
                            )
                            .child(
                                Text::new(tr.artist)
                                    .size(11.5)
                                    .tone(TextTone::Secondary)
                                    .ellipsis(),
                            ),
                    )
                    .child(Icon::svg(ICO_MORE, 14.0).tone(TextTone::Tertiary)),
            );
        }

        let mut new_week_row = Wrap::new().padding(Edges::xy(28.0, 0.0)).gap(16.0);
        for i in 0..5 {
            let tr = &tracks[(i + 4) % 12];
            let pi = play_index.clone();
            new_week_row = new_week_row.child(
                Container::column()
                    .width(Length::Px(170.0))
                    .flex_grow(1.0)
                    .gap(6.0)
                    .clip()
                    .on_click(move || {
                        pi((i + 4) % 12);
                        active_nav.set(3);
                    })
                    .child(
                        Image::from_decoded(tr.art.clone())
                            .height(Length::Px(152.0))
                            .radius(10.0),
                    )
                    .child(
                        Text::new(tr.album)
                            .size(12.5)
                            .weight(FontWeight::Semibold)
                            .tone(TextTone::Primary)
                            .ellipsis(),
                    )
                    .child(
                        Text::new(tr.artist)
                            .size(11.5)
                            .tone(TextTone::Secondary)
                            .ellipsis(),
                    ),
            );
        }

        let new_view = ScrollView::new()
            .gap(18.0)
            .child(Container::new().height(Length::Px(10.0)).drag_region())
            .child(
                Container::row().padding(Edges::xy(28.0, 0.0)).child(
                    Text::new("New")
                        .size(30.0)
                        .weight(FontWeight::Bold)
                        .tone(TextTone::Primary),
                ),
            )
            .child(hero_row)
            .child(
                Container::row().padding(Edges::xy(28.0, 2.0)).child(
                    Text::new("Best New Songs")
                        .size(17.0)
                        .weight(FontWeight::Bold)
                        .tone(TextTone::Primary),
                ),
            )
            .child(song_grid)
            .child(
                Container::row().padding(Edges::xy(28.0, 2.0)).child(
                    Text::new("New This Week")
                        .size(17.0)
                        .weight(FontWeight::Bold)
                        .tone(TextTone::Primary),
                ),
            )
            .child(new_week_row)
            .child(Container::new().height(Length::Px(88.0)));

        // ── 3. View 3: Now-Playing Atmospheric Stage (Transparent over Full-Window Bleed!) ──
        let mut featured_shows = Row::new().gap(14.0);
        for i in 0..6 {
            let tr = &tracks[(i + 2) % 12];
            let pi = play_index.clone();
            featured_shows = featured_shows.child(
                Container::column()
                    .width(Length::Px(0.0))
                    .flex_grow(1.0)
                    .gap(6.0)
                    .on_click(move || pi((i + 2) % 12))
                    .child(
                        Image::from_decoded(tr.art.clone())
                            .height(Length::Px(124.0))
                            .radius(8.0),
                    )
                    .child(
                        Text::new(tr.title)
                            .size(12.0)
                            .weight(FontWeight::Semibold)
                            .color(Color::from_rgba8(245, 245, 248, 255))
                            .ellipsis(),
                    )
                    .child(
                        Text::new(tr.artist)
                            .size(11.0)
                            .color(Color::from_rgba8(152, 152, 160, 255))
                            .ellipsis(),
                    ),
            );
        }

        let active_cover = |size: f32, radius: f32| {
            let imgs: Vec<Element> = tracks
                .iter()
                .map(|t| {
                    Image::from_decoded(t.art.clone())
                        .width(Length::Px(size))
                        .height(Length::Px(size))
                        .radius(radius)
                        .into_element()
                })
                .collect();
            Container::new()
                .width(Length::Px(size))
                .height(Length::Px(size))
                .child(Switcher::new(current_track, imgs).flex_grow(1.0))
        };

        let pi_prev_show = play_index.clone();
        let pi_next_1 = play_index.clone();
        let pi_next_2 = play_index.clone();

        let now_playing_stage = Container::column()
            .flex_grow(1.0)
            .padding(Edges::xy(32.0, 18.0))
            .gap(12.0)
            // Top bar: Back <, Forward >, Centered search pill, Close X
            .child(
                Row::new()
                    .height(Length::Px(36.0))
                    .align(AlignItems::Center)
                    .drag_region()
                    .child(
                        Button::icon_only(ICO_CHEVRON_LEFT, 16.0)
                            .ghost()
                            .tooltip("Back to Browse")
                            .on_click(move || active_nav.set(2)),
                    )
                    .child(
                        Button::icon_only(ICO_CHEVRON_RIGHT, 16.0)
                            .ghost()
                            .on_click(move || active_nav.set(4)),
                    )
                    .child(Spacer::flex())
                    .child(
                        Container::new()
                            .bind_material(search_stage_mat)
                            .radius(8.0)
                            .child(
                                TextInput::new(search_query)
                                    .placeholder("What are you looking for?...")
                                    .width(Length::Px(260.0))
                                    .material(Material::solid(Color::TRANSPARENT)),
                            ),
                    )
                    .child(Spacer::flex())
                    .child(
                        Button::icon_only(ICO_CLOSE, 15.0)
                            .ghost()
                            .tooltip("Close Stage")
                            .on_click(move || active_nav.set(2)),
                    ),
            )
            // Breathing space where the single unbroken audio visualizer wave flows cleanly
            .child(Container::new().height(Length::Px(132.0)))
            // Central 3-column stage: PREVIOUS TRACK | LIVE NOW ART | UPCOMING QUEUE
            .child(
                Row::new()
                    .padding(Edges::xy(28.0, 0.0))
                    .gap(36.0)
                    .align(AlignItems::Center)
                    // Left: Previous Track
                    .child(
                        Container::column()
                            .width(Length::Px(0.0))
                            .flex_grow(1.0)
                            .gap(6.0)
                            .padding(Edges::all(10.0))
                            .radius(10.0)
                            .hover_fill(Color::from_rgba8(255, 255, 255, 14))
                            .on_click(move || pi_prev_show((current_track.get() + 11) % 12))
                            .child(
                                Row::new()
                                    .gap(4.0)
                                    .align(AlignItems::Center)
                                    .child(
                                        Text::new("PREVIOUS TRACK")
                                            .size(11.0)
                                            .weight(FontWeight::Bold)
                                            .color(Color::from_rgba8(225, 225, 232, 230)),
                                    )
                                    .child(
                                        Icon::svg(ICO_CHEVRON_RIGHT, 12.0)
                                            .color(Color::from_rgba8(225, 225, 232, 200)),
                                    ),
                            )
                            .child(
                                Text::reactive({
                                    let tr = tracks.clone();
                                    move || {
                                        tr[(current_track.get() + 11) % tr.len()].album.to_string()
                                    }
                                })
                                .size(11.0)
                                .color(Color::from_rgba8(152, 152, 162, 220)),
                            )
                            .child(
                                Text::reactive({
                                    let tr = tracks.clone();
                                    move || {
                                        tr[(current_track.get() + 11) % tr.len()].title.to_string()
                                    }
                                })
                                .size(13.5)
                                .weight(FontWeight::Bold)
                                .color(Color::from_rgba8(255, 255, 255, 245)),
                            )
                            .child(
                                Text::reactive({
                                    let tr = tracks.clone();
                                    move || {
                                        format!(
                                            "{} • Click to replay previous",
                                            tr[(current_track.get() + 11) % tr.len()].artist
                                        )
                                    }
                                })
                                .size(11.5)
                                .color(Color::from_rgba8(156, 156, 166, 225)),
                            ),
                    )
                    // Center: LIVE NOW + Bleed-backed Active Artwork
                    .child(
                        Column::new()
                            .width(Length::Px(216.0))
                            .align(AlignItems::Center)
                            .gap(8.0)
                            .child(
                                Text::new("LIVE NOW:")
                                    .size(11.0)
                                    .weight(FontWeight::Bold)
                                    .color(Color::from_rgba8(235, 235, 242, 235)),
                            )
                            .child(
                                Container::new()
                                    .radius(8.0)
                                    .shadow(Color::from_rgba8(0, 0, 0, 170), 28.0, 12.0)
                                    .child(active_cover(176.0, 8.0)),
                            )
                            .child(
                                Text::reactive({
                                    let tr = tracks.clone();
                                    move || tr[current_track.get() % tr.len()].title.to_string()
                                })
                                .size(15.0)
                                .weight(FontWeight::Bold)
                                .color(Color::from_rgba8(255, 255, 255, 250)),
                            )
                            .child(
                                Text::reactive({
                                    let tr = tracks.clone();
                                    move || {
                                        let t = &tr[current_track.get() % tr.len()];
                                        format!("{} • {}", t.artist, format_mm_ss(t.duration_secs))
                                    }
                                })
                                .size(11.5)
                                .color(Color::from_rgba8(168, 168, 178, 230)),
                            ),
                    )
                    // Right: Clean Upcoming Queue
                    .child(
                        Column::new()
                            .width(Length::Px(0.0))
                            .flex_grow(1.0)
                            .gap(10.0)
                            .child(
                                Row::new()
                                    .gap(4.0)
                                    .align(AlignItems::Center)
                                    .child(
                                        Text::new("UPCOMING QUEUE")
                                            .size(11.0)
                                            .weight(FontWeight::Bold)
                                            .color(Color::from_rgba8(225, 225, 232, 230)),
                                    )
                                    .child(
                                        Icon::svg(ICO_CHEVRON_RIGHT, 12.0)
                                            .color(Color::from_rgba8(225, 225, 232, 200)),
                                    ),
                            )
                            .child(
                                Container::column()
                                    .gap(3.0)
                                    .padding(Edges::all(8.0))
                                    .radius(8.0)
                                    .hover_fill(Color::from_rgba8(255, 255, 255, 14))
                                    .on_click(move || pi_next_1((current_track.get() + 1) % 12))
                                    .child(
                                        Text::new("NEXT UP")
                                            .size(10.0)
                                            .weight(FontWeight::Semibold)
                                            .color(Color::from_rgba8(150, 150, 160, 215)),
                                    )
                                    .child(
                                        Text::reactive({
                                            let tr = tracks.clone();
                                            move || {
                                                tr[(current_track.get() + 1) % tr.len()]
                                                    .title
                                                    .to_string()
                                            }
                                        })
                                        .size(13.0)
                                        .weight(FontWeight::Bold)
                                        .color(Color::from_rgba8(255, 255, 255, 245)),
                                    )
                                    .child(
                                        Text::reactive({
                                            let tr = tracks.clone();
                                            move || {
                                                tr[(current_track.get() + 1) % tr.len()]
                                                    .artist
                                                    .to_string()
                                            }
                                        })
                                        .size(11.5)
                                        .color(Color::from_rgba8(156, 156, 166, 225)),
                                    ),
                            )
                            .child(
                                Container::column()
                                    .gap(3.0)
                                    .padding(Edges::all(8.0))
                                    .radius(8.0)
                                    .hover_fill(Color::from_rgba8(255, 255, 255, 14))
                                    .on_click(move || pi_next_2((current_track.get() + 2) % 12))
                                    .child(
                                        Text::new("IN QUEUE")
                                            .size(10.0)
                                            .weight(FontWeight::Semibold)
                                            .color(Color::from_rgba8(150, 150, 160, 215)),
                                    )
                                    .child(
                                        Text::reactive({
                                            let tr = tracks.clone();
                                            move || {
                                                tr[(current_track.get() + 2) % tr.len()]
                                                    .title
                                                    .to_string()
                                            }
                                        })
                                        .size(13.0)
                                        .weight(FontWeight::Bold)
                                        .color(Color::from_rgba8(255, 255, 255, 245)),
                                    )
                                    .child(
                                        Text::reactive({
                                            let tr = tracks.clone();
                                            move || {
                                                tr[(current_track.get() + 2) % tr.len()]
                                                    .artist
                                                    .to_string()
                                            }
                                        })
                                        .size(11.5)
                                        .color(Color::from_rgba8(156, 156, 166, 225)),
                                    ),
                            ),
                    ),
            )
            .child(Spacer::flex())
            .child(
                Text::new("FEATURED SHOWS")
                    .size(11.5)
                    .weight(FontWeight::Bold)
                    .color(Color::from_rgba8(220, 220, 228, 220)),
            )
            .child(featured_shows)
            .child(Container::new().height(Length::Px(72.0)));

        // ── 4. Search, Home, and 100k Library Views ─────────────────────────────
        let search_view = Column::new()
            .flex_grow(1.0)
            .padding(Edges::all(28.0))
            .gap(16.0)
            .child(
                Text::new("Search")
                    .size(30.0)
                    .weight(FontWeight::Bold)
                    .tone(TextTone::Primary),
            )
            .child(
                TextInput::new(search_query)
                    .placeholder("Artists, Songs, Lyrics, and More...")
                    .width(Length::Px(380.0)),
            )
            .child(
                VirtualizedList::new(tracks.len(), 52.0, {
                    let tr = tracks.clone();
                    move |idx| {
                        let t = &tr[idx % tr.len()];
                        VirtualRow {
                            title: t.title.into(),
                            subtitle: format!("{} • {}", t.artist, t.album),
                            badge: format_mm_ss(t.duration_secs),
                        }
                    }
                })
                .selected(current_track)
                .on_select({
                    let pi = play_index.clone();
                    move |i| {
                        pi(i);
                        active_nav.set(3);
                    }
                }),
            );

        let home_view = Column::new()
            .flex_grow(1.0)
            .padding(Edges::all(28.0))
            .gap(14.0)
            .child(
                Text::new("Home")
                    .size(30.0)
                    .weight(FontWeight::Bold)
                    .tone(TextTone::Primary),
            )
            .child(
                Text::new("Top Picks For You")
                    .size(17.0)
                    .weight(FontWeight::Bold)
                    .tone(TextTone::Primary),
            )
            .child(
                VirtualizedList::new(12, 54.0, {
                    let tr = tracks.clone();
                    move |idx| VirtualRow {
                        title: tr[idx % 12].title.into(),
                        subtitle: format!("{} • {}", tr[idx % 12].artist, tr[idx % 12].album),
                        badge: format_mm_ss(tr[idx % 12].duration_secs),
                    }
                })
                .selected(current_track)
                .on_select({
                    let pi = play_index.clone();
                    move |i| {
                        pi(i);
                        active_nav.set(3);
                    }
                }),
            );

        let library_100k = Column::new()
            .flex_grow(1.0)
            .padding(Edges::all(28.0))
            .gap(14.0)
            .child(
                Text::new("Songs • 100,000 Tracks")
                    .size(30.0)
                    .weight(FontWeight::Bold)
                    .tone(TextTone::Primary),
            )
            .child(
                VirtualizedList::new(100_000, 48.0, {
                    let tr = tracks.clone();
                    move |idx| {
                        let t = &tr[idx % tr.len()];
                        VirtualRow {
                            title: format!("{:05}  {}", idx + 1, t.title),
                            subtitle: format!("{} • {}", t.artist, t.album),
                            badge: format_mm_ss(t.duration_secs),
                        }
                    }
                })
                .selected(current_track)
                .on_select({
                    let pi = play_index.clone();
                    move |i| {
                        pi(i);
                        active_nav.set(3);
                    }
                }),
            );

        let content_switcher = Switcher::new(
            active_nav,
            vec![
                search_view.into_element(),
                home_view.into_element(),
                new_view.into_element(),
                now_playing_stage.into_element(),
                library_100k.into_element(),
            ],
        );

        // ── 5. Floating Queue Popover ───────────────────────────────────────────
        let queue_popover = Show::new(queue_open, {
            let tr = tracks.clone();
            let pi = play_index.clone();
            move || {
                let mut rows = Column::new().gap(4.0);
                let cur = current_track.try_get().unwrap_or(0);
                for step in 0..5 {
                    let idx = (cur + step) % tr.len();
                    let pi_cl = pi.clone();
                    rows = rows.child(
                        Container::row()
                            .height(Length::Px(40.0))
                            .padding(Edges::xy(8.0, 2.0))
                            .gap(10.0)
                            .radius(8.0)
                            .align(AlignItems::Center)
                            .hover_fill(hover_pill_fill)
                            .on_click(move || pi_cl(idx))
                            .child(
                                Image::from_decoded(tr[idx].art.clone())
                                    .width(Length::Px(30.0))
                                    .height(Length::Px(30.0))
                                    .radius(5.0),
                            )
                            .child(
                                Column::new()
                                    .flex_grow(1.0)
                                    .child(
                                        Text::new(tr[idx].title)
                                            .size(12.5)
                                            .weight(FontWeight::Semibold)
                                            .tone(TextTone::Primary),
                                    )
                                    .child(
                                        Text::new(tr[idx].artist)
                                            .size(11.0)
                                            .tone(TextTone::Secondary),
                                    ),
                            ),
                    );
                }

                Container::column()
                    .absolute()
                    .inset(Inset {
                        left: Length::Auto,
                        top: Length::Auto,
                        right: Length::Px(120.0),
                        bottom: Length::Px(84.0),
                    })
                    .width(Length::Px(290.0))
                    .padding(Edges::all(14.0))
                    .gap(8.0)
                    .radius(14.0)
                    .bind_material(popover_mat)
                    .child(
                        Text::new("Playing Next")
                            .size(13.5)
                            .weight(FontWeight::Bold)
                            .tone(TextTone::Primary),
                    )
                    .child(rows)
                    .into_element()
            }
        });

        // ── 6. Centered Liquid Glass Player Capsule ─────────────────────────────
        let play_pause_btn = Stack::new()
            .width(Length::Px(32.0))
            .height(Length::Px(32.0))
            .child(Show::new(is_paused.signal(), {
                let tp = toggle_play.clone();
                move || {
                    Button::icon_only(ICO_PLAY, 17.0)
                        .ghost()
                        .tooltip("Play")
                        .on_click({
                            let tp2 = tp.clone();
                            move || tp2()
                        })
                        .into_element()
                }
            }))
            .child(Show::new(is_playing, {
                let tp = toggle_play.clone();
                move || {
                    Button::icon_only(ICO_PAUSE, 17.0)
                        .ghost()
                        .tooltip("Pause")
                        .on_click({
                            let tp2 = tp.clone();
                            move || tp2()
                        })
                        .into_element()
                }
            }));

        let pi_prev = play_index.clone();
        let pi_next = play_index.clone();
        let player_capsule = Container::row()
            .absolute()
            .inset(Inset {
                left: Length::Px(16.0),
                right: Length::Px(16.0),
                top: Length::Auto,
                bottom: Length::Px(18.0),
            })
            .height(Length::Px(56.0))
            .padding(Edges::xy(16.0, 6.0))
            .gap(10.0)
            .radius(28.0)
            .align(AlignItems::Center)
            .bind_material(dock_mat)
            .child(Show::new(show_secondary.signal(), move || {
                Button::icon_only(ICO_SHUFFLE, 14.0)
                    .ghost()
                    .tooltip("Shuffle")
                    .on_click(move || shuffle_on.update(|v| *v = !*v))
                    .into_element()
            }))
            .child(
                Button::icon_only(ICO_PREV, 15.0)
                    .ghost()
                    .on_click(move || pi_prev((current_track.get() + 11) % 12)),
            )
            .child(play_pause_btn)
            .child(
                Button::icon_only(ICO_NEXT, 15.0)
                    .ghost()
                    .on_click(move || pi_next((current_track.get() + 1) % 12)),
            )
            .child(Show::new(show_secondary.signal(), move || {
                Button::icon_only(ICO_REPEAT, 14.0)
                    .ghost()
                    .tooltip("Repeat")
                    .on_click(move || repeat_on.update(|v| *v = !*v))
                    .into_element()
            }))
            .child(Spacer::flex())
            .child(
                Container::row()
                    .width(Length::Px(0.0))
                    .flex_grow(1.0)
                    .max_width(Length::Px(290.0))
                    .gap(10.0)
                    .align(AlignItems::Center)
                    .on_click(move || active_nav.set(3))
                    .child(active_cover(36.0, 6.0))
                    .child(
                        Column::new()
                            .width(Length::Px(0.0))
                            .flex_grow(1.0)
                            .gap(3.0)
                            .child(
                                Text::reactive({
                                    let tr = tracks.clone();
                                    move || tr[current_track.get() % tr.len()].title.to_string()
                                })
                                .size(12.5)
                                .weight(FontWeight::Semibold)
                                .tone(TextTone::Primary)
                                .ellipsis(),
                            )
                            .child(
                                Text::reactive({
                                    let tr = tracks.clone();
                                    move || {
                                        let t = &tr[current_track.get() % tr.len()];
                                        let cur = (progress.get() * t.duration_secs as f32) as u32;
                                        format!(
                                            "{} — {} • {}",
                                            t.artist,
                                            t.album,
                                            format_mm_ss(cur)
                                        )
                                    }
                                })
                                .size(10.5)
                                .tone(TextTone::Secondary)
                                .ellipsis(),
                            )
                            .child(
                                ProgressBar::new(progress)
                                    .width(Length::Percent(1.0))
                                    .height(3.0)
                                    .color(apple_pink),
                            ),
                    ),
            )
            .child(Spacer::flex())
            .child(
                Button::icon_only(ICO_WAVE, 14.0)
                    .ghost()
                    .tooltip("Now Playing Beats Stage")
                    .on_click(move || active_nav.set(3)),
            )
            .child(
                Button::icon_only(ICO_QUEUE, 15.0)
                    .ghost()
                    .tooltip("Playing Next Queue")
                    .on_click(move || queue_open.update(|v| *v = !*v)),
            )
            .child(Show::new(show_secondary.signal(), move || {
                Row::new()
                    .gap(6.0)
                    .align(AlignItems::Center)
                    .child(Icon::svg(ICO_VOLUME, 15.0).tone(TextTone::Secondary))
                    .child(Slider::new(volume, 0.0, 1.0).width(74.0))
                    .into_element()
            }));

        let right_canvas = Stack::new()
            .flex_grow(1.0)
            .bind_material(canvas_mat)
            .child(Show::new(is_compact.signal(), move || {
                Container::row()
                    .height(Length::Px(36.0))
                    .padding(Edges::xy(12.0, 6.0))
                    .child(
                        Button::icon_only(ICO_CHEVRON_LEFT, 14.0)
                            .ghost()
                            .tooltip("Toggle Sidebar")
                            .on_click(move || sidebar_open.update(|v| *v = !*v)),
                    )
                    .into_element()
            }))
            .child(content_switcher)
            .child(queue_popover)
            .child(player_capsule);

        // Full-window Stack: Album-art bleed + waveform sits at Layer 0 behind Frosted Sidebar AND Stage
        Stack::new()
            .flex_grow(1.0)
            .child(
                CustomGpuSurface::new(BeatsStagePass {
                    playing: is_playing,
                    active_nav,
                    viewport,
                    is_compact: is_compact.signal(),
                    track_idx: current_track,
                    color_pairs: color_pairs.clone(),
                    pipeline: None,
                    buf: None,
                    bg: None,
                    format: None,
                })
                .width(Length::Percent(1.0))
                .height(Length::Percent(1.0))
                .radius(0.0)
                .animate_when(bg_animating.signal())
                .redraw_on(active_nav)
                .redraw_on(current_track),
            )
            .child(
                Container::row()
                    .absolute()
                    .inset(Inset::all(Length::Px(0.0)))
                    .child(
                        Switcher::new(
                            sidebar_mode.signal(),
                            vec![sidebar.into_element(), compact_sidebar.into_element()],
                        )
                        .flex_grow(0.0)
                        .height(Length::Percent(1.0)),
                    )
                    .child(
                        Container::new()
                            .width(Length::Px(1.0))
                            .height(Length::Percent(1.0))
                            .bind_material(divider_mat),
                    )
                    .child(right_canvas),
            )
            .into_element()
    })
    .with_theme(theme_mode)
    .with_viewport(viewport)
    .show_fps(true);

    let app = app.add_window(WindowSpec {
        title: "Cadmium Music".into(),
        width: 1280.0,
        height: 800.0,
        min_width: Some(520.0),
        min_height: Some(420.0),
        borderless: true,
        clear_color: MacTheme::dark().window_bg,
        delegate: Box::new(window),
    });

    if let Err(err) = app.run() {
        eprintln!("Cadmium Music exited with error: {err}");
    }
}
