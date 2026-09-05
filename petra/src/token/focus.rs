//! The focus indicator: an underline below the node, and why it is not a colour.
//!
//! FR-015's rule is that no Petra-provided status may carry meaning by colour
//! alone, and keyboard focus is a status like any other. The indicator
//! answers that with a **shape that is present or absent**: a focused node
//! grows a bar [`FocusRing::thickness`] logical units thick, [`FocusRing::gap`]
//! below its bottom edge, that an unfocused node does not have. Print the
//! frame in greyscale and the bar is still there.
//!
//! The bar is painted in `accent.primary` (bound here as [`RING_TOKEN`]) and
//! sits *under* the node, centred at two thirds of the node's width, so it
//! never enters the rounded fill and cannot clip a corner. Full-width was
//! too long on a 2026-08-26 crop of Save.
//!
//! Contrast is against the card the bar sits on, not against the node's own
//! fill. `the_focus_underline_is_legible_on_the_card` measures that.
//!
//! The geometry lives here, in the engine, and not in `gorgon-petra-egui`:
//! how thick the bar is and where it sits relative to the node is a design
//! decision a second renderer must reproduce, and D-069 keeps this crate free
//! of any toolkit. What the adapter crate owns is the drawing.

use crate::geom::Rect;

/// Token naming the underline's fill. Bound to the theme's accent, so the
/// bar is the same blue as the primary button, not a second hue.
pub const RING_TOKEN: &str = "focus.ring";

/// Token naming the unused paper halo. Kept in the vocabulary because
/// `focus-inverse` and the theme builder still declare the pair; the
/// underline does not paint it.
pub const HALO_TOKEN: &str = "focus.ring-halo";

/// The focus underline's measurements, in logical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusRing {
    /// Height of the underline, and width of a hug bar.
    pub thickness: f32,
    /// Gap between the node's bottom edge and the underline's top edge.
    pub gap: f32,
    /// Gap between a well's left or right edge and the hug bar beside it.
    ///
    /// Wider than [`Self::gap`]. An underline sits in the empty run under a
    /// pill; a hug bar stands beside a filled well whose edge is a hard
    /// colour step, and at two units the bar read as part of that edge —
    /// the operator's "cramped" and "needs a bit of padding" on the Search
    /// and Number input rows, 2026-09-05.
    pub hug_gap: f32,
}

impl Default for FocusRing {
    fn default() -> Self {
        Self::STANDARD
    }
}

impl FocusRing {
    /// The shipped underline: three units thick, two units below the node,
    /// two thirds of the node's width, centred.
    ///
    /// Three, not two: the 2-unit strip was a hairline under a 40-unit
    /// button. Two thirds, not full width: edge-to-edge read as a second
    /// border under the pill.
    pub const STANDARD: Self = Self {
        thickness: 3.0,
        gap: 2.0,
        hug_gap: 4.0,
    };

    /// Fraction of the node's width the underline occupies.
    pub const WIDTH_FRACTION: f32 = 2.0 / 3.0;

    /// The underline for a focused node occupying `rect`.
    ///
    /// Centred, [`Self::WIDTH_FRACTION`] of `rect.w`, [`Self::gap`] below
    /// the bottom edge, [`Self::thickness`] tall.
    /// `the_underline_sits_below_the_node_at_two_thirds` asserts it.
    #[must_use]
    pub fn bar(self, rect: Rect) -> Rect {
        let w = (rect.w * Self::WIDTH_FRACTION).max(0.0);
        let x = rect.x + (rect.w - w) * 0.5;
        Rect::new(x, rect.bottom() + self.gap, w, self.thickness)
    }

    /// Left and right bars hugging `rect`: [`Self::thickness`] wide,
    /// [`Self::hug_gap`] outside each edge, exactly the node's height.
    ///
    /// Outside the node, so a well is bracketed rather than underlined, and
    /// never taller than it: a bar past the well's bottom rule reads as an
    /// overhang. `the_hugs_sit_outside_the_left_and_right` asserts it.
    #[must_use]
    pub fn hugs(self, rect: Rect) -> [Rect; 2] {
        let left = Rect::new(
            rect.x - self.hug_gap - self.thickness,
            rect.y,
            self.thickness,
            rect.h.max(0.0),
        );
        let right = Rect::new(
            rect.right() + self.hug_gap,
            rect.y,
            self.thickness,
            rect.h.max(0.0),
        );
        [left, right]
    }

    /// How far outside the node's own rect a bar (not its shadow) can
    /// reach: the wider of the two gaps plus the thickness.
    #[must_use]
    pub fn overhang(self) -> f32 {
        self.gap.max(self.hug_gap) + self.thickness
    }
}

#[cfg(test)]
mod tests {
    use super::FocusRing;
    use crate::geom::Rect;

    #[test]
    fn the_underline_sits_below_the_node_at_two_thirds() {
        let ring = FocusRing::STANDARD;
        let node = Rect::new(10.0, 20.0, 90.0, 40.0);
        let bar = ring.bar(node);
        assert_eq!(bar, Rect::new(25.0, 62.0, 60.0, 3.0));
        assert!((bar.w - node.w * FocusRing::WIDTH_FRACTION).abs() < f32::EPSILON);
        assert!((bar.x + bar.w / 2.0 - (node.x + node.w / 2.0)).abs() < f32::EPSILON);
        assert_eq!(bar.y, node.bottom() + ring.gap);
        assert_eq!(ring.overhang(), 7.0, "the hug gap is the wider reach");
    }

    #[test]
    fn the_hugs_sit_outside_the_left_and_right() {
        let ring = FocusRing::STANDARD;
        let node = Rect::new(10.0, 20.0, 90.0, 40.0);
        let [left, right] = ring.hugs(node);
        assert_eq!(left, Rect::new(3.0, 20.0, 3.0, 40.0));
        assert_eq!(right, Rect::new(104.0, 20.0, 3.0, 40.0));
        assert_eq!(left.right(), node.x - ring.hug_gap);
        assert_eq!(right.x, node.right() + ring.hug_gap);
        assert!(
            ring.hug_gap > ring.gap,
            "a hug stands off a filled well further than an underline stands \
             off a pill"
        );
        assert_eq!(left.y, node.y, "a hug starts at the well's top");
        assert_eq!(
            left.bottom(),
            node.bottom(),
            "a hug never overhangs the well's bottom rule"
        );
        assert_eq!(right.h, node.h);
    }

    #[test]
    fn a_node_narrower_than_the_ring_does_not_invert_it() {
        let bar = FocusRing::STANDARD.bar(Rect::new(0.0, 0.0, 1.0, 1.0));
        assert!(bar.w >= 0.0 && bar.h >= 0.0, "{bar:?}");
        assert!((bar.w - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn the_ring_changes_the_silhouette_by_more_than_a_hairline() {
        let ring = FocusRing::default();
        assert!(
            ring.thickness >= 2.0,
            "a bar thinner than this reads as a border, not as an indicator"
        );
        assert!(ring.gap >= 0.0);
    }
}
