//! Real pixels for the two-stroke rule material, sampled off the rendered
//! PNG bytes rather than read back from the requested theme offsets —
//! clamping is silent, so a test that asserts the request proves nothing.
//!
//! Acceptance criterion 3 of
//! `.agents/notes/proposed/architecture/2026-09-07-rule-material-two-stroke-groove.md`:
//! "A capture on each shipped theme shows the achieved strokes at the
//! expected levels, sampled from the image rather than the requested
//! offsets."
//!
//! `gorgon-petra-testkit`'s `Snapshotter` renders through a real headless
//! `wgpu` device — no window, no `DISPLAY`. This crate carries it as a
//! `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]` entry
//! (`Cargo.toml`), the same dependency `bin/gallery`'s own snapshot tests
//! use.

use std::ops::Range;
use std::sync::Arc;

use egui::{Context, Pos2, RawInput, Rect as EguiRect, vec2};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::{Presenter, ThemeMode, TokenName, dark, light};
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};
use gorgon_petra_egui::host::{App, Host};

/// Wide and tall enough to hold a real panel above the groove and the
/// groove itself, small enough that the capture is quick.
const WINDOW: [f32; 2] = [64.0, 48.0];

/// A shorthand for a well-formed shipped token name.
fn tok(name: &str) -> TokenName {
    TokenName::new(name).expect("well-formed token name")
}

/// One node: `surface.layer-one` fill, `border-bottom` bound to the rule
/// material's trigger token. The root, so it fills the whole viewport
/// regardless of `Stack`'s own zero-child measurement — the same
/// convention every fixture in this workspace's test suites relies on.
struct Fixture;

impl RowSource for Fixture {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Fixture {
    fn view(&mut self) -> ViewNode {
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("surface.layer-one"));
        props
            .tokens
            .insert("border-bottom".into(), tok("border.subtle"));
        let mut node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        node.constraints = Constraints {
            horizontal: AxisConstraint {
                min: Some(WINDOW[0]),
                max: Some(WINDOW[0]),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(WINDOW[1]),
                max: Some(WINDOW[1]),
                priority: 0,
            },
        };
        node
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

fn sized(mut input: RawInput) -> RawInput {
    input.screen_rect = Some(EguiRect::from_min_size(
        Pos2::ZERO,
        vec2(WINDOW[0], WINDOW[1]),
    ));
    input
}

fn headless() -> Context {
    let ctx = Context::default();
    ctx.run_ui(sized(RawInput::default()), |_| {})
        .drop_without_applying_deltas();
    ctx
}

/// One theme's capture: the rendered image, at 1 device pixel per logical
/// point so a pixel row is a device row with no scale arithmetic, and the
/// root node's own placed rect.
///
/// Set `PETRA_RULE_SHOT_DIR` to also write the PNG there — the same
/// discipline every other headless-capture test in this workspace uses
/// (`PETRA_SHOT_DIR`, `PETRA_TEXT_SHEET`), so the picture this test's
/// numbers came from can be opened and looked at rather than trusted.
fn capture(mode: ThemeMode) -> (image::RgbaImage, gorgon_petra::geom::Rect) {
    let (label, theme) = match mode {
        ThemeMode::Light => ("light", light()),
        ThemeMode::Dark => ("dark", dark()),
    };
    let ctx = headless();
    let mut host = Host::new(&ctx, Fixture, Presenter::new(theme));
    let output = ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx));
    let frame = host.frame().expect("the first pass produced a frame");
    let root = frame
        .placements
        .first()
        .expect("the fixture places its one node")
        .rect;

    let mut shooter = gorgon_petra_testkit::snapshot::Snapshotter::new();
    let shot = shooter
        .capture(&ctx, &output, frame, None)
        .expect("capture refused");
    output.drop_without_applying_deltas();

    if let Some(dir) = std::env::var_os("PETRA_RULE_SHOT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("shot dir");
        std::fs::write(dir.join(format!("rule-groove-{label}.png")), &shot.png)
            .expect("write shot");
    }

    let img = image::load_from_memory(&shot.png)
        .expect("shot is not a PNG")
        .to_rgba8();
    (img, root)
}

/// One [r, g, b] sample at device pixel `(x, y)`.
fn sample(img: &image::RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let p = img
        .get_pixel(x.min(img.width() - 1), y.min(img.height() - 1))
        .0;
    [p[0], p[1], p[2]]
}

/// Every channel of `a` within `tol` of the matching channel of `b` — a
/// small tolerance for the tessellator/GPU round trip, far inside the gap
/// between any two of {panel, shadow, highlight}, which are tens of levels
/// apart.
fn close(a: [u8; 3], b: [u8; 3], tol: i32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (i32::from(*x) - i32::from(*y)).abs() <= tol)
}

/// The groove, sampled from real rendered pixels, in both shipped themes.
///
/// Panel 242 (light `surface.layer-one`) grooves to shadow 214, highlight
/// 255. Panel 34 (dark `surface.layer-one`) grooves to shadow 6, highlight
/// 47. These are the exact numbers
/// `docs/experiments/successes/2026-09-07-deboss-rule-contrast-ratio.md`
/// sampled off its own winning specimen, and
/// `gorgon_petra::token::shipped::tests::rule_strokes_land_on_the_contract_construction`
/// holds the *token* values to them; this test holds the *painted* pixels
/// to them, through the real paint path, not the request.
#[test]
fn the_groove_paints_the_contracts_exact_levels_in_both_themes() {
    for (label, mode, panel, shadow, highlight) in [
        (
            "light",
            ThemeMode::Light,
            [242u8, 242, 242],
            [214u8, 214, 214],
            [255u8, 255, 255],
        ),
        (
            "dark",
            ThemeMode::Dark,
            [34u8, 34, 34],
            [6u8, 6, 6],
            [47u8, 47, 47],
        ),
    ] {
        let (img, root) = capture(mode);
        assert_eq!(
            img.width(),
            WINDOW[0] as u32,
            "{label}: captured at other than 1 device pixel per point"
        );
        let x = (root.x + root.w / 2.0) as u32;
        let bottom = (root.y + root.h) as u32 - 1;

        // Well above the groove: the panel itself, the control that proves
        // the fixture's own background is what the contract's numbers
        // assume.
        let panel_px = sample(&img, x, 4);
        assert!(
            close(panel_px, panel, 1),
            "{label}: panel sampled as {panel_px:?}, expected {panel:?}"
        );

        // The bottom two device rows: the highlight, physically lower —
        // light always comes from the top.
        let highlight_0 = sample(&img, x, bottom);
        let highlight_1 = sample(&img, x, bottom - 1);
        assert!(
            close(highlight_0, highlight, 1) && close(highlight_1, highlight, 1),
            "{label}: highlight rows sampled as {highlight_0:?} / \
             {highlight_1:?}, expected {highlight:?}"
        );

        // The two rows above that: the shadow.
        let shadow_0 = sample(&img, x, bottom - 2);
        let shadow_1 = sample(&img, x, bottom - 3);
        assert!(
            close(shadow_0, shadow, 1) && close(shadow_1, shadow, 1),
            "{label}: shadow rows sampled as {shadow_0:?} / {shadow_1:?}, \
             expected {shadow:?}"
        );

        // One row further up: back to the panel. The groove is exactly
        // four device pixels, not a thicker band.
        let past_groove = sample(&img, x, bottom - 4);
        assert!(
            close(past_groove, panel, 1),
            "{label}: the row above the groove is {past_groove:?}, not the \
             panel {panel:?} — the groove is thicker than four device \
             pixels"
        );
    }
}

/// The acceptance criterion this whole material exists to hold, read off
/// real pixels rather than off the theme: the highlight must be a lighter
/// grey than the shadow, in both themes. `#dddddd` (221) on a 242 panel —
/// the historical bug — would still sample as *a* highlight row here, but
/// it would not be lighter than a shadow computed the same broken way; this
/// is the pixel-level twin of
/// `the_rule_highlight_resolves_lighter_than_the_panel_it_offsets`.
#[test]
fn the_painted_highlight_is_lighter_than_the_painted_shadow_in_both_themes() {
    for (label, mode) in [("light", ThemeMode::Light), ("dark", ThemeMode::Dark)] {
        let (img, root) = capture(mode);
        let x = (root.x + root.w / 2.0) as u32;
        let bottom = (root.y + root.h) as u32 - 1;
        let highlight = sample(&img, x, bottom);
        let shadow = sample(&img, x, bottom - 2);
        assert!(
            u32::from(highlight[0]) > u32::from(shadow[0]),
            "{label}: painted highlight {highlight:?} is not lighter than \
             painted shadow {shadow:?}"
        );
    }
}

/// A panel filling the viewport with one [`NodeKind::Separator`] at its
/// leading edge, in `tone`, running along `axis`.
struct SeparatorFixture {
    axis: gorgon_petra::geom::Axis,
    tone: &'static str,
}

impl RowSource for SeparatorFixture {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for SeparatorFixture {
    fn view(&mut self) -> ViewNode {
        use gorgon_petra::geom::{Align, Axis};

        let mut rule_props = Props {
            axis: Some(self.axis),
            ..Props::default()
        };
        rule_props
            .tokens
            .insert("background".into(), tok(self.tone));
        let rule = ViewNode::new(NodeKind::Separator, "rule").with_props(rule_props);

        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("surface.layer-one"));
        // The stack runs across the rule so the rule is its first child at
        // the leading edge, and stretches on the cross axis so the rule
        // takes the panel's whole run. A separator measures zero along its
        // own axis under an unspecified offer; the stretch is what gives it
        // a length.
        props.axis = Some(match self.axis {
            Axis::Horizontal => Axis::Vertical,
            Axis::Vertical => Axis::Horizontal,
        });
        props.align = Some(Align::Stretch);
        let mut node = ViewNode::new(NodeKind::Stack, "root")
            .with_props(props)
            .child(rule);
        node.constraints = Constraints {
            horizontal: AxisConstraint {
                min: Some(WINDOW[0]),
                max: Some(WINDOW[0]),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(WINDOW[1]),
                max: Some(WINDOW[1]),
                priority: 0,
            },
        };
        node
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

/// Render one separator fixture and return the image.
fn capture_separator(
    axis: gorgon_petra::geom::Axis,
    tone: &'static str,
    mode: ThemeMode,
) -> image::RgbaImage {
    let (label, theme) = match mode {
        ThemeMode::Light => ("light", light()),
        ThemeMode::Dark => ("dark", dark()),
    };
    let ctx = headless();
    let mut host = Host::new(&ctx, SeparatorFixture { axis, tone }, Presenter::new(theme));
    let output = ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx));
    let frame = host.frame().expect("the first pass produced a frame");
    let mut shooter = gorgon_petra_testkit::snapshot::Snapshotter::new();
    let shot = shooter
        .capture(&ctx, &output, frame, None)
        .expect("capture refused");
    output.drop_without_applying_deltas();

    if let Some(dir) = std::env::var_os("PETRA_RULE_SHOT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("shot dir");
        let orientation = match axis {
            gorgon_petra::geom::Axis::Horizontal => "h",
            gorgon_petra::geom::Axis::Vertical => "v",
        };
        let name = format!(
            "separator-{orientation}-{}-{label}.png",
            tone.replace('.', "-")
        );
        std::fs::write(dir.join(name), &shot.png).expect("write shot");
    }

    image::load_from_memory(&shot.png)
        .expect("shot is not a PNG")
        .to_rgba8()
}

/// The second of the two constructions: a node that **is** a rule rather
/// than a node with a rule along one edge, grooving at the contract's exact
/// levels in both orientations and both themes.
///
/// Seven dividers in the component library were written the other way — a
/// childless stack carrying a `border.subtle` fill and a hand-pinned one
/// logical unit — and every one of them painted a flat line at level 97 on
/// the dark panel while a data-table row grooved to 6 and 47 on the same
/// token. Sampled off rendered pixels, because that is the only thing that
/// caught the split in the first place.
#[test]
fn a_separator_in_the_material_grooves_in_both_orientations_and_themes() {
    use gorgon_petra::geom::Axis;

    for (label, mode, panel, shadow, highlight) in [
        ("light", ThemeMode::Light, 242u8, 214u8, 255u8),
        ("dark", ThemeMode::Dark, 34u8, 6u8, 47u8),
    ] {
        // Horizontal: the groove runs across the top four device rows,
        // shadow above highlight, because light comes from above.
        let img = capture_separator(Axis::Horizontal, "border.subtle", mode);
        let x = img.width() / 2;
        let column: Vec<u8> = (0..5).map(|y| sample(&img, x, y)[0]).collect();
        assert_eq!(
            column,
            vec![shadow, shadow, highlight, highlight, panel],
            "{label} horizontal separator"
        );

        // Vertical: the same pair turned a quarter, shadow left of
        // highlight, because light also comes from the left.
        let img = capture_separator(Axis::Vertical, "border.subtle", mode);
        let y = img.height() / 2;
        let row: Vec<u8> = (0..5).map(|x| sample(&img, x, y)[0]).collect();
        assert_eq!(
            row,
            vec![shadow, shadow, highlight, highlight, panel],
            "{label} vertical separator"
        );
    }
}

/// The trigger is the token, and only the token. `border.strong` is a
/// control boundary rather than a rule in the contract's sense ("State
/// lines stay flat single strokes drawn over the top"), so a separator
/// carrying it paints one flat unit and reserves one, which is the number
/// `list_box`'s field arithmetic subtracts.
#[test]
fn a_separator_outside_the_material_stays_one_flat_unit() {
    use gorgon_petra::geom::Axis;

    let img = capture_separator(Axis::Horizontal, "border-strong", ThemeMode::Dark);
    let x = img.width() / 2;
    let first = sample(&img, x, 0);
    assert!(
        sample(&img, x, 1) == [34, 34, 34],
        "one device row of flat rule, then the panel: row 1 is {:?}",
        sample(&img, x, 1)
    );
    assert!(
        first != [34, 34, 34] && first[0] == first[1] && first[1] == first[2],
        "row 0 is one flat grey that is not the panel: {first:?}"
    );
}
