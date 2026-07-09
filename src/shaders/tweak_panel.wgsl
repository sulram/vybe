// Tweak panel pass: egui meshes over the finished frame. Vertices arrive in
// egui's coordinate space (points, y-down, origin top-left) with sRGB
// premultiplied colors; we map to clip space and blend premultiplied.

struct Uniforms {
    screen_size_points: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,   // points
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>, // sRGB, premultiplied (unorm from u8)
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>, // linear, premultiplied
};

fn srgb_to_linear(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / 12.92;
    let higher = pow((srgb + 0.055) / 1.055, vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

@vertex
fn vs(in: VsIn) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(
        2.0 * in.pos.x / u.screen_size_points.x - 1.0,
        1.0 - 2.0 * in.pos.y / u.screen_size_points.y,
        0.0,
        1.0,
    );
    out.uv = in.uv;
    out.color = vec4<f32>(srgb_to_linear(in.color.rgb), in.color.a);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return in.color * textureSample(tex, samp, in.uv);
}
