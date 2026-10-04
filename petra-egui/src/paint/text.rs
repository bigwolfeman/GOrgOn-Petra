//! The text run: shaping, colour runs, selection, the coverage-curve
//! repeat passes, and the `underline` slot's strip under each row.

use egui::{Color32, Painter};
use gorgon_petra::frame::{PaintContent, Placement};
use gorgon_petra::layout::TextRequest;
use gorgon_petra::token::resolve_slot;

use super::{
    DEFAULT_SELECTION_INK_TOKEN, DEFAULT_SELECTION_TOKEN, DEFAULT_TEXT_TOKEN, FOREGROUND_SLOT,
    NodeChrome, PaintEnv, PaintReport, SELECTION_INK_SLOT, SELECTION_SLOT, UNDERLINE_SLOT,
    device_snapped_width, resolve_or_record, text_inset_x,
};
use crate::host::{COVERAGE_TOKEN, FALLBACK_COVERAGE, coverage_plan};

pub(super) fn paint_text(
    painter: &Painter,
    placement: &Placement,
    content: &PaintContent,
    chrome: &NodeChrome,
    env: &mut PaintEnv<'_>,
    report: &mut PaintReport,
) -> usize {
    let rect = chrome.rect;
    let state = chrome.state;
    let mut shapes = 0_usize;

    if let Some(text) = &content.text {
        let token =
            resolve_slot(&content.tokens, FOREGROUND_SLOT, state).unwrap_or(DEFAULT_TEXT_TOKEN);
        let color = env.colors.color(token).unwrap_or_else(|| {
            report.unresolved_tokens.insert(token.to_owned());
            // Not a guess at the theme's intent: a visibly wrong colour is
            // better than invisible text, and the unresolved token is in the
            // report either way.
            Color32::PLACEHOLDER
        });
        // A style token the shaper has no binding for still shapes — at the
        // theme's body style, because blank text is worse on screen than
        // text at the wrong size — so the only way it can be noticed is for
        // the name to be reported here, the same as an unresolved colour.
        if let Some(unresolved) = env
            .shaper
            .typography()
            .resolve(text.style.as_deref())
            .1
            .map(str::to_owned)
        {
            report.unresolved_tokens.insert(unresolved);
        }
        // An `Input` is a leaf: padding is refused on it, so the chrome
        // rect *is* the placement. The galley still has to sit inside that
        // rect — 12 units in from the sides (`spacing-04`) and centred on
        // the vertical, Carbon's md field. Painting at `rect.min` put the
        // first glyph on the border. Shaping against the full width, then
        // shifting in, would push the last glyph through the other border.
        let input = placement.kind == gorgon_petra::tree::NodeKind::Input;
        let inset_x = text_inset_x(placement.kind, env.colors);
        let inner_w = (placement.rect.w - 2.0 * inset_x).max(0.0);
        // Colour runs, resolved here because this is where the theme is. A
        // run naming no token keeps `None` and takes `color` below, the same
        // ink the whole line would have taken; a run naming one this theme
        // cannot resolve is reported and then also falls back, for
        // `Color32::PLACEHOLDER`'s reason above — visibly wrong beats
        // invisible, and the name is in the report either way.
        let runs: Vec<(usize, Option<Color32>)> = text
            .runs
            .iter()
            .map(|run| {
                let ink = run.foreground.as_deref().and_then(|name| {
                    let found = env.colors.color(name);
                    if found.is_none() {
                        report.unresolved_tokens.insert(name.to_owned());
                    }
                    found
                });
                (run.len, ink)
            })
            .collect();
        let request = TextRequest {
            text: &text.text,
            style: text.style.as_deref(),
            wrap: text.wrap,
            max_lines: text.max_lines,
            available_width: Some(inner_w),
        };
        // The selected stretch, resolved as a pair. Neither half is usable
        // alone: the fill without the ink can sink a coloured run below AA,
        // and the ink without the fill recolours text for no visible reason.
        // So a node binding one and not the other is refused rather than
        // half-honoured, and a node binding neither takes the shipped pair —
        // which is a token the design system names, not a colour this painter
        // chose (`DEFAULT_SELECTION_TOKEN`).
        let selection = content.selection.as_ref().and_then(|range| {
            let bound = content.tokens.contains_key(SELECTION_SLOT)
                || content.tokens.contains_key(SELECTION_INK_SLOT);
            let (ground, ink) = if bound {
                (
                    resolve_slot(&content.tokens, SELECTION_SLOT, state)?,
                    resolve_slot(&content.tokens, SELECTION_INK_SLOT, state)?,
                )
            } else {
                (DEFAULT_SELECTION_TOKEN, DEFAULT_SELECTION_INK_TOKEN)
            };
            let ground = resolve_or_record(env.colors, ground, report)?;
            let ink = resolve_or_record(env.colors, ink, report)?;
            Some((range, ground, ink))
        });
        let runs = match &selection {
            Some((range, _, ink)) => {
                crate::text::runs_with_selection(&runs, text.text.len(), range, *ink)
            }
            None => runs,
        };
        let galley = env.shaper.galley_runs(&request, &runs);
        let text_pos = if input {
            let galley_h = galley.rect.height();
            let inset_y = ((rect.height() - galley_h) * 0.5).max(0.0);
            egui::pos2(rect.min.x + inset_x, rect.min.y + inset_y)
        } else {
            rect.min
        };
        // The atlas curve alone cannot reach every `passes` value the
        // `text.coverage-curve` token allows (SPEC.md §2.3: no
        // `FontColorTransferFunction` variant exists past two-pass
        // compositing), so the rest is spent here, by painting the same
        // galley `repeats` times under egui-wgpu's premultiplied
        // source-over — which composites to `1 - (1-a)^repeats`, the same
        // identity the atlas curve itself rests on
        // (`crate::host::coverage_plan`'s doc comment names the trap this
        // is the other half of). `report.texts` and `shapes` count the
        // logical run once regardless of how many physical paints it took —
        // see this module's doc comment and `PaintReport`'s.
        // Behind the glyphs, so the ink above is read against this fill and
        // not the other way round. One rectangle per row the selection
        // crosses, from the galley about to be painted — the same galley, so
        // the highlight and the letters cannot be measured differently.
        if let Some((range, ground, _)) = &selection {
            for band in crate::text::selection_rects(&galley, range) {
                painter.rect_filled(band.translate(text_pos.to_vec2()), 0.0, *ground);
                shapes += 1;
            }
        }
        let coverage = env
            .colors
            .coverage(COVERAGE_TOKEN)
            .unwrap_or(FALLBACK_COVERAGE);
        let plan = coverage_plan(coverage);
        for _ in 0..plan.repeats {
            painter.galley(text_pos, galley.clone(), color);
        }
        if plan.fraction > 0.0 {
            // ai-macs' fractional pass, ported verbatim: one further paint
            // at the colour's alpha scaled by the fractional remainder, not
            // the fraction silently dropped.
            painter.galley(
                text_pos,
                galley.clone(),
                color.gamma_multiply(plan.fraction),
            );
        }
        report.texts += 1;
        shapes += 1;
        // The underline is a strip one snapped unit deep under each row,
        // as wide as that row's glyphs, in the slot's own colour rather
        // than the text's: Carbon's link underline is `currentColor`, but a
        // slot that carried its own colour costs nothing more and is what
        // lets `underline@hover` name a tone. Drawn after the glyphs so a
        // descender crossing it stays legible.
        if let Some(token) = resolve_slot(&content.tokens, UNDERLINE_SLOT, state)
            && let Some(color) = resolve_or_record(env.colors, token, report)
        {
            let depth = device_snapped_width(1.0, env.scale);
            for row in &galley.rows {
                let run = row
                    .rect_without_leading_space()
                    .translate(text_pos.to_vec2());
                if run.width() <= 0.0 {
                    continue;
                }
                let strip = egui::Rect::from_min_size(
                    egui::pos2(run.min.x, run.max.y - depth),
                    egui::vec2(run.width(), depth),
                );
                painter.rect_filled(strip, 0.0, color);
                shapes += 1;
            }
        }
    }

    shapes
}
