//! Rainbow with memory: layers -> bridge -> feedback, one unbroken chain.
//!
//! The kata that pulled `Layers::render()`: the whole stack of chains becomes
//! one texture signal and enters the loop. Geometry world, composition, and
//! the Braid world — three ideas, one sentence.
//!
//! Run with:  cargo run --example rainbow_trails

use vybe::*;

fn main() {
    layers((0..10).map(|i| {
        let t = i as f32 / 10.0;
        circle(0.06)
            .soft(0.3) // slightly soft, so the trails blend like light
            .hue(t * 360.0) // the wheel, split ten ways
            .wave(Wave {
                amp: 0.35,
                x: 0.11 + t * 0.06, // each voice its own tempo…
                y: 0.17 + t * 0.04,
                phase: t, // …and its own place in the cycle
                ..Default::default()
            })
    }))
    .render() // the bridge: the whole stack becomes one signal
    .feedback(Swirl {
        decay: 0.02, // fraction surviving per second → a short trail
        angle: 0.12, // a faint spin on the trail, per second
        scale: 0.94, // and a slow outward drift, per second
    })
    .show();
}
