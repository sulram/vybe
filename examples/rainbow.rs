//! Ten circles, ten hues, ten tempos — layers born as the simplest chord.
//!
//! The kata that pulled `layers()`: chains composed into one scene, drawn in
//! order. Each voice is the same chain with three knobs turned — built in a
//! plain Rust loop, because a chain is just data.
//!
//! Run with:  cargo run --example rainbow

use vybe::*;

fn main() {
    layers((0..10).map(|i| {
        let t = i as f32 / 10.0;
        circle(0.06)
            .hue(t * 360.0) // the wheel, split ten ways
            .wave(Wave {
                amp: 0.35,
                x: 0.11 + t * 0.06, // each voice its own tempo…
                y: 0.17 + t * 0.04,
                phase: t, // …and its own place in the cycle
                ..Default::default()
            })
    }))
    .show();
}
