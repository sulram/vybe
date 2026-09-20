//! The [`Player`] performs a [`Patch`]: every frame it reads the inputs, moves
//! its objects (gates, ramps, the scene fader, the sequences' playheads), and
//! describes the picture as a `Recipe` with this frame's numbers filled in.
//!
//! It holds all the *behaviour* and none of the pixels. The recipe it emits is
//! keyed by node name ([`Recipe::Named`]), so the GPU side keeps a node's state
//! — a feedback trail — from one frame's description to the next, and a node
//! wired into two scenes is one node: a cut between them is seamless.

use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use super::vocabulary::{self, EffectKind, Family, ModKind, SourceKind};
use super::{
    Body, Comp, Expr, Item, Patch, Scalar, Severity, Source, Term, What, check, parse,
    resolve_frames,
};
use crate::clip::{Clip, Decoder, Pace, Slot};
use crate::input::{Inputs, Value};
use crate::objects::{Fader, Gate, Ramp, smooth};
use crate::recipe::{CompositeLayer, Fit, Image, Recipe, Stream, Stroke};
use crate::shell::{self, Perform};
use crate::stage::Stage;
use crate::sugar::{Blend, Hue, Swirl};

/// A `frames` sequence without `fps` plays at this rate.
const DEFAULT_FPS: f32 = 30.0;
/// A wire nested deeper than this is a loop (`vybe check` names it); the
/// player just stops following it (Principle 3: performing never panics).
const MAX_DEPTH: usize = 32;

pub struct Player {
    patch: Patch,
    /// Declared scenes, then the implicit ones (a node a transition goes to).
    scenes: Vec<SceneDef>,
    fader: Fader<usize>,
    /// Per transition: was its condition true last frame? (It fires on the
    /// frame it *becomes* true.)
    held: Vec<bool>,
    states: HashMap<String, State>,
    /// This frame's Scalars, by node name.
    values: HashMap<String, f32>,
    symbols: HashMap<String, String>,
    /// The Scalars that went from closed to open this frame.
    rose: HashSet<String>,
    sequences: HashMap<String, Sequence>,
    /// Pixels per scene unit — what `px` resolves against.
    unit_px: f32,
    time: f32,
    videos: HashMap<String, Video>,
    /// What opens a `video` — none, and videos draw nothing (`vybe check` says so).
    decoder: Option<Rc<dyn Decoder>>,
    pace: Pace,
    /// Where media paths resolve from — kept for [`Player::reload`].
    base: PathBuf,
    watch: Option<Watch>,
}

/// A patch file being watched for edits (see [`Player::watch`]).
struct Watch {
    path: PathBuf,
    seen: Option<std::time::SystemTime>,
    /// Seconds since the file was last looked at.
    since: f32,
}

/// How often a watched patch is looked at, in seconds.
const WATCH_EVERY: f32 = 0.25;

struct SceneDef {
    name: String,
    comp: Comp,
    /// Every sequence and video it reaches, through its sources and its wires.
    reaches: Vec<String>,
}

/// A `video` source: the behaviour around a playing [`Clip`]. The clip keeps
/// its own picture and sound in sync; this decides *when* it plays and how loud.
struct Video {
    path: PathBuf,
    source: Source,
    looping: bool,
    /// Opened on the first frame. `None` after a failed open, too: a video that
    /// won't play is a node that draws nothing, not a show that stops.
    clip: Option<Box<dyn Clip>>,
    tried: bool,
    /// Where its newest frame waits for the GPU.
    slot: Arc<Slot>,
    /// Seconds since it (re)started.
    playhead: f32,
    playing: bool,
    volume: f32,
    done: bool,
}

enum State {
    Gate(Gate),
    Ramp(Ramp),
}

/// A `frames` source's timeline.
struct Sequence {
    frames: Arc<[PathBuf]>,
    fps: f32,
    looping: bool,
    paused: bool,
    playhead: f32,
}

impl Sequence {
    fn duration(&self) -> f32 {
        self.frames.len() as f32 / self.fps
    }

    fn done(&self) -> bool {
        !self.looping && self.playhead >= self.duration()
    }

    fn index(&self) -> usize {
        let last = self.frames.len().saturating_sub(1);
        let frame = (self.playhead * self.fps) as usize;
        if self.looping {
            frame % self.frames.len().max(1)
        } else {
            frame.min(last)
        }
    }
}

/// The key of the clip behind item `index` of `owner`'s composition. A node
/// that *is* one sequence goes by its own name — so `clip done` finds it.
fn clip_key(owner: &str, index: usize, single: bool) -> String {
    if single {
        owner.to_owned()
    } else {
        format!("{owner}#{index}")
    }
}

impl Player {
    /// Media paths resolve against `base` — the patch's own folder.
    pub fn new(patch: Patch, base: &Path) -> Self {
        // Scenes: the declared ones, then any node a transition names directly.
        let mut scenes: Vec<(String, Comp)> = patch
            .scenes
            .iter()
            .map(|s| (s.name.clone(), s.comp.clone()))
            .collect();
        let wire = |name: &str| Comp {
            items: vec![Item {
                what: What::Wire(name.to_owned()),
                alpha: None,
                at: None,
                blend: Blend::Over,
            }],
            effect: None,
        };
        let draws = |name: &str| {
            patch
                .node(name)
                .is_some_and(|n| !matches!(n.body, Body::Scalar(_)))
        };
        for t in &patch.transitions {
            for end in t.from.iter().chain(Some(&t.to)) {
                if !scenes.iter().any(|(name, _)| name == end) && draws(end) {
                    scenes.push((end.clone(), wire(end)));
                }
            }
        }
        // No scenes at all: the last node that draws is the picture.
        if scenes.is_empty() {
            if let Some(last) = patch.nodes.iter().rev().find(|n| draws(&n.name)) {
                scenes.push((last.name.clone(), wire(&last.name)));
            }
        }

        let mut sequences = HashMap::new();
        let mut videos = HashMap::new();
        let mut collect = |owner: &str, comp: &Comp| {
            let single = comp.items.len() == 1;
            for (index, item) in comp.items.iter().enumerate() {
                if let What::Source(source) = &item.what {
                    if source.kind == SourceKind::Video {
                        videos.insert(
                            clip_key(owner, index, single),
                            Video {
                                path: base.join(source.text.as_deref().unwrap_or_default()),
                                source: source.clone(),
                                looping: source.modifier(ModKind::Loop).is_some(),
                                clip: None,
                                tried: false,
                                slot: Slot::new(),
                                playhead: 0.0,
                                playing: false,
                                volume: -1.0, // unset: the first frame sets it
                                done: false,
                            },
                        );
                    }
                    if source.kind == SourceKind::Frames {
                        let glob = source.text.as_deref().unwrap_or_default();
                        sequences.insert(
                            clip_key(owner, index, single),
                            Sequence {
                                frames: resolve_frames(base, glob).into(),
                                fps: DEFAULT_FPS,
                                looping: source.modifier(ModKind::Loop).is_some(),
                                paused: source.modifier(ModKind::Paused).is_some(),
                                playhead: 0.0,
                            },
                        );
                    }
                }
            }
        };
        for node in &patch.nodes {
            if let Body::Signal(comp) = &node.body {
                collect(&node.name, comp);
            }
        }
        for (name, comp) in &scenes {
            collect(&format!("scene:{name}"), comp);
        }

        let scenes: Vec<SceneDef> = scenes
            .into_iter()
            .map(|(name, comp)| {
                let mut reached = Vec::new();
                reach(&patch, &format!("scene:{name}"), &comp, &mut reached, 0);
                SceneDef {
                    name,
                    comp,
                    reaches: reached,
                }
            })
            .collect();

        // `px` is measured on the picture, which is the output unless it has a
        // size of its own.
        let unit_px = patch
            .out
            .as_ref()
            .and_then(|out| out.picture.or(out.size))
            .map_or(800.0, |[w, h]| w.min(h) as f32);
        Self {
            held: vec![false; patch.transitions.len()],
            fader: Fader::new(0),
            states: HashMap::new(),
            values: HashMap::new(),
            symbols: HashMap::new(),
            rose: HashSet::new(),
            scenes,
            sequences,
            unit_px,
            time: 0.0,
            videos,
            decoder: None,
            pace: Pace::Live,
            base: base.to_owned(),
            watch: None,
            patch,
        }
    }

    /// The output's shorter edge in pixels — what `px` is measured against.
    pub fn unit_px(mut self, pixels: f32) -> Self {
        self.unit_px = pixels.max(1.0);
        self
    }

    /// What opens this patch's `video` sources (`vybe-video`'s GStreamer, say).
    /// Without one, a video draws nothing.
    pub fn decoder(mut self, decoder: impl Decoder + 'static) -> Self {
        self.decoder = Some(Rc::new(decoder));
        self
    }

    /// Watches `path` (the patch's own file) and [`reload`](Player::reload)s
    /// whenever it is saved. A save that doesn't parse or check prints its
    /// findings and changes nothing — the show goes on with the last good patch.
    pub fn watch(mut self, path: &Path) -> Self {
        self.watch = Some(Watch {
            seen: modified(path),
            path: path.to_owned(),
            since: 0.0,
        });
        self
    }

    /// Swaps in an edited patch **without restarting the performance**. State is
    /// carried over by name — the same rule that keeps a node's pixels alive on
    /// the GPU (key = identity): a Scalar keeps its gate or ramp (with the new
    /// timings), a sequence that is still the same files keeps its playhead, and
    /// the scene that was showing still shows, if it still exists. Editing a
    /// condition never *fires* it: a transition only fires when its condition
    /// becomes true afterwards.
    pub fn reload(&mut self, patch: Patch) {
        let showing = self.scene().to_owned();
        let mut next = Player::new(patch, &self.base);
        next.unit_px = self.unit_px;
        next.time = self.time;
        next.watch = self.watch.take();
        next.states = std::mem::take(&mut self.states);
        next.values = std::mem::take(&mut self.values);
        next.symbols = std::mem::take(&mut self.symbols);
        next.decoder = self.decoder.clone();
        next.pace = self.pace;
        // A video that is still the same file keeps playing, uninterrupted.
        for (key, video) in &mut next.videos {
            if let Some(old) = self.videos.remove(key) {
                if old.path == video.path && old.looping == video.looping {
                    let source = std::mem::replace(&mut video.source, old.source.clone());
                    *video = Video { source, ..old };
                }
            }
        }
        for (key, clip) in &mut next.sequences {
            if let Some(old) = self
                .sequences
                .get(key)
                .filter(|old| old.frames == clip.frames)
            {
                clip.playhead = old.playhead;
            }
        }
        if let Some(index) = next.scenes.iter().position(|s| s.name == showing) {
            next.fader = Fader::new(index);
        }
        next.held = next
            .patch
            .transitions
            .iter()
            .map(|t| t.cond.iter().all(|term| next.holds(term)))
            .collect();
        *self = next;
    }

    /// Looks at the watched file now and then; reloads it when it was saved.
    fn poll(&mut self, dt: f32) {
        let Some(watch) = &mut self.watch else {
            return;
        };
        watch.since += dt;
        if watch.since < WATCH_EVERY {
            return;
        }
        watch.since = 0.0;
        // Mid-save the file may be missing for an instant; that is not an edit.
        let Some(now) = modified(&watch.path) else {
            return;
        };
        if watch.seen == Some(now) {
            return;
        }
        watch.seen = Some(now);
        let path = watch.path.clone();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let (patch, found) = match parse(&text) {
            Ok(patch) => {
                let found = check(&patch, &self.base);
                (Some(patch), found)
            }
            Err(errors) => (None, errors),
        };
        // Notes were read when the patch first ran; on every save, only what
        // needs attention is worth repeating.
        for finding in found.iter().filter(|d| d.severity > Severity::Note) {
            eprintln!("{}: {finding}", path.display());
        }
        match patch {
            Some(patch) if !found.iter().any(|d| d.severity == Severity::Error) => {
                if patch.out != self.patch.out {
                    eprintln!(
                        "{}: the `out` line changed — restart to apply it",
                        path.display()
                    );
                }
                self.reload(patch);
                eprintln!("{}: reloaded", path.display());
            }
            _ => eprintln!(
                "{}: not reloaded — still playing the last good patch",
                path.display()
            ),
        }
    }

    /// The scene showing now (during a fade: the one arriving).
    pub fn scene(&self) -> &str {
        self.scenes
            .get(*self.fader.current())
            .map_or("", |scene| scene.name.as_str())
    }

    /// This frame's value of a Scalar node.
    pub fn value(&self, name: &str) -> f32 {
        self.values.get(name).copied().unwrap_or(0.0)
    }

    /// The stage this patch plays on: a window shaped like its `out`, scaled
    /// to fit a laptop screen. Hang bindings on it, then `.show()`.
    pub fn stage(self) -> Stage {
        let [w, h] = self
            .patch
            .out
            .as_ref()
            .and_then(|out| out.size)
            .unwrap_or([800, 800])
            .map(|px| px as f32);
        let fit = (1280.0 / w).min(800.0 / h).min(1.0);
        let picture = self.patch.out.as_ref().and_then(|out| out.picture);
        let stage = Stage::new(shell::Source::Play(Box::new(self)))
            .size(w * fit, h * fit)
            .title("vybe — patch");
        match picture {
            Some([w, h]) => stage.picture(w, h),
            None => stage,
        }
    }

    /// Advances the performance by `dt` without describing the picture — what
    /// a scenario check runs on: 60 s of behaviour in a blink, no GPU.
    pub fn advance(&mut self, inputs: &Inputs, time: f32, dt: f32) {
        self.poll(dt);
        self.time = time;
        self.scalars(inputs, dt);
        for clip in self.sequences.values_mut() {
            if !clip.paused {
                clip.playhead += dt;
            }
        }
        self.fader.update(dt);
        self.transitions();
        self.videos(dt);
    }

    // -----------------------------------------------------------------------
    // Scalars
    // -----------------------------------------------------------------------

    /// Evaluates every Scalar node, in file order. A wire to a node further
    /// down reads last frame's value — one frame late, never undefined.
    fn scalars(&mut self, inputs: &Inputs, dt: f32) {
        self.rose.clear();
        for i in 0..self.patch.nodes.len() {
            let node = &self.patch.nodes[i];
            let Body::Scalar(scalar) = &node.body else {
                continue;
            };
            let name = node.name.clone();
            let before = self.value(&name);
            let value = match scalar {
                Scalar::Input { address, debounce } => {
                    let raw = match inputs.get(address) {
                        Some(Value::Num(n)) => *n,
                        Some(Value::Sym(symbol)) => {
                            self.symbols.insert(name.clone(), symbol.clone());
                            0.0
                        }
                        None => 0.0,
                    };
                    match debounce {
                        // Debounced, it is a gate: 0 or 1, and it says when it rose.
                        Some(seconds) => {
                            let seconds = self.eval(seconds);
                            // (A reload may have turned this node from a ramp
                            // into a gate: then it starts over as one.)
                            let state = self
                                .states
                                .entry(name.clone())
                                .or_insert_with(|| State::Gate(Gate::new(seconds)));
                            if !matches!(state, State::Gate(_)) {
                                *state = State::Gate(Gate::new(seconds));
                            }
                            let State::Gate(gate) = state else {
                                unreachable!("just made sure it is a gate");
                            };
                            gate.set_debounce(seconds);
                            gate.update(raw > 0.5, dt);
                            f32::from(u8::from(gate.on()))
                        }
                        None => raw,
                    }
                }
                Scalar::Ramp { gate, up, down } => {
                    let open = self.value(gate) > 0.5;
                    let (up, down) = (self.eval(up), self.eval(down));
                    let state = self
                        .states
                        .entry(name.clone())
                        .or_insert_with(|| State::Ramp(Ramp::new(up, down)));
                    if !matches!(state, State::Ramp(_)) {
                        *state = State::Ramp(Ramp::new(up, down));
                    }
                    let State::Ramp(ramp) = state else {
                        unreachable!("just made sure it is a ramp");
                    };
                    ramp.set(up, down);
                    ramp.update(open, dt);
                    ramp.value()
                }
                Scalar::Value(expr) => self.eval(expr),
            };
            if before <= 0.5 && value > 0.5 {
                self.rose.insert(name.clone());
            }
            self.values.insert(name, value);
        }
    }

    fn eval(&self, expr: &Expr) -> f32 {
        match expr {
            Expr::Num(n) => *n,
            Expr::Px(px) => px / self.unit_px,
            Expr::Wire(name) => self.value(name),
            Expr::Neg(a) => -self.eval(a),
            Expr::Add(a, b) => self.eval(a) + self.eval(b),
            Expr::Sub(a, b) => self.eval(a) - self.eval(b),
            Expr::Mul(a, b) => self.eval(a) * self.eval(b),
            Expr::Smooth(a) => smooth(self.eval(a)),
            Expr::Osc { hz, amp } => self.eval(amp) * (TAU * self.eval(hz) * self.time).sin(),
        }
    }

    // -----------------------------------------------------------------------
    // Scenes
    // -----------------------------------------------------------------------

    fn holds(&self, term: &Term) -> bool {
        match term {
            Term::Rise(name) => self.rose.contains(name),
            Term::Off(name) => self.value(name) <= 0.5,
            Term::Done(name) => {
                self.sequences.get(name).is_some_and(Sequence::done)
                    || self.videos.get(name).is_some_and(|video| video.done)
            }
            Term::Is(name, Value::Num(n)) => (self.value(name) - n).abs() < 1e-4,
            Term::Is(name, Value::Sym(symbol)) => self.symbols.get(name) == Some(symbol),
        }
    }

    /// Fires the first transition (in file order) whose condition *became* true
    /// this frame while its `from` scene shows. Every condition's memory is
    /// updated regardless — an edge missed in another scene stays missed.
    fn transitions(&mut self) {
        let current = *self.fader.current();
        let mut fired = None;
        for (i, t) in self.patch.transitions.iter().enumerate() {
            let now = t.cond.iter().all(|term| self.holds(term));
            let became = now && !self.held[i];
            self.held[i] = now;
            let from_here = t
                .from
                .as_ref()
                .is_none_or(|from| self.scenes.get(current).is_some_and(|s| s.name == *from));
            if became && from_here && fired.is_none() {
                fired = self
                    .scenes
                    .iter()
                    .position(|s| s.name == t.to)
                    .filter(|&to| to != current)
                    .map(|to| (to, t.fade.as_ref().map(|fade| self.eval(fade))));
            }
        }
        if let Some((to, fade)) = fired {
            match fade {
                Some(seconds) => self.fader.fade_to(to, seconds),
                None => self.fader.cut(to),
            }
            // What plays once starts over each time its scene is entered.
            for key in &self.scenes[to].reaches {
                if let Some(clip) = self.sequences.get_mut(key).filter(|c| !c.looping) {
                    clip.playhead = 0.0;
                }
                if let Some(video) = self.videos.get_mut(key).filter(|v| !v.looping) {
                    video.playhead = 0.0;
                    video.done = false;
                    if let Some(clip) = &mut video.clip {
                        clip.restart();
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Videos
    // -----------------------------------------------------------------------

    /// Runs the videos: each plays only while a visible scene reaches it, as
    /// loud as that scene is visible — so a crossfade is a crossfade of sound
    /// too — and hands its newest frame to the GPU.
    fn videos(&mut self, dt: f32) {
        let mut weights: HashMap<&str, f32> = HashMap::new();
        for (scene, weight) in self.fader.visible() {
            for key in self.scenes.get(scene).map_or(&[][..], |s| &s.reaches) {
                let heard = weights.entry(key.as_str()).or_default();
                *heard = heard.max(weight);
            }
        }
        let weights: HashMap<String, f32> = weights
            .into_iter()
            .map(|(key, weight)| (key.to_owned(), weight))
            .collect();

        let keys: Vec<String> = self.videos.keys().cloned().collect();
        for key in keys {
            let weight = weights.get(&key).copied().unwrap_or(0.0);
            // This frame's knobs — Scalars plug into `vol` like anywhere else.
            let (volume, held) = {
                let source = &self.videos[&key].source;
                let vol = source
                    .modifier(ModKind::Vol)
                    .and_then(|m| m.args.first())
                    .map_or(1.0, |v| self.eval(v));
                let muted = source.modifier(ModKind::Mute).is_some();
                (
                    if muted {
                        0.0
                    } else {
                        vol.clamp(0.0, 1.0) * weight
                    },
                    source.modifier(ModKind::Paused).is_some(),
                )
            };
            let (decoder, pace) = (self.decoder.clone(), self.pace);
            let Some(video) = self.videos.get_mut(&key) else {
                continue;
            };
            if !video.tried {
                video.tried = true;
                if let Some(decoder) = decoder {
                    match decoder.open(&video.path, video.looping, pace) {
                        Ok(mut clip) => {
                            // Opened paused and silent: it starts when seen.
                            clip.set_paused(true);
                            clip.set_volume(0.0);
                            video.clip = Some(clip);
                        }
                        Err(e) => eprintln!("vybe: {e}"),
                    }
                }
            }
            let Some(clip) = &mut video.clip else {
                continue;
            };
            let playing = weight > 0.0 && !held;
            if playing != video.playing {
                video.playing = playing;
                clip.set_paused(!playing);
            }
            if (volume - video.volume).abs() > 1e-3 {
                video.volume = volume;
                clip.set_volume(volume);
            }
            if playing {
                video.playhead += dt;
            }
            // Even held, a video shows its first frame.
            if let Some(frame) = clip.frame(video.playhead) {
                video.slot.put(frame);
            }
            video.done = clip.done();
        }
    }

    // -----------------------------------------------------------------------
    // Describing the picture
    // -----------------------------------------------------------------------

    /// The picture, now: the visible scenes, each by its weight. Scenes *add*
    /// (weights sum to 1), so a crossfade is an exact dissolve whatever the
    /// scenes' own transparency.
    fn describe(&self) -> Recipe {
        Recipe::Composite(
            self.fader
                .visible()
                .into_iter()
                .filter_map(|(index, weight)| {
                    let scene = self.scenes.get(index)?;
                    let key = format!("scene:{}", scene.name);
                    Some(CompositeLayer {
                        recipe: Recipe::Named(
                            key.clone(),
                            Box::new(self.comp(&scene.comp, &key, 0)),
                        ),
                        blend: Blend::Add,
                        alpha: weight,
                    })
                })
                .collect(),
        )
    }

    fn comp(&self, comp: &Comp, owner: &str, depth: usize) -> Recipe {
        let single = comp.items.len() == 1;
        let mut layers: Vec<CompositeLayer> = comp
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let alpha = item
                    .alpha
                    .as_ref()
                    .map_or(1.0, |a| self.eval(a).clamp(0.0, 1.0));
                let at = item.at.as_ref().map(|at| self.eval(at).clamp(0.0, 1.0));
                let recipe = match &item.what {
                    What::Wire(name) => self.wire(name, at, depth),
                    What::Source(source) => {
                        self.source(source, &clip_key(owner, index, single), at)
                    }
                };
                CompositeLayer {
                    recipe,
                    blend: item.blend,
                    alpha,
                }
            })
            .collect();

        // `| feedback`: the node's one shape becomes the loop's energy source.
        if let Some(effect) = &comp.effect {
            let EffectKind::Feedback = effect.kind;
            let word = vocabulary::effect("feedback").expect("feedback is in the vocabulary");
            let param = |name: &str| {
                let written = effect.params.iter().rev().find(|(p, _)| p == name);
                let default = word
                    .params
                    .iter()
                    .find(|(p, _)| *p == name)
                    .map_or(0.0, |p| p.1);
                written.map_or(default, |(_, value)| self.eval(value))
            };
            let source = match layers.pop() {
                Some(CompositeLayer {
                    recipe: Recipe::Shapes(strokes),
                    alpha,
                    ..
                }) => faded(strokes, alpha),
                _ => Vec::new(),
            };
            return Recipe::Feedback {
                source,
                swirl: Swirl {
                    decay: param("decay"),
                    angle: param("angle"),
                    scale: param("scale"),
                },
            };
        }

        // The cheap paths. One item as-is needs no stack; and plain geometry
        // stacked `over` is one painter's-order pass — exact as long as an
        // item's opacity lands on a single stroke.
        if let [
            CompositeLayer {
                alpha,
                blend: Blend::Over,
                ..
            },
        ] = layers.as_slice()
        {
            if *alpha == 1.0 {
                return layers.remove(0).recipe;
            }
        }
        let folds = layers.iter().all(|layer| {
            matches!(&layer.recipe, Recipe::Shapes(s) if layer.blend == Blend::Over && (layer.alpha == 1.0 || s.len() <= 1))
        });
        if folds {
            return Recipe::Shapes(
                layers
                    .into_iter()
                    .flat_map(|layer| match layer.recipe {
                        Recipe::Shapes(strokes) => faded(strokes, layer.alpha),
                        _ => Vec::new(),
                    })
                    .collect(),
            );
        }
        Recipe::Composite(layers)
    }

    /// A wire: *the* node of that name, whoever else is wired to it.
    fn wire(&self, name: &str, at: Option<f32>, depth: usize) -> Recipe {
        let inner = match self.patch.node(name).map(|n| &n.body) {
            Some(Body::Signal(comp)) if depth < MAX_DEPTH => match (at, comp.items.as_slice()) {
                // `node@x` scrubs the sequence the node is.
                (
                    Some(_),
                    [
                        Item {
                            what: What::Source(source),
                            ..
                        },
                    ],
                ) => self.source(source, name, at),
                _ => self.comp(comp, name, depth + 1),
            },
            // A leaf, a Scalar, an undefined name, a loop: nothing to draw.
            _ => Recipe::Shapes(Vec::new()),
        };
        Recipe::Named(name.to_owned(), Box::new(inner))
    }

    fn source(&self, source: &Source, clip: &str, at: Option<f32>) -> Recipe {
        let arg = |i: usize| source.args.get(i).map_or(0.0, |a| self.eval(a));
        let number = |kind| {
            source
                .modifier(kind)
                .and_then(|m| m.args.first())
                .map(|a| self.eval(a))
        };
        let mut place = [0.0, 0.0];
        if let Some(m) = source.modifier(ModKind::At) {
            place = [self.eval(&m.args[0]), self.eval(&m.args[1])];
        }
        place[0] += number(ModKind::X).unwrap_or(0.0);
        place[1] += number(ModKind::Y).unwrap_or(0.0);
        let alpha = number(ModKind::Alpha).unwrap_or(1.0).clamp(0.0, 1.0);

        let fit = match source.modifier(ModKind::Fit) {
            Some(_) => Fit::Frame,
            None => Fit::Unit,
        };
        if source.kind == SourceKind::Video {
            // Without a decoder a video draws nothing (`vybe check` says so).
            return match self.videos.get(clip).filter(|_| self.decoder.is_some()) {
                Some(video) => Recipe::Stream(Stream {
                    slot: video.slot.clone(),
                    fit,
                    place,
                    size: number(ModKind::Size).unwrap_or(1.0),
                    alpha,
                }),
                None => Recipe::Shapes(Vec::new()),
            };
        }
        if vocabulary::source_word(source.kind).family != Family::Shape {
            // `text` draws nothing yet (`vybe check` says so).
            let Some(clip) = self.sequences.get(clip).filter(|c| !c.frames.is_empty()) else {
                return Recipe::Shapes(Vec::new());
            };
            let last = clip.frames.len() - 1;
            return Recipe::Image(Image {
                frames: clip.frames.clone(),
                index: at.map_or(clip.index(), |at| (at * last as f32).round() as usize),
                fit,
                place,
                size: number(ModKind::Size).unwrap_or(1.0),
                alpha,
            });
        }

        let width = number(ModKind::Stroke);
        let mut stroke = match source.kind {
            SourceKind::Rect => Stroke {
                form: crate::recipe::Form::Rect,
                extent: [arg(0), arg(1)],
                outline: width.unwrap_or(0.0),
                ..Stroke::IDENTITY
            },
            SourceKind::Line => {
                Stroke::line([arg(0), arg(1)], [arg(2), arg(3)], width.unwrap_or(0.004))
            }
            _ => Stroke {
                radius: arg(0),
                outline: width.unwrap_or(0.0),
                ..Stroke::IDENTITY
            },
        };
        // A line already sits at its midpoint; `at` moves it from there.
        stroke.place.point[0] += place[0];
        stroke.place.point[1] += place[1];
        stroke.alpha = alpha;
        stroke.soft = number(ModKind::Soft).unwrap_or(0.0);
        stroke.value = number(ModKind::Gray).unwrap_or(1.0);
        if let Some(hue) = number(ModKind::Hue) {
            stroke.sat = 1.0;
            stroke.hue = Hue {
                base: hue,
                drift: number(ModKind::Drift).unwrap_or(0.0),
            };
        }
        if let Some(m) = source.modifier(ModKind::Wave) {
            stroke.wave.amp = self.eval(&m.args[0]);
            stroke.wave.x = self.eval(&m.args[1]);
            stroke.wave.y = self.eval(&m.args[2]);
        }
        if let Some(m) = source.modifier(ModKind::Grid) {
            stroke.cols = (self.eval(&m.args[0]) as u32).max(1);
            stroke.rows = (self.eval(&m.args[1]) as u32).max(1);
        }
        Recipe::Shapes(vec![stroke])
    }
}

/// When the file was last written, if it can be read right now.
fn modified(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Strokes with an item's opacity folded into them.
fn faded(mut strokes: Vec<Stroke>, alpha: f32) -> Vec<Stroke> {
    for stroke in &mut strokes {
        stroke.alpha *= alpha;
    }
    strokes
}

/// The sequences `comp` reaches, through its own sources and its wires.
fn reach(patch: &Patch, owner: &str, comp: &Comp, out: &mut Vec<String>, depth: usize) {
    let single = comp.items.len() == 1;
    for (index, item) in comp.items.iter().enumerate() {
        match &item.what {
            What::Source(_) => out.push(clip_key(owner, index, single)),
            What::Wire(name) if depth < MAX_DEPTH => {
                if let Some(Body::Signal(inner)) = patch.node(name).map(|n| &n.body) {
                    reach(patch, name, inner, out, depth + 1);
                }
            }
            What::Wire(_) => {}
        }
    }
}

impl Perform for Player {
    fn frame(&mut self, inputs: &Inputs, time: f32, dt: f32) -> Recipe {
        self.advance(inputs, time, dt);
        self.describe()
    }

    fn offline(&mut self) {
        self.pace = Pace::Offline;
    }

    fn media(&self) -> Vec<PathBuf> {
        self.sequences
            .values()
            .flat_map(|clip| clip.frames.iter().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::parse;

    const DT: f32 = 1.0 / 60.0;

    /// A performer's script: run the patch, feeding `/hands` from a timeline.
    struct Run {
        player: Player,
        inputs: Inputs,
        frame: u32,
    }

    impl Run {
        fn new(text: &str, base: &Path) -> Self {
            Self {
                player: Player::new(parse(text).expect("parses"), base),
                inputs: Inputs::default(),
                frame: 0,
            }
        }

        fn set(&mut self, address: &str, value: &str) {
            self.inputs.set(address, Value::parse(value));
        }

        fn run(&mut self, seconds: f32) {
            for _ in 0..(seconds * 60.0).round() as u32 {
                self.player
                    .advance(&self.inputs, self.frame as f32 * DT, DT);
                self.frame += 1;
            }
        }
    }

    /// A folder of empty files named like frames — the player never opens them.
    fn fake_frames(name: &str, count: usize) -> PathBuf {
        let base = std::env::temp_dir().join(format!("vybe-play-{name}-{}", std::process::id()));
        let dir = base.join("clip");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 1..=count {
            std::fs::write(dir.join(format!("{i:04}.png")), []).unwrap();
        }
        base
    }

    const FACE: &str = "hands = osc /hands debounce .08\n\
                        t     = ramp hands up 3s down 1.2s\n\
                        ss    = circle .1 soft 1\n\
                        clip  = frames clip/*.png\n\
                        touch = ss*(1-t)\n\
                        idle  : ss\n\
                        touch : touch\n\
                        play  : clip\n\
                        idle  -> touch  hands rise         cut\n\
                        touch -> play   t = 1              cut\n\
                        touch -> idle   t = 0 & hands off  cut\n\
                        play  -> idle   clip done          fade 1s";

    #[test]
    fn holding_hands_walks_idle_touch_play_and_back() {
        let base = fake_frames("walk", 60); // 2 s at 30 fps
        let mut run = Run::new(FACE, &base);
        run.run(1.0);
        assert_eq!(run.player.scene(), "idle");

        run.set("/hands", "1");
        run.run(0.5);
        assert_eq!(run.player.scene(), "touch");
        assert!(run.player.value("t") > 0.1);

        run.run(3.0); // the ramp reaches 1
        assert_eq!(run.player.scene(), "play");

        run.run(2.1); // the clip plays out, the fade home begins
        assert_eq!(run.player.scene(), "idle");
        assert_eq!(run.player.fader.visible().len(), 2);
        run.run(1.1);
        assert_eq!(run.player.fader.visible().len(), 1);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn letting_go_early_falls_back_to_idle() {
        let base = fake_frames("early", 60);
        let mut run = Run::new(FACE, &base);
        run.set("/hands", "1");
        run.run(1.0);
        assert_eq!(run.player.scene(), "touch");
        run.set("/hands", "0");
        run.run(0.2);
        assert_eq!(run.player.scene(), "touch"); // still falling
        run.run(1.5);
        assert_eq!(run.player.scene(), "idle");
        assert_eq!(run.player.value("t"), 0.0);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_play_once_clip_starts_over_each_time_its_scene_is_entered() {
        let base = fake_frames("again", 30); // 1 s
        let mut run = Run::new(FACE, &base);
        for _ in 0..2 {
            run.set("/hands", "1");
            run.run(3.2);
            assert_eq!(run.player.scene(), "play");
            assert!(run.player.sequences["clip"].playhead < 0.5);
            run.set("/hands", "0");
            run.run(3.0);
            assert_eq!(run.player.scene(), "idle");
        }
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_wildcard_fires_once_when_the_symbol_arrives_and_does_not_pin_the_scene() {
        let text = "mode = osc /mode\n\
                    go   = osc /go\n\
                    a    = circle .1\n\
                    grid = rect 1 1 stroke .01\n\
                    one  : a\n\
                    two  : a\n\
                    one -> two   go rise       cut\n\
                    * -> grid    mode = grid   cut\n\
                    * -> one     mode = show   cut";
        let mut run = Run::new(text, Path::new("."));
        run.set("/mode", "grid");
        run.run(0.1);
        assert_eq!(run.player.scene(), "grid"); // a node, used as a scene
        run.set("/mode", "show");
        run.run(0.1);
        assert_eq!(run.player.scene(), "one");
        // `mode` still reads `show`, yet the patch is free to move on.
        run.set("/go", "1");
        run.run(0.1);
        assert_eq!(run.player.scene(), "two");
    }

    #[test]
    fn an_edit_keeps_the_performance_where_it_was() {
        let base = fake_frames("reload", 60);
        let mut run = Run::new(FACE, &base);
        run.set("/hands", "1");
        run.run(1.5);
        assert_eq!(run.player.scene(), "touch");
        let t = run.player.value("t");
        let playhead = run.player.sequences["clip"].playhead;

        // Retime the ramp, recolour a node, add a scene: a live edit.
        let edited = FACE
            .replace("up 3s down 1.2s", "up 1.75s down 1.2s")
            .replace("circle .1 soft 1", "circle .2 soft 1 hue 40")
            + "\nextra : ss";
        run.player.reload(parse(&edited).expect("parses"));

        // Same scene, same ramp value, same playhead — nothing restarted…
        assert_eq!(run.player.scene(), "touch");
        assert_eq!(run.player.value("t"), t);
        assert_eq!(run.player.sequences["clip"].playhead, playhead);
        // …and the new timing is live. From t ≈ .47 the ramp needs ≈ .92 s at
        // the new `up 1.75s`; at the old 3 s it would need 1.58 s.
        run.run(1.0);
        assert_eq!(run.player.scene(), "play");
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn editing_a_condition_does_not_fire_it() {
        let text = "go = osc /go\na = circle .1\nb = circle .2\none : a\ntwo : b\none -> two  go rise  cut";
        let mut run = Run::new(text, Path::new("."));
        run.set("/go", "1");
        run.run(0.1);
        assert_eq!(run.player.scene(), "two");
        // While `go` is still held, a new way back appears whose condition is
        // already true. It must wait to *become* true.
        let edited = format!("{text}\ntwo -> one  go = 1  cut");
        run.player.reload(parse(&edited).expect("parses"));
        run.run(0.1);
        assert_eq!(run.player.scene(), "two");
        run.set("/go", "0");
        run.run(0.1);
        run.set("/go", "1");
        run.run(0.1);
        assert_eq!(run.player.scene(), "one");
    }

    #[test]
    fn a_node_that_changes_kind_starts_over_instead_of_panicking() {
        let mut run = Run::new(
            "g = osc /g debounce .01\nx = ramp g up 1s down 1s\na = circle .1",
            Path::new("."),
        );
        run.set("/g", "1");
        run.run(0.5);
        // `x` was a ramp; now it is a gate.
        run.player.reload(
            parse("g = osc /g debounce .01\nx = osc /g debounce .01\na = circle .1")
                .expect("parses"),
        );
        run.run(0.1);
        assert_eq!(run.player.value("x"), 1.0);
    }

    #[test]
    fn a_saved_file_reloads_and_a_broken_save_changes_nothing() {
        let dir = std::env::temp_dir().join(format!("vybe-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("live.vy");
        let save = |text: &str| {
            std::fs::write(&file, text).unwrap();
            // Make sure the timestamp moves even on a coarse clock.
            let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
            std::fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(later)
                .unwrap();
        };
        std::fs::write(&file, "a = circle .1").unwrap();
        let patch = parse("a = circle .1").unwrap();
        let mut run = Run {
            player: Player::new(patch, &dir).watch(&file),
            inputs: Inputs::default(),
            frame: 0,
        };
        run.run(0.5);
        assert_eq!(run.player.scene(), "a");

        save("a = circle .1\nb = circle .2 hue 40");
        run.run(0.5);
        assert_eq!(run.player.scene(), "b"); // the new last node shows

        save("a = circle .1\nb = circle sofft"); // a typo, mid-thought
        run.run(0.5);
        assert_eq!(run.player.scene(), "b"); // the last good patch plays on
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A clip that decodes nothing and remembers everything it was told.
    #[derive(Default)]
    struct Told {
        paused: bool,
        volume: f32,
        restarts: u32,
        /// Seconds of "file" left before it reports `done`.
        ends_after: f32,
        asked_until: f32,
    }

    struct FakeClip(Rc<std::cell::RefCell<Told>>);

    impl Clip for FakeClip {
        fn restart(&mut self) {
            let mut told = self.0.borrow_mut();
            told.restarts += 1;
            told.asked_until = 0.0;
        }
        fn set_paused(&mut self, paused: bool) {
            self.0.borrow_mut().paused = paused;
        }
        fn set_volume(&mut self, volume: f32) {
            self.0.borrow_mut().volume = volume;
        }
        fn done(&mut self) -> bool {
            let told = self.0.borrow();
            told.asked_until >= told.ends_after
        }
        fn frame(&mut self, time: f32) -> Option<crate::clip::Frame> {
            self.0.borrow_mut().asked_until = time;
            None
        }
    }

    struct FakeDecoder(Rc<std::cell::RefCell<Told>>);

    impl Decoder for FakeDecoder {
        fn open(&self, _: &Path, _: bool, _: Pace) -> Result<Box<dyn Clip>, String> {
            Ok(Box::new(FakeClip(self.0.clone())))
        }
    }

    #[test]
    fn a_video_plays_only_while_seen_as_loud_as_its_scene_and_ends_it() {
        let text = "go   = osc /go\n\
                    dot  = circle .1\n\
                    film = video film.mp4 vol .8\n\
                    idle : dot\n\
                    play : film\n\
                    idle -> play  go rise    fade 1s\n\
                    play -> idle  film done  fade 1s";
        let told = Rc::new(std::cell::RefCell::new(Told {
            ends_after: 3.0,
            ..Told::default()
        }));
        let mut run = Run::new(text, Path::new("."));
        run.player = run.player.decoder(FakeDecoder(told.clone()));

        // Unseen: opened, but held and silent.
        run.run(0.5);
        assert!(told.borrow().paused);
        assert_eq!(told.borrow().volume, 0.0);

        // Its scene fades in: it starts from the top, and its sound fades in too.
        run.set("/go", "1");
        run.run(0.5);
        assert_eq!(run.player.scene(), "play");
        assert_eq!(told.borrow().restarts, 1);
        assert!(!told.borrow().paused);
        let halfway = told.borrow().volume;
        assert!(halfway > 0.2 && halfway < 0.6, "{halfway}"); // ≈ .8 × ½
        run.run(1.0);
        assert!((told.borrow().volume - 0.8).abs() < 1e-3);

        // It plays out: `film done` takes the face home, and the sound with it.
        run.run(2.0);
        assert_eq!(run.player.scene(), "idle");
        run.run(1.2);
        assert!(told.borrow().paused);
        assert!(told.borrow().volume < 1e-3);
    }

    #[test]
    fn a_video_without_a_decoder_draws_nothing_and_breaks_nothing() {
        let mut run = Run::new("film = video film.mp4", Path::new("."));
        run.run(0.2);
        let Recipe::Composite(scenes) = run.player.describe() else {
            panic!("scenes composite");
        };
        assert_eq!(scenes.len(), 1);
    }

    #[test]
    fn a_patch_without_scenes_shows_its_last_node() {
        let run = Run::new("a = circle .1\nb = circle .2 hue 40", Path::new("."));
        assert_eq!(run.player.scene(), "b");
    }

    #[test]
    fn a_node_wired_into_two_scenes_is_one_node() {
        let base = fake_frames("shared", 4);
        let mut run = Run::new(FACE, &base);
        run.set("/hands", "1");
        run.run(0.5);
        // `touch` shows `ss*(1-t)`: the very `ss` idle shows, by name.
        let Recipe::Composite(scenes) = run.player.describe() else {
            panic!("scenes composite");
        };
        let Recipe::Named(key, inner) = &scenes[0].recipe else {
            panic!("a scene is keyed");
        };
        assert_eq!(key, "scene:touch");
        let Recipe::Named(node, inner) = &**inner else {
            panic!("the scene is the node `touch`");
        };
        assert_eq!(node, "touch");
        let Recipe::Composite(layers) = &**inner else {
            panic!("an item with opacity is a layer");
        };
        assert!(matches!(&layers[0].recipe, Recipe::Named(name, _) if name == "ss"));
        assert!(layers[0].alpha < 1.0);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn an_oscillator_plugs_into_a_position() {
        let text = "dot = circle .05 y osc .25hz .1";
        let mut run = Run::new(text, Path::new("."));
        run.run(1.0); // a quarter cycle of .25 Hz lands near the peak
        let Recipe::Composite(scenes) = run.player.describe() else {
            panic!("scenes composite");
        };
        let Recipe::Named(_, inner) = &scenes[0].recipe else {
            panic!("keyed");
        };
        let Recipe::Named(_, inner) = &**inner else {
            panic!("the node");
        };
        let Recipe::Shapes(strokes) = &**inner else {
            panic!("one shape");
        };
        assert!((strokes[0].place.point[1] - 0.1).abs() < 0.01);
    }
}
