//! vybe-remote · keystone calibration for a face, from a laptop.
//!
//! A vybe sketch like any other (dogfooding): the four corners are tunes, and
//! the interaction is two generic bindings — arrows nudge a tune, the mouse
//! drags a pair of tunes. `Peer` mirrors those tunes to the face over OSC.
//!
//! It maps whatever it connects to: the face says how big its output and its
//! picture are, and the remote draws both in their true proportions — a 16:9
//! screen, a cube's square face, anything.
//!
//!   cargo run -p vybe-remote                       # the face on this machine, port 9001
//!   cargo run -p vybe-remote -- 192.168.0.12:9001  # a face on the network
//!
//! drag a corner · arrows 1 px · shift 10 px · tab next corner
//! S save · R reload · 0 reset to rest · F fullscreen (saved with S)
//! G grid / W white / H gray · space back to the show

use std::net::{SocketAddr, ToSocketAddrs};

use vybe::stage::Warp;
use vybe::*;
use vybe_remote::{Peer, corner_tune, shape_tune};

/// The part of the window the output's frame may fill — with air around it, so
/// a corner can be pulled past the edge.
const BOUNDS: Rect = Rect {
    center: [0.0, 0.0],
    size: [1.20, 0.72],
};

/// A face's size in pixels, as last announced (16:10 until one speaks).
fn size_of(what: &str) -> [f32; 2] {
    [
        tune(&shape_tune(what, 'w'), 1920.0, 1.0..=16384.0),
        tune(&shape_tune(what, 'h'), 1200.0, 1.0..=16384.0),
    ]
}

/// The output's frame on the laptop's screen: its true shape, as big as fits.
fn frame() -> Rect {
    let [w, h] = size_of("output");
    BOUNDS.fit(w / h)
}

fn main() {
    let face: SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9001".into())
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .expect("usage: vybe-remote [host:port]   (default 127.0.0.1:9001)");
    let peer = Peer::connect(face).expect("no UDP socket to speak from");

    let mode = |peer: &Peer, word: &'static str| {
        let peer = peer.clone();
        move |_: &mut input::Io<'_>| peer.send("/mode", Some(word))
    };
    let tell = |peer: &Peer, address: &'static str| {
        let peer = peer.clone();
        move |_: &mut input::Io<'_>| peer.send(address, None)
    };

    live(|| {
        let selected = key_cycle("sel", Key::Tab, 4);
        let alive = tune("face/alive", 0.0, 0.0..=1.0) > 0.5;
        let frame = frame();
        // Where the picture rests, uncalibrated — also where the corners start.
        let rest = Warp::fit(size_of("picture"), size_of("output")).corners;
        let on_screen = |[u, v]: [f32; 2]| frame.map((u, v));
        let corners: Vec<(f32, f32)> = (0..4)
            .map(|i| {
                on_screen([
                    tune(&corner_tune(i, 'x'), rest[i][0], -0.25..=1.25),
                    tune(&corner_tune(i, 'y'), rest[i][1], -0.25..=1.25),
                ])
            })
            .collect();
        let edge = |a: usize, b: usize| line(corners[a], corners[b]);

        // Listed top first, like a layers panel.
        let mut stack = vec![
            // The status light: green while the face answers.
            circle(0.008)
                .at((-0.76, 0.46))
                .hue(if alive { 120.0 } else { 0.0 }),
        ];
        // Corners: the selected one is bigger, and amber.
        stack.extend(corners.iter().enumerate().map(|(i, &at)| {
            let picked = i == selected;
            circle(if picked { 0.016 } else { 0.010 })
                .at(at)
                .hue(if picked { 40.0 } else { 140.0 })
        }));
        // The picture as mapped, and its diagonals (symmetry at a glance).
        stack.extend((0..4).map(|i| edge(i, (i + 1) % 4).stroke(0.003).hue(140.0)));
        stack.extend([edge(0, 2), edge(1, 3)].map(|d| d.stroke(0.0015).gray(0.4)));
        // Behind: the picture at rest (a ghost to come back to), then the
        // output's own frame.
        stack.extend((0..4).map(|i| {
            line(on_screen(rest[i]), on_screen(rest[(i + 1) % 4]))
                .stroke(0.0015)
                .hue(210.0)
                .gray(0.5)
        }));
        stack.push(rect(frame.size[0], frame.size[1]).stroke(0.002).gray(0.35));
        layers(stack)
    })
    .size(1280.0, 800.0)
    .title("vybe remote")
    .with(peer.clone())
    .with(
        arrows_nudge("map/{sel}/*")
            .step_with(|| {
                let [w, h] = size_of("output");
                (1.0 / w, 1.0 / h) // one pixel of the output
            })
            .shift(10.0)
            .y_down(),
    )
    .with(mouse_drag("map/*", 0.04).within_with(frame).selects("sel"))
    .on(Key::Char('s'), tell(&peer, "/keystone/save"))
    .on(Key::Char('r'), tell(&peer, "/keystone/reload"))
    .on(Key::Char('0'), tell(&peer, "/keystone/reset"))
    .on(Key::Char('f'), tell(&peer, "/stage/fullscreen"))
    .on(Key::Char('g'), mode(&peer, "grid"))
    .on(Key::Char('w'), mode(&peer, "white"))
    .on(Key::Char('h'), mode(&peer, "gray"))
    .on(Key::Space, mode(&peer, "show"))
    .show();
}
