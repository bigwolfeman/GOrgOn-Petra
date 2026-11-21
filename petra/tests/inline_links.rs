//! A link inside a paragraph is a control of its own (`TextRun::link`).
//!
//! The text stays one placement that paints every glyph. Each link run gets a
//! placement under it, keyed by the link, with one rectangle per line its
//! words land on. These tests hold the four things that design promises:
//! the fragments follow a wrap, Tab visits links in reading order between the
//! controls around the paragraph, the pointer hits a link on its words and
//! nowhere else, and the semantic tree lists each link with its own label.
//!
//! `MonoContent` answers 8 units a character and 16 a line, breaking
//! anywhere, so every rectangle below is arithmetic on byte offsets, widened
//! by `PAD` on each side for the focus ring.

use std::collections::BTreeMap;

use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Point, Rect, Size};
use gorgon_petra::input::hit_test;
use gorgon_petra::semantic::project;
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{
    Interaction, Key, NodeKind, Props, Role, RunLink, TextRun, TextWrap, ViewNode, Violation,
    validate,
};

/// The focus ring's stroke and halo, which a link's rectangles leave clear on
/// each side of its words.
const PAD: f32 = 3.0;

/// "Read the spec and the plan." with "the spec" and "the plan" as links.
const PROSE: &str = "Read the spec and the plan.";

fn run(len: usize) -> TextRun {
    TextRun {
        len,
        ..TextRun::default()
    }
}

fn link(len: usize, key: &str, target: &str) -> TextRun {
    TextRun {
        len,
        underline: true,
        link: Some(RunLink {
            key: Key::new(key),
            target: Some(target.to_owned()),
        }),
        ..TextRun::default()
    }
}

/// `PROSE` and its two links, wrapping.
fn paragraph() -> ViewNode {
    // "Read " 5, "the spec" 8, " and " 5, "the plan" 8, "." 1.
    ViewNode::new(NodeKind::Text, "p").with_props(Props {
        text: Some(PROSE.to_owned()),
        wrap: Some(TextWrap::Wrap),
        runs: vec![
            run(5),
            link(8, "spec", "spec.md"),
            run(5),
            link(8, "plan", "plan.md"),
            run(1),
        ],
        ..Props::default()
    })
}

fn button(key: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key)
        .with_props(Props {
            text: Some(key.to_owned()),
            ..Props::default()
        })
        .interactive(Role::Button, key, &[Interaction::Focus, Interaction::Click])
}

/// A button, the paragraph, a button, in a window `width` wide (the
/// `width` a [`frame_of`] call is given), so the paragraph wraps inside it.
fn page() -> ViewNode {
    ViewNode::new(NodeKind::Stack, "root")
        .child(button("before"))
        .child(paragraph())
        .child(button("after"))
}

fn frame_of(tree: &ViewNode, width: f32) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(width, 300.0), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn placement<'a>(frame: &'a PetrifiedFrame, id: &str) -> &'a gorgon_petra::frame::Placement {
    frame
        .placement(id)
        .unwrap_or_else(|| panic!("{id} is placed; placed: {:?}", ids(frame)))
}

fn ids(frame: &PetrifiedFrame) -> Vec<&str> {
    frame.placements.iter().map(|p| p.id.as_str()).collect()
}

/// On one line, each link is one rectangle over its own bytes.
#[test]
fn a_link_on_one_line_is_one_rectangle_over_its_words() {
    let frame = frame_of(&page(), 400.0);
    let text = placement(&frame, "/root/p").rect;
    let spec = placement(&frame, "/root/p/spec");
    assert_eq!(
        spec.rect,
        Rect::new(
            text.x + 5.0 * 8.0 - PAD,
            text.y,
            8.0 * 8.0 + 2.0 * PAD,
            16.0
        )
    );
    assert!(
        spec.semantics.fragments.len() <= 1,
        "{:?}",
        spec.semantics.fragments
    );
    let plan = placement(&frame, "/root/p/plan");
    assert_eq!(
        plan.rect,
        Rect::new(
            text.x + 18.0 * 8.0 - PAD,
            text.y,
            8.0 * 8.0 + 2.0 * PAD,
            16.0
        )
    );
    assert_eq!(
        plan.parent.map(|i| frame.placements[i].id.as_str()),
        Some("/root/p"),
        "a link hangs under its text"
    );
}

/// At 20 characters a line, "the spec" (bytes 5..13) fits on line one and
/// "the plan" (bytes 18..26) breaks after "th": two fragments, and its rect
/// is their bounding box.
#[test]
fn a_wrapped_link_is_one_fragment_per_line() {
    let frame = frame_of(&page(), 20.0 * 8.0);
    let text = placement(&frame, "/root/p").rect;
    let plan = placement(&frame, "/root/p/plan");
    let absolute: Vec<Rect> = plan
        .semantics
        .fragments
        .iter()
        .map(|f| f.translate(plan.rect.x, plan.rect.y))
        .collect();
    assert_eq!(
        absolute,
        vec![
            Rect::new(
                text.x + 18.0 * 8.0 - PAD,
                text.y,
                2.0 * 8.0 + 2.0 * PAD,
                16.0
            ),
            Rect::new(text.x - PAD, text.y + 16.0, 6.0 * 8.0 + 2.0 * PAD, 16.0),
        ],
        "line one holds `th`, line two `e plan`"
    );
    assert_eq!(
        plan.rect,
        Rect::new(text.x - PAD, text.y, 20.0 * 8.0 + 2.0 * PAD, 32.0),
        "the rect is the fragments' bounding box"
    );
}

/// The pointer hits a link on its words. Inside its bounding box but over
/// words that are not it, the link is not there.
#[test]
fn the_pointer_hits_a_link_on_its_words_and_nowhere_else() {
    let frame = frame_of(&page(), 20.0 * 8.0);
    let text = placement(&frame, "/root/p").rect;
    let on = |col: f32, line: f32| Point::new(text.x + col * 8.0 + 4.0, text.y + line * 16.0 + 8.0);
    let hit = |p: Point| hit_test(&frame, p, Interaction::Click).map(|p| p.id.as_str());
    assert_eq!(hit(on(19.0, 0.0)), Some("/root/p/plan"), "`th` on line one");
    assert_eq!(hit(on(0.0, 1.0)), Some("/root/p/plan"), "`e` on line two");
    assert_eq!(
        hit(on(7.0, 0.0)),
        Some("/root/p/spec"),
        "`spec` on line one"
    );
    assert_eq!(
        hit(on(6.0, 1.0)),
        None,
        "the full stop is inside the plan's bounding box and is not the link"
    );
    // Just past the pad at the end of `e plan`: still not the link.
    assert_eq!(
        hit(Point::new(text.x + 6.0 * 8.0 + PAD + 1.0, text.y + 24.0)),
        None
    );
}

/// Tab order: the control before the paragraph, the links in reading order,
/// then the control after it.
#[test]
fn focus_visits_links_in_reading_order_between_their_neighbours() {
    let frame = frame_of(&page(), 20.0 * 8.0);
    let focus = FocusTree::from_placements(&frame.placements, &BTreeMap::new());
    assert_eq!(
        focus.order(),
        [
            "/root/before",
            "/root/p/spec",
            "/root/p/plan",
            "/root/after"
        ]
    );
}

/// The semantic tree has each link as a `link` labelled with its own words,
/// with its target as the value, under the paragraph.
#[test]
fn the_semantic_tree_lists_each_link_with_its_own_label() {
    let frame = frame_of(&page(), 400.0);
    let tree = project(&frame).expect("a semantic tree");
    let links: Vec<(String, String, Option<String>)> = tree
        .iter()
        .filter(|n| n.role == Some(Role::Link))
        .map(|n| (n.id.clone(), n.label.clone(), n.value.clone()))
        .collect();
    assert_eq!(
        links,
        vec![
            (
                "/root/p/spec".to_owned(),
                "the spec".to_owned(),
                Some("spec.md".to_owned())
            ),
            (
                "/root/p/plan".to_owned(),
                "the plan".to_owned(),
                Some("plan.md".to_owned())
            ),
        ]
    );
    let paragraph = tree.iter().find(|n| n.id == "/root/p").expect("paragraph");
    assert_eq!(
        paragraph.children.len(),
        2,
        "both links are the paragraph's"
    );
}

/// A wrapped link's focus ring marks the line it starts on, not the box
/// around both lines.
#[test]
fn a_wrapped_link_marks_the_line_it_starts_on() {
    let frame = frame_of(&page(), 20.0 * 8.0);
    let at = frame
        .placements
        .iter()
        .position(|p| p.id == "/root/p/plan")
        .expect("placed");
    let plan = &frame.placements[at];
    let first = plan.semantics.fragments[0].translate(plan.rect.x, plan.rect.y);
    assert_eq!(
        gorgon_petra::focus::marked_rect(
            &frame.placements,
            at,
            gorgon_petra::tree::FocusFigure::Border
        ),
        first
    );
}

/// A link a line cap cuts off is not drawn, so it is neither placed nor
/// reachable.
#[test]
fn a_link_cut_off_by_a_line_cap_is_not_placed() {
    let mut tree = page();
    let p = std::sync::Arc::make_mut(&mut tree.children[1]);
    p.props.max_lines = Some(1);
    let frame = frame_of(&tree, 10.0 * 8.0);
    assert!(
        frame.placement("/root/p/spec").is_some(),
        "`the s` is on line one"
    );
    assert!(
        frame.placement("/root/p/plan").is_none(),
        "line three is cut"
    );
}

/// Acceptance refuses the three links that could not be placements.
#[test]
fn acceptance_refuses_a_link_that_cannot_be_a_placement() {
    let refused = |node: ViewNode| {
        let root = ViewNode::new(NodeKind::Stack, "root").child(node);
        let registry = gorgon_petra::tree::Registry::with_vocabulary(
            gorgon_petra::token::standard_vocabulary(),
        );
        match validate(&root, &registry) {
            Ok(_) => Vec::new(),
            Err(errors) => errors
                .as_slice()
                .iter()
                .filter_map(|v| match &v.violation {
                    Violation::TextRunLinkRefused { key, .. } => Some(key.clone()),
                    _ => None,
                })
                .collect(),
        }
    };
    let twice = ViewNode::new(NodeKind::Text, "t").with_props(Props {
        text: Some("abcd".to_owned()),
        runs: vec![link(2, "x", "a"), link(2, "x", "b")],
        ..Props::default()
    });
    assert_eq!(refused(twice), ["x"], "a repeated key");
    let empty = ViewNode::new(NodeKind::Text, "t").with_props(Props {
        text: Some("ab".to_owned()),
        runs: vec![link(0, "y", "a"), run(2)],
        ..Props::default()
    });
    assert_eq!(refused(empty), ["y"], "an empty stretch");
    let field = ViewNode::new(NodeKind::Input, "f")
        .with_props(Props {
            text: Some("ab".to_owned()),
            runs: vec![link(2, "z", "a")],
            ..Props::default()
        })
        .interactive(
            Role::TextInput,
            "f",
            &[Interaction::Focus, Interaction::Key],
        );
    assert_eq!(refused(field), ["z"], "a field");
}
