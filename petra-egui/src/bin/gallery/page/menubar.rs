//! Catalog row 48, Menubar.

use gorgon_petra::component::{menu, menu_item, menubar_top, section};
use gorgon_petra::geom::Axis;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{NodeKind, Props, ViewNode};

use super::Page;
use super::common::{body, dismisses, path_has, sp, wrapped};

const FILE: &str = "mb-file";
const EDIT: &str = "mb-edit";
const VIEW: &str = "mb-view";
const TRIGGER: &str = "trigger";

const FILE_ITEMS: [(&str, &str); 3] = [
    ("mb-file-new", "New"),
    ("mb-file-open", "Open"),
    ("mb-file-quit", "Quit"),
];
const EDIT_ITEMS: [(&str, &str); 2] = [("mb-edit-cut", "Cut"), ("mb-edit-copy", "Copy")];
const VIEW_ITEMS: [(&str, &str); 2] = [("mb-view-list", "List"), ("mb-view-grid", "Grid")];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
    File,
    Edit,
    View,
}

/// Live state of the Menubar page: which top-level menu is open.
///
/// The bar itself is [`menubar_top`]: it docks to the window's top edge.
/// Open menus are a different constructor. This page mounts [`menu`] as a
/// sibling of the open label, keyed `trigger`, the same pair row 18 uses.
#[derive(Default)]
pub struct Menubar {
    open: Option<Open>,
}


impl Menubar {
    fn slot(which: Open, open: Option<Open>) -> ViewNode {
        let (id, label, items): (&str, &str, &[(&str, &str)]) = match which {
            Open::File => (FILE, "File", &FILE_ITEMS),
            Open::Edit => (EDIT, "Edit", &EDIT_ITEMS),
            Open::View => (VIEW, "View", &VIEW_ITEMS),
        };
        let item_key = if open == Some(which) { TRIGGER } else { id };
        let item = menu_item(item_key, label);
        if open != Some(which) {
            return item;
        }
        let slot_key = format!("{id}-open");
        // A hugging vertical stack, not `column()`. `column` is a one-track
        // Grid with `Weight { 1.0 }`. Inside the menubar's horizontal run
        // that track eats every remaining unit, so labels to the right
        // collapse. The menu is an overlay; the stack only has to keep
        // `trigger` and the menu as siblings.
        ViewNode::new(NodeKind::Stack, slot_key)
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .with_children(vec![
                item,
                menu(
                    format!("{id}-menu"),
                    label,
                    items
                        .iter()
                        .map(|(key, label)| menu_item(*key, *label))
                        .collect(),
                ),
            ])
    }
}

impl Page for Menubar {
    fn row(&self) -> &'static str {
        "Menubar"
    }

    fn body(&self) -> ViewNode {
        section(
            "bar",
            "Top edge",
            vec![body(
                "mb-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "mb-note",
                        "File, Edit and View dock to the top of the window. \
                         Press a label to open its menu.",
                    ),
                    menubar_top(
                        "menubar",
                        "Application",
                        vec![
                            Self::slot(Open::File, self.open),
                            Self::slot(Open::Edit, self.open),
                            Self::slot(Open::View, self.open),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        let in_items = |items: &[(&str, &str)]| items.iter().any(|(key, _)| path_has(node, key));
        if in_items(&FILE_ITEMS) || in_items(&EDIT_ITEMS) || in_items(&VIEW_ITEMS) {
            self.open = None;
            return true;
        }
        let hit = if path_has(node, FILE)
            || (self.open == Some(Open::File) && path_has(node, TRIGGER))
        {
            Some(Open::File)
        } else if path_has(node, EDIT)
            || (self.open == Some(Open::Edit) && path_has(node, TRIGGER))
        {
            Some(Open::Edit)
        } else if path_has(node, VIEW)
            || (self.open == Some(Open::View) && path_has(node, TRIGGER))
        {
            Some(Open::View)
        } else {
            None
        };
        let Some(which) = hit else {
            return false;
        };
        self.open = if self.open == Some(which) {
            None
        } else {
            Some(which)
        };
        true
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, "mb-file-menu")
            || dismisses(ids, "mb-edit-menu")
            || dismisses(ids, "mb-view-menu")
        {
            self.open = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EDIT, FILE, TRIGGER, VIEW, Menubar};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::geom::Point;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    fn press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: Point::ZERO,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn opening_edit_leaves_file_and_view_on_the_bar() {
        let mut page = Menubar::default();
        let tree = page.body();
        assert!(find(&tree, FILE).is_some());
        assert!(find(&tree, EDIT).is_some());
        assert!(find(&tree, VIEW).is_some());

        assert!(page.handle(&press(), "/page/menubar/content/mb-edit"));
        let tree = page.body();
        assert!(
            find(&tree, FILE).is_some(),
            "File must stay on the bar while Edit is open"
        );
        assert!(
            find(&tree, VIEW).is_some(),
            "View must stay on the bar while Edit is open"
        );
        assert!(
            find(&tree, TRIGGER).is_some(),
            "the open label is the menu's sibling trigger"
        );
        assert!(
            find(&tree, EDIT).is_none(),
            "the open label is re-keyed to trigger, not mb-edit"
        );
    }
}
