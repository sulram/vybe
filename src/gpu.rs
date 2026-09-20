//! THE GPU CORE — all of wgpu, hidden behind the knobs (Principle 4). Nothing
//! in this module ever shows up in a sketch.
//!
//! **The shape of it.** A [`Recipe`] flattens into a list of *nodes* in
//! dependency order; every node renders into its own signal texture, and a mix
//! node samples the nodes before it. The last node is the picture; one present
//! pass lands it on the output — a window's swapchain ([`State`]) or a headless
//! frame ([`Headless`]) — through the stage's [`Warp`]. One path for every
//! recipe: the same node list runs a bare circle and a four-scene patch.
//!
//! **Key = identity.** Nodes are keyed (by name where the recipe gives one, by
//! position otherwise). Re-describing a recipe reconciles against the running
//! nodes by key: a node whose key and kind survive keeps its GPU state — its
//! feedback trail, its particle buffer — and only its knobs move. That is what
//! lets a patch be re-described *every frame*.
//!
//! Uniforms are split by cadence: **group(0)** is the frame block, shared by
//! every pass and rewritten 60x/s (resolution, mouse, time); **group(1)** is
//! the knobs of one stroke or one swirl. That split is the anti-bottleneck
//! stance in binary form: per-frame traffic across the boundary is a handful
//! of floats, no matter how much is drawn.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use bytemuck::Zeroable;
use winit::window::Window;

use crate::media::{self, Pixels};
use crate::recipe::{Fit, Force, Form, Image, Recipe, Stroke};
use crate::stage::Warp;
use crate::sugar::{Blend, Osc, Swirl};

/// Signal format: float16 per channel, like Braid/Satin. HDR headroom so the
/// feedback can accumulate without clipping too early.
const SIGNAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// What a headless frame is written as: 8-bit sRGB, ready to be a PNG.
const CAPTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

// ---------------------------------------------------------------------------
// Uniform blocks — the data bridge recipe -> shader.
// ---------------------------------------------------------------------------

/// group(0): what changes every frame. Shared by every pass.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniforms {
    resolution: [f32; 2], // physical pixels
    mouse: [f32; 2],      // scene space
    time: f32,            // seconds since start
    dt: f32,              // seconds since last frame
    _pad: [f32; 2],
}

/// group(1) of the shape pipeline: one stroke's knobs, refreshed every frame.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StrokeUniforms {
    // Field order is alignment-driven: every vec2 (`[f32; 2]`) lands on an
    // 8-byte boundary so the Rust and WGSL layouts agree. Keep scalars packed
    // between vec2s when reordering.
    grid: [f32; 2],
    radius: f32,
    soft: f32,
    place: [f32; 2],
    place_mouse: f32, // 1 = placement follows the mouse
    grow_mouse: f32,  // 1 = the grow epicenter follows the mouse
    grow_at: [f32; 2],
    wave_amp: f32,
    wave_phase: f32,
    wave_freq: [f32; 2],
    wave_shape: f32, // Osc as an index; see shape.wgsl's oscillate()
    hue: f32,
    hue_drift: f32,
    sat: f32,
    falloff_min: f32,
    falloff_max: f32,
    falloff_scale: f32,
    form: f32, // Form as an index: 0 circle · 1 rect
    extent: [f32; 2],
    angle: f32,
    value: f32,
    alpha: f32,
    outline: f32,
}

impl StrokeUniforms {
    fn new(s: &Stroke) -> Self {
        Self {
            grid: [s.cols as f32, s.rows as f32],
            radius: s.radius,
            soft: s.soft,
            place: s.place.point,
            place_mouse: if s.place.from_mouse { 1.0 } else { 0.0 },
            grow_mouse: if s.grow_at.from_mouse { 1.0 } else { 0.0 },
            grow_at: s.grow_at.point,
            wave_amp: s.wave.amp,
            wave_phase: s.wave.phase,
            wave_freq: [s.wave.x, s.wave.y],
            wave_shape: match s.wave.shape {
                Osc::Sine => 0.0,
                Osc::Cosine => 1.0,
                Osc::Triangle => 2.0,
                Osc::Ramp => 3.0,
                Osc::Square => 4.0,
                Osc::Pulse => 5.0,
            },
            hue: s.hue.base,
            hue_drift: s.hue.drift,
            sat: s.sat,
            falloff_min: s.falloff.min,
            falloff_max: s.falloff.max,
            falloff_scale: s.falloff.scale,
            form: match s.form {
                Form::Circle => 0.0,
                Form::Rect => 1.0,
            },
            extent: s.extent,
            angle: s.angle,
            value: s.value,
            alpha: s.alpha,
            outline: s.outline,
        }
    }
}

/// group(1) of the feedback pipeline: the swirl knobs.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SwirlUniforms {
    decay: f32,
    angle: f32,
    scale: f32,
    _pad: f32,
}

impl SwirlUniforms {
    fn new(swirl: Swirl) -> Self {
        Self {
            decay: swirl.decay,
            angle: swirl.angle,
            scale: swirl.scale,
            _pad: 0.0,
        }
    }
}

/// The knob of one mix input (composite.wgsl): its opacity.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LayerUniforms {
    alpha: f32,
    _pad: [f32; 3],
}

/// An image's quad (image.wgsl), fitted on the CPU.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadUniforms {
    center: [f32; 2],
    half: [f32; 2],
    alpha: f32,
    _pad: [f32; 3],
}

/// The stage's warp (present.wgsl): a homography, a feather, and whether the
/// output keeps its alpha.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WarpUniforms {
    rows: [[f32; 4]; 3],
    feather: f32,
    keep_alpha: f32,
    _pad: [f32; 2],
}

/// One particle's state, in the storage buffer the compute shader steps.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Particle {
    pos: [f32; 2], // scene space
    vel: [f32; 2], // scene units per second
}

/// Seeds `count` particles across the scene with random positions and drift —
/// deterministically, from a hash of the index (reproducible; no `rand` dep).
fn seed_particles(count: u32) -> Vec<Particle> {
    // A cheap integer hash (PCG-style finalizer) → [0, 1).
    fn rnd(seed: u32) -> f32 {
        let mut x = seed.wrapping_mul(747796405).wrapping_add(2891336453);
        x = ((x >> ((x >> 28).wrapping_add(4))) ^ x).wrapping_mul(277803737);
        x = (x >> 22) ^ x;
        (x as f32) / (u32::MAX as f32)
    }
    (0..count)
        .map(|i| {
            let angle = rnd(i * 4 + 2) * std::f32::consts::TAU;
            let speed = 0.03 + rnd(i * 4 + 3) * 0.12; // scene units per second
            Particle {
                pos: [rnd(i * 4) - 0.5, rnd(i * 4 + 1) - 0.5],
                vel: [angle.cos() * speed, angle.sin() * speed],
            }
        })
        .collect()
}

/// Max forces a cloud can carry (a uniform-array bound; plenty for a sketch).
const MAX_FORCES: usize = 8;

/// One force encoded for the GPU. `kind`: 1 swirl · 2 gravity · 3 radial ·
/// 4 orbit (see particles.wgsl). Flat and 16-byte-friendly for a uniform array.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuForce {
    kind: f32,
    strength: f32,
    radius: f32,
    source_mouse: f32, // 1 = the source is the live mouse
    source: [f32; 2],  // fixed source point, or the gravity vector
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ForcesUniforms {
    count: u32,
    _pad: [u32; 3],
    items: [GpuForce; MAX_FORCES],
}

fn encode_forces(forces: &[Force]) -> ForcesUniforms {
    let mut items = [bytemuck::Zeroable::zeroed(); MAX_FORCES];
    for (slot, force) in items.iter_mut().zip(forces) {
        *slot = match *force {
            Force::Swirl { strength } => GpuForce {
                kind: 1.0,
                strength,
                ..bytemuck::Zeroable::zeroed()
            },
            Force::Gravity { dir } => GpuForce {
                kind: 2.0,
                source: dir,
                ..bytemuck::Zeroable::zeroed()
            },
            Force::Radial {
                at,
                strength,
                radius,
            } => GpuForce {
                kind: 3.0,
                strength,
                radius,
                source_mouse: if at.from_mouse { 1.0 } else { 0.0 },
                source: at.point,
                _pad: [0.0; 2],
            },
            Force::Orbit {
                at,
                strength,
                radius,
            } => GpuForce {
                kind: 4.0,
                strength,
                radius,
                source_mouse: if at.from_mouse { 1.0 } else { 0.0 },
                source: at.point,
                _pad: [0.0; 2],
            },
        };
    }
    ForcesUniforms {
        count: forces.len().min(MAX_FORCES) as u32,
        _pad: [0; 3],
        items,
    }
}

// ---------------------------------------------------------------------------
// The kit — every pipeline, layout and sampler, built once per device. All
// nodes render into `SIGNAL_FORMAT`, so one pipeline per shader serves them all.
// ---------------------------------------------------------------------------

struct Kit {
    frame_layout: wgpu::BindGroupLayout,
    stroke_layout: wgpu::BindGroupLayout,
    feedback_layout: wgpu::BindGroupLayout,
    /// A texture, its sampler, and one small uniform block — the layout shared
    /// by everything that samples a signal: mix, image, present.
    sampled_layout: wgpu::BindGroupLayout,
    shape_pipeline: wgpu::RenderPipeline,
    feedback_pipeline: wgpu::RenderPipeline,
    /// One compositor pipeline per [`Blend`] — same shader, different blend
    /// state — so a stack can mix modes without rebuilding anything per frame.
    add_pipeline: wgpu::RenderPipeline,
    over_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl Kit {
    fn new(device: &wgpu::Device) -> Self {
        let shader = |label: &str, source: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        };
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame bind group layout"),
            entries: &[uniform_entry(0)],
        });
        let stroke_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stroke bind group layout"),
            entries: &[uniform_entry(0)],
        });
        let feedback_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("feedback bind group layout"),
            entries: &[
                uniform_entry(0),
                texture_entry(1),
                sampler_entry(2),
                texture_entry(3),
            ],
        });
        let sampled_layout = sampled_bind_group_layout(device);

        let shape_pipeline = make_pipeline(
            device,
            &shader("shape.wgsl", include_str!("shaders/shape.wgsl")),
            &[&frame_layout, &stroke_layout],
            SIGNAL_FORMAT,
            // The SDF's anti-aliased rim needs alpha blending.
            Some(wgpu::BlendState::ALPHA_BLENDING),
            "shape pipeline",
        );
        let feedback_pipeline = make_pipeline(
            device,
            &shader("feedback.wgsl", include_str!("shaders/feedback.wgsl")),
            &[&frame_layout, &feedback_layout],
            SIGNAL_FORMAT,
            None,
            "feedback pipeline",
        );
        let composite = shader("composite.wgsl", include_str!("shaders/composite.wgsl"));
        let mix_pipeline = |blend, label| {
            make_pipeline(
                device,
                &composite,
                &[&sampled_layout],
                SIGNAL_FORMAT,
                Some(blend_state(blend)),
                label,
            )
        };
        let add_pipeline = mix_pipeline(Blend::Add, "composite pipeline (add)");
        let over_pipeline = mix_pipeline(Blend::Over, "composite pipeline (over)");
        let image_pipeline = make_pipeline(
            device,
            &shader("image.wgsl", include_str!("shaders/image.wgsl")),
            &[&frame_layout, &sampled_layout],
            SIGNAL_FORMAT,
            // The shader premultiplies; `over` reads straight off that.
            Some(blend_state(Blend::Over)),
            "image pipeline",
        );

        Self {
            frame_layout,
            stroke_layout,
            feedback_layout,
            sampled_layout,
            shape_pipeline,
            feedback_pipeline,
            add_pipeline,
            over_pipeline,
            image_pipeline,
            sampler: make_linear_clamp_sampler(device),
        }
    }

    /// The compositor pipeline for a layer's blend mode.
    fn mix_pipeline(&self, blend: Blend) -> &wgpu::RenderPipeline {
        match blend {
            Blend::Add => &self.add_pipeline,
            Blend::Over => &self.over_pipeline,
        }
    }
}

// ---------------------------------------------------------------------------
// Nodes — one per world in the flattened recipe.
// ---------------------------------------------------------------------------

/// One stroke, ready to draw: its uniform buffer and bind group.
struct StrokeDraw {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// The strokes a node draws (in painter's order) and their GPU-side twins.
/// `strokes` is the single source of truth — knobs are LIVE: refreshed to the
/// GPU every frame, so anything may retune them while the sketch runs (the
/// tweak panel, a patch's Scalars, OSC).
struct ShapePass {
    strokes: Vec<Stroke>,
    draws: Vec<StrokeDraw>,
}

impl ShapePass {
    fn new(device: &wgpu::Device, kit: &Kit, strokes: Vec<Stroke>) -> Self {
        Self {
            draws: build_strokes(device, &kit.stroke_layout, strokes.len()),
            strokes,
        }
    }

    /// Swap in re-described strokes. GPU twins are reused; only a change in
    /// stroke count allocates.
    fn replace(&mut self, device: &wgpu::Device, kit: &Kit, strokes: Vec<Stroke>) {
        if strokes.len() != self.draws.len() {
            self.draws = build_strokes(device, &kit.stroke_layout, strokes.len());
        }
        self.strokes = strokes;
    }

    /// Push this frame's knob values to the GPU (a handful of floats each).
    fn refresh(&self, queue: &wgpu::Queue) {
        for (draw, stroke) in self.draws.iter().zip(&self.strokes) {
            queue.write_buffer(
                &draw.buffer,
                0,
                bytemuck::bytes_of(&StrokeUniforms::new(stroke)),
            );
        }
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, kit: &Kit, frame_bg: &wgpu::BindGroup) {
        pass.set_pipeline(&kit.shape_pipeline);
        pass.set_bind_group(0, frame_bg, &[]);
        for (draw, stroke) in self.draws.iter().zip(&self.strokes) {
            pass.set_bind_group(1, &draw.bind_group, &[]);
            pass.draw(0..6, 0..stroke.cols * stroke.rows); // one quad per instance
        }
    }
}

/// A recipe, flattened: what one node is, with its inputs already resolved to
/// positions in the node list (always earlier ones — the list is in dependency
/// order by construction).
enum Desc {
    Shapes(Vec<Stroke>),
    Feedback { source: Vec<Stroke>, swirl: Swirl },
    Points { count: u32, forces: Vec<Force> },
    Image(Image),
    Mix(Vec<MixDesc>),
}

struct MixDesc {
    from: usize,
    blend: Blend,
    alpha: f32,
}

/// Flattens `recipe` into `out`, children before parents, and returns the
/// position of the node it became. A [`Recipe::Named`] already in the list is
/// not added again — it *is* that node (key = identity); an unnamed world is
/// keyed by its path in the tree, so a re-described chain of the same shape
/// lands on the same keys.
fn flatten(recipe: Recipe, key: String, out: &mut Vec<(String, Desc)>) -> usize {
    let desc = match recipe {
        Recipe::Named(name, inner) => {
            return match out.iter().position(|(k, _)| *k == name) {
                Some(index) => index,
                None => flatten(*inner, name, out),
            };
        }
        Recipe::Composite(layers) => Desc::Mix(
            layers
                .into_iter()
                .enumerate()
                .map(|(i, layer)| MixDesc {
                    from: flatten(layer.recipe, format!("{key}/{i}"), out),
                    blend: layer.blend,
                    alpha: layer.alpha,
                })
                .collect(),
        ),
        Recipe::Shapes(strokes) => Desc::Shapes(strokes),
        Recipe::Feedback { source, swirl } => Desc::Feedback { source, swirl },
        Recipe::Points { count, forces } => Desc::Points { count, forces },
        Recipe::Image(image) => Desc::Image(image),
    };
    out.push((key, desc));
    out.len() - 1
}

/// One world, rendering into its own signal texture(s).
struct Node {
    key: String,
    kind: Kind,
    /// Changes whenever this node's textures are (re)created — what a bind
    /// group sampling them is checked against.
    generation: u64,
}

enum Kind {
    /// Geometry, one pass.
    Shapes {
        shapes: ShapePass,
        target: wgpu::TextureView,
    },
    /// Two passes: strokes -> source signal; swirl(previous) + source -> next
    /// signal. The ping-pong stays hidden behind the knob.
    Feedback {
        shapes: ShapePass,
        swirl_uniforms: wgpu::Buffer,
        targets: FeedbackTargets,
    },
    /// The point-cloud signal type: a compute step + an instanced draw.
    Points {
        pass: Box<PointsPass>,
        target: wgpu::TextureView,
    },
    /// One frame of a sequence, as a textured quad.
    Image {
        image: Image,
        sizes: Vec<[u32; 2]>,
        /// `bgs[i]` samples frame `i`.
        bgs: Vec<wgpu::BindGroup>,
        uniforms: wgpu::Buffer,
        target: wgpu::TextureView,
    },
    /// A stack: samples earlier nodes, bottom to top, each by its blend and
    /// opacity — feedback on one layer, still geometry on another, no smearing
    /// across them.
    Mix {
        inputs: Vec<MixInput>,
        target: wgpu::TextureView,
    },
}

struct MixInput {
    from: usize,
    blend: Blend,
    alpha: f32,
    uniforms: wgpu::Buffer,
    sampled: Sampled,
}

/// Bind groups sampling another node's views, rebuilt only when that node's
/// textures change (its generation moves).
#[derive(Default)]
struct Sampled {
    generation: u64,
    bgs: Vec<wgpu::BindGroup>,
}

impl Sampled {
    /// The bind group sampling `node`'s current view through `uniforms`.
    fn of(
        &mut self,
        device: &wgpu::Device,
        kit: &Kit,
        node: &Node,
        uniforms: &wgpu::Buffer,
    ) -> &wgpu::BindGroup {
        if self.generation != node.generation {
            self.generation = node.generation;
            self.bgs = node
                .views()
                .iter()
                .map(|view| {
                    sampled_bind_group(device, &kit.sampled_layout, view, &kit.sampler, uniforms)
                })
                .collect();
        }
        &self.bgs[node.current()]
    }
}

impl Node {
    /// The texture(s) this node renders into. A feedback node has two (the
    /// ping-pong); [`Node::current`] says which holds this frame.
    fn views(&self) -> &[wgpu::TextureView] {
        match &self.kind {
            Kind::Shapes { target, .. }
            | Kind::Points { target, .. }
            | Kind::Image { target, .. }
            | Kind::Mix { target, .. } => std::slice::from_ref(target),
            Kind::Feedback { targets, .. } => &targets.ping_pong.views,
        }
    }

    fn current(&self) -> usize {
        match &self.kind {
            Kind::Feedback { targets, .. } => targets.ping_pong.front,
            _ => 0,
        }
    }

    /// Can this running node become `desc` without losing its state?
    fn accepts(&self, desc: &Desc) -> bool {
        match (&self.kind, desc) {
            (Kind::Shapes { .. }, Desc::Shapes(_))
            | (Kind::Feedback { .. }, Desc::Feedback { .. })
            | (Kind::Mix { .. }, Desc::Mix(_)) => true,
            // A cloud of another size is another buffer.
            (Kind::Points { pass, .. }, Desc::Points { count, .. }) => pass.count == *count,
            // Another sequence is another set of textures.
            (Kind::Image { image, .. }, Desc::Image(next)) => {
                Arc::ptr_eq(&image.frames, &next.frames) || image.frames == next.frames
            }
            _ => false,
        }
    }
}

/// Everything that depends on the output size — rebuilt on resize (the trail
/// is lost then; fine for now).
struct FeedbackTargets {
    source: wgpu::TextureView,
    ping_pong: PingPong,
    /// `feedback_bgs[i]` reads `views[i]` as the previous frame.
    feedback_bgs: [wgpu::BindGroup; 2],
}

/// A ping-pong pair of signal-textures. Each frame we read from one and write
/// to the other, then swap. Resolves the GPU aliasing and is the correct
/// pattern anyway — hidden behind the `decay` knob.
struct PingPong {
    // Views keep their textures alive in wgpu.
    views: [wgpu::TextureView; 2],
    /// The side holding the newest frame: read as "previous" while rendering,
    /// and — once the pass has written the other side and swapped — the output.
    front: usize,
}

impl PingPong {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        Self {
            views: [
                make_signal_texture(device, width, height, "signal ping"),
                make_signal_texture(device, width, height, "signal pong"),
            ],
            front: 0,
        }
    }

    fn write(&self) -> &wgpu::TextureView {
        &self.views[1 - self.front]
    }
    fn swap(&mut self) {
        self.front = 1 - self.front;
    }
}

/// A decoded image on the GPU. Shared: a sequence re-described, or used by two
/// nodes, uploads once.
struct ImageTex {
    view: wgpu::TextureView,
    size: [u32; 2],
}

// ---------------------------------------------------------------------------
// Engine — the device, the kit, and the running nodes. Knows nothing of where
// the picture goes; [`State`] and [`Headless`] each put a [`Presenter`] after it.
// ---------------------------------------------------------------------------

pub(crate) struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    kit: Kit,
    frame_uniforms: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
    nodes: Vec<Node>,
    images: HashMap<PathBuf, Rc<ImageTex>>,
    width: u32,
    height: u32,
    /// Mouse in scene space. Starts far away so nothing reacts before the
    /// cursor first enters the window.
    mouse: [f32; 2],
    generations: u64,
}

impl Engine {
    fn new(device: wgpu::Device, queue: wgpu::Queue, width: u32, height: u32) -> Self {
        let kit = Kit::new(&device);
        // The frame block: one small buffer shared by every pass (group 0).
        let frame_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame bind group"),
            layout: &kit.frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniforms.as_entire_binding(),
            }],
        });
        Self {
            device,
            queue,
            kit,
            frame_uniforms,
            frame_bg,
            nodes: Vec::new(),
            images: HashMap::new(),
            width,
            height,
            mouse: [1e9, 1e9], // far away: at rest until the cursor shows up
            generations: 0,
        }
    }

    /// Decodes and uploads images ahead of their first frame, so a sequence
    /// that only appears mid-performance doesn't stall the scene it enters.
    pub(crate) fn preload(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.image(path);
        }
    }

    /// The running picture becomes `recipe`. Nodes are reconciled by key: one
    /// whose key and kind survive keeps its GPU state and only its knobs move
    /// (cheap — this runs every frame under a patch); anything else is built.
    pub(crate) fn set_recipe(&mut self, recipe: Recipe) {
        let mut descs = Vec::new();
        flatten(recipe, "out".to_owned(), &mut descs);

        let mut running: Vec<Option<Node>> = std::mem::take(&mut self.nodes)
            .into_iter()
            .map(Some)
            .collect();
        for (key, desc) in descs {
            let kept = running
                .iter_mut()
                .find(|n| n.as_ref().is_some_and(|n| n.key == key && n.accepts(&desc)))
                .and_then(Option::take);
            let node = match kept {
                Some(mut node) => {
                    self.update(&mut node, desc);
                    node
                }
                None => self.build(key, desc),
            };
            self.nodes.push(node);
        }
    }

    /// Moves a kept node's knobs to `desc` ([`Node::accepts`] vouched for it).
    fn update(&self, node: &mut Node, desc: Desc) {
        match (&mut node.kind, desc) {
            (Kind::Shapes { shapes, .. }, Desc::Shapes(strokes)) => {
                shapes.replace(&self.device, &self.kit, strokes);
            }
            (
                Kind::Feedback {
                    shapes,
                    swirl_uniforms,
                    ..
                },
                Desc::Feedback { source, swirl },
            ) => {
                shapes.replace(&self.device, &self.kit, source);
                self.queue.write_buffer(
                    swirl_uniforms,
                    0,
                    bytemuck::bytes_of(&SwirlUniforms::new(swirl)),
                );
            }
            // Same-size cloud: only the forces changed — rewrite the tiny force
            // uniform and keep the particles' live positions.
            (Kind::Points { pass, .. }, Desc::Points { forces, .. }) => {
                self.queue.write_buffer(
                    &pass.forces_uniforms,
                    0,
                    bytemuck::bytes_of(&encode_forces(&forces)),
                );
            }
            (Kind::Image { image, .. }, Desc::Image(next)) => *image = next,
            (Kind::Mix { inputs, .. }, Desc::Mix(descs)) => {
                if inputs.len() == descs.len() {
                    for (input, desc) in inputs.iter_mut().zip(descs) {
                        input.from = desc.from;
                        input.blend = desc.blend;
                        input.alpha = desc.alpha;
                    }
                } else {
                    *inputs = self.mix_inputs(descs);
                }
            }
            _ => unreachable!("Node::accepts vouches for the pairing"),
        }
    }

    fn build(&mut self, key: String, desc: Desc) -> Node {
        let (w, h) = (self.width, self.height);
        let kind = match desc {
            Desc::Shapes(strokes) => Kind::Shapes {
                shapes: ShapePass::new(&self.device, &self.kit, strokes),
                target: make_signal_texture(&self.device, w, h, "shapes target"),
            },
            Desc::Feedback { source, swirl } => {
                let swirl_uniforms = uniform_buffer(
                    &self.device,
                    &self.queue,
                    "swirl uniforms",
                    &SwirlUniforms::new(swirl),
                );
                Kind::Feedback {
                    shapes: ShapePass::new(&self.device, &self.kit, source),
                    targets: build_feedback_targets(&self.device, &self.kit, w, h, &swirl_uniforms),
                    swirl_uniforms,
                }
            }
            Desc::Points { count, forces } => Kind::Points {
                pass: Box::new(build_points(
                    &self.device,
                    &self.queue,
                    &self.frame_uniforms,
                    &self.kit.frame_layout,
                    SIGNAL_FORMAT,
                    count,
                    forces,
                )),
                target: make_signal_texture(&self.device, w, h, "points target"),
            },
            Desc::Image(image) => {
                let uniforms = uniform_buffer(
                    &self.device,
                    &self.queue,
                    "image quad",
                    &QuadUniforms::zeroed(),
                );
                let frames = image.frames.clone();
                let texs: Vec<_> = frames.iter().map(|path| self.image(path)).collect();
                Kind::Image {
                    image,
                    sizes: texs.iter().map(|t| t.size).collect(),
                    bgs: texs
                        .iter()
                        .map(|t| {
                            sampled_bind_group(
                                &self.device,
                                &self.kit.sampled_layout,
                                &t.view,
                                &self.kit.sampler,
                                &uniforms,
                            )
                        })
                        .collect(),
                    uniforms,
                    target: make_signal_texture(&self.device, w, h, "image target"),
                }
            }
            Desc::Mix(descs) => Kind::Mix {
                inputs: self.mix_inputs(descs),
                target: make_signal_texture(&self.device, w, h, "mix target"),
            },
        };
        self.generations += 1;
        Node {
            key,
            kind,
            generation: self.generations,
        }
    }

    fn mix_inputs(&self, descs: Vec<MixDesc>) -> Vec<MixInput> {
        descs
            .into_iter()
            .map(|desc| MixInput {
                from: desc.from,
                blend: desc.blend,
                alpha: desc.alpha,
                uniforms: uniform_buffer(
                    &self.device,
                    &self.queue,
                    "layer uniforms",
                    &LayerUniforms::zeroed(),
                ),
                sampled: Sampled::default(),
            })
            .collect()
    }

    /// The texture of an image file — decoded and uploaded once, then shared.
    /// A file that won't load is one clear pixel and one line on stderr: the
    /// picture goes on (Principle 3); `vybe check` is where it's an error.
    fn image(&mut self, path: &Path) -> Rc<ImageTex> {
        if let Some(tex) = self.images.get(path) {
            return tex.clone();
        }
        let pixels = media::load_png(path).unwrap_or_else(|e| {
            eprintln!("vybe: {e}");
            Pixels {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            }
        });
        let size = wgpu::Extent3d {
            width: pixels.width,
            height: pixels.height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // sRGB in the file, linear in the signal world: the sampler decodes.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * pixels.width),
                rows_per_image: Some(pixels.height),
            },
            size,
        );
        let tex = Rc::new(ImageTex {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            size: [pixels.width, pixels.height],
        });
        self.images.insert(path.to_owned(), tex.clone());
        tex
    }

    /// Only the signal textures are size-dependent; pipelines, knob buffers and
    /// particle clouds survive. (A feedback trail is lost; fine for now.)
    fn resize(&mut self, width: u32, height: u32) {
        (self.width, self.height) = (width, height);
        for node in &mut self.nodes {
            let fresh = |label| make_signal_texture(&self.device, width, height, label);
            match &mut node.kind {
                Kind::Shapes { target, .. } => *target = fresh("shapes target"),
                Kind::Points { target, .. } => *target = fresh("points target"),
                Kind::Image { target, .. } => *target = fresh("image target"),
                Kind::Mix { target, .. } => *target = fresh("mix target"),
                Kind::Feedback {
                    swirl_uniforms,
                    targets,
                    ..
                } => {
                    *targets = build_feedback_targets(
                        &self.device,
                        &self.kit,
                        width,
                        height,
                        swirl_uniforms,
                    );
                }
            }
            self.generations += 1;
            node.generation = self.generations;
        }
    }

    /// Pixels (y-down, origin top-left) -> scene space (y-up, centered, the
    /// shorter edge spanning -0.5..+0.5). The sketch never sees a pixel.
    fn to_scene(&self, x: f32, y: f32) -> [f32; 2] {
        let (w, h) = (self.width as f32, self.height as f32);
        let unit = w.min(h);
        [(x - w * 0.5) / unit, (h * 0.5 - y) / unit]
    }

    /// Renders every node, in order, for the frame at `time`.
    fn render(&mut self, encoder: &mut wgpu::CommandEncoder, time: f32, dt: f32) {
        let u = FrameUniforms {
            resolution: [self.width as f32, self.height as f32],
            mouse: self.mouse,
            time,
            dt,
            _pad: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.frame_uniforms, 0, bytemuck::bytes_of(&u));

        let Self {
            device,
            queue,
            kit,
            frame_bg,
            nodes,
            ..
        } = self;
        let (device, queue, kit, frame_bg) = (&*device, &*queue, &*kit, &*frame_bg);
        let frame_size = [u.resolution[0], u.resolution[1]];
        for i in 0..nodes.len() {
            // A node samples only nodes before it, so the list splits cleanly
            // into "already rendered this frame" and "this one".
            let (before, rest) = nodes.split_at_mut(i);
            let clear = wgpu::Color::TRANSPARENT;
            match &mut rest[0].kind {
                Kind::Shapes { shapes, target } => {
                    shapes.refresh(queue); // knobs are live
                    let mut pass = begin_pass(encoder, target, "shapes pass", clear);
                    shapes.draw(&mut pass, kit, frame_bg);
                }
                Kind::Feedback {
                    shapes, targets, ..
                } => {
                    shapes.refresh(queue); // knobs are live
                    // Pass 1: the chain's geometry -> the source signal.
                    {
                        let mut pass = begin_pass(encoder, &targets.source, "source pass", clear);
                        shapes.draw(&mut pass, kit, frame_bg);
                    }
                    // Pass 2: swirl(previous) + source -> the next signal.
                    {
                        let mut pass =
                            begin_pass(encoder, targets.ping_pong.write(), "feedback pass", clear);
                        pass.set_pipeline(&kit.feedback_pipeline);
                        pass.set_bind_group(0, frame_bg, &[]);
                        pass.set_bind_group(1, &targets.feedback_bgs[targets.ping_pong.front], &[]);
                        pass.draw(0..3, 0..1); // fullscreen triangle
                    }
                    // Swap: what we wrote is this frame's output, and next
                    // frame's "previous".
                    targets.ping_pong.swap();
                }
                Kind::Points { pass, target } => pass.render(encoder, target, frame_bg, clear),
                Kind::Image {
                    image,
                    sizes,
                    bgs,
                    uniforms,
                    target,
                } => {
                    let mut pass = begin_pass(encoder, target, "image pass", clear);
                    // An empty sequence draws nothing (Principle 2).
                    if !bgs.is_empty() {
                        let index = image.index.min(bgs.len() - 1);
                        let quad = fit_quad(image, sizes[index], frame_size);
                        queue.write_buffer(uniforms, 0, bytemuck::bytes_of(&quad));
                        pass.set_pipeline(&kit.image_pipeline);
                        pass.set_bind_group(0, frame_bg, &[]);
                        pass.set_bind_group(1, &bgs[index], &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
                Kind::Mix { inputs, target } => {
                    // One pass draws the inputs bottom to top — the compositor
                    // is composite.wgsl, each input drawn with the pipeline for
                    // its own blend (over by alpha, or additive glow).
                    let mut pass = begin_pass(encoder, target, "mix pass", clear);
                    for input in inputs.iter_mut() {
                        let layer = LayerUniforms {
                            alpha: input.alpha,
                            _pad: [0.0; 3],
                        };
                        queue.write_buffer(&input.uniforms, 0, bytemuck::bytes_of(&layer));
                        let bg =
                            input
                                .sampled
                                .of(device, kit, &before[input.from], &input.uniforms);
                        pass.set_pipeline(kit.mix_pipeline(input.blend));
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..3, 0..1); // fullscreen triangle
                    }
                }
            }
        }
    }
}

/// An image's quad: contained in its [`Fit`] box, scaled, placed.
fn fit_quad(image: &Image, size: [u32; 2], frame: [f32; 2]) -> QuadUniforms {
    let unit = frame[0].min(frame[1]);
    let bounds = match image.fit {
        Fit::Unit => [1.0, 1.0],
        Fit::Frame => [frame[0] / unit, frame[1] / unit],
    };
    let (w, h) = (size[0].max(1) as f32, size[1].max(1) as f32);
    let scale = (bounds[0] / w).min(bounds[1] / h) * image.size * 0.5;
    QuadUniforms {
        center: image.place,
        half: [w * scale, h * scale],
        alpha: image.alpha,
        _pad: [0.0; 3],
    }
}

// ---------------------------------------------------------------------------
// Presenter — the stage's one pass: the final signal, through the warp, onto
// an output of some format.
// ---------------------------------------------------------------------------

struct Presenter {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    sampled: Sampled,
    warp: Warp,
    keep_alpha: bool,
}

impl Presenter {
    fn new(engine: &Engine, format: wgpu::TextureFormat, keep_alpha: bool) -> Self {
        let shader = engine
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("present.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/present.wgsl").into()),
            });
        Self {
            pipeline: make_pipeline(
                &engine.device,
                &shader,
                &[&engine.kit.sampled_layout],
                format,
                None,
                "present pipeline",
            ),
            uniforms: uniform_buffer(
                &engine.device,
                &engine.queue,
                "warp uniforms",
                &WarpUniforms::zeroed(),
            ),
            sampled: Sampled::default(),
            warp: Warp::default(),
            keep_alpha,
        }
    }

    /// Draws the engine's last node into `view`. Outside the warped quad the
    /// output is black (or clear, when it keeps its alpha).
    fn present(
        &mut self,
        engine: &Engine,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        let clear = if self.keep_alpha {
            wgpu::Color::TRANSPARENT
        } else {
            wgpu::Color::BLACK
        };
        let mut pass = begin_pass(encoder, view, "present pass", clear);
        let Some(picture) = engine.nodes.last() else {
            return;
        };
        let warp = WarpUniforms {
            rows: self.warp.rows(),
            feather: self.warp.feather,
            keep_alpha: if self.keep_alpha { 1.0 } else { 0.0 },
            _pad: [0.0; 2],
        };
        engine
            .queue
            .write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&warp));
        let bg = self
            .sampled
            .of(&engine.device, &engine.kit, picture, &self.uniforms);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Brings up a device. With a `surface` the adapter is one that can present to
/// it; without, any adapter will do (headless).
async fn request_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .expect("no compatible GPU adapter");
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("vybe device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        })
        .await
        .expect("failed to request device");
    (adapter, device, queue)
}

// ---------------------------------------------------------------------------
// Overlay — the one seam for front-ends drawn over a running sketch (the
// tweak panel today; a node editor or debug HUD tomorrow). The core knows
// this trait and nothing else: no UI library ever touches the engine.
// ---------------------------------------------------------------------------

pub(crate) trait Overlay {
    /// Sees window events before the scene does. Return true to consume one
    /// (e.g. the pointer is over a slider).
    fn event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool;
    /// Draws over the finished frame.
    fn frame(&mut self, ctx: OverlayFrame<'_>);
}

/// Everything an overlay may touch, for exactly one frame.
#[cfg_attr(not(feature = "tweak"), allow(dead_code))] // read only by overlays
pub(crate) struct OverlayFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub view: &'a wgpu::TextureView,
    pub size_px: [u32; 2],
    pub window: &'a Window,
}

// ---------------------------------------------------------------------------
// State — the engine, presenting to a window.
// ---------------------------------------------------------------------------

pub(crate) struct State {
    pub(crate) window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    engine: Engine,
    presenter: Presenter,
    overlay: Option<Box<dyn Overlay>>,
}

impl State {
    pub(crate) async fn new(window: Arc<Window>, recipe: Recipe) -> Self {
        let size = window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let (adapter, device, queue) = request_device(&instance, Some(&surface)).await;

        // Configure the window's swapchain.
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::default(),
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo, // vsync
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mut engine = Engine::new(device, queue, width, height);
        engine.set_recipe(recipe);
        let presenter = Presenter::new(&engine, format, false);

        Self {
            window,
            surface,
            config,
            engine,
            presenter,
            overlay: None,
        }
    }

    /// Attaches a front-end drawn over the running sketch.
    #[cfg_attr(not(feature = "tweak"), allow(dead_code))]
    pub(crate) fn set_overlay(&mut self, overlay: Box<dyn Overlay>) {
        self.overlay = Some(overlay);
    }

    #[cfg_attr(not(feature = "tweak"), allow(dead_code))]
    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.engine.device
    }

    #[cfg_attr(not(feature = "tweak"), allow(dead_code))]
    pub(crate) fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Routes a window event to the overlay first. Returns true when consumed.
    pub(crate) fn overlay_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        let window = self.window.clone();
        match &mut self.overlay {
            Some(overlay) => overlay.event(&window, event),
            None => false,
        }
    }

    pub(crate) fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub(crate) fn set_warp(&mut self, warp: Warp) {
        self.presenter.warp = warp;
    }

    pub(crate) fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.engine.device, &self.config);
        self.engine.resize(size.width, size.height);
    }

    /// The pointer moved: returns where, in scene space, and feeds the mouse
    /// signal unless `hold` (an overlay owns the pointer right now).
    pub(crate) fn pointer(
        &mut self,
        position: winit::dpi::PhysicalPosition<f64>,
        hold: bool,
    ) -> [f32; 2] {
        let scene = self.engine.to_scene(position.x as f32, position.y as f32);
        if !hold {
            self.engine.mouse = scene;
        }
        scene
    }

    /// The mouse signal at rest: far away, so proximity effects go quiet when
    /// the cursor leaves the window (matches the start-up state).
    pub(crate) fn rest_mouse(&mut self) {
        self.engine.mouse = [1e9, 1e9];
    }

    pub(crate) fn render(&mut self, time: f32, dt: f32) {
        use wgpu::CurrentSurfaceTexture::*;
        let frame = match self.surface.get_current_texture() {
            Success(f) | Suboptimal(f) => f,
            // Swapchain out of date (e.g. during resize) or unavailable — skip the frame.
            Outdated | Lost => {
                self.surface.configure(&self.engine.device, &self.config);
                return;
            }
            Timeout | Occluded | Validation => return,
        };
        let screen = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("frame"),
                });

        self.engine.render(&mut encoder, time, dt);
        self.presenter.present(&self.engine, &mut encoder, &screen);

        // The overlay (if any) draws last, over the finished frame.
        if let Some(overlay) = &mut self.overlay {
            overlay.frame(OverlayFrame {
                device: &self.engine.device,
                queue: &self.engine.queue,
                encoder: &mut encoder,
                view: &screen,
                size_px: [self.config.width, self.config.height],
                window: &self.window,
            });
        }

        self.engine.queue.submit(Some(encoder.finish()));
        self.engine.queue.present(frame);
    }
}

// ---------------------------------------------------------------------------
// Headless — the engine, presenting to a frame that can be read back. No
// window, no surface: what `vybe render` and the pixel-diff tests stand on.
// ---------------------------------------------------------------------------

pub(crate) struct Headless {
    engine: Engine,
    presenter: Presenter,
    frame: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    /// Row stride of the readback buffer (wgpu aligns copies to 256 bytes).
    stride: u32,
}

impl Headless {
    /// `keep_alpha`: frames keep their transparency (a PNG sequence another
    /// patch will layer) instead of landing on black.
    pub(crate) fn new(width: u32, height: u32, keep_alpha: bool) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let instance = wgpu::Instance::default();
        let (_, device, queue) = pollster::block_on(request_device(&instance, None));

        let frame = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless frame"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CAPTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let stride = (4 * width).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let engine = Engine::new(device, queue, width, height);
        let presenter = Presenter::new(&engine, CAPTURE_FORMAT, keep_alpha);
        Self {
            view: frame.create_view(&wgpu::TextureViewDescriptor::default()),
            frame,
            engine,
            presenter,
            readback,
            stride,
        }
    }

    pub(crate) fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub(crate) fn set_warp(&mut self, warp: Warp) {
        self.presenter.warp = warp;
    }

    /// Renders the frame at `time`; with `capture`, reads it back. Every frame
    /// must be rendered even when only some are kept — a feedback loop is the
    /// sum of its past.
    pub(crate) fn render(&mut self, time: f32, dt: f32, capture: bool) -> Option<Pixels> {
        let (width, height) = (self.engine.width, self.engine.height);
        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("headless frame"),
                });
        self.engine.render(&mut encoder, time, dt);
        self.presenter
            .present(&self.engine, &mut encoder, &self.view);
        if capture {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.frame,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(self.stride),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.engine.queue.submit(Some(encoder.finish()));
        if !capture {
            return None;
        }

        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("failed to map the readback buffer");
        });
        self.engine
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .expect("device lost while reading a frame back");
        let rgba = {
            let mapped = slice
                .get_mapped_range()
                .expect("readback buffer is not mapped");
            mapped
                .chunks_exact(self.stride as usize)
                .flat_map(|row| &row[..4 * width as usize])
                .copied()
                .collect()
        };
        self.readback.unmap();
        Some(Pixels {
            width,
            height,
            rgba,
        })
    }
}

// ---------------------------------------------------------------------------
// Builders — turn recipe data into GPU resources.
// ---------------------------------------------------------------------------

/// The point-cloud pass: a compute step over the particle buffer, then an
/// instanced draw reading it. Self-contained (owns its buffers and pipelines),
/// it renders into a signal texture like every other node — the rule that
/// everything composes, made literal.
struct PointsPass {
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    compute_frame_bg: wgpu::BindGroup, // frame block, visible to compute
    compute_particles_bg: wgpu::BindGroup, // storage, read_write (step)
    compute_forces_bg: wgpu::BindGroup, // the force stack (static uniform)
    forces_uniforms: wgpu::Buffer,     // kept so a tuned force can rewrite it
    render_particles_bg: wgpu::BindGroup, // storage, read-only (draw)
    count: u32,
}

impl PointsPass {
    /// Step the cloud, then draw it into `target` (cleared to `clear`). The
    /// draw reads `frame_bg` for resolution/aspect; the step reads its own.
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        frame_bg: &wgpu::BindGroup,
        clear: wgpu::Color,
    ) {
        // Pass 1: step every particle on the GPU (64 per workgroup).
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("particles step"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.compute_frame_bg, &[]);
            pass.set_bind_group(1, &self.compute_particles_bg, &[]);
            pass.set_bind_group(2, &self.compute_forces_bg, &[]);
            pass.dispatch_workgroups(self.count.div_ceil(64), 1, 1);
        }
        // Pass 2: draw the cloud, one instanced quad per particle.
        {
            let mut pass = begin_pass(encoder, target, "points pass", clear);
            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, frame_bg, &[]);
            pass.set_bind_group(1, &self.render_particles_bg, &[]);
            pass.draw(0..6, 0..self.count);
        }
    }
}

/// Build a [`PointsPass`] targeting `target_format`. Seeds the buffer, wires the compute step and the instanced
/// draw. The draw is additive so the crowd glows within its own texture.
#[allow(clippy::too_many_arguments)]
fn build_points(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame_uniforms: &wgpu::Buffer,
    frame_layout: &wgpu::BindGroupLayout,
    target_format: wgpu::TextureFormat,
    count: u32,
    forces: Vec<Force>,
) -> PointsPass {
    // The particle state buffer, seeded once on the CPU, then owned by the GPU
    // (the compute step reads and writes it every frame).
    let seed = seed_particles(count);
    let particles = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particles"),
        size: std::mem::size_of_val(seed.as_slice()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&particles, 0, bytemuck::cast_slice(&seed));

    // Frame block visible to the compute stage (its own layout/bg over the
    // shared frame buffer; the render pass reuses `frame_layout`).
    let compute_frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("compute frame layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let compute_frame_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("compute frame bind group"),
        layout: &compute_frame_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: frame_uniforms.as_entire_binding(),
        }],
    });

    let storage_layout = |read_only: bool, vis: wgpu::ShaderStages, label: &str| {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: vis,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    };
    let storage_bg = |layout: &wgpu::BindGroupLayout, label: &str| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: particles.as_entire_binding(),
            }],
        })
    };
    let compute_particles_layout =
        storage_layout(false, wgpu::ShaderStages::COMPUTE, "particles rw layout");
    let render_particles_layout =
        storage_layout(true, wgpu::ShaderStages::VERTEX, "particles ro layout");
    let compute_particles_bg = storage_bg(&compute_particles_layout, "particles rw");
    let render_particles_bg = storage_bg(&render_particles_layout, "particles ro");

    // The force stack: a static uniform, written once, read by the step.
    let forces_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("forces"),
        size: std::mem::size_of::<ForcesUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(
        &forces_uniforms,
        0,
        bytemuck::bytes_of(&encode_forces(&forces)),
    );
    let compute_forces_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("compute forces layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let compute_forces_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("compute forces bind group"),
        layout: &compute_forces_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: forces_uniforms.as_entire_binding(),
        }],
    });

    // Compute pipeline: the step.
    let step_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("particles.wgsl"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particles.wgsl").into()),
    });
    let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("particles step layout"),
        bind_group_layouts: &[
            Some(&compute_frame_layout),
            Some(&compute_particles_layout),
            Some(&compute_forces_layout),
        ],
        immediate_size: 0,
    });
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("particles step"),
        layout: Some(&compute_layout),
        module: &step_shader,
        entry_point: Some("step"),
        compilation_options: Default::default(),
        cache: None,
    });

    // Render pipeline: instanced points, additively blended so crowds glow.
    let points_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("points.wgsl"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/points.wgsl").into()),
    });
    let render_pipeline = make_pipeline(
        device,
        &points_shader,
        &[frame_layout, &render_particles_layout],
        target_format,
        Some(additive_blend()),
        "points pipeline",
    );

    PointsPass {
        compute_pipeline,
        render_pipeline,
        compute_frame_bg,
        compute_particles_bg,
        compute_forces_bg,
        forces_uniforms,
        render_particles_bg,
        count,
    }
}

/// One uniform buffer + bind group per stroke. Values arrive via
/// [`ShapePass::refresh`], every frame.
fn build_strokes(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    count: usize,
) -> Vec<StrokeDraw> {
    (0..count)
        .map(|_| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("stroke uniforms"),
                size: std::mem::size_of::<StrokeUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("stroke bind group"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            StrokeDraw { buffer, bind_group }
        })
        .collect()
}

/// The linear-clamp sampler shared by every pass that samples a signal.
fn make_linear_clamp_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("linear clamp"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

/// Additive blend: `src + dst`. Crowds and stacked layers glow instead of
/// occluding — black adds nothing, so what's beneath shows through.
fn additive_blend() -> wgpu::BlendState {
    let add = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    wgpu::BlendState {
        color: add,
        alpha: add,
    }
}

/// The compositor's blend state for a [`Blend`] mode. Signal textures are
/// premultiplied (geometry drawn with alpha onto transparent black), which is
/// what lets `over` read straight off them: `src + dst*(1 - src.a)`.
fn blend_state(blend: Blend) -> wgpu::BlendState {
    match blend {
        Blend::Add => additive_blend(),
        Blend::Over => {
            let over = wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            };
            wgpu::BlendState {
                color: over,
                alpha: over,
            }
        }
    }
}

/// A small uniform buffer holding `value`, rewritable later.
fn uniform_buffer<T: bytemuck::Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    value: &T,
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: std::mem::size_of::<T>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::bytes_of(value));
    buffer
}

/// The layout of everything that samples a signal: the texture, its sampler,
/// and one small uniform block (a layer's opacity, an image's quad, the warp).
fn sampled_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sampled bind group layout"),
        entries: &[texture_entry(0), sampler_entry(1), uniform_entry(2)],
    })
}

/// Sample `view` through `sampler`, with `uniforms` beside it.
fn sampled_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniforms: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sampled bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniforms.as_entire_binding(),
            },
        ],
    })
}

/// The size-dependent feedback resources: source + ping-pong textures, and
/// both sides' bind groups prebuilt (no per-frame allocation).
fn build_feedback_targets(
    device: &wgpu::Device,
    kit: &Kit,
    width: u32,
    height: u32,
    swirl_uniforms: &wgpu::Buffer,
) -> FeedbackTargets {
    let source = make_signal_texture(device, width, height, "source signal");
    let ping_pong = PingPong::new(device, width, height);

    let feedback_bg = |prev: &wgpu::TextureView| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("feedback bind group"),
            layout: &kit.feedback_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: swirl_uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(prev),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&kit.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&source),
                },
            ],
        })
    };
    let feedback_bgs = [
        feedback_bg(&ping_pong.views[0]),
        feedback_bg(&ping_pong.views[1]),
    ];

    FeedbackTargets {
        source,
        ping_pong,
        feedback_bgs,
    }
}

fn make_signal_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    label: &str,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SIGNAL_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

// ---------------------------------------------------------------------------
// wgpu boilerplate helpers — cut repetition, keep the core readable.
// ---------------------------------------------------------------------------

/// A render pass that clears `view` to `clear` and draws into it. Nodes clear
/// to *transparent* black so their empty regions carry no coverage — that's
/// what lets an `over` blend reveal the worlds beneath.
fn begin_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    label: &str,
    clear: wgpu::Color,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        // Shape strokes read their knobs in the vertex stage too.
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// Creates a bufferless render pipeline (vs `vs` + fs `fs`, no vertex buffer —
/// vertices come from the vertex/instance indices alone).
fn make_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
    target_format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    label: &str,
) -> wgpu::RenderPipeline {
    let groups: Vec<Option<&wgpu::BindGroupLayout>> =
        bind_group_layouts.iter().map(|l| Some(*l)).collect();
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &groups,
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
