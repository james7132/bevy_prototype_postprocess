[[block]]
struct View {
    view: mat4x4<f32>;
    view_inv: mat4x4<f32>;
    proj: mat4x4<f32>;
    proj_inv: mat4x4<f32>;
    view_proj: mat4x4<f32>;
    world_position: vec3<f32>;
};

struct Bloom {
    threshold: f32;
    intensity: f32;
    scatter: f32;
    tint: vec4<f32>;
    clamp: f32;
};

struct ChannelMixing {
    matrix: mat3x3<f32>;
};

[[block]]
struct Uber {
    flags: u32;
    bloom: Bloom;
    channel_mixing: ChannelMixing;
};

[[group(0), binding(0)]]
var view: View;
[[group(0), binding(1)]]
var color_texture: texture_2d<f32>;
[[group(0), binding(2)]]
var color_sampler: sampler;

[[group(1), binding(0)]]
var uber: Uber;

struct Vertex {
    [[builtin(vertex_index)]] index: u32;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] uv: vec2<f32>;
};

[[stage(vertex)]]
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    // Set up a single triangle
    let x = f32((vertex.index & 1u) << 2u);
    let y = f32((vertex.index & 2u) << 1u);
    out.uv = vec2<f32>(x * 0.5, 1.0 - (y * 0.5));
    out.clip_position = vec4<f32>(x - 1.0, y - 1.0, 0.0, 1.0);

    return out;
}

fn saturate(x: vec3<f32>) -> vec3<f32> {
    return max(vec3<f32>(0.0, 0.0, 0.0), min(vec3<f32>(1.0, 1.0, 1.0), x));
}

// Simple ACES tonemapper
fn ACES(x: vec3<f32>) -> vec3<f32> {
    var a: vec3<f32> = vec3<f32>(2.51, 2.51, 2.51);
    var b: vec3<f32> = vec3<f32>(0.03, 0.03, 0.03);
    var c: vec3<f32> = vec3<f32>(2.43, 2.43, 2.43);
    var d: vec3<f32> = vec3<f32>(0.59, 0.59, 0.59);
    var e: vec3<f32> = vec3<f32>(0.14, 0.14, 0.14);
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}

struct FragmentInput {
    [[location(0)]] uv: vec2<f32>;
};

[[stage(fragment)]]
fn fragment(in: FragmentInput) -> [[location(0)]] vec4<f32> {
    var color: vec4<f32> = textureSampleLevel(color_texture, color_sampler, in.uv, 0.0);
    // Channel Mixing.
    color = vec4<f32>(uber.channel_mixing.matrix * color.rgb, color.a);
    // ACES tonemapping
    color = vec4<f32>(ACES(color.rgb), color.a);
    return color;
}
