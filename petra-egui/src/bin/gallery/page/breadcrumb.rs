//! Inventory row 3, Breadcrumb.

use gorgon_petra::component::{breadcrumb, breadcrumb_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Breadcrumb page. It holds no live state.
pub struct Breadcrumb;

impl Page for Breadcrumb {
    fn row(&self) -> &'static str {
        "Breadcrumb"
    }

    fn body(&self) -> ViewNode {
        section(
            "trail",
            "Trail",
            vec![body(
                "crumbs",
                sp("spacing.md"),
                vec![breadcrumb(
                    "crumbs",
                    vec![
                        breadcrumb_item("bc-0", "Workspace"),
                        breadcrumb_item("bc-1", "Fibers"),
                        breadcrumb_item("bc-2", "Rebuild"),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
