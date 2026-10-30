//! Inventory row 31, Structured list.

use gorgon_petra::component::{
    heading, section, structured_list_row, structured_list_row_static, structured_list_sized,
    structured_list_weights_at, text,
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

    /// Carbon's structured-list skeleton (`_structured-list.scss`): each
    /// cell span is a `--skeleton` block, 16 high (1rem) × 75 % of the cell
    /// inline, in the row's column track widths.
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        use super::common::{column, row, sp};
        let cell = |key: &str, inline: f32| {
            skeleton(key, SkeletonAnatomy::Text, SizeStep::Md, inline, now)
        };
        Some(column(
            "skeleton-blocks",
            sp("spacing-05"),
            vec![
                row(
                    "skeleton-row-0",
                    sp("spacing.md"),
                    vec![cell("skeleton-c00", 160.0), cell("skeleton-c01", 160.0)],
                ),
                row(
                    "skeleton-row-1",
                    sp("spacing.md"),
                    vec![cell("skeleton-c10", 160.0), cell("skeleton-c11", 160.0)],
                ),
            ],
        ))
    }

    /// The documented disabled state (spec 008 T025, register row 461): the
    /// Selectable variant's rows carry the state (the Default variant is not
    /// operable and has none), so the cell is the selectable list with one
    /// row selected — row text in `text.muted` under `disabled()`.
    fn disabled(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::disabled;

        vec![disabled(structured_list_sized(
            "disabled",
            vec![text("disabled-h0", "Name"), text("disabled-h1", "Role")],
            vec![structured_list_row(
                "disabled-row",
                vec![
                    text("disabled-name", "alpha"),
                    text("disabled-role", "Fiber"),
                ],
                true,
            )],
            &[1.0, 2.0],
            true,
        ))]
    }

    /// Carbon's two row heights (register row #455; slice-e "Sizes"):
    /// Default 60 and Condensed 36 — one labelled cell per built step
    /// (FR-025), each a one-row list so the row height is the thing on
    /// show. Static rows: the live table above owns the selection.
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{
            structured_list, structured_list_condensed, structured_list_row, text,
        };

        use super::common::{column, row, sp};
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                row(
                    "size-default",
                    sp("spacing-03"),
                    vec![
                        text("size-default-label", "default · 60"),
                        structured_list(
                            "size-sl-default",
                            vec![text("size-h0", "Name"), text("size-h1", "Role")],
                            vec![structured_list_row(
                                "size-row-default",
                                vec![text("size-name", "alpha"), text("size-role", "runtime")],
                                false,
                            )],
                        ),
                    ],
                ),
                row(
                    "size-condensed",
                    sp("spacing-03"),
                    vec![
                        text("size-condensed-label", "condensed · 36"),
                        structured_list_condensed(
                            "size-sl-condensed",
                            vec![text("size-ch0", "Name"), text("size-ch1", "Role")],
                            vec![structured_list_row(
                                "size-row-condensed",
                                vec![text("size-cname", "alpha"), text("size-crole", "runtime")],
                                false,
                            )],
                        ),
                    ],
                ),
            ],
        )]
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
                    heading("static-title", "Default rows, non-interactive"),
                    // T055's variant cell: the Default variant's rows,
                    // `structured_list_row_static` — the row shell with no
                    // `interactive` dressing, so a press selects nothing.
                    structured_list_sized(
                        "static-list",
                        vec![text("static-h0", "Name"), text("static-h1", "Role")],
                        vec![
                            structured_list_row_static(
                                "static-row-0",
                                vec![
                                    text("static-name-0", "alpha"),
                                    text("static-role-0", "runtime"),
                                ],
                            ),
                            structured_list_row_static(
                                "static-row-1",
                                vec![
                                    text("static-name-1", "petra"),
                                    text("static-role-1", "layout"),
                                ],
                            ),
                        ],
                        &[1.0, 2.0],
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

#[cfg(test)]
mod tests {
    use super::StructuredList;
    use crate::page::Page;
    use crate::page::common::find;

    /// T055's variant cell (manifest row 463): the Default row is the row
    /// shell with no interactive dressing. The claim is negative and this
    /// is where it is made: the static rows declare no interaction at all,
    /// while the selectable row beside them still does.
    #[test]
    fn the_default_row_cell_is_non_interactive_and_the_selectable_one_is_not() {
        let tree = StructuredList::default().body();
        for key in ["static-row-0", "static-row-1"] {
            let row = find(&tree, key).unwrap_or_else(|| panic!("the cell carries {key}"));
            assert!(
                row.interactions.is_empty(),
                "{key} is a Default row and must declare no interaction"
            );
        }
        let live = find(&tree, "sl-0").expect("the selectable row is on the page");
        assert!(
            !live.interactions.is_empty(),
            "the selectable row beside them stays interactive, or the \
             difference the cell exists to show is gone"
        );
    }
}
