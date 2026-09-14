//! Catalog row 45, Context menu.

use gorgon_petra::component::{
    IconMark, button, context_menu, menu_flyout, menu_item_with, section,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::geom::{Align, Axis, Point};
use gorgon_petra::input::{InputEvent, PointerButton};
use gorgon_petra::tree::{Interaction, NodeKind, Props, ViewNode};

use super::Page;
use super::common::{body, column, dismisses, path_has, sp, wrapped};

const TRIGGER: &str = "show-ctx";
const MENU: &str = "ctx";
const ADD: &str = "ctx-add";
const FLYOUT: &str = "ctx-add-flyout";
const FLYOUT_TRIGGER: &str = "trigger";

const CLOSE: [&str; 3] = ["ctx-undo", "ctx-copy", "ctx-delete"];
const FOLDER: [&str; 3] = ["ctx-folder-work", "ctx-folder-personal", "ctx-folder-new"];

/// Live state of the Context menu page.
///
/// The overlay is [`gorgon_petra::tree::Anchor::Point`]: top-left of the
/// panel at the pointer. That is a context menu, not a dropdown. Row 18
/// (Menu) is the sibling-anchored dropdown. Starts open, fold-out included,
/// so a rest snapshot photographs the panel rather than the closed trigger.
/// Add to folder's flyout stays shut until the pointer sits on that row.
pub struct ContextMenu {
    open: bool,
    at: Point,
    /// Add to folder. Opened by hover, not at rest.
    flyout: bool,
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self {
            open: true,
            // In the page column, below the note. (24, 96) covered the
            // index; (300, 200) covered the section title. 160-wide rows
            // also cannot carry "Copy path" plus a shortcut, so that
            // label is "Copy".
            at: Point::new(340.0, 380.0),
            flyout: false,
        }
    }
}

impl ContextMenu {
    fn close_menu(&mut self) {
        self.open = false;
        self.flyout = false;
    }

    fn over_submenu(node: &str) -> bool {
        path_has(node, ADD)
            || path_has(node, FLYOUT)
            || path_has(node, "ctx-add-open")
            || FOLDER.iter().any(|key| path_has(node, key))
            || (path_has(node, FLYOUT_TRIGGER) && path_has(node, "ctx-add-open"))
    }

    fn add_row(&self) -> ViewNode {
        let item = menu_item_with(
            if self.flyout { FLYOUT_TRIGGER } else { ADD },
            "Add to folder",
            Some(IconMark::Add),
            None,
            true,
        );
        if !self.flyout {
            return item;
        }
        // A hugging vertical stack, not `column()`. `column` is a one-track
        // Grid with `Weight { 1.0 }` and would eat the list-box slot. The
        // flyout is an overlay; the stack only has to keep `trigger` and
        // the flyout as siblings, the same pair menubar uses for File.
        ViewNode::new(NodeKind::Stack, "ctx-add-open")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![
                item,
                menu_flyout(
                    FLYOUT,
                    "Add to folder",
                    vec![
                        menu_item_with("ctx-folder-work", "Work", None, None, false),
                        menu_item_with("ctx-folder-personal", "Personal", None, None, false),
                        menu_item_with(
                            "ctx-folder-new",
                            "Create new folder",
                            Some(IconMark::Add),
                            None,
                            false,
                        ),
                    ],
                ),
            ])
    }

    fn items(&self) -> Vec<ViewNode> {
        vec![
            menu_item_with(
                "ctx-undo",
                "Undo",
                Some(IconMark::Edit),
                Some("Ctrl+Z"),
                false,
            ),
            menu_item_with(
                "ctx-copy",
                "Copy",
                Some(IconMark::Copy),
                Some("Ctrl+C"),
                false,
            ),
            self.add_row(),
            menu_item_with(
                "ctx-delete",
                "Delete",
                Some(IconMark::Close),
                Some("Ctrl+D"),
                false,
            ),
        ]
    }
}

impl Page for ContextMenu {
    fn row(&self) -> &'static str {
        "Context menu"
    }

    fn body(&self) -> ViewNode {
        // A right-click on this button is the real trigger (T014); the
        // press itself still opens it too, so the row is reachable with a
        // primary click or Enter/Space, exactly like every other button.
        // `Interaction::SecondaryClick` is added rather than swapped in,
        // so the node keeps `button()`'s own `Click` declaration and both
        // buttons stay live.
        let mut trigger = button(TRIGGER, "Show menu");
        trigger.interactions.push(Interaction::SecondaryClick);
        let mut pair = vec![trigger];
        if self.open {
            pair.push(context_menu(
                MENU,
                "Actions",
                self.at.x,
                self.at.y,
                self.items(),
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
                        "Right-click Show menu to open it at the pointer; a \
                         plain click or Enter opens it at the button too. \
                         Rows carry a leading icon and a Ctrl+ shortcut. \
                         Hover Add to folder to fold it out to the right. \
                         Top-left of the panel is the press point. A \
                         dropdown from a button is row 18.",
                    ),
                    column("ctx-pair", None, pair),
                ],
            )],
        )
    }

    /// A secondary press on the trigger opens the menu at the press point —
    /// T014's own acceptance, a *real* right-click, not the primary-click
    /// stand-in this page used before the `Interaction::SecondaryClick`
    /// variant existed.
    ///
    /// Only fires from closed. A second secondary press while the menu is
    /// already open is left unconsumed here and falls through to the
    /// ordinary outside-press dismissal instead of repositioning: `gesture`
    /// runs *before* [`Page::dismissed`] delivers this same pass's outside
    /// presses (see that trait method's doc for why the order is chosen,
    /// for `handle`'s own toggle), so state set here would be erased by the
    /// dismissal `dismiss_requests` computes for the same press, the moment
    /// it lands outside the open menu's own rect — which the trigger's rect
    /// always is.
    fn gesture(&mut self, event: &InputEvent, node: &str, _frame: &PetrifiedFrame) -> bool {
        match event {
            InputEvent::PointerMoved { .. } if self.open => {
                self.flyout = Self::over_submenu(node);
            }
            InputEvent::PointerLeft if self.open => {
                self.flyout = false;
            }
            InputEvent::PointerPressed {
                pos,
                button: PointerButton::Secondary,
                ..
            } if !self.open && path_has(node, TRIGGER) => {
                self.at = *pos;
                self.open = true;
                self.flyout = false;
                return true;
            }
            _ => {}
        }
        false
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if path_has(node, ADD) || path_has(node, FLYOUT_TRIGGER) {
            self.flyout = !self.flyout;
            return true;
        }
        if FOLDER.iter().any(|key| path_has(node, key))
            || CLOSE.iter().any(|key| path_has(node, key))
        {
            self.close_menu();
            return true;
        }
        if !path_has(node, TRIGGER) {
            return false;
        }
        match event {
            InputEvent::PointerPressed { pos, .. } => {
                if self.open {
                    self.close_menu();
                } else {
                    self.at = *pos;
                    self.open = true;
                    self.flyout = false;
                }
            }
            _ => {
                self.open = !self.open;
                self.flyout = false;
            }
        }
        true
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, MENU) {
            self.close_menu();
        } else if dismisses(ids, FLYOUT) {
            self.flyout = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CLOSE, ContextMenu, FLYOUT, TRIGGER};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::component::{IconMark, IconTone, icon_toned};
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

    fn has_text(node: &ViewNode, text: &str) -> bool {
        node.props.text.as_deref() == Some(text) || node.children.iter().any(|c| has_text(c, text))
    }

    #[test]
    fn rest_open_shows_icons_shortcuts_and_the_folder_flyout() {
        let mut page = ContextMenu::default();
        let tree = page.body();
        for key in CLOSE {
            assert!(find(&tree, key).is_some(), "rest is missing {key}");
        }
        assert!(
            find(&tree, FLYOUT).is_none(),
            "the flyout stays shut until the pointer sits on Add to folder"
        );
        assert!(
            find(&tree, "ctx-add").is_some(),
            "the closed submenu row keeps key ctx-add"
        );
        assert!(
            has_text(&tree, "Ctrl+Z"),
            "Undo's shortcut is the secondary text"
        );

        let copy = find(&tree, "ctx-copy").expect("copy row");
        let icon = find(copy, "icon").expect("copy leading icon");
        assert_eq!(
            icon.props.canvas,
            icon_toned("icon", IconMark::Copy, IconTone::Primary)
                .props
                .canvas
        );

        assert!(page.handle(&press(), "/page/ctx-pair/show-ctx"));
        let tree = page.body();
        assert!(
            find(&tree, "ctx-undo").is_none(),
            "a press on Show menu must close the open panel"
        );
        assert!(find(&tree, FLYOUT).is_none());
        assert!(find(&tree, TRIGGER).is_some());

        assert!(page.handle(&press(), "/page/ctx-pair/show-ctx"));
        let tree = page.body();
        assert!(
            find(&tree, "ctx-undo").is_some(),
            "a second press must reopen the panel"
        );
        assert!(
            find(&tree, FLYOUT).is_none(),
            "reopening does not restore the fold-out; hover does"
        );
        assert!(find(&tree, "ctx-add").is_some());
    }
}
