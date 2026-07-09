// Composite pass: draws one layer's signal texture onto the screen, once per
// layer, bottom to top. Unlike present.wgsl (an opaque copy of the *final*
// signal), this carries the layer's alpha through untouched — the compositor's
// fixed-function blend state is what turns a stack into a sum (add) or a stack
// of worlds (over). A layer texture is premultiplied: geometry drawn with alpha
// blending onto transparent black leaves rgb already scaled by coverage, so
// both `src + dst` and `src + dst*(1 - src.a)` are correct straight off it.

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// uv is TEXEL space (y-down, v=0 at the top) — see feedback.wgsl for why.
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
    return textureSample(tex, samp, in.uv);
}
