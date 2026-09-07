//! Turning a two-ended selection into the per-run ranges the painter draws.
//!
//! [`crate::input::TextSelection`] names two points in two nodes. A painter
//! draws one rectangle band per run. This module is the step between, and it
//! is the only place in the engine that knows what "between two nodes" means.
//!
//! # Document order is placement order
//!
//! `crate::layout::place` walks the tree in pre-order and every container
//! pushes its own placement before any child's, so `placements[i]` for
//! ascending `i` is the order a reader meets the nodes in. That is the order
//! this module measures "between" in, and [`Placement::parent`]'s own
//! invariant — a parent's index is always lower than its children's — is the
//! same fact stated from the other side.
//!
//! Nothing here consults geometry. A run that a container has moved below its
//! following sibling is still, to a selection, where the tree put it; the
//! alternative is a selection whose extent changes when a flex line wraps.
//!
//! # Why this is a pass over the finished list and not part of the walk
//!
//! The walk meets the anchor and the focus in some order it cannot know in
//! advance, and the second one may never arrive at all — an application is
//! free to replace the text under a live gesture. A state machine carried
//! through the walk would have to paint a highlight from the first endpoint
//! onward and then discover, at the end of the frame, that there was no
//! second endpoint to stop at. Running afterwards, when both indices are
//! either known or known to be missing, has no such half-answer.

use std::ops::Range;

use crate::frame::digest::hash_paint_content;
use crate::frame::{PaintContent, Placement};
use crate::input::{TextPoint, TextSelection, claimed_by_a_control_in, paints_text_in};

/// Write `selection` onto every run it covers, and report which placements
/// changed.
///
/// The returned indices are the placements whose [`PaintContent::selection`]
/// this call set; each one's `PaintState::paint_hash` is recomputed here, so
/// the frame digest sees the highlight exactly as it sees every other thing
/// a node draws.
///
/// A selection whose anchor or focus names a node this frame did not place
/// covers nothing. That is the stale case — the application changed the tree
/// under a drag — and the honest answer to "where does the span end" when one
/// end is gone is "nowhere", not "at the bottom of the page".
///
/// # Which runs inside the span are skipped
///
/// The same rule the press obeys ([`crate::input::hit_text`]): a run a
/// control has claimed is the control's, and a browser does not put a
/// button's label on the clipboard either. So a span from a paragraph, across
/// a toolbar, and into the next paragraph highlights the two paragraphs and
/// leaves the buttons alone.
pub fn resolve(
    placements: &mut [Placement],
    content: &mut [PaintContent],
    selection: &TextSelection,
) -> Vec<usize> {
    let Some((first, last)) = ends_in_order(placements, selection) else {
        return Vec::new();
    };
    let mut touched = Vec::new();
    for index in first.index..=last.index {
        if !covers(placements, content, index) {
            continue;
        }
        let len = content[index]
            .text
            .as_ref()
            .map_or(0, |text| text.text.len());
        // Clamped, not trusted: the offsets were taken against the string the
        // *previous* frame painted, and nothing stops an application handing
        // a shorter one to this frame. A slice out of bounds here would be a
        // panic inside the layout pass.
        let start = if index == first.index {
            first.offset.min(len)
        } else {
            0
        };
        let end = if index == last.index {
            last.offset.min(len)
        } else {
            len
        };
        let range = clean(start..end, &content[index]);
        let Some(range) = range else { continue };
        content[index].selection = Some(range);
        placements[index].paint.paint_hash = hash_paint_content(&content[index]);
        touched.push(index);
    }
    touched
}

/// One end of a resolved span: which placement, and how far into its string.
struct End {
    index: usize,
    offset: usize,
}

/// The selection's two ends as `(earlier, later)` in document order, or
/// `None` when either end names a node this frame did not place.
fn ends_in_order(placements: &[Placement], selection: &TextSelection) -> Option<(End, End)> {
    let anchor = end_of(placements, &selection.anchor)?;
    let focus = end_of(placements, &selection.focus)?;
    if (anchor.index, anchor.offset) <= (focus.index, focus.offset) {
        Some((anchor, focus))
    } else {
        Some((focus, anchor))
    }
}

fn end_of(placements: &[Placement], point: &TextPoint) -> Option<End> {
    placements
        .iter()
        .position(|placement| placement.id == point.node)
        .map(|index| End {
            index,
            offset: point.offset,
        })
}

/// Whether the run at `index` takes part in a span that crosses it.
///
/// Exactly the pair of tests [`crate::input::hit_text`] applies to the run a
/// press lands on, called through the same two functions. A run that could be
/// selected but would not be highlighted, or the reverse, would be a second
/// answer to a question that has one.
fn covers(placements: &[Placement], content: &[PaintContent], index: usize) -> bool {
    paints_text_in(placements, content, index) && !claimed_by_a_control_in(placements, index)
}

/// The range as the painter will take it: ordered, on character boundaries,
/// and non-empty.
///
/// Boundary-snapping is not defensive. A shaper answers in char offsets that
/// this crate converts to bytes, and the frame those offsets were measured
/// against is the previous one; a string edited between the two frames can
/// leave an offset inside a multi-byte character, and `str` slicing there
/// panics. Snapping outward keeps the whole character the operator's pointer
/// was over rather than dropping it.
fn clean(range: Range<usize>, content: &PaintContent) -> Option<Range<usize>> {
    let text = content.text.as_ref()?;
    let text = text.text.as_str();
    let mut start = range.start.min(text.len());
    let mut end = range.end.min(text.len());
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    (start < end).then_some(start..end)
}

#[cfg(test)]
mod tests;
