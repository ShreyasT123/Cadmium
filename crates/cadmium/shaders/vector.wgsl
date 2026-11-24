struct Globals {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VertexIn {
    @location(0) position: vec2<f32>, // physical pixel coords
    @location(1) color: vec4<f32>,    // linear RGBA
}

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    let ndc = vec2<f32>(
        (in.position.x / globals.viewport_size.x) * 2.0 - 1.0,
        1.0 - (in.position.y / globals.viewport_size.y) * 2.0,
    );

    var out: VertexOut;
    out.clip_pos = vec4<f32>(ndc, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let a = in.color.a;
    if a <= 0.001 {
        discard;
    }
    return vec4<f32>(in.color.rgb * a, a);
}