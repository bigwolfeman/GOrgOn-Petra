//! Text measurement, wrapping, and truncation.
//!
//! A `text` node is a leaf (`layout/leaf.rs` is the pattern this follows):
//! [`measure`] answers a size, [`place`] pushes exactly one [`Placement`] and
//! no children. The one thing that makes text different from the other leaf
//! kinds is that its truncation is a fact the digest must carry
//! (`contracts/frame-identity.md` §3) and the semantic tree must audit
//! (`contracts/semantic-tree.md`: "every `truncated` flag corresponds to a
//! real truncation"), so this module — not the shaper behind
//! [`crate::layout::ContentMeasure`] — owns the truncation decision.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::Size;
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot, TextRequest, semantics_of};
use crate::tree::{KeyPath, TextProps, ViewNode};

/// Build the shaper request a text node's resolved props describe, offering
/// `available_width` (`None` for an open probe).
fn text_request<'a>(props: &TextProps<'a>, available_width: Option<f32>) -> TextRequest<'a> {
    TextRequest {
        text: props.text,
        style: props.style,
        wrap: props.wrap,
        max_lines: props.max_lines,
        available_width,
    }
}

/// Measure a text run under `proposal`, applying the node's wrap policy.
///
/// The horizontal offer becomes the shaper's available width; `Unbounded` and
/// `Unspecified` both pass through as an open probe (`Proposal::available`
/// already collapses them to `None`), which is what "full unwrapped run"
/// means for a shaper. The vertical offer never reaches the shaper —
/// `TextRequest` has no such field — because a shaper answers "how tall is
/// this run", not "how tall may it be"; instead, a firm `Exact` vertical
/// offer caps the answer directly here, so a parent that offers less height
/// than the run needs never gets back a size larger than what it offered.
pub fn measure(node: &ViewNode, ctx: &mut LayoutCtx<'_>, proposal: SizeProposal) -> Size {
    let props = node.props.text();
    let req = text_request(&props, proposal.horizontal.available());
    let mut size = ctx.content.text(&req).size;
    if let Proposal::Exact(h) = proposal.vertical {
        size.h = size.h.min(h.max(0.0));
    }
    size
}

/// Place a text run, recording truncation in the placement's paint state so
/// the semantic tree's `truncated` flag is real.
///
/// Re-measures against the rect this call actually received rather than
/// reusing whatever an earlier [`measure`] probe answered: a parent may probe
/// several proposals before committing a rect narrower or wider than any of
/// them, and the flag has to describe *this* frame, not the negotiation that
/// led to it.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let props = node.props.text();
    let width = slot.rect.w.max(0.0);
    let req = text_request(&props, Some(width));
    let measurement = ctx.content.text(&req);

    // The shaper's `truncated` answers only for width and `max_lines` — its
    // request carries no available height, so it has no opinion on the
    // vertical axis. Whether the lines it rendered fit the rect this node was
    // actually given is a fact about the rect, checked here, never inferred
    // from the shaper's own flag.
    let height = slot.rect.h.max(0.0);
    let truncated = measurement.truncated || measurement.size.h > height;

    let id = path.id();
    let semantics = semantics_of(node, &id, ctx.state);
    sink.push(Placement {
        id,
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: crate::frame::digest::hash_text(props.text),
            truncated,
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics,
        parent: None,
    });
}

#[cfg(test)]
mod tests {
    use super::{measure, place};
    use crate::frame::placement::PlacementList;
    use crate::geom::{Rect, Size};
    use crate::layout::{Proposal, SizeProposal, Slot};
    use crate::testing::Harness;
    use crate::tree::{KeyPath, NodeKind, Props, TextWrap, ViewNode};

    fn text_node(text: &str, wrap: TextWrap, max_lines: Option<usize>) -> ViewNode {
        ViewNode::new(NodeKind::Text, "t").with_props(Props {
            text: Some(text.to_owned()),
            wrap: Some(wrap),
            max_lines,
            ..Props::default()
        })
    }

    /// Mirrors how the dispatcher in `layout/mod.rs` would have arrived here:
    /// it pushes the node's own key before calling into the kind module.
    fn path_at(node: &ViewNode) -> KeyPath {
        let mut path = KeyPath::root();
        path.push(node.key.clone());
        path
    }

    fn open(horizontal: Proposal, vertical: Proposal) -> SizeProposal {
        SizeProposal {
            horizontal,
            vertical,
        }
    }

    // MonoContent: 8.0 logical units per character, 16.0 per line.

    #[test]
    fn wrap_grows_line_count_to_fit_the_offered_width() {
        let mut h = Harness::new();
        let node = text_node("abcdefgh", TextWrap::Wrap, None); // 8 chars
        let size = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Unbounded),
        );
        // 32 / 8 = 4 chars/line, 8 chars needs ceil(8/4) = 2 lines.
        assert_eq!(size, Size::new(32.0, 32.0));
    }

    #[test]
    fn ellipsis_stays_one_line_and_clips_to_the_offered_width() {
        let mut h = Harness::new();
        let node = text_node("abcdefgh", TextWrap::Ellipsis, None);
        let size = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Unbounded),
        );
        assert_eq!(size, Size::new(32.0, 16.0));
    }

    #[test]
    fn clip_stays_one_line_and_clips_to_the_offered_width() {
        let mut h = Harness::new();
        let node = text_node("abcdefgh", TextWrap::Clip, None);
        let size = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Unbounded),
        );
        assert_eq!(size, Size::new(32.0, 16.0));
    }

    #[test]
    fn an_exact_vertical_offer_caps_the_answer_but_never_grows_it() {
        let mut h = Harness::new();
        let node = text_node("abcdefghijklmnop", TextWrap::Wrap, None); // 16 chars
        // Unclipped, this run needs 4 lines at width 32: 4 * 16.0 = 64.0.
        let capped = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Exact(20.0)),
        );
        assert_eq!(
            capped,
            Size::new(32.0, 20.0),
            "the offer is smaller than the run, so it wins"
        );
        let generous = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Exact(200.0)),
        );
        assert_eq!(
            generous,
            Size::new(32.0, 64.0),
            "the offer is larger than the run, so the run wins"
        );
    }

    #[test]
    fn an_open_probe_answers_the_full_unwrapped_run_and_is_not_truncated() {
        let mut h = Harness::new();
        let node = text_node("abcdefgh", TextWrap::Wrap, None); // 8 chars
        let size = measure(&node, &mut h.ctx(), SizeProposal::unbounded());
        assert_eq!(size, Size::new(64.0, 16.0));

        // Placed into exactly the rect the open probe answered: nothing was
        // hidden.
        let mut path = path_at(&node);
        let mut sink = PlacementList::new();
        place(
            &node,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 64.0, 16.0)),
            &mut sink,
        );
        assert!(!sink.as_slice()[0].paint.truncated);
    }

    /// `max_lines` is a content policy: content past the cap is dropped
    /// regardless of how much vertical room the placement rect has. The same
    /// text without a cap renders every wrapped line and is not truncated,
    /// even though it is given the identical rect.
    #[test]
    fn max_lines_truncates_a_generous_rect_while_no_cap_does_not() {
        let mut h = Harness::new();
        let text = "abcdefghijklmnop"; // 16 chars, 4 chars/line at width 32 -> 4 lines unwrapped
        let rect = Rect::new(0.0, 0.0, 32.0, 100.0); // plenty tall for all 4 lines

        let capped = text_node(text, TextWrap::Wrap, Some(2));
        let mut path = path_at(&capped);
        let mut sink = PlacementList::new();
        place(&capped, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink);
        assert!(
            sink.as_slice()[0].paint.truncated,
            "max_lines(2) drops the remaining 2 lines even though the rect could hold all 4"
        );

        let uncapped = text_node(text, TextWrap::Wrap, None);
        let mut path = path_at(&uncapped);
        let mut sink = PlacementList::new();
        place(
            &uncapped,
            &mut h.ctx(),
            &mut path,
            Slot::new(rect),
            &mut sink,
        );
        assert!(!sink.as_slice()[0].paint.truncated);
    }

    /// The flag has to be honest in both directions: a run that fits its rect
    /// says so, and a run that does not also says so.
    #[test]
    fn the_truncation_flag_is_honest_in_both_directions() {
        let mut h = Harness::new();

        let fits = text_node("hi", TextWrap::Clip, None); // 2 chars * 8.0 = 16.0
        let mut path = path_at(&fits);
        let mut sink = PlacementList::new();
        place(
            &fits,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 16.0, 16.0)),
            &mut sink,
        );
        assert!(!sink.as_slice()[0].paint.truncated);

        let overflows = text_node("hello world", TextWrap::Clip, None); // 11 chars * 8.0 = 88.0
        let mut path = path_at(&overflows);
        let mut sink = PlacementList::new();
        place(
            &overflows,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 40.0, 16.0)),
            &mut sink,
        );
        assert!(sink.as_slice()[0].paint.truncated);
    }

    /// The case that separates "describes this frame" from "describes the
    /// probe": negotiation probes a tight width, which would truncate — but
    /// the rect the node is actually placed into is generous enough for the
    /// full run. The placement must describe the rect it received, not the
    /// earlier probe.
    #[test]
    fn a_tight_measure_probe_does_not_poison_a_generous_placement() {
        let mut h = Harness::new();
        let node = text_node("abcdefgh", TextWrap::Ellipsis, None); // 8 chars, full width 64.0

        let probed = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Unbounded),
        );
        assert_eq!(
            probed,
            Size::new(32.0, 16.0),
            "the probe was tight, so it truncated"
        );

        let mut path = path_at(&node);
        let mut sink = PlacementList::new();
        place(
            &node,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 64.0, 16.0)),
            &mut sink,
        );
        assert!(
            !sink.as_slice()[0].paint.truncated,
            "the assigned rect fits the whole run; the earlier tight probe must not leak into this frame"
        );
    }

    #[test]
    fn content_hash_changes_with_text_and_is_stable_for_the_same_text() {
        let mut h = Harness::new();
        let rect = Rect::new(0.0, 0.0, 200.0, 16.0);

        let a1 = text_node("hello", TextWrap::Clip, None);
        let mut path = path_at(&a1);
        let mut sink = PlacementList::new();
        place(&a1, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink);
        let hash_a1 = sink.as_slice()[0].paint.content_hash;

        let a2 = text_node("hello", TextWrap::Clip, None);
        let mut path = path_at(&a2);
        let mut sink = PlacementList::new();
        place(&a2, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink);
        let hash_a2 = sink.as_slice()[0].paint.content_hash;
        assert_eq!(
            hash_a1, hash_a2,
            "the same text hashes the same way every time"
        );
        assert_eq!(hash_a1, crate::frame::digest::hash_text("hello"));

        let b = text_node("world", TextWrap::Clip, None);
        let mut path = path_at(&b);
        let mut sink = PlacementList::new();
        place(&b, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink);
        let hash_b = sink.as_slice()[0].paint.content_hash;
        assert_ne!(hash_a1, hash_b, "different text must not collide");
    }

    #[test]
    fn the_placement_carries_the_context_theme_revision() {
        let mut h = Harness::new();
        h.theme_rev = 7;
        let node = text_node("hi", TextWrap::Clip, None);
        let mut path = path_at(&node);
        let mut sink = PlacementList::new();
        place(
            &node,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 16.0, 16.0)),
            &mut sink,
        );
        assert_eq!(sink.as_slice()[0].paint.token_revision, 7);
    }

    /// SC-009 seed. `MonoContent` is fixed-pitch — every `char` is exactly
    /// `char_w` wide, no matter its script. It has no idea that CJK glyphs
    /// typically run wider than Latin ones, that Arabic reorders
    /// right-to-left, or that an emoji is one glyph rendered from one `char`.
    /// It cannot stand in for real script metrics, so this test proves only
    /// that the engine's text path — character counting, wrap arithmetic,
    /// hashing — does not panic or mis-slice on non-ASCII, multi-byte input.
    /// The actual mixed-script rendering claim (SC-009) is verified against
    /// real shaping in `gorgon-petra-egui`'s T066 test, not here.
    #[test]
    fn mixed_script_text_measures_and_places_without_panicking() {
        let mut h = Harness::new();
        let sample = "Aloha 你好 مرحبا 😀";
        let char_count = sample.chars().count();
        let node = text_node(sample, TextWrap::Wrap, None);

        // Open probe: the full unwrapped run, one line. The expectation is
        // stated from the character count, not a hand-typed magic number —
        // the point of this test is that non-ASCII characters are counted
        // and shaped without panicking, not that the number itself is
        // meaningful for real typography.
        let (char_w, line_h) = (h.content.char_w, h.content.line_h);
        let open_size = measure(&node, &mut h.ctx(), SizeProposal::unbounded());
        assert_eq!(open_size, Size::new(char_count as f32 * char_w, line_h));

        // A width narrow enough to force wrapping through the mixed-script,
        // multi-byte run. If this module ever sliced text on byte offsets
        // instead of `char_indices`, a CJK or emoji boundary here is where it
        // would panic.
        let mut path = path_at(&node);
        let mut sink = PlacementList::new();
        place(
            &node,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 24.0, 200.0)),
            &mut sink,
        );
        assert_eq!(
            sink.as_slice()[0].paint.content_hash,
            crate::frame::digest::hash_text(sample)
        );
    }
}
