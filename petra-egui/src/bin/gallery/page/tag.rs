//! Inventory row 33, Tag.

use gorgon_petra::component::{dismissible_tag, section, selectable_tag, tag};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const TAG_SEL: &str = "tag-sel";

/// Live state of the Tag page.
#[derive(Default)]
pub struct Tag {
    tag_sel: bool,
}

impl Page for Tag {
    fn row(&self) -> &'static str {
        "Tag"
    }

    fn body(&self) -> ViewNode {
        section(
            "tags",
            "Tags",
            vec![body(
                "tag-row",
                sp("spacing.md"),
                vec![
                    tag("tag-ro", "Read only"),
                    dismissible_tag("tag-x", "Filter"),
                    // Two fixed instances, unselected and selected: the
                    // page's own interactive `TAG_SEL` only ever shows
                    // one state at a time (defaults false), so a capture
                    // could not show whether selectable_tag's selected
                    // and unselected fills are distinguishable — the
                    // A5 colour-channel audit's own defect in the
                    // catalog, not the component.
                    selectable_tag("tag-sel-off", "Unselected", false),
                    selectable_tag("tag-sel-on", "Selected", true),
                    selectable_tag(TAG_SEL, "Selectable", self.tag_sel),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, TAG_SEL) {
            self.tag_sel = !self.tag_sel;
        } else {
            return false;
        }
        true
    }
}
