//! The red-green colour-blindness simulation lane, shared by the token
//! gates in [`super::shipped`] and the component gates in
//! [`crate::component::colourblind`].
//!
//! Test-only: this module is `#[cfg(test)]` and nothing *here* ships. The
//! arithmetic it gates on lives in [`super::measure`] (spec 008 T038 moved
//! it: the warning/error pair has to be measurable outside the test build),
//! and is re-exported below so both gate levels still call one copy.
//!
//! It exists because the lane grew a second level. The token gates ask
//! whether two *names* in the shipped vocabulary stay apart to a dichromat.
//! That question is necessary and it is not sufficient: a component binds a
//! name, and nothing in a name-level sweep can see a component that binds a
//! colour from outside the family the sweep walks, or one that drops the
//! shape and text channels the colour was only ever meant to accompany. The
//! component gates ask the second question, and they have to measure ΔE the
//! same way or the two levels are two different floors wearing one number.
//!
//! Both levels therefore call one `simulate`, one `to_lab`, one `delta_e`
//! and one [`MIN_STATUS_SEPARATION`]. A second copy of this arithmetic is
//! how a gate quietly stops measuring what its name says.

use crate::token::name::TokenName;
use crate::token::value::{ColorValue, TokenValue};

pub(crate) use crate::token::measure::{
    DEUTERANOPE, MIN_STATUS_SEPARATION, MIN_SURFACE_CONTRAST, PROTANOPE, VISIONS, contrast,
    delta_e, separation, simulate, to_lab,
};

/// `token`'s colour in `theme`.
///
/// # Panics
/// When `token` is not a well-formed name, is absent from the theme, or
/// names something that is not a colour. All three are test-authoring
/// mistakes rather than states a gate should tolerate.
pub(crate) fn theme_color(theme: &crate::token::Theme, token: &str) -> ColorValue {
    match theme.value(&TokenName::new(token).unwrap()).unwrap() {
        TokenValue::Color(c) => *c,
        other => panic!("{token} is not a colour: {other:?}"),
    }
}

/// Every colour token in the status or support families, read off the
/// live vocabulary rather than listed here.
///
/// **This is the whole of the widening, and the hole it closes is
/// specific.** The separation gate used to iterate three literal names. The
/// Carbon inventory finds `$support-error` (12 references),
/// `$support-success` (8) and `$support-warning` (4) across the 42
/// components — 24 references to a family that is status-shaped, alert-
/// coloured, and was invisible to a three-name loop. A Notification
/// declaring its own `support.error` in Carbon's `#da1e28` would have
/// walked straight past a gate whose entire job is to keep that palette
/// out, because Carbon's alert colours fail `MIN_STATUS_SEPARATION` under
/// all three colour-blindness models tested (tightest pair ΔE\*ab 12.5).
///
/// Reading the vocabulary means the *next* status-shaped family is caught
/// on the day it is declared rather than on the day somebody remembers to
/// add it to a list here.
///
/// # Panics
/// When no status-family token is declared at all, which would mean the
/// gate is passing by iterating nothing.
pub(crate) fn status_family() -> Vec<String> {
    let vocab = crate::token::shipped::standard_vocabulary();
    let names: Vec<String> = vocab
        .names()
        .filter(|n| vocab.kind_of(n) == Some(crate::token::value::TokenKind::Color))
        .map(|n| n.as_str().to_owned())
        .filter(|n| n.starts_with("status.") || n.starts_with("support-"))
        .collect();
    assert!(
        names.len() >= 6,
        "expected at least the three statuses and their three support \
         aliases, found {names:?} — a gate that iterates nothing passes \
         for the wrong reason"
    );
    names
}
