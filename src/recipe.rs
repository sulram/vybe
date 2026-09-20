//! THE RECIPE — what the core draws, as plain data.
//!
//! This is "chain = AST" made literal: `a().b().c()` collapses into plain data
//! here. Two front-ends hand it over today — the Rust sugar (once, at the
//! terminal link, or again whenever a knob turns) and the patch player (every
//! frame, with this frame's Scalars already resolved to numbers). Every
//! dialect, every front-end, one recipe.

use std::path::PathBuf;
use std::sync::Arc;

use crate::sugar::{Blend, Falloff, Hue, Osc, Swirl, Wave};

/// What a chain describes, handed from a front-end to the core. One variant
/// per kind of world, plus the two that give a description *structure*:
/// [`Recipe::Composite`] (a stack) and [`Recipe::Named`] (an identity).
#[derive(Clone)]
pub(crate) enum Recipe {
    /// Geometry strokes, drawn in order.
    Shapes(Vec<Stroke>),
    /// Geometry strokes rendered as the energy source of a feedback loop.
    Feedback { source: Vec<Stroke>, swirl: Swirl },
    /// A point cloud: `count` particles simulated in a compute shader and
    /// drawn instanced. The third signal type — payload is a GPU buffer. The
    /// `forces` are the behavior the sketch composed; the compute step applies
    /// them (a bounded, declarative vocabulary — not a general value-flow).
    Points { count: u32, forces: Vec<Force> },
    /// One frame of an image sequence, placed in scene space.
    Image(Image),
    /// A playing clip (video): whatever frame its slot holds, placed like an image.
    Stream(Stream),
    /// A stack of worlds composited into one frame, bottom to top. Each
    /// [`CompositeLayer`] is its own sub-recipe rendered to its own signal
    /// texture, plus how it lands on the worlds beneath it. The seam
    /// `layers()` reaches for when a stack mixes worlds or a layer asks to
    /// *add* its light — a plain all-`over` stack of geometry stays one
    /// [`Recipe::Shapes`] (painter's-order alpha *is* over; the cheaper path).
    Composite(Vec<CompositeLayer>),
    /// **Key = identity.** The wrapped world is *the* node of that name: it is
    /// rendered once per frame however many times the name appears, and its GPU
    /// state (a feedback trail, a particle buffer) survives re-description for
    /// as long as the name keeps appearing. This is what makes a wire a wire.
    Named(String, Box<Recipe>),
}

/// One layer of a [`Recipe::Composite`]: the world to render offscreen, the
/// [`Blend`] with which it lands on the worlds beneath it, and its opacity.
#[derive(Clone)]
pub(crate) struct CompositeLayer {
    pub recipe: Recipe,
    pub blend: Blend,
    /// `1.0` = as rendered. The whole cross-scene transition is this number.
    pub alpha: f32,
}

/// One frame out of an image sequence. The sequence is shared (`Arc`) so a
/// re-described recipe points at the same files and the core keeps its
/// textures; only `index` moves.
#[derive(Clone)]
pub(crate) struct Image {
    pub frames: Arc<[PathBuf]>,
    pub index: usize,
    pub fit: Fit,
    /// Center, scene space.
    pub place: [f32; 2],
    /// Multiplies the fitted size.
    pub size: f32,
    pub alpha: f32,
}

/// A playing clip's picture. The `slot` is shared (`Arc`) with whoever decodes:
/// the same slot across re-descriptions is the same node, so the core keeps the
/// clip's texture and only uploads when a new frame arrives.
#[derive(Clone)]
pub(crate) struct Stream {
    pub slot: Arc<crate::clip::Slot>,
    pub fit: Fit,
    /// Center, scene space.
    pub place: [f32; 2],
    /// Multiplies the fitted size.
    pub size: f32,
    pub alpha: f32,
}

/// What an image is fitted into (always contained, never cropped).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fit {
    /// The scene's unit square — the shorter screen edge. A square image
    /// fills a square face.
    Unit,
    /// The whole frame, whatever its aspect.
    Frame,
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
            Recipe::Image(_) => "vybe — image",
            Recipe::Stream(_) => "vybe — video",
            Recipe::Composite(_) => "vybe — layers",
            Recipe::Named(_, inner) => inner.title(),
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

/// The prototype a stroke stamps. A line is not a third form: it is a [`Form::Rect`]
/// laid along its two points (see `sugar::line`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    Circle,
    Rect,
}

/// One flattened shape-chain — a single gesture: a prototype, its placement,
/// motion, and paint. [`Stroke::IDENTITY`] is the resting state (Principle 2):
/// one white circle, centered, still. Every link fills in only what it touches.
#[derive(Clone, Copy)]
pub(crate) struct Stroke {
    pub form: Form,
    /// Circle: the radius. As a fraction of the grid cell.
    pub radius: f32,
    /// Rect: width and height. As a fraction of the grid cell.
    pub extent: [f32; 2],
    /// Rotation, radians, counter-clockwise.
    pub angle: f32,
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
    /// Brightness, 0 = black, 1 = full. Set by the `gray()` link.
    pub value: f32,
    pub alpha: f32,
    pub soft: f32,
    /// Outline width in scene units, drawn inside the edge. 0 = filled.
    pub outline: f32,
    pub falloff: Falloff,
}

impl Stroke {
    pub(crate) const IDENTITY: Self = Self {
        form: Form::Circle,
        radius: 0.25,
        extent: [0.5, 0.5],
        angle: 0.0,
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
        value: 1.0,
        alpha: 1.0,
        soft: 0.0,
        outline: 0.0,
        falloff: Falloff {
            min: 0.0,
            max: 1.0,
            scale: 1.0,
        },
    };

    /// A stroke from `a` to `b`, `width` thick: a rect laid along the segment.
    pub(crate) fn line(a: [f32; 2], b: [f32; 2], width: f32) -> Self {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        Self {
            form: Form::Rect,
            extent: [dx.hypot(dy), width],
            angle: dy.atan2(dx),
            place: Source {
                point: [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5],
                from_mouse: false,
            },
            ..Self::IDENTITY
        }
    }
}
