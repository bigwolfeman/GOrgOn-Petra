//! Catalog row 44, Button group.

use gorgon_petra::component::{
    button, button_group, button_group_flush, ghost_button, primary_button, section,
    tertiary_button,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, sp};

/// `button()`'s Carbon md height (spec 009 T027's flush row shares it with
/// every child, since a flush segmented group is one uniform height).
const BUTTON_HEIGHT_MD: f32 = 40.0;

/// The Button group page. The container holds no state. Two sections: row 4's
/// buttons in a gapped row (spec 009's original shape), and the same buttons
/// in a zero-gap flush row so the squared seam is visible in a capture.
pub struct ButtonGroup;

impl Default for ButtonGroup {
    fn default() -> Self {
        Self
    }
}

impl Page for ButtonGroup {
    fn row(&self) -> &'static str {
        "Button group"
    }

    fn body(&self) -> ViewNode {
        column(
            "group",
            sp("spacing.lg"),
            vec![
                section(
                    "spaced",
                    "Spaced group",
                    vec![body(
                        "btns",
                        sp("spacing.md"),
                        vec![button_group(
                            "bg",
                            vec![
                                primary_button("bg-primary", "Save"),
                                button("bg-default", "Discard"),
                                tertiary_button("bg-tertiary", "More"),
                                ghost_button("bg-ghost", "Cancel"),
                            ],
                        )],
                    )],
                ),
                section(
                    "flush",
                    "Flush group",
                    vec![body(
                        "flush-btns",
                        sp("spacing.md"),
                        vec![button_group_flush(
                            "bg-flush",
                            BUTTON_HEIGHT_MD,
                            vec![
                                button("flush-first", "Left"),
                                button("flush-mid", "Middle"),
                                button("flush-last", "Right"),
                            ],
                        )],
                    )],
                ),
            ],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
