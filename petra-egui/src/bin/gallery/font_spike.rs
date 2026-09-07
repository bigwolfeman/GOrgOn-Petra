//! Typeface spike: the same realistic panel, in shipped IBM Plex Sans and
//! in Source Sans 3, light and dark, at 2x — the only alternative that
//! survived the digit-advance screen recorded in
//! `ignored/font-spike/measure.py` and its output.
//!
//! Test-only and throwaway, same as [`super::shots`]'s `Fixture`: nothing
//! here ships, and this file builds its own `App` and its own minimal
//! headless driver rather than reaching into `Camera`'s private fields.
//! [`gorgon_petra_egui::fonts`] embeds Plex at compile time
//! (`include_bytes!`), which is right for a shipped product and wrong for a
//! spike — the candidate face is a throwaway download under `ignored/`, not
//! an asset this crate should carry even conditionally. So the candidate is
//! read from disk at test time instead, and Plex needs no override at all:
//! it is already what [`gorgon_petra_egui::host::Host::new`] installs.
//!
//! Run with `PETRA_SHOT_DIR` set, then **look at the PNGs** — see the
//! module doc on `shots.rs` and `bin/gallery/CLAUDE.md` for why a frame
//! record is not a substitute for reading the picture.

#![cfg(test)]
#![allow(dead_code)]

use std::borrow::Cow;
use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily, FontTweak, RawInput};
use gorgon_petra::component::{
    button, data_table, data_table_row, field_labeled, ghost_button, heading, hinted,
    primary_button, text,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::Theme;
use gorgon_petra::tree::{InsetRefs, ViewNode};
use gorgon_petra_egui::fonts::{SANS_BOLD_FAMILY, SANS_MEDIUM_FAMILY, SANS_REGULAR_FAMILY};
use gorgon_petra_egui::host::{App, Host, default_presenter};
use gorgon_petra_testkit::snapshot::Snapshotter;

use crate::page::common::{body, column, row, sp, tok, wrapped};

/// A tall narrow panel, phone-width, the shape the operator asked the
/// comparison rendered at rather than a wide contact sheet.
const PANEL: [f32; 2] = [375.0, 900.0];

/// Same reasoning as `shots.rs`'s `CAPTURE_SCALE`: half-pixel defects only
/// exist at 1x, and the IBM Carbon reference captures at device pixel ratio
/// 2, so this spike matches it.
const CAPTURE_SCALE: f32 = 2.0;

/// Absolute paths to one candidate face's three weights, read from disk at
/// test time rather than `include_bytes!`'d — these are throwaway
/// downloads under `ignored/font-spike/faces/`, never compiled in.
struct CandidatePaths {
    regular: &'static str,
    medium: &'static str,
    bold: &'static str,
}

/// Source Sans 3: the one alternative to Plex that measured tabular by
/// default. See `ignored/font-spike/measure.py`'s output — every other
/// screened candidate (Inter, Geist, Public Sans, Figtree, Manrope) draws
/// proportional figures by default and is disqualified before it ever
/// reaches a picture.
const SOURCE_SANS_3: CandidatePaths = CandidatePaths {
    regular: "/home/wolfe/projects/GOrgOn/.claude/worktrees/integrate/ignored/font-spike/faces/SourceSans3-Regular.ttf",
    medium: "/home/wolfe/projects/GOrgOn/.claude/worktrees/integrate/ignored/font-spike/faces/SourceSans3-Medium.ttf",
    bold: "/home/wolfe/projects/GOrgOn/.claude/worktrees/integrate/ignored/font-spike/faces/SourceSans3-Bold.ttf",
};

/// Register `paths`' three weights under the exact family names the shipped
/// Plex stack uses ([`SANS_REGULAR_FAMILY`] and its medium/bold siblings),
/// so every component that asks for one of those families draws the
/// candidate instead, with no change to which component asked for which
/// weight and no change to the fallback tail behind it (egui's built-in
/// faces, for whatever the candidate does not carry).
///
/// Call after [`Host::new`] (which installs Plex) and before the next pass
/// egui runs: egui applies new font definitions at the start of the next
/// pass, and nothing has been shaped yet at that point, so there is no
/// stale glyph cache to clear.
fn install_candidate(ctx: &Context, paths: &CandidatePaths) {
    let mut defs = FontDefinitions::default();
    let fallback_tail = defs.families[&FontFamily::Proportional].clone();

    for (face_name, path) in [
        ("spike-regular", paths.regular),
        ("spike-medium", paths.medium),
        ("spike-bold", paths.bold),
    ] {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|err| panic!("font spike: cannot read {path}: {err}"));
        defs.font_data.insert(
            face_name.to_owned(),
            Arc::new(FontData {
                font: Cow::Owned(bytes),
                index: 0,
                tweak: FontTweak::default(),
            }),
        );
    }

    for (family, face) in [
        (SANS_REGULAR_FAMILY, "spike-regular"),
        (SANS_MEDIUM_FAMILY, "spike-medium"),
        (SANS_BOLD_FAMILY, "spike-bold"),
    ] {
        let mut chain = vec![face.to_owned()];
        chain.extend(fallback_tail.iter().cloned());
        defs.families.insert(FontFamily::Name(family.into()), chain);
    }

    // Proportional too, for anything that asks for it directly rather than
    // through the weight-named families.
    let mut proportional = vec!["spike-regular".to_owned()];
    proportional.extend(fallback_tail.iter().cloned());
    defs.families.insert(FontFamily::Proportional, proportional);

    ctx.set_fonts(defs);
}

/// A text leaf built with [`text`] (body style) and then restyled — the
/// shape every non-default type step in this panel takes, since
/// `gorgon_petra::component::text` exports only `text` (body) and
/// `heading` (heading) as public constructors.
fn styled(key: &str, content: &str, style: &str, muted: bool) -> ViewNode {
    let mut node = text(key, content);
    node.props.style = Some(tok(style));
    if muted {
        node.props
            .tokens
            .insert("foreground".into(), tok("text.muted"));
    }
    node
}

/// A test-only `App` that builds one realistic panel: title, section
/// heading, body copy, a numeric data table, a timestamp/metadata line, a
/// row of button labels, and a labelled field with helper text. Binds its
/// own ground and takes a theme, the same shape `shots.rs`'s `Fixture`
/// takes, so a picture of it is a picture of a plausible shipped screen
/// rather than a specimen sheet of the alphabet.
struct Specimen {
    /// Taken by the first [`App::theme_request`] and never asked for again.
    pending: Option<Theme>,
}

impl RowSource for Specimen {
    fn rows(&mut self, _source: &str, _range: std::ops::Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Specimen {
    fn view(&mut self) -> ViewNode {
        let title = heading("panel-title", "Runtime Diagnostics");

        let section = styled(
            "section-heading",
            "Recent activity",
            "typography.heading-sm",
            false,
        );

        let body_copy = column(
            "body-copy",
            sp("spacing.xs"),
            vec![
                wrapped(
                    "body-1",
                    "The scheduler retried three jobs in the last interval and logged one \
                     soft timeout.",
                ),
                wrapped(
                    "body-2",
                    "Nothing here needs attention yet; the queue is draining at its usual \
                     rate.",
                ),
            ],
        );

        let table = data_table(
            "table",
            vec![
                text("h-metric", "Metric"),
                text("h-count", "Count"),
                text("h-latency", "Latency"),
            ],
            vec![
                data_table_row(
                    "r-frames",
                    vec![
                        text("c1", "Frames"),
                        text("c2", "1,024"),
                        text("c3", "16.7 ms"),
                    ],
                    false,
                ),
                data_table_row(
                    "r-events",
                    vec![
                        text("c4", "Events"),
                        text("c5", "8,192"),
                        text("c6", "3.4 ms"),
                    ],
                    false,
                ),
                data_table_row(
                    "r-errors",
                    vec![
                        text("c7", "Errors"),
                        text("c8", "42"),
                        text("c9", "129.0 ms"),
                    ],
                    false,
                ),
                data_table_row(
                    "r-retries",
                    vec![
                        text("c10", "Retries"),
                        text("c11", "7"),
                        text("c12", "0.9 ms"),
                    ],
                    false,
                ),
            ],
        );

        let meta = styled(
            "meta",
            "Updated 2026-09-07 14:32:07 UTC \u{b7} Session #48213 \u{b7} 3 processes",
            "typography.label",
            true,
        );

        let buttons = row(
            "buttons",
            sp("spacing.sm"),
            vec![
                primary_button("btn-apply", "Apply"),
                button("btn-cancel", "Cancel"),
                ghost_button("btn-reset", "Reset"),
            ],
        );

        let field_helper = styled(
            "field-helper",
            "Shown next to your posts and comments.",
            "typography.label",
            true,
        );
        let field_well = hinted(field_labeled("field-name", "Display name"), "e.g. wolfe");
        let field_block = column(
            "field-block",
            sp("spacing.xs"),
            vec![field_well, field_helper],
        );

        let inner = column(
            "inner",
            sp("spacing.lg"),
            vec![title, section, body_copy, table, meta, buttons, field_block],
        );
        let mut page = body("body", sp("spacing.xl"), vec![inner]);
        page.props
            .tokens
            .insert("background".into(), tok("surface.base"));
        page.props.padding = Some(InsetRefs::all(tok("spacing.xl")));
        page
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    /// Rebuilds its whole tree every pass, so `All` is the only honest
    /// answer, the same reason `Fixture::take_changes` gives.
    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }

    /// Published once. [`Host::pass`] reads this at the top of a pass, so
    /// the theme is in force from the second pass on.
    fn theme_request(&mut self) -> Option<Theme> {
        self.pending.take()
    }
}

fn sized(mut input: RawInput) -> RawInput {
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(PANEL[0], PANEL[1]),
    ));
    input
}

fn timed(clock: &mut f64, mut raw: RawInput) -> RawInput {
    *clock += 1.0 / 60.0;
    raw.time = Some(*clock);
    sized(raw)
}

/// Mean luminance of a decoded PNG, 0.0 (black) to 1.0 (white). Copied in
/// miniature from `shots.rs`'s `Camera::mean_luma`, which this file cannot
/// call: `Camera`'s fields are private to that module and there is no
/// public constructor generic over an arbitrary `App`.
fn mean_luma(png: &[u8]) -> f32 {
    let image = image::load_from_memory(png)
        .unwrap_or_else(|err| panic!("font spike: shot is not a PNG: {err}"))
        .to_rgba8();
    let mut total = 0.0_f64;
    let mut count = 0usize;
    for px in image.pixels() {
        let [r, g, b, _] = px.0;
        total += 0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b);
        count += 1;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a mean of u8 luminances divided by 255 is inside f32"
    )]
    {
        (total / count as f64 / 255.0) as f32
    }
}

/// Host [`Specimen`] under `theme` ("light" or "dark"), optionally with
/// `candidate`'s faces installed over Plex, photograph the settled frame,
/// and write it to `$PETRA_SHOT_DIR/<name>.png`.
///
/// The pass sequence mirrors `shots.rs`'s `Camera::fixture` exactly: one
/// pass right after [`Host::new`] to register the font atlas, then two
/// settling passes (the first delivers the theme request, the second lays
/// out and paints under it — and, when `candidate` is `Some`, under the
/// swapped font definitions egui applies starting at its next pass), then
/// the capture pass itself.
///
/// # Panics
/// If a candidate face cannot be read, if no frame is produced, if the
/// capture is refused, or if the settled frame does not read as the
/// requested theme's luminance. All are defects in this driver rather than
/// in the panel it is photographing.
fn render(theme_name: &str, candidate: Option<&CandidatePaths>, name: &str) {
    let picked = match theme_name {
        "dark" => gorgon_petra::token::dark(),
        "light" => gorgon_petra::token::light(),
        other => panic!("no shipped theme is named {other:?}"),
    };
    let app = Specimen {
        pending: Some(picked),
    };

    let ctx = Context::default();
    ctx.run_ui(sized(RawInput::default()), |_| {})
        .drop_without_applying_deltas();
    ctx.set_pixels_per_point(CAPTURE_SCALE);

    let mut host = Host::new(&ctx, app, default_presenter());
    // First pass: registers the font atlas under Plex, `Host::new`'s
    // default, before anything is overridden.
    ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx))
        .drop_without_applying_deltas();

    if let Some(paths) = candidate {
        install_candidate(&ctx, paths);
    }
    host.set_reduced_motion(true);

    let mut clock = ctx.input(|i| i.time);
    for _ in 0..2 {
        let raw = timed(&mut clock, RawInput::default());
        ctx.run_ui(raw, |_| host.pass(&ctx))
            .drop_without_applying_deltas();
    }

    let raw = timed(&mut clock, RawInput::default());
    let out = ctx.run_ui(raw, |_| host.pass(&ctx));
    let frame = host
        .frame()
        .unwrap_or_else(|| panic!("font spike: {name}: the shooting pass produced no frame"));
    let mut shooter = Snapshotter::new();
    let shot = shooter
        .capture(&ctx, &out, frame, None)
        .unwrap_or_else(|err| panic!("font spike: {name}: capture refused: {err:?}"));
    out.drop_without_applying_deltas();

    if let Some(dir) = std::env::var_os("PETRA_SHOT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("shot dir");
        std::fs::write(dir.join(format!("{name}.png")), &shot.png).expect("write shot");
    }

    let luma = mean_luma(&shot.png);
    match theme_name {
        "light" => assert!(luma > 0.5, "{name}: fixture is not light: luma {luma}"),
        _ => assert!(luma < 0.5, "{name}: fixture is not dark: luma {luma}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{SOURCE_SANS_3, render};

    /// Plex, the control, both themes.
    #[test]
    fn plex_control() {
        render("light", None, "plex-light");
        render("dark", None, "plex-dark");
    }

    /// Source Sans 3, the one candidate that survived the digit-advance
    /// screen, both themes.
    #[test]
    fn source_sans_3_candidate() {
        render("light", Some(&SOURCE_SANS_3), "source-sans-3-light");
        render("dark", Some(&SOURCE_SANS_3), "source-sans-3-dark");
    }
}
