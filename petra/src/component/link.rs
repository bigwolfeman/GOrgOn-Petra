//! `link` — Carbon Link (slice-c): an activatable text control.
//!
//! There is no `Role::Link`. The activatable control is [`Role::Button`].
//!
//! # Two channels, never one
//!
//! Carbon's link is `$link-primary` ink, and `text-decoration: underline`
//! on hover, focus and active for the **standalone** form and always for
//! the **inline** form (`_link.scss`, slice-c "Link"). Until 2026-09-04
//! this library bound [`TEXT_PRIMARY`] and no underline, because
//! `tokens.rs` re-exported no link ink and the painter had no rule under
//! text. The operator's report on row 15 was "no indication what it is,
//! just looks like a text label", which was exactly right.
//!
//! Now the ink is [`LINK_PRIMARY`] and the rule is the `underline` slot,
//! both in the link's own colour. The underline is not decoration here: the
//! operator is red-green colour blind, and a rule under the words is the
//! channel that carries "this is a link" whether or not the hue does. So
//! [`link`] shows it on hover and while held as Carbon does (focus carries
//! it through [`FocusFigure`], the one focus channel this library draws on
//! every control), and [`link_inline`] shows it
//! at rest, which is the form to reach for when a link sits in prose and
//! must be found without a pointer.
//!
//! No fill. A link is words on whatever it sits on; a `SURFACE_BASE`
//! background used to be bound so `on_layer` could reseat it, and painted
//! a box of the wrong tone on any card the link was placed in.
//!
//! # A link lends out its text
//!
//! It does not declare [`crate::tree::Semantics::owns_its_text`], so its
//! words can be selected and copied like any other prose. The operator, after
//! dragging across a sentence with a link set in the middle of it:
//!
//! > *"links stop copying too"*
//!
//! Carbon's link is an `<a>`, and no browser makes an anchor's text
//! unselectable — `user-select: none` is what browsers apply to *form
//! controls*, and an anchor is not one. A link's words are content that
//! happens to be activatable, which is the same argument
//! [`crate::component::tile`] records for a clickable tile, decided by the
//! operator on the same day.
//!
//! The price is the same too, and it is real: this library activates on the
//! press, so a drag that starts inside a link follows the link as well as
//! selecting it. A browser would not, because a browser fires on the release.
//! Moving the catalog from press to release is the fix and it is not this
//! change (`.agents/notes/implemented/architecture/2026-09-06-selection-is-opt-out-not-opt-in.md`).

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::text::text;
use super::tokens::{LINK_PRIMARY, SPACING_03, t};
use crate::geom::{Align, Axis};
use crate::tree::{Behaviour, FocusFigure, Intent, Interaction, Key, Phase, Role, ViewNode};

/// Spec 010: one shot on release — a link is Activate's definition beside button.
const ACTIVATES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Activate,
    phase: Phase::OnRelease,
};

/// What a link answers to. `Hover` is here because the underline is
/// revealed by it: a node that does not declare `Hover` is never hovered
/// (`input::hit_test`), and `underline@hover` would never resolve.
const LINK_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Carbon's standalone link: [`LINK_PRIMARY`] ink, underlined under the
/// pointer and while it is held. `label` is required (FR-058).
pub fn link(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    link_built(key, label, &["underline@hover", "underline@active"])
}

/// Carbon's inline link: the same ink, underlined at rest as well as under
/// the pointer, for a link set in running text.
pub fn link_inline(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    link_built(key, label, &["underline"])
}

/// Carbon's `--icon` modifier (register row #231): the standalone link with
/// a trailing icon in the link's own ink — `margin-inline-start:
/// $spacing-03` (`_link.scss:125`), 16px glyph.
///
/// # Where the interactions sit, and what the icon costs the underline
///
/// The wrapper is the control: `Role::Button`, the label, Hover/Click/
/// Focus, [`ACTIVATES_ON_RELEASE`] — one node one intent, so a press
/// anywhere on the link activates it exactly once (`fired_behaviour` reads
/// the behaviour off the routed node and walks no ancestors).
///
/// The underline stays on the label's `Text` leaf, because the painter
/// underlines text rows and nothing else (`petra-egui/src/paint/text.rs`):
/// a `underline@hover` binding on the wrapper would hash into the digest
/// and never paint — the silent-placement trap
/// `contracts/interaction-state.md` §6 exists for. The label leaf declares
/// `Hover` of its own so the binding resolves when the pointer rests on the
/// words; the icon is a plain canvas child and carries no state.
///
/// The residual, named rather than papered over: Carbon's
/// `text-decoration` runs under the icon too, and this library's underline
/// is a strip under text rows only — so the icon form underlines its words
/// and not its glyph, and `underline@active` is left off the leaf because
/// the press routes to the wrapper and the leaf can never be the pressed
/// node. [`link`] (the plain form) carries the active underline in full.
pub fn link_with_icon(key: impl Into<Key>, label: impl Into<String>, mark: IconMark) -> ViewNode {
    let key = key.into();
    let label = label.into();
    let mut words = text("label", label.clone());
    words
        .props
        .tokens
        .insert("foreground".into(), t(LINK_PRIMARY));
    words
        .props
        .tokens
        .insert("underline@hover".into(), t(LINK_PRIMARY));
    // The leaf must own Hover for `underline@hover` to ever resolve: the
    // hit test lights the topmost node declaring the interaction, and the
    // wrapper under it being hovered does not reach this placement's flags
    // (`layout::semantics_of`, one id, no propagation). A node that
    // declares any interaction owes the validator a role and a label
    // (`tree::validate::check_semantics`), and the honest pair for the
    // link's words is "static text, labelled by their own content" — not a
    // second `Role::Button` nested inside the wrapper's.
    words = words.interactive(Role::Label, label.clone(), &[Interaction::Hover]);
    // The icon takes the link's ink, not `$icon-primary`: Carbon's `--icon`
    // anatomy says "same color as text" (slice-c).
    let mut glyph = icon_toned("icon", mark, IconTone::Primary);
    glyph
        .props
        .tokens
        .insert("foreground".into(), t(LINK_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![words, glyph]);
    node.props.align = Some(Align::Center);
    node.interactive(Role::Button, label, LINK_INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .with_focus_figure(FocusFigure::Sides)
}

/// One `Text` leaf, not a stack with a caption inside it.
///
/// Hover is a fact about one placement — the hit test names the topmost
/// node under the pointer that declares `Hover`, and `resolve_slot` reads
/// that placement's own flag. A stack wrapping a caption would put the
/// interactions on the wrapper and the `underline@hover` binding on the
/// child, and the child would never be the hovered one. The words and the
/// control are the same node, which is also what a link is.
///
/// [`link_with_icon`] is the one form that cannot be one leaf, and its doc
/// records what the split costs.
fn link_built(key: impl Into<Key>, label: impl Into<String>, underline_slots: &[&str]) -> ViewNode {
    let label = label.into();
    let mut node = text(key, label.clone());
    node.props
        .tokens
        .insert("foreground".into(), t(LINK_PRIMARY));
    for slot in underline_slots {
        node.props.tokens.insert((*slot).into(), t(LINK_PRIMARY));
    }
    // `Sides`, not the default `BarUnder`. `link_inline` paints its own
    // underline at rest and `link` grows one under the pointer, both in the
    // `underline` slot bound just above, so a focus bar two units below the
    // text box would be a second line under the same words told apart only
    // by thickness. That is the operator's *"underlines stick too far off and
    // look bad"* case, and it is also the rule that puts a button on `Sides`:
    // a link declares `Role::Button` because that is what it is.
    node.interactive(Role::Button, label, LINK_INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .with_focus_figure(FocusFigure::Sides)
}

#[cfg(test)]
mod tests {
    use super::{link, link_inline, link_with_icon};
    use crate::component::disabled;
    use crate::component::icon::IconMark;
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::{LINK_PRIMARY, SURFACE_BASE, SURFACE_RAISED, TEXT_PRIMARY};

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{Behaviour, Intent, Interaction, NodeKind, Phase, Role, ViewNode};

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    /// A link is its words: one `Text` leaf that is a labelled button in
    /// link ink, with no fill and no box.
    ///
    /// The ink is asserted **not** to be `text.primary` as well as to be
    /// `link-primary`: the row-15 defect was a link that resolved to the
    /// same tone as the prose beside it, and two names for one tone would
    /// pass the positive half alone.
    #[test]
    fn link_is_a_labelled_button_in_link_ink() {
        let node = link("docs", "Open docs");
        assert_eq!(node.kind, NodeKind::Text, "a link is its words");
        assert_eq!(node.props.text.as_deref(), Some("Open docs"));
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Open docs"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(
            node.interactions.contains(&Interaction::Hover),
            "without Hover the node is never hovered and `underline@hover` \
             never resolves"
        );
        assert!(!node.interactions.contains(&Interaction::Drag));
        assert_eq!(token(&node, "foreground"), Some(LINK_PRIMARY));
        assert_ne!(token(&node, "foreground"), Some(TEXT_PRIMARY));
        assert_eq!(token(&node, "background"), None, "a link has no fill");
        assert_eq!(token(&node, "border"), None);
    }

    /// The standalone link underlines under the pointer and not at rest;
    /// the inline one underlines at rest. Both rules are in link ink.
    ///
    /// Carbon `_link.scss`: `text-decoration: none` at rest, `underline` on
    /// `:hover` **and** `:active` (slice-c: "no underline except on
    /// hover/focus/active"), and `.cds--link--inline { text-decoration:
    /// underline }`.
    /// The rule is the channel a red-green colour-blind reader has, so
    /// which form shows it when is a contract and not a style.
    #[test]
    fn a_link_underlines_on_hover_and_active_and_an_inline_link_underlines_at_rest() {
        let standalone = link("docs", "Open docs");
        assert_eq!(token(&standalone, "underline@hover"), Some(LINK_PRIMARY));
        assert_eq!(
            token(&standalone, "underline@active"),
            Some(LINK_PRIMARY),
            "Carbon's standalone link underlines while it is held — the \
             pressed moment is a register row (#238), not a nicety"
        );
        assert_eq!(
            token(&standalone, "underline"),
            None,
            "Carbon's standalone link is not underlined at rest"
        );
        let inline = link_inline("docs", "Open docs");
        assert_eq!(token(&inline, "underline"), Some(LINK_PRIMARY));
        assert_eq!(token(&inline, "foreground"), Some(LINK_PRIMARY));
        assert_eq!(inline.semantics.role, Some(Role::Button));
        assert!(inline.interactions.contains(&Interaction::Click));
    }

    /// `link_with_icon` (register row #231): one control on the wrapper —
    /// role, label, the three intents, one shot on release — with the
    /// words' underline on the label leaf and the glyph in the link's ink.
    ///
    /// Falsified by moving the underline binding to the wrapper (the
    /// painter underlines text rows only, so it would hash and never
    /// paint), or by leaving `Hover` off the label leaf (nothing would
    /// ever resolve `underline@hover` there).
    #[test]
    fn link_with_icon_puts_the_control_on_the_wrapper_and_the_underline_on_the_words() {
        let node = link_with_icon("docs", "Open docs", IconMark::ChevronRight);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Open docs"));
        assert_eq!(
            node.behaviour,
            Some(Behaviour {
                intent: Intent::Activate,
                phase: Phase::OnRelease,
            })
        );
        for intent in [Interaction::Focus, Interaction::Click, Interaction::Hover] {
            assert!(
                node.interactions.contains(&intent),
                "wrapper misses {intent:?}"
            );
        }
        fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
            node.children
                .iter()
                .find(|c| c.key.as_str() == key)
                .unwrap_or_else(|| panic!("missing child {key}"))
        }
        let words = named(&node, "label");
        assert_eq!(words.props.text.as_deref(), Some("Open docs"));
        assert_eq!(token(words, "foreground"), Some(LINK_PRIMARY));
        assert_eq!(token(words, "underline@hover"), Some(LINK_PRIMARY));
        assert_eq!(
            words.semantics.role,
            Some(Role::Label),
            "the words are static text that accepts hover, not a second \
             button nested in the wrapper's"
        );
        assert!(
            words.interactions.contains(&Interaction::Hover),
            "without Hover on the leaf, `underline@hover` there can never \
             resolve — the wrapper's hover is a different placement"
        );
        assert!(
            !words.interactions.contains(&Interaction::Click),
            "the wrapper is the one press target; a second Click on the leaf \
             would split one control's activation across two nodes"
        );
        let glyph = named(&node, "icon");
        assert_eq!(
            glyph.props.tokens.get("foreground").map(|t| t.as_str()),
            Some(LINK_PRIMARY),
            "Carbon's --icon anatomy: the glyph is the text's colour"
        );
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: the link places with a real rect, none of its parts
    /// outside it.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(link("docs", "Open docs"));
        assert!(!frame.placements.is_empty(), "nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{} drew content larger than its own rect",
                p.id
            );
            if let Some(parent_idx) = p.parent {
                assert_fits_parent("", p, &frame.placements[parent_idx]);
            }
        }
    }

    /// Check F: an enabled link is reachable; a disabled one is not.
    #[test]
    fn a_link_is_reachable_unless_disabled() {
        let enabled = petrify_lone(link("docs", "Open docs"));
        let focus = crate::focus::FocusTree::from_placements(
            &enabled.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let node = enabled
            .placements
            .iter()
            .find(|p| p.id.ends_with("/docs"))
            .expect("the link is placed");
        assert!(
            order.iter().any(|o| o == &node.id),
            "the link declares Focus but is not in focus order"
        );

        let disabled_frame = petrify_lone(disabled(link("docs", "Open docs")));
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_order = disabled_focus.order();
        let disabled_node = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/docs"))
            .expect("the disabled link is placed");
        assert!(
            !disabled_order.iter().any(|o| o == &disabled_node.id),
            "a disabled link must not be reachable"
        );
    }

    /// Check E: the link's ink against the grounds a link is placed on, in
    /// both themes. A link has no fill of its own, so it is judged on the
    /// page and on a card. (`token::shipped` sweeps every layer; this is the
    /// component's own claim about the two it is put on in the catalog.)
    #[test]
    fn link_ink_clears_aa_contrast_on_the_page_and_on_a_card() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = link("docs", "Open docs");
            let fg_name = node
                .props
                .tokens
                .get("foreground")
                .expect("link binds a foreground");
            let opacity = node.props.opacity.unwrap_or(1.0);
            for ground in [SURFACE_BASE, SURFACE_RAISED] {
                let bg = color(&theme, ground);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "link ink {} at {ratio:.2}:1 on {ground} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }

    /// Spec 010: a link declares Activate / OnRelease (breadcrumb items
    /// compose this constructor).
    ///
    /// Falsified by dropping `.with_behaviour(...)` from [`link_built`]:
    ///
    /// ```text
    /// link declared behaviour None
    /// ```
    #[test]
    fn link_declares_activate_on_release() {
        let node = link("docs", "Open docs");
        assert_eq!(
            node.behaviour,
            Some(Behaviour {
                intent: Intent::Activate,
                phase: Phase::OnRelease,
            }),
            "link declared behaviour {:?}",
            node.behaviour
        );
    }
}
