# GLOSSARY.md — the vocabulary of `vybe`

Naming is design (README: "not an afterthought"). This is the shared
vocabulary across code, docs, and conversation — one place, one meaning.
The *why* lives in [README.md](../README.md); settled decisions in
[DECISIONS.md](DECISIONS.md); day-to-day rules in [CLAUDE.md](../CLAUDE.md).

## The language (how sketches are written)

| Term | Meaning |
|------|---------|
| **chain** | A sentence of links: `circle().wave().render().feedback().show()`. The artist's program — and the AST at the same time (chain = graph). |
| **link** | One method call in the chain. Each link fills in only what it touches; links never fight over a slot. |
| **knob** | A parameter the artist turns — always a plain struct with a useful `Default` (`Swirl { decay, .. }`). Technical complexity hides behind knobs (Principle 4). |
| **terminal link** | The link that ends description and starts execution: `.show()` (one day, `.out()`). Everything before it only builds data. |
| **tune()** | Picks one value out of a chain, by name: `max: tune("reach", 0.30, 0.0..=1.0)`. Registers the knob; returns its current value; total (no front-end → the default). Front-ends turn *only* what was picked. |
| **live()** | Runs the sketch as a *function of its knobs*: `live(\|\| circle(..)...)` re-describes the chain whenever a tuned value changes. The hot-reload seam in miniature — panel today; MIDI/OSC/scripts tomorrow. |
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
| **Stroke** | *Internal.* One flattened shape-chain — a single gesture: prototype + placement + motion + paint. |
| **Recipe** | *Internal.* The flattened chain the sugar hands the core at the terminal link. Chain = AST made literal; the seam where future dialects and the node front-end plug in. |
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
| **compositing** | A mixed `layers()` stack drawn as separate worlds — each layer rendered to its own signal texture, then combined onto the screen per the stack's **Blend**. What lets feedback sit on one layer without smearing the rest. |
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
