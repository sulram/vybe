//! TUNE — named knobs, picked one by one from inside a chain.
//!
//! `tune("reach", 0.3, 0.0..=1.0)` returns the knob's current value and, the
//! first time the name appears, registers it. In a `live()` sketch the
//! registry is the seam any front-end can turn — the tweak panel, a key, the
//! mouse, a remote over OSC ([`get`] / [`set`] / [`names`] are their whole
//! API). std-only: the core never learns what a slider is.

use std::cell::{Cell, RefCell};
use std::ops::RangeInclusive;

use crate::input::Key;

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
    /// A cycling knob: `key` steps it through `0..count` (see [`key_cycle`]).
    pub cycle: Option<(Key, u32)>,
}

/// The current value of the named knob; the first time the name appears it is
/// registered at `default`, with `range` as its reach. Total (Principle 3):
/// with no front-end attached it simply keeps returning `default`.
pub fn tune(name: &str, default: f32, range: RangeInclusive<f32>) -> f32 {
    pick(name, default, *range.start(), *range.end(), None)
}

/// A knob that counts: `key` steps it through `0..count`, wrapping. Picked by
/// name like any [`tune`] — `key_cycle("sel", Key::Tab, 4)` is "which of the
/// four", and anything that reads or sets tunes can read or set it.
pub fn key_cycle(name: &str, key: Key, count: u32) -> usize {
    let count = count.max(1);
    pick(name, 0.0, 0.0, (count - 1) as f32, Some((key, count))) as usize
}

fn pick(name: &str, default: f32, min: f32, max: f32, cycle: Option<(Key, u32)>) -> f32 {
    TUNES.with(|tunes| {
        let mut tunes = tunes.borrow_mut();
        match tunes.iter().find(|e| e.name == name) {
            Some(e) => e.value,
            None => {
                tunes.push(Entry {
                    name: name.to_owned(),
                    value: default,
                    min,
                    max,
                    cycle,
                });
                default
            }
        }
    })
}

/// The named knob's current value, if it was ever picked.
pub fn get(name: &str) -> Option<f32> {
    TUNES.with(|t| t.borrow().iter().find(|e| e.name == name).map(|e| e.value))
}

/// Turns the named knob (clamped to its range); the sketch is re-described
/// next frame. Returns false when nothing by that name was picked — a
/// front-end can only turn what the artist exposed.
pub fn set(name: &str, value: f32) -> bool {
    let turned = TUNES.with(|t| {
        let mut tunes = t.borrow_mut();
        let entry = tunes.iter_mut().find(|e| e.name == name)?;
        let value = value.clamp(entry.min, entry.max);
        let changed = entry.value != value;
        entry.value = value;
        Some(changed)
    });
    if turned == Some(true) {
        mark_dirty();
    }
    turned.is_some()
}

/// Every picked knob's name, in the order they were first picked.
pub fn names() -> Vec<String> {
    TUNES.with(|t| t.borrow().iter().map(|e| e.name.clone()).collect())
}

/// `key` went down: step every cycling knob bound to it.
pub(crate) fn cycle(key: Key) {
    let stepped = TUNES.with(|t| {
        let mut stepped = false;
        for entry in t.borrow_mut().iter_mut() {
            if let Some((bound, count)) = entry.cycle {
                if bound == key {
                    entry.value = ((entry.value as u32 + 1) % count) as f32;
                    stepped = true;
                }
            }
        }
        stepped
    });
    if stepped {
        mark_dirty();
    }
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
pub(crate) fn mark_dirty() {
    DIRTY.with(|d| d.set(true));
}

/// Consumes the dirty flag; checked once per frame by the shell.
pub(crate) fn take_dirty() -> bool {
    DIRTY.with(|d| d.replace(false))
}
