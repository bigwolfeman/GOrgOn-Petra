//! Inventory row 18, Menu.

use gorgon_petra::component::{button, menu, menu_item, section, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, dismissed_under, path_has, row, sp};

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
        use gorgon_petra::component::{
            IconMark, heading, menu_border, menu_item_group, menu_item_radio_group,
            menu_item_selectable, menu_item_with, menu_with_icons,
        };

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
                vec![
                    column("mn-pair", None, pair),
                    heading("variants-title", "The documented modifiers and sub-parts"),
                    row(
                        "mn-variants",
                        sp("spacing.lg"),
                        vec![
                            // T050 `--border`: the 1px $border-subtle
                            // outline the style page names as optional.
                            column(
                                "v-border",
                                sp("spacing.sm"),
                                vec![
                                    text("v-border-title", "--border"),
                                    button(TRIGGER, "Actions"),
                                    menu_border(
                                        "v-menu-border",
                                        "Actions",
                                        vec![
                                            menu_item("v-border-0", "Rename"),
                                            menu_item("v-border-1", "Delete"),
                                        ],
                                    ),
                                ],
                            ),
                            // T050 `--with-icons`: the widened 12rem
                            // minimum and the rows' leading icon column.
                            column(
                                "v-icons",
                                sp("spacing.sm"),
                                vec![
                                    text("v-icons-title", "--with-icons"),
                                    button(TRIGGER, "Actions"),
                                    menu_with_icons(
                                        "v-menu-icons",
                                        "Actions",
                                        vec![
                                            menu_item_with(
                                                "v-icons-0",
                                                "Rename",
                                                Some(IconMark::Edit),
                                                None,
                                                false,
                                            ),
                                            menu_item_with(
                                                "v-icons-1",
                                                "Delete",
                                                Some(IconMark::Close),
                                                None,
                                                false,
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                            // T050 selectable items: the selection column,
                            // a checkmark plus the semantic flag, never a fill.
                            column(
                                "v-selectable",
                                sp("spacing.sm"),
                                vec![
                                    text("v-selectable-title", "--with-selectable-items"),
                                    button(TRIGGER, "View"),
                                    menu(
                                        "v-menu-selectable",
                                        "View",
                                        vec![
                                            menu_item_selectable("v-sel-0", "Keep", None, false),
                                            menu_item_selectable(
                                                "v-sel-1",
                                                "Share",
                                                Some(IconMark::Check),
                                                true,
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                            // T050 `menu-item-group`: a labelled column
                            // of rows inside the menu.
                            column(
                                "v-group",
                                sp("spacing.sm"),
                                vec![
                                    text("v-group-title", "menu-item-group"),
                                    button(TRIGGER, "Actions"),
                                    menu(
                                        "v-menu-group",
                                        "Actions",
                                        vec![
                                            menu_item("v-group-before", "Rename"),
                                            menu_item_group(
                                                "v-group-inner",
                                                "View",
                                                vec![
                                                    menu_item("v-group-0", "Zoom in"),
                                                    menu_item("v-group-1", "Zoom out"),
                                                ],
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                            // T050 `menu-item-radio-group`: the same
                            // shape declaring `radiogroup` — one choice.
                            column(
                                "v-radio",
                                sp("spacing.sm"),
                                vec![
                                    text("v-radio-title", "menu-item-radio-group"),
                                    button(TRIGGER, "Size"),
                                    menu(
                                        "v-menu-radio",
                                        "Size",
                                        vec![menu_item_radio_group(
                                            "v-radio-inner",
                                            "Size",
                                            vec![
                                                menu_item_selectable(
                                                    "v-radio-0",
                                                    "Small",
                                                    None,
                                                    false,
                                                ),
                                                menu_item_selectable(
                                                    "v-radio-1",
                                                    "Large",
                                                    None,
                                                    true,
                                                ),
                                            ],
                                        )],
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
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
        // Under `mn-pair`, not bare `menu`: the section, the page root and
        // the live surface all carry the key `menu`, and the variant cells
        // mount their own surfaces open at rest — a bare segment match
        // read every press that shut one of those as a press that shut the
        // live menu, closing it the same event that opened it.
        if dismissed_under(ids, "mn-pair", MENU) {
            self.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MENU, Menu};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::tree::{NodeKind, Role};

    /// T050 `--border` cell: the menu wears the one-token outline and the
    /// plain menu beside the page does not.
    #[test]
    fn the_border_menu_cell_wears_the_outline() {
        let page = Menu { open: true };
        let tree = page.body();
        let bordered = find(&tree, "v-menu-border").expect("the --border menu is on the page");
        assert!(
            bordered.props.tokens.contains_key("border"),
            "the --border menu binds an outline token"
        );
        let plain = find(&tree, MENU).expect("the live menu cell is on the page");
        assert!(
            !plain.props.tokens.contains_key("border"),
            "the plain menu wears no outline"
        );
    }

    /// T050 `--with-icons` cell: the widened 12rem/192px minimum, the
    /// rows carrying their own leading icon column.
    #[test]
    fn the_with_icons_menu_cell_widens_its_minimum() {
        let tree = Menu::default().body();
        let cell = find(&tree, "v-menu-icons").expect("the --with-icons menu is on the page");
        assert_eq!(
            cell.constraints.horizontal.min,
            Some(192.0),
            "12rem at 16 units per rem"
        );
        assert!(find(cell, "v-icons-0").is_some(), "its rows are in it");
    }

    /// T050 selectable-items cell: the selection column is a checkmark
    /// when selected and an empty slot when not — never a recoloured row.
    #[test]
    fn the_selectable_items_cell_marks_selection_in_its_column() {
        let tree = Menu::default().body();
        let menu = find(&tree, "v-menu-selectable").expect("the selectable menu is on the page");
        let unselected = find(menu, "v-sel-0").expect("the unselected row is there");
        let selected = find(menu, "v-sel-1").expect("the selected row is there");
        let empty = find(unselected, "selection").expect("the unselected row reserves its column");
        assert_eq!(empty.kind, NodeKind::Stack, "an unselected column is empty");
        let mark = find(selected, "selection").expect("the selected row reserves its column");
        assert_eq!(
            mark.kind,
            NodeKind::Canvas,
            "a selected column is the checkmark glyph"
        );
    }

    /// T050 `menu-item-group` cell: the sub-part declares ARIA's own
    /// `group`, the label above its button rows.
    #[test]
    fn the_group_cell_declares_the_group_role() {
        let tree = Menu::default().body();
        let group = find(&tree, "v-group-inner").expect("the item group is on the page");
        assert_eq!(group.semantics.role, Some(Role::Custom("group".to_owned())));
        assert!(find(group, "v-group-0").is_some(), "its rows are in it");
    }

    /// T050 `menu-item-radio-group` cell: the same shape declaring
    /// `radiogroup` — the role is what says the collection picks one.
    #[test]
    fn the_radio_group_cell_declares_the_radiogroup_role() {
        let tree = Menu::default().body();
        let group = find(&tree, "v-radio-inner").expect("the radio group is on the page");
        assert_eq!(
            group.semantics.role,
            Some(Role::Custom("radiogroup".to_owned()))
        );
    }
}
