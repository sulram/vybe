//! Falling particles you part with the mouse.
//!
//! `gravity` is a constant pull (the vector is the force); particles fall and
//! wrap around to the top. `repel` pushes them from the cursor, so you carve a
//! parting through the rain as you move.
//!
//! Run with:  cargo run --example particles_rain --release

use vybe::*;

fn main() {
    particles(60_000)
        .gravity(0.0, -0.35) // fall (negative y is down in scene space)
        .repel(mouse(), 0.8) // parted by the cursor
        .show();
}
