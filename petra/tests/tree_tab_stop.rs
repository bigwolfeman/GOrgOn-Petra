//! A tree is one Tab stop (Carbon's tree view, roving tabindex).
//!
//! Every row stays focusable, so the pointer, a programmatic move and the
//! application's arrows reach it, but Tab visits one row: the focused one
//! when focus is in the tree, else the selected one, else the first.

use std::collections::BTreeMap;

use gorgon_petra::component::{tree_item, tree_view};
use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{Interaction, NodeKind, Props, Role, ViewNode};

fn button(key: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key)
        .with_props(Props {
            text: Some(key.to_owned()),
            ..Props::default()
        })
        .interactive(Role::Button, key, &[Interaction::Focus, Interaction::Click])
}

/// A button, a tree of three rows with `selected` selected (if any), a
/// button.
fn page(selected: Option<&str>) -> ViewNode {
    let rows = ["a", "b", "c"]
        .into_iter()
        .map(|k| tree_item(k, k, false, Some(k) == selected, Vec::new()))
        .collect();
    ViewNode::new(NodeKind::Stack, "root")
        .child(button("before"))
        .child(tree_view("tree", rows))
        .child(button("after"))
}

fn frame_of(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(400.0, 400.0), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

/// The ids Tab visits from `before`, until it comes back round.
fn tab_walk(frame: &PetrifiedFrame, dir: isize) -> Vec<String> {
    let scopes = BTreeMap::new();
    let mut focus = FocusTree::from_placements(&frame.placements, &scopes);
    focus.focus("/root/before").expect("focusable");
    let mut seen = vec!["/root/before".to_owned()];
    for _ in 0..10 {
        if dir > 0 {
            focus.next();
        } else {
            focus.previous();
        }
        let at = focus.current().expect("focus").to_owned();
        if at == "/root/before" {
            break;
        }
        seen.push(at);
    }
    seen
}

#[test]
fn tab_visits_the_first_row_of_a_tree_with_nothing_selected() {
    let frame = frame_of(&page(None));
    assert_eq!(
        tab_walk(&frame, 1),
        ["/root/before", "/root/tree/a", "/root/after"]
    );
}

#[test]
fn tab_and_shift_tab_land_on_the_selected_row() {
    let frame = frame_of(&page(Some("b")));
    assert_eq!(
        tab_walk(&frame, 1),
        ["/root/before", "/root/tree/b", "/root/after"]
    );
    assert_eq!(
        tab_walk(&frame, -1),
        ["/root/before", "/root/after", "/root/tree/b"]
    );
}

/// The arrows are the application's: a row it focused is the tree's stop,
/// and Tab from it leaves the tree.
#[test]
fn every_row_stays_focusable_and_tab_from_inside_leaves_the_tree() {
    let frame = frame_of(&page(Some("b")));
    let scopes = BTreeMap::new();
    let mut focus = FocusTree::from_placements(&frame.placements, &scopes);
    assert!(
        focus.order().iter().any(|id| id == "/root/tree/c"),
        "a row that is not the stop is still focusable: {:?}",
        focus.order()
    );
    focus
        .focus("/root/tree/c")
        .expect("a programmatic move reaches it");
    focus.next();
    assert_eq!(focus.current(), Some("/root/after"));
    focus.focus("/root/tree/c").expect("focusable");
    focus.previous();
    assert_eq!(focus.current(), Some("/root/before"));
    let reach = focus.reachable(&frame.placements, &scopes, 1);
    assert_eq!(
        reach.as_deref(),
        Some("/root/tree/b"),
        "the host's Tab path skips to the tree's selected row as well"
    );
}
