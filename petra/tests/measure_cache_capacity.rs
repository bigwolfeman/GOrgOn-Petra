//! The shipped cache bound must fit a real application's tree.
//!
//! `MeasureCache::DEFAULT_CAPACITY` was chosen against a virtualized list,
//! where one frame's working set is the visible window plus overscan — "tens to
//! low hundreds, not thousands", as the constant's own doc said. That reasoning
//! is sound for a scrolling list and wrong for a dense one. Spec 003's
//! acceptance application is the fiber inspector (D-074): an identity bar, a
//! fiber list, an effects tree, and several tables, all on screen at once and
//! none of them virtualized. Every node in such a tree is probed with several
//! proposals, so the working set is a multiple of the node count.
//!
//! The negotiation bench measured the consequence: a 4 225-node tree costs
//! 9.35 ms with room in the cache and 16.16 ms at a bound of 8 192, on 45 825
//! evictions — a cache thrashing against itself for a 73% slowdown, while
//! reporting a perfectly healthy hit rate on the scroll workload it was tuned
//! for.
//!
//! This test pins the bound to the shape it actually has to serve. It is
//! deliberately expressed in *nodes*, not in entries: the entry count is an
//! implementation detail of how many proposals a parent makes, and a future
//! container that probes once more per child must move the default rather than
//! quietly start evicting.

use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Size};
use gorgon_petra::layout::MeasureCache;
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{NodeKind, Props, ViewNode};

/// Panels of rows, none of them virtualized: the shape of a real inspector
/// screen rather than of a scrolling list.
///
/// `PANELS * ROWS + PANELS + 1` nodes.
const PANELS: usize = 128;
const ROWS: usize = 32;
const NODES: usize = PANELS * ROWS + PANELS + 1;

fn dense_tree() -> ViewNode {
    let mut root = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
        axis: Some(Axis::Vertical),
        ..Props::default()
    });
    for panel in 0..PANELS {
        let mut node = ViewNode::new(NodeKind::Stack, format!("panel-{panel}")).with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        });
        for row in 0..ROWS {
            node = node.child(
                ViewNode::new(NodeKind::Text, format!("row-{row}")).with_props(Props {
                    text: Some(format!("fiber {panel}/{row}")),
                    ..Props::default()
                }),
            );
        }
        root = root.child(node);
    }
    root
}

/// Negotiate `tree` once from cold in a cache bounded to `capacity`, and report
/// `(entries, evictions)`.
fn cold_pass(capacity: usize) -> (usize, u64) {
    let mut harness = Harness::new();
    harness.cache = MeasureCache::with_capacity(capacity);
    let tree = dense_tree();
    let _ = petrify(
        1,
        validated(&tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(900.0, 700.0), ThemeMode::Dark),
        TransitionActivity::default(),
    );
    (harness.cache.len(), harness.cache.evictions())
}

/// One cold negotiation of a dense screen must fit inside the shipped bound.
///
/// An eviction here is not a memory saving. Every entry dropped during a pass
/// is one the *same pass* may ask for again, so the cache spends the frame
/// evicting entries it is about to recompute — the failure mode SC-008's bound
/// exists to prevent, arriving through the other door.
#[test]
fn the_shipped_bound_fits_one_dense_screen() {
    let (entries, evictions) = cold_pass(MeasureCache::DEFAULT_CAPACITY);
    assert_eq!(
        evictions,
        0,
        "a {NODES}-node screen evicted {evictions} entries at the shipped bound of {}; \
         it holds {entries}",
        MeasureCache::DEFAULT_CAPACITY
    );
    assert!(
        entries > MeasureCache::DEFAULT_CAPACITY / 8,
        "this tree must actually exercise the bound, or the test proves nothing: \
         {entries} entries against a bound of {}",
        MeasureCache::DEFAULT_CAPACITY
    );
}

/// The control. A bound genuinely below the working set does evict, so the test
/// above is measuring the bound and not the absence of pressure.
#[test]
fn a_bound_below_the_working_set_does_evict() {
    let (entries, evictions) = cold_pass(1024);
    assert!(
        evictions > 0,
        "a 1024-entry bound must thrash on a {NODES}-node screen; it held {entries} \
         and evicted {evictions}"
    );
    assert!(entries <= 1024, "the bound still holds: {entries}");
}
