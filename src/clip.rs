//! CLIP — the seam moving pictures enter through. std-only.
//!
//! A [`Clip`] is something that *plays*: it has a timeline, it makes frames (and
//! whatever sound it makes, it makes on its own), and it can be restarted,
//! paused and turned down. A [`Decoder`] opens one from a file. The core knows
//! these two traits and nothing else — what actually decodes (GStreamer today)
//! is library glue in a crate of its own, like every integration.
//!
//! The division of labour: the **player** owns a clip's *behaviour* (when it
//! restarts, how loud it is, whether it is `done`); the **GPU** owns one texture
//! per clip and uploads a frame when there is a new one; the clip owns decoding
//! and — crucially — **its own audio/video sync**. The engine never paces sound.

use std::path::Path;
use std::sync::{Arc, Mutex};

/// One decoded picture: tightly packed RGBA8, straight alpha, sRGB, rows top
/// to bottom.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// How a clip is being asked to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pace {
    /// On its own clock, with sound — a performance. [`Clip::frame`] returns the
    /// newest frame; the time passed to it is ignored.
    Live,
    /// As fast as asked, silent — a headless render. [`Clip::frame`] returns the
    /// frame that belongs at exactly the time asked, so the same command
    /// renders the same pixels.
    Offline,
}

/// Something that plays. Every method is total (Principle 3): a clip that has
/// lost its file mid-show goes quiet and still, it does not panic.
pub trait Clip {
    /// Plays from the start (and is no longer [`done`](Clip::done)).
    fn restart(&mut self);
    fn set_paused(&mut self, paused: bool);
    /// `0.0` silent … `1.0` as recorded.
    fn set_volume(&mut self, volume: f32);
    /// Has it played to its end? Never true for a looping clip.
    fn done(&mut self) -> bool;
    /// A frame to show, if there is one newer than the last one returned. `time`
    /// is seconds since the clip (re)started — it matters only [`Pace::Offline`].
    fn frame(&mut self, time: f32) -> Option<Frame>;
}

/// Opens clips. One per backend; the CLI hands the player the one it links.
pub trait Decoder {
    /// IO, so a `Result` — and the error says what to do next.
    fn open(&self, path: &Path, looping: bool, pace: Pace) -> Result<Box<dyn Clip>, String>;
}

/// Where a clip's newest frame waits for the GPU. The player puts, the GPU
/// takes; a frame nobody took is simply replaced (the picture is always *now*).
/// Shared by `Arc`, so a re-described recipe points at the same slot and the
/// GPU keeps the clip's texture — key = identity, for pixels that stream.
#[derive(Default)]
pub(crate) struct Slot(Mutex<Option<Frame>>);

impl Slot {
    pub(crate) fn new() -> Arc<Self> {
        Arc::default()
    }

    pub(crate) fn put(&self, frame: Frame) {
        if let Ok(mut held) = self.0.lock() {
            *held = Some(frame);
        }
    }

    pub(crate) fn take(&self) -> Option<Frame> {
        self.0.lock().ok()?.take()
    }
}
