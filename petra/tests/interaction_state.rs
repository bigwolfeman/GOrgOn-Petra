//! The five interaction states reach the digest and the semantic tree.
//!
//! `contracts/interaction-state.md` §2 puts `hovered`, `active`, `captured`,
//! `read_only` and `skeleton` on the placement; §3 puts all five into the leaf
//! hash after `focused`, in that order, and bumps the frame domain to
//! `gorgon-petra-frame-v6`. Both halves are claims about the *picture*: each
//! state sends the same slot to a different token family, so two frames that
//! differ in one of them are two different pictures, exactly the argument
//! `focused` made at `v3`.
//!
//! Two of the five are app-declared and are driven here through the real
//! `petrify` path, the way `frame_digest_coverage.rs` drives its cases: a pair
//! of trees differing in exactly one `Semantics` field. The other three are
//! engine-derived from a pointer snapshot `LayoutState` does not carry yet, so
//! nothing can place a frame with them set and they are driven against
//! `digest::digest` directly. That is stated rather than hidden: when
//! `LayoutState` grows `hovered`/`pressed`/`capture`, these three move onto
//! the tree path and stop being the exception.

use gorgon_petra::frame::digest::digest;
use gorgon_petra::frame::{
    FrameDigest, PaintState, PetrifiedFrame, Placement, PlacementSemantics, TransitionActivity,
    Viewport, petrify,
};
use gorgon_petra::geom::{Rect, Size};
use gorgon_petra::semantic::{StateFlag, TreeQuery, project};
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{NodeKind, Props, Semantics, ViewNode};

fn viewport() -> Viewport {
    Viewport::new(Size::new(400.0, 200.0), ThemeMode::Dark)
}

fn frame_of(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        viewport(),
        TransitionActivity::default(),
    )
}

/// One text node carrying `semantics`, inside a stack — the smallest tree
/// that lays out identically whatever the semantic declarations say, so a
/// digest difference can only be the declaration.
fn labelled(semantics: Semantics) -> ViewNode {
    ViewNode::new(NodeKind::Stack, "app").child(
        ViewNode::new(NodeKind::Text, "field")
            .with_props(Props {
                text: Some("Fibers".into()),
                ..Props::default()
            })
            .with_semantics(semantics),
    )
}

/// A hand-built placement with every interaction flag clear.
fn placement() -> Placement {
    Placement {
        id: "/app/field".into(),
        kind: NodeKind::Text,
        rect: Rect::new(0.0, 0.0, 80.0, 20.0),
        z: 0,
        clip: Rect::new(0.0, 0.0, 400.0, 200.0),
        opacity: 1.0,
        paint: PaintState::default(),
        semantics: PlacementSemantics::default(),
        parent: None,
    }
}

fn dig(semantics: PlacementSemantics) -> FrameDigest {
    let mut p = placement();
    p.semantics = semantics;
    digest(&viewport(), &[p])
}

/// Each of the five states, on its own, is a different picture.
///
/// Five separate flips against one baseline, and every result compared with
/// every other result as well as with the baseline. The pairwise half is what
/// catches the mistake the leaf stream is most exposed to: two `w.bool` calls
/// in the wrong order, or one flag written twice and another not at all, would
/// still move the digest away from the baseline while making two different
/// states collide with each other.
#[test]
fn each_interaction_state_is_its_own_picture() {
    type Set = fn(&mut PlacementSemantics);
    let table: &[(&str, Set)] = &[
        ("hovered", |s| s.hovered = true),
        ("active", |s| s.active = true),
        ("captured", |s| s.captured = true),
        ("read_only", |s| s.read_only = true),
        ("skeleton", |s| s.skeleton = true),
    ];

    let baseline = dig(PlacementSemantics::default());
    let mut seen: Vec<(&str, FrameDigest)> = Vec::new();
    for (what, set) in table {
        let mut semantics = PlacementSemantics::default();
        set(&mut semantics);
        let moved = dig(semantics);
        assert_ne!(
            moved, baseline,
            "{what} decides the picture and the digest cannot see it"
        );
        for (other, previous) in &seen {
            assert_ne!(
                moved, *previous,
                "{what} and {other} produce one digest; the leaf stream is \
                 writing them in the wrong order or dropping one"
            );
        }
        seen.push((what, moved));
    }
}

/// `active` and `captured` are two bits, not one.
///
/// The state that separates them is real and reachable: a button pressed and
/// then dragged off its own rect keeps the capture — every move still routes
/// to it — and stops looking pressed (`contracts/interaction-state.md` §1).
/// A single bit standing for both would either leave the button lit with the
/// pointer elsewhere, or drop the gesture at the rect's edge.
#[test]
fn a_dragged_off_button_is_captured_without_being_active() {
    let pressed = dig(PlacementSemantics {
        active: true,
        captured: true,
        ..PlacementSemantics::default()
    });
    let dragged_off = dig(PlacementSemantics {
        active: false,
        captured: true,
        ..PlacementSemantics::default()
    });
    let released = dig(PlacementSemantics::default());

    assert_ne!(
        pressed, dragged_off,
        "a button dragged off its rect paints differently from one held on it"
    );
    assert_ne!(
        dragged_off, released,
        "a capture that outlives the press is still a capture"
    );
}

/// The two app-declared states, through the real tree path: the author writes
/// them on `Semantics`, `semantics_of` projects them, and the digest sees
/// them.
#[test]
fn read_only_and_skeleton_declared_on_a_tree_reach_the_digest() {
    let plain = frame_of(&labelled(Semantics::default()));
    let read_only = frame_of(&labelled(Semantics {
        read_only: true,
        ..Semantics::default()
    }));
    let skeleton = frame_of(&labelled(Semantics {
        skeleton: true,
        ..Semantics::default()
    }));

    // Same rects: the difference the digest reports is the declaration and
    // nothing else.
    let rects = |f: &PetrifiedFrame| f.placements.iter().map(|p| p.rect).collect::<Vec<_>>();
    assert_eq!(rects(&plain), rects(&read_only));
    assert_eq!(rects(&plain), rects(&skeleton));

    assert_ne!(plain.digest, read_only.digest, "read_only");
    assert_ne!(plain.digest, skeleton.digest, "skeleton");
    assert_ne!(read_only.digest, skeleton.digest, "read_only vs skeleton");
}

/// Read-only is not disabled, and the tree says so in two separate places.
///
/// `contracts/interaction-state.md` §5: a read-only node stays Tab-reachable
/// and stays a legal focus target, so an assistive technology told it was
/// disabled would skip a node the user can reach. The projection must
/// therefore publish the flag on its own, and a query for `disabled` must not
/// find it.
#[test]
fn a_read_only_node_is_not_a_disabled_node() {
    let frame = frame_of(&labelled(Semantics {
        read_only: true,
        ..Semantics::default()
    }));
    let tree = project(&frame).expect("the frame has placements");
    let matches = tree.find_all(&TreeQuery::new().with_state(StateFlag::ReadOnly));
    assert_eq!(matches.len(), 1, "exactly one node declared read-only");
    let node = matches[0];
    assert_eq!(node.id, "/app/field");
    assert!(node.state.read_only);
    assert!(!node.state.disabled, "read-only must not imply disabled");
    assert!(
        tree.find_all(&TreeQuery::new().with_state(StateFlag::Disabled))
            .is_empty(),
        "a `disabled` query must not return a read-only node"
    );
}

/// Every new state is queryable, and a node in none of them matches none of
/// them.
///
/// Driven off the placement rather than the tree because three of the five
/// have no tree path yet; `project` reads `placements`, which is a public
/// field, so setting the flag there exercises the same projection line a
/// placed frame would.
#[test]
fn every_new_state_flag_finds_exactly_the_node_it_is_set_on() {
    type Set = fn(&mut PlacementSemantics);
    let table: &[(StateFlag, Set)] = &[
        (StateFlag::Hovered, |s| s.hovered = true),
        (StateFlag::Active, |s| s.active = true),
        (StateFlag::Captured, |s| s.captured = true),
        (StateFlag::ReadOnly, |s| s.read_only = true),
        (StateFlag::Skeleton, |s| s.skeleton = true),
    ];

    let base = frame_of(&labelled(Semantics::default()));
    let target = base
        .placements
        .iter()
        .position(|p| p.id == "/app/field")
        .expect("the tree places its text node");

    for (flag, set) in table {
        let clean = project(&base).expect("the frame has placements");
        assert!(
            clean
                .find_all(&TreeQuery::new().with_state(*flag))
                .is_empty(),
            "{flag:?} must match nothing on a frame that declares nothing"
        );

        let mut frame = base.clone();
        set(&mut frame.placements[target].semantics);
        let tree = project(&frame).expect("the frame has placements");
        let found = tree.find_all(&TreeQuery::new().with_state(*flag));
        assert_eq!(found.len(), 1, "{flag:?} must find exactly one node");
        assert_eq!(found[0].id, "/app/field", "{flag:?}");
    }
}

/// The wire form omits a state that is not in force and names it when it is.
///
/// `contracts/semantic-tree.md`'s absent-means-`false` rule: a consumer
/// written against the older block reads an unchanged tree until a node is
/// actually in one of the new states.
#[test]
fn the_state_block_carries_only_the_states_in_force() {
    let plain = frame_of(&labelled(Semantics::default()));
    let json = serde_json::to_string(project(&plain).expect("placements").root())
        .expect("the tree serializes");
    for flag in ["hovered", "active", "captured", "read_only", "skeleton"] {
        assert!(
            !json.contains(flag),
            "{flag} must not reach the wire when it is not in force: {json}"
        );
    }

    let declared = frame_of(&labelled(Semantics {
        read_only: true,
        skeleton: true,
        ..Semantics::default()
    }));
    let json = serde_json::to_string(project(&declared).expect("placements").root())
        .expect("the tree serializes");
    assert!(json.contains(r#""read_only":true"#), "{json}");
    assert!(json.contains(r#""skeleton":true"#), "{json}");
}
