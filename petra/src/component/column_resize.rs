//! Draggable column boundaries, shared by every grid-shaped table.
//!
//! [`super::structured_list`] and [`super::data_table`] both interleave a
//! divider track between their data columns and both turn a drag on one into
//! new column weights. The divider node, its key, its intents, the narrowest
//! column and the weight arithmetic are one design, so they live here once.

use crate::geom::{Align, Axis, Point, Rect};
use crate::tree::{Behaviour, Intent, Interaction, Justify, Phase, Role, ViewNode};

use super::stack;
use super::tokens::BORDER_SUBTLE;

/// The width of a column divider's grid track, and so of its drag target.
///
/// Not a Carbon number — Carbon has no such control (slice-b:59). It is
/// `$spacing-03` (8) expressed as a length rather than a token, because a
/// track size is a number in this engine and not a token reference, and 8
/// is the smallest step in Carbon's own spacing set that is still a target
/// a pointer can land on. The rule the eye sees is the material's own
/// thickness ([`crate::token::rule::thickness`]), so the target is about
/// twice the mark. It used to be eight times: the rule was written here as
/// one logical unit against a groove four device pixels deep, which is why
/// this column seam painted a flat bright line beside grooved row rules.
pub(super) const DIVIDER: f32 = 8.0;

/// The narrowest a column may be dragged.
///
/// A cell carries `$spacing-05` (16) of inline padding on each side, so 32
/// is all padding. Wrap then stacks one glyph per line. 96 is that padding
/// plus 64 of content.
pub(super) const MIN_COLUMN: f32 = 96.0;

const _: () = assert!(MIN_COLUMN == 96.0);

/// The key prefix every divider carries: `div0` is the boundary between
/// column 0 and column 1.
pub(super) const DIVIDER_KEY: &str = "div";

/// What a column divider declares. `Drag` alone: a divider is not a focus
/// stop and has no click.
pub(super) const DIVIDER_INTENTS: &[Interaction] = &[Interaction::Drag];

/// Spec 010: live adjustment under drag.
pub(super) const ADJUSTS_ON_CHANGE: Behaviour = Behaviour {
    intent: Intent::Adjust,
    phase: Phase::OnChange,
};

/// One draggable column boundary: a [`DIVIDER`]-wide target with a
/// rule down its centre, at the material's own width.
///
/// `name` is the caption of the column to its left, so the accessible name
/// says which boundary this is. A divider with no name to its left still
/// gets a non-empty label, because an interactive node without one is an
/// audit violation (`ActionableNeedsRoleAndLabel`) and a blank string is
/// not a name.
pub(super) fn column_divider(index: usize, name: Option<&String>) -> ViewNode {
    let rule = super::rule("rule", Axis::Vertical, BORDER_SUBTLE);

    let label = match name {
        Some(name) if !name.trim().is_empty() => format!("Resize column {name}"),
        _ => format!("Resize column {}", index + 1),
    };
    let mut node = stack(
        format!("{DIVIDER_KEY}{index}"),
        Axis::Horizontal,
        None,
        vec![rule],
    );
    // Cross axis is the height: the rule runs the full height of the row.
    node.props.align = Some(Align::Stretch);
    // Main axis is the width: the one-unit rule sits in the middle of the
    // eight-unit target rather than against its leading edge.
    node.props.justify = Some(Justify::Center);
    node.interactive(Role::Separator, label, DIVIDER_INTENTS)
        .with_behaviour(ADJUSTS_ON_CHANGE)
}

/// The row id and the column index of the divider `node` names, or `None`.
///
/// By segment rather than by suffix, because a gesture can be routed to the
/// divider itself or to the `rule` leaf inside it, and both have to answer
/// the same boundary.
pub(super) fn divider_of(node: &str) -> Option<(&str, usize)> {
    let mut offset = 0usize;
    for segment in node.split('/') {
        let start = offset;
        offset += segment.len() + 1;
        if let Some(index) = segment
            .strip_prefix(DIVIDER_KEY)
            .and_then(|digits| digits.parse::<usize>().ok())
        {
            return Some((&node[..start.saturating_sub(1)], index));
        }
    }
    None
}

/// Whether a row child's own key names a divider (`div` and digits).
pub(super) fn is_divider_key(key: &str) -> bool {
    key.strip_prefix(DIVIDER_KEY)
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

/// `weights` at exactly `ncols` entries, every one strictly positive.
///
/// A non-positive weight is a tree-acceptance error
/// (`TrackSize::Weight`'s own contract), so a caller handing one in would
/// get a refused tree rather than a narrow column; 1.0 is the equal-columns
/// default the same caller would have got without weights.
pub(super) fn normalise_weights(weights: &[f32], ncols: usize) -> Vec<f32> {
    (0..ncols)
        .map(|i| match weights.get(i) {
            Some(w) if *w > 0.0 && w.is_finite() => *w,
            _ => 1.0,
        })
        .collect()
}

/// The weights a drag to `pos` on the divider between columns `index` and
/// `index + 1` asks for, given their placed rects `left` and `right`.
///
/// Only those two columns move, and their weights sum to what they summed
/// to before, so every other column keeps the pixel width it had. `None`
/// when `index` has no right-hand column in `weights`, or when the two
/// columns have less room between them than two [`MIN_COLUMN`]s: "this drag
/// changes nothing", never a silent zero.
pub(super) fn drag_pair(
    left: Rect,
    right: Rect,
    index: usize,
    pos: Point,
    weights: &[f32],
) -> Option<Vec<f32>> {
    if index + 1 >= weights.len() {
        return None;
    }
    // The two column tracks with the divider track between them.
    let travel = right.right() - left.x - DIVIDER;
    if travel <= 2.0 * MIN_COLUMN {
        return None;
    }
    // `pos.x` is where the *middle* of the divider should land.
    let want = (pos.x - DIVIDER / 2.0 - left.x).clamp(MIN_COLUMN, travel - MIN_COLUMN);
    let pair = weights[index] + weights[index + 1];
    let mut out = weights.to_vec();
    out[index] = pair * want / travel;
    out[index + 1] = pair - out[index];
    Some(out)
}
