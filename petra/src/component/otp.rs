//! `otp` — a one-time code, one well per digit (spec 009 T041).
//!
//! Anatomy: a horizontal row of `length` wells. Each well is a one-character
//! [`NodeKind::Input`] finished with [`bind_field_chrome`]: fill +
//! border-bottom, square, not a four-sided box. The group itself has no fill,
//! no shadow, and no extra box. MynaUI's slots are bordered boxes; this
//! constructor refuses that, because the rest of the form family already
//! settled on field chrome.
//!
//! Digits are [`Props`]: `value.chars()`, one char per well, padded with
//! empty wells when `value` is short and truncated when it is long. The form
//! fiber owns the string. This file does not filter to ASCII digits.
//!
//! Focus advance on entry is tier 2 (engine). This constructor does not move
//! focus, does not set `takes_focus`, and does not set `focus_run`. Each well
//! is its own [`Role::TextInput`] so the engine has a target per cell.
//!
//! # `length == 0`
//!
//! Still a labelled stack: label [`LABEL`], no wells, no panic. A count of
//! zero is an empty data source, not a programming error worth taking the
//! frame down for.

use super::field::{EDITABLE_TEXT_INTENTS, bind_field_chrome};
use super::stack;
use super::tokens::{SIZE_MD, SPACING_03, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY, t};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, FocusFigure, FocusShownOn, Key, NodeKind, Props, Role, ViewNode,
};

/// Accessible name of the group and of every well.
pub const LABEL: &str = "One-time code";

const _: () = assert!(SIZE_MD == 40.0);

/// A row of `length` one-character wells showing `value`.
///
/// Keys per cell are `"d0"` .. `"d{length-1}"`. Extra characters past
/// `length` are dropped. `length == 0` is a labelled empty stack.
#[must_use]
pub fn otp(key: impl Into<Key>, length: u32, value: &str) -> ViewNode {
    let digits: Vec<char> = value.chars().take(length as usize).collect();
    let shown: String = digits.iter().copied().collect();
    let mut wells = Vec::with_capacity(length as usize);
    for i in 0..length {
        wells.push(digit_well(format!("d{i}"), digits.get(i as usize).copied()));
    }
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), wells);
    node.props.align = Some(Align::Center);
    node.semantics.label = Some(LABEL.to_owned());
    if !shown.is_empty() {
        node.semantics.value = Some(shown);
    }
    node
}

/// One well: a square [`SIZE_MD`] Input with field chrome and at most one
/// character of text.
fn digit_well(key: impl Into<Key>, digit: Option<char>) -> ViewNode {
    let mut props = Props {
        text: digit.map(|c| c.to_string()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props.tokens.insert(
        "foreground".into(),
        t(if digit.is_some() {
            TEXT_PRIMARY
        } else {
            TEXT_MUTED
        }),
    );
    bind_field_chrome(&mut props);
    let mut node = ViewNode::new(NodeKind::Input, key)
        .with_props(props)
        .interactive(Role::TextInput, LABEL, EDITABLE_TEXT_INTENTS)
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(SIZE_MD),
                max: Some(SIZE_MD),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(SIZE_MD),
                max: Some(SIZE_MD),
                priority: 0,
            },
        });
    if let Some(c) = digit {
        node.semantics.value = Some(c.to_string());
    }
    // Each well is its own hull. Focus advance between wells is the
    // engine's; this leaf only declares the figure a field already wears.
    node.semantics.focus_figure = FocusFigure::Sides;
    node.semantics.focus_shown_on = FocusShownOn::Well;
    node
}

#[cfg(test)]
mod tests {
    use super::{EDITABLE_TEXT_INTENTS, LABEL, SIZE_MD, digit_well, otp};
    use crate::component::tokens::{BORDER_STRONG, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY};
    use crate::geom::Axis;
    use crate::tree::{FocusFigure, FocusShownOn, Interaction, NodeKind, Role, ViewNode};

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

    fn well_at<'a>(node: &'a ViewNode, i: u32) -> &'a ViewNode {
        child(node, &format!("d{i}"))
    }

    /// Carbon's field is a fill with a bottom rule: `$field` under
    /// `border-block-end: 1px solid $border-strong`, square, and **no** edge
    /// on the other three sides.
    fn assert_field_chrome(node: &ViewNode, label: &str) {
        assert_eq!(
            token(node, "background"),
            Some(SURFACE_RAISED),
            "{label}: the well is filled"
        );
        assert_eq!(
            token(node, "border-bottom"),
            Some(BORDER_STRONG),
            "{label}: the well's one edge is a strong bottom rule"
        );
        assert_eq!(
            token(node, "border"),
            None,
            "{label}: a field well is not boxed"
        );
        assert_eq!(
            token(node, "radius"),
            None,
            "{label}: a field well has square corners"
        );
        for side in ["border-top", "border-left", "border-right"] {
            assert_eq!(token(node, side), None, "{label}: no {side}");
        }
    }

    #[test]
    fn length_is_the_well_count_and_keys_are_d0_through_dn() {
        let node = otp("code", 6, "");
        assert_eq!(node.children.len(), 6);
        for i in 0..6 {
            let well = well_at(&node, i);
            assert_eq!(well.key.as_str(), format!("d{i}"));
            assert_eq!(well.kind, NodeKind::Input);
        }
    }

    #[test]
    fn value_chars_fill_wells_and_pad_the_rest() {
        let node = otp("code", 6, "12");
        assert_eq!(well_at(&node, 0).props.text.as_deref(), Some("1"));
        assert_eq!(well_at(&node, 1).props.text.as_deref(), Some("2"));
        for i in 2..6 {
            assert_eq!(
                well_at(&node, i).props.text,
                None,
                "unfilled well d{i} stays empty"
            );
        }
        assert_eq!(node.semantics.value.as_deref(), Some("12"));
    }

    #[test]
    fn extra_chars_past_length_are_dropped() {
        let node = otp("code", 4, "1234567");
        assert_eq!(node.children.len(), 4);
        assert_eq!(well_at(&node, 0).props.text.as_deref(), Some("1"));
        assert_eq!(well_at(&node, 3).props.text.as_deref(), Some("4"));
        assert_eq!(node.semantics.value.as_deref(), Some("1234"));
        assert!(
            node.children
                .iter()
                .all(|c| c.key.as_str() != "d4" && c.key.as_str() != "d6"),
            "a longer value must not mint extra wells"
        );
    }

    #[test]
    fn each_unicode_char_occupies_one_well() {
        let node = otp("code", 3, "1é🦀");
        assert_eq!(well_at(&node, 0).props.text.as_deref(), Some("1"));
        assert_eq!(well_at(&node, 1).props.text.as_deref(), Some("é"));
        assert_eq!(well_at(&node, 2).props.text.as_deref(), Some("🦀"));
        assert_eq!(node.semantics.value.as_deref(), Some("1é🦀"));
    }

    #[test]
    fn length_zero_is_a_labelled_empty_stack() {
        let node = otp("code", 0, "999");
        assert_eq!(node.kind, NodeKind::Stack);
        assert!(node.children.is_empty(), "a count of zero draws no wells");
        assert_eq!(node.semantics.label.as_deref(), Some(LABEL));
        assert_eq!(node.semantics.label.as_deref(), Some("One-time code"));
        assert!(node.semantics.role.is_none());
        assert!(!node.is_interactive());
        assert!(node.semantics.value.is_none());
        assert_eq!(token(&node, "border"), None, "the empty group is not boxed");
    }

    #[test]
    fn group_has_no_four_sided_box() {
        let node = otp("code", 4, "12");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert_eq!(node.semantics.label.as_deref(), Some(LABEL));
        assert!(
            node.semantics.role.is_none(),
            "the group must not steal TextInput from the wells"
        );
        assert_eq!(token(&node, "background"), None);
        assert_eq!(token(&node, "border"), None);
        assert_eq!(token(&node, "border-bottom"), None);
        assert_eq!(token(&node, "radius"), None);
        for side in ["border-top", "border-left", "border-right"] {
            assert_eq!(token(&node, side), None, "group has no {side}");
        }
    }

    #[test]
    fn each_well_is_a_square_text_input_with_field_chrome() {
        let node = otp("code", 4, "7");
        for i in 0..4 {
            let well = well_at(&node, i);
            assert_eq!(well.kind, NodeKind::Input);
            assert_eq!(well.semantics.role, Some(Role::TextInput));
            assert_eq!(well.semantics.label.as_deref(), Some(LABEL));
            assert_eq!(well.interactions.as_slice(), EDITABLE_TEXT_INTENTS);
            assert!(well.interactions.contains(&Interaction::Focus));
            assert!(well.interactions.contains(&Interaction::Click));
            assert!(well.interactions.contains(&Interaction::Key));
            assert!(well.interactions.contains(&Interaction::TextEdit));
            assert!(!well.semantics.read_only);
            assert!(!well.semantics.disabled);
            assert_eq!(well.constraints.horizontal.min, Some(SIZE_MD));
            assert_eq!(well.constraints.horizontal.max, Some(SIZE_MD));
            assert_eq!(well.constraints.vertical.min, Some(SIZE_MD));
            assert_eq!(well.constraints.vertical.max, Some(SIZE_MD));
            assert_eq!(well.semantics.focus_figure, FocusFigure::Sides);
            assert_eq!(well.semantics.focus_shown_on, FocusShownOn::Well);
            assert_field_chrome(well, &format!("d{i}"));
        }
        assert_eq!(
            token(well_at(&node, 0), "foreground"),
            Some(TEXT_PRIMARY),
            "a filled well is content ink"
        );
        assert_eq!(
            token(well_at(&node, 1), "foreground"),
            Some(TEXT_MUTED),
            "an empty well is hint ink"
        );
    }

    #[test]
    fn constructor_does_not_move_focus() {
        let node = otp("code", 3, "12");
        assert_ne!(node.props.takes_focus, Some(true));
        assert!(!node.semantics.focus_run);
        for i in 0..3 {
            let well = well_at(&node, i);
            assert_ne!(well.props.takes_focus, Some(true));
            assert!(!well.semantics.focus_run);
            assert!(!well.semantics.selected);
        }
    }

    #[test]
    fn digit_well_is_one_character() {
        let filled = digit_well("d0", Some('9'));
        assert_eq!(filled.props.text.as_deref(), Some("9"));
        assert_eq!(filled.semantics.value.as_deref(), Some("9"));
        let empty = digit_well("d1", None);
        assert_eq!(empty.props.text, None);
        assert!(empty.semantics.value.is_none());
        assert_eq!(empty.semantics.label.as_deref(), Some(LABEL));
    }
}
