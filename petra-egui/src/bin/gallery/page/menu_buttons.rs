//! Inventory row 19, Menu buttons.

use gorgon_petra::component::{menu_button, menu_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, sp};

/// `menu_button`'s own keys for its trigger and its menu.
const TRIGGER: &str = "trigger";
const MENU: &str = "menu";
const DUPLICATE: &str = "mb-0";

/// Live state of the Menu buttons page: whether the menu is open.
#[derive(Default)]
pub struct MenuButtons {
    open: bool,
}

impl Page for MenuButtons {
    fn row(&self) -> &'static str {
        "Menu buttons"
    }

    /// Carbon's menu-button ramp (slice-c "Sizes": "the same four-step
    /// ramp as Menu, keyed to the trigger button's size") — the four built
    /// trigger steps side by side, each labeled with its step and the
    /// height it lands on (FR-025). Closed: the open menu's rows follow
    /// the trigger, and the Menu page's sizes row shows that ramp.
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{menu_button_lg, menu_button_sm, menu_button_xs, text};

        use super::common::{column, row, sp};
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                row(
                    "size-xs",
                    sp("spacing-03"),
                    vec![
                        text("size-xs-label", "xs · 24"),
                        menu_button_xs("size-mb-xs", "More", false, vec![]),
                    ],
                ),
                row(
                    "size-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-sm-label", "sm · 32"),
                        menu_button_sm("size-mb-sm", "More", false, vec![]),
                    ],
                ),
                row(
                    "size-md",
                    sp("spacing-03"),
                    vec![
                        text("size-md-label", "md · 40"),
                        menu_button("size-mb-md", "More", false, vec![]),
                    ],
                ),
                row(
                    "size-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-lg-label", "lg · 48"),
                        menu_button_lg("size-mb-lg", "More", false, vec![]),
                    ],
                ),
            ],
        )]
    }

    /// The documented disabled state (spec 008 T025, register row 288): the
    /// trigger is ordinary Button anatomy, so `disabled()` on it resolves the
    /// filled family's own `@disabled` pair (register row 63 records it).
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![menu_button("disabled-trigger", "Actions", false, vec![])],
        ))]
    }

    fn body(&self) -> ViewNode {
        section(
            "mb",
            "Menu button",
            vec![body(
                "mb-body",
                sp("spacing.md"),
                vec![menu_button(
                    "mb",
                    "More",
                    self.open,
                    vec![menu_item(DUPLICATE, "Duplicate")],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // Only the body's button is live: the sizes row's triggers are a
        // geometry display, and every `menu_button` keys its trigger
        // `trigger`, so without this guard a press on any of them would
        // toggle the body's menu.
        if !path_has(node, "mb-body") {
            return false;
        }
        if path_has(node, DUPLICATE) {
            self.open = false;
        } else if path_has(node, TRIGGER) {
            self.open = !self.open;
        } else {
            return false;
        }
        true
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, MENU) {
            self.open = false;
        }
    }
}
