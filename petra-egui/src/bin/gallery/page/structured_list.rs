//! Inventory row 31, Structured list.

use gorgon_petra::component::{
    section, structured_list_row, structured_list_sized, structured_list_weights_at, text,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp, wrapped};

/// The rows, as the Carbon reference shot has them (`31-structured-list.png`).
const ROWS: [(&str, &str, &str); 2] = [("sl-0", "kernel", "runtime"), ("sl-1", "petra", "layout")];

/// Live state of the Structured list page.
///
/// Carbon's selectable structured list is single-select (a hidden radio
/// per row, slice-e anatomy 4), so a press on a row moves the selection to
/// it. The column weights are here rather than in the component for the
/// same reason a slider's value is: a component builds a tree and holds
/// nothing, so the thing a drag changes has to be the page's.
pub struct StructuredList {
    selected: usize,
    /// One weight per data column. A drag on a divider rewrites two of
    /// them, keeping their sum, so no other column moves.
    weights: Vec<f32>,
    /// Whether a press on a divider opened a gesture that has not ended.
    ///
    /// The whole reason this bit exists is round 2's slider: `gesture`
    /// there took a position from **every** positional event it was handed,
    /// so the operator's pointer merely passing the control moved it. The
    /// engine's rule is that a press on a `Drag` node grants the capture,
    /// every positional event until the gesture ends routes to the holder,
    /// and the end is always announced. This mirrors exactly that: set by
    /// the press, cleared by the end, and no move is a drag without it.
    dragging: bool,
}

impl Default for StructuredList {
    fn default() -> Self {
        Self {
            selected: 1,
            weights: vec![1.0, 1.0],
            dragging: false,
        }
    }
}

impl Page for StructuredList {
    fn row(&self) -> &'static str {
        "Structured list"
    }

    fn body(&self) -> ViewNode {
        let rows = ROWS
            .iter()
            .enumerate()
            .map(|(i, (key, name, role))| {
                structured_list_row(
                    *key,
                    vec![text("name", *name), text("role", *role)],
                    i == self.selected,
                )
            })
            .collect();
        section(
            "table",
            "Structured list",
            vec![body(
                "sl",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "note",
                        "This is also the Table row. Carbon Table is this \
                         structured list; there is no second component.",
                    ),
                    structured_list_sized(
                        "sl",
                        vec![text("h0", "Name"), text("h1", "Role")],
                        rows,
                        &self.weights,
                        // The operator's round-3 ask: dividers on by default.
                        // `structured_list_sized`'s flag is the "toggled off in
                        // code" half; this page is the on case.
                        true,
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        let Some(i) = ROWS.iter().position(|(key, _, _)| path_has(node, key)) else {
            return false;
        };
        self.selected = i;
        true
    }

    /// The press opens the drag and seats the boundary under the pointer;
    /// every move and the release while it is open follow the pointer; the
    /// gesture's end closes it. A move with no drag open is a pointer
    /// passing by, and this page does nothing with it.
    fn gesture(&mut self, event: &InputEvent, node: &str, frame: &PetrifiedFrame) -> bool {
        let moved = |page: &mut Self, pos| match structured_list_weights_at(
            frame,
            node,
            pos,
            &page.weights,
        ) {
            Some(weights) => {
                page.weights = weights;
                true
            }
            None => false,
        };
        match event {
            InputEvent::GestureEnded { .. } => {
                if !self.dragging {
                    return false;
                }
                self.dragging = false;
                true
            }
            InputEvent::PointerPressed { pos, .. } => {
                if !moved(self, *pos) {
                    return false;
                }
                self.dragging = true;
                true
            }
            InputEvent::PointerMoved { pos } | InputEvent::PointerReleased { pos, .. } => {
                if !self.dragging {
                    return false;
                }
                moved(self, *pos)
            }
            _ => false,
        }
    }
}
