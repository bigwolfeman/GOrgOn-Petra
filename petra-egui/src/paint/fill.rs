//! The background fill: the `background` slot painted onto a node's rect, or
//! the rule material's groove for a node that **is** the rule
//! (`gorgon_petra::tree::NodeKind::Separator`).

use egui::{Painter, Stroke};
use gorgon_petra::frame::{PaintContent, Placement};
use gorgon_petra::token::resolve_slot;
use gorgon_petra::tree::Edge;

use super::{
    BACKGROUND_SLOT, NodeChrome, PaintEnv, PaintReport, paint_rule_groove, resolve_or_record,
};

pub(super) fn paint_fill(
    painter: &Painter,
    placement: &Placement,
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

    if let Some(token) = resolve_slot(&content.tokens, BACKGROUND_SLOT, state) {
        // A separator does not have a rule along one edge. It **is** the
        // rule, so its whole rect is the groove and its own fill slot is
        // what names the material — there is no edge slot to bind, because
        // there is nothing else in the node for an edge to belong to.
        //
        // This is the second of the two constructions
        // `gorgon_petra::token::rule`'s module doc describes, and it exists
        // because the first one cannot express a standalone divider. Seven
        // of them shipped as a childless stack with a `border.subtle` fill,
        // which painted a flat line while the same token grooved on a table
        // row, and the author of each one had to pick a thickness by hand.
        //
        // Orientation comes off the rect rather than off `props.axis`,
        // which the frame does not carry: a rule is thin across itself, so
        // the long side is the run. `Edge::Top` and `Edge::Left` are the
        // shadow-first arms, and since the rect is exactly the groove's
        // thickness the opposite arms would land on the same pixels. A
        // square separator is degenerate and takes the horizontal reading.
        let separator = placement.kind == gorgon_petra::tree::NodeKind::Separator;
        if separator && outline.is_none() && token == gorgon_petra::token::rule::MATERIAL_TOKEN {
            let edge = if rect.width() >= rect.height() {
                Edge::Top
            } else {
                Edge::Left
            };
            shapes += paint_rule_groove(painter, rect, edge, corner_radius, env, report);
        } else if let Some(color) = resolve_or_record(env.colors, token, report) {
            match &outline {
                None => {
                    painter.rect_filled(rect, corner_radius, color);
                }
                Some(points) => {
                    painter.add(egui::Shape::convex_polygon(
                        points.clone(),
                        color,
                        Stroke::NONE,
                    ));
                }
            }
            report.fills += 1;
            shapes += 1;
        }
    }

    shapes
}
