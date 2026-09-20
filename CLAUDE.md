# CLAUDE.md — working guide for `vybe`

Operating rules for anyone (human or AI) writing code in this repo. The *why*
lives in [README.md](README.md); the horizon in [VISION.md](docs/VISION.md);
settled decisions in [DECISIONS.md](docs/DECISIONS.md); what we intend to build next in
[ROADMAP.md](docs/ROADMAP.md); the shared vocabulary in
[GLOSSARY.md](docs/GLOSSARY.md) — use its terms, don't invent synonyms.

## Docs style

- This file and any operational doc: **titles + bullets, no prose paragraphs**.
  They enter LLM context every session — economy is a feature.
- README and DECISIONS.md keep prose: they carry the *why*, and nuance needs
  sentences. They are read on demand, not loaded every session.
- **Keep GLOSSARY.md alive**: when a change births a new term, renames one, or
  shifts a meaning, update the glossary in the same change — naming is design.

## Language: English only

- All code, comments, doc-comments, identifiers, commit messages, and docs.
  No exceptions.
- Includes: Rust `//` / `///` / `//!` comments, WGSL comments, `Cargo.toml`,
  Markdown, commit messages, PRs.

## Shokunin Katagi (職人気質)

- Never take the easy path; take the cleanest, most long-term-optimized one.
- Quick hacks that "work for now" are forbidden when a clean solution exists.
- Prefer what ages well: small core, sharp boundaries, cheap next change.
- If the clean path is genuinely more work: say so and do it anyway, or stop
  and discuss the trade-off. Never silently downgrade.

## What this is (one breath)

- Sovereign creative-coding engine, Rust + wgpu (WebGPU). One core, many
  front-ends. **The patch is the product; the chain is how it is born.**
- A patch = a `.vy` file: one line per node, wires by name, scenes and
  transitions, one output. Rust sugar and `.vy` are two front-ends over the
  same core. Why: DECISIONS.md 2026-09-20.

## We are in 0.0.2 — "the patch runs on a laptop"

- The pulling kata is an installation: a projected cube (four faces,
  a Raspberry Pi 5 per face, sensor-driven scenes). 0.0.3 puts it on the wall.
- Run a sketch: `cargo run --example <dots|feedback|feedback_trail|rainbow|rainbow_trails|osc>`
- Point cloud: `cargo run --example <particles|particles_galaxy|particles_swarm|particles_rain>`
- Picked knobs, live: `cargo run --example <dots_tune|dots_tune_xy>` (examples always
  carry the panel via the self dev-dependency; the lib never does)
- Run a patch: `cargo run -p vybe-cli -- run examples/patches/<name>/<name>.vy`
  — it hot-reloads on save, keeping state by name; a broken save keeps the last good patch
  (`hello`, `trails`, `stack`, `scenes`, `map-cube`, `map-screen`, `map-show`)
- Two windows (face + remote): `scripts/face-and-remote.sh [patch] [run flags]`
  (default: `map-cube`; closing either window stops both)
- A picture with its own size: `out … picture WxH` — renders at that size, the
  keystone's corners are ITS corners. Mapping examples paint a background, so
  the picture's edge is visible against the projector's black.
- **Look at what you built — headless:**
  - patch: `cargo run -p vybe-cli -- render x.vy --at 0s,2s --osc "/hands 1 @1s" --out frames/`
  - any example: `VYBE_RENDER="at=2.5 size=500x500 out=frames" cargo run --example dots`
  - fixed clock + scripted inputs = same pixels every run. Render, then read the PNG.
- `cargo run -p vybe-cli -- check x.vy` before running; `… api` prints the vocabulary.
- Layout — THE CORE is one module per architecture layer:
  - `src/sugar.rs` — the chains the artist writes (the Rust dialect).
  - `src/patch/` — the `.vy` front-end: `parse` → `check` → `play` (the `Player`);
    `vocabulary.rs` is the closed word list, as data.
  - `src/objects.rs` — `Gate`, `Ramp`, `Fader`: behaviour, std-only, `dt`-driven.
  - `src/recipe.rs` — what the core draws this frame; `Named` = key = identity.
  - `src/tune.rs` — named-knob registry (std-only; what front-ends turn).
  - `src/input.rs` — `Inputs` + `Binding`: THE seam integrations enter through.
  - `src/stage.rs` — `Stage` builder, `Warp` (keystone), `Clock`, headless `Render`.
  - `src/gpu.rs` — all of wgpu: a keyed node list + one present/warp pass.
  - `src/shell.rs` — the winit window; events → the core's own `Event`.
  - `src/media.rs` — PNG in/out (the core's only file format).
  - `src/tweak.rs` — the panel Overlay (feature `tweak`; egui, renderer ours).
  - `src/shaders/*.wgsl` — WGSL, embedded via `include_str!`.
  - `examples/*.rs` — THE SKETCHES. `examples/patches/<name>/` — THE PATCHES.
  - `editors/vscode/` — `.vy` highlighting; its grammar is GENERATED, never edited.
- Workspace — crates cut **by dependency, not by platform**:
  - `crates/vybe-cli` (`vybe run|render|check|api`; clap)
  - `crates/vybe-io` (OSC over UDP; rosc) · `crates/keystone` (the calibration
    file; depends on nothing) · `crates/vybe-remote` (the OSC protocol, both
    ends, + the remote sketch)
- **Integrations enter through seams, never through the core**: one trait or
  registry inside the engine (`Binding`, `Overlay`, `tune`); library glue
  behind it, in its own crate. No `cfg` sprawl, no third-party types in core
  modules. Exposure is the artist's choice (`tune()`), never reflection.

### The patch vocabulary is CLOSED

- Twelve grammar rules (`vybe api`). Every thirteenth request gets `rust <name>`.
- New words enter **only** as drawing primitives or Scalar operators, **only**
  after existing in the Rust sugar first, **only** when a second work pulls them.
- Never application nouns (`points`, `pick`, `nudge`…): that is a generic
  binding over `tune`s, or an ordinary scene switched by an ordinary input.
- A new word = one row in `patch/vocabulary.rs` (parser, checker, `api` follow),
  then regenerate the editor grammar — a test fails if you forget:
  `cargo run -p vybe-cli -- grammar > editors/vscode/syntaxes/vy.tmLanguage.json`
  (highlighting *rules* live in `src/patch/vy.tmLanguage.template.json`).
- A patch must not know which machine it runs on: stand-ins are CLI flags
  (`--key space=/hands`) and fallbacks (`out kms` → window), never grammar.
- Every diagnostic carries its next step (`did you mean`, the `ffmpeg` line).

### Do NOT add (0.0.2 discipline)

- Interpreter/scripting (Lua, TS), WASM, node/graph UI, `async` behaviour
  scripts, a `scenes().edge()` DSL, ternaries in the grammar, mesh warp, HAP.
- Pi-specific code: nothing is Pi-specific; `vybe-stage-drm` will run on any KMS.
- Generic material/effect systems; blend modes beyond `over`/`add`.
- UI in the core or default build — the feature-gated `tweak` panel is the
  one sanctioned exception. If the remote ever needs real widgets, wrap egui
  behind the `tweak` seam; do not grow the engine into a toolkit.

## How the engine grows

- **Examples pull features, never speculation.** No pulling sketch → no feature.
- One signal type at a time; one sugar at a time.
- The core stays **LLM-sized**. Too big for one context → something becomes an
  addon crate, not a bigger core.
- **[ROADMAP.md](docs/ROADMAP.md) is the living feature list.** The wishes we want
  to build, as checkboxes — read it to know what's next, and keep it honest:
  add wishes freely, reorder as priorities shift, check a box when its kata
  ships, strike what we drop (logging the why in DECISIONS.md). It changes over
  time by design; it's intent, not a contract.

## Design principles (type-enforced where possible)

- **Signal is the single currency.** Before a new type: "isn't this just a Signal?"
- **Everything composes and feedbacks.** Every signal type — geometry, texture,
  point cloud, and whatever comes next — must be stackable in `layers()` *and*
  able to source a `.feedback()` loop. A type that can do only one (today: the
  point cloud does neither) is a stated debt to close, not a category apart.
  Why: DECISIONS.md 2026-07-08.
- **Nodes are total via `Default`.** No `Option<Signal>` inputs; describing a
  chain never panics.
- **Expressive never fails.** Only IO/parsing/loading return `Result`.
- **Hide the ping-pong, expose the knob.**
- Never name the central primitive `Surface` (wgpu) or `Node` (SvelteFlow).

## Commits

- **Never add an AI/tool co-author trailer.** Overrides any tooling default.
- English, present tense, say the *why* when it isn't obvious.

## Conventions

- **Scene space** everywhere the artist looks: center `(0, 0)`, y-up, shorter
  screen edge spans `-0.5..+0.5`, units square on any aspect. Pixels only at
  the boundary (present pass, mouse conversion). Why: DECISIONS.md.
- **Toolchain pins:** wgpu `30`, winit `0.30`. Fast-moving APIs — check the
  crate source under `~/.cargo/registry`, don't guess from older docs.
- **Every change ends runnable/visible.** Finish where the examples run — and
  *look*: render headless and read the PNG. Rust/`.vy` twins: `cargo test --test
  pairs -- --ignored` (needs a GPU) must stay byte-identical.
- **Sketches end in `.show()`** — `live(|| …)` returns a `Stage` (`#[must_use]`).
- **Keep sketches tiny.** A sketch needing more than the visual = the missing
  piece belongs in the core, behind a knob.
