//! [`ReservedChords`]: the non-empty set of chords no raw claim can receive.
//!
//! Spec 010 FR-021: "A non-empty reserved chord set MUST exist that no raw
//! claim can receive. An empty reserved set MUST be unrepresentable, not
//! merely discouraged." [`ReservedChords::new`] is therefore the only way to
//! build one, and it refuses an empty `Vec` rather than accepting it and
//! hoping nothing downstream ever produces one — a `ReservedChords` that
//! exists is proof the set is non-empty, the same shape
//! [`crate::keymap::binding::Binding::new`] gives a non-empty trigger
//! (FR-002).
//!
//! [`ReservedChords::default`] is exactly `{ shift-esc }` (FR-021a, the
//! operator's decision of 2026-09-08); see [`crate::keymap::chord::Chord::reserved_escape`]
//! for the argument.

use crate::keymap::chord::Chord;

/// Why a [`ReservedChords`] could not be constructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservedChordsError {
    /// FR-021: the set must be non-empty by construction.
    Empty,
}

impl std::fmt::Display for ReservedChordsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str(
                "a reserved chord set must not be empty (spec 010 FR-021: an \
                 empty reserved set must be unrepresentable, not merely \
                 discouraged)",
            ),
        }
    }
}

impl std::error::Error for ReservedChordsError {}

/// The chords that escape a raw claim, no exceptions (spec 010 FR-021).
///
/// Non-empty by construction: [`ReservedChords::new`] is the only
/// constructor other than [`ReservedChords::default`], and it refuses an
/// empty `Vec`. A raw claim outranks the binding table (FR-020), but nothing
/// outranks this set — [`crate::input::route_with_reserved`] checks it first,
/// ahead of the raw-claim check itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReservedChords {
    chords: Vec<Chord>,
}

impl ReservedChords {
    /// Builds a reserved set from `chords`, refusing an empty one (FR-021).
    ///
    /// Duplicates are not refused: a caller that lists the same chord twice
    /// has made a harmless mistake, not an unrepresentable one, and refusing
    /// it would be a second kind of error this type does not exist to catch.
    pub fn new(chords: Vec<Chord>) -> Result<Self, ReservedChordsError> {
        if chords.is_empty() {
            return Err(ReservedChordsError::Empty);
        }
        Ok(Self { chords })
    }

    /// Whether `chord` is in this set.
    #[must_use]
    pub fn contains(&self, chord: Chord) -> bool {
        self.chords.contains(&chord)
    }
}

impl Default for ReservedChords {
    /// `{ shift-esc }` (FR-021a). Infallible: a single known chord is never
    /// an empty `Vec`.
    fn default() -> Self {
        Self {
            chords: vec![Chord::reserved_escape()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyCode;

    #[test]
    fn an_empty_reserved_set_is_unrepresentable() {
        assert_eq!(
            ReservedChords::new(Vec::new()),
            Err(ReservedChordsError::Empty)
        );
    }

    #[test]
    fn the_default_reserved_chord_is_shift_esc() {
        let reserved = ReservedChords::default();
        assert!(reserved.contains(Chord::reserved_escape()));
        assert!(!reserved.contains(Chord::bare(KeyCode::Escape)));
    }

    #[test]
    fn a_constructed_set_contains_exactly_what_it_was_built_from() {
        let ctrl_q = Chord::bare(KeyCode::Char('q'));
        let reserved = ReservedChords::new(vec![ctrl_q]).unwrap();
        assert!(reserved.contains(ctrl_q));
        assert!(!reserved.contains(Chord::reserved_escape()));
    }

    #[test]
    fn duplicate_chords_are_not_refused() {
        let chord = Chord::reserved_escape();
        assert!(ReservedChords::new(vec![chord, chord]).is_ok());
    }
}
