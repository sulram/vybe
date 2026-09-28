# HOST.md — the device: vybe as an appliance

The plan for vybe as the core of a **projector device** — a small box (a Pi)
in the MadMapper spirit: you power it on, the show plays; you reach it over
the network to map, to change the media, to load something new. Where the
device came from and what it is made of. The engine's beliefs live in
[VISION.md](VISION.md); the day-to-day backlog in [ROADMAP.md](ROADMAP.md);
the *why* of each call in [DECISIONS.md](DECISIONS.md).

> A face on a wall has no keyboard. The device is the face; the host is what
> makes a computer behave like a face.

## What the device already is

Most of the hard parts shipped in 0.0.2, before the device was named:

- **The keystone is the present pass** — a 4-corner homography, perspective-
  correct, saved per room in `keystone.json`. Shape belongs to the work (the
  patch's `out … picture WxH`); calibration belongs to the room (the file).
- **The remote talks OSC** — corners, fullscreen, reset; the same UDP on one
  laptop today, across the room tomorrow.
- **Video plays with its sound** — GStreamer behind the `Clip`/`Decoder` seam,
  chosen because it is the same backend on the laptop and on the Pi.
- **Hot reload** — a saved patch swaps into the running show; a broken save
  never blacks out a wall.
- **The player is small** — 7.4 MB, linking against nothing but the OS. A
  device image is that binary plus a systemd unit.
- **Headless is honest** — `vybe render` and `vybe check` need no screen, so
  the device's own CI can prove a show before it ships.

## The host

The host is a small **supervisor**, not a new engine. It boots first, owns
nothing visual, and answers three questions: *what plays now, what plays at
power-on, and what else is on this box.*

```
vybe-host (daemon, boots first)
├── registry — the projects on the box; one is the default
├── runs the default at power-on — "turn it on, the show plays"
├── supervises — player and apps as child processes; a crash restarts
│   or falls back to the default, never to black
├── API (HTTP + OSC) — upload media, manage the playlist, switch the
│   project, set the startup project, ask the status
└── appliance rules — read-only rootfs, watchdog, atomic updates
```

**A project is a folder.** The patch or app, its media, its `keystone.json` —
everything a work needs, nothing it doesn't. Uploading a project is copying a
folder; backing up a device is tarring a directory; moving a show between
boxes is `scp`. The host's registry is just the folders it can see.

**The default project is a pointer, not a copy.** Setting the startup project
moves the pointer. Power-on plays whatever the pointer names — the venue's
show, a test pattern while installing, a *sos* patch that says "the box is
alive but empty" when nothing else exists.

## Three tiers of work — "compile on demand" means three things

The wish was a host that can compile on demand. It can — but each tier
compiles at its own speed, and the tier is chosen by what the work needs,
not by what the author prefers.

**Tier 1 · the patch (`.vy`) — instant, on the device.** For a patch,
"compiling" is parse → check → play: milliseconds, and safe by construction —
the checker refuses a broken patch and the last good one plays on. This is
the media player, the mapping show, the generative loop, the live edit from
the room. *Most of what the device will ever do is a patch.*

**Tier 2 · the scripted app — seconds, sandboxed, on the device.** For the
interactive works — sensors, logic, a game, a piece that listens and answers —
a script (Lua/WASM, the roadmap's deferred dialects, pulled forward by the
device) over the same core. A script can crash; the sandbox cannot take the
host down. This tier is where "custom apps" live. *(Not built yet — see
ROADMAP; the `Binding` seam and the `Inputs` address space were designed for
it.)*

**Tier 3 · Rust — full power, compiled off the device.** Rust is for what a
patch cannot say, and it enters two ways, both already principled:

- **`rust <name>` leaves** — compiled *into* the player, the way TouchDesigner
  has custom C++ operators. The leaf is a node in the patch; the patch stays
  the product.
- **A standalone binary** — `cargo build --release --example feedback` is
  already one. The host supervises it like any other child.

What the device does **not** do is run rustc: gigabytes of toolchain,
minutes of compiling, SD-card wear, and RAM pressure against a running show —
to produce a binary a laptop cross-compiles in seconds and pushes over the
network. And Rust has no stable ABI, so hot-loading Rust into the host would
pin every plugin to one compiler version. Process isolation instead of ABI
risk: a Rust app is a child the host can kill.

> The patch is the format that needs no compiler. The script is the format
> that needs no toolchain. Rust is the format that needs no limits.

## The API, smallest first

Everything the host does is reachable over the network — the venue's laptop,
a phone, eventually a little web panel. In the order the device pulls them:

1. **Status** — what is playing, since when, at what frame rate. (The remote
   already asks some of this over OSC.)
2. **Media upload/download** — files into the project's folder; the checker
   validates what a patch references before it goes live.
3. **Playlist** — a playlist is a patch: scenes and transitions over media
   files, so the player needs nothing new. Managing one is editing text.
4. **Switch / startup** — run this project now; make this project the default.
5. **Calibration** — the keystone remote, already speaking OSC, becomes one
   more client of the same box.

## Appliance rules

What makes it a device and not a laptop in a box:

- **Boot-to-show.** Power on → host → default project. No login, no desktop,
  no window manager in the path (the roadmap's `vybe-stage-drm`: KMS/GBM, no
  X).
- **Read-only rootfs.** The SD card dies by writes; the system partition
  mounts read-only, projects and media live on a data partition.
- **Watchdog.** A hung process reboots into the default show. A wall that
  goes black is a bug; a wall that stays black is a failure.
- **Atomic updates.** New binary or new project lands beside the old; the
  pointer flips; a bad flip flips back. The same rule hot reload already
  keeps: the last good show plays on.
- **mDNS** (`_vybe._tcp`, already on the roadmap) — the box announces itself;
  you never type an IP in a venue.

## What the host is not

- **Not a desktop.** No browser kiosk, no Electron panel on the box. The UI
  is anywhere else — the remote, a web page, a phone.
- **Not feature parity with MadMapper.** Mesh warp, DMX/Art-Net, Syphon are
  wishes that wait for the work that pulls them (Golden Rule #2 applies to
  the device too). The wedge is elsewhere: open, scriptable, cheap, and the
  patch is text you can read.
- **Not a cloud service.** The box is sovereign like the engine: it works in
  a room with no internet, forever.

## Open questions

- The Tier-2 script language: Lua via `mlua` (the roadmap's lean) or WASM
  guests (wasmtime)? Decide when the first interactive work pulls it.
- One box, many faces: the roadmap's cube on four Pi 5 units — does the host
  stay per-box with OSC between them, or does one host conduct several?
- Updates of the host itself: `apt` package, an image, or a self-swapping
  binary under the atomic rule?
- Where the web panel lives: served by the host (a few static files) or a
  remote that grew up.
