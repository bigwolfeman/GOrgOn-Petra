//! Inventory row 24, Popover.

use gorgon_petra::component::{button, popover, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, dismisses, path_has, sp};

const ANCHOR: &str = "pop-anchor";
const NOTE: &str = "pop-note";

/// Live state of the Popover page: whether the note is showing.
#[derive(Default)]
pub struct Popover {
    open: bool,
}

impl Page for Popover {
    fn row(&self) -> &'static str {
        "Popover"
    }

    fn body(&self) -> ViewNode {
        let mut pair = vec![button(ANCHOR, "Anchor")];
        if self.open {
            pair.push(popover(
                NOTE,
                "Note",
                ANCHOR,
                // Long enough to need two lines at the 368-unit ceiling, so the
                // row demonstrates that a bubble wraps. A one-line note
                // cannot show that, and a one-line note is why the cut-text
                // defect fixed on 2026-09-05 survived four rounds unseen.
                "This note is anchored to the button above it, and it is long \
                 enough to need a second line, which is how you can see that \
                 a bubble wraps its body instead of cutting it.",
            ));
        }
        section(
            "pop",
            "Popover",
            vec![body(
                "po",
                sp("spacing.md"),
                vec![column("po-pair", None, pair)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, ANCHOR) {
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
