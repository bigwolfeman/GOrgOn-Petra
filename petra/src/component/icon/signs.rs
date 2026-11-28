//! Signs: arrows with a qualifier, loops, stop shapes, and the node shapes
//! of a commit-style graph.
//!
//! Petra's own drawings, in the 32-unit viewBox at Carbon's 2-unit band
//! ([`BAND`]), scaled by `s`. Each sign differs from every other mark by its
//! outline, so none of them needs a hue to be read.

use super::files::{BAND, band, dashed_bar, outline, ring, stroke};
use super::{filled_poly, filled_tri, folder, pt, scaled, snapped_bar};
use crate::draw::{ColorRef, Command, Paint, PathVerb, arc_verbs};
use crate::geom::{Point, Size};

/// An up arrow in the left part of the box: stem from y 8 to 27 at x 10,
/// head bands to x 3.5 and 16.5.
fn left_arrow(dashed_stem: bool, s: f32, paint: &Paint) -> Vec<Command> {
    let mut out = if dashed_stem {
        dashed_bar((9.0, 9.0, 11.0, 28.0), 3.0, 2.5, s, paint)
    } else {
        vec![snapped_bar(9.0, 7.0, 11.0, 28.0, s, paint.clone())]
    };
    out.push(band(pt(10.0, 6.0), pt(3.5, 12.5), BAND, s, paint.clone()));
    out.push(band(pt(10.0, 6.0), pt(16.5, 12.5), BAND, s, paint.clone()));
    out
}

/// An open stroked path through `points`.
fn polyline(points: &[Point], s: f32, color: ColorRef) -> Command {
    let verbs = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 {
                PathVerb::MoveTo(scaled(*p, s))
            } else {
                PathVerb::LineTo(scaled(*p, s))
            }
        })
        .collect();
    Command::Path {
        verbs,
        closed: false,
        paint: stroke(color, s),
    }
}

/// An open stroked arc about `center`, from `start` through `sweep` degrees.
pub(super) fn arc(
    center: Point,
    radius: f32,
    start: f32,
    sweep: f32,
    s: f32,
    color: ColorRef,
) -> Command {
    Command::Path {
        verbs: arc_verbs(
            scaled(center, s),
            radius * s,
            start.to_radians(),
            sweep.to_radians(),
        ),
        closed: false,
        paint: stroke(color, s),
    }
}

/// A filled disc.
fn disc(center: Point, radius: f32, s: f32, color: ColorRef) -> Command {
    Command::Ellipse {
        center: scaled(center, s),
        radii: Size::new(radius * s, radius * s),
        paint: Paint::filled(color),
    }
}

/// An up arrow with a question mark to its right: not known.
pub(super) fn arrow_question(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let mut out = left_arrow(false, s, &paint);
    // The hook: from nine o'clock over the top to about four o'clock, then
    // down to the stem.
    out.push(arc(pt(23.0, 12.0), 4.0, 180.0, 220.0, s, color.clone()));
    out.push(polyline(
        &[pt(24.4, 15.7), pt(23.0, 17.0), pt(23.0, 20.5)],
        s,
        color.clone(),
    ));
    out.push(disc(pt(23.0, 25.0), 1.5, s, color));
    out
}

/// An up arrow with a cross to its right: gone.
pub(super) fn arrow_cross(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out = left_arrow(false, s, &paint);
    out.push(band(pt(19.0, 15.0), pt(28.0, 24.0), BAND, s, paint.clone()));
    out.push(band(pt(28.0, 15.0), pt(19.0, 24.0), BAND, s, paint));
    out
}

/// A centred up arrow whose stem is cut into dashes: not linked.
pub(super) fn arrow_dashed(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out = dashed_bar((15.0, 9.0, 17.0, 28.0), 3.0, 2.5, s, &paint);
    out.push(band(pt(16.0, 6.0), pt(7.5, 14.5), BAND, s, paint.clone()));
    out.push(band(pt(16.0, 6.0), pt(24.5, 14.5), BAND, s, paint));
    out
}

/// Two arrows chasing round a circle: an operation part way through.
pub(super) fn arrows_loop(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let c = pt(16.0, 16.0);
    let r = 10.0;
    let at = |deg: f32| {
        let t = deg.to_radians();
        pt(c.x + r * t.cos(), c.y + r * t.sin())
    };
    // Each arc ends in a head: a triangle whose base straddles the circle and
    // whose tip points along the clockwise tangent.
    let head = |deg: f32| {
        let t = deg.to_radians();
        let (rx, ry) = (t.cos(), t.sin());
        let (tx, ty) = (-ry, rx);
        let base = at(deg);
        [
            pt(base.x + rx * 4.0, base.y + ry * 4.0),
            pt(base.x - rx * 4.0, base.y - ry * 4.0),
            pt(base.x + tx * 5.0, base.y + ty * 5.0),
        ]
    };
    vec![
        arc(c, r, 200.0, 125.0, s, color.clone()),
        arc(c, r, 20.0, 125.0, s, color),
        filled_tri(head(325.0), s, paint.clone()),
        filled_tri(head(145.0), s, paint),
    ]
}

/// The band's centreline of the 4-to-28 square.
const SQUARE: [Point; 4] = [
    Point::new(5.0, 5.0),
    Point::new(27.0, 5.0),
    Point::new(27.0, 27.0),
    Point::new(5.0, 27.0),
];

/// A square holding three dots in a row: more than is shown.
pub(super) fn square_ellipsis(s: f32, color: ColorRef) -> Vec<Command> {
    vec![
        outline(&SQUARE, s, color.clone()),
        disc(pt(10.5, 16.0), 1.75, s, color.clone()),
        disc(pt(16.0, 16.0), 1.75, s, color.clone()),
        disc(pt(21.5, 16.0), 1.75, s, color),
    ]
}

/// A sieve: a rim, a bowl under it, and holes in the bowl.
pub(super) fn sieve(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    vec![
        snapped_bar(2.0, 10.0, 30.0, 12.0, s, paint),
        arc(pt(16.0, 11.0), 11.0, 0.0, 180.0, s, color.clone()),
        disc(pt(11.0, 16.0), 1.5, s, color.clone()),
        disc(pt(16.0, 16.0), 1.5, s, color.clone()),
        disc(pt(21.0, 16.0), 1.5, s, color.clone()),
        disc(pt(13.5, 20.5), 1.5, s, color.clone()),
        disc(pt(18.5, 20.5), 1.5, s, color),
    ]
}

/// An hourglass: a bar at the top and the bottom, an empty upper bulb, and a
/// filled lower bulb.
pub(super) fn hourglass(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    vec![
        snapped_bar(6.0, 3.0, 26.0, 5.0, s, paint.clone()),
        snapped_bar(6.0, 27.0, 26.0, 29.0, s, paint.clone()),
        outline(&[pt(9.0, 5.0), pt(23.0, 5.0), pt(16.0, 15.5)], s, color),
        filled_tri([pt(16.0, 16.0), pt(24.0, 27.0), pt(8.0, 27.0)], s, paint),
    ]
}

/// The band's centreline of a regular octagon, outer edge 3 to 29.
fn octagon() -> [Point; 8] {
    let (lo, hi, a, b) = (4.0, 28.0, 11.0, 21.0);
    [
        pt(a, lo),
        pt(b, lo),
        pt(hi, a),
        pt(hi, b),
        pt(b, hi),
        pt(a, hi),
        pt(lo, b),
        pt(lo, a),
    ]
}

/// An octagon holding a cross: stopped by a fault.
pub(super) fn octagon_cross(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    vec![
        outline(&octagon(), s, color),
        band(pt(11.0, 11.0), pt(21.0, 21.0), BAND, s, paint.clone()),
        band(pt(21.0, 11.0), pt(11.0, 21.0), BAND, s, paint),
    ]
}

/// An octagon holding a horizontal bar: stopped on purpose.
pub(super) fn octagon_bar(s: f32, color: ColorRef) -> Vec<Command> {
    vec![
        outline(&octagon(), s, color.clone()),
        snapped_bar(10.0, 15.0, 22.0, 17.0, s, Paint::filled(color)),
    ]
}

/// Petra's folder with its bottom edge broken and a crack up its face.
pub(super) fn folder_cracked(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    // `folder` draws its bottom edge as one bar, `(4, 26)` to `(28, 28)`;
    // that bar is replaced by two with the crack between them.
    let mut out = folder(s, color.clone());
    out.retain(|c| !matches!(c, Command::Rect { rect, .. } if rect.y >= 25.0 * s));
    out.push(snapped_bar(4.0, 26.0, 14.0, 28.0, s, paint.clone()));
    out.push(snapped_bar(19.0, 26.0, 28.0, 28.0, s, paint));
    out.push(polyline(
        &[
            pt(15.5, 28.0),
            pt(18.0, 22.5),
            pt(14.5, 18.5),
            pt(17.5, 13.0),
        ],
        s,
        color,
    ));
    out
}

/// Petra's folder with a person badge at its lower right: in use by someone.
pub(super) fn folder_person(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let mut out = folder(s, color.clone());
    // The badge sits on a cleared patch: the folder's right and bottom bars
    // stop short of it, and the person is a head and a shoulder arc.
    out.retain(|c| match c {
        Command::Rect { rect, .. } => !(rect.x >= 27.0 * s || rect.y >= 25.0 * s),
        _ => true,
    });
    out.push(snapped_bar(28.0, 10.0, 30.0, 16.0, s, paint.clone()));
    out.push(snapped_bar(4.0, 26.0, 16.0, 28.0, s, paint));
    out.push(disc(pt(24.0, 20.5), 3.0, s, color.clone()));
    out.push(arc(pt(24.0, 31.0), 6.0, 200.0, 140.0, s, color));
    out
}

/// A cloud outline: a flat base and three bumps.
pub(super) fn cloud(s: f32, color: ColorRef) -> Vec<Command> {
    // One closed outline: left bump, top bump, right bump, then the base.
    let mut verbs = arc_verbs(
        scaled(pt(9.0, 19.0), s),
        5.0 * s,
        90.0_f32.to_radians(),
        180.0_f32.to_radians(),
    );
    for (center, radius, start, sweep) in [
        (pt(16.5, 14.0), 7.5, 196.0_f32, 148.0_f32),
        (pt(23.5, 19.5), 4.5, 270.0, 180.0),
    ] {
        let mut more = arc_verbs(
            scaled(center, s),
            radius * s,
            start.to_radians(),
            sweep.to_radians(),
        );
        if let Some(PathVerb::MoveTo(p)) = more.first().cloned() {
            more[0] = PathVerb::LineTo(p);
        }
        verbs.extend(more);
    }
    vec![Command::Path {
        verbs,
        closed: true,
        paint: stroke(color, s),
    }]
}

/// A circle holding an exclamation mark.
pub(super) fn circle_exclamation(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    vec![
        ring(pt(16.0, 16.0), 12.0, s, color.clone()),
        snapped_bar(15.0, 8.0, 17.0, 19.0, s, paint),
        disc(pt(16.0, 23.0), 1.5, s, color),
    ]
}

/// A filled diamond, corners on the box's midlines: a graph's merge node.
pub(super) fn diamond_filled(s: f32, color: ColorRef) -> Vec<Command> {
    vec![filled_poly(
        &[pt(16.0, 3.0), pt(29.0, 16.0), pt(16.0, 29.0), pt(3.0, 16.0)],
        s,
        Paint::filled(color),
    )]
}

/// A filled disc with a bar under it: a graph's first node.
pub(super) fn disc_on_bar(s: f32, color: ColorRef) -> Vec<Command> {
    vec![
        disc(pt(16.0, 13.0), 8.5, s, color.clone()),
        snapped_bar(5.0, 25.0, 27.0, 28.0, s, Paint::filled(color)),
    ]
}
