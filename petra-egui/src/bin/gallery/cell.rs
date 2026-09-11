//! One gallery cell per inventory row, and what a cell can hand back today.
//!
//! Wave 1 rows are built constructors. Every other cell is
//! [`Content::Unbuilt`], and [`Cell::render`] refuses naming its row. The
//! catalog window builds live pages from application state; `render` on a
//! built row succeeds and returns nothing.

use crate::inventory::{ROWS, Row};

/// Inventory names wired into the catalog. Keep in lockstep with
/// `gorgon/xtask/src/carbon.rs` `BUILT_COMPONENTS` — a `gorgon-xtask` gate
/// that lives only in the private GOrgOn monorepo, not in this repository.
///
/// Test-only, and deliberately so: this list is a second, hand-written copy of
/// a fact `Cell` already derives, kept solely to fail loudly when the two
/// disagree. Compiling it into the shipped binary would let it drift into a
/// second source of truth.
#[cfg(test)]
const BUILT: &[&str] = &[
    "Accordion",
    "AI label",
    "Breadcrumb",
    "Button",
    "Checkbox",
    "Code snippet",
    "Contained list",
    "Content switcher",
    "Data table",
    "Date picker",
    "Dropdown",
    "File uploader",
    "Form",
    "Inline loading",
    "Link",
    "List",
    "Loading",
    "Menu",
    "Menu buttons",
    "Modal",
    "Notification",
    "Number input",
    "Pagination",
    "Popover",
    "Progress bar",
    "Progress indicator",
    "Radio button",
    "Search",
    "Select",
    "Slider",
    "Structured list",
    "Tabs",
    "Tag",
    "Text input",
    "Tile",
    "Toggle",
    "Toggletip",
    "Tooltip",
    "Tree view",
    "UI shell header",
    "UI shell left panel",
    "UI shell right panel",
    "Avatar",
    "Button group",
    "Context menu",
    "Drawer",
    "Input group",
    "Menubar",
    "OTP",
    "Rating",
    "Textarea",
    "Toggle button",
];

/// What a cell holds.
///
/// A second variant is a component, not a flag that one exists — so a cell
/// can never claim to be built while holding nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// No component exists for this row yet.
    Unbuilt,
    /// Carbon Accordion (inventory row 1).
    Accordion,
    /// Carbon AI label (inventory row 2).
    AiLabel,
    /// Carbon Breadcrumb (inventory row 3).
    Breadcrumb,
    /// Carbon Button (inventory row 4).
    Button,
    /// Carbon Checkbox (inventory row 5).
    Checkbox,
    /// Carbon Code snippet (inventory row 6).
    CodeSnippet,
    /// Carbon Contained list (inventory row 7).
    ContainedList,
    /// Carbon Content switcher (inventory row 8).
    #[allow(
        clippy::enum_variant_names,
        reason = "every variant is a Carbon inventory row name verbatim, so                   the catalog page and the inventory row cannot drift apart.                   That row 8 is called \"Content switcher\" and this enum is                   called `Content` is a collision with Carbon's naming, not a                   redundant prefix worth renaming a row over."
    )]
    ContentSwitcher,
    /// Carbon Data table (inventory row 9).
    DataTable,
    /// Carbon Date picker (inventory row 10).
    DatePicker,
    /// Carbon Dropdown (inventory row 11).
    Dropdown,
    /// Carbon File uploader (inventory row 12).
    FileUploader,
    /// Carbon Form (inventory row 13).
    Form,
    /// Carbon Inline loading (inventory row 14).
    InlineLoading,
    /// Carbon Link (inventory row 15).
    Link,
    /// Carbon List (inventory row 16).
    List,
    /// Carbon Loading (inventory row 17).
    Loading,
    /// Carbon Menu (inventory row 18).
    Menu,
    /// Carbon Menu buttons (inventory row 19).
    MenuButtons,
    /// Carbon Modal (inventory row 20).
    Modal,
    /// Carbon Notification (inventory row 21).
    Notification,
    /// Carbon Number input (inventory row 22).
    NumberInput,
    /// Carbon Pagination (inventory row 23).
    Pagination,
    /// Carbon Popover (inventory row 24).
    Popover,
    /// Carbon Progress bar (inventory row 25).
    ProgressBar,
    /// Carbon Progress indicator (inventory row 26).
    ProgressIndicator,
    /// Carbon Radio button (inventory row 27).
    RadioButton,
    /// Carbon Search (inventory row 28).
    Search,
    /// Carbon Select (inventory row 29).
    Select,
    /// Carbon Slider (inventory row 30).
    Slider,
    /// Carbon Structured list (inventory row 31).
    StructuredList,
    /// Carbon Tabs (inventory row 32).
    Tabs,
    /// Carbon Tag (inventory row 33).
    Tag,
    /// Carbon Text input (inventory row 34).
    TextInput,
    /// Carbon Tile (inventory row 35).
    Tile,
    /// Carbon Toggle (inventory row 36). The window builds the knobs from
    /// application state; this tag is the roster fact.
    Toggle,
    /// Carbon Toggletip (inventory row 37).
    Toggletip,
    /// Carbon Tooltip (inventory row 38).
    Tooltip,
    /// Carbon Tree view (inventory row 39).
    TreeView,
    /// Carbon UI shell header (inventory row 40).
    UiShellHeader,
    /// Carbon UI shell left panel (inventory row 41).
    UiShellLeftPanel,
    /// Carbon UI shell right panel (inventory row 42).
    UiShellRightPanel,
    /// Spec 009 avatar (catalog row 43).
    Avatar,
    /// Spec 009 button group (catalog row 44).
    ButtonGroup,
    /// Spec 009 context menu (catalog row 45).
    ContextMenu,
    /// Spec 009 drawer (catalog row 46).
    Drawer,
    /// Spec 009 input group (catalog row 47).
    InputGroup,
    /// Spec 009 menubar (catalog row 48).
    Menubar,
    /// Spec 009 OTP (catalog row 49).
    Otp,
    /// Spec 009 rating (catalog row 50).
    Rating,
    /// Spec 009 textarea (catalog row 51).
    Textarea,
    /// Spec 009 toggle button (catalog row 52).
    ToggleButton,
}

fn content_for(name: &str) -> Content {
    match name {
        "Accordion" => Content::Accordion,
        "AI label" => Content::AiLabel,
        "Breadcrumb" => Content::Breadcrumb,
        "Button" => Content::Button,
        "Checkbox" => Content::Checkbox,
        "Code snippet" => Content::CodeSnippet,
        "Contained list" => Content::ContainedList,
        "Content switcher" => Content::ContentSwitcher,
        "Data table" => Content::DataTable,
        "Date picker" => Content::DatePicker,
        "Dropdown" => Content::Dropdown,
        "File uploader" => Content::FileUploader,
        "Form" => Content::Form,
        "Inline loading" => Content::InlineLoading,
        "Link" => Content::Link,
        "List" => Content::List,
        "Loading" => Content::Loading,
        "Menu" => Content::Menu,
        "Menu buttons" => Content::MenuButtons,
        "Modal" => Content::Modal,
        "Notification" => Content::Notification,
        "Number input" => Content::NumberInput,
        "Pagination" => Content::Pagination,
        "Popover" => Content::Popover,
        "Progress bar" => Content::ProgressBar,
        "Progress indicator" => Content::ProgressIndicator,
        "Radio button" => Content::RadioButton,
        "Search" => Content::Search,
        "Select" => Content::Select,
        "Slider" => Content::Slider,
        "Structured list" => Content::StructuredList,
        "Tabs" => Content::Tabs,
        "Tag" => Content::Tag,
        "Text input" => Content::TextInput,
        "Tile" => Content::Tile,
        "Toggle" => Content::Toggle,
        "Toggletip" => Content::Toggletip,
        "Tooltip" => Content::Tooltip,
        "Tree view" => Content::TreeView,
        "UI shell header" => Content::UiShellHeader,
        "UI shell left panel" => Content::UiShellLeftPanel,
        "UI shell right panel" => Content::UiShellRightPanel,
        "Avatar" => Content::Avatar,
        "Button group" => Content::ButtonGroup,
        "Context menu" => Content::ContextMenu,
        "Drawer" => Content::Drawer,
        "Input group" => Content::InputGroup,
        "Menubar" => Content::Menubar,
        "OTP" => Content::Otp,
        "Rating" => Content::Rating,
        "Textarea" => Content::Textarea,
        "Toggle button" => Content::ToggleButton,
        _ => Content::Unbuilt,
    }
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
                content: content_for(row.component),
            })
            .collect()
    }

    /// Whether this cell can render a component.
    pub fn is_built(&self) -> bool {
        !matches!(self.content, Content::Unbuilt)
    }

    /// Render this cell.
    ///
    /// [`Content::Unbuilt`] refuses, naming the row and its slice file.
    /// [`Content::Toggle`] succeeds with no tree: the catalog window builds
    /// the knobs from application bools so a click can flip them. The
    /// window does not call this; the bin tests do.
    #[allow(dead_code)]
    pub fn render(&self) -> Result<(), String> {
        match self.content {
            Content::Unbuilt => Err(format!(
                "gallery cell {} ({}) is unbuilt: no component exists for this inventory row, \
                 so there is nothing to render. Detail for it is in slice-{}.md.",
                self.row.number,
                self.row.component,
                self.row.slice.letter().to_ascii_lowercase()
            )),
            _ => Ok(()),
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
    use super::{BUILT, Cell, Content, tally};

    #[test]
    fn there_is_one_cell_per_inventory_row_in_inventory_order() {
        let roster = Cell::roster();
        assert_eq!(roster.len(), crate::inventory::ROWS.len());
        for (index, cell) in roster.iter().enumerate() {
            let expected = u8::try_from(index + 1).expect("catalog length fits in u8");
            assert_eq!(cell.row.number, expected);
        }
    }

    /// Built constructors land all forty-two inventory rows. The tally is
    /// the coverage denominator.
    #[test]
    fn wave_one_rows_are_built() {
        let roster = Cell::roster();
        let built: Vec<_> = roster.iter().filter(|cell| cell.is_built()).collect();
        assert_eq!(built.len(), BUILT.len(), "Wave 1 built-row count");
        let names: Vec<_> = built.iter().map(|c| c.row.component).collect();
        assert_eq!(names, BUILT);
        assert_eq!(tally(&roster), (BUILT.len(), crate::inventory::ROWS.len()));
        for cell in &roster {
            if BUILT.contains(&cell.row.component) {
                assert!(cell.is_built());
                assert_ne!(cell.content, Content::Unbuilt);
            } else {
                assert!(!cell.is_built());
                assert_eq!(cell.content, Content::Unbuilt);
            }
        }
    }

    #[test]
    fn the_toggle_cell_render_succeeds_and_carries_no_tree() {
        let toggle = Cell::roster()
            .into_iter()
            .find(|cell| cell.content == Content::Toggle)
            .expect("Toggle is in the roster");
        toggle
            .render()
            .expect("a built Toggle cell does not refuse");
    }

    #[test]
    fn an_unbuilt_cell_refuses_naming_its_row_and_its_slice() {
        for cell in Cell::roster() {
            if cell.is_built() {
                continue;
            }
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
