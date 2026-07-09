//! A slowly rotating galaxy — the simplest force: one `swirl`.
//!
//! Same particle substrate as `particles`, a single force. `swirl` is a
//! rotation around the scene center; every particle orbits at its own radius,
//! so the whole field turns like a disc.
//!
//! Run with:  cargo run --example particles_galaxy --release

use vybe::*;

fn main() {
    particles(120_000).swirl(0.6).show();
}
