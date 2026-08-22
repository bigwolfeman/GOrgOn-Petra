//! An incremental frame must be indistinguishable from a full one.
//!
//! This is the only property that matters about subtree reuse, and it is the
//! one a digest comparison alone does not establish: a pass that reused
//! nothing at all would also produce a matching digest, and so would a pass
//! that reused everything wrongly if the wrongness happened to be invisible.
//! Every test here therefore checks two things together — that the frame
//! equals what a full negotiation produces, *and* that reuse actually
//! happened, read off [`ReuseStats`] rather than inferred.

use std::collections::BTreeSet;
use std::sync::Arc;

use gorgon_petra::frame::{TransitionActivity, Viewport, petrify, petrify_with_memo};
use gorgon_petra::geom::Size;
use gorgon_petra::layout::ChangeSet;
use gorgon_petra::layout::reuse::{FrameMemo, ReuseStats};
use gorgon_petra::testing::Harness;
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{Key, NodeKind, Props, ViewNode};

const VIEWPORT: Size = Size { w: 400.0, h: 600.0 };

fn viewport() -> Viewport {
    Viewport::new(VIEWPORT, ThemeMode::Dark)
}

fn row(key: &str, text: &str) -> Arc<ViewNode> {
    Arc::new(
        ViewNode::new(NodeKind::Text, Key::new(key)).with_props(Props {
            text: Some(text.to_owned()),
            ..Props::default()
        }),
    )
}

/// A panel of three rows, shared as one allocation so a caller can hand the
/// same subtree back next frame.
fn panel(key: &str, label: &str) -> Arc<ViewNode> {
    Arc::new(
        ViewNode::new(NodeKind::Stack, Key::new(key))
            .child_shared(row("a", &format!("{label} a")))
            .child_shared(row("b", &format!("{label} b")))
            .child_shared(row("c", &format!("{label} c"))),
    )
}

fn root_of(panels: &[Arc<ViewNode>]) -> Arc<ViewNode> {
    let mut root = ViewNode::new(NodeKind::Stack, "root");
    for p in panels {
        root = root.child_shared(Arc::clone(p));
    }
    Arc::new(root)
}

/// Negotiate `tree` from scratch, with nothing carried over.
fn full(tree: &Arc<ViewNode>) -> gorgon_petra::frame::PetrifiedFrame {
    let mut h = Harness::new();
    petrify(
        1,
        tree,
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    )
}

/// Negotiate `before`, adopt it as a memo, then negotiate `after` against it.
fn incremental(
    before: &Arc<ViewNode>,
    after: &Arc<ViewNode>,
    changes: &ChangeSet,
) -> (gorgon_petra::frame::PetrifiedFrame, ReuseStats) {
    let mut h = Harness::new();
    let first = petrify(
        1,
        before,
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let memo = FrameMemo::adopt(
        Arc::clone(before),
        first,
        h.state.clone(),
        h.theme_rev,
        h.scale,
    );
    // The host's job between frames, and the reason `MeasureCache::apply`
    // exists: without it the cache still holds the changed node's old size and
    // the frame is stale whatever reuse does.
    h.cache.apply(changes);
    let dirty = memo
        .dirty_ids(changes, &h.state)
        .expect("ChangeSet::All has no dirty set");
    petrify_with_memo(
        2,
        after,
        &mut h.ctx(),
        &memo,
        &dirty,
        viewport(),
        TransitionActivity::default(),
    )
}

/// Compare everything a consumer can observe, not just the digest.
fn assert_same_frame(
    incremental: &gorgon_petra::frame::PetrifiedFrame,
    full: &gorgon_petra::frame::PetrifiedFrame,
) {
    assert_eq!(
        incremental.placements.len(),
        full.placements.len(),
        "different placement counts"
    );
    for (i, (a, b)) in incremental
        .placements
        .iter()
        .zip(full.placements.iter())
        .enumerate()
    {
        assert_eq!(
            a, b,
            "placement {i} differs\nincremental: {a:#?}\nfull: {b:#?}"
        );
    }
    assert_eq!(incremental.content, full.content, "paint payloads differ");
    assert_eq!(incremental.subtree_len, full.subtree_len, "extents differ");
    assert_eq!(
        incremental.subtree_hashes, full.subtree_hashes,
        "subtree hashes differ"
    );
    assert_eq!(incremental.digest, full.digest, "digests differ");
}

/// Nothing changed at all: the same root allocation handed back.
#[test]
fn an_unchanged_tree_is_carried_over_whole() {
    let panels: Vec<_> = (0..4).map(|i| panel(&format!("p{i}"), "x")).collect();
    let tree = root_of(&panels);

    let (frame, stats) = incremental(&tree, &tree, &ChangeSet::None);
    assert_same_frame(&frame, &full(&tree));

    // One subtree reused: the root, covering every node.
    assert_eq!(stats.reused_subtrees, 1, "{stats:?}");
    assert_eq!(stats.reused_nodes, frame.placements.len(), "{stats:?}");
    assert_eq!(
        stats.replaced_nodes, 0,
        "nothing changed, so nothing should have been rebuilt: {stats:?}"
    );
}

/// One panel rebuilt, the rest handed back as the same allocations.
#[test]
fn untouched_siblings_are_not_rebuilt() {
    let panels: Vec<_> = (0..4).map(|i| panel(&format!("p{i}"), "x")).collect();
    let before = root_of(&panels);

    // The host rebuilds panel 0 with different text and hands back the other
    // three unchanged — exactly what an incremental application does. Rows
    // `b` and `c` are handed back as the same allocation too: only `a`
    // actually changed, and `change_set` below names only `a`. Rebuilding
    // `b`/`c` fresh here would be under-declaring by
    // `ReuseState::verify_declaration`'s own rule (F1) even though their
    // content is unchanged — that exact scenario is what
    // `an_undeclared_rebuild_panics_naming_the_node` below exists to prove.
    let mut after_panels = panels.clone();
    after_panels[0] = Arc::new(
        ViewNode::new(NodeKind::Stack, Key::new("p0"))
            .child_shared(row("a", "changed a"))
            .child_shared(Arc::clone(&panels[0].children[1]))
            .child_shared(Arc::clone(&panels[0].children[2])),
    );
    let after = root_of(&after_panels);

    let changes = ChangeSet::Nodes(BTreeSet::from(["/root/p0/a".to_owned()]));
    let (frame, stats) = incremental(&before, &after, &changes);
    assert_same_frame(&frame, &full(&after));

    // The three untouched panels are carried over whole (3 subtrees of 4
    // nodes each), and now that rows `b` and `c` are properly declared by
    // sharing their `Arc`, they are carried over individually too (2
    // subtrees of 1 node each) — 5 subtrees, 14 nodes in total. Only the
    // root, panel 0, and row `a` are rebuilt.
    assert_eq!(stats.reused_subtrees, 5, "{stats:?}");
    assert_eq!(stats.reused_nodes, 14, "{stats:?}");
    assert_eq!(stats.replaced_nodes, 3, "{stats:?}");
}

/// A declared change under a subtree stops it being carried over even when
/// the tree itself is pointer-identical. This is the case that matters for a
/// `collection`, whose rows come from the host and not from the tree.
#[test]
fn a_declared_change_defeats_pointer_identity() {
    let panels: Vec<_> = (0..4).map(|i| panel(&format!("p{i}"), "x")).collect();
    let tree = root_of(&panels);

    let changes = ChangeSet::Nodes(BTreeSet::from(["/root/p2/b".to_owned()]));
    let (frame, stats) = incremental(&tree, &tree, &changes);
    assert_same_frame(&frame, &full(&tree));

    assert!(
        stats.replaced_nodes >= 3,
        "the declared node, its panel and the root must all be rebuilt: {stats:?}"
    );
    assert!(
        stats.reused_nodes > 0,
        "the other three panels are still untouched: {stats:?}"
    );
    // The panel holding the declared change is rebuilt, so only three of the
    // four are carried over.
    assert_eq!(stats.reused_subtrees, 5, "{stats:?}");
}

/// Structural change: the last panel gains a row. The placement count moves,
/// so every index after it moves, and `parent` must still be right.
///
/// The growing panel is deliberately the **last** one. A panel that grows
/// pushes every later sibling down the main axis, which changes the slot each
/// of them is offered, and a changed slot correctly defeats reuse however
/// pointer-identical the subtree is. Putting the growth first would therefore
/// re-place the entire rest of the frame — correctly, but the test would prove
/// nothing about reuse. This is a real property of the design, not a quirk of
/// the fixture: reuse survives a resize only for siblings *before* it.
#[test]
fn a_subtree_that_grows_still_produces_the_full_frame() {
    let panels: Vec<_> = (0..3).map(|i| panel(&format!("p{i}"), "x")).collect();
    let before = root_of(&panels);

    let mut after_panels = panels.clone();
    after_panels[2] = Arc::new(
        ViewNode::new(NodeKind::Stack, Key::new("p2"))
            .child_shared(row("a", "x a"))
            .child_shared(row("b", "x b"))
            .child_shared(row("c", "x c"))
            .child_shared(row("d", "x d")),
    );
    let after = root_of(&after_panels);

    let changes = ChangeSet::Nodes(BTreeSet::from(["/root/p2".to_owned()]));
    let (frame, stats) = incremental(&before, &after, &changes);
    assert_same_frame(&frame, &full(&after));
    assert_eq!(
        stats.reused_subtrees, 2,
        "the two panels above the one that grew keep their slots: {stats:?}"
    );
    assert_eq!(stats.reused_nodes, 8, "{stats:?}");

    // Every parent index must point at a real earlier placement. A reused
    // subtree's parents are rebased, so a rebase off by the growth would show
    // up here rather than as a subtly wrong picture.
    for (i, p) in frame.placements.iter().enumerate() {
        if let Some(parent) = p.parent {
            assert!(parent < i, "placement {i} ({}) names parent {parent}", p.id);
        }
    }
}

/// The mirror of the test above: a panel that *shrinks* also moves every
/// later sibling, and the earlier ones still survive.
#[test]
fn a_subtree_that_shrinks_still_produces_the_full_frame() {
    let panels: Vec<_> = (0..3).map(|i| panel(&format!("p{i}"), "x")).collect();
    let before = root_of(&panels);

    let mut after_panels = panels.clone();
    after_panels[2] =
        Arc::new(ViewNode::new(NodeKind::Stack, Key::new("p2")).child_shared(row("a", "x a")));
    let after = root_of(&after_panels);

    let changes = ChangeSet::Nodes(BTreeSet::from(["/root/p2".to_owned()]));
    let (frame, stats) = incremental(&before, &after, &changes);
    assert_same_frame(&frame, &full(&after));
    assert_eq!(stats.reused_subtrees, 2, "{stats:?}");
    for (i, p) in frame.placements.iter().enumerate() {
        if let Some(parent) = p.parent {
            assert!(parent < i, "placement {i} ({}) names parent {parent}", p.id);
        }
    }
}

/// The slot condition, on its own.
///
/// The **first** panel gains a row, so the two after it shift down the main
/// axis. Those two are pointer-identical to the previous frame and nothing
/// under them is declared dirty, so pointer identity and the dirty set both
/// say "reuse" — and reusing them would place them at last frame's y, on top
/// of the panel that grew. Only the slot comparison stops it.
///
/// This test exists because the three conditions were sabotaged one at a time
/// and dropping the slot check broke nothing: every other test happened to
/// have a second reason to refuse. A condition no test can fail is a
/// condition that is not being tested.
#[test]
fn a_resized_sibling_moves_the_ones_after_it() {
    let panels: Vec<_> = (0..3).map(|i| panel(&format!("p{i}"), "x")).collect();
    let before = root_of(&panels);

    let mut after_panels = panels.clone();
    after_panels[0] = Arc::new(
        ViewNode::new(NodeKind::Stack, Key::new("p0"))
            .child_shared(row("a", "x a"))
            .child_shared(row("b", "x b"))
            .child_shared(row("c", "x c"))
            .child_shared(row("d", "x d")),
    );
    let after = root_of(&after_panels);

    let changes = ChangeSet::Nodes(BTreeSet::from(["/root/p0".to_owned()]));
    let (frame, stats) = incremental(&before, &after, &changes);

    // The frame must still be exactly what a full negotiation produces. This
    // is the assertion that goes red if a moved subtree is carried over: the
    // reused placements would hold the previous frame's coordinates.
    assert_same_frame(&frame, &full(&after));

    assert_eq!(
        stats.reused_subtrees, 0,
        "every panel after the one that grew was offered a different slot, so \
         none of them may be carried over: {stats:?}"
    );

    // And prove the panels really did move, so the test would notice if the
    // fixture ever stopped exercising this.
    let p1 = frame
        .placements
        .iter()
        .find(|p| p.id == "/root/p1")
        .expect("p1 is placed");
    let before_frame = full(&before);
    let p1_before = before_frame
        .placements
        .iter()
        .find(|p| p.id == "/root/p1")
        .expect("p1 was placed before too");
    assert_ne!(
        p1.rect.y, p1_before.rect.y,
        "the fixture must actually move p1, or this test proves nothing"
    );
}

/// F1: the debug verifier must panic, naming the node, when a change set
/// omits a node whose `Arc` moved.
///
/// Panel 0 is rebuilt in full, including row `b`, whose text content did not
/// change — but the change set names only row `a`. Row `b`'s Arc is a fresh
/// allocation that is neither `/root/p0/a` itself, an ancestor of it, nor a
/// descendant of it, so `ReuseState::verify_declaration` must catch it. This
/// is exactly the sloppiness `untouched_siblings_are_not_rebuilt` avoids by
/// sharing `b` and `c`'s `Arc` instead of rebuilding them.
#[test]
#[should_panic(expected = "/root/p0/b")]
fn an_undeclared_rebuild_panics_naming_the_node() {
    let panels: Vec<_> = (0..2).map(|i| panel(&format!("p{i}"), "x")).collect();
    let before = root_of(&panels);

    let mut after_panels = panels.clone();
    after_panels[0] = Arc::new(
        ViewNode::new(NodeKind::Stack, Key::new("p0"))
            .child_shared(row("a", "changed a"))
            .child_shared(row("b", "x b"))
            .child_shared(row("c", "x c")),
    );
    let after = root_of(&after_panels);

    let changes = ChangeSet::Nodes(BTreeSet::from(["/root/p0/a".to_owned()]));
    let _ = incremental(&before, &after, &changes);
}

/// F8: `FrameMemo` is `pub` with every field `pub`, so a caller from outside
/// this crate can build (or corrupt) one whose arrays disagree in length.
/// `ReuseState::reusable` must refuse such a memo — falling back to a full
/// negotiation — rather than let `ReuseState::subtree` slice past the end of
/// the shorter array.
///
/// Before the fix, `reusable` bounds-checked only `placements.len()`, which
/// this corruption leaves untouched, so it would have answered `true` for
/// the root and `subtree` would have sliced `memo.content[0..9]` against a
/// one-element `Vec` — an out-of-bounds panic, not a refusal.
#[test]
fn a_memo_with_mismatched_array_lengths_is_refused_not_panicked() {
    let panels: Vec<_> = (0..2).map(|i| panel(&format!("p{i}"), "x")).collect();
    let tree = root_of(&panels);

    let mut h = Harness::new();
    let first = petrify(
        1,
        &tree,
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let mut memo = FrameMemo::adopt(
        Arc::clone(&tree),
        first,
        h.state.clone(),
        h.theme_rev,
        h.scale,
    );

    assert!(
        memo.content.len() > 1,
        "fixture must produce more than one placement for this to prove anything"
    );
    memo.content.truncate(1);

    let dirty = BTreeSet::new();
    let (frame, stats) = petrify_with_memo(
        2,
        &tree,
        &mut h.ctx(),
        &memo,
        &dirty,
        viewport(),
        TransitionActivity::default(),
    );
    assert_same_frame(&frame, &full(&tree));
    assert_eq!(
        stats.reused_subtrees, 0,
        "a memo whose arrays disagree must be refused wholesale, not reused: {stats:?}"
    );
}

/// A theme change moves `PaintState::token_revision` on every placement while
/// leaving the tree pointer-identical and every slot the same. Nothing in the
/// per-subtree tests can see it, so reuse must be refused wholesale.
#[test]
fn a_theme_change_refuses_every_reuse() {
    let panels: Vec<_> = (0..3).map(|i| panel(&format!("p{i}"), "x")).collect();
    let tree = root_of(&panels);

    let mut h = Harness::new();
    let first = petrify(
        1,
        &tree,
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let memo = FrameMemo::adopt(
        Arc::clone(&tree),
        first,
        h.state.clone(),
        h.theme_rev,
        h.scale,
    );

    // The host bumps the theme snapshot.
    h.theme_rev = 2;
    h.cache.retain_theme_and_scale(2, h.scale);
    let mut next_viewport = viewport();
    next_viewport.theme_rev = 2;
    let dirty = BTreeSet::new();
    let (frame, stats) = petrify_with_memo(
        2,
        &tree,
        &mut h.ctx(),
        &memo,
        &dirty,
        next_viewport,
        TransitionActivity::default(),
    );

    assert_eq!(
        stats,
        ReuseStats::default(),
        "a theme change must set the whole memo aside, not reuse per subtree"
    );
    for p in &frame.placements {
        assert_eq!(
            p.paint.token_revision, 2,
            "{} kept the previous theme revision",
            p.id
        );
    }
}
