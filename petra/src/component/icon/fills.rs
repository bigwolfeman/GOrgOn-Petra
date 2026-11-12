//! Shapes with three fill forms, and the arrows.
//!
//! A shape here comes as an outline, a half-filled form, and a filled form,
//! so one mark can say "none", "some", or "all" of a thing by fill alone. Fill
//! is a shape and lightness cue, not a hue cue, so a reader who cannot tell
//! two hues apart still reads it. The half form fills the lower half: Petra's
//! [`super::IconMark::Incomplete`] fills the left half of a dashed ring, and
//! the two must not read as one picture.
//!
//! Every glyph is in the 32-unit viewBox, scaled by `s`, with Carbon's
//! 2-unit band ([`BAND`]). One ink cannot cut a hole, so a filled form that
//! must still show an inner figure (the bar of [`square_bar`], the
//! exclamation of [`triangle_alert`]) is built from the convex pieces around
//! that figure.

use super::files::{BAND, band, dashed_bar, outline, ring};
use super::{filled_poly, filled_quad, pt, scaled, snapped_bar};
use crate::draw::{ColorRef, Command, Paint, arc_verbs};
use crate::geom::{Point, Size};

/// How much of a shape is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fill {
    /// The outline only.
    Outline,
    /// The outline with its lower half filled.
    Half,
    /// Filled.
    Full,
}

const CENTER: Point = Point::new(16.0, 16.0);

/// A circle, outer edge from 4 to 28.
pub(super) fn circle(fill: Fill, s: f32, color: ColorRef) -> Vec<Command> {
    match fill {
        Fill::Outline => vec![ring(CENTER, 11.0, s, color)],
        Fill::Half => vec![
            ring(CENTER, 11.0, s, color.clone()),
            // From three o'clock, clockwise on screen through six to nine,
            // closed on the horizontal chord: a convex half disc.
            Command::Path {
                verbs: arc_verbs(scaled(CENTER, s), 11.0 * s, 0.0, std::f32::consts::PI),
                closed: true,
                paint: Paint::filled(color),
            },
        ],
        Fill::Full => vec![Command::Ellipse {
            center: scaled(CENTER, s),
            radii: Size::new(12.0 * s, 12.0 * s),
            paint: Paint::filled(color),
        }],
    }
}

/// The band's centreline of the 4-to-28 square.
const SQUARE: [Point; 4] = [
    Point::new(5.0, 5.0),
    Point::new(27.0, 5.0),
    Point::new(27.0, 27.0),
    Point::new(5.0, 27.0),
];

/// A square with a horizontal bar across its middle.
pub(super) fn square_bar(fill: Fill, s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let bar = snapped_bar(10.0, 15.0, 22.0, 17.0, s, paint.clone());
    match fill {
        Fill::Outline => vec![outline(&SQUARE, s, color), bar],
        Fill::Half => vec![
            outline(&SQUARE, s, color),
            bar,
            snapped_bar(4.0, 20.0, 28.0, 28.0, s, paint),
        ],
        // The filled square with the bar left open: above, below, and the two
        // pieces either side of the slot.
        Fill::Full => vec![
            snapped_bar(4.0, 4.0, 28.0, 14.0, s, paint.clone()),
            snapped_bar(4.0, 18.0, 28.0, 28.0, s, paint.clone()),
            snapped_bar(4.0, 14.0, 9.0, 18.0, s, paint.clone()),
            snapped_bar(23.0, 14.0, 28.0, 18.0, s, paint),
        ],
    }
}

/// A triangle holding an exclamation mark.
pub(super) fn triangle_alert(fill: Fill, s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let edge = [pt(16.0, 4.5), pt(28.0, 26.5), pt(4.0, 26.5)];
    let stem = snapped_bar(15.0, 11.0, 17.0, 19.0, s, paint.clone());
    match fill {
        Fill::Outline => vec![
            outline(&edge, s, color),
            stem,
            snapped_bar(15.0, 21.0, 17.0, 23.0, s, paint),
        ],
        // The lower half by area: a triangle's area halves where its width is
        // 1/sqrt(2) of the base, at y 20. The band holds the dot as a gap,
        // built from the pieces left of, right of, above, and below it; the
        // stem stays ink above the band with a 1-unit gap.
        Fill::Half => vec![
            outline(&edge, s, color),
            stem,
            filled_quad(
                [
                    pt(7.55, 20.0),
                    pt(15.0, 20.0),
                    pt(15.0, 26.5),
                    pt(4.0, 26.5),
                ],
                s,
                paint.clone(),
            ),
            filled_quad(
                [
                    pt(17.0, 20.0),
                    pt(24.45, 20.0),
                    pt(28.0, 26.5),
                    pt(17.0, 26.5),
                ],
                s,
                paint.clone(),
            ),
            snapped_bar(15.0, 20.0, 17.0, 21.0, s, paint.clone()),
            snapped_bar(15.0, 23.0, 17.0, 26.5, s, paint),
        ],
        // The filled triangle with the exclamation left open: the parts left
        // and right of its column, and the column's three pieces around the
        // stem and the dot. The edge from (16, 3) to (2, 28) is at y 4.79
        // where it crosses x 15.
        Fill::Full => vec![
            filled_poly(
                &[pt(15.0, 4.79), pt(15.0, 28.0), pt(2.0, 28.0)],
                s,
                paint.clone(),
            ),
            filled_poly(
                &[pt(17.0, 4.79), pt(30.0, 28.0), pt(17.0, 28.0)],
                s,
                paint.clone(),
            ),
            filled_poly(
                &[
                    pt(16.0, 3.0),
                    pt(17.0, 4.79),
                    pt(17.0, 11.0),
                    pt(15.0, 11.0),
                    pt(15.0, 4.79),
                ],
                s,
                paint.clone(),
            ),
            snapped_bar(15.0, 19.0, 17.0, 21.0, s, paint.clone()),
            snapped_bar(15.0, 23.0, 17.0, 28.0, s, paint),
        ],
    }
}

/// One small square of [`squares_arrow`], outer edge `lo` to `lo + 10`.
fn small_square(lo: f32, filled: bool, s: f32, color: &ColorRef) -> Command {
    if filled {
        snapped_bar(
            lo,
            lo,
            lo + 10.0,
            lo + 10.0,
            s,
            Paint::filled(color.clone()),
        )
    } else {
        let (a, b) = (lo + BAND / 2.0, lo + 10.0 - BAND / 2.0);
        outline(&[pt(a, a), pt(b, a), pt(b, b), pt(a, b)], s, color.clone())
    }
}

/// Two small squares, top left and bottom right, with an arrow from the first
/// to the second. The half form fills the second square only.
pub(super) fn squares_arrow(fill: Fill, s: f32, color: ColorRef) -> Vec<Command> {
    let (first, second) = match fill {
        Fill::Outline => (false, false),
        Fill::Half => (false, true),
        Fill::Full => (true, true),
    };
    let paint = Paint::filled(color.clone());
    vec![
        small_square(3.0, first, s, &color),
        small_square(19.0, second, s, &color),
        band(pt(12.0, 12.0), pt(15.5, 15.5), BAND, s, paint.clone()),
        filled_poly(&[pt(18.0, 13.0), pt(18.0, 18.0), pt(13.0, 18.0)], s, paint),
    ]
}

/// The plus inside [`square_plus`] and [`square_plus_dashed`].
fn plus(s: f32, paint: &Paint) -> [Command; 2] {
    [
        snapped_bar(15.0, 10.0, 17.0, 22.0, s, paint.clone()),
        snapped_bar(10.0, 15.0, 22.0, 17.0, s, paint.clone()),
    ]
}

/// A square holding a plus.
pub(super) fn square_plus(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let mut out = vec![outline(&SQUARE, s, color)];
    out.extend(plus(s, &paint));
    out
}

/// [`square_plus`] with its square cut into dashes.
pub(super) fn square_plus_dashed(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out = Vec::new();
    for side in [
        (4.0, 4.0, 28.0, 6.0),
        (4.0, 26.0, 28.0, 28.0),
        (4.0, 4.0, 6.0, 28.0),
        (26.0, 4.0, 28.0, 28.0),
    ] {
        out.extend(dashed_bar(side, 4.0, 3.0, s, &paint));
    }
    out.extend(plus(s, &paint));
    out
}

/// A square inside a square.
pub(super) fn square_in_square(s: f32, color: ColorRef) -> Vec<Command> {
    vec![
        outline(&SQUARE, s, color.clone()),
        outline(
            &[
                pt(12.0, 12.0),
                pt(20.0, 12.0),
                pt(20.0, 20.0),
                pt(12.0, 20.0),
            ],
            s,
            color,
        ),
    ]
}

/// An arrow pointing up: a stem and a head of two bands.
pub(super) fn arrow_up(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    vec![
        snapped_bar(15.0, 7.0, 17.0, 27.0, s, paint.clone()),
        band(pt(16.0, 6.0), pt(7.5, 14.5), BAND, s, paint.clone()),
        band(pt(16.0, 6.0), pt(24.5, 14.5), BAND, s, paint),
    ]
}

/// An arrow pointing down: [`arrow_up`] mirrored.
pub(super) fn arrow_down(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    vec![
        snapped_bar(15.0, 5.0, 17.0, 25.0, s, paint.clone()),
        band(pt(16.0, 26.0), pt(7.5, 17.5), BAND, s, paint.clone()),
        band(pt(16.0, 26.0), pt(24.5, 17.5), BAND, s, paint),
    ]
}
