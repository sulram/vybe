# vybe-host

The daemon that boots into the show — and keeps it playing.

`vybe` the command is for authoring: run a patch, check it, render a frame.
`vybe-host` is for the wall: power in, picture out. It starts before anything
user-facing, picks the project marked default, hands it to the engine, and
stands guard. A broken upload never reaches the wall — the checker sees it
first, and the current show plays on.

## Boot-to-show

1. The host starts. (On the appliance: read-only root, watchdog armed — that
   side lives in [vybe-pi](https://github.com/tekne-studio/vybe-pi).)
2. It reads the registry, finds the project marked default.
3. The engine plays it. This path is indestructible by design: anything that
   can fail fails *before* the show changes, never during.

## The registry

A project is a folder — the unit the host can boot:

```
projects/
  default              # one word: which project boots
  map-show/
    project.toml       # name, engine entry, options
    patch.vy           # or app.lua, or a Rust binary reference
    media/             # clips, images, audio
    keystone.json      # the calibration, next to the work as always
```

"Set as startup" rewrites the one-word `default` file — atomically, and
never touching the project itself.

## The control surface

One small API, two doors — HTTP for the web admin, OSC for the remote that
already calibrates the keystone:

- upload / list / delete media
- switch project, set startup project
- status: playing what, since when, disk, temperature

Every upload passes the patch checker before it can become the show. The
host's first loyalty is to what's playing.

## What runs on it — the three tiers

| Tier | What | Compiles where | When it fails |
|------|------|----------------|---------------|
| 1 | `.vy` patches | here, instantly, checked | the checker rejects; the show continues |
| 2 | Lua apps (mlua, sandboxed) | here, in seconds | the sandbox kills it; the fallback runs |
| 3 | Rust binaries | **off-device**, cross-compiled, pushed | the supervisor restarts it, or falls back |

No toolchain on the wall. A tier-3 app arrives as a binary and runs as a
supervised child — the host outlives it.

## The line with vybe-pi

**Needs the checker or hot-reload → this crate. Needs systemd or the imager
→ [vybe-pi](https://github.com/tekne-studio/vybe-pi).**

The web admin's assets (the router-style PWA) live in vybe-pi; the host just
serves the static directory it's given. And nothing here is Pi-specific —
the same daemon runs on a desktop that wants project switching, which is how
it gets developed and tested before any wall sees it.

## Status

Scaffold. Today the binary scans a registry and says what a boot would play:

```
cargo run -p vybe-host -- ./projects
```

Supervision and the control API land when the appliance kata pulls them —
features are pulled, never speculated.
