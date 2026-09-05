//! Inventory row 42, UI shell right panel.

use gorgon_petra::component::{
    section, ui_shell_header, ui_shell_header_action, ui_shell_right_panel_divider,
    ui_shell_switcher, ui_shell_switcher_item,
};
use gorgon_petra::geom::Align;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, TrackSize, ViewNode};

use super::Page;
use super::common::{filled_body, path_has, sp, tok};

const HEADER: &str = "shell-header";
const SWITCHER_TRIGGER: &str = "shell-switcher-trigger";
const SWITCHER: &str = "shell-switcher";
const FRAME: &str = "shell-frame";
const CONTENT: &str = "shell-content";
const PAGE: &str = "shell-page";
const ITEMS: [(&str, &str); 2] = [
    ("shell-switcher-petra", "Petra"),
    ("shell-switcher-inspector", "Inspector"),
];

/// How tall the stand-in viewport is.
///
/// Carbon's header panel is `inset-block: mini-units(6) 0` — from the
/// bottom of the header to the bottom of the *viewport* — and Petra's
/// `Anchor` enum has no viewport-edge dock (`Anchor::Viewport` centres). So
/// the page mounts a `Grid` standing in for the window: a header-height row
/// over a content row, with the panel as a cell of the second. The number
/// is the catalog card's, not Carbon's.
const FRAME_HEIGHT: f32 = 320.0;

/// Live state of the UI shell right panel page: which app is current, and
/// whether the switcher is open.
///
/// **Open by default.** It used to default shut, so the catalog's resting
/// photograph of row 42 was a header and an empty card — the row's own
/// subject was not in the row's own picture. Carbon's reference harness
/// captures this row open for the same reason.
pub struct UiShellRightPanel {
    open: bool,
    /// Index into [`ITEMS`]. Carbon's switcher item does carry a selected
    /// state: `_switcher.scss` has `--switcher__item-link--selected` and
    /// `SwitcherItem.js:29` has `isSelected`, against one sentence of
    /// usage-page prose that says it does not. T070 ranks the SCSS first.
    current: usize,
}

impl Default for UiShellRightPanel {
    fn default() -> Self {
        Self {
            open: true,
            current: 0,
        }
    }
}

impl Page for UiShellRightPanel {
    fn row(&self) -> &'static str {
        "UI shell right panel"
    }

    fn body(&self) -> ViewNode {
        let header = ui_shell_header(
            HEADER,
            "GOrgOn",
            None,
            vec![],
            vec![ui_shell_header_action(
                SWITCHER_TRIGGER,
                "App switcher",
                self.open,
            )],
        );

        // A cell of the frame, not a popover. `_header-panel.scss` is a box
        // pinned to two viewport edges with its height taken from the
        // viewport and its width from one boolean; it was built here as a
        // `Layer::Popup` surface anchored to a sibling with a content-sized
        // height, which is the one shape Carbon is not using.
        let switcher = ui_shell_switcher(
            SWITCHER,
            "App switcher",
            self.open,
            vec![
                ui_shell_switcher_item(ITEMS[0].0, ITEMS[0].1, self.current == 0),
                ui_shell_right_panel_divider("shell-switcher-div"),
                ui_shell_switcher_item(ITEMS[1].0, ITEMS[1].1, self.current == 1),
            ],
        );

        let mut stub = ViewNode::new(NodeKind::Grid, PAGE).with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            ..Props::default()
        });
        stub.props
            .tokens
            .insert("background".into(), tok("surface.base"));

        let content = ViewNode::new(NodeKind::Grid, CONTENT)
            .with_props(Props {
                columns: vec![TrackSize::Weight { weight: 1.0 }, TrackSize::FitContent],
                rows: vec![TrackSize::Weight { weight: 1.0 }],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![stub, switcher]);

        let mut frame = ViewNode::new(NodeKind::Grid, FRAME)
            .with_props(Props {
                columns: vec![TrackSize::Weight { weight: 1.0 }],
                rows: vec![TrackSize::FitContent, TrackSize::Weight { weight: 1.0 }],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![header, content])
            .with_constraints(Constraints {
                vertical: AxisConstraint {
                    min: Some(FRAME_HEIGHT),
                    max: Some(FRAME_HEIGHT),
                    priority: 0,
                },
                ..Constraints::default()
            });
        frame
            .props
            .tokens
            .insert("background".into(), tok("surface.base"));

        section(
            "shell-right-section",
            "Header with the switcher",
            vec![filled_body(
                "shell-right-body",
                sp("spacing.md"),
                vec![frame],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if let Some(i) = ITEMS.iter().position(|(key, _)| path_has(node, key)) {
            // Picking an app makes it current and closes the panel, which
            // is what Carbon's own `Switcher` does with a destination.
            self.current = i;
            self.open = false;
        } else if path_has(node, SWITCHER_TRIGGER) {
            self.open = !self.open;
        } else {
            return false;
        }
        true
    }
}
