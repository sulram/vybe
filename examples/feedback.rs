//! The first kata, purified: a feedback loop fed by the chain itself.
//!
//! In Phase 0 the light source lived hardcoded inside the feedback shader —
//! the necessary hack that kept the loop from starting black and staying
//! black. The `.render()` bridge made it honest: the source is now geometry,
//! described in the language. Same soul, no hack.
//!
//! Run with:  cargo run --example feedback

use vybe::*;

fn main() {
    circle(0.05)
        .soft(1.0) // a ball of light, not a disc
        .hue(Hue {
            base: 200.0,
            drift: 25.0,
        }) // color sliding around the wheel
        .wave(Wave::default()) // the orbit, once a hack, now a knob
        .render() // the bridge: geometry -> texture
        .feedback(Swirl {
            decay: 0.16, // fraction of the trail surviving per second → the memory
            angle: 0.9,  // radians per second → the swirl's spin
            scale: 0.74, // zoom per second (<1 = spirals outward)
        })
        .show();
}
