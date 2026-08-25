//! The focus indicator: where the ring goes, and why it is not a colour.
//!
//! FR-015's rule is that no Petra-provided status may carry meaning by colour
//! alone, and keyboard focus is a status like any other — the person this
//! project is built for is red-green colourblind, so "the same box in a
//! different colour" is not an indicator, it is a coin flip. The focus ring
//! answers that the way [`crate::token::status`] does, with channels that
//! survive desaturation:
//!
//! * **Geometry is the channel.** A focused node grows a ring
//!   [`FocusRing::thickness`] logical units thick, straddling its edge, that
//!   an unfocused node does not have. The difference between the two states
//!   is a *shape that is present or absent*, not a hue. Print the frame in
//!   greyscale, or collapse every hue onto one, and the indicator survives
//!   intact, because nothing about it was ever carried by hue.
//! * **Lightness is the second channel.** The ring is three concentric
//!   bands — halo, core, halo — and the two tokens are an ink/paper pair:
//!   one near-black, one near-white, both achromatic. That is what makes the
//!   ring visible over an arbitrary surface, and the arrangement is what
//!   makes it provable: **both** surfaces the ring can touch (the node's own
//!   fill on the inside, whatever is behind the node on the outside) have a
//!   halo band and a core band lying on them, and for any colour at all one
//!   of that pair clears a 3:1 contrast ratio against it. A two-band ring
//!   with one band per side does not have this property: a light node on a
//!   dark page can match both bands at once and swallow the whole ring.
//!   `the_focus_ring_is_visible_over_any_surface` in
//!   [`crate::token::shipped`] measures the claim over a luminance sweep
//!   rather than asserting it.
//!
//! What this deliberately does *not* promise: the ring does not avoid the
//! node's own content. Its inner bands are painted over the top-left few
//! units of whatever the node draws, which on a tightly-sized text node
//! clips the corner of a glyph. Focus that is visible over the text beats
//! focus that is invisible beside it.
//!
//! The geometry lives here, in the engine, and not in `gorgon-petra-egui`:
//! how thick the ring is and where it sits relative to the node is a design
//! decision a second renderer must reproduce, and D-069 keeps this crate free
//! of any toolkit. What the adapter crate owns is the drawing.

use crate::geom::Rect;

/// Token naming the ring's core band, the thick one on the node's edge.
pub const RING_TOKEN: &str = "focus.ring";

/// Token naming the two halo bands that flank the core.
///
/// Its job is to be the opposite lightness to [`RING_TOKEN`]: one of the pair
/// is legible over any surface either one alone could disappear into.
pub const HALO_TOKEN: &str = "focus.ring-halo";

/// One band of the focus ring: a stroke of `width` logical units, centred on
/// the path around `rect`, painted in the colour bound to `token`.
///
/// The rect is the stroke's **centreline**, so a renderer draws it with a
/// centred stroke and needs no inside/outside convention of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusBand {
    /// Centreline of the stroke, in logical units.
    pub rect: Rect,
    /// Stroke width in logical units.
    pub width: f32,
    /// Colour token for this band.
    pub token: &'static str,
    /// What to add to the focused node's own corner radius to round this
    /// band, in logical units. Negative for a band inside the node's edge.
    ///
    /// **A ring that ignores this draws a square around a rounded control**,
    /// which is what shipped until 2026-08-25: `paint_focus_ring` passed a
    /// hardcoded `0.0` corner radius, so a focused `button` — a
    /// `shape.corner-md`, eight-unit rounding — wore a hard rectangle. Next
    /// to two unfocused buttons that kept their corners, it read as a
    /// different component rather than as the same one with focus on it.
    ///
    /// The offset is here rather than in the renderer for the same reason
    /// the rest of this geometry is (D-069): where a band sits relative to
    /// the node is a design decision a second renderer has to reproduce, and
    /// concentric rounding is part of where it sits. A band whose rect is
    /// inset by `d` has to lose `d` of radius or it is not concentric with
    /// the edge it is tracking — it would bulge at the corners and pinch on
    /// the flats.
    pub radius_delta: f32,
}

/// The focus ring's measurements, in logical units.
///
/// The core is two units rather than one: at a display scale of 1.0 a
/// one-unit stroke is a single device pixel, which anti-aliases into a grey
/// hairline that reads as a border rather than as an indicator, and the point
/// of the geometric channel is that it is unmistakable at a glance. The halos
/// are one unit each, which is all a contrast guarantee needs, and keeps the
/// whole ring to four units — a 16-unit icon button gains a quarter of its own
/// width in banding rather than half.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusRing {
    /// Width of the core band, centred on the node's edge.
    pub core: f32,
    /// Width of each of the two halo bands flanking the core.
    pub halo: f32,
}

impl Default for FocusRing {
    fn default() -> Self {
        Self::STANDARD
    }
}

impl FocusRing {
    /// The shipped ring.
    pub const STANDARD: Self = Self {
        core: 2.0,
        halo: 1.0,
    };

    /// The three bands for a focused node occupying `rect`, innermost first.
    ///
    /// The core straddles the node's edge, so half the ring survives a clip
    /// that is exactly the node's own rect — the root of a frame, or a node
    /// filling its scroll viewport exactly. A ring drawn wholly outside the
    /// edge would be clipped away entirely in some of the cases where focus
    /// matters most.
    #[must_use]
    pub fn bands(self, rect: Rect) -> [FocusBand; 3] {
        let flank = self.core / 2.0 + self.halo / 2.0;
        [
            FocusBand {
                rect: rect.inset(flank),
                width: self.halo,
                token: HALO_TOKEN,
                radius_delta: -flank,
            },
            FocusBand {
                rect,
                width: self.core,
                token: RING_TOKEN,
                radius_delta: 0.0,
            },
            FocusBand {
                rect: rect.inset(-flank),
                width: self.halo,
                token: HALO_TOKEN,
                radius_delta: flank,
            },
        ]
    }

    /// Total thickness of the ring, both halos and the core.
    #[must_use]
    pub fn thickness(self) -> f32 {
        self.core + 2.0 * self.halo
    }

    /// How far outside the node's own rect the ring reaches.
    #[must_use]
    pub fn overhang(self) -> f32 {
        self.thickness() / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusRing, HALO_TOKEN, RING_TOKEN};
    use crate::geom::Rect;

    /// Each surface the ring can lie on — the node's fill inside the edge,
    /// whatever is behind the node outside it — carries one halo band and one
    /// core band. That pairing is the whole contrast argument, so it is
    /// asserted as geometry here and measured as colour in `shipped`.
    #[test]
    fn both_sides_of_the_edge_carry_a_halo_and_a_core_band() {
        let ring = FocusRing::STANDARD;
        let bands = ring.bands(Rect::new(10.0, 20.0, 100.0, 40.0));

        // Signed distance from the node's left edge to each band's span,
        // positive inward.
        let span = |b: &super::FocusBand| {
            let centre = b.rect.x - 10.0;
            (centre - b.width / 2.0, centre + b.width / 2.0)
        };
        assert_eq!(span(&bands[0]), (1.0, 2.0), "inner halo");
        assert_eq!(span(&bands[1]), (-1.0, 1.0), "core straddles the edge");
        assert_eq!(span(&bands[2]), (-2.0, -1.0), "outer halo");

        assert_eq!(bands[0].token, HALO_TOKEN);
        assert_eq!(bands[1].token, RING_TOKEN);
        assert_eq!(bands[2].token, HALO_TOKEN);
        assert_eq!(ring.thickness(), 4.0);
        assert_eq!(ring.overhang(), 2.0);
    }

    /// The bands are concentric and none of them is inside out.
    #[test]
    fn a_node_narrower_than_the_ring_does_not_invert_it() {
        let bands = FocusRing::STANDARD.bands(Rect::new(0.0, 0.0, 1.0, 1.0));
        for b in &bands {
            assert!(b.rect.w >= 0.0 && b.rect.h >= 0.0, "{b:?}");
        }
        assert_eq!(bands[0].rect.w, 0.0, "the inner halo collapses");
        assert_eq!(bands[2].rect.w, 4.0, "the outer halo still rings it");
    }

    /// The ring is four units of band on a node that had none. That is the
    /// non-colour channel, and it is a property of the geometry alone.
    #[test]
    fn the_ring_changes_the_silhouette_by_more_than_a_hairline() {
        let ring = FocusRing::default();
        assert!(
            ring.thickness() >= 4.0,
            "a ring thinner than this reads as a border, not as an indicator"
        );
        assert!(ring.core >= 2.0, "the core must survive anti-aliasing");
        assert!(ring.halo > 0.0, "both halos are drawn");
    }
}
