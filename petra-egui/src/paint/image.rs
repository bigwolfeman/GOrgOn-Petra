//! The image: a registered `ImageSources` entry resolved to a texture and
//! contained inside the node's rect, or named in `undrawn` when it is not.

use egui::{Color32, Painter};
use gorgon_petra::frame::PaintContent;

use super::{NodeChrome, PaintEnv, PaintReport};

pub(super) fn paint_image(
    painter: &Painter,
    content: &PaintContent,
    chrome: &NodeChrome,
    env: &mut PaintEnv<'_>,
    report: &mut PaintReport,
) -> usize {
    let rect = chrome.rect;
    let mut shapes = 0_usize;

    if let Some(source) = &content.image {
        // `resolve` is the whole of "decode/upload behind a source-keyed
        // cache" (T081): a source this pass has drawn before comes back
        // from the cache, a new one is decoded and uploaded once. Either
        // way what comes back is a texture with its own natural size, and
        // `image::contain` is the aspect handling — fit that size inside
        // `rect`, the space Petra offered this node, without stretching it.
        if let Some(handle) = env.images.resolve(painter.ctx(), source) {
            let target = crate::image::contain(rect, handle.size_vec2());
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(handle.id(), target, uv, Color32::WHITE);
            report.images += 1;
            shapes += 1;
        } else {
            // Not registered, or registered with pixels that failed to
            // decode — either way this is the source that could not be
            // drawn, named so the operator knows which picture is missing
            // rather than that "an image" is.
            report.undrawn.insert(format!("image:{source}"));
        }
    }

    shapes
}
