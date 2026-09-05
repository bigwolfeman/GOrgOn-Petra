//! The declared set of paint slots a node's `Props.tokens` may bind, and
//! what kind of token each slot accepts.
//!
//! Sibling of [`super::vocabulary::Vocabulary`] (token *names* the theme
//! must define) rather than a replacement for it: a `SlotSchema` entry says
//! "the `shadow` slot wants a `Color` token"; `Vocabulary` says "the theme
//! must define `shadow.ambient` as a `Color`". A binding is only sound when
//! both hold.
//!
//! This module declares the schema; `tree::Registry::slots` consults it at
//! acceptance, and `gorgon-petra-egui`'s painter draws every slot in it.
//!
//! That was not true when the module was written. It landed eleven slots
//! ahead of both consumers, on the argument that growing the set later would
//! cost a second migration — and the five that no painter ever came for were
//! retired on 2026-08-25 (FR-025). See [`standard_slots`] for what each one
//! turned out to be, and
//! `.agents/research/08-22-2026/Petra-Visual-Design/engine/token-slot-schema.md`
//! §3-§4 for the original argument.
//!
//! The lesson the retirement leaves behind is not "never land ahead of a
//! consumer": it is that a slot is a claim about *how a node's rect is
//! painted*, and four of the five failed on that rather than on timing. A
//! state is a different token in the same slot; a row separator is one edge
//! of a box; a scrim is a node. Only the gradient pair was simply unwanted.

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

    /// Whether `slot` is declared. A state-decorated key resolves to its base
    /// slot — see [`SlotSchema::get`].
    #[must_use]
    pub fn contains(&self, slot: &str) -> bool {
        self.get(slot).is_some()
    }

    /// The declared spec for `slot`, if declared.
    ///
    /// A **state-decorated key resolves to its base slot's spec**:
    /// `background@hover` answers with `background`'s entry. A state is a
    /// different token in the same slot (`crate::token::state`), so it wants
    /// the same [`TokenKind`] — and reading the base is what makes tree
    /// acceptance check a state binding at all. Without it every decorated
    /// key would fall into the "slot this schema never heard of" branch and
    /// be checked for existence only, so `background@hover` could bind a
    /// `Spacing` token and paint no colour, which is precisely the defect
    /// `tree::validate`'s slot check was added to close.
    #[must_use]
    pub fn get(&self, slot: &str) -> Option<&SlotSpec> {
        self.slots.get(crate::token::state::base_slot(slot))
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
/// committed to. Thirteen entries, and `gorgon-petra-egui`'s painter draws
/// all thirteen — `standard_slots()` and that painter's `KNOWN_SLOTS` are the
/// same set, which `the_shipped_schema_is_exactly_what_the_painter_draws`
/// holds them to. Six date from 2026-08-25; the four edge slots
/// (`border-top` … `border-left`) from 2026-09-04, see below; the two
/// selection slots from 2026-09-05, see below that.
///
/// # Five slots were retired on 2026-08-25 (FR-025)
///
/// This schema declared eleven and the painter read six. The five unread
/// ones — `highlight`, `divider`, `overlay`, `gradient-stop-1` and
/// `gradient-stop-2` — were landed ahead of a painter on the argument that
/// growing the schema later would be a second migration. Five painters later
/// arrived for none of them, and the Carbon component inventory
/// (`.agents/research/08-25-2026/Carbon-Component-Inventory/`) explains why:
/// four of the five were the wrong *shape* for the need they were declared
/// against, and the fifth had no need at all.
///
/// | retired slot | what the need turned out to be |
/// |---|---|
/// | `highlight` | Real (`$layer-hover` × 10, `$layer-selected` × 2, `$layer-active`) but it is a **state-keyed `background` binding**, not a second colour composited into one rect. Carbon never composites a state; it swaps the token bound to the same property. The tokens are in `token::shipped`; the slot was never the answer. |
/// | `divider` | Contained list, Structured list, Data table and Menu all separate rows with `border-bottom: 1px solid $border-subtle` — one *edge* of a box. The real gap is edge-selective borders, which is its own requirement and not a slot. |
/// | `overlay` | Real (`$overlay` × 6, the modal scrim), but a scrim is a **node** covering the viewport, painted through `background` like any other fill. Replaced by the `overlay.scrim` token. |
/// | `gradient-stop-1`/`-2` | One consumer across 42 components — the `ai-*` aura, which also needs a blur primitive M-Carbon declined. No gradient primitive exists in the painter or the frame. |
///
/// **FR-025 is satisfied by deletion, not by five new painter features.**
/// Reimplementing any of the five would be the defect M-Carbon counted at 12
/// of 18 names, aimed at the slot channel instead of the token channel.
///
/// The gradient pair's mutual-requirement rule went with them. It was
/// recorded here as *"not enforced anywhere yet"*, having previously cited a
/// `Violation::IncompleteGradient` that `tree::validate` does not define —
/// which is the kind of claim a schema with no consumer accumulates, and the
/// reason this doc now describes only what is declared.
///
/// Deliberately **not** declared: a `ring` slot. `focus.ring` and
/// `focus.ring-halo` are token *names* on the focus indicator's own
/// non-schema path, not slot names, and a `ring` slot would be confusable
/// with them.
///
/// `silhouette` is declared for the same reason `radius` is, and it is the
/// slot that makes `radius` finite: a corner radius spans a square to a
/// circle and no further, so a status marker whose only non-colour channel
/// was `radius` had exactly two silhouettes to distinguish four
/// `StatusShape` variants with — and, at the 4-versus-999 pair the library
/// actually bound, the two differed by 0.414 logical units, under one
/// device pixel. `silhouette` names the figure and `radius` rounds its
/// corners; the pair is what FR-015's shape channel needs to reach the
/// screen.
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
/// Deliberately **not** declared: a `coverage` slot for
/// `text.coverage-curve` (`TokenKind::Coverage`,
/// `ignored/builds/2026-08-24-text-pipeline-port/SPEC.md` §6.3).
///
/// Every slot this schema declares answers "what does *this node's rect*
/// paint with" — `background`'s colour, `radius`'s corner, `silhouette`'s
/// figure are all properties of one node's drawn shape, bound per node
/// through `props.tokens`. The coverage curve answers a different question:
/// how the text-rendering *atlas* composites coverage before any node's
/// glyphs are drawn from it. `gorgon-petra-egui`'s `bind_glyph_coverage`
/// reads it once per theme revision, at `Host` scope, the same way it reads
/// mode to pick a `FontColorTransferFunction` — not once per node the way
/// `paint::RADIUS_SLOT` reads `radius`. A `SlotSpec` here would promise a
/// per-node override the renderer has no mechanism to keep: two sibling
/// `Text` nodes cannot each choose their own atlas, because there is one
/// atlas. Declaring the slot anyway would be the same defect
/// `shipped.rs:46-51` names for an unread token, aimed the other way — a
/// schema entry nothing in the render path could honour per node, rather
/// than a vocabulary entry nothing reads at all.
///
/// # The four edge slots (2026-09-04)
///
/// `border-top`, `border-right`, `border-bottom` and `border-left` each
/// paint **one** edge of the node's rect in the bound colour, at the same
/// one-unit device-snapped width `border` uses, and nothing else. They are
/// the shape the retired `divider` slot's table above says the real need
/// was: Carbon separates data-table rows with `border-block-end`, structured
/// list rows with `border-block-start`, and pagination's nav buttons with
/// `border-inline-start` (slice-b:76, slice-e:98, slice-d:58). A four-sided
/// `border` on each of two adjacent rows drew every seam twice and every
/// column edge once, which is why the two tables read as a grid of boxes
/// under a green suite (`.agents/carbon-waves/ROUND2-DEFECTS.md` rows 9
/// and 31). A real 1-unit element per edge was the workaround three
/// components used first; it works for a bar's top rule and costs a node,
/// a key, and a constraint each time, and it cannot be state-decorated the
/// way a slot can (`border-bottom@selected`).
///
/// They are independent slots and not one `Edge` enum so a three-sided
/// case (Carbon's active header action, slice-f:150) binds three of them.
/// Adding them cost no `Props` field, no `PaintContent` field and no digest
/// bump: `props.tokens` is already a map keyed by slot name, and
/// `frame::digest::hash_paint_content` hashes every entry it holds.
///
/// # `underline`
///
/// The colour of a rule drawn under each row of this node's text, or absent
/// for none. Carbon's Link is `text-decoration: underline` on hover and, for
/// the inline variant, at rest (slice-c "Link"). For an operator who is
/// red-green colour blind the underline is not decoration: it is the channel
/// that says "this is a link" when the hue cannot. A slot, for the same
/// reasons the side borders are slots, and because `underline@hover` is
/// exactly Carbon's standalone link with nothing further to build.
/// # The two selection slots (2026-09-05)
///
/// `selection` is the ground filled **behind the stretch of this node's text
/// the operator has selected**, and `selection-ink` is the colour those
/// glyphs are then drawn in. The range comes from the frame
/// (`frame::PaintContent::selection`), which the host writes from a real
/// press-and-drag; these two say what it looks like.
///
/// **This is not `highlight` coming back.** The table above retires
/// `highlight` because the need behind it was a *state-keyed `background`*:
/// Carbon never composites a state, it swaps the token bound to the same
/// property, and `background@selected` already does that. A text selection
/// cannot be expressed that way and the difference is not a matter of
/// taste — `background` fills the node's **whole rect**, and a selection
/// covers a sub-range of one string, in as many rectangles as it crosses
/// rows. No state-decorated key can say "these seventeen bytes". That is why
/// this is a slot and `highlight` still is not.
///
/// **Two slots and not one**, and this is measured rather than symmetric: a
/// ground dark enough to be seen moves the ground the ink was measured
/// against. The code snippet's keyword class (`link-primary`) clears AA on
/// its own well in the light theme at 4.63:1 against a 4.5 floor, so any
/// visible selection ground drops it below AA — and a ground pale enough to
/// keep it is one nobody can see. The selected run therefore takes its own
/// ink, exactly as `::selection { color }` does in a browser. A node binding
/// one and not the other gets no highlight; they are resolved as a pair.
#[must_use]
pub fn standard_slots() -> SlotSchema {
    let mut s = SlotSchema::new();
    s.declare(SlotSpec::new("background", TokenKind::Color, false))
        .declare(SlotSpec::new("border", TokenKind::Color, false))
        .declare(SlotSpec::new("border-top", TokenKind::Color, false))
        .declare(SlotSpec::new("border-right", TokenKind::Color, false))
        .declare(SlotSpec::new("border-bottom", TokenKind::Color, false))
        .declare(SlotSpec::new("border-left", TokenKind::Color, false))
        .declare(SlotSpec::new("foreground", TokenKind::Color, false))
        .declare(SlotSpec::new("underline", TokenKind::Color, false))
        .declare(SlotSpec::new("shadow", TokenKind::Color, false))
        .declare(SlotSpec::new("radius", TokenKind::Shape, false))
        .declare(SlotSpec::new("silhouette", TokenKind::Silhouette, false))
        .declare(SlotSpec::new("selection", TokenKind::Color, false))
        .declare(SlotSpec::new("selection-ink", TokenKind::Color, false));
    s
}

#[cfg(test)]
mod tests {
    use super::{SlotSpec, standard_slots};
    use crate::token::value::TokenKind;

    /// The shipped schema is exactly the eleven slots the painter draws, at
    /// the kinds it draws them.
    ///
    /// The count is the load-bearing assertion, not the membership list. This
    /// schema once carried eleven entries against a six-entry painter for as
    /// long as nothing compared the two, and every one of the five extras
    /// read as a considered commitment rather than as a gap. A test that only
    /// checked membership would have passed the whole time.
    ///
    /// The other half of the claim — that the painter's own `KNOWN_SLOTS`
    /// is this same set — cannot be made from this crate, which does not
    /// depend on `gorgon-petra-egui`. It is made from that side, in
    /// `paint::tests::the_painter_draws_every_slot_the_schema_declares`.
    #[test]
    fn the_shipped_schema_is_exactly_what_the_painter_draws() {
        let schema = standard_slots();
        let expected = [
            ("background", TokenKind::Color),
            ("border", TokenKind::Color),
            ("border-top", TokenKind::Color),
            ("border-right", TokenKind::Color),
            ("border-bottom", TokenKind::Color),
            ("border-left", TokenKind::Color),
            ("foreground", TokenKind::Color),
            ("underline", TokenKind::Color),
            ("shadow", TokenKind::Color),
            ("radius", TokenKind::Shape),
            ("silhouette", TokenKind::Silhouette),
            ("selection", TokenKind::Color),
            ("selection-ink", TokenKind::Color),
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

    /// The five slots retired on 2026-08-25 stay retired.
    ///
    /// A separate test from the one above, because the two fail for different
    /// reasons and a reader should be able to tell them apart from the name
    /// alone: that one catches a slot count drifting, this one catches a
    /// retired slot being reinstated because somebody read the old design doc
    /// and not [`standard_slots`]'s table. FR-025 is satisfied by deletion,
    /// and the deletion is what this holds.
    #[test]
    fn the_five_retired_slots_stay_retired() {
        let schema = standard_slots();
        for retired in [
            "highlight",
            "divider",
            "overlay",
            "gradient-stop-1",
            "gradient-stop-2",
        ] {
            assert!(
                !schema.contains(retired),
                "`{retired}` was retired by FR-025; a state is a token swap on \
                 `background`, a row separator is one edge of a box, a scrim is \
                 a node painted through `background`, and no gradient primitive \
                 exists. See `standard_slots` before adding it back."
            );
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

    /// A state-decorated key is the base slot, at the base slot's kind.
    ///
    /// The negative half is the load-bearing one: decorating a slot the
    /// schema never declared must not conjure an entry, or the decoration
    /// would become a way to smuggle `ring@hover` past a schema that refuses
    /// `ring`.
    #[test]
    fn a_state_decorated_key_resolves_to_its_base_slot() {
        let schema = standard_slots();
        for decorated in [
            "background@hover",
            "background@active",
            "background@selected-hover",
            "radius@disabled",
        ] {
            let spec = schema
                .get(decorated)
                .unwrap_or_else(|| panic!("`{decorated}` must resolve to its base slot"));
            assert_eq!(spec.name(), super::super::state::base_slot(decorated));
            assert!(schema.contains(decorated));
        }
        assert_eq!(
            schema.get("background@hover").map(SlotSpec::kind),
            schema.get("background").map(SlotSpec::kind),
            "a state is a different token in the same slot, so it is the same kind"
        );
        for outsider in ["ring@hover", "highlight@selected", "@hover"] {
            assert!(
                !schema.contains(outsider),
                "decorating an undeclared slot must not declare it: {outsider}"
            );
        }
    }

    #[test]
    fn declaring_a_slot_twice_overwrites_the_earlier_entry() {
        let mut schema = standard_slots();
        let before = schema.len();
        assert!(!schema.get("background").unwrap().required());
        schema.declare(SlotSpec::new("background", TokenKind::Color, true));
        assert!(schema.get("background").unwrap().required());
        // Overwriting does not grow the set. Measured against the schema's own
        // length rather than against a literal, so this keeps testing
        // overwriting rather than turning into a second slot count that has to
        // be maintained beside the first.
        assert_eq!(schema.len(), before);
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
