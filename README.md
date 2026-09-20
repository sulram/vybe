# vybe

## Creative Coding Engine

> A Hydra with the soul of Braid, a base of Rust, and many tongues —
> but one that starts, as they all did, with a single shader on screen.

A sovereign creative-coding engine in **Rust + wgpu (WebGPU)**. One core, many
front-ends — **the patch is the product; the chain is how it is born.** Geometry,
feedback, layers, and a GPU point cloud are all one `Signal`: they compose, and
the chain never breaks. Native today (macOS/Windows/Linux); a Raspberry Pi on a
wall next.

## The patch

A work is a plain-text file: one line per node, wires by name, scenes and
transitions, one output. Short enough to read aloud, closed enough that a model
can't get it wrong.

```
held  = osc /key/space  debounce .05       # a gate: 0 or 1
t     = ramp held  up 2s  down .8s         # climbs while held

calm  = circle .25 grid 12 12  gray .4
storm = circle .07 soft 1  hue 330 drift 60  wave .32 .4hz .55hz
      | feedback decay .25 angle 1.4 scale .8

waiting : calm
charged : calm*(1-t) + storm*smooth(t) add
burst   : storm

waiting -> charged   held rise           cut
charged -> burst     t = 1               fade .6s
charged -> waiting   t = 0 & held off    cut
burst   -> waiting   held off            fade 1.5s
```

And the loop that makes it writable by something that can't see a screen:

```
vybe check  face.vy                                   # mistakes, before any GPU work
vybe render face.vy --at 0s,2s,4.5s --osc "/hands 1 @1s" --out frames/
vybe run    face.vy --key space=/hands                # a key stands in for the sensor;
                                                      # save the file and the window follows
vybe api                                              # the whole vocabulary, for a model's context
```

Headless rendering is deterministic — fixed clock, scripted inputs, same pixels
every time — so a patch and its Rust twin are compared pixel for pixel.

**Syntax highlighting** for `.vy` in VS Code (and anything that reads TextMate
grammars): `ln -s "$PWD/editors/vscode" ~/.vscode/extensions/vybe-vy`, then
reload the window. The grammar is generated from the engine's own vocabulary
(`vybe grammar`), so it can't fall behind the language.

## The chain

Nobody loves an engine — they love the language. A sketch is short, readable,
only the visual; all the wgpu hides in the lib.

**A feedback loop, fed by its own geometry:**

```rust
use vybe::*;

circle(0.05)
    .soft(1.0)                              // a ball of light, not a disc
    .hue(Hue { base: 200.0, drift: 25.0 })  // color drifting around the wheel
    .wave(Wave::default())                  // a slow orbit
    .render()                               // the bridge: geometry → texture
    .feedback(Swirl { decay: 0.16, angle: 0.9, scale: 0.74 })
    .show();
```

**Worlds of different types, stacked top-first:**

```rust
layers![
    particles(150_000).swirl(0.6).attract(mouse(), 0.5),   // sparks, in front
    circle(0.30).grid(16, 16).soft(0.9).hue(265.0),        // a field, behind
]
.show();
```

**Knobs you turn live** — name a value, get a slider:

```rust
live(|| {
    particles(80_000)
        .swirl(tune("swirl", 0.25, 0.0..=1.0))
        .repel(mouse(), tune("repel", 0.7, 0.0..=2.0))
})
.show();
```

> **Signal is the heart. Everything that flows in or out of a Signal is an
> addon. The graph is how Signals connect. And each language is just a way to
> write the chain.**

## Run the gallery

Every sketch in `examples/` runs straight through Cargo — no tool, no config:

```
cargo run --example feedback              # a feedback loop fed by the chain itself
cargo run --example dots                  # a grid of dots that swell near the mouse
cargo run --example layers_over           # particles composited over a soft field
cargo run --example particles --release   # 80k GPU particles, stirred by the mouse
```

The rest, grouped by family: `dots{,_tune,_tune_xy}` · `feedback{,_trail}` ·
`particles{,_galaxy,_swarm,_rain}` · `layers_{over,blend,aurora}` ·
`rainbow{,_trails}` · `osc`. Any of them renders headless:
`VYBE_RENDER="at=2.5 size=500x500 out=frames" cargo run --example feedback`.

The patches live one per folder in `examples/patches/`:

```
cargo run -p vybe-cli -- run examples/patches/hello/hello.vy       # the smallest patch
cargo run -p vybe-cli -- run examples/patches/trails/trails.vy     # `|` into feedback
cargo run -p vybe-cli -- run examples/patches/stack/stack.vy       # + * add, and Scalars
cargo run -p vybe-cli -- run examples/patches/scenes/scenes.vy     # scenes; hold SPACE
```

**Two windows — a face and its remote.** The installation this engine is being
built for projects onto a cube, one Raspberry Pi per face, calibrated from a
laptop. Until the Pi arrives, both ends run side by side:

```
scripts/face-and-remote.sh                                             # a cube's square face
scripts/face-and-remote.sh examples/patches/map-screen/map-screen.vy   # a 16:9 screen
```

Same tool, any shape. A patch says how big its picture is apart from the
projector's output — `out window 1920x1200  picture 1200x1200` — and the four
corners you drag are the corners *of that picture*; the face tells the remote its
proportions, so nothing is configured twice. Close either window to stop both.

(By hand: `cargo run -p vybe-cli -- run <patch>` for the face, `cargo run -p
vybe-remote` for the remote. The script passes `vybe run` flags through:
`scripts/face-and-remote.sh examples/patches/map-show/map-show.vy --key space=/hands`.)

Drag a corner in the remote (or TAB + arrows; SHIFT = 10 px) and the face warps;
`G`/`W`/`H` switch test patterns, SPACE returns to the show, `S` saves the
keystone next to the patch, `R` re-reads it, `0` goes back to uncalibrated. They speak OSC over UDP, so the face moves to
another machine unchanged. The full piece: `examples/patches/map-show/` (run its
`make-media.sh` once — vybe renders its own media).

## How it's built

**Core/sketch separation (the Processing model).** The **core** is the lib —
"don't touch it". You create only in **sketches** (`examples/*.rs`). The boundary
is crate/module, not language — which is what lets TS and Lua drive the same core
later, without a rewrite.

**Scene space** everywhere the artist looks: center `(0, 0)`, y-up, the shorter
edge spans `-0.5..+0.5`, square on any aspect. Pixels only at the boundary.

The core is one module per layer — `sugar` (the chains) · `patch` (the `.vy`
front-end: parse, check, play) · `objects` (gates, ramps, the scene fader) ·
`recipe` (what the core draws) · `tune` (named knobs) · `input` (the one seam
integrations enter through) · `stage` (clock, keystone, headless) · `gpu` (all of
wgpu, hidden) · `shell` (window) · `tweak` (the optional egui panel).

Around it, a workspace of crates **cut by dependency, not by platform**:
`vybe-cli` (the `vybe` command) · `vybe-io` (OSC) · `vybe-remote` (the remote
protocol, both ends, and the remote itself) · `keystone` (a projector's
calibration as a file; depends on nothing).

## Where it's going

**0.0.2 — the patch runs on a laptop** (here, mostly). **0.0.3 — the patch runs
on the wall**: a KMS/DRM output with no desktop, hardware video, audio, and four
Raspberry Pi 5s around a cube. Multi-Sugar happened sooner than planned, and as a
text format rather than a language binding: the `.vy` *is* the second dialect.
TS/JS, Lua and WASM remain on the horizon — consciously deferred.

The full picture — lineage, principles, and the road — is in
[VISION](docs/VISION.md). The *why* behind each decision lives in
[DECISIONS](docs/DECISIONS.md); what's next in [ROADMAP](docs/ROADMAP.md); the
vocabulary in [GLOSSARY](docs/GLOSSARY.md); the contributor rules in
[CLAUDE.md](CLAUDE.md).
