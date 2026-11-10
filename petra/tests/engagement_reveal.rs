//! Engagement reveal: controls that show only while the group around them is
//! hovered, holds focus, or is selected (`tree::Reveal`).
//!
//! Every case runs the real `petrify` path from a `LayoutState`, the same
//! snapshot a host publishes, so what is checked is what a host would paint
//! and route.

use std::collections::BTreeMap;
use std::sync::Arc;

use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{
    PetrifiedFrame, TransitionActivity, Viewport, petrify, petrify_with_memo,
};
use gorgon_petra::geom::{Axis, Point, Size};
use gorgon_petra::input::hit_test;
use gorgon_petra::layout::reuse::FrameMemo;
use gorgon_petra::layout::{ChangeSet, LayoutState};
use gorgon_petra::semantic::project;
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{
    Interaction, NodeKind, Props, Registry, Reveal, Role, ViewNode, Violation, validate,
};

fn viewport() -> Viewport {
    Viewport::new(Size::new(400.0, 200.0), ThemeMode::Dark)
}

fn stack(key: &str, axis: Axis, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(axis),
            ..Props::default()
        })
        .with_children(children)
}

fn text(key: &str, words: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key).with_props(Props {
        text: Some(words.into()),
        ..Props::default()
    })
}

/// A focusable, clickable, hoverable control.
fn control(key: &str, label: &str) -> ViewNode {
    text(key, label).interactive(
        Role::Button,
        label,
        &[Interaction::Click, Interaction::Focus, Interaction::Hover],
    )
}

/// One group: a title, then an action strip revealed by the group.
fn item(key: &str, selected: bool) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Horizontal,
        vec![
            text("title", key),
            stack(
                "actions",
                Axis::Horizontal,
                vec![control("rename", "Rename"), control("drop", "Drop")],
            )
            .revealed_with_group(),
        ],
    )
    .interactive(Role::ListItem, key, &[])
    .reveal_group();
    node.semantics.selected = selected;
    node
}

fn list(selected: Option<&str>) -> ViewNode {
    stack(
        "list",
        Axis::Vertical,
        ["a", "b"]
            .into_iter()
            .map(|key| item(key, selected == Some(key)))
            .collect(),
    )
}

fn frame_of(tree: &ViewNode, state: LayoutState) -> PetrifiedFrame {
    let mut harness = Harness::new();
    harness.state = state;
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        viewport(),
        TransitionActivity::default(),
    )
}

fn hovering(id: &str) -> LayoutState {
    LayoutState {
        hovered: Some(id.into()),
        ..LayoutState::default()
    }
}

fn focusing(id: &str) -> LayoutState {
    LayoutState {
        focused: Some(id.into()),
        ..LayoutState::default()
    }
}

/// Whether every placement at or under `id` is concealed, and at opacity 0.
/// Panics if the two disagree: concealment is one fact, painted and routed.
fn concealed(frame: &PetrifiedFrame, id: &str) -> bool {
    let under: Vec<_> = frame
        .placements
        .iter()
        .filter(|p| p.id == id || p.id.starts_with(&format!("{id}/")))
        .collect();
    assert!(!under.is_empty(), "nothing placed at {id}");
    let flags: Vec<bool> = under.iter().map(|p| p.semantics.concealed).collect();
    let all = flags.iter().all(|&c| c);
    assert!(
        all || flags.iter().all(|&c| !c),
        "{id}: half a subtree concealed: {flags:?}"
    );
    for p in &under {
        assert_eq!(
            p.opacity == 0.0,
            p.semantics.concealed,
            "{}: concealed must paint at opacity 0 and only then",
            p.id
        );
    }
    all
}

fn center(frame: &PetrifiedFrame, id: &str) -> Point {
    let rect = frame.placement(id).expect("placed").rect;
    Point::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
}

#[test]
fn with_nothing_engaged_every_action_strip_is_concealed() {
    let frame = frame_of(&list(None), LayoutState::default());
    assert!(concealed(&frame, "/list/a/actions"));
    assert!(concealed(&frame, "/list/b/actions"));
    // The group and its title are not revealed nodes; they always show.
    assert!(!concealed(&frame, "/list/a/title"));
    assert!(!frame.placement("/list/a").unwrap().semantics.concealed);
}

#[test]
fn hovering_the_group_reveals_its_own_actions_and_no_others() {
    let frame = frame_of(&list(None), hovering("/list/a"));
    assert!(!concealed(&frame, "/list/a/actions"));
    assert!(concealed(&frame, "/list/b/actions"));
}

#[test]
fn hovering_anything_inside_the_group_reveals_it() {
    for inside in ["/list/a/title", "/list/a/actions/rename"] {
        let frame = frame_of(&list(None), hovering(inside));
        assert!(!concealed(&frame, "/list/a/actions"), "hover on {inside}");
    }
    // A sibling whose name merely starts the same is not inside.
    let frame = frame_of(&list(None), hovering("/list/ab"));
    assert!(concealed(&frame, "/list/a/actions"));
}

#[test]
fn focus_within_the_group_reveals_it() {
    for inside in ["/list/b", "/list/b/actions/drop"] {
        let frame = frame_of(&list(None), focusing(inside));
        assert!(!concealed(&frame, "/list/b/actions"), "focus on {inside}");
        assert!(concealed(&frame, "/list/a/actions"), "focus on {inside}");
    }
}

#[test]
fn a_selected_group_shows_its_actions_without_pointer_or_focus() {
    let frame = frame_of(&list(Some("b")), LayoutState::default());
    assert!(!concealed(&frame, "/list/b/actions"));
    assert!(concealed(&frame, "/list/a/actions"));
}

/// No reflow: the strip keeps the exact box it has when shown, so a row
/// does not jitter as the pointer crosses it.
#[test]
fn concealment_keeps_every_rect() {
    let hidden = frame_of(&list(None), LayoutState::default());
    let shown = frame_of(&list(None), hovering("/list/a"));
    assert_eq!(hidden.placements.len(), shown.placements.len());
    for (h, s) in hidden.placements.iter().zip(&shown.placements) {
        assert_eq!(h.id, s.id);
        assert_eq!(h.rect, s.rect, "{} moved when revealed", h.id);
    }
}

/// A hidden control takes no hit; the pointer lands on the group behind it,
/// which is what engages the group and reveals the control.
#[test]
fn a_concealed_control_passes_hits_through_to_its_group() {
    let hidden = frame_of(&list(None), LayoutState::default());
    let at = center(&hidden, "/list/a/actions/rename");
    assert!(hit_test(&hidden, at, Interaction::Click).is_none());
    assert_eq!(
        hit_test(&hidden, at, Interaction::Hover).map(|p| p.id.as_str()),
        Some("/list/a"),
    );

    // The next frame, with that hover published, the control is hit.
    let shown = frame_of(&list(None), hovering("/list/a"));
    assert_eq!(
        hit_test(&shown, at, Interaction::Click).map(|p| p.id.as_str()),
        Some("/list/a/actions/rename"),
    );
    // And hovering it keeps the group engaged, so the reveal is stable.
    let held = frame_of(&list(None), hovering("/list/a/actions/rename"));
    assert!(!concealed(&held, "/list/a/actions"));
}

/// Keyboard and screen reader access never depend on the pointer.
#[test]
fn a_concealed_control_stays_in_the_focus_order_and_the_semantic_tree() {
    let hidden = frame_of(&list(None), LayoutState::default());
    let focus = FocusTree::from_placements(&hidden.placements, &BTreeMap::new());
    assert_eq!(
        focus.order(),
        [
            "/list/a/actions/rename",
            "/list/a/actions/drop",
            "/list/b/actions/rename",
            "/list/b/actions/drop",
        ]
    );
    let tree = project(&hidden).expect("projects");
    let rename = tree.find("/list/a/actions/rename").expect("listed");
    assert_eq!(rename.label, "Rename");
}

/// The frame changes only through the derived state: a hover somewhere
/// unrelated is the same frame as no hover at all, and reveal moves the
/// digest exactly as the opacity it drives does.
#[test]
fn the_digest_moves_only_with_engagement() {
    let tree = ViewNode::new(NodeKind::Stack, "page")
        .with_children(vec![list(None), control("elsewhere", "Elsewhere")]);
    let rest = frame_of(&tree, LayoutState::default());
    let again = frame_of(&tree, LayoutState::default());
    assert_eq!(rest.digest, again.digest);

    // `elsewhere` lights its own hover flag, so compare the list alone.
    let unrelated = frame_of(&tree, hovering("/page/elsewhere"));
    let list_of = |f: &PetrifiedFrame| {
        f.placements
            .iter()
            .filter(|p| p.id.starts_with("/page/list"))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(list_of(&rest), list_of(&unrelated));

    let engaged = frame_of(&tree, hovering("/page/list/a"));
    assert_ne!(rest.digest, engaged.digest);
}

/// An outer group holding an inner one: a revealed control of its own, and
/// the inner group with its strip.
fn nested() -> ViewNode {
    stack(
        "outer",
        Axis::Vertical,
        vec![
            text("title", "outer"),
            control("open", "Open").revealed_with_group(),
            item("inner", false),
        ],
    )
    .interactive(Role::ListItem, "outer", &[])
    .reveal_group()
}

/// The nearest group wins: hover or focus inside the inner group engages the
/// inner group only, and on the outer group's own parts the outer only.
#[test]
fn the_nearest_group_wins() {
    let shown = |state: LayoutState| {
        let frame = frame_of(&nested(), state);
        (
            !concealed(&frame, "/outer/open"),
            !concealed(&frame, "/outer/inner/actions"),
        )
    };
    // On the outer group, or its own title: the outer only.
    assert_eq!(shown(hovering("/outer")), (true, false));
    assert_eq!(shown(hovering("/outer/title")), (true, false));
    // On the inner group, inside it, or on its own control: the inner only.
    assert_eq!(shown(hovering("/outer/inner")), (false, true));
    assert_eq!(shown(hovering("/outer/inner/title")), (false, true));
    assert_eq!(
        shown(hovering("/outer/inner/actions/rename")),
        (false, true)
    );
    assert_eq!(shown(focusing("/outer/inner/actions/drop")), (false, true));
    // Focus on the outer group's control keeps the outer engaged.
    assert_eq!(shown(focusing("/outer/open")), (true, false));
    // Pointer on the inner, focus on the outer: each engages its own.
    let both = LayoutState {
        hovered: Some("/outer/inner/title".into()),
        focused: Some("/outer/open".into()),
        ..LayoutState::default()
    };
    assert_eq!(shown(both), (true, true));
}

/// Selection is the group's own fact, so a selected outer group shows its
/// controls whatever the pointer engages inside it.
#[test]
fn a_selected_outer_group_shows_while_an_inner_one_is_hovered() {
    let mut tree = nested();
    tree.semantics.selected = true;
    let frame = frame_of(&tree, hovering("/outer/inner/title"));
    assert!(!concealed(&frame, "/outer/open"));
    assert!(!concealed(&frame, "/outer/inner/actions"));
}

/// A carried-over subtree must not keep the reveal it was placed with.
#[test]
fn an_incremental_frame_reveals_like_a_full_one() {
    // A third group the hover never touches, so the pass has something to
    // carry over and the reuse path really runs.
    let tree = Arc::new(stack(
        "list",
        Axis::Vertical,
        vec![item("a", false), item("b", false), item("c", false)],
    ));
    let mut h = Harness::new();
    h.state = hovering("/list/a/title");
    let first = petrify(
        1,
        validated(&tree),
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
    h.state = hovering("/list/b/title");
    let dirty = memo
        .dirty_ids(&ChangeSet::None, &h.state)
        .expect("a dirty set");
    let (incremental, stats) = petrify_with_memo(
        2,
        validated(&tree),
        &mut h.ctx(),
        &memo,
        &dirty,
        viewport(),
        TransitionActivity::default(),
    );
    let full = frame_of(&tree, hovering("/list/b/title"));
    assert!(concealed(&incremental, "/list/a/actions"));
    assert!(!concealed(&incremental, "/list/b/actions"));
    assert_eq!(incremental.placements, full.placements);
    assert_eq!(incremental.digest, full.digest);
    assert!(
        stats.reused_subtrees > 0,
        "nothing was carried over: {stats:?}"
    );
}

#[test]
fn a_revealed_node_with_no_group_above_it_is_refused() {
    let tree = stack(
        "page",
        Axis::Vertical,
        vec![control("open", "Open").revealed_with_group()],
    );
    let errors = validate(&tree, &Registry::new()).expect_err("refused");
    assert!(errors.as_slice().iter().any(|e| e.path == "/page/open"
        && matches!(e.violation, Violation::RevealWithoutGroup)));
    // A contributed subtree may lean on a group its slot puts around it.
    assert!(!Violation::RevealWithoutGroup.judgeable_standalone());
}

#[test]
fn a_group_that_cannot_be_hovered_is_refused() {
    let mut tree = item("a", false);
    tree.interactions.clear();
    let errors = validate(&tree, &Registry::new()).expect_err("refused");
    assert!(
        errors
            .as_slice()
            .iter()
            .any(|e| e.path == "/a" && matches!(e.violation, Violation::RevealGroupWithoutHover))
    );
    // The builder is what makes this hard to get wrong.
    assert!(item("a", false).interactions.contains(&Interaction::Hover));
}

#[test]
fn reveal_rides_the_wire_and_is_absent_at_the_default() {
    let node = control("open", "Open").revealed_with_group();
    let wire = serde_json::to_value(&node).unwrap();
    assert_eq!(wire["reveal"], "with-group");
    let back: ViewNode = serde_json::from_value(wire).unwrap();
    assert_eq!(back.reveal, Reveal::WithGroup);
    let plain = serde_json::to_value(control("open", "Open")).unwrap();
    assert!(plain.get("reveal").is_none());
}
