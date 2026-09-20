//! INPUT — the one seam the outside world comes in through. std-only.
//!
//! Two halves. [`Inputs`] is an address space of live values — `/hands`,
//! `/mode`, `/key/space`, `/mouse/x` — that a patch reads with `osc <address>`;
//! whoever has something to say writes into it. [`Binding`] is the trait those
//! writers implement: a binding sees the window's events and every frame, and
//! may touch the inputs, the [`Warp`], and the `tune` registry. OSC, MIDI, a
//! sensor on a GPIO pin: each is library glue behind this trait, in a crate of
//! its own — the core never learns what a socket is.
//!
//! The bindings here are the generic ones every work can reuse: a key runs a
//! closure ([`on`]), arrows nudge a tune ([`arrows_nudge`]), the mouse drags a
//! pair of tunes ([`mouse_drag`]). None of them is a UI noun.

use std::collections::HashMap;

use crate::stage::Warp;
use crate::tune;

/// A live value: a number, or a symbol (`/mode grid`).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f32),
    Sym(String),
}

impl Value {
    /// Reads a word as a number if it is one, a symbol otherwise.
    pub fn parse(word: &str) -> Self {
        word.parse()
            .map_or_else(|_| Value::Sym(word.to_owned()), Value::Num)
    }
}

/// The address space of live values. Total (Principle 2): an address nobody
/// has written reads as `0` / no symbol — a sensor that never spoke is a
/// sensor at rest.
#[derive(Default, Debug)]
pub struct Inputs {
    values: HashMap<String, Value>,
}

impl Inputs {
    pub fn set(&mut self, address: &str, value: Value) {
        self.values.insert(address.to_owned(), value);
    }

    pub fn get(&self, address: &str) -> Option<&Value> {
        self.values.get(address)
    }

    /// The number at `address`; `0.0` when absent or a symbol.
    pub fn num(&self, address: &str) -> f32 {
        match self.values.get(address) {
            Some(Value::Num(n)) => *n,
            _ => 0.0,
        }
    }
}

/// What a binding may touch, for exactly one event or frame.
pub struct Io<'a> {
    pub inputs: &'a mut Inputs,
    pub warp: &'a mut Warp,
    /// Seconds since start, and since the last frame.
    pub time: f32,
    pub dt: f32,
    /// Set to retitle the window (a status line, until `text` exists).
    pub title: Option<String>,
}

/// A key, by what it means rather than where it sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Tab,
    Space,
    Enter,
    Escape,
    /// A printable key, lowercase.
    Char(char),
}

impl Key {
    /// The key's name — also its address under `/key/` in the [`Inputs`].
    pub fn name(&self) -> String {
        match self {
            Key::Left => "left".into(),
            Key::Right => "right".into(),
            Key::Up => "up".into(),
            Key::Down => "down".into(),
            Key::Tab => "tab".into(),
            Key::Space => "space".into(),
            Key::Enter => "enter".into(),
            Key::Escape => "escape".into(),
            Key::Char(c) => c.to_string(),
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            "tab" => Key::Tab,
            "space" => Key::Space,
            "enter" => Key::Enter,
            "escape" => Key::Escape,
            _ => {
                let mut chars = name.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Key::Char(c.to_ascii_lowercase()),
                    _ => return None,
                }
            }
        })
    }
}

/// A window event, in the artist's terms: keys by meaning, the pointer in
/// scene space. No winit type ever crosses this line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Key {
        key: Key,
        pressed: bool,
        /// The OS is auto-repeating a held key.
        repeat: bool,
        shift: bool,
    },
    /// The pointer moved.
    Pointer { at: [f32; 2] },
    /// The primary button went down or up.
    Button { pressed: bool, at: [f32; 2] },
}

/// Something that listens to the window and speaks into the [`Io`]. Both
/// methods default to nothing, so a binding implements only what it needs.
pub trait Binding {
    fn event(&mut self, _event: &Event, _io: &mut Io<'_>) {}
    /// Once per frame, before the picture is described.
    fn frame(&mut self, _io: &mut Io<'_>) {}
}

// ---------------------------------------------------------------------------
// The generic bindings
// ---------------------------------------------------------------------------

/// Runs `action` when `key` goes down (not on auto-repeat).
pub fn on(key: Key, action: impl FnMut(&mut Io<'_>) + 'static) -> impl Binding {
    OnKey {
        key,
        action: Box::new(action),
    }
}

struct OnKey {
    key: Key,
    action: Box<dyn FnMut(&mut Io<'_>)>,
}

impl Binding for OnKey {
    fn event(&mut self, event: &Event, io: &mut Io<'_>) {
        if let Event::Key {
            key,
            pressed: true,
            repeat: false,
            ..
        } = event
        {
            if *key == self.key {
                (self.action)(io);
            }
        }
    }
}

/// The arrow keys nudge tunes: left/right move every tune matching `glob` whose
/// name ends in `/x`, up/down every one ending in `/y`. A `{name}` in the glob
/// is replaced by that tune's current value as an integer — so
/// `arrows_nudge("map/{sel}/*")` moves whichever corner `sel` points at.
pub fn arrows_nudge(glob: &str) -> ArrowsNudge {
    ArrowsNudge {
        glob: glob.to_owned(),
        step: [0.01, 0.01],
        shift: 10.0,
        y_down: false,
    }
}

/// See [`arrows_nudge`].
pub struct ArrowsNudge {
    glob: String,
    step: [f32; 2],
    shift: f32,
    y_down: bool,
}

impl ArrowsNudge {
    /// How far one press moves, per axis (default `0.01`).
    pub fn step(mut self, x: f32, y: f32) -> Self {
        self.step = [x, y];
        self
    }

    /// The multiplier while shift is held (default `10`).
    pub fn shift(mut self, times: f32) -> Self {
        self.shift = times;
        self
    }

    /// The tunes count y downward (pixels, a projector's corners), so the up
    /// arrow must *decrease* them.
    pub fn y_down(mut self) -> Self {
        self.y_down = true;
        self
    }
}

impl Binding for ArrowsNudge {
    fn event(&mut self, event: &Event, _io: &mut Io<'_>) {
        let Event::Key {
            key,
            pressed: true,
            shift,
            ..
        } = event
        else {
            return;
        };
        let up = if self.y_down { -1.0 } else { 1.0 };
        let (axis, delta) = match key {
            Key::Left => ("/x", -self.step[0]),
            Key::Right => ("/x", self.step[0]),
            Key::Up => ("/y", up * self.step[1]),
            Key::Down => ("/y", -up * self.step[1]),
            _ => return,
        };
        let delta = if *shift { delta * self.shift } else { delta };
        let glob = resolve(&self.glob);
        for name in tune::names() {
            if name.ends_with(axis) && glob_match(&glob, &name) {
                if let Some(value) = tune::get(&name) {
                    tune::set(&name, value + delta);
                }
            }
        }
    }
}

/// An axis-aligned box in scene space, addressed by fractions: `(0, 0)` is its
/// top-left, `(1, 1)` its bottom-right, **y downward** — the way an output (a
/// projector, an image) counts. The bridge between tunes that live in such
/// fractions and the scene they are drawn in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub center: [f32; 2],
    pub size: [f32; 2],
}

impl Rect {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            center: [0.0, 0.0],
            size: [width, height],
        }
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.center = [x, y];
        self
    }

    /// Fraction of the box -> scene space.
    pub fn map(&self, (u, v): (f32, f32)) -> (f32, f32) {
        (
            self.center[0] + (u - 0.5) * self.size[0],
            self.center[1] + (0.5 - v) * self.size[1],
        )
    }

    /// Scene space -> fraction of the box.
    pub fn unmap(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (
            (x - self.center[0]) / self.size[0] + 0.5,
            0.5 - (y - self.center[1]) / self.size[1],
        )
    }
}

/// The mouse drags tune *pairs*: every `<prefix>/x` + `<prefix>/y` matching
/// `glob` is a point; pressing within `radius` (scene units) of one grabs it,
/// and it follows the pointer until release.
pub fn mouse_drag(glob: &str, radius: f32) -> MouseDrag {
    MouseDrag {
        glob: glob.to_owned(),
        radius,
        within: None,
        selects: None,
        held: None,
    }
}

/// See [`mouse_drag`].
pub struct MouseDrag {
    glob: String,
    radius: f32,
    within: Option<Rect>,
    selects: Option<String>,
    /// The prefix of the pair being dragged.
    held: Option<String>,
}

impl MouseDrag {
    /// The tunes are fractions of `rect` (y-down) rather than scene positions.
    pub fn within(mut self, rect: Rect) -> Self {
        self.within = Some(rect);
        self
    }

    /// Grabbing the n-th pair (by name) also sets the tune `name` to `n` — so a
    /// click selects what the arrows will then nudge.
    pub fn selects(mut self, name: &str) -> Self {
        self.selects = Some(name.to_owned());
        self
    }

    /// The draggable pairs, by prefix, in name order.
    fn pairs(&self) -> Vec<String> {
        let glob = resolve(&self.glob);
        let mut prefixes: Vec<String> = tune::names()
            .into_iter()
            .filter(|name| glob_match(&glob, name))
            .filter_map(|name| name.strip_suffix("/x").map(str::to_owned))
            .filter(|prefix| tune::get(&format!("{prefix}/y")).is_some())
            .collect();
        prefixes.sort();
        prefixes
    }

    fn to_scene(&self, point: (f32, f32)) -> (f32, f32) {
        self.within.map_or(point, |rect| rect.map(point))
    }
}

impl Binding for MouseDrag {
    fn event(&mut self, event: &Event, _io: &mut Io<'_>) {
        match *event {
            Event::Button { pressed: true, at } => {
                let nearest = self
                    .pairs()
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, prefix)| {
                        let x = tune::get(&format!("{prefix}/x"))?;
                        let y = tune::get(&format!("{prefix}/y"))?;
                        let (sx, sy) = self.to_scene((x, y));
                        let distance = (sx - at[0]).hypot(sy - at[1]);
                        (distance <= self.radius).then_some((distance, index, prefix))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, index, prefix)) = nearest {
                    if let Some(selects) = &self.selects {
                        tune::set(selects, index as f32);
                    }
                    self.held = Some(prefix);
                }
            }
            Event::Button { pressed: false, .. } => self.held = None,
            Event::Pointer { at } => {
                if let Some(prefix) = &self.held {
                    let point = (at[0], at[1]);
                    let (x, y) = self.within.map_or(point, |rect| rect.unmap(point));
                    tune::set(&format!("{prefix}/x"), x);
                    tune::set(&format!("{prefix}/y"), y);
                }
            }
            Event::Key { .. } => {}
        }
    }
}

/// Replaces each `{name}` in a glob with that tune's value, as an integer.
fn resolve(glob: &str) -> String {
    let mut out = String::new();
    let mut rest = glob;
    while let Some((before, after)) = rest.split_once('{') {
        out.push_str(before);
        match after.split_once('}') {
            Some((name, tail)) => {
                let value = tune::get(name).unwrap_or(0.0);
                out.push_str(&(value.round() as i64).to_string());
                rest = tail;
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `*` matches any run of characters (including none); everything else is
/// literal. Enough for address families: `map/*`, `map/2/*`, `*/x`.
pub fn glob_match(glob: &str, name: &str) -> bool {
    match glob.split_once('*') {
        None => glob == name,
        Some((head, tail)) => {
            name.strip_prefix(head).is_some_and(|rest| {
                // Try every split point for the rest of the pattern.
                (0..=rest.len())
                    .filter(|&i| rest.is_char_boundary(i))
                    .any(|i| glob_match(tail, &rest[i..]))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_address_reads_as_rest() {
        let inputs = Inputs::default();
        assert_eq!(inputs.num("/hands"), 0.0);
        assert!(inputs.get("/mode").is_none());
    }

    #[test]
    fn words_parse_as_numbers_or_symbols() {
        assert_eq!(Value::parse("1"), Value::Num(1.0));
        assert_eq!(Value::parse(".5"), Value::Num(0.5));
        assert_eq!(Value::parse("grid"), Value::Sym("grid".into()));
    }

    #[test]
    fn globs_match_address_families() {
        assert!(glob_match("map/*", "map/0/x"));
        assert!(glob_match("map/2/*", "map/2/y"));
        assert!(!glob_match("map/2/*", "map/3/y"));
        assert!(glob_match("*/x", "map/0/x"));
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exactly"));
    }

    #[test]
    fn a_rect_maps_fractions_y_down_and_back() {
        let rect = Rect::new(1.6, 1.0);
        assert_eq!(rect.map((0.0, 0.0)), (-0.8, 0.5)); // top-left
        assert_eq!(rect.map((1.0, 1.0)), (0.8, -0.5)); // bottom-right
        let (u, v) = rect.unmap(rect.map((0.25, 0.75)));
        assert!((u - 0.25).abs() < 1e-6 && (v - 0.75).abs() < 1e-6);
    }

    #[test]
    fn keys_round_trip_through_their_names() {
        for key in [Key::Left, Key::Space, Key::Tab, Key::Char('g')] {
            assert_eq!(Key::parse(&key.name()), Some(key));
        }
        assert_eq!(Key::parse("nonsense"), None);
    }
}
