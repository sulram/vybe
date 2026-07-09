//! Six oscillators, TouchDesigner-style: the same motion link, six waveforms.
//!
//! The kata that pulled the `Osc` menu into `Wave`: sine, cosine, triangle,
//! ramp, square, pulse — one row each, swinging horizontally. The waveform's
//! character shows as movement: sine eases, ramp sweeps and snaps back,
//! square teleports between sides, pulse mostly waits.
//!
//! Run with:  cargo run --example osc

use vybe::*;

fn main() {
    let waveforms = [
        Osc::Sine,
        Osc::Cosine,
        Osc::Triangle,
        Osc::Ramp,
        Osc::Square,
        Osc::Pulse,
    ];
    layers(waveforms.into_iter().enumerate().map(|(i, shape)| {
        circle(0.035)
            .at((0.0, 0.30 - i as f32 * 0.12)) // one row per waveform, top→bottom
            .hue(i as f32 * 60.0) // 360 / 6 around the wheel
            .wave(Wave {
                amp: 0.35,
                x: 0.2,     // a five-second cycle, horizontal only
                y: 0.0,     // 0 Hz: this axis stays still
                phase: 0.0, // all rows aligned, so the shapes compare
                shape,
            })
    }))
    .show();
}
