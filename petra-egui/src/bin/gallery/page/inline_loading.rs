//! Inventory row 14, Inline loading.

use gorgon_petra::component::{inline_loading, inline_loading_finished, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Inline loading page. Its only state is the host's clock, which the
/// active row's spinner turns on.
#[derive(Default)]
pub struct InlineLoading {
    now: f64,
}

impl Page for InlineLoading {
    fn row(&self) -> &'static str {
        "Inline loading"
    }

    fn tick(&mut self, now: f64) {
        self.now = now;
    }

    fn body(&self) -> ViewNode {
        section(
            "inline",
            "Inline loading",
            vec![body(
                "il",
                sp("spacing.md"),
                vec![
                    inline_loading("il-on", "Saving", self.now),
                    inline_loading_finished("il-off", "Saved"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
