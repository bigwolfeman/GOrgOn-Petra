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

use crate::tree::PropType;

/// `(component name, [(parameter, type)])`, sorted by component name.
const OPENABLE: &[(&str, &[(&str, PropType)])] = &[
    ("checkbox", &[("selected", PropType::Bool)]),
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
    ("list_row", &[("selected", PropType::Bool)]),
    ("toggle", &[("selected", PropType::Bool)]),
    ("toggle_sm", &[("selected", PropType::Bool)]),
    ("valued", &[("value", PropType::Str)]),
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
