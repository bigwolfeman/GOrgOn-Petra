//! One gallery cell per inventory row, and what a cell can hand back today.
//!
//! Today the answer is: nothing. Every cell is [`Content::Unbuilt`], and
//! [`Cell::render`] refuses naming its row. That is the whole point of a
//! scaffold — the shape of the surface exists and is walkable, and not one of
//! the 42 can be mistaken for finished.

use std::convert::Infallible;

use crate::inventory::{ROWS, Row};

/// What a cell holds.
///
/// One variant, on purpose. A second variant arrives with the first built
/// component, and it will carry that component rather than a flag saying one
/// exists — so a cell can never claim to be built while holding nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// No component exists for this row yet.
    Unbuilt,
}

/// One cell of the component gallery.
///
/// A cell is the unit `contracts/component-anatomy.md` §8 requires to be
/// reachable: a combination the capture loop cannot reach cannot be captured,
/// and therefore cannot be gated. At scaffold time a cell is one inventory row.
/// It multiplies by variant, size and state as components land — which is how
/// 42 rows become the 323-cell parity matrix, and why the roster is data rather
/// than 42 hand-written blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The inventory row this cell stands for.
    pub row: &'static Row,
    /// What the cell holds.
    pub content: Content,
}

impl Cell {
    /// Every cell, one per inventory row, in inventory order.
    pub fn roster() -> Vec<Cell> {
        ROWS.iter()
            .map(|row| Cell {
                row,
                content: Content::Unbuilt,
            })
            .collect()
    }

    /// Whether this cell can render a component.
    pub fn is_built(&self) -> bool {
        match self.content {
            Content::Unbuilt => false,
        }
    }

    /// Render this cell. **Refuses**, for every one of the 42.
    ///
    /// The success side is [`Infallible`] because there is no code path that
    /// produces a rendered cell yet. When the first component lands, both the
    /// type and the body change together; until then the compiler will not let
    /// a caller pretend otherwise.
    pub fn render(&self) -> Result<Infallible, String> {
        match self.content {
            Content::Unbuilt => Err(format!(
                "gallery cell {} ({}) is unbuilt: no component exists for this inventory row, \
                 so there is nothing to render. Detail for it is in slice-{}.md.",
                self.row.number,
                self.row.component,
                self.row.slice.letter().to_ascii_lowercase()
            )),
        }
    }
}

/// How many cells are built, and how many exist.
pub fn tally(roster: &[Cell]) -> (usize, usize) {
    (
        roster.iter().filter(|cell| cell.is_built()).count(),
        roster.len(),
    )
}

#[cfg(test)]
mod tests {
    use super::{Cell, Content, tally};

    #[test]
    fn there_is_one_cell_per_inventory_row_in_inventory_order() {
        let roster = Cell::roster();
        assert_eq!(roster.len(), 42);
        for (index, cell) in roster.iter().enumerate() {
            let expected = u8::try_from(index + 1).expect("42 fits in u8");
            assert_eq!(cell.row.number, expected);
        }
    }

    /// The scaffold's honest starting position, asserted rather than described.
    /// This test is meant to fail the day a component lands — that failure is
    /// the reminder to move the row out of "unbuilt" here and in the coverage
    /// gate at the same time.
    #[test]
    fn every_cell_is_unbuilt_today() {
        let roster = Cell::roster();
        assert!(roster.iter().all(|cell| cell.content == Content::Unbuilt));
        assert_eq!(tally(&roster), (0, 42));
    }

    #[test]
    fn an_unbuilt_cell_refuses_naming_its_row_and_its_slice() {
        for cell in Cell::roster() {
            let err = cell.render().unwrap_err();
            assert!(err.contains(cell.row.component), "{err}");
            assert!(err.contains("is unbuilt"), "{err}");
            assert!(
                err.contains(&format!(
                    "slice-{}.md",
                    cell.row.slice.letter().to_ascii_lowercase()
                )),
                "{err}"
            );
        }
    }
}
