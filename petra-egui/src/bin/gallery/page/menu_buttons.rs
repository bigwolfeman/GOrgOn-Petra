//! Inventory row 19, Menu buttons.

use gorgon_petra::component::{menu_button, menu_item, section, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, dismissed_under, path_has, row, sp};

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

    /// Carbon's menu-button ramps (slice-c "Sizes"): the trigger steps
    /// side by side — the plain menu button's four (24/32/40/48), the
    /// combo's three (no xs step: SCSS's `$triggerSizes` map has none),
    /// and the overflow's four — each labeled with its step and the height
    /// it lands on (FR-025: every sized constructor this component ships
    /// answers here). Closed: the open menu's rows follow the trigger, and
    /// the Menu page's sizes row shows that ramp.
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{
            combo_button, combo_button_lg, combo_button_sm, menu_button_lg, menu_button_sm,
            menu_button_xs, overflow_menu, overflow_menu_lg, overflow_menu_sm, overflow_menu_xs,
            text,
        };

        use super::common::{column, row, sp};
        // One labelled step row: `label` beside `cell`.
        let step = |key: &str, label: &str, cell: ViewNode| {
            row(
                &format!("size-{key}"),
                sp("spacing-03"),
                vec![text(format!("size-{key}-label"), label), cell],
            )
        };
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                step(
                    "xs",
                    "xs · 24",
                    menu_button_xs("size-mb-xs", "More", false, vec![]),
                ),
                step(
                    "sm",
                    "sm · 32",
                    menu_button_sm("size-mb-sm", "More", false, vec![]),
                ),
                step(
                    "md",
                    "md · 40",
                    menu_button("size-mb-md", "More", false, vec![]),
                ),
                step(
                    "lg",
                    "lg · 48",
                    menu_button_lg("size-mb-lg", "More", false, vec![]),
                ),
                step(
                    "combo-sm",
                    "combo sm · 32",
                    combo_button_sm("size-combo-sm", "Save", false, vec![]),
                ),
                step(
                    "combo-md",
                    "combo md · 40",
                    combo_button("size-combo-md", "Save", false, vec![]),
                ),
                step(
                    "combo-lg",
                    "combo lg · 48",
                    combo_button_lg("size-combo-lg", "Save", false, vec![]),
                ),
                step(
                    "ovf-xs",
                    "overflow xs · 24",
                    overflow_menu_xs("size-ovf-xs", "More", false, vec![]),
                ),
                step(
                    "ovf-sm",
                    "overflow sm · 32",
                    overflow_menu_sm("size-ovf-sm", "More", false, vec![]),
                ),
                step(
                    "ovf-md",
                    "overflow md · 40",
                    overflow_menu("size-ovf-md", "More", false, vec![]),
                ),
                step(
                    "ovf-lg",
                    "overflow lg · 48",
                    overflow_menu_lg("size-ovf-lg", "More", false, vec![]),
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
        use gorgon_petra::component::{combo_button, heading, overflow_menu};

        section(
            "mb",
            "Menu button",
            vec![body(
                "mb-body",
                sp("spacing.md"),
                vec![
                    // The live row, scoped by its own key so a press on
                    // the variant cells below never toggles this menu.
                    column(
                        "mb-live",
                        None,
                        vec![menu_button(
                            "mb",
                            "More",
                            self.open,
                            vec![menu_item(DUPLICATE, "Duplicate")],
                        )],
                    ),
                    heading("variants-title", "The documented trigger shapes"),
                    row(
                        "mb-variants",
                        sp("spacing.lg"),
                        vec![
                            // T051's combo button: the action half beside
                            // the chevron toggle, the menu under the split.
                            // Shown open — the pairing is the variant.
                            column(
                                "v-combo",
                                sp("spacing.sm"),
                                vec![
                                    text("v-combo-title", "Combo button"),
                                    combo_button(
                                        "v-combo-button",
                                        "Save",
                                        true,
                                        vec![
                                            menu_item("v-combo-0", "Save and continue"),
                                            menu_item("v-combo-1", "Save as draft"),
                                        ],
                                    ),
                                ],
                            ),
                            // T051's overflow menu: the one-glyph button
                            // whose only act is opening its list.
                            column(
                                "v-overflow",
                                sp("spacing.sm"),
                                vec![
                                    text("v-overflow-title", "Overflow menu"),
                                    overflow_menu(
                                        "v-overflow-button",
                                        "More",
                                        true,
                                        vec![
                                            menu_item("v-overflow-0", "Duplicate"),
                                            menu_item("v-overflow-1", "Archive"),
                                        ],
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
        // Only the live row's button is live: the sizes row's triggers and
        // the variant cells are geometry displays, and every `menu_button`
        // keys its trigger `trigger`, so without this guard a press on any
        // of them would toggle the live menu.
        if !path_has(node, "mb-live") {
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
        // Under `mb-live`, not bare `menu`: every `menu_button` anatomy
        // keys its surface `menu` (menu_button.rs's `menu_at("menu", …)`),
        // so the combo and overflow cells mounted open beside the live
        // row surface ids ending in `menu` too. A bare match closed the
        // live menu on the same press that opened it, because pressing its
        // trigger dismisses every other open surface on the page.
        if dismissed_under(ids, "mb-live", MENU) {
            self.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MenuButtons;
    use crate::page::Page;
    use crate::page::common::find;

    /// T051's combo button cell: the trigger is a split — the action half
    /// beside the chevron toggle — and the open menu hangs under both.
    #[test]
    fn the_combo_button_cell_is_a_split_trigger_with_its_menu() {
        let tree = MenuButtons::default().body();
        let combo = find(&tree, "v-combo-button").expect("the combo button is on the page");
        assert!(find(combo, "action").is_some(), "the action half is there");
        assert!(
            find(combo, "toggle").is_some(),
            "the chevron toggle is there"
        );
        let menu = find(combo, "menu").expect("the open menu hangs under the split");
        assert!(
            find(menu, "v-combo-0").is_some(),
            "the menu carries its items"
        );
    }

    /// T051's overflow menu cell: the one-glyph button and the list its
    /// only act opens.
    #[test]
    fn the_overflow_menu_cell_is_the_glyph_button_with_its_list() {
        let tree = MenuButtons::default().body();
        let overflow = find(&tree, "v-overflow-button").expect("the overflow menu is on the page");
        let menu = find(overflow, "menu").expect("the open list is under it");
        assert!(
            find(menu, "v-overflow-0").is_some() && find(menu, "v-overflow-1").is_some(),
            "the list carries its items"
        );
    }

    /// FR-025's step census: all three ramps this component ships —
    /// the plain trigger's four steps, the combo's three (no xs), the
    /// overflow's four.
    #[test]
    fn the_sizes_cell_answers_every_step_of_all_three_trigger_ramps() {
        let sizes = MenuButtons::default().sizes();
        assert_eq!(sizes.len(), 1, "one census cell");
        for key in [
            "size-mb-xs",
            "size-mb-sm",
            "size-mb-md",
            "size-mb-lg",
            "size-combo-sm",
            "size-combo-md",
            "size-combo-lg",
            "size-ovf-xs",
            "size-ovf-sm",
            "size-ovf-md",
            "size-ovf-lg",
        ] {
            assert!(
                find(&sizes[0], key).is_some(),
                "the step census carries {key}"
            );
        }
    }
}
