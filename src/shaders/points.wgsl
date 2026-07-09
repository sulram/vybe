// Point render (instanced): one soft round point per particle, its position
// read from the storage buffer by instance index. No vertex or instance
// buffers cross the bus — the vertex shader indexes the very buffer the
// compute step wrote. Blended additively, so the field glows where it crowds.

struct Frame {
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    time: f32,
    dt: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
};

@group(1) @binding(0) var<storage, read> particles: array<Particle>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>, // -1..+1 across the quad: the SDF domain
};

// Two triangles = one quad, from the vertex index alone (no vertex buffer).
@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    let corner = corners[vi];

    let center = particles[ii].pos;
    let r = 0.0022; // point radius, scene units

    // Scene -> clip: the shorter edge (±0.5 scene) maps to ±1 NDC; the wider
    // edge just sees more world, so points stay round on any aspect.
    let scene = center + corner * r;
    let clip = scene * 2.0 * min(frame.resolution.x, frame.resolution.y) / frame.resolution;

    var out: VsOut;
    out.pos = vec4<f32>(clip, 0.0, 1.0);
    out.local = corner;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // A soft round point: bright core fading to nothing at the rim. Dimmed so
    // that additive overlap reads as a glow gradient, not an instant white-out
    // (a dense cloud stacks many points per pixel).
    let a = 1.0 - smoothstep(0.0, 1.0, length(in.local));
    return vec4<f32>(vec3<f32>(a), a) * 0.4;
}
