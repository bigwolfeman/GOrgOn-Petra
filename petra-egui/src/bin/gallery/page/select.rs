//! Inventory row 29, Select.

use gorgon_petra::component::{section, select};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Select page. It holds no live state.
pub struct Select;

impl Page for Select {
    fn row(&self) -> &'static str {
        "Select"
    }

    fn body(&self) -> ViewNode {
        section(
            "select",
            "Closed select",
            vec![body(
                "sel",
                sp("spacing.md"),
                vec![select("theme", "Theme", "Dark")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
