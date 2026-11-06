//! A Tab whose target is off screen scrolls toward it, and focus lands when
//! the scroll arrives.
//!
//! The scroll is a set of running position transitions (a shipped button
//! declares `button-press`), so the target is still off screen on the pass
//! that handled the key. These tests drive the production path
//! (`Host::pass` -> `step_focus` -> the landing in `pass`) through a vertical
//! `Scroll` with a 100 tall viewport that holds button A, 300 points of
//! nothing, then button B. The gap is larger than the viewport, so scrolling
//! to B pushes A, the node that still holds focus, off screen first.

use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use egui::{Context, Event, Key, Modifiers, RawInput};
use gorgon_petra::component;
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::geom::Axis;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_egui::testing::{boot, step};

/// Seconds of simulated clock per pass: 60 Hz.
const TICK: f64 = 1.0 / 60.0;
/// Passes a walk may take. The longest shipped transition is 140 ms, 9 passes.
const BUDGET: usize = 60;

struct Travel {
    /// Whether button B is in the tree.
    with_b: bool,
}

fn fixed(height: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(height),
            max: Some(height),
            priority: 0,
        },
        ..Constraints::default()
    }
}

impl RowSource for Travel {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Travel {
    fn view(&mut self) -> ViewNode {
        let mut stack = ViewNode::new(NodeKind::Stack, "list")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(component::button("a", "A"))
            .child(ViewNode::new(NodeKind::Spacer, "gap").with_constraints(fixed(300.0)));
        if self.with_b {
            stack = stack.child(component::button("b", "B"));
        }
        let scroll = ViewNode::new(NodeKind::Scroll, "scroll")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .with_constraints(fixed(100.0))
            .child(stack);
        ViewNode::new(NodeKind::Stack, "root").child(scroll)
    }

    fn handle(&mut self, _: &InputEvent, _: &Route, _: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

struct Rig {
    ctx: Context,
    host: Host<Travel>,
    clock: f64,
}

impl Rig {
    fn new() -> Self {
        let (ctx, host) = boot(Travel { with_b: true });
        let mut rig = Self {
            ctx,
            host,
            clock: 0.0,
        };
        rig.pass(RawInput::default());
        rig
    }

    /// One pass at the next clock tick. Every pass must be complete.
    fn pass(&mut self, mut input: RawInput) -> Duration {
        self.clock += TICK;
        input.time = Some(self.clock);
        let delay = step(&self.ctx, &mut self.host, input);
        let report = self.host.report().expect("a report");
        assert!(report.is_complete(), "incomplete pass: {report:?}");
        delay
    }

    fn tab(&mut self, shift: bool) {
        let mut input = RawInput::default();
        input.events.push(Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift,
                ..Modifiers::NONE
            },
        });
        self.pass(input);
    }

    fn focused(&self) -> Option<String> {
        self.host.state().focused.clone()
    }

    /// Pass until focus is `id`, within [`BUDGET`].
    fn until_focused(&mut self, id: &str) {
        for _ in 0..BUDGET {
            if self.focused().as_deref() == Some(id) {
                return;
            }
            self.pass(RawInput::default());
        }
        panic!("focus never reached {id}; it is on {:?}", self.focused());
    }

    /// Pass until a pass asks for no repaint, within [`BUDGET`]; return how
    /// many passes that took.
    fn until_idle(&mut self) -> usize {
        for n in 0..BUDGET {
            if self.pass(RawInput::default()) == Duration::MAX {
                return n;
            }
        }
        panic!("the host kept asking for repaints");
    }
}

#[test]
fn tab_across_a_gap_taller_than_the_viewport_lands_on_the_far_button() {
    let mut rig = Rig::new();
    assert_eq!(rig.focused().as_deref(), Some("/root/scroll/list/a"));
    rig.tab(false);
    rig.until_focused("/root/scroll/list/b");
    let placed = rig
        .host
        .frame()
        .expect("a frame")
        .placement("/root/scroll/list/b")
        .expect("b is placed");
    assert!(placed.is_visible(), "focus is on B but B is off screen");
    rig.until_idle();
}

#[test]
fn shift_tab_back_lands_on_the_near_button() {
    let mut rig = Rig::new();
    rig.tab(false);
    rig.until_focused("/root/scroll/list/b");
    rig.until_idle();
    rig.tab(true);
    rig.until_focused("/root/scroll/list/a");
    rig.until_idle();
}

#[test]
fn the_old_focus_leaving_the_viewport_is_in_transit_not_blind() {
    let mut rig = Rig::new();
    rig.tab(false);
    let mut transit = 0;
    for _ in 0..BUDGET {
        let report = rig.host.report().expect("a report");
        transit += report.focus_in_transit;
        assert_eq!(report.blind_focus, 0, "{report:?}");
        if rig.focused().as_deref() == Some("/root/scroll/list/b") {
            break;
        }
        rig.pass(RawInput::default());
    }
    assert!(
        transit > 0,
        "the walk never counted a pass as in transit, so the fixture does not cross the gap"
    );
}

#[test]
fn a_target_removed_mid_scroll_is_dropped_without_a_repaint_loop() {
    let mut rig = Rig::new();
    rig.tab(false);
    assert_ne!(
        rig.focused().as_deref(),
        Some("/root/scroll/list/b"),
        "the move to B has already landed, so nothing is removed mid-scroll"
    );
    rig.host.app_mut().with_b = false;
    rig.pass(RawInput::default());
    let settled = rig.until_idle();
    assert!(settled < BUDGET);
    // Idle stays idle: a dropped target leaves nothing to retry.
    for _ in 0..5 {
        assert_eq!(rig.pass(RawInput::default()), Duration::MAX);
    }
    // Nothing focusable is on screen once B is gone and the scroll has
    // moved past A, so the ordinary vanished-focus rule leaves focus unset.
    // The next Tab starts the ring over at A and brings it back.
    if let Some(focused) = rig.focused() {
        let placed = rig.host.frame().expect("a frame").placement(&focused);
        assert!(
            placed.is_some_and(|p| p.is_visible()),
            "focus is on {focused} and it is not on screen"
        );
    }
    rig.tab(false);
    rig.until_focused("/root/scroll/list/a");
    rig.until_idle();
}
