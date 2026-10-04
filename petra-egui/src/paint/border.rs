//! The four-sided outline: the `border` slot stroked around a node's rect,
//! or around its silhouette polygon.

use egui::{Color32, Painter, Stroke};
use gorgon_petra::frame::PaintContent;
use gorgon_petra::token::resolve_slot;

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
        match &outline {
            None => {
                painter.rect_stroke(
                    rect,
                    corner_radius,
                    Stroke::new(width, color),
                    egui::StrokeKind::Inside,
                );
            }
            // A polygon stroke is centred on its path rather than inset the
            // way `StrokeKind::Inside` insets a rect's. At the one-unit
            // width this painter draws, that is half a logical unit of
            // overhang on a marker whose whole job is to be a recognisable
            // outline; correcting it would mean insetting the polygon, and
            // an inset that shrinks a triangle is not the same operation on
            // every figure. Left centred, and named here rather than
            // silently different.
            Some(points) => {
                painter.add(egui::Shape::convex_polygon(
                    points.clone(),
                    Color32::TRANSPARENT,
                    Stroke::new(width, color),
                ));
            }
        }
        shapes += 1;
    }

    shapes
}
