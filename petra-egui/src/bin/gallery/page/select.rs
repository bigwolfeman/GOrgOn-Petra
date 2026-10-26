//! Inventory row 29, Select.

use gorgon_petra::component::{dropdown_option, section, select, select_open};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{dismisses, filled_body, path_has, sp};

const THEME: &str = "theme";
/// The option list's key inside `select_open`.
const MENU: &str = "menu";
const OPTIONS: [(&str, &str); 3] = [
    ("opt-dark", "Dark"),
    ("opt-light", "Light"),
    ("opt-system", "System"),
];

/// Live state of the Select page: whether the list is open, and which
/// option is current.
#[derive(Default)]
pub struct Select {
    open: bool,
    selected: usize,
}

impl Page for Select {
    fn row(&self) -> &'static str {
        "Select"
    }

    /// The warning cell (spec 008 US4, FR-019): the select's own
    /// `select_warning`, one cell — the state the slice documents.
    fn warning(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::select_warning;

        vec![select_warning("sel-warn", "Theme", "Dark", "stale option")]
    }

    /// Carbon's select skeleton (`_select.scss`): the input block, 2.5rem
    /// (40) high at the md step, inline 100 %.
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        Some(skeleton(
            "skeleton-field",
            SkeletonAnatomy::Control,
            SizeStep::Md,
            320.0,
            now,
        ))
    }

    /// Carbon's select sizing (`_select.scss`, 32/40/48 by step): the three
    /// built steps side by side, each labeled with its step and the height
    /// it lands on (FR-025).
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{select_lg, select_sm, text};

        use super::common::{column, row, sp};
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                row(
                    "size-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-sm-label", "sm · 32"),
                        select_sm("size-select-sm", "Label", "Option"),
                    ],
                ),
                row(
                    "size-md",
                    sp("spacing-03"),
                    vec![
                        text("size-md-label", "md · 40"),
                        select("size-select-md", "Label", "Option"),
                    ],
                ),
                row(
                    "size-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-lg-label", "lg · 48"),
                        select_lg("size-select-lg", "Label", "Option"),
                    ],
                ),
            ],
        )]
    }

    /// The documented disabled state (spec 008 T025, register row 431):
    /// `.cds--label` reads in `text.muted` and `.cds--select__arrow` in the
    /// `$icon-disabled` fade (slice-e), with the field edge's own fade.
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![select("disabled-select", "Theme", "Dark")],
        ))]
    }

    fn body(&self) -> ViewNode {
        let value = OPTIONS[self.selected].1;
        // `filled_body`, not `body`: a Carbon select is 100% of its
        // container's width (slice-b §29), so the column stretches it.
        let field = if self.open {
            select_open(
                THEME,
                "Theme",
                value,
                OPTIONS
                    .iter()
                    .enumerate()
                    .map(|(i, (key, label))| dropdown_option(*key, *label, i == self.selected))
                    .collect(),
            )
        } else {
            select(THEME, "Theme", value)
        };
        section(
            "select",
            "Select",
            vec![filled_body("sel", sp("spacing.md"), vec![field])],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if let Some(hit) = OPTIONS.iter().position(|(key, _)| path_has(node, key)) {
            self.selected = hit;
            self.open = false;
        } else if path_has(node, THEME) {
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
