//! THE SUGAR — the Rust dialect of the chain, the base API from which the
//! TS/JS and Lua dialects will later derive (the Multi-Sugar pillar).
//!
//! Everything here only *describes*: a chain assembles a [`Recipe`] and hands
//! it to the core at a terminal link (`.show()`). Describing is total — it
//! never fails, never panics (Principles 2 and 3). And because chains are
//! plain data, every writing style produces the same recipe: fluent chaining,
//! line-by-line rebinding, loops that build layers — different spellings of
//! one AST.

use crate::recipe::{Force, Source, Stroke};
use crate::shell;

// ---------------------------------------------------------------------------
// The texture world (the Braid/Hydra lineage)
// ---------------------------------------------------------------------------

/// Births an empty [`Signal`]. Empty *is* the identity (Principle 2): with no
/// links and no source it shows black — it never explodes.
pub fn signal() -> Signal {
    Signal::default()
}

/// A texture-signal: what flows through the fullscreen/feedback world. Born
/// empty ([`signal()`]) or from geometry ([`Shape::render`] — the bridge).
#[derive(Clone, Default)]
pub struct Signal {
    pub(crate) source: Vec<Stroke>,
    pub(crate) swirl: Option<Swirl>,
}

impl Signal {
    /// Hangs a feedback loop on the signal, parametrized by a [`Swirl`]. The
    /// signal's source — whatever geometry was rendered into it — becomes the
    /// energy injected into the loop, every frame.
    pub fn feedback(mut self, swirl: Swirl) -> Self {
        self.swirl = Some(swirl);
        self
    }

    /// The terminal link: opens the window, brings up the GPU, and runs the
    /// render loop. (In the future this becomes `.out()`.)
    pub fn show(self) {
        shell::run(self.flatten());
    }
}

/// The bridge, implicit: a bare [`Shape`] stacked in [`layers()`] lifts into
/// the texture world exactly as `.render()` does — so a layer can be written
/// as plain geometry, and `layers()` accepts `Shape` and `Signal` side by side.
impl From<Shape> for Signal {
    fn from(shape: Shape) -> Self {
        shape.render()
    }
}

/// The feedback knobs — all **per second**, so the loop looks the same on any
/// monitor (the engine applies them by `dt` each frame). Everything has a
/// `Default` → the artist only touches what they want, and "unspecified" is
/// never an absent state, it's the default (Principle 2).
#[derive(Clone, Copy)]
pub struct Swirl {
    /// Fraction of the trail that survives per second (the memory). `[0,1]`.
    pub decay: f32,
    /// Rotation per second, in radians.
    pub angle: f32,
    /// Zoom per second. `<1` spirals outward, `>1` inward.
    pub scale: f32,
}

impl Default for Swirl {
    fn default() -> Self {
        Self {
            decay: 0.16,
            angle: 0.6,
            scale: 0.83,
        }
    }
}

// ---------------------------------------------------------------------------
// The geometry world
//
// Scene space (the artist's coordinate system, TouchDesigner-style): center
// (0,0), y-up, the SHORTER screen edge spans -0.5..+0.5. Units stay square on
// any window aspect (the longer edge just sees more world), so every distance
// knob reads as a fraction of the screen: 0.3 = 30%. Pixels exist only at the
// boundary and never reach a sketch.
// ---------------------------------------------------------------------------

/// Births a [`Shape`]: a circle whose radius is a fraction of its grid cell
/// (`0.5` = neighbouring circles touch). With no grid the whole scene is the
/// one cell, so the radius reads as a fraction of the screen.
pub fn circle(radius: f32) -> Shape {
    Shape {
        stroke: Stroke {
            radius,
            ..Stroke::IDENTITY
        },
    }
}

/// A geometry-signal: one shape, its placement, motion, and paint. Holds the
/// recipe until a terminal link runs it. Position composes additively —
/// `grid cell + at(..) + wave(..)` — each link fills in only what it touches.
#[derive(Clone)]
pub struct Shape {
    pub(crate) stroke: Stroke,
}

impl Shape {
    /// Scatters the shape on a centered, square-celled grid of `cols × rows` —
    /// one GPU instance per cell, a single draw call. No per-element data ever
    /// crosses to the GPU: cells are derived from the instance index alone.
    pub fn grid(mut self, cols: u32, rows: u32) -> Self {
        self.stroke.cols = cols.max(1);
        self.stroke.rows = rows.max(1);
        self
    }

    /// Places the shape at a position source: `at(mouse())` follows the
    /// pointer, `at((x, y))` pins it in scene space. A constant is just
    /// another signal — same socket, different plug.
    pub fn at(mut self, pos: impl Into<Pos>) -> Self {
        self.stroke.place = pos.into().into();
        self
    }

    /// Adds a lissajous motion around the shape's position, parametrized by a
    /// [`Wave`]. Declarative: the knobs cross to the GPU once; the shader
    /// runs the motion on its own, every frame.
    pub fn wave(mut self, wave: Wave) -> Self {
        self.stroke.wave = wave;
        self
    }

    /// Paints the shape, parametrized by a [`Hue`]. `hue(120.0)` is a plain
    /// angle on the color wheel; `Hue { drift, .. }` makes it slide over time.
    /// Unpainted shapes are white.
    pub fn hue(mut self, hue: impl Into<Hue>) -> Self {
        self.stroke.hue = hue.into();
        self.stroke.sat = 1.0;
        self
    }

    /// Softens the edge: `0.0` = crisp disc (anti-aliased), `1.0` = a ball of
    /// light fading from the center.
    pub fn soft(mut self, soft: f32) -> Self {
        self.stroke.soft = soft;
        self
    }

    /// Grows the shape by proximity to a position source, parametrized by a
    /// [`Falloff`]. The source is the swell's epicenter — `grow(mouse(), ..)`
    /// tracks the pointer, `grow((x, y), ..)` pins it, and a tuned pair
    /// `grow((tune("x", ..), tune("y", ..)), ..)` drives it by hand. Same plug
    /// as [`Shape::at`] (`impl Into<Pos>`). Declarative on purpose (the
    /// anti-bottleneck stance): the knobs cross to the GPU once per frame; no
    /// per-element closure ever runs on the CPU.
    pub fn grow(mut self, source: impl Into<Pos>, falloff: Falloff) -> Self {
        self.stroke.grow_at = source.into().into();
        self.stroke.falloff = falloff;
        self
    }

    /// The bridge-link: the geometry becomes a texture [`Signal`] and flows
    /// into the Braid world. The chain never breaks — it changes signal type
    /// at exactly this point.
    pub fn render(self) -> Signal {
        Signal {
            source: vec![self.stroke],
            swirl: None,
        }
    }

    /// The terminal link: opens the window, brings up the GPU, and runs the
    /// render loop.
    pub fn show(self) {
        shell::run(self.flatten());
    }
}

// ---------------------------------------------------------------------------
// Layers — chains composed into one scene
// ---------------------------------------------------------------------------

/// Composes chains into one scene, listed **top first** — item 0 is the
/// frontmost, like a layers panel (the core draws back-to-front for you). Each
/// item is a *world* — a bare [`Shape`] (lifted through the bridge) or a full
/// [`Signal`], feedback and all. Takes anything iterable: an array, or a `map`
/// over a range. For an explicit list of mixed *types*, use [`layers!`].
///
/// The engine picks the cheap path or the rich one by what's in the stack:
/// - **All plain geometry** (no feedback, no particles) on the default `over`
///   collapses into one geometry pass, drawn in painter's order — the same
///   efficient path `layers()` always was (painter's-order alpha *is* over).
/// - **Mixed worlds** (any layer carries feedback or particles, or asks to
///   *add* its light) composite: each layer renders to its own texture, then
///   the frames are combined onto the screen per each layer's [`Blend`] — so
///   feedback lives on one layer without smearing the others.
///
/// ```ignore
/// // Cheap path: ten shapes, one geometry pass.
/// layers((0..10).map(|i| circle(0.06).hue(i as f32 * 36.0))).show();
///
/// // Rich path: a feedback orbit over a still field of dots (orbit on top).
/// layers([
///     circle(0.05).soft(1.0).wave(Wave::default())
///         .render().feedback(Swirl::default()),          // foreground, with trails
///     circle(0.3).grid(24, 24).render(),                 // background, no feedback
/// ])
/// .show();
/// ```
pub fn layers<L: Into<Layer>>(items: impl IntoIterator<Item = L>) -> Layers {
    Layers {
        layers: items.into_iter().map(Into::into).collect(),
        blend: Blend::default(),
    }
}

/// Build a [`Layers`] stack from an explicit list of worlds — like [`layers()`],
/// but each item is converted to a [`Layer`] on its own, so you can freely mix
/// world *types* (a `Shape` beside a `Particles`) and mix bare worlds (which take
/// the stack's default blend) with `.blend(..)`ed ones. This is what a plain
/// array can't do — its items must all be one type — so reach for `layers![..]`
/// whenever a stack is heterogeneous, and `layers(iter)` when you have an
/// iterator (a `map` over a range).
///
/// ```ignore
/// layers![
///     spark,                    // a Shape, default blend (over)
///     swarm.blend(Blend::Add),  // a Particles with a per-layer override — different type, fine
///     field,                    // a Shape again, default blend
/// ]
/// .show();
/// ```
#[macro_export]
macro_rules! layers {
    ($($world:expr),+ $(,)?) => {
        $crate::layers([$( $crate::Layer::from($world) ),+])
    };
}

/// How the compositor combines a layer with the worlds beneath it. Set per
/// layer with [`AsLayer::blend`], or for the whole stack with [`Layers::blend`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blend {
    /// Alpha compositing — TouchDesigner's *over*, a design tool's *Normal*.
    /// Each layer sits on top by its own coverage, the world beneath showing
    /// through where it's transparent: opaque geometry occludes, soft edges
    /// feather. The default: a stack reads like a layers panel — top item in
    /// front — unless a layer asks to add its light instead.
    #[default]
    Over,
    /// `src + dst` — layers sum, so stacked light glows and black adds nothing
    /// (the world beneath shows through the dark). What crowds, feedback
    /// trails, and anything made of light reach for — one `.blend(Blend::Add)`
    /// away.
    Add,
}

/// The worlds a layer can be — one per signal type that can be composited. The
/// rule (DECISIONS 2026-07-08): everything composes, so every currency lands
/// here. Texture (geometry/feedback) and the point cloud today.
pub(crate) enum World {
    Signal(Signal),
    Particles(Particles),
}

/// One world in a [`layers()`] stack, paired with how it composites onto the
/// worlds beneath it. A bare [`Shape`], [`Signal`], or [`Particles`] becomes a
/// layer that inherits the stack's blend; [`AsLayer::blend`] pins a mode for
/// this layer alone. (Blend is a *relation* — how a world lands on the stack —
/// so it lives here at the composition boundary, not inside the world's chain.)
pub struct Layer {
    pub(crate) world: World,
    /// `None` = inherit the stack's blend (see [`Layers::blend`]).
    pub(crate) blend: Option<Blend>,
}

/// Turns a world into a [`Layer`] with a chosen [`Blend`]. Implemented for every
/// composable world — [`Shape`], [`Signal`], [`Particles`] — so
/// `world.blend(Blend::Add)` reads as one more link: the last one, the one that
/// says how the world stacks.
pub trait AsLayer: Into<Layer> {
    /// Composite this world onto the stack with `blend`, overriding the stack's
    /// default for this layer alone.
    fn blend(self, blend: Blend) -> Layer {
        Layer {
            blend: Some(blend),
            ..self.into()
        }
    }
}
impl AsLayer for Shape {}
impl AsLayer for Signal {}
impl AsLayer for Particles {}

impl From<Shape> for Layer {
    fn from(shape: Shape) -> Self {
        Layer {
            world: World::Signal(shape.into()),
            blend: None,
        }
    }
}
impl From<Signal> for Layer {
    fn from(signal: Signal) -> Self {
        Layer {
            world: World::Signal(signal),
            blend: None,
        }
    }
}
impl From<Particles> for Layer {
    fn from(particles: Particles) -> Self {
        Layer {
            world: World::Particles(particles),
            blend: None,
        }
    }
}

/// A stack of worlds composed into one scene (see [`layers()`]), listed top
/// first. Each item is a [`Layer`] — a [`Signal`] (or a [`Shape`] lifted through
/// the bridge) plus how it blends onto the worlds beneath it.
pub struct Layers {
    pub(crate) layers: Vec<Layer>,
    /// The blend a layer falls back to when it pins none of its own.
    pub(crate) blend: Blend,
}

impl Layers {
    /// The stack's default composite mode — every layer that doesn't pin its own
    /// [`AsLayer::blend`] falls back to this. `Blend::Over` (a layers panel)
    /// unless set. A pure-geometry stack on `over` keeps the cheap single-pass
    /// path (painter's-order alpha *is* over); asking for `Add` composites each
    /// layer to its own texture so the sum of light is honest.
    pub fn blend(mut self, blend: Blend) -> Self {
        self.blend = blend;
        self
    }

    /// The bridge-link: the whole stack collapses into one texture [`Signal`]
    /// (every layer's geometry, in order) and flows into the Braid world — so
    /// `layers(..).render().feedback(..)` runs *one* loop over the whole stack.
    /// Distinct from stacking feedback *layers*: this re-sources the stack as a
    /// single energy field (any per-layer feedback or blend is subsumed).
    pub fn render(self) -> Signal {
        Signal {
            // `.rev()`: the artist lists top-first, the pass draws back-to-front
            // (last on top) — same translation as `flatten`.
            source: self
                .layers
                .into_iter()
                .rev()
                .flat_map(|l| match l.world {
                    World::Signal(s) => s.source,
                    // A point cloud has no geometry strokes to fold in; folding a
                    // stack into one geometry source drops it (an edge case —
                    // `.render()` over particles is ill-defined by design).
                    World::Particles(_) => Vec::new(),
                })
                .collect(),
            swirl: None,
        }
    }

    /// The terminal link: opens the window, brings up the GPU, and runs the
    /// render loop.
    pub fn show(self) {
        shell::run(self.flatten());
    }
}

// ---------------------------------------------------------------------------
// The point-cloud world — the third signal type
//
// A `Points` is a Signal whose payload is a GPU buffer of particles, stepped
// in a compute shader and drawn instanced. The script sets the count once;
// the simulation runs on the GPU and the per-particle data never crosses back
// (the anti-bottleneck stance). Built for a million+.
// ---------------------------------------------------------------------------

/// Births a [`Particles`] cloud of `count` particles, seeded across the scene
/// with a little random drift and simulated on the GPU. Hang forces on it to
/// give it behavior — `.swirl()`, `.gravity()`, `.attract()`, `.repel()`,
/// `.orbit()` — composed in the chain, run on the GPU.
pub fn particles(count: u32) -> Particles {
    Particles {
        count: count.max(1),
        forces: Vec::new(),
    }
}

/// A point-cloud signal. Holds the recipe (count + forces) until a terminal
/// link runs it. Forces compose: each `.link()` adds one, applied every frame.
#[derive(Clone)]
pub struct Particles {
    pub(crate) count: u32,
    pub(crate) forces: Vec<Force>,
}

/// The default reach of the local forces (`attract`/`repel`/`orbit`), in scene
/// units — full effect at the source, fading to nothing at this distance.
const FORCE_RADIUS: f32 = 0.35;

impl Particles {
    /// Rotation around the scene center — a galaxy. `strength` in radians/sec.
    pub fn swirl(mut self, strength: f32) -> Self {
        self.forces.push(Force::Swirl { strength });
        self
    }

    /// A constant pull; the vector *is* the force (magnitude = strength).
    /// `gravity(0.0, -0.3)` rains down.
    pub fn gravity(mut self, x: f32, y: f32) -> Self {
        self.forces.push(Force::Gravity { dir: [x, y] });
        self
    }

    /// Pulls particles toward a source (the `mouse()` or a fixed point), within
    /// reach. A swarm that gathers.
    pub fn attract(mut self, at: impl Into<Pos>, strength: f32) -> Self {
        self.forces.push(Force::Radial {
            at: at.into().into(),
            strength,
            radius: FORCE_RADIUS,
        });
        self
    }

    /// Pushes particles away from a source, within reach. Drag to carve.
    pub fn repel(mut self, at: impl Into<Pos>, strength: f32) -> Self {
        self.forces.push(Force::Radial {
            at: at.into().into(),
            strength: -strength,
            radius: FORCE_RADIUS,
        });
        self
    }

    /// Swirls particles tangentially around a source, within reach — a local
    /// vortex. Compose with `repel` for a cursor you stir the cloud with.
    pub fn orbit(mut self, at: impl Into<Pos>, strength: f32) -> Self {
        self.forces.push(Force::Orbit {
            at: at.into().into(),
            strength,
            radius: FORCE_RADIUS,
        });
        self
    }

    /// The terminal link: opens the window, brings up the GPU, and runs the
    /// render loop.
    pub fn show(self) {
        shell::run(self.flatten());
    }
}

// ---------------------------------------------------------------------------
// live() — the sketch as a function, re-described whenever a knob turns
// ---------------------------------------------------------------------------

// The sealed-trait pattern: `Flatten` is unnameable outside the crate, so its
// method — whose return type is crate-private — is unreachable from user
// code. The lint can't see that; the allow is deliberate.
#[allow(private_interfaces)]
mod sealed {
    use super::{Blend, Layer, Layers, Particles, Shape, Signal, World};
    use crate::recipe::{CompositeLayer, Recipe};

    /// Flattens a chain into the recipe the core runs. Internal machinery —
    /// sketches never call this.
    pub trait Flatten {
        fn flatten(self) -> Recipe;
    }

    impl Flatten for Shape {
        fn flatten(self) -> Recipe {
            Recipe::Shapes(vec![self.stroke])
        }
    }
    impl Flatten for Layers {
        fn flatten(self) -> Recipe {
            // The artist lists top-first (item 0 is the frontmost, like a layers
            // panel); the core draws back-to-front. So `.rev()` here is the one
            // translation between the two orders — everything downstream draws in
            // vec order, last on top, as it always has.
            let stack = self.blend;
            let blend_of = |l: &Layer| l.blend.unwrap_or(stack);

            // Cheap path: a stack of pure geometry (no feedback, no particles)
            // compositing entirely `over` — the default — is one shapes pass in
            // painter's order, exactly what layers() always was. Over is
            // associative, so per-stroke alpha equals per-layer compositing;
            // the single pass isn't an approximation, it's the same image. Any
            // feedback or particle layer, or any layer asking to *add* its
            // light, pays for offscreen targets so the modes apply per layer.
            let plain_geometry =
                |l: &Layer| matches!(&l.world, World::Signal(s) if s.swirl.is_none());
            let all_geometry = self.layers.iter().all(plain_geometry);
            let all_over = self.layers.iter().all(|l| blend_of(l) == Blend::Over);
            if all_geometry && all_over {
                Recipe::Shapes(
                    self.layers
                        .into_iter()
                        .rev()
                        .flat_map(|l| match l.world {
                            World::Signal(s) => s.source,
                            World::Particles(_) => Vec::new(),
                        })
                        .collect(),
                )
            } else {
                Recipe::Composite(
                    self.layers
                        .into_iter()
                        .rev()
                        .map(|l| {
                            let blend = blend_of(&l);
                            let recipe = match l.world {
                                World::Signal(s) => s.flatten(),
                                World::Particles(p) => p.flatten(),
                            };
                            CompositeLayer { blend, recipe }
                        })
                        .collect(),
                )
            }
        }
    }
    impl Flatten for Signal {
        fn flatten(self) -> Recipe {
            match self.swirl {
                Some(swirl) => Recipe::Feedback {
                    source: self.source,
                    swirl,
                },
                // No feedback hung on the chain? Identity: just the source.
                None => Recipe::Shapes(self.source),
            }
        }
    }
    impl Flatten for Particles {
        fn flatten(self) -> Recipe {
            Recipe::Points {
                count: self.count,
                forces: self.forces,
            }
        }
    }
}
pub(crate) use sealed::Flatten;

/// Anything a sketch can end on: a [`Shape`], [`Layers`], [`Signal`], or
/// [`Particles`] chain.
pub trait Chain: sealed::Flatten {}

impl Chain for Shape {}
impl Chain for Layers {}
impl Chain for Signal {}
impl Chain for Particles {}

/// Runs the sketch as a *function of its knobs*: the chain is re-described
/// whenever a `tune(..)`d value changes, and the running picture follows.
/// This is the hot-reload seam in miniature — the tweak panel turns it
/// today; MIDI, OSC, and the script dialects will turn it tomorrow.
///
/// Without any front-end attached, `live` behaves exactly like `.show()`.
pub fn live<C: Chain>(sketch: impl Fn() -> C + 'static) {
    shell::run_live(Box::new(move || sketch().flatten()));
}

// ---------------------------------------------------------------------------
// Knobs and input signals
// ---------------------------------------------------------------------------

/// A position source for [`Shape::at`]: the mouse, or a fixed point in scene
/// space. In the dataflow view a constant is just another signal.
pub enum Pos {
    Mouse,
    Fixed(f32, f32),
}

impl From<Mouse> for Pos {
    fn from(_: Mouse) -> Self {
        Pos::Mouse
    }
}

impl From<(f32, f32)> for Pos {
    fn from((x, y): (f32, f32)) -> Self {
        Pos::Fixed(x, y)
    }
}

impl From<Pos> for Source {
    fn from(pos: Pos) -> Self {
        match pos {
            Pos::Mouse => Source::MOUSE,
            Pos::Fixed(x, y) => Source {
                point: [x, y],
                from_mouse: false,
            },
        }
    }
}

/// The waveform menu for [`Wave`] — TouchDesigner-style oscillator shapes.
/// All share the same vocabulary: one cycle per turn.
#[derive(Clone, Copy, Default)]
pub enum Osc {
    /// Eases through the cycle.
    #[default]
    Sine,
    /// Sine, a quarter-cycle ahead.
    Cosine,
    /// Linear there and back.
    Triangle,
    /// Sawtooth: rises across the cycle, then snaps back.
    Ramp,
    /// Half the cycle at +1, half at -1.
    Square,
    /// Like square, but high only 25% of the cycle.
    Pulse,
}

/// The motion knobs. Frequencies are in cycles per second; `phase` is in
/// turns (`1.0` = one full cycle); `amp` is in scene units; `shape` picks the
/// waveform ([`Osc`]). The axes run in quadrature (y a quarter-cycle ahead),
/// so equal frequencies orbit instead of sliding on a diagonal; an axis at
/// 0 Hz stays still. The defaults draw a slow, slightly irrational orbit —
/// trails that never quite repeat.
#[derive(Clone, Copy)]
pub struct Wave {
    /// Motion amplitude, in scene units.
    pub amp: f32,
    /// Horizontal frequency, cycles per second. `0.0` = still axis.
    pub x: f32,
    /// Vertical frequency, cycles per second. `0.0` = still axis.
    pub y: f32,
    /// Phase offset, in turns. Stagger it across layers to make voices.
    pub phase: f32,
    /// The waveform both axes run.
    pub shape: Osc,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            amp: 0.3,
            x: 0.16,
            y: 0.21,
            phase: 0.0,
            shape: Osc::Sine,
        }
    }
}

/// The paint knobs. `base` is degrees on the color wheel (0 = red, 120 =
/// green, 240 = blue); `drift` slides it over time, in degrees per second.
/// A bare `f32` converts: `hue(200.0)` just works.
#[derive(Clone, Copy, Default)]
pub struct Hue {
    pub base: f32,
    pub drift: f32,
}

impl From<f32> for Hue {
    fn from(base: f32) -> Self {
        Self { base, drift: 0.0 }
    }
}

/// The proximity-growth knobs. Distances are in scene units — fractions of
/// the shorter screen edge. Everything has a `Default` (Principle 2), and
/// every value is valid (Principle 3): `scale < 1.0` shrinks instead.
#[derive(Clone, Copy)]
pub struct Falloff {
    /// Inside this distance from the source, the effect is full.
    pub min: f32,
    /// Beyond this distance, no effect. In between: smoothstep.
    pub max: f32,
    /// Radius multiplier at the epicenter. `1.0` = none; `<1.0` shrinks.
    pub scale: f32,
}

impl Default for Falloff {
    fn default() -> Self {
        Self {
            min: 0.05,
            max: 0.30,
            scale: 3.0,
        }
    }
}

/// The mouse as an input signal: its position, in scene space, fed to the
/// chain by the core every frame. It plugs into sockets like [`Shape::at`]
/// and [`Shape::grow`] — the socket is the design; tomorrow `touch(0)` or an
/// oscillator plugs into the same place.
pub fn mouse() -> Mouse {
    Mouse
}

/// The mouse input signal (see [`mouse()`]).
#[derive(Clone, Copy, Default)]
pub struct Mouse;
