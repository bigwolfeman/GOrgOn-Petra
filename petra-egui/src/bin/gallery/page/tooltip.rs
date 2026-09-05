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

/// Live state of the Tooltip page: whether the pointer is on the trigger.
///
/// A tooltip is revealed by hover, and hover is the engine's, not the
/// application's (`contracts/interaction-state.md` §1): the *lit* state
/// of the trigger is derived by the host's hit test and painted through
/// `background@hover`, and this page never touches it. What the page owns
/// is the tree, and a bubble is a node, so whether the bubble is in the
/// tree is a fact only the page can hold. It is set from the routed
/// pointer events the chrome offers [`Page::gesture`] — the engine's own
/// route, not a second hit test — and cleared by a pointer-exit or by a
/// move that routed nowhere.
///
/// Not built: revealing on keyboard focus, which Carbon also does. Focus
/// is host-owned and no routed event tells the page it arrived.
#[derive(Default)]
pub struct Tooltip {
    open: bool,
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
        if self.open {
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
    /// hides it. Nothing is consumed: a move is not an activation, and a
    /// press on the trigger is still the button's to take.
    fn gesture(&mut self, event: &InputEvent, node: &str, _frame: &PetrifiedFrame) -> bool {
        match event {
            InputEvent::PointerMoved { .. } => self.open = path_has(node, TRIGGER),
            InputEvent::PointerLeft => self.open = false,
            _ => {}
        }
        false
    }
}
