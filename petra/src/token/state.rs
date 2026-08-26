//! Which token family a slot resolves to, given the state a node is in.
//!
//! Carbon **names the combination** rather than layering states: there is a
//! `$layer-selected-hover` in the theme, and it is its own colour, not
//! `$layer-hover` composited over `$layer-selected`. So resolving a state is
//! a *lookup* with a precedence chain and never a blend
//! (`contracts/interaction-state.md` §4, R-A §3), and this module is that
//! chain.
//!
//! Two halves:
//!
//! * [`resolve_state`] ranks the derived states — one function, so twelve
//!   node kinds and one painter cannot each decide differently whether a
//!   disabled node that is also hovered looks hovered.
//! * [`resolve_slot`] walks `slot@selected-R → slot@selected → slot@R →
//!   slot`, first hit wins.
//!
//! # The binding channel is the ordinary one
//!
//! An author writes state overrides as decorated keys in the *same*
//! `Props.tokens` map every resting binding lives in:
//!
//! ```text
//! background          -> surface.raised
//! background@hover    -> layer-hover
//! background@disabled -> layer-active
//! ```
//!
//! A second map keyed by state name was the other option and was refused: it
//! would need its own validation pass, its own digest contribution and its
//! own painter lookup, all to express something the flat map already
//! expresses with a suffix. As decorated keys, every state binding is checked
//! by `tree::validate`'s existing token check, reaches `PaintContent.tokens`
//! and therefore `PaintState::paint_hash` through the existing dispatcher,
//! and needs nothing new on the wire.
//!
//! [`base_slot`] is what keeps that true for the two consumers that ask
//! "which slot is this": [`super::SlotSchema`] checks a decorated key against
//! its base slot's declared [`TokenKind`](super::TokenKind), and the
//! painter's own known-slot list matches on the base too, so
//! `background@hover` is neither a kind-check hole nor an unknown slot.

use std::collections::BTreeMap;

use crate::frame::PlacementSemantics;

/// The character that separates a slot from the state it is bound for.
///
/// `@` and not `-`: every slot name and every state name is lowercase ASCII
/// with hyphens inside it (`read-only`, `selected-hover`), so a hyphen could
/// not be split on unambiguously. `@` appears in no token name, no slot name
/// and no state name.
pub const STATE_SEPARATOR: char = '@';

/// The rank a node's derived states resolve to
/// (`contracts/interaction-state.md` §4), highest first.
///
/// `focus`, `read-only` and `selected` are **not** here, and their absence is
/// the design:
///
/// * `focus` is additive geometry — the ring composes with every rank, so
///   ranking it would mean a focused hovered button had to choose.
/// * `read-only` forbids [`InteractionRank::Active`] and
///   [`InteractionRank::Hover`] from being entered (§5) but is not itself a
///   rank; a read-only field has no pressed look to rank against.
/// * `selected` picks which token *family* the lookup enters
///   ([`resolve_slot`]); it never competes with rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InteractionRank {
    /// Standing in for content that has not arrived. Outranks everything: a
    /// placeholder is not hoverable, pressable, or disabled-looking.
    Skeleton,
    /// Refusing interaction. Clears hover, active and capture.
    Disabled,
    /// Held down, with the pointer still inside.
    Active,
    /// Under the pointer.
    Hover,
    /// None of the above.
    Enabled,
}

impl InteractionRank {
    /// The suffix this rank contributes to a decorated slot key, or `None`
    /// for [`InteractionRank::Enabled`], which is the undecorated slot.
    #[must_use]
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            Self::Skeleton => Some("skeleton"),
            Self::Disabled => Some("disabled"),
            Self::Active => Some("active"),
            Self::Hover => Some("hover"),
            Self::Enabled => None,
        }
    }
}

/// The six flags [`resolve_state`] and [`resolve_slot`] read off a placement.
///
/// A struct rather than six positional `bool` parameters: six bools in a row
/// is six chances to transpose two of them at a call site, and every one of
/// those transpositions compiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DerivedState {
    /// Standing in for content that has not arrived.
    pub skeleton: bool,
    /// Refusing interaction.
    pub disabled: bool,
    /// Holds the pointer capture with the pointer inside its rect.
    pub active: bool,
    /// Under the pointer.
    pub hovered: bool,
    /// Part of the current selection. Picks the token family, not the rank.
    pub selected: bool,
    /// Shows a value it will not let this author edit.
    pub read_only: bool,
}

impl DerivedState {
    /// Read the six flags off a placement's semantics.
    #[must_use]
    pub fn of(semantics: &PlacementSemantics) -> Self {
        Self {
            skeleton: semantics.skeleton,
            disabled: semantics.disabled,
            active: semantics.active,
            hovered: semantics.hovered,
            selected: semantics.selected,
            read_only: semantics.read_only,
        }
    }
}

/// The one rank `state` resolves to (`contracts/interaction-state.md` §4).
///
/// Read-only suppresses the two interaction ranks rather than becoming a rank
/// of its own (§5): a read-only field still takes focus, still takes hover
/// *events* (a tooltip has to work), and still hit-tests — what it does not
/// have is a hovered or pressed *look*, because there is nothing to press.
/// Folding it into the disabled rank instead was the tempting mistake and is
/// exactly what §5 forbids: disabled leaves the focus tree and read-only does
/// not.
#[must_use]
pub fn resolve_state(state: DerivedState) -> InteractionRank {
    if state.skeleton {
        return InteractionRank::Skeleton;
    }
    if state.disabled {
        return InteractionRank::Disabled;
    }
    if state.read_only {
        return InteractionRank::Enabled;
    }
    if state.active {
        return InteractionRank::Active;
    }
    if state.hovered {
        return InteractionRank::Hover;
    }
    InteractionRank::Enabled
}

/// The base slot a possibly state-decorated key names: everything before the
/// first [`STATE_SEPARATOR`].
///
/// `base_slot("background@selected-hover")` is `"background"`;
/// `base_slot("background")` is `"background"`.
#[must_use]
pub fn base_slot(key: &str) -> &str {
    match key.split_once(STATE_SEPARATOR) {
        Some((base, _)) => base,
        None => key,
    }
}

/// The token bound to `slot` for a node in `state`, following the precedence
/// chain, first hit wins.
///
/// The chain is `slot@selected-R → slot@selected → slot@R → slot`
/// (`contracts/interaction-state.md` §4). Every step is a distinct name a
/// theme may or may not define, and a step nothing bound is skipped rather
/// than blended into the next — Carbon names the combination, and a design
/// system that composited them would produce a `selected-hover` no designer
/// ever chose.
///
/// `R` is [`resolve_state`]'s answer. At [`InteractionRank::Enabled`] the two
/// `@R` steps collapse into the two they would duplicate, so an unselected,
/// resting node costs one lookup.
#[must_use]
pub fn resolve_slot<'a>(
    tokens: &'a BTreeMap<String, String>,
    slot: &str,
    state: DerivedState,
) -> Option<&'a str> {
    let rank = resolve_state(state).as_str();
    let mut candidates: [Option<String>; 3] = [None, None, None];
    let mut len = 0;
    if state.selected {
        if let Some(rank) = rank {
            candidates[len] = Some(format!("{slot}{STATE_SEPARATOR}selected-{rank}"));
            len += 1;
        }
        candidates[len] = Some(format!("{slot}{STATE_SEPARATOR}selected"));
        len += 1;
    }
    if let Some(rank) = rank {
        candidates[len] = Some(format!("{slot}{STATE_SEPARATOR}{rank}"));
        len += 1;
    }
    for candidate in candidates.iter().take(len).flatten() {
        if let Some(found) = tokens.get(candidate) {
            return Some(found.as_str());
        }
    }
    tokens.get(slot).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::{DerivedState, InteractionRank, base_slot, resolve_slot, resolve_state};
    use std::collections::BTreeMap;

    fn bound(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    /// The rank is a total order and the whole of it is reachable.
    ///
    /// Every row sets one extra flag on top of the row above it, so this also
    /// checks that each rank actually *outranks* the ones below rather than
    /// merely being returned when it is the only flag set — which is the
    /// mistake an `if` chain in the wrong order makes and a one-flag-at-a-time
    /// test cannot see.
    #[test]
    fn the_precedence_rank_is_skeleton_disabled_active_hover_enabled() {
        let table: &[(DerivedState, InteractionRank)] = &[
            (DerivedState::default(), InteractionRank::Enabled),
            (
                DerivedState {
                    hovered: true,
                    ..DerivedState::default()
                },
                InteractionRank::Hover,
            ),
            (
                DerivedState {
                    hovered: true,
                    active: true,
                    ..DerivedState::default()
                },
                InteractionRank::Active,
            ),
            (
                DerivedState {
                    hovered: true,
                    active: true,
                    disabled: true,
                    ..DerivedState::default()
                },
                InteractionRank::Disabled,
            ),
            (
                DerivedState {
                    hovered: true,
                    active: true,
                    disabled: true,
                    skeleton: true,
                    ..DerivedState::default()
                },
                InteractionRank::Skeleton,
            ),
        ];
        for (state, want) in table {
            assert_eq!(resolve_state(*state), *want, "{state:?}");
        }
    }

    /// `selected` is orthogonal to rank: it changes which family the lookup
    /// enters and never which rank wins.
    #[test]
    fn selected_does_not_enter_the_precedence_rank() {
        for hovered in [false, true] {
            let plain = DerivedState {
                hovered,
                ..DerivedState::default()
            };
            let picked = DerivedState {
                selected: true,
                ..plain
            };
            assert_eq!(resolve_state(plain), resolve_state(picked));
        }
    }

    /// Read-only suppresses the two interaction ranks and nothing else. It is
    /// not a weaker `disabled`: a read-only node that is *also* disabled
    /// ranks disabled, because disabled is a real rank and read-only is not.
    #[test]
    fn read_only_suppresses_hover_and_active_without_becoming_disabled() {
        let state = DerivedState {
            read_only: true,
            hovered: true,
            active: true,
            ..DerivedState::default()
        };
        assert_eq!(resolve_state(state), InteractionRank::Enabled);
        assert_eq!(
            resolve_state(DerivedState {
                disabled: true,
                ..state
            }),
            InteractionRank::Disabled
        );
        assert_eq!(
            resolve_state(DerivedState {
                skeleton: true,
                ..state
            }),
            InteractionRank::Skeleton
        );
    }

    /// The four-step chain, walked from the top: with every step bound, each
    /// state picks exactly the step the contract names for it.
    #[test]
    fn the_precedence_chain_takes_the_first_step_that_is_bound() {
        let tokens = bound(&[
            ("background", "surface.raised"),
            ("background@hover", "layer-hover"),
            ("background@selected", "layer-selected"),
            ("background@selected-hover", "layer-selected-hover"),
        ]);
        let hovered = DerivedState {
            hovered: true,
            ..DerivedState::default()
        };
        let selected = DerivedState {
            selected: true,
            ..DerivedState::default()
        };
        let both = DerivedState {
            hovered: true,
            selected: true,
            ..DerivedState::default()
        };

        assert_eq!(
            resolve_slot(&tokens, "background", DerivedState::default()),
            Some("surface.raised")
        );
        assert_eq!(
            resolve_slot(&tokens, "background", hovered),
            Some("layer-hover")
        );
        assert_eq!(
            resolve_slot(&tokens, "background", selected),
            Some("layer-selected")
        );
        assert_eq!(
            resolve_slot(&tokens, "background", both),
            Some("layer-selected-hover"),
            "Carbon names the combination; the chain must find the named one \
             rather than falling back to either half"
        );
    }

    /// A missing step falls through to the next one down, and never blends.
    ///
    /// `selected-hover` unbound falls to `selected`, not to `hover`: the
    /// selected family is the one the node is in, and dropping out of it on
    /// hover would make a selected row stop looking selected the moment the
    /// pointer touched it.
    #[test]
    fn an_unbound_step_falls_to_the_next_step_and_never_blends() {
        let tokens = bound(&[
            ("background", "surface.raised"),
            ("background@hover", "layer-hover"),
            ("background@selected", "layer-selected"),
        ]);
        let both = DerivedState {
            hovered: true,
            selected: true,
            ..DerivedState::default()
        };
        assert_eq!(
            resolve_slot(&tokens, "background", both),
            Some("layer-selected")
        );

        let sparse = bound(&[("background", "surface.raised")]);
        assert_eq!(
            resolve_slot(&sparse, "background", both),
            Some("surface.raised")
        );
        assert_eq!(resolve_slot(&sparse, "border", both), None);
    }

    /// A state binding for one slot never leaks into another slot.
    #[test]
    fn a_state_binding_is_scoped_to_its_own_slot() {
        let tokens = bound(&[
            ("background", "surface.raised"),
            ("background@hover", "layer-hover"),
            ("foreground", "text.primary"),
        ]);
        let hovered = DerivedState {
            hovered: true,
            ..DerivedState::default()
        };
        assert_eq!(
            resolve_slot(&tokens, "foreground", hovered),
            Some("text.primary")
        );
    }

    #[test]
    fn base_slot_strips_exactly_one_decoration() {
        assert_eq!(base_slot("background"), "background");
        assert_eq!(base_slot("background@hover"), "background");
        assert_eq!(base_slot("background@selected-hover"), "background");
        assert_eq!(base_slot("@hover"), "");
    }
}
