//! Spec 009's Data table compound, hosted directly through the [`Compound`]
//! triple.
//!
//! Row 9, "Data table", is five pure, argument-driven `data_table_row_*`
//! atomics whose page owns selection and Kind text by hand. This page
//! instead owns `data_table::State` itself and drives it through
//! `DataTable::update`: sort, select, select-all, and expand are live
//! intents, not flags the page flips directly.

use gorgon_petra::component::section;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;
use gorgon_petra_compound::Compound;
use gorgon_petra_compound::data_table::{
    Column, DataTable as DataTableCompound, Intent, Props, Row, State,
};

use super::Page;
use super::common::{body, path_has, sp, wrapped};

/// Sample fiber-scheduling rows, distinct from row 9's kernel/petra/luau
/// set so the two pages are never mistaken for the same screenshot.
fn sample_rows() -> Vec<Row> {
    vec![
        Row::new("f0", ["scheduler", "running"]),
        Row::new("f1", ["layout", "idle"]).with_body("Retry policy: exponential backoff, 3 tries."),
        Row::new("f2", ["trace", "idle"]),
        Row::new("f3", ["snapshot", "running"]),
    ]
}

/// Live state of the Data table (compound) page.
pub struct DataTableCompoundPage {
    props: Props,
    state: State,
}

impl Default for DataTableCompoundPage {
    fn default() -> Self {
        let props = Props {
            columns: vec![Column::new("name", "Name"), Column::new("status", "Status")],
            rows: sample_rows(),
        };
        let mut state = DataTableCompound::init(&props);
        // Seeded through real intents, not hand-built state, so the
        // resting picture is what driving the compound actually produces:
        // sorted by name, one row selected, and its body open.
        DataTableCompound::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        DataTableCompound::update(
            &mut state,
            Intent::SelectRow { id: "f1".into() },
        );
        DataTableCompound::update(
            &mut state,
            Intent::ToggleExpand { id: "f1".into() },
        );
        Self { props, state }
    }
}

impl Page for DataTableCompoundPage {
    fn row(&self) -> &'static str {
        "Data table (compound)"
    }

    fn body(&self) -> ViewNode {
        section(
            "table-compound",
            "Data table (compound)",
            vec![body(
                "dtc-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "dtc-note",
                        "The same triple as spec 009 T006, driven through \
                         `Compound::update`: sort, select, select-all, and \
                         expand are live intents rather than flags the \
                         page flips directly.",
                    ),
                    DataTableCompound::view(&self.state, &self.props),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // Select-all is matched first: its own key is unambiguous and does
        // not appear inside any row or header cell's own path.
        if path_has(node, "select-all") {
            let ids: Vec<String> = self.props.rows.iter().map(|r| r.id.clone()).collect();
            DataTableCompound::update(&mut self.state, Intent::SelectAll { ids });
            return true;
        }
        // A sort header's own root key is the column id it sorts, so the
        // column is found the same way a row is found below: by which
        // column's id is a segment of the route.
        if path_has(node, "sort")
            && let Some(col) = self.props.columns.iter().find(|c| path_has(node, &c.id))
        {
            DataTableCompound::update(
                &mut self.state,
                Intent::SortBy {
                    column: col.id.clone(),
                },
            );
            return true;
        }
        // A row's own select checkbox and expand chevron are painted, not
        // independently interactive — `data_table_row`/`data_table_row_
        // expandable` declare `Interaction::Click` on the row as a whole
        // (`ROW_INTENTS`) and nowhere inside it, so hit-testing a press
        // anywhere in a row resolves to the row's own id, never a child's.
        // `page/data_table.rs` (row 9) is built on the same fact: a plain
        // row toggles selection on any press, and its one expandable row
        // toggles expansion on any press, never both. This page follows
        // that same rule rather than inventing a finer-grained hit target
        // the atomic does not offer.
        if let Some(r) = self.props.rows.iter().find(|r| path_has(node, &r.id)) {
            if r.body.is_some() {
                DataTableCompound::update(
                    &mut self.state,
                    Intent::ToggleExpand { id: r.id.clone() },
                );
            } else {
                DataTableCompound::update(&mut self.state, Intent::SelectRow { id: r.id.clone() });
            }
            return true;
        }
        false
    }
}
