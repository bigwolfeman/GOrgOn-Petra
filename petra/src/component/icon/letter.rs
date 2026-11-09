//! `letter_badge`: a solid diamond with one capital letter cut out of it.
//!
//! A badge names a kind with a letter where a picture would be a guess: a
//! file tree's repository, linked worktree and submodule are `R`, `W` and
//! `S` in the Git diamond (GOrgOn spec 016, `contracts/projections.md`
//! "Marks", operator review 2026-11-08). The letter is a shape, so the
//! badge holds without its colour, and the badge is one ink, so it holds
//! for a reader who cannot tell red from green.
//!
//! # The letter is a hole
//!
//! The letter is not ink painted over the diamond in some ground colour: a
//! row's ground changes under hover and selection, and a painted letter
//! would keep the old one. The diamond is drawn around the letter instead,
//! so whatever lies under the badge shows through it. [`DrawList::new`]
//! takes only convex fills, so the diamond is four convex pieces round the
//! letter's box (the tip above, the tip below, the left and right ends)
//! plus one snapped bar per run of unlit cells inside the box.
//!
//! The letter is drawn on whole units from a 5 by 7 face (`M` and `W` are 7
//! wide, `I` is 3), so at one device pixel per unit every cut is a pixel
//! edge, and the pieces meet on whole pixels with no seam. Only the four
//! diagonal edges antialias.

use super::{IconTone, filled_poly, filled_tri, mark_canvas, pt, snapped_bar};
use crate::component::tokens::t;
use crate::draw::{ColorRef, Command, DrawList, Paint};
use crate::tree::{Key, ViewNode};

/// The badge's node extent on both axes: 18. The 16 glyph box crowds a
/// 7-wide letter against the diamond's sides; a tree row is 24, so 18
/// centres on whole pixels.
pub const BADGE_BOX: f32 = 18.0;

/// The diamond's centre on both axes. The letters are odd widths and seven
/// rows, so they centre on a half unit; the diamond centres with them, half
/// a unit up and left of the box's middle.
const CENTRE: f32 = 8.5;
/// The diamond's reach from its centre to each tip.
const REACH: f32 = 8.2;
/// The letter's first row.
const TOP: u8 = 5;
/// Rows in every letter.
const ROWS: u8 = 7;

/// The face: one string per row, `#` a cut cell. Upper case only; every
/// letter is seven rows of one width. Kept as a grid: one row per string,
/// so a letter reads as its picture.
#[rustfmt::skip]
fn face(letter: char) -> [&'static str; 7] {
    match letter {
        'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'B' => ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."],
        'C' => [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."],
        'D' => ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
        'E' => ["#####", "#....", "#....", "####.", "#....", "#....", "#####"],
        'F' => ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
        'G' => [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".###."],
        'H' => ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'I' => ["###", ".#.", ".#.", ".#.", ".#.", ".#.", "###"],
        'J' => ["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."],
        'K' => ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
        'L' => ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
        'M' => [
            "#.....#", "##...##", "#.#.#.#", "#..#..#", "#.....#", "#.....#", "#.....#",
        ],
        'N' => ["#...#", "##..#", "##..#", "#.#.#", "#..##", "#..##", "#...#"],
        'O' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'P' => ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
        'Q' => [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"],
        'R' => ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
        'S' => [".###.", "#...#", "#....", ".###.", "....#", "#...#", ".###."],
        'T' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
        'U' => ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'V' => ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."],
        'W' => [
            "#.....#", "#.....#", "#..#..#", "#..#..#", "#.#.#.#", "##...##", "#.....#",
        ],
        'X' => ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"],
        'Y' => ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."],
        'Z' => ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"],
        _ => [".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."],
    }
}

/// A solid diamond in `tone` with `letter` cut out of it, [`BADGE_BOX`]
/// square. `letter` is upper-cased; anything outside `A` to `Z` cuts `?`.
/// No role and no label: as with [`super::icon`], the row that holds the
/// badge names it.
#[must_use]
pub fn letter_badge(key: impl Into<Key>, letter: char, tone: IconTone) -> ViewNode {
    let color = ColorRef::Token(t(tone.token()).as_str().to_owned());
    let list = DrawList::new(badge(letter, &Paint::filled(color)))
        .unwrap_or_else(|err| panic!("letter badge {letter:?} refused: {err}"));
    mark_canvas(key, list, BADGE_BOX, tone)
}

/// The diamond's half-width at height `y`.
fn half_width(y: f32) -> f32 {
    REACH - (y - CENTRE).abs()
}

/// The badge's pieces, in [`BADGE_BOX`] units.
fn badge(letter: char, paint: &Paint) -> Vec<Command> {
    let rows = face(letter.to_ascii_uppercase());
    let width = u8::try_from(rows[0].len()).expect("a face row is a few cells");
    let left = (BADGE_BOX as u8 - 1 - width) / 2;
    let (x0, x1) = (f32::from(left), f32::from(left + width));
    let (y0, y1) = (f32::from(TOP), f32::from(TOP + ROWS));
    let (w0, w1) = (half_width(y0), half_width(y1));
    let mut out = vec![
        filled_tri(
            [
                pt(CENTRE, CENTRE - REACH),
                pt(CENTRE + w0, y0),
                pt(CENTRE - w0, y0),
            ],
            1.0,
            paint.clone(),
        ),
        filled_tri(
            [
                pt(CENTRE - w1, y1),
                pt(CENTRE + w1, y1),
                pt(CENTRE, CENTRE + REACH),
            ],
            1.0,
            paint.clone(),
        ),
        filled_poly(
            &[
                pt(CENTRE - w0, y0),
                pt(x0, y0),
                pt(x0, y1),
                pt(CENTRE - w1, y1),
                pt(CENTRE - REACH, CENTRE),
            ],
            1.0,
            paint.clone(),
        ),
        filled_poly(
            &[
                pt(x1, y0),
                pt(CENTRE + w0, y0),
                pt(CENTRE + REACH, CENTRE),
                pt(CENTRE + w1, y1),
                pt(x1, y1),
            ],
            1.0,
            paint.clone(),
        ),
    ];
    for (row, cells) in (TOP..).zip(rows) {
        let mut run: Option<u8> = None;
        for (column, cell) in (left..).zip(cells.bytes().chain(*b"#")) {
            match (cell == b'#', run) {
                (false, None) => run = Some(column),
                (true, Some(start)) => {
                    out.push(bar(start, column, row, paint));
                    run = None;
                }
                _ => {}
            }
        }
    }
    out
}

/// The unlit cells `from..to` of one letter row, as one snapped bar.
fn bar(from: u8, to: u8, row: u8, paint: &Paint) -> Command {
    snapped_bar(
        f32::from(from),
        f32::from(row),
        f32::from(to),
        f32::from(row + 1),
        1.0,
        paint.clone(),
    )
}

/// Whether `p` lies inside the closed convex figure `corners` (either
/// winding), for the tests.
#[cfg(test)]
fn inside(corners: &[crate::geom::Point], p: crate::geom::Point) -> bool {
    let n = corners.len();
    let side = |i: usize| {
        let (a, b) = (corners[i], corners[(i + 1) % n]);
        (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
    };
    let signs: Vec<f32> = (0..n).map(side).collect();
    signs.iter().all(|s| *s >= 0.0) || signs.iter().all(|s| *s <= 0.0)
}

#[cfg(test)]
mod tests {
    use super::{BADGE_BOX, CENTRE, REACH, badge, face, inside, letter_badge};
    use crate::component::IconTone;
    use crate::draw::{ColorRef, Command, DrawList, Paint, PathVerb};
    use crate::geom::Point;
    use crate::tree::NodeKind;

    fn paint() -> Paint {
        Paint::filled(ColorRef::Token("icon-secondary".to_owned()))
    }

    /// How many pieces cover the centre of cell `(x, y)`.
    fn cover(commands: &[Command], x: u8, y: u8) -> usize {
        let p = Point::new(f32::from(x) + 0.5, f32::from(y) + 0.5);
        commands
            .iter()
            .filter(|command| match command {
                Command::Rect { rect, .. } => {
                    p.x > rect.x && p.x < rect.x + rect.w && p.y > rect.y && p.y < rect.y + rect.h
                }
                Command::Path { verbs, .. } => {
                    let corners: Vec<Point> = verbs
                        .iter()
                        .map(|verb| match verb {
                            PathVerb::MoveTo(q) | PathVerb::LineTo(q) => *q,
                            other => panic!("a badge piece is straight: {other:?}"),
                        })
                        .collect();
                    inside(&corners, p)
                }
                _ => false,
            })
            .count()
    }

    #[test]
    fn every_face_is_seven_rows_of_one_odd_width() {
        for letter in ('A'..='Z').chain(['?']) {
            let rows = face(letter);
            let width = rows[0].len();
            assert!(width % 2 == 1 && width <= 7, "{letter}: width {width}");
            assert!(rows.iter().all(|row| row.len() == width), "{letter}");
            assert!(
                rows.iter()
                    .all(|row| row.bytes().all(|b| b == b'#' || b == b'.'))
            );
        }
        assert_eq!(face('é'), face('?'), "anything else cuts a question mark");
    }

    /// Inside the diamond every cell is covered exactly once, except the
    /// letter's cells, which nothing covers: the letter is a hole, and no
    /// two pieces overlap into a darker seam.
    #[test]
    fn the_letter_is_a_hole_and_the_rest_is_covered_once() {
        for letter in ('A'..='Z').chain(['?']) {
            let commands = badge(letter, &paint());
            DrawList::new(commands.clone()).expect("every piece is convex");
            let rows = face(letter);
            let width = u8::try_from(rows[0].len()).unwrap();
            let left = (BADGE_BOX as u8 - 1 - width) / 2;
            for y in 0..BADGE_BOX as u8 {
                for x in 0..BADGE_BOX as u8 {
                    let (cx, cy) = (f32::from(x) + 0.5, f32::from(y) + 0.5);
                    let lit = (5..12).contains(&y)
                        && (left..left + width).contains(&x)
                        && rows[usize::from(y - 5)].as_bytes()[usize::from(x - left)] == b'#';
                    let within = (cx - CENTRE).abs() + (cy - CENTRE).abs() < REACH - 0.75;
                    let n = cover(&commands, x, y);
                    if lit {
                        assert_eq!(n, 0, "{letter}: cell ({x}, {y}) is cut");
                    } else if within {
                        assert_eq!(n, 1, "{letter}: cell ({x}, {y}) is covered once");
                    } else {
                        assert!(n <= 1, "{letter}: cell ({x}, {y}) overlaps");
                    }
                }
            }
        }
    }

    #[test]
    fn the_badge_is_a_canvas_in_its_box_bound_to_its_tone() {
        let node = letter_badge("b", 'w', IconTone::Secondary);
        assert_eq!(node.kind, NodeKind::Canvas);
        assert_eq!(node.constraints.horizontal.min, Some(BADGE_BOX));
        assert_eq!(node.constraints.vertical.max, Some(BADGE_BOX));
        assert!(node.props.tokens.contains_key("icon-secondary"));
        assert!(node.props.tokens.contains_key("icon-secondary@disabled"));
        let lower = letter_badge("b", 'w', IconTone::Secondary);
        let upper = letter_badge("b", 'W', IconTone::Secondary);
        assert_eq!(
            lower.props.canvas, upper.props.canvas,
            "letters are upper-cased"
        );
    }
}
