# vybe

## Creative Coding Engine

> A Hydra with the soul of Braid, a base of Rust, and many tongues —
> but one that starts, as they all did, with a single shader on screen.

A sovereign creative-coding engine in **Rust + wgpu (WebGPU)**. One core, many
sugars — and the **chain is the product**. Geometry, feedback, layers, and a GPU
point cloud are all one `Signal`: they compose, and the chain never breaks.
Native today (macOS/Windows/Linux); WASM next.

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
});
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
`rainbow{,_trails}` · `osc`.

## How it's built

**Core/sketch separation (the Processing model).** The **core** is the lib —
"don't touch it". You create only in **sketches** (`examples/*.rs`). The boundary
is crate/module, not language — which is what lets TS and Lua drive the same core
later, without a rewrite.

**Scene space** everywhere the artist looks: center `(0, 0)`, y-up, the shorter
edge spans `-0.5..+0.5`, square on any aspect. Pixels only at the boundary.

The core is one module per layer — `sugar` (the chains) · `recipe` (the flattened
chain = AST) · `tune` (named knobs) · `gpu` (all of wgpu, hidden) · `shell`
(window/input) · `tweak` (the optional egui panel).

## Where it's going

One core, **many sugars**: Rust today; **TS/JS** and **Lua/Luau** next, over the
same core compiled to **WASM**. And many front-ends — livecoding, a node graph,
and natural language: the chain and the graph are the same AST from different
angles, so an LLM is just one more way to write it.

The full picture — lineage, principles, and the road — is in
[VISION](docs/VISION.md). The *why* behind each decision lives in
[DECISIONS](docs/DECISIONS.md); what's next in [ROADMAP](docs/ROADMAP.md); the
vocabulary in [GLOSSARY](docs/GLOSSARY.md); the contributor rules in
[CLAUDE.md](CLAUDE.md).
