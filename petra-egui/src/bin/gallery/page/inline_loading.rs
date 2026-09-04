//! Inventory row 14, Inline loading.

use gorgon_petra::component::{inline_loading, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Inline loading page. It holds no live state.
pub struct InlineLoading;

impl Page for InlineLoading {
    fn row(&self) -> &'static str {
        "Inline loading"
    }

    fn body(&self) -> ViewNode {
        section(
            "inline",
            "Inline loading",
            vec![body(
                "il",
                sp("spacing.md"),
                vec![
                    inline_loading("il-on", "Saving", true),
                    inline_loading("il-off", "Saved", false),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
