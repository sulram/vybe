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
  sugars (Rust now; TS/JS and Lua later). The chain syntax is the product.

## We are in Phase 1

- Phase 0 (feedback POC) proved the idea. Phase 1 brought, each pulled by a
  kata: geometry (`Shape`), instancing, input-as-signal, the `.render()`
  bridge (geometry → texture), and `layers()`.
- Run any sketch: `cargo run --example <dots|feedback|feedback_trail|rainbow|rainbow_trails|osc>`
- Point cloud (GPU particles + forces): `cargo run --example <particles|particles_galaxy|particles_swarm|particles_rain>`
- Picked knobs, live: `cargo run --example <dots_tune|dots_tune_xy>` (examples always
  carry the panel via the self dev-dependency; the lib never does)
- Layout — THE CORE is one module per architecture layer:
  - `src/sugar.rs` — the chains the artist writes (the Rust dialect).
  - `src/recipe.rs` — the flattened chain (chain = AST), the Multi-Sugar seam.
  - `src/tune.rs` — named-knob registry (std-only; what front-ends turn).
  - `src/gpu.rs` — all of wgpu, hidden behind the knobs; the Overlay seam.
  - `src/shell.rs` — the winit window/input loop; live re-describe.
  - `src/tweak.rs` — the panel Overlay (feature `tweak`; egui, renderer ours).
  - `src/shaders/*.wgsl` — WGSL, embedded via `include_str!`.
  - `examples/*.rs` — THE SKETCHES. Short, readable, only the visual.
- **Integrations enter through seams, never through the core**: one trait or
  registry inside the engine; library glue behind it (feature/addon). No
  `cfg` sprawl, no third-party types in core modules. Exposure is the
  artist's choice (`tune()`), never reflection over everything.

### Do NOT add (Phase 1 discipline)

- Interpreter/scripting (Lua, TS), WASM, node/graph systems, generic node
  traits.
- Point clouds — signal types enter **one at a time** (texture + geometry
  exist today), each pulled by a sketch.
- Blend modes, addons (svg/fonts/image/audio), generic material/effect
  systems.
- UI in the core or default build — the feature-gated `tweak` panel is the
  one sanctioned exception (a dev tool, never shipped by default).

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
- **Every change ends runnable/visible.** Finish where the examples run.
- **Keep sketches tiny.** A sketch needing more than the visual = the missing
  piece belongs in the core, behind a knob.
