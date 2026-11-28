use std::path::{Path};

use cadmium::{
    cover_uv_rect, load_image_file, rasterize_svg, snap_rect_to_pixels, App, Color, CornerRadii,
    CursorStyle, DecodedImage, FontWeight, FrameContext, GlassOptics, GlassQuad, GpuContext,
    ImageQuad, MacTheme, Rect, SdfQuad, Spring, TextBlock, TextEngine, TextStyle, TextureHandle,
    WindowCommand, WindowDelegate, WindowSpec,
};

const LINKEDIN_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 93.06 93.06"><path fill="#ffffff" d="M11.185.08C5.004.08.001 5.092 0 11.259c0 6.173 5.003 11.184 11.186 11.184 6.166 0 11.176-5.011 11.176-11.184C22.362 5.091 17.351.08 11.185.08zM1.538 30.926h19.287V92.98H1.538zM69.925 29.383c-9.382 0-15.673 5.144-18.248 10.022h-.258v-8.479H32.92v62.053h19.27V62.281c0-8.093 1.541-15.932 11.575-15.932 9.89 0 10.022 9.256 10.022 16.451v30.178H93.06V58.942c0-16.707-3.605-29.559-23.135-29.559z"/></svg>"##;

const PRISM_ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#ffffff" d="m12 2-9 17h18L12 2zm0 4.2 6.14 11.8H5.86L12 6.2z"/></svg>"##;

const SPARKLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#ffffff" d="M12 2 9.5 9.5 2 12l7.5 2.5L12 22l2.5-7.5L22 12l-7.5-2.5L12 2z"/></svg>"##;

struct LiquidGlassScene {
    bg_tex: Option<TextureHandle>,
    bg_size: (u32, u32),
    linkedin_tex: Option<TextureHandle>,
    prism_tex: Option<TextureHandle>,
    sparkle_tex: Option<TextureHandle>,

    // Component 1: Draggable Magnifying Optical Loupe
    loupe_pos: (f32, f32),
    loupe_dragging: bool,
    loupe_drag_offset: (f32, f32),
    loupe_radius: f32,

    // Component 2: 3 Side-by-Side Glass Cards
    card_hovered: [bool; 3],
    card_springs: [Spring; 3],

    // Component 3: Dynamic Island / Pill Button
    gen_rect: Rect,
    gen_hovered: bool,
    gen_pressed: bool,
    gen_hover_spring: Spring,
    gen_press_spring: Spring,
    ripple_pulse: Spring,

    // Text cache
    heading: Option<TextBlock>,
    subheading: Option<TextBlock>,
    card_titles: Vec<TextBlock>,
    card_descs: Vec<TextBlock>,
    button_label: Option<TextBlock>,
    loupe_label: Option<TextBlock>,
    traffic_x: Option<TextBlock>,
    traffic_min: Option<TextBlock>,
    traffic_plus: Option<TextBlock>,
    hovered_traffic: bool,
}

impl LiquidGlassScene {
    fn new() -> Self {
        Self {
            bg_tex: None,
            bg_size: (1, 1),
            linkedin_tex: None,
            prism_tex: None,
            sparkle_tex: None,
            loupe_pos: (250.0, 480.0),
            loupe_dragging: false,
            loupe_drag_offset: (0.0, 0.0),
            loupe_radius: 80.0,
            card_hovered: [false; 3],
            card_springs: [
                Spring::new(0.0, 260.0, 20.0),
                Spring::new(0.0, 260.0, 20.0),
                Spring::new(0.0, 260.0, 20.0),
            ],
            gen_rect: Rect::default(),
            gen_hovered: false,
            gen_pressed: false,
            gen_hover_spring: Spring::new(0.0, 280.0, 24.0),
            gen_press_spring: Spring::new(0.0, 360.0, 26.0),
            ripple_pulse: Spring::new(0.0, 180.0, 14.0),
            heading: None,
            subheading: None,
            card_titles: Vec::new(),
            card_descs: Vec::new(),
            button_label: None,
            loupe_label: None,
            traffic_x: None,
            traffic_min: None,
            traffic_plus: None,
            hovered_traffic: false,
        }
    }
}

fn hit_test(r: Rect, x: f32, y: f32) -> bool {
    x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height
}

fn load_bg_or_fallback() -> DecodedImage {
    for candidate in [
        "examples/assets/bg.png",
    ] {
        if Path::new(candidate).exists() {
            if let Ok(img) = load_image_file(candidate) {
                return img;
            }
        }
    }

    // High-contrast, vibrant fallback underlay for refraction and prism testing
    let (w, h) = (1024u32, 768u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let wave1 = ((u * 8.0 + v * 4.0).sin() * 0.5 + 0.5) * 0.45;
            let wave2 = ((u * 14.0 - v * 9.0).cos() * 0.5 + 0.5) * 0.35;
            let r = (28.0 + 195.0 * (1.0 - v) + 90.0 * wave1).min(255.0) as u8;
            let g = (45.0 + 130.0 * u * (1.0 - v * 0.5) + 85.0 * wave2).min(255.0) as u8;
            let b = (95.0 + 160.0 * v + 75.0 * (1.0 - u)).min(255.0) as u8;
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
        let gold = Color::from_rgba8(255, 225, 160, 240);

        self.heading = Some(TextBlock::new(
            text,
            "Optical Liquid Glass Studio",
            TextStyle {
                font_size: 34.0,
                line_height: 42.0,
                color: white,
                weight: FontWeight::Bold,
            },
        ));

        self.subheading = Some(TextBlock::new(
            text,
            "Real-time convex lens refraction, chromatic dispersion (prism RGB split), and specular rims",
            TextStyle {
                font_size: 14.0,
                line_height: 20.0,
                color: Color::from_rgba8(255, 255, 255, 215),
                weight: FontWeight::Medium,
            },
        ));

        self.button_label = Some(TextBlock::new(
            text,
            "Conic Sheen Pill",
            TextStyle {
                font_size: 14.0,
                line_height: 18.0,
                color: white,
                weight: FontWeight::Semibold,
            },
        ));

        self.loupe_label = Some(TextBlock::new(
            text,
            "DRAG ME: OPTICAL LENS",
            TextStyle {
                font_size: 10.5,
                line_height: 13.0,
                color: gold,
                weight: FontWeight::Bold,
            },
        ));

        self.card_titles = vec![
            TextBlock::new(
                text,
                "01 • Pure Crystal",
                TextStyle {
                    font_size: 16.0,
                    line_height: 22.0,
                    color: white,
                    weight: FontWeight::Bold,
                },
            ),
            TextBlock::new(
                text,
                "02 • Prism Refraction",
                TextStyle {
                    font_size: 16.0,
                    line_height: 22.0,
                    color: white,
                    weight: FontWeight::Bold,
                },
            ),
            TextBlock::new(
                text,
                "03 • Smoked Vibrancy",
                TextStyle {
                    font_size: 16.0,
                    line_height: 22.0,
                    color: white,
                    weight: FontWeight::Bold,
                },
            ),
        ];

        self.card_descs = vec![
            TextBlock::new(
                text,
                "Frost: 0.05 • Pure IOR lens curvature with zero opacity wash.",
                TextStyle {
                    font_size: 12.0,
                    line_height: 16.0,
                    color: Color::from_rgba8(240, 240, 245, 220),
                    weight: FontWeight::Regular,
                },
            ),
            TextBlock::new(
                text,
                "Frost: 0.18 • Wavelength dispersion (RGB split) + wave pulse.",
                TextStyle {
                    font_size: 12.0,
                    line_height: 16.0,
                    color: Color::from_rgba8(240, 240, 245, 220),
                    weight: FontWeight::Regular,
                },
            ),
            TextBlock::new(
                text,
                "Frost: 0.60 • Multi-mip Dual-Kawase blur with crisp edge catchlights.",
                TextStyle {
                    font_size: 12.0,
                    line_height: 16.0,
                    color: Color::from_rgba8(240, 240, 245, 220),
                    weight: FontWeight::Regular,
                },
            ),
        ];

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

        if let Ok(li) = rasterize_svg(LINKEDIN_SVG, 128, 128) {
            self.linkedin_tex = Some(gpu.upload_image(&li));
        }
        if let Ok(pr) = rasterize_svg(PRISM_ICON_SVG, 128, 128) {
            self.prism_tex = Some(gpu.upload_image(&pr));
        }
        if let Ok(sp) = rasterize_svg(SPARKLE_SVG, 128, 128) {
            self.sparkle_tex = Some(gpu.upload_image(&sp));
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut active = self.gen_hover_spring.step(dt);
        active |= self.gen_press_spring.step(dt);
        active |= self.ripple_pulse.step(dt);
        for s in &mut self.card_springs {
            active |= s.step(dt);
        }
        active
    }

    fn cursor_style(&self) -> CursorStyle {
        if self.loupe_dragging {
            CursorStyle::Grabbing
        } else if self.gen_hovered {
            CursorStyle::Pointer
        } else {
            CursorStyle::Default
        }
    }

    fn is_drag_region(&self, x: f32, y: f32) -> bool {
        y < 44.0 && !hit_test(self.gen_rect, x, y)
    }

    fn on_pointer_move(&mut self, x: f32, y: f32) -> bool {
        let mut redraw = false;

        if self.loupe_dragging {
            self.loupe_pos = (
                x - self.loupe_drag_offset.0,
                y - self.loupe_drag_offset.1,
            );
            return true;
        }

        let in_traffic = (14.0..=76.0).contains(&x) && (14.0..=36.0).contains(&y);
        if in_traffic != self.hovered_traffic {
            self.hovered_traffic = in_traffic;
            redraw = true;
        }

        let in_gen = hit_test(self.gen_rect, x, y);
        if in_gen != self.gen_hovered {
            self.gen_hovered = in_gen;
            self.gen_hover_spring.set_target(if in_gen { 1.0 } else { 0.0 });
            redraw = true;
        }

        // Check card hovers
        let card_w = 330.0;
        let card_h = 240.0;
        let start_y = 170.0;
        for i in 0..3 {
            let cx = 60.0 + (i as f32) * (card_w + 35.0);
            let rect = Rect::new(cx, start_y, card_w, card_h);
            let inside = hit_test(rect, x, y);
            if inside != self.card_hovered[i] {
                self.card_hovered[i] = inside;
                self.card_springs[i].set_target(if inside { 1.0 } else { 0.0 });
                redraw = true;
            }
        }

        redraw
    }

    fn on_pointer_down(&mut self, x: f32, y: f32) -> (bool, WindowCommand) {
        for (tx, cmd) in [
            (20.0, WindowCommand::Close),
            (40.0, WindowCommand::Minimize),
            (60.0, WindowCommand::ToggleMaximize),
        ] {
            if (tx..=tx + 12.0).contains(&x) && (20.0..=32.0).contains(&y) {
                return (false, cmd);
            }
        }

        // Loupe hit test (circle check)
        let dx = x - self.loupe_pos.0;
        let dy = y - self.loupe_pos.1;
        if dx * dx + dy * dy <= self.loupe_radius * self.loupe_radius {
            self.loupe_dragging = true;
            self.loupe_drag_offset = (dx, dy);
            self.ripple_pulse.snap_to(1.2);
            self.ripple_pulse.set_target(0.0);
            return (true, WindowCommand::None);
        }

        if hit_test(self.gen_rect, x, y) {
            self.gen_pressed = true;
            self.gen_press_spring.set_target(1.0);
            self.ripple_pulse.snap_to(1.5);
            self.ripple_pulse.set_target(0.0);
            return (true, WindowCommand::None);
        }

        // Clicking background triggers a ripple pulse
        self.ripple_pulse.snap_to(1.0);
        self.ripple_pulse.set_target(0.0);

        (true, WindowCommand::None)
    }

    fn on_pointer_up(&mut self) -> bool {
        let mut redraw = false;
        if self.loupe_dragging {
            self.loupe_dragging = false;
            redraw = true;
        }
        if self.gen_pressed {
            self.gen_pressed = false;
            self.gen_press_spring.set_target(0.0);
            redraw = true;
        }
        redraw
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) {
        let scale = ctx.scale;
        let theme = MacTheme::light();
        let pulse = self.ripple_pulse.value.max(0.0);

        // =========================================================================
        // LAYER 1: Full-Bleed Wallpaper Photo
        // =========================================================================
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

        // =========================================================================
        // LAYER 2: High-Contrast Underlay Graphics (Drawn BEFORE glass so glass bends them!)
        // =========================================================================
        // Bold diagonal graphic stripes & typography under the glass cards
        for (i, col) in [
            Color::from_rgba8(255, 60, 100, 230),
            Color::from_rgba8(60, 180, 255, 230),
            Color::from_rgba8(255, 210, 40, 230),
            Color::from_rgba8(50, 225, 120, 230),
        ]
        .into_iter()
        .enumerate()
        {
            let bx = 110.0 + (i as f32) * 270.0;
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(Rect::new(bx, 150.0, 80.0, 310.0), scale),
                radii: CornerRadii::all(20.0).scale(scale),
                fill: col,
                rotation: -0.22,
                ..Default::default()
            });
        }

        // High-contrast test grid and text that the Draggable Loupe will magnify
        ctx.push_sdf(SdfQuad {
            rect: snap_rect_to_pixels(Rect::new(80.0, 440.0, 520.0, 140.0), scale),
            radii: CornerRadii::all(14.0).scale(scale),
            fill: Color::from_rgba8(12, 14, 18, 210),
            border_color: Color::from_rgba8(255, 255, 255, 40),
            border_width: 1.0 * scale,
            ..Default::default()
        });

        // =========================================================================
        // LAYER 3: Window Chrome & Header Text
        // =========================================================================
        for (tx, color, glyph_opt) in [
            (20.0, theme.traffic_close, &self.traffic_x),
            (40.0, theme.traffic_minimize, &self.traffic_min),
            (60.0, theme.traffic_zoom, &self.traffic_plus),
        ] {
            ctx.push_sdf(SdfQuad {
                rect: Rect::new(tx, 16.0, 12.0, 12.0).scale(scale),
                radii: CornerRadii::all(6.0).scale(scale),
                fill: color,
                border_color: theme.traffic_border,
                border_width: 1.0,
                ..Default::default()
            });
            if self.hovered_traffic && let Some(g) = glyph_opt {
                ctx.push_text(g, tx + 2.8, 16.0);
            }
        }

        if let Some(h) = &self.heading {
            ctx.push_text(h, 60.0, 48.0);
        }
        if let Some(sh) = &self.subheading {
            ctx.push_text(sh, 60.0, 92.0);
        }
        if let Some(lbl) = &self.loupe_label {
            ctx.push_text(lbl, 104.0, 460.0);
        }

        // =========================================================================
        // LAYER 4: Component Showcase — 3 Side-by-Side Glass Cards
        // =========================================================================
        let card_w = 330.0;
        let card_h = 240.0;
        let start_y = 170.0;

        for i in 0..3 {
            let cx = 60.0 + (i as f32) * (card_w + 35.0);
            let hover_t = self.card_springs[i].value;
            let card_rect = Rect::new(cx, start_y - hover_t * 6.0, card_w, card_h);

            // Card Shadow
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(card_rect, scale),
                radii: CornerRadii::all(24.0).scale(scale),
                fill: Color::TRANSPARENT,
                shadow_color: Color::from_rgba8(0, 0, 0, (65.0 + hover_t * 35.0) as u8),
                shadow_offset: [0.0, (8.0 + hover_t * 8.0) * scale],
                shadow_blur: (22.0 + hover_t * 12.0) * scale,
                ..Default::default()
            });

            // Specific Glass Quad Presets
            match i {
                // 0: Pure Crystal Liquid (Ultra-low frost, high refraction, high specular)
                0 => {
                    ctx.push_glass(GlassQuad {
                        rect: snap_rect_to_pixels(card_rect, scale),
                        radii: CornerRadii::all(24.0).scale(scale),
                        tint: Color::from_rgba8(255, 255, 255, (12.0 + hover_t * 12.0) as u8),
                        saturation: 1.15,
                        specular: 1.0,
                        frost: 0.04,
                        liquid_ripple: 0.75 + pulse * 0.8,
                        rim_angle: -1.309,
                        sheen_progress: hover_t,
                        conic_strength: 0.0,
                        optics: GlassOptics::LIQUID,
                        border_color: Color::from_rgba8(255, 255, 255, 55),
                        border_width: 1.0 * scale,
                    });
                }
                // 1: Prism Refraction (Medium frost, high chromatic dispersion wave)
                1 => {
                    ctx.push_glass(GlassQuad {
                        rect: snap_rect_to_pixels(card_rect, scale),
                        radii: CornerRadii::all(24.0).scale(scale),
                        tint: Color::from_rgba8(240, 248, 255, (22.0 + hover_t * 14.0) as u8),
                        saturation: 1.25,
                        specular: 0.95,
                        frost: 0.16,
                        liquid_ripple: 0.95 + pulse * 1.2,
                        rim_angle: -1.05,
                        sheen_progress: hover_t * 0.8,
                        conic_strength: 0.0,
                        optics: GlassOptics::LIQUID,
                        border_color: Color::from_rgba8(255, 255, 255, 75),
                        border_width: 1.2 * scale,
                    });
                }
                // 2: Smoked Frosted Vibrancy (Deep blur with sharp edges)
                _ => {
                    ctx.push_glass(GlassQuad {
                        rect: snap_rect_to_pixels(card_rect, scale),
                        radii: CornerRadii::all(24.0).scale(scale),
                        tint: Color::from_rgba8(24, 22, 30, (90.0 + hover_t * 30.0) as u8),
                        saturation: 1.30,
                        specular: 0.85,
                        frost: 0.55,
                        liquid_ripple: 0.20 + pulse * 0.4,
                        rim_angle: -1.309,
                        sheen_progress: 0.0,
                        conic_strength: 0.0,
                        optics: GlassOptics::LIQUID,
                        border_color: Color::from_rgba8(255, 255, 255, 45),
                        border_width: 1.0 * scale,
                    });
                }
            }

            // Card Content
            if let Some(t) = self.card_titles.get(i) {
                ctx.push_text(t, card_rect.x + 22.0, card_rect.y + 24.0);
            }
            if let Some(d) = self.card_descs.get(i) {
                ctx.push_text(d, card_rect.x + 22.0, card_rect.y + 54.0);
            }

            // Embedded icon inside each card
            let icon_tex = match i {
                0 => self.sparkle_tex,
                1 => self.prism_tex,
                _ => self.linkedin_tex,
            };
            if let Some(tex) = icon_tex {
                let sz = 44.0;
                ctx.push_image(ImageQuad {
                    rect: Rect::new(
                        (card_rect.x + card_w - sz - 22.0) * scale,
                        (card_rect.y + card_h - sz - 20.0) * scale,
                        sz * scale,
                        sz * scale,
                    ),
                    uv_rect: [0.0, 0.0, 1.0, 1.0],
                    radii: CornerRadii::all(10.0).scale(scale),
                    tint: Color::rgba(1.0, 1.0, 1.0, 0.90),
                    texture: tex,
                });
            }
        }

        // =========================================================================
        // LAYER 5: Draggable Magnifying Optical Loupe (Floating Liquid Lens)
        // =========================================================================
        let (lx, ly) = self.loupe_pos;
        let lr = self.loupe_radius;
        let loupe_rect = Rect::new(lx - lr, ly - lr, lr * 2.0, lr * 2.0);

        // Soft lens shadow
        ctx.push_sdf(SdfQuad {
            rect: snap_rect_to_pixels(loupe_rect, scale),
            radii: CornerRadii::all(lr).scale(scale),
            fill: Color::TRANSPARENT,
            shadow_color: Color::from_rgba8(0, 0, 0, 95),
            shadow_offset: [0.0, 10.0 * scale],
            shadow_blur: 26.0 * scale,
            ..Default::default()
        });

        // The Optical Loupe Glass: High convex refraction with chromatic dispersion!
        ctx.push_glass(GlassQuad {
            rect: snap_rect_to_pixels(loupe_rect, scale),
            radii: CornerRadii::all(lr).scale(scale),
            tint: Color::from_rgba8(255, 255, 255, 14),
            saturation: 1.25,
            specular: 1.0,
            frost: 0.02,                   // Ultra clear
            liquid_ripple: 1.25 + pulse * 1.5, // Strong lens bending
            rim_angle: -1.309,
            sheen_progress: 0.0,
            conic_strength: 0.0,
            optics: GlassOptics::LIQUID,
            border_color: Color::from_rgba8(255, 255, 255, 95),
            border_width: 1.5 * scale,
        });

        // Center crosshair / lens ring inside loupe
        ctx.push_sdf(SdfQuad {
            rect: snap_rect_to_pixels(Rect::new(lx - 12.0, ly - 12.0, 24.0, 24.0), scale),
            radii: CornerRadii::all(12.0).scale(scale),
            fill: Color::TRANSPARENT,
            border_color: Color::from_rgba8(255, 225, 120, 180),
            border_width: 1.5 * scale,
            ..Default::default()
        });

        // =========================================================================
        // LAYER 6: Floating Conic Glass Island Dock (Bottom Center)
        // =========================================================================
        let hv = self.gen_hover_spring.value;
        let pr = self.gen_press_spring.value;
        let btn_scale = (1.0 - hv * 0.025) * (1.0 - pr * 0.04);
        let btn_w = 260.0 * btn_scale;
        let btn_h = 56.0 * btn_scale;
        let btn_x = (ctx.width - btn_w) * 0.5;
        let btn_y = ctx.height - 84.0 + pr * 2.0;
        self.gen_rect = Rect::new(btn_x, btn_y, btn_w, btn_h);

        // Dock shadow
        ctx.push_sdf(SdfQuad {
            rect: snap_rect_to_pixels(self.gen_rect, scale),
            radii: CornerRadii::all(btn_h * 0.5).scale(scale),
            fill: Color::TRANSPARENT,
            shadow_color: Color::from_rgba8(0, 0, 0, (75.0 + hv * 25.0) as u8),
            shadow_offset: [0.0, (6.0 - hv * 2.0) * scale],
            shadow_blur: (16.0 - hv * 4.0) * scale,
            ..Default::default()
        });

        // Conic Pill with Rotating Rim Catchlight
        let rim_angle = -75.0_f32.to_radians() - hv * 50.0_f32.to_radians() + pr * 0.4;
        let sheen_prog = (hv * 0.6 + pr * 0.4).clamp(0.0, 1.0);
        ctx.push_glass(GlassQuad {
            rect: snap_rect_to_pixels(self.gen_rect, scale),
            radii: CornerRadii::all(btn_h * 0.5).scale(scale),
            tint: Color::from_rgba8(255, 255, 255, (20.0 + hv * 16.0) as u8),
            saturation: 1.15,
            specular: 1.0,
            frost: 0.12 - hv * 0.06,
            liquid_ripple: 0.35 + pulse * 0.6,
            rim_angle,
            sheen_progress: sheen_prog,
            conic_strength: 1.0,
            optics: GlassOptics::LIQUID,
            border_color: Color::from_rgba8(255, 255, 255, 60),
            border_width: 1.0 * scale,
        });

        if let Some(lbl) = &self.button_label {
            let (tw, th) = (118.0, 18.0);
            ctx.push_text(
                lbl,
                btn_x + (btn_w - tw) * 0.5,
                btn_y + (btn_h - th) * 0.5 - 1.0,
            );
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,wgpu=warn")
        .init();

    let app = App::new().add_window(WindowSpec {
        title: "Cadmium • Optical Liquid Glass Studio".into(),
        width: 1180.0,
        height: 760.0,
        min_width: Some(720.0),
        min_height: Some(540.0),
        borderless: true,
        clear_color: Color::from_rgba8(16, 18, 22, 255),
        delegate: Box::new(LiquidGlassScene::new()),
    });

    if let Err(err) = app.run() {
        eprintln!("Liquid Glass Studio error: {err}");
    }
}