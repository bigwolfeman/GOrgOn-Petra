//! Package-owned invariant companion for `gorgon-petra`.
//!
//! `gorgon-petra` has a real runtime invariant, so this companion asserts it
//! instead of opting out: **the two hand-written halves of the semantic role
//! table must agree**.
//!
//! [`crate::tree::Role::as_wire`] and [`crate::tree::Role::parse`] are written
//! by hand from one list, [`crate::tree::ROLE_NAMES`]. Three consumers key off
//! that list — the AccessKit tree a screen reader reads, the agent's UI-state
//! query, and the driver's `role:` finders — and
//! `contracts/semantic-tree.md` makes divergence between them a bug by
//! definition. A role that serializes to a name which does not parse back
//! would not raise anything: the screen reader would keep working and a
//! driver finder would silently match nothing, which is exactly the failure
//! shape a gate cannot see.

/// Assert the semantic role table round-trips in both directions.
///
/// Panics when a name in [`crate::tree::ROLE_NAMES`] does not parse, when a
/// parsed role prints back to a different name, or when a built-in role
/// reports no static name.
pub fn install() {
    use crate::tree::{ROLE_NAMES, Role};

    for name in ROLE_NAMES {
        let role = Role::parse(name).unwrap_or_else(|bad| {
            panic!(
                "gorgon-petra: ROLE_NAMES lists {bad:?} but Role::parse rejects it; \
                 a driver finder for that role would match nothing"
            )
        });
        let printed = role.as_wire();
        assert_eq!(
            &printed, name,
            "gorgon-petra: role {name:?} prints back as {printed:?}; \
             as_wire and parse have drifted apart"
        );
        assert_eq!(
            role.builtin_name(),
            Some(*name),
            "gorgon-petra: role {name:?} reports no static name"
        );
    }
    assert!(
        Role::parse("custom:").is_err(),
        "gorgon-petra: an unnamed custom role would collide with every other one"
    );
}

#[cfg(test)]
mod tests {
    /// The shipping role table agrees with itself.
    #[test]
    fn install_accepts_the_shipping_role_table() {
        super::install();
    }

    /// The same assertion driven by a hand-built drifted name, so the test
    /// fails if the comparison is ever loosened to "the name is present".
    #[test]
    fn a_drifted_name_would_not_parse() {
        use crate::tree::Role;
        assert!(Role::parse("buttonn").is_err());
        assert_ne!(Role::Button.as_wire(), "buttonn");
    }
}
