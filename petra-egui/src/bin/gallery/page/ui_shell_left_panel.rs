//! Inventory row 41, UI shell left panel.

use gorgon_petra::component::{
    section, ui_shell_left_panel, ui_shell_left_panel_item, ui_shell_left_panel_subitem,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const KERNEL: &str = "shell-left-kernel";
const FIBERS: &str = "shell-left-fibers";
const PETRA: &str = "shell-left-petra";

/// Which row of the panel is the current page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Current {
    /// The nested row under "Kernel".
    Fibers,
    /// The flat top-level row.
    Petra,
}

/// Live state of the UI shell left panel page.
///
/// It held none, and a panel whose only sub-menu cannot be opened or closed
/// is the "none of the elements actually do anything" complaint in one
/// control: the nested row mounts only while its parent is expanded
/// (FR-026), so with `expanded` frozen at `true` the caret pointed up
/// forever and pressing it did nothing.
pub struct UiShellLeftPanel {
    /// Whether the "Kernel" sub-menu is open. Its children mount only while
    /// this is true, and the caret follows it.
    expanded: bool,
    current: Current,
}

impl Default for UiShellLeftPanel {
    fn default() -> Self {
        Self {
            expanded: true,
            current: Current::Petra,
        }
    }
}

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
                            KERNEL,
                            "Kernel",
                            self.expanded,
                            false,
                            vec![ui_shell_left_panel_subitem(
                                FIBERS,
                                "Fibers",
                                self.current == Current::Fibers,
                            )],
                        ),
                        ui_shell_left_panel_item(
                            PETRA,
                            "Petra",
                            false,
                            self.current == Current::Petra,
                            vec![],
                        ),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // The nested row is tested first. It is a descendant of `KERNEL` in
        // the tree but not in the id path — `ui_shell_left_panel_item` keys
        // its children under itself — so a press on "Fibers" names both, and
        // testing the parent first would collapse the sub-menu out from
        // under the row the operator just chose.
        if path_has(node, FIBERS) {
            self.current = Current::Fibers;
            true
        } else if path_has(node, PETRA) {
            self.current = Current::Petra;
            true
        } else if path_has(node, KERNEL) {
            self.expanded = !self.expanded;
            true
        } else {
            false
        }
    }
}
