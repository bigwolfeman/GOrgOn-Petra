//! Design §6's built-in behaviours as slot writes (D-109 step 4).
//!
//! Every interactive Carbon component declares its interaction as a slot
//! write, implemented in Rust with no Lua (`.agents/notes/proposed/
//! architecture/2026-09-27-bound-slot-table-ui-model.md` §6). This module
//! is that table for the single components: given the slot a control
//! writes, what the control currently holds, and the version the change
//! reaches, it answers the exact [`SlotChange`] set of one commit group
//! (design §1). Pure — no table, no daemon, no clock: the versions are the
//! authority's to assign (§2), so they arrive as arguments rather than
//! being invented here, and the wiring that binds a control to its slot
//! comes with the shell's pixel turn (§8).
//!
//! | §6 row | Interaction | `Behaviour` | Write |
//! |---|---|---|---|
//! | Toggle, checkbox, expand | click | `Toggle` / `OnRelease` | [`toggle`] |
//! | Radio, tabs, content switcher | select | `Select` / `OnRelease` | [`select`] |
//! | Slider | drag | `Adjust` / `OnChange` | [`adjust`], [`CoalescedWrites`] |
//!
//! §6's two data-table rows are a compound's business and live in
//! `gorgon-petra-compound`'s `data_table` module; text input is step 8's
//! `Text` slot and deliberately has no row here.

use std::collections::BTreeMap;

use crate::tree::{SlotChange, SlotKey, SlotValue};

/// Toggle, checkbox, expand: a click writes `Bool := !v` (§6).
///
/// `held` is the slot's current value — what the control presently is: a
/// checked checkbox, an on toggle, a disclosed accordion body. The write is
/// the flip, never a literal `true` or `false`, so a control cannot
/// disagree with the table it writes.
#[must_use]
pub fn toggle(slot: impl Into<SlotKey>, held: bool, at: u32) -> Vec<SlotChange> {
    vec![SlotChange::new(slot, at, SlotValue::Bool(!held))]
}

/// Radio, tabs, content switcher: a selection writes `Enum := option` (§6).
///
/// `Enum` arrives as [`SlotValue::Str`] (design §3), and `option` is the
/// chosen option's own name: the radio's value, the tab's key, the
/// content-switcher section's id.
#[must_use]
pub fn select(slot: impl Into<SlotKey>, option: &str, at: u32) -> Vec<SlotChange> {
    vec![SlotChange::new(slot, at, SlotValue::Str(option.to_owned()))]
}

/// Slider: one adjustment writes `F64 := value` (§6) — [`SlotValue::Num`],
/// where both `I64` and `F64` land (design §3).
///
/// A drag is [`CoalescedWrites`], not one of these per motion event.
#[must_use]
pub fn adjust(slot: impl Into<SlotKey>, value: f64, at: u32) -> Vec<SlotChange> {
    vec![SlotChange::new(slot, at, SlotValue::Num(value))]
}

/// A drag's writes, coalesced per slot (§6: `F64`, coalesced while
/// dragging).
///
/// A drag is a stream of motion events and a table write per motion is a
/// commit per motion for a picture that only ever shows the latest value.
/// [`push`](Self::push) replaces whatever was pending for that slot;
/// [`commit`](Self::commit) drains what survived into one change per slot,
/// in slot order. Nothing is dropped without a newer value replacing it —
/// T5's coalescing rule (never a silent drop) in miniature.
#[derive(Clone, Debug, Default)]
pub struct CoalescedWrites {
    /// Latest pending value per slot. `BTreeMap` so a commit group's order
    /// is a function of the slot names alone (frame identity's
    /// determinism rule reaches the wire, not just the frame).
    pending: BTreeMap<SlotKey, SlotValue>,
}

impl CoalescedWrites {
    /// Record `value` for `slot`, replacing whatever was pending there.
    pub fn push(&mut self, slot: impl Into<SlotKey>, value: SlotValue) {
        self.pending.insert(slot.into(), value);
    }

    /// Whether nothing is pending.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// How many slots hold a pending write.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Drain into one commit group: a [`SlotChange`] per pending slot, in
    /// slot order, each stamped with `at(slot)` — the version that slot
    /// reaches at this commit. The caller's `at` because versions are the
    /// authority's (§2), and per-slot because they are per-slot (§3).
    #[must_use]
    pub fn commit(self, mut at: impl FnMut(&SlotKey) -> u32) -> Vec<SlotChange> {
        self.pending
            .into_iter()
            .map(|(slot, value)| {
                let version = at(&slot);
                SlotChange::new(slot, version, value)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §6 row 1 — toggle, checkbox, expand: one click writes one
    /// `Bool := !v` and nothing else.
    #[test]
    fn a_toggle_click_writes_exactly_one_bool_flip() {
        assert_eq!(
            toggle("on", false, 4),
            vec![SlotChange::new("on", 4, SlotValue::Bool(true))],
            "off flips to on"
        );
        assert_eq!(
            toggle("expanded", true, 5),
            vec![SlotChange::new("expanded", 5, SlotValue::Bool(false))],
            "and on flips to off — the same rule, whatever the control is"
        );
    }

    /// §6 row 2 — radio, tabs, content switcher: one selection writes one
    /// `Enum := option`, as the `Str` `Enum` arrives as (design §3).
    #[test]
    fn a_selection_writes_exactly_one_enum_option() {
        assert_eq!(
            select("theme", "dark", 7),
            vec![SlotChange::new(
                "theme",
                7,
                SlotValue::Str("dark".to_owned())
            )]
        );
    }

    /// §6 row 5 — slider: one adjustment writes one `F64` (`Num`).
    #[test]
    fn an_adjustment_writes_exactly_one_number() {
        assert_eq!(
            adjust("vol", 0.4, 2),
            vec![SlotChange::new("vol", 2, SlotValue::Num(0.4))]
        );
    }

    /// §6's slider coalescing: a drag writes the latest value per slot,
    /// once per slot per commit — never one change per motion event, and
    /// never a dropped value with nothing newer in its place.
    #[test]
    fn a_drag_coalesces_to_the_latest_value_per_slot() {
        let mut drag = CoalescedWrites::default();
        drag.push("vol", SlotValue::Num(0.1));
        drag.push("vol", SlotValue::Num(0.4));
        drag.push("vol", SlotValue::Num(0.35));
        drag.push("zoom", SlotValue::Num(2.0));
        assert!(!drag.is_empty(), "four motions left two slots pending");
        assert_eq!(
            drag.commit(|slot| if slot.as_str() == "vol" { 3 } else { 9 }),
            vec![
                SlotChange::new("vol", 3, SlotValue::Num(0.35)),
                SlotChange::new("zoom", 9, SlotValue::Num(2.0)),
            ],
            "the three `vol` motions become one change carrying the last \
             value; `zoom` is untouched by them and stamps its own version"
        );
    }

    /// A commit with nothing pending is an empty group, not an error and
    /// not a leftover.
    #[test]
    fn an_idle_drag_commits_nothing() {
        let drag = CoalescedWrites::default();
        assert!(drag.is_empty());
        assert_eq!(drag.commit(|_| 1), Vec::<SlotChange>::new());
    }
}
