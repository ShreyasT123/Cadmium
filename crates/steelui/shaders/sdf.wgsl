struct Globals {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

struct InstanceIn {
    @location(0) rect: vec4<f32>,
    @location(1) radii: vec4<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) fill_color_end: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) border_width: f32,
    @location(6) gradient_dir: vec2<f32>,
    @location(7) shadow_color: vec4<f32>,
    @location(8) shadow_params: vec3<f32>,
}

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) fill_color: vec4<f32>,
    @location(4) fill_color_end: vec4<f32>,
    @location(5) border_color: vec4<f32>,
    @location(6) border_width: f32,
    @location(7) gradient_dir: vec2<f32>,
    @location(8) shadow_color: vec4<f32>,
    @location(9) shadow_params: vec3<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, inst: InstanceIn) -> VertexOut {
    let corner = vec2<f32>(
        f32(vertex_index & 1u),
        f32((vertex_index >> 1u) & 1u),
    );

    let shadow_reach = select(
        0.0,
        inst.shadow_params.z * 1.7 + max(abs(inst.shadow_params.x), abs(inst.shadow_params.y)),
        inst.shadow_color.a > 0.001,
    );
    let pad = max(1.0, shadow_reach + 2.0);

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
    out.fill_color = inst.fill_color;
    out.fill_color_end = inst.fill_color_end;
    out.border_color = inst.border_color;
    out.border_width = inst.border_width;
    out.gradient_dir = inst.gradient_dir;
    out.shadow_color = inst.shadow_color;
    out.shadow_params = inst.shadow_params;
    return out;
}

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    let rx = select(vec2<f32>(r.w, r.x), vec2<f32>(r.z, r.y), p.x > 0.0);
    let corner_r = select(rx.y, rx.x, p.y > 0.0);
    let clamped_r = min(corner_r, min(b.x, b.y));
    let q = abs(p) - b + vec2<f32>(clamped_r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - clamped_r;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let dist = sd_rounded_box(in.local_pos, in.half_size, in.radii);
    let outer_alpha = 1.0 - smoothstep(-0.5, 0.5, dist);

    let box_size = max(in.half_size * 2.0, vec2<f32>(1.0));
    let box_uv = (in.local_pos / box_size) + vec2<f32>(0.5);
    let g_t = clamp(dot(box_uv - vec2<f32>(0.5), in.gradient_dir) + 0.5, 0.0, 1.0);
    let base_fill = mix(in.fill_color, in.fill_color_end, g_t);

    var box_color = base_fill;
    if in.border_width > 0.001 {
        let bw = min(in.border_width, min(in.half_size.x, in.half_size.y));
        let border_mix = smoothstep(-bw - 0.5, -bw + 0.5, dist);

        let src_a = in.border_color.a;
        let dst_a = base_fill.a;
        let out_a = src_a + dst_a * (1.0 - src_a);
        let out_rgb = select(
            vec3<f32>(0.0),
            (in.border_color.rgb * src_a + base_fill.rgb * dst_a * (1.0 - src_a)) / max(out_a, 0.0001),
            out_a > 0.0001,
        );
        let border_over_fill = vec4<f32>(out_rgb, out_a);
        box_color = mix(base_fill, border_over_fill, border_mix);
    }

    let box_a = box_color.a * outer_alpha;
    let box_pm = vec4<f32>(box_color.rgb * box_a, box_a);

    var shad_pm = vec4<f32>(0.0);
    if in.shadow_color.a > 0.001 {
        let blur = max(in.shadow_params.z, 1.0);
        let s_dist = sd_rounded_box(in.local_pos - in.shadow_params.xy, in.half_size, in.radii);
        let sigma = blur * 0.44;
        let t = max(s_dist + blur * 0.22, 0.0) / sigma;
        var s_alpha = exp(-0.5 * t * t) * in.shadow_color.a;

        if base_fill.a < 0.99 {
            s_alpha *= smoothstep(-0.5, 0.5, dist);
        }
        shad_pm = vec4<f32>(in.shadow_color.rgb * s_alpha, s_alpha);
    }

    let out_pm = box_pm + shad_pm * (1.0 - box_pm.a);
    if out_pm.a <= 0.0001 {
        discard;
    }
    return out_pm;
}
