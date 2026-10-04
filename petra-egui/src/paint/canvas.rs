//! The canvas: the engine-drawn draw list a `NodeKind::Canvas` executes,
//! with missing `Sprite` assets counted rather than skipped.

use egui::Painter;
use gorgon_petra::frame::{PaintContent, Placement};

use super::{PaintEnv, PaintReport};

pub(super) fn paint_canvas_node(
    painter: &Painter,
    placement: &Placement,
    content: &PaintContent,
    env: &mut PaintEnv<'_>,
    report: &mut PaintReport,
) -> usize {
    let mut shapes = 0_usize;

    if let Some(list) = &content.canvas {
        // A canvas is drawn by the engine, not by a host painter, which is
        // the whole reason it is a second node kind rather than a mode of
        // `custom`: every coordinate here is already in the digest, so what
        // reaches the screen is a function of the frame's identity.
        //
        // Assets are the one part the digest cannot see — it hashes the
        // asset's name and its rects, never decoded pixels — so a `Sprite`
        // that does not resolve is counted rather than quietly skipped.
        let ctx = painter.ctx().clone();
        let mut assets = crate::draw::HostImages::new(env.images, &ctx);
        let canvas = crate::draw::paint_canvas(
            painter,
            list,
            placement.rect,
            env.scale,
            env.colors,
            &mut assets,
        );
        report.missing_assets += canvas.missing_assets;
        for name in &canvas.undrawn {
            report.undrawn.insert(format!("canvas:{name}"));
        }
        if canvas.shapes > 0 {
            report.canvases += 1;
            shapes += canvas.shapes;
        }
    }

    shapes
}
