//! Inventory row 41, UI shell left panel.

use gorgon_petra::component::{
    section, ui_shell_left_panel, ui_shell_left_panel_item, ui_shell_left_panel_subitem,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The UI shell left panel page. It holds no live state.
pub struct UiShellLeftPanel;

impl Page for UiShellLeftPanel {
    fn row(&self) -> &'static str {
        "UI shell left panel"
    }

    fn body(&self) -> ViewNode {
        section(
            "shell-left-section",
            "Fixed panel",
            vec![body(
                "shell-left-body",
                sp("spacing.md"),
                vec![ui_shell_left_panel(
                    "shell-left",
                    vec![
                        ui_shell_left_panel_item(
                            "shell-left-kernel",
                            "Kernel",
                            true,
                            false,
                            vec![ui_shell_left_panel_subitem(
                                "shell-left-fibers",
                                "Fibers",
                                false,
                            )],
                        ),
                        ui_shell_left_panel_item("shell-left-petra", "Petra", false, true, vec![]),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
