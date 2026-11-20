//! The first frame after a layout change paints one frame, not two.
//!
//! A reflow moved a button and a toggle down the page. On the first frame
//! after it, the button's fill and the focus bar painted where the previous
//! frame had placed them while the button's label, and every plain text,
//! painted where the new frame placed it. About 70 to 160 ms later the
//! picture was right. Two things caused it: the transition engine moved an
//! animated box without its subtree (and animated a press definition on
//! layout travel, and a knob on its ancestor's travel), and the focus caret
//! flew to its own node's new rect instead of riding with it.
//!
//! The check is whole-picture: after a reflow that changes no state, the
//! first frame must paint exactly what a host that booted straight into the
//! new layout paints. Every shape, by count and by visual bounds.

use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use egui::epaint::ClippedShape;
use egui::{Context, RawInput};
use gorgon_petra::component;
use gorgon_petra::frame::{FocusTarget, PetrifiedFrame};
use gorgon_petra::geom::Axis;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_egui::testing::boot;

const TICK: f64 = 1.0 / 60.0;
const BUDGET: usize = 120;
const BUTTON: &str = "/root/page/list/go";

/// A page whose leading gap the test can grow: the button, the toggle and
/// the text below it all move down when it does.
struct Page {
    lead: f32,
    focus: Option<FocusTarget>,
}

impl RowSource for Page {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

fn tall(height: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(height),
            max: Some(height),
            priority: 10,
        },
        ..Constraints::default()
    }
}

impl App for Page {
    fn view(&mut self) -> ViewNode {
        let list = ViewNode::new(NodeKind::Stack, "list")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Spacer, "lead").with_constraints(tall(self.lead)))
            .child(component::button("go", "Go"))
            .child(component::toggle("switch", "Switch", true))
            .child(component::text("note", "stored at user"));
        let page = ViewNode::new(NodeKind::Scroll, "page")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(list);
        ViewNode::new(NodeKind::Stack, "root").child(page)
    }

    fn handle(&mut self, _: &InputEvent, _: &Route, _: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }

    fn focus_request(&mut self) -> Option<FocusTarget> {
        self.focus.take()
    }
}

struct Rig {
    ctx: Context,
    host: Host<Page>,
    clock: f64,
}

impl Rig {
    /// Boot at `lead`, focus the button, and pass until nothing asks for a
    /// frame: every caret hop and transition has landed.
    fn settled(lead: f32) -> Self {
        let (ctx, host) = boot(Page {
            lead,
            focus: Some(FocusTarget::Node(BUTTON.to_owned())),
        });
        let mut rig = Self {
            ctx,
            host,
            clock: 0.0,
        };
        for _ in 0..BUDGET {
            if rig.pass().1 == Duration::MAX {
                break;
            }
        }
        assert_eq!(
            rig.host.state().focused.as_deref(),
            Some(BUTTON),
            "the fixture focuses the button so the caret is in the picture"
        );
        rig
    }

    /// One pass; the shapes it painted and the repaint delay it asked for.
    fn pass(&mut self) -> (Vec<ClippedShape>, Duration) {
        self.clock += TICK;
        let input = RawInput {
            time: Some(self.clock),
            ..RawInput::default()
        };
        let host = &mut self.host;
        let ctx = &self.ctx;
        let out = self.ctx.run_ui(input, |_| host.pass(ctx));
        let delay = out
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(Duration::MAX);
        (out.shapes, delay)
    }
}

/// Every shape's visual bounds, in paint order, rounded to a hundredth of a
/// unit so float noise in tessellation-free bounds cannot fail the check.
fn bounds(shapes: &[ClippedShape]) -> Vec<[i64; 4]> {
    shapes
        .iter()
        .map(|s| {
            let r = s.shape.visual_bounding_rect();
            let q = |v: f32| (v * 100.0).round() as i64;
            [q(r.min.x), q(r.min.y), q(r.max.x), q(r.max.y)]
        })
        .collect()
}

#[test]
fn the_first_frame_after_a_reflow_paints_the_new_layout_only() {
    let mut moved = Rig::settled(20.0);
    let mut reference = Rig::settled(46.0);
    // Same clock on both, so nothing time-driven can tell them apart.
    moved.clock = reference.clock;

    moved.host.app_mut().lead = 46.0;
    let (first, _) = moved.pass();
    let (want, _) = reference.pass();

    let got = bounds(&first);
    let wanted = bounds(&want);
    assert_eq!(
        got.len(),
        wanted.len(),
        "the first frame after the reflow painted a different number of shapes"
    );
    let torn: Vec<String> = got
        .iter()
        .zip(&wanted)
        .enumerate()
        .filter(|(_, (g, w))| g != w)
        .map(|(i, (g, w))| format!("shape {i}: painted {g:?}, settled {w:?}"))
        .collect();
    assert!(
        torn.is_empty(),
        "the first frame after a reflow mixed two layouts ({} of {} shapes \
         not where the new layout puts them, units x100):\n{}",
        torn.len(),
        got.len(),
        torn.join("\n")
    );
}
