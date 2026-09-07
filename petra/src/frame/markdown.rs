//! What a copied selection puts in front of a line.
//!
//! A selection copies the strings the frame painted, joined in placement
//! order ([`PetrifiedFrame::selected_text`]). Two things a reader needs are
//! not in any of those strings: how deep in a list a line sits, and whether
//! the line is a list item at all. The first is an indent this module counts
//! off the tree. The second is a mark the component that drew it declares in
//! [`crate::tree::Props::markdown`], because a bullet in this library is a
//! picture and not a character — `crate::component::list`: *"a typed marker
//! cannot be sized, centred or snapped apart from a character"*.
//!
//! So a copied list arrives as Markdown a renderer reads:
//!
//! ```text
//! - Inbox
//!   - Archive
//!     - 2026
//! ```
//!
//! **The engine recognises no bullets.** It emits what a node said it stands
//! for, beside the run that node was drawn in front of. Nothing here knows
//! what a list looks like, which is what keeps the next drawn mark — a task
//! list's `- [ ]`, say — a one-line change in the component that draws it.

#[cfg(test)]
mod tests;

use super::PetrifiedFrame;
use crate::tree::Role;

/// What precedes a run that begins a line in a copied selection: the list
/// indent, then the Markdown of a mark drawn immediately before it.
///
/// `from_the_top` is whether the selection took this run from its first byte.
/// A drag that starts halfway down "Archive" copies `ive`, and a bullet in
/// front of half a word claims a list item that the operator did not select.
/// The indent is not conditional the same way: a partial line is still at its
/// own depth.
pub(super) fn line_lead(frame: &PetrifiedFrame, index: usize, from_the_top: bool) -> String {
    let mut lead = indent(frame, index);
    if from_the_top && let Some(mark) = mark_before(frame, index) {
        lead.push_str(mark);
        lead.push(' ');
    }
    lead
}

/// The leading spaces for a run, two per list level below the first.
///
/// Counted off [`Role::List`] and [`Role::Tree`] ancestors rather than read
/// back off the rects. The rect's indent is a number the layout chose for
/// pixels — Carbon hangs a marker in a gutter and the label's left edge moves
/// with the marker's width — and reading it back as though it were structure
/// would put a nested item's depth at the mercy of which bullet it drew.
fn indent(frame: &PetrifiedFrame, index: usize) -> String {
    let mut levels = 0usize;
    let mut cursor = Some(index);
    while let Some(at) = cursor {
        let placement = &frame.placements[at];
        if matches!(placement.semantics.role, Some(Role::List | Role::Tree)) {
            levels += 1;
        }
        cursor = placement.parent;
    }
    " ".repeat(levels.saturating_sub(1) * 2)
}

/// The Markdown of the mark drawn immediately before `index`, if there is one.
///
/// "Immediately before" is the nearest preceding sibling, not the nearest
/// declaration anywhere above: a mark stands in front of the run it belongs
/// to, and reaching further would let one row's bullet lead another row's
/// text.
fn mark_before(frame: &PetrifiedFrame, index: usize) -> Option<&str> {
    let parent = frame.placements[index].parent;
    let (at, _) = frame.placements[..index]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, sibling)| sibling.parent == parent)?;
    let content = frame.content.get(at)?;
    // A node that paints its own string already copies that string, so
    // reading a mark off it as well would say the same thing twice. This is
    // where `Props::markdown`'s "read only from nodes that paint no text"
    // is enforced rather than merely documented.
    if content.text.is_some() {
        return None;
    }
    content.markdown.as_deref()
}
