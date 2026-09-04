//! Inventory row 26, Progress indicator.

use gorgon_petra::component::{progress_indicator, progress_step, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Progress indicator page. It holds no live state.
pub struct ProgressIndicator;

impl Page for ProgressIndicator {
    fn row(&self) -> &'static str {
        "Progress indicator"
    }

    fn body(&self) -> ViewNode {
        section(
            "steps",
            "Steps",
            vec![body(
                "pi",
                sp("spacing.md"),
                vec![progress_indicator(
                    "pi",
                    vec![
                        progress_step("st-0", "Clone", true, false),
                        progress_step("st-1", "Build", false, true),
                        progress_step("st-2", "Run", false, false),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
