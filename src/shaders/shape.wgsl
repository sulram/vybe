// Shape pass: instanced circles — one quad per instance, the circle cut by an
// SDF in the fragment shader. Everything derives from the instance index and
// two small uniform blocks; no vertex or instance buffers ever cross to the
// GPU (the anti-bottleneck stance, literal).
//
// Position composes additively: grid cell + at(..) + wave(..).
// Scene space: center (0,0), y-up, the shorter screen edge spans -0.5..+0.5
// (TouchDesigner-style). Units are square on any window aspect.

// group(0): the frame block — rewritten every frame, shared by every pass.
struct Frame {
    resolution: vec2<f32>, // physical pixels
    mouse: vec2<f32>,      // scene space
    time: f32,             // seconds since start (wall clock)
    dt: f32,               // seconds since last frame
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;

// group(1): one stroke's knobs — static, written once at build time.
struct Stroke {
    grid: vec2<f32>,       // cols, rows
    radius: f32,           // dot radius, as a fraction of the cell
    soft: f32,             // 0 = crisp disc, 1 = light fading from the center
    place: vec2<f32>,      // base position, scene space
    place_mouse: f32,      // 1 = placement follows the mouse instead
    grow_mouse: f32,       // 1 = the grow epicenter follows the mouse
    grow_at: vec2<f32>,    // grow epicenter, scene space (when not the mouse)
    wave_amp: f32,         // motion amplitude, scene units
    wave_phase: f32,       // turns
    wave_freq: vec2<f32>,  // cycles per second, per axis (0 = still axis)
    wave_shape: f32,       // oscillator index; see oscillate()
    hue: f32,              // degrees on the color wheel
    hue_drift: f32,        // degrees per second
    sat: f32,              // 0 = white (unpainted), 1 = full hue
    falloff_min: f32,      // scene units: full effect inside this distance
    falloff_max: f32,      // scene units: no effect beyond this distance
    falloff_scale: f32,    // radius multiplier at the epicenter
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(1) @binding(0) var<uniform> stroke: Stroke;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>, // -1..+1 across the quad: the SDF domain
};

const TAU: f32 = 6.28318530718;

// One oscillator, one cycle per turn. The menu (TouchDesigner-style):
// 0 sine · 1 cosine · 2 triangle · 3 ramp · 4 square · 5 pulse (25% duty).
fn oscillate(shape: u32, u: f32) -> f32 {
    let p = fract(u);
    var v = 0.0;
    switch shape {
        case 0u: { v = sin(TAU * p); }
        case 1u: { v = cos(TAU * p); }
        case 2u: { v = 1.0 - 4.0 * abs(p - 0.5); }
        case 3u: { v = 2.0 * p - 1.0; }
        case 4u: { v = select(-1.0, 1.0, p < 0.5); }
        default: { v = select(-1.0, 1.0, p < 0.25); }
    }
    return v;
}

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

    // Which cell am I? Everything comes from the instance index. Square
    // cells: the grid is a centered patch whose longer side spans the scene's
    // unit square.
    let col = f32(ii % u32(stroke.grid.x));
    let row = f32(ii / u32(stroke.grid.x));
    let cell = 1.0 / max(stroke.grid.x, stroke.grid.y);
    let cell_center = vec2<f32>(
        (col + 0.5 - stroke.grid.x * 0.5) * cell,
        (row + 0.5 - stroke.grid.y * 0.5) * cell,
    );

    // Position composes additively: cell + base (fixed or mouse) + wave.
    // Wave axes run in quadrature (y a quarter-cycle ahead), so equal
    // frequencies orbit instead of sliding on a diagonal; a 0 Hz axis is still.
    let base = mix(stroke.place, frame.mouse, stroke.place_mouse);
    let u_wave = frame.time * stroke.wave_freq + vec2<f32>(stroke.wave_phase);
    let osc = u32(stroke.wave_shape);
    let wave = stroke.wave_amp * vec2<f32>(
        oscillate(osc, u_wave.x) * step(1e-6, abs(stroke.wave_freq.x)),
        oscillate(osc, u_wave.y + 0.25) * step(1e-6, abs(stroke.wave_freq.y)),
    );
    let center = cell_center + base + wave;

    // The grow knob: swell by proximity to the epicenter (mouse or a fixed
    // point) — full inside falloff_min, nothing beyond falloff_max, smoothstep
    // in between.
    let grow_center = mix(stroke.grow_at, frame.mouse, stroke.grow_mouse);
    let d = distance(center, grow_center);
    let t = 1.0 - smoothstep(stroke.falloff_min, stroke.falloff_max, d);
    let r = stroke.radius * cell * mix(1.0, stroke.falloff_scale, t);

    // Scene -> clip: the shorter edge (±0.5 scene) maps to ±1 NDC; the longer
    // edge just sees more world, so circles stay circles on any aspect.
    let scene = center + corner * r;
    let clip = scene * 2.0 * min(frame.resolution.x, frame.resolution.y) / frame.resolution;

    var out: VsOut;
    out.pos = vec4<f32>(clip, 0.0, 1.0);
    out.local = corner;
    return out;
}

// Hue (degrees) + saturation + value -> RGB.
fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let p = abs(fract(vec3<f32>(h / 360.0) + vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0);
    return v * mix(vec3<f32>(1.0), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), s);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // A circle out of the quad: radial distance; the edge is either a ~1px
    // anti-aliased rim (soft = 0) or a fade toward the center (soft -> 1).
    let d = length(in.local);
    let edge = max(stroke.soft, fwidth(d));
    let alpha = 1.0 - smoothstep(1.0 - edge, 1.0, d);

    let color = hsv2rgb(stroke.hue + stroke.hue_drift * frame.time, stroke.sat, 1.0);
    return vec4<f32>(color, alpha);
}
