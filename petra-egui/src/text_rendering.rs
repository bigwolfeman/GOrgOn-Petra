//! How glyphs reach the screen: the coverage curve, the hinting mode and
//! sub-pixel placement, and the one place that writes them into egui.
//!
//! egui draws text itself. `skrifa` loads and hints an outline, `vello_cpu`
//! rasterises it to greyscale coverage, epaint writes that coverage into one
//! white atlas through a [`FontColorTransferFunction`], and the shader
//! multiplies it by the run's colour. Nothing the desktop configures
//! (fontconfig's `hintstyle`, `lcdfilter`, `rgba`) reaches any of that, so a
//! machine that wants a different edge has to say so here.
//!
//! # Where the values come from
//!
//! A [`TextRendering`] has three fields. Two come from the theme's
//! `text.coverage-curve` token ([`CoverageValue`]): `passes` and `snap`.
//! Hinting has no token, because it depends on the display (its pixel
//! density, how its panel renders a grey edge), not on the theme's look; it
//! defaults to [`Hinting::default`]. An application replaces the whole
//! thing with [`crate::host::Host::set_text_rendering`] when a user setting
//! names one, and clears the override to go back to the theme's.
//!
//! # How a change reaches the atlas
//!
//! epaint keeps one atlas per [`egui::epaint::text::TextOptions`] and
//! recreates it at the start of the pass after the options change
//! (`Fonts::begin_pass`). The curve, sub-pixel binning and the on/off
//! hinting switch are all `TextOptions` fields, written into both egui
//! styles by [`bind_text_options`]. The hinting *target* (full against
//! light) is not: it lives in each face's [`egui::FontTweak`], so a change
//! to it re-installs the current font definitions with every face retuned
//! ([`tune_definitions`]). Both writes land on the same pass boundary, so a
//! change costs one atlas rebuild, which [`AtlasWatch`] observes to drop
//! every galley shaped against the old atlas.

use std::sync::Arc;

use egui::epaint::FontColorTransferFunction;
use egui::epaint::text::{HintingTarget, SmoothHinting, TextOptions};
use egui::{Context, FontDefinitions};
use gorgon_petra::token::value::CoverageValue;
use gorgon_petra::token::{Theme, TokenName, TokenValue};

use crate::paint::TokenSource;

/// The token name every coverage-curve lookup reads, spelled once so the
/// atlas binding and [`crate::paint::TokenSource::coverage`] cannot drift
/// onto two different strings.
pub const COVERAGE_TOKEN: &str = "text.coverage-curve";

/// The pair a theme predating [`COVERAGE_TOKEN`] falls back to: exactly what
/// both shipped themes assign, so an application that hands the host an
/// older vocabulary draws text the way a current one does. `snap: false`
/// matches every theme this workspace has ever shipped.
pub const FALLBACK_COVERAGE: CoverageValue = CoverageValue {
    passes: 1.25,
    snap: false,
};

/// `theme`'s `text.coverage-curve`, or [`FALLBACK_COVERAGE`] when the theme's
/// vocabulary predates the token (an application built its own theme against
/// an older [`gorgon_petra::token::standard_vocabulary`] snapshot) or the
/// name resolves to some other token kind (a theme bug `Theme::build` would
/// have refused had this been declared at the wrong kind; defensive here
/// regardless, because this function must never panic on a hand-built
/// `Theme`).
pub fn coverage_value(theme: &Theme) -> CoverageValue {
    let name = TokenName::new(COVERAGE_TOKEN)
        .expect("\"text.coverage-curve\" is a well-formed token name");
    match theme.value(&name) {
        Some(TokenValue::Coverage(value)) => *value,
        _ => FALLBACK_COVERAGE,
    }
}

/// How hard the outline is pushed onto the pixel grid before it is
/// rasterised.
///
/// Hinting trades the designer's shapes for sharp stems. At 1x on a text
/// editor that trade is visible on every line: full hinting snaps a bowl's
/// extremes to whole pixels and the curve between them steps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Hinting {
    /// Fit both axes: the font's own instructions run unrestricted, so stems
    /// snap to whole pixels horizontally as well as vertically. Crispest
    /// stems, most distorted shapes. `skrifa`'s strong target
    /// (`HintingTarget::Mono`, FreeType's v35 interpreter): every smooth
    /// target runs a TrueType font in v40 backward-compatibility mode, which
    /// drops horizontal moves, so "normal" smooth hinting draws pixel for
    /// pixel what light does on IBM Plex and is not a third choice.
    ///
    /// The default: picked by the operator from a side-by-side A/B of every
    /// coverage and hinting pair at 1x (2026-11-23), together with
    /// `passes: 1.25`. The fractional pass puts weight back on the thin
    /// strokes that whole-pixel stems leave grey.
    #[default]
    Full,
    /// Fit vertical metrics only (FreeType's `hintslight`, and what its v40
    /// interpreter does for any smooth target): baselines and x-height land
    /// on whole pixels, horizontal shapes keep the font's own proportions.
    /// The default most Linux desktops ship, and what egui's own default
    /// tweak already drew, so it is the mode every Petra build drew with
    /// before this setting existed.
    Light,
    /// Rasterise the unhinted outline. Truest shapes, softest stems.
    Off,
}

impl Hinting {
    /// Every mode, in the order a picker lists them.
    pub const ALL: [Self; 3] = [Self::Full, Self::Light, Self::Off];

    /// The stable lower-case name a settings file stores.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Light => "light",
            Self::Off => "off",
        }
    }

    /// The mode `name` spells, or `None` for anything else.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == name)
    }

    /// Whether outlines are hinted at all.
    #[must_use]
    pub const fn enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// The `FontTweak::hinting_target` this mode installs. `Off` keeps the
    /// light target: with hinting disabled the target is never read, and a
    /// fixed value keeps [`definitions_tuned`] a plain equality.
    #[must_use]
    pub fn target(self) -> HintingTarget {
        match self {
            Self::Full => HintingTarget::Mono,
            Self::Light | Self::Off => HintingTarget::Smooth(SmoothHinting {
                light: true,
                ..SmoothHinting::default()
            }),
        }
    }
}

/// Everything about how a glyph is drawn that is not its face, size or
/// colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRendering {
    /// The coverage curve and sub-pixel placement; see [`CoverageValue`].
    pub coverage: CoverageValue,
    /// How hard outlines are fitted to the pixel grid.
    pub hinting: Hinting,
}

impl Default for TextRendering {
    fn default() -> Self {
        Self {
            coverage: FALLBACK_COVERAGE,
            hinting: Hinting::default(),
        }
    }
}

impl TextRendering {
    /// The theme's coverage curve with the default hinting: what a host
    /// draws with when no application override is in force.
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            coverage: coverage_value(theme),
            hinting: Hinting::default(),
        }
    }

    /// What this resolves to on the paint path.
    #[must_use]
    pub fn plan(&self) -> CoveragePlan {
        coverage_plan(self.coverage)
    }

    /// `Ok` when `passes` is finite and inside
    /// `[CoverageValue::MIN_PASSES, CoverageValue::MAX_PASSES]`, the same
    /// range `Theme::build` holds a theme's token to. An override is refused
    /// on the same terms, never clamped into range.
    ///
    /// # Errors
    ///
    /// [`PassesOutOfRange`] naming the refused value.
    pub fn check(&self) -> Result<(), PassesOutOfRange> {
        let passes = self.coverage.passes;
        if passes.is_finite()
            && (CoverageValue::MIN_PASSES..=CoverageValue::MAX_PASSES).contains(&passes)
        {
            Ok(())
        } else {
            Err(PassesOutOfRange(passes))
        }
    }
}

/// A [`TextRendering`] whose `passes` is not finite or falls outside the
/// legal range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassesOutOfRange(pub f32);

impl std::fmt::Display for PassesOutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "text coverage {} is outside [{}, {}]",
            self.0,
            CoverageValue::MIN_PASSES,
            CoverageValue::MAX_PASSES
        )
    }
}

impl std::error::Error for PassesOutOfRange {}

/// What one [`CoverageValue`] resolves to on the paint path: the atlas curve
/// [`bind_text_options`] writes onto the `Context`, and the paint-repeat
/// count and fractional-pass alpha [`crate::paint`] reads to repeat the
/// galley emission.
///
/// Public because it is the *only* authority on that mapping, and an
/// inspection tool that re-derived it would be a second authority able to
/// disagree, silently, in exactly the artifact a human is looking at to
/// judge the real thing. `tests/text_size_probe.rs` is the caller this was
/// opened for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoveragePlan {
    /// The curve the glyph atlas rasterises through.
    pub curve: FontColorTransferFunction,
    /// How many whole times to paint the galley.
    pub repeats: u8,
    /// `0.0` for no further pass; otherwise the fraction of the text
    /// colour's alpha to paint one further time, ai-macs' fractional-pass
    /// model ported verbatim (SPEC.md §1.4, §2.3).
    pub fraction: f32,
    /// Whether a glyph's horizontal origin snaps to a whole device pixel.
    /// Carried straight through from [`CoverageValue::snap`]; see that
    /// field's doc comment for the inverted mapping onto
    /// [`TextOptions::subpixel_binning`].
    pub snap: bool,
}

/// The gamma exponent that thins an edge the way `passes = n < 1` asks.
///
/// `1 - (1 - c)^n` is the curve every other `passes` value follows, but no
/// [`FontColorTransferFunction`] variant draws it for `n < 1`, and painting
/// at a lower alpha would only dim the whole glyph: the solid stem would go
/// grey with the edge. `Gamma(g)` (`alpha = c^g`) leaves full coverage at
/// full ink and only lowers partial coverage, so it is the right shape.
/// `g` is chosen so the two curves agree at half coverage, where an edge
/// pixel spends most of its time:
///
/// ```text
/// c^g = 1 - (1 - c)^n  at c = 1/2   =>   g = ln(1 - 2^-n) / ln(1/2)
/// ```
///
/// `n = 1` gives `g = 1` (the identity), `n = 0.7` gives `g ≈ 1.379` and
/// `n = 0.5` gives `g ≈ 1.772`.
#[must_use]
pub fn thinning_gamma(passes: f32) -> f32 {
    (1.0 - 0.5_f32.powf(passes)).ln() / 0.5_f32.ln()
}

/// `passes → (atlas curve, paint count, fractional-pass alpha)`. The rows at
/// and above `1.0` are the contract table `SPEC.md` §6.3/§6.4 pins; the
/// thinning row below `1.0` is the operator's (2026-11-23):
///
/// | `passes` | atlas curve | paints | effective n |
/// |---|---|---|---|
/// | `[0.5, 1.0)` | `Gamma(thinning_gamma(n))` | 1 | n (matched at c = ½) |
/// | 1.0 | `Off` | 1 | 1 |
/// | 2.0 | `TwoCoverageMinusCoverageSq` | 1 | 2 |
/// | 3.0 | `Off` | 3 | 3 |
/// | 4.0 | `TwoCoverageMinusCoverageSq` | 2 | 4 |
///
/// # The trap
///
/// `2c - c² ≡ 1 - (1-c)²`: `TwoCoverageMinusCoverageSq` *is* two-pass
/// compositing, so the atlas curve and the paint count MULTIPLY:
/// `effective_n = n_atlas · repeats`. `passes = 2.0` deliberately spends the
/// atlas curve rather than two paints (identical output, one fewer draw);
/// `passes = 4.0` spends it twice. Every other value at or above `1.0`
/// (every odd integer, and every fractional value, including one whose
/// floor is even) goes through `Off` instead, so the fractional pass below
/// always composites against raw coverage and stays exactly ai-macs' own
/// formula rather than needing a second, curve-scaled fractional term.
/// Painting `TwoCov` three times would be `n = 6`, not `n = 3`; no row of
/// this table, and no value this function returns for any legal `passes`,
/// pairs `TwoCov` with three paints
/// (`two_cov_at_three_paints_would_be_n_six_and_no_row_produces_it`).
#[must_use]
pub fn coverage_plan(value: CoverageValue) -> CoveragePlan {
    /// Float wobble tolerance for recognising the exact anchor points a
    /// value most likely lands on. Well under any daylight between two
    /// legal `passes` values anyone would assign on purpose.
    const EPS: f32 = 1e-4;

    let (curve, repeats, fraction) = if value.passes < 1.0 - EPS {
        (
            FontColorTransferFunction::Gamma(thinning_gamma(value.passes)),
            1,
            0.0,
        )
    } else if (value.passes - 2.0).abs() < EPS {
        (
            FontColorTransferFunction::TwoCoverageMinusCoverageSq,
            1,
            0.0,
        )
    } else if (value.passes - 4.0).abs() < EPS {
        (
            FontColorTransferFunction::TwoCoverageMinusCoverageSq,
            2,
            0.0,
        )
    } else {
        let whole = value.passes.floor().max(1.0);
        let frac = value.passes - whole;
        (
            FontColorTransferFunction::Off,
            whole as u8,
            if frac > 0.001 { frac } else { 0.0 },
        )
    };
    CoveragePlan {
        curve,
        repeats,
        fraction,
        snap: value.snap,
    }
}

/// Write `rendering`'s atlas inputs into both egui styles.
///
/// # Why this is a correctness fix and not a tuning knob
///
/// epaint rasterises every glyph once, white, into one atlas, and the shader
/// multiplies that coverage by the run's colour in gamma space
/// (`egui.wgsl`: `in.color * tex_gamma`). One atlas cannot be right for both
/// dark-on-light and light-on-dark, so epaint bends the coverage on the way
/// in and says so itself (`epaint/src/image.rs`): *"This whole thing is less
/// than rigorous."* The bend used to be hardcoded per `ThemeMode`, which
/// fixed light mode at the identity whatever anyone measured; it is now a
/// function of [`TextRendering`] only, so light and dark draw identically
/// unless a theme or a user deliberately diverges them.
///
/// # Why both egui themes are written
///
/// egui keeps a `Style` per *its own* theme and `Options::style()` picks by
/// that, which this crate never sets. Writing only the active one would
/// leave a host that later calls `Context::set_theme` reading a stale
/// curve, and writes nothing else of an application's `Visuals`.
///
/// No atlas clear is needed: `Fonts::begin_pass` compares the incoming
/// `TextOptions` against the ones the atlas was built with and recreates it
/// when they differ.
pub fn bind_text_options(ctx: &Context, rendering: &TextRendering) {
    let plan = rendering.plan();
    for egui_theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(egui_theme, |style| {
            let options = &mut style.visuals.text_options;
            options.color_transfer_function = plan.curve;
            options.subpixel_binning = !plan.snap;
            options.font_hinting = rendering.hinting.enabled();
        });
    }
}

/// Retune every face in `definitions` to `hinting`. Returns whether any face
/// changed.
///
/// Every face, not only Petra's own: a fallback face drawing one Arabic word
/// in a line of Plex should be fitted the way the line around it is. Public
/// so an application installing its own definitions through
/// `Context::set_fonts` can tune them first; [`crate::host::Host`] retunes a
/// stack that arrives untuned on the pass after it loads, at the cost of one
/// extra atlas rebuild.
pub fn tune_definitions(definitions: &mut FontDefinitions, hinting: Hinting) -> bool {
    let mut changed = false;
    for data in definitions.font_data.values_mut() {
        if !face_tuned(&data.tweak, hinting) {
            let tweak = &mut Arc::make_mut(data).tweak;
            tweak.hinting = Some(hinting.enabled());
            tweak.hinting_target = hinting.target();
            changed = true;
        }
    }
    changed
}

/// Whether every face in `definitions` is already tuned to `hinting`.
#[must_use]
pub fn definitions_tuned(definitions: &FontDefinitions, hinting: Hinting) -> bool {
    definitions
        .font_data
        .values()
        .all(|data| face_tuned(&data.tweak, hinting))
}

fn face_tuned(tweak: &egui::FontTweak, hinting: Hinting) -> bool {
    tweak.hinting == Some(hinting.enabled()) && tweak.hinting_target == hinting.target()
}

/// Installs font definitions tuned to the hinting in force, and retunes a
/// stack that arrives untuned.
///
/// egui applies `Context::set_fonts` at the start of the *next* pass, and
/// until then `Context::fonts` still answers with the old stack. A retune
/// that read the loaded stack while an install was still queued would
/// re-queue the old faces over the new ones (`Host::new` installs Plex, an
/// application sets a hinting mode in the same breath, and the shell draws
/// in egui's default face). So the tuner keeps what it queued until it
/// sees it loaded, and retunes that instead.
///
/// It cannot see a queue it did not make. An application that calls
/// `Context::set_fonts` itself should tune its definitions first
/// ([`tune_definitions`]); otherwise its stack is retuned on the pass after
/// it loads, at the cost of one more atlas rebuild.
#[derive(Debug, Default)]
pub(crate) struct FontTuner {
    queued: Option<(FontDefinitions, u64)>,
}

impl FontTuner {
    /// Tune `definitions` to `hinting` and queue them on `ctx`.
    pub(crate) fn install(
        &mut self,
        ctx: &Context,
        mut definitions: FontDefinitions,
        hinting: Hinting,
    ) {
        tune_definitions(&mut definitions, hinting);
        let key = definitions_key(&definitions);
        ctx.set_fonts(definitions.clone());
        self.queued = Some((definitions, key));
    }

    /// Re-queue the newest stack (the one queued, else the one loaded)
    /// tuned to `hinting`, when it is not already. Returns whether anything
    /// was queued.
    pub(crate) fn retune(&mut self, ctx: &Context, hinting: Hinting) -> bool {
        let loaded = ctx.fonts(|fonts| definitions_key(fonts.definitions()));
        if self.queued.as_ref().is_some_and(|(_, key)| *key == loaded) {
            self.queued = None;
        }
        let base = match &self.queued {
            Some((queued, _)) => (!definitions_tuned(queued, hinting)).then(|| queued.clone()),
            None => ctx.fonts(|fonts| {
                (!definitions_tuned(fonts.definitions(), hinting))
                    .then(|| fonts.definitions().clone())
            }),
        };
        let Some(definitions) = base else {
            return false;
        };
        self.install(ctx, definitions, hinting);
        true
    }
}

/// Notices that epaint recreated the glyph atlas since the last look.
///
/// A galley carries texture coordinates into the atlas it was shaped
/// against, so every galley a cache holds across a rebuild points at glyphs
/// that are no longer there. epaint rebuilds for three reasons, and none
/// announces itself: the `TextOptions` changed, the font definitions changed
/// (`Context::set_fonts`), or the atlas passed 80% full (`Fonts::begin_pass`).
/// The first two are compared directly. The third shows as the image height
/// going down: inside one atlas it only grows, and a full atlas is far
/// taller than the 32 rows a fresh one starts at.
#[derive(Debug, Default)]
pub(crate) struct AtlasWatch {
    last: Option<AtlasMark>,
    rebuilds: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AtlasMark {
    options: TextOptions,
    height: usize,
    /// The face handles and the family chains, hashed. Faces are compared by
    /// `Arc` identity, not by bytes: `set_fonts` hands over new `Arc`s for
    /// any face it changed, and comparing the bytes of a 32 MiB CJK stack
    /// every pass is what egui's own `set_fonts` comment warns about.
    definitions: u64,
}

fn definitions_key(definitions: &FontDefinitions) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for (name, data) in &definitions.font_data {
        name.hash(&mut hasher);
        Arc::as_ptr(data).hash(&mut hasher);
    }
    definitions.families.hash(&mut hasher);
    hasher.finish()
}

impl AtlasWatch {
    /// Look at `ctx`'s atlas. `true` when it was rebuilt since the last look.
    /// The first look only records.
    pub(crate) fn observe(&mut self, ctx: &Context) -> bool {
        let now = ctx.fonts(|fonts| AtlasMark {
            options: *fonts.options(),
            height: fonts.font_image_size()[1],
            definitions: definitions_key(fonts.definitions()),
        });
        let rebuilt = self.last.is_some_and(|last| {
            last.options != now.options
                || last.definitions != now.definitions
                || now.height < last.height
        });
        self.last = Some(now);
        if rebuilt {
            self.rebuilds += 1;
        }
        rebuilt
    }

    /// Rebuilds observed since construction.
    pub(crate) fn rebuilds(&self) -> u64 {
        self.rebuilds
    }
}

/// A token source that answers [`COVERAGE_TOKEN`] with the coverage in force
/// rather than the theme's, and everything else from the theme.
///
/// The paint path reads the repeat half of the coverage plan off its token
/// source ([`crate::paint::TokenSource::coverage`]); an override that only
/// reached the atlas would pair an override curve with the theme's paint
/// count. Every typed reader on [`TokenSource`] defaults to
/// [`TokenSource::value`], so overriding `value` here covers all of them.
pub(crate) struct WithCoverage<'a> {
    inner: &'a dyn TokenSource,
    coverage: TokenValue,
}

impl<'a> WithCoverage<'a> {
    pub(crate) fn new(inner: &'a dyn TokenSource, coverage: CoverageValue) -> Self {
        Self {
            inner,
            coverage: TokenValue::Coverage(coverage),
        }
    }
}

impl TokenSource for WithCoverage<'_> {
    fn color(&self, token: &str) -> Option<egui::Color32> {
        self.inner.color(token)
    }

    fn value(&self, token: &str) -> Option<&TokenValue> {
        if token == COVERAGE_TOKEN {
            Some(&self.coverage)
        } else {
            self.inner.value(token)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(passes: f32) -> CoveragePlan {
        coverage_plan(CoverageValue {
            passes,
            snap: false,
        })
    }

    /// The thinning row matches `1 - (1 - c)^n` at half coverage, leaves
    /// full coverage at full ink, and lies below the identity everywhere
    /// in between, so an edge gets lighter and a stem does not.
    #[test]
    fn a_pass_count_below_one_thins_partial_coverage_and_keeps_full_ink() {
        for n in [0.5_f32, 0.6, 0.7, 0.85, 0.99] {
            let curve = plan(n).curve;
            let FontColorTransferFunction::Gamma(g) = curve else {
                panic!("passes {n} must thin through a gamma curve, got {curve:?}");
            };
            assert!(g > 1.0, "passes {n}: gamma {g} must exceed 1");
            let target = 1.0 - 0.5_f32.powf(n);
            assert!(
                (curve.alpha_from_coverage(0.5) - target).abs() < 1e-5,
                "passes {n}: half coverage must land on 1-(1-c)^n = {target}"
            );
            assert!((curve.alpha_from_coverage(1.0) - 1.0).abs() < 1e-6);
            assert_eq!(curve.alpha_from_coverage(0.0), 0.0);
            for step in 1..20 {
                let c = step as f32 / 20.0;
                assert!(
                    curve.alpha_from_coverage(c) < c,
                    "passes {n}: coverage {c} must paint lighter than the identity"
                );
            }
            assert_eq!(plan(n).repeats, 1, "thinning never repeats a paint");
            assert_eq!(plan(n).fraction, 0.0);
        }
    }

    /// The two values the operator named, pinned to the formula's numbers.
    #[test]
    fn the_operators_two_thin_settings_map_to_their_gammas() {
        assert!((thinning_gamma(0.5) - 1.7716).abs() < 1e-3);
        assert!((thinning_gamma(0.7) - 1.3794).abs() < 1e-3);
        assert!((thinning_gamma(1.0) - 1.0).abs() < 1e-6);
    }

    /// Lighter as `passes` falls: the dial moves one way.
    #[test]
    fn thinning_is_monotone_in_the_pass_count() {
        let mut last = plan(1.0).curve.alpha_from_coverage(0.4);
        for n in [0.9_f32, 0.8, 0.7, 0.6, 0.5] {
            let now = plan(n).curve.alpha_from_coverage(0.4);
            assert!(now < last, "passes {n} must be lighter than the step above");
            last = now;
        }
    }

    #[test]
    fn one_pass_is_the_identity_with_no_curve() {
        assert_eq!(plan(1.0).curve, FontColorTransferFunction::Off);
        assert_eq!(plan(1.0).repeats, 1);
    }

    #[test]
    fn an_override_out_of_range_is_refused_not_clamped() {
        for bad in [0.0_f32, 0.49, 4.01, f32::NAN, f32::INFINITY] {
            let rendering = TextRendering {
                coverage: CoverageValue {
                    passes: bad,
                    snap: false,
                },
                hinting: Hinting::Light,
            };
            assert!(rendering.check().is_err(), "{bad} must be refused");
        }
        for ok in [0.5_f32, 0.7, 1.0, 4.0] {
            let rendering = TextRendering {
                coverage: CoverageValue {
                    passes: ok,
                    snap: false,
                },
                hinting: Hinting::Off,
            };
            assert_eq!(rendering.check(), Ok(()), "{ok} must be usable");
        }
    }

    #[test]
    fn hinting_names_round_trip() {
        for mode in Hinting::ALL {
            assert_eq!(Hinting::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(Hinting::parse("slight"), None);
    }

    /// Tuning reaches every face, reports a change exactly once, and the
    /// targets differ between full and light (the point of the setting).
    #[test]
    fn tuning_retargets_every_face_once() {
        let mut definitions = crate::fonts::design_system_definitions();
        assert!(!definitions_tuned(&definitions, Hinting::Light));
        assert!(tune_definitions(&mut definitions, Hinting::Light));
        assert!(definitions_tuned(&definitions, Hinting::Light));
        assert!(!tune_definitions(&mut definitions, Hinting::Light));
        assert!(!definitions_tuned(&definitions, Hinting::Full));
        assert_ne!(Hinting::Full.target(), Hinting::Light.target());
        assert!(tune_definitions(&mut definitions, Hinting::Off));
        assert!(
            definitions
                .font_data
                .values()
                .all(|data| data.tweak.hinting == Some(false))
        );
    }

    /// The paint path's token source answers the override for the coverage
    /// token and the theme for everything else.
    #[test]
    fn the_coverage_override_reaches_the_paint_path_and_nothing_else() {
        let theme = gorgon_petra::token::shipped(gorgon_petra::token::ThemeId::Gray100);
        let snapshot = gorgon_petra::token::ThemeSnapshot::new(theme, 1);
        let thin = CoverageValue {
            passes: 0.7,
            snap: true,
        };
        let tokens = WithCoverage::new(&snapshot, thin);
        assert_eq!(tokens.coverage(COVERAGE_TOKEN), Some(thin));
        assert_eq!(
            tokens.color("text.primary"),
            TokenSource::color(&snapshot, "text.primary")
        );
        assert_eq!(
            tokens.spacing("spacing-05"),
            TokenSource::spacing(&snapshot, "spacing-05")
        );
    }
}
