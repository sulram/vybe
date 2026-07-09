//! THE GPU CORE — all of wgpu, hidden behind the knobs (Principle 4). Nothing
//! in this module ever shows up in a sketch.
//!
//! Uniforms are split by cadence: **group(0)** is the frame block, shared by
//! every pass and rewritten 60x/s (resolution, mouse, time); **group(1)** is
//! the static knobs of one stroke or one swirl, written once at build time.
//! That split is the anti-bottleneck stance in binary form: per-frame traffic
//! across the boundary is a handful of floats, no matter how much is drawn.

use std::sync::Arc;
use std::time::Instant;

use winit::window::Window;

use crate::recipe::{CompositeLayer, Force, Recipe, Stroke};
use crate::sugar::{Blend, Osc, Swirl};

/// Signal format: float16 per channel, like Braid/Satin. HDR headroom so the
/// feedback can accumulate without clipping too early.
const SIGNAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

// ---------------------------------------------------------------------------
// Uniform blocks — the data bridge recipe -> shader.
// ---------------------------------------------------------------------------

/// group(0): what changes every frame. Shared by every pass.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniforms {
    resolution: [f32; 2], // physical pixels
    mouse: [f32; 2],      // scene space
    time: f32,            // seconds since start (wall clock)
    dt: f32,              // seconds since last frame
    _pad: [f32; 2],
}

/// The engine's sense of time: wall-clock, so motion is framerate-independent
/// (`Wave` in cycles/second, `Hue.drift` in degrees/second mean what they say
/// on any monitor). `dt` lets per-frame effects (the feedback loop) run at a
/// rate, not a cadence.
///
/// This struct is the one place the engine reads a real clock — the seam to
/// swap for Phase 2/WASM, where `Instant::now()` panics on
/// `wasm32-unknown-unknown` (use `web_time` there).
struct Clock {
    start: Instant,
    last: Instant,
}

impl Clock {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
        }
    }

    /// (seconds since start, seconds since last frame). `dt` is clamped so a
    /// stall or the first frame can't jolt the animation with a huge step.
    fn tick(&mut self) -> (f32, f32) {
        let now = Instant::now();
        let time = now.duration_since(self.start).as_secs_f32();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        (time, dt)
    }
}

/// group(1) of the shape pipeline: one stroke's knobs, written once.
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
    _pad: [f32; 3],
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
            _pad: [0.0; 3],
        }
    }
}

/// group(1) of the feedback pipeline: the swirl knobs, written once.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SwirlUniforms {
    decay: f32,
    angle: f32,
    scale: f32,
    _pad: f32,
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
// GPU resources per recipe kind.
// ---------------------------------------------------------------------------

/// One stroke, ready to draw: its uniform buffer and bind group.
struct StrokeDraw {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// The shape pipeline, the strokes it draws (in painter's order), and their
/// GPU-side twins. `strokes` is the single source of truth — knobs are LIVE:
/// refreshed to the GPU every frame, so anything may retune them while the
/// sketch runs (the tweak panel today; MIDI/OSC/scripts tomorrow).
struct ShapePass {
    pipeline: wgpu::RenderPipeline,
    strokes: Vec<Stroke>,
    draws: Vec<StrokeDraw>,
}

impl ShapePass {
    /// Swap in a live sketch's re-described strokes. GPU twins are reused;
    /// only a change in stroke count allocates.
    fn replace(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        strokes: Vec<Stroke>,
    ) {
        if strokes.len() != self.draws.len() {
            self.draws = build_strokes(device, layout, strokes.len());
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

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, frame_bg: &wgpu::BindGroup) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, frame_bg, &[]);
        for (draw, stroke) in self.draws.iter().zip(&self.strokes) {
            pass.set_bind_group(1, &draw.bind_group, &[]);
            pass.draw(0..6, 0..stroke.cols * stroke.rows); // one quad per instance
        }
    }
}

/// The GPU resources of each recipe kind. One variant per [`Recipe`] variant.
// Exactly one `Passes` exists per window, for the whole run — the variant size
// gap is a few bytes of one-time memory, so boxing (heap + indirection every
// render match) would trade clarity for nothing.
#[allow(clippy::large_enum_variant)]
enum Passes {
    /// One render pass straight to the swapchain; one draw per stroke.
    Shapes(ShapePass),
    /// Three passes: strokes -> source signal; swirl(previous) + source ->
    /// next signal; present. The ping-pong stays hidden behind the knob.
    Feedback {
        shapes: ShapePass, // targets the source signal, not the screen
        feedback_pipeline: wgpu::RenderPipeline,
        feedback_layout: wgpu::BindGroupLayout,
        present_pipeline: wgpu::RenderPipeline,
        present_layout: wgpu::BindGroupLayout,
        swirl_uniforms: wgpu::Buffer,
        sampler: wgpu::Sampler,
        targets: FeedbackTargets,
    },
    /// The point-cloud signal type: a [`PointsPass`] (compute step + instanced
    /// draw) rendered straight to the screen.
    Points(PointsPass),
    /// A stack of worlds, each rendered to its own offscreen signal texture,
    /// then combined onto the screen — feedback on one layer, still geometry on
    /// another, no smearing across them. The compositor is `composite.wgsl`
    /// drawn once per layer, its fixed-function blend chosen by the stack's
    /// [`Blend`]: over (alpha, one world on another — the default) or additive
    /// (glow).
    Composite(Composite),
}

/// The composited stack: the layers plus the pieces shared across them (the
/// compositor pipeline, the two bind-group layouts a layer rebuilds on resize,
/// and the one sampler). Owns nothing per-frame.
struct Composite {
    layers: Vec<Layer>,
    /// One compositor pipeline per [`Blend`] — same shader, different blend
    /// state — so a stack can mix modes (add on one layer, over on the next)
    /// without rebuilding anything per frame.
    add_pipeline: wgpu::RenderPipeline,
    over_pipeline: wgpu::RenderPipeline,
    present_layout: wgpu::BindGroupLayout,
    feedback_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Composite {
    /// The compositor pipeline for a layer's blend mode.
    fn pipeline_for(&self, blend: Blend) -> &wgpu::RenderPipeline {
        match blend {
            Blend::Add => &self.add_pipeline,
            Blend::Over => &self.over_pipeline,
        }
    }
}

/// One world in a [`Composite`]: renders to its own signal texture, which the
/// compositor samples. A plain-geometry layer is one pass; a feedback layer is
/// the same ping-pong dance as [`Passes::Feedback`] minus the present (the
/// compositor plays that role, per the stack's blend, for the whole stack).
enum Layer {
    Shapes(ShapesLayer),
    // Boxed: a `Vec<Layer>` is mostly the smaller variant, and a feedback or
    // points layer carries pipelines (and buffers/ping-pong) — indirection
    // keeps every geometry layer in the stack from paying for it.
    Feedback(Box<FeedbackLayer>),
    Points(Box<PointsLayer>),
}

/// A geometry layer: strokes drawn into `target`; `composite_bg` samples it.
/// `blend` is how it lands on the worlds beneath it in the compositor.
struct ShapesLayer {
    shapes: ShapePass,
    target: wgpu::TextureView,
    composite_bg: wgpu::BindGroup,
    blend: Blend,
}

/// A point-cloud layer: the cloud drawn into `target`; `composite_bg` samples
/// it. `blend` is how it lands on the worlds beneath it in the compositor.
struct PointsLayer {
    pass: PointsPass,
    target: wgpu::TextureView,
    composite_bg: wgpu::BindGroup,
    blend: Blend,
}

/// A feedback layer: the source pass + the feedback pass, its ping-pong the
/// layer's output. `swirl_uniforms` is kept only so a resize can rebuild the
/// size-dependent targets.
struct FeedbackLayer {
    shapes: ShapePass,
    feedback_pipeline: wgpu::RenderPipeline,
    swirl_uniforms: wgpu::Buffer,
    targets: FeedbackTargets,
    blend: Blend,
}

impl Layer {
    /// Render this layer's world into its offscreen texture(s) for this frame.
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        frame_bg: &wgpu::BindGroup,
    ) {
        match self {
            Layer::Shapes(l) => {
                l.shapes.refresh(queue); // knobs are live
                let mut pass = begin_pass_clear(
                    encoder,
                    &l.target,
                    "layer shapes pass",
                    wgpu::Color::TRANSPARENT,
                );
                l.shapes.draw(&mut pass, frame_bg);
            }
            Layer::Points(l) => {
                // The cloud steps and draws into its own transparent texture, so
                // its empty space carries no coverage and `over` reveals what's
                // beneath (add ignores alpha either way).
                l.pass
                    .render(encoder, &l.target, frame_bg, wgpu::Color::TRANSPARENT);
            }
            Layer::Feedback(l) => {
                l.shapes.refresh(queue); // knobs are live
                // Pass 1: the layer's geometry -> its source signal.
                {
                    let mut pass = begin_pass_clear(
                        encoder,
                        &l.targets.source,
                        "layer source pass",
                        wgpu::Color::TRANSPARENT,
                    );
                    l.shapes.draw(&mut pass, frame_bg);
                }
                // Pass 2: swirl(previous) + source -> the next signal.
                {
                    let mut pass =
                        begin_pass(encoder, l.targets.ping_pong.write(), "layer feedback pass");
                    pass.set_pipeline(&l.feedback_pipeline);
                    pass.set_bind_group(0, frame_bg, &[]);
                    pass.set_bind_group(1, &l.targets.feedback_bgs[l.targets.ping_pong.front], &[]);
                    pass.draw(0..3, 0..1);
                }
            }
        }
    }

    /// The bind group the compositor samples this frame (a feedback layer's
    /// output is the ping-pong side just written).
    fn composite_bg(&self) -> &wgpu::BindGroup {
        match self {
            Layer::Shapes(l) => &l.composite_bg,
            Layer::Points(l) => &l.composite_bg,
            Layer::Feedback(l) => &l.targets.present_bgs[1 - l.targets.ping_pong.front],
        }
    }

    /// How this layer blends onto the worlds beneath it in the compositor.
    fn blend(&self) -> Blend {
        match self {
            Layer::Shapes(l) => l.blend,
            Layer::Points(l) => l.blend,
            Layer::Feedback(l) => l.blend,
        }
    }

    /// After compositing, advance a feedback layer's ping-pong (geometry layers
    /// hold nothing that alternates).
    fn swap(&mut self) {
        if let Layer::Feedback(l) = self {
            l.targets.ping_pong.swap();
        }
    }

    /// Rebuild the size-dependent resources (the pipelines and knob buffers
    /// survive) — the composite counterpart of [`build_feedback_targets`].
    fn resize(
        &mut self,
        device: &wgpu::Device,
        present_layout: &wgpu::BindGroupLayout,
        feedback_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) {
        match self {
            Layer::Shapes(l) => {
                l.target = make_signal_texture(device, width, height, "layer target");
                l.composite_bg = present_bind_group(device, present_layout, &l.target, sampler);
            }
            Layer::Points(l) => {
                // Only the target is size-dependent; the cloud (buffer, pipelines)
                // survives, so the simulation keeps running across a resize.
                l.target = make_signal_texture(device, width, height, "layer points target");
                l.composite_bg = present_bind_group(device, present_layout, &l.target, sampler);
            }
            Layer::Feedback(l) => {
                l.targets = build_feedback_targets(
                    device,
                    width,
                    height,
                    sampler,
                    &l.swirl_uniforms,
                    feedback_layout,
                    present_layout,
                );
            }
        }
    }
}

/// Everything that depends on the window size — rebuilt on resize (the trail
/// is lost then; fine for now).
struct FeedbackTargets {
    source: wgpu::TextureView,
    ping_pong: PingPong,
    /// `feedback_bgs[i]` reads `views[i]` as the previous frame.
    feedback_bgs: [wgpu::BindGroup; 2],
    /// `present_bgs[i]` presents `views[i]`.
    present_bgs: [wgpu::BindGroup; 2],
}

/// A ping-pong pair of signal-textures. Each frame we read from one and write
/// to the other, then swap. Resolves the GPU aliasing and is the correct
/// pattern anyway — hidden behind the `decay` knob.
struct PingPong {
    // Views keep their textures alive in wgpu.
    views: [wgpu::TextureView; 2],
    front: usize, // index we READ from this frame
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
// State — the window's GPU context plus the per-recipe passes.
// ---------------------------------------------------------------------------

pub(crate) struct State {
    pub(crate) window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    frame_uniforms: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
    passes: Passes,
    overlay: Option<Box<dyn Overlay>>,

    // Kept so a live sketch can rebuild its passes when the recipe changes.
    shape_shader: wgpu::ShaderModule,
    frame_layout: wgpu::BindGroupLayout,
    stroke_layout: wgpu::BindGroupLayout,

    /// Mouse in scene space. Starts far away so nothing reacts before the
    /// cursor first enters the window.
    mouse: [f32; 2],
    clock: Clock,
}

impl State {
    pub(crate) async fn new(window: Arc<Window>, recipe: Recipe) -> Self {
        let size = window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
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

        // The frame block: one small buffer shared by every pass (group 0).
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame bind group layout"),
            entries: &[uniform_entry(0)],
        });
        let frame_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame bind group"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniforms.as_entire_binding(),
            }],
        });

        // Shape pipeline pieces — both recipe kinds draw geometry.
        let shape_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shape.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shape.wgsl").into()),
        });
        let stroke_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stroke bind group layout"),
            entries: &[uniform_entry(0)],
        });

        let passes = build_passes(
            &device,
            &queue,
            &shape_shader,
            &frame_layout,
            &frame_uniforms,
            &stroke_layout,
            format,
            width,
            height,
            recipe,
        );

        Self {
            window,
            surface,
            device,
            queue,
            config,
            frame_uniforms,
            frame_bg,
            passes,
            overlay: None,
            shape_shader,
            frame_layout,
            stroke_layout,
            mouse: [1e9, 1e9], // far away: at rest until the cursor shows up
            clock: Clock::new(),
        }
    }

    /// Attaches a front-end drawn over the running sketch.
    #[cfg_attr(not(feature = "tweak"), allow(dead_code))]
    pub(crate) fn set_overlay(&mut self, overlay: Box<dyn Overlay>) {
        self.overlay = Some(overlay);
    }

    #[cfg_attr(not(feature = "tweak"), allow(dead_code))]
    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.device
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

    /// Replaces the running recipe — the live-sketch seam. Same shape of
    /// recipe: only the knob values move (cheap, every slider tick). A
    /// structural change (a live sketch may branch into another kind)
    /// rebuilds the passes.
    pub(crate) fn set_recipe(&mut self, recipe: Recipe) {
        let recipe = match (&mut self.passes, recipe) {
            (Passes::Shapes(shapes), Recipe::Shapes(strokes)) => {
                shapes.replace(&self.device, &self.stroke_layout, strokes);
                return;
            }
            (
                Passes::Feedback {
                    shapes,
                    swirl_uniforms,
                    ..
                },
                Recipe::Feedback { source, swirl },
            ) => {
                shapes.replace(&self.device, &self.stroke_layout, source);
                self.queue.write_buffer(
                    swirl_uniforms,
                    0,
                    bytemuck::bytes_of(&SwirlUniforms {
                        decay: swirl.decay,
                        angle: swirl.angle,
                        scale: swirl.scale,
                        _pad: 0.0,
                    }),
                );
                return;
            }
            // Same-size cloud: only the forces changed (a tuned knob) — rewrite
            // the tiny force uniform and keep the particles' live positions.
            (Passes::Points(points), Recipe::Points { count, forces }) if points.count == count => {
                self.queue.write_buffer(
                    &points.forces_uniforms,
                    0,
                    bytemuck::bytes_of(&encode_forces(&forces)),
                );
                return;
            }
            (_, recipe) => recipe,
        };
        self.passes = build_passes(
            &self.device,
            &self.queue,
            &self.shape_shader,
            &self.frame_layout,
            &self.frame_uniforms,
            &self.stroke_layout,
            self.config.format,
            self.config.width,
            self.config.height,
            recipe,
        );
    }

    pub(crate) fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        // Only the offscreen signal textures are size-dependent; shape passes
        // own nothing sized. Rebuild them (the trail is lost then; fine for now).
        match &mut self.passes {
            Passes::Feedback {
                feedback_layout,
                present_layout,
                swirl_uniforms,
                sampler,
                targets,
                ..
            } => {
                *targets = build_feedback_targets(
                    &self.device,
                    size.width,
                    size.height,
                    sampler,
                    swirl_uniforms,
                    feedback_layout,
                    present_layout,
                );
            }
            Passes::Composite(Composite {
                layers,
                present_layout,
                feedback_layout,
                sampler,
                ..
            }) => {
                for layer in layers.iter_mut() {
                    layer.resize(
                        &self.device,
                        present_layout,
                        feedback_layout,
                        sampler,
                        size.width,
                        size.height,
                    );
                }
            }
            Passes::Shapes(_) | Passes::Points(_) => {}
        }
    }

    /// Pixels (y-down, origin top-left) -> scene space (y-up, centered, the
    /// shorter edge spanning -0.5..+0.5). The sketch never sees a pixel.
    pub(crate) fn set_mouse(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        let (w, h) = (self.config.width as f32, self.config.height as f32);
        let unit = w.min(h);
        self.mouse = [
            (position.x as f32 - w * 0.5) / unit,
            (h * 0.5 - position.y as f32) / unit,
        ];
    }

    /// The mouse signal at rest: far away, so proximity effects go quiet when
    /// the cursor leaves the window (matches the start-up state).
    pub(crate) fn rest_mouse(&mut self) {
        self.mouse = [1e9, 1e9];
    }

    pub(crate) fn render(&mut self) {
        let (time, dt) = self.clock.tick();
        let u = FrameUniforms {
            resolution: [self.config.width as f32, self.config.height as f32],
            mouse: self.mouse,
            time,
            dt,
            _pad: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.frame_uniforms, 0, bytemuck::bytes_of(&u));

        use wgpu::CurrentSurfaceTexture::*;
        let frame = match self.surface.get_current_texture() {
            Success(f) | Suboptimal(f) => f,
            // Swapchain out of date (e.g. during resize) or unavailable — skip the frame.
            Outdated | Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Timeout | Occluded | Validation => return,
        };
        let screen = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });

        match &mut self.passes {
            Passes::Shapes(shapes) => {
                shapes.refresh(&self.queue); // knobs are live
                let mut pass = begin_pass(&mut encoder, &screen, "shapes pass");
                shapes.draw(&mut pass, &self.frame_bg);
            }
            Passes::Feedback {
                shapes,
                feedback_pipeline,
                present_pipeline,
                targets,
                ..
            } => {
                shapes.refresh(&self.queue); // knobs are live
                // Pass 1: the chain's geometry -> the source signal.
                {
                    let mut pass = begin_pass(&mut encoder, &targets.source, "source pass");
                    shapes.draw(&mut pass, &self.frame_bg);
                }
                // Pass 2: swirl(previous) + source -> the next signal.
                {
                    let mut pass =
                        begin_pass(&mut encoder, targets.ping_pong.write(), "feedback pass");
                    pass.set_pipeline(feedback_pipeline);
                    pass.set_bind_group(0, &self.frame_bg, &[]);
                    pass.set_bind_group(1, &targets.feedback_bgs[targets.ping_pong.front], &[]);
                    pass.draw(0..3, 0..1); // fullscreen triangle
                }
                // Pass 3: present the freshly written signal to the window.
                {
                    let mut pass = begin_pass(&mut encoder, &screen, "present pass");
                    pass.set_pipeline(present_pipeline);
                    pass.set_bind_group(0, &targets.present_bgs[1 - targets.ping_pong.front], &[]);
                    pass.draw(0..3, 0..1);
                }
                // Swap: what we wrote becomes next frame's "previous".
                targets.ping_pong.swap();
            }
            Passes::Points(points) => {
                points.render(&mut encoder, &screen, &self.frame_bg, wgpu::Color::BLACK);
            }
            Passes::Composite(comp) => {
                // Each layer renders its own world into its offscreen texture(s).
                for layer in &comp.layers {
                    layer.render(&mut encoder, &self.queue, &self.frame_bg);
                }
                // One pass draws the layers onto the screen, bottom to top —
                // the compositor is composite.wgsl, each layer drawn with the
                // pipeline for its own blend (over by alpha, or additive glow).
                {
                    let mut pass = begin_pass(&mut encoder, &screen, "composite pass");
                    for layer in &comp.layers {
                        pass.set_pipeline(comp.pipeline_for(layer.blend()));
                        pass.set_bind_group(0, layer.composite_bg(), &[]);
                        pass.draw(0..3, 0..1); // fullscreen triangle
                    }
                }
                // Advance each feedback layer's ping-pong for next frame.
                for layer in &mut comp.layers {
                    layer.swap();
                }
            }
        }

        // The overlay (if any) draws last, over the finished frame.
        if let Some(overlay) = &mut self.overlay {
            overlay.frame(OverlayFrame {
                device: &self.device,
                queue: &self.queue,
                encoder: &mut encoder,
                view: &screen,
                size_px: [self.config.width, self.config.height],
                window: &self.window,
            });
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
    }
}

// ---------------------------------------------------------------------------
// Builders — turn recipe data into GPU resources.
// ---------------------------------------------------------------------------

/// The point-cloud pass: a compute step over the particle buffer, then an
/// instanced draw reading it. Self-contained (owns its buffers and pipelines)
/// so it renders the same whether it targets the screen ([`Passes::Points`]) or
/// a layer's offscreen texture ([`Layer::Points`]) — the rule that everything
/// composes, made literal.
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
            let mut pass = begin_pass_clear(encoder, target, "points pass", clear);
            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, frame_bg, &[]);
            pass.set_bind_group(1, &self.render_particles_bg, &[]);
            pass.draw(0..6, 0..self.count);
        }
    }
}

/// Build a [`PointsPass`] targeting `target_format` (the screen's, or a layer's
/// `SIGNAL_FORMAT`). Seeds the buffer, wires the compute step and the instanced
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

/// Builds the GPU passes for a recipe. Called at start-up and again by
/// [`State::set_recipe`] when a live sketch changes the recipe's kind.
#[allow(clippy::too_many_arguments)]
fn build_passes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shape_shader: &wgpu::ShaderModule,
    frame_layout: &wgpu::BindGroupLayout,
    frame_uniforms: &wgpu::Buffer,
    stroke_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    recipe: Recipe,
) -> Passes {
    match recipe {
        Recipe::Shapes(strokes) => {
            let shapes = build_shape_pass(
                device,
                shape_shader,
                frame_layout,
                stroke_layout,
                surface_format,
                strokes,
                "shape pipeline",
            );
            Passes::Shapes(shapes)
        }
        Recipe::Feedback { source, swirl } => {
            // Pass 1: the same shape pipeline, but into the source signal.
            let shapes = build_shape_pass(
                device,
                shape_shader,
                frame_layout,
                stroke_layout,
                SIGNAL_FORMAT,
                source,
                "shape pipeline (source)",
            );
            let sampler = make_linear_clamp_sampler(device);
            // The swirl knobs: written now, refreshed by the live seam.
            let swirl_uniforms = make_swirl_uniforms(device, queue, swirl);

            // Pass 2: feedback (reads previous + source, writes next).
            let feedback_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("feedback.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/feedback.wgsl").into()),
            });
            let feedback_layout = feedback_bind_group_layout(device);
            let feedback_pipeline = make_pipeline(
                device,
                &feedback_shader,
                &[frame_layout, &feedback_layout],
                SIGNAL_FORMAT,
                None,
                "feedback pipeline",
            );

            // Pass 3: present (signal -> swapchain).
            let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("present.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/present.wgsl").into()),
            });
            let present_layout = present_bind_group_layout(device);
            let present_pipeline = make_pipeline(
                device,
                &present_shader,
                &[&present_layout],
                surface_format,
                None,
                "present pipeline",
            );

            let targets = build_feedback_targets(
                device,
                width,
                height,
                &sampler,
                &swirl_uniforms,
                &feedback_layout,
                &present_layout,
            );

            Passes::Feedback {
                shapes,
                feedback_pipeline,
                feedback_layout,
                present_pipeline,
                present_layout,
                swirl_uniforms,
                sampler,
                targets,
            }
        }
        Recipe::Points { count, forces } => Passes::Points(build_points(
            device,
            queue,
            frame_uniforms,
            frame_layout,
            surface_format,
            count,
            forces,
        )),
        Recipe::Composite(sub_layers) => {
            // Pieces shared by every layer: one sampler, the two layouts a layer
            // rebuilds on resize, and one compositor pipeline per blend mode
            // (same shader, different blend state) so layers can mix modes.
            let sampler = make_linear_clamp_sampler(device);
            let present_layout = present_bind_group_layout(device);
            let feedback_layout = feedback_bind_group_layout(device);

            let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("composite.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/composite.wgsl").into()),
            });
            let composite_pipeline = |blend, label| {
                make_pipeline(
                    device,
                    &composite_shader,
                    &[&present_layout],
                    surface_format,
                    Some(blend_state(blend)),
                    label,
                )
            };
            let add_pipeline = composite_pipeline(Blend::Add, "composite pipeline (add)");
            let over_pipeline = composite_pipeline(Blend::Over, "composite pipeline (over)");

            // The feedback shader is shared by every feedback layer.
            let feedback_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("feedback.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/feedback.wgsl").into()),
            });

            let layers = sub_layers
                .into_iter()
                .map(|sub| {
                    build_layer(
                        device,
                        queue,
                        shape_shader,
                        &feedback_shader,
                        frame_layout,
                        frame_uniforms,
                        stroke_layout,
                        &feedback_layout,
                        &present_layout,
                        &sampler,
                        width,
                        height,
                        sub,
                    )
                })
                .collect();

            Passes::Composite(Composite {
                layers,
                add_pipeline,
                over_pipeline,
                present_layout,
                feedback_layout,
                sampler,
            })
        }
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

/// A shape pass targeting `target_format`: the instanced-circle pipeline plus
/// one uniform buffer/bind-group per stroke. The screen path, the feedback
/// source, and every composite layer all build their geometry through this.
fn build_shape_pass(
    device: &wgpu::Device,
    shape_shader: &wgpu::ShaderModule,
    frame_layout: &wgpu::BindGroupLayout,
    stroke_layout: &wgpu::BindGroupLayout,
    target_format: wgpu::TextureFormat,
    strokes: Vec<Stroke>,
    label: &str,
) -> ShapePass {
    let pipeline = make_pipeline(
        device,
        shape_shader,
        &[frame_layout, stroke_layout],
        target_format,
        // The SDF circle's anti-aliased rim needs alpha blending.
        Some(wgpu::BlendState::ALPHA_BLENDING),
        label,
    );
    let draws = build_strokes(device, stroke_layout, strokes.len());
    ShapePass {
        pipeline,
        strokes,
        draws,
    }
}

/// The linear-clamp sampler shared by the feedback and present/composite passes.
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

/// The compositor's blend state for a stack's [`Blend`] mode. Layer textures
/// are premultiplied (geometry drawn with alpha onto transparent black), which
/// is what lets `over` read straight off them: `src + dst*(1 - src.a)`.
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

/// The swirl knobs buffer, written once (the live seam refreshes it later).
fn make_swirl_uniforms(device: &wgpu::Device, queue: &wgpu::Queue, swirl: Swirl) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("swirl uniforms"),
        size: std::mem::size_of::<SwirlUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(
        &buffer,
        0,
        bytemuck::bytes_of(&SwirlUniforms {
            decay: swirl.decay,
            angle: swirl.angle,
            scale: swirl.scale,
            _pad: 0.0,
        }),
    );
    buffer
}

/// group(1) layout of the feedback pass: the swirl knobs, the previous signal,
/// a sampler, and the source signal.
fn feedback_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("feedback bind group layout"),
        entries: &[
            uniform_entry(0),
            texture_entry(1),
            sampler_entry(2),
            texture_entry(3),
        ],
    })
}

/// The present/composite layout: a signal texture + its sampler.
fn present_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("present bind group layout"),
        entries: &[texture_entry(0), sampler_entry(1)],
    })
}

/// A present/composite bind group: sample `view` through `sampler`.
fn present_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("present bind group"),
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
        ],
    })
}

/// Build one composite [`Layer`] from its sub-recipe: geometry (`Shapes`),
/// feedback, or a point cloud (`Points`) — the three signal types, each
/// composable. Anything else renders as an empty (black) geometry layer.
#[allow(clippy::too_many_arguments)]
fn build_layer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shape_shader: &wgpu::ShaderModule,
    feedback_shader: &wgpu::ShaderModule,
    frame_layout: &wgpu::BindGroupLayout,
    frame_uniforms: &wgpu::Buffer,
    stroke_layout: &wgpu::BindGroupLayout,
    feedback_layout: &wgpu::BindGroupLayout,
    present_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    layer: CompositeLayer,
) -> Layer {
    let CompositeLayer { recipe, blend } = layer;
    match recipe {
        Recipe::Points { count, forces } => {
            let pass = build_points(
                device,
                queue,
                frame_uniforms,
                frame_layout,
                SIGNAL_FORMAT,
                count,
                forces,
            );
            let target = make_signal_texture(device, width, height, "layer points target");
            let composite_bg = present_bind_group(device, present_layout, &target, sampler);
            Layer::Points(Box::new(PointsLayer {
                pass,
                target,
                composite_bg,
                blend,
            }))
        }
        Recipe::Feedback { source, swirl } => {
            let shapes = build_shape_pass(
                device,
                shape_shader,
                frame_layout,
                stroke_layout,
                SIGNAL_FORMAT,
                source,
                "layer shape pipeline (source)",
            );
            let swirl_uniforms = make_swirl_uniforms(device, queue, swirl);
            let feedback_pipeline = make_pipeline(
                device,
                feedback_shader,
                &[frame_layout, feedback_layout],
                SIGNAL_FORMAT,
                None,
                "layer feedback pipeline",
            );
            let targets = build_feedback_targets(
                device,
                width,
                height,
                sampler,
                &swirl_uniforms,
                feedback_layout,
                present_layout,
            );
            Layer::Feedback(Box::new(FeedbackLayer {
                shapes,
                feedback_pipeline,
                swirl_uniforms,
                targets,
                blend,
            }))
        }
        other => {
            let strokes = match other {
                Recipe::Shapes(strokes) => strokes,
                _ => Vec::new(),
            };
            let shapes = build_shape_pass(
                device,
                shape_shader,
                frame_layout,
                stroke_layout,
                SIGNAL_FORMAT,
                strokes,
                "layer shape pipeline",
            );
            let target = make_signal_texture(device, width, height, "layer target");
            let composite_bg = present_bind_group(device, present_layout, &target, sampler);
            Layer::Shapes(ShapesLayer {
                shapes,
                target,
                composite_bg,
                blend,
            })
        }
    }
}

/// The size-dependent feedback resources: source + ping-pong textures, and
/// both sides' bind groups prebuilt (no per-frame allocation).
fn build_feedback_targets(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sampler: &wgpu::Sampler,
    swirl_uniforms: &wgpu::Buffer,
    feedback_layout: &wgpu::BindGroupLayout,
    present_layout: &wgpu::BindGroupLayout,
) -> FeedbackTargets {
    let source = make_signal_texture(device, width, height, "source signal");
    let ping_pong = PingPong::new(device, width, height);

    let feedback_bg = |prev: &wgpu::TextureView| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("feedback bind group"),
            layout: feedback_layout,
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
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&source),
                },
            ],
        })
    };
    let present_bg =
        |view: &wgpu::TextureView| present_bind_group(device, present_layout, view, sampler);

    let feedback_bgs = [
        feedback_bg(&ping_pong.views[0]),
        feedback_bg(&ping_pong.views[1]),
    ];
    let present_bgs = [
        present_bg(&ping_pong.views[0]),
        present_bg(&ping_pong.views[1]),
    ];

    FeedbackTargets {
        source,
        ping_pong,
        feedback_bgs,
        present_bgs,
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

/// A render pass that clears to opaque black and draws into `view`.
fn begin_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    label: &str,
) -> wgpu::RenderPass<'a> {
    begin_pass_clear(encoder, view, label, wgpu::Color::BLACK)
}

/// A render pass that clears `view` to `clear` and draws into it. Composite
/// layers clear to *transparent* black so their empty regions carry no
/// coverage — that's what lets an `over` blend reveal the worlds beneath.
fn begin_pass_clear<'a>(
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
