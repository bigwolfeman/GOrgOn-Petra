//! Which component parameters may come from a slot.
//!
//! A component reference may carry `bound` sources
//! ([`crate::tree::ComponentRef::bound`]) for the parameters listed here and
//! for no others; anything else is refused by name at expansion, the same
//! refusal a literal-only reference always gave. Each row is a parameter a
//! gallery page or a shell surface actually drives from state — a toggle's
//! position, a row's selection, a sort header's direction, a cell's value —
//! and its [`PropType`] is the one scalar the constructor's parameter struct
//! deserializes it as.
//!
//! A parameter joins this table when a caller needs it, never speculatively:
//! each row is a promise that re-expanding the component on that
//! parameter's slot commit is bounded and correct
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).
//!
//! Two kinds of row are not a plain flag. `checkbox_tristate.state` is the
//! three-state box as its wire spelling (`"unchecked"`, `"checked"`,
//! `"mixed"`), so a `Str` (or enum) slot drives the indeterminate state and
//! a value outside the three is the constructor's named refusal. The
//! disclosure rows (`expanded` on an accordion item or an expandable tile)
//! are *visibility*: the
//! parameter decides whether the item's body is mounted at all, so a commit
//! adds or removes that subtree by key inside the regrown unit.

use crate::tree::PropType;

/// `(component name, [(parameter, type)])`, sorted by component name.
const OPENABLE: &[(&str, &[(&str, PropType)])] = &[
    ("accordion_item_spaced", &[("expanded", PropType::Bool)]),
    (
        "accordion_item_with_spaced",
        &[("expanded", PropType::Bool)],
    ),
    ("checkbox", &[("selected", PropType::Bool)]),
    ("checkbox_tristate", &[("state", PropType::Str)]),
    ("contained_tab", &[("selected", PropType::Bool)]),
    ("content_switcher_item", &[("selected", PropType::Bool)]),
    ("data_table_row", &[("selected", PropType::Bool)]),
    (
        "data_table_row_expandable",
        &[("expanded", PropType::Bool), ("selected", PropType::Bool)],
    ),
    ("data_table_row_lg", &[("selected", PropType::Bool)]),
    ("data_table_row_md", &[("selected", PropType::Bool)]),
    ("data_table_row_sm", &[("selected", PropType::Bool)]),
    ("data_table_row_xl", &[("selected", PropType::Bool)]),
    ("data_table_row_xs", &[("selected", PropType::Bool)]),
    ("data_table_sort_header", &[("selected", PropType::Bool)]),
    ("expandable_tile", &[("expanded", PropType::Bool)]),
    ("list_row", &[("selected", PropType::Bool)]),
    ("radio", &[("selected", PropType::Bool)]),
    ("selectable_tag", &[("selected", PropType::Bool)]),
    ("selectable_tile", &[("selected", PropType::Bool)]),
    ("tab", &[("selected", PropType::Bool)]),
    ("toggle", &[("selected", PropType::Bool)]),
    ("toggle_sm", &[("selected", PropType::Bool)]),
    ("ui_shell_header_menu_trigger", &[("open", PropType::Bool)]),
    ("ui_shell_header_nav_item", &[("selected", PropType::Bool)]),
    ("valued", &[("value", PropType::Str)]),
    ("vertical_tab", &[("selected", PropType::Bool)]),
];

/// The parameters `component` declares openable, with the type each holds.
/// Empty for a component that opens none.
#[must_use]
pub fn openable(component: &str) -> &'static [(&'static str, PropType)] {
    OPENABLE
        .binary_search_by(|(name, _)| name.cmp(&component))
        .map_or(&[], |ix| OPENABLE[ix].1)
}

/// Every `(component, parameters)` row, in component-name order — what the
/// Lua factory is seeded with, so both languages refuse the same set.
#[must_use]
pub fn openable_table() -> &'static [(&'static str, &'static [(&'static str, PropType)])] {
    OPENABLE
}

/// The declared type of `param` on `component`, or `None` when that
/// parameter is not openable.
#[must_use]
pub fn openable_param(component: &str, param: &str) -> Option<(&'static str, PropType)> {
    openable(component)
        .iter()
        .find(|(name, _)| *name == param)
        .copied()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::component::registry::lookup;

    #[test]
    fn the_table_is_sorted_so_the_probe_is_a_binary_search() {
        assert!(
            OPENABLE.windows(2).all(|w| w[0].0 < w[1].0),
            "OPENABLE must be strictly sorted by component name"
        );
    }

    /// Every row names a registered component, a field its parameter struct
    /// really has, and a type that field really deserializes as: the row's
    /// type is flipped to a value of another type and the constructor must
    /// refuse it by that field's name.
    #[test]
    fn every_openable_parameter_is_a_real_field_of_its_declared_type() {
        for (component, params) in OPENABLE {
            let entry = lookup(component)
                .unwrap_or_else(|| panic!("openable row `{component}` names no component"));
            for (param, ty) in *params {
                assert!(
                    entry.luau.contains(&format!("{param}:")),
                    "`{component}` renders `{}`, which has no `{param}` field",
                    entry.luau
                );
                let wrong = match ty {
                    PropType::Bool => json!("not a flag"),
                    PropType::Str => json!(true),
                    PropType::Num => json!("not a number"),
                    PropType::Rows => panic!(
                        "`{component}.{param}` is declared Rows; no constructor parameter \
                         deserializes rows"
                    ),
                };
                let mut table = serde_json::Map::new();
                table.insert((*param).to_owned(), wrong);
                let err = (entry.ctor)(&serde_json::Value::Object(table))
                    .expect_err("a wrongly typed parameter must be refused");
                assert!(
                    err.reason.contains("invalid type"),
                    "`{component}.{param}` must refuse a value of another type as a type \
                     error, got: {}",
                    err.reason
                );
            }
        }
    }

    /// The tristate's `Str` row admits exactly the three wire spellings: a
    /// fourth string is the constructor's refusal naming the variants, never
    /// a silent fallback to one of them.
    #[test]
    fn a_tristate_state_outside_the_three_is_refused_by_name() {
        let entry = lookup("checkbox_tristate").expect("registered");
        for state in ["unchecked", "checked", "mixed"] {
            (entry.ctor)(&json!({ "key": "c", "label": "C", "state": state }))
                .unwrap_or_else(|err| panic!("`{state}` builds: {}", err.reason));
        }
        let err = (entry.ctor)(&json!({ "key": "c", "label": "C", "state": "half" }))
            .expect_err("a fourth state refuses");
        assert!(
            err.reason.contains("half") && err.reason.contains("mixed"),
            "the refusal names the value and the variants: {}",
            err.reason
        );
    }

    #[test]
    fn a_component_that_opens_nothing_answers_empty() {
        assert!(openable("button").is_empty());
        assert!(openable_param("toggle", "label").is_none());
        assert_eq!(
            openable_param("toggle", "selected"),
            Some(("selected", PropType::Bool))
        );
    }
}
