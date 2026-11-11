//! The compact variants a registry name has under
//! [`ControlSize::Compact`](crate::tree::ControlSize::Compact).
//!
//! Each row has the name and the parameter shape of the row it stands in for
//! and is never listed in [`super::entries`]: a plugin writes `button`, and a
//! container decides which size that is. So the catalog and the Luau stubs
//! have one name per component, not two.

use super::Entry;
use crate::component as lib;
use crate::component::params::{KeyChildren, KeyLabel, KeyLabelSelected};

/// The compact rows. A name absent here has no compact variant and is built
/// as it always was.
pub(super) const COMPACT: &[Entry] = &[
    row!("text", KeyLabel, |p| lib::text_xs(p.key, p.label)),
    row!("button", KeyLabel, |p| lib::button_xs(p.key, p.label)),
    row!("primary_button", KeyLabel, |p| lib::primary_button_xs(
        p.key, p.label
    )),
    row!("tertiary_button", KeyLabel, |p| lib::tertiary_button_xs(
        p.key, p.label
    )),
    row!("ghost_button", KeyLabel, |p| lib::ghost_button_xs(
        p.key, p.label
    )),
    row!("danger_button", KeyLabel, |p| lib::danger_button_xs(
        p.key, p.label
    )),
    row!("danger_tertiary_button", KeyLabel, |p| {
        lib::danger_tertiary_button_xs(p.key, p.label)
    }),
    row!("danger_ghost_button", KeyLabel, |p| {
        lib::danger_ghost_button_xs(p.key, p.label)
    }),
    row!("checkbox", KeyLabelSelected, |p| lib::checkbox_xs(
        p.key, p.label, p.selected
    )),
    row!("chrome_strip", KeyChildren, |p| lib::chrome_strip_xs(
        p.key, p.children
    )),
];

/// The compact row for `name`, if it has one.
pub(super) fn lookup(name: &str) -> Option<Entry> {
    COMPACT.iter().find(|entry| entry.name == name).copied()
}
