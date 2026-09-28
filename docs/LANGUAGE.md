# LANGUAGE.md — growing .vy: how the patch becomes a powerful language

The patch is Tier 1 of the device ([HOST.md](HOST.md)): the more a `.vy` can
say, the fewer works need to escape to a script or to Rust. This document is
the growth plan — what power the language needs, in what order, and the rules
that keep it a language people love instead of a language that accreted.
The beliefs underneath are in [VISION.md](VISION.md); each word that actually
enters is logged in [DECISIONS.md](DECISIONS.md) and lands in
[ROADMAP.md](ROADMAP.md).

> A patch does not run; it *is*. The language grows by learning to say more
> things that exist — never by learning to do more steps.

## The contract (what must not break)

Every addition below is judged against four promises the patch already keeps:

1. **Declarative.** A patch describes a graph of things that exist and flow.
   No procedures, no hidden order of execution. The moment a line's meaning
   depends on the line above it, we have written Lua with worse syntax.
2. **Checkable.** `vybe check` proves a patch without a GPU, and a broken
   save never blacks out a wall. New words must be checkable with the same
   totality — the checker is the language's immune system.
3. **Hot-swappable.** State survives re-description by name (key = identity).
   A new construct must say what it preserves across a reload.
4. **LLM-legible.** Orthogonal verbs, one vocabulary table, errors that say
   what to write. The patch is the dialect an LLM writes best — that is a
   feature to defend, not a constraint to escape.

And the standing law (Golden Rule #2): **a word enters when a kata pulls it.**
Nothing below is a spec; each item names the work that would pull it.

## Direction 1 · Logic & state — from player to instrument

Today: scenes, transitions on conditions, `Gate`, `Ramp`, `Fader`. The gap
between that and an *installation* is small and precise:

- **Expressions in conditions.** Scalars already compose in `( expr )`;
  transitions should read them: `when /sensor/near > 0.6 and dwell > 5`. The
  grammar grows where the values already live. *Kata: a presence piece that
  wakes when approached and settles when left alone.*
- **Timers as first-class objects.** "After 30 s idle, go to the attract
  scene" is the most-said sentence in installation art; it should be one
  line. An `idle` source and a `after` condition cover most of it.
  *Kata: the attract loop — every signage work's skeleton.*
- **Counters and latches.** `count`, `toggle`, `hold` — three more std-only
  objects in the family Gate/Ramp/Fader started. With them: "every third
  touch", "stay until reset", "remember that it happened". *Kata: a wall
  that fills one cell per visitor.*
- **Edges, not levels, everywhere it matters.** The decided semantics
  (a transition fires on *becoming* true) must hold for every new condition
  source — editing a condition never fires it.

**What stays out:** loops, variables, functions. The day a patch needs a
`for`, the work has told us it is a script (Direction 5).

## Direction 2 · The senses — input is an address space

The `Inputs`/`Binding` seam already unified OSC, keys, and mouse: one address
space, one way in. Growing the senses is adding *sources*, never concepts:

- **GPIO / I2C on the Pi** (roadmap: `vybe-io` GPIO). A distance sensor
  becomes `/sensor/near`, a button becomes `/button/1` — indistinguishable
  from an OSC address, bindable by the same words. *This is the device's
  whole point:* a sensor is an address. *Kata: the presence piece, again —
  it pulls both this and the conditions above.*
- **MIDI.** Every controller in a booth speaks it; one more address family
  (`/midi/cc/21`). *Kata: the remote's knobs grew up.*
- **OSC out.** A patch that can `send /light/dim 0.8` conducts other gear —
  a DMX bridge, a sound box, the other three faces of the cube. Sending is a
  node like any other: it exists, it has an input, it fires on edges.
  *Kata: two faces answering each other across the room.*
- **Time and frame as sources.** `time`, `dt`, `frame` addressable like any
  scalar — the vocabulary already assumes them; naming them makes them
  composable.

## Direction 3 · Media & text — the player basics

A device whose Tier 1 is a media player must say these plainly:

- **Playlists and folders.** `video` and `frames` over a folder, with order,
  duration, and per-item `vol`. A playlist is a patch (HOST.md); the words
  should make a *good* playlist a short patch. *Kata: the signage loop —
  three clips and a holding card, forever.*
- **`text` + `Draw`/`canvas()`** — already the biggest 0.0.2 leftover, and
  bigger on the device: status lines, labels on the calibration grid,
  captions, clocks, the loading ring. The open decision stands (embedded
  free mono vs. a 5×7 bitmap font) and the artist owns it.
- **Scrubbing with `@` on video** — seek on a paused pipeline, when a work
  pulls frame-exact control.
- **Images** (`image` as a peer of `video` and `frames`) — the gap nobody
  notices until the signage loop's holding card is a PNG.

## Direction 4 · Reuse — patches at scale

The first box holds five works; the tenth holds fifty. Reuse is what keeps
the fiftieth patch as short as the first:

- **Subpatches (`include`).** A named sub-graph used as a node: the venue's
  attract loop, the house color field, the standard fade. One definition,
  many patches; state identity still by name, now scoped by the include's
  name. *Kata: two works sharing one attract loop.*
- **`Param` — the tune that persists** (roadmap). A knob whose value survives
  reboot, addressable as `/param/<name>`. Params are an application's
  *settings* — brightness per venue, sensor threshold per room — and they
  belong in the room's file beside `keystone.json`, not in the work.
- **Templates.** `vybe new --from media-player` prints the canonical player
  patch. Templates are how the vocabulary teaches itself: the example is the
  documentation.
- **`vybe fmt` and clean diffs** (roadmap) — reuse lives in version control;
  canonical spacing is what makes a patch reviewable.

## Direction 5 · The escape hatches — power by delegation

The deepest power of the language is what it refuses to say. Two doors out,
both principled, both already in the architecture:

- **`rust <name>` leaves** — the algorithmic leaf compiled into the player
  (a physics sim, a protocol, a tracker). The patch *uses* the leaf as one
  node; the language stays small; the work gets Rust's full speed.
  TouchDesigner's custom operator, with the patch as the `.toe`.
- **The script tier (Lua/WASM)** — for works that are procedures: sensor
  fusion, game logic, conversation between machines. Crucially, **the script
  is not a rival vocabulary**: the words are data, so the Lua binding builds
  the same recipes from the same table. Every word the patch gains is a word
  the script can speak. Growing `.vy` grows everything.

The boundary, stated once: **if it is a graph of things that exist and flow,
it is a patch; if it is a procedure that decides, it is a script.**

## What .vy must never become

- **Not Turing-complete on purpose.** No unbounded loops, no recursion, no
  mutable variables. The checker can only promise "this wall never goes
  black" because the language cannot surprise it.
- **Not a second Rust.** Types stay implicit and total; `Default` keeps the
  absent state nonexistent (Core Principle 2).
- **Not a grab-bag.** Synonyms, one-off words, and work-specific hacks are
  how vocabularies rot. One verb per idea; the idea earns the verb.
- **Not faster than the katas.** A word ahead of its kata is a guess; the
  gallery is the proof the language sings.

## How a word enters (the protocol)

1. **A kata pulls it** — a concrete work that cannot be said without it.
2. **The word is proposed against the contract** — declarative? checkable?
   hot-swappable? legible?
3. **It lands in the vocabulary table** — parser, checker, `vybe api`, and
   the generated grammar all learn it from that one place; the stale-grammar
   test enforces it.
4. **The kata ships** in both dialects where it applies (`.rs`/`.vy` pair,
   byte-identical).
5. **DECISIONS.md logs the why** — including the calls the brief left open.

## The order for art installations

Pulled by the works the device is for, smallest and most-pulling first:

1. **Timers + expressions in conditions** — the attract loop, the presence
   piece. (Most-said sentences first.)
2. **GPIO/sensors as addresses** — the piece that answers the room.
3. **`text` + `Draw`** — the status line, the label, the caption.
4. **OSC out** — the piece that conducts.
5. **Subpatches + `Param`** — the tenth work on the box.
6. **Playlists and folders** — the signage works, whenever they arrive.

Each is one kata away. The gallery decides the rest.
