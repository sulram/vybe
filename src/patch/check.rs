//! `vybe check` — everything that can be known about a patch without running
//! it: undefined names, a Scalar where a Signal belongs, `@` on something with
//! no timeline, missing media, a scene with no way out. Runs before any GPU
//! work, and every finding says what to do next.

use std::collections::HashSet;
use std::path::Path;

use super::vocabulary::{self, EffectKind, Family, ModKind, SourceKind};
use super::{
    Body, Comp, Diagnostic, Expr, Item, OutKind, Patch, Scalar, Severity, Source, Term, What,
    did_you_mean, resolve_frames,
};

/// Reads `patch` for mistakes. Media paths resolve against `base` (the patch's
/// own folder). Sorted by line; [`Severity::Error`] means it will not run.
pub fn check(patch: &Patch, base: &Path) -> Vec<Diagnostic> {
    let mut checker = Checker {
        patch,
        base,
        found: Vec::new(),
    };
    checker.names();
    for node in &patch.nodes {
        checker.node(node);
    }
    for scene in &patch.scenes {
        checker.comp(&scene.comp, scene.line);
    }
    checker.cycles();
    checker.transitions();
    checker.output();
    let mut found = checker.found;
    found.sort_by_key(|d| d.line);
    found
}

/// True when any finding stops the patch from running.
pub fn has_errors(found: &[Diagnostic]) -> bool {
    found.iter().any(|d| d.severity == Severity::Error)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Scalar,
    Signal,
}

struct Checker<'a> {
    patch: &'a Patch,
    base: &'a Path,
    found: Vec<Diagnostic>,
}

impl Checker<'_> {
    fn kind(&self, name: &str) -> Option<Kind> {
        self.patch.node(name).map(|node| match node.body {
            Body::Scalar(_) => Kind::Scalar,
            Body::Signal(_) | Body::Leaf(_) => Kind::Signal,
        })
    }

    /// An undefined name, with the closest defined one as the next step.
    fn undefined(&mut self, line: usize, name: &str) {
        let known = self.patch.nodes.iter().map(|n| n.name.as_str());
        let mut e = Diagnostic::error(line, format!("`{name}` is not defined"));
        e = match did_you_mean(name, known) {
            Some(close) => e.help(format!("did you mean `{close}`?")),
            None => e.help(format!("define it above:  {name} = ...")),
        };
        self.found.push(e);
    }

    fn names(&mut self) {
        let mut seen = HashSet::new();
        for node in &self.patch.nodes {
            if !seen.insert(&node.name) {
                self.found.push(
                    Diagnostic::error(node.line, format!("`{}` is defined twice", node.name))
                        .help("a name is one node; rename one of them"),
                );
            }
        }
        let mut seen = HashSet::new();
        for scene in &self.patch.scenes {
            if !seen.insert(&scene.name) {
                self.found.push(Diagnostic::error(
                    scene.line,
                    format!("scene `{}` is declared twice", scene.name),
                ));
            }
        }
        let shows_something = self
            .patch
            .nodes
            .iter()
            .any(|n| !matches!(n.body, Body::Scalar(_)));
        if !shows_something && self.patch.scenes.is_empty() {
            self.found.push(
                Diagnostic::error(1, "this patch has nothing to show")
                    .help("add a node that draws:  dot = circle .2 hue 200"),
            );
        } else if self.patch.scenes.is_empty() {
            if let Some(last) = self
                .patch
                .nodes
                .iter()
                .rev()
                .find(|n| !matches!(n.body, Body::Scalar(_)))
            {
                self.found.push(Diagnostic::note(
                    last.line,
                    format!(
                        "no scenes declared: `{}` (the last node that draws) is what shows",
                        last.name
                    ),
                ));
            }
        }
    }

    fn node(&mut self, node: &super::Node) {
        match &node.body {
            Body::Scalar(Scalar::Input { debounce, .. }) => {
                if let Some(debounce) = debounce {
                    self.number(debounce, node.line);
                }
            }
            Body::Scalar(Scalar::Ramp { gate, up, down }) => {
                match self.kind(gate) {
                    Some(Kind::Scalar) => {}
                    Some(Kind::Signal) => self.found.push(
                        Diagnostic::error(
                            node.line,
                            format!("`ramp` is driven by a Scalar, but `{gate}` draws"),
                        )
                        .help("drive it with an input:  hands = osc /hands debounce .08"),
                    ),
                    None => self.undefined(node.line, gate),
                }
                self.number(up, node.line);
                self.number(down, node.line);
            }
            Body::Scalar(Scalar::Value(expr)) => self.number(expr, node.line),
            Body::Signal(comp) => self.comp(comp, node.line),
            Body::Leaf(name) => self.found.push(
                Diagnostic::warning(
                    node.line,
                    format!("`rust {name}`: leaves are not wired yet — it draws nothing"),
                )
                .help("they arrive with Draw/canvas(); until then compose it from sources"),
            ),
        }
    }

    /// Every wire inside a number socket must be a Scalar.
    fn number(&mut self, expr: &Expr, line: usize) {
        match expr {
            Expr::Num(_) | Expr::Px(_) => {}
            Expr::Wire(name) => match self.kind(name) {
                Some(Kind::Scalar) => {}
                Some(Kind::Signal) => self.found.push(
                    Diagnostic::error(line, format!("`{name}` draws; a number is wanted here"))
                        .help("wire a Scalar instead (an `osc`, a `ramp`, or an expression)"),
                ),
                None => self.undefined(line, name),
            },
            Expr::Neg(a) | Expr::Smooth(a) => self.number(a, line),
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) => {
                self.number(a, line);
                self.number(b, line);
            }
            Expr::Osc { hz, amp } => {
                self.number(hz, line);
                self.number(amp, line);
            }
        }
    }

    fn comp(&mut self, comp: &Comp, line: usize) {
        for item in &comp.items {
            self.item(item, line);
        }
        if let Some(effect) = &comp.effect {
            for (_, value) in &effect.params {
                self.number(value, line);
            }
            let EffectKind::Feedback = effect.kind;
            let feeds_on_a_shape = matches!(
                comp.items.as_slice(),
                [Item { what: What::Source(s), .. }] if vocabulary::source_word(s.kind).family == Family::Shape
            );
            if !feeds_on_a_shape {
                self.found.push(
                    Diagnostic::error(line, "`| feedback` takes one shape as its source, for now")
                        .help("aura = circle .06 soft 1 | feedback decay .3\n(feedback over a composition waits for the sketch that needs it)"),
                );
            }
        }
    }

    fn item(&mut self, item: &Item, line: usize) {
        match &item.what {
            What::Wire(name) => match self.kind(name) {
                Some(Kind::Signal) => {}
                Some(Kind::Scalar) => self.found.push(
                    Diagnostic::error(
                        line,
                        format!(
                            "`{name}` is a Scalar — a number, not a picture; it can't be stacked"
                        ),
                    )
                    .help(format!("use it as a number:  a * {name}   or   a @ {name}")),
                ),
                None => self.undefined(line, name),
            },
            What::Source(source) => self.source(source, line),
        }
        if let Some(alpha) = &item.alpha {
            self.number(alpha, line);
        }
        if let Some(at) = &item.at {
            self.number(at, line);
            if self.timeline(&item.what).is_none() {
                self.found.push(
                    Diagnostic::error(line, "`@` positions something in time, but this has no timeline")
                        .help("`@` follows a `frames` source, or the name of a node that is one:  trans@smooth(t)"),
                );
            }
        }
    }

    /// The media source behind `what`, if it has a timeline.
    fn timeline<'p>(&'p self, what: &'p What) -> Option<&'p Source> {
        let source = match what {
            What::Source(source) => source,
            What::Wire(name) => match &self.patch.node(name)?.body {
                Body::Signal(Comp {
                    items,
                    effect: None,
                }) => match items.as_slice() {
                    [
                        Item {
                            what: What::Source(source),
                            ..
                        },
                    ] => source,
                    _ => return None,
                },
                _ => return None,
            },
        };
        (vocabulary::source_word(source.kind).family == Family::Media).then_some(source)
    }

    fn source(&mut self, source: &Source, line: usize) {
        for arg in &source.args {
            self.number(arg, line);
        }
        for modifier in &source.mods {
            for arg in &modifier.args {
                self.number(arg, line);
            }
            if matches!(modifier.kind, ModKind::Mute | ModKind::Vol) {
                self.found.push(Diagnostic::note(
                    line,
                    "audio arrives with vybe-audio; `mute`/`vol` are read and ignored for now",
                ));
            }
        }
        let text = source.text.as_deref().unwrap_or_default();
        match source.kind {
            SourceKind::Frames => {
                if resolve_frames(self.base, text).is_empty() {
                    self.found.push(
                        Diagnostic::error(line, format!("no files match `{text}` (looked in {})", self.base.display()))
                            .help(format!(
                                "a sequence is numbered PNGs:  {}\nfrom a video:   ffmpeg -i clip.mp4 {}\nfrom a patch:   vybe render clip.vy --seq 4s --fps 30 --alpha --out {}",
                                text.replace('*', "0001"),
                                text.replace('*', "%04d"),
                                Path::new(text).parent().map_or_else(|| ".".into(), |p| p.display().to_string()),
                            )),
                    );
                }
            }
            SourceKind::Video => {
                let stem = Path::new(text).with_extension("");
                let stem = stem.display();
                self.found.push(
                    Diagnostic::error(line, format!("`video {text}`: video decoding arrives with vybe-video (0.0.3)"))
                        .help(format!(
                            "until then a PNG sequence plays the same way:\n    ffmpeg -i {text} -vf fps=30 {stem}/%04d.png\n    ... = frames {stem}/*.png loop"
                        )),
                );
            }
            SourceKind::Text => self.found.push(
                Diagnostic::warning(line, "`text` arrives with Draw — it draws nothing yet")
                    .help("the rest of the line still shows"),
            ),
            SourceKind::Circle | SourceKind::Rect | SourceKind::Line => {}
        }
    }

    /// A node wired into itself, however far around.
    fn cycles(&mut self) {
        fn wires(comp: &Comp) -> impl Iterator<Item = &str> {
            comp.items.iter().filter_map(|item| match &item.what {
                What::Wire(name) => Some(name.as_str()),
                What::Source(_) => None,
            })
        }
        fn visit<'p>(patch: &'p Patch, name: &'p str, path: &mut Vec<&'p str>) -> bool {
            if path.contains(&name) {
                path.push(name);
                return true;
            }
            let Some(Body::Signal(comp)) = patch.node(name).map(|n| &n.body) else {
                return false;
            };
            path.push(name);
            if wires(comp).any(|next| visit(patch, next, path)) {
                return true;
            }
            path.pop();
            false
        }
        for node in &self.patch.nodes {
            let mut path = Vec::new();
            if visit(self.patch, &node.name, &mut path) && path.last() == Some(&node.name.as_str())
            {
                self.found.push(
                    Diagnostic::error(
                        node.line,
                        format!(
                            "`{}` is wired into itself: {}",
                            node.name,
                            path.join(" -> ")
                        ),
                    )
                    .help("a loop in time is `| feedback`; a loop in wires can't be drawn"),
                );
            }
        }
    }

    /// A scene is one that was declared, or any node that draws (a transition
    /// may go straight to it).
    fn is_scene(&self, name: &str) -> bool {
        self.patch.scene(name).is_some() || self.kind(name) == Some(Kind::Signal)
    }

    fn transitions(&mut self) {
        let patch = self.patch;
        for t in &patch.transitions {
            for end in t.from.iter().chain(Some(&t.to)) {
                if !self.is_scene(end) {
                    let known = patch.scenes.iter().map(|s| s.name.as_str());
                    let mut e = Diagnostic::error(t.line, format!("`{end}` is not a scene"));
                    e = match did_you_mean(end, known) {
                        Some(close) => e.help(format!("did you mean `{close}`?")),
                        None => e.help(format!("declare it:  {end} : <node>")),
                    };
                    self.found.push(e);
                }
            }
            for term in &t.cond {
                let name = term.node();
                match (term, self.kind(name)) {
                    (_, None) => self.undefined(t.line, name),
                    (Term::Done(_), Some(Kind::Scalar)) => self.found.push(
                        Diagnostic::error(t.line, format!("`{name} done`: a Scalar never ends"))
                            .help("`done` is for a sequence that plays once:  clip = frames clip/*.png"),
                    ),
                    (Term::Done(_), Some(Kind::Signal)) => {
                        let plays_once = self
                            .timeline(&What::Wire(name.to_owned()))
                            .is_some_and(|s| s.modifier(ModKind::Loop).is_none());
                        if !plays_once {
                            self.found.push(
                                Diagnostic::error(t.line, format!("`{name} done` never happens: `{name}` is not a sequence that plays once"))
                                    .help("`done` follows a `frames` node without `loop`"),
                            );
                        }
                    }
                    (_, Some(Kind::Signal)) => self.found.push(
                        Diagnostic::error(t.line, format!("`{name}` draws; a condition reads a Scalar"))
                            .help("conditions:  gate rise  ·  gate off  ·  clip done  ·  scalar = N"),
                    ),
                    (_, Some(Kind::Scalar)) => {}
                }
            }
            if let Some(fade) = &t.fade {
                self.number(fade, t.line);
            }
            if t.sound.is_some() {
                self.found.push(Diagnostic::note(
                    t.line,
                    "audio arrives with vybe-audio; the transition's sound is read and ignored for now",
                ));
            }
        }

        // A scene with no way out is a performance that gets stuck.
        let mut scenes: Vec<(&str, usize)> = patch
            .scenes
            .iter()
            .map(|s| (s.name.as_str(), s.line))
            .collect();
        for t in &patch.transitions {
            if !scenes.iter().any(|(name, _)| *name == t.to) {
                scenes.push((&t.to, t.line));
            }
        }
        if scenes.len() > 1 {
            for (scene, line) in scenes {
                let has_exit = patch.transitions.iter().any(|t| match &t.from {
                    Some(from) => from == scene && t.to != scene,
                    None => t.to != scene,
                });
                if !has_exit {
                    self.found.push(
                        Diagnostic::warning(line, format!("scene `{scene}` has no way out")).help(
                            format!("add a transition from it:  {scene} -> <scene>  <cond>  cut"),
                        ),
                    );
                }
            }
        }
    }

    fn output(&mut self) {
        let Some(out) = &self.patch.out else {
            return;
        };
        if out.kind == OutKind::Kms && !cfg!(target_os = "linux") {
            self.found.push(Diagnostic::note(
                out.line,
                "`out kms` needs Linux KMS/DRM (vybe-stage-drm, 0.0.3); here it opens a window of the same shape",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::parse;

    fn findings(text: &str) -> Vec<Diagnostic> {
        check(&parse(text).expect("parses"), Path::new("."))
    }

    fn errors(text: &str) -> Vec<Diagnostic> {
        findings(text)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect()
    }

    #[test]
    fn a_clean_patch_has_no_errors() {
        let text = "hands = osc /key/space debounce .05\n\
                    t = ramp hands up 1s down 1s\n\
                    a = circle .2 hue 200\n\
                    b = rect .4 .4 * t\n\
                    one : a\n\
                    two : a + b\n\
                    one -> two  hands rise  fade 1s\n\
                    two -> one  hands off   cut";
        assert_eq!(errors(text), vec![]);
    }

    #[test]
    fn an_undefined_name_suggests_the_close_one() {
        let e = errors("aura = circle .1\nidle : aurra");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].help.as_deref(), Some("did you mean `aura`?"));
    }

    #[test]
    fn a_scalar_cannot_be_stacked_and_a_signal_is_not_a_number() {
        let e = errors("t = osc /x\na = circle .1\nb = a + t\nc = circle .1 * a");
        assert_eq!(e.len(), 2);
        assert!(e[0].message.contains("Scalar"));
        assert!(e[1].message.contains("draws"));
    }

    #[test]
    fn at_needs_a_timeline() {
        let e = errors("t = osc /x\na = circle .1\nb = a@t");
        assert!(e.iter().any(|d| d.message.contains("no timeline")), "{e:?}");
    }

    #[test]
    fn video_teaches_the_ffmpeg_line() {
        let e = errors("v = video agua.mp4 loop");
        assert!(e[0].help.as_ref().unwrap().contains("ffmpeg -i agua.mp4"));
    }

    #[test]
    fn missing_frames_say_how_to_make_them() {
        let e = errors("f = frames nowhere/*.png");
        assert!(e[0].message.contains("no files match"));
        assert!(e[0].help.as_ref().unwrap().contains("vybe render"));
    }

    #[test]
    fn a_wire_loop_is_named() {
        let e = errors("a = b + b\nb = a");
        assert!(
            e.iter().any(|d| d.message.contains("wired into itself")),
            "{e:?}"
        );
    }

    #[test]
    fn a_scene_with_no_way_out_is_a_warning() {
        let found = findings(
            "g = osc /g\na = circle .1\nb = circle .2\none : a\ntwo : b\none -> two g rise cut",
        );
        assert!(
            found
                .iter()
                .any(|d| d.severity == Severity::Warning
                    && d.message.contains("`two` has no way out"))
        );
    }

    #[test]
    fn a_wildcard_is_a_way_out_of_everything_but_its_target() {
        let found = findings(
            "m = osc /mode\na = circle .1\nb = circle .2\none : a\n* -> b  m = b  cut\n* -> one  m = show  cut",
        );
        assert!(
            !found.iter().any(|d| d.message.contains("no way out")),
            "{found:?}"
        );
    }

    #[test]
    fn feedback_over_a_composition_is_not_yet() {
        let e = errors("a = circle .1\nb = a + a | feedback decay .2");
        assert!(e[0].message.contains("one shape"));
    }
}
