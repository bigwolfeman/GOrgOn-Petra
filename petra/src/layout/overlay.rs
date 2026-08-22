//! The overlay container: every child gets the container's proposal; z-order is child order unless declared.
//!
//! `contracts/view-tree.md`'s grid/overlay/scroll rule says the whole of it:
//! "every child receives the container's own proposal; z-order is child
//! order unless declared." There is no distribution to negotiate — every
//! child is asked the identical question the overlay itself was asked, and
//! answers for itself — so this module is mostly about the z-order half: a
//! child with no declared `props.z` needs its paint order set from its
//! position in `children` right here, because [`crate::layout::place`] (the
//! dispatcher every child goes through) only ever applies a *declared*
//! `props.z` on its own (`layout/mod.rs`'s `place` wrapper). Applying the
//! index-derived `z` there too, for a child that already declared one, would
//! stack the index on top of the declaration instead of falling back to it.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::Size;
use crate::layout::{LayoutCtx, SizeProposal, Slot, semantics_of};
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
    // Every child answers the same `proposal` the overlay itself was asked
    // (there is no cross-axis or budget to split, unlike a stack or a grid),
    // and the overlay reports the per-axis max of those answers — a `Zero`
    // probe answers the max of the children's `Zero` responses, an
    // `Unbounded` probe the max of their `Unbounded` responses, and so on.
    let mut w = 0.0f32;
    let mut h = 0.0f32;
    for child in &node.children {
        let size = crate::layout::measure(child, ctx, path, proposal);
        w = w.max(size.w);
        h = h.max(size.h);
    }
    Size::new(w, h)
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

    for (i, child) in node.children.iter().enumerate() {
        // A child with its own declared `z` gets it from the dispatcher's
        // `place` wrapper the moment we hand the child to
        // `crate::layout::place` below — that is the "if present" half of
        // the rule, and applying an index on top of it here would turn a
        // declared `z` into `z + index` instead of leaving it exactly `z`.
        // A child with none gets its order from `i` right here, since
        // nothing downstream will ever supply it otherwise.
        let child_slot = if child.props.z.is_none() {
            slot.above(i as i32)
        } else {
            slot
        };
        crate::layout::place(child, ctx, path, child_slot, sink);
    }

    sink.leave();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::placement::PlacementList;
    use crate::geom::Rect;
    use crate::testing::Harness;
    use crate::tree::{AxisConstraint, NodeKind};

    fn overlay(children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Overlay, "o").with_children(children)
    }

    fn spacer(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, key)
    }

    fn text(key: &str, s: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key).with_props(crate::tree::Props {
            text: Some(s.to_owned()),
            ..crate::tree::Props::default()
        })
    }

    /// Mirrors how the dispatcher in `layout/mod.rs` would have arrived here:
    /// it pushes the node's own key before calling into the kind module.
    fn path_at(node: &ViewNode) -> KeyPath {
        let mut path = KeyPath::root();
        path.push(node.key.clone());
        path
    }

    /// Pin one axis constraint to a single value, so a node's measured
    /// response along that axis is the same constant no matter what raw
    /// answer its content gave — the trick the size-max test below uses to
    /// make two children's answers differ identically across every probe
    /// kind, including `Unbounded`, without depending on shaped text.
    fn pinned(value: f32) -> AxisConstraint {
        AxisConstraint {
            min: Some(value),
            max: Some(value),
            priority: 0,
        }
    }

    #[test]
    fn every_child_gets_the_containers_rect() {
        let node = overlay(vec![spacer("a"), spacer("b"), text("c", "hi there")]);
        let mut h = Harness::new();
        let mut path = path_at(&node);
        let mut sink = PlacementList::new();
        let rect = Rect::new(10.0, 20.0, 300.0, 150.0);
        place(&node, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink);

        assert_eq!(sink.len(), 4); // the overlay itself plus its three children
        for placement in &sink.as_slice()[1..] {
            assert_eq!(placement.rect, rect, "{}", placement.id);
        }
    }

    /// Overlapping is the point, not a bug: unlike a stack, every child of an
    /// overlay is placed into the identical rect its siblings get, on
    /// purpose. The no-overlap property a stack's tests assert would be a
    /// false claim here, so this proves the opposite instead of merely
    /// omitting the stack's check.
    #[test]
    fn children_are_allowed_to_fully_overlap_by_design() {
        let node = overlay(vec![spacer("a"), spacer("b")]);
        let mut h = Harness::new();
        let mut path = path_at(&node);
        let mut sink = PlacementList::new();
        place(
            &node,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 50.0, 50.0)),
            &mut sink,
        );
        let (a, b) = (sink.as_slice()[1].rect, sink.as_slice()[2].rect);
        assert_eq!(a, b);
        assert!(a.overlaps(b));
    }

    #[test]
    fn declared_z_wins_absent_z_falls_back_to_child_index_and_ties_hold_child_order() {
        // Run the whole build-and-place cycle 100 times: nothing in this
        // module consults a hash map or anything else whose iteration order
        // could vary between runs, and this is what proves it rather than
        // assuming it.
        for _ in 0..100 {
            let mut c1 = spacer("c1");
            c1.props.z = Some(5);
            let mut c3 = spacer("c3");
            c3.props.z = Some(5); // ties c1 on declared z
            let node = overlay(vec![spacer("c0"), c1, spacer("c2"), c3, spacer("c4")]);

            let mut h = Harness::new();
            let mut path = path_at(&node);
            let mut sink = PlacementList::new();
            place(
                &node,
                &mut h.ctx(),
                &mut path,
                Slot::new(Rect::new(0.0, 0.0, 10.0, 10.0)),
                &mut sink,
            );

            let placements = &sink.as_slice()[1..];
            let zs: Vec<i32> = placements.iter().map(|p| p.z).collect();
            // c0, c2, c4 have no declared z: their z is their index in
            // `children`. c1 and c3 declared 5 and keep it verbatim.
            assert_eq!(zs, vec![0, 5, 2, 5, 4]);

            // The tie between c1 and c3 (both z = 5) resolves to child
            // order: placements arrive in declaration order regardless of z,
            // which is what `Placement::z`'s doc means by "ties break on
            // placement order".
            let ids: Vec<&str> = placements.iter().map(|p| p.id.as_str()).collect();
            assert_eq!(ids, vec!["/o/c0", "/o/c1", "/o/c2", "/o/c3", "/o/c4"]);
        }
    }

    /// The overlay's measured size is the per-axis max of its children's
    /// answers, for every one of the four probe kinds. Pinning each child's
    /// response with equal `min`/`max` constraints (see [`pinned`]) makes
    /// the two children disagree identically no matter which probe produced
    /// the raw answer — including `Unbounded`, where an unconstrained spacer
    /// would otherwise report the same enormous extent as every other
    /// spacer and hide whether the max is really per-child or just a
    /// coincidence of both children agreeing.
    #[test]
    fn measured_size_is_the_per_axis_max_of_children_for_every_probe_kind() {
        let mut a = spacer("a");
        a.constraints.horizontal = pinned(80.0);
        a.constraints.vertical = pinned(5.0);
        let mut b = spacer("b");
        b.constraints.horizontal = pinned(20.0);
        b.constraints.vertical = pinned(40.0);
        let node = overlay(vec![a, b]);

        let mut h = Harness::new();
        for proposal in [
            SizeProposal::zero(),
            SizeProposal::unbounded(),
            SizeProposal::unspecified(),
            SizeProposal::exact(Size::new(500.0, 500.0)),
        ] {
            let mut path = path_at(&node);
            let size = measure(&node, &mut h.ctx(), &mut path, proposal);
            assert_eq!(
                size,
                Size::new(80.0, 40.0),
                "{proposal:?} should answer the per-axis max of the children, \
                 not one child's whole size"
            );
        }
    }
}
