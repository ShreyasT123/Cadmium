struct Globals {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(1) @binding(0)
var img_tex: texture_2d<f32>;

@group(1) @binding(1)
var img_sampler: sampler;

struct ImageIn {
    @location(0) rect: vec4<f32>,
    @location(1) uv_rect: vec4<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) tint: vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tint: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, inst: ImageIn) -> VertexOut {
    let corner = vec2<f32>(
        f32(vertex_index & 1u),
        f32((vertex_index >> 1u) & 1u),
    );

    let pixel_pos = inst.rect.xy + corner * inst.rect.zw;
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
    out.uv = mix(inst.uv_rect.xy, inst.uv_rect.zw, corner);
    out.tint = inst.tint;
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
    let sample = textureSample(img_tex, img_sampler, in.uv) * in.tint;
    let dist = sd_rounded_box(in.local_pos, in.half_size, in.radii);
    let edge_alpha = 1.0 - smoothstep(-0.5, 0.5, dist);
    let a = sample.a * edge_alpha;
    if a <= 0.001 {
        discard;
    }
    return vec4<f32>(sample.rgb * a, a);
}
