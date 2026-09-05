//! Inventory row 37, Toggletip.

use gorgon_petra::component::{heading, text};
use gorgon_petra::component::{link, section, toggletip_with};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, row, sp, wrapped};

/// `toggletip`'s own keys for its trigger and its popover.
const TRIGGER: &str = "trigger";
const TIP: &str = "tip";

/// Live state of the Toggletip page: whether the tip is showing.
///
/// It starts **open**, for the reason row 24 does: a closed disclosure
/// photographs as one control and no disclosure, and "I cannot tell these
/// two rows apart" is answered by a picture or not at all.
pub struct Toggletip {
    open: bool,
}

impl Default for Toggletip {
    fn default() -> Self {
        Self { open: true }
    }
}

impl Page for Toggletip {
    fn row(&self) -> &'static str {
        "Toggletip"
    }

    fn body(&self) -> ViewNode {
        section(
            "tt",
            "Toggletip",
            vec![body(
                "tt-body",
                sp("spacing.md"),
                vec![
                    heading("tt-title", "Why is this fiber parked?"),
                    // Carbon's toggletip: a click-triggered disclosure on
                    // the popover surface, 288 rather than 368, inverse
                    // rather than `$layer`, and an optional actions row —
                    // which is the half that makes it a toggletip and not
                    // a tooltip, since a tooltip may hold nothing a person
                    // can press.
                    toggletip_with(
                        "tt",
                        "Why",
                        self.open,
                        vec![
                            wrapped(
                                "tt-note",
                                "It is waiting on a channel no other fiber \
                                 writes to.",
                            ),
                            row(
                                "tt-actions",
                                sp("spacing.sm"),
                                vec![link("tt-more", "Open the trace")],
                            ),
                        ],
                    ),
                    text("tt-hint", "Press the mark to open and again to close."),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, TRIGGER) {
            self.open = !self.open;
            return true;
        }
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, TIP) {
            self.open = false;
        }
    }
}
