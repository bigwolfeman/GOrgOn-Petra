//! The rule material: a two-stroke groove, not a colour.
//!
//! Settled by operator sight-test, 2026-09-07, and written into
//! `specs/009-petra-design-language/contracts/design-language.md`'s "The
//! construction". A rule is not a flat line — it is a shadow stroke with a
//! highlight stroke directly against it, so it reads as pressed into the
//! surface rather than drawn on it.
//!
//! # Where the light comes from
//!
//! **Above and to the left.** Operator decision, 2026-09-09, from specimens.
//! The original construction said "from the top", which is the same thing for
//! a horizontal rule: the upper wall turns away from the light and the lower
//! wall turns toward it, so the shipped 28-and-13 pair is unchanged and the
//! shadow is still the physically higher stroke.
//!
//! Straight overhead was what excluded vertical rules, and the exclusion was
//! real: a vertical groove lit from directly above has both walls at the same
//! angle to the light, which is a uniform darkening and not a bevel. Moving
//! the light to the top-left costs the horizontal case nothing and gives the
//! vertical case a shadow side (left) and a highlight side (right), the same
//! two strokes at the same two levels. Measured on specimens in both shipped
//! themes: 6 then 47 on the dark panel, 214 then 255 on the light one,
//! identical to the horizontal pair.
//!
//! # Why this lives beside [`super::focus`] rather than inside [`super::shipped`]
//!
//! Same shape as that module's own reason: how thick a stroke is and which
//! one sits on top is a design decision a second renderer must reproduce
//! (D-069 keeps [`super::shipped`]'s crate free of any toolkit), so the
//! *geometry* — how many strokes, which order, how many units each — lives
//! here as plain constants, while the *colour* each stroke resolves to is an
//! ordinary theme token declared in [`super::shipped`] and looked up by the
//! painter through [`SHADOW_TOKEN`] and [`HIGHLIGHT_TOKEN`], exactly the way
//! [`super::focus::BAR_SHADOW_TOKEN`] names a token rather than carrying a
//! colour of its own.
//!
//! # The trigger token
//!
//! [`MATERIAL_TOKEN`] is the same string as the shipped theme's decorative
//! `border.subtle` tone. The painter grooves when (and only when) the token
//! bound is this one; `border.strong` (the control-boundary tone: the
//! checkbox square, the radio ring, the toggle track, a field's bottom rule)
//! and every state token (focus, error, warning, selection) keep painting as
//! the flat single stroke they always have, because they are not a "rule" in
//! this document's sense — see the contract: "State lines stay flat single
//! strokes drawn over the top." A test in `super::shipped` holds the two
//! strings equal so this cannot drift.
//!
//! # Two constructions, one material
//!
//! A hairline is written two ways in this codebase, and both are legitimate:
//!
//! 1. **An edge slot.** "This node's edge is a rule." `border-top`,
//!    `border-bottom`, `border-left` or `border-right` bound to
//!    [`MATERIAL_TOKEN`]. This is what a table row, a field and a list option
//!    use, where the rule belongs to something that has other content.
//! 2. **A separator.** "This node *is* a rule."
//!    [`crate::tree::NodeKind::Separator`] with [`MATERIAL_TOKEN`] as its
//!    `background`. This is what a standalone divider uses, and it takes its
//!    thickness from [`thickness`] rather than from a number at the call site
//!    — which is the point. Before 2026-09-09 seven dividers each pinned
//!    their own one logical unit and painted a flat fill, so the same token
//!    produced a groove in a table and a flat line in an accordion.
//!
//! # Why a node never grooves a corner
//!
//! The contract's "never meet a corner" mandate used to be satisfied for free:
//! rules were horizontal only, so no stroke could reach a corner. Vertical
//! rules remove that guarantee, and a node binding, say, `border-top` and
//! `border-left` to the material would draw two walls of a bevelled box —
//! the retro tell the contract refuses.
//!
//! So the painter refuses instead: a node whose material-bound edges are
//! **perpendicular** grooves none of them and draws flat lines. A parallel
//! pair (`border-top` with `border-bottom`) is not a corner and still
//! grooves. See [`corner_free`].

/// The bound token that turns an edge slot into a groove. The same string as
/// the shipped theme's decorative border tone ("border.subtle") by
/// construction; see the module doc for why this is a second constant
/// rather than a re-export.
pub const MATERIAL_TOKEN: &str = "border.subtle";

/// Token naming the groove's shadow stroke: a signed sRGB level offset from
/// the panel it sits on, clamped per channel — never blended toward an
/// extreme. Computed and assigned in `super::shipped`.
pub const SHADOW_TOKEN: &str = "rule.shadow";

/// Token naming the groove's highlight stroke, directly below the shadow.
pub const HIGHLIGHT_TOKEN: &str = "rule.highlight";

/// Height of the shadow stroke, in **absolute device pixels** — not logical
/// points. Operator decision, 2026-09-07: a stroke anchored to the physical
/// pixel grid never lands on a fractional boundary and never blurs. The
/// accepted cost is that the groove's apparent size falls as display density
/// rises; do not convert this to logical units to "fix" that.
pub const SHADOW_UNITS: f32 = 2.0;

/// Height of the highlight stroke, directly below the shadow. Equal to
/// [`SHADOW_UNITS`] — thickness moves in pairs, by operator decision: a
/// top-heavy pair was offered three ways and rejected all three times, so a
/// future author who thickens one stroke thickens the other exactly as much.
pub const HIGHLIGHT_UNITS: f32 = 2.0;

/// Total groove height in absolute device pixels: [`SHADOW_UNITS`] +
/// [`HIGHLIGHT_UNITS`].
#[must_use]
pub const fn total_units() -> f32 {
    SHADOW_UNITS + HIGHLIGHT_UNITS
}

/// The groove's thickness in **logical units** at `scale`.
///
/// [`SHADOW_UNITS`] and [`HIGHLIGHT_UNITS`] are absolute device pixels by
/// operator decision, so the logical extent a layout must reserve for a
/// groove falls as display density rises. This is the one conversion; a
/// caller that hardcodes a number instead is the defect this function
/// exists to remove.
///
/// [`crate::layout::leaf::SEPARATOR_THICKNESS`] is what a separator measures
/// when its token is *not* the material, and the two are deliberately
/// different: a flat line is one logical unit at any density, because it is
/// a line and not a machined edge.
#[must_use]
pub fn thickness(scale: crate::geom::Scale) -> f32 {
    total_units() / scale.factor()
}

/// Whether the material-bound edges of one node can groove without meeting
/// at a corner.
///
/// The contract's "never meet a corner" mandate was satisfied for free while
/// rules were horizontal only: no stroke could reach a corner, so nothing
/// had to check. Vertical rules remove that, and a node binding a horizontal
/// edge and a vertical edge to [`MATERIAL_TOKEN`] would draw two walls of a
/// bevelled box, which the contract refuses as the retro tell.
///
/// A parallel pair is not a corner: `border-top` with `border-bottom` is a
/// strip ruled above and below, and both grooves are wanted. Only a mix of
/// orientations is refused, and the refusal is to draw flat lines rather
/// than to draw nothing — the author asked for a line and gets one.
#[must_use]
pub fn corner_free(top: bool, right: bool, bottom: bool, left: bool) -> bool {
    !((top || bottom) && (left || right))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Scale;

    /// The whole reason [`thickness`] exists: a separator that reserved one
    /// logical unit painted four device pixels of groove into it. Seven
    /// dividers shipped that way.
    #[test]
    fn the_thickness_is_the_whole_groove_not_one_unit() {
        assert_eq!(thickness(Scale::ONE), 4.0);
        assert_eq!(thickness(Scale::new(2.0).expect("2.0 is a scale")), 2.0);
        assert_eq!(total_units(), 4.0);
    }

    /// A mixed pair is a corner and is refused; a parallel pair is not.
    #[test]
    fn only_perpendicular_material_edges_refuse_to_groove() {
        assert!(corner_free(true, false, false, false), "one edge alone");
        assert!(corner_free(true, false, true, false), "top and bottom");
        assert!(corner_free(false, true, false, true), "left and right");
        assert!(!corner_free(true, false, false, true), "top and left");
        assert!(!corner_free(false, true, true, false), "bottom and right");
        assert!(!corner_free(true, true, true, true), "all four is the box");
    }
}
