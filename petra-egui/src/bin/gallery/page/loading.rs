//! Inventory row 17, Loading.

use gorgon_petra::component::{loading, loading_sm, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Loading page. It holds no live state.
pub struct Loading;

impl Page for Loading {
    fn row(&self) -> &'static str {
        "Loading"
    }

    fn body(&self) -> ViewNode {
        section(
            "spinner",
            "Loading",
            vec![body(
                "load",
                sp("spacing.md"),
                vec![
                    loading("load-lg", "Working"),
                    loading_sm("load-sm", "Working"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
