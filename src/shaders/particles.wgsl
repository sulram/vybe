// Particle step (compute): integrate every particle on the GPU and wrap it at
// the scene edges. State lives in a storage buffer that never crosses back to
// the CPU — the sketch set the recipe once; this runs every frame on its own
// (the anti-bottleneck stance, literal). One invocation per particle.
//
// This is the point-cloud signal type: a `Signal` whose payload is a buffer.

struct Frame {
    resolution: vec2<f32>, // physical pixels
    mouse: vec2<f32>,      // scene space
    time: f32,             // seconds since start
    dt: f32,               // seconds since last frame
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;

struct Particle {
    pos: vec2<f32>, // scene space
    vel: vec2<f32>, // scene units per second
};

@group(1) @binding(0) var<storage, read_write> particles: array<Particle>;

// group(2): the force stack the sketch composed — static, written once.
// kind: 1 swirl · 2 gravity · 3 radial (attract +, repel −) · 4 orbit.
struct Force {
    kind: f32,
    strength: f32,
    radius: f32,
    source_mouse: f32, // 1 = source is the live mouse
    source: vec2<f32>, // fixed source point, or the gravity vector
    _pad: vec2<f32>,
};

struct Forces {
    count: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
    items: array<Force, 8>,
};

@group(2) @binding(0) var<uniform> forces: Forces;

// Wrap v toroidally into [-half, +half].
fn wrap(v: f32, half: f32) -> f32 {
    let span = 2.0 * half;
    return v - span * floor((v + half) / span);
}

// Sum every force acting on a particle at `pos` this frame.
fn accel_at(pos: vec2<f32>, mouse: vec2<f32>) -> vec2<f32> {
    var a = vec2<f32>(0.0);
    for (var i = 0u; i < forces.count; i = i + 1u) {
        let f = forces.items[i];
        let kind = u32(f.kind);
        if (kind == 1u) {
            // Swirl: rotation around the scene center (divergence-free).
            a = a + vec2<f32>(-pos.y, pos.x) * f.strength;
        } else if (kind == 2u) {
            // Gravity: a constant pull (the source vector is the force).
            a = a + f.source;
        } else {
            // Radial / orbit: relative to a source (mouse or fixed point),
            // fading over `radius`.
            let pole = select(f.source, mouse, f.source_mouse > 0.5);
            let to_t = pole - pos;
            let d = length(to_t);
            let dir = to_t / max(d, 1e-4);
            let influence = 1.0 - smoothstep(0.0, f.radius, d);
            if (kind == 3u) {
                a = a + dir * f.strength * influence; // toward (+) / away (−)
            } else {
                a = a + vec2<f32>(-dir.y, dir.x) * f.strength * influence; // tangential
            }
        }
    }
    return a;
}

@compute @workgroup_size(64)
fn step(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&particles)) {
        return;
    }

    var p = particles[i];

    // The behavior is the sum of the forces the sketch composed (plus the
    // particle's own seeded drift). Empty force stack → the cloud just drifts.
    let accel = accel_at(p.pos, frame.mouse);
    p.pos = p.pos + (p.vel + accel) * frame.dt;

    // Scene bounds: the shorter screen edge is ±0.5; the wider edge sees more
    // world (±0.5 · aspect). Wrap to those so the field is seamless and square.
    let unit = min(frame.resolution.x, frame.resolution.y);
    p.pos.x = wrap(p.pos.x, 0.5 * frame.resolution.x / unit);
    p.pos.y = wrap(p.pos.y, 0.5 * frame.resolution.y / unit);

    particles[i] = p;
}
