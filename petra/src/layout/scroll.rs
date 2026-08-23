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
//! # The scroll ancestor owns the axis and the overscan
//! Both belong to the container that has a viewport and an offset, so both
//! are read off the `scroll`, never off the `collection`. [`place`] and
//! [`measure`] push a [`crate::layout::ScrollFrame`] around their child, and
//! [`place_collection`]/[`measure_collection`] read the innermost frame back
//! off the context. Tree acceptance refuses a `collection` that declares
//! `overscan`, or an `axis` that disagrees with its scroll's, so a
//! declaration is never silently overridden
//! (`crate::tree::Violation::ScrollParamOwnedByAncestor`).
//!
//! A `collection` with *no* `scroll` ancestor is legal and keeps its own
//! `axis` and `overscan`: nothing can scroll it, so nothing else can own
//! those values, and its window is simply whatever its clip shows.
//!
//! # Where the offset comes from
//! Nowhere, as a number. A `scroll` places its child at `-offset`, so by the
//! time a `collection` is placed the offset is already in its rect: the
//! distance from the collection's own rect to the clip in force is the
//! offset that reached it, composed across every container and every nested
//! scroll in between. That is why this module never looks an offset up by
//! id for anything but the `scroll`'s own placement, and why a stale entry
//! under a `stack`'s id cannot be mistaken for one.
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
use crate::layout::{LayoutCtx, Proposal, ScrollFrame, SizeProposal, Slot, semantics_of};
use crate::tree::props::{DEFAULT_ROW_EXTENT, ScrollProps};
use crate::tree::{KeyPath, ViewNode};

/// Measure this container under `proposal`.
///
/// `path` already names this node: the dispatcher pushed it. Child measurement
/// goes through [`crate::layout::measure`], which pushes the child's own key.
///
/// Padding insets the *viewport* this container reports, exactly as it insets
/// the actual clip region at `place` time (see `place`'s doc): the padded
/// space is never available to the scrolling content, on either axis, so a
/// shorter viewport is what a parent negotiates against. The proposal is
/// shrunk by the padding first — the same reserve-before-distribute shape
/// `stack.rs`'s `spacing` already uses (`layout-insets.md` §4) — and the
/// padding is added back to whatever this container ends up reporting, so an
/// offer smaller than the padding itself still answers at least the padding
/// (a scroll container can never be smaller than its own padding).
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let scroll = node.props.scroll();
    let axis = scroll.axis;
    let padding = ctx.padding(&node.props.padding);
    let Some(child) = node.children.first() else {
        // No content: nothing to negotiate against, and this container's
        // own declared `constraints` (applied centrally by the dispatcher
        // after this returns) are the only size signal left — never an echo
        // of whatever the parent offered. Echoing the proposal here (the
        // same shape the viewport axis uses below, once there *is* a child)
        // would turn a childless scroll into an elastic spacer that takes
        // whatever budget `distribute` offers it, which defeats the point
        // of a scroll region declaring its own minimum in a negotiation
        // (`layout_matrix.rs`'s `the_concession_order_is_slack_then_...`
        // fixture uses exactly this shape: an empty `Scroll` standing in
        // for not-yet-materialized content, relying on its own `min`, not
        // on echoing its offer, to hold a floor during `concede`).
        return Size::from_axes(axis, padding.along(axis), padding.along(axis.cross()));
    };

    let padded_proposal = SizeProposal {
        horizontal: proposal.horizontal.shrink(padding.along(Axis::Horizontal)),
        vertical: proposal.vertical.shrink(padding.along(Axis::Vertical)),
    };

    // The scrolling axis always probes the content's maximum useful extent,
    // regardless of what this container was itself offered on that axis —
    // that is the number a `Zero`/`Unbounded`/`Unspecified` proposal needs
    // below, and it is also how `place` later learns the content extent for
    // offset clamping. The cross axis passes the incoming (padded) offer
    // straight through, unchanged.
    let child_proposal = padded_proposal.with_axis(axis, Proposal::Unbounded);
    // Measured inside this container's own scroll frame: a `collection`
    // below answers a different size depending on which axis scrolls, and
    // that axis is this node's.
    let child_size = ctx.within_scroll(frame_for(node, path), |ctx| {
        crate::layout::measure(child, ctx, path, child_proposal)
    });

    // On the scrolling axis this container answers what its *viewport*
    // takes, not what its content takes: an `Exact` offer is honoured
    // exactly, `Zero` is the minimum, and only the two open probes reach for
    // the content extent just measured.
    let viewport_extent = match padded_proposal.axis(axis) {
        Proposal::Exact(v) => v.max(0.0),
        Proposal::Zero => 0.0,
        Proposal::Unbounded | Proposal::Unspecified => child_size.along(axis),
    };
    Size::from_axes(
        axis,
        viewport_extent + padding.along(axis),
        child_size.across(axis) + padding.along(axis.cross()),
    )
}

/// Place this container and everything under it into `slot`.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let id = path.id();
    let semantics = semantics_of(node, &id, ctx.state);
    let me = sink.push(Placement {
        id,
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: 0,
            truncated: false,
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics,
        parent: None,
    });
    sink.enter(me);

    if let Some(child) = node.children.first() {
        let scroll = node.props.scroll();
        let axis = scroll.axis;
        // The padded interior: this container's own placed rect (`slot.rect`,
        // pushed above, unmodified) never moves or shrinks for its own
        // padding — only what it offers its child does. Padding insets the
        // *viewport* here (`layout-insets.md` §3, §8 step 5), so `content`,
        // not `slot.rect`, is what every offset/clip computation below is
        // relative to: the padded strip is a permanent gutter the content
        // never enters, at either scroll extreme.
        let padding = ctx.padding(&node.props.padding);
        let content = slot.rect.inset_edges(padding);
        let viewport_size = content.size();

        // Same child-proposal shape as `measure`, but the cross axis is now
        // an `Exact` offer taken from the settled (padded) viewport rather
        // than whatever this container's own incoming proposal happened to
        // be: by `place` time negotiation is over and the viewport size is a
        // fact, not one of several offers being probed.
        let child_proposal =
            SizeProposal::exact(viewport_size).with_axis(axis, Proposal::Unbounded);
        let frame = frame_for(node, path);
        let content_size = ctx.within_scroll(frame.clone(), |ctx| {
            crate::layout::measure(child, ctx, path, child_proposal)
        });

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

        let main_origin = origin_along(content, axis) - offset;
        let child_rect = axis_rect(
            content,
            axis,
            main_origin,
            content_extent,
            content_size.across(axis),
        );
        // The child is placed at its full content size so ordinary children
        // (a stack of rows, say) lay out naturally; only the *paint* clip is
        // narrowed to this container's padded interior, which is what makes
        // overflow reachable-but-hidden rather than painted outside — and
        // keeps content from ever drawing over the padding gutter, at either
        // scroll extreme.
        let child_slot = Slot {
            rect: child_rect,
            z: slot.z,
            clip: slot.clip,
            opacity: slot.opacity,
        }
        .clipped_to(content);

        ctx.within_scroll(frame, |ctx| {
            crate::layout::place(child, ctx, path, child_slot, sink);
        });
    }

    sink.leave();
}

/// The scroll context this `scroll` node imposes on everything under it.
fn frame_for(node: &ViewNode, path: &KeyPath) -> ScrollFrame {
    ScrollFrame {
        id: path.id(),
        props: node.props.scroll(),
    }
}

/// The scrolling parameters in force for a `collection`.
///
/// The nearest enclosing `scroll` owns them. A `collection` outside every
/// scroll owns its own, and resolves them exactly as a `scroll` would.
fn collection_scroll(node: &ViewNode, ctx: &LayoutCtx<'_>) -> ScrollProps {
    ctx.enclosing_scroll()
        .map_or_else(|| node.props.scroll(), |frame| frame.props)
}

/// Measure a `collection` node: a virtualized row range read from a store-side
/// source through [`crate::layout::RowSource`].
pub fn measure_collection(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    _path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let Some(collection) = node.props.collection() else {
        return Size::ZERO;
    };
    // The scrolling axis decides which axis carries the row total and which
    // passes the offer through, so it has to be the same axis `place` will
    // lay the rows out along — the ancestor's, not this node's.
    let axis = collection_scroll(node, ctx).axis;
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
    let id = path.id();
    let semantics = semantics_of(node, &id, ctx.state);
    let me = sink.push(Placement {
        id,
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: 0,
            truncated: false,
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics,
        parent: None,
    });
    sink.enter(me);

    if let Some(collection) = node.props.collection() {
        let scroll = collection_scroll(node, ctx);
        let axis = scroll.axis;
        let row_extent = sane_row_extent(collection.estimated_extent);
        let total_extent = collection.total_count as f32 * row_extent;

        // `slot.rect` is this node's own placement: for a collection that is
        // the content-sized rect `measure_collection` answered (`place`
        // above positions a child at `-offset` and only narrows `clip` to
        // the viewport). The viewport is therefore `slot.clip` — the
        // narrowest region an ancestor has clipped this subtree to, which is
        // exactly the region a row can be seen in — never `slot.rect`.
        let viewport_extent = slot.clip.size().along(axis).max(0.0);

        // How far this list's own leading edge has been scrolled past the
        // visible region. Every enclosing scroll already moved `slot.rect`
        // by its offset, and every container in between already added its
        // own leading content, so this one subtraction is the composed
        // answer — see the module doc's "Where the offset comes from".
        let offset = origin_along(slot.clip, axis) - origin_along(slot.rect, axis);

        // A list scrolled entirely out of the visible region gets an empty
        // window (`window_end <= window_start`) and materializes nothing,
        // which is the honest answer: none of its rows can be seen.
        let window_start = (offset - scroll.overscan).max(0.0);
        let window_end = (offset + viewport_extent + scroll.overscan).min(total_extent);

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
    use crate::layout::{MeasureCache, Proposal, SizeProposal, Slot};
    use crate::testing::{GeneratedRows, Harness, MonoContent, gap_token};
    use crate::tree::{AxisConstraint, Constraints, InsetRefs, KeyPath, NodeKind, Props, ViewNode};

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

    /// A `scroll` named `key` carrying `props`, wrapping a `collection` named
    /// `rows` that declares nothing about scrolling itself.
    fn collection_in_scroll(
        key: &str,
        props: Props,
        total_count: usize,
        estimated_extent: f32,
    ) -> ViewNode {
        let collection = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(total_count),
            source: Some("fibers".into()),
            estimated_extent: Some(estimated_extent),
            ..Props::default()
        });
        ViewNode::new(NodeKind::Scroll, key)
            .with_props(props)
            .child(collection)
    }

    /// The `scroll`/`collection` pair every virtualization test drives.
    ///
    /// `overscan` is declared on the **`scroll`**, which owns it; tree
    /// acceptance refuses it on the collection. It is also never
    /// `DEFAULT_OVERSCAN` in a caller that asserts a row count: it used to
    /// be 64.0 everywhere, which *is* the default, so the count came out the
    /// same whether the declaration travelled or not.
    fn scroll_with_collection(
        total_count: usize,
        estimated_extent: f32,
        overscan: f32,
    ) -> ViewNode {
        collection_in_scroll(
            "list",
            Props {
                overscan: Some(overscan),
                ..Props::default()
            },
            total_count,
            estimated_extent,
        )
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

    /// The behaviour this leaf exists to add, pinned by name (gate G4).
    ///
    /// Padding insets the *viewport* a scroll offers/clips its content to —
    /// the content's own extent (the child's fixed 400.0) is untouched, and
    /// so is the scroll's own placed rect (padding is inside the box). Only
    /// what the scroll offers its child — the origin the content starts at,
    /// and the clip it is bounded by — shrinks to the padded interior.
    #[test]
    fn a_padded_scroll_insets_the_viewport_not_the_content_extent() {
        let mut h = Harness::new();
        let tree = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                padding: Some(InsetRefs::all(gap_token(10.0))),
                ..Props::default()
            })
            .child(fixed_extent_child(400.0));
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
        let scroll = &placements[0];
        let child = &placements[1];

        assert_eq!(
            scroll.rect,
            Rect::new(0.0, 0.0, 120.0, 100.0),
            "padding is inside the box: the scroll's own placed rect never \
             moves or shrinks for its own padding"
        );
        assert_eq!(
            child.rect,
            Rect::new(10.0, 10.0, 100.0, 400.0),
            "content keeps its full, unshrunk 400.0 extent, placed from the \
             padded (10.0, 10.0) origin — not the outer (0.0, 0.0) one"
        );
        assert_eq!(
            child.clip,
            Rect::new(10.0, 10.0, 100.0, 80.0),
            "the clip is the padded 100x80 viewport, not the outer 120x100 rect"
        );
    }

    /// The scroll gutter decision, both extremes: padding never scrolls away
    /// and never becomes visible content — the clip is identical whether the
    /// list is scrolled to the top or all the way to the bottom, and the
    /// content's near edge always sits flush with the padded rect's edge on
    /// that side. This is the case the module doc warns a wrong design "looks
    /// fine until content is longer than the viewport": a design that instead
    /// clipped to the *outer* rect would let the last row's bottom edge run
    /// past the padded interior and into the bottom gutter once scrolled all
    /// the way down, which this test would catch.
    #[test]
    fn a_padded_scroll_keeps_its_gutter_unbroken_at_both_scroll_extremes() {
        let place_at = |offset: f32| -> (Rect, Rect) {
            let mut h = Harness::new();
            h.set_scroll("/list", offset);
            let tree = ViewNode::new(NodeKind::Scroll, "list")
                .with_props(Props {
                    padding: Some(InsetRefs::all(gap_token(10.0))),
                    ..Props::default()
                })
                .child(fixed_extent_child(400.0));
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
            (child.rect, child.clip)
        };

        // Scrolled to the top (offset 0.0): the content's leading edge sits
        // exactly at the padded origin.
        let (top_rect, top_clip) = place_at(0.0);
        assert_eq!(
            top_rect.y, 10.0,
            "top gutter: content starts at the padded origin"
        );
        assert_eq!(top_clip, Rect::new(10.0, 10.0, 100.0, 80.0));

        // Scrolled to the bottom: max_offset = content 400.0 - viewport 80.0
        // = 320.0. The content's trailing edge lands exactly on the padded
        // rect's bottom edge (10.0 + 80.0 = 90.0), not the outer rect's
        // (100.0) — the bottom gutter is 10.0 wide, same as the top.
        let (bottom_rect, bottom_clip) = place_at(320.0);
        assert_eq!(bottom_rect.y, 10.0 - 320.0);
        assert_eq!(
            bottom_rect.y + bottom_rect.h,
            90.0,
            "content's trailing edge is flush with the padded rect's bottom \
             edge, not the outer rect's"
        );
        assert_eq!(
            bottom_clip, top_clip,
            "the clip never changes with scroll position: the gutter is a \
             fixed strip, not something the content can scroll into"
        );
    }

    /// `measure`'s open-probe (natural size) answer includes the padding,
    /// symmetrically on both axes, on top of whatever the content itself
    /// needs.
    #[test]
    fn a_padded_scrolls_natural_size_includes_its_own_padding() {
        let mut h = Harness::new();
        let tree = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                padding: Some(InsetRefs::all(gap_token(10.0))),
                ..Props::default()
            })
            .child(fixed_extent_child(400.0));
        let mut path = KeyPath::root();
        let size =
            crate::layout::measure(&tree, &mut h.ctx(), &mut path, SizeProposal::unspecified());
        // Content is 400.0 tall (fixed), 0.0 wide (an unconstrained spacer's
        // ideal is nothing) — plus 10.0 padding on every edge: 20.0 added to
        // each axis.
        assert_eq!(
            size,
            Size::new(20.0, 420.0),
            "the scroll's natural size is its content's extent plus its own padding"
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
    ///
    /// The window must also *be* at that scroll position. Counting rows is
    /// not enough on its own: a build that ignored the offset entirely and
    /// always drew the first screenful satisfied every bound below, so the
    /// first materialized index is asserted too.
    #[test]
    fn bounded_materialization_holds_at_every_scroll_offset() {
        // (offset, first row index). window_start = offset - 64 overscan,
        // floored into 24-unit rows: 0, floor(4936 / 24) = 205,
        // floor(1_999_936 / 24) = 83_330. The largest offset is still short
        // of the 2_399_600 the content allows, so none of them is clamped.
        for (offset, first_index) in [(0.0_f32, 0_usize), (5_000.0, 205), (2_000_000.0, 83_330)] {
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
            let placed: Vec<&str> = sink
                .as_slice()
                .iter()
                .filter_map(|p| p.id.strip_prefix("/list/rows/row-"))
                .collect();
            assert_eq!(
                placed.first().copied(),
                Some(first_index.to_string().as_str()),
                "offset {offset}: the window must sit at the offset, not at the top"
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

    /// The window is the viewport plus the *declared* overscan, not the
    /// default one.
    ///
    /// The overscan here is deliberately 120.0 rather than 64.0. Every caller
    /// of this fixture used to pass 64.0, which is `DEFAULT_OVERSCAN` — so the
    /// row count came out the same whether the declaration was read or
    /// ignored, and deleting the parameter left every scroll test green.
    /// Verified by sabotage: making `collection_scroll` ignore the enclosing
    /// frame's overscan fails this assertion.
    #[test]
    fn total_count_reaches_semantics_while_placement_count_is_only_the_window() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        h.set_scroll("/list", 0.0);
        let tree = scroll_with_collection(100_000, 24.0, 120.0);
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
        // 400-unit viewport, the declared 120-unit overscan, and 24-unit rows
        // the window is ceil((400 + 120) / 24) = 22 rows. With the default
        // 64-unit overscan it would be 20, which is what makes this assertion
        // able to tell the two apart.
        let row_count = placements.len() - 2;
        assert_eq!(row_count, 22);
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

    /// A `collection` with no `overscan` of its own must take the one its
    /// `scroll` ancestor declares. The window is the viewport plus that
    /// overscan at both ends.
    #[test]
    fn the_scrolls_declared_overscan_reaches_its_collection() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        h.set_scroll("/list", 0.0);
        let tree = collection_in_scroll(
            "list",
            Props {
                overscan: Some(120.0),
                ..Props::default()
            },
            100_000,
            24.0,
        );
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        // scroll (1) + collection (1) + rows. ceil((400 + 120) / 24) = 22
        // with the declared overscan; 20 with the 64-unit default.
        let row_count = sink.as_slice().len() - 2;
        assert_eq!(
            row_count, 22,
            "the window is the viewport plus the scroll's declared overscan, not the default"
        );
    }

    /// The offset that drives a collection is its `scroll` ancestor's, not
    /// whichever ancestor id happens to have an entry in the offset map.
    #[test]
    fn a_stale_offset_on_a_plain_ancestor_does_not_drive_the_collection() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        // `/root` is a `stack`. It is not a scroll container, and this entry
        // is stale. The `scroll` between it and the collection has never been
        // scrolled, so it has no entry at all.
        h.set_scroll("/root", 5_000.0);
        let tree = ViewNode::new(NodeKind::Stack, "root").child(collection_in_scroll(
            "list",
            Props::default(),
            100_000,
            24.0,
        ));
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        let ids: Vec<&str> = sink.as_slice().iter().map(|p| p.id.as_str()).collect();
        assert!(
            ids.contains(&"/root/list/rows/row-0"),
            "an unscrolled scroll shows its first row; placed: {ids:?}"
        );
        assert!(
            !ids.iter().any(|id| id.starts_with("/root/list/rows/row-2")
                && id.len() > "/root/list/rows/row-2".len()),
            "no row from the stale 5000-unit window is materialized; placed: {ids:?}"
        );
    }

    /// A `collection` lays its rows out along its `scroll` ancestor's axis.
    #[test]
    fn the_scrolls_axis_reaches_its_collection() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 1_000));
        h.set_scroll("/strip", 0.0);
        let tree = collection_in_scroll(
            "strip",
            Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            },
            1_000,
            24.0,
        );
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 400.0, 200.0)),
            &mut sink,
        );

        let rows: Vec<Rect> = sink
            .as_slice()
            .iter()
            .filter(|p| p.id.starts_with("/strip/rows/row-"))
            .map(|p| p.rect)
            .collect();
        assert!(rows.len() >= 2, "at least two rows are materialized");
        assert_eq!(rows[0].x, 0.0);
        assert_eq!(
            rows[1].x, 24.0,
            "rows advance along the scroll's horizontal axis"
        );
        assert_eq!(rows[0].y, rows[1].y, "rows share the cross-axis origin");
    }

    /// Nested scrolls: the *nearest* one drives the list, not the outermost
    /// one that happens to have an entry in the offset map.
    #[test]
    fn the_nearest_scroll_ancestor_wins_over_a_farther_one() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        h.set_scroll("/outer", 0.0);
        let inner = collection_in_scroll(
            "inner",
            Props {
                overscan: Some(0.0),
                ..Props::default()
            },
            100_000,
            24.0,
        );
        let tree = ViewNode::new(NodeKind::Scroll, "outer")
            .with_props(Props {
                overscan: Some(200.0),
                ..Props::default()
            })
            .child(inner);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        // outer (1) + inner (1) + collection (1) + rows. The inner scroll
        // declares no overscan at all, so the window is exactly the 400-unit
        // visible region: ceil(400 / 24) = 17 rows. The outer scroll's
        // 200-unit overscan would make it 25.
        let row_count = sink.as_slice().len() - 3;
        assert_eq!(
            row_count, 17,
            "the inner scroll's overscan is the one in force"
        );
    }

    /// A `collection` outside every `scroll` is legal, and keeps its own
    /// axis and overscan: nothing can scroll it, so nothing else owns them.
    #[test]
    fn a_collection_with_no_scroll_ancestor_keeps_its_own_axis_and_overscan() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 1_000));
        let tree = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(1_000),
            source: Some("fibers".into()),
            estimated_extent: Some(24.0),
            axis: Some(Axis::Horizontal),
            overscan: Some(96.0),
            ..Props::default()
        });
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 400.0, 200.0)),
            &mut sink,
        );

        let rows: Vec<Rect> = sink
            .as_slice()
            .iter()
            .filter(|p| p.id.starts_with("/rows/row-"))
            .map(|p| p.rect)
            .collect();
        // ceil((400 + 96) / 24) = 21 rows; the 64-unit default would give 20.
        assert_eq!(rows.len(), 21, "its own overscan is honoured");
        assert_eq!(rows[1].x, 24.0, "its own axis is honoured");
        assert_eq!(rows[0].y, rows[1].y);
    }

    /// A `surface` is anchored in viewport coordinates, so an ancestor
    /// `scroll` is not the scroll context of what is inside it.
    #[test]
    fn a_surface_starts_a_fresh_scroll_context() {
        use crate::tree::{Anchor, Layer};

        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 30));
        h.set_scroll("/list", 240.0);
        let collection = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(30),
            source: Some("fibers".into()),
            estimated_extent: Some(24.0),
            overscan: Some(96.0),
            ..Props::default()
        });
        let tree = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                overscan: Some(200.0),
                ..Props::default()
            })
            .child(
                ViewNode::new(NodeKind::Surface, "popup")
                    .with_props(Props {
                        layer: Some(Layer::Popup),
                        anchor: Some(Anchor::Viewport),
                        ..Props::default()
                    })
                    .child(collection),
            );
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        let ids: Vec<&str> = sink.as_slice().iter().map(|p| p.id.as_str()).collect();
        assert!(
            ids.contains(&"/list/popup/rows/row-0"),
            "the popup is anchored, not scrolled by /list; placed: {ids:?}"
        );
        // scroll (1) + surface (1) + collection (1) + rows. The popup is
        // clamped to the 400-unit window, and the collection keeps its own
        // 96-unit overscan: ceil((400 + 96) / 24) = 21 rows. The enclosing
        // scroll's 200-unit overscan would give 25.
        let row_count = sink.as_slice().len() - 3;
        assert_eq!(
            row_count, 21,
            "the enclosing scroll's overscan does not reach inside a surface"
        );
    }

    /// Scroll the whole 100 000-row list past the viewport and place every
    /// frame of it, then read the measurement cache.
    ///
    /// Returns `(entries, hits, misses, evictions)`.
    fn scroll_the_whole_list(capacity: usize) -> (usize, u64, u64, u64) {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        h.cache.set_capacity(capacity);
        let tree = scroll_with_collection(100_000, 24.0, 64.0);
        let rect = Rect::new(0.0, 0.0, 200.0, 400.0);
        // 5 000 frames a viewport apart covers 2 400 000 units, which is the
        // full 100 000 rows at 24 units each: every row is measured once.
        for frame in 0..5_000 {
            h.set_scroll("/list", frame as f32 * 480.0);
            let mut path = KeyPath::root();
            let mut sink = PlacementList::new();
            crate::layout::place(&tree, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink);
        }
        let (hits, misses) = h.cache.stats();
        (h.cache.len(), hits, misses, h.cache.evictions())
    }

    /// SC-008's memory clause: "memory that does not grow with total row
    /// count". The measurement cache is keyed per node id, so before it was
    /// bounded this run left one entry per row ever scrolled past — 100 001
    /// entries for 100 000 rows, climbing by 20 every frame.
    #[test]
    fn scrolling_a_whole_collection_leaves_the_measure_cache_bounded() {
        let capacity = MeasureCache::DEFAULT_CAPACITY;
        let (entries, _, misses, evictions) = scroll_the_whole_list(capacity);
        assert!(
            entries <= capacity,
            "cache holds {entries} entries, over its {capacity}-entry bound"
        );
        assert!(
            misses > 100_000,
            "the run must actually measure the whole list, not a corner of it: \
             {misses} misses"
        );
        assert!(
            evictions > 0,
            "nothing was evicted, so this run never reached the bound and \
             proves nothing"
        );
    }

    /// Bounding the cache must not cost a hit: the working set of one frame,
    /// and of the frame before it, is orders of magnitude below the default
    /// bound, so the LRU only ever drops rows that have left the window.
    ///
    /// The control is the same run with the bound effectively removed. Equal
    /// hit and miss counts is the strongest available statement — not "the
    /// rate is still good", but "eviction changed nothing except memory".
    #[test]
    fn the_default_bound_costs_no_cache_hits() {
        let bounded = scroll_the_whole_list(MeasureCache::DEFAULT_CAPACITY);
        let unbounded = scroll_the_whole_list(usize::MAX);
        assert_eq!(
            (bounded.1, bounded.2),
            (unbounded.1, unbounded.2),
            "bounded (hits, misses) must equal unbounded's"
        );
        assert!(bounded.1 > 0, "a run with no hits would prove nothing");
        assert!(
            bounded.0 <= MeasureCache::DEFAULT_CAPACITY,
            "bounded run holds {} entries",
            bounded.0
        );
        assert!(
            unbounded.0 > 100_000,
            "the control must show the growth this bound removes: {} entries",
            unbounded.0
        );
        assert_eq!(unbounded.3, 0, "the control must not have evicted anything");
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
