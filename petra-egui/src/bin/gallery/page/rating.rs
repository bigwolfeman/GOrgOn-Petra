//! Catalog row 50, Rating.

use gorgon_petra::component::IconMark;
use gorgon_petra::component::{rating, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Rating page. The score is a prop, not a click target, so the page
/// holds the number the photograph should show.
pub struct Rating {
    value: u32,
}

impl Default for Rating {
    fn default() -> Self {
        Self { value: 3 }
    }
}

impl Page for Rating {
    fn row(&self) -> &'static str {
        "Rating"
    }

    fn body(&self) -> ViewNode {
        section(
            "score",
            "Score",
            vec![body(
                "rt-body",
                sp("spacing.md"),
                vec![rating("rating", self.value, 5, IconMark::Check)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
