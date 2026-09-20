// Image pass: one textured quad in scene space — a frame of a sequence, fitted
// on the CPU (the quad arrives as a center and a half-size). PNGs carry
// straight alpha; the signal world is premultiplied, so the multiply happens
// here, once, on the way in.

struct Frame {
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    time: f32,
    dt: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;

struct Quad {
    center: vec2<f32>, // scene space
    half: vec2<f32>,   // scene units
    alpha: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var<uniform> quad: Quad;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    let corner = corners[vi];
    let scene = quad.center + corner * quad.half;
    let clip = scene * 2.0 * min(frame.resolution.x, frame.resolution.y) / frame.resolution;

    var out: VsOut;
    out.pos = vec4<f32>(clip, 0.0, 1.0);
    // Texel space is y-down: the quad's top edge reads the image's first row.
    out.uv = vec2<f32>(corner.x * 0.5 + 0.5, 0.5 - corner.y * 0.5);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    let a = c.a * quad.alpha;
    return vec4<f32>(c.rgb * a, a);
}
