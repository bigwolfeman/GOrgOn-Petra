//! The file marks: the glyphs a file tree hangs on a row (GOrgOn spec 016,
//! `contracts/projections.md` "Marks").
//!
//! Every glyph is in the 32-unit viewBox and scaled by `s`, as every mark in
//! the parent module is. Carbon's 2-unit outline bands become snapped bars
//! where they are axis-aligned, convex quads where they are diagonal, and
//! strokes down the band's centreline where they curve. Carbon's 1- and
//! 2-unit corner radii are dropped: they are a pixel at 16.

use super::{filled_quad, pt, scaled, snapped_bar};
use crate::draw::{ColorRef, Command, Paint, PathVerb, Stroke, Width, arc_verbs};
use crate::geom::{Point, Size};

/// Carbon's outline band.
const BAND: f32 = 2.0;

/// A `width`-wide band down the centreline `a` to `b`, as one convex quad.
fn band(a: Point, b: Point, width: f32, s: f32, paint: Paint) -> Command {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt();
    let (nx, ny) = (-dy / len * width / 2.0, dx / len * width / 2.0);
    filled_quad(
        [
            pt(a.x + nx, a.y + ny),
            pt(a.x - nx, a.y - ny),
            pt(b.x - nx, b.y - ny),
            pt(b.x + nx, b.y + ny),
        ],
        s,
        paint,
    )
}

fn stroke(color: ColorRef, s: f32) -> Paint {
    Paint::stroked(Stroke {
        width: Width::Logical(BAND * s),
        color,
    })
}

/// A ring: the band's centreline circle at `radius`, stroked.
fn ring(center: Point, radius: f32, s: f32, color: ColorRef) -> Command {
    Command::Ellipse {
        center: scaled(center, s),
        radii: Size::new(radius * s, radius * s),
        paint: stroke(color, s),
    }
}

/// One stroked run: an optional straight lead-in from `from`, an arc about
/// `center` at `radius` from `start` degrees through `sweep` degrees
/// (clockwise on screen from three o'clock), and an optional straight
/// lead-out to `to`. The band is [`BAND`] wide.
#[allow(clippy::too_many_arguments)]
fn arc_run(
    from: Option<Point>,
    center: Point,
    radius: f32,
    start: f32,
    sweep: f32,
    to: Option<Point>,
    s: f32,
    color: ColorRef,
) -> Command {
    let arc = arc_verbs(
        scaled(center, s),
        radius * s,
        start.to_radians(),
        sweep.to_radians(),
    );
    let mut verbs = Vec::with_capacity(arc.len() + 2);
    match from {
        Some(lead) => {
            verbs.push(PathVerb::MoveTo(scaled(lead, s)));
            verbs.extend(arc.into_iter().map(|verb| match verb {
                PathVerb::MoveTo(p) => PathVerb::LineTo(p),
                other => other,
            }));
        }
        None => verbs.extend(arc),
    }
    if let Some(end) = to {
        verbs.push(PathVerb::LineTo(scaled(end, s)));
    }
    Command::Path {
        verbs,
        closed: false,
        paint: stroke(color, s),
    }
}

/// A closed stroked polygon down the band's centreline.
fn outline(corners: &[Point], s: f32, color: ColorRef) -> Command {
    let verbs = corners
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
        closed: true,
        paint: stroke(color, s),
    }
}

/// An axis-aligned bar from `(x0, y0)` to `(x1, y1)` cut into dashes along
/// its long axis. Both ends are dashes, so a corner where two dashed sides
/// meet stays closed; the dashes are spread evenly, each about `dash` long
/// with gaps about `gap`.
fn dashed_bar(
    (x0, y0, x1, y1): (f32, f32, f32, f32),
    dash: f32,
    gap: f32,
    s: f32,
    paint: &Paint,
) -> Vec<Command> {
    let horizontal = x1 - x0 >= y1 - y0;
    let (lo, hi) = if horizontal { (x0, x1) } else { (y0, y1) };
    let len = hi - lo;
    // `n` dashes and `n - 1` gaps fill the run exactly.
    let n = (((len + gap) / (dash + gap)).round() as usize).max(1);
    let unit = len / (n as f32 * dash + (n as f32 - 1.0) * gap);
    let (d, g) = (dash * unit, gap * unit);
    (0..n)
        .map(|k| {
            let a = lo + k as f32 * (d + g);
            let b = (a + d).min(hi);
            if horizontal {
                snapped_bar(a, y0, b, y1, s, paint.clone())
            } else {
                snapped_bar(x0, a, x1, b, s, paint.clone())
            }
        })
        .collect()
}

/// `IconMark::Launch`: Carbon `launch`.
pub(super) fn launch(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out: Vec<Command> = [
        // The box, open at its top right: left, top, bottom, right.
        (4.0, 4.0, 6.0, 28.0),
        (6.0, 4.0, 16.0, 6.0),
        (6.0, 26.0, 28.0, 28.0),
        (26.0, 16.0, 28.0, 26.0),
        // The arrowhead's two arms.
        (20.0, 2.0, 30.0, 4.0),
        (28.0, 4.0, 30.0, 12.0),
    ]
    .into_iter()
    .map(|(x0, y0, x1, y1)| snapped_bar(x0, y0, x1, y1, s, paint.clone()))
    .collect();
    // The shaft.
    out.push(filled_quad(
        [
            pt(18.0, 12.586),
            pt(19.414, 14.0),
            pt(28.0, 5.414),
            pt(26.586, 4.0),
        ],
        s,
        paint,
    ));
    out
}

/// `IconMark::LaunchOutside`: [`launch`] with a boundary bar down its left
/// edge, the line the link crosses.
pub(super) fn launch_outside(s: f32, color: ColorRef) -> Vec<Command> {
    let mut out = launch(s, color.clone());
    out.push(snapped_bar(0.0, 2.0, 2.0, 30.0, s, Paint::filled(color)));
    out
}

/// `IconMark::Unlink`: Carbon `unlink`, two link halves pulled apart with
/// four break ticks.
pub(super) fn unlink(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let mut out: Vec<Command> = [
        (11.0, 2.0, 13.0, 6.0),
        (2.0, 11.0, 6.0, 13.0),
        (26.0, 19.0, 30.0, 21.0),
        (19.0, 26.0, 21.0, 30.0),
    ]
    .into_iter()
    .map(|(x0, y0, x1, y1)| snapped_bar(x0, y0, x1, y1, s, paint.clone()))
    .collect();
    // The two diagonal ticks, Carbon's 2 by 4.83 bars turned by 45 degrees.
    out.push(band(pt(4.29, 4.29), pt(7.71, 7.71), BAND, s, paint.clone()));
    out.push(band(pt(24.29, 24.29), pt(27.71, 27.71), BAND, s, paint));
    // Each half: a straight side, a half turn, a straight side, down the
    // centreline between Carbon's radius-4 and radius-6 edges.
    out.push(arc_run(
        Some(pt(17.29, 21.78)),
        pt(10.04, 21.96),
        5.0,
        45.0,
        180.0,
        Some(pt(10.22, 14.7)),
        s,
        color.clone(),
    ));
    out.push(arc_run(
        Some(pt(14.71, 10.22)),
        pt(21.96, 10.04),
        5.0,
        225.0,
        180.0,
        Some(pt(21.78, 17.29)),
        s,
        color,
    ));
    out
}

/// `IconMark::Locked`: Carbon `locked`.
pub(super) fn locked(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let mut out: Vec<Command> = [
        // The body: left, right, top, bottom.
        (6.0, 14.0, 8.0, 30.0),
        (24.0, 14.0, 26.0, 30.0),
        (8.0, 14.0, 24.0, 16.0),
        (8.0, 28.0, 24.0, 30.0),
        // The shackle's legs.
        (10.0, 8.0, 12.0, 14.0),
        (20.0, 8.0, 22.0, 14.0),
    ]
    .into_iter()
    .map(|(x0, y0, x1, y1)| snapped_bar(x0, y0, x1, y1, s, paint.clone()))
    .collect();
    // The shackle's arch, between Carbon's radius-4 and radius-6 edges.
    out.push(arc_run(
        None,
        pt(16.0, 8.0),
        5.0,
        180.0,
        180.0,
        None,
        s,
        color,
    ));
    out
}

/// `IconMark::ViewOff`: Carbon `view--off`, the eye with a slash.
pub(super) fn view_off(s: f32, color: ColorRef) -> Vec<Command> {
    vec![
        // The lens, down the centreline of Carbon's two arcs.
        Command::Ellipse {
            center: scaled(pt(16.0, 16.0), s),
            radii: Size::new(14.0 * s, 10.0 * s),
            paint: stroke(color.clone(), s),
        },
        ring(pt(16.0, 16.0), 5.0, s, color.clone()),
        band(pt(29.3, 2.7), pt(2.7, 29.3), BAND, s, Paint::filled(color)),
    ]
}

/// `IconMark::FolderDashed`: Petra's `Folder` outline cut into dashes.
pub(super) fn folder_dashed(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out: Vec<Command> = [
        (2.0, 4.0, 4.0, 28.0),
        (4.0, 4.0, 11.17, 6.0),
        (15.17, 8.0, 30.0, 10.0),
        (28.0, 10.0, 30.0, 28.0),
        (4.0, 26.0, 28.0, 28.0),
    ]
    .into_iter()
    .flat_map(|side| dashed_bar(side, 4.0, 2.5, s, &paint))
    .collect();
    out.push(filled_quad(
        [
            pt(11.17, 4.0),
            pt(16.0, 8.0),
            pt(15.17, 10.0),
            pt(11.17, 6.0),
        ],
        s,
        paint,
    ));
    out
}

/// How a branch glyph draws its three nodes.
#[derive(Clone, Copy, PartialEq)]
enum Node {
    /// A hollow ring.
    Ring,
    /// Hollow squares.
    Square,
}

/// One cell of the 16 pixel grid the branch glyphs are drawn on, or a run of
/// them, as a snapped bar in the 32 box: pixel `p` is units `2p..2p + 2`.
///
/// Carbon's 32 box scaled to 16 put 1 pixel strokes on pixel edges and
/// 1.5 pixel rings into 4 pixel blots (operator review 2026-11-08: "the git
/// icon in the box looks bad"). These glyphs are drawn for 16 pixels, every
/// stroke on whole pixels, so at the tree's size they are crisp; at 32 they
/// are the same drawing at 2 pixel strokes.
fn cells(x0: u8, y0: u8, x1: u8, y1: u8, s: f32, paint: &Paint) -> Command {
    snapped_bar(
        f32::from(x0) * 2.0,
        f32::from(y0) * 2.0,
        f32::from(x1) * 2.0 + 2.0,
        f32::from(y1) * 2.0 + 2.0,
        s,
        paint.clone(),
    )
}

/// A node centred on pixel `(cx, cy)`, 5 pixels across: a ring has its
/// corners cut, so at 5 pixels it reads round and keeps a 3 by 3 hole.
fn node(cx: u8, cy: u8, shape: Node, s: f32, paint: &Paint) -> Vec<Command> {
    let cut = u8::from(shape == Node::Ring);
    vec![
        cells(cx - 2 + cut, cy - 2, cx + 2 - cut, cy - 2, s, paint),
        cells(cx - 2 + cut, cy + 2, cx + 2 - cut, cy + 2, s, paint),
        cells(cx - 2, cy - 1, cx - 2, cy + 1, s, paint),
        cells(cx + 2, cy - 1, cx + 2, cy + 1, s, paint),
    ]
}

/// A branch on the 16 grid: nodes at top left, bottom left and top right,
/// the trunk between the left two, and the side branch down from the right
/// node and across onto the trunk at a square corner, the way a commit graph
/// draws it.
fn branch_parts(shape: Node, s: f32, color: &ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let mut out = node(4, 3, shape, s, &paint);
    out.extend(node(4, 12, shape, s, &paint));
    out.extend(node(11, 3, shape, s, &paint));
    // The trunk, the drop from the leaf, and the bar across onto the trunk.
    out.push(cells(4, 6, 4, 9, s, &paint));
    out.push(cells(11, 6, 11, 8, s, &paint));
    out.push(cells(5, 8, 10, 8, s, &paint));
    out
}

/// `IconMark::Branch`: a branch with three hollow nodes.
pub(super) fn branch(s: f32, color: ColorRef) -> Vec<Command> {
    branch_parts(Node::Ring, s, &color)
}

/// `IconMark::BranchWorktree`: [`branch`] with its leaf solid, the
/// checkout that hangs off another repository.
pub(super) fn branch_worktree(s: f32, color: ColorRef) -> Vec<Command> {
    let mut out = branch_parts(Node::Ring, s, &color);
    out.push(cells(10, 2, 12, 4, s, &Paint::filled(color)));
    out
}

/// `IconMark::BranchBoxed`: [`branch`] with square nodes. A frame round a
/// 16 pixel branch left a pixel between frame and branch, and the frame was
/// the heaviest line in the tree; the box moves into the nodes.
pub(super) fn branch_boxed(s: f32, color: ColorRef) -> Vec<Command> {
    branch_parts(Node::Square, s, &color)
}

/// `IconMark::Unknown`: Carbon `unknown`, a diamond holding a question
/// mark, which is the shape of U+FFFD, the replacement character.
pub(super) fn unknown(s: f32, color: ColorRef) -> Vec<Command> {
    vec![
        outline(
            &[pt(16.0, 3.4), pt(28.6, 16.0), pt(16.0, 28.6), pt(3.4, 16.0)],
            s,
            color.clone(),
        ),
        // The hook: up from the left end, over the top, round the right
        // side, back to the stem.
        arc_run(
            Some(pt(12.0, 13.5)),
            pt(15.0, 13.0),
            3.0,
            180.0,
            90.0,
            None,
            s,
            color.clone(),
        ),
        arc_run(
            Some(pt(15.0, 10.0)),
            pt(17.0, 13.0),
            3.0,
            270.0,
            180.0,
            Some(pt(16.0, 16.0)),
            s,
            color.clone(),
        ),
        snapped_bar(15.0, 16.0, 17.0, 19.0, s, Paint::filled(color.clone())),
        Command::Ellipse {
            center: scaled(pt(16.0, 22.5), s),
            radii: Size::new(1.5 * s, 1.5 * s),
            paint: Paint::filled(color),
        },
    ]
}

/// `IconMark::DocumentInvalid`: Petra's `Document` sheet with a cross where
/// its text lines were.
pub(super) fn document_invalid(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out: Vec<Command> = [
        (6.0, 2.0, 8.0, 30.0),
        (8.0, 2.0, 18.0, 4.0),
        (24.0, 10.0, 26.0, 30.0),
        (8.0, 28.0, 24.0, 30.0),
        (16.0, 4.0, 18.0, 12.0),
        (18.0, 10.0, 24.0, 12.0),
    ]
    .into_iter()
    .map(|(x0, y0, x1, y1)| snapped_bar(x0, y0, x1, y1, s, paint.clone()))
    .collect();
    out.push(filled_quad(
        [pt(18.0, 2.0), pt(26.0, 10.0), pt(23.6, 10.0), pt(18.0, 4.4)],
        s,
        paint.clone(),
    ));
    out.push(band(pt(11.5, 15.5), pt(20.5, 24.5), BAND, s, paint.clone()));
    out.push(band(pt(20.5, 15.5), pt(11.5, 24.5), BAND, s, paint));
    out
}
