//! Inventory row 17, Loading.

use gorgon_petra::component::{loading, loading_sm, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, row, sp};

/// The Loading page. Its only state is the host's clock, which the two
/// spinners turn on.
#[derive(Default)]
pub struct Loading {
    now: f64,
}

impl Page for Loading {
    fn row(&self) -> &'static str {
        "Loading"
    }

    fn tick(&mut self, now: f64) {
        self.now = now;
    }

    fn body(&self) -> ViewNode {
        // Large and small side by side, centred on each other, as the
        // Carbon reference page lays them out (`17-loading.png`).
        section(
            "spinner",
            "Loading",
            vec![body(
                "load",
                sp("spacing.md"),
                vec![row(
                    "sizes",
                    sp("spacing-05"),
                    vec![
                        loading("load-lg", "Working", self.now),
                        loading_sm("load-sm", "Working", self.now),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
