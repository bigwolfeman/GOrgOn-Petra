//! The 42 inventory rows, as data.
//!
//! This is identity only — a number, a name, and the research slice that holds
//! the component's detail. No variant, no size, no state, and deliberately **no
//! token name anywhere in this file**: the token vocabulary is being renamed
//! and five slots retired under the same spec, and a scaffold that named tokens
//! would collide with that for no gain. Cell identity does not need them.
//!
//! [`tests::the_rows_match_the_checked_in_inventory`] parses
//! `INVENTORY.md`'s own table and compares it row for row, so this array cannot
//! drift from the ground truth without a test saying which row moved.

/// Which research slice file holds a component's anatomy, variants, sizes and
/// states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slice {
    /// `slice-a.md`
    A,
    /// `slice-b.md`
    B,
    /// `slice-c.md`
    C,
    /// `slice-d.md`
    D,
    /// `slice-e.md`
    E,
    /// `slice-f.md`
    F,
}

impl Slice {
    /// The single letter `INVENTORY.md` prints in its last column.
    pub fn letter(self) -> char {
        match self {
            Slice::A => 'A',
            Slice::B => 'B',
            Slice::C => 'C',
            Slice::D => 'D',
            Slice::E => 'E',
            Slice::F => 'F',
        }
    }
}

/// One row of `INVENTORY.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// The row number, 1 through 42, as printed in the inventory.
    pub number: u8,
    /// Carbon's own name for the component, spelled as the inventory spells it.
    pub component: &'static str,
    /// The slice file carrying this component's detail.
    pub slice: Slice,
}

/// Every inventory row, in inventory order.
///
/// The 42 are the scope, not a starting selection: `contracts/component-anatomy.md`
/// §2 says each row ends built or excluded-with-a-reason, and that silence is
/// not a third option.
pub const ROWS: [Row; 42] = [
    Row {
        number: 1,
        component: "Accordion",
        slice: Slice::A,
    },
    Row {
        number: 2,
        component: "AI label",
        slice: Slice::A,
    },
    Row {
        number: 3,
        component: "Breadcrumb",
        slice: Slice::A,
    },
    Row {
        number: 4,
        component: "Button",
        slice: Slice::A,
    },
    Row {
        number: 5,
        component: "Checkbox",
        slice: Slice::A,
    },
    Row {
        number: 6,
        component: "Code snippet",
        slice: Slice::A,
    },
    Row {
        number: 7,
        component: "Contained list",
        slice: Slice::A,
    },
    Row {
        number: 8,
        component: "Content switcher",
        slice: Slice::B,
    },
    Row {
        number: 9,
        component: "Data table",
        slice: Slice::B,
    },
    Row {
        number: 10,
        component: "Date picker",
        slice: Slice::B,
    },
    Row {
        number: 11,
        component: "Dropdown",
        slice: Slice::B,
    },
    Row {
        number: 12,
        component: "File uploader",
        slice: Slice::B,
    },
    Row {
        number: 13,
        component: "Form",
        slice: Slice::B,
    },
    Row {
        number: 14,
        component: "Inline loading",
        slice: Slice::B,
    },
    Row {
        number: 15,
        component: "Link",
        slice: Slice::C,
    },
    Row {
        number: 16,
        component: "List",
        slice: Slice::C,
    },
    Row {
        number: 17,
        component: "Loading",
        slice: Slice::C,
    },
    Row {
        number: 18,
        component: "Menu",
        slice: Slice::C,
    },
    Row {
        number: 19,
        component: "Menu buttons",
        slice: Slice::C,
    },
    Row {
        number: 20,
        component: "Modal",
        slice: Slice::C,
    },
    Row {
        number: 21,
        component: "Notification",
        slice: Slice::C,
    },
    Row {
        number: 22,
        component: "Number input",
        slice: Slice::D,
    },
    Row {
        number: 23,
        component: "Pagination",
        slice: Slice::D,
    },
    Row {
        number: 24,
        component: "Popover",
        slice: Slice::D,
    },
    Row {
        number: 25,
        component: "Progress bar",
        slice: Slice::D,
    },
    Row {
        number: 26,
        component: "Progress indicator",
        slice: Slice::D,
    },
    Row {
        number: 27,
        component: "Radio button",
        slice: Slice::D,
    },
    Row {
        number: 28,
        component: "Search",
        slice: Slice::D,
    },
    Row {
        number: 29,
        component: "Select",
        slice: Slice::E,
    },
    Row {
        number: 30,
        component: "Slider",
        slice: Slice::E,
    },
    Row {
        number: 31,
        component: "Structured list",
        slice: Slice::E,
    },
    Row {
        number: 32,
        component: "Tabs",
        slice: Slice::E,
    },
    Row {
        number: 33,
        component: "Tag",
        slice: Slice::E,
    },
    Row {
        number: 34,
        component: "Text input",
        slice: Slice::E,
    },
    Row {
        number: 35,
        component: "Tile",
        slice: Slice::E,
    },
    Row {
        number: 36,
        component: "Toggle",
        slice: Slice::F,
    },
    Row {
        number: 37,
        component: "Toggletip",
        slice: Slice::F,
    },
    Row {
        number: 38,
        component: "Tooltip",
        slice: Slice::F,
    },
    Row {
        number: 39,
        component: "Tree view",
        slice: Slice::F,
    },
    Row {
        number: 40,
        component: "UI shell header",
        slice: Slice::F,
    },
    Row {
        number: 41,
        component: "UI shell left panel",
        slice: Slice::F,
    },
    Row {
        number: 42,
        component: "UI shell right panel",
        slice: Slice::F,
    },
];

#[cfg(test)]
mod tests {
    use super::{ROWS, Slice};

    /// `INVENTORY.md`, read at compile time from the research tree it lives
    /// in. Five `..` segments reach the repository root from
    /// `gorgon/petra-egui/src/bin/gallery/`.
    const INVENTORY_MD: &str = include_str!(
        "../../../../../.agents/research/08-25-2026/Carbon-Component-Inventory/INVENTORY.md"
    );

    /// Every `| n | Component | ... | Slice |` row of the inventory table, as
    /// `(number, component, slice letter)`.
    fn parsed_inventory() -> Vec<(u8, String, char)> {
        let mut rows = Vec::new();
        for line in INVENTORY_MD.lines() {
            let line = line.trim();
            if !line.starts_with('|') {
                continue;
            }
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            if cells.len() != 5 {
                continue;
            }
            let Ok(number) = cells[0].parse::<u8>() else {
                continue;
            };
            let mut letters = cells[4].chars();
            let (Some(letter), None) = (letters.next(), letters.next()) else {
                continue;
            };
            rows.push((number, cells[1].to_string(), letter));
        }
        rows
    }

    /// The scaffold's identity is only worth anything if it is the inventory's
    /// identity. A hand-typed copy of a 42-row table drifts; this makes the
    /// drift a failing test naming the row rather than a wrong gallery.
    #[test]
    fn the_rows_match_the_checked_in_inventory() {
        let parsed = parsed_inventory();
        assert_eq!(
            parsed.len(),
            ROWS.len(),
            "INVENTORY.md has {} component rows, this scaffold has {}",
            parsed.len(),
            ROWS.len()
        );
        for (row, (number, component, letter)) in ROWS.iter().zip(parsed) {
            assert_eq!(row.number, number, "row {number}: number");
            assert_eq!(row.component, component, "row {number}: component name");
            assert_eq!(row.slice.letter(), letter, "row {number}: slice");
        }
    }

    #[test]
    fn the_rows_are_numbered_one_to_forty_two_in_order() {
        for (index, row) in ROWS.iter().enumerate() {
            let expected = u8::try_from(index + 1).expect("42 fits in u8");
            assert_eq!(row.number, expected);
        }
    }

    #[test]
    fn component_names_are_unique() {
        let mut names: Vec<&str> = ROWS.iter().map(|row| row.component).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "a component name appears twice");
    }

    /// Seven rows per slice, six slices. Wrong here means a slice file is
    /// being read for a component it does not cover.
    #[test]
    fn every_slice_carries_seven_rows() {
        for slice in [Slice::A, Slice::B, Slice::C, Slice::D, Slice::E, Slice::F] {
            let count = ROWS.iter().filter(|row| row.slice == slice).count();
            assert_eq!(count, 7, "slice {}", slice.letter());
        }
    }
}
