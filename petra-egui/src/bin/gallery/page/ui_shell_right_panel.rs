//! Inventory row 42, UI shell right panel.

use gorgon_petra::component::{
    section, ui_shell_header_action, ui_shell_right_panel_divider, ui_shell_switcher_item,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The UI shell right panel page. It holds no live state.
pub struct UiShellRightPanel;

impl Page for UiShellRightPanel {
    fn row(&self) -> &'static str {
        "UI shell right panel"
    }

    fn body(&self) -> ViewNode {
        section(
            "shell-right-section",
            "Switcher trigger and items",
            vec![body(
                "shell-right-body",
                sp("spacing.md"),
                vec![
                    // `ui_shell_right_panel`/`ui_shell_switcher` build a
                    // `Surface` anchored via `Anchor::Node`, the same
                    // open-anchored-overlay shape the catalog cannot
                    // validate nested in a page (see the Popover page
                    // above). The trigger and the switcher's own rows
                    // (plain buttons, no anchor) are shown standalone
                    // instead of inside the anchored panel.
                    ui_shell_header_action("shell-switcher-trigger", "App switcher", false),
                    ui_shell_switcher_item("shell-switcher-petra", "Petra"),
                    ui_shell_right_panel_divider("shell-switcher-div"),
                    ui_shell_switcher_item("shell-switcher-inspector", "Inspector"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
