//! The closed vocabulary of a patch, as data. One table per word class — the
//! parser reads them, the checker's hints come from them, and `vybe api` prints
//! them, so the three can never drift apart. A model that has read `vybe api`
//! cannot hallucinate a word.

/// What a source word produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Circle,
    Rect,
    Line,
    Frames,
    Video,
    Text,
}

/// A word that modifies the source before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModKind {
    Soft,
    Hue,
    Drift,
    Wave,
    Gray,
    Alpha,
    Stroke,
    At,
    X,
    Y,
    Grid,
    Size,
    Fit,
    Loop,
    Fps,
    Mute,
    Vol,
    Paused,
}

/// A GPU effect a node is wired into with `|`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Feedback,
}

/// What kind of thing a modifier can follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// circle, rect, line
    Shape,
    /// frames, video
    Media,
    Text,
}

pub struct SourceWord {
    pub kind: SourceKind,
    pub name: &'static str,
    /// Numeric arguments, by name.
    pub args: &'static [&'static str],
    /// The textual argument, if it takes one (a path, a glob, a string).
    pub text: Option<&'static str>,
    pub family: Family,
    pub doc: &'static str,
}

pub struct ModWord {
    pub kind: ModKind,
    pub name: &'static str,
    pub args: &'static [&'static str],
    pub families: &'static [Family],
    pub doc: &'static str,
}

pub struct EffectWord {
    pub kind: EffectKind,
    pub name: &'static str,
    /// Named parameters with their defaults.
    pub params: &'static [(&'static str, f32)],
    pub doc: &'static str,
}

pub const SOURCES: &[SourceWord] = &[
    SourceWord {
        kind: SourceKind::Circle,
        name: "circle",
        args: &["radius"],
        text: None,
        family: Family::Shape,
        doc: "a disc; radius as a fraction of its grid cell (no grid: of the scene)",
    },
    SourceWord {
        kind: SourceKind::Rect,
        name: "rect",
        args: &["width", "height"],
        text: None,
        family: Family::Shape,
        doc: "a rectangle; `rect 1 1` is the scene's unit square",
    },
    SourceWord {
        kind: SourceKind::Line,
        name: "line",
        args: &["x0", "y0", "x1", "y1"],
        text: None,
        family: Family::Shape,
        doc: "a line between two scene points; `stroke` is its width",
    },
    SourceWord {
        kind: SourceKind::Frames,
        name: "frames",
        args: &[],
        text: Some("glob"),
        family: Family::Media,
        doc: "a PNG sequence (alpha kept), in name order; plays once, or `loop`s, or is scrubbed with `@`",
    },
    SourceWord {
        kind: SourceKind::Video,
        name: "video",
        args: &[],
        text: Some("file"),
        family: Family::Media,
        doc: "a video file, with its sound; plays once (then `done`) or `loop`s; restarts when its scene is entered",
    },
    SourceWord {
        kind: SourceKind::Text,
        name: "text",
        args: &[],
        text: Some("\"string\""),
        family: Family::Text,
        doc: "a line of text (arrives with Draw; draws nothing yet)",
    },
];

const SHAPE: &[Family] = &[Family::Shape];
const MEDIA: &[Family] = &[Family::Media];
const PLACED: &[Family] = &[Family::Shape, Family::Media, Family::Text];
const PAINTED: &[Family] = &[Family::Shape, Family::Text];

pub const MODS: &[ModWord] = &[
    ModWord {
        kind: ModKind::At,
        name: "at",
        args: &["x", "y"],
        families: PLACED,
        doc: "position, scene space: center (0,0), y up, shorter edge spans -.5...5",
    },
    ModWord {
        kind: ModKind::X,
        name: "x",
        args: &["x"],
        families: PLACED,
        doc: "horizontal position alone (adds to `at`)",
    },
    ModWord {
        kind: ModKind::Y,
        name: "y",
        args: &["y"],
        families: PLACED,
        doc: "vertical position alone (adds to `at`)",
    },
    ModWord {
        kind: ModKind::Alpha,
        name: "alpha",
        args: &["a"],
        families: PLACED,
        doc: "opacity, 0..1",
    },
    ModWord {
        kind: ModKind::Size,
        name: "size",
        args: &["s"],
        families: &[Family::Media, Family::Text],
        doc: "media: scale of the fitted image; text: height in scene units",
    },
    ModWord {
        kind: ModKind::Gray,
        name: "gray",
        args: &["v"],
        families: PAINTED,
        doc: "brightness, 0 black .. 1 white",
    },
    ModWord {
        kind: ModKind::Hue,
        name: "hue",
        args: &["degrees"],
        families: PAINTED,
        doc: "color wheel: 0 red, 120 green, 240 blue",
    },
    ModWord {
        kind: ModKind::Drift,
        name: "drift",
        args: &["degrees/s"],
        families: PAINTED,
        doc: "the hue slides over time",
    },
    ModWord {
        kind: ModKind::Soft,
        name: "soft",
        args: &["s"],
        families: SHAPE,
        doc: "edge: 0 crisp .. 1 a ball of light fading from the center",
    },
    ModWord {
        kind: ModKind::Stroke,
        name: "stroke",
        args: &["width"],
        families: SHAPE,
        doc: "draw an outline of this width instead of a fill (a line: its width)",
    },
    ModWord {
        kind: ModKind::Wave,
        name: "wave",
        args: &["amp", "x-hz", "y-hz"],
        families: SHAPE,
        doc: "lissajous motion around the position, run on the GPU",
    },
    ModWord {
        kind: ModKind::Grid,
        name: "grid",
        args: &["cols", "rows"],
        families: SHAPE,
        doc: "repeat on a centered grid, one instance per cell",
    },
    ModWord {
        kind: ModKind::Fit,
        name: "fit",
        args: &[],
        families: MEDIA,
        doc: "contain in the whole frame (default: in the scene's unit square)",
    },
    ModWord {
        kind: ModKind::Loop,
        name: "loop",
        args: &[],
        families: MEDIA,
        doc: "play forever (default: once, then `done`)",
    },
    ModWord {
        kind: ModKind::Fps,
        name: "fps",
        args: &["n"],
        families: MEDIA,
        doc: "frames per second of a sequence (default 30)",
    },
    ModWord {
        kind: ModKind::Paused,
        name: "paused",
        args: &[],
        families: MEDIA,
        doc: "hold the first frame",
    },
    ModWord {
        kind: ModKind::Mute,
        name: "mute",
        args: &[],
        families: MEDIA,
        doc: "video: no sound",
    },
    ModWord {
        kind: ModKind::Vol,
        name: "vol",
        args: &["v"],
        families: MEDIA,
        doc: "video: volume, 0..1 — it also follows the scene's fade, so a crossfade is one of sound too",
    },
];

pub const EFFECTS: &[EffectWord] = &[EffectWord {
    kind: EffectKind::Feedback,
    name: "feedback",
    params: &[("decay", 0.16), ("angle", 0.6), ("scale", 0.83)],
    doc: "a feedback loop fed by the node: trail surviving per second, radians per second, zoom per second",
}];

/// Scalar sources: `name = <one of these>`.
pub const SCALARS: &[(&str, &str)] = &[
    (
        "osc /address [debounce s]",
        "a live input (OSC, or /key/<name>, /mouse/x|y|down); with a debounce it is a gate, 0 or 1",
    ),
    (
        "osc <hz> <amp>",
        "an oscillator: a sine around zero; also usable inline as any number",
    ),
    (
        "ramp <gate> up <s> down <s>",
        "0..1: climbs while the gate is open, falls back while it is closed; reaches both ends exactly",
    ),
    ("( expr )", "arithmetic over Scalars: + - * and smooth(x)"),
];

/// The twelve rules, as `vybe api` prints them.
pub const GRAMMAR: &str = "\
name = source args  mod args  mod args     node: a source plus modifiers, any order
       | effect param value ...            wire into a GPU effect (line continues)
a + b                                      stack (b on top)
a * x                                      alpha (x: number or Scalar)
a @ x                                      position in time, 0..1 (frames)
a add                                      additive blend (default is over)
name : node                                scene (the first one shows first)
a -> b  cond  cut|fade Ns  [sound]         transition;  * = from any scene
cond:  node rise|off|done | node = N | c & c
units: s  hz  px    - no unit = scene units (shorter edge is 1)
name = rust <fn>                           imperative leaf, registered at compile time
out <output> args  picture WxH  keystone <file>  remote <port>

`picture`: the face's own size when it isn't the output's (a square on a 16:10 projector);
the keystone's four corners are then the picture's corners.
A number socket takes: a number, a Scalar's name, (an expression), smooth(x), or osc <hz> <amp>.
A transition fires on the frame its whole condition BECOMES true while its `from` scene shows.
A line starting with | or + continues the node above it.  # starts a comment.";

pub fn source(name: &str) -> Option<&'static SourceWord> {
    SOURCES.iter().find(|w| w.name == name)
}

pub fn source_word(kind: SourceKind) -> &'static SourceWord {
    SOURCES
        .iter()
        .find(|w| w.kind == kind)
        .expect("every SourceKind has a word")
}

pub fn modifier(name: &str) -> Option<&'static ModWord> {
    MODS.iter().find(|w| w.name == name)
}

pub fn effect(name: &str) -> Option<&'static EffectWord> {
    EFFECTS.iter().find(|w| w.name == name)
}

/// The whole vocabulary as text — what `vybe api` prints.
pub fn api() -> String {
    let mut out = String::from("# vybe patch vocabulary (.vy)\n\n## grammar\n\n");
    out.push_str(GRAMMAR);
    out.push_str("\n\n## sources\n\n");
    for w in SOURCES {
        let args: Vec<&str> = w.args.iter().copied().chain(w.text).collect();
        out.push_str(&format!(
            "{:<34} {}\n",
            format!("{} {}", w.name, args.join(" ")),
            w.doc
        ));
    }
    out.push_str("\n## scalars\n\n");
    for (form, doc) in SCALARS {
        out.push_str(&format!("{form:<34} {doc}\n"));
    }
    out.push_str("\n## modifiers\n\n");
    for w in MODS {
        out.push_str(&format!(
            "{:<34} {}\n",
            format!("{} {}", w.name, w.args.join(" ")),
            w.doc
        ));
    }
    out.push_str("\n## effects\n\n");
    for w in EFFECTS {
        let params: Vec<String> = w.params.iter().map(|(n, d)| format!("{n} {d}")).collect();
        out.push_str(&format!(
            "| {} {}\n    {}\n",
            w.name,
            params.join(" "),
            w.doc
        ));
    }
    out
}

/// A TextMate grammar for `.vy` — what `vybe grammar` prints, and what editors
/// highlight with. The rules are a template; the *words* come from the tables
/// above, so highlighting can't fall behind the language.
pub fn textmate() -> String {
    let words = |names: Vec<&str>| names.join("|");
    let of_family = |family: Family| {
        SOURCES
            .iter()
            .filter(move |w| w.family == family)
            .map(|w| w.name)
    };
    let params = EFFECTS.iter().flat_map(|e| e.params.iter().map(|p| p.0));
    include_str!("vy.tmLanguage.template.json")
        .replace("@MEDIA@", &words(of_family(Family::Media).collect()))
        .replace(
            "@SOURCES@",
            &words(SOURCES.iter().map(|w| w.name).collect()),
        )
        .replace("@MODS@", &words(MODS.iter().map(|w| w.name).collect()))
        .replace(
            "@EFFECTS@",
            &words(EFFECTS.iter().map(|w| w.name).collect()),
        )
        .replace("@PARAMS@", &words(params.collect()))
}
