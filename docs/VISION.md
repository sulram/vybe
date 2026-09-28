# VISION.md — the horizon (not today's scope)

Where vybe is going, and the beliefs under it. The runnable *now* lives in the
[README](../README.md); the day-to-day rules in [CLAUDE.md](../CLAUDE.md); the
decision-by-decision *why* in [DECISIONS.md](DECISIONS.md); the order and the
katas that pace everything below in [ROADMAP.md](ROADMAP.md).

A creative-coding engine that is **native and web-native at the same time**,
sovereign, hackable to the bone, where the *language is the product* — in the
spirit of Olivia Jack's Hydra, with the architecture of Dimitre's Braid, on a
Rust/wgpu base. The heart of it is **one core, many front-ends**: the engine
is written once, in Rust, and each way of writing a work — the Rust sugar, the
`.vy` patch, a node graph, natural language — is a different view of the same
chain on the same core. No front-end is privileged; they are all dialects.

Since 0.0.2 the horizon has three named territories. **All of it is
kata-gated** (Golden Rule #2): nothing here is a spec — a plan ahead of its
kata is horizon, not scope, and the katas pace it.

- **The device** — vybe as an appliance: a box you power on, the show plays.
- **The language** — `.vy` growing from player to instrument, keeping its
  promises.
- **The editor** — vybe-flow: the graph as a view of the patch, playing live.

## Principles that don't change

1. **Sovereign base.** The engine is ours, in Rust + wgpu. We depend on no one's
   engine — it's the "apparatus" in Flusser's sense: built, not operated.
2. **Truly cross-platform.** The same core runs native **and** in the browser via
   WASM. WebGPU is the bet on the future, not legacy OpenGL.
3. **The language is the product.** As in Hydra, people love the language, not
   the engine. Since 0.0.2 that language is spelled two ways over one core:
   **the patch is the product; the chain is how it is born** (DECISIONS
   2026-09-20).
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
┌─ FRONT-ENDS ───────────────────────────────────────────────┐
│  Rust sugar (chains) · .vy (the patch) · vybe-flow (graph) │
│  the remote (OSC)    · LLM / natural language              │
├─ CORE ─────────────────────────────────────────────────────┤
│  Rust + wgpu — Signal, Recipe, Player, GPU                 │
│  runs: window · headless · KMS (0.0.3) · WASM (later)      │
├─ THE DEVICE ───────────────────────────────────────────────┤
│  vybe-host — supervisor: registry, power-on default, API   │
└────────────────────────────────────────────────────────────┘
```

Tekne Flow (SvelteKit + SvelteFlow + Paper.js) is not the center — its proven
patterns feed the vybe-flow editor below, and its language ideas the roadmap;
we built the foundation that was missing underneath it.

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
- **The open device** — against the MadMapper-style black box: a projector
  appliance that is scriptable, cheap, and whose show is text you can read.

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

## The device — vybe as an appliance

A **projector device** in the MadMapper spirit: a small box (a Pi), power it
on, the show plays; reach it over the network to map, change the media, load
something new. Most of the hard parts shipped in 0.0.2 before the device was
named — the keystone *is* the present pass, the remote talks OSC, video plays
with its sound, hot reload never blacks out a wall, the player is 7.4 MB, and
headless render/check need no screen. What remains is the **host**.

> A face on a wall has no keyboard. The device is the face; the host is what
> makes a computer behave like a face.

- **The host is a supervisor, not a new engine.** It boots first, owns nothing
  visual, and answers three questions: *what plays now, what plays at
  power-on, what else is on this box.* It supervises the player and apps as
  child processes; a crash restarts or falls back to the default — never to
  black.
- **A project is a folder.** The patch or app, its media, its `keystone.json` —
  everything a work needs, nothing it doesn't. Upload = copy a folder; backup
  = tar; move a show = `scp`. The host's registry is the folders it can see.
- **The default project is a pointer, not a copy.** Power-on plays whatever
  the pointer names — the venue's show, a test pattern while installing, a
  *sos* patch when nothing else exists.
- **Three tiers of work** — "compile on demand" means three things, and the
  tier is chosen by what the work needs:
  - *Tier 1 · the patch (`.vy`)* — instant, on the device: parse → check →
    play, milliseconds, safe by construction. Most of what the device will
    ever do is a patch.
  - *Tier 2 · the scripted app* — seconds, sandboxed, on the device (Lua or
    WASM guests; decided when the first interactive work pulls it). Where
    sensor fusion, game logic, works that listen and answer live; a script
    can crash, the sandbox cannot take the host down.
  - *Tier 3 · Rust* — full power, compiled **off** the device: `rust <name>`
    leaves compiled into the player, or a standalone binary the host
    supervises like any child. **The device never runs rustc** — process
    isolation instead of ABI risk (DECISIONS 2026-09-29).
  - The patch is the format that needs no compiler; the script, no toolchain;
    Rust, no limits.
- **The API, smallest first:** status → media upload/download (the checker
  validates before anything goes live) → playlist (a playlist *is* a patch) →
  switch / startup → calibration (the keystone remote becomes one more
  client of the same box).
- **Appliance rules:** boot-to-show (no login, no desktop in the path —
  `vybe-stage-drm`); read-only rootfs (projects on a data partition);
  watchdog (a wall that goes black is a bug; a wall that *stays* black is a
  failure); atomic updates (the hot-reload rule at system scale: the last
  good show plays on); mDNS (`_vybe._tcp` — never type an IP in a venue).
- **What the host is not:** a desktop or browser kiosk (the UI lives anywhere
  else — the remote, a page, a phone); feature parity with MadMapper (mesh
  warp, DMX, Syphon wait for the work that pulls them — the wedge is open,
  scriptable, cheap, and the show is text); a cloud service (the box works in
  a room with no internet, forever).

## The language — growing .vy

The patch is Tier 1 of the device: the more a `.vy` can say, the fewer works
escape to a script or to Rust. It grows by learning to say more things that
**exist** — never by learning to do more steps.

**The contract (what must not break):**

1. **Declarative.** A graph of things that exist and flow; no procedures, no
   hidden order of execution. A line whose meaning depends on the line above
   is Lua with worse syntax.
2. **Checkable.** `vybe check` proves a patch without a GPU; the checker is
   the language's immune system, and a broken save never blacks out a wall.
3. **Hot-swappable.** State survives re-description by name (key = identity);
   a new construct says what it preserves across a reload.
4. **LLM-legible.** Orthogonal verbs, one vocabulary table, errors that say
   what to write — a feature to defend, not a constraint to escape.

**Five directions of growth** (each item's kata and order live in ROADMAP):

- **Logic & state** — expressions in conditions (`when /sensor/near > 0.6`),
  timers (`idle`, `after` — installation art's most-said sentence in one
  line), counters and latches (`count`, `toggle`, `hold` — the Gate/Ramp/
  Fader family continued). Edges, not levels, everywhere it matters. What
  stays out: loops, variables, functions — the day a patch needs a `for`,
  the work has told us it is a script.
- **The senses** — input is an address space; growing the senses adds
  *sources*, never concepts. GPIO/I2C (a sensor is an address —
  `/sensor/near`; the device's whole point), MIDI (`/midi/cc/21`), OSC out
  (a patch that `send`s conducts other gear), time/dt/frame as named
  sources.
- **Media & text** — the player basics: playlists over folders, `text` +
  `Draw`/`canvas()`, `image` as a peer of `video` and `frames`, scrubbing
  video with `@` when a work pulls frame-exact control.
- **Reuse** — what keeps the fiftieth patch as short as the first:
  subpatches (`include`, state scoped by the include's name), `Param` (the
  tune that persists — a room's settings beside `keystone.json`, not in the
  work), templates (`vybe new --from media-player` — the example is the
  documentation), `vybe fmt` (reuse lives in version control).
- **The escape hatches** — the deepest power of the language is what it
  refuses to say. `rust <name>` leaves (the algorithmic node compiled into
  the player); the script tier — which is **not a rival vocabulary**: the
  words are data, so every word the patch gains is a word the script speaks.

**The boundary, stated once:** if it is a graph of things that exist and
flow, it is a patch; if it is a procedure that decides, it is a script.

**What .vy must never become:** Turing-complete (no unbounded loops, no
recursion, no mutable variables — "this wall never goes black" is provable
only because the language cannot surprise the checker); a second Rust; a
grab-bag of synonyms and one-off words; faster than its katas — a word ahead
of its kata is a guess.

**How a word enters:** a kata pulls it → proposed against the contract → one
row in the vocabulary table (parser, checker, `vybe api`, generated grammar
all follow; the stale-grammar test enforces it) → the kata ships in both
dialects, byte-identical → DECISIONS.md logs the why.

## The editor — vybe-flow

A node editor where the graph **is** the patch, the text is always visible,
and every edit plays JIT — the same engine, running native behind a Tauri
shell.

> The graph is not a second language. It is the patch, seen from above.

- **Tauri, and the engine never leaves home.** The webview holds the editor
  (Svelte); the engine is the native core behind a handful of commands
  (`load_patch`, `set_input`, `set_tune`, `status`); the stage is the native
  wgpu window that already exists — nothing crosses the bridge per frame.
  Every gap the WASM path carried (no GStreamer, no OSC/UDP, WebGPU compute
  a lottery) is not solved but *absent* — and the frame brings the packaging
  the roadmap wanted: the `.app` with the `.vy` association, installers, an
  updater. (DECISIONS 2026-09-29.)
- **JIT is hot reload with a string for a file.** Every graph edit
  regenerates the `.vy` and sends the same swap, state preserved by name;
  the last-good-patch guarantee — a broken edit dims nothing — comes free
  from work already done. Check errors cross to JS: the editor underlines
  the line and says what to write, in the patch's own words.
- **The palette is generated from the vocabulary table** — the VS Code
  grammar's trick, so the editor can never drift from the language. Two
  socket types (chains and scalars); feedback is a node, never a back-edge.
  Graph → patch ships first (the serializer is a printer); patch → graph is
  v2.
- **The text view is the truth** — it is what the device runs. The graph's
  layout lives in a sidecar beside the `.vy`, never inside the patch: a
  project is a folder, and the patch stays the whole truth of the work.
- **One frontend, three backends.** The Svelte editor never learns where
  `load_patch` lives: a Tauri command (desktop, native engine); the host's
  API (the same editor served by the box itself — `load_patch` becomes a
  request to the wall); a wasm-bindgen call (the browser, when webview GPUs
  grow up — a show you send as a link is deferred, not deleted).
- **What vybe-flow is not:** a renderer (it draws nothing itself — every
  pixel is the real engine, so the stage shows what the wall gets); a new
  node vocabulary (an editor that invents nodes the language doesn't have is
  a fork, not a front-end); tekne-flow continued (its patterns cross over —
  GraphAPI, layout, dynamic outputs — its columnar data model does not).

## The road ahead

The order and the katas live in [ROADMAP.md](ROADMAP.md). The shape of it:

- **0.0.2 finishes first.** The "Pick up here" list outranks every territory
  above; each plan still waits for the work that pulls it.
- **The device rehearses on the desk.** The face/remote split already runs as
  two windows on one Mac; 0.0.3 puts it on the wall; the host grows around
  the player that exists.
- **The language grows by most-said sentences.** Timers + expressions in
  conditions lead — the cube's sensor-driven scenes pull them anyway.
- **The editor starts as a textarea.** The shell spike and `load_patch` are
  days because the engine was designed for them; the graph rides tekne-flow's
  scar tissue; WASM is the third backend, never the critical path.
- **Multi-Sugar waits for the tiers.** The TS/JS and Lua dialects enter when
  the script tier or the browser backend pulls them — dialects of the same
  words, never rival vocabularies.

When the script tier exists, one rule holds the hot path: **script describes,
GPU executes** — the sketch assembles the recipe once, buffers stay on the GPU,
large data never crosses the boundary. A declarative API makes the bottleneck
*architecturally impossible*.
