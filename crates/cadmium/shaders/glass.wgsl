struct Globals {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(1) @binding(0)
var sharp_tex: texture_2d<f32>;

@group(1) @binding(1)
var sharp_sampler: sampler;

@group(2) @binding(0)
var blur_tex: texture_2d<f32>;

@group(2) @binding(1)
var blur_sampler: sampler;

struct GlassIn {
    @location(0) rect: vec4<f32>,
    @location(1) radii: vec4<f32>,
    @location(2) tint_color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_width: f32,
    @location(5) saturation: f32,
    @location(6) specular: f32,
    @location(7) frost: f32,
    @location(8) glass_fx: vec4<f32>, // x: liquid_ripple, y: rim_angle, z: sheen_progress, w: conic_strength
}

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) tint_color: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) border_width: f32,
    @location(6) saturation: f32,
    @location(7) specular: f32,
    @location(8) frost: f32,
    @location(9) glass_fx: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, inst: GlassIn) -> VertexOut {
    let corner = vec2<f32>(
        f32(vertex_index & 1u),
        f32((vertex_index >> 1u) & 1u),
    );

    let pad = 1.0;
    let origin = inst.rect.xy - vec2<f32>(pad);
    let size = inst.rect.zw + vec2<f32>(pad * 2.0);
    let pixel_pos = origin + corner * size;

    let ndc = vec2<f32>(
        (pixel_pos.x / globals.viewport_size.x) * 2.0 - 1.0,
        1.0 - (pixel_pos.y / globals.viewport_size.y) * 2.0,
    );

    let half_size = inst.rect.zw * 0.5;
    let center = inst.rect.xy + half_size;

    var out: VertexOut;
    out.clip_pos = vec4<f32>(ndc, 0.0, 1.0);
    out.local_pos = pixel_pos - center;
    out.half_size = half_size;
    out.radii = inst.radii;
    out.tint_color = inst.tint_color;
    out.border_color = inst.border_color;
    out.border_width = inst.border_width;
    out.saturation = inst.saturation;
    out.specular = inst.specular;
    out.frost = inst.frost;
    out.glass_fx = inst.glass_fx;
    return out;
}

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    let rx = select(vec2<f32>(r.w, r.x), vec2<f32>(r.z, r.y), p.x > 0.0);
    let corner_r = select(rx.y, rx.x, p.y > 0.0);
    let clamped_r = min(corner_r, min(b.x, b.y));
    let q = abs(p) - b + vec2<f32>(clamped_r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - clamped_r;
}

fn linear_to_srgb_vec(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn srgb_to_linear_vec(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow(max((c + 0.055) / 1.055, vec3<f32>(0.0)), vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let dist = sd_rounded_box(in.local_pos, in.half_size, in.radii);
    let outer_alpha = 1.0 - smoothstep(-0.5, 0.5, dist);

    let e = vec2<f32>(0.75, 0.0);
    let grad = vec2<f32>(
        sd_rounded_box(in.local_pos + e.xy, in.half_size, in.radii)
            - sd_rounded_box(in.local_pos - e.xy, in.half_size, in.radii),
        sd_rounded_box(in.local_pos + e.yx, in.half_size, in.radii)
            - sd_rounded_box(in.local_pos - e.yx, in.half_size, in.radii),
    );
    let g_len = length(grad);
    let normal = select(vec2<f32>(0.0, -1.0), grad / max(g_len, 0.0001), g_len > 0.0001);

    // Monotonic convex lens bevel (26px rim, strictly positive derivative -> zero folding/creases)
    let bevel = min(26.0, min(in.half_size.x, in.half_size.y) * 0.48);
    let rim_t = smoothstep(-bevel, 0.0, dist);
    let bend = rim_t * rim_t;

    // C-infinity smooth organic liquid wave
    let ripple = in.glass_fx.x;
    let wave_x = sin(in.local_pos.y * 0.022 + in.local_pos.x * 0.009)
               + 0.45 * cos(in.local_pos.y * 0.039 - in.local_pos.x * 0.014);
    let wave_y = cos(in.local_pos.x * 0.022 - in.local_pos.y * 0.009)
               + 0.45 * sin(in.local_pos.x * 0.037 + in.local_pos.y * 0.013);
    let organic_px = vec2<f32>(wave_x, wave_y) * ripple * 7.5;

    let uv_offset = (normal * bend * 14.5 + organic_px) / globals.viewport_size;
    // Optical prism dispersion (~3.6px RGB split on curved bevel)
    let disp = (normal * bend * 3.6 + organic_px * 0.18) / globals.viewport_size;
    let screen_uv = in.clip_pos.xy / globals.viewport_size;

    let uv_r = clamp(screen_uv + uv_offset - disp, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_g = clamp(screen_uv + uv_offset,        vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_b = clamp(screen_uv + uv_offset + disp, vec2<f32>(0.001), vec2<f32>(0.999));

    let sharp_bg = vec3<f32>(
        textureSample(sharp_tex, sharp_sampler, uv_r).r,
        textureSample(sharp_tex, sharp_sampler, uv_g).g,
        textureSample(sharp_tex, sharp_sampler, uv_b).b,
    );
    let blur_bg = textureSample(blur_tex, blur_sampler, uv_g).rgb;

    if outer_alpha <= 0.0 {
        discard;
    }

    let frost_amt = mix(in.frost, in.frost * 0.20, bend);
    let bg = mix(sharp_bg, blur_bg, frost_amt);

    let luma = dot(bg, vec3<f32>(0.2126, 0.7152, 0.0722));
    let saturated = clamp(mix(vec3<f32>(luma), bg, in.saturation), vec3<f32>(0.0), vec3<f32>(1.0));
    // Subtle curved light-source reflection across upper-left glass face (in exact sRGB)
    let p_uv = in.local_pos / max(in.half_size, vec2<f32>(1.0));
    let refl_coord = p_uv.x * 0.55 + p_uv.y * 0.85 + 0.12 * sin(p_uv.x * 2.4);
    let refl_band = (1.0 - smoothstep(-0.45, 0.20, refl_coord)) * smoothstep(-1.25, -0.35, refl_coord);
    let glass_sheen_srgb = refl_band * 0.065 * in.specular;

    let bg_srgb = linear_to_srgb_vec(saturated);
    let tint_srgb = linear_to_srgb_vec(in.tint_color.rgb);
    let mixed_srgb = clamp(mix(bg_srgb, tint_srgb, in.tint_color.a) + vec3<f32>(glass_sheen_srgb), vec3<f32>(0.0), vec3<f32>(1.0));
    var surface_rgb = srgb_to_linear_vec(mixed_srgb);

    let sheen_prog = in.glass_fx.z;
    let conic_str = in.glass_fx.w;
    if conic_str > 0.001 {
        let p_norm = in.local_pos / max(in.half_size, vec2<f32>(1.0));
        let sweep_coord = dot(p_norm, vec2<f32>(0.7071, -0.7071));
        let band_center = mix(-0.95, 0.55, sheen_prog);
        let sheen = exp(-pow((sweep_coord - band_center) / 0.24, 2.0)) * 0.34;
        surface_rgb = clamp(surface_rgb + vec3<f32>(sheen), vec3<f32>(0.0), vec3<f32>(1.0));

        let top_lip = smoothstep(-3.0, 0.0, dist) * clamp(-normal.y, 0.0, 1.0);
        let bot_lip = smoothstep(-4.0, 0.0, dist) * clamp(normal.y, 0.0, 1.0);
        surface_rgb *= 1.0 - top_lip * 0.08;
        surface_rgb = mix(surface_rgb, vec3<f32>(1.0), bot_lip * 0.50);

        let angle = atan2(in.local_pos.y, in.local_pos.x) - in.glass_fx.y;
        let dark_arc = pow(abs(cos(angle)), 8.0);
        let rim_ring = smoothstep(-2.4, -1.1, dist) * (1.0 - smoothstep(-0.6, 0.2, dist));
        let rim_col = mix(vec3<f32>(0.90), vec3<f32>(0.10), dark_arc * 0.65);
        surface_rgb = mix(surface_rgb, rim_col, rim_ring * 0.72 * conic_str);
    } else if in.specular > 0.001 {
        let p_norm = in.local_pos / max(in.half_size, vec2<f32>(1.0));
        let p_len = length(p_norm);
        let p_dir = select(vec2<f32>(-0.7071, -0.7071), p_norm / max(p_len, 0.0001), p_len > 0.0001);
        let diag_align = dot(p_dir, vec2<f32>(0.7071, 0.7071));

        let tl_peak = pow(max(-diag_align, 0.0), 1.6);
        let br_peak = pow(max(diag_align, 0.0), 1.6);
        let corner_glow = tl_peak + br_peak;

        // Crisp 3.5px opposing-corner specular bevel (matches CodePen --edge)
        let soft_inset = smoothstep(-3.8, -0.4, dist);
        let rim_line = smoothstep(-1.8, -0.6, dist) * (1.0 - smoothstep(-0.6, 0.2, dist));

        let edge_white = (soft_inset * 0.34 * corner_glow + rim_line * (0.14 + 0.40 * corner_glow)) * in.specular;
        surface_rgb = mix(surface_rgb, vec3<f32>(1.0), clamp(edge_white, 0.0, 0.82));
    }

    var final_rgb = surface_rgb;
    if in.border_width > 0.001 {
        let bw = min(in.border_width, min(in.half_size.x, in.half_size.y));
        let border_mix = smoothstep(-bw - 0.5, -bw + 0.5, dist);
        final_rgb = mix(surface_rgb, in.border_color.rgb, border_mix * in.border_color.a);
    }

    return vec4<f32>(final_rgb * outer_alpha, outer_alpha);
}
