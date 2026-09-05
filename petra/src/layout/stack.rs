//! The stack container: priority groups, flexibility-ascending equal share,
//! surplus roll-forward, and roll-back of whatever that leaves unspent.
//!
//! This file is a transcription of the binding algorithm in
//! `contracts/view-tree.md` §3, in its order, and the order is the whole
//! point. Two things are easy to get wrong and both are load-bearing:
//!
//! * **Negotiation order is not placement order.** Distribution walks children
//!   by priority group and then by ascending flexibility; placement always
//!   walks declaration order. A stack that let the negotiation order reach the
//!   cursor would reorder a toolbar whenever an author changed a `min`.
//! * **Surplus rolls forward.** A child offered an equal share that takes less
//!   than its share leaves the difference in the pot for the next, *more*
//!   flexible child. Dropping that is what turns "distribute 300 units" into
//!   "hand out 300 units per child", and it is why the group is walked from
//!   least flexible to most: the rigid answers land before the pot is split.
//! * **Surplus also rolls back.** Rolling forward only ever reaches children
//!   the walk has not visited yet, so a child limited by its own equal share
//!   while a *later* sibling declined room stayed small in a row that had the
//!   space. Whatever the walk leaves over is offered back round to the
//!   children that were limited by their offer, until it is gone or nobody
//!   can use it. [`settle_group`] states the whole policy, shrink included.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::{Align, Axis, Rect, Size};
use crate::layout::constraints::{FIT_EPSILON, Give, concede};
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot, semantics_of};
use crate::tree::{KeyPath, NodeKind, ViewNode};

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
    let main = node.props.axis.unwrap_or(Axis::Vertical);
    if node.children.is_empty() {
        return Size::ZERO;
    }
    // Padding is reserved before anything is distributed, the same way
    // `spacing` already is (`contracts/view-tree.md` negotiation item 3.d,
    // extended one level up by `.agents/research/08-22-2026/Petra-Visual-Design/
    // engine/layout-insets.md` §4): it is pure arithmetic on the proposal here,
    // never a `Give` `concede` can raid. `Proposal::shrink` passes an open
    // probe through unchanged, so `Zero`/`Unbounded` measurement still answers
    // truthfully about the padded budget, not the whole rect.
    let padding = ctx.padding(&node.props.padding);
    let spacing = reserved_spacing(ctx.spacing(&node.props.spacing), node.children.len());
    let cross = proposal
        .axis(main.cross())
        .shrink(padding.along(main.cross()));
    let main_proposal = proposal.axis(main).shrink(padding.along(main));

    let Some(budget) = main_proposal.exact() else {
        // An open or minimum probe is answered by asking every child the same
        // question. There is nothing to distribute: the parent has not said
        // how much room there is, so the stack reports the sum of the answers
        // and lets the parent decide.
        let probe = main_proposal;
        let mut total = spacing;
        let mut widest = 0.0f32;
        for child in &node.children {
            let got = crate::layout::measure(child, ctx, path, offer(main, probe, cross));
            total += got.along(main);
            widest = widest.max(got.across(main));
        }
        return Size::from_axes(
            main,
            total + padding.along(main),
            widest + padding.along(main.cross()),
        );
    };

    let taken = distribute(node, ctx, path, main, (budget - spacing).max(0.0), cross).taken;
    // Deliberately not clamped to the offer: a response bigger than the parent
    // can place is the parent's problem to concede at placement, and clamping
    // it here would hide the overflow from the flag the contract requires.
    let total = spacing + taken.iter().map(|s| s.along(main)).sum::<f32>();
    let widest = taken.iter().fold(0.0f32, |acc, s| acc.max(s.across(main)));
    Size::from_axes(
        main,
        total + padding.along(main),
        widest + padding.along(main.cross()),
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
    let main = node.props.axis.unwrap_or(Axis::Vertical);
    let align = node.props.align.unwrap_or_default();
    let justify = node.props.justify.unwrap_or_default();
    // Padding is inside the box: the stack's own placed rect (pushed below,
    // `rect: slot.rect`) never moves or shrinks because of its own padding —
    // only what it offers its children does. `content` floors at zero rather
    // than going negative (`Rect::inset_edges`), the same floor `spacing`
    // already applies to itself a few lines down, and a genuine collapse
    // (the box smaller than its own declared insets) is flagged exactly the
    // way a spacing collapse already is, via `padding_collapsed` below.
    let padding = ctx.padding(&node.props.padding);
    let padding_collapsed = (slot.rect.w - padding.left - padding.right) < -FIT_EPSILON
        || (slot.rect.h - padding.top - padding.bottom) < -FIT_EPSILON;
    let content = slot.rect.inset_edges(padding);
    let main_extent = content.size().along(main).max(0.0);
    let cross_extent = content.size().across(main).max(0.0);
    let gaps = node.children.len().saturating_sub(1) as f32;
    let declared = ctx.spacing(&node.props.spacing).max(0.0);
    // Spacing is reserved before distribution, but it cannot reserve room the
    // container does not have. A rect too small to hold its own gaps would
    // otherwise push the last child clean outside its parent, where nothing
    // can reach it — which the contract forbids outright, and no amount of
    // conceding child extents can undo.
    let gap = if gaps > 0.0 && declared * gaps > main_extent {
        main_extent / gaps
    } else {
        declared
    };
    let spacing = gap * gaps;

    // The cross axis is settled by the time a stack places: the slot says how
    // much room there is, so children are asked the exact question rather than
    // the one the stack's own parent asked.
    let cross = Proposal::Exact(cross_extent);
    let plan = distribute(
        node,
        ctx,
        path,
        main,
        (main_extent - spacing).max(0.0),
        cross,
    );

    let mut extents: Vec<f32> = plan.taken.iter().map(|s| s.along(main)).collect();
    let wanted = spacing + extents.iter().sum::<f32>();
    let mut lost = padding_collapsed
        || gap < declared - FIT_EPSILON
        || plan
            .taken
            .iter()
            .any(|s| s.across(main) > cross_extent + FIT_EPSILON);

    if wanted > main_extent + FIT_EPSILON {
        let mut gives: Vec<Give> = (0..extents.len())
            .map(|i| {
                let give = Give::new(i, extents[i], plan.floors[i], plan.flexibility[i]);
                if scroll_absorbs(&node.children[i], main) {
                    give.absorbing(0.0)
                } else {
                    give
                }
            })
            .collect();
        let outcome = concede(&mut gives, wanted - main_extent);
        for g in &gives {
            extents[g.index] = g.extent;
        }
        lost |= outcome.truncated();
    }

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
            // Set here rather than on the child: the stack is what ran out of
            // room, and the child's own placement records only what its own
            // content lost.
            truncated: lost,
            // Never the stack: `overflowed` is "the content is bigger than
            // the rect it was given", and only the leaf that owns the content
            // can measure that. A stack that ran out of room says so with
            // `truncated` above, which is the FR-005 concession it actually
            // performed.
            overflowed: false,
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics,
        parent: None,
    });
    sink.enter(me);

    // Declaration order, always. `plan` was negotiated in another order
    // entirely and indexes back into this one.
    //
    // The cursor is an **absolute** coordinate on the main axis, not an offset
    // from the slot. That is load-bearing, not a style choice. A relative
    // cursor makes child `i`'s trailing edge `(slot.origin + cursor) + extent`
    // while child `i + 1`'s leading edge is `slot.origin + (cursor + extent)`.
    // Those are one number in exact arithmetic and two `f32` values about one
    // adjacent pair in twenty thousand — and `crate::frame::rounding` then
    // rounds the two sides of one seam to two different device pixels, which
    // is a one-pixel gap or a one-pixel overlap between rows that touch. It
    // defeats the whole reason `round_rect` rounds four edges rather than an
    // origin and a size, and it is not a fractional-scale-only defect: a
    // six-row column at 1.0 scale inside a container starting at y = 1.0
    // shows it. Accumulating absolutely makes the next leading edge the same
    // expression as this trailing edge, bit for bit, whenever the gap is zero
    // — and where the gap is not zero the rects do not abut, so there is no
    // seam to close. `gorgon/petra/tests/layout_matrix.rs`'s
    // `abutting_rows_share_a_device_edge_from_a_shifted_origin` pins it.
    // `justify` only ever moves this starting point, or — `SpaceBetween` —
    // grows every gap by the same `spread`. It never changes how much room
    // a child got — `extents` is already final, concede included — so a
    // container that ran out of room (`final_wanted >= main_extent`)
    // computes a leading offset and a spread of zero either way
    // (`Justify::offset` and `Justify::spread` floor at zero) and packs
    // exactly as it always did.
    let final_wanted = spacing + extents.iter().sum::<f32>();
    let leading = justify.offset(main_extent, final_wanted);
    let spread = justify.spread(main_extent, final_wanted, gaps);
    let mut cursor = match main {
        Axis::Horizontal => content.x + leading,
        Axis::Vertical => content.y + leading,
    };
    for (i, child) in node.children.iter().enumerate() {
        let extent = extents[i];
        // `align_self` overrides the container's own `align` for this one
        // child (CSS `align-self`); absent, the container's `align` decides,
        // which is the only behaviour a `stack` had before the field existed.
        let child_align = child.props.align_self.unwrap_or(align);
        let across = match child_align {
            // Stretch fills the cross extent, but a declared maximum on the
            // child's own cross axis still wins (2026-08-22: constraints beat
            // Stretch, see `.agents/tallies/QUESTIONS.md` Round 3 item 2 and
            // `.agents/notes/implemented/bug-fix/2026-08-22-petra-stretch-honours-constraints.md`).
            // This routes through `AxisConstraint::clamp` — the crate's one
            // clamp function, also reached via `Constraints::clamp_size` from
            // `crate::layout::measure` — rather than a second, hand-rolled
            // `.min(max)` here that could drift from it. `plan.taken[i]` is
            // not the right input: an intrinsically-sized child (an `Image`)
            // answers its own size regardless of the cross offer, and Stretch
            // must override that, not read it back.
            Align::Stretch => child
                .constraints
                .axis(main.cross())
                .clamp(cross_extent)
                // A declared minimum bigger than the cross extent is honoured
                // in the response (the clamp above), but placement never
                // grows past what the parent actually has — same rule the
                // non-Stretch arm already applies below, and `plan.taken`
                // already flagged the loss as `truncated`.
                .min(cross_extent),
            _ => plan.taken[i].across(main).min(cross_extent),
        };
        let offset = child_align.offset(cross_extent, across);
        let rect = match main {
            Axis::Horizontal => Rect::new(cursor, content.y + offset, extent, across),
            Axis::Vertical => Rect::new(content.x + offset, cursor, across, extent),
        };
        crate::layout::place(child, ctx, path, slot.with_rect(rect), sink);
        cursor += extent + gap + spread;
    }

    sink.leave();
}

/// What one distribution pass settled on, indexed by declaration order.
struct Plan {
    /// Each child's response to the offer it was finally given.
    taken: Vec<Size>,
    /// Each child's `Zero` response on the main axis: the extent below which a
    /// concession starts costing content.
    floors: Vec<f32>,
    /// Each child's `Unbounded` minus `Zero` response on the main axis.
    flexibility: Vec<f32>,
}

/// Run the binding algorithm over `node`'s children for `budget` main-axis
/// units, spacing already reserved.
fn distribute(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    main: Axis,
    budget: f32,
    cross: Proposal,
) -> Plan {
    let n = node.children.len();
    let mut floors = Vec::with_capacity(n);
    let mut flexibility = Vec::with_capacity(n);
    // The `Unbounded` response itself, kept beside the flexibility derived
    // from it: the roll-back caps its offers with it, and reconstructing it as
    // `floors + flexibility` would not survive the `max(0.0)` on that line.
    let mut highs = Vec::with_capacity(n);
    let mut priorities = Vec::with_capacity(n);
    for child in &node.children {
        // Both bounds, up front: step (b) needs every lower-priority child's
        // minimum before the first group may spend anything.
        let low = crate::layout::measure(child, ctx, path, offer(main, Proposal::Zero, cross))
            .along(main);
        let high =
            crate::layout::measure(child, ctx, path, offer(main, Proposal::Unbounded, cross))
                .along(main);
        floors.push(low);
        flexibility.push((high - low).max(0.0));
        highs.push(high);
        priorities.push(child.constraints.axis(main).priority);
    }

    // Highest priority first. The sort is stable, so children inside one group
    // stay in declaration order — which is the tie-break the flexibility sort
    // below then inherits.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(priorities[i]));

    // Suffix sums over that order: `reserved[k]` is the sum of the `Zero`
    // responses of every child at or after position k, so a group at [k, end)
    // reads `reserved[end]` and sees exactly the lower-priority minimums it
    // must leave behind. Sorting inside a group permutes only [k, end) and
    // cannot change the sums past it.
    let mut reserved = vec![0.0f32; n + 1];
    for k in (0..n).rev() {
        reserved[k] = reserved[k + 1] + floors[order[k]];
    }

    let mut taken = vec![Size::ZERO; n];
    let mut offers = Offers {
        node,
        ctx,
        path,
        main,
        cross,
    };
    let mut pot = budget;
    let mut k = 0;
    while k < n {
        let priority = priorities[order[k]];
        let mut end = k;
        while end < n && priorities[order[end]] == priority {
            end += 1;
        }
        let group = &mut order[k..end];
        // Ascending flexibility: the least flexible child answers first, so
        // whatever it declines is still in the pot when the children that can
        // actually use it are offered their share.
        group.sort_by(|&a, &b| flexibility[a].total_cmp(&flexibility[b]));

        let spent = settle_group(
            &mut offers,
            group,
            (pot - reserved[end]).max(0.0),
            &highs,
            &mut taken,
        );
        pot = (pot - spent).max(0.0);
        k = end;
    }

    Plan {
        taken,
        floors,
        flexibility,
    }
}

/// Everything one child offer needs except the child and the number.
///
/// Bundled so [`settle_group`] can be a five-argument function rather than a
/// nine-argument one, and so both of its passes reach a child through the same
/// [`Offers::measure`] rather than through two hand-written calls that could
/// drift in what they offer on the cross axis.
struct Offers<'a, 'ctx> {
    node: &'a ViewNode,
    ctx: &'a mut LayoutCtx<'ctx>,
    path: &'a mut KeyPath,
    main: Axis,
    cross: Proposal,
}

impl Offers<'_, '_> {
    /// Offer child `index` exactly `extent` on the main axis, and answer what
    /// it takes.
    fn measure(&mut self, index: usize, extent: f32) -> Size {
        crate::layout::measure(
            &self.node.children[index],
            self.ctx,
            self.path,
            offer(self.main, Proposal::Exact(extent), self.cross),
        )
    }
}

/// Hand one priority group `avail` main-axis units, write each child's
/// response into `taken` at its declaration index, and answer what the group
/// spent.
///
/// # What a child gets, exactly
///
/// **Pass one** is `contracts/view-tree.md` step 3.c verbatim: walk the group
/// least flexible first, offer each child an equal share of what is left when
/// the walk reaches it, and let a child that takes less than its share roll
/// the difference forward to the next, more flexible child.
///
/// **Pass two onwards** is the roll-*back*, `contracts/view-tree.md` step
/// 3.c-bis. Roll-forward only ever reaches children the walk has
/// not visited yet, so a child that was limited by its own share while a later
/// sibling declined room ends up smaller than the row had space for. That is
/// not a hypothetical: a row measures under `Unbounded`, its parent places it
/// at exactly the width that answer named, and the widest child — which, if it
/// declares a minimum, is also the *least* flexible and therefore first in the
/// walk — is handed the mean of the row's naturals instead of its own. A
/// 104-unit label in a row of five 88-unit ones was placed at 91.2 and broke
/// mid-word, in a row that had been sized for all of them.
///
/// So: whatever the walk leaves unspent is offered back round to the children
/// that have **not reached their own ceiling** — their `Unbounded` response,
/// the largest extent they said they could use — in the same
/// least-flexible-first order, each getting an equal share of the leftover on
/// top of what it already holds. Repeat until the leftover is gone or nobody
/// gains. A child already at its ceiling is never asked, which is what keeps
/// the roll-back from eating the slack `justify` spends: three 64-unit images
/// in a 400-unit `SpaceBetween` row are all at their ceiling after the walk,
/// so the 208 units they left stay in the gaps.
///
/// # The shrink policy, stated
///
/// When the group's children want more than `avail`, they shrink to an **equal
/// share of the budget**, not proportionally to their natural sizes: three
/// children in a row too narrow for them end at a third each, whatever their
/// naturals were. Declared minimums and priority both outrank that — a lower
/// group's minimums are reserved out of `avail` before this function sees it,
/// and a child's own `min` clamps its response above its share, which the
/// container then reports as overflow rather than hiding. Shrinking never
/// happens while budget is unspent, which is the whole content of the
/// roll-back above.
///
/// Both halves are pinned:
/// `a_row_placed_at_the_width_it_asked_for_gives_every_child_what_it_asked_for`
/// for the first, `a_row_too_narrow_for_its_children_shrinks_them_to_an_equal_share`
/// for the second.
fn settle_group(
    o: &mut Offers<'_, '_>,
    group: &[usize],
    avail: f32,
    highs: &[f32],
    taken: &mut [Size],
) -> f32 {
    let mut extents = vec![0.0f32; group.len()];
    let mut remaining = avail;
    let mut left = group.len();
    for (g, &i) in group.iter().enumerate() {
        let share = remaining / left as f32;
        left -= 1;
        let got = o.measure(i, share);
        taken[i] = got;
        extents[g] = got.along(o.main);
        // Surplus roll-forward.
        remaining = (remaining - extents[g]).max(0.0);
    }

    // The roll-back. A child is out of the queue once it has said no: either
    // it is already at the largest extent it declared it could use, or a
    // re-offer bought it nothing. Every pass therefore removes a child or
    // spends the leftover, so `group.len()` passes is a bound the loop
    // reaches only if it is about to stop anyway — and a layout pass must
    // terminate whatever a child answers.
    let mut done = vec![false; group.len()];
    for _ in 0..group.len() {
        // Recomputed from a fresh sum rather than carried on from the walk:
        // the running subtraction above is one rounding deep per child by the
        // time it gets here.
        remaining = (avail - extents.iter().sum::<f32>()).max(0.0);
        if remaining <= FIT_EPSILON {
            break;
        }
        let claimants: Vec<usize> = (0..group.len())
            .filter(|&g| !done[g] && extents[g] < highs[group[g]] - FIT_EPSILON)
            .collect();
        if claimants.is_empty() {
            break;
        }
        let mut left = claimants.len();
        let mut gained = false;
        for &g in &claimants {
            let high = highs[group[g]];
            let want = extents[g] + remaining / left as f32;
            left -= 1;
            // The ceiling is a snap, not a `min`, and that is the point. A
            // hand-back is a sum of `f32` shares and lands a few parts in ten
            // million either side of the ceiling it was built from, which is
            // how a row placed at exactly the 456 units it answered put a
            // 104-unit label at 103.999985 and rounded it to a different
            // device pixel than the answer did. `FIT_EPSILON` is already this
            // file's definition of "the same extent"; an offer that close to
            // a child's ceiling *is* that ceiling.
            let ask = if want >= high - FIT_EPSILON {
                high
            } else {
                want
            };
            if ask - extents[g] <= FIT_EPSILON {
                done[g] = true;
                continue;
            }
            let got = o.measure(group[g], ask);
            let extent = got.along(o.main);
            if extent < ask - FIT_EPSILON {
                // It declined the bigger offer, so it will decline the next
                // one too. Take the answer — it is the response to an offer
                // this pass is committing — and stop asking.
                done[g] = true;
            }
            if extent - extents[g] <= FIT_EPSILON {
                done[g] = true;
                continue;
            }
            taken[group[g]] = got;
            remaining = (remaining - (extent - extents[g])).max(0.0);
            extents[g] = extent;
            gained = true;
        }
        if !gained {
            break;
        }
    }

    extents.iter().sum()
}

/// Spacing reserved before distribution: one gap between each adjacent pair.
fn reserved_spacing(spacing: f32, children: usize) -> f32 {
    spacing.max(0.0) * children.saturating_sub(1) as f32
}

/// A per-axis offer built from a main-axis and a cross-axis proposal.
fn offer(axis: Axis, main: Proposal, cross: Proposal) -> SizeProposal {
    SizeProposal::both(cross).with_axis(axis, main)
}

/// Whether this child is a scroll region along the axis being negotiated, and
/// so may absorb overflow instead of losing content to it.
fn scroll_absorbs(child: &ViewNode, main: Axis) -> bool {
    child.kind == NodeKind::Scroll && child.props.scroll().axis == main
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::frame::placement::{Placement, PlacementList};
    use crate::geom::{Align, Axis, Insets, Rect, Size};
    use crate::layout::{SizeProposal, Slot};
    use crate::testing::{Harness, gap};
    use crate::tree::{
        AxisConstraint, Constraints, Justify, Key, KeyPath, NodeKind, Props, ViewNode,
    };

    /// Constraints that touch one axis only, so a test says what it means
    /// about the main axis without accidentally pinning the cross axis.
    fn on(axis: Axis, c: AxisConstraint) -> Constraints {
        match axis {
            Axis::Horizontal => Constraints {
                horizontal: c,
                vertical: AxisConstraint::default(),
            },
            Axis::Vertical => Constraints {
                horizontal: AxisConstraint::default(),
                vertical: c,
            },
        }
    }

    /// A spacer clamped to `[min, max]` on `axis`: it answers `Zero` with
    /// `min`, `Unbounded` with `max`, and `Exact(v)` with `v` clamped into the
    /// band. That makes its flexibility exactly `max - min`.
    fn flexible(key: &str, axis: Axis, min: f32, max: f32, priority: i32) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, Key::new(key)).with_constraints(on(
            axis,
            AxisConstraint {
                min: Some(min),
                max: Some(max),
                priority,
            },
        ))
    }

    /// An image: 64 by 64 whatever it is offered. The child that refuses to
    /// shrink, which is the case the concession order exists for.
    fn rigid(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Image, Key::new(key)).with_props(Props {
            image: Some("icon".into()),
            ..Props::default()
        })
    }

    fn stack(axis: Axis, spacing: f32, align: Align, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Stack, Key::new("stack"))
            .with_props(Props {
                axis: Some(axis),
                spacing: gap(spacing),
                align: Some(align),
                ..Props::default()
            })
            .with_children(children)
    }

    /// Same as [`stack`], with `padding` declared — its own dimension, kept
    /// out of `stack()` so every existing test above stays exactly the
    /// zero-padding fixture it always was.
    fn stack_with_padding(
        axis: Axis,
        padding: Insets,
        align: Align,
        children: Vec<ViewNode>,
    ) -> ViewNode {
        ViewNode::new(NodeKind::Stack, Key::new("stack"))
            .with_props(Props {
                axis: Some(axis),
                align: Some(align),
                padding: Some(crate::testing::gap_insets(padding)),
                ..Props::default()
            })
            .with_children(children)
    }

    fn placements(tree: &ViewNode, rect: Rect) -> Vec<Placement> {
        let mut h = Harness::new();
        // The swept fixtures below generate fractional gaps, which the
        // pre-bound whole-unit range does not cover. The names carry the
        // numbers, so the harness reads them off the tree.
        h.bind_tree_gaps(tree);
        let mut ctx = h.ctx();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(tree, &mut ctx, &mut path, Slot::new(rect), &mut sink);
        sink.into_vec()
    }

    fn measured(tree: &ViewNode, proposal: SizeProposal) -> Size {
        let mut h = Harness::new();
        h.bind_tree_gaps(tree);
        let mut ctx = h.ctx();
        let mut path = KeyPath::root();
        crate::layout::measure(tree, &mut ctx, &mut path, proposal)
    }

    /// The hand-computed case, pinned so the algorithm is checked against
    /// arithmetic rather than against itself.
    ///
    /// Column of 300 with two 10-unit gaps: budget 280. `c` has priority 1, so
    /// it negotiates first against 280 minus the 60 units of minimum owed to
    /// `a` and `b`; offered 220 it takes its 100-unit maximum. The remaining
    /// group holds 180 and is walked least-flexible first: `a` is offered 90,
    /// takes 40, and rolls 50 forward, so `b` is offered 140 rather than 90.
    #[test]
    fn three_children_split_a_three_hundred_unit_column_by_the_binding_algorithm() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            10.0,
            Align::Start,
            vec![
                flexible("a", axis, 40.0, 40.0, 0),
                flexible("b", axis, 20.0, 200.0, 0),
                flexible("c", axis, 50.0, 100.0, 1),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 120.0, 300.0));

        let rects: Vec<Rect> = out.iter().map(|p| p.rect).collect();
        assert_eq!(
            rects,
            vec![
                Rect::new(0.0, 0.0, 120.0, 300.0),
                Rect::new(0.0, 0.0, 120.0, 40.0),
                Rect::new(0.0, 50.0, 120.0, 140.0),
                Rect::new(0.0, 200.0, 120.0, 100.0),
            ]
        );
        assert!(!out[0].paint.truncated);
        assert_eq!(
            measured(&tree, SizeProposal::exact(Size::new(120.0, 300.0))),
            Size::new(120.0, 300.0)
        );
    }

    /// The roll-forward line itself. Two children in one group: the first is
    /// rigid at 20 and cannot use its 50-unit share, so the second must be
    /// offered 80, not 50.
    #[test]
    fn surplus_a_rigid_child_declines_rolls_forward_to_the_flexible_one() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            0.0,
            Align::Start,
            vec![
                flexible("rigid", axis, 20.0, 20.0, 0),
                flexible("flexible", axis, 0.0, 500.0, 0),
            ],
        );
        // Measure first, and assert on it: `place` runs a concession pass that
        // would quietly repair an over-allocation, so the placed rects alone
        // cannot tell "distributed 100 units" from "handed out 100 units
        // twice and then took 20 back". The answer to the offer can.
        assert_eq!(
            measured(&tree, SizeProposal::exact(Size::new(50.0, 100.0))).h,
            100.0,
            "the stack must hand out its 100-unit budget once: without the \
             surplus roll-forward the flexible child is offered the whole \
             budget again and the stack answers 120 for a 100-unit offer"
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 50.0, 100.0));
        assert_eq!(out[1].rect.h, 20.0, "the rigid child takes its 20 units");
        assert_eq!(
            out[2].rect.h, 80.0,
            "the 30 units the rigid child declined must roll forward to the \
             flexible child, which would otherwise be offered only its equal \
             share of 50"
        );
        assert_eq!(out[2].rect.y, 20.0);
        assert!(
            !out[0].paint.truncated,
            "the fit was exact, so nothing was conceded and nothing was lost; \
             a stack that over-allocates and then concedes back to the same \
             rects still lies about truncation, and the flag is a digest input"
        );
    }

    /// Least flexible first is what makes roll-forward reach anything: reverse
    /// the walk and the flexible child eats the share the rigid one cannot
    /// use. Declaration order here puts the flexible child first, so the test
    /// fails if the group is walked in declaration order too.
    #[test]
    fn a_group_is_walked_least_flexible_first_whatever_the_declaration_order() {
        let axis = Axis::Horizontal;
        let tree = stack(
            axis,
            0.0,
            Align::Start,
            vec![
                flexible("flexible", axis, 0.0, 500.0, 0),
                flexible("rigid", axis, 20.0, 20.0, 0),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(out[1].rect.w, 80.0);
        assert_eq!(out[2].rect.w, 20.0);
        // Placement order is still declaration order.
        let ids: Vec<&str> = out.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["/stack", "/stack/flexible", "/stack/rigid"]);
    }

    /// Priority group before equal share: the high-priority child is satisfied
    /// out of the whole budget, and the low-priority one lives on what is
    /// left. Without groups both are in one pot and split it 50/50.
    #[test]
    fn a_high_priority_child_is_satisfied_before_a_low_priority_one_gives_ground() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            0.0,
            Align::Start,
            vec![
                flexible("greedy", axis, 0.0, 1000.0, 0),
                flexible("important", axis, 0.0, 80.0, 5),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 50.0, 100.0));
        assert_eq!(out[2].rect.h, 80.0, "the priority child takes its maximum");
        assert_eq!(
            out[1].rect.h, 20.0,
            "the other child lives on the remainder"
        );
    }

    /// Step (b): a group negotiates against the budget *minus* the minimums of
    /// every lower-priority group, so priority cannot starve a declared
    /// minimum.
    #[test]
    fn a_lower_priority_childs_declared_minimum_is_reserved_before_the_group_spends() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            0.0,
            Align::Start,
            vec![
                flexible("floor", axis, 30.0, 1000.0, 0),
                flexible("important", axis, 0.0, 80.0, 5),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 50.0, 100.0));
        assert_eq!(
            out[2].rect.h, 70.0,
            "the priority child is capped at 100 minus the 30 units reserved \
             for the lower-priority minimum"
        );
        assert_eq!(out[1].rect.h, 30.0);
    }

    #[test]
    fn spacing_is_reserved_before_anything_is_distributed() {
        let axis = Axis::Horizontal;
        let tree = stack(
            axis,
            12.0,
            Align::Start,
            vec![
                flexible("a", axis, 0.0, 500.0, 0),
                flexible("b", axis, 0.0, 500.0, 0),
                flexible("c", axis, 0.0, 500.0, 0),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 100.0, 20.0));
        // 100 minus two 12-unit gaps is 76, split three ways.
        for p in &out[1..] {
            assert!((p.rect.w - 76.0 / 3.0).abs() < 1e-3, "{:?}", p.rect);
        }
        assert!((out[2].rect.x - out[1].rect.right() - 12.0).abs() < 1e-3);
        assert!((out[3].rect.x - out[2].rect.right() - 12.0).abs() < 1e-3);
        assert!(out[3].rect.right() <= 100.0 + 1e-3);
    }

    /// Padding is reserved before anything is distributed, the same way
    /// `spacing` already is — §4 of `layout-insets.md`. A 100×50 rect with
    /// 10pt insets on every edge offers its children an 80×30 content region,
    /// not the rect's own 100×50; three equal children split the 80, and
    /// `Align::Stretch` fills the padded 30, not the outer 50.
    #[test]
    fn padding_is_reserved_before_anything_is_distributed() {
        let axis = Axis::Horizontal;
        let tree = stack_with_padding(
            axis,
            Insets::all(10.0),
            Align::Stretch,
            vec![
                flexible("a", axis, 0.0, 500.0, 0),
                flexible("b", axis, 0.0, 500.0, 0),
                flexible("c", axis, 0.0, 500.0, 0),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 100.0, 50.0));
        // 100 minus 10pt of left and right padding is 80, split three ways.
        for p in &out[1..] {
            assert!((p.rect.w - 80.0 / 3.0).abs() < 1e-3, "{:?}", p.rect);
            // The cross axis is inset too: Stretch fills the padded 30-unit
            // height (50 minus 10pt top and bottom), not the stack's own 50.
            assert!((p.rect.h - 30.0).abs() < 1e-3, "{:?}", p.rect);
            assert!((p.rect.y - 10.0).abs() < 1e-3, "{:?}", p.rect);
        }
        assert!((out[1].rect.x - 10.0).abs() < 1e-3, "{:?}", out[1].rect);
        // The stack's own placed rect never moves or shrinks because of its
        // own padding — only what it offers its children does.
        assert_eq!(out[0].rect, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert!(!out[0].paint.truncated);
        assert_eq!(
            measured(&tree, SizeProposal::exact(Size::new(100.0, 50.0))).w,
            100.0,
            "measure adds its own padding back onto what it reports: the \
             children take the full 80-unit content budget between them, and \
             the stack answers 80 plus the 20 units of padding it reserved \
             before ever offering them anything"
        );
    }

    /// §4's worked example, reproduced exactly: a 15.1pt column with 3pt
    /// insets on every edge, containing one child that needs 14pt. Padding is
    /// never a participant in the deficit, so the label — not the padding —
    /// absorbs the shortfall: truncated to the 9.1pt (15.1 - 2×3.0) the
    /// padding leaves behind, flagged, and never overlapping the inset band
    /// on either edge. This is the case the old spacer-composed padding got
    /// backwards: it handed the label 5.0pt instead of 9.1, because the label
    /// out-flexed the padding spacers in `concede`. Reserving padding before
    /// distribution removes it from `concede`'s pool entirely.
    #[test]
    fn a_padded_stack_too_small_for_its_content_truncates_without_overlapping() {
        let axis = Axis::Vertical;
        let label = flexible("label", axis, 14.0, 14.0, 0);
        let tree = stack_with_padding(axis, Insets::all(3.0), Align::Start, vec![label]);
        let out = placements(&tree, Rect::new(0.0, 0.0, 50.0, 15.1));

        assert_eq!(out.len(), 2, "the stack and the one label");
        assert!(
            (out[1].rect.h - 9.1).abs() < 1e-3,
            "expected the label truncated to 15.1 - 2*3.0 = 9.1, got {:?}",
            out[1].rect
        );
        assert!(
            (out[1].rect.y - 3.0).abs() < 1e-3,
            "the label starts after the top inset, not at the stack's own edge"
        );
        assert!(
            out[1].rect.bottom() <= 15.1 - 3.0 + 1e-3,
            "{:?} overlaps the bottom inset — the glyphs left the box",
            out[1].rect
        );
        // The stack's own placed rect never moves or shrinks because of its
        // own padding.
        assert_eq!(out[0].rect, Rect::new(0.0, 0.0, 50.0, 15.1));
        assert!(
            out[0].paint.truncated,
            "a box too small for its content, even after padding is \
             honoured, must say so"
        );
    }

    #[test]
    fn the_probes_pass_straight_through_to_the_children() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            10.0,
            Align::Start,
            vec![
                flexible("a", axis, 40.0, 40.0, 0),
                flexible("b", axis, 20.0, 200.0, 0),
                flexible("c", axis, 50.0, 100.0, 1),
            ],
        );
        // Minimums plus spacing.
        assert_eq!(measured(&tree, SizeProposal::zero()).h, 130.0);
        // Maximums plus spacing.
        assert_eq!(measured(&tree, SizeProposal::unbounded()).h, 360.0);
        // A spacer's ideal is nothing, so each child answers with its minimum.
        assert_eq!(measured(&tree, SizeProposal::unspecified()).h, 130.0);
    }

    #[test]
    fn a_stack_with_no_children_measures_and_places_as_nothing() {
        let tree = stack(Axis::Vertical, 8.0, Align::Start, vec![]);
        assert_eq!(
            measured(&tree, SizeProposal::exact(Size::new(50.0, 50.0))),
            Size::ZERO
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(out.len(), 1);
        assert!(!out[0].paint.truncated);
    }

    /// A child that refuses to shrink is conceded against and flagged, never
    /// laid on top of its neighbour.
    #[test]
    fn a_child_that_refuses_to_shrink_is_truncated_and_flagged() {
        let tree = stack(Axis::Vertical, 0.0, Align::Start, vec![rigid("icon")]);
        let out = placements(&tree, Rect::new(0.0, 0.0, 64.0, 40.0));
        assert_eq!(out[1].rect.h, 40.0);
        assert!(
            out[0].paint.truncated,
            "the stack lost 24 units of the image and must say so"
        );
    }

    /// The impossible fit: three declared minimums of 40 in a 50-unit column.
    /// Truncation takes from the tail, and every child still gets a rect that
    /// touches its neighbour rather than overlapping it.
    #[test]
    fn minimums_that_cannot_fit_truncate_from_the_tail_without_overlapping() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            0.0,
            Align::Start,
            vec![
                flexible("a", axis, 40.0, 40.0, 0),
                flexible("b", axis, 40.0, 40.0, 0),
                flexible("c", axis, 40.0, 40.0, 0),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 30.0, 50.0));
        assert_eq!(out[1].rect.h, 40.0);
        assert_eq!(out[2].rect.h, 10.0);
        assert_eq!(out[3].rect.h, 0.0);
        assert!(out[0].paint.truncated);
        for (i, a) in out[1..].iter().enumerate() {
            for b in &out[i + 2..] {
                assert!(!a.rect.overlaps(b.rect), "{:?} over {:?}", a.rect, b.rect);
            }
        }
    }

    #[test]
    fn stretch_fills_the_cross_axis_and_the_other_alignments_offset_within_it() {
        let tree = |align| {
            stack(
                Axis::Vertical,
                0.0,
                align,
                vec![ViewNode::new(NodeKind::Image, Key::new("icon"))],
            )
        };
        let rect = Rect::new(0.0, 0.0, 200.0, 64.0);
        // `icon` declares no `constraints` at all (`Constraints::default()`),
        // so the 2026-08-22 rule ("a declared maximum beats Stretch") does
        // not apply to this fixture and 200.0 is still the right answer —
        // Stretch fills the whole cross extent when nothing caps it.
        assert_eq!(placements(&tree(Align::Stretch), rect)[1].rect.w, 200.0);
        assert_eq!(placements(&tree(Align::Start), rect)[1].rect.x, 0.0);
        assert_eq!(placements(&tree(Align::Center), rect)[1].rect.x, 68.0);
        assert_eq!(placements(&tree(Align::End), rect)[1].rect.x, 136.0);
        assert_eq!(placements(&tree(Align::Center), rect)[1].rect.w, 64.0);
    }

    /// The 2026-08-22 decision (`.agents/tallies/QUESTIONS.md` Round 3 item 2): a
    /// declared cross-axis maximum beats `Align::Stretch`. Same fixture shape
    /// as the test above, but the child now declares `max: 30.0` on the
    /// cross axis, so Stretch must stop at 30, not fill the 200-unit column.
    #[test]
    fn stretch_stops_at_a_declared_cross_axis_maximum() {
        let capped = flexible("capped", Axis::Horizontal, 0.0, 30.0, 0);
        let tree = stack(Axis::Vertical, 0.0, Align::Stretch, vec![capped]);
        let rect = Rect::new(0.0, 0.0, 200.0, 64.0);
        let out = placements(&tree, rect);
        assert_eq!(
            out[1].rect.w, 30.0,
            "Stretch must stop at the child's declared max, not fill the \
             200-unit column"
        );
        // A declared minimum bigger than the cross extent still wins in the
        // response, but placement never grows past what the parent actually
        // has to give — the same rule the non-Stretch alignments already
        // follow via `.min(cross_extent)`.
        let oversized = flexible("oversized", Axis::Horizontal, 300.0, 300.0, 0);
        let tree = stack(Axis::Vertical, 0.0, Align::Stretch, vec![oversized]);
        let out = placements(&tree, rect);
        assert_eq!(
            out[1].rect.w, 200.0,
            "a declared min bigger than the cross extent must not place the \
             child wider than the stack itself"
        );
        assert!(
            out[0].paint.truncated,
            "the oversized minimum must still be flagged as lost room"
        );
    }

    /// A lone child narrower than the row sits flush at the leading edge by
    /// default (`Align::Start`, unchanged), and `justify` moves it within
    /// whatever main-axis space is left over — the exact defect
    /// `ai_label`'s `centered_caption` spacer-pair workaround exists for.
    #[test]
    fn justify_moves_the_leftover_main_axis_space() {
        let tree = |justify: Option<Justify>| {
            let mut node = stack(
                Axis::Horizontal,
                0.0,
                Align::Start,
                vec![ViewNode::new(NodeKind::Image, Key::new("icon"))],
            );
            node.props.justify = justify;
            node
        };
        let rect = Rect::new(0.0, 0.0, 200.0, 64.0);
        // Undeclared `justify` is `Align::Start`, unchanged: the leftover
        // 136 units trail after the 64-wide image, exactly as every `stack`
        // placed it before this field existed.
        assert_eq!(placements(&tree(None), rect)[1].rect.x, 0.0);
        assert_eq!(placements(&tree(Some(Justify::Start)), rect)[1].rect.x, 0.0);
        assert_eq!(
            placements(&tree(Some(Justify::Center)), rect)[1].rect.x,
            68.0
        );
        assert_eq!(placements(&tree(Some(Justify::End)), rect)[1].rect.x, 136.0);
        // A lone child has no gap to spread into, so `SpaceBetween` reads
        // as `Start`.
        assert_eq!(
            placements(&tree(Some(Justify::SpaceBetween)), rect)[1]
                .rect
                .x,
            0.0
        );
        // `justify` never changes how much room the child got.
        assert_eq!(
            placements(&tree(Some(Justify::Center)), rect)[1].rect.w,
            64.0
        );
    }

    /// `SpaceBetween` spends the leftover in the gaps: the first child
    /// stays at the leading edge, the last lands on the trailing one, and
    /// the declared spacing is the floor every gap grows from. The shape a
    /// field's value-and-chevron row and a menu button's label-and-caret
    /// row need, whatever width they are stretched to.
    #[test]
    fn space_between_pushes_the_last_child_to_the_trailing_edge() {
        let mut node = stack(
            Axis::Horizontal,
            8.0,
            Align::Start,
            vec![rigid("a"), rigid("b"), rigid("c")],
        );
        node.props.justify = Some(Justify::SpaceBetween);
        // Three 64-wide images and two 8-unit gaps want 208 of 400: 192
        // left over, 96 more in each gap.
        let out = placements(&node, Rect::new(0.0, 0.0, 400.0, 64.0));
        assert_eq!(
            out[1].rect.x, 0.0,
            "the first child stays at the leading edge"
        );
        assert_eq!(out[2].rect.x, 64.0 + 8.0 + 96.0);
        assert_eq!(
            out[3].rect.x + out[3].rect.w,
            400.0,
            "the last child ends on the trailing edge"
        );
        assert!(
            out.iter().skip(1).all(|p| p.rect.w == 64.0),
            "no child grew"
        );
        // With no slack it packs as `Start` does.
        let tight = placements(&node, Rect::new(0.0, 0.0, 208.0, 64.0));
        assert_eq!(tight[2].rect.x, 72.0);
        assert_eq!(tight[3].rect.x, 144.0);
    }

    /// `justify` only ever moves the starting cursor; a container that ran
    /// out of room still concedes exactly as it always did; the leading
    /// offset floors at zero rather than pushing an already-oversized run
    /// further past the container's own edge.
    #[test]
    fn justify_does_nothing_once_the_row_has_no_slack_left() {
        let mut node = stack(
            Axis::Horizontal,
            0.0,
            Align::Start,
            vec![rigid("a"), rigid("b"), rigid("c")],
        );
        node.props.justify = Some(Justify::Center);
        let rect = Rect::new(0.0, 0.0, 100.0, 64.0);
        let out = placements(&node, rect);
        // Three 64-wide rigid images in a 100-wide row: this already
        // truncates, and `justify: Center` must not move that truncated run
        // any further than an undeclared `justify` would.
        assert_eq!(out[1].rect.x, 0.0);
        assert!(out[0].paint.truncated);
    }

    /// One child's `align_self` overrides the container's own `align`,
    /// leaving every other child governed by it — the shape
    /// `pagination`'s `nav_divider` needs (full-height rule) beside a
    /// centred sibling in the same bar.
    #[test]
    fn align_self_overrides_the_containers_align_for_one_child() {
        let sibling = ViewNode::new(NodeKind::Image, Key::new("label"));
        let mut divider = ViewNode::new(NodeKind::Image, Key::new("divider"));
        divider.props.align_self = Some(Align::Stretch);
        let tree = stack(Axis::Horizontal, 0.0, Align::Center, vec![sibling, divider]);
        let rect = Rect::new(0.0, 0.0, 200.0, 100.0);
        let out = placements(&tree, rect);
        // `label` declares no `align_self`, so the container's own
        // `Align::Center` still decides it: a 64-tall image centred in a
        // 100-tall row sits at y = 18.
        assert_eq!(out[1].rect.h, 64.0);
        assert_eq!(out[1].rect.y, 18.0);
        // `divider`'s `align_self` overrides `Center` with `Stretch`: full
        // 100, flush at y = 0, in the very same row.
        assert_eq!(out[2].rect.h, 100.0);
        assert_eq!(out[2].rect.y, 0.0);
    }

    #[derive(Clone, Copy, Debug)]
    enum ChildSpec {
        Flexible { min: f32, span: f32, priority: i32 },
        Rigid,
    }

    #[derive(Clone, Debug)]
    struct Spec {
        axis: Axis,
        align: Align,
        spacing: f32,
        main: f32,
        cross: f32,
        children: Vec<ChildSpec>,
    }

    impl Spec {
        fn tree(&self) -> ViewNode {
            let children = self
                .children
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let key = format!("c{i}");
                    match *c {
                        ChildSpec::Flexible {
                            min,
                            span,
                            priority,
                        } => flexible(&key, self.axis, min, min + span, priority),
                        ChildSpec::Rigid => rigid(&key),
                    }
                })
                .collect();
            stack(self.axis, self.spacing, self.align, children)
        }

        fn rect(&self) -> Rect {
            match self.axis {
                Axis::Horizontal => Rect::new(0.0, 0.0, self.main, self.cross),
                Axis::Vertical => Rect::new(0.0, 0.0, self.cross, self.main),
            }
        }

        fn along(&self, rect: Rect) -> (f32, f32) {
            match self.axis {
                Axis::Horizontal => (rect.x, rect.w),
                Axis::Vertical => (rect.y, rect.h),
            }
        }
    }

    fn child_spec() -> impl Strategy<Value = ChildSpec> {
        prop_oneof![
            4 => (0f32..120.0, 0f32..400.0, -2i32..3i32)
                .prop_map(|(min, span, priority)| ChildSpec::Flexible { min, span, priority }),
            1 => Just(ChildSpec::Rigid),
        ]
    }

    fn spec() -> impl Strategy<Value = Spec> {
        (
            prop_oneof![Just(Axis::Horizontal), Just(Axis::Vertical)],
            prop_oneof![
                Just(Align::Start),
                Just(Align::Center),
                Just(Align::End),
                Just(Align::Stretch)
            ],
            0f32..24.0,
            0f32..900.0,
            0f32..500.0,
            prop::collection::vec(child_spec(), 0..7),
        )
            .prop_map(|(axis, align, spacing, main, cross, children)| Spec {
                axis,
                align,
                spacing,
                main,
                cross,
                children,
            })
    }

    proptest! {
        /// Count preservation: one placement for the stack and exactly one for
        /// each child, all parented to the stack.
        #[test]
        fn every_child_is_placed_exactly_once(spec in spec()) {
            let tree = spec.tree();
            let out = placements(&tree, spec.rect());
            prop_assert_eq!(out.len(), spec.children.len() + 1);
            prop_assert_eq!(out[0].parent, None);
            for (i, child) in tree.children.iter().enumerate() {
                prop_assert_eq!(out[i + 1].parent, Some(0));
                prop_assert_eq!(&out[i + 1].id, &format!("/stack/{}", child.key.as_str()));
            }
        }

        /// No sibling overlap. Touching edges are not an overlap, so a stack
        /// with zero spacing is expected to produce abutting rects.
        #[test]
        fn siblings_never_overlap(spec in spec()) {
            let out = placements(&spec.tree(), spec.rect());
            for (i, a) in out[1..].iter().enumerate() {
                for b in &out[i + 2..] {
                    prop_assert!(
                        !a.rect.overlaps(b.rect),
                        "{:?} overlaps {:?}", a.rect, b.rect
                    );
                }
            }
        }

        /// Order stability: main-axis position never goes backwards in
        /// declaration order, whatever order the negotiation used.
        #[test]
        fn main_axis_positions_follow_declaration_order(spec in spec()) {
            let out = placements(&spec.tree(), spec.rect());
            let mut previous = f32::NEG_INFINITY;
            for p in &out[1..] {
                let (start, _) = spec.along(p.rect);
                prop_assert!(start >= previous, "{start} came after {previous}");
                previous = start;
            }
        }

        /// Spacing reservation: the declared gap survives between every
        /// adjacent pair, including after a concession.
        #[test]
        fn the_declared_gap_survives_between_every_adjacent_pair(spec in spec()) {
            // Only when the container can afford its own gaps. A rect smaller
            // than the declared spacing has nothing to reserve from, and
            // `a_stack_too_small_for_its_own_gaps_keeps_its_children_inside`
            // pins what happens instead.
            let gaps = spec.children.len().saturating_sub(1) as f32;
            prop_assume!(spec.spacing * gaps <= spec.main);
            let out = placements(&spec.tree(), spec.rect());
            for pair in out[1..].windows(2) {
                let (start, extent) = spec.along(pair[0].rect);
                let (next, _) = spec.along(pair[1].rect);
                prop_assert!(
                    next - (start + extent) >= spec.spacing - 1e-3,
                    "gap {} is under the declared {}", next - (start + extent), spec.spacing
                );
            }
        }

        /// FR-006: the same tree in the same rect lays out identically. The
        /// negotiation sorts on measured floats, which is exactly where a
        /// non-total comparison would leak run-to-run variation.
        #[test]
        fn the_same_tree_lays_out_identically_twice(spec in spec()) {
            let tree = spec.tree();
            prop_assert_eq!(placements(&tree, spec.rect()), placements(&tree, spec.rect()));
        }

        /// Nothing leaves the stack's own rect along the main axis. This is
        /// the property the concession order exists to keep.
        #[test]
        fn no_child_is_placed_past_the_end_of_the_stack(spec in spec()) {
            let out = placements(&spec.tree(), spec.rect());
            for p in &out[1..] {
                let (start, extent) = spec.along(p.rect);
                prop_assert!(extent >= 0.0, "negative extent {extent}");
                prop_assert!(
                    start + extent <= spec.main + 1e-3,
                    "{} runs past {}", start + extent, spec.main
                );
            }
        }
    }

    /// The measure/place agreement the dispatcher relies on: what a stack says
    /// it takes for an exact offer is what it lays out in a rect that size.
    #[test]
    fn what_the_stack_answers_is_what_it_places() {
        let axis = Axis::Horizontal;
        let tree = stack(
            axis,
            6.0,
            Align::Start,
            vec![
                flexible("a", axis, 10.0, 30.0, 0),
                flexible("b", axis, 10.0, 400.0, 0),
                rigid("c"),
            ],
        );
        let rect = Rect::new(0.0, 0.0, 300.0, 64.0);
        let answered = measured(&tree, SizeProposal::exact(rect.size()));
        let out = placements(&tree, rect);
        let placed = out[3].rect.right() - out[1].rect.x;
        assert!(
            (answered.w - placed).abs() < 1e-3,
            "{answered:?} vs {placed}"
        );
    }

    #[test]
    fn an_exact_offer_is_answered_with_what_the_children_take_not_the_offer() {
        let axis = Axis::Vertical;
        // Both children cap out at 30, so a 500-unit column is answered with
        // 60: a stack reports what it takes, and the parent places it.
        let tree = stack(
            axis,
            0.0,
            Align::Start,
            vec![
                flexible("a", axis, 0.0, 30.0, 0),
                flexible("b", axis, 0.0, 30.0, 0),
            ],
        );
        assert_eq!(
            measured(&tree, SizeProposal::exact(Size::new(40.0, 500.0))).h,
            60.0
        );
    }

    #[test]
    fn an_offer_smaller_than_the_minimums_is_answered_honestly_not_clamped() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            0.0,
            Align::Start,
            vec![
                flexible("a", axis, 40.0, 40.0, 0),
                flexible("b", axis, 40.0, 40.0, 0),
            ],
        );
        // 80 for a 30-unit offer: the overflow belongs to the parent's
        // concession rule, and hiding it here would hide the truncation.
        assert_eq!(
            measured(&tree, SizeProposal::exact(Size::new(20.0, 30.0))).h,
            80.0
        );
    }

    #[test]
    fn a_nested_stack_negotiates_as_one_child_of_its_parent() {
        let axis = Axis::Vertical;
        let inner = stack(
            axis,
            0.0,
            Align::Stretch,
            vec![
                flexible("x", axis, 10.0, 10.0, 0),
                flexible("y", axis, 10.0, 10.0, 0),
            ],
        );
        let outer = ViewNode::new(NodeKind::Stack, Key::new("outer"))
            .with_props(Props {
                axis: Some(axis),
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![inner, flexible("tail", axis, 0.0, 500.0, 0)]);
        let out = placements(&outer, Rect::new(0.0, 0.0, 50.0, 100.0));

        let ids: Vec<&str> = out.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "/outer",
                "/outer/stack",
                "/outer/stack/x",
                "/outer/stack/y",
                "/outer/tail"
            ]
        );
        // The inner stack answers every probe with 20 — two children capped at
        // 10 — so it is the rigid child of the outer group and the tail takes
        // the 80 units it rolled forward.
        assert_eq!(out[1].rect.h, 20.0);
        assert_eq!(out[4].rect.h, 80.0);
        assert_eq!(out[4].rect.y, 20.0);
    }

    #[test]
    fn the_measurement_cache_absorbs_the_repeated_probes() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            0.0,
            Align::Start,
            vec![rigid("a"), rigid("b"), rigid("c")],
        );
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        {
            let mut ctx = h.ctx();
            crate::layout::measure(
                &tree,
                &mut ctx,
                &mut path,
                SizeProposal::exact(Size::new(64.0, 300.0)),
            );
        }
        let first = h.content.calls;
        {
            let mut ctx = h.ctx();
            crate::layout::measure(
                &tree,
                &mut ctx,
                &mut path,
                SizeProposal::exact(Size::new(64.0, 300.0)),
            );
        }
        assert_eq!(h.content.calls, first, "a repeat probe must not re-measure");
        let (hits, _) = h.cache.stats();
        assert!(hits > 0);
    }

    #[test]
    fn a_horizontal_stack_distributes_across_x_and_a_vertical_one_across_y() {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let tree = stack(
                axis,
                0.0,
                Align::Start,
                vec![
                    flexible("a", axis, 25.0, 25.0, 0),
                    flexible("b", axis, 25.0, 25.0, 0),
                ],
            );
            let out = placements(&tree, Rect::new(0.0, 0.0, 50.0, 50.0));
            match axis {
                Axis::Horizontal => {
                    assert_eq!((out[1].rect.x, out[2].rect.x), (0.0, 25.0));
                    assert_eq!((out[1].rect.y, out[2].rect.y), (0.0, 0.0));
                }
                Axis::Vertical => {
                    assert_eq!((out[1].rect.y, out[2].rect.y), (0.0, 25.0));
                    assert_eq!((out[1].rect.x, out[2].rect.x), (0.0, 0.0));
                }
            }
        }
    }

    /// The degenerate rect: three children and a 20-unit gap in a 10-unit
    /// column. Nothing can be reserved, so the gap gives ground too — a child
    /// pushed outside its parent is unreachable, and no concession on child
    /// extents can pull it back.
    #[test]
    fn a_stack_too_small_for_its_own_gaps_keeps_its_children_inside() {
        let axis = Axis::Vertical;
        let tree = stack(
            axis,
            20.0,
            Align::Start,
            vec![
                flexible("a", axis, 0.0, 100.0, 0),
                flexible("b", axis, 0.0, 100.0, 0),
                flexible("c", axis, 0.0, 100.0, 0),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 30.0, 10.0));
        assert_eq!(out.len(), 4);
        for p in &out[1..] {
            assert_eq!(p.rect.h, 0.0);
            assert!(p.rect.bottom() <= 10.0, "{:?} is outside the stack", p.rect);
        }
        assert_eq!(out[2].rect.y, 5.0, "the two gaps split the 10 units");
        assert_eq!(out[3].rect.y, 10.0);
        assert!(out[0].paint.truncated, "the declared spacing was cut");
    }

    /// The end-to-end concession into a scroll child: overflow is absorbed by
    /// the scroll region rather than truncating anything.
    ///
    /// This test's absence used to be excused by a comment saying the path
    /// "cannot be exercised until the scroll container itself lands (T019)".
    /// T019 landed and the comment stayed, so the gap outlived its reason.
    ///
    /// The shape matters: `Concession::ScrollRegion` is only reachable when the
    /// scroll region declares a minimum. Without one its floor is zero, step
    /// one of the order already takes it to zero, and step two finds nothing
    /// left to give — so a scroll child with no declared `min` never exercises
    /// the branch at all, whatever the overflow.
    #[test]
    fn a_stack_concedes_into_a_scroll_child_instead_of_truncating() {
        let axis = Axis::Vertical;
        let rigid_child = flexible("head", axis, 70.0, 70.0, 0);
        let scroller = ViewNode::new(NodeKind::Scroll, Key::new("log"))
            .with_props(Props {
                axis: Some(axis),
                ..Props::default()
            })
            .with_constraints(on(
                axis,
                AxisConstraint {
                    min: Some(50.0),
                    max: None,
                    priority: 0,
                },
            ));
        let tree = stack(axis, 0.0, Align::Start, vec![rigid_child, scroller]);

        // 70 + 50 declared into 100 available: 20 units of overflow, and the
        // rigid child is already at both its bounds.
        let out = placements(&tree, Rect::new(0.0, 0.0, 40.0, 100.0));
        assert_eq!(out.len(), 3, "the stack, the head, and the scroll region");
        assert_eq!(out[1].rect.h, 70.0, "the rigid child keeps its declaration");
        assert_eq!(
            out[2].rect.h, 30.0,
            "the scroll region absorbs all 20 units of overflow by shrinking \
             past its own minimum: it grows a scroll range rather than losing \
             content"
        );
        assert!(
            !out[0].paint.truncated,
            "nothing was truncated — absorption is the whole point of putting \
             ScrollRegion before Truncate in FR-005's order: {:?}",
            out[0].paint
        );
        assert!(
            out[2].rect.bottom() <= 100.0 + 1e-3,
            "{:?} is outside the stack",
            out[2].rect
        );
    }

    /// Which children may absorb rather than truncate.
    #[test]
    fn only_a_scroll_child_along_the_main_axis_absorbs_overflow() {
        let scroller = |axis| {
            ViewNode::new(NodeKind::Scroll, Key::new("log")).with_props(Props {
                axis: Some(axis),
                ..Props::default()
            })
        };
        assert!(super::scroll_absorbs(
            &scroller(Axis::Vertical),
            Axis::Vertical
        ));
        assert!(
            !super::scroll_absorbs(&scroller(Axis::Horizontal), Axis::Vertical),
            "a region that scrolls across the axis being negotiated cannot \
             absorb along it: shrinking it there loses content"
        );
        assert!(!super::scroll_absorbs(&rigid("icon"), Axis::Vertical));
    }

    // ---- the widest child of a row -------------------------------------

    /// A label `chars` characters wide under `MonoContent` (8 units each),
    /// optionally carrying declared horizontal bounds.
    fn label(key: &str, chars: usize, min: Option<f32>, max: Option<f32>) -> ViewNode {
        let node = ViewNode::new(NodeKind::Text, Key::new(key)).with_props(Props {
            text: Some("x".repeat(chars)),
            ..Props::default()
        });
        if min.is_none() && max.is_none() {
            return node;
        }
        node.with_constraints(on(
            Axis::Horizontal,
            AxisConstraint {
                min,
                max,
                priority: 0,
            },
        ))
    }

    /// A row of `n` children: `n - 1` bare 11-character labels (88 units
    /// each) and a 13-character label (104 units) last, carrying a declared
    /// 40-unit minimum. The minimum is what makes the widest child the
    /// *least* flexible of the row -- 104 - 40 = 64 against the bare labels'
    /// 88 - 0 = 88 -- so the ascending-flexibility walk reaches it first,
    /// before any sibling has declined anything.
    fn widest_last(n: usize) -> ViewNode {
        let mut children: Vec<ViewNode> = (0..n - 1)
            .map(|i| label(&format!("bare{i}"), 11, None, None))
            .collect();
        children.push(label("wide", 13, Some(40.0), None));
        stack(Axis::Horizontal, 0.0, Align::Start, children)
    }

    #[test]
    fn a_row_placed_at_the_width_it_asked_for_gives_every_child_what_it_asked_for() {
        let mut placed = Vec::new();
        for n in 1..=5usize {
            let tree = widest_last(n);
            let natural = measured(&tree, SizeProposal::unbounded()).w;
            assert_eq!(
                natural,
                88.0 * (n as f32 - 1.0) + 104.0,
                "n = {n}: the row's own unbounded answer moved"
            );
            let out = placements(&tree, Rect::new(0.0, 0.0, natural, 40.0));
            let wide = out
                .iter()
                .find(|p| p.id.ends_with("wide"))
                .expect("the widest child is placed");
            placed.push(wide.rect.w);
        }
        assert!(
            placed.iter().all(|w| (w - 104.0).abs() < 1e-3),
            "a row placed at exactly the width it answered gave its 104-unit child \
             {placed:?} for n = 1..=5"
        );
    }

    /// The main-axis extent of the placement whose id ends in `key`.
    fn extent_of(out: &[Placement], key: &str, axis: Axis) -> f32 {
        let p = out
            .iter()
            .find(|p| p.id.ends_with(key))
            .unwrap_or_else(|| panic!("{key} is placed"));
        match axis {
            Axis::Horizontal => p.rect.w,
            Axis::Vertical => p.rect.h,
        }
    }

    /// The roll-back is not a text or a horizontal phenomenon: any child that
    /// answers an `Exact` offer with the offer is limited by it, and a spacer
    /// clamped into a band does exactly that. Four spacers capped at 88 and
    /// one capped at 104 whose declared 40-unit minimum makes it the least
    /// flexible of the five, on each axis in turn.
    #[test]
    fn the_roll_back_settles_both_axes() {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let mut children: Vec<ViewNode> = (0..4)
                .map(|i| flexible(&format!("bare{i}"), axis, 0.0, 88.0, 0))
                .collect();
            children.push(flexible("wide", axis, 40.0, 104.0, 0));
            let tree = stack(axis, 0.0, Align::Start, children);
            let natural = measured(&tree, SizeProposal::unbounded()).along(axis);
            assert_eq!(natural, 456.0, "{axis:?}");
            let rect = match axis {
                Axis::Horizontal => Rect::new(0.0, 0.0, natural, 60.0),
                Axis::Vertical => Rect::new(0.0, 0.0, 60.0, natural),
            };
            let out = placements(&tree, rect);
            assert_eq!(
                extent_of(&out, "wide", axis),
                104.0,
                "{axis:?}: the least flexible child was left on the mean of \
                 the row's naturals while its siblings declined 12.8 units"
            );
            assert!(!out[0].paint.truncated, "{axis:?}");
        }
    }

    /// The shrink policy, pinned: a row genuinely too narrow for its children
    /// splits what it has **equally**, not in proportion to their naturals.
    /// 104 and 88 in a 150-unit row are 75 and 75, not the 81.25 and 68.75
    /// a proportional policy would place.
    #[test]
    fn a_row_too_narrow_for_its_children_shrinks_them_to_an_equal_share() {
        let tree = widest_last(2);
        let out = placements(&tree, Rect::new(0.0, 0.0, 150.0, 60.0));
        assert_eq!(extent_of(&out, "wide", Axis::Horizontal), 75.0);
        assert_eq!(extent_of(&out, "bare0", Axis::Horizontal), 75.0);
    }

    /// A declared minimum outranks the equal share: the wide label holds its
    /// 60 units in a 100-unit row and the sibling lives on the 40 that are
    /// left, rather than both landing on 50.
    #[test]
    fn a_declared_minimum_outranks_the_equal_share_a_narrow_row_hands_out() {
        let tree = stack(
            Axis::Horizontal,
            0.0,
            Align::Start,
            vec![
                label("bare0", 11, None, None),
                label("wide", 13, Some(60.0), None),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 100.0, 60.0));
        assert_eq!(extent_of(&out, "wide", Axis::Horizontal), 60.0);
        assert_eq!(extent_of(&out, "bare0", Axis::Horizontal), 40.0);
    }

    /// A child already at its declared maximum takes no part in the roll-back:
    /// it answered *less* than it was offered, which is the engine's one
    /// signal for "satisfied", and the 16 units it left over stay leftover for
    /// `justify` to spend.
    #[test]
    fn the_roll_back_never_grows_a_child_past_its_declared_maximum() {
        let tree = stack(
            Axis::Horizontal,
            0.0,
            Align::Start,
            vec![
                label("bare0", 11, None, None),
                label("wide", 13, Some(40.0), Some(96.0)),
            ],
        );
        let out = placements(&tree, Rect::new(0.0, 0.0, 200.0, 60.0));
        assert_eq!(extent_of(&out, "wide", Axis::Horizontal), 96.0);
        assert_eq!(extent_of(&out, "bare0", Axis::Horizontal), 88.0);
        assert_eq!(
            measured(&tree, SizeProposal::exact(Size::new(200.0, 60.0))).w,
            184.0,
            "the row answers what its children take, and 16 units stay unspent"
        );
    }

    /// The roll-back spends the group's own budget and nothing else: a
    /// lower-priority child's declared minimum was reserved out of that budget
    /// before the group ever saw it, and it is still there afterwards.
    #[test]
    fn the_roll_back_cannot_raid_a_lower_prioritys_reserved_minimum() {
        let mut wide = label("wide", 13, Some(40.0), None);
        wide.constraints.horizontal.priority = 5;
        let mut bare = label("bare0", 11, None, None);
        bare.constraints.horizontal.priority = 5;
        let tree = stack(
            Axis::Horizontal,
            0.0,
            Align::Start,
            vec![
                wide,
                bare,
                flexible("floor", Axis::Horizontal, 30.0, 1000.0, 0),
            ],
        );
        // 104 + 88 + 30 = 222, and every one of the three gets its number.
        let out = placements(&tree, Rect::new(0.0, 0.0, 222.0, 60.0));
        assert_eq!(extent_of(&out, "wide", Axis::Horizontal), 104.0);
        assert_eq!(extent_of(&out, "bare0", Axis::Horizontal), 88.0);
        assert_eq!(extent_of(&out, "floor", Axis::Horizontal), 30.0);
        assert!(!out[0].paint.truncated);
    }

    /// The roll-back must not eat the slack `justify` spends. It only ever
    /// re-offers to a child that answered with at least what it was given;
    /// both children here answered with less, so the 108 units they declined
    /// go into the gap, exactly as they did before the roll-back existed.
    #[test]
    fn a_satisfied_row_keeps_the_slack_space_between_spends() {
        let mut tree = widest_last(2);
        tree.props.justify = Some(Justify::SpaceBetween);
        let out = placements(&tree, Rect::new(0.0, 0.0, 300.0, 60.0));
        assert_eq!(extent_of(&out, "bare0", Axis::Horizontal), 88.0);
        assert_eq!(extent_of(&out, "wide", Axis::Horizontal), 104.0);
        let wide = out.iter().find(|p| p.id.ends_with("wide")).unwrap();
        assert_eq!(out[1].rect.x, 0.0);
        assert_eq!(wide.rect.right(), 300.0);
    }

    /// How a row comes to be placed at exactly the width it answered, which is
    /// the shape the defect needs and the reason it reached a shipped page.
    /// A column that aligns its children `Start` places each one across at its
    /// own measured extent rather than at the column's width — so the row is
    /// handed back precisely the number it built out of its children's
    /// naturals, and then has to reproduce it.
    #[test]
    fn a_row_start_aligned_inside_a_column_still_gives_its_widest_child_its_width() {
        let keyed = |mut n: ViewNode, k: &str| {
            n.key = Key::new(k);
            n
        };
        let mut column = stack(
            Axis::Vertical,
            0.0,
            Align::Start,
            vec![keyed(widest_last(3), "r3"), keyed(widest_last(5), "r5")],
        );
        column.key = Key::new("column");
        let out = placements(&column, Rect::new(0.0, 0.0, 900.0, 200.0));
        // Both rows are Start-aligned, so each is exactly as wide as it asked.
        let rows: Vec<f32> = out
            .iter()
            .filter(|p| p.id == "/column/r3" || p.id == "/column/r5")
            .map(|p| p.rect.w)
            .collect();
        assert_eq!(rows, vec![280.0, 456.0]);
        for w in out
            .iter()
            .filter(|p| p.id.ends_with("wide"))
            .map(|p| p.rect.w)
        {
            assert_eq!(w, 104.0, "a Start-aligned row clipped its widest child");
        }
    }
}
