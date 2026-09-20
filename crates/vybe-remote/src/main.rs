//! vybe-remote · keystone calibration for a face, from a laptop.
//!
//! A vybe sketch like any other (dogfooding): the four corners are tunes, and
//! the interaction is two generic bindings — arrows nudge a tune, the mouse
//! drags a pair of tunes. `Peer` mirrors those tunes to the face over OSC.
//!
//!   cargo run -p vybe-remote                       # the face on this machine, port 9001
//!   cargo run -p vybe-remote -- 192.168.0.12:9001  # a face on the network
//!
//! drag a corner · arrows 1 px · shift 10 px · tab next corner
//! S save · R reload · G grid / W white / H gray · space back to the show

use std::net::{SocketAddr, ToSocketAddrs};

use vybe::*;
use vybe_remote::{Peer, corner_tune};

/// The output being calibrated, in pixels: arrow keys move by one of these.
const OUTPUT: [u32; 2] = [1920, 1200];

fn main() {
    let face: SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9001".into())
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .expect("usage: vybe-remote [host:port]   (default 127.0.0.1:9001)");
    let peer = Peer::connect(face, OUTPUT).expect("no UDP socket to speak from");

    // The projector's frame on the laptop's screen: 16:10, with air around it
    // so a corner can be pulled past the edge.
    let frame = Rect::new(1.12, 0.70);

    let mode = |peer: &Peer, word: &'static str| {
        let peer = peer.clone();
        move |_: &mut input::Io<'_>| peer.send("/mode", Some(word))
    };
    let tell = |peer: &Peer, address: &'static str| {
        let peer = peer.clone();
        move |_: &mut input::Io<'_>| peer.send(address, None)
    };

    live(move || {
        let selected = key_cycle("sel", Key::Tab, 4);
        let alive = tune("face/alive", 0.0, 0.0..=1.0) > 0.5;
        let rest = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]; // TL TR BR BL
        let corners: Vec<(f32, f32)> = (0..4)
            .map(|i| {
                frame.map((
                    tune(&corner_tune(i, 'x'), rest[i].0, -0.2..=1.2),
                    tune(&corner_tune(i, 'y'), rest[i].1, -0.2..=1.2),
                ))
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
        // The warped quad, and its diagonals (symmetry at a glance).
        stack.extend((0..4).map(|i| edge(i, (i + 1) % 4).stroke(0.003).hue(140.0)));
        stack.extend([edge(0, 2), edge(1, 3)].map(|d| d.stroke(0.0015).gray(0.4)));
        // The projector's own frame, behind everything.
        stack.push(rect(frame.size[0], frame.size[1]).stroke(0.0015).gray(0.3));
        layers(stack)
    })
    .size(1280.0, 800.0)
    .title("vybe remote")
    .with(peer.clone())
    .with(
        arrows_nudge("map/{sel}/*")
            .step(1.0 / OUTPUT[0] as f32, 1.0 / OUTPUT[1] as f32)
            .shift(10.0)
            .y_down(),
    )
    .with(mouse_drag("map/*", 0.04).within(frame).selects("sel"))
    .on(Key::Char('s'), tell(&peer, "/keystone/save"))
    .on(Key::Char('r'), tell(&peer, "/keystone/reload"))
    .on(Key::Char('g'), mode(&peer, "grid"))
    .on(Key::Char('w'), mode(&peer, "white"))
    .on(Key::Char('h'), mode(&peer, "gray"))
    .on(Key::Space, mode(&peer, "show"))
    .show();
}
