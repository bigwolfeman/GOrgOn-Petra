//! [`Scope`]: where a binding is reachable from.
//!
//! Spec 010 FR-008: exactly two shapes, and no when-clause expression
//! language. If a third shape is ever needed, that is a spec amendment, not
//! a change made here.

use serde::{Deserialize, Serialize};

/// Where a binding is reachable from.
///
/// Two shapes, and only two (FR-008):
///
/// * [`Scope::Subtree`] — reachable only while focus is inside the named
///   root. A subtree whose root is absent from the current frame simply does
///   not match (FR-008a); that is the normal case for a mounted-but-hidden
///   contribution, not a fault.
/// * [`Scope::Global`] — reachable regardless of focus. Requires an operator
///   grant (FR-012); this type does not enforce that grant itself, since
///   granting is the contribution ledger's concern, not the scope's.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Scope {
    /// Reachable only while focus is inside the subtree rooted at `root`.
    Subtree {
        /// Canonical id of the subtree's root node.
        root: String,
    },
    /// Reachable regardless of focus.
    Global,
}

impl Scope {
    /// A human-readable description of this scope, for refusal and log
    /// text (FR-010, SC-002).
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Subtree { root } => format!("subtree {root:?}"),
            Self::Global => "global".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtree_and_global_are_distinct_scopes() {
        let a = Scope::Subtree {
            root: "panel:1".to_owned(),
        };
        let b = Scope::Subtree {
            root: "panel:2".to_owned(),
        };
        assert_ne!(a, b, "different roots are different scopes");
        assert_ne!(a, Scope::Global);
    }

    #[test]
    fn describe_names_the_root() {
        let scope = Scope::Subtree {
            root: "panel:1".to_owned(),
        };
        assert!(scope.describe().contains("panel:1"));
        assert_eq!(Scope::Global.describe(), "global");
    }

    #[test]
    fn serde_round_trips_both_shapes() {
        let subtree = Scope::Subtree {
            root: "panel:1".to_owned(),
        };
        let json = serde_json::to_string(&subtree).unwrap();
        assert_eq!(json, r#"{"subtree":{"root":"panel:1"}}"#);
        assert_eq!(serde_json::from_str::<Scope>(&json).unwrap(), subtree);

        let global = Scope::Global;
        let json = serde_json::to_string(&global).unwrap();
        assert_eq!(json, r#""global""#);
        assert_eq!(serde_json::from_str::<Scope>(&json).unwrap(), global);
    }
}
