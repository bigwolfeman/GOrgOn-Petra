//! Inventory row 7, Contained list.

use gorgon_petra::component::{contained_list, list_row, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const ROWS: [(&str, &str); 2] = [("cl-0", "Trace"), ("cl-1", "Store")];

/// Live state of the Contained list page: which row, if any, is selected.
///
/// `list_row` is a selectable `Role::ListItem` that declares `Click`, so a
/// page that mounts it and holds no selection has a row that lights on
/// hover and does nothing on press — the pattern this catalog is fixing.
#[derive(Default)]
pub struct ContainedList {
    selected: Option<usize>,
}

impl Page for ContainedList {
    fn row(&self) -> &'static str {
        "Contained list"
    }

    /// Carbon's contained-list ramp (slice-a "Sizes"): on-page sm 32 /
    /// md 40 / lg 48 with header and rows matching, xl header 48 / row 64,
    /// and the disclosed pair — header 32 / row 48 — beside them. Each
    /// labeled with its step and the heights it lands on (FR-025).
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{
            contained_list_disclosed, contained_list_lg, contained_list_sm, contained_list_xl,
            list_row, text,
        };

        use super::common::{column, sp};
        let rows = |tag: &str| vec![list_row(format!("size-clr-{tag}"), "alpha", false)];
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                column(
                    "size-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-sm-label", "sm · 32/32"),
                        contained_list_sm("size-cl-sm", "Recent", rows("sm")),
                    ],
                ),
                column(
                    "size-md",
                    sp("spacing-03"),
                    vec![
                        text("size-md-label", "md · 40/40"),
                        contained_list("size-cl-md", "Recent", rows("md")),
                    ],
                ),
                column(
                    "size-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-lg-label", "lg · 48/48"),
                        contained_list_lg("size-cl-lg", "Recent", rows("lg")),
                    ],
                ),
                column(
                    "size-xl",
                    sp("spacing-03"),
                    vec![
                        text("size-xl-label", "xl · 48/64"),
                        contained_list_xl("size-cl-xl", "Recent", rows("xl")),
                    ],
                ),
                column(
                    "size-disclosed",
                    sp("spacing-03"),
                    vec![
                        text("size-disclosed-label", "disclosed · 32/48"),
                        contained_list_disclosed("size-cl-disclosed", "Recent", rows("dis")),
                    ],
                ),
            ],
        )]
    }

    /// The documented disabled state (spec 008 T025, register row 107): a
    /// disabled contained list — item text in `text.muted`, the item edge
    /// in the `border@disabled` fade (slice-a States).
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![contained_list(
                "disabled-list",
                "Fibers",
                vec![
                    list_row("disabled-row", "alpha", false),
                    list_row("disabled-row-2", "beta", false),
                ],
            )],
        ))]
    }

    fn body(&self) -> ViewNode {
        section(
            "on-page",
            "On-page header",
            vec![filled_body(
                "contained",
                sp("spacing.md"),
                vec![contained_list(
                    "cl",
                    "Recent",
                    ROWS.iter()
                        .enumerate()
                        .map(|(i, (key, label))| list_row(*key, *label, self.selected == Some(i)))
                        .collect(),
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        let Some(hit) = ROWS.iter().position(|(key, _)| path_has(node, key)) else {
            return false;
        };
        // A second press on the selected row clears the selection, so
        // "nothing selected" is reachable again.
        self.selected = (self.selected != Some(hit)).then_some(hit);
        true
    }
}
