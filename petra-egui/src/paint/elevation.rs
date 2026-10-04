//! The elevation one placement casts: the `background`-adjacent `shadow`
//! slot's shadow, when the node binds one.
//!
//! Painted first of the kinds `super::paint_one` dispatches to, because a
//! shadow lies behind the thing that casts it.

use egui::Painter;
use gorgon_petra::frame::PaintContent;
use gorgon_petra::token::{InteractionRank, SHADOW_GEOMETRY, resolve_slot};

use super::{NodeChrome, PaintEnv, PaintReport, SHADOW_SLOT, resolve_or_record};

pub(super) fn paint_elevation(
    painter: &Painter,
    content: &PaintContent,
    chrome: &NodeChrome,
    env: &mut PaintEnv<'_>,
    report: &mut PaintReport,
) -> usize {
    let rect = chrome.rect;
    let corner_radius = chrome.corner_radius;
    let rank = chrome.rank;
    let outline = &chrome.outline;
    let state = chrome.state;
    let mut shapes = 0_usize;

    // Elevation goes down first, because a shadow is behind the thing that
    // casts it. Drawn after the fill it would sit *on* the card, which is not
    // a subtle mistake -- it is an obviously wrong picture, and that is the
    // reason this paints ahead of the fill and the border rather than with
    // them.
    if let Some(token) = resolve_slot(&content.tokens, SHADOW_SLOT, state) {
        // A rectangular shadow under a triangle is worse than no shadow, and
        // `epaint::Shadow::as_shape` can only make a `RectShape`. A node that
        // asked for both gets its silhouette honoured and its elevation
        // dropped, and nothing in `report` claims a shadow was drawn.
        //
        // A **disabled** node casts no shadow either, and that is FR-010
        // rather than a style preference. Depth is this painter's "you can
        // press this" channel, so a control that cannot be pressed must not
        // have it: a button lying flat beside two that are lifted reads as
        // unavailable *before* any of its colours do, and it survives a
        // reader who cannot separate the colours at all. The disabled colour
        // family is the second channel, not the only one — a colour-only
        // disabled state is exactly what FR-010 forbids.
        if outline.is_none()
            && rank != InteractionRank::Disabled
            && let Some(color) = resolve_or_record(env.colors, token, report)
            && let Some((_, geometry)) = SHADOW_GEOMETRY.iter().find(|(name, _)| *name == token)
        {
            let shadow = egui::epaint::Shadow {
                offset: geometry.offset,
                blur: geometry.blur,
                spread: geometry.spread,
                color,
            };
            // A shadow is the one thing on this page that has to draw
            // *outside* the node casting it, and every placement arrives here
            // already clipped to its own bounds. Painting it through the
            // inherited clip drew nothing at all: the token resolved, the
            // report counted a fill, and the window was pixel-identical --
            // the exact silent failure the design contract predicted for this
            // block, arriving through a mechanism the contract did not.
            //
            // The clip is widened by the shadow's own reach and no further:
            // `spread` grows the rect, `blur` feathers past that, and
            // `offset` displaces the whole thing. A shadow cannot escape by
            // more than it was declared to extend.
            //
            // And it may not leave the surface at all, which is what the
            // intersection with `screen_rect` is for. A shadow is allowed
            // outside its *node*; it is not allowed outside the *page*. The
            // difference is not cosmetic: `petra-egui/examples/parity.rs`
            // pins `RawInput::screen_rect` to a fixed rectangle so the two
            // targets lay out identically whatever size a window manager
            // grants, and `petra-parity` refuses the capture if anything is
            // painted beyond that pin -- which is exactly what a card near the
            // bottom edge did once elevation landed, because widening a clip
            // by 13 units walks straight through a boundary nothing else in
            // this painter can reach. `PaintEnv::page` is Petra's own
            // viewport, which is the pinned rectangle on that page and the
            // real surface everywhere else, so one intersection is correct in
            // both cases.
            let reach = f32::from(geometry.spread)
                + f32::from(geometry.blur)
                + f32::from(geometry.offset[0].abs().max(geometry.offset[1].abs()));
            // `Painter::with_clip_rect` INTERSECTS -- `rect.intersect(self.clip_rect)`
            // in egui-0.36.1's `painter.rs:73`. It can only ever narrow, so
            // widening through it is a silent no-op, and that is exactly how
            // the first attempt at this block failed: the shadow drew, and
            // survived only in the corner cut-outs the card's own rounding
            // left behind. `set_clip_rect` is the one that replaces.
            let mut cast = painter.clone();
            cast.set_clip_rect(painter.clip_rect().expand(reach).intersect(env.page));
            cast.add(egui::Shape::Rect(shadow.as_shape(rect, corner_radius)));
            report.fills += 1;
            shapes += 1;
        }
    }

    shapes
}
