//! Inventory row 40, UI shell header.

use gorgon_petra::component::{
    section, ui_shell_header, ui_shell_header_action, ui_shell_header_menu_trigger,
    ui_shell_header_nav_item,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const MENU: &str = "shell-menu";
const NAV: [(&str, &str); 2] = [
    ("shell-nav-overview", "Overview"),
    ("shell-nav-fibers", "Fibers"),
];
const ACTIONS: [(&str, &str); 2] = [
    ("shell-action-notify", "Notifications"),
    ("shell-action-switcher", "App switcher"),
];

/// Live state of the UI shell header page.
///
/// It held none, which is what "none of the elements actually do anything"
/// was about: every control was built from a literal, so the page rendered
/// one fixed picture and swallowed every click. Each of the three now has
/// the state Carbon gives it — the current nav item, whether the hamburger
/// is expanded, and which utility is active — and each of the three has a
/// visible resting/active pair, so a click changes the picture.
#[derive(Default)]
pub struct UiShellHeader {
    /// Which nav item is the current page.
    nav: usize,
    /// Whether the hamburger reads as expanded.
    menu_open: bool,
    /// Which utility is active, if any. Carbon's `--active` action takes
    /// `$layer` and opens into a panel below it; only one can be open.
    action: Option<usize>,
}

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
                    Some(ui_shell_header_menu_trigger(MENU, self.menu_open)),
                    NAV.iter()
                        .enumerate()
                        .map(|(i, (key, label))| {
                            ui_shell_header_nav_item(*key, *label, i == self.nav)
                        })
                        .collect(),
                    ACTIONS
                        .iter()
                        .enumerate()
                        .map(|(i, (key, label))| {
                            ui_shell_header_action(*key, *label, self.action == Some(i))
                        })
                        .collect(),
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, MENU) {
            self.menu_open = !self.menu_open;
            return true;
        }
        if let Some(i) = NAV.iter().position(|(key, _)| path_has(node, key)) {
            self.nav = i;
            return true;
        }
        if let Some(i) = ACTIONS.iter().position(|(key, _)| path_has(node, key)) {
            // A second press on the open utility closes it, the way pressing
            // its own trigger closes any Carbon panel.
            self.action = if self.action == Some(i) {
                None
            } else {
                Some(i)
            };
            return true;
        }
        false
    }
}
