//! The scroll container and its virtualized `collection` child.
//!
//! `scroll` clips and offsets one child along a declared axis
//! (`contracts/view-tree.md` §"Negotiation algorithm" item 4: "the scrolling
//! axis proposes `Unbounded` to content; the viewport axis passes through").
//! `collection` is a virtualized row range read from
//! [`crate::layout::RowSource`] rather than a child list: only the visible
//! window plus declared overscan is ever materialized (FR-009), and
//! `total_count` reaches the semantic tree through
//! [`crate::layout::semantics_of`] regardless of how few rows are placed.
//!
//! # Row identity
//! A materialized row's placement id is the row node's own `key`
//! (`row-{index}` from [`crate::testing::GeneratedRows::row`], or whatever a
//! real store-side source names it) appended to the collection's path by the
//! ordinary [`crate::layout::place`] dispatch. Nothing in this module
//! invents or renumbers an id, so the same content index always produces the
//! same id no matter where the scroll window currently sits.
//!
//! # A `collection`'s axis and overscan are its own
//! `measure`/`place` only ever see (node, ctx, path, proposal/slot) — there
//! is no way to reach a `collection`'s *scroll ancestor's* `ViewNode` from
//! here to read its declared axis or overscan off it. `Props` is a flat bag
//! shared by every kind (`contracts/view-tree.md` §"Tree shape"), so a
//! `collection` reads `axis` and `overscan` off its own props, the same way
//! a `scroll` does. An author who nests a `collection` in a `scroll` is
//! expected to declare the same axis on both (both default to `Vertical`,
//! so the common case agrees for free); nothing here checks that agreement,
//! because there is nothing here to check it against.
//!
//! # Known approximation
//! Row *positions* come from a uniform `index * estimated_extent` grid; row
//! *sizes* come from actually measuring each materialized row, which may
//! answer a different extent than the estimate (documented at
//! [`place_collection`]). A window containing non-uniform rows can therefore
//! show a small overlap or gap at those rows. Correcting that would mean
//! measuring every row from index 0 to place one, which defeats
//! virtualization; real content should keep `estimated_extent` close to the
//! rows' true extent for the approximation to stay unnoticeable.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::{Axis, Rect, Size};
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot, semantics_of};
use crate::tree::props::{DEFAULT_OVERSCAN, DEFAULT_ROW_EXTENT};
use crate::tree::{KeyPath, ViewNode};

/// Measure this container under `proposal`.
///
/// `path` already names this node: the dispatcher pushed it. Child measurement
/// goes through [`crate::layout::measure`], which pushes the child's own key.
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let scroll = node.props.scroll();
    let axis = scroll.axis;
    let Some(child) = node.children.first() else {
        return Size::ZERO;
    };

    // The scrolling axis always probes the content's maximum useful extent,
    // regardless of what this container was itself offered on that axis —
    // that is the number a `Zero`/`Unbounded`/`Unspecified` proposal needs
    // below, and it is also how `place` later learns the content extent for
    // offset clamping. The cross axis passes the incoming offer straight
    // through, unchanged.
    let child_proposal = proposal.with_axis(axis, Proposal::Unbounded);
    let child_size = crate::layout::measure(child, ctx, path, child_proposal);

    // On the scrolling axis this container answers what its *viewport*
    // takes, not what its content takes: an `Exact` offer is honoured
    // exactly, `Zero` is the minimum, and only the two open probes reach for
    // the content extent just measured.
    let viewport_extent = match proposal.axis(axis) {
        Proposal::Exact(v) => v.max(0.0),
        Proposal::Zero => 0.0,
        Proposal::Unbounded | Proposal::Unspecified => child_size.along(axis),
    };
    Size::from_axes(axis, viewport_extent, child_size.across(axis))
}

/// Place this container and everything under it into `slot`.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let me = sink.push(Placement {
        id: path.id(),
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: 0,
            truncated: false,
            token_revision: ctx.theme_rev,
        },
        semantics: semantics_of(node),
        parent: None,
    });
    sink.enter(me);

    if let Some(child) = node.children.first() {
        let scroll = node.props.scroll();
        let axis = scroll.axis;
        let viewport_size = slot.rect.size();

        // Same child-proposal shape as `measure`, but the cross axis is now
        // an `Exact` offer taken from the settled viewport rather than
        // whatever this container's own incoming proposal happened to be:
        // by `place` time negotiation is over and the viewport size is a
        // fact, not one of several offers being probed.
        let child_proposal =
            SizeProposal::exact(viewport_size).with_axis(axis, Proposal::Unbounded);
        let content_size = crate::layout::measure(child, ctx, path, child_proposal);

        let viewport_extent = viewport_size.along(axis);
        let content_extent = content_size.along(axis);
        let max_offset = (content_extent - viewport_extent).max(0.0);

        // `scroll_offset` already floors a missing/negative/non-finite entry
        // at zero; clamping against `max_offset` is the other half — a
        // snapshot taken before a prepend or a resize can name an offset
        // past the content that exists now, and that must not blank the
        // viewport.
        let raw_offset = ctx.state.scroll_offset(&path.id());
        let offset = raw_offset.clamp(0.0, max_offset);

        let main_origin = origin_along(slot.rect, axis) - offset;
        let child_rect = axis_rect(
            slot.rect,
            axis,
            main_origin,
            content_extent,
            content_size.across(axis),
        );
        // The child is placed at its full content size so ordinary children
        // (a stack of rows, say) lay out naturally; only the *paint* clip is
        // narrowed to this container's own rect, which is what makes
        // overflow reachable-but-hidden rather than painted outside.
        let child_slot = Slot {
            rect: child_rect,
            z: slot.z,
            clip: slot.clip,
            opacity: slot.opacity,
        }
        .clipped_to(slot.rect);

        crate::layout::place(child, ctx, path, child_slot, sink);
    }

    sink.leave();
}

/// Measure a `collection` node: a virtualized row range read from a store-side
/// source through [`crate::layout::RowSource`].
pub fn measure_collection(
    node: &ViewNode,
    _ctx: &mut LayoutCtx<'_>,
    _path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let Some(collection) = node.props.collection() else {
        return Size::ZERO;
    };
    let axis = node.props.axis.unwrap_or(Axis::Vertical);
    let row_extent = sane_row_extent(collection.estimated_extent);

    // Rows are never fetched to answer a measurement (FR-009): the content
    // extent is the declared total under the uniform-row assumption, not an
    // observed one. A real per-row measurement here would mean walking every
    // row just to answer a size probe, which is exactly what virtualization
    // exists to avoid.
    let content_extent = collection.total_count as f32 * row_extent;
    let cross = proposal.axis(axis.cross()).available().unwrap_or(0.0);
    Size::from_axes(axis, content_extent, cross)
}

/// Place a `collection` node: only the visible window plus declared overscan
/// is materialized, and `total_count` reaches the semantic tree.
pub fn place_collection(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let me = sink.push(Placement {
        id: path.id(),
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: 0,
            truncated: false,
            token_revision: ctx.theme_rev,
        },
        semantics: semantics_of(node),
        parent: None,
    });
    sink.enter(me);

    if let Some(collection) = node.props.collection() {
        let axis = node.props.axis.unwrap_or(Axis::Vertical);
        let row_extent = sane_row_extent(collection.estimated_extent);
        let total_extent = collection.total_count as f32 * row_extent;

        // `slot.rect` is this node's own placement: for a collection that is
        // the content-sized rect `measure_collection` answered (`place`
        // above positions a child at `-offset` and only narrows `clip` to
        // the viewport). The viewport is therefore `slot.clip`'s extent —
        // the narrowest region an ancestor scroll has clipped this subtree
        // to — never `slot.rect`'s.
        let viewport_extent = slot.clip.size().along(axis).max(0.0);

        let overscan = node
            .props
            .overscan
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(DEFAULT_OVERSCAN);

        // The offset lives on the *scroll container's* id, not this node's.
        // `enclosing_scroll_offset` walks the path this call arrived on to
        // find it — documented at its own definition.
        let raw_offset = enclosing_scroll_offset(ctx, path);
        let max_offset = (total_extent - viewport_extent).max(0.0);
        let offset = raw_offset.clamp(0.0, max_offset);

        let window_start = (offset - overscan).max(0.0);
        let window_end = (offset + viewport_extent + overscan).min(total_extent);

        if collection.total_count > 0 && window_end > window_start {
            let start_index =
                ((window_start / row_extent).floor() as usize).min(collection.total_count);
            let end_index = ((window_end / row_extent).ceil() as usize).min(collection.total_count);

            if start_index < end_index {
                let cross_extent = slot.rect.size().across(axis);
                // The window is at most `viewport + 2 * overscan` rows wide,
                // never `total_count`: `rows` is never called with a range
                // wider than that.
                let fetched = ctx.rows.rows(&collection.source, start_index..end_index);
                // A source that cannot answer the whole range returns fewer
                // rows (`RowSource::rows`'s contract); that shortfall is
                // reported by placing fewer rows, never by fabricating the
                // rest.
                for (offset_in_window, row_node) in fetched.iter().enumerate() {
                    let row_index = start_index + offset_in_window;
                    let row_proposal =
                        SizeProposal::exact(Size::from_axes(axis, row_extent, cross_extent));
                    // A row that measures a different extent than
                    // `estimated_extent` is placed at its own measured
                    // extent (see the module doc's "Known approximation").
                    let row_size = crate::layout::measure(row_node, ctx, path, row_proposal);
                    let main_origin = origin_along(slot.rect, axis) + row_index as f32 * row_extent;
                    let row_rect = axis_rect(
                        slot.rect,
                        axis,
                        main_origin,
                        row_size.along(axis),
                        row_size.across(axis),
                    );
                    let row_slot = Slot {
                        rect: row_rect,
                        z: slot.z,
                        clip: slot.clip,
                        opacity: slot.opacity,
                    };
                    crate::layout::place(row_node, ctx, path, row_slot, sink);
                }
            }
        }
    }

    sink.leave();
}

/// The scroll offset of the nearest enclosing scroll container, or `0.0` when
/// none is found.
///
/// A `collection` does not carry a scroll offset itself — only a `scroll`
/// container's id is a key in `ctx.state.scroll_offsets`. This node's `path`
/// is the only handle back to that id, so this rebuilds each ancestor's
/// canonical id from a shrinking prefix of `path.segments()` (nearest parent
/// first) and returns the offset of the first one present in the map.
///
/// This is a best-effort match, not a tree walk: nothing here can tell a
/// `scroll` ancestor's id from a plain `stack`'s, so an ancestor that has
/// never been scrolled (and so has no entry in the map at all) is
/// indistinguishable from "not a scroll container" and this keeps walking
/// past it. In the common case — a `collection` as the direct child of the
/// `scroll` it belongs to — the first candidate checked is that scroll's own
/// id, which is what every test in this module relies on.
fn enclosing_scroll_offset(ctx: &LayoutCtx<'_>, path: &KeyPath) -> f32 {
    let segments = path.segments();
    let parent_len = segments.len().saturating_sub(1); // exclude this node itself
    for len in (0..=parent_len).rev() {
        let mut ancestor = KeyPath::root();
        for key in &segments[..len] {
            ancestor.push(key.clone());
        }
        if let Some(&offset) = ctx.state.scroll_offsets.get(&ancestor.id()) {
            return if offset.is_finite() {
                offset.max(0.0)
            } else {
                0.0
            };
        }
    }
    0.0
}

/// A declared `estimated_extent`, or the shared default when the declared
/// value is not a usable positive number.
///
/// `Props::collection` already substitutes [`DEFAULT_ROW_EXTENT`] for an
/// *absent* value; this additionally guards a present-but-unusable one (zero,
/// negative, non-finite), since it is a divisor below and a stray author
/// input must not turn virtualization into a division by zero.
fn sane_row_extent(estimated: f32) -> f32 {
    if estimated.is_finite() && estimated > 0.0 {
        estimated
    } else {
        DEFAULT_ROW_EXTENT
    }
}

/// The origin of `rect` along `axis` (`x` for horizontal, `y` for vertical).
fn origin_along(rect: Rect, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => rect.x,
        Axis::Vertical => rect.y,
    }
}

/// Build a rect from a main-axis origin and extent plus a cross-axis extent,
/// keeping the cross-axis origin from `base`. The rect analogue of
/// [`Size::from_axes`], which `geom` has no equivalent of.
fn axis_rect(
    base: Rect,
    axis: Axis,
    main_origin: f32,
    main_extent: f32,
    cross_extent: f32,
) -> Rect {
    let main_extent = main_extent.max(0.0);
    let cross_extent = cross_extent.max(0.0);
    match axis {
        Axis::Horizontal => Rect::new(main_origin, base.y, main_extent, cross_extent),
        Axis::Vertical => Rect::new(base.x, main_origin, cross_extent, main_extent),
    }
}

#[cfg(test)]
mod tests {
    use crate::frame::placement::PlacementList;
    use crate::geom::{Axis, Rect, Size};
    use crate::layout::{Proposal, SizeProposal, Slot};
    use crate::testing::{GeneratedRows, Harness, MonoContent};
    use crate::tree::{AxisConstraint, Constraints, KeyPath, NodeKind, Props, ViewNode};

    /// A spacer whose measured extent on the vertical axis is pinned to
    /// `main`, no matter what proposal it is probed with — a deterministic
    /// stand-in for "a child with a known content extent" that does not
    /// depend on this module's own `Unbounded`-probing behaviour to produce
    /// its size.
    fn fixed_extent_child(main: f32) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, "content").with_constraints(Constraints {
            horizontal: AxisConstraint::default(),
            vertical: AxisConstraint {
                min: Some(main),
                max: Some(main),
                priority: 0,
            },
        })
    }

    fn scroll_with_collection(
        total_count: usize,
        estimated_extent: f32,
        overscan: f32,
    ) -> ViewNode {
        let collection = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(total_count),
            source: Some("fibers".into()),
            estimated_extent: Some(estimated_extent),
            ..Props::default()
        });
        ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                overscan: Some(overscan),
                ..Props::default()
            })
            .child(collection)
    }

    #[test]
    fn the_scrolling_axis_proposes_unbounded_while_the_cross_axis_passes_through() {
        let mut h = Harness::new();
        let tree =
            ViewNode::new(NodeKind::Scroll, "list").child(ViewNode::new(NodeKind::Spacer, "gap"));
        let mut path = KeyPath::root();

        // The viewport axis (vertical) answers the exact offer it was given;
        // the cross axis (horizontal) passes that same offer straight to the
        // spacer, which answers it exactly too.
        let viewport = crate::layout::measure(
            &tree,
            &mut h.ctx(),
            &mut path,
            SizeProposal {
                horizontal: Proposal::Exact(120.0),
                vertical: Proposal::Exact(50.0),
            },
        );
        assert_eq!(viewport, Size::new(120.0, 50.0));

        // An `Unbounded` probe on the scroll axis reaches the content extent
        // regardless of the incoming vertical proposal — a spacer's
        // `Unbounded` answer is `SPACER_MAX_EXTENT` — while the cross axis
        // still passes the horizontal offer through unchanged.
        let content = crate::layout::measure(
            &tree,
            &mut h.ctx(),
            &mut path,
            SizeProposal {
                horizontal: Proposal::Exact(120.0),
                vertical: Proposal::Unbounded,
            },
        );
        assert_eq!(
            content,
            Size::new(120.0, crate::layout::leaf::SPACER_MAX_EXTENT)
        );
    }

    #[test]
    fn placing_moves_the_child_by_exactly_the_negative_offset_and_clips_to_the_viewport() {
        let mut h = Harness::new();
        h.set_scroll("/list", 40.0);
        let tree = ViewNode::new(NodeKind::Scroll, "list").child(fixed_extent_child(400.0));
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 120.0, 100.0)),
            &mut sink,
        );

        let placements = sink.as_slice();
        assert_eq!(placements.len(), 2);
        let child = &placements[1];
        assert_eq!(
            child.rect,
            Rect::new(0.0, -40.0, 120.0, 400.0),
            "moved by exactly -offset on the scroll axis, full content extent otherwise"
        );
        assert_eq!(
            child.clip,
            Rect::new(0.0, 0.0, 120.0, 100.0),
            "clipped to the container's own viewport rect"
        );
    }

    #[test]
    fn a_stale_offset_past_the_content_end_clamps_instead_of_blanking() {
        let mut h = Harness::new();
        h.set_scroll("/list", 100_000.0);
        let tree = ViewNode::new(NodeKind::Scroll, "list").child(fixed_extent_child(400.0));
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 120.0, 100.0)),
            &mut sink,
        );

        let child = &sink.as_slice()[1];
        // max_offset = content_extent(400) - viewport_extent(100) = 300.
        assert_eq!(
            child.rect.y, -300.0,
            "clamped to content_extent - viewport_extent, not the stale offset"
        );
        assert!(child.rect.y.is_finite());
        assert!(child.rect.bottom() > 0.0, "the viewport is not left blank");
    }

    /// SC-008 seed: a 100 000-row collection in a 400-unit viewport with
    /// 24-unit rows must never materialize more than a small, bounded window
    /// of rows, at any scroll position — including deep into the list.
    #[test]
    fn bounded_materialization_holds_at_every_scroll_offset() {
        for offset in [0.0_f32, 5_000.0, 2_000_000.0] {
            let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
            h.set_scroll("/list", offset);
            let tree = scroll_with_collection(100_000, 24.0, 64.0);
            let mut path = KeyPath::root();
            let mut sink = PlacementList::new();
            crate::layout::place(
                &tree,
                &mut h.ctx(),
                &mut path,
                Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
                &mut sink,
            );

            assert!(
                h.rows.served < 100,
                "offset {offset}: served {} of 100 000 rows, expected a small bounded window",
                h.rows.served
            );
            assert!(
                h.rows.max_index_seen < 100_000,
                "offset {offset}: touched index {}, which is out of range",
                h.rows.max_index_seen
            );
        }
    }

    #[test]
    fn a_materialized_rows_id_is_stable_across_a_scroll() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        let tree = scroll_with_collection(100_000, 24.0, 64.0);
        let rect = Rect::new(0.0, 0.0, 200.0, 400.0);
        let id = "/list/rows/row-15";

        h.set_scroll("/list", 0.0);
        let mut path = KeyPath::root();
        let mut sink_a = PlacementList::new();
        crate::layout::place(&tree, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink_a);
        let a = sink_a
            .as_slice()
            .iter()
            .find(|p| p.id == id)
            .expect("row 15 is in the window at offset 0");

        h.set_scroll("/list", 240.0);
        let mut path = KeyPath::root();
        let mut sink_b = PlacementList::new();
        crate::layout::place(&tree, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink_b);
        let b = sink_b
            .as_slice()
            .iter()
            .find(|p| p.id == id)
            .expect("row 15 is still in the window at offset 240");

        assert_eq!(a.id, b.id);
        assert_ne!(
            a.rect, b.rect,
            "the same row still moves when the scroll offset changes"
        );
    }

    #[test]
    fn total_count_reaches_semantics_while_placement_count_is_only_the_window() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        h.set_scroll("/list", 0.0);
        let tree = scroll_with_collection(100_000, 24.0, 64.0);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        let placements = sink.as_slice();
        let collection = placements
            .iter()
            .find(|p| p.id == "/list/rows")
            .expect("the collection itself is placed");
        assert_eq!(collection.semantics.total_count, Some(100_000));

        // scroll (1) + collection (1) + materialized rows. At offset 0 with a
        // 400-unit viewport, 64-unit overscan, and 24-unit rows the window is
        // ceil((400 + 64) / 24) = 20 rows.
        let row_count = placements.len() - 2;
        assert_eq!(row_count, 20);
        assert!(row_count < 100_000);
    }

    #[test]
    fn a_row_extent_of_zero_falls_back_to_the_default_rather_than_dividing_by_zero() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 10));
        h.set_scroll("/list", 0.0);
        let tree = scroll_with_collection(10, 0.0, 8.0);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        // Must not panic, hang, or produce a NaN/infinite rect.
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
            &mut sink,
        );
        for placement in sink.as_slice() {
            assert!(placement.rect.x.is_finite());
            assert!(placement.rect.y.is_finite());
            assert!(placement.rect.w.is_finite());
            assert!(placement.rect.h.is_finite());
        }
    }

    #[test]
    fn axis_rect_keeps_the_cross_axis_origin_from_base() {
        let base = Rect::new(5.0, 7.0, 30.0, 40.0);
        let horizontal = super::axis_rect(base, Axis::Horizontal, 100.0, 20.0, 9.0);
        assert_eq!(horizontal, Rect::new(100.0, 7.0, 20.0, 9.0));
        let vertical = super::axis_rect(base, Axis::Vertical, 100.0, 20.0, 9.0);
        assert_eq!(vertical, Rect::new(5.0, 100.0, 9.0, 20.0));
    }
}
