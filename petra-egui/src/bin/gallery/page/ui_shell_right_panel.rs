//! Inventory row 42, UI shell right panel.

use gorgon_petra::component::{
    section, ui_shell_header, ui_shell_header_action, ui_shell_right_panel_divider,
    ui_shell_switcher, ui_shell_switcher_item,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, dismisses, path_has, sp};

const HEADER: &str = "shell-header";
const SWITCHER_TRIGGER: &str = "shell-switcher-trigger";
const SWITCHER: &str = "shell-switcher";
const ITEMS: [(&str, &str); 2] = [
    ("shell-switcher-petra", "Petra"),
    ("shell-switcher-inspector", "Inspector"),
];

/// Live state of the UI shell right panel page: whether the switcher is
/// open.
#[derive(Default)]
pub struct UiShellRightPanel {
    open: bool,
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
        // The panel is anchored to the *header*, not to the action that
        // opens it. `ui_shell_switcher` names its anchor by sibling key,
        // and the action is not a sibling: it sits inside the header's own
        // `actions` stack, where Carbon puts it. The two anchors are the
        // same rect edge — the action is 48 tall in a 48-tall bar and is
        // its trailing child, so the action's bottom-right corner is the
        // header's — and Carbon's own panel is `top: 3rem; right: 0`,
        // which is "under the header, at its trailing edge" and not "under
        // the icon". Naming the header keeps the action where Carbon puts
        // it and keeps the page ignorant of its own mount path, which an
        // `Anchor::Node` id would have hard-coded.
        let mut pair = vec![header];
        if self.open {
            pair.push(ui_shell_switcher(
                SWITCHER,
                "App switcher",
                HEADER,
                vec![
                    ui_shell_switcher_item(ITEMS[0].0, ITEMS[0].1),
                    ui_shell_right_panel_divider("shell-switcher-div"),
                    ui_shell_switcher_item(ITEMS[1].0, ITEMS[1].1),
                ],
            ));
        }
        section(
            "shell-right-section",
            "Header with the switcher",
            vec![body(
                "shell-right-body",
                sp("spacing.md"),
                vec![column("shell-right", None, pair)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if ITEMS.iter().any(|(key, _)| path_has(node, key)) {
            self.open = false;
        } else if path_has(node, SWITCHER_TRIGGER) {
            self.open = !self.open;
        } else {
            return false;
        }
        true
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, SWITCHER) {
            self.open = false;
        }
    }
}
