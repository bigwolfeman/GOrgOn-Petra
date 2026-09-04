//! Inventory row 10, Date picker.

use gorgon_petra::component::{date_picker, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Date picker page. It holds no live state.
pub struct DatePicker;

impl Page for DatePicker {
    fn row(&self) -> &'static str {
        "Date picker"
    }

    fn body(&self) -> ViewNode {
        section(
            "date",
            "Closed date picker",
            vec![body(
                "dp",
                sp("spacing.md"),
                vec![date_picker("when", "Date", "2026-08-30")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
