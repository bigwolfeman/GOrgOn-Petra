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
//!
//! Truncation is two decisions, not one. [`PaintState::truncated`] is "a
//! policy hid something on purpose"; [`PaintState::overflowed`] is "the box
//! was too small". [`place`] sets them separately and clips the run to its own
//! rect, so the second one is a report rather than a picture of one row
//! painted over another.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::Size;
use crate::layout::{LayoutCtx, SizeProposal, Slot, TextRequest, semantics_of};
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
/// means for a shaper. The vertical offer never reaches the shaper at all —
/// `TextRequest` has no such field — because a shaper answers "how tall is
/// this run", not "how tall may it be".
///
/// # Why an `Exact` vertical offer is not a cap
///
/// It used to be one: `size.h = size.h.min(offered)`, so a run asked whether
/// it fit in 37 units answered "37" whatever it actually needed. That is the
/// opposite of the rule every container in this engine negotiates by, written
/// out in `stack::measure`'s own words — *"deliberately not clamped to the
/// offer: a response bigger than the parent can place is the parent's problem
/// to concede at placement, and clamping it here would hide the overflow from
/// the flag the contract requires."*
///
/// Hiding it is exactly what happened. `stack::distribute` offers each child
/// in a priority group an equal share of the pot and rolls the surplus
/// forward, so the first child of a two-child column is offered half the
/// column even when the column is big enough for both. A capped text run
/// accepted that half, the stack read the accepted answer as "it fits",
/// conceded nothing, flagged nothing, and placed a three-line run in a
/// two-line box — which then painted over its neighbour. Measured in the
/// shipped inspector at 2009x1392: `identity/why/label` needed 60.0 units,
/// answered 37.3, and got 37.3, while its sibling panel left 68.0 units of
/// the same column unused.
///
/// Answering honestly makes the run a rigid child (its `Zero` and `Unbounded`
/// responses are the same number, so its `Give::flexibility` is zero), which
/// is what it is: a paragraph at a fixed width has one height. A parent that
/// genuinely cannot fit it now sees `wanted > main_extent`, runs the FR-005
/// concession order, and flags `truncated` on itself — the path that was
/// already written and was simply never reached.
pub fn measure(node: &ViewNode, ctx: &mut LayoutCtx<'_>, proposal: SizeProposal) -> Size {
    let props = node.props.text();
    let req = text_request(&props, proposal.horizontal.available());
    ctx.content.text(&req).size
}

/// Place a text run, recording truncation in the placement's paint state so
/// the semantic tree's `truncated` flag is real.
///
/// Re-measures against the rect this call actually received rather than
/// reusing whatever an earlier [`measure`] probe answered: a parent may probe
/// several proposals before committing a rect narrower or wider than any of
/// them, and the flag has to describe *this* frame, not the negotiation that
/// led to it.
///
/// # The clip is this node's own rect, not its ancestor's
///
/// [`Slot::clip`] arrives as the nearest clipping ancestor's rect — a whole
/// scrolling panel, usually. Painting a run under that clip means a run that
/// does not fit its row draws straight over the row beneath it: the painter
/// is doing exactly what the placement told it to, and the placement was
/// describing the panel rather than the box. So the clip written here is the
/// intersection of the inherited clip with this node's own rect. Two
/// consequences worth stating rather than discovering:
///
/// * The invariant becomes structural. `clip ⊆ rect` for every text
///   placement, so "a text run never paints outside the box it was given" is
///   a property of the frame, checkable without a renderer, and
///   `gorgon/inspector/tests/layout_overlap.rs` checks it — in the private
///   GOrgOn monorepo, where `gorgon-inspector` lives; not in this repository.
/// * `clip` is a digest input, so this moves the digest of every frame that
///   contains text. That is the intended, deliberate re-baseline recorded in
///   the owning agent note, not a side effect.
///
/// A run that overflows is still *reported* — see [`PaintState::overflowed`].
/// Clipping it stops the corruption; it does not pretend the run fit.
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

    // Two different facts, kept apart. The shaper's `truncated` answers only
    // for width and `max_lines` — its request carries no available height, so
    // it has no opinion on the vertical axis. It means "an ellipsis policy
    // fired", which is a policy working, not a defect.
    //
    // Whether the lines it rendered fit the rect this node was actually given
    // is a fact about the rect, checked here, and it means the opposite: the
    // box was too small and the tail of the run is now clipped away. Folding
    // the two into one flag — which this function did until the clip fix —
    // made a correctly-elided title indistinguishable from a corrupted panel,
    // and `semantic::audit`'s `TruncationIsReal` reads the result.
    let height = slot.rect.h.max(0.0);
    let truncated = measurement.truncated;
    let overflowed = measurement.size.h > height;

    let id = path.id();
    let semantics = semantics_of(node, &id, ctx.state);
    sink.push(Placement {
        id,
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip.intersect(slot.rect),
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: crate::frame::digest::hash_text(props.text),
            truncated,
            overflowed,
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

    /// The reverse of what this file used to assert, and the reversal is the
    /// whole of the 2026-08-24 overdraw fix.
    ///
    /// A run asked "do you fit in 20 units?" must answer with its real
    /// height, not with 20. `stack::distribute` offers each child in a
    /// priority group an equal share of the pot before it knows what any of
    /// them needs, so the first child of a two-child column is routinely
    /// offered half a column that is big enough for all of it. A run that
    /// clamped its answer to that offer told the stack it fit; the stack
    /// conceded nothing, flagged nothing, and placed a four-line run in a
    /// one-line box, which then painted over the row beneath it.
    ///
    /// Answering honestly costs nothing and puts the decision where FR-005
    /// says it belongs: `stack::place` compares the sum against the rect and
    /// runs the concession order.
    #[test]
    fn an_exact_vertical_offer_does_not_cap_the_answer() {
        let mut h = Harness::new();
        let node = text_node("abcdefghijklmnop", TextWrap::Wrap, None); // 16 chars
        // Unclipped, this run needs 4 lines at width 32: 4 * 16.0 = 64.0.
        let cramped = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Exact(20.0)),
        );
        assert_eq!(
            cramped,
            Size::new(32.0, 64.0),
            "a stingy vertical offer does not make the run shorter; it makes              the parent's fit impossible, which is the parent's to concede"
        );
        let generous = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Exact(200.0)),
        );
        assert_eq!(
            generous,
            Size::new(32.0, 64.0),
            "and a generous one does not make it taller either — a paragraph              at a fixed width has exactly one height"
        );
    }

    /// The flexibility a text run reports to `stack::distribute`.
    ///
    /// `Give::flexibility` is the `Unbounded` response minus the `Zero`
    /// response, and the stack offers the least flexible child in a group
    /// first so the rigid answers land before the pot is split. A run's
    /// height is a function of its width alone, so both probes answer the
    /// same number and the run is rigid — which is the property that makes
    /// the fix above work rather than merely make it honest.
    #[test]
    fn a_text_run_is_rigid_on_the_axis_it_does_not_wrap_on() {
        let mut h = Harness::new();
        let node = text_node("abcdefghijklmnop", TextWrap::Wrap, None);
        let floor = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Zero),
        );
        let ceiling = measure(
            &node,
            &mut h.ctx(),
            open(Proposal::Exact(32.0), Proposal::Unbounded),
        );
        assert_eq!(
            (ceiling.h - floor.h).max(0.0),
            0.0,
            "zero flexibility: {floor:?} to {ceiling:?}"
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

    /// The invariant the 2026-08-24 overdraw fix establishes, stated on the
    /// frame rather than on a screenshot: a text run's clip never reaches
    /// past its own rect, so a run that does not fit cannot paint over the
    /// row beneath it.
    ///
    /// The inherited clip still applies — a run inside a scrolled panel is
    /// clipped by the panel too — which is why this is an intersection and
    /// not a replacement.
    #[test]
    fn a_text_placement_is_clipped_to_its_own_rect_inside_the_inherited_clip() {
        let mut h = Harness::new();
        let node = text_node("abcdefghijklmnop", TextWrap::Wrap, None);
        let mut path = path_at(&node);
        let mut sink = PlacementList::new();
        // A row 16 units tall inside a panel 400 tall: the run needs four
        // lines at this width and gets one.
        let row = Rect::new(10.0, 100.0, 32.0, 16.0);
        let panel = Rect::new(0.0, 0.0, 200.0, 400.0);
        let mut slot = Slot::new(row);
        slot.clip = panel;
        place(&node, &mut h.ctx(), &mut path, slot, &mut sink);

        let placed = &sink.as_slice()[0];
        assert_eq!(
            placed.clip, row,
            "the clip must be the row, not the panel it sits in"
        );
        assert_eq!(placed.rect, row, "the rect itself does not move");

        // And the inherited clip is still honoured: a row half outside its
        // panel is clipped to the half that is inside.
        let mut path = path_at(&node);
        let mut sink = PlacementList::new();
        let mut slot = Slot::new(Rect::new(10.0, 380.0, 32.0, 40.0));
        slot.clip = panel;
        place(&node, &mut h.ctx(), &mut path, slot, &mut sink);
        assert_eq!(
            sink.as_slice()[0].clip,
            Rect::new(10.0, 380.0, 32.0, 20.0),
            "the intersection, not either rect alone"
        );
    }

    /// The two flags mean opposite things and must not be readable as one.
    ///
    /// A `Clip`-wrapped run wider than its rect is an ellipsis policy doing
    /// its job: `truncated`, not `overflowed`. A wrapping run taller than its
    /// rect is the defect: `overflowed`, not `truncated`. Before the split
    /// both arrived at `semantic::audit` as the same bit.
    #[test]
    fn truncation_and_overflow_are_separate_flags() {
        let mut h = Harness::new();

        // 11 chars * 8.0 = 88.0 wide, cut to 40.0 on one 16.0-tall line.
        let clipped = text_node("hello world", TextWrap::Clip, None);
        let mut path = path_at(&clipped);
        let mut sink = PlacementList::new();
        place(
            &clipped,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 40.0, 16.0)),
            &mut sink,
        );
        let policy = sink.as_slice()[0].paint;
        assert!(policy.truncated, "the line cap fired");
        assert!(
            !policy.overflowed,
            "one line in a one-line box is not an overflow"
        );

        // 16 chars wrapped at 32.0 needs four 16.0-tall lines; it gets one.
        let overflowing = text_node("abcdefghijklmnop", TextWrap::Wrap, None);
        let mut path = path_at(&overflowing);
        let mut sink = PlacementList::new();
        place(
            &overflowing,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 32.0, 16.0)),
            &mut sink,
        );
        let spill = sink.as_slice()[0].paint;
        assert!(
            !spill.truncated,
            "no policy fired: an uncapped wrap elides nothing"
        );
        assert!(spill.overflowed, "four lines in a one-line box");
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
