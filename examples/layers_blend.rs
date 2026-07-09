//! Blend modes, mixed *per layer* — and written the other way round: each world
//! bound to a name first, then stacked. No one long chain; the structure is
//! three worlds and a composition, one of them a point cloud.
//!
//! The stack is listed **top first** (item 0 is the frontmost):
//!   - `spark` — a crisp disc laid *over* everything (the default), occluding
//!     what it covers;
//!   - `particles_swarm` — a galaxy of sparks whose light *adds* onto the floor;
//!   - `field` — a deep-blue floor at the back.
//!
//! So: swarm `add`, spark and field on the default `over` — mixed modes in one
//! stack, which the whole-stack `.blend()` (see `layers_aurora`) can't say. Only the
//! layer that *differs* from the default spells a blend; the rest stay bare.
//! Written with the `layers![..]` macro so a `Shape`, a `Particles`, and a
//! `.blend(..)`ed layer can share one list despite being different types.
//!
//! Run with:  cargo run --example layers_blend

use vybe::*;

fn main() {
    let field = circle(0.30).grid(14, 14).soft(0.9).hue(Hue {
        base: 220.0,
        drift: 0.0,
    });

    let swarm = particles(150_000)
        .swirl(0.7) // an ambient galaxy spin…
        .attract(mouse(), 0.7); // …gathering toward the cursor

    let spark = circle(0.18)
        .soft(0.0) // crisp and opaque, so `over` cleanly covers
        .hue(Hue {
            base: 35.0,
            drift: 0.0,
        })
        .wave(Wave {
            amp: 0.33,
            x: 0.06,
            y: 0.05,
            ..Default::default()
        });

    layers![
        spark,                   // top — default (over): the disc covers what it crosses
        swarm.blend(Blend::Add), // middle — add: the galaxy's light glows onto the floor
        field,                   // back — default (over; on black = itself)
    ]
    .show();
}
