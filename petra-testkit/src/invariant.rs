//! Package-owned invariant companion for `gorgon-petra-testkit`.
//!
//! `gorgon-petra-testkit` now has a real runtime invariant: [`crate::wire`]'s
//! closed error-kind vocabulary is spelled by hand twice —
//! [`crate::wire::ErrorKind::as_wire`] writes the wire string a client reads,
//! and [`crate::wire::ErrorKind::parse`] is the inverse a second
//! implementation of the contract (or a future client, T033) would use to
//! read one back. `driver-protocol.md` fixes these five spellings by name;
//! the two functions drifting apart — one accepting a kind the other cannot
//! parse back — would be invisible to every test in this crate that only
//! ever goes one direction (every `WireError` this crate builds is spelled
//! with `as_wire` and immediately serialized, never round-tripped through
//! `parse`), and would surface only in a client nobody has written yet.
//!
//! Same shape as `gorgon-petra`'s own role-table invariant
//! (`petra/src/invariant.rs`): both hand-written halves come from one
//! closed enum instead of one written list, so there is nothing here for the
//! two to drift *from* — but `as_wire`/`parse` are still two separate `match`
//! arms someone edits by hand, and this is what notices if one arm moves
//! without the other.

/// Assert every [`crate::wire::ErrorKind`] round-trips through its wire
/// spelling.
///
/// Panics naming the kind whose `as_wire` output `parse` does not accept
/// back to the same kind.
pub fn install() {
    use crate::wire::ErrorKind;

    for kind in ErrorKind::ALL {
        let spelled = kind.as_wire();
        let parsed = ErrorKind::parse(spelled).unwrap_or_else(|| {
            panic!(
                "gorgon-petra-testkit: ErrorKind::{kind:?} spells as {spelled:?}, which \
                 ErrorKind::parse does not accept back; a client would silently fail to \
                 recognize this error kind"
            )
        });
        assert_eq!(
            parsed, kind,
            "gorgon-petra-testkit: {spelled:?} parses back as {parsed:?}, not {kind:?}; \
             as_wire and parse have drifted apart"
        );
    }
}

#[cfg(test)]
mod tests {
    /// The shipping error-kind vocabulary agrees with itself.
    #[test]
    fn install_accepts_the_shipping_error_kinds() {
        super::install();
    }

    /// The same assertion driven by a hand-built drifted spelling, so the
    /// test fails if the comparison is ever loosened to "the spelling is
    /// present".
    #[test]
    fn a_drifted_spelling_would_not_parse() {
        use crate::wire::ErrorKind;
        assert!(ErrorKind::parse("unknown_verb").is_none());
        assert_ne!(ErrorKind::UnknownVerb.as_wire(), "unknown_verb");
    }
}
