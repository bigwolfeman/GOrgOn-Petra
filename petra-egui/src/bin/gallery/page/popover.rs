//! Inventory row 24, Popover.

use gorgon_petra::component::{button, heading, popover_with, primary_button, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, dismisses, path_has, row, sp, wrapped};

const ANCHOR: &str = "pop-anchor";
const NOTE: &str = "pop-note";
const SHUT: &str = "pop-shut";

/// Live state of the Popover page: whether the panel is showing.
///
/// It starts **open**. A closed disclosure is one button and no
/// disclosure, which is what the operator walked past on rows 24 and 37;
/// the resting capture has to photograph the thing itself.
pub struct Popover {
    open: bool,
}

impl Default for Popover {
    fn default() -> Self {
        Self { open: true }
    }
}

impl Page for Popover {
    fn row(&self) -> &'static str {
        "Popover"
    }

    fn body(&self) -> ViewNode {
        let mut pair = vec![button(ANCHOR, "Filters")];
        if self.open {
            // A popover is the *container* Toggletip, Tooltip, Dropdown and
            // Menu all compose on, so the only thing that demonstrates it
            // is what lives inside: a title, a line of prose that has to
            // wrap at the 368 ceiling, and two real controls. Row 37's
            // bubble is 288 wide, a different tone, and holds a note.
            pair.push(popover_with(
                NOTE,
                "Filter fibers",
                ANCHOR,
                vec![
                    heading("pop-title", "Filter fibers"),
                    wrapped(
                        "pop-body",
                        "A popover is the anchored surface every other overlay \
                         in this library is built on. Its contents are the \
                         caller's, and they keep their own roles.",
                    ),
                    row(
                        "pop-actions",
                        sp("spacing.sm"),
                        vec![
                            button("pop-cancel", "Cancel"),
                            primary_button("pop-apply", "Apply"),
                        ],
                    ),
                ],
            ));
        }
        section(
            "pop",
            "Popover",
            vec![body(
                "po",
                sp("spacing.md"),
                vec![
                    column("po-pair", None, pair),
                    column("po-shut", None, vec![button(SHUT, "Closed trigger")]),
                    wrapped(
                        "po-note",
                        "Press either trigger. The panel is 368 wide on \
                         surface.raised; a toggletip is 288 on the ramp's \
                         last rung.",
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, ANCHOR) || path_has(node, SHUT) {
            self.open = !self.open;
            return true;
        }
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, NOTE) {
            self.open = false;
        }
    }
}
