//! Registry rows: spec 013's 23 previously-unregistered constructors.
//!
//! `button.rs`, `button_group.rs`, `text.rs`, `section.rs`, `icon.rs`,
//! `drawer.rs`, and `list_row.rs` hold 25 `pub fn ... -> ViewNode`
//! constructors with no registry row (`specs/013-lua-gallery-parity/spec.md`).
//! 23 ship here. `text` and `button` are withheld: `install`
//! (`gorgon/kernel-lua/src/ui/mod.rs:130`) merges every `MODULES` table into
//! `builders.lua`'s own and errors if a name collides, and `builders.lua`
//! already exports `text` and `button` as primitives. A `component::text` or
//! `component::button` row cannot be registered under its own name until the
//! operator picks one of the four ways out spec 013's T001 records. That
//! ruling is not this task's to make, so those two names stay out of
//! [`ENTRIES`] and out of the coverage test's required set below.
//!
//! A new file rather than an extension of [`super::new_atomics`], because
//! that file's own module doc scopes it to "wave-1 atomics that are not in
//! the original 42" — six named files, not these seven. Registered through
//! [`super::register_external`] from `seed_first_party`, the same hook
//! `new_atomics::ENTRIES` uses.

use serde::Deserialize;
use serde_json::Value;

use super::Entry;
use crate::component as lib;
use crate::component::params::{
    KeyChildren, KeyLabel, KeyLabelChildren, KeyLabelSelected, ParamError, ParamShape,
};
use crate::component::{IconBox, IconMark, IconTone};
use crate::tree::{Edge, InputPolicy, Key, ViewNode};

fn fail(component: &'static str, e: impl std::fmt::Display) -> ParamError {
    ParamError {
        component: component.to_owned(),
        reason: e.to_string(),
    }
}

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
// Wire IconMark. Copied from `registry/navigation.rs` and
// `registry/new_atomics.rs`: group files do not import siblings, and this
// file follows the same rule. Extracting one shared wire enum is a later
// simplification, not this leaf — the third file to say so.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum IconMarkParam {
    Check,
    Calendar,
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    Close,
    Copy,
    Add,
    Subtract,
    Search,
    Menu,
    Notification,
    Switcher,
    CaretLeft,
    CaretRight,
    CheckmarkOutline,
    CircleDash,
    Incomplete,
    Checkmark,
    ErrorFilled,
    WarningFilled,
    InformationFilled,
    CheckmarkFilled,
    CaretDown,
    Edit,
    BulletDisc,
    BulletCircle,
    BulletSquare,
    BulletDash,
}

impl From<IconMarkParam> for IconMark {
    fn from(m: IconMarkParam) -> Self {
        match m {
            IconMarkParam::Check => IconMark::Check,
            IconMarkParam::Calendar => IconMark::Calendar,
            IconMarkParam::ChevronDown => IconMark::ChevronDown,
            IconMarkParam::ChevronUp => IconMark::ChevronUp,
            IconMarkParam::ChevronLeft => IconMark::ChevronLeft,
            IconMarkParam::ChevronRight => IconMark::ChevronRight,
            IconMarkParam::Close => IconMark::Close,
            IconMarkParam::Copy => IconMark::Copy,
            IconMarkParam::Add => IconMark::Add,
            IconMarkParam::Subtract => IconMark::Subtract,
            IconMarkParam::Search => IconMark::Search,
            IconMarkParam::Menu => IconMark::Menu,
            IconMarkParam::Notification => IconMark::Notification,
            IconMarkParam::Switcher => IconMark::Switcher,
            IconMarkParam::CaretLeft => IconMark::CaretLeft,
            IconMarkParam::CaretRight => IconMark::CaretRight,
            IconMarkParam::CheckmarkOutline => IconMark::CheckmarkOutline,
            IconMarkParam::CircleDash => IconMark::CircleDash,
            IconMarkParam::Incomplete => IconMark::Incomplete,
            IconMarkParam::Checkmark => IconMark::Checkmark,
            IconMarkParam::ErrorFilled => IconMark::ErrorFilled,
            IconMarkParam::WarningFilled => IconMark::WarningFilled,
            IconMarkParam::InformationFilled => IconMark::InformationFilled,
            IconMarkParam::CheckmarkFilled => IconMark::CheckmarkFilled,
            IconMarkParam::CaretDown => IconMark::CaretDown,
            IconMarkParam::Edit => IconMark::Edit,
            IconMarkParam::BulletDisc => IconMark::BulletDisc,
            IconMarkParam::BulletCircle => IconMark::BulletCircle,
            IconMarkParam::BulletSquare => IconMark::BulletSquare,
            IconMarkParam::BulletDash => IconMark::BulletDash,
        }
    }
}

macro_rules! icon_mark_luau {
    () => {
        "\"check\" | \"calendar\" | \"chevron-down\" | \"chevron-up\" | \"chevron-left\" | \
         \"chevron-right\" | \"close\" | \"copy\" | \"add\" | \"subtract\" | \"search\" | \
         \"menu\" | \"notification\" | \"switcher\" | \"caret-left\" | \"caret-right\" | \
         \"checkmark-outline\" | \"circle-dash\" | \"incomplete\" | \"checkmark\" | \
         \"error-filled\" | \"warning-filled\" | \"information-filled\" | \"checkmark-filled\" | \
         \"caret-down\" | \"edit\" | \"bullet-disc\" | \"bullet-circle\" | \"bullet-square\" | \
         \"bullet-dash\""
    };
}

// ---------------------------------------------------------------------
// Wire IconTone and IconBox. Neither derives `Deserialize` on the shipped
// enum (`icon.rs`'s own three-name closed vocabulary is deliberate, see its
// module doc), so these are wire copies the same way `IconMarkParam` above
// is — local to this file, not shared.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum IconToneParam {
    OnAccent,
    Primary,
    Secondary,
    Disabled,
    Accent,
    Danger,
}

impl From<IconToneParam> for IconTone {
    fn from(t: IconToneParam) -> Self {
        match t {
            IconToneParam::OnAccent => IconTone::OnAccent,
            IconToneParam::Primary => IconTone::Primary,
            IconToneParam::Secondary => IconTone::Secondary,
            IconToneParam::Disabled => IconTone::Disabled,
            IconToneParam::Accent => IconTone::Accent,
            IconToneParam::Danger => IconTone::Danger,
        }
    }
}

macro_rules! icon_tone_luau {
    () => {
        "\"on-accent\" | \"primary\" | \"secondary\" | \"disabled\" | \"accent\" | \"danger\""
    };
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum IconBoxParam {
    Glyph,
    Header,
}

impl From<IconBoxParam> for IconBox {
    fn from(b: IconBoxParam) -> Self {
        match b {
            IconBoxParam::Glyph => IconBox::Glyph,
            IconBoxParam::Header => IconBox::Header,
        }
    }
}

macro_rules! icon_box_luau {
    () => {
        "\"glyph\" | \"header\""
    };
}

// ---------------------------------------------------------------------
// One-off shapes. Field names match the Rust constructors. `Edge` and
// `InputPolicy` already derive `Deserialize` (`crate::tree::props`, both
// `#[serde(rename_all = "kebab-case")]` or `"lowercase"`), the same fact
// `registry/containment.rs`'s `popover_with_placement` and
// `registry/navigation.rs`'s `menubar` rely on, so those two are used
// directly here rather than wrapped.
// ---------------------------------------------------------------------

/// `button_group_flush(key, height, children)`. `KeyChildren` plus the one
/// `height: f32` the flush form needs to compute its squared seams.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ButtonGroupFlushParams {
    key: Key,
    height: f32,
    /// `#[serde(default)]`: an empty Lua table for an empty child list hits
    /// the same empty-list-vs-empty-map ambiguity `KeyChildren` documents.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for ButtonGroupFlushParams {
    const LUAU: &'static str = "{ key: string, height: number, children: { ViewNode } }";
}

/// `icon(key, mark)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct IconParams {
    key: Key,
    mark: IconMarkParam,
}
impl ParamShape for IconParams {
    const LUAU: &'static str = concat!("{ key: string, mark: ", icon_mark_luau!(), " }");
}

/// `icon_toned(key, mark, tone)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct IconTonedParams {
    key: Key,
    mark: IconMarkParam,
    tone: IconToneParam,
}
impl ParamShape for IconTonedParams {
    const LUAU: &'static str = concat!(
        "{ key: string, mark: ",
        icon_mark_luau!(),
        ", tone: ",
        icon_tone_luau!(),
        " }"
    );
}

/// `icon_in(key, mark, boxed, tone)`. `boxed`, not `box`: `box` is a
/// reserved Rust keyword, and the Rust constructor's own parameter is
/// already spelled `boxed` (`icon.rs`), so the wire name matches it rather
/// than inventing a third spelling.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct IconInParams {
    key: Key,
    mark: IconMarkParam,
    boxed: IconBoxParam,
    tone: IconToneParam,
}
impl ParamShape for IconInParams {
    const LUAU: &'static str = concat!(
        "{ key: string, mark: ",
        icon_mark_luau!(),
        ", boxed: ",
        icon_box_luau!(),
        ", tone: ",
        icon_tone_luau!(),
        " }"
    );
}

/// `drawer(key, edge, open, body)` / `sheet(key, edge, open, body)`. Same
/// four fields build both — the two names pick a policy internally
/// (`drawer.rs`'s own module doc) — so one shape serves both rows.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DrawerSheetParams {
    key: Key,
    edge: Edge,
    open: bool,
    body: ViewNode,
}
impl ParamShape for DrawerSheetParams {
    const LUAU: &'static str = "{ key: string, \
         edge: \"top\" | \"bottom\" | \"left\" | \"right\", open: boolean, body: ViewNode }";
}

/// `docked(key, edge, open, body, policy)`: [`DrawerSheetParams`] plus the
/// [`InputPolicy`] `drawer`/`sheet` each pick for their caller.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DockedParams {
    key: Key,
    edge: Edge,
    open: bool,
    body: ViewNode,
    policy: InputPolicy,
}
impl ParamShape for DockedParams {
    const LUAU: &'static str = "{ key: string, \
         edge: \"top\" | \"bottom\" | \"left\" | \"right\", open: boolean, body: ViewNode, \
         policy: \"block\" | \"passthrough\" | \"dismiss-outside\" }";
}

/// `list_row_with(key, label, selected, icon?, shortcut?)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListRowWithParams {
    key: Key,
    label: String,
    selected: bool,
    #[serde(default)]
    icon: Option<IconMarkParam>,
    #[serde(default)]
    shortcut: Option<String>,
}
impl ParamShape for ListRowWithParams {
    const LUAU: &'static str = concat!(
        "{ key: string, label: string, selected: boolean, icon: ",
        icon_mark_luau!(),
        "?, shortcut: string? }"
    );
}

// ---------------------------------------------------------------------
// This group's constructors. 23 of the 25 spec 013 counts across these
// seven files; `text` and `button` are withheld, see the module doc.
// ---------------------------------------------------------------------
pub const ENTRIES: &[Entry] = &[
    // -- button.rs (11 of 12; `button` itself is T001) -------------------
    row!("button_xs", KeyLabel, |p| lib::button_xs(p.key, p.label)),
    row!("button_sm", KeyLabel, |p| lib::button_sm(p.key, p.label)),
    row!("button_lg", KeyLabel, |p| lib::button_lg(p.key, p.label)),
    row!("button_xl", KeyLabel, |p| lib::button_xl(p.key, p.label)),
    row!("button_2xl", KeyLabel, |p| lib::button_2xl(p.key, p.label)),
    row!("primary_button", KeyLabel, |p| lib::primary_button(
        p.key, p.label
    )),
    row!("tertiary_button", KeyLabel, |p| lib::tertiary_button(
        p.key, p.label
    )),
    row!("ghost_button", KeyLabel, |p| lib::ghost_button(
        p.key, p.label
    )),
    row!("danger_button", KeyLabel, |p| lib::danger_button(
        p.key, p.label
    )),
    row!("danger_tertiary_button", KeyLabel, |p| {
        lib::danger_tertiary_button(p.key, p.label)
    }),
    row!("danger_ghost_button", KeyLabel, |p| {
        lib::danger_ghost_button(p.key, p.label)
    }),
    // -- button_group.rs ---------------------------------------------------
    row!("button_group", KeyChildren, |p| lib::button_group(
        p.key, p.children
    )),
    row!("button_group_flush", ButtonGroupFlushParams, |p| {
        lib::button_group_flush(p.key, p.height, p.children)
    }),
    // -- text.rs (1 of 2; `text` itself is T001) --------------------------
    row!("heading", KeyLabel, |p| lib::heading(p.key, p.label)),
    // -- section.rs ---------------------------------------------------------
    row!("section", KeyLabelChildren, |p| lib::section(
        p.key, p.label, p.children
    )),
    // -- icon.rs --------------------------------------------------------------
    row!("icon", IconParams, |p| lib::icon(p.key, p.mark.into())),
    row!("icon_toned", IconTonedParams, |p| lib::icon_toned(
        p.key,
        p.mark.into(),
        p.tone.into()
    )),
    row!("icon_in", IconInParams, |p| lib::icon_in(
        p.key,
        p.mark.into(),
        p.boxed.into(),
        p.tone.into()
    )),
    // -- drawer.rs --------------------------------------------------------------
    row!("drawer", DrawerSheetParams, |p| lib::drawer(
        p.key, p.edge, p.open, p.body
    )),
    row!("sheet", DrawerSheetParams, |p| lib::sheet(
        p.key, p.edge, p.open, p.body
    )),
    row!("docked", DockedParams, |p| lib::docked(
        p.key, p.edge, p.open, p.body, p.policy
    )),
    // -- list_row.rs --------------------------------------------------------------
    row!("list_row", KeyLabelSelected, |p| lib::list_row(
        p.key, p.label, p.selected
    )),
    row!("list_row_with", ListRowWithParams, |p| {
        lib::list_row_with(
            p.key,
            p.label,
            p.selected,
            p.icon.map(Into::into),
            p.shortcut.as_deref(),
        )
    }),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;

    use super::{
        ButtonGroupFlushParams, DockedParams, DrawerSheetParams, ENTRIES, IconInParams, IconParams,
        IconTonedParams, ListRowWithParams,
    };

    /// Every `pub fn ... -> ViewNode` in the seven files this group owns has
    /// exactly one row, found by scanning the files' own text rather than
    /// trusting a hand-written list — a constructor added later and never
    /// registered fails this the moment it lands, not at review time.
    ///
    /// The heuristic matches `registry/new_atomics.rs`: no signature in
    /// these files has a `{` before the one that opens its own body, so the
    /// text between `pub fn ` and the next `{` is the whole signature, and
    /// it names a `ViewNode` constructor exactly when that text contains
    /// `-> ViewNode`.
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

    /// **Falsified** by removing the `icon` row and re-running. Real panic:
    /// "these `pub fn ... -> ViewNode` constructors have no registry row:
    /// \[\"icon (icon.rs)\"\]", then the row was restored byte-identical.
    #[test]
    fn every_view_node_constructor_has_exactly_one_row() {
        let sources: &[(&str, &str)] = &[
            ("button.rs", include_str!("../button.rs")),
            ("button_group.rs", include_str!("../button_group.rs")),
            ("text.rs", include_str!("../text.rs")),
            ("section.rs", include_str!("../section.rs")),
            ("icon.rs", include_str!("../icon.rs")),
            ("drawer.rs", include_str!("../drawer.rs")),
            ("list_row.rs", include_str!("../list_row.rs")),
        ];
        let registered: BTreeSet<&str> = ENTRIES.iter().map(|e| e.name).collect();
        assert_eq!(
            registered.len(),
            ENTRIES.len(),
            "two rows in this file share one name"
        );
        let mut missing = Vec::new();
        let mut source_names: BTreeSet<String> = BTreeSet::new();
        for (file, source) in sources {
            for name in public_view_node_constructors(source) {
                // `text` and `button` are blocked on the operator ruling
                // spec 013's T001 (`specs/013-lua-gallery-parity/tasks.md`)
                // records: `install` (`gorgon/kernel-lua/src/ui/mod.rs:130`)
                // merges every `MODULES` table into `builders.lua`'s own and
                // errors on a name collision, and `builders.lua` already
                // exports both names as primitives. A `component::text` or
                // `component::button` row cannot be registered under its own
                // name until the operator picks one of T001's four ways out,
                // so both are excluded here rather than worked around.
                if name == "text" || name == "button" {
                    source_names.insert(name);
                    continue;
                }
                if !registered.contains(name.as_str()) {
                    missing.push(format!("{name} ({file})"));
                }
                source_names.insert(name);
            }
        }
        assert!(
            missing.is_empty(),
            "these `pub fn ... -> ViewNode` constructors have no registry row: {missing:?}"
        );
        let extra: Vec<&str> = registered
            .iter()
            .filter(|name| !source_names.contains(**name))
            .copied()
            .collect();
        assert!(
            extra.is_empty(),
            "these rows name no `pub fn ... -> ViewNode` in the seven files this group owns: {extra:?}"
        );
    }

    /// T002: every new shape this file adds deserializes the wire shape its
    /// own `ParamShape::LUAU` describes. Not exhaustive over every field
    /// combination — `registry/mod.rs`'s
    /// `every_rendered_field_name_is_one_the_deserializer_accepts` already
    /// probes every registered shape's field names against its Luau
    /// rendering — this is the narrower claim that a realistic wire table
    /// for each of the six shapes `KeyLabel`/`KeyChildren`/`KeyLabelChildren`/
    /// `KeyLabelSelected` do not already cover actually parses.
    ///
    /// **Falsified** by misspelling `ButtonGroupFlushParams`'s probe field
    /// as `height_wrong_field`. Real panic: "ButtonGroupFlushParams
    /// round-trips: Error(\"unknown field `height_wrong_field`, expected
    /// one of `key`, `height`, `children`\", line: 0, column: 0)", then the
    /// probe was restored byte-identical.
    #[test]
    fn the_six_new_shapes_round_trip_through_serde_json_from_value() {
        serde_json::from_value::<ButtonGroupFlushParams>(json!({
            "key": "row", "height": 40.0, "children": []
        }))
        .expect("ButtonGroupFlushParams round-trips");

        serde_json::from_value::<IconParams>(json!({ "key": "i", "mark": "check" }))
            .expect("IconParams round-trips");

        serde_json::from_value::<IconTonedParams>(json!({
            "key": "i", "mark": "check", "tone": "primary"
        }))
        .expect("IconTonedParams round-trips");

        serde_json::from_value::<IconInParams>(json!({
            "key": "i", "mark": "check", "boxed": "glyph", "tone": "on-accent"
        }))
        .expect("IconInParams round-trips");

        serde_json::from_value::<DrawerSheetParams>(json!({
            "key": "d", "edge": "left", "open": true,
            "body": {"kind": "stack", "key": "b"},
        }))
        .expect("DrawerSheetParams round-trips");

        serde_json::from_value::<DockedParams>(json!({
            "key": "d", "edge": "left", "open": true,
            "body": {"kind": "stack", "key": "b"}, "policy": "block",
        }))
        .expect("DockedParams round-trips");

        serde_json::from_value::<ListRowWithParams>(json!({
            "key": "r", "label": "Row", "selected": false,
            "icon": "menu", "shortcut": "Ctrl+R",
        }))
        .expect("ListRowWithParams round-trips");
    }
}
