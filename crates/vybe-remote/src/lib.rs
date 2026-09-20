//! # vybe-remote
//!
//! The remote protocol, both ends of it, as [`Binding`]s — library glue behind
//! the engine's one input seam. OSC over UDP, TouchDesigner-compatible (so TD
//! is an emergency remote):
//!
//! | direction     | address             | args       | effect on the face                          |
//! |---------------|---------------------|------------|---------------------------------------------|
//! | remote → face | `/keystone/get`     |            | reply `/keystone/state`; also the heartbeat |
//! | face → remote | `/keystone/state`   | 8 f + f    | four corners (TL TR BR BL) + feather        |
//! | remote → face | `/keystone/corner`  | `i x y`    | live, RAM only                              |
//! | remote → face | `/keystone/feather` | `f`        | live, RAM only                              |
//! | remote → face | `/keystone/save`    |            | atomic write + `.bak`; reply `/keystone/saved ok` |
//! | remote → face | `/keystone/reload`  |            | drop RAM state, re-read the file            |
//! | remote → face | `/param/<name>`     | `v`        | turns a `tune`                              |
//! | anyone → face | anything else       | `v`        | lands in the patch's inputs (`/hands 1`, `/mode grid`) |
//!
//! [`Face`] is the player's end: `remote 9001` in a patch's `out` line is all
//! it takes. [`Peer`] is the laptop's end: it mirrors the `map/<i>/x|y` tunes
//! onto the face's corners. The corners are *tunes* and the test patterns are
//! *scenes*; nothing here is a calibration feature the engine had to learn.

use std::cell::RefCell;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;

use keystone::Keystone;
use vybe::input::{Binding, Io, Value};
use vybe::stage::Warp;
use vybe::tune;
use vybe_io::{Arg, Message, Osc};

/// The corner tunes the remote sketch picks: `map/0/x` … `map/3/y`.
pub fn corner_tune(index: usize, axis: char) -> String {
    format!("map/{index}/{axis}")
}

fn state_message(keystone: &Keystone) -> Message {
    let corners = keystone.corners.iter().flatten().map(|&v| Arg::Float(v));
    Message::new(
        "/keystone/state",
        corners.chain([Arg::Float(keystone.feather)]),
    )
}

// ---------------------------------------------------------------------------
// The face's end
// ---------------------------------------------------------------------------

/// The player's end of the protocol: listens on a port, keeps the keystone in
/// RAM, writes it only when told to, and hands every other message to the
/// patch as an input.
pub struct Face {
    osc: Osc,
    keystone: Keystone,
    /// Where `/keystone/save` writes. `None`: calibration lives in RAM only.
    path: Option<PathBuf>,
}

impl Face {
    /// Listens on `port`; loads `path` if it exists (uncalibrated otherwise).
    /// A keystone file that exists but is broken is an error — better to
    /// refuse to start than to project uncalibrated over a damaged file.
    pub fn listen(port: u16, path: Option<PathBuf>) -> Result<Self, String> {
        let osc = Osc::bind(port).map_err(|e| {
            format!("can't listen for OSC on port {port}: {e}\n    help: another player may hold it — pick another with `remote <port>`")
        })?;
        let keystone = match &path {
            Some(path) => Keystone::load_or_default(path).map_err(|e| e.to_string())?,
            None => Keystone::default(),
        };
        Ok(Self {
            osc,
            keystone,
            path,
        })
    }

    fn handle(&mut self, message: &Message, from: SocketAddr, io: &mut Io<'_>) {
        let reply = |osc: &Osc, message: Message| {
            // A reply that can't be sent is the remote's heartbeat to notice.
            let _ = osc.send(from, &message);
        };
        match message.address.as_str() {
            "/keystone/get" => reply(&self.osc, state_message(&self.keystone)),
            "/keystone/corner" => {
                let corner = message.number(0).map(|i| i as usize);
                if let (Some(i @ 0..=3), Some(x), Some(y)) =
                    (corner, message.number(1), message.number(2))
                {
                    self.keystone.corners[i] = [x, y];
                }
            }
            "/keystone/feather" => {
                if let Some(feather) = message.number(0) {
                    self.keystone.feather = feather.clamp(0.0, 0.5);
                }
            }
            "/keystone/save" => {
                let result = match &self.path {
                    Some(path) => self.keystone.save(path).map_err(|e| e.to_string()),
                    None => {
                        Err("this patch names no keystone file (`out … keystone <file>`)".into())
                    }
                };
                let status = result.err().unwrap_or_else(|| "ok".into());
                reply(
                    &self.osc,
                    Message::new("/keystone/saved", [Arg::Str(status)]),
                );
            }
            "/keystone/reload" => {
                if let Some(path) = &self.path {
                    match Keystone::load_or_default(path) {
                        Ok(keystone) => self.keystone = keystone,
                        Err(e) => eprintln!("vybe: {e}"),
                    }
                }
                reply(&self.osc, state_message(&self.keystone));
            }
            address => {
                let Some(arg) = message.args.first() else {
                    return;
                };
                match (address.strip_prefix("/param/"), arg.number()) {
                    (Some(name), Some(value)) => {
                        tune::set(name, value);
                    }
                    _ => {
                        let value = match arg {
                            Arg::Str(symbol) => Value::Sym(symbol.clone()),
                            other => Value::Num(other.number().unwrap_or(0.0)),
                        };
                        io.inputs.set(address, value);
                    }
                }
            }
        }
    }
}

impl Binding for Face {
    fn frame(&mut self, io: &mut Io<'_>) {
        while let Some((message, from)) = self.osc.recv() {
            self.handle(&message, from, io);
        }
        *io.warp = Warp {
            corners: self.keystone.corners,
            feather: self.keystone.feather,
        };
    }
}

// ---------------------------------------------------------------------------
// The remote's end
// ---------------------------------------------------------------------------

/// Seconds between heartbeats, and how long a face may stay silent before it
/// counts as gone.
const HEARTBEAT: f32 = 1.0;
const SILENCE: f32 = 2.5;

/// The laptop's end: mirrors the `map/*` tunes onto a face's corners. A cheap
/// handle — clone one onto the stage (`.with(peer.clone())`) and keep the
/// others for the keys that `send`.
#[derive(Clone)]
pub struct Peer(Rc<RefCell<PeerState>>);

struct PeerState {
    osc: Osc,
    face: SocketAddr,
    /// The output's size in pixels — only to print a corner the way a
    /// projector counts it.
    output: [u32; 2],
    /// What the face is known to hold; a tune that differs gets sent.
    sent: [[f32; 2]; 4],
    since_beat: f32,
    since_heard: f32,
    alive: bool,
    /// Take the face's next `/keystone/state` as the truth: on (re)connect and
    /// after a reload. Otherwise the remote is the writer — adopting every
    /// heartbeat would fight the hand that is dragging.
    adopt: bool,
    /// Turned since the last save.
    dirty: bool,
    note: Option<String>,
}

impl Peer {
    pub fn connect(face: SocketAddr, output: [u32; 2]) -> std::io::Result<Self> {
        Ok(Self(Rc::new(RefCell::new(PeerState {
            osc: Osc::open()?,
            face,
            output,
            sent: Keystone::default().corners,
            since_beat: HEARTBEAT, // beat on the first frame
            since_heard: SILENCE,
            alive: false,
            adopt: true,
            dirty: false,
            note: None,
        }))))
    }

    /// Sends `/address [word]` to the face.
    pub fn send(&self, address: &str, word: Option<&str>) {
        let mut state = self.0.borrow_mut();
        // The face answers a reload with the state it re-read.
        state.adopt |= address == "/keystone/reload";
        let _ = state
            .osc
            .send(state.face, &Message::new(address, word.map(Arg::from)));
    }
}

impl Binding for Peer {
    fn frame(&mut self, io: &mut Io<'_>) {
        let mut state = self.0.borrow_mut();
        let state = &mut *state;

        state.since_beat += io.dt;
        state.since_heard += io.dt;
        if state.since_beat >= HEARTBEAT {
            state.since_beat = 0.0;
            let _ = state
                .osc
                .send(state.face, &Message::new("/keystone/get", []));
        }

        while let Some((message, _)) = state.osc.recv() {
            state.since_heard = 0.0;
            match message.address.as_str() {
                "/keystone/state" if state.adopt => {
                    for i in 0..4 {
                        let (x, y) = (corner_tune(i, 'x'), corner_tune(i, 'y'));
                        if let (Some(fx), Some(fy)) =
                            (message.number(2 * i), message.number(2 * i + 1))
                        {
                            tune::set(&x, fx);
                            tune::set(&y, fy);
                        }
                        // What the tunes now hold (a range may have clamped it)
                        // is what counts as sent — adopting is not an edit.
                        if let (Some(tx), Some(ty)) = (tune::get(&x), tune::get(&y)) {
                            state.sent[i] = [tx, ty];
                        }
                    }
                    state.adopt = false;
                    state.dirty = false;
                }
                "/keystone/saved" => {
                    let status = message.args.first().and_then(Arg::text).unwrap_or("?");
                    state.dirty = status != "ok";
                    state.note = Some(match status {
                        "ok" => "saved".to_owned(),
                        failed => format!("save failed: {failed}"),
                    });
                }
                _ => {}
            }
        }
        let alive = state.since_heard < SILENCE;
        if alive != state.alive {
            state.alive = alive;
            state.adopt |= !alive; // whoever answers next may be another face
            state.note = None;
            // A tune, so the sketch redraws its status light by itself.
            tune::set("face/alive", f32::from(u8::from(alive)));
        }

        // Mirror: whatever the hand turned since last frame goes to the face.
        for i in 0..4 {
            let (Some(x), Some(y)) = (
                tune::get(&corner_tune(i, 'x')),
                tune::get(&corner_tune(i, 'y')),
            ) else {
                continue;
            };
            if state.alive && state.sent[i] != [x, y] {
                state.sent[i] = [x, y];
                state.dirty = true;
                state.note = None;
                let corner =
                    Message::new("/keystone/corner", [(i as i32).into(), x.into(), y.into()]);
                let _ = state.osc.send(state.face, &corner);
            }
        }

        // The status line lives in the title until the engine can draw text.
        let selected = tune::get("sel").unwrap_or(0.0) as usize % 4;
        let [x, y] = state.sent[selected];
        io.title = Some(format!(
            "vybe remote — {} · {} · {} {:.0}, {:.0} px{}{}",
            state.face,
            if state.alive { "connected" } else { "no reply" },
            ["TL", "TR", "BR", "BL"][selected],
            x * state.output[0] as f32,
            y * state.output[1] as f32,
            if state.dirty { " •" } else { "" },
            state
                .note
                .as_ref()
                .map(|n| format!(" · {n}"))
                .unwrap_or_default(),
        ));
    }
}
