//! The per-edge rules: each bound `border-*` edge slot stroked as one line
//! segment down the inside of that edge, or grooved when it binds the rule
//! material.

use egui::{Painter, Stroke};
use gorgon_petra::frame::PaintContent;
use gorgon_petra::token::resolve_slot;

use super::{
    BORDER_BOTTOM_SLOT, BORDER_LEFT_SLOT, BORDER_RIGHT_SLOT, BORDER_TOP_SLOT, EDGE_SLOTS,
    NodeChrome, PaintEnv, PaintReport, device_snapped_width, edge_segment, paint_rule_groove,
    resolve_or_record,
};

pub(super) fn paint_edge_rules(
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

    // One edge at a time. Each bound edge slot is one line segment down the
    // inside of that edge — the same placement `StrokeKind::Inside` gives
    // the four-sided outline `paint_border` draws, so a node that swaps
    // `border` for `border-bottom` keeps its bottom rule on the same device
    // pixels. A silhouette has no edges to pick from, so on a non-rect
    // figure the slot is recorded undrawn rather than drawn on the wrong
    // shape.
    //
    // Any edge bound to the rule material's trigger token —
    // `gorgon_petra::token::rule::MATERIAL_TOKEN`, the same string as
    // `border.subtle` — grooves instead of drawing the flat line every other
    // edge slot draws: see [`paint_rule_groove`]. All four edges, since the
    // light moved to the top-left on 2026-09-09; before that a vertical
    // groove had both walls at the same angle to an overhead light and
    // painted a uniform darkening rather than a bevel.
    //
    // Unless the bound edges would meet at a corner. While rules were
    // horizontal only, the contract's "never meet a corner" mandate cost
    // nothing to keep: no stroke could reach one. Vertical rules remove that,
    // so `corner_free` decides it explicitly, and a node binding a horizontal
    // and a vertical edge to the material draws two flat lines rather than
    // two walls of a bevelled box. Computed once, before the loop, because
    // the answer is about the *set* of bound edges and no single pass through
    // the loop can see it.
    let grooves = {
        let material = |slot: &str| {
            resolve_slot(&content.tokens, slot, state)
                .is_some_and(|token| token == gorgon_petra::token::rule::MATERIAL_TOKEN)
        };
        gorgon_petra::token::rule::corner_free(
            material(BORDER_TOP_SLOT),
            material(BORDER_RIGHT_SLOT),
            material(BORDER_BOTTOM_SLOT),
            material(BORDER_LEFT_SLOT),
        )
    };
    for (slot, edge) in EDGE_SLOTS {
        let Some(token) = resolve_slot(&content.tokens, slot, state) else {
            continue;
        };
        let Some(color) = resolve_or_record(env.colors, token, report) else {
            continue;
        };
        if outline.is_some() {
            report.undrawn.insert(slot.to_owned());
            continue;
        }
        if grooves && token == gorgon_petra::token::rule::MATERIAL_TOKEN {
            shapes += paint_rule_groove(painter, rect, edge, corner_radius, env, report);
            continue;
        }
        let width = device_snapped_width(1.0, env.scale);
        painter.line_segment(
            edge_segment(rect, edge, width, corner_radius),
            Stroke::new(width, color),
        );
        shapes += 1;
    }

    shapes
}
