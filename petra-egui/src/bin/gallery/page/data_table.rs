//! Inventory row 9, Data table.

use gorgon_petra::component::{data_table_row, data_table_zebra, section, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Data table page. It holds no live state.
pub struct DataTable;

impl Page for DataTable {
    fn row(&self) -> &'static str {
        "Data table"
    }

    fn body(&self) -> ViewNode {
        section(
            "table",
            "Data table",
            vec![body(
                "dt",
                sp("spacing.md"),
                // Four rows, not two: with two, the one striped row is
                // also the selected row and the stripe cannot be seen.
                vec![data_table_zebra(
                    "dt",
                    vec![text("h0", "Name"), text("h1", "Kind")],
                    vec![
                        data_table_row(
                            "dt-0",
                            vec![text("c0", "kernel"), text("c1", "runtime")],
                            false,
                        ),
                        data_table_row(
                            "dt-1",
                            vec![text("c0", "petra"), text("c1", "layout")],
                            true,
                        ),
                        data_table_row(
                            "dt-2",
                            vec![text("c0", "luau"), text("c1", "plugin host")],
                            false,
                        ),
                        data_table_row(
                            "dt-3",
                            vec![text("c0", "helix"), text("c1", "editor")],
                            false,
                        ),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
