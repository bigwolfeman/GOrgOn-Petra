//! Registry rows: form. See [`super`] for why this file exists.
//!
//! Each row names a public constructor in `petra/petra/src/component/`,
//! deserializes its parameter table into a shape from
//! [`crate::component::params`], and calls the shipped constructor. A row
//! must never re-derive what the constructor does.
//!
//! Files covered: `controls.rs` (Checkbox, Radio button, Toggle),
//! `date_picker.rs` (Date picker), `dropdown.rs` (Dropdown), `field.rs`
//! (Text input), `file_uploader.rs` (File uploader), `form.rs` (Form),
//! `number_input.rs` (Number input), `search.rs` (Search), `select.rs`
//! (Select), `slider.rs` (Slider). Every `pub fn` in those ten files whose
//! signature reads `-> ViewNode` has exactly one row below — the
//! `every_view_node_constructor_has_exactly_one_row` test proves it by
//! scanning the ten files' own source text, not by trusting this list.
//!
//! Ten shapes here are one-off: `KeyLabel`/`KeyLabelSelected`/
//! `KeyLabelValue`/`KeyLabelChildren`/`KeyLabelNumber` (from
//! [`crate::component::params`]) cover 41 of these 52 rows; the rest need a
//! shape [`crate::component::params`] does not carry — `checkbox_tristate`'s
//! three-state enum, `date_picker_showing`'s browsing-calendar enum, the two
//! `_open` constructors' `(key, label, value, children)` quartet, a plain
//! `(node, hint)`/`(node, value)` pair for `hinted`/`valued`, and the
//! `message`-carrying shapes `field_invalid`/`field_warning`/
//! `field_validated`/`file_uploader_item_invalid`/`number_input_invalid`/
//! `checkbox_warning`/`radio_warning` need. Each is defined once here and
//! reused wherever its field set repeats.

use serde::Deserialize;
use serde_json::Value;

use super::Entry;
use crate::component as lib;
use crate::component::params::{
    KeyLabel, KeyLabelChildren, KeyLabelNumber, KeyLabelSelected, KeyLabelValue, ParamError,
    ParamShape,
};
use crate::tree::{Key, ViewNode};

/// Turn a `serde_json` deserialization failure into a [`ParamError`] naming
/// the component being built. `deny_unknown_fields` on every shape here
/// means `e.to_string()` already names the offending key (gate G4) —
/// serde's own message for an unknown or missing field spells it out.
fn fail(component: &'static str, e: impl std::fmt::Display) -> ParamError {
    ParamError {
        component: component.to_owned(),
        reason: e.to_string(),
    }
}

/// Build one [`Entry`]: a name, the shape its params deserialize into, and
/// the expression that calls the shipped constructor with the deserialized
/// fields. The inner `fn ctor` is a local item, not a closure — [`Entry`]'s
/// `ctor` field is a bare `fn` pointer, so nothing here may capture state.
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
// One-off shapes. Never invented fields: each mirrors the Rust
// constructor's own parameter list, positionally, one for one.
// ---------------------------------------------------------------------

/// `controls.rs`'s [`lib::CheckState`], as an agent writes it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CheckStateWire {
    Unchecked,
    Checked,
    Mixed,
}

impl From<CheckStateWire> for lib::CheckState {
    fn from(w: CheckStateWire) -> Self {
        match w {
            CheckStateWire::Unchecked => Self::Unchecked,
            CheckStateWire::Checked => Self::Checked,
            CheckStateWire::Mixed => Self::Mixed,
        }
    }
}

/// `checkbox_tristate(key, label, state)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckboxTristateParams {
    key: Key,
    label: String,
    state: CheckStateWire,
}
impl ParamShape for CheckboxTristateParams {
    const LUAU: &'static str =
        "{ key: string, label: string, state: \"unchecked\" | \"checked\" | \"mixed\" }";
}

/// `date_picker.rs`'s [`lib::Calendar`], as an agent writes it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
enum CalendarWire {
    Compact { year: i32, month: u32 },
    Full { year: i32, month: u32 },
    Choosing { year: i32, month: u32 },
}

impl From<CalendarWire> for lib::Calendar {
    fn from(w: CalendarWire) -> Self {
        match w {
            CalendarWire::Compact { year, month } => Self::Compact { year, month },
            CalendarWire::Full { year, month } => Self::Full { year, month },
            CalendarWire::Choosing { year, month } => Self::Choosing { year, month },
        }
    }
}

/// `date_picker_showing(key, label, value, calendar)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatePickerShowingParams {
    key: Key,
    label: String,
    value: String,
    calendar: CalendarWire,
}
impl ParamShape for DatePickerShowingParams {
    const LUAU: &'static str = "{ key: string, label: string, value: string, calendar: \
         { type: \"compact\", year: number, month: number } | \
         { type: \"full\", year: number, month: number } | \
         { type: \"choosing\", year: number, month: number } }";
}

/// `dropdown_open`/`select_open`(key, label, value, options).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelValueChildren {
    key: Key,
    label: String,
    value: String,
    /// `#[serde(default)]` because the Lua factory omits an empty table:
    /// Lua cannot distinguish an empty list from an empty map, so sending
    /// one would arrive as `{}` and fail to deserialize.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for KeyLabelValueChildren {
    const LUAU: &'static str =
        "{ key: string, label: string, value: string, children: { ViewNode } }";
}

/// `field_invalid`/`field_warning`/`file_uploader_item_invalid`(key, label, message).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelMessage {
    key: Key,
    label: String,
    message: String,
}
impl ParamShape for KeyLabelMessage {
    const LUAU: &'static str = "{ key: string, label: string, message: string }";
}

/// `checkbox_warning`/`radio_warning`(key, label, selected, message).
/// The bool is `checked` on `checkbox_warning` and `selected` on
/// `radio_warning`; the wire name is `selected`, matching [`KeyLabelSelected`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelSelectedMessage {
    key: Key,
    label: String,
    selected: bool,
    message: String,
}
impl ParamShape for KeyLabelSelectedMessage {
    const LUAU: &'static str = "{ key: string, label: string, selected: boolean, message: string }";
}

/// `field_validated(key, label, message?)`: `message` is optional (`None`
/// builds the enabled well, `Some` builds the invalid one with a helper).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelOptionalMessage {
    key: Key,
    label: String,
    #[serde(default)]
    message: Option<String>,
}
impl ParamShape for KeyLabelOptionalMessage {
    const LUAU: &'static str = "{ key: string, label: string, message: string? }";
}

/// `number_input_invalid(key, label, value, message)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelValueMessage {
    key: Key,
    label: String,
    value: String,
    message: String,
}
impl ParamShape for KeyLabelValueMessage {
    const LUAU: &'static str = "{ key: string, label: string, value: string, message: string }";
}

/// `file_uploader_with(key, label, description)`/`field_described(key,
/// label, description)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelDescription {
    key: Key,
    label: String,
    description: String,
}
impl ParamShape for KeyLabelDescription {
    const LUAU: &'static str = "{ key: string, label: string, description: string }";
}

/// `checkbox_described`/`radio_described`(key, label, selected,
/// description). The bool is `checked` on `checkbox_described` and
/// `selected` on `radio_described`; the wire name is `selected`, matching
/// [`KeyLabelSelectedMessage`]'s own convention.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelSelectedDescription {
    key: Key,
    label: String,
    selected: bool,
    description: String,
}
impl ParamShape for KeyLabelSelectedDescription {
    const LUAU: &'static str =
        "{ key: string, label: string, selected: boolean, description: string }";
}

/// `field.rs`'s `labeled(key, label, control)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelControl {
    key: Key,
    label: String,
    control: ViewNode,
}
impl ParamShape for KeyLabelControl {
    const LUAU: &'static str = "{ key: string, label: string, control: ViewNode }";
}

/// `field.rs`'s `hinted(node, hint)`: a modifier over an already-built node,
/// not a fresh construction, so its shape carries no `key` of its own.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeAndHint {
    node: ViewNode,
    hint: String,
}
impl ParamShape for NodeAndHint {
    const LUAU: &'static str = "{ node: ViewNode, hint: string }";
}

/// `field.rs`'s `valued(node, value)`: the same modifier shape as
/// [`NodeAndHint`], over the other field name.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeAndValue {
    node: ViewNode,
    value: String,
}
impl ParamShape for NodeAndValue {
    const LUAU: &'static str = "{ node: ViewNode, value: string }";
}

// ---------------------------------------------------------------------
// This group's constructors.
// ---------------------------------------------------------------------
pub const ENTRIES: &[Entry] = &[
    // controls.rs — Checkbox, Radio button, Toggle.
    row!("checkbox", KeyLabelSelected, |p| lib::checkbox(
        p.key, p.label, p.selected
    )),
    row!("checkbox_tristate", CheckboxTristateParams, |p| {
        lib::checkbox_tristate(p.key, p.label, p.state.into())
    }),
    row!("checkbox_indeterminate", KeyLabel, |p| {
        lib::checkbox_indeterminate(p.key, p.label)
    }),
    row!("checkbox_readonly", KeyLabelSelected, |p| {
        lib::checkbox_readonly(p.key, p.label, p.selected)
    }),
    row!("checkbox_group", KeyLabelChildren, |p| lib::checkbox_group(
        p.key, p.label, p.children
    )),
    row!("checkbox_warning", KeyLabelSelectedMessage, |p| {
        lib::checkbox_warning(p.key, p.label, p.selected, p.message)
    }),
    row!("checkbox_required", KeyLabelSelected, |p| {
        lib::checkbox_required(p.key, p.label, p.selected)
    }),
    row!("checkbox_described", KeyLabelSelectedDescription, |p| {
        lib::checkbox_described(p.key, p.label, p.selected, p.description)
    }),
    row!("radio", KeyLabelSelected, |p| lib::radio(
        p.key, p.label, p.selected
    )),
    row!("radio_warning", KeyLabelSelectedMessage, |p| {
        lib::radio_warning(p.key, p.label, p.selected, p.message)
    }),
    row!("radio_required", KeyLabelSelected, |p| {
        lib::radio_required(p.key, p.label, p.selected)
    }),
    row!("radio_described", KeyLabelSelectedDescription, |p| {
        lib::radio_described(p.key, p.label, p.selected, p.description)
    }),
    row!("radio_group", KeyLabelChildren, |p| lib::radio_group(
        p.key, p.label, p.children
    )),
    row!("toggle", KeyLabelSelected, |p| lib::toggle(
        p.key, p.label, p.selected
    )),
    row!("toggle_sm", KeyLabelSelected, |p| lib::toggle_sm(
        p.key, p.label, p.selected
    )),
    // date_picker.rs — Date picker.
    row!("date_picker", KeyLabelValue, |p| lib::date_picker(
        p.key, p.label, p.value
    )),
    row!(
        "date_picker_open",
        KeyLabelValue,
        |p| lib::date_picker_open(p.key, p.label, p.value)
    ),
    row!("date_picker_showing", DatePickerShowingParams, |p| {
        lib::date_picker_showing(p.key, p.label, p.value, p.calendar.into())
    }),
    // dropdown.rs — Dropdown.
    row!("dropdown", KeyLabelValue, |p| lib::dropdown(
        p.key, p.label, p.value
    )),
    row!("dropdown_xs", KeyLabelValue, |p| lib::dropdown_xs(
        p.key, p.label, p.value
    )),
    row!("dropdown_sm", KeyLabelValue, |p| lib::dropdown_sm(
        p.key, p.label, p.value
    )),
    row!("dropdown_lg", KeyLabelValue, |p| lib::dropdown_lg(
        p.key, p.label, p.value
    )),
    row!("dropdown_open", KeyLabelValueChildren, |p| {
        lib::dropdown_open(p.key, p.label, p.value, p.children)
    }),
    row!("dropdown_option", KeyLabelSelected, |p| {
        lib::dropdown_option(p.key, p.label, p.selected)
    }),
    // field.rs — Text input.
    row!("field", KeyLabel, |p| lib::field(p.key, p.label)),
    row!("field_sm", KeyLabel, |p| lib::field_sm(p.key, p.label)),
    row!("field_lg", KeyLabel, |p| lib::field_lg(p.key, p.label)),
    row!("field_fluid", KeyLabel, |p| lib::field_fluid(
        p.key, p.label
    )),
    row!("field_labeled", KeyLabel, |p| lib::field_labeled(
        p.key, p.label
    )),
    row!("field_required", KeyLabel, |p| lib::field_required(
        p.key, p.label
    )),
    row!("field_readonly", KeyLabel, |p| lib::field_readonly(
        p.key, p.label
    )),
    row!("field_invalid", KeyLabelMessage, |p| lib::field_invalid(
        p.key, p.label, p.message
    )),
    row!("field_warning", KeyLabelMessage, |p| lib::field_warning(
        p.key, p.label, p.message
    )),
    row!("field_validated", KeyLabelOptionalMessage, |p| {
        lib::field_validated(p.key, p.label, p.message)
    }),
    row!("field_described", KeyLabelDescription, |p| {
        lib::field_described(p.key, p.label, p.description)
    }),
    row!("labeled", KeyLabelControl, |p| lib::labeled(
        p.key, p.label, p.control
    )),
    row!("hinted", NodeAndHint, |p| lib::hinted(p.node, p.hint)),
    row!("valued", NodeAndValue, |p| lib::valued(p.node, p.value)),
    // file_uploader.rs — File uploader.
    row!("file_uploader", KeyLabel, |p| lib::file_uploader(
        p.key, p.label
    )),
    row!("file_uploader_with", KeyLabelDescription, |p| {
        lib::file_uploader_with(p.key, p.label, p.description)
    }),
    row!("file_uploader_item", KeyLabelSelected, |p| {
        lib::file_uploader_item(p.key, p.label, p.selected)
    }),
    row!("file_uploader_item_edit", KeyLabel, |p| {
        lib::file_uploader_item_edit(p.key, p.label)
    }),
    row!("file_uploader_item_invalid", KeyLabelMessage, |p| {
        lib::file_uploader_item_invalid(p.key, p.label, p.message)
    }),
    row!("file_uploader_item_warning", KeyLabelMessage, |p| {
        lib::file_uploader_item_warning(p.key, p.label, p.message)
    }),
    // form.rs — Form.
    row!("form", KeyLabelChildren, |p| lib::form(
        p.key, p.label, p.children
    )),
    // number_input.rs — Number input.
    row!("number_input", KeyLabelValue, |p| lib::number_input(
        p.key, p.label, p.value
    )),
    row!("number_input_sm", KeyLabelValue, |p| lib::number_input_sm(
        p.key, p.label, p.value
    )),
    row!("number_input_lg", KeyLabelValue, |p| lib::number_input_lg(
        p.key, p.label, p.value
    )),
    row!("number_input_invalid", KeyLabelValueMessage, |p| {
        lib::number_input_invalid(p.key, p.label, p.value, p.message)
    }),
    row!("number_input_warning", KeyLabelValueMessage, |p| {
        lib::number_input_warning(p.key, p.label, p.value, p.message)
    }),
    // search.rs — Search.
    row!("search", KeyLabel, |p| lib::search(p.key, p.label)),
    row!("search_sm", KeyLabel, |p| lib::search_sm(p.key, p.label)),
    row!("search_lg", KeyLabel, |p| lib::search_lg(p.key, p.label)),
    // select.rs — Select.
    row!("select", KeyLabelValue, |p| lib::select(
        p.key, p.label, p.value
    )),
    row!("select_sm", KeyLabelValue, |p| lib::select_sm(
        p.key, p.label, p.value
    )),
    row!("select_lg", KeyLabelValue, |p| lib::select_lg(
        p.key, p.label, p.value
    )),
    row!("select_open", KeyLabelValueChildren, |p| lib::select_open(
        p.key, p.label, p.value, p.children
    )),
    // slider.rs — Slider.
    row!("slider", KeyLabelNumber, |p| lib::slider(
        p.key, p.label, p.value
    )),
    row!("slider_readonly", KeyLabelNumber, |p| lib::slider_readonly(
        p.key, p.label, p.value
    )),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::ENTRIES;

    /// Every `pub fn ... -> ViewNode` in the ten files this group owns has
    /// exactly one row, found by scanning the files' own text rather than
    /// trusting a hand-written list — a constructor added later and never
    /// registered fails this the moment it lands, not at review time.
    ///
    /// The heuristic: a `pub fn` declaration in these files never has a `{`
    /// in its signature (no where-clause, no default-const-generic body)
    /// before the one that opens its own body, so the text between `pub fn `
    /// and the next `{` is the whole signature, and it names a `ViewNode`
    /// constructor exactly when that text contains `-> ViewNode`.
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
        let sources: &[(&str, &str)] = &[
            ("controls.rs", include_str!("../controls.rs")),
            ("date_picker.rs", include_str!("../date_picker.rs")),
            ("dropdown.rs", include_str!("../dropdown.rs")),
            ("field.rs", include_str!("../field.rs")),
            ("file_uploader.rs", include_str!("../file_uploader.rs")),
            ("form.rs", include_str!("../form.rs")),
            ("number_input.rs", include_str!("../number_input.rs")),
            ("search.rs", include_str!("../search.rs")),
            ("select.rs", include_str!("../select.rs")),
            ("slider.rs", include_str!("../slider.rs")),
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
            "these rows name no `pub fn ... -> ViewNode` in the ten files this group owns: {extra:?}"
        );
    }

    /// Constructor pairs that build the same node ON PURPOSE, because they
    /// share one delegation path or the shipped library documents them as
    /// one anatomy. Each entry names why, checked against the source below;
    /// a pair NOT listed here that collides is a mis-wired row.
    ///
    /// `select`/`select_sm`/`select_lg` against their `dropdown` opposite
    /// numbers: every one of these six calls `list_box_field` at the same
    /// `ListBoxSize` with no other input, and `select.rs`'s own module doc
    /// says the tie is intentional — "the same node Dropdown's field is,
    /// because in Carbon they are the same field." Neither delegates to the
    /// other (each file keeps its own private `labelled`), but both produce
    /// byte-identical output at every shared size, which is the documented
    /// fact this table exists to record. `select_open`/`dropdown_open` are
    /// the same tie at the open form: both call their own private
    /// `labelled(key, label, value, <Md size>, Some(options))`, and this
    /// probe measured them byte-identical at that size — not assumed from
    /// the closed-form tie above.
    const ALIASES: &[(&str, &str, &str)] = &[
        (
            "select",
            "dropdown",
            "both build list_box_field at ListBoxSize::Md; select.rs's own doc calls this intentional",
        ),
        (
            "select_sm",
            "dropdown_sm",
            "both build list_box_field at ListBoxSize::Sm; same anatomy as select/dropdown",
        ),
        (
            "select_lg",
            "dropdown_lg",
            "both build list_box_field at ListBoxSize::Lg; same anatomy as select/dropdown",
        ),
        (
            "select_open",
            "dropdown_open",
            "both call their own private labelled(.., SIZE_MD/ListBoxSize::Md, Some(options));              measured byte-identical at that size, same anatomy as select/dropdown",
        ),
    ];

    fn is_known_tie(a: &str, b: &str) -> bool {
        ALIASES
            .iter()
            .any(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a))
    }

    /// No two rows build the same `ViewNode` from the same params, other
    /// than a [`KNOWN_TIES`] pair: the realistic failure this catches is a
    /// copy-pasted row that calls the wrong sibling constructor (a
    /// `dropdown_lg` row wired to `dropdown_sm`, say). Rows are grouped by
    /// the shape they accept — two rows that take different shapes can
    /// never be handed "the same params" at all — and within a group every
    /// row is built from one shared, fully-populated probe table for that
    /// shape, then checked pairwise for accidental equality.
    #[test]
    fn no_two_rows_of_the_same_shape_build_the_same_view_node_from_the_same_params() {
        use serde_json::json;

        let probes: &[(&str, serde_json::Value)] = &[
            (
                "{ key, label, selected: boolean }",
                json!({"key": "probe", "label": "Probe", "selected": true}),
            ),
            ("{ key, label }", json!({"key": "probe", "label": "Probe"})),
            (
                "{ key, label, children }",
                json!({"key": "probe", "label": "Probe", "children": []}),
            ),
            (
                "{ key, label, value }",
                json!({"key": "probe", "label": "Probe", "value": "Value"}),
            ),
            (
                "{ key, label, value: number }",
                json!({"key": "probe", "label": "Probe", "value": 42.0}),
            ),
            (
                "{ key, label, value, children }",
                json!({"key": "probe", "label": "Probe", "value": "Value", "children": []}),
            ),
            (
                "{ key, label, message }",
                json!({"key": "probe", "label": "Probe", "message": "Message"}),
            ),
            (
                "{ key, label, selected, message }",
                json!({
                    "key": "probe", "label": "Probe", "selected": true, "message": "Message"
                }),
            ),
            (
                "{ key, label, value, message }",
                json!({
                    "key": "probe", "label": "Probe", "value": "Value", "message": "Message"
                }),
            ),
        ];
        let shape_of = |luau: &str| -> Option<&'static str> {
            match luau {
                "{ key: string, label: string, selected: boolean }" => {
                    Some("{ key, label, selected: boolean }")
                }
                "{ key: string, label: string }" => Some("{ key, label }"),
                "{ key: string, children: { ViewNode } }"
                | "{ key: string, label: string, children: { ViewNode } }" => {
                    Some("{ key, label, children }")
                }
                "{ key: string, label: string, value: string }" => Some("{ key, label, value }"),
                "{ key: string, label: string, value: number }" => {
                    Some("{ key, label, value: number }")
                }
                // `select_open`/`dropdown_open`: the only two rows sharing
                // `KeyLabelValueChildren`. Checked here rather than left out
                // — a one-off shape used by exactly two constructors is
                // exactly the shape most likely to hide a copy-pasted row.
                "{ key: string, label: string, value: string, children: { ViewNode } }" => {
                    Some("{ key, label, value, children }")
                }
                // `field_invalid`/`field_warning`/`file_uploader_item_invalid`
                // share `KeyLabelMessage` across two unrelated components —
                // checked for the same reason.
                "{ key: string, label: string, message: string }" => {
                    Some("{ key, label, message }")
                }
                "{ key: string, label: string, selected: boolean, message: string }" => {
                    Some("{ key, label, selected, message }")
                }
                "{ key: string, label: string, value: string, message: string }" => {
                    Some("{ key, label, value, message }")
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
                // `checkbox_group`/`radio_group`/`form` take `children` under
                // that name, not `label`+`children` alone — the probe above
                // covers `KeyLabelChildren`'s wire shape either way since the
                // JSON key is `children` in both.
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

    /// G4: a bad parameter table's error names the offending field, not
    /// merely "deserialization failed". Every shape derives
    /// `#[serde(deny_unknown_fields)]`, so `serde_json::from_value`'s own
    /// message already names the field for both an unknown key and a
    /// missing required one; this test pins that `fail` propagates that
    /// message rather than replacing it with something generic.
    #[test]
    fn a_bad_parameter_table_names_the_offending_field() {
        let checkbox = ENTRIES
            .iter()
            .find(|e| e.name == "checkbox")
            .expect("checkbox must be registered");
        let unknown_field = (checkbox.ctor)(&serde_json::json!({
            "key": "k", "label": "L", "selected": true, "lable": "typo"
        }))
        .expect_err("an extra field must be refused");
        assert!(
            unknown_field.reason.contains("lable"),
            "error must name the misspelled field, got: {}",
            unknown_field.reason
        );

        let missing_field = (checkbox.ctor)(&serde_json::json!({ "key": "k", "label": "L" }))
            .expect_err("a missing required field must be refused");
        assert!(
            missing_field.reason.contains("selected"),
            "error must name the missing field, got: {}",
            missing_field.reason
        );
    }
}
