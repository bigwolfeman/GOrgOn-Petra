//! Contrast as drawn, not as declared (spec 005 FR-010, spec 008 FR-023).
//!
//! Every contrast assertion that compares two *token values* is true about
//! the theme and stops being true about the screen the moment anything
//! upstream composites the node: `Props.opacity` fades a whole subtree at
//! paint time, so a label declared at 10.7:1 and drawn at 45% opacity
//! reaches the reader at about 3.4:1 and every value-level check still
//! passes. This module is the measurement that reads through that fade —
//! and through the state slot the painter would resolve, so a hovered or
//! disabled node is measured in the family it is actually painted in.
//!
//! One home for the compositing, used by `petra-egui`'s text-contrast gate
//! and by the gallery's disabled-state cases (`spec 008` T027): a gate and a
//! per-state test that computed their own fades could disagree about what
//! "as drawn" means while both stayed green.
//!
//! The second half of this file is the colour-blindness arithmetic
//! (`spec 008` T038's measurement home): one `simulate`, one `to_lab`, one
//! `delta_e`, and the simulation matrices, moved here from
//! [`crate::token::colourblind`] when the warning/error pair had to be
//! measured outside the test build. `colourblind` re-exports them, so the
//! token and component gates still call one copy. A second copy of this
//! arithmetic is how a gate quietly stops measuring what its name says.
//!
//! # The two holes this closes
//!
//! * spec 005 FR-010: `examples/gallery.rs` expressed "unavailable" by
//!   fading a subtree. The suite could not see it.
//! * spec 008 FR-023: disabled labels were exempt from the floor and held
//!   to a different rule, which meant nothing measured them at all. The
//!   disabled treatment binds `text.muted` (`contracts/disabled.md`), so
//!   they now clear 4.5:1 *as drawn* and the exemption can go.

use std::collections::BTreeMap;

use crate::frame::PetrifiedFrame;
use crate::token::state::{DerivedState, resolve_slot};
use crate::token::theme::Theme;
use crate::token::value::{ColorValue, TokenValue};

/// The ink slot a text placement paints from.
const FOREGROUND_SLOT: &str = "foreground";

/// The ground slot a node paints its fill from.
const BACKGROUND_SLOT: &str = "background";

/// What the painter draws when a text placement binds no ink slot.
const DEFAULT_TEXT_TOKEN: &str = "text.primary";

/// The ground a page paints on when nothing above a node binds one.
const PAGE: &str = "surface.base";

fn colour(theme: &Theme, token: &str) -> ColorValue {
    let name = crate::token::TokenName::new(token).unwrap_or_else(|err| panic!("`{token}`: {err}"));
    match theme.value(&name) {
        Some(TokenValue::Color(c)) => *c,
        other => panic!("`{token}` is not a colour in this theme: {other:?}"),
    }
}

/// One node's resolved ink and ground, already faded and composited — what a
/// reader actually sees.
///
/// The ground is the nearest ancestor that binds one, resolved in *its* own
/// state — a hovered row repaints its fill, and its label is read against the
/// fill that is actually there. Walking rather than assuming the parent,
/// because a label's parent is often a bare layout box. Both halves are
/// faded by the cumulative opacity the painter composited them under.
#[must_use]
pub fn drawn_pair(frame: &PetrifiedFrame, theme: &Theme, index: usize) -> (ColorValue, ColorValue) {
    let placement = &frame.placements[index];
    let state = DerivedState::of(&placement.semantics);
    let ink = colour(
        theme,
        resolve_slot(&frame.content[index].tokens, FOREGROUND_SLOT, state)
            .unwrap_or(DEFAULT_TEXT_TOKEN),
    );

    let mut ground = colour(theme, PAGE);
    let mut ground_opacity = 1.0;
    let mut cursor = Some(index);
    while let Some(at) = cursor {
        let node = &frame.placements[at];
        if let Some(token) = resolve_slot(
            &frame.content[at].tokens,
            BACKGROUND_SLOT,
            DerivedState::of(&node.semantics),
        ) {
            ground = colour(theme, token);
            ground_opacity = node.opacity;
            break;
        }
        cursor = node.parent;
    }

    let page = colour(theme, PAGE);
    let ground = ground.faded(ground_opacity).over(page);
    let ink = ink.faded(placement.opacity).over(ground);
    (ink, ground)
}

/// Every text placement, measured as [`drawn_pair`] measures it: `(id,
/// ratio, opacity)` per label, in frame order.
#[must_use]
pub fn label_contrasts(frame: &PetrifiedFrame, theme: &Theme) -> Vec<(String, f32, f32)> {
    let mut out = Vec::new();
    for (index, placement) in frame.placements.iter().enumerate() {
        if frame.content[index].text.is_none() {
            continue;
        }
        let (ink, ground) = drawn_pair(frame, theme, index);
        out.push((
            placement.id.clone(),
            ink.contrast_ratio(ground),
            placement.opacity,
        ));
    }
    out
}

/// Convenience for gates that hold a map rather than a frame: the token
/// names one node binds, keyed by slot, as [`resolve_slot`] wants them.
#[must_use]
pub fn slot_map(tokens: &BTreeMap<String, crate::token::TokenName>) -> BTreeMap<String, String> {
    tokens
        .iter()
        .map(|(slot, name)| (slot.clone(), name.as_str().to_owned()))
        .collect()
}

// ---------------------------------------------------------------------------
// Colour-blindness simulation: the pair separation (spec 008 FR-020)
// ---------------------------------------------------------------------------

/// The floor every pair of shipped status colours must clear, in CIE
/// ΔE\*ab, after the frame is simulated through red-green colour
/// blindness. 30 is chosen as "two colours a reader will not confuse at a
/// glance"; for scale, the palette this replaced scored 5.5 in dark mode
/// and 8.8 in light, which is the same colour twice.
pub const MIN_STATUS_SEPARATION: f32 = 30.0;

/// The floor a status colour must clear against the surface it is painted
/// on, as a WCAG contrast ratio. Without this a palette could win the
/// separation test by being invisible in three different ways.
pub const MIN_SURFACE_CONTRAST: f32 = 3.0;

/// Viénot-Brettel-Mollon 1999 reduced matrices, applied to *linear* RGB.
/// Deuteranopia (no green cone) and protanopia (no red cone) are both
/// simulated because "red-green colour blind" covers both and they do not
/// collapse the same pairs.
///
/// These are not transcribed from a blog post. They are the composition
/// `LMS→RGB · reduce · RGB→LMS` of the three matrices the 1999 paper
/// publishes (Eqs. 4-6), and `shipped`'s own
/// `the_simulation_matrices_are_the_ones_the_paper_derives` recomposes
/// them from those equations and fails on a changed digit. See
/// `.agents/notes/implemented/bug-fix/2026-08-24-colourblind-simulation-matrices.md`
/// for the pair these replaced and what that cost the gate.
pub const DEUTERANOPE: [[f32; 3]; 3] = [
    [0.29275, 0.70725, 0.0],
    [0.29275, 0.70725, 0.0],
    [-0.02234, 0.02234, 1.0],
];

/// Protanope half of [`DEUTERANOPE`]'s pair, derived the same way and
/// checked by the same recomposition test.
pub const PROTANOPE: [[f32; 3]; 3] = [
    [0.11238, 0.88762, 0.0],
    [0.11238, 0.88762, 0.0],
    [0.00401, -0.00401, 1.0],
];

/// Machado, Oliveira & Fernandes 2009 deuteranopia at severity 1.0 (full
/// dichromacy), applied to *linear* RGB.
///
/// **SOURCED**, published severity-1.0 matrix from Machado's own project
/// page (IEEE TVCG 15(6):1291-1298, 2009), recorded verbatim in
/// `.agents/research/08-24-2026/Design-Language/colour-construction.md` §5.
/// This is the third simulation model `token-vocabulary.md`'s gate names
/// ("Machado 2009, the repo's matrices, VBM 1999"). The 2026-08-24 note
/// rejected Machado as the *gate's* model on scope; T038 of spec 008 puts
/// it back as a *measurement* — the warning/error pair is the palette's
/// tightest pair (dark, deuteranope, `degraded` vs `down`, ΔE\*ab 30.57
/// against this floor), the operator is red-green colour blind, and one
/// independent model family is the cheapest way to know the margin is not
/// an artefact of the transform that produced it.
pub const MACHADO_DEUTERANOPE: [[f32; 3]; 3] = [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.011820, 0.042940, 0.968881],
];

/// Machado 2009 protanopia at severity 1.0, same source and the same
/// reasoning as [`MACHADO_DEUTERANOPE`].
pub const MACHADO_PROTANOPE: [[f32; 3]; 3] = [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
];

/// Both red-green dichromacies under Viénot-Brettel-Mollon 1999, labelled,
/// so a caller sweeps the pair rather than remembering to name the second
/// one. What the token and component gates have always swept.
pub const VISIONS: [(&str, &[[f32; 3]; 3]); 2] =
    [("deuteranope", &DEUTERANOPE), ("protanope", &PROTANOPE)];

/// The three simulation models a measurement sweeps, as four labelled
/// matrices: the two dichromacies under each model family.
///
/// "The repo's matrices" *are* the VBM 1999 composition — that identity is
/// the whole finding of the 2026-08-24 note, pinned by
/// `the_simulation_matrices_are_the_ones_the_paper_derives` — so the
/// three named models (`token-vocabulary.md`: "Machado 2009, the repo's
/// matrices, VBM 1999") resolve to two independent transforms. The label
/// says so rather than letting a reader count four rows as four models.
pub const SIMULATION_MODELS: [(&str, &[[f32; 3]; 3]); 4] = [
    ("VBM 1999 deuteranope (= the repo's matrix)", &DEUTERANOPE),
    ("VBM 1999 protanope (= the repo's matrix)", &PROTANOPE),
    ("Machado 2009 deuteranopia, severity 1.0", &MACHADO_DEUTERANOPE),
    ("Machado 2009 protanopia, severity 1.0", &MACHADO_PROTANOPE),
];

/// `c` as a dichromat with deficiency `m` sees it. The matrix applies to
/// `ColorValue`'s linear-light channels (`token/value.rs` stores linear).
pub fn simulate(c: ColorValue, m: &[[f32; 3]; 3]) -> [f32; 3] {
    let v = [c.r, c.g, c.b];
    std::array::from_fn(|i| (m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2]).clamp(0.0, 1.0))
}

/// Linear sRGB to CIE L\*a\*b\* under D65, the space ΔE\*ab is defined in.
pub fn to_lab(v: [f32; 3]) -> [f32; 3] {
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
pub fn delta_e(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d: [f32; 3] = std::array::from_fn(|i| a[i] - b[i]);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// How far apart two colours read to a dichromat with deficiency `m`, in
/// ΔE\*ab. The composition every caller of this lane wants, so no caller
/// has to remember that the simulation comes before the conversion.
pub fn separation(a: ColorValue, b: ColorValue, m: &[[f32; 3]; 3]) -> f32 {
    delta_e(to_lab(simulate(a, m)), to_lab(simulate(b, m)))
}

/// One definition of "how far apart do these two read", shared with every
/// other consumer. It used to live here as a private pair of helpers,
/// which meant the FR-010 gate in `gorgon-petra-egui` — the one that has
/// to composite through `Props.opacity` before it measures anything —
/// would have had to carry a second copy of the same arithmetic.
pub fn contrast(a: ColorValue, b: ColorValue) -> f32 {
    a.contrast_ratio(b)
}

#[cfg(test)]
mod tests {
    use super::{MIN_STATUS_SEPARATION, SIMULATION_MODELS, separation};
    use crate::token::colourblind::{status_family, theme_color};
    use crate::token::shipped::{dark, light};
    use crate::token::value::ColorValue;

    /// T038 (spec 008 FR-020, spec 005 FR-005): the warning/error pair,
    /// measured against the shipped ΔE\*ab floor under all three
    /// simulation models, before any component state is built on it.
    ///
    /// The pair is `support-warning` against `support-error` — the two
    /// colours the form-family treatments bind — and "against its
    /// neighbours" is the rest of the status family, swept pairwise the
    /// way `shipped`'s name-level gate sweeps them. What this test adds
    /// over that gate is the third model family (Machado 2009,
    /// [`SIMULATION_MODELS`]): the gate's two models are one transform,
    /// and this pair clears its floor in dark mode by 0.57 ΔE\*ab, which
    /// is thin enough that the margin is measured against an independent
    /// family before it is trusted.
    ///
    /// MEASURED 2026-10-25, both themes, 96 pairs swept, floor 30.0. The
    /// warning/error pair *is* the family's tightest pair in every model —
    /// exactly the pair the operator's red-green colour blindness puts
    /// first — and it clears everywhere:
    ///
    /// | model | light | dark |
    /// |---|---|---|
    /// | VBM 1999 deuteranope (= the repo's matrix) | 51.05 | **30.57** |
    /// | VBM 1999 protanope (= the repo's matrix) | 46.09 | 42.13 |
    /// | Machado 2009 deuteranopia, severity 1.0 | 48.86 | **31.64** |
    /// | Machado 2009 protanopia, severity 1.0 | 48.53 | 40.40 |
    ///
    /// The tightest reading, dark deuteranope, is the one the 2026-08-24
    /// matrix note predicted (30.57, "treat that pair as at the floor"),
    /// and Machado's independent transform lands 1.07 ΔE\*ab away from it,
    /// so the margin is not an artefact of the model that produced it.
    /// No divergence is recorded against FR-020: the pair clears the
    /// floor under all three models, so the treatment's shape is decided
    /// by T039's non-colour bar alone.
    #[test]
    fn the_status_family_clears_the_floor_under_all_three_simulation_models() {
        for (label, theme) in [("light", light()), ("dark", dark())] {
            let names = status_family();
            let colors: Vec<ColorValue> = names.iter().map(|n| theme_color(&theme, n)).collect();
            for (model, matrix) in SIMULATION_MODELS {
                for i in 0..names.len() {
                    for j in (i + 1)..names.len() {
                        // `support-error` *is* `status.down`: two names for
                        // one colour, ΔE 0, nothing to confuse.
                        if colors[i] == colors[j] {
                            continue;
                        }
                        let d = separation(colors[i], colors[j], matrix);
                        assert!(
                            d >= MIN_STATUS_SEPARATION,
                            "{label}: {} and {} are only ΔE*ab {d:.2} apart to \
                             {model} (floor is {MIN_STATUS_SEPARATION})",
                            names[i],
                            names[j],
                        );
                        // The measurement this task names, printed so a run
                        // re-quotes the table rather than trusting this doc.
                        if names[i] == "support-error" && names[j] == "support-warning" {
                            println!(
                                "T038 {label:5} | {model:42} | support-error vs \
                                 support-warning | ΔE*ab {d:6.2}"
                            );
                        }
                    }
                }
            }
        }
    }
}
