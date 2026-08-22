//! Logical geometry. Everything in the engine is logical units until petrify;
//! device pixels appear once, in [`crate::frame::rounding`].

use serde::{Deserialize, Serialize};

/// A width/height pair in logical units. Always finite and non-negative once
/// it leaves a measurement: [`Size::sane`] is the enforcement point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    /// Extent along the horizontal axis.
    pub w: f32,
    /// Extent along the vertical axis.
    pub h: f32,
}

impl Size {
    /// The zero size.
    pub const ZERO: Self = Self { w: 0.0, h: 0.0 };

    /// A size from its two extents.
    #[must_use]
    pub const fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }

    /// The extent along `axis`.
    #[must_use]
    pub fn along(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.w,
            Axis::Vertical => self.h,
        }
    }

    /// The extent across `axis`.
    #[must_use]
    pub fn across(self, axis: Axis) -> f32 {
        self.along(axis.cross())
    }

    /// Build a size from a main-axis and a cross-axis extent.
    #[must_use]
    pub fn from_axes(axis: Axis, main: f32, cross: f32) -> Self {
        match axis {
            Axis::Horizontal => Self::new(main, cross),
            Axis::Vertical => Self::new(cross, main),
        }
    }

    /// Clamp both extents to finite, non-negative values.
    ///
    /// A measurement is a load-bearing input to the digest, so a NaN or a
    /// negative extent may not travel: it would make one frame's identity
    /// depend on which comparison happened to run first. Both collapse to
    /// zero here, at the one place every measurement passes through.
    #[must_use]
    pub fn sane(self) -> Self {
        Self::new(sane_extent(self.w), sane_extent(self.h))
    }

    /// The larger extent on each axis.
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        Self::new(self.w.max(other.w), self.h.max(other.h))
    }

    /// The smaller extent on each axis.
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        Self::new(self.w.min(other.w), self.h.min(other.h))
    }
}

fn sane_extent(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 { v } else { 0.0 }
}

/// A point in logical units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position.
    pub y: f32,
}

impl Point {
    /// The origin.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// A point from its coordinates.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned rectangle in logical units, origin top-left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// The empty rectangle at the origin.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };

    /// A rectangle from an origin and a size.
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// A rectangle from an origin point and a size.
    #[must_use]
    pub fn at(origin: Point, size: Size) -> Self {
        Self::new(origin.x, origin.y, size.w, size.h)
    }

    /// This rectangle's size.
    #[must_use]
    pub fn size(self) -> Size {
        Size::new(self.w, self.h)
    }

    /// This rectangle's origin.
    #[must_use]
    pub fn origin(self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Right edge (`x + w`).
    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    /// Bottom edge (`y + h`).
    #[must_use]
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    /// Whether this rectangle contains `p`, half-open on the far edges so two
    /// abutting rectangles never both claim the same point.
    #[must_use]
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// Whether two rectangles share any interior area. Touching edges do not
    /// overlap; the stack property tests depend on that reading.
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// The intersection, or an empty rectangle when they do not overlap.
    #[must_use]
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Self::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }

    /// Move by a delta.
    #[must_use]
    pub fn translate(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Pull every edge inward by `by`, or outward when `by` is negative.
    ///
    /// Extent never goes below zero: insetting a rect narrower than `2 * by`
    /// collapses it rather than turning it inside out, which is what keeps a
    /// caller from drawing a negative-width band on a one-pixel node.
    #[must_use]
    pub fn inset(self, by: f32) -> Self {
        Self::new(
            self.x + by,
            self.y + by,
            (self.w - 2.0 * by).max(0.0),
            (self.h - 2.0 * by).max(0.0),
        )
    }

    /// This rect's interior after pulling each edge in by the matching field
    /// of `insets`. Unlike [`Rect::inset`], which is used only by
    /// `token::focus`'s symmetric focus-ring flank, this is per-edge and never
    /// produces a negative extent: an inset wider than the rect collapses
    /// that axis to zero rather than turning inside out, the same floor
    /// `Rect::inset` already uses.
    #[must_use]
    pub fn inset_edges(self, insets: Insets) -> Self {
        let w = (self.w - insets.left - insets.right).max(0.0);
        let h = (self.h - insets.top - insets.bottom).max(0.0);
        Self::new(self.x + insets.left, self.y + insets.top, w, h)
    }
}

/// Content insets from a container's own edges, logical units.
///
/// Padding, not margin: `Insets` describes the gap between a container's own
/// rect and the rect it offers its children — the container's *own* placed
/// rect never moves or shrinks because of its own padding (`Placement::rect`
/// is unaffected; only what a container hands its children is). This mirrors
/// SwiftUI's `.padding()` and CSS's border-box model, both of which put
/// padding inside the box rather than around it
/// (<https://developer.apple.com/documentation/swiftui/view/padding(_:_:)>,
/// <https://developer.mozilla.org/en-US/docs/Learn/CSS/Building_blocks/The_box_model>).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Insets {
    /// Top edge.
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub top: f32,
    /// Right edge.
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub right: f32,
    /// Bottom edge.
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub bottom: f32,
    /// Left edge.
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub left: f32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

impl Insets {
    /// No inset on any edge. The all-zero value every absent `padding` resolves to.
    pub const NONE: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// The same inset on all four edges.
    #[must_use]
    pub fn all(v: f32) -> Self {
        let v = v.max(0.0);
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    /// Horizontal inset on left/right, vertical inset on top/bottom —
    /// the `EdgeInsets.symmetric` shape
    /// (<https://api.flutter.dev/flutter/painting/EdgeInsets-class.html>).
    #[must_use]
    pub fn symmetric(horizontal: f32, vertical: f32) -> Self {
        let (h, v) = (horizontal.max(0.0), vertical.max(0.0));
        Self {
            top: v,
            right: h,
            bottom: v,
            left: h,
        }
    }

    /// Total inset along `axis` (`left + right` for horizontal, `top + bottom`
    /// for vertical). Non-negative: every field is clamped to `>= 0` at
    /// resolution (`Props::padding`, `tree/validate.rs`'s check), so this can
    /// only be negative from a hand-built `Insets` a caller assembled outside
    /// `Props`, which `.max(0.0)` closes off regardless.
    #[must_use]
    pub fn along(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => (self.left + self.right).max(0.0),
            Axis::Vertical => (self.top + self.bottom).max(0.0),
        }
    }

    /// Inset at the leading edge of `axis` (`left` for horizontal, `top` for vertical).
    #[must_use]
    pub fn leading(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.left,
            Axis::Vertical => self.top,
        }
    }
}

/// A layout axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    /// Left to right.
    Horizontal,
    /// Top to bottom.
    Vertical,
}

impl Axis {
    /// This axis's wire name, the same spelling `serde` reads and writes.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }

    /// The other axis.
    #[must_use]
    pub fn cross(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

/// Cross-axis placement of a child inside the space its parent gives it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    /// Against the leading edge.
    #[default]
    Start,
    /// Centred.
    Center,
    /// Against the trailing edge.
    End,
    /// Filled to the available extent.
    Stretch,
}

impl Align {
    /// The leading offset for a child of `child` extent inside `available`.
    #[must_use]
    pub fn offset(self, available: f32, child: f32) -> f32 {
        match self {
            Self::Start | Self::Stretch => 0.0,
            Self::Center => ((available - child) / 2.0).max(0.0),
            Self::End => (available - child).max(0.0),
        }
    }
}

/// Display scale factor (device pixels per logical unit).
///
/// Held as a rational-free `f32` but compared and hashed by bit pattern, so
/// 1.25 is one scale everywhere and never two that differ in the last bit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scale(f32);

impl Scale {
    /// Unscaled.
    pub const ONE: Self = Self(1.0);

    /// A scale factor. Non-finite or non-positive input is rejected.
    ///
    /// # Errors
    /// Returns the offending value when it is not a usable scale.
    pub fn new(factor: f32) -> Result<Self, f32> {
        if factor.is_finite() && factor > 0.0 {
            Ok(Self(factor))
        } else {
            Err(factor)
        }
    }

    /// The factor as a float.
    #[must_use]
    pub fn factor(self) -> f32 {
        self.0
    }
}

impl Default for Scale {
    fn default() -> Self {
        Self::ONE
    }
}

impl Eq for Scale {}

impl std::hash::Hash for Scale {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::{Align, Axis, Insets, Point, Rect, Scale, Size};

    /// The hand-written name and the serde name are one fact, so a rename of
    /// either without the other fails here rather than in a wire consumer.
    #[test]
    fn an_axis_spells_itself_the_same_way_serde_does() {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            assert_eq!(
                serde_json::to_string(&axis).unwrap(),
                format!("\"{}\"", axis.as_str())
            );
        }
    }

    #[test]
    fn sane_collapses_nan_and_negatives() {
        assert_eq!(Size::new(f32::NAN, -3.0).sane(), Size::ZERO);
        assert_eq!(Size::new(f32::INFINITY, 2.0).sane(), Size::new(0.0, 2.0));
        assert_eq!(Size::new(4.0, 2.0).sane(), Size::new(4.0, 2.0));
    }

    #[test]
    fn axis_projection_round_trips() {
        let s = Size::new(10.0, 4.0);
        assert_eq!(s.along(Axis::Horizontal), 10.0);
        assert_eq!(s.across(Axis::Horizontal), 4.0);
        assert_eq!(Size::from_axes(Axis::Vertical, 4.0, 10.0), s);
    }

    #[test]
    fn touching_rects_do_not_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(10.0, 0.0, 10.0, 10.0);
        assert!(!a.overlaps(b));
        assert!(a.overlaps(Rect::new(9.9, 0.0, 1.0, 1.0)));
    }

    #[test]
    fn contains_is_half_open() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.contains(Point::new(0.0, 0.0)));
        assert!(!a.contains(Point::new(10.0, 0.0)));
    }

    #[test]
    fn align_offsets_never_go_negative() {
        assert_eq!(Align::Center.offset(10.0, 4.0), 3.0);
        assert_eq!(Align::End.offset(10.0, 4.0), 6.0);
        assert_eq!(Align::Center.offset(4.0, 10.0), 0.0);
        assert_eq!(Align::Stretch.offset(10.0, 4.0), 0.0);
    }

    #[test]
    fn scale_rejects_unusable_factors() {
        assert!(Scale::new(0.0).is_err());
        assert!(Scale::new(f32::NAN).is_err());
        assert_eq!(Scale::new(1.25).unwrap().factor(), 1.25);
    }

    /// An inset wider than the rect collapses that axis to zero rather than
    /// turning inside out, the same floor `Rect::inset` already uses.
    #[test]
    fn inset_edges_floors_at_zero_rather_than_going_negative() {
        let rect = Rect::new(0.0, 0.0, 10.0, 4.0);
        let huge = Insets::all(100.0);
        let inset = rect.inset_edges(huge);
        assert_eq!(inset.w, 0.0);
        assert_eq!(inset.h, 0.0);
        // Origin still walks in by the declared amount, even once the
        // extent has floored: the rect is degenerate, not undefined.
        assert_eq!(inset.x, 100.0);
        assert_eq!(inset.y, 100.0);
    }

    /// `Insets::all(v)` on all four edges must agree with the existing
    /// uniform `Rect::inset(v)` — the new per-edge path and the old uniform
    /// one describe the same fact when every edge is equal.
    #[test]
    fn insets_all_agrees_with_the_existing_uniform_inset() {
        let rect = Rect::new(2.0, 3.0, 20.0, 12.0);
        for v in [0.0_f32, 1.5, 6.0] {
            assert_eq!(rect.inset_edges(Insets::all(v)), rect.inset(v));
        }
    }

    #[test]
    fn insets_symmetric_splits_horizontal_and_vertical() {
        let insets = Insets::symmetric(4.0, 8.0);
        assert_eq!(insets.left, 4.0);
        assert_eq!(insets.right, 4.0);
        assert_eq!(insets.top, 8.0);
        assert_eq!(insets.bottom, 8.0);
        assert_eq!(insets.along(Axis::Horizontal), 8.0);
        assert_eq!(insets.along(Axis::Vertical), 16.0);
        assert_eq!(insets.leading(Axis::Horizontal), 4.0);
        assert_eq!(insets.leading(Axis::Vertical), 8.0);
    }

    #[test]
    fn insets_none_is_the_all_zero_default() {
        assert_eq!(Insets::NONE, Insets::default());
        assert_eq!(Insets::all(0.0), Insets::NONE);
        let rect = Rect::new(1.0, 2.0, 10.0, 10.0);
        assert_eq!(rect.inset_edges(Insets::NONE), rect);
    }

    /// Negative constructor inputs never produce a negative inset — the
    /// same non-negativity `tree::validate` also enforces for a declared
    /// `padding`, but a hand-built `Insets` must not depend on validation
    /// having run.
    #[test]
    fn insets_constructors_reject_negative_input() {
        assert_eq!(Insets::all(-5.0), Insets::NONE);
        assert_eq!(Insets::symmetric(-2.0, -3.0), Insets::NONE);
    }

    /// `Insets::NONE` serializes to an empty table (every field is
    /// individually `skip_serializing_if`), and a declared value round-trips
    /// exactly — the same "absence is zero, presence is exact" shape every
    /// other wire type in this crate follows.
    #[test]
    fn insets_serde_round_trips() {
        assert_eq!(serde_json::to_string(&Insets::NONE).unwrap(), "{}");

        let insets = Insets {
            top: 4.0,
            right: 8.0,
            bottom: 4.0,
            left: 8.0,
        };
        let json = serde_json::to_string(&insets).unwrap();
        let back: Insets = serde_json::from_str(&json).unwrap();
        assert_eq!(back, insets);
        assert!(json.contains("\"top\":4"), "{json}");
    }
}
