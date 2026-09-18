//! Registry rows: data. See [`super`] for why this file exists.
//!
//! Each row names a public constructor in `petra/petra/src/component/`,
//! deserializes its parameter table into a shape from
//! [`crate::component::params`], and calls the shipped constructor. A row
//! must never re-derive what the constructor does.
//!
//! File covered: `data_table.rs` (Data table) — seventeen public
//! constructors (ten pre-T034, plus T034's toolbar tier: search/filter,
//! column visibility, the batch bar, the row menu, and the skeleton), all
//! registered below. The `every_view_node_constructor_has_exactly_one_row`
//! test proves it by scanning that file's own source text, not by trusting
//! this list.

use serde::Deserialize;
use serde_json::Value;

use super::Entry;
use crate::component as lib;
use crate::component::params::{
    KeyChildrenSelected, KeyLabel, KeyLabelSelected, KeyOnly, ParamError, ParamShape,
};
use crate::tree::{Key, ViewNode};

/// See `registry/form.rs`'s `fail`: names the component being built so a
/// bad parameter table's message points at it.
fn fail(component: &'static str, e: impl std::fmt::Display) -> ParamError {
    ParamError {
        component: component.to_owned(),
        reason: e.to_string(),
    }
}

/// See `registry/form.rs`'s `row!`: one [`Entry`] per macro invocation, a
/// local `fn ctor` coerced to the bare `fn` pointer [`Entry::ctor`] holds.
macro_rules! row {
    ($name:literal, $shape:ty, |$p:ident| $body:expr) => {{
        fn ctor(params: &Value) -> Result<ViewNode, ParamError> {
            let $p: $shape = serde_json::from_value(params.clone()).map_err(|e| fail($name, e))?;
            Ok($body)
        }
        Entry {
            name: $name,
            ctor,
            luau: <$shape as ParamShape>::LUAU,
        }
    }};
}

// ---------------------------------------------------------------------
// One-off shapes.
// ---------------------------------------------------------------------

/// `data_table`/`data_table_zebra`(key, header, rows). Two `Vec<ViewNode>`
/// fields, so neither shared `KeyChildren` (one) nor `KeyChildrenSelected`
/// (one plus a bool) fits.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableParams {
    key: Key,
    /// `#[serde(default)]` because the Lua factory omits an empty table:
    /// Lua cannot distinguish an empty list from an empty map, so sending
    /// one would arrive as `{}` and fail to deserialize.
    #[serde(default)]
    header: Vec<ViewNode>,
    /// See `header`'s doc: same reason.
    #[serde(default)]
    rows: Vec<ViewNode>,
}
impl ParamShape for DataTableParams {
    const LUAU: &'static str = "{ key: string, header: { ViewNode }, rows: { ViewNode } }";
}

/// `data_table_row_expandable(key, cells, selected, expanded, body)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableRowExpandableParams {
    key: Key,
    /// `#[serde(default)]` because the Lua factory omits an empty table:
    /// Lua cannot distinguish an empty list from an empty map, so sending
    /// one would arrive as `{}` and fail to deserialize.
    #[serde(default)]
    children: Vec<ViewNode>,
    selected: bool,
    expanded: bool,
    body: String,
}
impl ParamShape for DataTableRowExpandableParams {
    const LUAU: &'static str = "{ key: string, children: { ViewNode }, selected: boolean, expanded: boolean, body: string }";
}

/// `data_table_row_expandable_actions(key, cells, selected, expanded, body,
/// menu)` (T034, anatomy 5): the expandable row's own row-menu form. The
/// union of `DataTableRowExpandableParams` and the `menu` field, spelled
/// out rather than composed because a `#[serde(flatten)]` on a struct with
/// `deny_unknown_fields` is a known serde conflict and this file's other
/// twelve shapes are flat too.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableRowExpandableActionsParams {
    key: Key,
    /// See `DataTableParams::header`'s doc: same reason.
    #[serde(default)]
    cells: Vec<ViewNode>,
    selected: bool,
    expanded: bool,
    body: String,
    menu: ViewNode,
}
impl ParamShape for DataTableRowExpandableActionsParams {
    const LUAU: &'static str = "{ key: string, cells: { ViewNode }, selected: boolean, expanded: boolean, body: string, menu: ViewNode }";
}

/// `data_table_row_actions(key, cells, selected, menu)` (T034, anatomy 5).
/// `menu` is a single built `ViewNode` (the trigger-plus-overlay control),
/// not one of the shared shapes: no other row constructor in this file
/// takes both a `Vec<ViewNode>` and a lone `ViewNode`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableRowActionsParams {
    key: Key,
    /// See `DataTableParams::header`'s doc: same reason.
    #[serde(default)]
    cells: Vec<ViewNode>,
    selected: bool,
    menu: ViewNode,
}
impl ParamShape for DataTableRowActionsParams {
    const LUAU: &'static str =
        "{ key: string, cells: { ViewNode }, selected: boolean, menu: ViewNode }";
}

/// `data_table_toolbar_menu(key, label, open, items)` (T034, anatomy 2):
/// the toolbar's own settings control. Same four fields as
/// `menu_button`'s row in `navigation.rs`, declared here rather than
/// shared because the two constructors are different controls that happen
/// to take the same arguments, and a shared shape would invite the next
/// reader to conclude they are interchangeable. They are not: see
/// `data_table_toolbar_menu`'s own doc.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableToolbarMenuParams {
    key: Key,
    label: String,
    open: bool,
    /// See `DataTableParams::header`'s doc: same reason.
    #[serde(default)]
    items: Vec<ViewNode>,
}
impl ParamShape for DataTableToolbarMenuParams {
    const LUAU: &'static str = "{ key: string, label: string, open: boolean, items: { ViewNode } }";
}

/// `data_table_toolbar(key, search, trailing)` (T034, anatomy 2).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableToolbarParams {
    key: Key,
    search: ViewNode,
    /// See `DataTableParams::header`'s doc: same reason.
    #[serde(default)]
    trailing: Vec<ViewNode>,
}
impl ParamShape for DataTableToolbarParams {
    const LUAU: &'static str = "{ key: string, search: ViewNode, trailing: { ViewNode } }";
}

/// `data_table_batch_bar(key, count, cancel, actions)` (T034, anatomy 2a).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableBatchBarParams {
    key: Key,
    count: usize,
    cancel: ViewNode,
    /// See `DataTableParams::header`'s doc: same reason.
    #[serde(default)]
    actions: Vec<ViewNode>,
}
impl ParamShape for DataTableBatchBarParams {
    const LUAU: &'static str =
        "{ key: string, count: number, cancel: ViewNode, actions: { ViewNode } }";
}

/// `data_table_sized`/`data_table_zebra_sized`(key, header, rows, weights,
/// dividers, reorderable). `DataTableParams`'s two `Vec<ViewNode>` fields
/// plus the column-weight, divider-policy, and row-reorder trio the sized
/// pair adds over the plain constructors. `weights` is `#[serde(default)]`
/// for the same reason `header`/`rows` are: an empty Lua table for an empty
/// weight list is indistinguishable from an empty map on the wire.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableSizedParams {
    key: Key,
    #[serde(default)]
    header: Vec<ViewNode>,
    #[serde(default)]
    rows: Vec<ViewNode>,
    #[serde(default)]
    weights: Vec<f32>,
    dividers: bool,
    reorderable: bool,
}
impl ParamShape for DataTableSizedParams {
    const LUAU: &'static str = "{ key: string, header: { ViewNode }, rows: { ViewNode }, \
         weights: { number }, dividers: boolean, reorderable: boolean }";
}

/// `data_table_skeleton(key, ncols, nrows)` (T034). Two `usize` fields; no
/// shared shape carries a bare number pair.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableSkeletonParams {
    key: Key,
    ncols: usize,
    nrows: usize,
}
impl ParamShape for DataTableSkeletonParams {
    const LUAU: &'static str = "{ key: string, ncols: number, nrows: number }";
}

// ---------------------------------------------------------------------
// This group's constructors.
// ---------------------------------------------------------------------
pub const ENTRIES: &[Entry] = &[
    row!("data_table", DataTableParams, |p| lib::data_table(
        p.key, p.header, p.rows
    )),
    row!("data_table_zebra", DataTableParams, |p| {
        lib::data_table_zebra(p.key, p.header, p.rows)
    }),
    row!("data_table_row", KeyChildrenSelected, |p| {
        lib::data_table_row(p.key, p.children, p.selected)
    }),
    row!("data_table_row_xs", KeyChildrenSelected, |p| {
        lib::data_table_row_xs(p.key, p.children, p.selected)
    }),
    row!("data_table_row_sm", KeyChildrenSelected, |p| {
        lib::data_table_row_sm(p.key, p.children, p.selected)
    }),
    row!("data_table_row_md", KeyChildrenSelected, |p| {
        lib::data_table_row_md(p.key, p.children, p.selected)
    }),
    row!("data_table_row_lg", KeyChildrenSelected, |p| {
        lib::data_table_row_lg(p.key, p.children, p.selected)
    }),
    row!("data_table_row_xl", KeyChildrenSelected, |p| {
        lib::data_table_row_xl(p.key, p.children, p.selected)
    }),
    row!(
        "data_table_row_expandable",
        DataTableRowExpandableParams,
        |p| lib::data_table_row_expandable(p.key, p.children, p.selected, p.expanded, p.body)
    ),
    // The wire shape is still a bool (`selected`, reused here for
    // "ascending"): the Lua host has no way to ask for
    // `SortDirection::Sortable` yet. That third state is real only to
    // Rust-side composition (`gorgon_petra_compound::data_table`), which
    // calls `data_table_sort_header` directly and never through this row.
    // Widening the wire itself is a separate, larger change (a new
    // `ParamShape`, a new Luau union) and is not done here.
    row!("data_table_sort_header", KeyLabelSelected, |p| {
        lib::data_table_sort_header(
            p.key,
            p.label,
            if p.selected {
                lib::SortDirection::Ascending
            } else {
                lib::SortDirection::Descending
            },
        )
    }),
    // ---- T034: the toolbar tier -------------------------------------
    row!("data_table_row_actions", DataTableRowActionsParams, |p| {
        lib::data_table_row_actions(p.key, p.cells, p.selected, p.menu)
    }),
    row!(
        "data_table_row_expandable_actions",
        DataTableRowExpandableActionsParams,
        |p| lib::data_table_row_expandable_actions(
            p.key, p.cells, p.selected, p.expanded, p.body, p.menu
        )
    ),
    row!("data_table_toolbar", DataTableToolbarParams, |p| {
        lib::data_table_toolbar(p.key, p.search, p.trailing)
    }),
    row!("data_table_toolbar_menu", DataTableToolbarMenuParams, |p| {
        lib::data_table_toolbar_menu(p.key, p.label, p.open, p.items)
    }),
    row!("data_table_batch_bar", DataTableBatchBarParams, |p| {
        lib::data_table_batch_bar(p.key, p.count, p.cancel, p.actions)
    }),
    row!("data_table_batch_cancel", KeyOnly, |p| {
        lib::data_table_batch_cancel(p.key)
    }),
    row!("data_table_batch_action", KeyLabel, |p| {
        lib::data_table_batch_action(p.key, p.label)
    }),
    row!("data_table_row_menu_trigger", KeyLabel, |p| {
        lib::data_table_row_menu_trigger(p.key, p.label)
    }),
    row!("data_table_skeleton", DataTableSkeletonParams, |p| {
        lib::data_table_skeleton(p.key, p.ncols, p.nrows)
    }),
    row!("data_table_sized", DataTableSizedParams, |p| {
        lib::data_table_sized(
            p.key,
            p.header,
            p.rows,
            &p.weights,
            p.dividers,
            p.reorderable,
        )
    }),
    row!("data_table_zebra_sized", DataTableSizedParams, |p| {
        lib::data_table_zebra_sized(
            p.key,
            p.header,
            p.rows,
            &p.weights,
            p.dividers,
            p.reorderable,
        )
    }),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::ENTRIES;

    /// See `registry/form.rs`'s identical helper for the heuristic this
    /// relies on: no signature in `data_table.rs` has a `{` before the one
    /// that opens its body.
    fn public_view_node_constructors(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = source;
        while let Some(at) = rest.find("pub fn ") {
            rest = &rest[at + "pub fn ".len()..];
            let Some(open) = rest.find('(') else { break };
            let name = rest[..open].trim().to_owned();
            let Some(brace) = rest.find('{') else { break };
            let signature = &rest[..brace];
            if signature.contains("-> ViewNode") {
                out.push(name);
            }
            rest = &rest[brace..];
        }
        out
    }

    #[test]
    fn every_view_node_constructor_has_exactly_one_row() {
        let source = include_str!("../data_table.rs");
        let registered: BTreeSet<&str> = ENTRIES.iter().map(|e| e.name).collect();
        assert_eq!(
            registered.len(),
            ENTRIES.len(),
            "two rows in this file share one name"
        );
        let source_names: BTreeSet<String> =
            public_view_node_constructors(source).into_iter().collect();
        // `data_table_menu` is the Fit::Content overflow used by
        // `data_table_toolbar_menu` and the compound row menu, not a Lua
        // constructor of its own.
        let missing: Vec<&String> = source_names
            .iter()
            .filter(|n| !registered.contains(n.as_str()) && n.as_str() != "data_table_menu")
            .collect();
        assert!(
            missing.is_empty(),
            "these `pub fn ... -> ViewNode` constructors in data_table.rs have no registry row: {missing:?}"
        );
        let extra: Vec<&str> = registered
            .iter()
            .filter(|name| !source_names.contains(**name))
            .copied()
            .collect();
        assert!(
            extra.is_empty(),
            "these rows name no `pub fn ... -> ViewNode` in data_table.rs: {extra:?}"
        );
    }

    /// Constructor pairs that build the same node ON PURPOSE — see
    /// `registry/form.rs`'s `ALIASES` for the pattern this follows. A pair
    /// NOT listed here that collides in the probe below is a mis-wired row.
    ///
    /// `data_table_row`/`data_table_row_lg`: `data_table.rs` defines
    /// `data_table_row` itself as `data_table_row_sized(.., RowSize::Lg)`,
    /// and its own doc on `data_table_row_lg` says so outright — "the same
    /// row `data_table_row` builds." One delegates to the other with a
    /// hardcoded size argument; they are not two implementations that
    /// happen to agree by coincidence. `data_table_row_xs/_sm/_md/_xl` each
    /// pass a DIFFERENT `RowSize` into the same `data_table_row_sized` and
    /// so genuinely diverge from every other row at this probe — confirmed
    /// by reading `data_table_row_sized`'s size-to-height mapping, not
    /// assumed from the name.
    const ALIASES: &[(&str, &str, &str)] = &[(
        "data_table_row",
        "data_table_row_lg",
        "data_table_row is itself defined as data_table_row_sized(.., RowSize::Lg); \
         data_table_row_lg's own doc comment says it builds the same row",
    )];

    fn is_known_tie(a: &str, b: &str) -> bool {
        ALIASES
            .iter()
            .any(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a))
    }

    /// See `registry/form.rs`'s identical test for the reasoning.
    #[test]
    fn no_two_rows_of_the_same_shape_build_the_same_view_node_from_the_same_params() {
        use serde_json::json;

        let probes: &[(&str, serde_json::Value)] = &[
            (
                "{ key, label, selected: boolean }",
                json!({"key": "probe", "label": "Probe", "selected": true}),
            ),
            (
                "{ key, children, selected: boolean }",
                json!({"key": "probe", "children": [], "selected": true}),
            ),
            (
                // `data_table_batch_action`/`data_table_row_menu_trigger`:
                // T034's own pair sharing `KeyLabel`, the same "one-off
                // shape used by few constructors" risk the `header`/`rows`
                // probe above already flags.
                "{ key, label }",
                json!({"key": "probe", "label": "Probe"}),
            ),
            (
                // Two rows, not zero: `data_table_zebra` only differs from
                // `data_table` by tagging odd-indexed rows, so an empty
                // `rows` gives both nothing to diverge on and would collide
                // for a reason that has nothing to do with mis-wiring.
                // Confirmed by reading `data_table_zebra`'s body: it maps
                // `rows.enumerate()` and inserts a background token on
                // `i % 2 == 1` before delegating to `data_table`.
                "{ key, header, rows }",
                json!({
                    "key": "probe",
                    "header": [],
                    "rows": [
                        {"kind": "stack", "key": "r0"},
                        {"kind": "stack", "key": "r1"},
                    ],
                }),
            ),
        ];
        let shape_of = |luau: &str| -> Option<&'static str> {
            match luau {
                "{ key: string, label: string, selected: boolean }" => {
                    Some("{ key, label, selected: boolean }")
                }
                "{ key: string, children: { ViewNode }, selected: boolean }" => {
                    Some("{ key, children, selected: boolean }")
                }
                "{ key: string, label: string }" => Some("{ key, label }"),
                // `data_table`/`data_table_zebra`: the only two rows
                // sharing `DataTableParams`. Checked here rather than left
                // out — a one-off shape used by exactly two constructors is
                // exactly the shape most likely to hide a copy-pasted row.
                "{ key: string, header: { ViewNode }, rows: { ViewNode } }" => {
                    Some("{ key, header, rows }")
                }
                _ => None,
            }
        };
        for (shape_name, probe) in probes {
            let mut built: Vec<(&str, crate::tree::ViewNode)> = Vec::new();
            for entry in ENTRIES {
                if shape_of(entry.luau) != Some(*shape_name) {
                    continue;
                }
                let node = (entry.ctor)(probe).unwrap_or_else(|e| {
                    panic!(
                        "probe for shape {shape_name:?} failed on `{}`: {e:?}",
                        entry.name
                    )
                });
                for (other_name, other_node) in &built {
                    if is_known_tie(entry.name, other_name) {
                        continue;
                    }
                    assert_ne!(
                        &node, other_node,
                        "`{}` and `{other_name}` build the identical ViewNode from the same \
                         params ({shape_name}); one of them is wired to the other's constructor",
                        entry.name
                    );
                }
                built.push((entry.name, node));
            }
        }
    }

    /// G4: a bad parameter table's error names the offending field. See
    /// `registry/form.rs`'s identical test for why `deny_unknown_fields`
    /// makes this hold without any hand-written field matching.
    #[test]
    fn a_bad_parameter_table_names_the_offending_field() {
        let sort_header = ENTRIES
            .iter()
            .find(|e| e.name == "data_table_sort_header")
            .expect("data_table_sort_header must be registered");
        let unknown_field = (sort_header.ctor)(&serde_json::json!({
            "key": "k", "label": "L", "selected": true, "lable": "typo"
        }))
        .expect_err("an extra field must be refused");
        assert!(
            unknown_field.reason.contains("lable"),
            "error must name the misspelled field, got: {}",
            unknown_field.reason
        );

        let missing_field = (sort_header.ctor)(&serde_json::json!({ "key": "k", "label": "L" }))
            .expect_err("a missing required field must be refused");
        assert!(
            missing_field.reason.contains("selected"),
            "error must name the missing field, got: {}",
            missing_field.reason
        );
    }
}
