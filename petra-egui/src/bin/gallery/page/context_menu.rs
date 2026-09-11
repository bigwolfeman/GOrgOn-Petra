//! Catalog row 45, Context menu.

use gorgon_petra::component::{button, context_menu, menu_item, section};
use gorgon_petra::geom::Point;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, dismisses, path_has, sp, wrapped};

const TRIGGER: &str = "show-ctx";
const MENU: &str = "ctx";
const ITEMS: [(&str, &str); 3] = [
    ("ctx-rename", "Rename"),
    ("ctx-copy", "Copy path"),
    ("ctx-delete", "Delete"),
];

/// Live state of the Context menu page.
///
/// The overlay is [`gorgon_petra::tree::Anchor::Point`]: top-left of the
/// panel at the pointer. That is a context menu, not a dropdown. Row 18
/// (Menu) is the sibling-anchored dropdown. Starts closed so the trigger
/// is what you click.
pub struct ContextMenu {
    open: bool,
    at: Point,
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self {
            open: false,
            at: Point::new(0.0, 0.0),
        }
    }
}

impl Page for ContextMenu {
    fn row(&self) -> &'static str {
        "Context menu"
    }

    fn body(&self) -> ViewNode {
        let mut pair = vec![button(TRIGGER, "Show menu")];
        if self.open {
            pair.push(context_menu(
                MENU,
                "Actions",
                self.at.x,
                self.at.y,
                ITEMS
                    .iter()
                    .map(|(key, label)| menu_item(*key, *label))
                    .collect(),
            ));
        }
        section(
            "ctx-sec",
            "Point-anchored menu",
            vec![body(
                "ctx-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "ctx-note",
                        "Opens at the pointer. Top-left of the panel is \
                         the click. A dropdown from a button is row 18.",
                    ),
                    column("ctx-pair", None, pair),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if ITEMS.iter().any(|(key, _)| path_has(node, key)) {
            self.open = false;
            return true;
        }
        if !path_has(node, TRIGGER) {
            return false;
        }
        match event {
            InputEvent::PointerPressed { pos, .. } => {
                if self.open {
                    self.open = false;
                } else {
                    self.at = *pos;
                    self.open = true;
                }
            }
            _ => self.open = !self.open,
        }
        true
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, MENU) {
            self.open = false;
        }
    }
}
