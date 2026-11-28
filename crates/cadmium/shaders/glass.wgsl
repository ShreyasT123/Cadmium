struct Globals {
    viewport_size: vec2<f32>,
    scale: f32,
    _pad: f32,
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
    @location(8) glass_fx: vec4<f32>, // ripple, rim angle, sheen progress, conic strength
    @location(9) glass_lens: vec4<f32>, // ior, bevel width, thickness, dispersion
    @location(10) glass_light: vec4<f32>, // light angle, fresnel, convexity, edge width
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
    @location(10) glass_lens: vec4<f32>,
    @location(11) glass_light: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, inst: GlassIn) -> VertexOut {
    let corner = vec2<f32>(
        f32(vertex_index & 1u),
        f32((vertex_index >> 1u) & 1u),
    );

    // Slight padding so the SDF anti-alias fringe is never clipped.
    let pad = 1.5;
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
    out.glass_lens = inst.glass_lens;
    out.glass_light = inst.glass_light;
    return out;
}

// Exact rounded-box SDF (Inigo Quilez style, clamped per-corner).
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

fn linear_to_oklab(c: vec3<f32>) -> vec3<f32> {
    let l = 0.4122214708 * c.x + 0.5363325363 * c.y + 0.0514459929 * c.z;
    let m = 0.2119034982 * c.x + 0.6806995451 * c.y + 0.1073969566 * c.z;
    let s = 0.0883024619 * c.x + 0.2817188376 * c.y + 0.6299787005 * c.z;
    let l_ = pow(max(l, 0.0), 1.0 / 3.0);
    let m_ = pow(max(m, 0.0), 1.0 / 3.0);
    let s_ = pow(max(s, 0.0), 1.0 / 3.0);
    return vec3<f32>(
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    );
}

fn oklab_to_linear(c: vec3<f32>) -> vec3<f32> {
    let l_ = c.x + 0.3963377774 * c.y + 0.2158037573 * c.z;
    let m_ = c.x - 0.1055613458 * c.y - 0.0638541728 * c.z;
    let s_ = c.x - 0.0894841775 * c.y - 1.2914855480 * c.z;
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;
    return vec3<f32>(
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    );
}

// Perceptual chroma adjustment. Values above 1 boost OKLab chroma with a
// lightness-preserving gamut compression; values below 1 fade toward luma.
fn boost_chroma(rgb: vec3<f32>, amount: f32) -> vec3<f32> {
    if (amount <= 1.0) {
        let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        return mix(vec3<f32>(luma), rgb, clamp(amount, 0.0, 1.0));
    }
    let lab = linear_to_oklab(rgb);
    var scale = amount;
    var out = rgb;
    for (var i: i32 = 0; i < 6; i = i + 1) {
        let candidate = oklab_to_linear(vec3<f32>(lab.x, lab.y * scale, lab.z * scale));
        let lo = min(min(candidate.x, candidate.y), candidate.z);
        let hi = max(max(candidate.x, candidate.y), candidate.z);
        if (lo >= 0.0 && hi <= 1.0) {
            out = candidate;
            break;
        }
        scale = scale * 0.75;
    }
    return out;
}

// Lateral displacement of a vertical view ray refracted by a bevel tilted
// by `sin_i`, travelling `depth` through a medium with index `ior`.
fn refract_offset(n: vec2<f32>, sin_i: f32, ior: f32, depth: f32) -> vec2<f32> {
    let sin_t = clamp(sin_i / ior, 0.0, 0.95);
    let tan_t = sin_t / sqrt(max(1.0 - sin_t * sin_t, 0.02));
    return n * tan_t * depth;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let dist = sd_rounded_box(in.local_pos, in.half_size, in.radii);
    let outer_alpha = 1.0 - smoothstep(-0.6, 0.6, dist);

    // Analytic gradient of the SDF gives the outward surface normal.
    let e = vec2<f32>(0.85, 0.0);
    let grad = vec2<f32>(
        sd_rounded_box(in.local_pos + e.xy, in.half_size, in.radii)
            - sd_rounded_box(in.local_pos - e.xy, in.half_size, in.radii),
        sd_rounded_box(in.local_pos + e.yx, in.half_size, in.radii)
            - sd_rounded_box(in.local_pos - e.yx, in.half_size, in.radii),
    );
    let g_len = length(grad);
    let normal = select(vec2<f32>(0.0, -1.0), grad / max(g_len, 1e-4), g_len > 1e-4);

    // ------------------------------------------------------------------
    // 1. Convex bevel: flat plateau, monotone x^1.5 tilt across the rim.
    // ------------------------------------------------------------------
    let s = max(globals.scale, 0.01);
    let bevel_w = min(in.glass_lens.y * s, min(in.half_size.x, in.half_size.y) * 0.48);
    let x = clamp(1.0 + dist / max(bevel_w, 0.001), 0.0, 1.0);
    let height = pow(x, 1.5);
    let slope = 1.5 * sqrt(x);

    // Steepness setting maps to a maximum surface tilt of sin(theta) 0.30..0.95.
    let max_sin = mix(0.30, 0.95, clamp(in.glass_light.z, 0.0, 1.0));
    let sin_i = slope / 1.5 * max_sin;
    let ior = max(in.glass_lens.x, 1.0);

    // Clamp the refraction depth so the sample mapping stays monotone: the
    // displacement must move less than the bevel width as x goes 0 -> 1.
    let max_tan = 0.95 / sqrt(max(1.0 - 0.95 * 0.95, 0.02));
    let depth = min(in.glass_lens.z * s, bevel_w * 0.55 / max_tan);

    // ------------------------------------------------------------------
    // 2. Organic ripple, in logical pixels (clamped by the shader).
    // ------------------------------------------------------------------
    let ripple = min(max(in.glass_fx.x, 0.0), 2.0);
    let wave_x = sin(in.local_pos.y * 0.021 + in.local_pos.x * 0.0085)
               + 0.42 * cos(in.local_pos.y * 0.037 - in.local_pos.x * 0.013);
    let wave_y = cos(in.local_pos.x * 0.021 - in.local_pos.y * 0.0085)
               + 0.42 * sin(in.local_pos.x * 0.035 + in.local_pos.y * 0.012);
    let organic = vec2<f32>(wave_x, wave_y) * ripple * 6.8 * s;

    // ------------------------------------------------------------------
    // 3. Seven spectral taps integrate wavelength-dependent refraction.
    // ------------------------------------------------------------------
    let screen_uv = in.clip_pos.xy / globals.viewport_size;
    let dispersion = clamp(in.glass_lens.w, 0.0, 0.5);
    var spectral = vec3<f32>(0.0);
    var weights = vec3<f32>(0.0);
    for (var k: i32 = 0; k < 7; k = k + 1) {
        let f = f32(k) / 6.0;
        let n = ior + dispersion * (f - 0.5);
        let offset = refract_offset(normal, sin_i, n, depth) + organic;
        let uv = clamp(screen_uv + offset / globals.viewport_size,
                       vec2<f32>(0.001), vec2<f32>(0.999));
        let sample = textureSample(sharp_tex, sharp_sampler, uv).rgb;

        let dr = f / 0.34;
        let dg = (f - 0.5) / 0.34;
        let db = (f - 1.0) / 0.34;
        let w = vec3<f32>(exp(-dr * dr), exp(-dg * dg), exp(-db * db));
        spectral = spectral + sample * w;
        weights = weights + w;
    }
    let sharp_bg = spectral / max(weights, vec3<f32>(0.001));

    let blur_offset = refract_offset(normal, sin_i, ior, depth) + organic;
    let blur_uv = clamp(screen_uv + blur_offset / globals.viewport_size,
                        vec2<f32>(0.001), vec2<f32>(0.999));
    let blur_bg = textureSample(blur_tex, blur_sampler, blur_uv).rgb;

    if (outer_alpha <= 0.001) {
        discard;
    }

    // ------------------------------------------------------------------
    // 4. Frost (the rim stays sharper so refraction remains visible).
    // ------------------------------------------------------------------
    let frost_amt = mix(in.frost, in.frost * 0.15, smoothstep(0.0, 1.0, x));
    let bg = mix(sharp_bg, blur_bg, clamp(frost_amt, 0.0, 1.0));
    let saturated = boost_chroma(bg, in.saturation);

    // Subtle curved light-source reflection on the upper-left face.
    let p_uv = in.local_pos / max(in.half_size, vec2<f32>(1.0));
    let refl_coord = p_uv.x * 0.52 + p_uv.y * 0.82 + 0.11 * sin(p_uv.x * 2.3);
    let refl_band = (1.0 - smoothstep(-0.42, 0.18, refl_coord))
                  * smoothstep(-1.22, -0.32, refl_coord);
    let glass_sheen = refl_band * 0.055 * in.specular;

    // Tint is authored in linear space; mix in sRGB for a predictable look.
    let bg_srgb = linear_to_srgb_vec(clamp(saturated, vec3<f32>(0.0), vec3<f32>(1.0)));
    let tint_srgb = linear_to_srgb_vec(in.tint_color.rgb);
    let mixed_srgb = clamp(mix(bg_srgb, tint_srgb, in.tint_color.a) + vec3<f32>(glass_sheen),
                           vec3<f32>(0.0), vec3<f32>(1.0));
    var surface_rgb = srgb_to_linear_vec(mixed_srgb);

    // ------------------------------------------------------------------
    // 5. Fresnel catchlights, cross-fading into the animated conic sheen.
    // ------------------------------------------------------------------
    let conic_str = clamp(in.glass_fx.w, 0.0, 1.0);
    let light_dir = vec2<f32>(cos(in.glass_light.x), sin(in.glass_light.x));
    let fresnel = clamp(in.glass_light.y, 0.0, 2.0);
    let edge_w = max(in.glass_light.w, 0.25) * s;

    let front = pow(clamp(dot(normal, light_dir), 0.0, 1.0), 3.0);
    let back = pow(clamp(dot(normal, -light_dir), 0.0, 1.0), 4.0);
    let bevel_band = smoothstep(-bevel_w, -0.5 * s, dist)
                   * (1.0 - smoothstep(-0.5 * s, 0.6 * s, dist));
    let hairline = smoothstep(-edge_w, -0.2 * s, dist)
                 * (1.0 - smoothstep(-0.2 * s, 0.6 * s, dist));
    let catchlight = (bevel_band * (front + 0.45 * back) * 0.55 + hairline * 0.35)
                   * fresnel * in.specular * (1.0 - conic_str);
    surface_rgb = mix(surface_rgb, vec3<f32>(1.0), clamp(catchlight, 0.0, 0.78));

    if (conic_str > 0.001) {
        // Animated conic sheen (pill buttons).
        let p_norm = in.local_pos / max(in.half_size, vec2<f32>(1.0));
        let sweep_coord = dot(p_norm, vec2<f32>(0.7071, -0.7071));
        let band_center = mix(-0.92, 0.58, in.glass_fx.z);
        let ds = (sweep_coord - band_center) / 0.22;
        let sheen = exp(-ds * ds) * 0.32;
        surface_rgb = clamp(surface_rgb + vec3<f32>(sheen), vec3<f32>(0.0), vec3<f32>(1.0));

        // Soft top/bottom lip shading for extra thickness.
        let top_lip = smoothstep(-3.2, 0.0, dist) * clamp(-normal.y, 0.0, 1.0);
        let bot_lip = smoothstep(-4.2, 0.0, dist) * clamp(normal.y, 0.0, 1.0);
        surface_rgb *= 1.0 - top_lip * 0.07;
        surface_rgb = mix(surface_rgb, vec3<f32>(1.0), bot_lip * 0.42);

        // Conic rim ring.
        let angle = atan2(in.local_pos.y, in.local_pos.x) - in.glass_fx.y;
        let dark_arc = pow(abs(cos(angle)), 7.5);
        let rim_ring = smoothstep(-2.6, -1.0, dist) * (1.0 - smoothstep(-0.7, 0.25, dist));
        let rim_col = mix(vec3<f32>(0.92), vec3<f32>(0.08), dark_arc * 0.68);
        surface_rgb = mix(surface_rgb, rim_col, rim_ring * 0.70 * conic_str);
    }

    // ------------------------------------------------------------------
    // 6. Optional border.
    // ------------------------------------------------------------------
    var final_rgb = surface_rgb;
    if (in.border_width > 0.001) {
        let bw = min(in.border_width, min(in.half_size.x, in.half_size.y));
        let border_mix = smoothstep(-bw - 0.55, -bw + 0.55, dist);
        final_rgb = mix(surface_rgb, in.border_color.rgb, border_mix * in.border_color.a);
    }

    // Premultiplied alpha (required by the pipeline blend state).
    return vec4<f32>(final_rgb * outer_alpha, outer_alpha);
}
