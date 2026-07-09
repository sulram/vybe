//! The dots kata with two knobs picked out — not every parameter, the ones
//! this sketch cares about. `tune(..)` picks a value by name; `live(..)`
//! keeps the sketch re-describable; the panel shows exactly those knobs and
//! nothing else. (Examples always carry the panel; the lib never does.)
//!
//! Run with:  cargo run --example dots_tune

use vybe::*;

fn main() {
    live(|| {
        circle(0.3).grid(32, 32).grow(
            mouse(),
            Falloff {
                min: 0.05,
                max: tune("reach", 0.30, 0.0..=1.0),
                scale: tune("burst", 3.0, 0.0..=6.0),
            },
        )
    });
}
