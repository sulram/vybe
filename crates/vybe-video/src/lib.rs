//! # vybe-video
//!
//! Video with sound, behind the core's [`Clip`] seam. The decoder is
//! **GStreamer**, because a patch doesn't need frames — it needs a *player*:
//! GStreamer's `playbin` decodes (in hardware where there is any: VideoToolbox
//! on a Mac, V4L2 on a Pi), keeps picture and sound in sync on its own clock,
//! and sends the sound to the speakers. All this crate does is take the picture
//! off the end of that pipeline (an `appsink`) and hand it to the core as RGBA.
//!
//! Two paces ([`Pace`]):
//! - **Live** — the pipeline runs on its clock, in sync, with sound; the engine
//!   takes whatever frame is newest. Late frames are dropped, never queued: the
//!   picture is always *now*.
//! - **Offline** — no clock, no sound: frames are pulled one after another and
//!   the one belonging at exactly the asked time is returned, so a headless
//!   render is the same pixels every run.
//!
//! Needs GStreamer installed (`brew install gstreamer`; on Debian/Raspberry Pi
//! OS: `gstreamer1.0-plugins-{base,good,bad} gstreamer1.0-libav` and the `-dev`
//! packages to build).

use std::path::Path;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSink;
use vybe::clip::{Clip, Decoder, Frame, Pace};

/// Opens video files with GStreamer.
pub struct GStreamer;

impl GStreamer {
    /// Brings GStreamer up. The error says how to install it.
    pub fn new() -> Result<Self, String> {
        gst::init().map(|()| Self).map_err(|e| {
            format!(
                "GStreamer did not start: {e}\n    help: install it —  macOS: brew install gstreamer   \
                 Debian/Pi: apt install gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-libav"
            )
        })
    }
}

impl Decoder for GStreamer {
    fn open(&self, path: &Path, looping: bool, pace: Pace) -> Result<Box<dyn Clip>, String> {
        Video::open(path, looping, pace).map(|video| Box::new(video) as Box<dyn Clip>)
    }
}

struct Video {
    pipeline: gst::Element,
    sink: AppSink,
    looping: bool,
    pace: Pace,
    done: bool,
    /// Offline: the frame pulled past the asked time, waiting for its turn.
    ahead: Option<(f32, Frame)>,
    /// Offline: how far into the file a loop has already carried us.
    lap: f32,
    /// Offline: has any frame been shown since the (re)start?
    shown: bool,
}

impl Video {
    fn open(path: &Path, looping: bool, pace: Pace) -> Result<Self, String> {
        let shown = path.display();
        let path = path.canonicalize().map_err(|e| format!("{shown}: {e}"))?;
        let uri = gst::glib::filename_to_uri(&path, None).map_err(|e| format!("{shown}: {e}"))?;

        // The picture's exit: RGBA, one frame held at most. Live, it follows the
        // clock and drops what arrives late; offline, it holds the pipeline back
        // until each frame is taken.
        let live = pace == Pace::Live;
        let sink = AppSink::builder()
            .caps(
                &gst::Caps::builder("video/x-raw")
                    .field("format", "RGBA")
                    .build(),
            )
            .max_buffers(1)
            .drop(live)
            .sync(live)
            .build();

        let missing = |what: &str| {
            format!(
                "{shown}: GStreamer has no `{what}` element\n    help: reinstall it with its plugins (macOS: brew reinstall gstreamer)"
            )
        };
        let pipeline = gst::ElementFactory::make("playbin")
            .property("uri", uri.as_str())
            .property("video-sink", &sink)
            .build()
            .map_err(|_| missing("playbin"))?;
        if !live {
            // A render makes no sound, and must not wait for a sound card.
            let silence = gst::ElementFactory::make("fakesink")
                .property("sync", false)
                .build()
                .map_err(|_| missing("fakesink"))?;
            pipeline.set_property("audio-sink", &silence);
        }

        let video = Self {
            pipeline,
            sink,
            looping,
            pace,
            done: false,
            ahead: None,
            lap: 0.0,
            shown: false,
        };
        video
            .pipeline
            .set_state(gst::State::Playing)
            .map_err(|_| video.why(&format!("{shown}: GStreamer could not play it")))?;
        Ok(video)
    }

    /// The pipeline's own account of what went wrong, if it has one.
    fn why(&self, fallback: &str) -> String {
        let bus = self.pipeline.bus();
        let error = bus.and_then(|bus| bus.pop_filtered(&[gst::MessageType::Error]));
        match error.as_ref().map(|m| m.view()) {
            Some(gst::MessageView::Error(e)) => format!(
                "{fallback}: {}\n    help: a codec GStreamer can't decode? re-encode:  ffmpeg -i in.mov -c:v libx264 -pix_fmt yuv420p -c:a aac out.mp4",
                e.error()
            ),
            _ => fallback.to_owned(),
        }
    }

    fn rewind(&mut self) {
        let flags = gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT;
        let _ = self.pipeline.seek_simple(flags, gst::ClockTime::ZERO);
    }

    /// Reads the pipeline's messages: the end of the file, above all.
    fn listen(&mut self) {
        let Some(bus) = self.pipeline.bus() else {
            return;
        };
        while let Some(message) = bus.pop() {
            match message.view() {
                gst::MessageView::Eos(_) if self.looping => self.rewind(),
                gst::MessageView::Eos(_) => self.done = true,
                gst::MessageView::Error(e) => {
                    eprintln!("vybe: video: {}", e.error());
                    self.done = true;
                }
                _ => {}
            }
        }
    }

    /// Offline: the next frame in the file, with its time — crossing the end of
    /// a looping file as if it were endless.
    fn next(&mut self) -> Option<(f32, Frame)> {
        loop {
            match self.sink.pull_sample() {
                Ok(sample) => {
                    let (at, frame) = picture(&sample)?;
                    return Some((self.lap + at, frame));
                }
                Err(_) if self.looping && !self.done => {
                    // The end: go around, and remember how long a lap was.
                    let lap = self
                        .pipeline
                        .query_duration::<gst::ClockTime>()
                        .map_or(0.0, |d| d.seconds_f32());
                    if lap <= 0.0 {
                        self.done = true;
                        return None;
                    }
                    self.lap += lap;
                    self.rewind();
                }
                Err(_) => {
                    self.done = true;
                    return None;
                }
            }
        }
    }
}

impl Clip for Video {
    fn restart(&mut self) {
        self.done = false;
        self.ahead = None;
        self.lap = 0.0;
        self.shown = false;
        self.rewind();
        let _ = self.pipeline.set_state(gst::State::Playing);
    }

    fn set_paused(&mut self, paused: bool) {
        // Offline nothing runs on a clock: a frame is only decoded when asked for.
        if self.pace == Pace::Live {
            let state = if paused {
                gst::State::Paused
            } else {
                gst::State::Playing
            };
            let _ = self.pipeline.set_state(state);
        }
    }

    fn set_volume(&mut self, volume: f32) {
        self.pipeline
            .set_property("volume", f64::from(volume.clamp(0.0, 1.0)));
    }

    fn done(&mut self) -> bool {
        if self.pace == Pace::Live {
            self.listen();
        }
        self.done
    }

    fn frame(&mut self, time: f32) -> Option<Frame> {
        match self.pace {
            Pace::Live => {
                self.listen();
                let sample = self.sink.try_pull_sample(gst::ClockTime::ZERO)?;
                picture(&sample).map(|(_, frame)| frame)
            }
            // The frame on screen at `time` is the last one whose own time has
            // come: pull forward until one is still in the future, and keep it
            // for later.
            Pace::Offline => {
                let mut showing = None;
                loop {
                    let (at, frame) = match self.ahead.take() {
                        Some(ahead) => ahead,
                        None => match self.next() {
                            Some(next) => next,
                            None => return showing,
                        },
                    };
                    // (A file's first frame is rarely stamped exactly zero, yet
                    // it is what shows from the start.)
                    if at > time && self.shown {
                        self.ahead = Some((at, frame));
                        return showing;
                    }
                    self.shown = true;
                    showing = Some(frame);
                }
            }
        }
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

/// A sample's pixels and its time in the file. Rows may be padded to a stride;
/// the core wants them tight.
fn picture(sample: &gst::Sample) -> Option<(f32, Frame)> {
    let structure = sample.caps()?.structure(0)?;
    let width = u32::try_from(structure.get::<i32>("width").ok()?).ok()?;
    let height = u32::try_from(structure.get::<i32>("height").ok()?).ok()?;
    let buffer = sample.buffer()?;
    let at = buffer.pts().map_or(0.0, |t| t.seconds_f32());
    let map = buffer.map_readable().ok()?;
    let (row, rows) = (4 * width as usize, height as usize);
    if rows == 0 || map.len() < row * rows {
        return None;
    }
    let stride = map.len() / rows;
    let rgba = if stride == row {
        map[..row * rows].to_vec()
    } else {
        map.chunks_exact(stride)
            .take(rows)
            .flat_map(|line| &line[..row])
            .copied()
            .collect()
    };
    Some((
        at,
        Frame {
            width,
            height,
            rgba,
        },
    ))
}
