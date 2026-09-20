//! THE STAGE — where the picture meets the world: the clock it runs on, the
//! warp that squares it on a wall, and the headless output that needs neither
//! window nor wall. (The window itself is `shell`; a KMS output will sit
//! beside it.) std-only: the GPU side of the warp is one pass in `gpu`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::gpu::Headless;
use crate::input::{self, Binding, Inputs, Io, Key};
use crate::media;
use crate::shell::{Show, Source};

// ---------------------------------------------------------------------------
// Stage — a picture about to run, and everything listening to it
// ---------------------------------------------------------------------------

/// A picture about to run: what describes it (a live sketch, a patch player),
/// plus the [`Binding`]s listening to the window and speaking into it. Built by
/// [`live()`](crate::live) and by [`Player::stage`](crate::patch::Player::stage);
/// ends, like every chain, on `.show()`.
#[must_use = "a stage does nothing until `.show()` (or `.render(..)`)"]
pub struct Stage {
    show: Show,
}

impl Stage {
    pub(crate) fn new(source: Source) -> Self {
        Self {
            show: Show::new(source),
        }
    }

    /// Hangs a binding on the stage: the generic ones
    /// ([`arrows_nudge`](crate::arrows_nudge), [`mouse_drag`](crate::mouse_drag)),
    /// or an integration from another crate (OSC, a sensor).
    pub fn with(mut self, binding: impl Binding + 'static) -> Self {
        self.show.bindings.push(Box::new(binding));
        self
    }

    /// Runs `action` when `key` goes down.
    pub fn on(self, key: Key, action: impl FnMut(&mut Io<'_>) + 'static) -> Self {
        self.with(input::on(key, action))
    }

    /// Gives the picture a size of its own, in pixels, apart from its output's —
    /// a square face on a 16:10 projector. It renders at this size and the
    /// [`Warp`] lands it in the output, so a keystone's four corners are the
    /// *picture's* corners. At rest it sits contained and centered.
    pub fn picture(mut self, width: u32, height: u32) -> Self {
        self.show.picture = Some([width.max(1), height.max(1)]);
        self
    }

    /// Where the picture rests, uncalibrated, in an output of this shape.
    pub fn rest(&self, output: [f32; 2]) -> Warp {
        self.show.rest(output)
    }

    /// The window's size, in logical pixels (default 800 × 800).
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.show.size = [width, height];
        self
    }

    /// Opens on the whole screen, borderless (Escape leaves it).
    pub fn fullscreen(mut self, whole: bool) -> Self {
        self.show.fullscreen = whole;
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.show.title = Some(title.to_owned());
        self
    }

    /// The terminal link: opens the window, brings up the GPU, and runs the
    /// render loop.
    pub fn show(self) {
        self.show.run();
    }

    /// The other terminal link: no window — a fixed clock, and PNGs out.
    /// Returns the files written.
    pub fn render(self, render: &Render) -> Result<Vec<PathBuf>, String> {
        render.run(self.show)
    }
}

// ---------------------------------------------------------------------------
// Warp — the keystone
// ---------------------------------------------------------------------------

/// Where the picture's four corners land on the output — a 4-corner keystone.
/// Exact for a flat face: the homography below is the whole correction, no
/// mesh needed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warp {
    /// TL, TR, BR, BL — each a fraction of the output, x rightward, **y
    /// downward** (the projector's own coordinates: this is the boundary, where
    /// pixels live). A corner may sit outside `0..1`.
    pub corners: [[f32; 2]; 4],
    /// Soft edge, as a fraction of the picture — lets neighbouring faces
    /// overlap without a seam. `0.0` = hard edge.
    pub feather: f32,
}

impl Default for Warp {
    /// At rest the picture is the whole output.
    fn default() -> Self {
        Self {
            corners: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            feather: 0.0,
        }
    }
}

impl Warp {
    /// A picture of one shape at rest in an output of another: contained,
    /// centered, undistorted — where a face sits before it is calibrated.
    /// (Sizes in any unit; only the two shapes matter.)
    pub fn fit(picture: [f32; 2], output: [f32; 2]) -> Self {
        let scale = (output[0] / picture[0]).min(output[1] / picture[1]);
        let half = [
            picture[0] * scale / output[0] * 0.5,
            picture[1] * scale / output[1] * 0.5,
        ];
        let (l, r, t, b) = (0.5 - half[0], 0.5 + half[0], 0.5 - half[1], 0.5 + half[1]);
        Self {
            corners: [[l, t], [r, t], [r, b], [l, b]],
            feather: 0.0,
        }
    }

    /// The homography taking the picture's uv (texel space: `(0,0)` top-left)
    /// to homogeneous clip space, as rows. Emitting its `w` from the vertex
    /// stage is what makes the GPU interpolate perspective-correctly.
    /// (Heckbert's unit-square-to-quad mapping.)
    pub(crate) fn rows(&self) -> [[f32; 4]; 3] {
        // Output fractions (y-down) -> clip space (y-up, -1..+1).
        let [p0, p1, p2, p3] = self.corners.map(|[x, y]| [2.0 * x - 1.0, 1.0 - 2.0 * y]);
        let (sx, sy) = (p0[0] - p1[0] + p2[0] - p3[0], p0[1] - p1[1] + p2[1] - p3[1]);
        let (dx1, dy1) = (p1[0] - p2[0], p1[1] - p2[1]);
        let (dx2, dy2) = (p3[0] - p2[0], p3[1] - p2[1]);
        let det = dx1 * dy2 - dy1 * dx2;
        // A parallelogram (or a degenerate quad) has no perspective term.
        let (g, h) = if det.abs() < 1e-9 {
            (0.0, 0.0)
        } else {
            ((sx * dy2 - sy * dx2) / det, (dx1 * sy - dy1 * sx) / det)
        };
        [
            [
                p1[0] - p0[0] + g * p1[0],
                p3[0] - p0[0] + h * p3[0],
                p0[0],
                0.0,
            ],
            [
                p1[1] - p0[1] + g * p1[1],
                p3[1] - p0[1] + h * p3[1],
                p0[1],
                0.0,
            ],
            [g, h, 1.0, 0.0],
        ]
    }
}

// ---------------------------------------------------------------------------
// Clock — the engine's sense of time
// ---------------------------------------------------------------------------

/// The one place the engine reads time. **Wall**: real seconds, so motion is
/// framerate-independent (`Wave` in cycles/second means what it says on any
/// monitor). **Fixed**: every frame is exactly one step long — deterministic,
/// faster or slower than real time as the GPU allows; what headless rendering
/// runs on. (Also the seam to swap for WASM, where `Instant::now()` panics.)
pub(crate) enum Clock {
    Wall { start: Instant, last: Instant },
    Fixed { step: f64, frame: u64 },
}

impl Clock {
    pub(crate) fn wall() -> Self {
        let now = Instant::now();
        Clock::Wall {
            start: now,
            last: now,
        }
    }

    pub(crate) fn fixed(fps: f32) -> Self {
        Clock::Fixed {
            step: 1.0 / f64::from(fps.max(1.0)),
            frame: 0,
        }
    }

    /// (seconds since start, seconds since last frame). On the wall clock `dt`
    /// is clamped so a stall or the first frame can't jolt the animation with
    /// a huge step. The first fixed frame is at `t = 0`.
    pub(crate) fn tick(&mut self) -> (f32, f32) {
        match self {
            Clock::Wall { start, last } => {
                let now = Instant::now();
                let time = now.duration_since(*start).as_secs_f32();
                let dt = now.duration_since(*last).as_secs_f32().min(0.1);
                *last = now;
                (time, dt)
            }
            Clock::Fixed { step, frame } => {
                let time = *step * *frame as f64;
                *frame += 1;
                (time as f32, *step as f32)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Headless — render to PNG, no window
// ---------------------------------------------------------------------------

/// What to render without a window, and where to put it: a fixed clock, a
/// fixed size, and the moments worth keeping. Same input, same pixels.
#[derive(Clone, Debug)]
pub struct Render {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    /// The moments to save, in seconds. Every frame up to the last one is
    /// rendered regardless — a feedback loop is the sum of its past.
    pub at: Vec<f32>,
    /// Save *every* frame from `0` up to this many seconds (a PNG sequence).
    pub seq: Option<f32>,
    /// Keep transparency instead of landing on black.
    pub alpha: bool,
    pub out: PathBuf,
}

impl Default for Render {
    fn default() -> Self {
        Self {
            width: 800,
            height: 800,
            fps: 60.0,
            at: vec![0.0],
            seq: None,
            alpha: false,
            out: PathBuf::from("frames"),
        }
    }
}

impl Render {
    /// Reads the `VYBE_RENDER` spec any sketch obeys, so every example renders
    /// headless with no code of its own:
    /// `VYBE_RENDER="at=0,2,4.5 size=800x800 fps=60 out=frames" cargo run --example dots`
    /// (also `seq=3` for every frame of the first 3 s, and `alpha`).
    pub(crate) fn from_env() -> Option<Self> {
        let spec = std::env::var("VYBE_RENDER").ok()?;
        let mut render = Self::default();
        for word in spec.split_whitespace() {
            let (key, value) = word.split_once('=').unwrap_or((word, ""));
            match key {
                "at" => render.at = value.split(',').filter_map(|t| t.parse().ok()).collect(),
                "seq" => render.seq = value.parse().ok(),
                "fps" => render.fps = value.parse().unwrap_or(render.fps),
                "alpha" => render.alpha = true,
                "out" => render.out = PathBuf::from(value),
                "size" => {
                    if let Some((w, h)) = value.split_once('x') {
                        render.width = w.parse().unwrap_or(render.width);
                        render.height = h.parse().unwrap_or(render.height);
                    }
                }
                _ => eprintln!("vybe: VYBE_RENDER: unknown word `{word}`"),
            }
        }
        Some(render)
    }

    /// Runs `show` on a fixed clock with no window, saving the asked frames as
    /// PNGs. Returns the files written, in order.
    pub(crate) fn run(&self, mut show: Show) -> Result<Vec<PathBuf>, String> {
        std::fs::create_dir_all(&self.out).map_err(|e| format!("{}: {e}", self.out.display()))?;
        let step = 1.0 / self.fps.max(1.0);
        let frame_of = |t: f32| (t.max(0.0) / step).round() as u64;
        let mut keep: Vec<u64> = self.at.iter().map(|&t| frame_of(t)).collect();
        if let Some(seq) = self.seq {
            keep.extend(0..frame_of(seq));
        }
        keep.sort_unstable();
        keep.dedup();
        let Some(&last) = keep.last() else {
            return Ok(Vec::new());
        };

        if let Source::Play(player) = &mut show.source {
            player.offline();
        }
        let mut gpu = Headless::new(self.width, self.height, show.picture, self.alpha);
        gpu.engine().preload(&show.media());
        let mut clock = Clock::fixed(self.fps);
        let mut inputs = Inputs::default();
        let mut warp = show.rest([self.width as f32, self.height as f32]);
        let mut written = Vec::new();
        for frame in 0..=last {
            let (time, dt) = clock.tick();
            let mut io = Io {
                inputs: &mut inputs,
                warp: &mut warp,
                time,
                dt,
                title: None,
                fullscreen: None,
            };
            for binding in &mut show.bindings {
                binding.frame(&mut io);
            }
            if let Some(recipe) = show.describe(&inputs, time, dt, frame == 0) {
                gpu.engine().set_recipe(recipe);
            }
            gpu.set_warp(warp);
            let capture = keep.binary_search(&frame).is_ok();
            if let Some(pixels) = gpu.render(time, dt, capture) {
                let path = self.frame_path(frame, time);
                media::save_png(&path, &pixels)?;
                written.push(path);
            }
        }
        Ok(written)
    }

    /// A sequence numbers its frames (`0001.png`, from 1, as ffmpeg and
    /// `frames` expect); picked moments are named by their time (`t002.500.png`).
    fn frame_path(&self, frame: u64, time: f32) -> PathBuf {
        let name = match self.seq {
            Some(_) => format!("{:04}.png", frame + 1),
            None => format!("t{time:07.3}.png"),
        };
        Path::new(&self.out).join(name)
    }
}

/// Scripted input for a headless run: sets `address` to `value` at `time`.
/// The `--osc "/hands 1 @1s"` of `vybe render`, as a [`Binding`] like any other.
pub struct Script {
    /// Sorted by time; `next` is the first cue not yet played.
    cues: Vec<(f32, String, crate::input::Value)>,
    next: usize,
}

impl Script {
    pub fn new(mut cues: Vec<(f32, String, crate::input::Value)>) -> Self {
        cues.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self { cues, next: 0 }
    }
}

impl Binding for Script {
    fn frame(&mut self, io: &mut Io<'_>) {
        while let Some((at, address, value)) = self.cues.get(self.next) {
            if *at > io.time {
                break;
            }
            io.inputs.set(address, value.clone());
            self.next += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(rows: [[f32; 4]; 3], u: f32, v: f32) -> [f32; 2] {
        let dot = |r: [f32; 4]| r[0] * u + r[1] * v + r[2];
        let w = dot(rows[2]);
        [dot(rows[0]) / w, dot(rows[1]) / w]
    }

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
    }

    #[test]
    fn a_square_picture_rests_centered_in_a_wide_output() {
        let warp = Warp::fit([1200.0, 1200.0], [1920.0, 1200.0]);
        let inset = (1920.0 - 1200.0) / 2.0 / 1920.0;
        assert!(close(warp.corners[0], [inset, 0.0]));
        assert!(close(warp.corners[2], [1.0 - inset, 1.0]));
        // Same shape: the whole output.
        assert_eq!(Warp::fit([4.0, 3.0], [800.0, 600.0]), Warp::default());
    }

    #[test]
    fn warp_at_rest_is_the_whole_output() {
        let rows = Warp::default().rows();
        assert!(close(project(rows, 0.0, 0.0), [-1.0, 1.0])); // top-left
        assert!(close(project(rows, 1.0, 1.0), [1.0, -1.0])); // bottom-right
        assert!(close(project(rows, 0.5, 0.5), [0.0, 0.0]));
    }

    #[test]
    fn warp_lands_each_corner_where_it_was_pulled() {
        let warp = Warp {
            corners: [[0.1, 0.05], [0.9, 0.0], [1.0, 0.95], [-0.05, 1.0]],
            feather: 0.0,
        };
        let rows = warp.rows();
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for (corner, [u, v]) in warp.corners.iter().zip(uv) {
            let clip = [2.0 * corner[0] - 1.0, 1.0 - 2.0 * corner[1]];
            assert!(close(project(rows, u, v), clip));
        }
    }

    #[test]
    fn fixed_clock_starts_at_zero_and_never_drifts() {
        let mut clock = Clock::fixed(60.0);
        assert_eq!(clock.tick().0, 0.0);
        for _ in 1..600 {
            clock.tick();
        }
        let (time, dt) = clock.tick();
        assert!((time - 10.0).abs() < 1e-6);
        assert!((dt - 1.0 / 60.0).abs() < 1e-9);
    }
}
