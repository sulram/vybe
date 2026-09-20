//! Kata pairs: the same picture written twice — as a Rust chain and as a `.vy`
//! patch — must render the same pixels. This is the only thing that keeps the
//! two front-ends from drifting apart.
//!
//! These bring up a real GPU (headless), so they are ignored by default:
//!   cargo test --test pairs -- --ignored

use std::path::{Path, PathBuf};

use vybe::patch::{self, Player};
use vybe::stage::{Render, Stage};
use vybe::*;

fn render(name: &str, stage: Stage) -> Vec<u8> {
    let out = std::env::temp_dir().join(format!("vybe-pair-{name}-{}", std::process::id()));
    let render = Render {
        width: 256,
        height: 256,
        at: vec![1.5],
        out: out.clone(),
        ..Render::default()
    };
    let frames: Vec<PathBuf> = stage.render(&render).expect("renders");
    let bytes = std::fs::read(&frames[0]).expect("a frame was written");
    std::fs::remove_dir_all(out).unwrap();
    bytes
}

fn patch(text: &str) -> Stage {
    Player::new(patch::parse(text).expect("parses"), Path::new(".")).stage()
}

#[test]
#[ignore = "needs a GPU"]
fn feedback_trail() {
    let rs = live(|| {
        circle(0.05)
            .soft(1.0)
            .hue(Hue {
                base: 190.0,
                drift: 25.0,
            })
            .wave(Wave {
                amp: 0.3,
                x: 0.16,
                y: 0.21,
                ..Wave::default()
            })
            .render()
            .feedback(Swirl {
                decay: 0.16,
                angle: 0.9,
                scale: 0.74,
            })
    });
    let vy = patch(
        "comet = circle .05 soft 1  hue 190 drift 25  wave .3 .16hz .21hz\n\
               | feedback decay .16 angle .9 scale .74",
    );
    assert!(
        render("trail-rs", rs) == render("trail-vy", vy),
        "the pair drifted"
    );
}

#[test]
#[ignore = "needs a GPU"]
fn stacked_geometry() {
    let rs = live(|| {
        layers![
            rect(1.0, 1.0).stroke(0.01).gray(0.8),
            line((-0.5, 0.0), (0.5, 0.0)).stroke(0.004).hue(40.0),
            circle(0.2).grid(8, 8).gray(0.35),
        ]
    });
    let vy = patch(
        "all = circle .2 grid 8 8 gray .35\n\
             + line -.5 0 .5 0 stroke .004 hue 40\n\
             + rect 1 1 stroke .01 gray .8",
    );
    assert!(
        render("stack-rs", rs) == render("stack-vy", vy),
        "the pair drifted"
    );
}
