//! Catalog row 45, Context menu.

use gorgon_petra::component::{button, context_menu, menu_item, section};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::geom::Point;
use gorgon_petra::input::{InputEvent, PointerButton};
use gorgon_petra::tree::{Interaction, ViewNode};

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
                        "Right-click Show menu to open it at the pointer; a \
                         plain click or Enter opens it at the button too. \
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
        if self.open || !path_has(node, TRIGGER) {
            return false;
        }
        if let InputEvent::PointerPressed {
            pos,
            button: PointerButton::Secondary,
            ..
        } = event
        {
            self.at = *pos;
            self.open = true;
            return true;
        }
        false
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
