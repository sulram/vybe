//! TUNE — named knobs, picked one by one from inside a chain.
//!
//! `tune("reach", 0.3, 0.0..=1.0)` returns the knob's current value and, the
//! first time the name appears, registers it. In a `live()` sketch the
//! registry is the seam any front-end can turn — the tweak panel today;
//! MIDI, OSC, and script dialects tomorrow. std-only: the core never learns
//! what a slider is.

use std::cell::{Cell, RefCell};
use std::ops::RangeInclusive;

thread_local! {
    static TUNES: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
    static DIRTY: Cell<bool> = const { Cell::new(false) };
}

/// One picked knob.
#[cfg_attr(not(feature = "tweak"), allow(dead_code))]
pub(crate) struct Entry {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

/// The current value of the named knob; the first time the name appears it is
/// registered at `default`, with `range` as its reach. Total (Principle 3):
/// with no front-end attached it simply keeps returning `default`.
pub fn tune(name: &str, default: f32, range: RangeInclusive<f32>) -> f32 {
    TUNES.with(|tunes| {
        let mut tunes = tunes.borrow_mut();
        match tunes.iter().find(|e| e.name == name) {
            Some(e) => e.value,
            None => {
                tunes.push(Entry {
                    name: name.to_owned(),
                    value: default,
                    min: *range.start(),
                    max: *range.end(),
                });
                default
            }
        }
    })
}

/// True when at least one knob was picked.
#[cfg_attr(not(feature = "tweak"), allow(dead_code))]
pub(crate) fn any() -> bool {
    TUNES.with(|t| !t.borrow().is_empty())
}

/// Lets a front-end edit the knobs in place (the panel's sliders).
#[cfg_attr(not(feature = "tweak"), allow(dead_code))]
pub(crate) fn edit(f: impl FnOnce(&mut Vec<Entry>)) {
    TUNES.with(|t| f(&mut t.borrow_mut()));
}

/// A front-end changed something: the sketch should be re-described.
#[cfg_attr(not(feature = "tweak"), allow(dead_code))]
pub(crate) fn mark_dirty() {
    DIRTY.with(|d| d.set(true));
}

/// Consumes the dirty flag; checked once per frame by the shell.
pub(crate) fn take_dirty() -> bool {
    DIRTY.with(|d| d.replace(false))
}
