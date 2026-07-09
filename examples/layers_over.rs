//! *Over* — alpha compositing, the stack default: one world laid on top of
//! another by its own coverage, not added to it. A soft violet field is the
//! night; a galaxy of white sparks swirls *over* it, gathering toward the
//! mouse. Where the swarm is dense it sits on top; where it thins, the field
//! shows through. No blend is spelled anywhere — a bare stack composites like
//! a layers panel, top item in front.
//!
//! This is also the kata that pulled **particles into `layers()`** — a point
//! cloud is now a composable world like any other (the rule: everything
//! composes). Add `.blend(Blend::Add)` to the stack and the sparks stop
//! covering the field and start adding their light to it instead (`layers_aurora`
//! is a stack that wants exactly that; `layers_blend` mixes modes per layer).
//!
//! The worlds are a `Shape` and a `Particles` (different types), so they go in
//! the `layers![..]` macro, which converts each on its own; a plain `[..]`
//! array can't hold two types.
//!
//! Run with:  cargo run --example layers_over

use vybe::*;

fn main() {
    // Listed top first: the swarm in front, the field behind.
    layers![
        // A galaxy of white sparks, swirling and gathering to the cursor.
        particles(150_000).swirl(0.6).attract(mouse(), 0.5),
        // The night: a soft violet field, laid on black.
        circle(0.30).grid(16, 16).soft(0.9).hue(Hue {
            base: 265.0,
            drift: 0.0
        }),
    ]
    .show();
}
