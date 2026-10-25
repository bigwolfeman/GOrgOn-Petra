//! Inventory row 28, Search.

use gorgon_petra::component::{search, section, valued};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const Q: &str = "query";

/// The Search page.
///
/// Holds the query, so the field can contain something. `search` writes only
/// a placeholder, the same gap row 34 had: the operator typed into it, saw
/// nothing appear, and called it broken.
#[derive(Default)]
pub struct Search {
    query: String,
}

impl Page for Search {
    fn row(&self) -> &'static str {
        "Search"
    }

    /// Carbon's search skeleton (`_search.scss`): the input block with
    /// `border-radius-04`, inline 100 %, 32/40/48 by step.
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        use super::common::{row, sp};
        Some(row(
            "skeleton-blocks",
            sp("spacing-05"),
            vec![
                skeleton(
                    "skeleton-input-sm",
                    SkeletonAnatomy::Control,
                    SizeStep::Sm,
                    320.0,
                    now,
                ),
                skeleton(
                    "skeleton-input-md",
                    SkeletonAnatomy::Control,
                    SizeStep::Md,
                    320.0,
                    now,
                ),
                skeleton(
                    "skeleton-input-lg",
                    SkeletonAnatomy::Control,
                    SizeStep::Lg,
                    320.0,
                    now,
                ),
            ],
        ))
    }

    /// The documented disabled state (spec 008 T025, register row 413): the
    /// `--disabled` modifier's field (slice-d States) — label and value in
    /// `text.muted`, magnifier and field edge in their fades.
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![search("disabled-search", "Search")],
        ))]
    }

    fn body(&self) -> ViewNode {
        section(
            "search",
            "Search",
            vec![filled_body(
                "q",
                sp("spacing.md"),
                vec![valued(search(Q, "Filter fibers"), self.query.clone())],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if !path_has(node, Q) {
            return false;
        }
        match event {
            InputEvent::Text(typed) => {
                self.query.push_str(typed);
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } => {
                self.query.pop();
                true
            }
            _ => false,
        }
    }
}
