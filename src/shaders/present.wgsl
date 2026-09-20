// Present pass: the final signal (an Rgba16Float texture) lands on the output —
// the window's swapchain, or the headless frame. This is the stage's one pass,
// so it is also where the keystone lives: the signal is drawn as a quad whose
// four corners the artist may pull, to square a projection on a real wall. At
// rest the quad is the whole output and this is a plain copy.

struct Warp {
    // A homography (rows), mapping the quad's uv to homogeneous clip space.
    // Emitting its w lets the GPU interpolate uv perspective-correctly — a
    // 4-corner keystone is exact for a flat face, no mesh needed.
    row0: vec4<f32>,
    row1: vec4<f32>,
    row2: vec4<f32>,
    feather: f32,    // soft edge, as a fraction of the quad
    keep_alpha: f32, // 1 = write straight alpha (a PNG with transparency)
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> warp: Warp;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// uv is TEXEL space (y-down, v=0 at the top) — see feedback.wgsl for why.
@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let uv = corners[i];
    let p = vec3<f32>(uv, 1.0);
    var out: VsOut;
    out.pos = vec4<f32>(dot(warp.row0.xyz, p), dot(warp.row1.xyz, p), 0.0, dot(warp.row2.xyz, p));
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);

    // Feather: fade the quad's rim to nothing, so neighbouring faces overlap
    // without a seam.
    let rim = min(in.uv, vec2<f32>(1.0) - in.uv);
    let f = max(warp.feather, 1e-6);
    let fade = select(1.0, smoothstep(0.0, f, rim.x) * smoothstep(0.0, f, rim.y), warp.feather > 0.0);

    // The signal is premultiplied. Opaque output reads it over black; a PNG
    // with transparency wants straight alpha back.
    let a = clamp(c.a, 0.0, 1.0);
    let straight = vec4<f32>(c.rgb / max(a, 1e-6), a) * vec4<f32>(1.0, 1.0, 1.0, fade);
    let opaque = vec4<f32>(c.rgb * fade, 1.0);
    return select(opaque, straight, warp.keep_alpha > 0.5);
}
