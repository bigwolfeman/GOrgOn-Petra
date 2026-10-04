//! The custom node: a host-registered `CustomPainters` painter, which is
//! counted only when it says it drew, and named in `undrawn` otherwise.

use egui::Painter;
use gorgon_petra::frame::PaintContent;

use super::{CustomPaintCtx, NodeChrome, PaintEnv, PaintReport};

pub(super) fn paint_custom(
    painter: &Painter,
    content: &PaintContent,
    chrome: &NodeChrome,
    env: &mut PaintEnv<'_>,
    report: &mut PaintReport,
) -> usize {
    let rect = chrome.rect;
    let mut shapes = 0_usize;

    if let Some(name) = &content.custom {
        let ctx = CustomPaintCtx {
            rect,
            clip: painter.clip_rect(),
            opacity: painter.opacity(),
            scale: env.scale,
            tokens: env.colors,
        };
        // `is_some_and` rather than an `if let` that ignores the bool: a
        // registered painter that draws nothing must be treated exactly
        // like no painter at all, and this is the one line where that
        // equivalence either holds or quietly stops holding.
        if env
            .painters
            .get(name)
            .is_some_and(|paint| paint(painter, &ctx))
        {
            report.customs += 1;
            shapes += 1;
        } else {
            report.undrawn.insert(format!("custom:{name}"));
        }
    }

    shapes
}
