//! Action marks: the glyphs a command row hangs at its leading edge (new,
//! move, trash, undo, collapse).
//!
//! Each follows its Carbon namesake's outline in the 32-unit viewBox at the
//! 2-unit band, scaled by `s`, the way the file marks do: axis-aligned bands
//! are snapped bars, diagonal ones convex quads, curved ones strokes down
//! the band's centreline. Carbon's 1- and 2-unit corner radii are dropped.

use super::files::BAND;
use super::signs::arc;
use super::{filled_quad, pt, snapped_bar};
use crate::draw::{ColorRef, Command, Paint};

/// Carbon's `+` badge in the lower right corner of `document--add` and
/// `folder--add`: a 10-unit cross centred on (25, 25).
fn add_badge(paint: &Paint, s: f32) -> [Command; 2] {
    [
        snapped_bar(24.0, 20.0, 26.0, 30.0, s, paint.clone()),
        snapped_bar(20.0, 24.0, 30.0, 26.0, s, paint.clone()),
    ]
}

/// Bars `(x0, y0, x1, y1)` in the 32 box, as snapped commands.
fn bars(spans: &[(f32, f32, f32, f32)], s: f32, paint: &Paint) -> Vec<Command> {
    spans
        .iter()
        .map(|&(x0, y0, x1, y1)| snapped_bar(x0, y0, x1, y1, s, paint.clone()))
        .collect()
}

/// `IconMark::DocumentAdd`: Carbon `document--add`. The document's sheet
/// with its right side and bottom cut short where the badge sits.
pub(super) fn document_add(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out = bars(
        &[
            // Left, top, the right side down to the badge, the bottom up to it.
            (6.0, 2.0, 8.0, 30.0),
            (8.0, 2.0, 18.0, 4.0),
            (24.0, 10.0, 26.0, 16.0),
            (8.0, 28.0, 16.0, 30.0),
            // The fold: its left side and its lower edge.
            (16.0, 4.0, 18.0, 12.0),
            (18.0, 10.0, 24.0, 12.0),
        ],
        s,
        &paint,
    );
    out.push(filled_quad(
        [pt(18.0, 2.0), pt(26.0, 10.0), pt(23.6, 10.0), pt(18.0, 4.4)],
        s,
        paint.clone(),
    ));
    out.extend(add_badge(&paint, s));
    out
}

/// `IconMark::FolderAdd`: Carbon `folder--add`. Petra's folder with its
/// right side and bottom cut short where the badge sits.
pub(super) fn folder_add(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out = bars(
        &[
            (2.0, 4.0, 4.0, 28.0),
            (4.0, 4.0, 11.17, 6.0),
            (15.17, 8.0, 30.0, 10.0),
            (28.0, 10.0, 30.0, 18.0),
            (4.0, 26.0, 16.0, 28.0),
        ],
        s,
        &paint,
    );
    out.push(filled_quad(
        [
            pt(11.17, 4.0),
            pt(16.0, 8.0),
            pt(15.17, 10.0),
            pt(11.17, 6.0),
        ],
        s,
        paint.clone(),
    ));
    out.extend(add_badge(&paint, s));
    out
}

/// `IconMark::FolderMoveTo`: Carbon `folder--move-to`. A folder from 4 to
/// 28 holding a right-pointing arrow.
pub(super) fn folder_move_to(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let mut out = bars(
        &[
            // The folder: left, top-left, top-right, right, bottom.
            (4.0, 4.0, 6.0, 28.0),
            (6.0, 4.0, 11.59, 6.0),
            (15.59, 8.0, 28.0, 10.0),
            (26.0, 10.0, 28.0, 28.0),
            (6.0, 26.0, 26.0, 28.0),
            // The arrow's stem.
            (10.0, 17.0, 17.6, 19.0),
        ],
        s,
        &paint,
    );
    // The tab's slope.
    out.push(filled_quad(
        [
            pt(11.59, 4.0),
            pt(16.41, 8.0),
            pt(15.59, 10.0),
            pt(11.59, 6.0),
        ],
        s,
        paint.clone(),
    ));
    // The head: two wings meeting at the tip (21, 18). Two quads because a
    // filled path must be convex.
    out.push(filled_quad(
        [
            pt(16.0, 13.0),
            pt(21.0, 18.0),
            pt(18.17, 18.0),
            pt(14.59, 14.41),
        ],
        s,
        paint.clone(),
    ));
    out.push(filled_quad(
        [
            pt(18.17, 18.0),
            pt(21.0, 18.0),
            pt(16.0, 23.0),
            pt(14.59, 21.59),
        ],
        s,
        paint,
    ));
    out
}

/// `IconMark::TrashCan`: Carbon `trash-can`. Handle, lid, an open-topped
/// body and two slats.
pub(super) fn trash_can(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    bars(
        &[
            // Handle and lid.
            (12.0, 2.0, 20.0, 4.0),
            (4.0, 6.0, 28.0, 8.0),
            // Body: left, right, bottom.
            (6.0, 8.0, 8.0, 30.0),
            (24.0, 8.0, 26.0, 30.0),
            (8.0, 28.0, 24.0, 30.0),
            // Slats.
            (12.0, 12.0, 14.0, 24.0),
            (18.0, 12.0, 20.0, 24.0),
        ],
        s,
        &paint,
    )
}

/// `IconMark::Undo`: Carbon `undo`. A left-pointing head on a stem that
/// turns down a half circle (centre (20, 18), centreline radius 7) and
/// comes back as the bottom bar.
pub(super) fn undo(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    let mut out = bars(
        &[
            // The stem from the head's notch to the turn, and the bottom bar.
            (6.8, 10.0, 20.0, 12.0),
            (12.0, 24.0, 20.0, 26.0),
        ],
        s,
        &paint,
    );
    // The head: the tip at (4, 11), wings to (10, 5) and (10, 17), split at
    // the tip's row so each half is convex.
    out.push(filled_quad(
        [
            pt(4.0, 11.0),
            pt(10.0, 5.0),
            pt(11.41, 6.41),
            pt(6.82, 11.0),
        ],
        s,
        paint.clone(),
    ));
    out.push(filled_quad(
        [
            pt(4.0, 11.0),
            pt(6.82, 11.0),
            pt(11.41, 15.59),
            pt(10.0, 17.0),
        ],
        s,
        paint,
    ));
    // Top of the circle, clockwise through its right side, to the bottom.
    out.push(arc(
        pt(20.0, 18.0),
        8.0 - BAND / 2.0,
        270.0,
        180.0,
        s,
        color,
    ));
    out
}

/// `IconMark::CollapseAll`: Carbon `collapse-all`. A box with two
/// receding corner brackets behind it: a stack folded into one.
pub(super) fn collapse_all(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    bars(
        &[
            // The back bracket: top and right.
            (13.0, 5.0, 30.0, 7.0),
            (28.0, 7.0, 30.0, 15.0),
            // The middle bracket.
            (8.0, 10.0, 25.0, 12.0),
            (23.0, 12.0, 25.0, 20.0),
            // The front box: top, bottom, left, right.
            (2.0, 15.0, 20.0, 17.0),
            (2.0, 25.0, 20.0, 27.0),
            (2.0, 17.0, 4.0, 25.0),
            (18.0, 17.0, 20.0, 25.0),
        ],
        s,
        &paint,
    )
}
