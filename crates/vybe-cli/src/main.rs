//! `vybe` — the patch's command line.
//!
//!   vybe check  face.vy                     read it for mistakes (no GPU)
//!   vybe run    face.vy --key space=/hands  play it in a window
//!   vybe render face.vy --at 0s,2s,4.5s --osc "/hands 1 @1s" --out frames/
//!   vybe api                                the whole vocabulary, to paste into a model's context
//!   vybe grammar                            the TextMate grammar, generated from that vocabulary
//!
//! For an author who can't look at a screen — a model — `render` and `check`
//! *are* the screen: write, render, look, correct, alone.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use keystone::Keystone;
use vybe::input::{Binding, Event, Io, Key, Value};
use vybe::patch::{self, Patch, Player, Severity};
use vybe::stage::{Render, Script, Stage, Warp};
use vybe_remote::Face;

#[derive(Parser)]
#[command(name = "vybe", version, about = "Run, render and check .vy patches")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Play a patch in a window (hot keys stand in for sensors)
    Run {
        patch: PathBuf,
        /// Map a key onto an input while held: `--key space=/hands`
        #[arg(long = "key", value_name = "KEY=/ADDRESS")]
        keys: Vec<String>,
    },
    /// Render a patch headless, on a fixed clock, to PNGs
    Render {
        patch: PathBuf,
        /// Moments to save: `--at 0s,2s,4.5s`
        #[arg(long, value_delimiter = ',', value_name = "TIMES")]
        at: Vec<String>,
        /// Save every frame of the first N seconds (a PNG sequence): `--seq 4s`
        #[arg(long, value_name = "DURATION")]
        seq: Option<String>,
        #[arg(long, default_value_t = 60.0)]
        fps: f32,
        /// Frame size (default: the patch's `out` size, else 800x800)
        #[arg(long, value_name = "WxH")]
        size: Option<String>,
        /// Scripted inputs: `--osc "/hands 1 @1s, /hands 0 @3s"`
        #[arg(long, value_name = "CUES")]
        osc: Option<String>,
        /// Keep transparency instead of landing on black
        #[arg(long)]
        alpha: bool,
        #[arg(long, default_value = "frames")]
        out: PathBuf,
    },
    /// Read a patch for mistakes, before any GPU work
    Check { patch: PathBuf },
    /// Print the whole patch vocabulary
    Api,
    /// Print the TextMate grammar editors highlight `.vy` with
    /// (`vybe grammar > editors/vscode/syntaxes/vy.tmLanguage.json`)
    Grammar,
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Api => {
            print!("{}", patch::vocabulary::api());
            Ok(())
        }
        Command::Grammar => {
            print!("{}", patch::vocabulary::textmate());
            Ok(())
        }
        Command::Check { patch } => load(&patch).map(|_| println!("{}: ok", patch.display())),
        Command::Run { patch, keys } => run(&patch, &keys),
        Command::Render {
            patch,
            at,
            seq,
            fps,
            size,
            osc,
            alpha,
            out,
        } => render(
            &patch,
            &at,
            seq.as_deref(),
            fps,
            size.as_deref(),
            osc.as_deref(),
            alpha,
            out,
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Parses and checks a patch, printing every finding. `Err` when it won't run.
fn load(path: &Path) -> Result<Patch, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (patch, found) = match patch::parse(&text) {
        Ok(patch) => {
            let found = patch::check(&patch, base_of(path));
            (Some(patch), found)
        }
        Err(errors) => (None, errors),
    };
    for finding in &found {
        eprintln!("{}: {finding}", path.display());
    }
    let errors = found
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    match patch {
        Some(patch) if errors == 0 => Ok(patch),
        _ => Err(format!("{}: {errors} error(s) — not run", path.display())),
    }
}

/// A patch's media and keystone paths are relative to its own folder.
fn base_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// The stage a patch plays on, with its `out` line honoured: `remote <port>`
/// brings up the face's end of the protocol, `keystone <file>` is what it loads
/// and saves. Without `listen` (headless) the keystone still applies — a render
/// is what the wall will show — but it is fixed, and the network stays out.
fn stage(path: &Path, patch: Patch, listen: bool) -> Result<Stage, String> {
    let base = base_of(path);
    let out = patch.out.clone();
    let mut stage = Player::new(patch, base).stage().title(&format!(
        "vybe — {}",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    if let Some(out) = out {
        let keystone = out.keystone.map(|file| base.join(file));
        match out.remote {
            Some(port) if listen => {
                stage = stage.with(Face::listen(port, keystone)?);
                eprintln!("vybe: listening for OSC on port {port}");
            }
            _ => {
                if let Some(file) = keystone {
                    let keystone = Keystone::load_or_default(&file).map_err(|e| e.to_string())?;
                    stage = stage.with(Fixed(Warp {
                        corners: keystone.corners,
                        feather: keystone.feather,
                    }));
                }
            }
        }
    }
    Ok(stage)
}

fn run(path: &Path, keys: &[String]) -> Result<(), String> {
    let mut stage = stage(path, load(path)?, true)?;
    for mapping in keys {
        let bad = || format!("--key {mapping}: expected KEY=/ADDRESS, like `space=/hands`");
        let (key, address) = mapping.split_once('=').ok_or_else(bad)?;
        let key = Key::parse(key).ok_or_else(bad)?;
        stage = stage.with(KeyInput {
            key,
            address: address.to_owned(),
        });
    }
    stage.show();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render(
    path: &Path,
    at: &[String],
    seq: Option<&str>,
    fps: f32,
    size: Option<&str>,
    osc: Option<&str>,
    alpha: bool,
    out: PathBuf,
) -> Result<(), String> {
    let patch = load(path)?;
    let [width, height] = match size {
        Some(size) => size
            .split_once('x')
            .and_then(|(w, h)| Some([w.parse().ok()?, h.parse().ok()?]))
            .ok_or(format!("--size {size}: expected WxH, like 1200x1200"))?,
        None => patch
            .out
            .as_ref()
            .and_then(|o| o.size)
            .unwrap_or([800, 800]),
    };
    let mut times = Vec::new();
    for t in at {
        times.push(seconds(t)?);
    }
    let seq = seq.map(seconds).transpose()?;
    if times.is_empty() && seq.is_none() {
        times.push(0.0);
    }
    let render = Render {
        width,
        height,
        fps,
        at: times,
        seq,
        alpha,
        out,
    };
    // Headless, the network stays out of it: inputs are the script's alone, so
    // the same command always renders the same pixels.
    let cues = osc.map(cues).transpose()?.unwrap_or_default();
    let stage = stage(path, patch, false)?.with(Script::new(cues));
    for frame in stage.render(&render)? {
        println!("{}", frame.display());
    }
    Ok(())
}

/// `2s`, `4.5s`, `2` — seconds.
fn seconds(text: &str) -> Result<f32, String> {
    let number = text.trim().trim_end_matches('s');
    number
        .parse()
        .map_err(|_| format!("`{text}` is not a time — write `2s` or `4.5s`"))
}

/// `/hands 1 @1s, /hands 0 @3s` — scripted inputs.
fn cues(text: &str) -> Result<Vec<(f32, String, Value)>, String> {
    text.split(',')
        .map(str::trim)
        .filter(|cue| !cue.is_empty())
        .map(|cue| {
            let bad =
                || format!("--osc `{cue}`: expected `/address value @time`, like `/hands 1 @1s`");
            let words: Vec<&str> = cue.split_whitespace().collect();
            let [address, value, at] = words.as_slice() else {
                return Err(bad());
            };
            let at = seconds(at.strip_prefix('@').ok_or_else(bad)?)?;
            Ok((at, (*address).to_owned(), Value::parse(value)))
        })
        .collect()
}

/// A calibration that nobody is turning.
struct Fixed(Warp);

impl Binding for Fixed {
    fn frame(&mut self, io: &mut Io<'_>) {
        *io.warp = self.0;
    }
}

/// A key held is an input at 1 — the laptop's stand-in for a sensor, so the
/// patch reads `/hands` on the desk exactly as it will on the wall.
struct KeyInput {
    key: Key,
    address: String,
}

impl Binding for KeyInput {
    fn event(&mut self, event: &Event, io: &mut Io<'_>) {
        if let Event::Key {
            key,
            pressed,
            repeat: false,
            ..
        } = event
        {
            if *key == self.key {
                let held = if *pressed { 1.0 } else { 0.0 };
                io.inputs.set(&self.address, Value::Num(held));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// The checked-in grammar is generated, never edited: a new word in the
    /// vocabulary must reach the editors in the same change.
    #[test]
    fn the_editor_grammar_is_up_to_date() {
        let checked_in = include_str!("../../../editors/vscode/syntaxes/vy.tmLanguage.json");
        assert!(
            checked_in == vybe::patch::vocabulary::textmate(),
            "stale grammar — regenerate it:\n    cargo run -p vybe-cli -- grammar > editors/vscode/syntaxes/vy.tmLanguage.json"
        );
    }
}
