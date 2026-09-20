# GLOSSARY.md — the vocabulary of `vybe`

Naming is design (README: "not an afterthought"). This is the shared
vocabulary across code, docs, and conversation — one place, one meaning.
The *why* lives in [README.md](../README.md); settled decisions in
[DECISIONS.md](DECISIONS.md); day-to-day rules in [CLAUDE.md](../CLAUDE.md).

## The patch (the work, as text)

| Term | Meaning |
|------|---------|
| **patch** | A `.vy` file; the work. One line per node, wires by name, scenes and transitions, one output. The second front-end over the core, beside the Rust sugar — and since 0.0.2, the product (DECISIONS 2026-09-20). |
| **node** | One line: `name = source args mods…`, or a composition of other nodes. A **Signal** node draws; a **Scalar** node is a number. A node can't be named after a word of the language. |
| **wire** | A node's name used by another node (`ss*(1-t)`), or `\|` into an effect. A wire is *the* node, not a copy: **key = identity**. |
| **key = identity** | A named node renders **once** per frame however many scenes wire to it, and its GPU state (a feedback trail) survives re-description for as long as its name keeps appearing. Why a cut between two scenes sharing a node is seamless. *Internal:* `Recipe::Named`. |
| **scene** | `name : node` — a picture that can be shown. The first one shows first. Any node that draws may be a transition's target without being declared. No scenes at all: the last node that draws is the picture. |
| **transition** | `a -> b  cond  cut\|fade Ns` (`*` = from any scene). **Fires on the frame its whole condition *becomes* true while its `from` scene shows** — never again until it has been false in between. |
| **condition** | `node rise` (an *edge*: true for one frame), `node off` / `node done` / `node = N` (*levels*: true while they hold), joined by `&`. The transition takes the edge of the conjunction. |
| **Scalar** | A value over time (TouchDesigner's CHOP; a **Signal** is its TOP). Sources: `osc /address` (a live input; with `debounce` a gate), `osc <hz> <amp>` (an oscillator), `ramp <gate> up s down s`, `( expr )`. Plugs into any number socket. |
| **video** | `video <file>` — a playing file, *with its sound*. Plays once (then `done`) or `loop`s; restarts when its scene is entered; paused and silent while no visible scene reaches it; **as loud as its scene is visible** (`vol` × the fade's weight), so a crossfade is one of sound too. One texture on the GPU however long the file. Not scrubbable (`@`) — that is what `frames` is for. |
| **Clip / Decoder** | The seam moving pictures enter through (`src/clip.rs`). A **Clip** *plays* — restart, pause, volume, `done`, "a new frame, if any" — and keeps its own audio/video sync; a **Decoder** opens one. The core knows the two traits and nothing else; `vybe-video` implements them with GStreamer. Two **paces**: *live* (own clock, sound, newest frame) and *offline* (headless: silent, the exact frame for the asked time). |
| **composition** | `a + b` stacks (right on top), `a * x` is opacity, `a @ x` positions a sequence in time (0..1), `a add` sums light instead of covering. Plain geometry stacked `over` still folds into one pass. |
| **effect** | A GPU pass a node is wired into with `\|`: `\| feedback decay .3 angle .2 scale .9`. |
| **leaf** | `name = rust <fn>` — the escape hatch: anything that needs a `for` or a mutable list, written in Rust, returning a texture. Leaves never contain chains; chains never contain closures. (Parsed today; draws with `Draw`.) |
| **vocabulary** | The closed word list of a patch, as *data* (`patch/vocabulary.rs`): parser, checker hints, `vybe api` and the editors' highlighting grammar (`vybe grammar`) all read the same tables. Grows only with drawing primitives and Scalar operators — never application nouns. |
| **Patch / Player** | *Internal-ish.* `Patch`: the parsed AST, plain data. `Player`: performs it — each frame moves its **objects** and emits a **Recipe** with this frame's numbers filled in. Holds all the behaviour, none of the pixels. |
| **objects** | The small stateful things behaviour is made of, std-only, advanced by an explicit `dt`: **Gate** (a debounced boolean: levels `on`/`off`, edges `rose`/`fell`), **Ramp** (0..1 pushed by a gate; linear and clamped, so it reaches its ends *exactly*), **Fader** (which scene shows, which is leaving; weights sum to 1). Shared by every front-end. |
| **hot reload** | `vybe run` watches the patch's file and swaps in each save without restarting the performance (`Player::reload`). State is carried over **by name** — the same rule as **key = identity**, on the CPU side: gates, ramps, playheads, the showing scene. A broken save changes nothing: the last good patch plays on. |
| **diagnostic** | A finding of `parse`/`check`, tied to a line, carrying its next step (`did you mean`, the `ffmpeg` line). Errors teach. |
| **kata pair** | The same picture as an `.rs` chain and a `.vy` patch, rendered headless and compared pixel for pixel (`tests/pairs.rs`) — the only thing that keeps the two front-ends from drifting. |

## The stage (where the picture meets the world)

| Term | Meaning |
|------|---------|
| **stage** | The output plus everything listening to it: window (or headless frame), **warp**, **bindings**, clock. `Stage` is the builder `live()` and `Player::stage()` return; like every chain it ends on `.show()` (or `.render(..)`). |
| **Inputs** | The address space of live values — `/hands`, `/mode`, `/key/space`, `/mouse/x`. A patch reads it with `osc <address>`; OSC, the keyboard and the mouse all write into it, which is why a patch can't tell a laptop from a wall. Total: a silent address reads as rest. |
| **Binding** | The one seam integrations enter through: sees window events and every frame; may touch the **Inputs**, the **Warp**, the `tune` registry. OSC, the remote protocol, scripted inputs and the generic bindings are all implementations — the core never learns what a socket is. |
| **generic bindings** | `arrows_nudge(glob)` (arrows move `…/x` / `…/y` tunes; `{name}` in the glob reads a tune) and `mouse_drag(glob, r)` (the mouse drags `<prefix>/x`+`/y` *pairs*), plus `.on(Key, ..)` and `key_cycle(name, key, n)`. Reusable in any work; none is a UI noun. |
| **picture** | What lands on the wall. By default it *is* the output. With `out … picture WxH` it has a size of its own (a square face on a 16:10 projector): it renders at that size, **rests** contained and centered, and the keystone's corners are *its* corners. Not called "source" — that word is taken by `circle`/`frames`/… |
| **Warp / keystone** | Where the **picture's** four corners land on the output (TL TR BR BL, fractions of the output, **y down** — the boundary, where pixels live) plus a `feather`. A 4-corner homography is exact for a flat face. The present pass *is* the keystone; at rest it is a plain copy. The `keystone` crate is only the *file* (JSON, atomic save, `.bak`) — **the room's half of a work**: what differs per machine (the corners, the feather, whether the player opens fullscreen). |
| **remote** | `remote <port>` on a patch's `out` line brings up the face's end of the OSC protocol (`vybe-remote::Face`); `vybe-remote` the program is a vybe sketch whose corners are `tune`s mirrored to the face (`Peer`). The face announces its shape (`/keystone/shape`), so one remote maps any proportions — a 16:9 screen, a cube's square face. The player only *reads* the keystone file; the remote *writes* it (via `/keystone/save`). |
| **headless / Render** | No window: a **fixed clock**, scripted inputs, PNGs out. Same input, same pixels. `vybe render`, or `VYBE_RENDER="at=2 size=500x500"` on any example. |
| **Clock** | The one place time is read: *wall* (real seconds; framerate-independent motion) or *fixed* (every frame exactly one step — deterministic, what headless runs on). |

## The language (how sketches are written)

| Term | Meaning |
|------|---------|
| **chain** | A sentence of links: `circle().wave().render().feedback().show()`. The artist's program — and the AST at the same time (chain = graph). |
| **link** | One method call in the chain. Each link fills in only what it touches; links never fight over a slot. |
| **knob** | A parameter the artist turns — always a plain struct with a useful `Default` (`Swirl { decay, .. }`). Technical complexity hides behind knobs (Principle 4). |
| **terminal link** | The link that ends description and starts execution: `.show()` (one day, `.out()`). Everything before it only builds data. |
| **tune()** | Picks one value out of a chain, by name: `max: tune("reach", 0.30, 0.0..=1.0)`. Registers the knob; returns its current value; total (no front-end → the default). Front-ends turn *only* what was picked. |
| **live()** | Runs the sketch as a *function of its knobs*: `live(\|\| circle(..)...).show()` re-describes the chain whenever a tuned value changes. Returns a **Stage**, so bindings hang on it before `.show()`. The same seam a patch **Player** uses — it just re-describes every frame. |
| **the bridge** | `.render()` — geometry becomes a texture `Signal` and flows into the Braid world. The chain never breaks; it changes signal type at this explicit point. |
| **sugar** | A dialect for writing chains. Rust is the base dialect; TS/JS and Lua come later. Every dialect produces the same recipe (Multi-Sugar). |
| **socket / plug** | A link argument that accepts an input signal: `at(mouse())` today; `touch(0)` or an oscillator tomorrow — same socket, different plug. |

## The types (what flows)

| Term | Meaning |
|------|---------|
| **Signal** | The single currency (Principle 1). Today: the texture world — fullscreen effects, feedback. Before adding a type, ask "isn't this just a Signal?". |
| **Shape** | The geometry world: one shape, its placement, motion, and paint. Becomes a `Signal` at the bridge. |
| **Layers** | A stack of *worlds* composed into one scene, built by `layers(iter)` over **Layer**s — `Shape`s, `Signal`s, `Particles` alike, each with its **Blend**. Listed **top first**: item 0 is the frontmost (like a layers panel); the core draws back-to-front. An all-`over` geometry stack (the default) stays one cheap pass — painter's-order alpha *is* over; if any layer carries feedback/particles (or asks to *add*), the stack **composites** — each layer to its own texture, combined onto the screen per its blend, so feedback lives on one layer without smearing the others. `.render()` instead folds the whole stack into *one* `Signal` (a single loop over everything). |
| **Particles** | The point-cloud world (third signal type): `particles(n)` seeds a GPU buffer of positions/velocities, stepped in a compute shader and drawn instanced. The payload is a buffer, not a recipe — built for a million+, with the per-particle data never crossing back to the CPU. |
| **forces** | The composable *behavior* a `particles(..)` chain hangs on the cloud, applied by the compute step: `swirl` (rotate around center), `gravity(x,y)` (constant pull), `attract`/`repel` a `Pos` (radial, within reach), `orbit` a `Pos` (tangential vortex). A bounded, declarative vocabulary (a small GPU force stack) — not a general value-flow. Tunable live via `tune()`/`live()`. |
| **Stroke** | *Internal.* One flattened shape-chain — a single gesture: a form (circle or rect; a `line` is a rect laid along its two points) + placement + motion + paint (hue, `gray`, `alpha`, `stroke` outline). |
| **Recipe** | *Internal.* What the core draws *this frame*, as plain data with numbers in it. Handed over by the Rust sugar (once, or when a knob turns) and by a patch **Player** (every frame). The **Patch** is the AST above it. Every front-end, one recipe. |
| **the Braid world** | The texture/feedback side of the chain. Lineage: Dimitre's Braid, Olivia Jack's Hydra. |

## The knob structs

| Term | Meaning |
|------|---------|
| **Swirl** | The feedback knobs, all **per second** (the engine applies them by `dt`, so the loop looks the same on any monitor). `new = swirl(previous) · decay + source`. `decay` = fraction of the trail surviving per second; `angle` = radians per second; `scale` = zoom per second (`<1` drifts outward, `>1` sucks inward). |
| **Wave** | Motion added to a shape's position. `amp` (scene units), `x`/`y` (frequencies, cycles per second; `0.0` = still axis), `phase` (turns — stagger it across layers to make voices), `shape` (an `Osc`). Axes run in quadrature (y a quarter-cycle ahead), so equal frequencies orbit instead of sliding on a diagonal. |
| **Osc** | The waveform menu for `Wave`, TouchDesigner-style: Sine, Cosine, Triangle, Ramp (sawtooth), Square, Pulse (high 25% of the cycle). One cycle per turn. |
| **Hue** | Paint. `base` = degrees on the color wheel (0 red, 120 green, 240 blue); `drift` = degrees per second it slides. A bare `f32` converts. Unpainted shapes are white. |
| **Falloff** | Proximity growth. Full effect inside `min`, none beyond `max` (scene units), smoothstep between; radius × `scale` at the epicenter (`<1` shrinks instead). |
| **Pos** | A position source shared by `at(..)` (placement) and `grow(.., ..)` (the swell's epicenter): `mouse()` or a fixed `(x, y)`. A constant is just another signal; a tuned pair `(tune("x", ..), tune("y", ..))` drives it by hand. |
| **Source** | *Internal.* The resolved `Pos` stored in a `Stroke`: a fixed scene point or the live mouse. Resolved in the shader as `mix(point, mouse, from_mouse)`. One type behind both `at` and `grow`; a future `touch(0)`/oscillator is a third variant. |

## Spaces & conventions

| Term | Meaning |
|------|---------|
| **scene space** | The artist's coordinates: center `(0, 0)`, y-up, the shorter screen edge spans `-0.5..+0.5` (TouchDesigner-style). Units square on any aspect; every distance reads as a fraction of the screen. |
| **cell** | One slot of a `grid(cols, rows)`. A shape's radius is a fraction of its cell (`0.5` = neighbours touch); with no grid, the whole scene is the one cell. |
| **texel space (uv)** | Inside fullscreen passes, `uv` is the fragment's own texel coordinate (y-down, `v = 0` at the top). Write and sample coincide — no pass hides a flip (see DECISIONS: the mirror bug). |
| **pixels** | Exist only at the boundary: the present pass and the mouse-event conversion. They never reach a sketch. |

## Engine internals

| Term | Meaning |
|------|---------|
| **core** | The `vybe` lib — "don't touch it". Creating happens in sketches only (the Processing model, born on day 1). |
| **the four modules** | One module per architecture layer: `sugar` (the chains) · `recipe` (the AST) · `gpu` (all of wgpu) · `shell` (the winit loop). |
| **sketch / kata** | One runnable example in `examples/*.rs` — short, readable, only the visual. Each kata pulls exactly one new capability and leaves a design lesson. |
| **ping-pong** | Two signal textures alternating read/write each frame. The hidden machinery behind the `decay` knob. |
| **frame block** | `group(0)` uniforms: resolution, mouse, time — rewritten 60×/s, shared by every pass. |
| **stroke knobs** | `group(1)` uniforms: one stroke's knobs. **Live**: refreshed from the recipe every frame (a handful of floats), so front-ends — panel now; MIDI/OSC/scripts later — retune a running sketch. The frame/stroke split still keeps per-frame traffic tiny. |
| **Overlay** | The one seam for front-ends drawn over a running sketch: sees events first, paints over the finished frame. The core knows this trait and nothing else — no UI library ever touches the engine. |
| **tweak panel** | The first Overlay: one slider per `tune()`d knob, nothing else (`src/tweak.rs`, feature `tweak`, zero cost by default). egui core + egui-winit from crates; the *renderer* is ours (~150 lines on our wgpu), because egui-wgpu pins wgpu 29 while the core rides 30. |
| **instancing** | One draw call, many instances; each grid cell (or particle) derives from the instance index alone. No vertex or instance buffers are ever uploaded. |
| **compute step** | The GPU simulation pass for `Particles`: one compute invocation per particle integrates its state in a storage buffer each frame (`particles.wgsl`). The buffer stays on the GPU; the draw reads the same buffer the step wrote. |
| **SDF circle** | The circle is cut from a quad in the fragment shader by a signed-distance function — anti-aliasing (and the `soft` knob) for free. |
| **painter's order** | A plain geometry stack draws in sequence, later over earlier (one pass). The artist lists **top first**, so `flatten` reverses the list into this draw order — the one place the two orders meet. Painter's-order alpha *is* `Over` (over is associative), which is why an all-over geometry stack needs no compositor. |
| **compositing** | A mixed `layers()` stack (or a patch composition) drawn as separate worlds — each rendered to its own signal texture, then combined by a **mix node** per each layer's **Blend** and opacity. What lets feedback sit on one layer without smearing the rest. |
| **node list** | *Internal.* How the GPU core runs any recipe: flattened into nodes in dependency order, each rendering into its own signal texture; a mix node samples earlier ones; one present pass lands the last on the output. Reconciled **by key** on re-description, so surviving nodes keep their state. Replaced the parallel `Passes`/`Layer` hierarchies (DECISIONS 2026-09-20). |
| **Blend** | How the compositor combines a layer with the worlds beneath it. `Over` (default): alpha compositing (TouchDesigner's *over*, a design tool's *Normal*), each layer on top by its own coverage, the world beneath showing through where it's transparent — a stack reads like a layers panel. `Add`: `src + dst`, so light glows and black adds nothing — crowds and feedback trails, one `.blend(Blend::Add)` away. Set per layer (`world.blend(..)`) or for the whole stack (`layers().blend(..)`, the per-layer default) — a stack can mix modes. Layer textures are premultiplied (geometry drawn onto *transparent* black), which is what lets `over` read straight off them. Why over won the default: DECISIONS 2026-07-08. |
| **Layer** | One world in a `layers()` stack paired with its **Blend** — any composable world (a `Signal`, a `Shape` lifted through the bridge, or a `Particles` cloud) plus how it lands on the worlds beneath. `world.blend(mode)` builds one; a bare world inherits the stack's default. Blend lives here, at the composition boundary, not inside the world's own chain — it's a *relation*, so the same world can stack differently elsewhere. |
| **`layers!` macro** | Builds a stack from an explicit list, converting each item to a **Layer** independently — so a `Shape`, a `Particles`, and a `.blend(..)`ed layer can share one list despite being different types (a plain `[..]` array can't; its items must all be one type). Bare items take the default blend; spell `.blend(..)` only where a layer differs. `layers(iter)` (the function) stays for an iterator — a `map` over a range. |
| **World** | The internal sum of what a **Layer** can hold — the composable signal types (texture `Signal`, point-cloud `Particles`). The seam the rule "everything composes" widens as new signal types arrive. |

## Method & philosophy

| Term | Meaning |
|------|---------|
| **examples pull features** | Golden Rule #2: no core capability enters without the sketch that demanded it. Speculation never does. |
| **identity / total** | Every link left unset is a valid resting state (Principle 2), and describing never fails or panics — only IO returns `Result` (Principle 3). |
| **hide the ping-pong, expose the knob** | Principle 4: the artist sees `decay`, never the two buffers. Machine coordinate systems get the same treatment. |
| **Multi-Sugar** | One core, many dialects (Rust now; TS/JS, Lua later). The reason the core exists separate from everything else. |
| **LLM-sized** | The core must fit comfortably in one LLM context (Principle 7). Growing past that means something becomes an addon crate. |
| **Shokunin (職人気質)** | The craftsman's spirit: never the easy path — the cleanest, most long-term-optimized one. |
| **vybe / tekne** | *vybe*: the engine crate — a play on *vibe coding* (its natural-language interface), read as a backronym for a **V**isual **E**ngine. *Tekne* (τέχνη): the craft — the brand and future workspace, once a second project exists. |
