//! Inventory row 14, Inline loading.

use gorgon_petra::component::{
    inline_loading, inline_loading_error, inline_loading_finished, inline_loading_in_button,
    section,
};
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
                    // Register row 228, the error state: the filled
                    // warning in support-error, still as a finished mark.
                    inline_loading_error("il-err", "Save failed"),
                    // Register row 224, the button-context modifier: the
                    // same loader with the 32-unit floor taken off, the
                    // height its host button governs instead.
                    inline_loading_in_button("il-btn", "Saving", self.now),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
