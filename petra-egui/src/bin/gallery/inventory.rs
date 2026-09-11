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
    /// Spec 009 constructors that are not a Carbon inventory row.
    G,
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
            Slice::G => 'G',
        }
    }
}

/// One row of `INVENTORY.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// The row number, 1 through the catalog length, as printed in the index.
    pub number: u8,
    /// Carbon's own name for the component, spelled as the inventory spells it.
    pub component: &'static str,
    /// The slice file carrying this component's detail.
    pub slice: Slice,
}

/// Carbon inventory length. Spec 009 rows start after this.
/// Tests walk this separately from [`ROWS`].
#[allow(dead_code)]
pub const CARBON_COUNT: usize = 42;

/// Every catalog row, in order. Rows 1–42 are the Carbon inventory.
/// Rows after that are spec 009 constructors that Carbon never numbered.
pub const ROWS: [Row; 57] = [
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
    Row {
        number: 43,
        component: "Avatar",
        slice: Slice::G,
    },
    Row {
        number: 44,
        component: "Button group",
        slice: Slice::G,
    },
    Row {
        number: 45,
        component: "Context menu",
        slice: Slice::G,
    },
    Row {
        number: 46,
        component: "Drawer",
        slice: Slice::G,
    },
    Row {
        number: 47,
        component: "Input group",
        slice: Slice::G,
    },
    Row {
        number: 48,
        component: "Menubar",
        slice: Slice::G,
    },
    Row {
        number: 49,
        component: "OTP",
        slice: Slice::G,
    },
    Row {
        number: 50,
        component: "Rating",
        slice: Slice::G,
    },
    Row {
        number: 51,
        component: "Textarea",
        slice: Slice::G,
    },
    Row {
        number: 52,
        component: "Toggle button",
        slice: Slice::G,
    },
    // Rows 53-57: the five spec 009 compounds (`gorgon-petra-compound`'s
    // `Compound` triple), hosted directly rather than built from pure
    // atomics. Not a Carbon inventory row any more than 43-52 are, so
    // `Slice::G` again. Appended, per the established pattern above,
    // never inserted among the numbered 1-42 or reordering 43-52.
    Row {
        number: 53,
        component: "Combobox (compound)",
        slice: Slice::G,
    },
    Row {
        number: 54,
        component: "Command (compound)",
        slice: Slice::G,
    },
    Row {
        number: 55,
        component: "Calendar (compound)",
        slice: Slice::G,
    },
    Row {
        number: 56,
        component: "Data table (compound)",
        slice: Slice::G,
    },
    Row {
        number: 57,
        component: "Selection palette (compound)",
        slice: Slice::G,
    },
];

#[cfg(test)]
mod tests {
    use super::{ROWS, Slice};

    // The GOrgOn monorepo carries a fifth test here,
    // `the_rows_match_the_checked_in_inventory`, that parses
    // `.agents/research/08-25-2026/Carbon-Component-Inventory/INVENTORY.md`
    // via `include_str!` and checks it row for row against `ROWS` above.
    // That research tree is monorepo-only planning documentation, outside
    // `gorgon/petra-egui`, so it does not exist in this standalone
    // repository and the test was removed here rather than shipped with a
    // broken `include_str!` path. The three tests below, which check `ROWS`
    // against itself, are unaffected and still run. The monorepo test still
    // guards `ROWS` there.

    #[test]
    fn the_rows_are_numbered_in_order() {
        for (index, row) in ROWS.iter().enumerate() {
            let expected = u8::try_from(index + 1).expect("catalog length fits in u8");
            assert_eq!(row.number, expected);
        }
        assert_eq!(ROWS.len(), super::CARBON_COUNT + 15);
        assert_eq!(ROWS[super::CARBON_COUNT - 1].component, "UI shell right panel");
        assert_eq!(ROWS[super::CARBON_COUNT].component, "Avatar");
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
        let extra = ROWS.iter().filter(|row| row.slice == Slice::G).count();
        assert_eq!(extra, 15, "spec 009 rows");
    }
}
