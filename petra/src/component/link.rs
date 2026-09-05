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
//! [`link`] shows it on hover as Carbon does, and [`link_inline`] shows it
//! at rest, which is the form to reach for when a link sits in prose and
//! must be found without a pointer.
//!
//! No fill. A link is words on whatever it sits on; a `SURFACE_BASE`
//! background used to be bound so `on_layer` could reseat it, and painted
//! a box of the wrong tone on any card the link was placed in.

use super::text::text;
use super::tokens::{LINK_PRIMARY, t};
use crate::tree::{Interaction, Key, Role, ViewNode};

/// What a link answers to. `Hover` is here because the underline is
/// revealed by it: a node that does not declare `Hover` is never hovered
/// (`input::hit_test`), and `underline@hover` would never resolve.
const LINK_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Carbon's standalone link: [`LINK_PRIMARY`] ink, underlined under the
/// pointer. `label` is required (FR-058).
pub fn link(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    link_built(key, label, "underline@hover")
}

/// Carbon's inline link: the same ink, underlined at rest as well as under
/// the pointer, for a link set in running text.
pub fn link_inline(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    link_built(key, label, "underline")
}

/// One `Text` leaf, not a stack with a caption inside it.
///
/// Hover is a fact about one placement — the hit test names the topmost
/// node under the pointer that declares `Hover`, and `resolve_slot` reads
/// that placement's own flag. A stack wrapping a caption would put the
/// interactions on the wrapper and the `underline@hover` binding on the
/// child, and the child would never be the hovered one. The words and the
/// control are the same node, which is also what a link is.
fn link_built(key: impl Into<Key>, label: impl Into<String>, underline_slot: &str) -> ViewNode {
    let label = label.into();
    let mut node = text(key, label.clone());
    node.props
        .tokens
        .insert("foreground".into(), t(LINK_PRIMARY));
    node.props
        .tokens
        .insert(underline_slot.into(), t(LINK_PRIMARY));
    node.interactive(Role::Button, label, LINK_INTENTS)
}

#[cfg(test)]
mod tests {
    use super::{link, link_inline};
    use crate::component::disabled;
    use crate::component::tokens::{LINK_PRIMARY, SURFACE_BASE, SURFACE_RAISED, TEXT_PRIMARY};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

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
    /// `:hover`, and `.cds--link--inline { text-decoration: underline }`.
    /// The rule is the channel a red-green colour-blind reader has, so
    /// which form shows it when is a contract and not a style.
    #[test]
    fn a_link_underlines_on_hover_and_an_inline_link_underlines_at_rest() {
        let standalone = link("docs", "Open docs");
        assert_eq!(token(&standalone, "underline@hover"), Some(LINK_PRIMARY));
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

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
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
                let parent = &frame.placements[parent_idx];
                let fits = p.rect.x >= parent.rect.x - 0.01
                    && p.rect.y >= parent.rect.y - 0.01
                    && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                    && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                assert!(
                    fits,
                    "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                    p.id, p.rect, parent.id, parent.rect
                );
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
}
