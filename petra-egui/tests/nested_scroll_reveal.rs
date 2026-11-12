//! A vertical scroll that holds a horizontal one still reveals a focused
//! node by moving the vertical scroll, and Tab walks every focusable node in
//! order without bouncing.
//!
//! The fixture is the settings page's shape: a vertical `Scroll` with a 100
//! tall viewport holding a `tab_bar` (whose tabs sit in a horizontal `Scroll`
//! viewport), then a gap taller than the viewport, then buttons. Every
//! button below the gap starts off screen.

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

const TICK: f64 = 1.0 / 60.0;
const BUDGET: usize = 60;

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

struct Page;

impl RowSource for Page {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Page {
    fn view(&mut self) -> ViewNode {
        let strip = component::tab_bar(
            "tabs",
            vec![
                component::tab("one", "One", true),
                component::tab("two", "Two", false),
            ],
        );
        let stack = ViewNode::new(NodeKind::Stack, "list")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(strip)
            .child(ViewNode::new(NodeKind::Spacer, "gap").with_constraints(fixed(300.0)))
            .child(component::button("b", "B"))
            .child(component::button("c", "C"));
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
    host: Host<Page>,
    clock: f64,
}

impl Rig {
    fn new() -> Self {
        let (ctx, host) = boot(Page);
        let mut rig = Self {
            ctx,
            host,
            clock: 0.0,
        };
        rig.pass(RawInput::default());
        rig
    }

    fn pass(&mut self, mut input: RawInput) -> Duration {
        self.clock += TICK;
        input.time = Some(self.clock);
        step(&self.ctx, &mut self.host, input)
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

    /// Tab once, then pass until focus leaves where it was. Returns where it
    /// went, or where it stayed after the budget.
    fn tab_and_settle(&mut self, shift: bool) -> Option<String> {
        let from = self.focused();
        self.tab(shift);
        for _ in 0..BUDGET {
            if self.focused() != from {
                break;
            }
            self.pass(RawInput::default());
        }
        // Let the scroll finish so the next Tab starts from a settled frame.
        for _ in 0..BUDGET {
            if self.pass(RawInput::default()) == Duration::MAX {
                break;
            }
        }
        self.focused()
    }

    fn visible(&self, id: &str) -> bool {
        self.host
            .frame()
            .and_then(|f| f.placement(id))
            .is_some_and(|p| p.is_visible())
    }
}

const ONE: &str = "/root/scroll/list/tabs/viewport/row/one";
const TWO: &str = "/root/scroll/list/tabs/viewport/row/two";
const B: &str = "/root/scroll/list/b";
const C: &str = "/root/scroll/list/c";

#[test]
fn tab_walks_past_a_horizontal_strip_and_reveals_each_node_below_it() {
    let mut rig = Rig::new();
    // Start at the first tab, whatever the boot focus is.
    for _ in 0..8 {
        if rig.focused().as_deref() == Some(ONE) {
            break;
        }
        rig.tab_and_settle(false);
    }
    assert_eq!(rig.focused().as_deref(), Some(ONE));
    for want in [TWO, B, C] {
        let got = rig.tab_and_settle(false);
        assert_eq!(got.as_deref(), Some(want), "forward Tab skipped or stalled");
        assert!(rig.visible(want), "focus is on {want} but it is off screen");
    }
    for want in [B, TWO, ONE] {
        let got = rig.tab_and_settle(true);
        assert_eq!(got.as_deref(), Some(want), "Shift-Tab skipped or stalled");
        assert!(rig.visible(want), "focus is on {want} but it is off screen");
    }
}
