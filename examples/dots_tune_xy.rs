//! A lens over the grid: move its focus with sliders, not the mouse.
//!
//! The kata that pulled `grow`'s generalization. `grow` used to be wired to
//! the mouse alone; now its epicenter is a position source — the same plug as
//! `at()`. Here two tuned scalars compose that position (a vector is just
//! scalars that travel together), so the swell is driven by hand, fully
//! decoupled from the pointer. `burst < 1.0` turns the lens into a pinch.
//!
//! Run with:  cargo run --example dots_tune_xy
//! (Drag x / y to move the focus; reach / burst shape it.)

use vybe::*;

fn main() {
    live(|| {
        circle(0.3).grid(32, 32).grow(
            (tune("x", 0.0, -0.5..=0.5), tune("y", 0.0, -0.5..=0.5)),
            Falloff {
                min: 0.02,
                max: tune("reach", 0.35, 0.0..=0.8),
                scale: tune("burst", 3.5, 0.0..=6.0),
            },
        )
    });
}
