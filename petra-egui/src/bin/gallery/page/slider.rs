//! Inventory row 30, Slider.

use gorgon_petra::component::{section, slider};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Slider page. It holds no live state.
pub struct Slider;

impl Page for Slider {
    fn row(&self) -> &'static str {
        "Slider"
    }

    fn body(&self) -> ViewNode {
        section(
            "slide",
            "Slider",
            vec![body(
                "slid",
                sp("spacing.md"),
                vec![slider("vol", "Volume", 0.4)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
