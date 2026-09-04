//! Inventory row 21, Notification.

use gorgon_petra::component::{notification, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Notification page. It holds no live state.
pub struct Notification;

impl Page for Notification {
    fn row(&self) -> &'static str {
        "Notification"
    }

    fn body(&self) -> ViewNode {
        section(
            "note",
            "Notification",
            vec![body(
                "nt",
                sp("spacing.md"),
                vec![notification(
                    "nt",
                    "Rebuild finished",
                    "12 fibers reloaded.",
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
