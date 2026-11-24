struct Globals {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(1) @binding(0)
var atlas_tex: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

struct GlyphIn {
    @location(0) rect: vec4<f32>,    // x, y, w, h (physical px)
    @location(1) uv_rect: vec4<f32>, // u0, v0, u1, v1
    @location(2) color: vec4<f32>,   // linear RGBA
}

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    glyph: GlyphIn,
) -> VertexOut {
    let corner = vec2<f32>(
        f32(vertex_index & 1u),
        f32((vertex_index >> 1u) & 1u),
    );

    let pixel_pos = glyph.rect.xy + corner * glyph.rect.zw;
    let ndc = vec2<f32>(
        (pixel_pos.x / globals.viewport_size.x) * 2.0 - 1.0,
        1.0 - (pixel_pos.y / globals.viewport_size.y) * 2.0,
    );

    var out: VertexOut;
    out.clip_pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = mix(glyph.uv_rect.xy, glyph.uv_rect.zw, corner);
    out.color = glyph.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let sample = textureSample(atlas_tex, atlas_sampler, in.uv);
    let c = sample * in.color;
    if c.a <= 0.001 {
        discard;
    }
    return vec4<f32>(c.rgb * c.a, c.a);
}
