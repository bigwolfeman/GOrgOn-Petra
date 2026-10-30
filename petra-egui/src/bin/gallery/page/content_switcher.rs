//! Inventory row 8, Content switcher.

use gorgon_petra::component::{content_switcher, content_switcher_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const SW_0: &str = "sw-0";
const SW_1: &str = "sw-1";

/// Live state of the Content switcher page.
#[derive(Default)]
pub struct ContentSwitcher {
    switcher: u8,
}

impl Page for ContentSwitcher {
    fn row(&self) -> &'static str {
        "Content switcher"
    }

    /// Carbon's content-switcher sizing (`_content-switcher.scss`,
    /// 32/40/48): the three built ramps side by side — the default ground,
    /// low contrast, and icon only — each step labeled with its step and
    /// the height it lands on (FR-025: a page whose component has a ramp
    /// answers for every step of every sized constructor it ships).
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{
            IconMark, SwitcherSize, content_switcher_icon_only, content_switcher_icon_only_lg,
            content_switcher_icon_only_sm, content_switcher_item,
            content_switcher_item_icon_with_size, content_switcher_item_low_contrast,
            content_switcher_lg, content_switcher_low_contrast, content_switcher_low_contrast_lg,
            content_switcher_low_contrast_sm, content_switcher_sm, text,
        };

        use super::common::{column, row, sp};
        // One labelled step row: `label` beside `cell`, keyed `key`.
        let step = |key: &str, label: &str, cell: ViewNode| {
            row(
                &format!("size-row-{key}"),
                sp("spacing-03"),
                vec![text(format!("size-label-{key}"), label), cell],
            )
        };
        let plain = |key: &str, step: &str| match step {
            "sm" => content_switcher_sm(
                key,
                vec![
                    content_switcher_item(format!("{key}-a"), "List", true),
                    content_switcher_item(format!("{key}-b"), "Grid", false),
                ],
            ),
            "md" => content_switcher(
                key,
                vec![
                    content_switcher_item(format!("{key}-a"), "List", true),
                    content_switcher_item(format!("{key}-b"), "Grid", false),
                ],
            ),
            _ => content_switcher_lg(
                key,
                vec![
                    content_switcher_item(format!("{key}-a"), "List", true),
                    content_switcher_item(format!("{key}-b"), "Grid", false),
                ],
            ),
        };
        let low = |key: &str, step: &str| match step {
            "sm" => content_switcher_low_contrast_sm(
                key,
                vec![
                    content_switcher_item_low_contrast(format!("{key}-a"), "List", true),
                    content_switcher_item_low_contrast(format!("{key}-b"), "Grid", false),
                ],
            ),
            "md" => content_switcher_low_contrast(
                key,
                vec![
                    content_switcher_item_low_contrast(format!("{key}-a"), "List", true),
                    content_switcher_item_low_contrast(format!("{key}-b"), "Grid", false),
                ],
            ),
            _ => content_switcher_low_contrast_lg(
                key,
                vec![
                    content_switcher_item_low_contrast(format!("{key}-a"), "List", true),
                    content_switcher_item_low_contrast(format!("{key}-b"), "Grid", false),
                ],
            ),
        };
        // The icon-only ramp's items are sized per step (`*_with_size` at
        // the step the switcher pins to), the glyph 16 at sm/md and 20 at lg.
        let icon = |key: &str, step: &str| {
            let size = match step {
                "sm" => SwitcherSize::Sm,
                "md" => SwitcherSize::Md,
                _ => SwitcherSize::Lg,
            };
            let items = vec![
                content_switcher_item_icon_with_size(
                    format!("{key}-a"),
                    "List",
                    IconMark::Menu,
                    true,
                    size,
                ),
                content_switcher_item_icon_with_size(
                    format!("{key}-b"),
                    "Grid",
                    IconMark::Switcher,
                    false,
                    size,
                ),
            ];
            match step {
                "sm" => content_switcher_icon_only_sm(key, items),
                "md" => content_switcher_icon_only(key, items),
                _ => content_switcher_icon_only_lg(key, items),
            }
        };
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                step("sm", "sm · 32", plain("size-sw-sm", "sm")),
                step("md", "md · 40", plain("size-sw-md", "md")),
                step("lg", "lg · 48", plain("size-sw-lg", "lg")),
                step("low-sm", "low sm · 32", low("size-low-sm", "sm")),
                step("low-md", "low md · 40", low("size-low-md", "md")),
                step("low-lg", "low lg · 48", low("size-low-lg", "lg")),
                step("icon-sm", "icon sm · 32", icon("size-icon-sm", "sm")),
                step("icon-md", "icon md · 40", icon("size-icon-md", "md")),
                step("icon-lg", "icon lg · 48", icon("size-icon-lg", "lg")),
            ],
        )]
    }

    /// The documented disabled state (spec 008 T025, register row 122): a
    /// disabled switcher — item labels in `text.muted`, the inverse ground
    /// structural and dropped with the painter's elevation.
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![content_switcher(
                "disabled-switcher",
                vec![
                    content_switcher_item("disabled-a", "Graph", true),
                    content_switcher_item("disabled-b", "Grid", false),
                ],
            )],
        ))]
    }

    fn body(&self) -> ViewNode {
        use gorgon_petra::component::{
            IconMark, content_switcher_icon_only, content_switcher_item_icon,
            content_switcher_item_icon_popover, content_switcher_item_low_contrast,
            content_switcher_low_contrast, heading,
        };

        use super::common::column;
        section(
            "switcher",
            "Switcher",
            vec![body(
                "switch",
                sp("spacing.md"),
                vec![
                    content_switcher(
                        "sw",
                        vec![
                            content_switcher_item(SW_0, "List", self.switcher == 0),
                            content_switcher_item(SW_1, "Grid", self.switcher == 1),
                        ],
                    ),
                    heading("low-title", "Low contrast"),
                    // T053's low-contrast variant: the selection never
                    // inverts — `layer.selected` fill and `text.primary`
                    // ink on the item, the ground one step down.
                    column(
                        "low-column",
                        None,
                        vec![content_switcher_low_contrast(
                            "low-contrast",
                            vec![
                                content_switcher_item_low_contrast("low-0", "List", true),
                                content_switcher_item_low_contrast("low-1", "Grid", false),
                            ],
                        )],
                    ),
                    heading("icon-title", "Icon only"),
                    // T053's icon-only variant: square tabs, the glyph
                    // the whole of the label, the accessible name beside it.
                    column(
                        "icon-column",
                        None,
                        vec![content_switcher_icon_only(
                            "icon-only",
                            vec![
                                content_switcher_item_icon("icon-0", "List", IconMark::Menu, true),
                                content_switcher_item_icon(
                                    "icon-1",
                                    "Grid",
                                    IconMark::Switcher,
                                    false,
                                ),
                            ],
                        )],
                    ),
                    heading("popover-title", "Popover-wrapped"),
                    // Register row 113's `--popover-wrapped` modifier:
                    // the icon tab wearing its tooltip-style label — the
                    // bubble shown mounted (the bubble is the variant),
                    // the plain tab beside it for the contrast.
                    column(
                        "popover-column",
                        None,
                        vec![content_switcher_icon_only(
                            "popover-wrapped",
                            vec![
                                content_switcher_item_icon_popover(
                                    "wrap-0",
                                    "List",
                                    IconMark::Menu,
                                    true,
                                ),
                                content_switcher_item_icon(
                                    "wrap-1",
                                    "Grid",
                                    IconMark::Switcher,
                                    false,
                                ),
                            ],
                        )],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, SW_0) {
            self.switcher = 0;
        } else if path_has(node, SW_1) {
            self.switcher = 1;
        } else {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ContentSwitcher;
    use crate::page::Page;
    use crate::page::common::find;

    /// T053's low-contrast variant cell: the switcher and both its items.
    #[test]
    fn the_page_carries_the_low_contrast_switcher_with_its_items() {
        let tree = ContentSwitcher::default().body();
        let cell = find(&tree, "low-contrast").expect("the low-contrast switcher is on the page");
        assert!(find(cell, "low-0").is_some(), "its first item is there");
        assert!(find(cell, "low-1").is_some(), "its second item is there");
    }

    /// T053's icon-only variant cell: square tabs whose glyphs stand in
    /// for the words — each item carries its glyph node.
    #[test]
    fn the_page_carries_the_icon_only_switcher_with_glyph_tabs() {
        let tree = ContentSwitcher::default().body();
        let cell = find(&tree, "icon-only").expect("the icon-only switcher is on the page");
        for key in ["icon-0", "icon-1"] {
            let item = find(cell, key).unwrap_or_else(|| panic!("the switcher carries {key}"));
            assert!(find(item, "glyph").is_some(), "{key} is an icon tab");
        }
    }

    /// FR-025's step census: the default, low-contrast and icon-only
    /// ramps each answer for all three steps beside each other.
    #[test]
    fn the_sizes_cell_answers_every_step_of_all_three_ramps() {
        let sizes = ContentSwitcher::default().sizes();
        assert_eq!(sizes.len(), 1, "one census cell");
        for key in [
            "size-sw-sm",
            "size-sw-md",
            "size-sw-lg",
            "size-low-sm",
            "size-low-md",
            "size-low-lg",
            "size-icon-sm",
            "size-icon-md",
            "size-icon-lg",
        ] {
            assert!(
                find(&sizes[0], key).is_some(),
                "the step census carries {key}"
            );
        }
    }
}
