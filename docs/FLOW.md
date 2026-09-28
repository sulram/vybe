# FLOW.md — vybe-flow: the editor that writes .vy and plays it live

The desktop front-end for vybe: a node editor where the graph **is** a
patch, the text is always visible, and every edit plays **JIT** — the same
engine, running native behind a Tauri shell. Where it comes from, what must
be built, and in what order. The beliefs underneath are in
[VISION.md](VISION.md) (principle 5: *chain and graph are the same AST from
different angles*); the engine side of the road is in
[ROADMAP.md](ROADMAP.md); the language this editor speaks is
[LANGUAGE.md](LANGUAGE.md); the device it rehearses for is
[HOST.md](HOST.md).

> The graph is not a second language. It is the patch, seen from above.

## The shape — Tauri, and the engine never leaves home

```
┌─ vybe-flow (Tauri app) ───────────────────────────┐
│  webview window: the node editor (Svelte)          │
│       ↕ Tauri commands — five of them, native IPC  │
│  the Rust core: vybe itself, compiled NATIVE       │
│       ↓                                            │
│  the stage: a native wgpu window (the shell that   │
│  already exists)                                   │
└────────────────────────────────────────────────────┘
```

The first sketch of this document put the engine in the browser, compiled to
WASM. Tauri replaces that bet with a better one: **the engine stays native**
and only the editor's face is web. Every gap the WASM path carried — no
GStreamer, no OSC, no UDP, WebGPU compute a lottery — is not solved but
*absent*: video plays with its sound, the remote talks real OSC, the
keystone pulls real corners, particles run at full speed.

And the frame brings gifts that were already on the roadmap: the macOS
`.app` with the `.vy` association (0.0.2's "player, packaged"), Windows and
Linux installers, an auto-updater. One Linux `cross` away, the same app runs
on the Pi — the editor on the device itself, if a work ever wants that.

**JIT is already built.** On the device, hot reload watches a file and swaps
the patch into the running show, state preserved by name. In the editor the
"file" is a string in memory: every graph edit regenerates the `.vy` and
sends the same swap. Live coding with the last-good-patch guarantee — a
broken edit dims nothing — comes free from work already done.

## What must be built

### 1 · The node editor (the webview shell)

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

### 3 · The commands (five functions, no per-frame IPC)

The editor and the engine meet at the same five verbs the WASM sketch
wanted — but as Tauri commands over native IPC, and *cheaper*: the engine
runs its own render loop in its own window, so nothing crosses the bridge
per frame. The editor talks only when something changes:

```rust
load_patch(src) -> Result<_, Vec<CheckError>>  // check + swap; hot-reload semantics
set_input(addr, value)                          // the Inputs address space, from JS
set_tune(name, value)                           // knobs, bound to sliders
status() -> Playing                             // what plays, since when, at what rate
// the fifth is not a command: the stage window itself
```

The **check errors cross to JS** — the editor underlines the line and says
what to write, in the patch's own words. The verification loop (check,
render, api) that 0.0.2 built for authors who can't see the screen is the
editor's error UI, already designed.

### 4 · The stage, in three grades (smallest first)

- **A separate native window** — winit, the shell that exists today; the
  `face-and-remote.sh` topology formalized: editor window, stage window, and
  on the desk the same split the device will have across the room. Zero new
  rendering code. *Ship this first.*
- **A preview pane in the webview** — the engine renders offscreen and
  frames stream into a `<canvas>`; one GPU→CPU copy per frame, fine at
  preview size. The full-quality stage stays the native window.
- **WebGPU inside the webview** — the one-window dream, and a platform
  lottery (WebView2/WKWebView support is not ours to schedule). Adopted when
  the webviews grow up, never bet on.

## The gaps, named honestly

- **GStreamer inside the app.** The roadmap's "only hard part" of the
  packaged player returns: ~100–200 MB of dylibs to bundle and re-path per
  OS. The seam is the same; the work is known; do it when there is someone
  to hand the app to who won't install it themselves.
- **Two windows are two windows.** Until the preview pane exists, the editor
  and the stage are separate — some will read that as a flaw; the device
  reads it as rehearsal.
- **The webview is not the web.** The Svelte shell must never call a
  browser-only API outside a seam, or the browser variant (below) dies
  quietly.

## One frontend, three backends

The Svelte editor never learns where `load_patch` lives. Behind the same JS
interface:

- **Desktop** — a Tauri command, the engine native (this document's plan).
- **The device** — the host's HTTP/OSC API (HOST.md): the same editor served
  by the box itself, `load_patch` becoming a request to the wall.
- **The browser** — a wasm-bindgen call, when the WASM build is worth its
  gaps (no video, no OSC, WebGPU compute to verify). The spike is deferred,
  not deleted: a show you can send as a link is still the dream, it is just
  no longer on the critical path.

One core, many sugars; one editor, many shells. The doctrine holds at every
layer.

## What vybe-flow is not

- **Not a renderer.** Paper.js drew tekne-flow's pictures; vybe-flow draws
  nothing itself — every pixel is the real engine, so what the stage shows
  is what the wall gets.
- **Not a new node vocabulary.** The words are the patch's words. An editor
  that invents nodes the language doesn't have is a fork, not a front-end.
- **Not tekne-flow continued.** The columnar CPU streams were that
  prototype's bet; this one rides the GPU recipe. The patterns cross over
  (GraphAPI, layout, dynamic outputs); the data model does not.

## The order

1. **The shell spike: Tauri + one hardcoded patch on a native stage.** A
   window, a command, a running show. The risk here is small and mostly
   packaging; a day answers it.
2. **`load_patch` + a textarea.** A bare live-coding pane: edit the `.vy`,
   watch the stage, errors inline. Already a useful tool; already the JIT
   promise kept.
3. **The graph on top.** GraphAPI + the generated palette, serializing into
   the same text. The textarea never goes away — it becomes the patch view.
4. **Polish.** Sliders from `tune`, the preview pane, patch → graph
   round-trip, the `.vy` file association, GStreamer in the bundle.

Steps 1–2 are days, because the engine was designed for them. Step 3 is
where tekne-flow's scar tissue pays off. Step 4 is when the editor becomes
the thing you hand to someone.

## Open questions

- Fork tekne-flow's UI shell, or lift the patterns into a fresh, lean app?
  (The graph semantics differ enough that a fresh shell may cost less than
  un-teaching the old one its streams.)
- The preview pane's copy path: readback per frame, or a shared texture
  where the platform allows? Measured, not guessed, when the pane is pulled.
- One project file or two: does the editor save the graph's layout beside
  the `.vy` (a `.flow` sidecar), or does the layout live in patch comments
  so one file stays the whole truth?
