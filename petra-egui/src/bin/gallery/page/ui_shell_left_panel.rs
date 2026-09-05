//! Inventory row 41, UI shell left panel.

use gorgon_petra::component::{
    IconMark, LeftPanelMode, content_switcher, content_switcher_item, disabled, section,
    ui_shell_header_menu_trigger, ui_shell_left_panel_icon_item, ui_shell_left_panel_icon_subitem,
    ui_shell_left_panel_in,
};
use gorgon_petra::geom::Align;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, TrackSize, ViewNode};

use super::Page;
use super::common::{filled_body, path_has, row, sp, tok};

const KERNEL: &str = "shell-left-kernel";
const FIBERS: &str = "shell-left-fibers";
const PETRA: &str = "shell-left-petra";
const TRACE: &str = "shell-left-trace";
const MENU: &str = "shell-left-menu";
const FRAME: &str = "shell-left-frame";
const CONTENT: &str = "shell-left-content";
const PAGE: &str = "shell-left-page";

/// The four widths Carbon's side nav has, in the order
/// `_side-nav.scss` declares them.
const MODES: [(&str, &str, LeftPanelMode); 4] = [
    ("shell-left-rail", "Rail", LeftPanelMode::Rail),
    ("shell-left-fixed", "Fixed", LeftPanelMode::Fixed),
    (
        "shell-left-expandable",
        "Expandable",
        // The `expanded` field is replaced from `nav_open` at build time;
        // a constant cannot read state.
        LeftPanelMode::Expandable { expanded: false },
    ),
    ("shell-left-hidden", "Hidden", LeftPanelMode::Hidden),
];

/// How tall the stand-in viewport is.
///
/// Carbon's side nav runs from the bottom of the header to the bottom of
/// the screen (`.cds--header ~ .cds--side-nav { block-size: calc(100% -
/// 48px) }`), and Petra has no viewport-edge dock, so the page mounts a
/// `Grid` standing in for the window: a header-height row over a content
/// row. The number is the catalog card's, not Carbon's — what Carbon
/// states is "to the bottom", and any bounded height demonstrates that.
const FRAME_HEIGHT: f32 = 300.0;

/// Which row of the panel is the current page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Current {
    /// The nested row under "Kernel".
    Fibers,
    /// A flat top-level row.
    Petra,
    /// The other flat top-level row.
    Trace,
}

/// Live state of the UI shell left panel page.
///
/// The page used to build one panel in one mode, titled "Fixed panel", with
/// no icons and no trigger. The operator: *"that is not what these are,
/// these are 2 different kinds of panels that can be used in different
/// ways."* He is right — the left panel **is** its width modes
/// (`_side-nav.scss:65-117`, every modifier sets `inline-size` and nothing
/// else), and a page showing one of the four shows none of the component.
pub struct UiShellLeftPanel {
    /// Index into [`MODES`].
    mode: usize,
    /// The hamburger's boolean. Carbon hands the same one to
    /// `HeaderMenuButton` as `isActive` and to `SideNav` as `expanded`
    /// (`HeaderContainer.js:24-35`); so does this page.
    nav_open: bool,
    /// Whether the "Kernel" sub-menu is open. Its children mount only while
    /// this is true, and the caret follows it.
    expanded: bool,
    current: Current,
}

impl Default for UiShellLeftPanel {
    fn default() -> Self {
        Self {
            mode: 1,
            nav_open: true,
            expanded: true,
            current: Current::Petra,
        }
    }
}

impl UiShellLeftPanel {
    /// The chosen mode, with the hamburger's boolean folded in.
    fn mode(&self) -> LeftPanelMode {
        match MODES[self.mode].2 {
            LeftPanelMode::Expandable { .. } => LeftPanelMode::Expandable {
                expanded: self.nav_open,
            },
            other => other,
        }
    }
}

impl Page for UiShellLeftPanel {
    fn row(&self) -> &'static str {
        "UI shell left panel"
    }

    fn body(&self) -> ViewNode {
        let mode = self.mode();
        // A rail has nowhere to put a sub-menu's children: 48px hides every
        // label, so an expanded branch draws one blank 32px band and calls
        // it a nested row. Carbon's answer is hover-to-expand
        // (`SideNav.js:34-102`, a 100 ms `enterDelayMs`), which needs a
        // delayed state transition Petra cannot express yet; until it can,
        // this page shuts the branch in Rail rather than showing the band.
        let expanded = self.expanded && !matches!(mode, LeftPanelMode::Rail);
        let panel = ui_shell_left_panel_in(
            "shell-left",
            mode,
            vec![
                // An icon-bearing branch, so its nested row takes Carbon's
                // `.__item--icon a.__link` indent of 72 rather than 32.
                ui_shell_left_panel_icon_item(
                    KERNEL,
                    "Kernel",
                    IconMark::Switcher,
                    expanded,
                    false,
                    vec![ui_shell_left_panel_icon_subitem(
                        FIBERS,
                        "Fibers",
                        self.current == Current::Fibers,
                    )],
                ),
                ui_shell_left_panel_icon_item(
                    PETRA,
                    "Petra",
                    IconMark::Edit,
                    false,
                    self.current == Current::Petra,
                    vec![],
                ),
                ui_shell_left_panel_icon_item(
                    TRACE,
                    "Trace",
                    IconMark::Search,
                    false,
                    self.current == Current::Trace,
                    vec![],
                ),
            ],
        );

        // Carbon hides the hamburger outright when there is no collapsible
        // nav for it to drive (`isCollapsible={false}` adds
        // `.cds--header__menu-toggle__hidden`). This page greys it instead:
        // a catalog row that shows all four modes must not change height
        // when the operator picks one, and "unavailable here" is the honest
        // reading of a control whose one consumer is the mode beside it.
        let expandable = matches!(mode, LeftPanelMode::Expandable { .. });
        let trigger = ui_shell_header_menu_trigger(MENU, expandable && self.nav_open);
        let trigger = if expandable {
            trigger
        } else {
            disabled(trigger)
        };

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
                columns: vec![TrackSize::FitContent, TrackSize::Weight { weight: 1.0 }],
                rows: vec![TrackSize::Weight { weight: 1.0 }],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![panel, stub]);

        let mut frame = ViewNode::new(NodeKind::Grid, FRAME)
            .with_props(Props {
                columns: vec![TrackSize::Weight { weight: 1.0 }],
                rows: vec![TrackSize::FitContent, TrackSize::Weight { weight: 1.0 }],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![row("shell-left-bar", None, vec![trigger]), content])
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
            "shell-left-section",
            "Width modes",
            vec![filled_body(
                "shell-left-body",
                sp("spacing.md"),
                vec![
                    content_switcher(
                        "shell-left-modes",
                        MODES
                            .iter()
                            .enumerate()
                            .map(|(i, (key, label, _))| {
                                content_switcher_item(*key, *label, i == self.mode)
                            })
                            .collect(),
                    ),
                    frame,
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if let Some(i) = MODES.iter().position(|(key, _, _)| path_has(node, key)) {
            self.mode = i;
            return true;
        }
        // The nested row is tested first. It is a descendant of `KERNEL` in
        // the tree but not in the id path — `ui_shell_left_panel_item` keys
        // its children under itself — so a press on "Fibers" names both, and
        // testing the parent first would collapse the sub-menu out from
        // under the row the operator just chose.
        if path_has(node, FIBERS) {
            self.current = Current::Fibers;
        } else if path_has(node, PETRA) {
            self.current = Current::Petra;
        } else if path_has(node, TRACE) {
            self.current = Current::Trace;
        } else if path_has(node, KERNEL) {
            self.expanded = !self.expanded;
        } else if path_has(node, MENU) {
            self.nav_open = !self.nav_open;
        } else {
            return false;
        }
        true
    }
}
