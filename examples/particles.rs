//! A GPU particle cloud you stir with the mouse — and tune live with sliders.
//!
//! The point-cloud signal type. `particles(n)` seeds a GPU buffer; the forces
//! hung on the chain are the behavior, run by the compute step every frame; an
//! instanced draw reads the buffer. Wrapping the chain in `live(|| ..)` and
//! reading each force strength from `tune(..)` gives a panel of sliders — drag
//! them while the cloud stirs, and the particles keep their positions (only the
//! tiny force uniform is rewritten, no reseed).
//!
//! Move the mouse to stir; drag the sliders to reshape the vortex. Other
//! clouds: `particles_galaxy`, `particles_swarm`, `particles_rain` — same substrate, different forces.
//!
//! Run with:  cargo run --example particles --release
//! (`count` is structural — change it in the source, not a slider.)

use vybe::*;

fn main() {
    live(|| {
        particles(80_000)
            .swirl(tune("swirl", 0.25, 0.0..=1.0)) // ambient rotation around center
            .repel(mouse(), tune("repel", 0.7, 0.0..=2.0)) // push from the cursor
            .orbit(mouse(), tune("orbit", 1.1, 0.0..=3.0)) // swirl around it
    });
}
