# FLOW.md — vybe-flow: the editor that writes .vy and plays it live

The browser front-end for vybe: a node editor where the graph **is** a patch,
the text is always visible, and every edit plays **JIT** — the same engine,
compiled to WASM, running in a canvas. Where it comes from, what must be
built, and in what order. The beliefs underneath are in
[VISION.md](VISION.md) (principle 5: *chain and graph are the same AST from
different angles*); the engine side of the road is in
[ROADMAP.md](ROADMAP.md); the language this editor speaks is
[LANGUAGE.md](LANGUAGE.md).

> The graph is not a second language. It is the patch, seen from above.

## The shape

```
┌─ vybe-flow (browser) ─────────────────────────────┐
│  node editor (the graph)                           │
│       ↕ serialize                                  │
│  .vy patch text — always visible, always editable  │
│       ↕ load_patch()                               │
│  vybe-wasm — the real engine, compiled to WASM     │
│       ↓                                            │
│  <canvas> — wgpu on WebGPU                         │
└────────────────────────────────────────────────────┘
```

**JIT is already built.** On the device, hot reload watches a file and swaps
the patch into the running show, state preserved by name. In the browser the
"file" is a string in memory: every graph edit regenerates the `.vy` and
calls the same swap. Live coding with the last-good-patch guarantee — a
broken edit dims nothing — comes free from work already done.

## What must be built

### 1 · The node editor (the UI shell)

Born from tekne-flow's patterns, not from zero — that prototype already
solved the hard UI problems: **GraphAPI** (one source of truth for every
mutation, event-driven, scriptable by humans, tests, and LLMs alike),
auto-layout, dynamic outputs, param panels, bypass. What changes is the
palette:

- **The palette is generated from the vocabulary table** (`vybe api`) — the
  same trick as the VS Code grammar, so the editor can never drift from the
  language. A word enters the table; the editor learns it the same day.
- **Two socket types, already named**: chains (`structural`) and scalars
  (`value`) — tekne-flow's `STREAM`/`VALUE` distinction carried onto the
  patch's two kinds of plugs.
- **Feedback is a node, not a back-edge.** vybe's `| feedback` is a
  first-class construct; the editor draws it as a node in the chain, never
  as a wire that loops.

### 2 · Graph ↔ patch (the heart)

- **Graph → patch** is the required direction, and it is easy: each node is
  one line, wires are names, scenes and transitions are named regions. The
  serializer is a printer, and `Recipe::to_vy()` (roadmap) is its engine-side
  twin.
- **Patch → graph** (paste a patch, see it as nodes) is v2: the hand-written
  parser and clean AST make it possible, but nothing in the JIT loop needs
  it. Ship one-way first.

The text stays visible and editable either way — two views of one AST, and
the text view is the truth (it is what the device will run).

### 3 · vybe-wasm (the real work, and where the risk lives)

Compile the core to `wasm32-unknown-unknown`. The codebase was built for
this: the `Clock` seam exists *because* `Instant::now()` panics on WASM;
wgpu's web backend targets WebGPU natively; the parser, checker, and Player
are std-only. What remains is one small crate with five functions:

```rust
load_patch(src) -> Result<_, Vec<CheckError>>  // check + swap; hot-reload semantics
set_input(addr, value)                          // the Inputs address space, from JS
set_tune(name, value)                           // knobs, bound to sliders
frame(dt)                                       // driven by requestAnimationFrame
// the canvas is wgpu's web surface
```

The **check errors cross to JS** — the editor underlines the line and says
what to write, in the patch's own words. The verification loop (check,
render, api) that 0.0.2 built for authors who can't see the screen is the
editor's error UI, already designed.

### 4 · The shell's browser twin

The `shell` layer (window/input) gets a browser sibling: canvas resize, and
mouse/touch/keyboard written into the **same address space** OSC lands in —
`/mouse`, `/key/space`. Consequence worth the whole project: `scenes.vy`
runs unchanged in the browser, SPACE and all. A patch authored in the editor
is a patch the device can play; the preview is not a simulation of the
engine, it is the engine.

## The gaps, named honestly

- **Video.** GStreamer does not exist in the browser — but `Clip`/`Decoder`
  is a seam, and seams are where backends swap: a web decoder
  (HTMLVideoElement / WebCodecs → external texture) behind the same
  interface. Real work, isolated work. `frames` and images need no such
  bridge.
- **Feedback and particles on WebGPU.** Compute and storage buffers exist on
  the web backend; "should work" is not "works". This is the spike's first
  question.
- **OSC.** No UDP in browsers. A WebSocket bridge can carry the address
  space later; the JIT preview needs none of it.
- **Keystone, remote, `out kms`.** Device words. The editor shows them as
  such (a "device" region of the patch, greyed in preview) rather than
  pretending a canvas has corners to pull.

## What vybe-flow is not

- **Not a renderer.** Paper.js drew tekne-flow's pictures; vybe-flow draws
  nothing itself — every pixel is the WASM core, so what the browser shows
  is what the wall gets.
- **Not a new node vocabulary.** The words are the patch's words. An editor
  that invents nodes the language doesn't have is a fork, not a front-end.
- **Not tekne-flow continued.** The columnar CPU streams were that
  prototype's bet; this one rides the GPU recipe. The patterns cross over
  (GraphAPI, layout, dynamic outputs); the data model does not.

## The order

1. **The spike: vybe-wasm renders one hardcoded patch to a canvas.** All the
   risk lives here — wgpu-on-web, feedback, particles. A day that answers
   yes/no is worth a week of editor built on a maybe.
2. **`load_patch` + a textarea.** A bare live-coding page: edit the `.vy`,
   watch it play, errors inline. Already a useful tool; already the JIT
   promise kept.
3. **The graph on top.** GraphAPI + the generated palette, serializing into
   the same text. The textarea never goes away — it becomes the patch view.
4. **Polish.** Sliders from `tune`, media by URL, patch → graph round-trip,
   share-by-URL (the patch is text; a link can carry a show).

Steps 1–2 are days, because the engine was designed for them. Step 3 is
where tekne-flow's scar tissue pays off. Step 4 is when a show becomes a
link you can send.

## Open questions

- Fork tekne-flow's UI shell, or lift the patterns into a fresh, lean app?
  (The graph semantics differ enough that a fresh shell may cost less than
  un-teaching the old one its streams.)
- One canvas or many: does the editor preview scenes as thumbnails beside
  the stage?
- Where does vybe-flow live when it grows up — a page on the device itself
  (HOST.md's web panel), a static site, both? The patch being text makes
  "both" cheap: same editor, same engine, different save button.
