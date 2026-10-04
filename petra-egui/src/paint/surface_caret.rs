//! The caret an anchored surface draws back at its anchor: `caret_of`'s
//! triangle, in the fill it continues (`contracts/anchored-placement.md` §5).

use egui::Painter;
use gorgon_petra::frame::PaintContent;
use gorgon_petra::token::resolve_slot;

use super::{BACKGROUND_SLOT, NodeChrome, PaintEnv, PaintReport, resolve_or_record};

pub(super) fn paint_surface_caret(
    painter: &Painter,
    content: &PaintContent,
    chrome: &NodeChrome,
    env: &mut PaintEnv<'_>,
    report: &mut PaintReport,
) -> usize {
    let state = chrome.state;
    let mut shapes = 0_usize;

    // An anchored surface's caret, drawn with the fill it continues and
    // therefore immediately after it: `crate::triangle` owns the shape, the
    // engine owns the geometry (`contracts/anchored-placement.md` §5), and a
    // surface that binds no background has no colour to draw one in.
    //
    // The fill goes through `resolve_slot`, not a bare map lookup, for the
    // same reason the fill does: a hovered surface binding
    // `background@hover` would otherwise grow a caret in its resting colour,
    // and a caret that disagrees with the shape it continues is worse than no
    // caret at all.
    //
    // Through a widened clip, for the shadow's reason: the caret is the
    // other thing on this page that draws *outside* the node it belongs to
    // (`caret_of` puts the tip `h` past the surface's near edge, back at the
    // anchor), and every placement arrives here clipped to its own bounds.
    // Painted through the inherited clip the caret drew nothing: the report
    // counted a fill, `every_built_page_paints_with_nothing_silent` was
    // green, and rows 24 and 38 showed a bubble with no beak. Widened by
    // the caret's own depth and no further, and never past the page.
    if let Some(caret) = &content.caret {
        let fill = resolve_slot(&content.tokens, BACKGROUND_SLOT, state)
            .and_then(|token| resolve_or_record(env.colors, token, report));
        let mut cast = painter.clone();
        cast.set_clip_rect(painter.clip_rect().expand(caret.h).intersect(env.page));
        if crate::triangle::paint_caret(&cast, caret, fill, env.scale) {
            report.fills += 1;
            shapes += 1;
        } else {
            report.undrawn.insert("caret".to_owned());
        }
    }

    shapes
}
