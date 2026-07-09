// Feedback pass: reads the previous frame's signal, applies the swirl
// (rotation + zoom around the center) and the decay, adds the source signal,
// and writes the new signal. Runs 60x/s on the GPU on its own — the sketch
// only describes the recipe.
//
// The energy source is no longer hardcoded here (that was Phase 0's necessary
// hack): it arrives through the chain, rendered by the shape pass via
// `.render()`. Pure feedback: new = swirl(previous) * decay + source.

// group(0): the frame block — rewritten every frame, shared by every pass.
struct Frame {
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    time: f32,
    dt: f32,   // seconds since last frame — turns per-second knobs into per-frame steps
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;

// group(1): the swirl knobs (static, per SECOND) and this frame's textures.
struct Swirl {
    decay: f32, // fraction of the trail that survives PER SECOND
    angle: f32, // rotation in radians PER SECOND
    scale: f32, // zoom PER SECOND; <1 spirals outward, >1 inward
    _pad: f32,
};

@group(1) @binding(0) var<uniform> swirl: Swirl;
@group(1) @binding(1) var prev_tex: texture_2d<f32>;   // previous frame's signal
@group(1) @binding(2) var samp: sampler;
@group(1) @binding(3) var source_tex: texture_2d<f32>; // the chain's rendered geometry

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle: 3 vertices cover the whole screen (no vertex buffer trick).
// uv is TEXEL space (y-down, v=0 at the top): the fragment's own texel and the
// uv it samples coincide, so no pass ever hides a vertical flip. (NDC is y-up;
// forgetting this inversion once made every feedback frame alternate mirrored.)
@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    var verts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let xy = verts[i];
    var out: VsOut;
    out.pos = vec4<f32>(xy, 0.0, 1.0);
    out.uv = vec2<f32>(xy.x * 0.5 + 0.5, 0.5 - xy.y * 0.5);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // Correct the aspect ratio so the swirl is circular, not elliptical.
    let aspect = frame.resolution.x / frame.resolution.y;
    var c = in.uv - 0.5;
    c.x = c.x * aspect;

    // Per-second knobs → this frame's step, via dt: rotation scales linearly,
    // zoom and decay compound (x per second = x^dt per frame). Framerate drops
    // out — the loop spins and fades at the same rate on any monitor.
    let angle = swirl.angle * frame.dt;
    let scale = pow(swirl.scale, frame.dt);
    let decay = pow(swirl.decay, frame.dt);

    // Rotate and scale around the center → where each pixel comes from in the
    // previous frame.
    let ca = cos(angle);
    let sa = sin(angle);
    let rot = mat2x2<f32>(ca, -sa, sa, ca);
    var src = (rot * c) * scale;
    src.x = src.x / aspect;

    // Carry alpha alongside colour: it is the layer's coverage/energy, so a
    // dark, empty region stays transparent and the compositor's `over` lets the
    // worlds beneath show through it. Additive compositing ignores alpha, so the
    // colour maths — and every existing feedback sketch — is unchanged.
    //
    // The trail: previous frame displaced, dimmed by the decay.
    let trail = textureSample(prev_tex, samp, src + 0.5) * decay;

    // The energy source: whatever the chain rendered this frame.
    let source = textureSample(source_tex, samp, in.uv);

    let out = trail + source;
    // Premultiplied: rgb may glow past 1 (fine), but coverage saturates at 1.
    return vec4<f32>(out.rgb, min(out.a, 1.0));
}
