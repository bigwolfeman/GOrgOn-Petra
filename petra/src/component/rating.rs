//! `rating` — a discrete score of `value` marks out of `max`.
//!
//! Spec 009 T031 / D-092. Not a Carbon inventory row. Glyph and count are
//! both parameters: five of any one mark is a call, not a law. [`IconMark`]
//! has no `Star`; that is a named gap for the default call. Any shipped
//! mark stands in until the gap closes.
//!
//! Anatomy:
//! 1. `"marks"` — `max` copies of `mark`. The first `value` copies use
//!    [`IconTone::Accent`]; the rest use [`IconTone::Secondary`], the
//!    furniture tone. Same glyph, two tones, plus the caption, so the
//!    score never rides on hue alone.
//! 2. `"value"` — visible text `"{value} of {max}"`. The operator cannot
//!    use hue as the only channel; the number is the second one.
//!
//! `Role` has no Meter. [`Role::Progress`] is the bar that fills a job.
//! This is a score, so the node is [`Role::Status`] with the caption as
//! both label and [`Semantics::value`].
//!
//! No score bands. A low score and a high score use the same pair of tones.
//! Hue does not swap with the ratio.
//!
//! # `max == 0`
//!
//! Still a labelled node: caption `"0 of 0"`, no marks, no panic. A count
//! of zero is an empty data source, not a programming error worth taking
//! the frame down for. `value` above `max` clamps to `max` so the drawn
//! count and the caption cannot disagree.

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::text::text;
use super::tokens::{SPACING_02, SPACING_03};
use crate::geom::{Align, Axis};
use crate::tree::{Key, Role, Semantics, ViewNode};

/// Visible and semantic caption. One format, so the two cannot drift.
fn readout(value: u32, max: u32) -> String {
    format!("{value} of {max}")
}

/// A read-only score: `max` marks, the first `value` filled, and a numeric
/// caption so the marks are never the only channel.
///
/// `max` is the mark count, not a constant five. `value` clamps to `max`.
/// `max == 0` yields a labelled node with no marks (see the module doc).
/// `mark` is any [`IconMark`]; there is no `Star` yet (D-092 named gap).
#[must_use]
pub fn rating(key: impl Into<Key>, value: u32, max: u32, mark: IconMark) -> ViewNode {
    let shown = value.min(max);
    let caption = readout(shown, max);

    let mut children = Vec::with_capacity(if max == 0 { 1 } else { 2 });
    if max > 0 {
        let mut marks = Vec::with_capacity(max as usize);
        for i in 0..max {
            let tone = if i < shown {
                IconTone::Accent
            } else {
                IconTone::Secondary
            };
            marks.push(icon_toned(format!("mark-{i}"), mark, tone));
        }
        let mut row = stack("marks", Axis::Horizontal, Some(SPACING_02), marks);
        row.props.align = Some(Align::Center);
        children.push(row);
    }
    children.push(text("value", caption.clone()));

    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), children);
    node.props.align = Some(Align::Center);
    node.semantics = Semantics {
        role: Some(Role::Status),
        label: Some(caption.clone()),
        value: Some(caption),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::{IconMark, IconTone, icon_toned, rating, readout};
    use crate::tree::{Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn mark_at<'a>(node: &'a ViewNode, i: u32) -> &'a ViewNode {
        child(child(node, "marks"), &format!("mark-{i}"))
    }

    #[test]
    fn max_is_the_mark_count() {
        let five = rating("r", 3, 5, IconMark::CheckmarkFilled);
        assert_eq!(child(&five, "marks").children.len(), 5);
        let ten = rating("r", 3, 10, IconMark::CheckmarkFilled);
        assert_eq!(child(&ten, "marks").children.len(), 10);
        assert_eq!(ten.semantics.value.as_deref(), Some("3 of 10"));
    }

    #[test]
    fn value_clamps_to_max() {
        let node = rating("r", 9, 5, IconMark::Check);
        assert_eq!(node.semantics.value.as_deref(), Some("5 of 5"));
        assert_eq!(child(&node, "value").props.text.as_deref(), Some("5 of 5"));
        let marks = child(&node, "marks");
        assert_eq!(marks.children.len(), 5);
        for i in 0..5 {
            assert_eq!(
                mark_at(&node, i).props.canvas,
                icon_toned(format!("mark-{i}"), IconMark::Check, IconTone::Accent)
                    .props
                    .canvas,
                "clamped fill is every mark, not a band"
            );
        }
    }

    #[test]
    fn max_zero_is_labelled_and_has_no_marks() {
        let node = rating("r", 4, 0, IconMark::Check);
        assert!(
            node.children.iter().all(|c| c.key.as_str() != "marks"),
            "a count of zero draws no marks"
        );
        assert_eq!(child(&node, "value").props.text.as_deref(), Some("0 of 0"));
        assert_eq!(node.semantics.role, Some(Role::Status));
        assert_eq!(node.semantics.label.as_deref(), Some("0 of 0"));
        assert_eq!(node.semantics.value.as_deref(), Some("0 of 0"));
        assert!(!node.is_interactive());
    }

    #[test]
    fn caption_and_semantics_carry_the_number() {
        let node = rating("r", 2, 5, IconMark::Checkmark);
        let text = readout(2, 5);
        assert_eq!(text, "2 of 5");
        assert_eq!(
            child(&node, "value").props.text.as_deref(),
            Some(text.as_str())
        );
        assert_eq!(node.semantics.label.as_deref(), Some(text.as_str()));
        assert_eq!(node.semantics.value.as_deref(), Some(text.as_str()));
        assert_eq!(node.semantics.role, Some(Role::Status));
    }

    #[test]
    fn first_value_marks_are_accent_the_rest_are_muted() {
        let node = rating("r", 3, 5, IconMark::CheckmarkFilled);
        for i in 0..3 {
            assert_eq!(
                mark_at(&node, i).props.canvas,
                icon_toned(
                    format!("mark-{i}"),
                    IconMark::CheckmarkFilled,
                    IconTone::Accent
                )
                .props
                .canvas,
            );
        }
        for i in 3..5 {
            assert_eq!(
                mark_at(&node, i).props.canvas,
                icon_toned(
                    format!("mark-{i}"),
                    IconMark::CheckmarkFilled,
                    IconTone::Secondary
                )
                .props
                .canvas,
            );
        }
        assert_ne!(
            mark_at(&node, 0).props.canvas,
            mark_at(&node, 3).props.canvas,
            "filled and empty tones must actually differ"
        );
    }

    #[test]
    fn a_zero_score_still_draws_max_muted_marks() {
        let node = rating("r", 0, 4, IconMark::BulletDisc);
        let marks = child(&node, "marks");
        assert_eq!(marks.children.len(), 4);
        for i in 0..4 {
            assert_eq!(
                mark_at(&node, i).props.canvas,
                icon_toned(
                    format!("mark-{i}"),
                    IconMark::BulletDisc,
                    IconTone::Secondary
                )
                .props
                .canvas,
            );
        }
        assert_eq!(node.semantics.value.as_deref(), Some("0 of 4"));
    }

    #[test]
    fn any_shipped_mark_stands_in_for_the_star_gap() {
        let node = rating("r", 1, 1, IconMark::CheckmarkOutline);
        assert_eq!(
            mark_at(&node, 0).props.canvas,
            icon_toned("mark-0", IconMark::CheckmarkOutline, IconTone::Accent)
                .props
                .canvas,
        );
        let other = rating("r", 1, 1, IconMark::Close);
        assert_ne!(
            mark_at(&node, 0).props.canvas,
            mark_at(&other, 0).props.canvas,
            "the mark argument chooses the glyph"
        );
    }

    #[test]
    fn rating_is_not_interactive() {
        let node = rating("r", 1, 3, IconMark::Check);
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
    }
}
