//! The four-sided outline: the `border` slot stroked around a node's rect,
//! or around its silhouette polygon, in the line its `border-style` slot
//! names ([`BorderStyle`]).

use egui::{Color32, Painter, Pos2, Rect, Stroke};
use gorgon_petra::frame::PaintContent;
use gorgon_petra::token::{BorderStyle, resolve_slot};

use super::{
    BORDER_SLOT, NodeChrome, PaintEnv, PaintReport, device_snapped_width, resolve_or_record,
};

pub(super) fn paint_border(
    painter: &Painter,
    content: &PaintContent,
    chrome: &NodeChrome,
    env: &mut PaintEnv<'_>,
    report: &mut PaintReport,
) -> usize {
    let rect = chrome.rect;
    let state = chrome.state;
    let outline = &chrome.outline;
    let corner_radius = chrome.corner_radius;
    let mut shapes = 0_usize;

    if let Some(token) = resolve_slot(&content.tokens, BORDER_SLOT, state)
        && let Some(color) = resolve_or_record(env.colors, token, report)
    {
        let width = device_snapped_width(1.0, env.scale);
        let stroke = Stroke::new(width, color);
        match (&outline, chrome.border_style) {
            (None, BorderStyle::Solid) => {
                painter.rect_stroke(rect, corner_radius, stroke, egui::StrokeKind::Inside);
                shapes += 1;
            }
            // A polygon stroke is centred on its path rather than inset the
            // way `StrokeKind::Inside` insets a rect's. At the one-unit
            // width this painter draws, that is half a logical unit of
            // overhang on a marker whose whole job is to be a recognisable
            // outline; correcting it would mean insetting the polygon, and
            // an inset that shrinks a triangle is not the same operation on
            // every figure. Left centred, and named here rather than
            // silently different. A notch is a rect's cut corner, so a
            // silhouette keeps its own figure under `Notched`.
            (Some(points), BorderStyle::Solid | BorderStyle::Notched) => {
                painter.add(egui::Shape::convex_polygon(
                    points.clone(),
                    Color32::TRANSPARENT,
                    stroke,
                ));
                shapes += 1;
            }
            (Some(points), BorderStyle::Dashed) => {
                shapes += dashed(painter, points, stroke);
            }
            (None, BorderStyle::Dashed) => {
                shapes += dashed(painter, &inset_corners(rect, width), stroke);
            }
            (None, BorderStyle::Notched) => {
                painter.add(egui::Shape::closed_line(notched(rect, width), stroke));
                shapes += 1;
            }
        }
    }

    shapes
}

/// A dash and the gap after it, in logical units: long enough that a dash
/// reads as a mark and not a dot at chip size, short enough that a 40-unit
/// side still shows several breaks.
pub(crate) const DASH: f32 = 3.0;
/// See [`DASH`].
pub(crate) const GAP: f32 = 2.0;

/// The longest corner cut of a notched outline, in logical units. A cut
/// never takes more than a quarter of the shorter side, so a small box
/// keeps flat sides between its notches.
pub(crate) const NOTCH: f32 = 5.0;

/// The rect's four corners, inset by half the stroke so the line sits
/// inside the node the way an inside-stroked rect does. Clockwise from
/// top-left.
fn inset_corners(rect: Rect, width: f32) -> Vec<Pos2> {
    let r = rect.shrink(width / 2.0);
    vec![
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ]
}

/// The closed path `points` stroked as dashes. A dashed rect ignores its
/// corner radius: a dash pattern on a curve at chip size reads as noise,
/// and the break in the line is the cue, not the corner.
fn dashed(painter: &Painter, points: &[Pos2], stroke: Stroke) -> usize {
    let mut path = points.to_vec();
    if let Some(first) = points.first() {
        path.push(*first);
    }
    let dashes = egui::Shape::dashed_line(&path, stroke, DASH, GAP);
    let count = dashes.len();
    painter.extend(dashes);
    count
}

/// The inset rect with its four corners cut on a diagonal of [`NOTCH`]
/// (at most a quarter of the shorter side). Clockwise from the top edge.
pub(crate) fn notched(rect: Rect, width: f32) -> Vec<Pos2> {
    let r = rect.shrink(width / 2.0);
    let cut = NOTCH.min(r.width() / 4.0).min(r.height() / 4.0).max(0.0);
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    vec![
        egui::pos2(l + cut, t),
        egui::pos2(rt - cut, t),
        egui::pos2(rt, t + cut),
        egui::pos2(rt, b - cut),
        egui::pos2(rt - cut, b),
        egui::pos2(l + cut, b),
        egui::pos2(l, b - cut),
        egui::pos2(l, t + cut),
    ]
}
