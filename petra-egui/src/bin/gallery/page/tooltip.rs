//! Inventory row 38, Tooltip.

use gorgon_petra::component::{ghost_button, section, tooltip};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, path_has, sp};

/// The key `tooltip` anchors to: its trigger must be a sibling with this key.
const TRIGGER: &str = "trigger";
const BUBBLE: &str = "bubble";

/// Live state of the Tooltip page: whether the pointer is on the trigger,
/// and whether keyboard focus is.
///
/// Carbon reveals a tooltip on hover and on focus, and hides it when both
/// are gone (slice-c §38). Both states are the engine's, not the
/// application's (`contracts/interaction-state.md` §1): the trigger's *lit*
/// state is derived by the host's hit test and painted through
/// `background@hover`, and the focus ring is painted from the host's focus
/// tree; this page never touches either. What the page owns is the tree,
/// and a bubble is a node, so whether the bubble is in the tree is a fact
/// only the page can hold. `hovered` is set from the routed pointer events
/// the chrome offers [`Page::gesture`] — the engine's own route, not a
/// second hit test — and cleared by a pointer-exit or by a move that routed
/// nowhere. `focused` is set from [`Page::focused`], the host's own focus
/// tree reporting a move.
///
/// Two flags and not one, because they leave separately: the pointer can
/// wander off while the trigger still has focus, and Tab can move focus on
/// while the pointer sits on the trigger. The bubble stays until both are
/// gone.
#[derive(Default)]
pub struct Tooltip {
    hovered: bool,
    focused: bool,
}

impl Tooltip {
    fn open(&self) -> bool {
        self.hovered || self.focused
    }
}

impl Page for Tooltip {
    fn row(&self) -> &'static str {
        "Tooltip"
    }

    fn body(&self) -> ViewNode {
        // A ghost button, not `link`: the trigger has to declare `Hover`
        // to be a hit-test candidate for a pointer move, and `link` does
        // not. Carbon's own tooltip demo hangs off a ghost button too.
        let mut pair = vec![ghost_button(TRIGGER, "Save")];
        if self.open() {
            pair.push(tooltip(BUBBLE, "Save", "Save writes the composition."));
        }
        section(
            "tip",
            "Tooltip",
            vec![body(
                "tip-body",
                sp("spacing.md"),
                vec![column("tip-pair", None, pair)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }

    /// A move that routed to the trigger shows the bubble; a move that
    /// routed anywhere else, or nowhere (`node` empty), or a pointer-exit,
    /// drops the hover reason for it. Nothing is consumed: a move is not an
    /// activation, and a press on the trigger is still the button's to take.
    fn gesture(&mut self, event: &InputEvent, node: &str, _frame: &PetrifiedFrame) -> bool {
        match event {
            InputEvent::PointerMoved { .. } => self.hovered = path_has(node, TRIGGER),
            InputEvent::PointerLeft => self.hovered = false,
            _ => {}
        }
        false
    }

    /// Focus landing on the trigger shows the bubble; focus landing
    /// anywhere else, or nowhere, drops the focus reason for it.
    fn focused(&mut self, node: Option<&str>) {
        self.focused = node.is_some_and(|node| path_has(node, TRIGGER));
    }
}
