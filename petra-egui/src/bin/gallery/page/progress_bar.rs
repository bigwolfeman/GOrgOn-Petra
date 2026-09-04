//! Inventory row 25, Progress bar.

use gorgon_petra::component::{progress, progress_sm, progress_with_helper, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Progress bar page. It holds no live state.
pub struct ProgressBar;

impl Page for ProgressBar {
    fn row(&self) -> &'static str {
        "Progress bar"
    }

    fn body(&self) -> ViewNode {
        section(
            "bars",
            "Determinate",
            vec![body(
                "progress",
                sp("spacing.md"),
                vec![
                    progress("prog-big", "Rebuild", 0.62),
                    progress_sm("prog-sm", "Upload", 0.25),
                    progress_with_helper("prog-help", "Index", 1.0, "Complete"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
