//! A size ladder of **pure white text on one flat grey**, rendered headless
//! at native resolution, for judging glyph rasterisation by eye.
//!
//! Opt-in: `PETRA_TEXT_PROBE=<path>` renders and writes a raw `P6` PPM;
//! unset, the test prints why it skipped and passes.
//!
//! ```text
//! PETRA_TEXT_PROBE=/tmp/probe.ppm \
//!   cargo test -p gorgon-petra-egui --test text_size_probe -- --nocapture
//! ```
//!
//! # Why white, and why one flat grey
//!
//! Because the last defect reported as "the text is greying out" was **not**
//! a rasteriser defect at all — it was `text.muted` sitting below the WCAG AA
//! contrast floor against the deeper surfaces
//! (`.agents/notes/implemented/bug-fix/2026-08-25-muted-text-tone-fails-aa.md`).
//! That one took a long time to find partly because every artifact used to
//! judge sharpness also carried the theme's tone decisions, so "is this glyph
//! soft?" and "is this colour too quiet?" arrived as one question.
//!
//! This probe answers only the first. `#ffffff` is the one text colour with
//! no tone decision in it, and a single flat ground means every difference
//! down the ladder is the rasteriser and nothing else. It is deliberately the
//! **only** literal colour in this file; everything else comes from the
//! shipped theme.
//!
//! # Why native resolution rather than the magnified contact sheet
//!
//! `tests/text_contact_sheet.rs` magnifies 5x, because it exists to make a
//! *metric* checkable — you cannot see a one-column ink difference at 1x.
//! This file exists for the opposite job: what the glyph actually looks like
//! at the size a reader gets. A 5x sheet cannot answer that, because
//! nearest-neighbour magnification makes every edge look hard.
//!
//! # Knobs
//!
//! | variable | default | meaning |
//! |---|---|---|
//! | `PETRA_TEXT_PROBE` | unset (skip) | output path, always a `P6` PPM |
//! | `PETRA_TEXT_PROBE_SURFACE` | `surface.layer-one` | which shipped surface token to fill with |
//! | `PETRA_TEXT_PROBE_PASSES` | the shipped theme's own `text.coverage-curve` | sweep the sharpness dial |
//! | `PETRA_TEXT_PROBE_THEME` | `dark` | `light` picks the light theme's surface |
//!
//! `PASSES` is resolved through [`gorgon_petra_egui::host::coverage_plan`],
//! the same function the real paint path uses, so this probe cannot drift
//! from what ships.

mod support;

use egui::{Color32, Context, FontFamily, FontId, Pos2, RawInput, Rect, vec2};
use gorgon_petra::token::value::CoverageValue;
use gorgon_petra::token::{Presenter, dark, light};
use gorgon_petra_egui::fonts;
use gorgon_petra_egui::host::{coverage_plan, coverage_value};
use gorgon_petra_egui::paint::TokenSource;
use support::{Gpu, write_ppm};

/// The one literal colour in this file. See the module doc: a tone decision
/// in the probe is a confound in the thing the probe exists to isolate.
const INK: Color32 = Color32::WHITE;

/// The ladder. Dense at the bottom because that is where a 1.2px stem lands
/// between pixels and where every rasteriser argument actually lives; sparse
/// at the top, where the stem is wide enough that phase stops mattering and
/// the rows are only there to prove that.
const SIZES: [f32; 10] = [12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 32.0, 40.0, 48.0, 64.0];

/// Mixed stems, bowls, diagonals and a numeral run — the letters that expose
/// grid-fitting. `Hamburgefonstiv` is the type-design standard for exactly
/// this; the trailing digits catch a font whose figures are hinted
/// differently from its lowercase.
const SAMPLE: &str = "Hamburgefonstiv 0123";

/// Repeated identical stems with separating spaces. Identical shape at
/// identical size means any difference between these is *placement*, which
/// is the one thing a reader perceives as "some characters are greyer".
const STEMS: &str = "l l l l l l l l";

/// Left gutter holding each row's size label.
const GUTTER: f32 = 64.0;
/// Point size the gutter label is set at, small and fixed so it never
/// competes with the row it labels.
const LABEL_SIZE: f32 = 11.0;
/// Horizontal padding either side of a sample.
const PAD: f32 = 12.0;
/// Canvas width. Wide enough for the 64pt row's two samples side by side.
const WIDTH: f32 = 1500.0;

/// Vertical room a row of `size` gets: the line box plus breathing room, so
/// adjacent rows never share an anti-aliased edge.
fn row_height(size: f32) -> f32 {
    (size * 1.45).ceil() + 10.0
}

#[test]
fn text_size_probe() {
    let Some(path) = std::env::var_os("PETRA_TEXT_PROBE") else {
        println!(
            "text_size_probe: skipping (set PETRA_TEXT_PROBE=<path> to render). Written as a raw \
             P6 PPM regardless of the path's extension. See this file's module doc for the knobs."
        );
        return;
    };
    let path = std::path::PathBuf::from(path);

    let theme_name = std::env::var("PETRA_TEXT_PROBE_THEME").unwrap_or_else(|_| "dark".to_owned());
    let theme = if theme_name == "light" {
        light()
    } else {
        dark()
    };

    let surface_token = std::env::var("PETRA_TEXT_PROBE_SURFACE")
        .unwrap_or_else(|_| "surface.layer-one".to_owned());

    // The shipped value unless overridden, so an unparameterised run shows
    // what a reader actually gets rather than a tester's favourite setting.
    let shipped = coverage_value(&theme);
    let passes = match std::env::var("PETRA_TEXT_PROBE_PASSES") {
        Ok(raw) => raw
            .trim()
            .parse::<f32>()
            .unwrap_or_else(|_| panic!("PETRA_TEXT_PROBE_PASSES={raw}: not a number")),
        Err(_) => shipped.passes,
    };
    let plan = coverage_plan(CoverageValue {
        passes,
        snap: shipped.snap,
    });

    let presenter = Presenter::new(theme);
    let snapshot = presenter.current();
    let ground = snapshot
        .color(&surface_token)
        .unwrap_or_else(|| panic!("the {theme_name} theme has no colour token {surface_token:?}"));

    let ctx = Context::default();
    fonts::install_design_system(&ctx);
    // Both style slots, the same reason `host::bind_glyph_coverage` writes
    // both: a curve written to the slot `ctx.theme()` does not currently
    // select is a silent miss.
    for egui_theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(egui_theme, |style| {
            style.visuals.text_options.color_transfer_function = plan.curve;
            style.visuals.text_options.subpixel_binning = !plan.snap;
        });
    }

    let height: f32 = SIZES.iter().copied().map(row_height).sum::<f32>() + 8.0;
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(WIDTH, height))),
        ..RawInput::default()
    };

    let output = ctx.run_ui(input, |ui| {
        let painter = ui.painter();
        // Overdrawn past the edge so no clip-rect rounding leaves a sliver.
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(-4.0, -4.0), vec2(WIDTH + 8.0, height + 8.0)),
            0.0,
            ground,
        );

        let mut y = 4.0;
        for size in SIZES {
            let label = ui.ctx().fonts_mut(|f| {
                f.layout_no_wrap(
                    format!("{size:.0}px"),
                    FontId::new(LABEL_SIZE, FontFamily::Proportional),
                    INK,
                )
            });
            let sample = ui.ctx().fonts_mut(|f| {
                f.layout_no_wrap(
                    SAMPLE.to_owned(),
                    FontId::new(size, FontFamily::Proportional),
                    INK,
                )
            });
            let stems = ui.ctx().fonts_mut(|f| {
                f.layout_no_wrap(
                    STEMS.to_owned(),
                    FontId::new(size, FontFamily::Proportional),
                    INK,
                )
            });

            // Baselines aligned by centring each galley in the row box, so
            // the ladder reads as a ladder rather than as a staircase.
            let box_h = row_height(size);
            let centre = |g: &egui::Galley| y + (box_h - g.size().y) / 2.0;

            painter.galley(Pos2::new(4.0, centre(&label)), label, INK);
            // `plan.repeats` paints of the same galley at the same position:
            // egui-wgpu's premultiplied source-over composites this to
            // `1-(1-a)^repeats`, which is the shipped sharpening mechanic.
            let sample_x = GUTTER;
            let stems_x = GUTTER + sample.size().x + PAD * 2.0;
            for _ in 0..plan.repeats {
                painter.galley(Pos2::new(sample_x, centre(&sample)), sample.clone(), INK);
            }
            for _ in 0..plan.repeats {
                painter.galley(Pos2::new(stems_x, centre(&stems)), stems.clone(), INK);
            }
            y += box_h;
        }
    });

    let (w, h) = (WIDTH as u32, height as u32);
    let pixels = gpu_capture(&ctx, output, w, h);
    // Drop alpha: the pass fills an opaque ground first, so every texel is
    // already opaque and compositing here would only invent a second answer.
    let rgb: Vec<u8> = pixels
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    write_ppm(&path, w, h, &rgb);

    println!("text_size_probe: wrote {}x{} to {}", w, h, path.display());
    println!(
        "text_size_probe: theme={theme_name} surface={surface_token} ink=#ffffff \
         passes={passes} (shipped {shipped_passes}) -> curve={curve:?} repeats={repeats} \
         subpixel_binning={binning}",
        shipped_passes = shipped.passes,
        curve = plan.curve,
        repeats = plan.repeats,
        binning = !plan.snap,
    );
    println!(
        "text_size_probe: sizes {SIZES:?}; left sample {SAMPLE:?}, right sample {STEMS:?} \
         (identical stems, so any difference between them is placement)"
    );
}

/// Bring the device up, render, and hand back RGBA8.
///
/// Split out only so the `TexturesDelta` clear sits next to the capture it
/// belongs to: `TexturesDelta` panics in its own destructor when dropped
/// unapplied (`epaint-0.36.1/src/textures.rs:337`), and `Gpu::capture` has
/// already applied every entry.
fn gpu_capture(ctx: &Context, mut output: egui::FullOutput, w: u32, h: u32) -> Vec<u8> {
    let gpu = Gpu::new();
    let pixels = gpu.capture(ctx, &output, w, h);
    output.textures_delta.clear();
    pixels
}
