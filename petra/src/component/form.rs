//! `form` — Carbon Form (slice-b): a labelled vertical stack of fields.
//!
//! Carbon documents Form as a spacing/label shell, not a control. There is
//! no `Role::Group`, so this constructor sets no role and no interactions.
//! Legend colour is `$text-secondary` → [`TEXT_MUTED`].
//!
//! # Two gaps, and a `stack` has one
//!
//! Carbon's form has **two** vertical gaps and they are not the same size:
//!
//! - Legend to the first item: the legend is a `.cds--label`, whose
//!   `margin-block-end` is `$spacing-03` (8) — slice-b:227 anatomy 2a.
//! - Item to item: `margin-bottom` `$spacing-07` (32) — slice-b:230.
//!
//! Measured on `ignored/carbon-ref/shots/13-form.png`, that is 9 units
//! between the legend's line box and the label's, and 27 between the
//! field's bottom rule and the checkbox's box. This file shipped one
//! uniform `$spacing-05` (16) for both, under a comment that called 16 "the
//! constructor contract" — a contract with nobody: no spec, no gate and no
//! other test ever named the number, and Carbon is the conformance target
//! (spec 005, open decision 0).
//!
//! A `Stack` carries exactly one gap, so this builds the small one and
//! stands a [`ITEM_FILLER`]-tall [`NodeKind::Spacer`] between consecutive
//! items to make up the large one: `8 + 16 + 8 = 32`.
//!
//! **A nesting level would be tidier and is deliberately not used.** The
//! obvious shape is `legend` beside an `items` stack of its own, which is
//! what Carbon's `<fieldset>` is. It would move every field one segment
//! deeper, and `component/tests.rs:1424` pins
//! `root/carbon2/fm-signup/fm-name` as a **direct** child of the form. That
//! file belongs to another wave this round, so the spacer is the shape that
//! fixes the geometry without reaching into it. Collapsing the two into an
//! `items` stack is a one-line change here plus that one path there, and
//! whoever does it should.
//!
//! # What this file does *not* do
//!
//! It sets no cross-axis `align`. Round 2 recorded row 13's field as hugging
//! its own content; measured on 2026-09-05 it does not, at either level —
//! `a_labelled_form_item_fills_the_form_it_sits_in` holds with the align
//! declared and with it absent, because a `stack` measures an `Input` leaf
//! at the cross extent it was offered. The line was written, falsified, and
//! taken out again rather than left in as a claim nothing tests.

use super::stack;
use super::text::text;
use super::tokens::{SPACING_03, TEXT_MUTED, t};
use crate::geom::Axis;
use crate::tree::{AxisConstraint, Constraints, Key, NodeKind, ViewNode};

/// What stands between two consecutive form items, so that the `Stack`'s
/// own `$spacing-03` on each side of it adds up to Carbon's `$spacing-07`
/// item gap: `32 - 2 x 8`. See the module doc for why a spacer and not a
/// nesting level.
const ITEM_FILLER: f32 = 16.0;

const _: () = assert!(ITEM_FILLER == 32.0 - 2.0 * 8.0);

/// A vertical field group with a muted legend and no role.
///
/// `children` are the fields. The legend is a `text` child keyed `"legend"`.
/// Not interactive: Form does not submit, focus, or click.
pub fn form(key: impl Into<Key>, legend: impl Into<String>, children: Vec<ViewNode>) -> ViewNode {
    let mut legend_node = text("legend", legend);
    legend_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let mut rows = Vec::with_capacity(children.len() * 2 + 1);
    rows.push(legend_node);
    for (i, child) in children.into_iter().enumerate() {
        if i > 0 {
            rows.push(item_filler(i - 1));
        }
        rows.push(child);
    }
    stack(key, Axis::Vertical, Some(SPACING_03), rows)
}

/// The blank between form item `index` and the one after it.
fn item_filler(index: usize) -> ViewNode {
    ViewNode::new(NodeKind::Spacer, format!("gap{index}")).with_constraints(Constraints {
        vertical: AxisConstraint {
            min: Some(ITEM_FILLER),
            max: Some(ITEM_FILLER),
            priority: 0,
        },
        ..Constraints::default()
    })
}

#[cfg(test)]
mod tests {
    use super::form;
    use crate::component::field::{field, field_labeled};
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::{SPACING_03, TEXT_MUTED};

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{NodeKind, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    /// Carbon's two gaps, measured in the frame rather than read off the
    /// props: the legend sits `$spacing-03` (8) above the first item and
    /// consecutive items sit `$spacing-07` (32) apart (slice-b:227, :230).
    /// This asserted one uniform `spacing-05` until 2026-09-05 and cited a
    /// "constructor contract" that no spec or gate ever wrote down.
    #[test]
    fn form_puts_carbons_two_gaps_between_its_legend_and_its_items() {
        let node = form(
            "signup",
            "Account",
            vec![field("name", "Name"), field("email", "Email")],
        );
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(crate::geom::Axis::Vertical));
        assert_eq!(
            node.props.spacing.as_ref().map(|n| n.as_str()),
            Some(SPACING_03)
        );

        let frame = petrify_lone(node);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the frame"))
                .rect
        };
        let legend = rect("/signup/legend");
        let first = rect("/signup/name");
        let second = rect("/signup/email");
        assert_eq!(
            first.y - legend.bottom(),
            8.0,
            "the legend's gap is `$spacing-03`, not the item gap"
        );
        assert_eq!(
            second.y - first.bottom(),
            32.0,
            "consecutive items are `$spacing-07` apart"
        );
    }

    /// Row 13, round 3. The guard for the claim round 2 made and this
    /// round refuted: that this row's field hugs its own content.
    ///
    /// It does not, and it did not — a `stack` measures an `Input` leaf at
    /// the cross extent it was offered, so a labelled item and a bare one
    /// both come out the form's own width. Kept as a regression guard
    /// rather than deleted: the claim will be made again, and this is the
    /// measurement that answers it.
    #[test]
    fn a_labelled_form_item_fills_the_form_it_sits_in() {
        let node = form(
            "signup",
            "Account",
            vec![field_labeled("name", "Name"), field("email", "Email")],
        );
        let frame = petrify_lone(node);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the frame"))
                .rect
        };
        let form_rect = rect("/signup");
        let labelled = rect("/signup/name");
        let bare = rect("/signup/email");
        assert_eq!(
            labelled.w, form_rect.w,
            "the labelled item hugged its own label instead of filling the form"
        );
        assert_eq!(bare.w, form_rect.w, "the bare field stopped filling too");
    }

    #[test]
    fn form_has_no_role_and_is_not_interactive() {
        let node = form("signup", "Account", vec![field("name", "Name")]);
        assert!(node.semantics.role.is_none());
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert!(node.semantics.label.is_none());
    }

    #[test]
    fn legend_is_muted_text() {
        let node = form("signup", "Account", vec![field("name", "Name")]);
        let legend = child(&node, "legend");
        assert_eq!(legend.kind, NodeKind::Text);
        assert_eq!(legend.props.text.as_deref(), Some("Account"));
        assert_eq!(token(legend, "foreground"), Some(TEXT_MUTED));
        let _ = child(&node, "name");
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: the legend and both fields place with a real rect, none
    /// of them outside the form.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = form(
            "signup",
            "Account",
            vec![field("name", "Name"), field("email", "Email")],
        );
        let frame = petrify_lone(node);
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

    /// Check F: `form` declares no interaction of its own, but its field
    /// children keep theirs — the shell does not steal focus reachability
    /// from what it groups.
    #[test]
    fn fields_stay_reachable_inside_a_form() {
        let node = form(
            "signup",
            "Account",
            vec![field("name", "Name"), field("email", "Email")],
        );
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        for suffix in ["/name", "/email"] {
            let p = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the petrified frame"));
            assert!(
                focus.order().iter().any(|o| o == &p.id),
                "{suffix} declares Focus but is not in focus order"
            );
        }
    }

    /// Check E: the legend against the page ground it is read on
    /// (`surface.base`, matching how `text()` itself is styled to sit on
    /// the base layer — `form` sets no fill of its own), read through
    /// `Props.opacity`.
    #[test]
    fn legend_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            let node = form("signup", "Account", vec![field("name", "Name")]);
            let legend = child(&node, "legend");
            let fg_name = legend
                .props
                .tokens
                .get("foreground")
                .expect("legend binds a foreground");
            let opacity = legend.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
            let ratio = fg.contrast_ratio(bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "legend at {ratio:.2}:1 against {SURFACE_BASE} fails AA {MIN_TEXT_CONTRAST}:1"
            );
        }
    }
}
