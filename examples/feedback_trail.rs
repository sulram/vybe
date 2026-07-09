//! A circle follows the mouse; the feedback turns motion into memory.
//!
//! The kata that pulled the bridge: `.render()` turns geometry into a texture
//! signal, and from there the chain flows through the Braid world. The chain
//! never breaks — it changes signal type at exactly that link.
//!
//! Run with:  cargo run --example feedback_trail

use vybe::*;

fn main() {
    circle(0.04)
        .soft(0.8) // a ball of light, not a disc
        .hue(Hue {
            base: 0.0,
            drift: 60.0,
        }) // the trail cycles the color wheel
        .at(mouse()) // input as a signal, plugged in
        .render() // the bridge: geometry -> texture
        .feedback(Swirl {
            decay: 0.02, // fraction surviving per second → a short comet tail
            angle: 0.12, // a faint spin on the trail, per second
            scale: 0.94, // and a slow outward drift, per second
        })
        .show();
}
