//! Inventory row 11, Dropdown.

use gorgon_petra::component::{
    dropdown, dropdown_lg, dropdown_open, dropdown_option, dropdown_option_multiselected,
    dropdown_sm, dropdown_xs, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, sp};

const DD: &str = "dd";
/// The option list's key inside `dropdown_open`.
const MENU: &str = "menu";
const OPTIONS: [(&str, &str); 3] = [
    ("opt-dark", "Dark"),
    ("opt-light", "Light"),
    ("opt-system", "System"),
];

/// Live state of the Dropdown page: whether the list is open, and which
/// option is current.
#[derive(Default)]
pub struct Dropdown {
    open: bool,
    selected: usize,
}

impl Page for Dropdown {
    fn row(&self) -> &'static str {
        "Dropdown"
    }

    /// The warning cell (spec 008 US4, FR-019): the dropdown's own
    /// `dropdown_warning`, one cell — the state the slice documents.
    fn warning(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::dropdown_warning;

        vec![dropdown_warning("dd-warn", "Theme", "Dark", "stale option")]
    }

    /// Carbon's dropdown skeleton (`_dropdown.scss`): the field block with
    /// `border-radius-04`, the md step's 40-high fill at the field's inline
    /// extent.
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

    /// The documented disabled state (spec 008 T025, register row 191): the
    /// documented Select pair at Dropdown's own anatomy — label and value in
    /// `text.muted`, chevron and field edge in their `@disabled` fades.
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![dropdown("disabled-dropdown", "Label", "Choose one")],
        ))]
    }

    fn body(&self) -> ViewNode {
        let value = OPTIONS[self.selected].1;
        let field = if self.open {
            dropdown_open(
                DD,
                "Theme",
                value,
                OPTIONS
                    .iter()
                    .enumerate()
                    .map(|(i, (key, label))| dropdown_option(*key, *label, i == self.selected))
                    .collect(),
            )
        } else {
            dropdown(DD, "Theme", value)
        };
        section(
            "drop",
            "Dropdown",
            vec![
                body("dd-body", sp("spacing.md"), vec![field]),
                // All four of Carbon's list-box sizes, stacked so the
                // xs-to-lg step is one picture to look at rather than a
                // fact only a unit test can see (`button.rs`'s "sizes"
                // row does the same for its own six).
                body(
                    "dd-sizes",
                    sp("spacing.md"),
                    vec![
                        dropdown_xs("dd-xs", "Xs 24", "Dark"),
                        dropdown_sm("dd-sm", "Sm 32", "Dark"),
                        dropdown("dd-md", "Md 40", "Dark"),
                        dropdown_lg("dd-lg", "Lg 48", "Dark"),
                    ],
                ),
                // Register rows #194 (`active`) and #196 (`multiselected`),
                // menu-option sub-states: one plain row the camera holds
                // down, and the multiselect pair — leading checkbox box,
                // checked and unchecked.
                body(
                    "dd-options",
                    sp("spacing.md"),
                    vec![
                        dropdown_option("opt-plain", "Plain", false),
                        dropdown_option_multiselected("opt-multi", "Multi", false),
                        dropdown_option_multiselected("opt-multi-on", "Multi on", true),
                    ],
                ),
            ],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // An option is inside the dropdown's own subtree, so it is matched
        // before the trigger.
        if let Some(hit) = OPTIONS.iter().position(|(key, _)| path_has(node, key)) {
            self.selected = hit;
            self.open = false;
        } else if path_has(node, DD) {
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
