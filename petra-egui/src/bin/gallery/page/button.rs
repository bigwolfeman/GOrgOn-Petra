//! Inventory row 4, Button.

use gorgon_petra::component::{
    button, button_2xl, button_lg, button_sm, button_xl, button_xs, danger_button,
    danger_ghost_button, danger_tertiary_button, ghost_button, primary_button, section,
    tertiary_button,
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
                    // This row used to carry a claim that the three had to
                    // sit apart from the kinds row because
                    // `layout::stack::distribute` clipped a mixed-width
                    // sibling and broke `Delete`'s label across two lines.
                    // Round 3 (wave STACK) put the trio back into the kinds
                    // row and rasterized twice, with and without a
                    // candidate fix: byte-identical both times, one line
                    // both times. The defect did not reproduce, so that
                    // justification was stale (`ROUND3-DEFECTS.md`). The
                    // split here is a grouping choice, not a layout
                    // workaround; not re-verified in this change.
                    row(
                        "danger",
                        sp("spacing.md"),
                        vec![
                            danger_button("btn-danger", "Delete"),
                            danger_tertiary_button("btn-danger-tertiary", "Delete"),
                            danger_ghost_button("btn-danger-ghost", "Delete"),
                        ],
                    ),
                    // All six of Carbon's size steps, in order, so the
                    // xs-to-2xl progression is one row to look at rather
                    // than a fact only a unit test can see.
                    row(
                        "sizes",
                        sp("spacing.md"),
                        vec![
                            button_xs("btn-xs", "X-small 24"),
                            button_sm("btn-sm", "Small 32"),
                            button("btn-md", "Medium 40"),
                            button_lg("btn-lg", "Large 48"),
                            button_xl("btn-xl", "X-large 64"),
                            button_2xl("btn-2xl", "2X-large 80"),
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
