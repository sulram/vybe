# VISION.md — the horizon (not today's scope)

Where vybe is going, and the beliefs under it. The runnable *now* lives in the
[README](../README.md); the day-to-day rules in [CLAUDE.md](../CLAUDE.md); the
decision-by-decision *why* in [DECISIONS.md](DECISIONS.md).

A creative-coding engine that is **web-native and native at the same time**,
sovereign, hackable to the bone, where the *sugar syntax is the product* — in
the spirit of Olivia Jack's Hydra, with the architecture of Dimitre's Braid, on
a Rust/wgpu base that compiles to both native desktop and WASM. The heart of it
is **one core, many languages (Multi-Sugar)**: the engine is written once, in
Rust, and each language — Rust, TS/JS, Lua/Luau — is a different way to write the
same chain on the same core. No language is privileged; they are all dialects.

## Principles that don't change

1. **Sovereign base.** The engine is ours, in Rust + wgpu. We depend on no one's
   engine — it's the "apparatus" in Flusser's sense: built, not operated.
2. **Truly cross-platform.** The same core runs native **and** in the browser via
   WASM. WebGPU is the bet on the future, not legacy OpenGL.
3. **The sugar syntax is the product.** As in Hydra, people love the language,
   not the engine. The chain is the star.
4. **Multi-Sugar — one core, many tongues.** Rust (base API), TS/JS (web,
   vibe-coding), Lua/Luau (native, hot-reload) coexist as costumes of the same
   chain. The reason the core exists separate from everything.
5. **One base, many front-ends.** The same core accepts livecoding (text),
   node-graph, and natural language / LLM. Chain and graph are the same AST from
   different angles — the LLM is *one* front-end among several, not the center.
6. **Shokunin.** Factory quality from the first prototype; the clean separation
   is born on day 1, never retrofitted.

## Target architecture (the layers)

```
┌─ FRONT-ENDS ──────────────────────────────────────────────┐
│  livecoding (text)  ·  node-graph (Tekne Flow)  ·  LLM     │
├─ SUGAR — dialects over the same core (Multi-Sugar) ────────┤
│  Rust (base API)  ·  TS/JS (web/WASM)  ·  Lua/Luau         │
├─ CORE ─────────────────────────────────────────────────────┤
│  Rust + wgpu — Signal, chain, pipelines, GPU               │
│  compiles to: native  AND  WASM                            │
└────────────────────────────────────────────────────────────┘
```

The **Tekne Flow** (SvelteKit + SvelteFlow + Paper.js) is not the center — in
time it becomes the "node-graph" front-end plus the "TS/JS" dialect, plugged
into this core. We're building the foundation that was missing underneath it.

## The chain is dataflow — the difference from Hydra

Hydra is pure raster (everything is a fullscreen texture). Here the chain is a
**dataflow of signals** whose payload can change type: **texture/field** (the
Hydra/Braid world — feedback, fullscreen effects), **geometry** (circles,
paths — the Paper.js world), **point cloud** (particles, data). The bridge is a
link of the chain itself: `.render()` turns geometry into texture, and from
there it flows through the Braid world. **The chain never breaks — it just
changes signal type at an explicit point.**

## Positioning (why this exists)

- **Web/WebGPU** — runs from a link, not an installer. Online generative art.
- **LLM-first / vibe-coding** — designed from the base for natural-language
  creation, not AI bolted onto a pre-LLM app.
- **Sovereign and authorial** — hackable, extensible, carrying the Tekne
  aesthetic/conceptual signature (Flusser, the untranslatables, the repertoire).

## Core Design Principles

Distilled from analyzing **Braid**. The turn: in the TS world these were
*discipline* (a convention to remember); in the **Rust** core they become *type
guarantees* the compiler enforces. These are referenced by number across the
codebase — keep them stable.

1. **Signal is the single currency.** One abstraction collapses everything.
   Before a new type: "isn't this just a Signal?"
2. **Nodes are total, not defensive — via `Default`.** No `Option<Signal>`
   input; the "absent" state doesn't exist. `Signal::default()` *is* the
   identity; describing a chain never panics.
3. **Expressive never fails; only resources return `Result`.** Algebra and
   transforms return the value directly; only IO/parsing/loading can fail — and
   the type says so.
4. **Hide the ping-pong, expose the knob.** Double-buffering, dirty propagation,
   batching → invisible. The artist sees `decay`, not two buffers.
5. **Small core + addons by dependency weight.** SVG, fonts, image, audio =
   separate, opt-in crates. The discipline is what you *refuse* to put in the
   core.
6. **Every milestone is something you see/run.** If a step doesn't end in a
   visible demo, it's too big.
7. **LLM-sized is a feature.** The core must fit comfortably in one LLM context.
   Growing past that is a signal something should become an addon.

**What NOT to copy.** Colliding names: `braid::Surface` vs `wgpu::Surface` was a
real cognitive cost — the central primitive is **not** named `Surface` (wgpu)
nor `Node` (SvelteFlow); it's `Signal`. And we skip the Paper.js per-object
churn: on the metal, GPU instancing is how you draw.

> **Signal is the heart. Everything that flows in or out of a Signal is an
> addon. The graph is how Signals connect. And each language is just a way to
> write the chain.**

## The road ahead

- **Phase 2 — external sugar + web.** Compile the core to **WASM**; the first
  non-Rust sugar (**TS/JS**, the path to adapting the Tekne Flow), then
  **Lua/Luau** via `mlua`. Each language drives the *same* core.
- **Phase 3+ — graph, Tekne Flow, and LLM.** The node editor renders the chain
  (same AST) as one front-end; a natural-language / MCP front-end generates
  sketches.

When the script layer exists, one rule holds the hot path: **script describes,
GPU executes** — the sketch assembles the recipe once, buffers stay on the GPU,
large data never crosses the boundary. A declarative API makes the bottleneck
*architecturally impossible*.
