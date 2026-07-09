//! Integration tests over the public surface — what a sketch author can touch.
//! The terminal links (`show`, `live`) bring up the GPU and open a window, so
//! CI can't run them; we exercise everything up to them — building every chain
//! kind, and the `tune()` knob registry, which is std-only and observable.

use vybe::*;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn tune_registers_once_and_then_keeps_its_value() {
    // First mention registers at the default and returns it.
    assert!(approx(tune("api_reach", 0.30, 0.0..=1.0), 0.30));
    // A later mention with a different default keeps the registered value —
    // a knob is named state, not a fresh default each call.
    assert!(approx(tune("api_reach", 0.99, 0.0..=1.0), 0.30));
}

#[test]
fn tune_is_total_without_a_frontend() {
    // Principle 3: with no panel attached, tune just returns the default.
    for i in 0..3 {
        let name = format!("api_total_{i}");
        assert!(approx(tune(&name, 0.5, 0.0..=1.0), 0.5));
    }
}

#[test]
fn blend_defaults_to_over() {
    assert_eq!(Blend::default(), Blend::Over);
}

#[test]
fn every_public_chain_kind_builds_without_panic() {
    // Describing is total (Principles 2 & 3): each constructor and link is
    // reachable and none of these panic. We stop just before the GPU terminal.
    let _feedback = circle(0.05)
        .soft(1.0)
        .hue(Hue {
            base: 200.0,
            drift: 25.0,
        })
        .wave(Wave::default())
        .render()
        .feedback(Swirl::default());

    let _dots = circle(0.3).grid(32, 32).grow(mouse(), Falloff::default());

    let _cloud = particles(1000)
        .swirl(0.6)
        .gravity(0.0, -0.3)
        .attract(mouse(), 0.5);

    // A heterogeneous stack (Particles beside a Shape) via the layers! macro.
    let _mixed = layers![
        particles(1000)
            .swirl(0.6)
            .attract(mouse(), 0.5)
            .blend(Blend::Add),
        circle(0.30).grid(16, 16).soft(0.9).hue(265.0),
    ];

    // A homogeneous stack from an iterator.
    let _fan = layers((0..8).map(|i| circle(0.06).hue(i as f32 * 36.0)));
}
