//! Catalog row 46, Drawer.

use gorgon_petra::component::{button, docked, section, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{Edge, InputPolicy, ViewNode};

use super::Page;
use super::common::{body, column, path_has, sp, wrapped};

const OPEN: &str = "open-drawer";
const PANEL: &str = "drawer";
const CLOSE: &str = "close-drawer";

/// Live state of the Drawer page.
///
/// The catalog uses [`InputPolicy::Passthrough`]. `drawer()` is
/// [`InputPolicy::Block`] and would swallow Prev/Next and the index.
/// A gallery that cannot page is not a gallery. `sheet()` would close
/// on an outside press; Passthrough leaves the panel up and lets the
/// chrome through. Start closed so the first frame is the trigger.
#[derive(Default)]
pub struct Drawer {
    open: bool,
}

impl Page for Drawer {
    fn row(&self) -> &'static str {
        "Drawer"
    }

    fn body(&self) -> ViewNode {
        let mut children = vec![
            button(OPEN, "Open drawer"),
            wrapped(
                "drawer-note",
                "The panel docks to the right edge. The catalog still \
                 takes clicks: this page uses Passthrough, not Block.",
            ),
        ];
        children.push(docked(
            PANEL,
            Edge::Right,
            self.open,
            column(
                "drawer-body",
                sp("spacing.md"),
                vec![
                    text("drawer-title", "Session"),
                    text("drawer-copy", "The panel docks inside the window edge."),
                    button(CLOSE, "Close"),
                ],
            ),
            InputPolicy::Passthrough,
        ));
        section(
            "panel",
            "Drawer",
            vec![body(
                "dr-body",
                sp("spacing.md"),
                vec![column("dr-col", sp("spacing.md"), children)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, CLOSE) {
            self.open = false;
        } else if path_has(node, OPEN) {
            self.open = true;
        } else {
            return false;
        }
        true
    }
}
