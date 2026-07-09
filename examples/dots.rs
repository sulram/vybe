//! The second kata: a grid of dots that swell near the mouse.
//!
//! This is the sketch that pulls geometry, instancing, and input-as-signal
//! into the core — the "many balls" + "reacts to the mouse" steps of the
//! README progression, in one visual. All of wgpu stays hidden in `vybe`.
//!
//! Run with:  cargo run --example dots

use vybe::*;

fn main() {
    // circle() births a Shape (the geometry-signal); grid() scatters it as GPU
    // instances (one draw call); grow() hangs the mouse falloff; show() runs.
    circle(0.3) // radius: 30% of a grid cell
        .grid(32, 32) // 32×32 instances
        .grow(
            mouse(),
            Falloff {
                min: 0.05,  // full effect inside 5% of the screen
                max: 0.30,  // fades to nothing at 30% of the screen
                scale: 3.0, // radius multiplier at the epicenter (<1.0 shrinks)
            },
        )
        .show();
}
