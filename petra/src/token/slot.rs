//! The declared set of paint slots a node's `Props.tokens` may bind, and
//! what kind of token each slot accepts.
//!
//! Sibling of [`super::vocabulary::Vocabulary`] (token *names* the theme
//! must define) rather than a replacement for it: a `SlotSchema` entry says
//! "the `shadow` slot wants a `Color` token"; `Vocabulary` says "the theme
//! must define `shadow.ambient` as a `Color`". A binding is only sound when
//! both hold.
//!
//! This module declares the schema only. Nothing in `gorgon-petra` or
//! `gorgon-petra-egui` reads it yet — `tree::validate` does not check a
//! node's slot names against it, and `gorgon-petra-egui`'s painter still
//! carries its own three-entry `KNOWN_SLOTS`. Landing the full declared set
//! now, ahead of both consumers, means a later leaf wires up the check
//! without a second migration to grow the slot set itself. See
//! `.agents/research/08-22-2026/Petra-Visual-Design/engine/token-slot-schema.md`
//! §3-§4 for the full argument.

use std::collections::BTreeMap;

use crate::token::value::TokenKind;

/// One declared paint slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotSpec {
    name: &'static str,
    kind: TokenKind,
    required: bool,
}

impl SlotSpec {
    /// Declare a slot. `required = true` means a node that omits this slot
    /// gets no default from the schema (the *painter* may still choose one,
    /// as `FOREGROUND_SLOT` does today with `DEFAULT_TEXT_TOKEN` — a painter
    /// default is a rendering choice, not a schema promise).
    #[must_use]
    pub const fn new(name: &'static str, kind: TokenKind, required: bool) -> Self {
        Self {
            name,
            kind,
            required,
        }
    }

    /// The slot's name, as it appears in `Props.tokens`'s keys.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The value shape a token bound to this slot must resolve to.
    #[must_use]
    pub fn kind(&self) -> TokenKind {
        self.kind
    }

    /// Whether the schema itself requires this slot to be bound.
    #[must_use]
    pub fn required(&self) -> bool {
        self.required
    }
}

/// The full declared set of paint slots. Analogous to
/// [`super::vocabulary::Vocabulary`] but keyed on slot name rather than
/// token name, and carrying a [`TokenKind`] per entry rather than requiring
/// every entry be the same kind.
#[derive(Clone, Debug, Default)]
pub struct SlotSchema {
    slots: BTreeMap<&'static str, SlotSpec>,
}

impl SlotSchema {
    /// An empty schema.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a slot. A later call under the same name overwrites the
    /// earlier one, mirroring `Vocabulary::declare`.
    pub fn declare(&mut self, spec: SlotSpec) -> &mut Self {
        self.slots.insert(spec.name, spec);
        self
    }

    /// Whether `slot` is declared.
    #[must_use]
    pub fn contains(&self, slot: &str) -> bool {
        self.slots.contains_key(slot)
    }

    /// The declared spec for `slot`, if declared.
    #[must_use]
    pub fn get(&self, slot: &str) -> Option<&SlotSpec> {
        self.slots.get(slot)
    }

    /// All declared slots, in a stable (sorted-by-name) order.
    pub fn slots(&self) -> impl Iterator<Item = &SlotSpec> {
        self.slots.values()
    }

    /// How many slots are declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether no slots are declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// The shipped slot schema: every paint slot the design system has
/// committed to, whether or not `gorgon-petra-egui`'s painter draws it yet
/// (design doc §4). Three of these — `background`, `border`, `foreground`
/// — are drawn today; the rest (`shadow`, `highlight`, `divider`, the
/// `gradient-stop-1`/`gradient-stop-2` pair, `overlay`) are declared ahead
/// of their painter implementation so a later leaf only has to add drawing
/// code, not grow the schema.
///
/// `gradient-stop-1` and `gradient-stop-2` are mutually required by
/// design (design doc §4: "No (both, or neither)") — binding one without
/// the other is a tree-acceptance error. That pairing is not expressible on
/// a single [`SlotSpec`] and is deliberately **not** encoded here. Neither
/// gradient stop is individually `required`: `SlotSpec::required` means
/// "every node binding this slot must supply it," which is not the
/// gradient pair's rule. **The rule is not enforced anywhere yet** — this
/// doc used to cite a `Violation::IncompleteGradient` that does not exist
/// in `tree::validate`, which is the kind of claim a schema with no
/// consumer accumulates.
///
/// Deliberately **not** declared: a `ring` slot. `focus.ring` and
/// `focus.ring-halo` are token *names* on the focus indicator's own
/// non-schema path, not slot names, and a `ring` slot would be confusable
/// with them.
///
/// `radius` **is** declared, reversing an earlier decision recorded here.
/// The argument was that a corner radius is a property of the rect every
/// slot draws into rather than a colour one slot contributes, so "it
/// belongs on `Props`, not the slot schema". The code went the other way:
/// `radius` is bound through `props.tokens` by eight components and read
/// through `paint::RADIUS_SLOT` by the painter, and `props.tokens` **is**
/// the slot channel. Refusing it here while the whole library binds it
/// left the schema disagreeing with the painter in both directions —
/// `radius` painter-known and schema-refused, `shadow` schema-declared and
/// painter-unknown — for as long as nothing consulted the schema. It is
/// consulted now (`tree::Registry::slots`), so the disagreement had to be
/// resolved rather than restated. `shadow` stays declared: a slot the
/// shipped painter does not draw yet lands in `PaintReport::unknown_slots`,
/// which is that report's job and not tree acceptance's.
#[must_use]
pub fn standard_slots() -> SlotSchema {
    let mut s = SlotSchema::new();
    s.declare(SlotSpec::new("background", TokenKind::Color, false))
        .declare(SlotSpec::new("border", TokenKind::Color, false))
        .declare(SlotSpec::new("foreground", TokenKind::Color, false))
        .declare(SlotSpec::new("shadow", TokenKind::Color, false))
        .declare(SlotSpec::new("highlight", TokenKind::Color, false))
        .declare(SlotSpec::new("divider", TokenKind::Color, false))
        .declare(SlotSpec::new("gradient-stop-1", TokenKind::Color, false))
        .declare(SlotSpec::new("gradient-stop-2", TokenKind::Color, false))
        .declare(SlotSpec::new("overlay", TokenKind::Color, false))
        .declare(SlotSpec::new("radius", TokenKind::Shape, false));
    s
}

#[cfg(test)]
mod tests {
    use super::{SlotSpec, standard_slots};
    use crate::token::value::TokenKind;

    /// The shipped schema declares every slot the painter knows (design doc
    /// §4's full table), not just the three `gorgon-petra-egui` draws today
    /// — landing the whole set now avoids a second migration when the
    /// painter grows to draw the rest.
    #[test]
    fn the_shipped_schema_declares_every_slot_the_painter_knows() {
        let schema = standard_slots();
        let expected = [
            ("background", TokenKind::Color),
            ("border", TokenKind::Color),
            ("foreground", TokenKind::Color),
            ("shadow", TokenKind::Color),
            ("highlight", TokenKind::Color),
            ("divider", TokenKind::Color),
            ("gradient-stop-1", TokenKind::Color),
            ("gradient-stop-2", TokenKind::Color),
            ("overlay", TokenKind::Color),
            ("radius", TokenKind::Shape),
        ];
        assert_eq!(
            schema.len(),
            expected.len(),
            "shipped schema slot count drifted from the declared set"
        );
        for (name, kind) in expected {
            let spec = schema
                .get(name)
                .unwrap_or_else(|| panic!("shipped schema is missing slot `{name}`"));
            assert_eq!(spec.name(), name);
            assert_eq!(spec.kind(), kind, "slot `{name}` has the wrong TokenKind");
            assert!(schema.contains(name));
        }
    }

    /// A slot the schema never declared — including `ring`, which the design
    /// doc explicitly refuses to add — is not accepted: `contains` is false
    /// and `get` is `None`. `radius` is no longer in this list; see
    /// [`standard_slots`] for why that decision reversed.
    #[test]
    fn a_slot_outside_the_schema_is_not_accepted() {
        let schema = standard_slots();
        for outsider in ["ring", "tint", "background-color", ""] {
            assert!(
                !schema.contains(outsider),
                "unschema'd slot `{outsider}` was reported as contained"
            );
            assert!(
                schema.get(outsider).is_none(),
                "unschema'd slot `{outsider}` resolved to a spec"
            );
        }
    }

    #[test]
    fn declaring_a_slot_twice_overwrites_the_earlier_entry() {
        let mut schema = standard_slots();
        assert!(!schema.get("background").unwrap().required());
        schema.declare(SlotSpec::new("background", TokenKind::Color, true));
        assert!(schema.get("background").unwrap().required());
        // Overwriting does not grow the set.
        assert_eq!(schema.len(), 10);
    }

    #[test]
    fn slots_iterates_in_sorted_name_order() {
        let schema = standard_slots();
        let names: Vec<&str> = schema.slots().map(SlotSpec::name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }
}
