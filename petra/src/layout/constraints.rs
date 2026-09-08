//! The deterministic concession order.
//!
//! Clamping a single response to a node's own min/max is
//! [`crate::tree::Constraints::clamp_size`], applied once by the dispatcher.
//! This module is the other half of FR-005: what a *container* does when the
//! clamped responses of its children add up to more than it has to give.
//!
//! The order is fixed and the same every time — flexible slack, then declared
//! scroll regions, then truncation (`contracts/view-tree.md`). Order matters
//! more than the individual steps: a container that gave ground in a different
//! order on two runs of the same tree would produce two digests for one frame,
//! and FR-006 is the claim that cannot happen.

/// The order in which a container gives ground when the fit is impossible
/// (FR-005): flexible slack first, then declared scroll regions, then
/// truncation. Deterministic, and the same order every time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Concession {
    /// Take from the most flexible children first.
    FlexibleSlack,
    /// Let a declared scroll region absorb the overflow.
    ScrollRegion,
    /// Truncate content, recording it for the semantic tree.
    Truncate,
}

/// One child's negotiated extent along the axis under negotiation, and what it
/// is able to give up.
///
/// The slice a caller hands [`concede`] is in declaration order; the function
/// reorders nothing, so a caller reads the conceded extents straight back out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Give {
    /// Position of this child in its parent's declaration order. Carried so a
    /// container that concedes over a filtered subset can still write the
    /// result back to the right child.
    pub index: usize,
    /// The extent as negotiated. [`concede`] lowers this in place.
    pub extent: f32,
    /// The extent below which this child stops being merely smaller and starts
    /// losing content: its `Zero` response. Steps one and two stop here.
    pub floor: f32,
    /// `Unbounded` response minus `Zero` response. The most flexible child
    /// gives first, which is the whole content of step one.
    pub flexibility: f32,
    /// For a declared scroll region negotiating along the axis it scrolls: the
    /// extent it may shrink past its floor to while its content stays
    /// reachable. `None` for every other child — shrinking those past the
    /// floor is truncation, not absorption.
    pub scroll_floor: Option<f32>,
}

impl Give {
    /// A child that is not a scroll region.
    #[must_use]
    pub fn new(index: usize, extent: f32, floor: f32, flexibility: f32) -> Self {
        Self {
            index,
            extent,
            floor,
            flexibility,
            scroll_floor: None,
        }
    }

    /// This child marked as a scroll region that may absorb down to `floor`.
    #[must_use]
    pub fn absorbing(mut self, floor: f32) -> Self {
        self.scroll_floor = Some(floor.max(0.0));
        self
    }
}

/// What one concession pass did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Conceded {
    /// The steps that actually gave ground, in the order FR-005 mandates. A
    /// step that had nothing to give does not appear, so an empty list means
    /// the fit was possible after all.
    pub steps: Vec<Concession>,
    /// Overflow no step could absorb, because every child was already at zero.
    /// The container reports this rather than placing children on top of each
    /// other: the contract's "never silently overlapped".
    pub unresolved: f32,
}

impl Conceded {
    /// Whether content was lost, and the container must flag `truncated`.
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.steps.contains(&Concession::Truncate) || self.unresolved > 0.0
    }
}

/// Overflow at or below this many logical units is float dust from summing
/// extents, not a fit that failed. Without the guard a stack whose children
/// add up to exactly its rect would record a truncation on some inputs and not
/// on others, and `truncated` is a digest input.
///
/// Public so every container measures "does it fit" with the same tolerance;
/// two containers disagreeing about that by an ulp is a frame that truncates
/// on one machine and not another.
pub const FIT_EPSILON: f32 = 1e-4;

/// Give up `overflow` logical units across `items`, in the FR-005 order.
///
/// `items` is in declaration order and is modified in place: each `extent`
/// comes back reduced by what that child conceded. The return value names the
/// steps that were used and any overflow that survived all three.
pub fn concede(items: &mut [Give], overflow: f32) -> Conceded {
    let mut left = if overflow.is_finite() { overflow } else { 0.0 };
    let mut steps = Vec::new();
    if left <= FIT_EPSILON {
        return Conceded {
            steps,
            unresolved: 0.0,
        };
    }

    // Most flexible first, ties in declaration order. `sort_by` is stable and
    // `items` arrives in declaration order, so the tie-break is free — and it
    // is the reason two runs of one tree concede identically.
    let mut by_flexibility: Vec<usize> = (0..items.len()).collect();
    by_flexibility.sort_by(|&a, &b| items[b].flexibility.total_cmp(&items[a].flexibility));

    if take(items, &by_flexibility, &mut left, |g| g.floor) {
        steps.push(Concession::FlexibleSlack);
    }
    // A scroll region shrinking past its floor loses no content: it grows a
    // scroll range instead. That is why it goes before truncation and not
    // after — the contract's "content never becomes unreachable".
    if left > FIT_EPSILON && take(items, &by_flexibility, &mut left, scroll_floor) {
        steps.push(Concession::ScrollRegion);
    }
    if left > FIT_EPSILON {
        // Tail first. The contract fixes truncation as the last step but not
        // which child loses; declaration order is reading order, so the last
        // child is the one a reader misses least. Reverse index order, not
        // flexibility order: a rigid child that already refused to shrink is
        // not a better thing to cut than the trailing one.
        let tail: Vec<usize> = (0..items.len()).rev().collect();
        if take(items, &tail, &mut left, |_| 0.0) {
            steps.push(Concession::Truncate);
        }
    }

    Conceded {
        steps,
        unresolved: if left > FIT_EPSILON { left } else { 0.0 },
    }
}

/// The floor a scroll region may absorb down to. A child that is not one
/// reports its own current extent, so it has nothing to give in that step.
fn scroll_floor(give: &Give) -> f32 {
    give.scroll_floor.unwrap_or(give.extent)
}

/// Walk `order`, taking from each child down to `floor`, until `left` is gone.
/// Answers whether anything was actually taken.
fn take(items: &mut [Give], order: &[usize], left: &mut f32, floor: impl Fn(&Give) -> f32) -> bool {
    let mut gave = false;
    for &i in order {
        if *left <= FIT_EPSILON {
            break;
        }
        let available = items[i].extent - floor(&items[i]);
        if available <= 0.0 {
            continue;
        }
        let given = available.min(*left);
        items[i].extent -= given;
        *left -= given;
        gave = true;
    }
    gave
}

#[cfg(test)]
mod tests {
    use super::{Conceded, Concession, Give, concede};

    fn extents(items: &[Give]) -> Vec<f32> {
        items.iter().map(|g| g.extent).collect()
    }

    #[test]
    fn a_fit_that_works_concedes_nothing() {
        let mut items = [
            Give::new(0, 50.0, 10.0, 40.0),
            Give::new(1, 50.0, 10.0, 40.0),
        ];
        let out = concede(&mut items, 0.0);
        assert_eq!(out, Conceded::default());
        assert_eq!(extents(&items), [50.0, 50.0]);
        assert!(!out.truncated());
    }

    #[test]
    fn flexible_slack_gives_before_anything_else() {
        // Rigid child first in declaration order, so a rule that just walked
        // the slice would take from the wrong one.
        let mut items = [
            Give::new(0, 40.0, 40.0, 0.0),
            Give::new(1, 60.0, 10.0, 90.0),
        ];
        let out = concede(&mut items, 25.0);
        assert_eq!(out.steps, vec![Concession::FlexibleSlack]);
        assert_eq!(extents(&items), [40.0, 35.0]);
        assert_eq!(out.unresolved, 0.0);
    }

    #[test]
    fn the_most_flexible_child_gives_first_and_the_next_one_finishes_the_job() {
        let mut items = [
            Give::new(0, 50.0, 20.0, 30.0),
            Give::new(1, 50.0, 20.0, 100.0),
        ];
        let out = concede(&mut items, 40.0);
        assert_eq!(out.steps, vec![Concession::FlexibleSlack]);
        // Child 1 is the most flexible: it gives its whole 30 units of slack
        // before child 0 gives the remaining 10.
        assert_eq!(extents(&items), [40.0, 20.0]);
    }

    #[test]
    fn ties_in_flexibility_break_on_declaration_order() {
        let mut items = [
            Give::new(0, 50.0, 0.0, 25.0),
            Give::new(1, 50.0, 0.0, 25.0),
            Give::new(2, 50.0, 0.0, 25.0),
        ];
        concede(&mut items, 60.0);
        assert_eq!(extents(&items), [0.0, 40.0, 50.0]);
    }

    #[test]
    fn a_scroll_region_absorbs_only_after_flexible_slack_is_gone() {
        let mut items = [
            Give::new(0, 40.0, 30.0, 10.0),
            Give::new(1, 60.0, 60.0, 0.0).absorbing(0.0),
        ];
        let out = concede(&mut items, 35.0);
        assert_eq!(
            out.steps,
            vec![Concession::FlexibleSlack, Concession::ScrollRegion]
        );
        // 10 units of slack from child 0, the remaining 25 absorbed by the
        // scroll region shrinking past its floor. Nothing truncated.
        assert_eq!(extents(&items), [30.0, 35.0]);
        assert!(!out.truncated());
    }

    #[test]
    fn a_scroll_region_stops_at_its_own_absorb_floor() {
        let mut items = [Give::new(0, 60.0, 60.0, 0.0).absorbing(20.0)];
        let out = concede(&mut items, 50.0);
        assert_eq!(
            out.steps,
            vec![Concession::ScrollRegion, Concession::Truncate]
        );
        assert_eq!(extents(&items), [10.0]);
        assert!(out.truncated());
    }

    #[test]
    fn a_scroll_region_across_its_scrolling_axis_absorbs_nothing() {
        // `scroll_floor: None` is how a container says "this child scrolls,
        // but not along the axis being negotiated".
        let mut items = [Give::new(0, 60.0, 60.0, 0.0)];
        let out = concede(&mut items, 20.0);
        assert_eq!(out.steps, vec![Concession::Truncate]);
        assert_eq!(extents(&items), [40.0]);
    }

    #[test]
    fn truncation_takes_from_the_tail_so_the_leading_content_survives() {
        let mut items = [
            Give::new(0, 30.0, 30.0, 0.0),
            Give::new(1, 30.0, 30.0, 0.0),
            Give::new(2, 30.0, 30.0, 0.0),
        ];
        let out = concede(&mut items, 45.0);
        assert_eq!(out.steps, vec![Concession::Truncate]);
        assert_eq!(extents(&items), [30.0, 15.0, 0.0]);
        assert!(out.truncated());
    }

    /// The impossible fit the task names: every child's declared minimum is
    /// already more than the container has.
    #[test]
    fn minimums_that_exceed_the_budget_truncate_rather_than_overlap() {
        let mut items = [
            Give::new(0, 40.0, 40.0, 0.0),
            Give::new(1, 40.0, 40.0, 0.0),
            Give::new(2, 40.0, 40.0, 0.0),
        ];
        let out = concede(&mut items, 70.0);
        assert_eq!(out.steps, vec![Concession::Truncate]);
        assert_eq!(extents(&items), [40.0, 10.0, 0.0]);
        // The sum of what is left is exactly the budget: 120 negotiated minus
        // 70 of overflow. No child was placed on top of another to get there.
        assert_eq!(extents(&items).iter().sum::<f32>(), 50.0);
        assert_eq!(out.unresolved, 0.0);
    }

    /// Determinism is the claim FR-006 rests on, so it is asserted rather than
    /// assumed: the same impossible fit, conceded a hundred times.
    #[test]
    fn the_same_impossible_fit_concedes_identically_every_time() {
        let seed = [
            Give::new(0, 40.0, 20.0, 20.0),
            Give::new(1, 55.0, 55.0, 0.0).absorbing(5.0),
            Give::new(2, 60.0, 30.0, 30.0),
            Give::new(3, 25.0, 25.0, 0.0),
        ];
        let mut first = seed;
        let expected = concede(&mut first, 90.0);
        assert_eq!(
            expected.steps,
            vec![Concession::FlexibleSlack, Concession::ScrollRegion]
        );
        for _ in 0..100 {
            let mut items = seed;
            let out = concede(&mut items, 90.0);
            assert_eq!(out, expected);
            assert_eq!(extents(&items), extents(&first));
        }
    }

    #[test]
    fn overflow_nothing_can_absorb_is_reported_rather_than_hidden() {
        let mut items = [Give::new(0, 10.0, 10.0, 0.0)];
        let out = concede(&mut items, 30.0);
        assert_eq!(out.steps, vec![Concession::Truncate]);
        assert_eq!(extents(&items), [0.0]);
        assert_eq!(out.unresolved, 20.0);
        assert!(out.truncated());
    }

    #[test]
    fn a_container_with_no_children_reports_the_whole_overflow() {
        let out = concede(&mut [], 12.0);
        assert!(out.steps.is_empty());
        assert_eq!(out.unresolved, 12.0);
    }

    #[test]
    fn float_dust_is_not_a_truncation() {
        let mut items = [Give::new(0, 40.0, 40.0, 0.0)];
        let out = concede(&mut items, 1e-6);
        assert!(out.steps.is_empty());
        assert_eq!(extents(&items), [40.0]);
        assert!(!out.truncated());
    }

    #[test]
    fn a_non_finite_overflow_concedes_nothing() {
        let mut items = [Give::new(0, 40.0, 10.0, 30.0)];
        let out = concede(&mut items, f32::NAN);
        assert!(out.steps.is_empty());
        assert_eq!(extents(&items), [40.0]);
    }
}

/// Bounded proofs over this module. See
/// `.agents/notes/proposed/testing/2026-08-30-kani-bounded-verification.md`.
#[cfg(kani)]
mod proofs {
    use super::{Give, concede};

    /// For 2 children with a non-negative floor no greater than their
    /// starting extent, `concede` never leaves an item with a negative
    /// extent, for any overflow amount, any scroll-absorb floor, and any
    /// flexibility. `take` is called with three different floor closures
    /// (`|g| g.floor`, `scroll_floor`, `|_| 0.0` for truncation), and this
    /// checks all three keep their promise rather than trusting it by
    /// inspection. Magnitudes are bounded so CBMC's float decision
    /// procedure has a small search space, not the full `f32` range.
    #[kani::proof]
    #[kani::unwind(4)]
    fn concede_never_leaves_a_negative_extent() {
        let e0: f32 = kani::any();
        let f0: f32 = kani::any();
        let flex0: f32 = kani::any();
        let has_scroll0: bool = kani::any();
        let scroll_floor0: f32 = kani::any();

        let e1: f32 = kani::any();
        let f1: f32 = kani::any();
        let flex1: f32 = kani::any();
        let has_scroll1: bool = kani::any();
        let scroll_floor1: f32 = kani::any();

        let overflow: f32 = kani::any();

        kani::assume(e0.is_finite() && f0.is_finite() && flex0.is_finite());
        kani::assume(e1.is_finite() && f1.is_finite() && flex1.is_finite());
        kani::assume(scroll_floor0.is_finite() && scroll_floor1.is_finite());
        kani::assume(overflow.is_finite());
        kani::assume((0.0..=1000.0).contains(&e0) && (0.0..=1000.0).contains(&e1));
        kani::assume(f0 >= 0.0 && f0 <= e0);
        kani::assume(f1 >= 0.0 && f1 <= e1);
        kani::assume(flex0 >= 0.0 && flex1 >= 0.0);
        kani::assume((0.0..=e0).contains(&scroll_floor0));
        kani::assume((0.0..=e1).contains(&scroll_floor1));
        kani::assume((-2000.0..=2000.0).contains(&overflow));

        let mut g0 = Give::new(0, e0, f0, flex0);
        if has_scroll0 {
            g0 = g0.absorbing(scroll_floor0);
        }
        let mut g1 = Give::new(1, e1, f1, flex1);
        if has_scroll1 {
            g1 = g1.absorbing(scroll_floor1);
        }

        let mut items = [g0, g1];
        let _ = concede(&mut items, overflow);

        assert!(items[0].extent >= 0.0);
        assert!(items[1].extent >= 0.0);
        assert!(items[0].extent.is_finite());
        assert!(items[1].extent.is_finite());
    }
}
