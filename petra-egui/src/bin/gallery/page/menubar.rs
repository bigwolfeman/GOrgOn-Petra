//! Catalog row 48, Menubar.

use gorgon_petra::component::{
    IconMark, menu, menu_flyout, menu_item, menu_item_with, menubar_top, section,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{NodeKind, Props, ViewNode};

use super::Page;
use super::common::{body, path_has, sp, wrapped};

const FILE: &str = "mb-file";
const EDIT: &str = "mb-edit";
const VIEW: &str = "mb-view";
const TRIGGER: &str = "trigger";
const FILE_MENU: &str = "mb-file-menu";
const EDIT_MENU: &str = "mb-edit-menu";
const VIEW_MENU: &str = "mb-view-menu";
const RECENT_ITEM: &str = "mb-file-recent-item";
const RECENT_FLYOUT: &str = "mb-file-recent";
const RECENT_OPEN: &str = "mb-file-recent-open";

const FILE_LEAVES: [&str; 3] = ["mb-file-new", "mb-file-open", "mb-file-quit"];
const EDIT_ITEMS: [(&str, &str); 3] = [
    ("mb-edit-cut", "Cut"),
    ("mb-edit-copy", "Copy"),
    ("mb-edit-find", "Find"),
];
const VIEW_ITEMS: [(&str, &str); 2] = [("mb-view-list", "List"), ("mb-view-grid", "Grid")];
const RECENT_LEAVES: [(&str, &str); 2] = [("project-one", "GOrgOn"), ("project-two", "Petra")];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
    File,
    Edit,
    View,
}

/// Live state of the Menubar page: which top-level menu is open, and whether
/// File's Open Recent flyout is showing.
///
/// The bar itself is [`menubar_top`]: it docks to the window's top edge.
/// Open menus are a different constructor. This page mounts [`menu`] as a
/// sibling of the open label, keyed `trigger`, the same pair row 18 uses.
/// File's Open Recent row mounts [`menu_flyout`] the same way, in a nested
/// hugging stack, so the two `trigger` keys live under different parents.
pub struct Menubar {
    open: Option<Open>,
    flyout: bool,
}

impl Default for Menubar {
    fn default() -> Self {
        Self {
            open: Some(Open::File),
            flyout: false,
        }
    }
}

impl Menubar {
    fn over_recent(node: &str) -> bool {
        path_has(node, RECENT_ITEM)
            || path_has(node, RECENT_OPEN)
            || path_has(node, RECENT_FLYOUT)
            || RECENT_LEAVES.iter().any(|(key, _)| path_has(node, key))
    }

    fn slot(which: Open, open: Option<Open>, flyout: bool) -> ViewNode {
        let (id, label) = match which {
            Open::File => (FILE, "File"),
            Open::Edit => (EDIT, "Edit"),
            Open::View => (VIEW, "View"),
        };
        let item_key = if open == Some(which) { TRIGGER } else { id };
        let item = menu_item(item_key, label);
        if open != Some(which) {
            return item;
        }
        let items = match which {
            Open::File => file_items(flyout),
            Open::Edit => edit_items(),
            Open::View => view_items(),
        };
        hugging(
            format!("{id}-open"),
            vec![item, menu(format!("{id}-menu"), label, items)],
        )
    }
}

/// A hugging vertical stack, not `column()`. `column` is a one-track Grid
/// with `Weight { 1.0 }`. Inside the menubar's horizontal run that track
/// eats every remaining unit, so labels to the right collapse. The same
/// thing happens if a flyout sibling sits in a column inside the open
/// File menu. The overlay takes no flow size; the stack only has to keep
/// `trigger` and the overlay as siblings. Stretch on the cross axis makes
/// a nested Open Recent row as wide as the menu it sits in, so the flyout
/// leaves from the menu's trailing edge.
fn hugging(key: impl Into<String>, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key.into())
        .with_props(Props {
            axis: Some(Axis::Vertical),
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(children)
}

fn file_items(flyout: bool) -> Vec<ViewNode> {
    let recent = if flyout {
        hugging(
            RECENT_OPEN,
            vec![
                menu_item_with(TRIGGER, "Open Recent", Some(IconMark::Menu), None, true),
                menu_flyout(
                    RECENT_FLYOUT,
                    "Open Recent",
                    RECENT_LEAVES
                        .iter()
                        .map(|(key, label)| menu_item(*key, *label))
                        .collect(),
                ),
            ],
        )
    } else {
        menu_item_with(RECENT_ITEM, "Open Recent", Some(IconMark::Menu), None, true)
    };
    vec![
        menu_item_with(
            "mb-file-new",
            "New",
            Some(IconMark::Add),
            Some("Ctrl+N"),
            false,
        ),
        menu_item_with(
            "mb-file-open",
            "Open",
            Some(IconMark::Search),
            Some("Ctrl+O"),
            false,
        ),
        recent,
        menu_item_with(
            "mb-file-quit",
            "Quit",
            Some(IconMark::Close),
            Some("Ctrl+Q"),
            false,
        ),
    ]
}

fn edit_items() -> Vec<ViewNode> {
    vec![
        menu_item_with(
            "mb-edit-cut",
            "Cut",
            Some(IconMark::Subtract),
            Some("Ctrl+X"),
            false,
        ),
        menu_item_with(
            "mb-edit-copy",
            "Copy",
            Some(IconMark::Copy),
            Some("Ctrl+C"),
            false,
        ),
        menu_item_with(
            "mb-edit-find",
            "Find",
            Some(IconMark::Search),
            Some("Ctrl+F"),
            false,
        ),
    ]
}

fn view_items() -> Vec<ViewNode> {
    VIEW_ITEMS
        .iter()
        .map(|(key, label)| menu_item(*key, *label))
        .collect()
}

/// Whether any dismissed id *is* the surface keyed `key`, not merely
/// contains it. The Open Recent flyout lives inside the File menu, so its
/// path carries `mb-file-menu` as an ancestor; matching any segment would
/// close File when only the flyout was dismissed.
fn dismissed_surface(ids: &[String], key: &str) -> bool {
    ids.iter().any(|id| id.rsplit('/').next() == Some(key))
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
                         Menus list icons and Ctrl+ shortcuts. Hover File → \
                         Open Recent to fold it out to the right.",
                    ),
                    menubar_top(
                        "menubar",
                        "Application",
                        vec![
                            Self::slot(Open::File, self.open, self.flyout),
                            Self::slot(Open::Edit, self.open, self.flyout),
                            Self::slot(Open::View, self.open, self.flyout),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        let in_keys = |keys: &[&str]| keys.iter().any(|key| path_has(node, key));
        let in_items = |items: &[(&str, &str)]| items.iter().any(|(key, _)| path_has(node, key));

        if in_items(&RECENT_LEAVES) {
            self.open = None;
            self.flyout = false;
            return true;
        }
        if path_has(node, RECENT_ITEM) || path_has(node, RECENT_OPEN) {
            self.flyout = !self.flyout;
            self.open = Some(Open::File);
            return true;
        }
        if in_keys(&FILE_LEAVES) || in_items(&EDIT_ITEMS) || in_items(&VIEW_ITEMS) {
            self.open = None;
            self.flyout = false;
            return true;
        }
        let hit = if path_has(node, FILE)
            || (self.open == Some(Open::File) && path_has(node, TRIGGER))
        {
            Some(Open::File)
        } else if path_has(node, EDIT) || (self.open == Some(Open::Edit) && path_has(node, TRIGGER))
        {
            Some(Open::Edit)
        } else if path_has(node, VIEW) || (self.open == Some(Open::View) && path_has(node, TRIGGER))
        {
            Some(Open::View)
        } else {
            None
        };
        let Some(which) = hit else {
            return false;
        };
        if self.open == Some(which) {
            self.open = None;
            self.flyout = false;
        } else {
            self.open = Some(which);
            self.flyout = false;
        }
        true
    }

    fn gesture(&mut self, event: &InputEvent, node: &str, _frame: &PetrifiedFrame) -> bool {
        if self.open != Some(Open::File) {
            return false;
        }
        match event {
            InputEvent::PointerMoved { .. } => {
                self.flyout = Self::over_recent(node);
            }
            InputEvent::PointerLeft => {
                self.flyout = false;
            }
            _ => {}
        }
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismissed_surface(ids, RECENT_FLYOUT) {
            self.flyout = false;
        }
        if dismissed_surface(ids, FILE_MENU)
            || dismissed_surface(ids, EDIT_MENU)
            || dismissed_surface(ids, VIEW_MENU)
        {
            self.open = None;
            self.flyout = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EDIT, FILE, FILE_MENU, Menubar, RECENT_FLYOUT, TRIGGER, VIEW};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::geom::Point;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};
    use gorgon_petra::tree::ViewNode;

    fn press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: Point::ZERO,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    fn contains_text(node: &ViewNode, needle: &str) -> bool {
        if node.props.text.as_deref() == Some(needle) {
            return true;
        }
        node.children
            .iter()
            .any(|child| contains_text(child, needle))
    }

    #[test]
    fn default_rests_on_file_with_the_open_recent_flyout() {
        let tree = Menubar::default().body();
        assert!(
            find(&tree, FILE_MENU).is_some(),
            "File's menu is open at rest"
        );
        assert!(
            find(&tree, RECENT_FLYOUT).is_none(),
            "Open Recent stays shut until the pointer sits on it"
        );
        assert!(
            contains_text(&tree, "Ctrl+N"),
            "File → New carries a Ctrl+N shortcut"
        );
        assert!(
            find(&tree, TRIGGER).is_some(),
            "File is re-keyed to trigger while its menu is open"
        );
        assert!(
            find(&tree, FILE).is_none(),
            "the open File label is trigger, not mb-file"
        );
        assert!(find(&tree, EDIT).is_some(), "Edit stays on the bar");
        assert!(find(&tree, VIEW).is_some(), "View stays on the bar");
    }

    #[test]
    fn opening_edit_leaves_file_and_view_on_the_bar() {
        let mut page = Menubar::default();
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
