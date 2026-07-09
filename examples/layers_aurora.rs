//! Two worlds in one frame: a quiet field of dots, and a rainbow orbit that
//! remembers where it's been. The kata that pulled **compositing** — `layers()`
//! stops meaning "stack geometry" and starts meaning "stack worlds": feedback
//! lives on one layer without smearing the other.
//!
//! How it reads. The outer `layers([..])` composites two [`Signal`]s. The
//! engine notices one carries feedback and leaves the cheap single-pass path
//! for offscreen-per-layer + a compositor. The stack asks for `Add` — this is
//! a sky made of light, summed onto the floor: black adds nothing, so the dots
//! show through wherever the trail has faded to dark. (Bare, the stack would
//! composite `over` — the sky *covering* the floor by its coverage instead.)
//! The inner `layers(..).render().feedback(..)` is the older move (a whole
//! stack folded into ONE loop, from `rainbow_trails`), now itself just one
//! layer in the stack. Two meanings of `layers`, composing cleanly.
//!
//! Run with:  cargo run --example layers_aurora

use vybe::*;

fn main() {
    // Listed top first: the trailing sky in front, the still floor behind.
    layers([
        // The sky: six hues orbiting, each its own tempo, folded into one
        // feedback loop — a rainbow that trails light behind it.
        layers((0..6).map(|i| {
            let t = i as f32 / 6.0;
            circle(0.05)
                .soft(1.0) // balls of light, so the trails blend
                .hue(Hue {
                    base: t * 360.0,
                    drift: 40.0,
                })
                .wave(Wave {
                    amp: 0.33,
                    x: 0.12 + t * 0.05, // each voice its own tempo…
                    y: 0.18 + t * 0.04,
                    phase: t, // …and its own place in the cycle
                    ..Default::default()
                })
        }))
        .render() // the bridge: the whole rainbow stack becomes one signal
        .feedback(Swirl {
            decay: 0.05, // a short, luminous trail
            angle: 0.25, // a gentle spin on the memory
            scale: 0.9,  // drifting slowly outward
        }),
        // The floor: a still grid of soft dots that swells toward the mouse.
        // `.render()` lifts it into the texture world as a plain, feedback-free
        // layer — proof each layer is a full, live world of its own.
        circle(0.22)
            .grid(24, 24)
            .grow(
                mouse(),
                Falloff {
                    min: 0.05,
                    max: 0.35,
                    scale: 2.2,
                },
            )
            .render(),
    ])
    .blend(Blend::Add) // a stack of light: the sky sums onto the floor
    .show();
}
