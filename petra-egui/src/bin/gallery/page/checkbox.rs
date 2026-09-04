//! Inventory row 5, Checkbox.

use gorgon_petra::component::{
    checkbox, checkbox_group, checkbox_indeterminate, checkbox_readonly, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const CHECK_A: &str = "check-a";
const CHECK_B: &str = "check-b";

/// Live state of the Checkbox page.
pub struct Checkbox {
    check_a: bool,
    check_b: bool,
}

impl Default for Checkbox {
    fn default() -> Self {
        Self {
            check_a: true,
            check_b: false,
        }
    }
}

impl Page for Checkbox {
    fn row(&self) -> &'static str {
        "Checkbox"
    }

    fn body(&self) -> ViewNode {
        section(
            "states",
            "States",
            vec![body(
                "checks",
                sp("spacing.md"),
                vec![checkbox_group(
                    "check-group",
                    "Notifications",
                    vec![
                        checkbox(CHECK_A, "Email", self.check_a),
                        checkbox(CHECK_B, "Push", self.check_b),
                        checkbox_indeterminate("check-mixed", "Mixed"),
                        checkbox_readonly("check-ro", "Read only", true),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, CHECK_A) {
            self.check_a = !self.check_a;
        } else if path_has(node, CHECK_B) {
            self.check_b = !self.check_b;
        } else {
            return false;
        }
        true
    }
}
