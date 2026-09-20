# DECISIONS.md — design decisions & kata lessons

The running log of decisions that shape the engine, and of what each kata
(example) taught us. The vision behind them lives in [README.md](../README.md);
the enforceable day-to-day rules in [CLAUDE.md](../CLAUDE.md).

Each entry records what we decided, why, and what we rejected — so nothing gets
re-litigated by accident. Reversing an entry is allowed, but consciously,
against the "why" written here.

---

## 2026-07-05 — Scene space: the artist's coordinate system

**Decision.** One artist-facing coordinate system, called **scene space**:
center `(0, 0)`, **y-up**, the **shorter** screen edge spans `-0.5..+0.5`
(TouchDesigner-style). Units stay square on any window aspect — the longer
edge just sees more world (`±0.5·aspect`). Circles are circles everywhere.

**Why.**

- **Knob legibility is the product.** With the whole screen worth 1 unit,
  every value self-describes: `max: 0.30` = 30% of the screen, `circle(0.1)` =
  10%. This compounds in Multi-Sugar (TS/Lua inherit one story, no conversion
  tables) and in the LLM front-end (self-describing values are easier to
  generate and review). It is why ±0.5 beat NDC-style ±1: with ±1 you halve in
  your head forever, and ±1 only helps *inside* shaders — a multiply the core
  hides anyway.
- **Generative math is radial.** Rotation, symmetry, kaleid, polar
  coordinates, oscillators — all orbit the origin. Top-left coordinates force
  a `translate(0.5)` sandwich around every rotation (the p5 push/pop dance).
  Our own `feedback.wgsl` does `uv - 0.5` as its first useful line: the shader
  was already asking for the centered space.
- **Y-up** matches math intuition (sine goes up), wgpu's NDC (one flip less in
  the core), and TouchDesigner. The web's y-down is a rasterization detail —
  it stays at the boundary.

**Rejected.**

- **Pixels as artist coordinates.** A sketch written in pixels encodes the
  monitor it was born on, and breaks on retina (DPR), WASM canvases of
  arbitrary size, and hi-res export. Pixels exist only at the boundary: the
  present pass, the mouse-event conversion, and a future `px()` escape hatch
  *if* a sketch ever pulls it (hairline strokes).
- **Top-left UV (0..1) as artist coordinates.** Right for texture *sampling*
  (it stays alive inside the Braid/Hydra world's shaders), wrong as the space
  the artist thinks in — see the radial-math point above.
- **Stretch-to-fit both axes.** Would turn every circle into an ellipse on a
  non-square window. "The longer edge sees more world" is the correct
  behavior.

**The rule that falls out: three spaces exist, one is exposed.** Scene space
(the artist) · UV (texture sampling, internal) · pixels (device boundary). The
future `.render()` bridge converts geometry-scene → texture-UV; the core
converts mouse pixels → scene. Principle 4 ("hide the ping-pong, expose the
knob"), applied to coordinates.

---

## 2026-07-05 — Lessons from the dots kata (`examples/dots.rs`)

The kata: `circle(0.3).grid(32, 32).grow(mouse(), Falloff { .. }).show()` — a
grid of dots that swell near the mouse. It pulled four capabilities into the
core (the geometry-signal, instancing, input-as-signal, the first modulator)
and taught the following:

- **Input is a signal plugged into a socket, not a special case.** The mouse
  is an *argument* to the link — `grow(mouse(), ..)` — not a field inside
  `Falloff`. The socket is the design: `touch(0)` or an oscillator plug into
  the same place tomorrow without touching the knobs.
- **Modulators are declarative knob-structs, never per-element closures.** The
  tempting API — `.each(|dot| ..)` — would run once per dot per frame on the
  CPU and could never survive the Lua/TS boundary (the exact bottleneck the
  README forbids). `Falloff` crosses the boundary once per frame as uniforms;
  the WGSL runs the recipe. The anti-bottleneck stance became *structural*.
- **Instancing with zero buffers.** Every cell derives from `instance_index`
  alone; one `draw(0..6, 0..cols*rows)` call, no vertex or instance buffer is
  ever uploaded. Shapes are SDFs in the fragment shader — anti-aliasing for
  free.
- **The chain flattens into a `Recipe`.** At the terminal link, the sugar
  hands the core a small enum (`Recipe::Feedback(..)` / `Recipe::Dots(..)`).
  This is the first concrete "chain = AST" seam — where the TS/Lua dialects
  and the node front-end will plug in later.
- **Two units, both resolution-free.** Shape sizes are cell-relative
  (`circle(0.3)` = 30% of its grid cell; `0.5` = neighbours touch), so
  changing the grid from 32 to 64 keeps the picture's proportions. Distances
  (falloff `min`/`max`) are scene units.
- **`grid()` entered, `scatter()` waited.** The kata needed a deterministic
  grid, not randomness — so only `grid()` went in (Golden Rule #2). When a
  sketch pulls random scattering, `grid` may become sugar over
  `scatter(Grid { .. })`.
- **Totality held under pressure.** No `.grow()` on the chain → a falloff that
  multiplies by 1 (identity, Principle 2). `scale < 1.0` shrinks instead of
  growing — every value is valid, nothing returns `Result` (Principle 3).

**Phase note.** This kata opens **Phase 1** (the first geometry signal),
pulled exactly the way the README prescribes — by a concrete visual we wanted
to make. Phase 0's success criterion was met first: the feedback loop sang.

---

## 2026-07-05 — The bridge, layers, and the core split

Two katas pulled this round: `examples/trail.rs` (a circle follows the mouse
into a feedback loop) and `examples/rainbow.rs` (ten circles, ten hues, ten
tempos, stacked as layers). Together they brought `.render()`, `layers()`,
and the links `at()` / `wave()` / `hue()` / `soft()` — and the core grew
enough to deserve its module split.

**The bridge purified the feedback.** `.render()` turns geometry into a
texture signal (`new = swirl(previous) · decay + source`). Consequence: the
light source that lived *hardcoded* inside `feedback.wgsl` — Phase 0's
necessary hack, admitted as such at the time — died. The original feedback
kata is now expressed in the language itself: `circle().soft(1.0).hue(..)
.wave(..).render().feedback(..)`. The lesson generalizes: **when a new link
arrives, look for the hack it was born to replace.**

**A chain flattens into a `Stroke`.** One gesture = prototype + placement +
motion + paint, and position composes additively: `grid cell + at(..) +
wave(..)` — each link fills only what it touches, so links never fight over a
slot. `at()` takes `mouse()` or a constant `(x, y)` through the same socket
(`Into<Pos>`): in the dataflow view a constant is just another signal.

**Uniforms split by cadence, not by pass.** `group(0)` is the frame block
(resolution, mouse, time), rewritten 60×/s and shared by every pass;
`group(1)` is the static knobs of one stroke or one swirl, written once at
build time. This is the anti-bottleneck stance in binary form: per-frame
traffic across the CPU→GPU boundary is a handful of floats, no matter how
much is drawn. Feedback bind groups are prebuilt for both ping-pong sides —
zero per-frame allocation.

**Layers are a `Vec<Stroke>`, drawn in painter's order.** One render pass, N
draws. Honest note: ten *equal* circles could have been one instanced draw —
layers earn their keep composing *different* chains; this kata births the
mechanism with the simplest content. Blend modes (`.blend("add")`) and
texture-world layers wait for the sketches that pull them.

**The core split by layer, not by node.** `sugar.rs` (the chains) ·
`recipe.rs` (the flattened AST — the Multi-Sugar seam) · `gpu.rs` (all of
wgpu) · `shell.rs` (winit). The file structure now teaches the target
architecture from the README. Per-node files were rejected for now: nodes are
thin (a knob struct + WGSL + a match arm); the shared machinery is the bulk.
If a layer file one day bloats with many nodes, *that layer* splits per node —
modularize by pressure, not by category.

**Rejected along the way.**
- A generic node trait to unify `Passes` variants — still forbidden; the
  pressure of ~5 real nodes will reveal its true shape.
- Per-frame bind-group creation (the Phase 0 pattern) — replaced by prebuilt
  pairs; the old way was the easy path, not the clean one.
- Keeping the hardcoded feedback source alongside the bridge — two sources of
  truth for "where energy comes from" is exactly the debt Shokunin forbids.

---

## 2026-07-05 — uv convention: fullscreen uv IS texel space (the mirror bug)

**Decision.** In every fullscreen pass, `uv` means *the fragment's own texel
coordinate* (y-down, `v = 0` at the top): `uv = (x·0.5 + 0.5, 0.5 − y·0.5)`.
Write position and sample position coincide; no pass hides a flip.

**The bug it fixes.** wgpu has two opposite y-axes: NDC (drawing) is y-up;
texture sampling is y-down. The Phase 0 fullscreen vertex shader mapped
NDC→uv without inverting y, so *every sampling pass silently mirrored the
image vertically*. In the feedback cycle that meant an odd number of flips
per loop — the trail alternated orientation every frame, and the eye
integrated both: the image plus its mirrored ghost, each decaying.

**Why nobody saw it in Phase 0.** The source was a symmetric orb orbiting the
center — the mirrored ghost read as intentional kaleidoscopics. The mouse
trail kata broke the symmetry and exposed it. Lesson, again: **the example is
the test**; asymmetric input finds what symmetric beauty hides.

**The rule.** Machine coordinate systems (NDC y-up, texel y-down, pixels)
never leak past a single, named convention — the same discipline as scene
space, applied one layer down.

---

## 2026-07-05 — The oscillator menu (`Osc`)

Pulled by `examples/osc.rs`: six waveforms, TouchDesigner-style — Sine,
Cosine, Triangle, Ramp, Square, Pulse.

**Decision.** The waveform is a knob *on* `Wave` (`shape: Osc`), evaluated in
the shader, not a separate node. Conventions that fell out:

- **Quadrature.** Both axes run the same waveform, y a quarter-cycle ahead —
  so equal frequencies orbit instead of sliding on a diagonal, and the old
  sin/cos default is preserved exactly (sine in quadrature *is* cosine).
- **0 Hz = still axis.** A frequency of zero contributes nothing (not a
  frozen sample of the waveform). Total, and what a sketch means by "don't
  move on y".
- **Pulse duty is fixed at 25%** until a sketch pulls the knob.

**Deferred (the TD dream, on purpose).** Oscillators as *value signals*
pluggable into any knob — `radius(osc(..))`, `hue(osc(..))` — is the real
TouchDesigner model and the natural next step for the socket/plug design.
It's a parameter-modulation seam that deserves its own pulling sketch;
hanging it all on `Wave` today would be speculation. Per-axis waveforms and
per-axis phases were skipped for the same reason.

---

## 2026-07-06 — The tweak panel: live knobs, and our own egui renderer

> **Superseded the same day** by the entry below ("take two"). The renderer
> decision survived; the shape of the integration did not — it cross-cut the
> core and exposed every knob instead of letting the sketch pick. Kept here
> because the rejection is the lesson.

Pulled by `examples/dots_tweak.rs`: every parameter of a running sketch
editable in realtime. Two decisions worth their ink:

**Knobs became live.** Stroke uniforms were "written once"; now they are
refreshed from the recipe every frame. The recipe is the single source of
truth, and anything may retune it while the sketch runs — the panel today;
MIDI, OSC, and hot-reloaded scripts through the *same seam* tomorrow. The
cost is a handful of floats per stroke per frame; the frame/stroke cadence
split still holds (the anti-bottleneck stance survives intact).

**The renderer is ours.** egui's core is pure math and egui-winit matches our
winit — but egui-wgpu pins wgpu 29 while the core rides wgpu 30, and two wgpu
versions cannot share a device. Instead of downgrading the engine for a dev
tool, `tweak.rs` implements egui's paint contract directly (~150 lines:
textured triangles, scissor rects, one pipeline, font-atlas uploads).
"Sovereign base, hackable to the bone", made literal. Swap for egui-wgpu if
it catches up *and* earns it.

**Containment.** The whole thing is feature-gated (`--features tweak`):
optional deps, zero cost and zero compilation in default builds. Panel events
are consumed before the scene's (a pointer on a slider doesn't move the
scene's mouse). When the `tekne` workspace exists, this module is the seed of
the `vybe-tweak` addon crate (Principle 5 — modularize by dependency
weight).

**Rejected.**
- Downgrading to wgpu 29 — regressing the engine's toolchain for a panel.
- dev-dependencies with the integration in the example — would force exposing
  device/queue/passes, breaking "all of wgpu hidden in the core".
- Waiting for egui-wgpu to catch up — blocks a capability the katas want now.

**Phase note.** This amends the "UI panels" prohibition: it now means *no UI
in the core or default build*. A feature-gated dev tool that never ships by
default is a knob on the workshop wall, not product UI.

---

## 2026-07-06 — Take two: tune()/live(), and the Overlay seam

The first tweak panel was rejected in review, for two reasons that are now
rules. It auto-exposed *every* stroke field — but the artist's gesture is to
**pick** ("pinçar") the few knobs a sketch is about. And its plumbing
cross-cut the core: `#[cfg(feature = "tweak")]` in `gpu.rs`, `shell.rs`, the
`State` — the engine had learned what a panel was. Both are the same failure:
integration by invasion instead of integration by seam.

**The shape that replaced it — three small, general pieces:**

- **`tune(name, default, range)`** — picks one value, by name, from inside
  the chain: `max: tune("reach", 0.30, 0.0..=1.0)`. A std-only registry in
  the core; total (with no front-end it just returns the default). The panel
  shows *exactly* the picked knobs, nothing else.
- **`live(|| chain)`** — the sketch as a *function of its knobs*, re-described
  whenever one changes. `set_recipe` on the core makes the running picture
  follow: same-kind changes swap stroke values (cheap, every slider tick);
  structural changes rebuild passes. This is the hot-reload seam in
  miniature — the exact mechanism Phase 2's script dialects and MIDI/OSC
  will drive.
- **`Overlay`** — the one trait for anything drawn over a running sketch
  (events-first + paint-over-the-frame). The core knows the seam, never the
  UI library: zero egui types, zero `cfg`, outside `tweak.rs` and the one
  line in the shell that attaches the panel.

**What survived from take one.** The live-knob refresh (recipe → GPU every
frame) and our own ~150-line egui renderer (egui-wgpu still pins wgpu 29).
Both were correct; they were just wearing the wrong architecture.

**The rules this writes down.**
- **Integrations enter through seams, never through the core.** One trait or
  registry in the engine; the library-specific glue lives behind it, feature-
  gated or as a future addon crate.
- **Exposure is the artist's choice.** Front-ends turn what the sketch
  picked; nothing is auto-exposed by reflection.

**Addendum (same day).** A self dev-dependency (`vybe = { path = ".",
features = ["tweak"] }`) turns the feature on for examples and tests only —
no flag to type, the workshop always has the panel. Dev-dependencies never
leak downstream: the lib and its consumers stay egui-free. The `--features
tweak` flag remains for anyone consuming the crate who wants the panel.

---

## 2026-07-06 — grow() takes a source: the method vindicating itself

Pulled by `examples/lens.rs`: a grid whose swell-point you drive with tuned
`x`/`y` sliders, decoupled from the mouse.

**The story is the point.** A review (Codex) flagged that `grow(mouse(), ..)`
threw its source away and called the API "more general than the runtime". The
turn before, I had argued to *defer* generalizing it, citing Golden Rule #2
(no sketch pulls a non-mouse source yet). Then Marlus pulled it — with this
exact sketch. The defer was never "never"; it was "until a visual asks". The
first user of the language felt the specificity and supplied the puller. That
is the method working, not being broken.

**The shape.** `grow` now takes `impl Into<Pos>`, mirroring `at()` exactly.
Both placement and the grow epicenter are a `Source` (a fixed scene point or
the live mouse) — one internal type replacing the old `pos + follow_mouse`
pair, resolved in the shader by `mix(point, mouse, from_mouse)`. When
`touch(0)` or an oscillator-position arrives, it's a third `Source` variant
and nothing above changes.

**The elegant kicker: `tune` needed zero change.** Two scalar tunes compose a
position — `grow((tune("x", ..), tune("y", ..)), ..)` — because `(f32, f32)`
is already `Into<Pos>`. A vector is just scalars that travel together; no
"vector knob" primitive was invented.

**This closes finding #2** (the tune take-two round listed it as "defer"): the
source is no longer discarded — it lives in the AST and the shader reads it.

**Rejected.** A bespoke source-plumbing type built ahead of the sketch (the
speculation Golden Rule #2 forbids); a new vector-valued `tune` (scalars
compose); keeping `grow` mouse-only (the honesty smell the pull removed).

---

## 2026-07-06 — Time is wall-clock, not frame count

The temporal debt the Codex review flagged, cleared. `time = frame / 60.0`
coupled every animation to frame count, so `Wave` and `Hue.drift` ran fast on
120/144 Hz and dragged under load — the "cycles per second" / "degrees per
second" labels were lies. The feedback `Swirl` had the same disease (its
per-frame decay/angle/scale drifted with framerate).

**Decision.** The frame block carries wall-clock `time` (seconds) and per-frame
`dt` (seconds since last frame), from a `Clock` (`std::time::Instant`). `dt` is
clamped to 0.1 s so a stall or the first frame can't jolt the animation.

- **Wave / Hue** read real seconds → their per-second labels become true with
  no math change.
- **Swirl** knobs move from **per-frame** to **per-second**, applied in the
  shader by `dt`: `angle · dt` (linear), `scale^dt` and `decay^dt` (compound).
  Framerate drops out of the loop entirely.
- Defaults and the feedback examples were re-tuned from their old per-frame
  values (`x^60` for decay/scale, `x·60` for angle) to preserve the look. The
  *feel* wants a human eye — the math preserves it, but trail length is
  aesthetic.

**The `Clock` is the one wall-clock read** — the seam to swap for Phase 2/WASM,
where `Instant::now()` panics on `wasm32-unknown-unknown` (`web_time` there).
Machine time, like machine coordinates, lives behind a single named boundary.

**Also (quick win).** `mouse()` now returns to rest on `CursorLeft` (it froze
at the last edge position before), matching the far-away start-up state.

**Deferred honestly.** Exponential decay is unintuitive in *any* unit; a
future `decay` expressed as a half-life (seconds) may read better than a
survival fraction. Not now — the pull would be a sketch that makes the current
form feel wrong.

---

## 2026-07-06 — The point-cloud signal type is born (particles)

Pulled by `examples/particles.rs`: a GPU-simulated point cloud — the third
signal type, alongside texture and geometry. The trial by fire.

**Look vs. scale (a lesson the smoke test couldn't catch).** The substrate
*runs* a million particles clean, but a million white additive points on an
800px window (640k pixels) is denser than the pixel grid — it saturates to
flat white, and a uniform field shows no motion. Two fixes: a readable density
(the example seeds ~80k, dimmed and small) and a coherent **rotational field**
in the compute step so the cloud visibly swirls. The count is a knob; a million
is a luminous-fog setting, not the default. Smoke tests prove "no crash", never
"looks right" — the eye is still the test.

**The shape.** A `Points` recipe carries a count; the core builds a storage
buffer of `Particle { pos, vel }`, a **compute** pipeline that steps it, and an
instanced render that reads it. Two passes per frame: dispatch the step
(`count / 64` workgroups), then draw one additively-blended quad per particle.

**The principles held, literally:**
- **Signal stays the single currency** (P1). A point cloud is a `Signal` whose
  *payload is a GPU buffer* — geometry is a recipe of strokes, texture is a
  texture, points are a buffer. Same chain, new payload, `Particles` is a
  `Chain`/`Flatten` like the rest.
- **Anti-bottleneck, made real.** The sketch sets the count once; the
  per-particle data lives on the GPU and never crosses back. The compute step
  reads and writes the buffer in place; the CPU touches nothing per-frame,
  per-particle.
- **Zero-buffer instancing, again.** The draw uploads no vertex/instance data —
  the vertex shader indexes the same storage buffer the compute step wrote,
  by `instance_index`.

**Decisions baked in:**
- **Storage buffers + compute, not the position-texture hack.** We're
  WebGPU-native, so particles live in a real `array<Particle>` in a storage
  buffer — the debt WebGL-era tools carried (encoding position into RGBA
  textures) is skipped, like instancing from day 1. Verified `Limits::default`
  gives 8 storage buffers/stage.
- **Deterministic seeding, no `rand` dep.** Initial positions/velocities come
  from a PCG-style hash of the index — reproducible sketches, one fewer
  dependency (Principle 5, the discipline of what you refuse to add).
- **Separate read_write (compute) and read-only (vertex) storage layouts** over
  the one buffer — read-write storage in a vertex stage is a portability
  hazard; the two-layout split keeps the draw honest.

**Not yet (the next katas pull them):**
- `shape.wgsl`'s circle SDF was *not* shared yet — the point render uses its
  own tiny soft-dot fragment. The WGSL prelude extraction waits for the moment
  the duplication actually bites (a third user).
- Fluid is this substrate with the force swapped for **curl-noise**; voronoi is
  a fullscreen pass seeded by the buffer (a points → texture bridge). Both
  reuse everything here; that reuse is the proof the substrate is right.

---

## 2026-07-06 — Direction: vibe-coding-first; import tekne-flow's language, not its editor

A strategic alignment after reading the tekne-flow-2 prototype (Svelte + SvelteFlow + Paper.js). Marlus's call: he has cooled on the **node-based editor** as the interface and now bets on **vibe coding** (natural language → the chain, via an LLM) as the differentiator. The WebGPU core stays; it became vybe. The Tekne Flow node editor is *deferred, not killed* — it may return later as one front-end **on top of vybe** (which was always the README's endgame: node-graph is one of several front-ends, not the center).

**The precedent.** The Quartz Composer lineage (Vade, Reza Ali) resolved to **code frameworks** — Satin, Fabric in Swift/Metal — not node editors, for expressive creative work. Vibe coding is the natural-language layer *over* a code framework. vybe (the code framework) + vibe coding (the interface) is that stack, current and coherent — and more aligned with vybe's own thesis ("the sugar syntax is the product") than a node editor ever was.

**The principle this sets.** *Vibe coding raises the value of the **language** — a rich, orthogonal, LLM-legible vocabulary — and lowers the value of the visual **editor**.* So from tekne-flow we import the language and drop the editor.

**Import (the language, pulled by katas — see ROADMAP):**
- The **columnar `Stream`/`ColumnStore`** model: evolve `Signal`/`Stroke` toward per-element attribute *columns* (`x, y, index, t, scale, noise…`) in SoA. It maps 1:1 to GPU storage buffers — the particle buffer is already vybe's first de-facto ColumnStore — and it's what turns a baked `Falloff` into a composable chain.
- **Normalized `t` (0→1)** as a first-class column — tekne-flow's signature win over integer-only indexing; trivially LLM-addressable ("color by t").
- **Generator / transform separation** — generators emit pure geometry; scale/rotation/offset are dedicated composable steps (matches "each link fills only what it touches").
- **Late-binding style** — color/size/etc. as a recipe resolved at render, drivable by any context column (`$ref: t → gradient`), not baked scalars.
- **The composable value vocabulary** — `distance`, `map` (remap), `noise`, `sin/cos`, `modulo`, `instance` (points × shape). These are the verbs an LLM composes; orthogonality and legibility are the product.

**Do NOT import (the editor machinery — obsoleted by conversation-as-interface):** SvelteFlow, GraphAPI, ELK auto-layout, dynamic output ports, subgraph copy/paste, one-connection-per-input validation, the visual node UI. "Chain = graph" survives as a possible *read-only* visualization; authoring is text/voice.

**Implication (reweights the roadmap):** vibe-coding-first raises the priority of the **TS/JS sugar dialect + WASM** — LLMs are most fluent in TS/JS, and vibe-coding a web-native creative tool wants the browser. The core stays Rust; TS becomes the dialect people actually vibe-code in. Node-graph front-end drops in priority.

**Rejected.** Building the node editor now (the interface bet moved); chasing the docs' unbuilt vector ambitions (kurbo/SDF/Vello/text — years of work, and the docs' elaborate specs for nonexistent nodes are the cautionary tale). Golden Rule #2 still governs: a real piece pulls the minimum.

---

## 2026-07-06 — Particle behavior as a force stack (the cheap slice, applied)

Pulled by the critique that `particles(80_000).show()` hid everything — the
behavior (an ambient swirl, the mouse repel/orbit) was hardcoded in the shader,
the Phase 0 feedback-source hack reincarnated. Fixed by moving behavior into the
chain as **composable forces**: `swirl`, `gravity`, `attract`, `repel`, `orbit`
— the same declarative-knob pattern as `Falloff`/`Wave`/`Swirl`.

**Shape.** The chain builds a `Vec<Force>`; the core uploads a small fixed
uniform array (cap 8) bound to the compute step, which loops and sums them each
frame. Local forces (`attract`/`repel`/`orbit`) take a `Pos` source — so
`repel(mouse(), ..)` and `repel((x, y), ..)` share the `at()`/`grow()` plug, and
the mouse reaches the GPU sim through the frame block (no per-particle CPU work).
New behaviors are new sketches (`particles_galaxy`, `particles_swarm`, `particles_rain`), not new core.

**This is the LEGACY_FLOW "cheap slice," made concrete.** A *bounded,
declarative* force vocabulary — emphatically not the general value-flow we
deferred. The neat sugar held: `particles(n).swirl(..).repel(mouse(), ..)`.

**Live tuning without reseeding.** `set_recipe` gained a `Points` arm: when the
count is unchanged, it rewrites only the tiny force uniform and keeps the
particles' live positions — so `live(|| particles(n).swirl(tune(..))..)` drags a
slider without the cloud jumping back to its seed. Same `tune`/`live`/Overlay
seam as `dots_tune`, now reaching the point cloud.

**Gotcha logged.** `target` is a reserved word in WGSL — a force helper's local
had to be renamed. Smoke tests catch it (shader validation aborts at start-up);
the eye would not.

---

## 2026-07-07 — Compositing: layers() stacks worlds, not just geometry

Pulled by `examples/aurora.rs`: a rainbow orbit that leaves feedback trails,
over a still field of dots that swells toward the mouse — two worlds in one
frame, feedback on *one* of them. The question that pulled it ("can I mix and
keep feedback on only one layer?") had no answer in the old engine: one
`Recipe`/`Passes` per window, and `layers()` only stacked geometry into a
single pass. This is the kata that finally pulled **mixed worlds (texture
layers)** — the item the `Layers` doc-comment and the roadmap had parked.

**`layers()` generalized instead of a new verb.** The artist's stated
preference: one word that does the right thing. `layers()` now takes anything
`Into<Signal>` — a bare `Shape` (lifted through the bridge, implicitly) or a
full `Signal`, feedback and all — and the *flatten* decides the path:

- **All plain geometry** (no layer carries a swirl) collapses into one
  `Recipe::Shapes`, painter's order — byte-for-byte the old behavior, so
  `rainbow` and `rainbow_trails` don't regress and don't pay for compositing.
- **Any feedback layer** switches to `Recipe::Composite(Vec<Recipe>)`: each
  layer renders to its own signal texture; a compositor sums them onto the
  screen.

**A layer is a `Signal` (Principle 1, again).** No new "Layer" type was
invented — the texture-world currency already *is* the unit of a layer, and
`From<Shape> for Signal` (the bridge, made implicit) lets a `map` of shapes and
an array of mixed worlds both satisfy one `Into<Signal>` bound. When a *particle*
layer is one day pulled, `Layers` widens from `Vec<Signal>` to a broader layer
type; the compositor already takes `Vec<Recipe>`, so only the sugar moves.

**Additive is the default, and it needed no new shader.** The compositor is
`present.wgsl` drawn once per layer with additive blend: black adds nothing, so
wherever a trail has decayed to dark the dots show through. This fell out of the
shaders as they already were — `shape.wgsl` writes real SDF alpha, `feedback.wgsl`
decays to ~black where there's no light — so **alpha-over would have needed the
feedback pass to invent an alpha channel; additive did not.** It also matches the
point cloud's "crowds glow". Blend-mode *selection* (`.blend("over" | "screen")`)
is the next knob on this seam, not a rewrite of it.

**A feedback layer is the standalone loop minus its present.** The compositor
plays the present's role for the whole stack, so a composited feedback layer
reuses the exact ping-pong dance (`build_feedback_targets`, the prebuilt
`present_bgs` *are* its per-frame composite bind groups) and just skips the final
blit. The single-world `Passes::Feedback` path stays untouched and proven.

**The refactor that came free.** Pulling out `build_shape_pass`,
`make_swirl_uniforms`, `make_linear_clamp_sampler`, `additive_blend`, and the two
layout builders de-duplicated the Shapes/Feedback/Points arms *and* fed the
compositor from the same helpers — one source of truth per GPU idiom. `Layer`'s
feedback variant is boxed: unlike `Passes` (one per window), `Layer` lives in a
`Vec`, so the size gap that's harmless there is real here.

**Rejected.**
- **A separate `stack()`/`compose()` verb.** Two names for "put these together"
  is the friction the artist asked to avoid; the engine picking cheap-vs-rich by
  the stack's contents is the cleaner story (and keeps the fast path honest).
- **Alpha-over as the default.** Correct-looking only if the feedback pass grows
  a real alpha channel — surgery on a proven shader for a default additive gives
  for free, and that reads right for luminous-on-black content.
- **Unifying the single-world paths under the compositor now.** Tempting ("one
  layer + present" describes everything), but it would re-plumb three proven
  paths for elegance the kata didn't ask for. Smallest clean change: add the
  `Composite` path beside them; unify later if a sketch makes the seam obvious.
- **Live-tuning a composite.** `set_recipe` rebuilds a composite wholesale (it
  isn't a `live()` sketch yet); the per-layer swap-without-reseed optimization
  waits for the kata that turns a knob on a composited layer.

---

## 2026-07-08 — Blend modes, per layer; and the rule: everything composes and feedbacks

Pulled by wanting `over` (TouchDesigner's alpha compositing) instead of only the
additive glow the compositor shipped with, then by wanting **mixed** modes in one
stack (`add` on one layer, `over` on the next).

**Blend is a *relation*, spoken at the composition boundary.** How a world lands
on the stack is not a property of the world — the same `field` could stack
additively here and over there. So the mode lives on the layer, set by a final
link `world.blend(Blend::Over)`, with `layers().blend(..)` as the stack-wide
default a layer falls back to (`l.blend.unwrap_or(stack_default)`, resolved in
`flatten`). Rejected `.blend()` as a link *inside* the world's chain (couples a
pure world to one stacking context) and a leading `layer(mode, [..])` wrapper
(a second constructor beside `layers()`, and Rust's homogeneous arrays force
every item to the same type either way — so the trailing link reads cleaner and
stays symmetric with the stack-level verb already shipped).

**A new `Layer` type, at last — because `Signal` stopped being enough.** The
compositing entry (2026-07-07) predicted `Layers` would widen "when a particle
layer is pulled". Per-layer blend pulled it first: a layer is now `{ signal,
blend: Option<Blend> }`, and `layers()` takes `Into<Layer>` (`Shape`/`Signal`
lift with no blend = inherit). This is the seam particles slot into next.

**`over` needed the alpha the additive default let us dodge.** The compositing
entry rejected alpha-over precisely because it "would need the feedback pass to
invent an alpha channel". Shipping `over` paid exactly that price, and it was
right to: layer textures now clear to **transparent** black (not opaque), so
empty regions carry no coverage; `feedback.wgsl` now carries alpha as trail
energy (dark = transparent), so a feedback layer composites `over` by where its
light actually is. Additive ignores alpha, so every existing sketch is
unchanged. A dedicated `composite.wgsl` (samples premultiplied rgba) keeps the
single-world `present.wgsl` path — an opaque final blit — byte-identical. One
compositor pipeline per blend mode (same shader, different blend state), picked
per layer each frame; no per-frame rebuild.

**The rule this crystallized (artist's words): _everything is composable and
feedbackable._** Every signal type — geometry, texture/feedback, and the point
cloud — must be stackable in `layers()` *and* able to source a feedback loop.
Today geometry and texture satisfy both; the point cloud satisfies neither yet
(it renders straight to screen, and isn't a `Signal`). That gap is now a stated
debt, not an oversight: the next particle katas are "particles as a composited
layer" and "particles into `.feedback()`", and the `Layer` type + the recipe's
already-recursive `Composite(Vec<CompositeLayer>)` are the seams they enter
through. See CLAUDE.md design principles.

**Rejected.**
- **`.blend()` inside the world chain**, and **`layer(mode, [..])`** — see above.
- **`screen`/`multiply` now.** Both are pure fixed-function (`One,OneMinusSrc`;
  `Dst,Zero`) and trivial to add, but no sketch pulls them yet; the `Blend` enum
  has room. (`multiply` also blacks out where a layer is empty — a footgun for a
  layer stack, worth a deliberate kata.)
- **Per-*group* blend trees** (`(1 add 2) over 3` as a nested unit). The flat
  per-layer model (each layer onto the running accumulation) covers the katas so
  far; nesting a `layers()` inside a `layers()` waits for a sketch that needs the
  group-as-a-unit — the recursive recipe already permits it.

---

## 2026-07-08 — Particles compose; `layers!`; and stacks list top-first

Three moves in one session, all pulled by making the blend examples beautiful
with particles (`over`, `blend`).

**Particles are a composited world now** (closing half the "everything composes"
debt from earlier today). The point-cloud build — compute step + instanced draw
— was extracted into a reusable `PointsPass` with a `render(encoder, target,
frame_bg, clear)` method, so the *same* cloud renders whether it targets the
screen (`Passes::Points`) or a layer's offscreen texture (`Layer::Points`). The
sugar `Layer` widened from holding a `Signal` to a `World` sum (`Signal` |
`Particles`); the recipe's `CompositeLayer` already carried any `Recipe`, so the
core recursion needed nothing. A points layer clears transparent and draws
additively into its own texture, so `over` reads its accumulated coverage. (Not
yet done: particle *colour* — points are white — and particles into
`.feedback()`. Both are logged wishes.)

**`layers!` macro — because Rust arrays are homogeneous.** Mixing world *types*
in one stack (a `Shape` beside a `Particles`) can't be an array literal: its
items must be one type. `.blend(..)` incidentally unified them (all became
`Layer`), which made blend feel *mandatory* on mixed stacks — a wart the artist
hit twice. `layers![a, b, c]` calls `Layer::from` on each item independently
(reflexive `From<Layer>` lets an already-`.blend(..)`ed layer pass through), so
bare worlds (default blend) and per-layer overrides mix freely. The function
`layers(iter)` stays for iterators (a `map` over a range). Rejected a no-arg
`.layer()` converter (shipped it, then removed it same session): the macro does
the same job better, and two ways to unify is the friction to avoid.

**Stacks list top-first (reversed the 2026-07-07 "bottom to top").** The artist
asked: "I want item 0 above 1 above 2." A `layers([..])` now reads like a
Photoshop/After Effects layers panel — **item 0 is the frontmost** — not like
imperative draw calls (background first). The core still draws back-to-front;
`flatten` (and `Layers::render`) `.rev()` the list into draw order — the single
place the two orders meet, so nothing downstream changed. Consciously reverses
the earlier decision: a *declarative* layer stack reads top-down like every
design tool, which beat the p5/Hydra "later call = on top" now that `layers` is
a described list, not a sequence of draws. `layers_aurora`, `layers_over`, `layers_blend` reordered to
match; the additive `.map()` sketches (`rainbow*`) are order-invariant, untouched.

---

## 2026-07-08 — `Over` becomes the stack default (reverses this morning's `Add`)

The blend enum shipped hours ago with `Add` as the default — the glow the
compositor was born with. Living with it for one session surfaced the tension:
we had just decided a stack **reads like a layers panel** (top-first), and a
layers panel's default blend is *Normal* — over. With `Add` as the default the
metaphor stopped halfway: additive compositing is commutative, so the order we
made load-bearing changed nothing in a bare stack. `Over` is the mode where
listing something *first* means something. Defaults should complete the mental
model they invoke, so over won.

**Three reasons, one direction.**
- **The metaphor pays off.** Photoshop, Figma, After Effects, TouchDesigner's
  comp, even Hydra's `layer()` — every tool the artist arrives from composites
  over by default. A bare `layers![a, b]` now does what a panel does: `a` in
  front, covering `b` by its own coverage.
- **The cheap path becomes exact.** Painter's-order alpha *is* over (over is
  associative, so per-stroke blending equals per-layer compositing — the same
  image, not an approximation). Under `Add`-default the all-geometry single
  pass was quietly rendering over-semantics while the API said "add"; under
  `Over`-default the label and the pixels agree. An explicit `.blend(Add)` on
  geometry now honestly composites (true sum of light) instead of silently
  taking the alpha path.
- **It faces where the engine is going.** The design bar (vector, graphic,
  Tati-level work) is an over-shaped world. The light-synthesis lineage
  (Braid/Satin feedback, glow) keeps its mode one visible link away — and a
  stack of light *saying* `.blend(Blend::Add)` reads as intent, not accident.

**Migration (the whole cost, today).** `layers_aurora` spells `.blend(Blend::Add)` —
its sky is light summed onto the floor, and now says so (it doubles as the
stack-wide `.blend()` demo). `over` drops its now-redundant `.blend(Over)` and
demonstrates the default. `blend` flips which layer differs: the swarm spells
`add`, the disc rides the default. `rainbow`/`osc` (all-geometry, cheap path —
byte-identical either way) and `rainbow_trails` (folds via `.render()`, blend
subsumed) are untouched. The flip itself: move `#[default]`, flip the cheap-path
predicate (`all_add` → `all_over`), docs.

**Rejected: keeping `Add`.** Defensible — it's the engine's aesthetic lineage,
and every sketch so far is made of light. But the default would then serve the
engine's past instead of the artist's arrival: single-digit sketches was the
cheapest this flip would ever be, and waiting meant more stacks encoding the
old default. Deciding now, while the API is a day old, is the point.

---

## 2026-07-08 — The crate is renamed `poiesis` → `vybe`

The engine crate is now **`vybe`** (was `poiesis`). The old name named the
*making* — *poiesis* (ποίησις), the Greek counterpart of *tekne*. Elegant, but
it pointed at a general act of creation, not at what actually differentiates
this project. The 2026-07-06 direction settled that: the bet is **vibe coding**
— natural language → the chain, via an LLM — not a node editor. The name should
carry the thesis. `vybe` does: a play on *vibe coding*, read as a backronym for
a **V**isual **E**ngine. It's short, lowercase, sits on the same shelf as Hydra
and Braid, and is free on crates.io (`vibe` is taken; `vybe` was not).

**Scope.** Crate name and every code reference (`use vybe::*;`, device label,
recipe labels, the self dev-dependency), plus all docs. The rename is mechanical
everywhere except the naming rationale (README, the glossary row), which was
rewritten rather than find-replaced — *vybe* is not a Greek word, so sed-ing
"*Poiesis* (ποίησις): the act of making" would have produced nonsense.

**What did not change.** **`tekne`** stays the brand/home and the future Cargo
*workspace* name; only the crate underneath it was renamed. The old
poiesis/tekne pairing was etymological (two Greek words); that link is gone, but
`tekne` stands on its own as the umbrella — `vybe/` still becomes a workspace
member the day a second project exists.

**Housekeeping.** Publish a minimal `vybe 0.0.1` early: crates.io names are
first-come and cannot be reserved without a publish, so claiming `cargo add
vybe` before the name leaks is cheap insurance.

---

## 2026-09-20 — 0.0.2: the patch is the product; the chain is how it is born

A design day (2026-09-19, recorded in the 0.0.2 design brief) that began as
"which Raspberry Pi for a media player" ended as a new thesis. 0.0.1 said *the
chain syntax is the product.* 0.0.2 says **the patch is the product; the chain
is how it is born.** A patch is a plain-text file (`.vy`): one line per node,
wires by name, scenes and transitions, one output. The first real work — a
projected cube, four faces, sensor-driven scenes — is the kata that pulls
it all: scenes, Scalars, keystone, headless render.

**How we got there.** We wrote one cube face six ways. A declarative state
machine (`scenes().edge().on()`) was elegant on paper and hard to debug. An
`async` script (`show(x).until(gate).await`) read like a screenplay and was the
worst to verify. Imperative-with-helpers (`Fader`, `Ramp`, `Gate`) was the best
*Rust*. Then the question moved from "sugar for a person" to "sugar for an LLM",
and the answer was not syntax at all: it was a **verification loop** (headless
deterministic render, scripted inputs, static checks) plus a **closed, tiny
vocabulary**. That is what a `.vy` is: the whole face in ~25 lines a human can
read and a model cannot get wrong.

**Principles adopted.**

1. *One currency, many surfaces.* A `Signal` (texture over time) is the only
   thing that crosses between modes; chains, patches and (later) imperative
   sketches all produce and consume it.
2. *Hydra outside, p5 inside, when needed.* Composition, time and transitions
   are declarative. Whatever needs a `for` or a mutable list is a `rust <name>`
   leaf returning a texture. Leaves never contain chains; chains never contain
   closures.
3. *Effects live in chains; behaviour lives in Rust; the bridge is a number or a
   texture.* `Fader` never becomes a chain op; `feedback` never becomes a draw
   call.
4. *The vocabulary is closed.* Twelve grammar rules. Words enter only as drawing
   primitives or Scalar operators — never as application nouns. (We caught
   ourselves inventing `points`, `pick`, `nudge` for the keystone remote. The
   correction: corners are `tune`s; "arrows nudge a tune" and "the mouse drags a
   tune pair" are *generic bindings*; test patterns are ordinary scenes switched
   by an ordinary input.)
5. *Verifiable beats elegant.* For LLM authorship the product is `vybe render` +
   `vybe check` + `vybe api`, not the syntax. Katas ship as `.rs`/`.vy` pairs,
   rendered headless and compared pixel for pixel.

**Rejected:** `async/until/await` as the behaviour model; the `scenes().edge()`
DSL; a separate `vybepi` library (embedded support is crates in the same
workspace, *cut by dependency, not by platform*); UI nouns in the grammar;
ternaries in the grammar.

This supersedes the Phase 1 "Do NOT add" list where they collide (a text
front-end, an OSC integration, a CLI). What it does **not** supersede: examples
pull features, integrations enter through seams, the core stays LLM-sized.

### What the first implementation decided

The brief settled the design with zero lines written. These are the judgment
calls made while writing them — each one a place the brief was silent,
ambiguous, or (twice) overruled.

**Patch above Recipe, not instead of it.** The brief says "`Recipe` becomes
serialisable; `.vy` is its text form." Built, it is two layers: the **`Patch`**
is the AST (serialisable, holds Scalar *expressions*, scenes, transitions); the
**`Recipe`** stays what the GPU draws *this frame*, with numbers. A **`Player`**
sits between: each frame it moves its objects and emits a `Recipe` with this
frame's Scalars resolved. This is `live()`'s seam, generalised — `live()`
re-describes when a knob turns; a player re-describes every frame — so the
GPU core learned nothing new about *time*. When the Rust sugar grows Scalars,
it will build a `Patch` too; today's `Shape → Recipe` is the degenerate case.

**Key = identity** (`Recipe::Named`). The brief's §3.3 ("state survives
re-description; nodes are keyed by name") turned out to be the same decision as
"what is a wire". A named node renders **once** per frame however many scenes
reference it, and its GPU state (a feedback trail, a particle buffer) survives
for as long as its name keeps appearing. Consequence worth the whole design: in
the cube face, `idle` shows `ss` and `touch` shows `ss*(1-t)` — it is *the same* `ss`,
so the cut between them is seamless, trail and all. Unnamed worlds are keyed by
their path in the tree, which also closes an old ROADMAP debt for free: a
`live()` composite no longer rebuilds wholesale when a knob turns.

**One node list replaces `Passes` + `Layer`.** The GPU core had two parallel
hierarchies (top-level `Passes`, per-layer `Layer`), each with its own
feedback/points/shapes variant. Now a recipe flattens to one list of nodes in
dependency order; each renders into its own signal texture; a mix node samples
earlier ones; one present pass lands the last on the output. Costs one extra
fullscreen pass for a bare circle. Buys: nested composition, per-layer opacity,
headless output, and the keystone — all through the same path. Pipelines moved
into a per-device `Kit` (they used to be rebuilt per layer).

**The present pass *is* the keystone.** A 4-corner homography is exact for a
flat face, so the warp is one quad whose vertex stage emits homogeneous
coordinates (the GPU then interpolates perspective-correctly). At rest it is a
plain copy. The `keystone` crate holds only the *file* (JSON, atomic save,
`.bak`) and depends on nothing; the math lives with the pass, in the core's
`stage`. (The first cut warped the whole output; see the next entry.)

**Time semantics (the brief's risk #6), decided:** a *level* holds while its
condition holds (`off`, `done`, `= N`); an *edge* is true for one update
(`rise`). **A transition fires on the frame its whole condition *becomes* true
while its `from` scene shows** — never again until it has been false. Pure
levels were tried on paper and fail immediately: `* -> idle  mode = show` would
pin the face to `idle` forever, because `/mode` still *reads* `show` long after
the message arrived. Ramps are linear and clamped so they reach `0` and `1`
*exactly* (`t = 1` is a condition you can build on); ease afterwards with
`smooth(t)`. Objects advance by an explicit `dt` and never read a clock, so a
fixed step replays a performance exactly.

**A scene crossfade adds, it doesn't cover.** Scenes composite with `add` and
weights summing to 1 — an exact dissolve whatever the scenes' own transparency.
`over` would dip to dark mid-fade between two opaque scenes.

**The patch is identical on the laptop and on the wall.** Two temptations
refused, both grammar creep: a `key` source for the sensor stand-in, and an
`out window` variant of every patch. Instead the window writes keys and mouse
into the *same address space OSC lands in* (`/key/space`, `/mouse/x`), the CLI
maps one onto the other (`vybe run face.vy --key space=/hands`), and `out kms …`
falls back to a window of the same shape where there is no KMS. Nothing in a
patch says which machine it is on.

**Overruled: a node named after a word of the language.** The brief's own sample
has `frames = frames transicao/*.png`, then `frames@smooth(t)`. That makes
`frames` at the head of an item ambiguous between the wire and the source word —
resolvable only by definition order, which is exactly the kind of rule a model
gets wrong. Vocabulary words can't name nodes; the error says `rename it`. (The
fixture uses `trans`.) Unambiguous beats faithful-to-the-sample.

**Overruled: chumsky.** The grammar is twelve line-oriented rules. A hand-written
parser keeps the core dependency-free and affords what a combinator can't:
on-demand lexing (so `*` is a multiply in `ss*(1-t)` and a glob in
`transicao/*.png` with neither needing quotes), every broken line reported in
one pass, and errors that say what to write instead. The vocabulary is *data*
(`patch/vocabulary.rs`): the parser, the checker's hints and `vybe api` all read
the same tables, so they cannot drift.

**One seam for every integration: `Binding`.** A binding sees window events and
every frame, and may touch the `Inputs`, the `Warp`, and the `tune` registry.
OSC (`vybe-io`), both ends of the remote protocol (`vybe-remote`), scripted
inputs for headless runs, the CLI's key mapping, and the two generic bindings
(`arrows_nudge`, `mouse_drag`) are all just implementations. The core never
learns what a socket is. `tune` grew the front-end API they share
(`get`/`set`/`names`).

**`live()` returns a `Stage`; sketches end in `.show()`.** The brief writes the
remote as `live(|| …).with(…).on(…).show()`, and there was nowhere to hang a
binding on a function that opened the window and never returned. It is more
coherent anyway — every sketch now ends on the same terminal link — but it is a
breaking change with a silent failure mode (`live(|| …);` compiles and does
nothing), so `Stage` is `#[must_use]` and CI's `-D warnings` makes that an
error. Three examples gained one line each.

**Deferred, and the checker says so rather than failing quietly:** `video`
(GStreamer, 0.0.3 — the error prints the `ffmpeg` line that makes a PNG sequence
instead), `text` and `rust` leaves (they arrive with `Draw`), audio (`mute`,
`vol`, a transition's sound: parsed, noted, ignored), `vybe fmt`,
`Recipe::to_vy()`, `vybe check --scenario`, mDNS discovery, `Param`
persistence. Until `text` exists, the remote's status line lives in its window
title.

**Media renders itself.** The `map-show` patch needs PNG sequences; they are
produced by `vybe render` from two small generator patches
(`examples/patches/map-show/make-media.sh`), so the repo carries no binaries and
the headless path is exercised by its first real user.

---

## 2026-09-20 — The picture has a size of its own; the keystone maps *it*

The first keystone pulled the corners of the **whole output**. It rendered a
correct homography and still felt broken the moment a hand was on it: the face
being mapped is a square, the projector is 16:10, so the handle you dragged was
the corner of a *black bar*, and the square's corner went somewhere else. The
saved file proved the protocol worked; the picture proved the model was wrong.

**The decision.** A picture may have a size of its own, apart from its output's:
`out … 1920x1200  picture 1200x1200`. The core renders at the picture's size; the
present pass lands that texture in the output through the warp; and so **a
keystone's four corners are the picture's corners** — measured on a render, they
land where asked to within a pixel. Uncalibrated, a picture *rests* contained and
centered (`Warp::fit`), which is also what `/keystone/reset` returns to. Without
`picture`, the picture is the output and everything behaves as before.

**Where the size is said: the `out` line, not the keystone file.** The brief put
`source {w,h}` in `keystone.json`. But the picture's shape is a property of the
*work* — the patch composes in a scene space whose proportions it must know — and
calibration is a property of the *room*. One patch, many rooms: the shape belongs
with the patch. It is an argument of `out` (rule twelve), not a thirteenth rule.

**Named `picture`, not `source`.** Inside vybe a *source* is already `circle` /
`frames` / `video` (the vocabulary's word class). The glossary already calls what
lands on the wall "the picture". One meaning, one term.

**One calibration tool for every shape.** The remote hard-coded 16:10. Now the
face announces its shape (`/keystone/shape ow oh pw ph`, sent with every state)
and the remote draws the output's frame and the picture's quad in their true
proportions — a 16:9 screen (`map-screen`) and a cube's square face (`map-cube`)
are the same program pointed at different patches. `/keystone/state` itself is
untouched (8 floats + feather), so the brief's TouchDesigner-compatible message
stays compatible; shape is a separate, ignorable message. To make the remote's
geometry live, the generic bindings learned to ask for their box and step at
event time (`within_with`, `step_with`) — still generic, still not UI nouns.

**Examples carry a background colour.** A picture on black is indistinguishable
from the projector's own black, which is exactly what hid this bug. The mapping
patches now paint the picture edge to edge, so where it ends is visible.

*Left open:* with a picture of its own size under a warp, the window's mouse is
not un-projected back into the picture (`at(mouse())` would be off). A face on a
wall has no mouse; the day a warped, mouse-driven picture exists, invert the
homography at the boundary.

---

## 2026-09-20 — Hot reload: state survives an edit, by name, on both sides

`vybe run` now follows the patch's file as it is saved. The brief called
"state survives re-description" *the most important core decision* the
installation forces; this is its second half. The first half was the GPU's
(key = identity: a node whose name and kind survive keeps its feedback trail).
This half is the player's, and it is the **same rule**: `Player::reload` builds a
fresh player from the edited patch and carries over, *by node name*, the gate and
ramp states, this frame's values and symbols, the playhead of any sequence that
is still the same files, and the scene that was showing (if it still exists).

Three choices worth keeping:

- **Objects are retunable, not rebuilt.** `Ramp::set` / `Gate::set_debounce` are
  applied every frame, so editing `up 3s` to `up 1.75s` takes effect on a ramp
  that is mid-climb *without* resetting its value. (It also means a duration can
  be a Scalar.) A node that changes *kind* simply starts over as the new kind.
- **Editing a condition never fires it.** After a reload every transition's
  memory is set to what its condition reads *now*, so a newly written
  `two -> one  go = 1` whose condition is already true waits to *become* true.
  An edit is not an event.
- **A broken save changes nothing.** A file that doesn't parse or check prints
  its findings (warnings and errors only — notes were read at start-up) and the
  last good patch plays on. On a wall, a typo must not black out a face.

Polling the file's mtime four times a second, from the player itself: std-only,
no watcher dependency, no thread. Headless renders never watch — a render is one
fixed take. A changed `out` line (size, port) says "restart to apply".

---

## 2026-09-20 — Video, with its sound: a player behind a seam, not a decoder

A real clip arrived (1080p, 2 min 18 s, stereo) and settled two things at once.
`frames` keeps every frame as a GPU texture — fine for a 4-second transition
that gets *scrubbed*, absurd for 4,100 frames (~33 GB). And the requirement was
said plainly: **video with sound, not just frames.**

**The seam: `Clip` and `Decoder`** (`src/clip.rs`, std-only). A clip is something
that *plays*: restart, pause, volume, `done`, and "a frame, if there is a new
one". The division of labour is the point: the **player** owns a clip's
*behaviour* (when it restarts, how loud, whether a transition may fire on
`done`); the **GPU** owns one texture per clip, refilled when a frame arrives —
so a clip costs the same memory whatever its length; and the clip owns decoding
and **its own audio/video sync**. The engine never paces sound. Frames cross in
a `Slot` shared by `Arc`, which is key = identity again: the same slot across
re-descriptions is the same GPU node.

**GStreamer, not ffmpeg.** The first instinct was an ffmpeg subprocess piping raw
frames. It is the wrong shape of tool: ffmpeg *decodes*; it hands you frames and
samples and leaves you to build the player — A/V sync, an audio device, looping,
seeking, volume. With sound in scope that is reinventing a media player, and the
hard part (sync) is exactly the part we would own. GStreamer *is* a player:
`playbin` decodes (in hardware where there is any — VideoToolbox here, V4L2 HEVC
on a Pi 5), keeps picture and sound in sync on its own clock, and drives the
speakers. `vybe-video` is the glue: an `appsink` at the end of the pipeline,
RGBA out. And it is the *same* backend on the laptop and on the wall — the
brief's choice for the Pi — with a zero-copy path (DMA-BUF) open later that a
subprocess could never have. Considered and set aside: linked libav
(`ffmpeg-next`, `video-rs`: decode only, and a build welded to a libav version),
`libmpv` (a fine player whose render API wants OpenGL, not wgpu), pure Rust
(`symphonia` + `openh264`: no system dependency, but our sync, H.264 only, no
hardware decode — and the Pi needs HEVC), `gpu-video` (hardware decode straight
to wgpu, elegant, but Vulkan-only). The cost — a system library — is contained:
it is one crate, and `vybe-cli --no-default-features` builds without it.

**Two paces.** *Live*: the pipeline runs on its clock, with sound; the engine
takes whatever frame is newest, and late frames are dropped, never queued — the
picture is always *now*. *Offline* (headless): no clock, no sound; frames are
pulled in order and the one that belongs at exactly the asked time is returned,
so `vybe render` stays "same input, same pixels" even with video.

**A video is as loud as its scene is visible.** The brief asked that audio weight
follow visual weight. It fell out of the `Fader` for free: a video's volume is
`vol` × the weight of the most visible scene that reaches it. A crossfade is a
crossfade of sound; a video no scene shows is paused and silent, and a
play-once video restarts when its scene is entered — the same rule `frames`
already followed. `vol` and `mute` stopped being notes and became words that work.

**The checker asks what *this* program can play** (`check_with(.., Support)`).
With a decoder, `video` needs only its file. Without one, the error says how to
install GStreamer. `@` on a video is refused for now: a video plays on its own
clock with its sound; scrubbing is what `frames` is for.
