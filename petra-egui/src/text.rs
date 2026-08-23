//! Text shaping: Petra's [`ContentMeasure`] boundary, implemented with egui
//! galleys.
//!
//! Petra asks "how big is this run, and did anything get dropped"; this module
//! answers by laying out a real galley and reading it. The galley is kept, so
//! the run that gets painted is byte-for-byte the run that was measured — the
//! alternative, measuring with one code path and painting with another, is how
//! a toolkit ends up with text that overflows the box it was sized for.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Context, FontFamily, FontId, Galley};
use gorgon_petra::cache::LruCache;
use gorgon_petra::geom::Size;
use gorgon_petra::layout::{ContentMeasure, SizeProposal, TextMeasurement, TextRequest};
use gorgon_petra::token::{Theme, TokenValue, TypographyValue, TypographyWeight};
use gorgon_petra::tree::TextWrap;

/// The character appended where a run was elided.
pub const ELLIPSIS: char = '…';

/// Which egui font family draws each weight class.
///
/// [`TypographyWeight`] names a role — regular, medium, bold — and egui has
/// no weight axis to hand it to: a [`FontId`] carries a size and a *family*,
/// and a weight is a different face installed under a different family name.
/// This is that mapping. It is host state rather than theme state because
/// which faces exist is a property of the font stack the host installed, not
/// of the design system the theme describes.
///
/// # What the default cannot do
///
/// [`FontFaces::default`] points all three classes at
/// [`FontFamily::Proportional`], because egui's built-in font definitions
/// install exactly one proportional face (`Ubuntu-Light`) and one monospace
/// face — there is no bold face to point at. Under the default, a token
/// declared `Bold` therefore paints at regular weight. That is a real limit
/// of the default font stack and not a hole in this map, so it is *stated*
/// rather than hidden: [`FontFaces::distinguishes_weight`] answers `false`
/// and a host can say so before an operator has to notice on screen.
///
/// A host that installs a bold face through `egui::Context::set_fonts` binds
/// it here with [`FontFaces::set`] and the weight channel starts working,
/// with no change to any theme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFaces {
    regular: FontFamily,
    medium: FontFamily,
    bold: FontFamily,
}

impl Default for FontFaces {
    fn default() -> Self {
        Self {
            regular: FontFamily::Proportional,
            medium: FontFamily::Proportional,
            bold: FontFamily::Proportional,
        }
    }
}

impl FontFaces {
    /// One family per weight class.
    #[must_use]
    pub fn new(regular: FontFamily, medium: FontFamily, bold: FontFamily) -> Self {
        Self {
            regular,
            medium,
            bold,
        }
    }

    /// The family `weight` draws through.
    #[must_use]
    pub fn family(&self, weight: TypographyWeight) -> &FontFamily {
        match weight {
            TypographyWeight::Regular => &self.regular,
            TypographyWeight::Medium => &self.medium,
            TypographyWeight::Bold => &self.bold,
        }
    }

    /// Point `weight` at `family`.
    pub fn set(&mut self, weight: TypographyWeight, family: FontFamily) {
        let slot = match weight {
            TypographyWeight::Regular => &mut self.regular,
            TypographyWeight::Medium => &mut self.medium,
            TypographyWeight::Bold => &mut self.bold,
        };
        *slot = family;
    }

    /// Whether a bold token can actually paint bolder than a regular one.
    ///
    /// `false` when two weight classes share one family — which is what
    /// [`FontFaces::default`] does, because the default font stack has one
    /// face to share. This is the honest report that the weight channel is
    /// off, and it is the reason `weight` is not silently dropped here.
    #[must_use]
    pub fn distinguishes_weight(&self) -> bool {
        self.regular != self.medium && self.medium != self.bold && self.regular != self.bold
    }
}

/// One typography token, resolved into what egui needs to shape it.
///
/// A [`FontId`] is not the whole of a typography token. The token also
/// carries a line height, and egui takes that on the text *format*, not on
/// the font — so a map of `name -> FontId` has nowhere to put it, and
/// `TypographyValue::line_height` had no consumer anywhere in the workspace
/// for exactly that reason. Carrying both together is what stops the
/// line-height half from being dropped between the theme and the galley.
///
/// Weight is not a third field here: it is spent at construction, choosing
/// the family in `font` through [`FontFaces`] — see that type for what the
/// default font stack can and cannot express.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Size and family.
    pub font: FontId,
    /// Distance between the bottom rows of two subsequent lines, in logical
    /// units. `None` leaves it to the font's own metrics.
    pub line_height: Option<f32>,
}

impl TextStyle {
    /// A style at `font`, using the font's own line height.
    #[must_use]
    pub fn new(font: FontId) -> Self {
        Self {
            font,
            line_height: None,
        }
    }

    /// The same style with an explicit line height.
    #[must_use]
    pub fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = Some(line_height);
        self
    }

    /// What a theme's typography token asks for, drawn through `faces`.
    #[must_use]
    pub fn from_token(value: &TypographyValue, faces: &FontFaces) -> Self {
        Self {
            font: FontId::new(value.size, faces.family(value.weight).clone()),
            line_height: Some(value.line_height),
        }
    }
}

/// Maps Petra typography token names to egui text styles.
///
/// One namespace, end to end. The keys are the token names the theme
/// declares and the view tree references (`typography.body`,
/// `typography.heading`, `typography.heading-lg`, …) because they are read
/// *off the theme* by [`Typography::from_theme`], not written out a second
/// time here.
///
/// The first version of this map was written out a second time here, keyed
/// on `body`/`heading`/`small`/`mono` — a private vocabulary no theme and no
/// component ever used. Every lookup missed, every miss fell back to the
/// default, and the entire declared type ramp painted at 14 units with every
/// gate green. Deriving the keys from the theme is what makes that
/// unrepresentable rather than merely fixed; [`Typography::resolve`] is what
/// makes a miss visible if one ever happens again.
#[derive(Clone, Debug)]
pub struct Typography {
    default: TextStyle,
    styles: BTreeMap<String, TextStyle>,
}

impl Default for Typography {
    /// The shipped design system's ramp, taken from
    /// [`gorgon_petra::token::dark`] — which is the same ramp
    /// [`gorgon_petra::token::light`] carries, because typography is type
    /// and not colour and the two shipped themes share one ramp.
    fn default() -> Self {
        Self::from_theme(&gorgon_petra::token::dark(), &FontFaces::default())
    }
}

impl Typography {
    /// The token a run that names no style of its own is shaped at.
    ///
    /// `Props::style` is documented as "`None` for the theme's body style",
    /// and this is that style's name.
    pub const BODY: &'static str = "typography.body";

    /// The size the fallback style takes when a theme declares no
    /// [`Typography::BODY`]. Reachable only through a theme that has no body
    /// text, which the shipped vocabulary forbids.
    const FALLBACK_SIZE: f32 = 14.0;

    /// An empty map whose every lookup falls back to `default`.
    #[must_use]
    pub fn new(default: TextStyle) -> Self {
        Self {
            default,
            styles: BTreeMap::new(),
        }
    }

    /// Every typography token `theme` defines, under its own name.
    ///
    /// This is the binding contract C6 asks for on the typography side: the
    /// names a tree is validated against and the names this map answers for
    /// both come from one theme, so a run whose `style` passed validation
    /// resolves here.
    #[must_use]
    pub fn from_theme(theme: &Theme, faces: &FontFaces) -> Self {
        let mut styles = BTreeMap::new();
        for (name, value) in theme.values() {
            if let TokenValue::Typography(typography) = value {
                styles.insert(
                    name.as_str().to_owned(),
                    TextStyle::from_token(typography, faces),
                );
            }
        }
        let default = styles.get(Self::BODY).cloned().unwrap_or_else(|| {
            TextStyle::new(FontId::new(
                Self::FALLBACK_SIZE,
                faces.family(TypographyWeight::Regular).clone(),
            ))
        });
        Self { default, styles }
    }

    /// Bind a token name to a style.
    pub fn set(&mut self, name: impl Into<String>, style: TextStyle) {
        self.styles.insert(name.into(), style);
    }

    /// The style bound to `name`, or `None` when nothing is bound.
    #[must_use]
    pub fn style(&self, name: &str) -> Option<&TextStyle> {
        self.styles.get(name)
    }

    /// The style a run shapes at, paired with the token name that did not
    /// resolve.
    ///
    /// The two answers come back together on purpose. Shaping still has to
    /// produce a style — a run with no font is a blank panel, which is worse
    /// on screen than a run at the wrong size — and the caller still has to
    /// be able to say *which* name went unresolved. Returning only the
    /// style, with the miss folded into the default, is exactly what let the
    /// whole ramp paint at one size: the theme-completeness check that was
    /// cited as the safety net proves a token exists in the *vocabulary* and
    /// says nothing about this map.
    ///
    /// [`crate::paint::paint_frame`] puts the second half of this answer
    /// into [`crate::paint::PaintReport::unresolved_tokens`], where every
    /// other token that failed to resolve already is.
    #[must_use]
    pub fn resolve<'a>(&self, style: Option<&'a str>) -> (&TextStyle, Option<&'a str>) {
        let Some(name) = style else {
            return (&self.default, None);
        };
        match self.styles.get(name) {
            Some(bound) => (bound, None),
            None => (&self.default, Some(name)),
        }
    }

    /// Bound token names, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.styles.keys().map(String::as_str).collect()
    }
}

/// Everything that decides what a galley looks like.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct GalleyKey {
    text: String,
    /// Size, family, and line height flattened into one comparable string:
    /// two runs that differ in any of the three are two galleys. Bit
    /// patterns, not values, for the same reason `wrap_bits` is.
    style: String,
    /// Bit pattern, not value: two wrap widths that differ in the last bit are
    /// two galleys, and comparing them as floats would return one for both.
    wrap_bits: u32,
    /// Display scale, same bit-pattern reasoning as `wrap_bits`.
    ///
    /// Shaping is scale-dependent: glyph advances are measured at
    /// `round(font_size * pixels_per_point)` physical pixels and row heights
    /// are rounded to whole pixels, so one run answers different extents — and
    /// wraps at different words — on two monitors of different scale. egui
    /// keys its own galley cache by `(job, pixels_per_point)` for the same
    /// reason. Leaving it out here served the old monitor's text after a
    /// window was dragged to the new one.
    scale_bits: u32,
    max_rows: usize,
    break_anywhere: bool,
    overflow: Option<char>,
}

/// Content measurement backed by egui galleys.
pub struct GalleyShaper {
    ctx: Context,
    typography: Typography,
    galleys: LruCache<GalleyKey, Arc<Galley>>,
    /// Image sources asked for but not loadable yet, recorded rather than
    /// silently sized to zero.
    unsupported_images: BTreeSet<String>,
    /// Custom node kinds asked for with no registered measurer.
    unsupported_customs: BTreeSet<String>,
    shaped: u64,
    reused: u64,
}

impl GalleyShaper {
    /// Galleys a shaper from [`GalleyShaper::new`] keeps.
    ///
    /// The cache is keyed by the text itself, so it grows with every distinct
    /// string ever shaped — a long list of distinct row labels would grow it
    /// without limit, which is the same thing SC-008 forbids of the
    /// measurement cache. The bound is lower than the measurement cache's
    /// because a galley carries its glyph rows and is the heavier object; it
    /// is still well above the number of distinct runs one screen holds.
    /// Raise it with [`GalleyShaper::set_capacity`] and watch
    /// [`GalleyShaper::evictions`] to find out whether you need to.
    pub const DEFAULT_CAPACITY: usize = 2048;

    /// A shaper over `ctx` with the default typography.
    #[must_use]
    pub fn new(ctx: Context) -> Self {
        Self::with_typography(ctx, Typography::default())
    }

    /// A shaper over `ctx` with an explicit typography map.
    #[must_use]
    pub fn with_typography(ctx: Context, typography: Typography) -> Self {
        Self {
            ctx,
            typography,
            galleys: LruCache::with_capacity(Self::DEFAULT_CAPACITY),
            unsupported_images: BTreeSet::new(),
            unsupported_customs: BTreeSet::new(),
            shaped: 0,
            reused: 0,
        }
    }

    /// The galley bound in force.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.galleys.capacity()
    }

    /// Change the galley bound, evicting immediately if it shrank.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.galleys.set_capacity(capacity);
    }

    /// Galleys dropped to stay inside the bound since construction. A number
    /// that climbs every frame means one screen holds more distinct runs than
    /// the bound, and the shaper is re-shaping text it just had.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.galleys.evictions()
    }

    /// The typography map in force — the read side of the binding, and what
    /// [`crate::paint::paint_frame`] consults to name a style token that did
    /// not resolve.
    #[must_use]
    pub fn typography(&self) -> &Typography {
        &self.typography
    }

    /// The typography map, for the theme to rebind. [`crate::host::Host`]
    /// writes it from the published theme on every revision it has not
    /// already bound, so the names this map answers for and the names a tree
    /// was validated against stay one namespace.
    pub fn typography_mut(&mut self) -> &mut Typography {
        &mut self.typography
    }

    /// Drop every cached galley.
    ///
    /// Call this when the font definitions or the theme's typography change.
    /// Everything else that affects a galley — the display scale included — is
    /// in the key, so a scale change needs no call: the entries for the old
    /// scale are simply never asked for again and age out.
    pub fn clear(&mut self) {
        self.galleys.clear();
    }

    /// Galleys shaped and galleys reused since construction.
    #[must_use]
    pub fn stats(&self) -> (u64, u64) {
        (self.shaped, self.reused)
    }

    /// Image sources this shaper could not size, sorted.
    #[must_use]
    pub fn unsupported_images(&self) -> Vec<&str> {
        self.unsupported_images.iter().map(String::as_str).collect()
    }

    /// Custom kinds this shaper could not size, sorted.
    #[must_use]
    pub fn unsupported_customs(&self) -> Vec<&str> {
        self.unsupported_customs
            .iter()
            .map(String::as_str)
            .collect()
    }

    /// The galley for `req`, shaping it if this is the first time.
    ///
    /// The painter calls this with the same request the measurement used, gets
    /// the same `Arc`, and therefore paints exactly what was measured.
    pub fn galley(&mut self, req: &TextRequest<'_>) -> Arc<Galley> {
        // Cloned rather than borrowed: the shaping below needs `&mut self`
        // for the cache and the counters, and a `TextStyle` is a size, a
        // family handle and an optional float.
        let style = self.typography.resolve(req.style).0.clone();
        let (max_rows, break_anywhere, overflow) = wrapping(req);
        let wrap_width = req.available_width.unwrap_or(f32::INFINITY);
        let key = GalleyKey {
            text: req.text.to_owned(),
            style: format!(
                "{}:{:?}:{:?}",
                style.font.size.to_bits(),
                style.line_height.map(f32::to_bits),
                style.font.family
            ),
            wrap_bits: wrap_width.to_bits(),
            scale_bits: self.ctx.pixels_per_point().to_bits(),
            max_rows,
            break_anywhere,
            overflow,
        };
        if let Some(hit) = self.galleys.get(&key) {
            self.reused += 1;
            return Arc::clone(hit);
        }
        self.shaped += 1;
        // Built section-by-hand rather than through `LayoutJob::simple`,
        // which takes a `FontId` and has nowhere to put a line height.
        // `single_section` leaves `wrap` at its default, so the wrap width
        // `simple` would have set is set here.
        let mut job = LayoutJob::single_section(
            req.text.to_owned(),
            TextFormat {
                font_id: style.font.clone(),
                color: Color32::PLACEHOLDER,
                line_height: style.line_height,
                ..TextFormat::default()
            },
        );
        job.wrap.max_width = wrap_width;
        job.wrap.max_rows = max_rows;
        job.wrap.break_anywhere = break_anywhere;
        job.wrap.overflow_character = overflow;
        let galley = self.ctx.fonts_mut(|fonts| fonts.layout_job(job));
        self.galleys.insert(key, Arc::clone(&galley));
        galley
    }
}

/// The egui wrapping parameters for a Petra text request.
fn wrapping(req: &TextRequest<'_>) -> (usize, bool, Option<char>) {
    match req.wrap {
        // Wrapping prefers word boundaries. An ellipsis appears only when a
        // line cap forces the run to stop early — an uncapped wrap that runs
        // out of height is truncated by the parent, not here.
        TextWrap::Wrap => (
            req.max_lines.unwrap_or(usize::MAX).max(1),
            false,
            req.max_lines.map(|_| ELLIPSIS),
        ),
        // Single line with a marker. egui's own note: with `max_rows = 1`,
        // `break_anywhere` must be on or the run is cut at the last word
        // boundary instead of at the box edge.
        TextWrap::Ellipsis => (1, true, Some(ELLIPSIS)),
        // Single line, hard cut, no marker.
        TextWrap::Clip => (1, true, None),
    }
}

impl ContentMeasure for GalleyShaper {
    fn text(&mut self, req: &TextRequest<'_>) -> TextMeasurement {
        let galley = self.galley(req);
        let size = galley.rect.size();
        TextMeasurement {
            size: Size::new(size.x, size.y),
            truncated: galley.elided,
            lines: galley.rows.len(),
        }
    }

    fn image(&mut self, source: &str, _proposal: SizeProposal) -> Size {
        // No image loader is wired yet. Returning zero is the honest answer —
        // an invented size would reserve space for a picture that will never
        // appear — and the source is recorded so the host can say which images
        // it could not draw instead of the panel just looking wrong.
        self.unsupported_images.insert(source.to_owned());
        Size::ZERO
    }

    fn custom(&mut self, name: &str, _proposal: SizeProposal) -> Size {
        // Same reasoning as `image`: a custom kind with no registered measurer
        // takes no space, and says so.
        self.unsupported_customs.insert(name.to_owned());
        Size::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::{ELLIPSIS, FontFaces, GalleyShaper, TextStyle, Typography};
    use egui::{Context, FontFamily, FontId, RawInput};
    use gorgon_petra::layout::{ContentMeasure, SizeProposal, TextRequest};
    use gorgon_petra::token::{TypographyValue, TypographyWeight, dark, light};
    use gorgon_petra::tree::TextWrap;

    /// A headless egui context, brought up and torn down honestly.
    ///
    /// Two things a windowless test has to do that a real integration does for
    /// free. egui has no fonts until a pass has run, and shaping fills the font
    /// atlas with texture deltas that epaint refuses to let you drop unhandled
    /// — a real integration uploads them to the GPU. This guard runs a pass at
    /// construction to bring the fonts up and another on drop to take the
    /// deltas the test produced, so a headless test is not mistaken for a
    /// leaking integration.
    struct Headless(Context);

    impl Headless {
        fn new() -> Self {
            let ctx = Context::default();
            pass(&ctx);
            Self(ctx)
        }

        fn shaper(&self) -> GalleyShaper {
            GalleyShaper::new(self.0.clone())
        }

        fn shaper_with(&self, typography: Typography) -> GalleyShaper {
            GalleyShaper::with_typography(self.0.clone(), typography)
        }

        /// Move this context to a different display scale, the way dragging a
        /// window between two monitors does. egui applies the new factor at
        /// the start of the next pass, so a pass is run here.
        fn set_scale(&self, pixels_per_point: f32) {
            self.0.set_pixels_per_point(pixels_per_point);
            pass(&self.0);
            assert_eq!(self.0.pixels_per_point(), pixels_per_point);
        }
    }

    impl Drop for Headless {
        fn drop(&mut self) {
            pass(&self.0);
        }
    }

    fn pass(ctx: &Context) {
        ctx.run_ui(RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }

    fn req<'a>(text: &'a str, width: Option<f32>, wrap: TextWrap) -> TextRequest<'a> {
        TextRequest {
            text,
            style: None,
            wrap,
            max_lines: None,
            available_width: width,
        }
    }

    #[test]
    fn an_open_probe_gives_one_unwrapped_line() {
        let h = Headless::new();
        let mut s = h.shaper();
        let m = s.text(&req("the quick brown fox", None, TextWrap::Wrap));
        assert_eq!(m.lines, 1);
        assert!(!m.truncated);
        assert!(m.size.w > 0.0 && m.size.h > 0.0, "{:?}", m.size);
    }

    #[test]
    fn a_narrow_offer_wraps_and_grows_taller() {
        let h = Headless::new();
        let mut s = h.shaper();
        let wide = s.text(&req("the quick brown fox jumps", None, TextWrap::Wrap));
        let narrow = s.text(&req(
            "the quick brown fox jumps",
            Some(60.0),
            TextWrap::Wrap,
        ));
        assert!(narrow.lines > wide.lines, "{narrow:?} vs {wide:?}");
        assert!(narrow.size.h > wide.size.h);
        assert!(narrow.size.w <= 60.0 + 0.01, "{:?}", narrow.size);
        assert!(!narrow.truncated, "an uncapped wrap drops nothing");
    }

    /// `elided` is the only honest source for the semantic tree's `truncated`
    /// flag: it is set by the shaper that actually dropped the glyphs.
    #[test]
    fn clipping_and_eliding_report_truncation_and_wrapping_does_not() {
        let h = Headless::new();
        let mut s = h.shaper();
        let text = "the quick brown fox jumps over the lazy dog";
        assert!(s.text(&req(text, Some(40.0), TextWrap::Clip)).truncated);
        assert!(s.text(&req(text, Some(40.0), TextWrap::Ellipsis)).truncated);
        assert!(!s.text(&req(text, Some(40.0), TextWrap::Wrap)).truncated);
        for wrap in [TextWrap::Clip, TextWrap::Ellipsis] {
            assert_eq!(s.text(&req(text, Some(40.0), wrap)).lines, 1);
        }
    }

    #[test]
    fn a_line_cap_truncates_and_marks_the_cut() {
        let h = Headless::new();
        let mut s = h.shaper();
        let text = "the quick brown fox jumps over the lazy dog";
        let capped = s.text(&TextRequest {
            max_lines: Some(2),
            ..req(text, Some(60.0), TextWrap::Wrap)
        });
        assert_eq!(capped.lines, 2);
        assert!(capped.truncated);
        let galley = s.galley(&TextRequest {
            max_lines: Some(2),
            ..req(text, Some(60.0), TextWrap::Wrap)
        });
        assert!(
            galley.job.text.is_empty() || galley.elided,
            "a capped run must report the cut"
        );
        assert_eq!(ELLIPSIS, '…');
    }

    /// The measured galley and the painted galley must be the same object, or
    /// what is drawn is not what was sized.
    #[test]
    fn measuring_then_painting_reuses_one_galley() {
        let h = Headless::new();
        let mut s = h.shaper();
        let r = req("reused", Some(200.0), TextWrap::Wrap);
        let first = s.galley(&r);
        let _ = s.text(&r);
        let second = s.galley(&r);
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert_eq!(s.stats().0, 1, "shaped exactly once");
        assert_eq!(s.stats().1, 2, "reused twice");
        // `clear()` drops this shaper's map, not egui's. egui memoizes galleys
        // too, so the re-shaped galley is legitimately the same `Arc` — what
        // `clear()` guarantees is that this shaper asked again, which is what
        // the miss count shows.
        s.clear();
        let _third = s.galley(&r);
        assert_eq!(s.stats().0, 2, "clear() forces a fresh lookup");
        assert_eq!(s.stats().1, 2, "and it is not served from the local map");
    }

    /// A galley is shaped for a display scale: glyph advances are measured at
    /// `round(font_size * pixels_per_point)` physical pixels and row heights
    /// are rounded to whole pixels, so one run answers a different extent at
    /// 1.0 and at 2.0. The key must carry the scale, or dragging a window
    /// between two monitors of different scale paints the old monitor's text.
    #[test]
    fn a_scale_change_is_a_different_galley() {
        let h = Headless::new();
        let mut s = h.shaper();
        let r = req(
            "the quick brown fox jumps over the lazy dog",
            Some(120.0),
            TextWrap::Wrap,
        );
        let at_one = s.text(&r);

        h.set_scale(2.0);
        // A cold shaper is the ground truth at the new scale.
        let mut cold = h.shaper();
        let truth = cold.text(&r);
        assert_ne!(
            truth.size, at_one.size,
            "this run must actually shape differently at the two scales, or \
             the test cannot tell a stale galley from a fresh one"
        );

        let served = s.text(&r);
        assert_eq!(
            served.size, truth.size,
            "the warm cache served the 1.0 galley at scale 2.0"
        );
        assert!(
            std::sync::Arc::ptr_eq(&s.galley(&r), &cold.galley(&r)),
            "the warm cache must hand back the same galley a cold shaper \
             produces at this scale"
        );
    }

    /// The galley cache is keyed by the text, so a list of distinct row labels
    /// would grow it forever. SC-008's memory clause applies here exactly as
    /// it does to the measurement cache.
    #[test]
    fn shaping_many_distinct_runs_stays_inside_the_bound() {
        let h = Headless::new();
        let mut s = h.shaper();
        s.set_capacity(64);
        for row in 0..5_000 {
            let text = format!("fiber {row} — running");
            s.galley(&req(&text, Some(200.0), TextWrap::Wrap));
        }
        assert_eq!(s.capacity(), 64);
        assert!(
            s.evictions() >= 5_000 - 64,
            "evicted only {}, so the bound never bit",
            s.evictions()
        );
        // The most recent run is still resident: eviction follows use.
        let last = format!("fiber {} — running", 4_999);
        let before = s.stats();
        s.galley(&req(&last, Some(200.0), TextWrap::Wrap));
        assert_eq!(
            s.stats(),
            (before.0, before.1 + 1),
            "the newest run is a hit"
        );
    }

    #[test]
    fn distinct_requests_are_distinct_galleys() {
        let h = Headless::new();
        let mut s = h.shaper();
        s.galley(&req("a", Some(100.0), TextWrap::Wrap));
        s.galley(&req("a", Some(101.0), TextWrap::Wrap));
        s.galley(&req("a", Some(100.0), TextWrap::Clip));
        s.galley(&req("b", Some(100.0), TextWrap::Wrap));
        assert_eq!(s.stats(), (4, 0));
    }

    /// The map is keyed on the *theme's* names, so the ramp a component
    /// binds is the ramp that reaches the shaper.
    ///
    /// This is the regression guard for the defect that made every run on
    /// screen paint at 14 units: the map used to be keyed on `body` and
    /// `heading` while the vocabulary declared `typography.body` and
    /// `typography.heading`, so every lookup missed and every miss fell back
    /// to one size.
    #[test]
    fn the_default_map_is_keyed_on_the_shipped_vocabulary() {
        let t = Typography::default();
        for step in [
            "typography.body",
            "typography.heading-sm",
            "typography.heading",
            "typography.heading-lg",
        ] {
            assert!(
                t.style(step).is_some(),
                "{step} is declared by the shipped vocabulary and must be \
                 bound here; bound names are {:?}",
                t.names()
            );
        }
        assert_eq!(t.style("typography.body").unwrap().font.size, 14.0);
        assert_eq!(t.style("typography.heading").unwrap().font.size, 20.0);
        assert_eq!(t.style("typography.heading-lg").unwrap().font.size, 28.0);
        // The pre-vocabulary names are gone, not aliased: an alias would
        // have hidden the split instead of closing it.
        assert!(t.style("heading").is_none());
        assert!(t.style("body").is_none());
    }

    /// Typography is type, not colour, so both shipped themes carry one
    /// ramp — and `Typography::default` may therefore name either of them.
    #[test]
    fn both_shipped_themes_give_the_same_ramp() {
        let faces = FontFaces::default();
        let from_light = Typography::from_theme(&light(), &faces);
        let from_dark = Typography::from_theme(&dark(), &faces);
        assert_eq!(from_light.names(), from_dark.names());
        for name in from_light.names() {
            assert_eq!(
                from_light.style(name),
                from_dark.style(name),
                "{name} differs between the two shipped themes"
            );
        }
    }

    /// A run whose style token is not bound still shapes — blank text is
    /// worse than wrong-sized text — but the name comes back so the caller
    /// can report it. Returning only the style is what hid the defect above.
    #[test]
    fn an_unbound_style_resolves_to_the_body_style_and_names_itself() {
        let t = Typography::default();
        let (body, miss) = t.resolve(None);
        assert!(miss.is_none());
        assert_eq!(body, t.style(Typography::BODY).unwrap());

        let (fallback, miss) = t.resolve(Some("typography.no-such-step"));
        assert_eq!(
            miss,
            Some("typography.no-such-step"),
            "an unbound name must come back named, not swallowed"
        );
        assert_eq!(fallback, body, "and still shape at the body style");

        let (bound, miss) = t.resolve(Some("typography.heading-lg"));
        assert!(miss.is_none());
        assert_eq!(bound.font.size, 28.0);
    }

    /// `TypographyValue::line_height` reaches the galley. egui takes it on
    /// the text *format*, which is why `Typography` maps to a `TextStyle`
    /// and not to a bare `FontId` — a `FontId` has nowhere to put it, and
    /// that is why the field had no consumer at all before.
    #[test]
    fn line_height_reaches_the_shaped_rows() {
        let faces = FontFaces::default();
        let mut t = Typography::new(TextStyle::new(FontId::new(14.0, FontFamily::Proportional)));
        t.set(
            "typography.tight",
            TextStyle::from_token(
                &TypographyValue {
                    size: 14.0,
                    line_height: 16.0,
                    weight: TypographyWeight::Regular,
                },
                &faces,
            ),
        );
        t.set(
            "typography.airy",
            TextStyle::from_token(
                &TypographyValue {
                    size: 14.0,
                    line_height: 40.0,
                    weight: TypographyWeight::Regular,
                },
                &faces,
            ),
        );

        let h = Headless::new();
        let mut s = h.shaper_with(t);
        let text = "the quick brown fox jumps over the lazy dog";
        let tight = s.text(&TextRequest {
            style: Some("typography.tight"),
            ..req(text, Some(80.0), TextWrap::Wrap)
        });
        let airy = s.text(&TextRequest {
            style: Some("typography.airy"),
            ..req(text, Some(80.0), TextWrap::Wrap)
        });
        assert_eq!(
            tight.lines, airy.lines,
            "same size and width must wrap the same, or this compares two \
             different layouts instead of two line heights"
        );
        assert!(
            airy.size.h > tight.size.h * 2.0,
            "line height 40 must lay out far taller than line height 16 over \
             {} rows: {:?} vs {:?}",
            tight.lines,
            airy.size,
            tight.size
        );
    }

    /// Weight is spent choosing the egui family, because a `FontId` has no
    /// weight axis. The default font stack has one proportional face, so
    /// the default mapping cannot tell the classes apart and says so; a host
    /// that binds a second face gets a different family out of a bold token.
    #[test]
    fn weight_selects_the_bound_face_and_the_default_admits_it_cannot() {
        let plain = FontFaces::default();
        assert!(
            !plain.distinguishes_weight(),
            "egui's default fonts install one proportional face, so the \
             default mapping must not claim to render three weights"
        );

        // Monospace stands in for an installed bold face: it is the second
        // family egui's defaults do have, so the assertion is about the
        // mapping and not about a font this test would have to ship.
        let faces = FontFaces::new(
            FontFamily::Proportional,
            FontFamily::Proportional,
            FontFamily::Monospace,
        );
        assert!(!faces.distinguishes_weight(), "medium still shares regular");

        let heading = TextStyle::from_token(
            &TypographyValue {
                size: 20.0,
                line_height: 28.0,
                weight: TypographyWeight::Bold,
            },
            &faces,
        );
        assert_eq!(heading.font.family, FontFamily::Monospace);
        assert_eq!(heading.line_height, Some(28.0));

        let body = TextStyle::from_token(
            &TypographyValue {
                size: 14.0,
                line_height: 20.0,
                weight: TypographyWeight::Regular,
            },
            &faces,
        );
        assert_eq!(body.font.family, FontFamily::Proportional);

        // And the whole ramp built through those faces carries it.
        let t = Typography::from_theme(&dark(), &faces);
        assert_eq!(
            t.style("typography.heading").unwrap().font.family,
            FontFamily::Monospace
        );
        assert_eq!(
            t.style("typography.body").unwrap().font.family,
            FontFamily::Proportional
        );
    }

    /// The ramp has to reach the glyphs, not just the map: a heading run
    /// must measure taller than a body run through the real shaper.
    #[test]
    fn a_heading_shapes_larger_than_body_text() {
        let h = Headless::new();
        let mut s = h.shaper();
        let big = s.text(&TextRequest {
            style: Some("typography.heading"),
            ..req("Ag", None, TextWrap::Wrap)
        });
        let small = s.text(&TextRequest {
            style: Some("typography.body"),
            ..req("Ag", None, TextWrap::Wrap)
        });
        assert!(
            big.size.h > small.size.h && big.size.w > small.size.w,
            "typography.heading (20/28) must shape larger than \
             typography.body (14/20): {big:?} vs {small:?}"
        );
    }

    /// Mixed script must shape without panicking. This is not a claim that the
    /// glyphs are correct — the missing-glyph assertion is task T066's, against
    /// the real font stack — only that the boundary handles non-ASCII.
    #[test]
    fn mixed_script_shapes_without_panicking() {
        let h = Headless::new();
        let mut s = h.shaper();
        for text in [
            "Latin",
            "日本語のテキスト",
            "نص عربي",
            "emoji 🙂 tail",
            "混合 mixed نص",
        ] {
            let m = s.text(&req(text, Some(120.0), TextWrap::Wrap));
            assert!(m.size.h > 0.0, "{text:?} measured to nothing");
            assert!(m.lines >= 1);
        }
    }

    /// An unsupported source takes no space and says so. A guessed size would
    /// reserve room for a picture that never arrives.
    #[test]
    fn unsupported_content_is_recorded_not_invented() {
        let h = Headless::new();
        let mut s = h.shaper();
        assert_eq!(
            s.image("icons/fiber.png", SizeProposal::unspecified()),
            gorgon_petra::geom::Size::ZERO
        );
        assert_eq!(
            s.custom("gauge", SizeProposal::unspecified()),
            gorgon_petra::geom::Size::ZERO
        );
        assert_eq!(s.unsupported_images(), ["icons/fiber.png"]);
        assert_eq!(s.unsupported_customs(), ["gauge"]);
    }
}
