//! A circular arc as cubic Bézier verbs.
//!
//! `contracts/draw-list.md` §2 keeps arcs out of the command set and says
//! where they go instead: *"convert to cubics at authoring time"*. This is
//! that conversion, in one place, so the loading spinner, the inline
//! loading ring and the dashed step glyphs of the progress indicator all
//! trace a circle the same way and none of them carries its own
//! approximation.
//!
//! # Orientation
//!
//! Angles are radians, measured from three o'clock, and a positive sweep
//! turns **clockwise on screen**. That is the SVG convention every Carbon
//! number in this crate was measured in: a `<circle>`'s stroke starts at
//! three o'clock and its dash runs clockwise, and `transform: rotate(360deg)`
//! turns clockwise. Screen y grows downward, so the point at angle `θ` is
//! `(cx + r cos θ, cy + r sin θ)` with no sign flip.
//!
//! # Accuracy
//!
//! One cubic per quarter turn at most. The classic handle length
//! `4/3 · tan(Δ/4)` puts a quarter-turn cubic within `2.7e-4 · r` of the true
//! circle, which is under a hundredth of a device pixel at every radius this
//! crate draws. The control points of a quarter arc sit `√(1 + k²) ≈ 1.14 r`
//! from the centre — outside the circle — which matters to a caller fitting
//! an arc into a box: a filled half disc of radius `r` needs a box of
//! half-extent at least `1.14 r`, and [`DrawList::new`] measures convexity on
//! those control points, not on the curve.
//!
//! [`DrawList::new`]: super::DrawList::new

use super::PathVerb;
use crate::geom::Point;

/// Verbs tracing the arc of the circle at `center` with radius `radius`,
/// from angle `start` through `sweep` radians, clockwise on screen for a
/// positive sweep.
///
/// The first verb is a `MoveTo` at the start point, so the result begins a
/// sub-path and can be pushed straight into [`super::Command::Path`]. A zero
/// sweep yields the `MoveTo` alone, which the interpreter reports as silent
/// rather than drawing a dot.
#[must_use]
pub fn arc_verbs(center: Point, radius: f32, start: f32, sweep: f32) -> Vec<PathVerb> {
    let at = |theta: f32| {
        Point::new(
            center.x + radius * theta.cos(),
            center.y + radius * theta.sin(),
        )
    };
    let mut verbs = vec![PathVerb::MoveTo(at(start))];
    if sweep == 0.0 || radius <= 0.0 {
        return verbs;
    }
    // Quarter turns at most, so the handle formula stays inside its accuracy
    // envelope. `ceil` so a sweep of exactly a quarter is one segment.
    let segments = (sweep.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0);
    let delta = sweep / segments;
    let handle = 4.0 / 3.0 * (delta / 4.0).tan() * radius;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = segments as usize;
    let mut theta = start;
    for _ in 0..count {
        let next = theta + delta;
        // The circle's tangent at θ is (−sin θ, cos θ); the handle runs along
        // it, and `handle` already carries the sweep's sign through `tan`.
        let c1 = Point::new(
            center.x + radius * theta.cos() - handle * theta.sin(),
            center.y + radius * theta.sin() + handle * theta.cos(),
        );
        let c2 = Point::new(
            center.x + radius * next.cos() + handle * next.sin(),
            center.y + radius * next.sin() - handle * next.cos(),
        );
        verbs.push(PathVerb::CubicTo {
            c1,
            c2,
            to: at(next),
        });
        theta = next;
    }
    verbs
}

#[cfg(test)]
mod tests {
    use super::arc_verbs;
    use crate::draw::{ColorRef, Command, DrawList, Paint, PathVerb, Stroke, Width};
    use crate::geom::Point;
    use std::f32::consts::{FRAC_PI_2, PI, TAU};

    /// Evaluate one cubic at `t`.
    fn cubic(p0: Point, c1: Point, c2: Point, p3: Point, t: f32) -> Point {
        let u = 1.0 - t;
        let b0 = u * u * u;
        let b1 = 3.0 * u * u * t;
        let b2 = 3.0 * u * t * t;
        let b3 = t * t * t;
        Point::new(
            b0 * p0.x + b1 * c1.x + b2 * c2.x + b3 * p3.x,
            b0 * p0.y + b1 * c1.y + b2 * c2.y + b3 * p3.y,
        )
    }

    /// Walk every cubic and return the largest distance from the true circle.
    fn worst_radial_error(verbs: &[PathVerb], center: Point, radius: f32) -> f32 {
        let mut current = match verbs[0] {
            PathVerb::MoveTo(p) => p,
            other => panic!("an arc starts with MoveTo, got {other:?}"),
        };
        let mut worst = 0.0_f32;
        for verb in &verbs[1..] {
            let PathVerb::CubicTo { c1, c2, to } = *verb else {
                panic!("an arc is cubics after its MoveTo, got {verb:?}");
            };
            for step in 0..=64 {
                let t = step as f32 / 64.0;
                let p = cubic(current, c1, c2, to, t);
                let r = ((p.x - center.x).powi(2) + (p.y - center.y).powi(2)).sqrt();
                worst = worst.max((r - radius).abs());
            }
            current = to;
        }
        worst
    }

    /// The whole reason to use cubics rather than a polyline: at the loading
    /// spinner's radius the curve is within a hundredth of a device pixel of
    /// a circle, at both device scales the catalog captures at.
    #[test]
    fn a_full_turn_stays_on_the_circle_at_the_spinners_radius() {
        let center = Point::new(44.0, 44.0);
        let radius = 38.72;
        let verbs = arc_verbs(center, radius, 0.0, TAU);
        assert_eq!(verbs.len(), 5, "MoveTo plus four quarter turns");
        let worst = worst_radial_error(&verbs, center, radius);
        assert!(worst < 0.011, "worst radial error {worst} logical units");
        // Ends where it started.
        let PathVerb::CubicTo { to, .. } = verbs[4] else {
            unreachable!()
        };
        assert!((to.x - (center.x + radius)).abs() < 1e-3 && (to.y - center.y).abs() < 1e-3);
    }

    /// Orientation is the SVG one: from three o'clock, a positive quarter
    /// sweep ends at six o'clock (screen y grows downward), which is what
    /// makes a Carbon dash angle usable here without a sign flip.
    #[test]
    fn a_positive_sweep_turns_clockwise_on_screen() {
        let verbs = arc_verbs(Point::new(0.0, 0.0), 10.0, 0.0, FRAC_PI_2);
        assert_eq!(verbs[0], PathVerb::MoveTo(Point::new(10.0, 0.0)));
        let PathVerb::CubicTo { to, .. } = verbs[1] else {
            panic!("{verbs:?}")
        };
        assert!((to.x).abs() < 1e-5 && (to.y - 10.0).abs() < 1e-5, "{to:?}");
        // And the mirror: a negative sweep goes up the screen.
        let verbs = arc_verbs(Point::new(0.0, 0.0), 10.0, 0.0, -FRAC_PI_2);
        let PathVerb::CubicTo { to, .. } = verbs[1] else {
            panic!("{verbs:?}")
        };
        assert!((to.x).abs() < 1e-5 && (to.y + 10.0).abs() < 1e-5, "{to:?}");
    }

    /// Sweeps split into at most quarter turns, and short sweeps stay one
    /// segment: a Carbon dash of 18° is a single cubic.
    #[test]
    fn segments_are_at_most_a_quarter_turn_each() {
        let center = Point::new(8.0, 8.0);
        assert_eq!(arc_verbs(center, 6.5, 0.0, 18.0_f32.to_radians()).len(), 2);
        assert_eq!(arc_verbs(center, 6.5, 0.0, FRAC_PI_2).len(), 2);
        assert_eq!(arc_verbs(center, 6.5, 0.0, PI).len(), 3);
        assert_eq!(arc_verbs(center, 6.5, 0.0, 0.81 * TAU).len(), 5);
        assert_eq!(arc_verbs(center, 6.5, 0.0, 0.0).len(), 1);
    }

    /// A half disc closed on its chord is convex on its control polygon, so
    /// the draw list accepts it filled — this is the Incomplete glyph's left
    /// half and it must not need a polygon fan.
    #[test]
    fn a_closed_half_turn_is_a_convex_fill() {
        let verbs = arc_verbs(Point::new(8.0, 8.0), 7.0, FRAC_PI_2, PI);
        let list = DrawList::new(vec![Command::Path {
            verbs,
            closed: true,
            paint: Paint::filled(ColorRef::Token("accent.primary".into())),
        }]);
        assert!(list.is_ok(), "{list:?}");
    }

    /// The stroked form, open, is what a spinner is: one path, no fill.
    #[test]
    fn an_open_arc_strokes_as_one_path() {
        let verbs = arc_verbs(Point::new(8.0, 8.0), 6.72, 0.0, 0.48 * TAU);
        let list = DrawList::new(vec![Command::Path {
            verbs,
            closed: false,
            paint: Paint::stroked(Stroke {
                width: Width::Logical(2.56),
                color: ColorRef::Token("accent.primary".into()),
            }),
        }])
        .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list.path_verbs(), 3);
    }
}
