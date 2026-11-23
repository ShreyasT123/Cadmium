use std::path::Path;

use steelui::{
    App, Color, CornerRadii, CursorStyle, DecodedImage, FontWeight, FrameContext, GlassQuad,
    GpuContext, ImageQuad, MacTheme, Rect, SdfQuad, Spring, TextBlock, TextEngine, TextStyle,
    TextureHandle, WindowCommand, WindowDelegate, WindowSpec, cover_uv_rect, load_image_file,
    rasterize_svg, snap_rect_to_pixels,
};

const LINKEDIN_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 93.06 93.06"><path fill="#ffffff" d="M11.185.08C5.004.08.001 5.092 0 11.259c0 6.173 5.003 11.184 11.186 11.184 6.166 0 11.176-5.011 11.176-11.184C22.362 5.091 17.351.08 11.185.08zM1.538 30.926h19.287V92.98H1.538zM69.925 29.383c-9.382 0-15.673 5.144-18.248 10.022h-.258v-8.479H32.92v62.053h19.27V62.281c0-8.093 1.541-15.932 11.575-15.932 9.89 0 10.022 9.256 10.022 16.451v30.178H93.06V58.942c0-16.707-3.605-29.559-23.135-29.559z"/></svg>"##;

const FRONTENDLY_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 384 212" fill="none"><path fill="#cf84fc" d="M100.5 180.25.5 123V97l100-57.25 18.25 37.5L36 118.75v-17.5l82.75 41.5-18.25 37.5Zm14.106 31.25L219.106 0h46.5l-104 211.5h-47Zm168.84-31.25-18.25-37.5 82.75-41.5v17.5l-82.75-41.5 18.25-37.5 100 57.25v26l-100 57.25Z"/></svg>"##;

struct LiquidGlassScene {
    bg_tex: Option<TextureHandle>,
    bg_size: (u32, u32),
    linkedin_tex: Option<TextureHandle>,
    frontendly_tex: Option<TextureHandle>,

    // Liquid Glass Menu state
    expanded: bool,
    expand_spring: Spring,
    offset_x: Spring,
    offset_y: Spring,
    dragging: bool,
    moved_during_drag: bool,
    drag_start_cursor: (f32, f32),
    drag_start_offset: (f32, f32),
    card_rect: Rect,
    card_hovered: bool,

    // "Generate" Conic Glass Pill Button state
    gen_rect: Rect,
    gen_hovered: bool,
    gen_pressed: bool,
    gen_hover_spring: Spring,
    gen_press_spring: Spring,
    gen_count: usize,

    hovered_traffic: bool,
    heading: Option<TextBlock>,
    gen_label: Option<TextBlock>,
    description: Option<TextBlock>,
    traffic_x: Option<TextBlock>,
    traffic_min: Option<TextBlock>,
    traffic_plus: Option<TextBlock>,
}

impl LiquidGlassScene {
    fn new() -> Self {
        Self {
            bg_tex: None,
            bg_size: (1, 1),
            linkedin_tex: None,
            frontendly_tex: None,
            expanded: false,
            expand_spring: Spring::new(0.0, 260.0, 18.0),
            offset_x: Spring::new(0.0, 180.0, 11.0),
            offset_y: Spring::new(0.0, 180.0, 11.0),
            dragging: false,
            moved_during_drag: false,
            drag_start_cursor: (0.0, 0.0),
            drag_start_offset: (0.0, 0.0),
            card_rect: Rect::default(),
            card_hovered: false,
            gen_rect: Rect::default(),
            gen_hovered: false,
            gen_pressed: false,
            gen_hover_spring: Spring::new(0.0, 280.0, 26.0),
            gen_press_spring: Spring::new(0.0, 360.0, 24.0),
            gen_count: 0,
            hovered_traffic: false,
            heading: None,
            gen_label: None,
            description: None,
            traffic_x: None,
            traffic_min: None,
            traffic_plus: None,
        }
    }
}

fn hit_test(r: Rect, x: f32, y: f32) -> bool {
    x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height
}

fn load_bg_or_fallback() -> DecodedImage {
    for candidate in [
        "bg.png",
        "assets/bg.png",
        "apps/dogfood/assets/bg.png",
        "examples/liquid_glass/bg.png",
    ] {
        if Path::new(candidate).exists() {
            match load_image_file(candidate) {
                Ok(img) => {
                    tracing::info!(
                        "Loaded background image from {candidate} ({}x{})",
                        img.width,
                        img.height
                    );
                    return img;
                }
                Err(e) => tracing::warn!("Failed to decode {candidate}: {e}"),
            }
        }
    }

    let (w, h) = (512u32, 384u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let wave = ((u * 8.0 + v * 5.0).sin() * 0.5 + 0.5) * 0.45
                + ((u * 14.0 - v * 9.0).cos() * 0.5 + 0.5) * 0.35;
            let r = (45.0 + 185.0 * (1.0 - v) * 0.7 + 90.0 * wave).min(255.0) as u8;
            let g = (78.0 + 120.0 * u * (1.0 - v * 0.5) + 75.0 * wave).min(255.0) as u8;
            let b = (110.0 + 130.0 * v + 60.0 * (1.0 - u)).min(255.0) as u8;
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    DecodedImage {
        width: w,
        height: h,
        rgba,
    }
}

impl WindowDelegate for LiquidGlassScene {
    fn init(&mut self, text: &mut TextEngine) {
        let white = Color::from_rgba8(255, 255, 255, 255);
        self.heading = Some(TextBlock::new(
            text,
            "Liquid Glass Menu",
            TextStyle {
                font_size: 46.0,
                line_height: 54.0,
                color: white,
                weight: FontWeight::Bold,
            },
        ));
        self.gen_label = Some(TextBlock::new(
            text,
            "Generate",
            TextStyle {
                font_size: 22.0,
                line_height: 26.0,
                color: Color::from_rgba8(245, 247, 252, 255),
                weight: FontWeight::Medium,
            },
        ));
        self.description = Some(TextBlock::new(
            text,
            "Click menu to expand, drag to move, hover Generate pill",
            TextStyle {
                font_size: 16.0,
                line_height: 22.0,
                color: Color::from_rgba8(255, 255, 255, 230),
                weight: FontWeight::Medium,
            },
        ));

        let theme = MacTheme::light();
        let gs = TextStyle {
            font_size: 9.5,
            line_height: 11.0,
            color: theme.traffic_glyph,
            weight: FontWeight::Bold,
        };
        self.traffic_x = Some(TextBlock::new(text, "×", gs));
        self.traffic_min = Some(TextBlock::new(text, "−", gs));
        self.traffic_plus = Some(TextBlock::new(text, "+", gs));
    }

    fn init_gpu(&mut self, gpu: &mut GpuContext) {
        let bg = load_bg_or_fallback();
        self.bg_size = (bg.width, bg.height);
        self.bg_tex = Some(gpu.upload_image(&bg));

        if let Ok(li) = rasterize_svg(LINKEDIN_SVG, 160, 160) {
            self.linkedin_tex = Some(gpu.upload_image(&li));
        }
        if let Ok(fe) = rasterize_svg(FRONTENDLY_SVG, 200, 120) {
            self.frontendly_tex = Some(gpu.upload_image(&fe));
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut active = self.expand_spring.step(dt);
        active |= self.gen_hover_spring.step(dt);
        active |= self.gen_press_spring.step(dt);
        if !self.dragging {
            active |= self.offset_x.step(dt);
            active |= self.offset_y.step(dt);
        }
        active
    }

    fn cursor_style(&self) -> CursorStyle {
        if self.dragging {
            CursorStyle::Grabbing
        } else if self.gen_hovered {
            CursorStyle::Pointer
        } else if self.card_hovered {
            CursorStyle::Grab
        } else {
            CursorStyle::Default
        }
    }

    fn is_drag_region(&self, x: f32, y: f32) -> bool {
        y < 48.0 && !hit_test(self.card_rect, x, y) && !hit_test(self.gen_rect, x, y)
    }

    fn on_pointer_move(&mut self, x: f32, y: f32) -> bool {
        if self.dragging {
            let dx = x - self.drag_start_cursor.0;
            let dy = y - self.drag_start_cursor.1;
            if dx * dx + dy * dy > 25.0 {
                self.moved_during_drag = true;
            }
            self.offset_x.snap_to(self.drag_start_offset.0 + dx);
            self.offset_y.snap_to(self.drag_start_offset.1 + dy);
            return true;
        }

        let in_traffic = x >= 14.0 && x <= 76.0 && y >= 14.0 && y <= 36.0;
        let in_gen = hit_test(self.gen_rect, x, y);
        let in_card = hit_test(self.card_rect, x, y);

        let mut changed = in_traffic != self.hovered_traffic
            || in_gen != self.gen_hovered
            || in_card != self.card_hovered;

        self.hovered_traffic = in_traffic;
        self.card_hovered = in_card;
        if in_gen != self.gen_hovered {
            self.gen_hovered = in_gen;
            self.gen_hover_spring
                .set_target(if in_gen { 1.0 } else { 0.0 });
            changed = true;
        }

        changed
    }

    fn on_pointer_down(&mut self, x: f32, y: f32) -> (bool, WindowCommand) {
        for (tx, cmd) in [
            (20.0, WindowCommand::Close),
            (40.0, WindowCommand::Minimize),
            (60.0, WindowCommand::ToggleMaximize),
        ] {
            if x >= tx && x <= tx + 12.0 && y >= 20.0 && y <= 32.0 {
                return (false, cmd);
            }
        }

        if hit_test(self.gen_rect, x, y) {
            self.gen_pressed = true;
            self.gen_press_spring.set_target(1.0);
            return (true, WindowCommand::None);
        }

        if hit_test(self.card_rect, x, y) {
            self.dragging = true;
            self.moved_during_drag = false;
            self.drag_start_cursor = (x, y);
            self.drag_start_offset = (self.offset_x.value, self.offset_y.value);
            return (true, WindowCommand::None);
        }

        (false, WindowCommand::None)
    }

    fn on_pointer_up(&mut self) -> bool {
        let mut redraw = false;

        if self.gen_pressed {
            self.gen_pressed = false;
            self.gen_press_spring.set_target(0.0);
            if self.gen_hovered {
                self.gen_count += 1;
                self.expanded = !self.expanded;
                self.expand_spring
                    .set_target(if self.expanded { 1.0 } else { 0.0 });
            }
            redraw = true;
        }

        if self.dragging {
            self.dragging = false;
            if !self.moved_during_drag {
                self.expanded = !self.expanded;
                self.expand_spring
                    .set_target(if self.expanded { 1.0 } else { 0.0 });
            }
            self.offset_x.set_target(0.0);
            self.offset_y.set_target(0.0);
            redraw = true;
        }

        redraw
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) {
        let scale = ctx.scale;
        let theme = MacTheme::light();

        // 1. Full-bleed background photo (`bg.png` with `cover` aspect ratio)
        if let Some(bg_tex) = self.bg_tex {
            let uv_rect = cover_uv_rect(self.bg_size.0, self.bg_size.1, ctx.width, ctx.height);
            ctx.push_image(ImageQuad {
                rect: Rect::new(0.0, 0.0, ctx.width * scale, ctx.height * scale),
                uv_rect,
                radii: CornerRadii::ZERO,
                tint: Color::rgba(1.0, 1.0, 1.0, 1.0),
                texture: bg_tex,
            });
        }

        // Traffic lights
        for (tx, color, glyph_opt) in [
            (20.0, theme.traffic_close, &self.traffic_x),
            (40.0, theme.traffic_minimize, &self.traffic_min),
            (60.0, theme.traffic_zoom, &self.traffic_plus),
        ] {
            ctx.push_sdf(SdfQuad {
                rect: Rect::new(tx, 20.0, 12.0, 12.0).scale(scale),
                radii: CornerRadii::all(6.0).scale(scale),
                fill: color,
                border_color: theme.traffic_border,
                border_width: 1.0,
                ..Default::default()
            });
            if self.hovered_traffic {
                if let Some(g) = glyph_opt {
                    ctx.push_text(g, tx + 2.8, 20.0);
                }
            }
        }

        if let Some(h) = &self.heading {
            ctx.push_text(h, (ctx.width - 404.0) * 0.5, 24.0);
        }
        if let Some(d) = &self.description {
            ctx.push_text(d, (ctx.width - 440.0) * 0.5, ctx.height - 42.0);
        }

        // 2. Crystal-Clear Liquid Glass Menu Card (CodePen --bend + --face + --edge)
        let t = self.expand_spring.value;
        let card_w = lerp(260.0, 440.0, t);
        let card_h = lerp(230.0, 380.0, t);
        let pad = lerp(28.0, 38.0, t);
        let gap = lerp(16.0, 28.0, t);
        let radius = 32.0;
        let icon_size = lerp(64.0, 112.0, t);
        let icon_radius = lerp(16.0, 28.0, t);
        let icon_pad = lerp(12.0, 20.0, t);

        let card_x = (ctx.width - card_w) * 0.5 + self.offset_x.value;
        let card_y = (ctx.height - card_h) * 0.5 - 38.0 + self.offset_y.value;
        self.card_rect = Rect::new(card_x, card_y, card_w, card_h);

        ctx.push_sdf(SdfQuad {
            rect: snap_rect_to_pixels(self.card_rect, scale),
            radii: CornerRadii::all(radius).scale(scale),
            fill: Color::TRANSPARENT,
            shadow_color: Color::from_rgba8(0, 0, 0, 52),
            shadow_offset: [0.0, 8.0 * scale],
            shadow_blur: 18.0 * scale,
            ..Default::default()
        });

        ctx.push_glass(GlassQuad {
            rect: snap_rect_to_pixels(self.card_rect, scale),
            radii: CornerRadii::all(radius).scale(scale),
            tint: Color::TRANSPARENT,
            saturation: 1.08,
            specular: 0.95,
            frost: 0.20,
            liquid_ripple: 1.0, // Organic glass wave
            ..Default::default()
        });

        let li_rect = Rect::new(card_x + pad, card_y + pad, icon_size, icon_size);
        ctx.push_sdf(SdfQuad {
            rect: snap_rect_to_pixels(li_rect, scale),
            radii: CornerRadii::all(icon_radius).scale(scale),
            fill: Color::from_rgba8(26, 118, 173, 255),
            fill_end: Some(Color::from_rgba8(40, 142, 201, 255)),
            gradient_dir: [1.0, 0.0],
            border_color: Color::from_rgba8(255, 255, 255, 125),
            border_width: 1.5,
            ..Default::default()
        });
        if let Some(tex) = self.linkedin_tex {
            ctx.push_image(ImageQuad::new(
                tex,
                Rect::new(
                    li_rect.x + icon_pad,
                    li_rect.y + icon_pad,
                    icon_size - icon_pad * 2.0,
                    icon_size - icon_pad * 2.0,
                )
                .scale(scale),
            ));
        }

        let fe_rect = Rect::new(
            li_rect.x + icon_size + gap,
            card_y + pad,
            icon_size,
            icon_size,
        );
        ctx.push_sdf(SdfQuad {
            rect: snap_rect_to_pixels(fe_rect, scale),
            radii: CornerRadii::all(icon_radius).scale(scale),
            fill: Color::from_rgba8(38, 38, 38, 255),
            fill_end: Some(Color::from_rgba8(52, 52, 52, 255)),
            gradient_dir: [1.0, 0.0],
            border_color: Color::from_rgba8(255, 255, 255, 125),
            border_width: 1.5,
            ..Default::default()
        });
        if let Some(tex) = self.frontendly_tex {
            let iw = icon_size - icon_pad * 1.6;
            let ih = iw * 0.60;
            ctx.push_image(ImageQuad::new(
                tex,
                Rect::new(
                    fe_rect.x + (icon_size - iw) * 0.5,
                    fe_rect.y + (icon_size - ih) * 0.5,
                    iw,
                    ih,
                )
                .scale(scale),
            ));
        }

        // 3. "Generate" Conic Liquid Glass Pill Button (with rotating conic border + diagonal sweep!)
        let hv = self.gen_hover_spring.value;
        let pr = self.gen_press_spring.value;
        let btn_scale = lerp(1.0, 0.975, hv) * lerp(1.0, 0.96, pr);
        let btn_w = 196.0 * btn_scale;
        let btn_h = 62.0 * btn_scale;
        let btn_x = (ctx.width - btn_w) * 0.5;
        let btn_y = (ctx.height - 138.0) + (62.0 - btn_h) * 0.5 + pr * 2.5;
        self.gen_rect = Rect::new(btn_x, btn_y, btn_w, btn_h);

        // Dynamic shadow: pulls closer and sharpens on hover
        let shadow_y = lerp(8.0, 4.0, hv) + pr * 2.0;
        let shadow_blur = lerp(18.0, 10.0, hv);
        ctx.push_sdf(SdfQuad {
            rect: snap_rect_to_pixels(self.gen_rect, scale),
            radii: CornerRadii::all(btn_h * 0.5).scale(scale),
            fill: Color::TRANSPARENT,
            shadow_color: Color::from_rgba8(
                0,
                0,
                0,
                (lerp(68.0, 82.0, hv) * lerp(1.0, 0.75, pr)) as u8,
            ),
            shadow_offset: [0.0, shadow_y * scale],
            shadow_blur: shadow_blur * scale,
            ..Default::default()
        });

        // Conic glass pill (--angle-1: -75deg -> -125deg on hover, sheen sweeps diagonally)
        let rim_angle = lerp(-75.0_f32.to_radians(), -125.0_f32.to_radians(), hv) + pr * 0.5;
        let sheen_progress = (hv * 0.50 + pr * 0.50).clamp(0.0, 1.0);
        ctx.push_glass(GlassQuad {
            rect: snap_rect_to_pixels(self.gen_rect, scale),
            radii: CornerRadii::all(btn_h * 0.5).scale(scale),
            tint: Color::from_rgba8(255, 255, 255, 24),
            saturation: 1.10,
            specular: 1.0,
            frost: lerp(0.18, 0.05, hv),
            liquid_ripple: 0.25,
            rim_angle,
            sheen_progress,
            conic_strength: 1.0,
            ..Default::default()
        });

        if let Some(gl) = &mut self.gen_label {
            let te = ctx.text_engine();
            if self.gen_count > 0 && self.expanded {
                gl.set_text(te, "Collapse");
            } else {
                gl.set_text(te, "Generate");
            }
            let (tw, th) = gl.measure(te, None);
            ctx.push_text(
                gl,
                btn_x + (btn_w - tw) * 0.5,
                btn_y + (btn_h - th) * 0.5 - 1.0,
            );
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,wgpu=warn")
        .init();

    let app = App::new().add_window(WindowSpec {
        title: "Liquid Glass Menu".into(),
        width: 1020.0,
        height: 760.0,
        borderless: true,
        clear_color: Color::from_rgba8(24, 28, 36, 255),
        delegate: Box::new(LiquidGlassScene::new()),
    });

    if let Err(err) = app.run() {
        eprintln!("Liquid Glass Menu exited with error: {err}");
    }
}
