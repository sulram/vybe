//! THE PATCH — a `.vy` file: the work, as plain text. One line per node, wires
//! by name, scenes and transitions, one output.
//!
//! ```text
//! hands = osc /hands            debounce .08
//! t     = ramp hands            up 3s  down 1.2s
//! aura  = circle .06 soft 1     hue 160 drift 6
//!       | feedback decay .3 angle .2 scale .9
//! idle  : aura
//! touch : aura*(1-t)
//! idle  -> touch   hands rise   cut
//! ```
//!
//! This module is the second front-end over the core, beside the Rust sugar:
//! [`parse`] turns the text into a [`Patch`] (the AST, plain data), [`check`]
//! reads it for mistakes before any GPU work, and a [`Player`] performs it —
//! evaluating its Scalars each frame and handing the core the same `Recipe` a
//! Rust chain does. std-only.
//!
//! **The vocabulary is closed** ([`vocabulary`]): it grows only with drawing
//! primitives and Scalar operators, never with application nouns. The escape
//! for everything else is `rust <name>`.

mod check;
mod parse;
mod play;
pub mod vocabulary;

use std::fmt;
use std::path::{Path, PathBuf};

pub use check::{Support, check, check_with, has_errors};
pub use parse::parse;
pub use play::Player;
pub use vocabulary::{EffectKind, ModKind, SourceKind};

use crate::input::Value;
use crate::sugar::Blend;

/// A parsed `.vy` file. Plain data: what the text said, nothing resolved.
#[derive(Clone, Debug, Default)]
pub struct Patch {
    /// In file order.
    pub nodes: Vec<Node>,
    pub scenes: Vec<Scene>,
    pub transitions: Vec<Transition>,
    pub out: Option<Out>,
}

impl Patch {
    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }

    pub fn scene(&self, name: &str) -> Option<usize> {
        self.scenes.iter().position(|s| s.name == name)
    }
}

/// `name = …` — one line of the patch.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub line: usize,
    pub body: Body,
}

#[derive(Clone, Debug)]
pub enum Body {
    /// A value over time.
    Scalar(Scalar),
    /// A texture over time: a source, or a composition of sources and wires.
    Signal(Comp),
    /// `rust <fn>` — an imperative leaf, registered at compile time.
    Leaf(String),
}

/// Where a Scalar comes from.
#[derive(Clone, Debug)]
pub enum Scalar {
    /// `osc /address [debounce s]` — a live input. With a debounce it is a
    /// gate (0 or 1); without, the raw number.
    Input {
        address: String,
        debounce: Option<Expr>,
    },
    /// `ramp <gate> up s down s` — 0..1, climbing while the gate is open.
    Ramp { gate: String, up: Expr, down: Expr },
    /// Arithmetic over other Scalars, or an oscillator (`osc .07hz .03`).
    Value(Expr),
}

/// A number, or what computes one each frame. Plugs into any numeric socket.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f32),
    /// Pixels, resolved against the output's shorter edge.
    Px(f32),
    /// Another node, by name.
    Wire(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    /// `smooth(x)` — ease both ends of 0..1.
    Smooth(Box<Expr>),
    /// `osc <hz> <amp>` — a sine around zero.
    Osc {
        hz: Box<Expr>,
        amp: Box<Expr>,
    },
}

/// `a + b*x add + c@t  | effect` — items stacked bottom to top.
#[derive(Clone, Debug)]
pub struct Comp {
    pub items: Vec<Item>,
    pub effect: Option<Effect>,
}

/// One term of a composition, with how it lands on what is beneath it.
#[derive(Clone, Debug)]
pub struct Item {
    pub what: What,
    /// `* x`
    pub alpha: Option<Expr>,
    /// `@ x` — position in time, 0..1.
    pub at: Option<Expr>,
    /// `add`, or the default over.
    pub blend: Blend,
}

#[derive(Clone, Debug)]
pub enum What {
    Wire(String),
    Source(Source),
}

/// A source word, its arguments, its modifiers.
#[derive(Clone, Debug)]
pub struct Source {
    pub kind: SourceKind,
    /// Numeric arguments (a circle's radius, a line's four coordinates).
    pub args: Vec<Expr>,
    /// The textual one (a path, a glob, a string), for the sources that take it.
    pub text: Option<String>,
    pub mods: Vec<Mod>,
}

impl Source {
    pub fn modifier(&self, kind: ModKind) -> Option<&Mod> {
        self.mods.iter().rev().find(|m| m.kind == kind)
    }
}

#[derive(Clone, Debug)]
pub struct Mod {
    pub kind: ModKind,
    pub args: Vec<Expr>,
}

/// `| feedback decay .3 angle .2 scale .9`
#[derive(Clone, Debug)]
pub struct Effect {
    pub kind: EffectKind,
    /// Named parameters, in the order written.
    pub params: Vec<(String, Expr)>,
}

/// `name : node`
#[derive(Clone, Debug)]
pub struct Scene {
    pub name: String,
    pub line: usize,
    pub comp: Comp,
}

/// `a -> b  cond  cut|fade Ns  [sound]`
#[derive(Clone, Debug)]
pub struct Transition {
    /// `None` is `*`: from any scene.
    pub from: Option<String>,
    pub to: String,
    /// Every term must hold (`&`).
    pub cond: Vec<Term>,
    /// `None` cuts.
    pub fade: Option<Expr>,
    pub sound: Option<String>,
    pub line: usize,
}

/// One condition. **A transition fires on the frame its whole condition
/// becomes true** while its `from` scene is showing — never again until it has
/// been false in between. (`rise` is already a one-frame edge; `off`, `done`
/// and `=` are levels; the edge is taken over their conjunction.)
#[derive(Clone, Debug, PartialEq)]
pub enum Term {
    /// The node went from closed to open this frame.
    Rise(String),
    /// The node is closed.
    Off(String),
    /// The node (a sequence) has played to its end.
    Done(String),
    /// The node equals a number (`t = 1`) or a symbol (`mode = grid`).
    Is(String, Value),
}

impl Term {
    pub fn node(&self) -> &str {
        match self {
            Term::Rise(n) | Term::Off(n) | Term::Done(n) | Term::Is(n, _) => n,
        }
    }
}

/// `out <output> args  picture WxH  keystone <file>  remote <port>`
#[derive(Clone, Debug, PartialEq)]
pub struct Out {
    pub kind: OutKind,
    /// A KMS connector (`HDMI-A-1`).
    pub connector: Option<String>,
    pub size: Option<[u32; 2]>,
    /// The picture's own size, when it isn't the output's: a square face on a
    /// 16:10 projector. The keystone's corners are then the picture's corners.
    pub picture: Option<[u32; 2]>,
    pub hz: Option<f32>,
    pub keystone: Option<String>,
    pub remote: Option<u16>,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutKind {
    Window,
    /// Straight to a display, no desktop (Linux KMS/DRM).
    Kms,
}

/// The files of a sequence: `glob` (a `*` in the file name) against `base`, in
/// name order — so numbered frames play in order.
pub(crate) fn resolve_frames(base: &Path, glob: &str) -> Vec<PathBuf> {
    let pattern = base.join(glob);
    let (Some(dir), Some(name)) = (pattern.parent(), pattern.file_name()) else {
        return Vec::new();
    };
    let name = name.to_string_lossy();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| crate::input::glob_match(&name, &e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect();
    files.sort();
    files
}

// ---------------------------------------------------------------------------
// Diagnostics — errors that teach the next step
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Works, but you should know.
    Note,
    /// Runs, but not as written.
    Warning,
    /// Will not run.
    Error,
}

/// One finding, tied to a line, with — wherever there is one — the next step.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub line: usize,
    pub severity: Severity,
    pub message: String,
    pub help: Option<String>,
}

impl Diagnostic {
    pub(crate) fn error(line: usize, message: impl Into<String>) -> Self {
        Self::new(line, Severity::Error, message)
    }

    pub(crate) fn warning(line: usize, message: impl Into<String>) -> Self {
        Self::new(line, Severity::Warning, message)
    }

    pub(crate) fn note(line: usize, message: impl Into<String>) -> Self {
        Self::new(line, Severity::Note, message)
    }

    fn new(line: usize, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            line,
            severity,
            message: message.into(),
            help: None,
        }
    }

    pub(crate) fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = match self.severity {
            Severity::Note => "note",
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        write!(f, "line {}: {severity}: {}", self.line, self.message)?;
        if let Some(help) = &self.help {
            for line in help.lines() {
                write!(f, "\n    help: {line}")?;
            }
        }
        Ok(())
    }
}

/// The closest of `known` to `word`, if any is close enough to be a typo.
pub(crate) fn did_you_mean<'a>(
    word: &str,
    known: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    known
        .into_iter()
        .map(|k| (distance(word, k), k))
        .filter(|(d, k)| *d <= 2 && *d < k.len().max(word.len()))
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

/// Levenshtein distance, over chars.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            let next = (diagonal + cost).min(row[j] + 1).min(row[j + 1] + 1);
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}
