//! Inventory row 5, Checkbox.

use gorgon_petra::component::{
    CheckState, checkbox, checkbox_group, checkbox_readonly, checkbox_tristate, checkbox_warning,
    disabled, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const CHECK_A: &str = "check-a";
const CHECK_B: &str = "check-b";
const CHECK_MIXED: &str = "check-mixed";

/// Live state of the Checkbox page.
pub struct Checkbox {
    check_a: bool,
    check_b: bool,
    /// The "Mixed" row cycles through all three states so the operator can
    /// see each one and get back to mixed. Carbon's own `indeterminate` is
    /// an application-owned prop, so what a press does to it is the
    /// application's choice; a cycle is the one that shows every state.
    mixed: CheckState,
}

impl Default for Checkbox {
    fn default() -> Self {
        Self {
            check_a: true,
            check_b: false,
            mixed: CheckState::Mixed,
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
                        checkbox_tristate(CHECK_MIXED, "Mixed", self.mixed),
                        checkbox_readonly("check-ro", "Read only", true),
                        // T0.1: the disabled ink family, on both an
                        // unchecked and a checked box, so the operator can
                        // see the outline fade and the fill swap in the
                        // same shot rather than only one of the two.
                        disabled(checkbox("check-disabled", "Disabled", false)),
                        disabled(checkbox("check-disabled-on", "Disabled, on", true)),
                        checkbox_warning(
                            "check-warn",
                            "Required consent",
                            false,
                            "must be checked",
                        ),
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
        } else if path_has(node, CHECK_MIXED) {
            self.mixed = match self.mixed {
                CheckState::Mixed => CheckState::Checked,
                CheckState::Checked => CheckState::Unchecked,
                CheckState::Unchecked => CheckState::Mixed,
            };
        } else {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{CHECK_MIXED, Checkbox};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    /// "mixed does not work": the row was built by a constructor with no
    /// state behind it. Three presses now walk mixed, checked, unchecked
    /// and back, and the tree the page builds says which it is each time.
    #[test]
    fn a_press_on_mixed_cycles_through_all_three_states() {
        let mut page = Checkbox::default();
        let state = |page: &Checkbox| {
            let tree = page.body();
            let node = find(&tree, CHECK_MIXED).unwrap();
            (node.semantics.selected, node.semantics.value.clone())
        };
        assert_eq!(state(&page), (false, Some("mixed".into())));
        let press = InputEvent::PointerPressed {
            pos: gorgon_petra::geom::Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        assert!(page.handle(&press, "/page/checks/check-group/items/check-mixed/box"));
        assert_eq!(state(&page), (true, None), "mixed then checked");
        assert!(page.handle(&press, "/page/checks/check-group/items/check-mixed"));
        assert_eq!(state(&page), (false, None), "checked then unchecked");
        assert!(page.handle(&press, "/page/checks/check-group/items/check-mixed"));
        assert_eq!(
            state(&page),
            (false, Some("mixed".into())),
            "and back to mixed"
        );
    }
}
