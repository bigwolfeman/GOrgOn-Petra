//! Inventory row 40, UI shell header.

use gorgon_petra::component::{
    section, ui_shell_header, ui_shell_header_action, ui_shell_header_menu_trigger,
    ui_shell_header_nav_item,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The UI shell header page. It holds no live state.
pub struct UiShellHeader;

impl Page for UiShellHeader {
    fn row(&self) -> &'static str {
        "UI shell header"
    }

    fn body(&self) -> ViewNode {
        section(
            "shell-header-section",
            "Header",
            vec![body(
                "shell-header-body",
                sp("spacing.md"),
                vec![ui_shell_header(
                    "shell-header",
                    "GOrgOn",
                    Some(ui_shell_header_menu_trigger("shell-menu", false)),
                    vec![
                        ui_shell_header_nav_item("shell-nav-overview", "Overview", true),
                        ui_shell_header_nav_item("shell-nav-fibers", "Fibers", false),
                    ],
                    vec![
                        ui_shell_header_action("shell-action-notify", "Notifications", false),
                        ui_shell_header_action("shell-action-switcher", "App switcher", false),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
