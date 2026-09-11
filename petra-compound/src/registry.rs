//! Registry rows for the five compounds: name a pure `view` from Lua.
//!
//! `gorgon_petra::component::registry`'s own module doc names this crate as
//! the intended caller of `register_external`, and settles the design this
//! file follows without re-opening it: a row here is a **pure view
//! constructor**. It deserializes both the compound's props and its whole
//! state from the params table and calls `Compound::view`. It never calls
//! `Compound::init` — that would render, say, a combobox that is
//! permanently shut or a calendar permanently on one month, which is a lie
//! about an interactive component. `Compound::State` is bound
//! `Serialize + DeserializeOwned` (`lib.rs`) precisely so this is free. The
//! live loop reaches the same `view` the other way, through a fiber, in a
//! later leaf — both doors open onto one function.
//!
//! # Why every name carries `_compound`
//!
//! `combobox`, `command` and `calendar` are all free in the merged table at
//! the time this was written, but `data_table` is already the name of
//! `gorgon_petra::component::data_table`'s pure, argument-driven atomic — a
//! genuinely different constructor from this crate's stateful
//! `DataTable::view`, and giving both the same name would leave a plugin
//! author unable to tell, from the name alone, which one a call reaches.
//! All five rows carry the same `_compound` suffix rather than three plain
//! names and two disambiguated ones, so a reader scanning the generated
//! Luau stubs sees one family instead of noticing the collision only on the
//! row that happened to need it.
//!
//! # Where `register_external` is called
//!
//! `gorgon_petra`'s merged table is a `OnceLock`: whichever of `lookup`,
//! `entries`, `build` or `expand` runs first freezes it, and a
//! `register_external` after that panics rather than silently dropping the
//! slice. This crate has no code that runs before another crate's `main` —
//! Rust has no static-constructor mechanism this workspace pulls in — so
//! the only honest place to call it is a function the embedder calls
//! itself, before touching the registry. That is what [`register`] is:
//! idempotent (`std::sync::Once`), so a caller unsure whether an earlier
//! call already happened may call it again for free. Today the only
//! caller is this module's own test suite below, which proves the whole
//! path — register, then look up by the wire name, then build a real
//! `ViewNode` from a hand-written params table — genuinely works end to
//! end. No production embedder calls it yet: the shell-side expansion path
//! (`registry/mod.rs`'s "Where expansion happens") and the kernel-hosting
//! fiber are later leaves' work, and wiring a call into, say, the gallery
//! binary now would be a call with nothing on the other end of it.

use std::sync::Once;

use serde::Deserialize;
use serde_json::Value;

use gorgon_petra::component::params::{ParamError, ParamShape};
use gorgon_petra::component::registry::{Entry, register_external};
use gorgon_petra::tree::ViewNode;

use crate::Compound;
use crate::calendar::{self, Calendar};
use crate::combobox::{self, Combobox};
use crate::command::{self, Command};
use crate::data_table::{self, DataTable};
use crate::selection_palette::{self, SelectionPalette};

fn fail(component: &'static str, e: impl std::fmt::Display) -> ParamError {
    ParamError {
        component: component.to_owned(),
        reason: e.to_string(),
    }
}

// ---------------------------------------------------------------------
// Combobox
// ---------------------------------------------------------------------

/// Wire mirror of [`combobox::Props`]. `Props` is not itself `Deserialize`
/// — only `Compound::State` carries that bound, because state is what a
/// reload round-trips and props is author-supplied — so the wire shape is
/// repeated here and converted, the same pattern
/// `gorgon_petra::component::registry::new_atomics` uses for every
/// constructor whose argument type is not already a wire shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComboboxPropsWire {
    label: String,
    items: Vec<String>,
}

impl From<ComboboxPropsWire> for combobox::Props {
    fn from(w: ComboboxPropsWire) -> Self {
        combobox::Props {
            label: w.label,
            items: w.items,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComboboxWire {
    props: ComboboxPropsWire,
    state: combobox::State,
}

impl ParamShape for ComboboxWire {
    const LUAU: &'static str = "{ props: { label: string, items: { string } }, state: \
         { query: string?, highlighted: number?, open: boolean? } }";
}

fn combobox_ctor(params: &Value) -> Result<ViewNode, ParamError> {
    let wire: ComboboxWire =
        serde_json::from_value(params.clone()).map_err(|e| fail("combobox_compound", e))?;
    Ok(Combobox::view(&wire.state, &wire.props.into()))
}

// ---------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandItemWire {
    id: String,
    label: String,
}

impl From<CommandItemWire> for command::Item {
    fn from(w: CommandItemWire) -> Self {
        command::Item::new(w.id, w.label)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandPropsWire {
    items: Vec<CommandItemWire>,
}

impl From<CommandPropsWire> for command::Props {
    fn from(w: CommandPropsWire) -> Self {
        command::Props {
            items: w.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandWire {
    props: CommandPropsWire,
    state: command::State,
}

impl ParamShape for CommandWire {
    const LUAU: &'static str = "{ props: { items: { { id: string, label: string } } }, state: \
         { query: string?, highlighted: number?, open: boolean? } }";
}

fn command_ctor(params: &Value) -> Result<ViewNode, ParamError> {
    let wire: CommandWire =
        serde_json::from_value(params.clone()).map_err(|e| fail("command_compound", e))?;
    Ok(Command::view(&wire.state, &wire.props.into()))
}

// ---------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------

/// `calendar::Mode` already derives `Serialize`/`Deserialize` (it round-trips
/// as part of `State`), so unlike the other four compounds this wire needs
/// no mirror for it — only the struct around it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalendarPropsWire {
    label: String,
    #[serde(default)]
    mode: calendar::Mode,
}

impl From<CalendarPropsWire> for calendar::Props {
    fn from(w: CalendarPropsWire) -> Self {
        calendar::Props {
            label: w.label,
            mode: w.mode,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalendarWire {
    props: CalendarPropsWire,
    state: calendar::State,
}

impl ParamShape for CalendarWire {
    const LUAU: &'static str = "{ props: { label: string, mode: (\"Single\" | \"Range\" | \
         \"Multi\")? }, state: { year: number?, month: number?, selected: { { year: number, \
         month: number, day: number } }?, range_anchor: { year: number, month: number, day: \
         number }?, mode: (\"Single\" | \"Range\" | \"Multi\")? } }";
}

fn calendar_ctor(params: &Value) -> Result<ViewNode, ParamError> {
    let wire: CalendarWire =
        serde_json::from_value(params.clone()).map_err(|e| fail("calendar_compound", e))?;
    Ok(Calendar::view(&wire.state, &wire.props.into()))
}

// ---------------------------------------------------------------------
// Data table
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableColumnWire {
    id: String,
    label: String,
}

impl From<DataTableColumnWire> for data_table::Column {
    fn from(w: DataTableColumnWire) -> Self {
        data_table::Column::new(w.id, w.label)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableRowWire {
    id: String,
    cells: Vec<String>,
    #[serde(default)]
    body: Option<String>,
}

impl From<DataTableRowWire> for data_table::Row {
    fn from(w: DataTableRowWire) -> Self {
        let row = data_table::Row::new(w.id, w.cells);
        match w.body {
            Some(body) => row.with_body(body),
            None => row,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTablePropsWire {
    columns: Vec<DataTableColumnWire>,
    rows: Vec<DataTableRowWire>,
}

impl From<DataTablePropsWire> for data_table::Props {
    fn from(w: DataTablePropsWire) -> Self {
        data_table::Props {
            columns: w.columns.into_iter().map(Into::into).collect(),
            rows: w.rows.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataTableWire {
    props: DataTablePropsWire,
    state: data_table::State,
}

impl ParamShape for DataTableWire {
    const LUAU: &'static str = "{ props: { columns: { { id: string, label: string } }, rows: \
         { { id: string, cells: { string }, body: string? } } }, state: { sort: { column: \
         string, ascending: boolean }?, selection: { string }?, expansion: { string }? } }";
}

fn data_table_ctor(params: &Value) -> Result<ViewNode, ParamError> {
    let wire: DataTableWire =
        serde_json::from_value(params.clone()).map_err(|e| fail("data_table_compound", e))?;
    Ok(DataTable::view(&wire.state, &wire.props.into()))
}

// ---------------------------------------------------------------------
// Selection palette
// ---------------------------------------------------------------------

/// `selection_palette::Props` is a unit struct — everything the palette
/// draws from lives in `State` — so its wire mirror carries no fields. It
/// still has to exist and still has to be named in the params table: a
/// row's contract is "props and state both come from the table", not
/// "props when there happen to be any."
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionPalettePropsWire {}

impl From<SelectionPalettePropsWire> for selection_palette::Props {
    fn from(_: SelectionPalettePropsWire) -> Self {
        selection_palette::Props
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionPaletteWire {
    props: SelectionPalettePropsWire,
    state: selection_palette::State,
}

impl ParamShape for SelectionPaletteWire {
    const LUAU: &'static str = "{ props: {}, state: { bold: boolean?, italic: boolean?, point: { x: number, y: \
         number }? } }";
}

fn selection_palette_ctor(params: &Value) -> Result<ViewNode, ParamError> {
    let wire: SelectionPaletteWire = serde_json::from_value(params.clone())
        .map_err(|e| fail("selection_palette_compound", e))?;
    Ok(SelectionPalette::view(&wire.state, &wire.props.into()))
}

// ---------------------------------------------------------------------
// The table itself
// ---------------------------------------------------------------------

const ENTRIES: &[Entry] = &[
    Entry {
        name: "combobox_compound",
        ctor: combobox_ctor,
        luau: ComboboxWire::LUAU,
    },
    Entry {
        name: "command_compound",
        ctor: command_ctor,
        luau: CommandWire::LUAU,
    },
    Entry {
        name: "calendar_compound",
        ctor: calendar_ctor,
        luau: CalendarWire::LUAU,
    },
    Entry {
        name: "data_table_compound",
        ctor: data_table_ctor,
        luau: DataTableWire::LUAU,
    },
    Entry {
        name: "selection_palette_compound",
        ctor: selection_palette_ctor,
        luau: SelectionPaletteWire::LUAU,
    },
];

/// Register every compound's view constructor with `gorgon_petra`'s
/// registry. Idempotent, so a caller unsure whether an earlier call already
/// happened may call this again for free.
///
/// # Panics
/// Only on the call that actually performs the registration, and only if
/// `gorgon_petra`'s registry table was already built by something else
/// touching `lookup`, `entries`, `build` or `expand` first — see the module
/// doc's "Where `register_external` is called".
pub fn register() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        register_external(ENTRIES);
    });
}

#[cfg(test)]
mod tests {
    use gorgon_petra::component::registry::{build, lookup};
    use gorgon_petra::tree::{Anchor, Role, ViewNode};
    use serde_json::json;

    use super::register;

    fn find<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|c| find(c, key))
    }

    #[test]
    fn all_five_compound_rows_are_registered_under_distinct_names() {
        register();
        for name in [
            "combobox_compound",
            "command_compound",
            "calendar_compound",
            "data_table_compound",
            "selection_palette_compound",
        ] {
            assert!(
                lookup(name).is_some(),
                "{name} missing from the merged table"
            );
        }
    }

    /// The whole point of a pure view row: an `open: true, highlighted: 1`
    /// state renders the open list with the second filtered row marked,
    /// with no call to `Combobox::init` anywhere in the path.
    #[test]
    fn combobox_row_renders_an_open_list_from_state_alone() {
        register();
        let params = json!({
            "props": { "label": "Theme", "items": ["alpha", "bravo", "alpine"] },
            "state": { "query": "al", "highlighted": 1, "open": true }
        });
        let node = build("combobox_compound", &params).expect("valid params must build");
        assert_eq!(node.semantics.expanded, Some(true));
        let menu = find(&node, "menu").expect("open state must mount the list");
        // Filtering "al" over [alpha, bravo, alpine] keeps [alpha, alpine]
        // (`filter_indices`'s own test proves the order and the match);
        // highlighted=1 is the second filtered row, alpine, key `opt-2`.
        let alpine = find(menu, "opt-2").expect("alpine must be in the filtered list");
        assert!(
            alpine.semantics.selected,
            "highlighted=1 must mark the second filtered row"
        );
        let bravo_absent = find(menu, "opt-1");
        assert!(bravo_absent.is_none(), "bravo must drop out of the filter");
    }

    #[test]
    fn combobox_row_closed_state_mounts_no_list() {
        register();
        let params = json!({
            "props": { "label": "Theme", "items": ["alpha"] },
            "state": { "query": "", "highlighted": 0, "open": false }
        });
        let node = build("combobox_compound", &params).expect("valid params must build");
        assert_eq!(node.semantics.expanded, Some(false));
        assert!(find(&node, "menu").is_none());
    }

    /// The picked day is marked on the grid straight from state, with no
    /// `Calendar::init` and no `Pick` intent replayed.
    #[test]
    fn calendar_row_marks_the_selected_day_from_state_alone() {
        register();
        let params = json!({
            "props": { "label": "When", "mode": "Multi" },
            "state": {
                "year": 2026, "month": 8,
                "selected": [{"year": 2026, "month": 8, "day": 30}],
                "range_anchor": null,
                "mode": "Multi"
            }
        });
        let node = build("calendar_compound", &params).expect("valid params must build");
        let day30 = find(&node, "day-30").expect("the picked day must be on the grid");
        assert!(
            day30.semantics.selected,
            "the state's own selection must mark the day without any Pick intent replayed"
        );
        let day29 = find(&node, "day-29").expect("day-29 must also be on the grid");
        assert!(!day29.semantics.selected, "only the picked day is marked");
    }

    /// A sort and a selection recorded straight in `state` show up in the
    /// built rows without any `SortBy` or `SelectRow` intent being replayed.
    #[test]
    fn data_table_row_sorts_and_selects_from_state_alone() {
        register();
        let params = json!({
            "props": {
                "columns": [{"id": "name", "label": "Name"}],
                "rows": [
                    {"id": "r0", "cells": ["bravo"]},
                    {"id": "r1", "cells": ["alpha"]}
                ]
            },
            "state": {
                "sort": {"column": "name", "ascending": true},
                "selection": ["r1"],
                "expansion": []
            }
        });
        let node = build("data_table_compound", &params).expect("valid params must build");
        let table = find(&node, "table").expect("root data table");
        // Row 0 is the header; body rows follow, sorted ascending by name:
        // alpha (r1) before bravo (r0).
        let row_keys: Vec<&str> = table
            .children
            .iter()
            .skip(1)
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(row_keys, vec!["r1", "r0"]);
    }

    /// The palette anchors at whatever point `state` names, closed to open
    /// exactly like the other two overlay compounds.
    #[test]
    fn command_row_is_a_frame_wide_overlay_only_while_open() {
        register();
        let items = json!([{"id": "open", "label": "Open file"}]);
        let closed = build(
            "command_compound",
            &json!({
                "props": { "items": items },
                "state": { "query": "", "highlighted": 0, "open": false }
            }),
        )
        .expect("valid params must build");
        assert_eq!(closed.semantics.expanded, Some(false));
        assert_ne!(closed.semantics.role, Some(Role::Overlay));

        let open = build(
            "command_compound",
            &json!({
                "props": { "items": items },
                "state": { "query": "", "highlighted": 0, "open": true }
            }),
        )
        .expect("valid params must build");
        assert_eq!(open.semantics.role, Some(Role::Overlay));
    }

    #[test]
    fn selection_palette_row_anchors_at_the_state_point() {
        register();
        let params = json!({
            "props": {},
            "state": { "bold": true, "italic": false, "point": { "x": 12.0, "y": 34.0 } }
        });
        let node = build("selection_palette_compound", &params).expect("valid params must build");
        assert_eq!(node.props.anchor, Some(Anchor::Point { x: 12.0, y: 34.0 }));
        let marks = find(&node, "marks").expect("the mark group");
        let bold = find(marks, "bold").expect("bold toggle");
        assert!(bold.semantics.selected);
    }

    #[test]
    fn an_unknown_component_name_is_refused() {
        register();
        assert!(build("no-such-compound", &json!({})).is_err());
    }

    /// A params table missing `state` is refused rather than silently
    /// rendering `Default::default()` — a row never calls `Compound::init`
    /// and it must not quietly stand in for it either.
    #[test]
    fn a_params_table_missing_state_is_refused() {
        register();
        let params = json!({ "props": { "label": "Theme", "items": ["alpha"] } });
        assert!(build("combobox_compound", &params).is_err());
    }
}
