//! A swarm that gathers on the cursor and orbits it.
//!
//! Two forces toward the mouse: `attract` pulls particles in, `orbit` sends
//! them around — together they form a living knot that follows the pointer.
//! When the cursor leaves the window the mouse rests far away, so the forces
//! fade and the cloud drifts free.
//!
//! Run with:  cargo run --example particles_swarm --release

use vybe::*;

fn main() {
    particles(80_000)
        .attract(mouse(), 0.6) // gather toward the cursor…
        .orbit(mouse(), 0.5) // …and orbit it — a swarm that follows
        .show();
}
