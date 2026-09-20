//! OBJECTS — the small stateful things behaviour is made of: a [`Gate`] that
//! debounces a sensor, a [`Ramp`] it drives, a [`Fader`] between scenes.
//! std-only and GPU-free, so they are tested like arithmetic.
//!
//! They are shared by every front-end: the patch player runs a `.vy` on them,
//! and a Rust `Sketch` will hold the very same structs. (Effects live in
//! chains; behaviour lives here; the bridge between them is a number.)
//!
//! **Time semantics, decided once** (they break every transition if vague):
//! - A *level* is true for as long as its condition holds: [`Gate::on`],
//!   [`Gate::off`], [`Ramp::value`] `== 1.0`.
//! - An *edge* is true for exactly one update — the one where the level
//!   changed: [`Gate::rose`], [`Gate::fell`].
//! - Every object advances by an explicit `dt`. None reads a clock, so a fixed
//!   step replays a performance exactly.

/// A boolean over time, debounced: the raw input must hold its new state for
/// `debounce` seconds before the gate follows it. A hand brushing a sensor
/// doesn't open it; a hand resting on it does.
#[derive(Clone, Debug)]
pub struct Gate {
    debounce: f32,
    on: bool,
    /// How long the raw input has disagreed with `on`.
    pending: f32,
    edge: Edge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    None,
    Rose,
    Fell,
}

impl Gate {
    pub fn new(debounce: f32) -> Self {
        Self {
            debounce: debounce.max(0.0),
            on: false,
            pending: 0.0,
            edge: Edge::None,
        }
    }

    /// Retunes the debounce without touching the gate's state — so a patch
    /// edited while it runs keeps its gate open.
    pub fn set_debounce(&mut self, debounce: f32) {
        self.debounce = debounce.max(0.0);
    }

    /// Feeds this frame's raw input. Call exactly once per frame: edges last
    /// until the next update.
    pub fn update(&mut self, raw: bool, dt: f32) {
        self.edge = Edge::None;
        if raw == self.on {
            self.pending = 0.0;
            return;
        }
        self.pending += dt;
        if self.pending >= self.debounce {
            self.on = raw;
            self.pending = 0.0;
            self.edge = if raw { Edge::Rose } else { Edge::Fell };
        }
    }

    /// Level: the gate is open.
    pub fn on(&self) -> bool {
        self.on
    }

    /// Level: the gate is closed.
    pub fn off(&self) -> bool {
        !self.on
    }

    /// Edge: the gate opened on this update.
    pub fn rose(&self) -> bool {
        self.edge == Edge::Rose
    }

    /// Edge: the gate closed on this update.
    pub fn fell(&self) -> bool {
        self.edge == Edge::Fell
    }
}

/// A value in `0..=1` pushed by a gate: it climbs while the gate is open
/// (reaching `1` after `up` seconds) and falls back while it is closed (`down`
/// seconds). Linear, and clamped — so it reaches its ends *exactly*, and
/// `== 1.0` is a condition you can build a transition on; shape it afterwards
/// (`smooth(t)`) rather than here.
#[derive(Clone, Debug)]
pub struct Ramp {
    up: f32,
    down: f32,
    value: f32,
}

impl Ramp {
    pub fn new(up: f32, down: f32) -> Self {
        Self {
            up: up.max(0.0),
            down: down.max(0.0),
            value: 0.0,
        }
    }

    /// Retunes the durations without touching the value — so a patch edited
    /// while it runs keeps its ramp where it was.
    pub fn set(&mut self, up: f32, down: f32) {
        self.up = up.max(0.0);
        self.down = down.max(0.0);
    }

    pub fn update(&mut self, open: bool, dt: f32) {
        let (seconds, toward) = if open {
            (self.up, 1.0)
        } else {
            (self.down, -1.0)
        };
        // A zero-second ramp is a switch.
        let step = if seconds > 0.0 { dt / seconds } else { 1.0 };
        self.value = (self.value + toward * step).clamp(0.0, 1.0);
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

/// Which scene is showing, and — during a fade — which one is leaving.
/// `visible()` is the whole interface to the picture: a list of scenes with
/// weights that always sum to `1`. (Audio weight will follow the same numbers.)
///
/// A fade interrupted by another transition snaps: the scene that was arriving
/// becomes the one leaving, at full weight. Two scenes at most, ever.
#[derive(Clone, Debug)]
pub struct Fader<S> {
    current: S,
    leaving: Option<S>,
    progress: f32,
    seconds: f32,
}

impl<S: Clone + PartialEq> Fader<S> {
    pub fn new(first: S) -> Self {
        Self {
            current: first,
            leaving: None,
            progress: 1.0,
            seconds: 0.0,
        }
    }

    pub fn current(&self) -> &S {
        &self.current
    }

    /// Switches at once.
    pub fn cut(&mut self, to: S) {
        self.current = to;
        self.leaving = None;
        self.progress = 1.0;
    }

    /// Crossfades over `seconds` (a zero-second fade is a cut).
    pub fn fade_to(&mut self, to: S, seconds: f32) {
        if seconds <= 0.0 {
            return self.cut(to);
        }
        self.leaving = Some(std::mem::replace(&mut self.current, to));
        self.progress = 0.0;
        self.seconds = seconds;
    }

    pub fn update(&mut self, dt: f32) {
        if self.leaving.is_some() {
            self.progress = (self.progress + dt / self.seconds).min(1.0);
            if self.progress >= 1.0 {
                self.leaving = None;
            }
        }
    }

    /// The scenes to draw, bottom first, with their weights.
    pub fn visible(&self) -> Vec<(S, f32)> {
        match &self.leaving {
            Some(leaving) => vec![
                (leaving.clone(), 1.0 - self.progress),
                (self.current.clone(), self.progress),
            ],
            None => vec![(self.current.clone(), 1.0)],
        }
    }
}

/// Hermite ease over `0..=1` — the `smooth(t)` of a patch. Clamps.
pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn a_brush_does_not_open_the_gate_but_a_rest_does() {
        let mut gate = Gate::new(0.08);
        // Two frames of contact (33 ms) — under the debounce.
        gate.update(true, DT);
        gate.update(true, DT);
        gate.update(false, DT);
        assert!(gate.off());
        // Held: opens once 80 ms have passed, and the edge lasts one update.
        let mut rose = 0;
        for _ in 0..30 {
            gate.update(true, DT);
            rose += usize::from(gate.rose());
        }
        assert!(gate.on());
        assert_eq!(rose, 1);
    }

    #[test]
    fn a_gate_without_debounce_follows_at_once() {
        let mut gate = Gate::new(0.0);
        gate.update(true, DT);
        assert!(gate.on() && gate.rose());
        gate.update(true, DT);
        assert!(gate.on() && !gate.rose());
        gate.update(false, DT);
        assert!(gate.off() && gate.fell());
    }

    #[test]
    fn a_ramp_reaches_its_ends_exactly() {
        let mut ramp = Ramp::new(3.0, 1.2);
        for _ in 0..(3 * 60 + 2) {
            ramp.update(true, DT);
        }
        assert_eq!(ramp.value(), 1.0);
        for _ in 0..(2 * 60) {
            ramp.update(false, DT);
        }
        assert_eq!(ramp.value(), 0.0);
    }

    #[test]
    fn a_ramp_falls_faster_than_it_climbs_when_told_to() {
        let mut ramp = Ramp::new(3.0, 1.0);
        for _ in 0..60 {
            ramp.update(true, DT);
        }
        let climbed = ramp.value();
        assert!((climbed - 1.0 / 3.0).abs() < 1e-3);
        for _ in 0..20 {
            ramp.update(false, DT);
        }
        assert!(ramp.value() < 1e-3);
    }

    #[test]
    fn a_fader_shows_one_scene_or_two_summing_to_one() {
        let mut fader = Fader::new("idle");
        assert_eq!(fader.visible(), vec![("idle", 1.0)]);

        fader.fade_to("play", 1.0);
        for _ in 0..30 {
            fader.update(DT);
        }
        let visible = fader.visible();
        assert_eq!(visible.len(), 2);
        assert_eq!((visible[0].0, visible[1].0), ("idle", "play"));
        assert!((visible[0].1 + visible[1].1 - 1.0).abs() < 1e-6);
        assert!((visible[1].1 - 0.5).abs() < 1e-3);

        for _ in 0..31 {
            fader.update(DT);
        }
        assert_eq!(fader.visible(), vec![("play", 1.0)]);
    }

    #[test]
    fn a_cut_is_immediate_and_interrupts_a_fade() {
        let mut fader = Fader::new(0);
        fader.fade_to(1, 2.0);
        fader.update(DT);
        fader.cut(2);
        assert_eq!(fader.visible(), vec![(2, 1.0)]);
        assert_eq!(*fader.current(), 2);
    }

    #[test]
    fn smooth_eases_both_ends_and_clamps() {
        assert_eq!(smooth(0.0), 0.0);
        assert_eq!(smooth(1.0), 1.0);
        assert_eq!(smooth(0.5), 0.5);
        assert!(smooth(0.1) < 0.1 && smooth(0.9) > 0.9);
        assert_eq!(smooth(7.0), 1.0);
    }
}
