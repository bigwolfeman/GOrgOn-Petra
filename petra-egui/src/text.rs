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

use egui::text::LayoutJob;
use egui::{Color32, Context, FontFamily, FontId, Galley};
use gorgon_petra::cache::LruCache;
use gorgon_petra::geom::Size;
use gorgon_petra::layout::{ContentMeasure, SizeProposal, TextMeasurement, TextRequest};
use gorgon_petra::tree::TextWrap;

/// The character appended where a run was elided.
pub const ELLIPSIS: char = '…';

/// Maps Petra typography token names to egui fonts.
///
/// Token names arrive from the view tree (`props.style`); the values come from
/// the theme. Until the token vocabulary lands, this holds the mapping directly
/// so the boundary is complete and testable on its own.
#[derive(Clone, Debug)]
pub struct Typography {
    default: FontId,
    styles: BTreeMap<String, FontId>,
}

impl Default for Typography {
    fn default() -> Self {
        let mut styles = BTreeMap::new();
        styles.insert("body".into(), FontId::new(14.0, FontFamily::Proportional));
        styles.insert(
            "heading".into(),
            FontId::new(20.0, FontFamily::Proportional),
        );
        styles.insert("small".into(), FontId::new(11.0, FontFamily::Proportional));
        styles.insert("mono".into(), FontId::new(13.0, FontFamily::Monospace));
        Self {
            default: FontId::new(14.0, FontFamily::Proportional),
            styles,
        }
    }
}

impl Typography {
    /// An empty map whose every lookup falls back to `default`.
    #[must_use]
    pub fn new(default: FontId) -> Self {
        Self {
            default,
            styles: BTreeMap::new(),
        }
    }

    /// Bind a token name to a font.
    pub fn set(&mut self, name: impl Into<String>, font: FontId) {
        self.styles.insert(name.into(), font);
    }

    /// The font for a token name.
    ///
    /// An unknown name resolves to the default rather than failing: a missing
    /// typography token is caught by the theme-completeness check
    /// (`data-model.md` §5), and failing again here would turn one reported
    /// problem into a crash at paint time.
    #[must_use]
    pub fn font(&self, style: Option<&str>) -> FontId {
        style
            .and_then(|name| self.styles.get(name))
            .cloned()
            .unwrap_or_else(|| self.default.clone())
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
    font: String,
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

    /// The typography map, for the theme to rebind.
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
        let font = self.typography.font(req.style);
        let (max_rows, break_anywhere, overflow) = wrapping(req);
        let wrap_width = req.available_width.unwrap_or(f32::INFINITY);
        let key = GalleyKey {
            text: req.text.to_owned(),
            font: format!("{}:{:?}", font.size.to_bits(), font.family),
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
        let mut job =
            LayoutJob::simple(req.text.to_owned(), font, Color32::PLACEHOLDER, wrap_width);
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
    use super::{ELLIPSIS, GalleyShaper, Typography};
    use egui::{Context, FontFamily, FontId, RawInput};
    use gorgon_petra::layout::{ContentMeasure, SizeProposal, TextRequest};
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

    #[test]
    fn typography_falls_back_rather_than_failing() {
        let mut t = Typography::default();
        t.set("code", FontId::new(9.0, FontFamily::Monospace));
        assert_eq!(t.font(Some("code")).size, 9.0);
        assert_eq!(t.font(Some("no-such-token")), t.font(None));
        assert!(t.names().contains(&"code"));

        let h = Headless::new();
        let mut s = h.shaper_with(t);
        let big = s.text(&TextRequest {
            style: Some("heading"),
            ..req("Ag", None, TextWrap::Wrap)
        });
        let small = s.text(&TextRequest {
            style: Some("small"),
            ..req("Ag", None, TextWrap::Wrap)
        });
        assert!(big.size.h > small.size.h, "{big:?} vs {small:?}");
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
