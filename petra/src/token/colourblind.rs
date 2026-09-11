//! The red-green colour-blindness simulation lane, shared by the token gates
//! in [`super::shipped`] and the component gates in
//! [`crate::component::colourblind`].
//!
//! Test-only: this module is `#[cfg(test)]` and nothing here ships.
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
//! Both levels therefore call one `simulate`, one `to_lab`, one `delta_e` and
//! one [`MIN_STATUS_SEPARATION`]. A second copy of this arithmetic is how a
//! gate quietly stops measuring what its name says.

use crate::token::name::TokenName;
use crate::token::value::{ColorValue, TokenValue};

/// The floor every pair of shipped status colours must clear, in CIE
/// ΔE*ab, after the frame is simulated through red-green colour
/// blindness. 30 is chosen as "two colours a reader will not confuse at a
/// glance"; for scale, the palette this replaced scored 5.5 in dark mode
/// and 8.8 in light, which is the same colour twice.
pub(crate) const MIN_STATUS_SEPARATION: f32 = 30.0;

/// The floor a status colour must clear against the surface it is painted
/// on, as a WCAG contrast ratio. Without this a palette could win the
/// separation test by being invisible in three different ways.
pub(crate) const MIN_SURFACE_CONTRAST: f32 = 3.0;

/// Viénot-Brettel-Mollon 1999 reduced matrices, applied to *linear* RGB.
/// Deuteranopia (no green cone) and protanopia (no red cone) are both
/// simulated because "red-green colour blind" covers both and they do not
/// collapse the same pairs.
///
/// These are not transcribed from a blog post. They are the composition
/// `LMS→RGB · reduce · RGB→LMS` of the three matrices the 1999 paper
/// publishes (Eqs. 4-6), and `shipped`'s own
/// `the_simulation_matrices_are_the_ones_the_paper_derives` recomposes
/// them from those equations and fails on a changed digit. The matrix
/// this file shipped before was the "colorjack ColorMatrix" deuteranope
/// matrix (`0.625/0.375`, `0.700/0.300`, `0/0.300/0.700`), whose own
/// author disclaims it as inaccurate, plus an untraceable `1/6, 5/6`
/// protanope matrix. That pair did not simulate what the comment claimed:
/// pure red and pure green stayed ΔE\*ab 88.4 apart under it, against
/// 30.4 under the real transform, so the separation gate was scoring the
/// palette for a reader who can still tell red from green.
pub(crate) const DEUTERANOPE: [[f32; 3]; 3] = [
    [0.29275, 0.70725, 0.0],
    [0.29275, 0.70725, 0.0],
    [-0.02234, 0.02234, 1.0],
];

/// Protanope half of [`DEUTERANOPE`]'s pair, derived the same way and
/// checked by the same recomposition test.
pub(crate) const PROTANOPE: [[f32; 3]; 3] = [
    [0.11238, 0.88762, 0.0],
    [0.11238, 0.88762, 0.0],
    [0.00401, -0.00401, 1.0],
];

/// Both simulations, labelled, so a caller sweeps the pair rather than
/// remembering to name the second one.
pub(crate) const VISIONS: [(&str, &[[f32; 3]; 3]); 2] =
    [("deuteranope", &DEUTERANOPE), ("protanope", &PROTANOPE)];

/// `c` as a dichromat with deficiency `m` sees it.
pub(crate) fn simulate(c: ColorValue, m: &[[f32; 3]; 3]) -> [f32; 3] {
    let v = [c.r, c.g, c.b];
    std::array::from_fn(|i| (m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2]).clamp(0.0, 1.0))
}

/// Linear sRGB to CIE L\*a\*b\* under D65, the space ΔE\*ab is defined in.
pub(crate) fn to_lab(v: [f32; 3]) -> [f32; 3] {
    const M: [[f32; 3]; 3] = [
        [0.412_456_4, 0.357_576_1, 0.180_437_5],
        [0.212_672_9, 0.715_152_2, 0.072_175],
        [0.019_333_9, 0.119_192, 0.950_304_1],
    ];
    const WHITE: [f32; 3] = [0.950_47, 1.0, 1.088_83];
    let f: [f32; 3] = std::array::from_fn(|i| {
        let t = (M[i][0] * v[0] + M[i][1] * v[1] + M[i][2] * v[2]) / WHITE[i];
        if t > 0.008_856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    });
    [
        116.0 * f[1] - 16.0,
        500.0 * (f[0] - f[1]),
        200.0 * (f[1] - f[2]),
    ]
}

/// Euclidean distance in L\*a\*b\*, which is what ΔE\*ab is.
pub(crate) fn delta_e(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d: [f32; 3] = std::array::from_fn(|i| a[i] - b[i]);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// How far apart two colours read to a dichromat with deficiency `m`, in
/// ΔE\*ab. The composition every caller of this lane wants, so no caller
/// has to remember that the simulation comes before the conversion.
pub(crate) fn separation(a: ColorValue, b: ColorValue, m: &[[f32; 3]; 3]) -> f32 {
    delta_e(to_lab(simulate(a, m)), to_lab(simulate(b, m)))
}

/// One definition of "how far apart do these two read", shared with every
/// other consumer. It used to live here as a private pair of helpers,
/// which meant the FR-010 gate in `gorgon-petra-egui` — the one that has
/// to composite through `Props.opacity` before it measures anything —
/// would have had to carry a second copy of the same arithmetic.
pub(crate) fn contrast(a: ColorValue, b: ColorValue) -> f32 {
    a.contrast_ratio(b)
}

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
