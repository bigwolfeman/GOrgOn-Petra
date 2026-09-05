//! Inventory row 4, Button.

use gorgon_petra::component::{
    button, button_lg, button_sm, danger_button, danger_ghost_button, danger_tertiary_button,
    ghost_button, primary_button, section, tertiary_button,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, row, sp};

/// The Button page. It holds no live state.
pub struct Button;

impl Page for Button {
    fn row(&self) -> &'static str {
        "Button"
    }

    fn body(&self) -> ViewNode {
        section(
            "variants",
            "Variants and sizes",
            vec![body(
                "buttons",
                sp("spacing.md"),
                vec![
                    row(
                        "kinds",
                        sp("spacing.md"),
                        vec![
                            primary_button("btn-primary", "Primary"),
                            button("btn-default", "Default"),
                            tertiary_button("btn-tertiary", "Tertiary"),
                            ghost_button("btn-ghost", "Ghost"),
                        ],
                    ),
                    // Carbon's danger triple, directly under the four safe
                    // variants so "danger against default" is one glance.
                    // All three lead with the `status.down` octagon in
                    // `support-error`: the channel that tells danger from
                    // default without spending a colour alone.
                    //
                    // **Why the three sit alone in their own row.**
                    // `layout::stack::distribute` clips the child with the
                    // largest natural width to the mean of the row's
                    // naturals, and a button that grew a 16-unit mark is
                    // exactly that child. Measured 2026-09-05: `Cancel`
                    // (72) beside `Delete` (98) placed the second at 84.7
                    // and broke its label across two lines inside a
                    // 40-unit box, while two `Delete`s beside each other
                    // both placed at 98. Equal naturals, no clip. The
                    // reproduction is in this wave's Agent Note, and
                    // `the_danger_buttons_carry_a_red_octagon...` asserts
                    // the label stays one line so it cannot regress
                    // quietly.
                    row(
                        "danger",
                        sp("spacing.md"),
                        vec![
                            danger_button("btn-danger", "Delete"),
                            danger_tertiary_button("btn-danger-tertiary", "Delete"),
                            danger_ghost_button("btn-danger-ghost", "Delete"),
                        ],
                    ),
                    row(
                        "sizes",
                        sp("spacing.md"),
                        vec![
                            button_sm("btn-sm", "Small 32"),
                            button("btn-md", "Medium 40"),
                            button_lg("btn-lg", "Large 48"),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
