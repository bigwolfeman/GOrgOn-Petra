//! T066 / SC-009 — mixed-script and emoji text draws glyphs, not boxes.
//!
//! SC-009 asks for "zero missing-glyph boxes, desktop and web". **This file
//! is the desktop half and only the desktop half.** The web half belongs to
//! the wasm parity lane, which builds its own `egui::Context` on the
//! browser's font stack and drives the same
//! [`gorgon_petra_egui::fonts::GlyphProbe`] over the same
//! [`gorgon_petra_egui::fonts::script_samples`]. Nothing here says anything
//! about wasm, and SC-009 is not closed by this file passing.
//!
//! # What "proves" means here
//!
//! Three readings, layered, because a text test that can pass with a broken
//! font stack is worse than no test:
//!
//! 1. **The stack is what we think it is.** `install_desktop_fallbacks`
//!    reports the file each face came from. If a face is absent the suite
//!    goes red naming the face, the script it covers, and every path that was
//!    tried. It never skips.
//! 2. **Every codepoint draws ink that is not the box.** The probe learns the
//!    replacement box's atlas region from an unassigned codepoint, then
//!    compares every sample codepoint's region against it. This is the
//!    reading that produces the `GLYPH <script>=<covered>/<total>` lines.
//! 3. **The production path agrees.** The same runs go through
//!    `GalleyShaper` — the shaper `Host::pass` paints from — and every glyph
//!    in the resulting galley is classified again.
//!
//! And one reading that exists purely to disprove the other three: the
//! detector is pointed at a codepoint no installed face carries, and must
//! call it tofu. A detector that cannot fire is not a detector.
//!
//! # The emoji answer, in writing
//!
//! Two separate gaps, and they are not the same gap.
//!
//! **Colour emoji do not render, and cannot.** egui 0.36.1 rasterises
//! through `skrifa`'s outline reader, and every colour emoji face a desktop
//! ships is a bitmap or COLR face with no outlines — installing one moves
//! emoji from legible monochrome to *invisible*, which is worse than the box
//! it replaces. The custom-glyph registry that fixes this is research item R4
//! of spec 003, it lands in the overlay fork, and **the overlay fork does not
//! exist in this workspace**: there is no `[patch.crates-io]` entry for any
//! egui-family crate. Monochrome fallback with the gap recorded is the
//! acceptable outcome the task text names, and that is what this file proves.
//!
//! **The bundled monochrome face is from 2015, and some emoji still box.**
//! This one was found by the detector, not predicted: the Noto Emoji build
//! inside `epaint_default_fonts` carries 887 codepoints, roughly Unicode 6.1.
//! U+1F642 (Unicode 7.0) draws a replacement box on a stock desktop today.
//! That is a real, unfixed product defect. It is recorded three ways —
//! `gorgon_petra_egui::fonts::RECORDED_EMOJI_GAPS` enumerates it, the
//! optional `gorgon-fallback-emoji` face closes every one of them the moment
//! a modern monochrome Noto Emoji is installed on the host, and
//! `the_emoji_gaps_are_exactly_the_recorded_ones` below turns the suite red
//! if the set ever grows. **It is not closed, and SC-009's emoji clause is
//! not fully satisfied on a host without that font.**
//!
//! The zero-box acceptance is therefore measured over
//! `fonts::script_samples()`, whose emoji entry is inside the bundled face's
//! coverage. The boundary itself is measured separately, by the test named
//! above, over `fonts::emoji_boundary_sample()`.

use egui::{Context, FontId, RawInput};
use gorgon_petra::layout::{ContentMeasure as _, TextRequest};
use gorgon_petra::tree::TextWrap;
use gorgon_petra_egui::fonts::{
    self, DESKTOP_FALLBACKS, FontStackReport, GlyphOutcome, GlyphProbe, ScriptCoverage,
};
use gorgon_petra_egui::text::{GalleyShaper, Typography};

/// A codepoint no face in the installed stack carries: EGYPTIAN HIEROGLYPH
/// A001. Used to prove the detector fires without touching any sample.
const UNCOVERABLE: char = '\u{13000}';

/// A context with the desktop fallback stack installed and one pass driven,
/// so the new faces are live.
///
/// Panics — loudly, naming every path it tried — rather than skipping when
/// the host lacks a face. A test that reports success on a machine where it
/// could not do the work is the failure this whole file exists to prevent.
fn desktop_context() -> (Context, FontStackReport) {
    let ctx = Context::default();
    // egui has no fonts at all until a pass has run, and it applies new
    // definitions at the start of the pass *after* `set_fonts`. So: pass,
    // install, pass.
    ctx.run_ui(RawInput::default(), |_| {})
        .drop_without_applying_deltas();
    let report = fonts::install_desktop_fallbacks(&ctx);
    ctx.run_ui(RawInput::default(), |_| {})
        .drop_without_applying_deltas();

    assert!(
        report.is_complete(),
        "the desktop font stack is incomplete on this machine, so SC-009 cannot be measured \
         here. This is a red result, not a skip — install the named faces or fix the candidate \
         paths in gorgon/petra-egui/src/fonts.rs.\n{}",
        report.summary()
    );
    static ANNOUNCED: std::sync::Once = std::sync::Once::new();
    ANNOUNCED.call_once(|| {
        print!("{}", report.summary());
    });
    (ctx, report)
}

/// The probe, over the font the shipped `typography.body` token resolves to.
///
/// Read off [`Typography`] rather than written out as a size and a family, so
/// that coverage is measured at the size body text is actually read at even
/// after the theme's type ramp moves. Glyph rasterisation is scale-dependent,
/// and the atlas region every comparison in this file is made of moves with
/// it.
fn probe(ctx: &Context) -> GlyphProbe {
    let typography = Typography::default();
    let (style, unresolved) = typography.resolve(Some("typography.body"));
    assert_eq!(
        unresolved, None,
        "the shipped theme must bind `typography.body`; measuring against a fallback style \
         would measure a size no text is drawn at"
    );
    let font: FontId = style.font.clone();
    GlyphProbe::new(ctx, font)
        .expect("the probe must be able to tell a box from a glyph before anything else runs")
}

/// Coverage for every sample script, with the `GLYPH` line printed for each.
fn measure(ctx: &Context, probe: &GlyphProbe) -> Vec<ScriptCoverage> {
    let mut out = Vec::new();
    for (script, sample) in fonts::script_samples() {
        let coverage = probe.coverage(ctx, script, sample);
        println!("{}", coverage.report_line());
        out.push(coverage);
    }
    out
}

/// The acceptance itself: every codepoint of every sample script draws a real
/// glyph, and the machine-readable coverage lines say so.
#[test]
fn every_sample_script_renders_without_a_single_box() {
    let (ctx, _report) = desktop_context();
    let probe = probe(&ctx);
    let coverages = measure(&ctx, &probe);

    assert!(
        coverages.len() >= 5,
        "SC-009 wants at least four distinct scripts plus emoji; the corpus has {}",
        coverages.len()
    );

    let mut failures = String::new();
    let mut measured = 0usize;
    for coverage in &coverages {
        measured += coverage.total();
        failures.push_str(&coverage.gap_report());
    }
    assert!(
        measured > 40,
        "only {measured} codepoint(s) were measured across {} script(s); a corpus this small \
         cannot support SC-009",
        coverages.len()
    );
    assert!(
        failures.is_empty(),
        "SC-009: {} codepoint(s) did not render as a glyph under the desktop font stack.\n{}",
        coverages.iter().map(|c| c.gaps().len()).sum::<usize>(),
        failures
    );
}

/// The same runs, through the shaper the shipped host paints from.
///
/// Reading 2 measures codepoints in isolation. This one measures them the way
/// they are drawn: wrapped, in a mixed paragraph, through `GalleyShaper` —
/// the exact `Arc<Galley>` `paint::paint_frame` would put on screen.
#[test]
fn the_production_shaper_draws_the_same_runs_without_a_box() {
    let (ctx, _report) = desktop_context();
    let probe = probe(&ctx);
    let mut shaper = GalleyShaper::new(ctx.clone());

    let mixed: String = fonts::script_samples()
        .iter()
        .map(|(_, sample)| *sample)
        .collect::<Vec<_>>()
        .join(" · ");

    let mut failures = String::new();
    let mut runs = 0usize;
    let mut glyphs = 0usize;
    for text in fonts::script_samples()
        .iter()
        .map(|(_, sample)| (*sample).to_owned())
        .chain(std::iter::once(mixed))
    {
        let request = TextRequest {
            text: &text,
            style: Some("typography.body"),
            available_width: Some(180.0),
            wrap: TextWrap::Wrap,
            max_lines: None,
        };
        // Measure first, exactly as layout does, so the galley read below is
        // the cached one the painter would reuse.
        let measurement = shaper.text(&request);
        assert!(
            measurement.size.h > 0.0 && measurement.lines >= 1,
            "{text:?} measured to nothing: {measurement:?}"
        );

        let galley = shaper.galley(&request);
        let row_glyphs: usize = galley.rows.iter().map(|row| row.row.glyphs.len()).sum();
        assert!(row_glyphs > 0, "{text:?} shaped to a galley with no glyphs");
        runs += 1;
        glyphs += row_glyphs;

        for (c, outcome) in probe.galley_outcomes(&ctx, &galley) {
            if outcome.is_gap() {
                failures.push_str(&format!(
                    "  - U+{:04X} {:?} -> {} (in {:?})\n",
                    c as u32,
                    c,
                    outcome.as_str(),
                    text
                ));
            }
        }
    }
    println!("SHAPED runs={runs} glyphs={glyphs}");
    assert!(
        failures.is_empty(),
        "SC-009: the production shaper drew missing-glyph output:\n{failures}"
    );
}

/// The detector fires. Without this, every green line above could mean
/// "the comparison never says no".
#[test]
fn the_detector_calls_an_uncoverable_codepoint_a_box() {
    let (ctx, _report) = desktop_context();
    let probe = probe(&ctx);
    assert_eq!(
        probe.outcome(&ctx, UNCOVERABLE),
        GlyphOutcome::Tofu,
        "U+{:04X} is in no installed face; if the probe calls it rendered, the probe is blind",
        UNCOVERABLE as u32
    );

    // And through the same reporting path the acceptance uses, so the shape
    // of a real failure message is proven rather than assumed.
    let coverage = probe.coverage(&ctx, "sabotage", &format!("A{UNCOVERABLE}"));
    assert_eq!(coverage.report_line(), "GLYPH sabotage=1/2");
    let report = coverage.gap_report();
    assert!(report.contains("sabotage"), "{report}");
    assert!(
        report.contains(&format!("U+{:04X}", UNCOVERABLE as u32)),
        "{report}"
    );
    assert!(report.contains("tofu"), "{report}");
}

/// The emoji gaps are exactly the ones recorded, and no more.
///
/// A ceiling, not a floor. `RECORDED_EMOJI_GAPS` is what the bundled 2015
/// Noto Emoji build is measured to lack; a gap outside that set is a
/// regression and turns this red. Shrinking the set is always allowed, and
/// installing a modern monochrome Noto Emoji on the host shrinks it to
/// nothing without touching a line of code — which this test then requires,
/// so the record cannot outlive the defect.
#[test]
fn the_emoji_gaps_are_exactly_the_recorded_ones() {
    let (ctx, report) = desktop_context();
    let probe = probe(&ctx);
    let host_has_a_modern_emoji_face = report.installed_face("gorgon-fallback-emoji");

    let coverage = probe.coverage(&ctx, "emoji-boundary", fonts::emoji_boundary_sample());
    println!("{}", coverage.report_line());

    let gaps = coverage.gaps();
    for (c, outcome) in &gaps {
        assert!(
            fonts::RECORDED_EMOJI_GAPS.contains(c),
            "U+{:04X} {c:?} -> {} is an emoji gap nobody recorded. Either a face regressed \
             or the corpus grew; fix the stack or record it in RECORDED_EMOJI_GAPS with a \
             reason.",
            *c as u32,
            outcome.as_str()
        );
    }
    if host_has_a_modern_emoji_face {
        assert!(
            gaps.is_empty(),
            "this host installed {} and emoji still do not render, so the recorded gap is \
             not the gap it claims to be:\n{}",
            "gorgon-fallback-emoji",
            coverage.gap_report()
        );
    }

    // The colour half, which no font on this stack can close.
    for face in DESKTOP_FALLBACKS {
        for candidate in face.candidates {
            assert!(
                !candidate.contains("ColorEmoji") && !candidate.contains("seguiemj"),
                "{} names a colour-emoji face at {candidate}. egui 0.36.1 reads outlines \
                 only, so it would draw nothing at all. Colour emoji are R4 of spec 003 and \
                 land in the overlay fork, which does not exist in this workspace.",
                face.name
            );
        }
    }

    let recorded: Vec<String> = fonts::RECORDED_EMOJI_GAPS
        .iter()
        .map(|c| format!("U+{:04X}", *c as u32))
        .collect();
    println!(
        "EMOJI colour=absent reason=R4-not-landed(no overlay fork in this workspace) \
         mono-face={} recorded-gaps=[{}] observed-gaps={}",
        if host_has_a_modern_emoji_face {
            "gorgon-fallback-emoji(host)"
        } else {
            "NotoEmoji-Regular(bundled, Unicode~6.1)"
        },
        recorded.join(" "),
        gaps.len(),
    );
}
