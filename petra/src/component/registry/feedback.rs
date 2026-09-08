//! Registry rows: feedback. See [`super`] for why this file exists.
//!
//! Each row names a public constructor in `petra/petra/src/component/`,
//! deserializes its parameter table into a shape from
//! [`crate::component::params`], and calls the shipped constructor. A row
//! must never re-derive what the constructor does.
//!
//! Files covered: `ai_label.rs` (AI label), `inline_loading.rs` (Inline
//! loading), `loading.rs` (Loading), `notification.rs` (Notification),
//! `progress.rs` (Progress bar), `progress_indicator.rs` (Progress
//! indicator), `tag.rs` (Tag). Every `pub fn` in those seven files whose
//! signature reads `-> ViewNode` has exactly one row below — the
//! `every_view_node_constructor_has_exactly_one_row` test proves it by
//! scanning the seven files' own source text, not by trusting this list.
//!
//! `loading.rs` and `progress.rs` each carry real `pub fn ... -> ViewNode`
//! constructors (`loading`/`loading_sm`, `progress`/`progress_sm`/
//! `progress_with_helper`); `tag.rs` carries five. Spec 005's tasks.md still
//! shows T086/T094/T102 unchecked, but the checkbox is a bookkeeping gap,
//! not an implementation one — all three are registered below like every
//! other constructor in this file.

use serde::Deserialize;
use serde_json::Value;

use super::Entry;
use crate::component as lib;
use crate::component::params::{
    KeyChildren, KeyLabel, KeyLabelNumber, KeyLabelSelected, KeyLabelSelectedValue, KeyLabelValue,
    ParamError, ParamShape,
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

/// `ai_label_with_actions(key, label, open, body, actions)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AiLabelActionsParams {
    key: Key,
    label: String,
    open: bool,
    body: String,
    /// `#[serde(default)]` because the Lua factory omits an empty table:
    /// Lua cannot distinguish an empty list from an empty map, so sending
    /// one would arrive as `{}` and fail to deserialize.
    #[serde(default)]
    actions: Vec<ViewNode>,
}
impl ParamShape for AiLabelActionsParams {
    const LUAU: &'static str =
        "{ key: string, label: string, open: boolean, body: string, actions: { ViewNode } }";
}

/// `notification.rs`'s [`lib::NotificationKind`], as an agent writes it. A
/// closed four-variant union rather than a free string, so a plugin cannot
/// name a notification kind that does not exist (the operator's own
/// requirement for this shape).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum NotificationKindWire {
    Error,
    Warning,
    Info,
    Success,
}
impl From<NotificationKindWire> for lib::NotificationKind {
    fn from(w: NotificationKindWire) -> Self {
        match w {
            NotificationKindWire::Error => Self::Error,
            NotificationKindWire::Warning => Self::Warning,
            NotificationKindWire::Info => Self::Info,
            NotificationKindWire::Success => Self::Success,
        }
    }
}

/// `notification_toast_kind`/`notification_inline_kind`(key, kind, title,
/// body). One shape for both: the field set is identical.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NotificationKindParams {
    key: Key,
    kind: NotificationKindWire,
    title: String,
    body: String,
}
impl ParamShape for NotificationKindParams {
    const LUAU: &'static str = "{ key: string, kind: \"error\" | \"warning\" | \"info\" | \"success\", title: string, body: string }";
}

/// `notification_actionable(key, title, body, action)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NotificationActionableParams {
    key: Key,
    title: String,
    body: String,
    action: String,
}
impl ParamShape for NotificationActionableParams {
    const LUAU: &'static str = "{ key: string, title: string, body: string, action: string }";
}

/// `notification_actionable_kind(key, kind, title, body, action)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NotificationActionableKindParams {
    key: Key,
    kind: NotificationKindWire,
    title: String,
    body: String,
    action: String,
}
impl ParamShape for NotificationActionableKindParams {
    const LUAU: &'static str = "{ key: string, kind: \"error\" | \"warning\" | \"info\" | \"success\", \
         title: string, body: string, action: string }";
}

/// `progress_with_helper(key, label, value, helper)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgressWithHelperParams {
    key: Key,
    label: String,
    value: f32,
    helper: String,
}
impl ParamShape for ProgressWithHelperParams {
    const LUAU: &'static str = "{ key: string, label: string, value: number, helper: string }";
}

/// `progress_step(key, label, complete, current)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgressStepParams {
    key: Key,
    label: String,
    complete: bool,
    current: bool,
}
impl ParamShape for ProgressStepParams {
    const LUAU: &'static str =
        "{ key: string, label: string, complete: boolean, current: boolean }";
}

// ---------------------------------------------------------------------
// This group's constructors.
// ---------------------------------------------------------------------
pub const ENTRIES: &[Entry] = &[
    // ai_label.rs — AI label.
    row!("ai_label", KeyLabelSelectedValue, |p| lib::ai_label(
        p.key, p.label, p.selected, p.value
    )),
    row!("ai_label_mini", KeyLabelSelectedValue, |p| {
        lib::ai_label_mini(p.key, p.label, p.selected, p.value)
    }),
    row!(
        "ai_label_2xs",
        KeyLabelSelectedValue,
        |p| lib::ai_label_2xs(p.key, p.label, p.selected, p.value)
    ),
    row!("ai_label_xs", KeyLabelSelectedValue, |p| lib::ai_label_xs(
        p.key, p.label, p.selected, p.value
    )),
    row!("ai_label_sm", KeyLabelSelectedValue, |p| lib::ai_label_sm(
        p.key, p.label, p.selected, p.value
    )),
    row!("ai_label_lg", KeyLabelSelectedValue, |p| lib::ai_label_lg(
        p.key, p.label, p.selected, p.value
    )),
    row!("ai_label_xl", KeyLabelSelectedValue, |p| lib::ai_label_xl(
        p.key, p.label, p.selected, p.value
    )),
    row!("ai_label_with_actions", AiLabelActionsParams, |p| {
        lib::ai_label_with_actions(p.key, p.label, p.open, p.body, p.actions)
    }),
    row!("ai_label_revert", KeyLabel, |p| lib::ai_label_revert(
        p.key, p.label
    )),
    row!("ai_label_inline", KeyLabelSelectedValue, |p| {
        lib::ai_label_inline(p.key, p.label, p.selected, p.value)
    }),
    row!("ai_label_inline_sm", KeyLabelSelectedValue, |p| {
        lib::ai_label_inline_sm(p.key, p.label, p.selected, p.value)
    }),
    row!("ai_label_inline_lg", KeyLabelSelectedValue, |p| {
        lib::ai_label_inline_lg(p.key, p.label, p.selected, p.value)
    }),
    // inline_loading.rs — Inline loading.
    row!("inline_loading", KeyLabelNumber, |p| lib::inline_loading(
        p.key,
        p.label,
        f64::from(p.value)
    )),
    row!("inline_loading_finished", KeyLabel, |p| {
        lib::inline_loading_finished(p.key, p.label)
    }),
    // loading.rs — Loading.
    row!("loading", KeyLabelNumber, |p| lib::loading(
        p.key,
        p.label,
        f64::from(p.value)
    )),
    row!("loading_sm", KeyLabelNumber, |p| lib::loading_sm(
        p.key,
        p.label,
        f64::from(p.value)
    )),
    // notification.rs — Notification.
    row!("notification", KeyLabelValue, |p| lib::notification(
        p.key, p.label, p.value
    )),
    row!("notification_toast", KeyLabelValue, |p| {
        lib::notification_toast(p.key, p.label, p.value)
    }),
    row!("notification_toast_kind", NotificationKindParams, |p| {
        lib::notification_toast_kind(p.key, p.kind.into(), p.title, p.body)
    }),
    row!("notification_inline", KeyLabelValue, |p| {
        lib::notification_inline(p.key, p.label, p.value)
    }),
    row!("notification_inline_kind", NotificationKindParams, |p| {
        lib::notification_inline_kind(p.key, p.kind.into(), p.title, p.body)
    }),
    row!(
        "notification_actionable",
        NotificationActionableParams,
        |p| lib::notification_actionable(p.key, p.title, p.body, p.action)
    ),
    row!(
        "notification_actionable_kind",
        NotificationActionableKindParams,
        |p| lib::notification_actionable_kind(p.key, p.kind.into(), p.title, p.body, p.action)
    ),
    // progress.rs — Progress bar.
    row!("progress", KeyLabelNumber, |p| lib::progress(
        p.key, p.label, p.value
    )),
    row!("progress_sm", KeyLabelNumber, |p| lib::progress_sm(
        p.key, p.label, p.value
    )),
    row!("progress_with_helper", ProgressWithHelperParams, |p| {
        lib::progress_with_helper(p.key, p.label, p.value, p.helper)
    }),
    // progress_indicator.rs — Progress indicator.
    row!("progress_indicator", KeyChildren, |p| {
        lib::progress_indicator(p.key, p.children)
    }),
    row!("progress_step", ProgressStepParams, |p| lib::progress_step(
        p.key, p.label, p.complete, p.current
    )),
    // tag.rs — Tag.
    row!("tag", KeyLabel, |p| lib::tag(p.key, p.label)),
    row!("tag_sm", KeyLabel, |p| lib::tag_sm(p.key, p.label)),
    row!("tag_lg", KeyLabel, |p| lib::tag_lg(p.key, p.label)),
    row!("dismissible_tag", KeyLabel, |p| lib::dismissible_tag(
        p.key, p.label
    )),
    row!("selectable_tag", KeyLabelSelected, |p| lib::selectable_tag(
        p.key, p.label, p.selected
    )),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::ENTRIES;

    /// See `registry/form.rs`'s identical helper for the heuristic this
    /// relies on: no signature in these seven files has a `{` before the one
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
        let sources: &[(&str, &str)] = &[
            ("ai_label.rs", include_str!("../ai_label.rs")),
            ("inline_loading.rs", include_str!("../inline_loading.rs")),
            ("loading.rs", include_str!("../loading.rs")),
            ("notification.rs", include_str!("../notification.rs")),
            ("progress.rs", include_str!("../progress.rs")),
            (
                "progress_indicator.rs",
                include_str!("../progress_indicator.rs"),
            ),
            ("tag.rs", include_str!("../tag.rs")),
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
            "these rows name no `pub fn ... -> ViewNode` in the seven files this group owns: {extra:?}"
        );
    }

    /// Constructor pairs that build the same node ON PURPOSE — see
    /// `registry/form.rs`'s `ALIASES` for the pattern this follows. A pair
    /// NOT listed here that collides in the probe below is a mis-wired row.
    ///
    /// `notification`/`notification_toast`: `notification.rs`'s own body
    /// says so — `pub fn notification(..) -> ViewNode { notification_toast(key,
    /// NotificationKind::default's caller, title, body) }` is a literal
    /// one-line delegation, not two independent implementations that
    /// happen to agree. `notification_toast_kind`/`notification_inline_kind`
    /// both take an explicit `kind`, so the probe below (fixed at one kind)
    /// cannot mask a real divergence between toast and inline forms — they
    /// are on `Layer::Toast` and a different layer respectively, per each
    /// function's own doc comment, and are not aliased here.
    const ALIASES: &[(&str, &str, &str)] = &[(
        "notification",
        "notification_toast",
        "notification(k,t,b) is defined as notification_toast(k,t,b); one delegates to the other",
    )];

    fn is_known_tie(a: &str, b: &str) -> bool {
        ALIASES
            .iter()
            .any(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a))
    }

    /// See `registry/form.rs`'s identical test for the reasoning: rows are
    /// grouped by the shape they accept, and every row in a group is fed one
    /// shared, fully-populated probe for that shape.
    #[test]
    fn no_two_rows_of_the_same_shape_build_the_same_view_node_from_the_same_params() {
        use serde_json::json;

        let probes: &[(&str, serde_json::Value)] = &[
            (
                "{ key, label, selected: boolean }",
                json!({"key": "probe", "label": "Probe", "selected": true}),
            ),
            ("{ key, label }", json!({"key": "probe", "label": "Probe"})),
            ("{ key, children }", json!({"key": "probe", "children": []})),
            (
                "{ key, label, value }",
                json!({"key": "probe", "label": "Probe", "value": "Value"}),
            ),
            (
                "{ key, label, value: number }",
                json!({"key": "probe", "label": "Probe", "value": 42.0}),
            ),
            (
                "{ key, label, selected: boolean, value }",
                json!({"key": "probe", "label": "Probe", "selected": true, "value": "Value"}),
            ),
            (
                "{ key, kind, title, body }",
                json!({"key": "probe", "kind": "info", "title": "Title", "body": "Body"}),
            ),
        ];
        let shape_of = |luau: &str| -> Option<&'static str> {
            match luau {
                "{ key: string, label: string, selected: boolean }" => {
                    Some("{ key, label, selected: boolean }")
                }
                "{ key: string, label: string }" => Some("{ key, label }"),
                "{ key: string, children: { ViewNode } }" => Some("{ key, children }"),
                "{ key: string, label: string, value: string }" => Some("{ key, label, value }"),
                "{ key: string, label: string, value: number }" => {
                    Some("{ key, label, value: number }")
                }
                "{ key: string, label: string, selected: boolean, value: string }" => {
                    Some("{ key, label, selected: boolean, value }")
                }
                // `notification_toast_kind`/`notification_inline_kind`: the
                // only two rows sharing `NotificationKindParams`. Checked
                // here rather than left out — one-off shapes used by
                // exactly two constructors are exactly where a copy-pasted
                // row hides.
                "{ key: string, kind: \"error\" | \"warning\" | \"info\" | \"success\", title: string, body: string }" => {
                    Some("{ key, kind, title, body }")
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
        let tag = ENTRIES
            .iter()
            .find(|e| e.name == "tag")
            .expect("tag must be registered");
        let unknown_field =
            (tag.ctor)(&serde_json::json!({ "key": "k", "label": "L", "lable": "typo" }))
                .expect_err("an extra field must be refused");
        assert!(
            unknown_field.reason.contains("lable"),
            "error must name the misspelled field, got: {}",
            unknown_field.reason
        );

        let missing_field = (tag.ctor)(&serde_json::json!({ "key": "k" }))
            .expect_err("a missing required field must be refused");
        assert!(
            missing_field.reason.contains("label"),
            "error must name the missing field, got: {}",
            missing_field.reason
        );
    }
}
