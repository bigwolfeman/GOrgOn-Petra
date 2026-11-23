//! Incremental frames through the shipped `Host`: an application that
//! rebuilds its whole tree every pass still pays only for what moved.
//!
//! The application here does what the GOrgOn shell does: it builds a fresh
//! `ViewNode` tree from its state on every pass and answers
//! `ChangeSet::in_tree()`, naming nothing. The host grafts each tree onto the
//! last (`gorgon_petra::tree::graft_shared`) and carries every unchanged
//! subtree over (`gorgon_petra::layout::reuse`). A collection's rows are
//! grafted by the engine onto its last fetch (`gorgon_petra::layout::row_memo`).
//!
//! Measurement is counted, not inferred from a digest: every leaf that takes
//! room is a `custom` node whose host measurer counts its calls by key. A
//! correct full negotiation and a correct incremental one produce the same
//! digest, so only the count can tell them apart.
//!
//! Every pass runs with the reuse audit on (`Host::set_reuse_audit`), except
//! the one whose measurements are counted: each frame that carried a subtree
//! over is checked against a full petrify of the same tree from a cold
//! cache, and a difference panics by name.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use egui::{Context, Event, Key, Modifiers, Pos2, RawInput};
use gorgon_petra::component;
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::geom::{Axis, Size};
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_egui::testing::{construct, step};

const PROBE: &str = "probe";
const PANELS: usize = 20;
const ROWS: usize = 50;
const LIST_ID: &str = "/root/list/rows";

/// Measure calls per probe key, shared with the measurer closure.
type Calls = Arc<Mutex<BTreeMap<String, usize>>>;

/// A leaf that takes `width` × 20 and is measured by the counting measurer.
/// Its key in the measurer's count is `name:width`, so a changed width is a
/// changed node and a different count entry.
fn probe(key: &str, width: u32) -> ViewNode {
    ViewNode::new(NodeKind::Custom, key).with_props(Props {
        custom_kind: Some(PROBE.to_owned()),
        custom_key: Some(format!("{key}:{width}")),
        ..Props::default()
    })
}

fn stack(key: &str, axis: Axis) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key).with_props(Props {
        axis: Some(axis),
        ..Props::default()
    })
}

fn text(key: &str, body: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key).with_props(Props {
        text: Some(body.to_owned()),
        ..Props::default()
    })
}

struct Rebuilds {
    /// The one leaf most passes change.
    counter: u64,
    panel_widths: Vec<u32>,
    row_widths: Vec<u32>,
    /// What `take_changes` answers.
    answer: ChangeSet,
}

impl Rebuilds {
    fn new() -> Self {
        Self {
            counter: 0,
            panel_widths: vec![40; PANELS],
            row_widths: vec![40; ROWS],
            answer: ChangeSet::in_tree(),
        }
    }
}

impl RowSource for Rebuilds {
    fn rows(&mut self, source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        assert_eq!(source, "rows");
        // Fresh allocations every call, as a row source that renders from
        // its store does.
        range
            .filter(|n| *n < ROWS)
            .map(|n| {
                Arc::new(
                    stack(&format!("r{n}"), Axis::Horizontal)
                        .child(probe(&format!("row-probe-{n}"), self.row_widths[n])),
                )
            })
            .collect()
    }
}

impl App for Rebuilds {
    fn view(&mut self) -> ViewNode {
        let mut panels = stack("panels", Axis::Vertical);
        for (n, width) in self.panel_widths.iter().enumerate() {
            panels = panels.child(
                stack(&format!("panel-{n}"), Axis::Horizontal)
                    .child(probe(&format!("panel-probe-{n}"), *width))
                    .child(text("label", &format!("Panel {n}"))),
            );
        }
        let list = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .with_constraints(Constraints {
                vertical: AxisConstraint {
                    min: Some(120.0),
                    max: Some(120.0),
                    priority: 0,
                },
                ..Constraints::default()
            })
            .child(
                ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
                    total_count: Some(ROWS),
                    source: Some("rows".to_owned()),
                    estimated_extent: Some(20.0),
                    ..Props::default()
                }),
            );
        stack("root", Axis::Vertical)
            .child(text("counter", &format!("pass {}", self.counter)))
            .child(
                stack("buttons", Axis::Horizontal)
                    .child(component::button("one", "One"))
                    .child(component::button("two", "Two")),
            )
            .child(panels)
            .child(list)
    }

    fn handle(&mut self, _: &InputEvent, _: &Route, _: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        self.answer.clone()
    }
}

struct Rig {
    ctx: Context,
    host: Host<Rebuilds>,
    calls: Calls,
    window: [f32; 2],
}

impl Rig {
    fn new() -> Self {
        let (ctx, mut host) = construct(Rebuilds::new());
        let calls: Calls = Arc::default();
        let counted = Arc::clone(&calls);
        host.registry_mut().register_custom_kind(PROBE);
        host.measurers_mut().register(PROBE, move |asked| {
            let key = asked.key.expect("every probe names itself");
            *counted
                .lock()
                .expect("calls")
                .entry(key.to_owned())
                .or_default() += 1;
            let width = key
                .rsplit_once(':')
                .and_then(|(_, w)| w.parse::<f32>().ok())
                .expect("a width");
            Size::new(width, 20.0)
        });
        // Drawn as nothing, so every probe is accounted for in the report.
        host.painters_mut().register(PROBE, |_, _| true);
        host.set_reuse_audit(true);
        let mut rig = Self {
            ctx,
            host,
            calls,
            window: [800.0, 900.0],
        };
        for _ in 0..3 {
            rig.pass(RawInput::default());
        }
        rig
    }

    fn pass(&mut self, mut input: RawInput) {
        input.screen_rect = Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(self.window[0], self.window[1]),
        ));
        step(&self.ctx, &mut self.host, input);
        let report = self.host.report().expect("a report");
        assert!(report.is_complete(), "incomplete pass: {report:?}");
    }

    /// Run one pass and return the probe measurements it took, by key.
    ///
    /// Without the audit for this one pass: the audit's full petrify
    /// measures every probe from a cold cache, which is its job, and would
    /// be counted here as the pass's own work.
    fn measured(&mut self, input: RawInput) -> BTreeMap<String, usize> {
        self.host.set_reuse_audit(false);
        self.calls.lock().expect("calls").clear();
        self.pass(input);
        let calls = std::mem::take(&mut *self.calls.lock().expect("calls"));
        self.host.set_reuse_audit(true);
        calls
    }

    fn width_of(&self, id: &str) -> f32 {
        self.host
            .frame()
            .expect("a frame")
            .placement(id)
            .unwrap_or_else(|| panic!("{id} is placed"))
            .rect
            .w
    }
}

fn key(key: Key, shift: bool) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift,
                ..Modifiers::NONE
            },
        }],
        ..RawInput::default()
    }
}

/// The main claim: one leaf moved, every other subtree was rebuilt by the
/// application as a fresh allocation, and none of it is measured again.
#[test]
fn an_unchanged_subtree_is_not_measured_again_when_one_leaf_moves() {
    let mut rig = Rig::new();
    rig.host.app_mut().counter += 1;
    let measured = rig.measured(RawInput::default());
    assert!(
        measured.is_empty(),
        "a pass that moved one text leaf re-measured {measured:?}"
    );
    let reuse = rig.host.reuse_stats();
    assert!(
        reuse.reused_nodes > reuse.replaced_nodes,
        "the pass carried {} placement(s) over and placed {} itself",
        reuse.reused_nodes,
        reuse.replaced_nodes
    );
}

/// The other half: what did move is measured again, and nothing beside it.
///
/// The panel narrows rather than widens on purpose. A panel wider than every
/// other widens the whole column, which offers every sibling a new width and
/// so a new measurement key: re-measuring them then is correct, not a missed
/// reuse (the arrangement term the note's §10 names). Narrowing one panel
/// inside a column its siblings still set leaves every other offer as it was.
#[test]
fn a_changed_node_is_measured_again_and_nothing_beside_it() {
    let mut rig = Rig::new();
    rig.host.app_mut().panel_widths[7] = 30;
    let measured = rig.measured(RawInput::default());
    assert!(
        measured.keys().all(|k| k == "panel-probe-7:30") && !measured.is_empty(),
        "changing panel 7 measured {measured:?}"
    );
    assert_eq!(rig.width_of("/root/panels/panel-7/panel-probe-7"), 30.0);
    assert_eq!(rig.width_of("/root/panels/panel-6/panel-probe-6"), 40.0);
}

/// A row that changed under its old key: the engine notices without the
/// application saying so, and the frame shows the new size.
#[test]
fn a_row_that_changed_under_its_key_is_measured_again_and_its_neighbours_are_not() {
    let mut rig = Rig::new();
    rig.host.app_mut().row_widths[2] = 90;
    let measured = rig.measured(RawInput::default());
    assert!(
        measured.keys().all(|k| k == "row-probe-2:90") && !measured.is_empty(),
        "changing row 2 measured {measured:?}"
    );
    assert_eq!(rig.width_of(&format!("{LIST_ID}/r2/row-probe-2")), 90.0);
    assert_eq!(rig.width_of(&format!("{LIST_ID}/r1/row-probe-1")), 40.0);
}

/// The control: `ChangeSet::All` is still the honest "I do not know", and
/// measures every probe on screen.
#[test]
fn change_set_all_still_measures_everything() {
    let mut rig = Rig::new();
    rig.host.app_mut().answer = ChangeSet::All;
    let measured = rig.measured(RawInput::default());
    for n in 0..PANELS {
        assert!(
            measured.contains_key(&format!("panel-probe-{n}:40")),
            "ChangeSet::All skipped panel {n}: {measured:?}"
        );
    }
    assert_eq!(rig.host.reuse_stats().reused_nodes, 0);
}

/// Every input a frame reads outside the tree, moved one at a time, under
/// the audit: focus, hover, press, scroll, resize, and theme. Each pass is
/// proved equal to a full petrify; the pass after each settles back to
/// carrying the frame over and measuring nothing.
#[test]
fn focus_hover_scroll_resize_and_theme_each_repaint_what_they_moved() {
    let mut rig = Rig::new();
    let settled = |rig: &mut Rig, what: &str| {
        rig.pass(RawInput::default());
        let measured = rig.measured(RawInput::default());
        assert!(
            measured.is_empty(),
            "after {what}, an idle pass measured {measured:?}"
        );
        assert!(
            rig.host.reuse_stats().reused_nodes > 0,
            "after {what}, an idle pass carried nothing over"
        );
    };

    // Focus: Tab onto the first button, then the second.
    rig.pass(key(Key::Tab, false));
    rig.pass(key(Key::Tab, false));
    assert!(rig.host.state().focused.is_some(), "Tab focused nothing");
    settled(&mut rig, "a focus move");

    // Hover: over the second button, then off it.
    let two = rig
        .host
        .frame()
        .expect("a frame")
        .placement("/root/buttons/two")
        .expect("two is placed")
        .rect;
    let over = Pos2::new(two.x + two.w / 2.0, two.y + two.h / 2.0);
    rig.pass(RawInput {
        events: vec![Event::PointerMoved(over)],
        ..RawInput::default()
    });
    assert!(
        rig.host.state().hovered.is_some(),
        "the pointer hovers nothing"
    );
    rig.pass(RawInput {
        events: vec![Event::PointerMoved(Pos2::new(790.0, 890.0))],
        ..RawInput::default()
    });
    settled(&mut rig, "a hover move");

    // Press and release on the first button.
    let one = rig
        .host
        .frame()
        .expect("a frame")
        .placement("/root/buttons/one")
        .expect("one is placed")
        .rect;
    let at = Pos2::new(one.x + one.w / 2.0, one.y + one.h / 2.0);
    for pressed in [true, false] {
        rig.pass(RawInput {
            events: vec![
                Event::PointerMoved(at),
                Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            ..RawInput::default()
        });
    }
    settled(&mut rig, "a press");

    // Scroll the list: new rows come into the window.
    rig.host
        .state_mut()
        .scroll_offsets
        .insert("/root/list".to_owned(), 300.0);
    rig.pass(RawInput::default());
    assert!(
        rig.host
            .frame()
            .expect("a frame")
            .placement(&format!("{LIST_ID}/r15"))
            .is_some(),
        "the scroll brought no new row into the window"
    );
    settled(&mut rig, "a scroll");

    // Resize the window.
    rig.window = [700.0, 860.0];
    rig.pass(RawInput::default());
    settled(&mut rig, "a resize");

    // Theme.
    rig.host.presenter().publish(gorgon_petra::token::light());
    rig.pass(RawInput::default());
    settled(&mut rig, "a theme change");
}
