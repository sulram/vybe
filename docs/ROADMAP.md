# ROADMAP.md — what we intend to build next

The living backlog: the features we *want*, as checkboxes. The forward-looking
counterpart to [DECISIONS.md](DECISIONS.md) (what we already decided). Nothing
here is a commitment — items get reordered, reshaped, or dropped as we learn.

**How to read it.** A checkbox is a *wish*, not a spec. Per the project's law
(Golden Rule #2), a feature only truly enters when a **kata pulls it** — a
concrete visual we want to make. So most items name the sketch that would pull
them. Checking a box means: the kata shipped, the capability is in, the lesson
is logged in DECISIONS.md.

**How to change it.** Freely. Add a wish the moment it occurs to you; move the
order when priorities shift; strike an item we've decided against (and log the
why in DECISIONS.md). Keeping this file honest is part of the work — an
abandoned roadmap lies.

---

## Now — 0.0.2: "the patch runs on a laptop"

The thesis shifted (DECISIONS 2026-09-20): **the patch is the product.** The
pulling kata is a projected cube installation; this section follows the order
of the 0.0.2 design brief. Acceptance: `vybe render remote-keystone.vy --osc
"/hands 1 @1s"` shows idle → touch → play on a Mac, no Pi, no GStreamer — **met**
(`examples/patches/remote-keystone/`).

- [x] `vybe render`: headless, fixed `Clock`, scripted inputs (`--osc`), PNG out;
      identical across runs. Also `VYBE_RENDER=…` on any `.rs` example.
- [x] GPU core as one keyed node list (replaces `Passes` + `Layer`); **key =
      identity** — state survives re-description, a wire is one node.
- [x] `Gate`, `Ramp`, `Fader` as std-only objects, unit-tested; time semantics
      decided (edges vs levels; transitions fire on *becoming* true).
- [x] Scalars in the patch: `osc /addr`, `osc hz amp`, `ramp`, `( expr )`,
      `smooth()`; plug into any number socket.
- [x] `.vy` parser (hand-written) + checker + `vybe run` + `vybe api`; the
      vocabulary as data. Fixture: the brief's cube face parses whole.
- [x] Scenes + transitions on `Fader`; crossfade = additive dissolve.
- [x] `frames` from PNG (in-memory textures), played, looped, or scrubbed (`@`).
- [x] New forms in both dialects: `rect`, `line`; `stroke`, `gray`, `alpha`.
- [x] Keystone: the present pass is the warp; `keystone` crate (file, atomic
      save); OSC protocol both ends (`vybe-remote`); **two windows on one Mac**.
- [x] Keyboard + mouse as `Inputs`; `Binding` seam; `arrows_nudge`,
      `mouse_drag`, `.on(Key)`, `key_cycle`. `live()` returns a `Stage`.
- [x] Kata pairs `.rs`/`.vy` byte-identical (`tests/pairs.rs`, `--ignored`: GPU).
- [ ] **Keystone `source` size**: render the face at its own size (1200²) and
      warp *its* corners into the output (1920×1200). Today the warp pulls the
      whole output's corners — calibrates fine, but the handles aren't the
      square's. The brief's `keystone.json` already anticipates `source {w,h}`.
- [ ] `Draw` + `canvas()` + `rust <name>` leaves; `text` (one embedded mono
      font), `polygon`, `arc`. Kata: `canvas_ring` (the loading ring). Until
      then the remote's status line lives in its window title.
- [ ] `Scalar` in the Rust sugar (`tune()` returns one) — the sugar then builds a
      `Patch`, and `Recipe::to_vy()` prints any chain as a patch.
- [ ] `vybe check --scenario hands.toml`: 60 s of behaviour in a blink
      (`Player::advance` already runs GPU-free) — scenes visited, dwell, stuck
      fades. State bugs are invisible in a PNG and obvious here.
- [ ] Hot reload of a running patch by node-name diff (the GPU half exists: it
      *is* key = identity; missing: watch the file, swap the `Patch`, keep the
      player's objects by name).
- [ ] `vybe fmt` (canonical spacing, so patches diff cleanly); `vybe set`.
- [ ] `Param`: a `tune` that persists (TOML) and is addressable as
      `/param/<name>` (incoming is handled; persistence + announce are not).
- [ ] `| feedback` over a composition (today: one shape), when a patch pulls it.
- [ ] Pixel-pair CI needs a GPU runner (or lavapipe); today the pairs run locally.
- [ ] Split `gpu.rs` (2 k lines — the core's biggest file): `gpu/points.rs` is a
      clean cut. LLM-sized is a rule, not a wish.
- [ ] Colour: `gray .5` is *linear* .5 (≈ 74 % on screen). Decide whether paint
      knobs should be perceptual before a calibration pattern depends on it.

## Next — 0.0.3: "the patch runs on the wall"

- [ ] `vybe-stage-drm`: KMS/GBM shell, no window (`out kms` stops falling back).
      Spike wgpu-on-KMS in `examples/` first — the brief's risk #1.
- [ ] `vybe-video`: GStreamer HEVC (`v4l2h265dec`), DMA-BUF import behind a
      feature, NV12 CPU-copy fallback. `video` stops being a checker error.
- [ ] `vybe-audio` (`mute`, `vol`, a transition's sound), `vybe-io` GPIO.
- [ ] mDNS `_vybe._tcp` discovery; the remote cycles faces (Alt ↑↓); MJPEG preview.
- [ ] ASTC cache for `frames()` (~1 MB/frame instead of 9).
- [ ] Half-resolution feedback as an option (fill rate on VideoCore VII).
- [ ] The cube on four Pi 5 units.

Consciously deferred: TS/Lua dialects, WASM, node-graph UI, `async` scripting,
ternaries in the grammar, mesh warp, HAP (no S3TC on VideoCore — do not revisit).

---

## Before 0.0.2 — the trial by fire

Two things stand between us and "the best creative-coding library in the
world": the temporal model must be honest, and the third signal type
(point cloud) must be born. The rest is polish.

### 1 · Fix the temporal model (correctness debt)

The engine promises `Wave.x` in "cycles per second" and `Hue.drift` in
"degrees per second" — but `time = frame / 60.0` ([gpu.rs]) couples animation
to frame count, so on 120/144 Hz everything runs fast, under load it drags.
The labels lie today. This corrupts every future time-based kata; clear it
first.

- [x] Add a wall-clock `time` (seconds) and per-frame `dt` (delta seconds) to
      the frame uniform block.
- [x] Source them from `std::time::Instant`, behind a tiny `Clock` seam so
      Phase 2 (WASM) can swap it — `Instant::now()` panics on
      `wasm32-unknown-unknown`; the seam is the escape hatch.
- [x] `Wave` and `Hue.drift` read real seconds → their "per second" labels
      become true with no knob change.
- [x] Feedback `Swirl` moves from **per-frame** to **per-second**: apply
      `decay^dt`, `angle * dt`, `scale^dt` each frame. Retune the defaults to
      preserve the current look; update the docs.
- [x] Update [GLOSSARY.md](GLOSSARY.md) (Swirl now per-second) and log the
      decision.

### 2 · Quick wins (independent, cheap, do alongside)

- [x] `mouse()` returns to rest on `CursorLeft` (today it freezes at the last
      position off-window). Reset to far-away, the existing rest convention.
- [x] Fix the 2 clippy lints (`-D warnings` clean).
- [ ] Decide rustfmt governance: run `cargo fmt` on `src/`, and consciously
      choose whether it rules the hand-aligned `examples/` too.

### 3 · Open the point-cloud signal type (the new phase)

The third signal type, alongside texture and geometry. A `Points` is a
`Signal` whose payload is a GPU **buffer** of particles — simulated in a
compute shader, drawn instanced. The chain stays singular; the payload is new.
The north star: **one million+ particles**, script touching only parameters,
never per-element data (the README's anti-bottleneck section, literal).

Decisions baked in (change consciously): **storage buffers + compute**, not
the WebGL-era position-texture hack — we're WebGPU-native and get to skip that
debt (like instancing from day 1). Build the substrate first; fluid and
voronoi are the same substrate with one piece swapped.

**Substrate — `examples/particles.rs`** (the kata that pulls the whole tier):
- [x] `Points` signal: a storage buffer of `Particle { pos, vel }`.
- [x] Seed/emit: deterministic hash-seeded positions + drift (no `rand` dep).
- [x] Compute step: `pos += (vel + field) * dt`, on the GPU each frame.
- [x] Instanced draw reading the buffer by instance index (own soft-dot
      fragment for now; SDF-circle sharing waits for the prelude, below).
- [x] Script sets only the count; data never crosses the CPU boundary.

**Fluid — `examples/flow.rs`** (substrate + one force):
- [ ] Curl-noise flow field as a force building block → "fluid-like" motion.
      (Real SPH — neighbour search, spatial hashing — waits for a sketch that
      truly needs it; curl-noise is the minimum that gives the look.)

**Voronoi — `examples/voronoi.rs`** (the two-bridge kata):
- [ ] Walking dots are the seeds; the diagram is a fullscreen pass that reads
      the seed buffer — a **points → texture** bridge, the `.render()` idea in
      a new type.
- [ ] Brute-force nearest-seed for N ≤ hundreds; note jump-flood (JFA) as the
      path to the million-seed diagram (a chain of fullscreen passes).

**Cross-cutting for this phase:**
- [ ] First shared **WGSL prelude**: extract the circle SDF (and later noise)
      when the second shader needs it — building blocks are *functions*, not
      pipelines (the geometry-world composition model). Compose via string
      concat until WGSL `#include` matures.
- [ ] Keep `shape.wgsl` as the geometry renderer — do **not** absorb particles
      into it. Scale comes from the signal-type boundary, not one megashader.

---

## Later — deferred wishes (logged, not urgent)

Each waits for the kata that pulls it (Golden Rule #2).

- [ ] `tune` lifecycle: mark-and-sweep orphaned knobs + reconcile ranges when a
      `live()` tree changes (Codex finding #3; latent until a sketch makes
      `tune()` calls conditional).
- [ ] Position source variants: `touch(0)`, an oscillator-position — the third
      `Source` plug for `at`/`grow`.
- [ ] `scatter(n)` — random placement, when a sketch pulls randomness (may
      subsume `grid` as sugar over `scatter(Grid { .. })`).
- [x] `layers()` composites mixed worlds — feedback on one layer, plain geometry
      on another (pulled by `layers_aurora`; additive default). See DECISIONS 2026-07-07.
- [x] `layers().blend(..)` — *select* the blend mode on the compositing seam
      (`Over` alpha, `Add` glow; pulled by `layers_over`). An enum, not a string — the
      Rust dialect stays type-safe; scripting dialects can map strings onto it
      later. `screen`/`multiply` wait for a sketch that needs them.
- [x] Per-layer blend: `world.blend(Blend::Add)` sets one layer's mode (the
      stack `.blend()` is the default it falls back to), so a stack can mix
      modes — add on one layer, over on the next (pulled by `layers_blend`).
- [x] `Over` becomes the stack default (was `Add`) — a bare stack composites
      like a layers panel, top item in front; glow is a spelled choice
      (`.blend(Blend::Add)`, see `layers_aurora`). Also makes the cheap all-geometry
      path exact, not approximate. See DECISIONS 2026-07-08.
- [ ] **Everything composes and feedbacks** (the rule, DECISIONS 2026-07-08).
      Two debts to close for the point cloud:
  - [x] Particles as a composited layer — a `particles(..)` world in `layers()`,
        blended add/over like any other (pulled by `layers_over`/`blend`). Extracted a
        reusable `PointsPass` (compute step + draw) that targets the screen or a
        layer's texture; `Layer` widened to a `World` sum (`Signal` | `Particles`).
  - [x] Live-tune a particle layer — closed for free by the keyed node list
        (0.0.2): a re-described composite keeps every node whose key and kind
        survive, so the cloud keeps its positions.
  - [ ] Particles into `.feedback()` — the cloud rendered to a signal texture
        that can source a feedback loop (trails of a swarm).
- [ ] Particle colour: a hue/tint knob on `particles(..)` (points render white
      today) — needed the moment two clouds share a frame and must read apart.
- [x] SDF shape library, first step: `rect` (and `line`, a rect laid along two
      points) selected by index inside `shape.wgsl`; outlines via `stroke`.
      Pulled by the calibration grid. `star`, `arc`, `polygon` wait their turn.
- [x] First tests: pure logic that deserves them. (0.0.2: 67 across the
      workspace — objects, parser, checker, player, warp math, OSC, protocol.)
- [ ] Texture-world effects as chained passes: `bloom`, `kaleid`, `blur` — the
      Hydra lineage, each a fullscreen pass (the texture-world composition
      model).
- [ ] Live particle knobs: expose count / density / field strength via `tune()`
      so the cloud calibrates by slider (`Particles` is already a `Chain`, so
      `live(|| particles(..))` works). Note the split: `count` is *structural*
      (a change rebuilds the buffer through `set_recipe` — already supported,
      just heavier), while density/brightness/field are shader constants today
      and must first become uniforms before they're tunable.

### Imported from tekne-flow (the language, not the editor)

The proven ideas from the tekne-flow prototype worth growing into vybe, under
the vibe-coding-first direction (see DECISIONS 2026-07-06). Each waits for a kata
that a real piece pulls — do NOT bulk-build them.

- [ ] **Columnar `Stream`/context** — evolve `Signal`/`Stroke` toward per-element
      attribute *columns* (`x, y, index, t, scale, noise…`) in SoA. Maps 1:1 to a
      GPU storage buffer (the particle buffer is the first de-facto ColumnStore).
      Turns baked structs (`Falloff`) into composable chains. The big one.
- [ ] **Normalized `t` (0→1)** as a first-class column — the natural handle for
      "color/scale by position along the set" (LLM-addressable).
- [ ] **Late-binding style** — color/size drivable by any context column
      (`hue(by("t"))`, a gradient over `t`), resolved at render, not baked scalars.
- [ ] **Composable value vocabulary** — `distance`, `map` (remap range), `noise`,
      `sin/cos`, `modulo`, `instance` (points × shape); generators stay pure,
      transforms are dedicated steps. Orthogonal, LLM-legible verbs are the product.
- [ ] **Reprioritized by vibe-coding:** the **TS/JS sugar dialect + WASM** (Phase 2)
      rises — LLMs are most fluent in TS/JS and vibe-coding a web tool wants the
      browser. The node-graph front-end drops in priority (deferred, not killed).

---

## Shipped — the gallery so far

The katas that already pulled their features (the proof it sings).

- [x] `feedback` — the signal-texture + feedback loop (Phase 0).
- [x] `dots` — geometry, instancing, input-as-signal (opened Phase 1).
- [x] `feedback_trail` — the `.render()` bridge (geometry → texture).
- [x] `rainbow` / `rainbow_trails` — `layers()` and `Layers::render()`.
- [x] `osc` — the `Osc` waveform menu on `Wave`.
- [x] `dots_tune` — `tune()` / `live()` and the `Overlay` seam (the panel).
- [x] `dots_tune_xy` — `grow()` takes a position `Source`, driven by tuned x/y.
- [x] `layers_aurora` — compositing: `layers()` stacks mixed worlds, feedback on one.
- [x] `layers_over` — alpha compositing (now the stack default, spelled nowhere): a
      particle galaxy laid over a field (also pulled particles into `layers()`).
- [x] `layers_blend` — per-layer `world.blend(..)`: mixed modes in one stack (a
      particle swarm spells `add`, a disc rides the `over` default), worlds
      bound to lets then composed.

Patches (`examples/patches/<name>/`), 0.0.2:

- [x] `hello` — the smallest patch. `trails` — `|` into `feedback` (twin of the
      `feedback` sketch, pixel for pixel). `stack` — `+ * add` and Scalars.
- [x] `scenes` — scenes, transitions, a key as a gate.
- [x] `calibration` — test patterns as scenes; the face the remote talks to.
- [x] `remote-keystone` — the cube face, laptop edition: scenes + keystone +
      remote in one patch; renders its own media.
