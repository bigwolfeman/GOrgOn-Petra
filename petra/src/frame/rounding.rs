//! The one device-rounding rule.
//!
//! Logical units become device pixels exactly once, here, at petrify. The
//! renderer and the digest both call this function, which is what makes
//! `contracts/frame-identity.md`'s "the SAME rounding the renderer uses"
//! true by construction rather than by review.

use crate::geom::{Rect, Scale};

/// A rect in whole device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeviceRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

impl DeviceRect {
    /// Right edge.
    #[must_use]
    pub fn right(self) -> i32 {
        self.x + self.w
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(self) -> i32 {
        self.y + self.h
    }
}

/// Round one logical coordinate to a device pixel.
///
/// `f32::round` is half-away-from-zero and identical on every target we build
/// for, so two machines that agree on the input agree on the output. Ties are
/// not resolved to even: banker's rounding would make a rect's width depend on
/// where it sits, and adjacent rows at 1.5 scale would alternate thickness.
#[must_use]
pub fn round_coord(logical: f32, scale: Scale) -> i32 {
    let scaled = logical * scale.factor();
    if !scaled.is_finite() {
        return 0;
    }
    let rounded = scaled.round();
    // The clamp keeps a pathological viewport from wrapping the integer; a
    // rect that far off-screen is clipped away regardless.
    rounded.clamp(f32::from(i16::MIN) * 64.0, f32::from(i16::MAX) * 64.0) as i32
}

/// Round a logical rect to device pixels by rounding its four edges.
///
/// Edges, not origin-plus-size: two rects that share an edge in logical units
/// must share it in device pixels, or every adjacent pair of rows grows a
/// seam or an overlap at fractional scale.
#[must_use]
pub fn round_rect(rect: Rect, scale: Scale) -> DeviceRect {
    let x0 = round_coord(rect.x, scale);
    let y0 = round_coord(rect.y, scale);
    let x1 = round_coord(rect.right(), scale);
    let y1 = round_coord(rect.bottom(), scale);
    DeviceRect {
        x: x0,
        y: y0,
        w: (x1 - x0).max(0),
        h: (y1 - y0).max(0),
    }
}

#[cfg(test)]
mod tests {
    use super::{round_coord, round_rect};
    use crate::geom::{Rect, Scale};

    fn scale(f: f32) -> Scale {
        Scale::new(f).unwrap()
    }

    #[test]
    fn unscaled_rounding_is_the_identity_on_integers() {
        let r = Rect::new(3.0, 4.0, 10.0, 20.0);
        let d = round_rect(r, Scale::ONE);
        assert_eq!((d.x, d.y, d.w, d.h), (3, 4, 10, 20));
    }

    /// The seam test. Three stacked rows of 10.5 logical units at 1.25 scale
    /// must tile with no gap and no overlap, which only edge rounding gives.
    #[test]
    fn adjacent_rects_tile_without_seams_at_fractional_scale() {
        for s in [1.25_f32, 1.5, 2.0, 1.75] {
            let mut y = 0.0_f32;
            let mut prev_bottom = round_coord(0.0, scale(s));
            for _ in 0..8 {
                let row = Rect::new(0.0, y, 100.0, 10.5);
                let d = round_rect(row, scale(s));
                assert_eq!(d.y, prev_bottom, "seam at scale {s}");
                prev_bottom = d.bottom();
                y += 10.5;
            }
        }
    }

    /// Origin-plus-size rounding is the bug this rule exists to prevent. If
    /// this ever passes, the rule has been replaced by the wrong one.
    #[test]
    fn edge_rounding_differs_from_size_rounding_where_it_matters() {
        let s = scale(1.5);
        let r = Rect::new(1.0, 0.0, 1.0, 1.0);
        let d = round_rect(r, s);
        let naive_w = (r.w * 1.5).round() as i32;
        assert_eq!(d.x, 2, "1.0 * 1.5 = 1.5 rounds away from zero");
        assert_eq!(d.w, 1, "edges 1.5 -> 2 and 3.0 -> 3 give a width of 1");
        assert_ne!(d.w, naive_w, "size rounding would have said 2");
    }

    #[test]
    fn degenerate_input_cannot_produce_a_negative_extent() {
        let d = round_rect(Rect::new(10.0, 10.0, -5.0, -5.0), Scale::ONE);
        assert_eq!((d.w, d.h), (0, 0));
        assert_eq!(round_coord(f32::NAN, Scale::ONE), 0);
        assert_eq!(round_coord(f32::INFINITY, Scale::ONE), 0);
    }

    #[test]
    fn rounding_is_deterministic_across_repeats() {
        let r = Rect::new(3.3333, 7.7777, 11.1111, 13.9999);
        let first = round_rect(r, scale(1.25));
        for _ in 0..100 {
            assert_eq!(round_rect(r, scale(1.25)), first);
        }
    }
}
