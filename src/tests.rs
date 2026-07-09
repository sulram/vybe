//! First tests — the pure, GPU-free half of the engine: that describing a
//! chain is total (never panics), and that the sugar flattens to the [`Recipe`]
//! the core runs. No window, no device — only chain → recipe. The GPU itself is
//! exercised by the example gallery (a broken pipeline fails those at compile
//! time); here we pin the sugar↔recipe seam that has no visual to guard it.

use crate::recipe::Recipe;
use crate::sugar::{Blend, Flatten, Hue, Pos, Swirl, circle, layers, mouse, particles, signal};

/// Float compare without tripping `clippy::float_cmp` — values here are exact
/// round-trips, so any real epsilon does.
fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn empty_signal_flattens_to_no_strokes() {
    // Principle 2: empty is the identity — it describes cleanly and shows black.
    let Recipe::Shapes(strokes) = signal().flatten() else {
        panic!("empty signal should flatten to Shapes");
    };
    assert!(strokes.is_empty());
}

#[test]
fn rendered_shape_is_one_stroke() {
    let Recipe::Shapes(strokes) = circle(0.2).render().flatten() else {
        panic!("a rendered shape should flatten to Shapes");
    };
    assert_eq!(strokes.len(), 1);
    assert!(approx(strokes[0].radius, 0.2));
}

#[test]
fn hanging_feedback_flattens_to_feedback() {
    let recipe = circle(0.05).render().feedback(Swirl::default()).flatten();
    assert!(matches!(recipe, Recipe::Feedback { .. }));
}

#[test]
fn particles_flatten_to_points_carrying_their_forces() {
    let Recipe::Points { count, forces } = particles(1000).swirl(0.3).gravity(0.0, -0.2).flatten()
    else {
        panic!("particles should flatten to Points");
    };
    assert_eq!(count, 1000);
    assert_eq!(forces.len(), 2);
}

#[test]
fn zero_inputs_clamp_instead_of_degenerating() {
    // Principle 3: every value is valid. grid(0,0) and particles(0) cannot make
    // a degenerate zero — they clamp up to one.
    let Recipe::Shapes(strokes) = circle(0.1).grid(0, 0).render().flatten() else {
        panic!("grid should still flatten to Shapes");
    };
    assert_eq!(strokes[0].cols, 1);
    assert_eq!(strokes[0].rows, 1);

    let Recipe::Points { count, .. } = particles(0).flatten() else {
        panic!("particles(0) should still flatten to Points");
    };
    assert_eq!(count, 1);
}

#[test]
fn plain_over_stack_collapses_to_one_pass_back_to_front() {
    // All plain geometry on the default `over` → the cheap single Shapes pass.
    // Listed top-first (0.1 in front); flattened back-to-front (0.1 drawn last).
    let Recipe::Shapes(strokes) = layers([circle(0.1), circle(0.2), circle(0.3)]).flatten() else {
        panic!("a plain over stack should collapse to one Shapes pass");
    };
    assert_eq!(strokes.len(), 3);
    assert!(approx(strokes[0].radius, 0.3));
    assert!(approx(strokes[2].radius, 0.1));
}

#[test]
fn add_blend_forces_the_composite_path() {
    // Asking the stack to add its light can't be one painter's-order pass.
    let recipe = layers([circle(0.1), circle(0.2)])
        .blend(Blend::Add)
        .flatten();
    assert!(matches!(recipe, Recipe::Composite(_)));
}

#[test]
fn a_feedback_layer_forces_the_composite_path() {
    // A mixed stack (one layer carries feedback) needs offscreen targets.
    let stack = crate::layers![
        circle(0.05).render().feedback(Swirl::default()),
        circle(0.3).grid(8, 8),
    ];
    assert!(matches!(stack.flatten(), Recipe::Composite(_)));
}

#[test]
fn a_particle_layer_composites_and_keeps_every_layer() {
    let stack = crate::layers![particles(500).swirl(0.2), circle(0.3).grid(8, 8)];
    let Recipe::Composite(sublayers) = stack.flatten() else {
        panic!("a stack with particles composites");
    };
    assert_eq!(sublayers.len(), 2);
}

#[test]
fn f32_reads_as_a_hue() {
    let h: Hue = 200.0.into();
    assert!(approx(h.base, 200.0));
    assert!(approx(h.drift, 0.0));
}

#[test]
fn a_pos_is_the_mouse_or_a_fixed_point() {
    assert!(matches!(Pos::from((0.1, 0.2)), Pos::Fixed(_, _)));
    assert!(matches!(Pos::from(mouse()), Pos::Mouse));
}
