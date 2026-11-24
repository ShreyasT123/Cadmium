struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let x = f32(i32(vertex_index) / 2) * 4.0 - 1.0;
    let y = f32(i32(vertex_index) % 2) * 4.0 - 1.0;

    var out: VertexOut;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@group(0) @binding(0)
var src_tex: texture_2d<f32>;

@group(0) @binding(1)
var src_sampler: sampler;

struct BlurParams {
    texel_size: vec2<f32>,
    offset: f32,
    _pad: f32,
}

@group(1) @binding(0)
var<uniform> params: BlurParams;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let o = params.texel_size * params.offset;
    var sum = textureSample(src_tex, src_sampler, in.uv) * 0.25;
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>(-o.x, -o.y)) * 0.0625;
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>( o.x, -o.y)) * 0.0625;
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>(-o.x,  o.y)) * 0.0625;
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>( o.x,  o.y)) * 0.0625;
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>( 0.0, -o.y)) * 0.125;
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>( 0.0,  o.y)) * 0.125;
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>(-o.x,  0.0)) * 0.125;
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>( o.x,  0.0)) * 0.125;
    return sum;
}
