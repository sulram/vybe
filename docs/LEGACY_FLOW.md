# LEGACY_FLOW.md — the columnar / node-flow reflection (parked)

- A place to return to, not a plan. On 2026-07-06 we explored bringing the
  **Tekne Flow** node-based idea (and its columnar data model) into vybe, then
  chose to *defer* it and keep the chain sugar. This preserves the *why* so it
  can be resumed without re-deriving it.
- Live crumbs: DECISIONS ("Direction: vibe-coding-first…") and a ROADMAP block
  ("Imported from tekne-flow"). This is the long-form behind them.

## What we looked at

- `tekne/tekne-flow-2` — a **web-native, node-based** creative-coding tool
  (Svelte 5 + SvelteFlow + Paper.js + Tweakpane), pitched as a browser-based,
  node-driven motion/generative tool. ~31 nodes, 9 preset graphs, an FFI-ready
  `GraphAPI`.

## Two findings that reframed it

- **vybe and Tekne Flow are two halves of one vision.** Tekne Flow's own docs
  name its target engine as *"Rust → WASM, Lyon + wgpu → Vello"*, `GraphAPI` as
  the FFI boundary. **vybe is that intended core**; the node editor was always
  meant to be one front-end on top of it.
- **Tekne Flow's "Stream" == vybe's "Signal"**, designed independently by the
  same person. Stream = `{ count, context: ColumnStore, geometry, style }`,
  where ColumnStore is **SoA**: per-element attribute columns
  (`x, y, index, t, scale, noise…`). Same single currency.

## Reality check (docs ≠ code)

- Tekne Flow's 22 design docs are **aspirational** — many describe nodes that
  don't exist (`SamplePath`, `Text`, `SVG`, an SDF pipeline). Shipped code draws
  only filled circles and axis-aligned rectangles (no Béziers, paths, text, SVG).
- The **vector/path world is unbuilt in BOTH** — a shared frontier, not
  something Tekne Flow has that vybe lacks.
- Visual caliber roughly equal; vybe is arguably **ahead** (feedback, a million
  GPU particles — Tekne Flow has no compute).
- Its own honest verdict: low on a maturity scale (~12–15 of 100); recommended
  niche (generative typography + web-native, share-by-link) fits vybe.

## Direction chosen: vibe-coding-first

- Cooled on the **node editor** as the interface; bet on **vibe coding**
  (natural language → the chain, via an LLM) as the differentiator. The core
  stays; it became vybe. The editor is *deferred, not killed*.
- Precedent: the Quartz Composer lineage (Vade, Reza Ali) resolved to **code
  frameworks** (Satin, Fabric), not node editors, for expressive work. Vibe
  coding is the natural-language layer *over* a framework.
- **Principle set:** vibe coding raises the value of the **language** (rich,
  orthogonal, LLM-legible) and lowers the value of the visual **editor**. Import
  the language, drop the editor.

## The columnar question

- Fear: columns turn the chain into node soup (`Grid → Distance → Map →
  SetScale`). That verbosity is a property of a *node editor*, not of columns —
  a language can have columns underneath and stay neat on top.
- **vybe already has two composition mechanisms; columns would be the third:**
  1. **Host-language composition** (Rust closures/loops) — variation known at
     author time (`t = i/10` reused in `hue` and `wave`; this is `rainbow.rs`).
  2. **Baked GPU modulators** (`Falloff`, `Wave`, `hue-drift`) — a *fixed* set
     of per-frame per-element effects, in the shader.
  3. **Columnar GPU value-flow (not built)** — *arbitrary* per-element
     modulation the user composes (color by `t`, scale by mouse distance,
     rotation by noise). The only thing 1 and 2 can't do; distance from a
     million particles to the live mouse *must* be on the GPU.
- **Cheap slice (worth it, when pulled):** a few built-in columns (`t` 0→1,
  `index`; in the cloud `age`, `vel`) the sugar can *read*, plus late-binding a
  few properties (`hue(ramp(0..360))` over `t`, `hue(by("age"))`). Additive —
  `grow`/`wave`/`hue` stay. Cheapest home: the **point cloud**, where columns
  already half-exist (`pos`/`vel`).
- **General value-flow (defer hard, maybe never):** arbitrary value composition
  is, in practice, a **WGSL expression compiler** in the core — the biggest
  complexity bomb on a small clean core, and the thing that *would* uglify the
  sugar (`Falloff { min, max, scale }` is cute *because* it's baked).
- **Why vibe coding weakens the general case:** a node editor needed value-flow
  because a designer can't write WGSL. If the **LLM** writes the code, it authors
  a bespoke modulator on demand — composition happens in code generation, not a
  runtime value-graph. Note: Tekne Flow itself never built the general value-flow
  either (its nodes are hardcoded TS).

## Verdict — what "return later" means

- **Keep the neat sugar.** Take only the **cheap slice** of columns, and only
  when a kata pulls it ("color particles by age", "a gradient over `t`").
- **Refuse the general value-flow.** Hit a wall → *"LLM, write a modulator,"*
  not *"build a value-graph engine."*
- **Reweight for vibe-coding:** the TS/JS sugar + WASM rises (LLMs are fluent in
  TS/JS; vibe-coding a web tool wants the browser). Node-graph front-end drops.
- **Import (the language):** columnar context (built-in columns), normalized
  `t`, late-binding style, the value vocabulary (`distance`, `map`, `noise`,
  `sin/cos`, `modulo`, `instance`), generator/transform separation.
- **Drop (the editor):** SvelteFlow, GraphAPI, ELK layout, dynamic ports,
  subgraph copy/paste, the node UI. "Chain = graph" survives only as a possible
  read-only *visualization*; authoring stays text/voice.
- **Return conditions:** a real piece the baked structs genuinely can't express
  pulls the cheap columnar slice; or TS/WASM becomes the priority and
  vibe-coding-in-the-browser is the target.
