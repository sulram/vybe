//! THE RECIPE — the flattened chain the sugar hands to the core.
//!
//! This is "chain = AST" made literal: `a().b().c()` collapses into plain data
//! here. Today it's a Rust enum; tomorrow it is the seam where the TS/Lua
//! dialects and the node front-end plug in — every dialect, every front-end,
//! one recipe.

use crate::sugar::{Blend, Falloff, Hue, Osc, Swirl, Wave};

/// What a chain describes, handed from the sugar to the core by a terminal
/// link. One variant per chain kind that exists today.
#[derive(Clone)]
pub(crate) enum Recipe {
    /// Geometry strokes drawn straight to the screen, in order.
    Shapes(Vec<Stroke>),
    /// Geometry strokes rendered as the energy source of a feedback loop.
    Feedback { source: Vec<Stroke>, swirl: Swirl },
    /// A point cloud: `count` particles simulated in a compute shader and
    /// drawn instanced. The third signal type — payload is a GPU buffer. The
    /// `forces` are the behavior the sketch composed; the compute step applies
    /// them (a bounded, declarative vocabulary — not a general value-flow).
    Points { count: u32, forces: Vec<Force> },
    /// A stack of worlds composited into one frame, bottom to top. Each
    /// [`CompositeLayer`] is its own sub-recipe (some with feedback, some
    /// without) rendered to an offscreen signal texture, plus how it blends onto
    /// the worlds beneath it; the compositor combines them onto the screen. The
    /// seam `layers()` reaches for when a stack mixes worlds or a layer asks to
    /// *add* its light — a plain all-`over` stack of geometry stays one
    /// [`Recipe::Shapes`] (painter's-order alpha *is* over; the cheaper path).
    Composite(Vec<CompositeLayer>),
}

/// One layer of a [`Recipe::Composite`]: the world to render offscreen, and the
/// [`Blend`] with which it lands on the worlds beneath it (resolved from the
/// layer's own choice or the stack's default at flatten time).
#[derive(Clone)]
pub(crate) struct CompositeLayer {
    pub recipe: Recipe,
    pub blend: Blend,
}

/// One force acting on every particle each frame — the composable behavior a
/// `particles(..)` chain hangs on the cloud. All strengths are in scene units
/// per second per second (acceleration).
#[derive(Clone, Copy)]
pub(crate) enum Force {
    /// Rotation around the scene center (a galaxy). Divergence-free.
    Swirl { strength: f32 },
    /// A constant pull (the vector is the force): gravity, wind.
    Gravity { dir: [f32; 2] },
    /// Toward a source within `radius` (`+` attracts, `-` repels).
    Radial {
        at: Source,
        strength: f32,
        radius: f32,
    },
    /// Tangential around a source within `radius` (a local vortex).
    Orbit {
        at: Source,
        strength: f32,
        radius: f32,
    },
}

impl Recipe {
    pub(crate) fn title(&self) -> &'static str {
        match self {
            Recipe::Shapes(_) => "vybe — shapes",
            Recipe::Feedback { .. } => "vybe — feedback",
            Recipe::Points { .. } => "vybe — particles",
            Recipe::Composite(_) => "vybe — layers",
        }
    }
}

/// Where a link takes its position from: a fixed point in scene space, or the
/// live mouse. The one plug shared by `at()` (placement) and `grow()` (the
/// swell's epicenter) — resolved to a scene position in the shader. In the
/// dataflow view a constant is just another signal; tomorrow `touch(0)` or an
/// oscillator is a third `Source` variant, and nothing above has to change.
#[derive(Clone, Copy)]
pub(crate) struct Source {
    pub point: [f32; 2],
    pub from_mouse: bool,
}

impl Source {
    pub(crate) const ORIGIN: Self = Self {
        point: [0.0, 0.0],
        from_mouse: false,
    };
    pub(crate) const MOUSE: Self = Self {
        point: [0.0, 0.0],
        from_mouse: true,
    };
}

/// One flattened shape-chain — a single gesture: a prototype, its placement,
/// motion, and paint. [`Stroke::IDENTITY`] is the resting state (Principle 2):
/// one white circle, centered, still. Every link fills in only what it touches.
#[derive(Clone, Copy)]
pub(crate) struct Stroke {
    pub radius: f32,
    pub cols: u32,
    pub rows: u32,
    /// Where the shape sits (the `at()` link).
    pub place: Source,
    /// The epicenter the `grow()` falloff measures from.
    pub grow_at: Source,
    pub wave: Wave,
    pub hue: Hue,
    /// 0 = white (unpainted), 1 = full hue. Set by the `hue()` link.
    pub sat: f32,
    pub soft: f32,
    pub falloff: Falloff,
}

impl Stroke {
    pub(crate) const IDENTITY: Self = Self {
        radius: 0.25,
        cols: 1,
        rows: 1,
        place: Source::ORIGIN,
        // Irrelevant until a `grow()` link sets a real falloff; mouse is the
        // least-surprising default (grow was mouse-only before it took a source).
        grow_at: Source::MOUSE,
        wave: Wave {
            amp: 0.0,
            x: 0.0,
            y: 0.0,
            phase: 0.0,
            shape: Osc::Sine,
        },
        hue: Hue {
            base: 0.0,
            drift: 0.0,
        },
        sat: 0.0,
        soft: 0.0,
        falloff: Falloff {
            min: 0.0,
            max: 1.0,
            scale: 1.0,
        },
    };
}
