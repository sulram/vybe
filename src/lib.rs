//! # vybe
//!
//! A creative-coding engine on WebGPU: one core, many sugars. Chains
//! describe; the GPU executes. This crate is the core plus its first dialect
//! (Rust) — the lib is "don't touch it"; sketches live in `examples/`.
//!
//! The map (the target architecture, in miniature — one module per layer):
//! - `sugar` — the chains the artist writes: `circle().wave().render()...`
//! - `recipe` — the flattened chain (chain = AST), the Multi-Sugar seam
//! - `gpu` — all of wgpu, hidden behind the knobs
//! - `shell` — the winit window/input loop
//!
//! Key vocabulary:
//! - **Signal** — the single currency (Principle 1); today, the texture world.
//! - **Shape** — the geometry world; `.render()` is the bridge between the
//!   two. The chain never breaks — it changes signal type at that point.
//! - **Scene space** — the artist's coordinates: center `(0, 0)`, y-up, the
//!   shorter screen edge spanning `-0.5..+0.5` (TouchDesigner-style). Pixels
//!   never reach a sketch.

mod gpu;
mod recipe;
mod shell;
mod sugar;
#[cfg(test)]
mod tests;
mod tune;
#[cfg(feature = "tweak")]
mod tweak;

pub use sugar::*;
pub use tune::tune;
