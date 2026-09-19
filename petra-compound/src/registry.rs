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
//! call already happened may call it again for free.
//!
//! The callers, each as early as it can be in its own process:
//!
//! * `gorgon/gorgond/src/boot.rs` and `gorgon/inspector/src/main.rs` — the
//!   two production embedders.
//! * `gorgon/xtask/src/ui_stubs.rs`'s `component_shapes` — spec 014 A1.
//!   Without it the generator walks `entries()` before this slice arrives
//!   and `plugin.d.luau` names none of these five, which is what made them
//!   unreachable from Lua for as long as the hook existed.
//! * `petra/petra-egui/src/bin/gallery/lua_parity.rs` — spec 014 A3, so the
//!   parity harness can expand a page that names a compound.
//! * This module's own test suite below, which proves the whole path —
//!   register, look up by the wire name, build a real `ViewNode` from a
//!   hand-written params table — end to end.

use std::sync::Once;

use serde::Deserialize;
use serde_json::Value;

use gorgon_petra::component::params::{ParamError, ParamShape};
use gorgon_petra::component::registry::{Entry, IconMarkParam, register_external};
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
///
/// `pub`: `gorgond`'s `gorgon-view-fiber::combobox` row hosts the same
/// `combobox::Props` from a row's `config.props`, and needs the identical
/// wire shape to build it. Reusing this type rather than a second,
/// hand-rolled mirror in `gorgond` is what keeps the two parses from
/// drifting apart — see `gorgon/gorgond/src/compound.rs`'s module doc.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComboboxPropsWire {
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
struct CommandModeWire {
    /// One character; serde `char` refuses a longer string at parse time.
    key: char,
    label: String,
    /// The mode tile's glyph, if it has one. See [`CommandItemWire::icon`]
    /// for why this is a wire field at all.
    #[serde(default)]
    icon: Option<IconMarkParam>,
}

impl From<CommandModeWire> for command::Mode {
    fn from(w: CommandModeWire) -> Self {
        let mode = command::Mode::new(w.key, w.label);
        match w.icon {
            Some(mark) => mode.with_icon(mark.into()),
            None => mode,
        }
    }
}

/// Wire mirror of [`command::Item`].
///
/// `icon` used to be omitted here, on the reasoning that
/// [`gorgon_petra::component::IconMark`] is a large enum without
/// `Deserialize` and the wire could not name it without copying the whole
/// 30-name vocabulary. That reasoning was sound and its conclusion was
/// wrong: `gorgon_petra` had copied that vocabulary three times already
/// (`registry/navigation.rs`, `registry/new_atomics.rs`,
/// `registry/atoms.rs`, each saying extraction was a later leaf's job), so
/// the cost of a fourth copy was the thing to remove, not the field. Spec
/// 014 extracted the mirror to
/// [`gorgon_petra::component::registry::IconMarkParam`] and this row uses
/// it.
///
/// Dropping the field was not free: `Command::view` mounts a glyph child
/// per item that carries one (`command.rs`'s `leading.push(icon_toned(…))`),
/// so a Lua author naming `command_compound` could not reproduce a palette
/// any Rust caller can build, and the gallery's own Command page — which
/// gives all five of its items icons — had no expressible Lua form at all.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandItemWire {
    id: String,
    label: String,
    #[serde(default)]
    shortcut: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    icon: Option<IconMarkParam>,
}

impl From<CommandItemWire> for command::Item {
    fn from(w: CommandItemWire) -> Self {
        command::Item {
            categories: w.categories,
            shortcut: w.shortcut,
            icon: w.icon.map(Into::into),
            ..command::Item::new(w.id, w.label)
        }
    }
}

fn default_list_columns() -> u8 {
    1
}

fn default_tile_columns() -> u8 {
    4
}

/// `pub` for the same reason as [`ComboboxPropsWire`]: `gorgond`'s
/// `gorgon-view-fiber::command` row parses `command::Props` from a row's
/// `config.props` through this exact wire shape.
///
/// `mode_surface` is not a wire field. It is a `ViewNode` the host passes
/// in Rust only; a YAML/Lua row that names it is refused
/// (`deny_unknown_fields`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandPropsWire {
    items: Vec<CommandItemWire>,
    #[serde(default = "default_list_columns")]
    list_columns: u8,
    #[serde(default = "default_tile_columns")]
    tile_columns: u8,
    #[serde(default)]
    default_view: command::ViewKind,
    #[serde(default)]
    modes: Vec<CommandModeWire>,
}

impl From<CommandPropsWire> for command::Props {
    fn from(w: CommandPropsWire) -> Self {
        command::Props {
            items: w.items.into_iter().map(Into::into).collect(),
            list_columns: w.list_columns,
            tile_columns: w.tile_columns,
            default_view: w.default_view,
            modes: w.modes.into_iter().map(Into::into).collect(),
            mode_surface: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct CommandStateWire {
    query: String,
    highlighted: usize,
    open: bool,
    view: command::ViewKind,
    category: Option<String>,
    favorites: Vec<String>,
}

impl From<CommandStateWire> for command::State {
    fn from(w: CommandStateWire) -> Self {
        command::State {
            query: w.query,
            highlighted: w.highlighted,
            open: w.open,
            view: w.view,
            category: w.category,
            favorites: w.favorites.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandWire {
    props: CommandPropsWire,
    state: CommandStateWire,
}

impl ParamShape for CommandWire {
    const LUAU: &'static str = concat!(
        "{ props: { items: { { id: string, label: string, shortcut: string?, categories: \
         { string }?, icon: ",
        gorgon_petra::icon_mark_luau!(),
        "? } }, list_columns: number?, tile_columns: number?, default_view: (\"list\" | \
         \"tiles\")?, modes: { { key: string, label: string, icon: ",
        gorgon_petra::icon_mark_luau!(),
        "? } }? }, state: { query: string?, highlighted: number?, open: boolean?, view: \
         (\"list\" | \"tiles\")?, category: string?, favorites: { string }? } }"
    );
}

fn command_ctor(params: &Value) -> Result<ViewNode, ParamError> {
    let wire: CommandWire =
        serde_json::from_value(params.clone()).map_err(|e| fail("command_compound", e))?;
    Ok(Command::view(&wire.state.into(), &wire.props.into()))
}

// ---------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------

/// `calendar::Mode` already derives `Serialize`/`Deserialize` (it round-trips
/// as part of `State`), so unlike the other four compounds this wire needs
/// no mirror for it — only the struct around it.
///
/// `pub` for the same reason as [`ComboboxPropsWire`]: `gorgond`'s
/// `gorgon-view-fiber::calendar` row parses `calendar::Props` from a row's
/// `config.props` through this exact wire shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarPropsWire {
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
struct DataTableActionWire {
    id: String,
    label: String,
}

impl From<DataTableActionWire> for data_table::ActionItem {
    fn from(w: DataTableActionWire) -> Self {
        data_table::ActionItem::new(w.id, w.label)
    }
}

/// `pub` for the same reason as [`ComboboxPropsWire`]: `gorgond`'s
/// `gorgon-view-fiber::data-table` row parses `data_table::Props` from a
/// row's `config.props` through this exact wire shape.
///
/// `batch_actions`/`row_actions`/`loading` are `#[serde(default)]` (T034):
/// a row written before the toolbar tier existed still parses — an empty
/// batch/row-action list and `loading: false` is the pre-T034 behaviour
/// exactly, not a guess at one.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataTablePropsWire {
    columns: Vec<DataTableColumnWire>,
    rows: Vec<DataTableRowWire>,
    #[serde(default)]
    batch_actions: Vec<DataTableActionWire>,
    #[serde(default)]
    row_actions: Vec<DataTableActionWire>,
    #[serde(default)]
    loading: bool,
}

impl From<DataTablePropsWire> for data_table::Props {
    fn from(w: DataTablePropsWire) -> Self {
        data_table::Props {
            columns: w.columns.into_iter().map(Into::into).collect(),
            rows: w.rows.into_iter().map(Into::into).collect(),
            batch_actions: w.batch_actions.into_iter().map(Into::into).collect(),
            row_actions: w.row_actions.into_iter().map(Into::into).collect(),
            loading: w.loading,
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
         { { id: string, cells: { string }, body: string? } }, batch_actions: { { id: string, \
         label: string } }?, row_actions: { { id: string, label: string } }?, loading: boolean? \
         }, state: { sort: { column: string, ascending: boolean }?, selection: { string }?, \
         expansion: { string }?, query: string?, hidden_columns: { string }?, column_menu_open: \
         boolean?, row_menu_open: string?, weights: { number }? } }";
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
///
/// `pub` for the same reason as [`ComboboxPropsWire`]: `gorgond`'s
/// `gorgon-view-fiber::selection-palette` row parses
/// `selection_palette::Props` from a row's `config.props` through this
/// exact (empty) wire shape.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionPalettePropsWire {}

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

    /// The pre-modes fixture `{ items: [{id, label}] }` is still the
    /// documented YAML/Lua shape. Omitted columns, view, modes, category
    /// and favorites take the wire defaults rather than failing parse.
    #[test]
    fn command_props_wire_old_fixture_gets_the_documented_defaults() {
        let wire: super::CommandPropsWire = serde_json::from_value(json!({
            "items": [{"id": "open", "label": "Open"}]
        }))
        .expect("items-only props must still parse");
        assert_eq!(wire.list_columns, 1);
        assert_eq!(wire.tile_columns, 4);
        assert_eq!(wire.default_view, crate::command::ViewKind::List);
        assert!(wire.modes.is_empty());
        let state: super::CommandStateWire = serde_json::from_value(json!({
            "query": "",
            "highlighted": 0,
            "open": false
        }))
        .expect("query/highlighted/open state must still parse");
        assert_eq!(state.view, crate::command::ViewKind::List);
        assert_eq!(state.category, None);
        assert!(state.favorites.is_empty());
    }

    #[test]
    fn command_row_accepts_categories_shortcut_modes_and_view() {
        register();
        let node = build(
            "command_compound",
            &json!({
                "props": {
                    "items": [{
                        "id": "open",
                        "label": "Open",
                        "shortcut": "Ctrl+O",
                        "categories": ["File"]
                    }],
                    "list_columns": 2,
                    "tile_columns": 6,
                    "default_view": "tiles",
                    "modes": [{"key": "f", "label": "Files"}]
                },
                "state": {
                    "query": "",
                    "highlighted": 0,
                    "open": true,
                    "view": "list",
                    "category": "File",
                    "favorites": ["open"]
                }
            }),
        )
        .expect("full command wire must build");
        assert_eq!(node.semantics.role, Some(Role::Overlay));
    }

    /// Hosts pass `mode_surface` in Rust only. Naming it on the YAML/Lua
    /// wire is a misspelling, not a slot.
    #[test]
    fn command_row_refuses_mode_surface_on_props() {
        register();
        let err = build(
            "command_compound",
            &json!({
                "props": {
                    "items": [{"id": "open", "label": "Open"}],
                    "mode_surface": {}
                },
                "state": { "query": "", "highlighted": 0, "open": false }
            }),
        )
        .expect_err("mode_surface is not a wire field");
        assert!(
            err.reason.contains("mode_surface"),
            "refusal must name the unknown field: {}",
            err.reason
        );
    }

    /// **Icons cross the wire**, which they did not until spec 014. The
    /// glyph `Command::view` mounts for an item that carries one is a child
    /// of that item's row, so a wire that drops `icon` cannot build the
    /// palette a Rust caller builds — and the gallery's own Command page
    /// gives all five of its items an icon.
    ///
    /// The assertion is against `Command::view` called directly with the
    /// same props, not against a hand-counted child list: one
    /// implementation on both sides, so this fails exactly when the wire
    /// stops carrying what `Props` carries.
    ///
    /// Falsified 2026-09-19 by dropping `icon: w.icon.map(Into::into)` from
    /// `From<CommandItemWire> for command::Item`, which is what this file
    /// shipped before:
    ///
    /// ```text
    /// thread 'registry::tests::command_row_carries_item_and_mode_icons'
    /// panicked at petra/petra-compound/src/registry.rs:823:9:
    /// assertion `left == right` failed: the wire must build the same palette
    /// Command::view builds from the same props
    ///   left: ViewNode { kind: Surface, key: Key("command"), ...
    /// ```
    ///
    /// — two whole trees, the way `ViewNode`'s `Debug` always reports a
    /// mismatch; the difference is the `icon` child under the `rebuild` row
    /// and under the `mode-f` tile.
    ///
    /// Restored byte-identical afterwards and re-run green.
    #[test]
    fn command_row_carries_item_and_mode_icons() {
        use crate::Compound;
        use crate::command::{Command, Item, Mode, Props, State, ViewKind};
        use gorgon_petra::component::IconMark;

        register();
        let params = json!({
            "props": {
                "items": [{
                    "id": "rebuild",
                    "label": "Rebuild fiber",
                    "shortcut": "Ctrl+R",
                    "categories": ["Edit"],
                    "icon": "menu"
                }],
                "modes": [{ "key": "f", "label": "Files", "icon": "search" }]
            },
            "state": { "query": "", "highlighted": 0, "open": true }
        });
        let built = build("command_compound", &params).expect("icons must parse");

        let props = Props {
            items: vec![
                Item::new("rebuild", "Rebuild fiber")
                    .with_icon(IconMark::Menu)
                    .with_shortcut("Ctrl+R")
                    .with_categories(["Edit"]),
            ],
            list_columns: 1,
            tile_columns: 4,
            default_view: ViewKind::List,
            modes: vec![Mode::new('f', "Files").with_icon(IconMark::Search)],
            mode_surface: None,
        };
        let state = State {
            open: true,
            ..State::default()
        };
        assert_eq!(
            built,
            Command::view(&state, &props),
            "the wire must build the same palette Command::view builds from the same props"
        );
    }

    /// An icon name outside `IconMark`'s closed vocabulary is refused by
    /// name rather than silently dropped, which is the whole reason the wire
    /// mirrors the enum instead of taking a free string.
    #[test]
    fn command_row_refuses_an_icon_name_that_does_not_exist() {
        register();
        let err = build(
            "command_compound",
            &json!({
                "props": { "items": [{ "id": "a", "label": "A", "icon": "sparkles" }] },
                "state": { "query": "", "highlighted": 0, "open": false }
            }),
        )
        .expect_err("`sparkles` is not an IconMark");
        assert!(
            err.reason.contains("sparkles"),
            "the refusal must name the icon the author wrote: {}",
            err.reason
        );
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
