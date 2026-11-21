//! A focus request lands on a row of a virtualized tree whatever gives the
//! tree its height.
//!
//! The tree is a scroll over a collection of 1000 `treeitem` rows. It takes
//! the height its siblings leave in one of three ways: as the whole window,
//! as a grid's middle row between two fit-content rows, and as the priority
//! child of a vertical stack. The application asks, once, for row 700 to be
//! revealed and focused. Row 700 is below the fold in every layout.

use std::ops::Range;
use std::sync::Arc;

use egui::{Pos2, RawInput, Rect as EguiRect, vec2};
use gorgon_petra::frame::{FocusTarget, PetrifiedFrame, RevealAlign, ScrollRequest};
use gorgon_petra::geom::Axis;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::tree::{
    AxisConstraint, Constraints, Interaction, NodeKind, Props, Role, TrackSize, ViewNode,
};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_egui::testing::{boot_in, headless, step};

const WINDOW: [f32; 2] = [400.0, 600.0];

#[derive(Clone, Copy, Debug)]
enum Shape {
    /// The scroll is the whole window: the control.
    Alone,
    /// A grid whose rows are fit-content, fit-content, fit-content: header,
    /// tree, footer. The tree's row is clamped to what the header and the
    /// footer leave.
    GridFitContent,
    /// A grid whose middle row is a weight: the leftover by name.
    GridWeight,
    /// A vertical stack of header, tree, footer, the tree with the highest
    /// vertical priority so it is the child that takes the leftover.
    StackPriority,
    /// A grid of two rows, the tree's a weight and a block of sections
    /// under it fit-content: the sidebar the source-control worker tried.
    GridTreeThenSections,
    /// A vertical stack of the tree and the sections, the sections with the
    /// higher priority: the other form that worker tried.
    StackSectionsPriority,
    /// [`Shape::GridTreeThenSections`] with a section that is itself a long
    /// scrolling list, and the tree declaring a minimum. Before the grid
    /// reserved weight tracks' minimums, the fit-content row took the whole
    /// window and the tree was placed zero tall, where no row can take focus.
    GridTreeOverLongList,
    /// [`Shape::StackSectionsPriority`] with the long list, the tree
    /// declaring a minimum.
    StackLongListPriority,
}

/// The least the tree keeps when a long section shares its axis.
const TREE_FLOOR: f32 = 120.0;

fn floored_tree() -> ViewNode {
    let mut node = tree();
    node.constraints.vertical.min = Some(TREE_FLOOR);
    node
}

/// Twelve rows of words and a button: a block taller than a header.
fn sections() -> ViewNode {
    let mut block = ViewNode::new(NodeKind::Stack, "sections").with_props(Props {
        axis: Some(Axis::Vertical),
        ..Props::default()
    });
    for i in 0..12 {
        block = block.child(label(&format!("section-{i}")));
    }
    block.child(label("commit").interactive(
        Role::Button,
        "Commit",
        &[Interaction::Focus, Interaction::Click],
    ))
}

struct Tree {
    shape: Shape,
    reveal: Option<ScrollRequest>,
    focus: Option<FocusTarget>,
}

impl RowSource for Tree {
    fn rows(&mut self, _source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        range
            .map(|i| {
                Arc::new(
                    ViewNode::new(NodeKind::Text, format!("item-{i}"))
                        .with_props(Props {
                            text: Some(format!("item {i}")),
                            ..Props::default()
                        })
                        .interactive(Role::TreeItem, format!("item {i}"), &[Interaction::Focus]),
                )
            })
            .collect()
    }
}

fn label(key: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key).with_props(Props {
        text: Some(key.to_owned()),
        ..Props::default()
    })
}

fn tree() -> ViewNode {
    let rows = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
        total_count: Some(1000),
        source: Some("tree".into()),
        estimated_extent: Some(20.0),
        ..Props::default()
    });
    ViewNode::new(NodeKind::Scroll, "scroll")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .interactive(Role::Tree, "tree", &[Interaction::Scroll])
        .child(rows)
}

impl App for Tree {
    fn view(&mut self) -> ViewNode {
        match self.shape {
            Shape::Alone => ViewNode::new(NodeKind::Stack, "root").child(tree()),
            Shape::GridFitContent | Shape::GridWeight => {
                let middle = match self.shape {
                    Shape::GridWeight => TrackSize::Weight { weight: 1.0 },
                    _ => TrackSize::FitContent,
                };
                ViewNode::new(NodeKind::Grid, "root")
                    .with_props(Props {
                        columns: vec![TrackSize::Weight { weight: 1.0 }],
                        rows: vec![TrackSize::FitContent, middle, TrackSize::FitContent],
                        ..Props::default()
                    })
                    .child(label("header"))
                    .child(tree())
                    .child(label("footer"))
            }
            Shape::GridTreeThenSections => ViewNode::new(NodeKind::Grid, "root")
                .with_props(Props {
                    columns: vec![TrackSize::Weight { weight: 1.0 }],
                    rows: vec![TrackSize::Weight { weight: 1.0 }, TrackSize::FitContent],
                    ..Props::default()
                })
                .child(tree())
                .child(sections()),
            Shape::GridTreeOverLongList => ViewNode::new(NodeKind::Grid, "root")
                .with_props(Props {
                    columns: vec![TrackSize::Weight { weight: 1.0 }],
                    rows: vec![TrackSize::Weight { weight: 1.0 }, TrackSize::FitContent],
                    ..Props::default()
                })
                .child(floored_tree())
                .child(scrolling_sections()),
            Shape::StackLongListPriority => {
                let mut list = scrolling_sections();
                list.constraints.vertical.priority = 1;
                ViewNode::new(NodeKind::Stack, "root")
                    .with_props(Props {
                        axis: Some(Axis::Vertical),
                        ..Props::default()
                    })
                    .child(floored_tree())
                    .child(list)
            }
            Shape::StackSectionsPriority => {
                let mut block = sections();
                block.constraints.vertical.priority = 1;
                ViewNode::new(NodeKind::Stack, "root")
                    .with_props(Props {
                        axis: Some(Axis::Vertical),
                        ..Props::default()
                    })
                    .child(tree())
                    .child(block)
            }
            Shape::StackPriority => {
                let mut scroll = tree();
                scroll.constraints = Constraints {
                    vertical: AxisConstraint {
                        priority: 1,
                        ..AxisConstraint::default()
                    },
                    ..Constraints::default()
                };
                ViewNode::new(NodeKind::Stack, "root")
                    .with_props(Props {
                        axis: Some(Axis::Vertical),
                        ..Props::default()
                    })
                    .child(label("header"))
                    .child(scroll)
                    .child(label("footer"))
            }
        }
    }

    fn handle(&mut self, _: &InputEvent, _: &Route, _: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }

    fn scroll_request(&mut self) -> Option<ScrollRequest> {
        self.reveal.take()
    }

    fn focus_request(&mut self) -> Option<FocusTarget> {
        self.focus.take()
    }
}

fn sized() -> RawInput {
    RawInput {
        screen_rect: Some(EguiRect::from_min_size(
            Pos2::ZERO,
            vec2(WINDOW[0], WINDOW[1]),
        )),
        ..RawInput::default()
    }
}

/// Boot `shape`, ask for row 700, run a few passes, and return the host.
fn asked(shape: Shape) -> Host<Tree> {
    let (ctx, mut host) = boot_in(
        headless(),
        Tree {
            shape,
            reveal: None,
            focus: None,
        },
        gorgon_petra_egui::host::default_presenter(),
        sized(),
    );
    step(&ctx, &mut host, sized());
    host.app_mut().focus = Some(FocusTarget::Row {
        source: "tree".into(),
        key: "item-700".into(),
    });
    host.app_mut().reveal = Some(ScrollRequest::row("tree", 700, RevealAlign::Center));
    for _ in 0..6 {
        step(&ctx, &mut host, sized());
    }
    host
}

fn assert_lands(shape: Shape) {
    let host = asked(shape);
    let frame = host.frame().expect("a frame");
    let scroll = frame
        .placements
        .iter()
        .find(|p| p.id.ends_with("/scroll"))
        .expect("the tree's scroll is placed");
    assert!(
        scroll.rect.h > 0.0 && scroll.rect.h < WINDOW[1] + 1.0,
        "{shape:?}: the tree is given the leftover height, not its content's: {:?}",
        scroll.rect
    );
    let focused = host.focus().current().map(str::to_owned);
    assert!(
        focused
            .as_deref()
            .is_some_and(|id| id.ends_with("/rows/item-700")),
        "{shape:?}: the request lands on row 700; focus is {focused:?}; \
         placed rows: {:?}",
        frame
            .placements
            .iter()
            .filter(|p| p.id.contains("/rows/item-"))
            .map(|p| (
                p.id.rsplit('/').next().unwrap_or_default().to_owned(),
                p.is_visible()
            ))
            .take(4)
            .collect::<Vec<_>>()
    );
}

/// Type-ahead: focus is on row 2, a key moves the model to row 5, which is
/// already on screen, and the application asks for it the way the shell's
/// tree does, a nearest reveal and a focus request in the same pass.
fn assert_type_ahead_lands(shape: Shape) {
    let (ctx, mut host) = boot_in(
        headless(),
        Tree {
            shape,
            reveal: None,
            focus: None,
        },
        gorgon_petra_egui::host::default_presenter(),
        sized(),
    );
    step(&ctx, &mut host, sized());
    for key in ["item-2", "item-5"] {
        host.app_mut().focus = Some(FocusTarget::Row {
            source: "tree".into(),
            key: key.into(),
        });
        let index: usize = key[5..].parse().expect("index");
        host.app_mut().reveal = Some(ScrollRequest::row("tree", index, RevealAlign::Nearest));
        for _ in 0..3 {
            step(&ctx, &mut host, sized());
        }
        let focused = host.focus().current().map(str::to_owned);
        assert!(
            focused
                .as_deref()
                .is_some_and(|id| id.ends_with(&format!("/rows/{key}"))),
            "{shape:?}: the request lands on {key}; focus is {focused:?}; tree rect {:?}",
            host.frame()
                .and_then(|f| f.placements.iter().find(|p| p.id.ends_with("/scroll")))
                .map(|p| p.rect)
        );
    }
}

#[test]
fn type_ahead_lands_in_every_shape() {
    for shape in [
        Shape::Alone,
        Shape::GridFitContent,
        Shape::GridWeight,
        Shape::StackPriority,
        Shape::GridTreeThenSections,
        Shape::StackSectionsPriority,
        Shape::GridTreeOverLongList,
        Shape::StackLongListPriority,
    ] {
        assert_type_ahead_lands(shape);
    }
}

/// A long fit-content list leaves the weighted tree its declared minimum.
/// Falsify by dropping `weight_floor` from `grid::spend_declared`: the tree
/// is placed zero tall and the request never lands.
#[test]
fn a_focus_request_lands_on_a_tree_above_a_long_fit_content_list() {
    assert_lands(Shape::GridTreeOverLongList);
    let host = asked(Shape::GridTreeOverLongList);
    let tree = host
        .frame()
        .and_then(|f| f.placements.iter().find(|p| p.id.ends_with("/scroll")))
        .map(|p| p.rect.h);
    assert_eq!(tree, Some(TREE_FLOOR), "the list takes the rest");
}

#[test]
fn a_focus_request_lands_on_a_tree_under_a_prioritised_long_list() {
    assert_lands(Shape::StackLongListPriority);
}

#[test]
fn a_focus_request_lands_on_a_tree_above_fit_content_sections() {
    assert_lands(Shape::GridTreeThenSections);
}

#[test]
fn a_focus_request_lands_on_a_tree_under_prioritised_sections() {
    assert_lands(Shape::StackSectionsPriority);
}

#[test]
fn a_focus_request_lands_on_a_tree_that_is_the_whole_window() {
    assert_lands(Shape::Alone);
}

#[test]
fn a_focus_request_lands_on_a_tree_in_a_grid_fit_content_row() {
    assert_lands(Shape::GridFitContent);
}

#[test]
fn a_focus_request_lands_on_a_tree_in_a_grid_weight_row() {
    assert_lands(Shape::GridWeight);
}

#[test]
fn a_focus_request_lands_on_a_tree_given_stack_priority() {
    assert_lands(Shape::StackPriority);
}

/// A git status list: a scroll over 80 rows, longer than the window.
fn scrolling_sections() -> ViewNode {
    let rows = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
        total_count: Some(80),
        source: Some("git".into()),
        estimated_extent: Some(24.0),
        ..Props::default()
    });
    ViewNode::new(NodeKind::Scroll, "git")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(rows)
}
