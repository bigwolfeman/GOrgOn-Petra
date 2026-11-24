//! A text-rendering change reaches the glyphs through the real host, live:
//! one atlas rebuild per change, every face retuned, no galley left pointing
//! into the old atlas, and the pixels after a live change are the pixels a
//! host built with that rendering from the start.
//!
//! `src/text_rendering.rs` holds the mapping's unit tests. This file is the
//! half that needs a `Context`, the font stack and a GPU.

use std::ops::Range;
use std::sync::Arc;

use egui::epaint::FontColorTransferFunction;
use egui::epaint::text::HintingTarget;
use egui::{Context, Pos2, RawInput, Rect as EguiRect, vec2};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::value::CoverageValue;
use gorgon_petra::token::{Presenter, ThemeId, TokenName, shipped};
use gorgon_petra::tree::{NodeKind, Props, TextWrap, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_egui::text_rendering::{Hinting, TextRendering};
use gorgon_petra_testkit::snapshot::Snapshotter;

const WINDOW: [f32; 2] = [420.0, 120.0];
const PROSE: &str = "Round bowls and straight stems: cocoa, illuminate, waxy vow 0O 1lI.";

#[derive(Default)]
struct Page {
    asks: Option<TextRendering>,
}

impl RowSource for Page {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Page {
    fn view(&mut self) -> ViewNode {
        let paragraph = ViewNode::new(NodeKind::Text, "p").with_props(Props {
            text: Some(PROSE.to_owned()),
            wrap: Some(TextWrap::Wrap),
            ..Props::default()
        });
        let mut page = ViewNode::new(NodeKind::Stack, "root").child(paragraph);
        page.props.tokens.insert(
            "background".into(),
            TokenName::new("surface.base").expect("token"),
        );
        page
    }

    fn handle(&mut self, _: &InputEvent, _: &Route, _: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }

    fn text_rendering_request(&mut self) -> Option<TextRendering> {
        self.asks
    }
}

fn sized() -> RawInput {
    RawInput {
        screen_rect: Some(EguiRect::from_min_size(
            Pos2::ZERO,
            vec2(WINDOW[0], WINDOW[1]),
        )),
        ..RawInput::default()
    }
}

fn host(page: Page) -> (Context, Host<Page>) {
    let ctx = Context::default();
    ctx.run_ui(sized(), |_| {}).drop_without_applying_deltas();
    let mut host = Host::new(&ctx, page, Presenter::new(shipped(ThemeId::Gray100)));
    host.set_reduced_motion(true);
    (ctx, host)
}

fn passes(ctx: &Context, host: &mut Host<Page>, n: usize) {
    for _ in 0..n {
        ctx.run_ui(sized(), |_| host.pass(ctx))
            .drop_without_applying_deltas();
    }
}

fn rendering(passes: f32, hinting: Hinting, snap: bool) -> TextRendering {
    TextRendering {
        coverage: CoverageValue { passes, snap },
        hinting,
    }
}

/// The loaded faces' hinting, one entry per distinct tweak.
fn loaded_targets(ctx: &Context) -> Vec<(Option<bool>, HintingTarget)> {
    let mut out: Vec<_> = ctx.fonts(|fonts| {
        fonts
            .definitions()
            .font_data
            .values()
            .map(|data| (data.tweak.hinting, data.tweak.hinting_target))
            .collect()
    });
    out.dedup();
    out
}

fn loaded_has_plex(ctx: &Context) -> bool {
    ctx.fonts(|fonts| {
        fonts
            .definitions()
            .font_data
            .contains_key("gorgon-plex-sans-regular")
    })
}

/// One change costs one rebuild, on the pass after it; idle passes and a
/// repeated identical request cost none; going back to the theme costs one.
#[test]
fn a_change_rebuilds_the_atlas_once_and_retunes_every_face() {
    let (ctx, mut host) = host(Page::default());
    passes(&ctx, &mut host, 3);
    assert_eq!(host.text_rendering(), TextRendering::default());
    assert_eq!(
        loaded_targets(&ctx),
        [(Some(true), Hinting::default().target())]
    );
    let before = host.atlas_rebuilds();

    let thin_full = rendering(0.7, Hinting::Full, true);
    host.set_text_rendering(Some(thin_full)).expect("in range");
    passes(&ctx, &mut host, 6);
    assert_eq!(host.atlas_rebuilds(), before + 1, "one change, one rebuild");
    assert_eq!(host.text_rendering(), thin_full);
    assert_eq!(loaded_targets(&ctx), [(Some(true), HintingTarget::Mono)]);
    let options = ctx.fonts(|fonts| *fonts.options());
    assert!(matches!(
        options.color_transfer_function,
        FontColorTransferFunction::Gamma(g) if g > 1.0
    ));
    assert!(!options.subpixel_binning, "snap on is binning off");
    assert!(loaded_has_plex(&ctx), "retuning kept the design faces");

    for _ in 0..3 {
        host.set_text_rendering(Some(thin_full)).expect("in range");
        passes(&ctx, &mut host, 1);
    }
    assert_eq!(host.atlas_rebuilds(), before + 1, "the same value is free");

    host.set_text_rendering(None)
        .expect("clearing is always legal");
    passes(&ctx, &mut host, 4);
    assert_eq!(host.atlas_rebuilds(), before + 2);
    assert_eq!(host.text_rendering(), TextRendering::default());
    assert_eq!(
        loaded_targets(&ctx),
        [(Some(true), Hinting::default().target())]
    );
}

/// `Host::new` queues the design faces; a hinting change asked for before
/// the first pass must retune *those*, not the egui defaults still loaded.
#[test]
fn a_change_before_the_first_pass_keeps_the_design_faces() {
    let (ctx, mut host) = host(Page::default());
    host.set_text_rendering(Some(rendering(1.0, Hinting::Off, false)))
        .expect("in range");
    passes(&ctx, &mut host, 3);
    assert!(loaded_has_plex(&ctx));
    assert_eq!(loaded_targets(&ctx), [(Some(false), Hinting::Off.target())]);
}

/// A stack installed after a change arrives tuned to the mode in force.
#[test]
fn script_fallbacks_install_already_tuned() {
    let (ctx, mut host) = host(Page::default());
    host.set_text_rendering(Some(rendering(1.0, Hinting::Full, false)))
        .expect("in range");
    passes(&ctx, &mut host, 3);
    let before = host.atlas_rebuilds();
    let _ = host.install_script_fallbacks(&ctx);
    passes(&ctx, &mut host, 4);
    assert_eq!(loaded_targets(&ctx), [(Some(true), HintingTarget::Mono)]);
    assert_eq!(
        host.atlas_rebuilds(),
        before + 1,
        "no second, retuning rebuild"
    );
}

/// A stack the application installs by hand, untuned, is retuned on the
/// pass after it loads.
#[test]
fn a_hand_installed_stack_is_retuned() {
    let (ctx, mut host) = host(Page::default());
    passes(&ctx, &mut host, 2);
    ctx.set_fonts(gorgon_petra_egui::fonts::design_system_definitions());
    passes(&ctx, &mut host, 4);
    assert_eq!(
        loaded_targets(&ctx),
        [(Some(true), Hinting::default().target())]
    );
}

/// The application's request is the setting's path: it reaches the host
/// without a call from outside the pass, and an out-of-range one is
/// refused by the setter.
#[test]
fn the_application_request_reaches_the_glyphs() {
    let want = rendering(0.5, Hinting::Off, false);
    let (ctx, mut host) = host(Page { asks: Some(want) });
    passes(&ctx, &mut host, 3);
    assert_eq!(host.text_rendering(), want);
    assert!(
        host.set_text_rendering(Some(rendering(0.4, Hinting::Off, false)))
            .is_err()
    );
    assert_eq!(host.text_rendering(), want, "a refusal changes nothing");
}

fn shoot(shooter: &mut Snapshotter, ctx: &Context, host: &mut Host<Page>) -> Vec<u8> {
    let output = ctx.run_ui(sized(), |_| host.pass(ctx));
    let frame = host.frame().expect("a frame").clone();
    let shot = shooter
        .capture(ctx, &output, &frame, None)
        .expect("capture");
    output.drop_without_applying_deltas();
    shot.png
}

/// How many bytes of the two decoded pictures differ.
fn differing(a: &[u8], b: &[u8]) -> usize {
    let decode = |png: &[u8]| {
        image::load_from_memory(png)
            .expect("png")
            .to_rgba8()
            .into_raw()
    };
    let (a, b) = (decode(a), decode(b));
    assert_eq!(a.len(), b.len(), "same picture size");
    a.iter().zip(&b).filter(|(x, y)| x != y).count()
}

/// The pixels after a live change are the pixels of a host that started
/// with that rendering: nothing shaped against the old atlas survives the
/// rebuild, and the paint repeats follow the override, not the theme.
#[test]
fn a_live_change_draws_what_a_fresh_host_draws() {
    let mut shooter = Snapshotter::new();
    let (live_ctx, mut live) = host(Page::default());
    passes(&live_ctx, &mut live, 3);
    let theme_png = shoot(&mut shooter, &live_ctx, &mut live);
    for want in [
        rendering(0.7, Hinting::Full, false),
        rendering(2.0, Hinting::Off, true),
        rendering(1.5, Hinting::Light, false),
    ] {
        live.set_text_rendering(Some(want)).expect("in range");
        passes(&live_ctx, &mut live, 3);
        let live_png = shoot(&mut shooter, &live_ctx, &mut live);

        let (fresh_ctx, mut fresh) = host(Page { asks: Some(want) });
        passes(&fresh_ctx, &mut fresh, 3);
        let fresh_png = shoot(&mut shooter, &fresh_ctx, &mut fresh);

        if let Some(dir) = std::env::var_os("PETRA_SHOT_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).expect("dir");
            let name = format!(
                "live-{}-{}-{}",
                want.coverage.passes,
                want.hinting.as_str(),
                want.coverage.snap
            );
            std::fs::write(dir.join(format!("{name}.png")), &live_png).expect("png");
        }
        assert_eq!(
            differing(&live_png, &fresh_png),
            0,
            "{want:?}: a live change must draw what a fresh host draws"
        );
        assert!(
            differing(&live_png, &theme_png) > 0,
            "{want:?}: the change must be visible"
        );
    }
}
