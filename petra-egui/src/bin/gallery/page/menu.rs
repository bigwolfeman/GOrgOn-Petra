//! Inventory row 18, Menu.

use gorgon_petra::component::{button, menu, menu_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, dismisses, path_has, sp};

/// The key `menu` anchors to: its trigger must be a sibling with this key.
const TRIGGER: &str = "trigger";
const MENU: &str = "menu";
const ITEMS: [(&str, &str); 2] = [("mn-0", "Rename"), ("mn-1", "Delete")];

/// Live state of the Menu page: whether the menu is open.
///
/// Carbon opens a Menu from a context (a right-click) or from a button;
/// the page gives it a button, because a menu with nothing to open it
/// is the row the operator saw.
#[derive(Default)]
pub struct Menu {
    open: bool,
}

impl Page for Menu {
    fn row(&self) -> &'static str {
        "Menu"
    }

    /// Carbon's menu-item heights (slice-c "Sizes": "driven by the
    /// trigger that opened the menu") — the four built steps side by side,
    /// each labeled with its step and the row height it lands on (FR-025).
    /// Each pair owns its own `trigger` sibling, the key `menu` anchors to.
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{menu_item, menu_lg, menu_sm, menu_xs, text};

        use super::common::{column, sp};
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                column(
                    "size-pair-xs",
                    None,
                    vec![
                        text("size-xs-label", "xs · 24"),
                        button("trigger", "Actions"),
                        menu_xs(
                            "size-menu-xs",
                            "Actions",
                            vec![menu_item("size-mn-xs-0", "Rename")],
                        ),
                    ],
                ),
                column(
                    "size-pair-sm",
                    None,
                    vec![
                        text("size-sm-label", "sm · 32"),
                        button("trigger", "Actions"),
                        menu_sm(
                            "size-menu-sm",
                            "Actions",
                            vec![menu_item("size-mn-sm-0", "Rename")],
                        ),
                    ],
                ),
                column(
                    "size-pair-md",
                    None,
                    vec![
                        text("size-md-label", "md · 40"),
                        button("trigger", "Actions"),
                        menu(
                            "size-menu-md",
                            "Actions",
                            vec![menu_item("size-mn-md-0", "Rename")],
                        ),
                    ],
                ),
                column(
                    "size-pair-lg",
                    None,
                    vec![
                        text("size-lg-label", "lg · 48"),
                        button("trigger", "Actions"),
                        menu_lg(
                            "size-menu-lg",
                            "Actions",
                            vec![menu_item("size-mn-lg-0", "Rename")],
                        ),
                    ],
                ),
            ],
        )]
    }

    /// The documented disabled state (spec 008 T025, register row 273): a
    /// disabled menu — item labels in `text.muted`, the item glyph in the
    /// `icon-*@disabled` fade (slice-c States: Disabled is one of Menu's seven).
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::{IconMark, disabled, menu_item_with};

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            // The trigger and the menu are siblings, because `menu` anchors
            // to the sibling key `trigger` — a menu without one is refused
            // at acceptance rather than floated over nothing.
            vec![
                button("trigger", "Actions"),
                menu(
                    "disabled-menu",
                    "Actions",
                    vec![
                        menu_item("disabled-item", "Delete"),
                        menu_item_with(
                            "disabled-item-icon",
                            "Archive",
                            Some(IconMark::Check),
                            None,
                            false,
                        ),
                    ],
                ),
            ],
        ))]
    }

    fn body(&self) -> ViewNode {
        let mut pair = vec![button(TRIGGER, "Actions")];
        if self.open {
            pair.push(menu(
                MENU,
                "Actions",
                ITEMS
                    .iter()
                    .map(|(key, label)| menu_item(*key, *label))
                    .collect(),
            ));
        }
        section(
            "menu",
            "Menu",
            vec![body(
                "mn",
                sp("spacing.md"),
                // The trigger and the menu share one child list with no
                // spacing, so the sibling anchor resolves and nothing
                // below the trigger moves when the menu opens.
                vec![column("mn-pair", None, pair)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // Only the body's pair is live: the sizes row's triggers and rows
        // are a geometry display, and without this guard a press on one
        // would toggle the body's menu.
        if !path_has(node, "mn-pair") {
            return false;
        }
        if ITEMS.iter().any(|(key, _)| path_has(node, key)) {
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
