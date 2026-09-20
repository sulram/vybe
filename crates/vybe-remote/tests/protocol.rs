//! The two windows, without the windows: a `Peer` (the laptop) and a `Face`
//! (the player) talking over real UDP on loopback, each driven frame by frame
//! the way a stage drives its bindings.

use std::net::SocketAddr;
use std::time::Duration;

use vybe::input::{Binding, Inputs, Io, Value};
use vybe::stage::Warp;
use vybe::tune;
use vybe_remote::{Face, Peer, corner_tune, shape_tune};

/// One side of the conversation: a binding plus the state a stage would own.
struct Side<B: Binding> {
    binding: B,
    inputs: Inputs,
    warp: Warp,
    /// The last window mode the binding asked its stage for.
    fullscreen: Option<bool>,
}

impl<B: Binding> Side<B> {
    fn new(binding: B) -> Self {
        Self {
            binding,
            inputs: Inputs::default(),
            warp: Warp::default(),
            fullscreen: None,
        }
    }

    fn frame(&mut self) -> Option<String> {
        let mut io = Io {
            inputs: &mut self.inputs,
            warp: &mut self.warp,
            time: 0.0,
            dt: 1.0 / 60.0,
            title: None,
            fullscreen: None,
        };
        self.binding.frame(&mut io);
        self.fullscreen = io.fullscreen.or(self.fullscreen);
        io.title
    }
}

/// Runs both sides for a few frames, giving UDP a moment to cross.
fn converse(face: &mut Side<Face>, peer: &mut Side<Peer>) -> String {
    let mut title = String::new();
    for _ in 0..6 {
        title = peer.frame().unwrap_or(title);
        std::thread::sleep(Duration::from_millis(5));
        face.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    title
}

#[test]
fn a_dragged_corner_reaches_the_wall_and_a_save_survives_a_restart() {
    let dir = std::env::temp_dir().join(format!("vybe-remote-{}", std::process::id()));
    let file = dir.join("config").join("keystone.json");
    let port = 19_001;
    let address: SocketAddr = ([127, 0, 0, 1], port).into();

    // A square face on a 16:10 output: uncalibrated, it rests centered.
    let picture = [1200, 1200];
    let uncalibrated = keystone::Keystone {
        output: [1920, 1200],
        corners: Warp::fit([1200.0, 1200.0], [1920.0, 1200.0]).corners,
        ..keystone::Keystone::default()
    };
    let listen = |file| Face::listen(port, Some(file), uncalibrated.clone(), picture).unwrap();
    let mut face = Side::new(listen(file.clone()));
    let handle = Peer::connect(address).unwrap();
    let mut peer = Side::new(handle.clone());

    // The remote sketch picks its corner tunes (here: at rest).
    let rest = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    for (i, [x, y]) in rest.into_iter().enumerate() {
        tune(&corner_tune(i, 'x'), x, -0.2..=1.2);
        tune(&corner_tune(i, 'y'), y, -0.2..=1.2);
    }
    tune("face/alive", 0.0, 0.0..=1.0);
    for what in ["output", "picture"] {
        tune(&shape_tune(what, 'w'), 16.0, 1.0..=16384.0);
        tune(&shape_tune(what, 'h'), 10.0, 1.0..=16384.0);
    }

    // Heartbeat out, state back: connected.
    let title = converse(&mut face, &mut peer);
    assert!(title.contains("connected"), "{title}");
    assert_eq!(tune::get("face/alive"), Some(1.0));
    // …and the remote now knows what it is mapping: a square, on 16:10.
    assert_eq!(tune::get(&shape_tune("output", 'w')), Some(1920.0));
    assert_eq!(tune::get(&shape_tune("picture", 'h')), Some(1200.0));

    // A hand drags the bottom-right corner; the wall follows, in RAM only.
    tune::set(&corner_tune(2, 'x'), 0.9);
    tune::set(&corner_tune(2, 'y'), 0.95);
    let title = converse(&mut face, &mut peer);
    assert_eq!(face.warp.corners[2], [0.9, 0.95]);
    assert!(title.contains('•'), "unsaved edits show: {title}");
    assert!(!file.exists());

    // S: the face writes its file and says so.
    handle.send("/keystone/save", None);
    let title = converse(&mut face, &mut peer);
    assert!(title.contains("saved") && !title.contains('•'), "{title}");
    assert!(file.exists());

    // F: the face takes the whole screen — live at once, unsaved until S.
    assert_eq!(face.fullscreen, None);
    handle.send("/stage/fullscreen", None);
    let title = converse(&mut face, &mut peer);
    assert_eq!(face.fullscreen, Some(true));
    assert!(title.contains('•'), "{title}");
    handle.send("/keystone/save", None);
    converse(&mut face, &mut peer);

    // Anything that isn't protocol is the patch's input.
    handle.send("/mode", Some("grid"));
    converse(&mut face, &mut peer);
    assert_eq!(face.inputs.get("/mode"), Some(&Value::Sym("grid".into())));

    // The player restarts (a power cut): the calibration is still there.
    drop(face);
    let mut face = Side::new(listen(file));
    face.frame();
    assert_eq!(face.warp.corners[2], [0.9, 0.95]);
    assert_eq!(face.fullscreen, Some(true)); // …and it opens the way it was saved

    // Reset: back to rest on the wall and in the remote's hands — RAM only, so
    // the saved calibration is still one `R` away.
    handle.send("/keystone/reset", None);
    converse(&mut face, &mut peer);
    assert_eq!(face.warp.corners, uncalibrated.corners);
    assert_eq!(tune::get(&corner_tune(2, 'x')), Some(0.8125)); // the square's edge, not the output's
    handle.send("/keystone/reload", None);
    converse(&mut face, &mut peer);
    assert_eq!(face.warp.corners[2], [0.9, 0.95]);

    std::fs::remove_dir_all(dir).unwrap();
}
