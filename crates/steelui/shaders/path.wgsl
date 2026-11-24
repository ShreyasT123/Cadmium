struct Globals {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
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
    if in.color.a <= 0.001 {
        discard;
    }
    // Premultiplied alpha output
    return vec4<f32>(in.color.rgb * in.color.a, in.color.a);
}
