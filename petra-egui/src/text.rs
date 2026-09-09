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
use gorgon_petra::token::{Theme, TokenValue, TypographyFamily, TypographyValue, TypographyWeight};
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
    mono: FontFamily,
}

impl Default for FontFaces {
    fn default() -> Self {
        Self {
            regular: FontFamily::Proportional,
            medium: FontFamily::Proportional,
            bold: FontFamily::Proportional,
            mono: FontFamily::Monospace,
        }
    }
}

impl FontFaces {
    /// One family per weight class, with the default monospace face.
    ///
    /// The signature is unchanged from before the mono face existed, and
    /// deliberately: every caller installs a proportional stack and wants
    /// egui's own `FontFamily::Monospace` for code, so making them all pass a
    /// fourth argument would be churn with one possible answer. A host with a
    /// mono face of its own binds it through [`FontFaces::with_mono`].
    #[must_use]
    pub fn new(regular: FontFamily, medium: FontFamily, bold: FontFamily) -> Self {
        Self {
            regular,
            medium,
            bold,
            mono: FontFamily::Monospace,
        }
    }

    /// The same faces, drawing [`TypographyFamily::Mono`] through `family`.
    #[must_use]
    pub fn with_mono(mut self, family: FontFamily) -> Self {
        self.mono = family;
        self
    }

    /// The family a token draws through.
    ///
    /// Family wins over weight, and there is only one sensible order: a
    /// monospace face has its own weights and a `Mono`/`Bold` token asking for
    /// the proportional bold face would paint code in a proportional font,
    /// which is the one thing the family field exists to prevent. The default
    /// stack installs a single mono face, so `Mono` ignores weight entirely
    /// there — the same honest limit [`FontFaces::distinguishes_weight`]
    /// already reports for the proportional side.
    #[must_use]
    pub fn family(&self, family: TypographyFamily, weight: TypographyWeight) -> &FontFamily {
        match family {
            TypographyFamily::Mono => &self.mono,
            TypographyFamily::Sans => match weight {
                TypographyWeight::Regular => &self.regular,
                TypographyWeight::Medium => &self.medium,
                TypographyWeight::Bold => &self.bold,
            },
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
    /// Extra tracking between glyphs, in logical units, from the token's
    /// `letter_spacing`. Reaches the galley as epaint's
    /// `TextFormat::extra_letter_spacing`.
    pub extra_letter_spacing: f32,
}

impl TextStyle {
    /// A style at `font`, using the font's own line height.
    #[must_use]
    pub fn new(font: FontId) -> Self {
        Self {
            font,
            line_height: None,
            extra_letter_spacing: 0.0,
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
            font: FontId::new(value.size, faces.family(value.family, value.weight).clone()),
            line_height: Some(value.line_height),
            extra_letter_spacing: value.letter_spacing,
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
                faces
                    .family(TypographyFamily::Sans, TypographyWeight::Regular)
                    .clone(),
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
    /// Colour runs over the text, as `(byte length, colour bits)` with
    /// `None` for a run that takes the painter's fallback ink.
    ///
    /// Empty for every uncoloured run, which is almost all of them, so a
    /// measurement and an ordinary paint of the same string still land on
    /// exactly the key they landed on before this field existed. A coloured
    /// run gets its own entry: two inks over one string are two pictures and
    /// must not share a galley.
    runs: Vec<(usize, Option<u32>)>,
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
        self.galley_runs(req, &[])
    }

    /// [`GalleyShaper::galley`] with the text split into coloured runs.
    ///
    /// `runs` is `(byte length, ink)` pairs tiling `req.text`; `None` ink
    /// means the section keeps [`Color32::PLACEHOLDER`], which is what
    /// `egui::Painter::galley`'s fallback colour fills in — so an uncoloured
    /// stretch takes the node's own `foreground` with no special case here.
    /// An empty slice is the ordinary single-section path.
    ///
    /// **Colour is not a shaping input.** The sections a colour run creates
    /// change no advance, no break and no row height, so the galley this
    /// returns has the same geometry as the one [`GalleyShaper::galley`]
    /// returns for the same request — which is what lets the painter colour a
    /// run that was measured without colour and still paint what was
    /// measured. `a_coloured_galley_has_the_same_geometry_as_a_plain_one`
    /// holds that rather than assuming it.
    ///
    /// A `runs` slice that does not tile `req.text` is refused by tree
    /// acceptance long before here (`Violation::TextRunsDoNotCoverTheText`);
    /// this function clamps rather than panics, because a shaper is not the
    /// place to discover a malformed tree.
    pub fn galley_runs(
        &mut self,
        req: &TextRequest<'_>,
        runs: &[(usize, Option<Color32>)],
    ) -> Arc<Galley> {
        // Cloned rather than borrowed: the shaping below needs `&mut self`
        // for the cache and the counters, and a `TextStyle` is a size, a
        // family handle and an optional float.
        let style = self.typography.resolve(req.style).0.clone();
        let (max_rows, break_anywhere, overflow) = wrapping(req);
        let wrap_width = req.available_width.unwrap_or(f32::INFINITY);
        let key = GalleyKey {
            text: req.text.to_owned(),
            // Tracking is in the key because it changes the shaped run's
            // width: two styles differing only in `letter_spacing` measure
            // differently, and sharing a galley between them would hand the
            // painter a run that was measured for the other one. That is the
            // exact failure this cache's "measure and paint the same galley"
            // rule exists to prevent.
            style: format!(
                "{}:{:?}:{:?}:{}",
                style.font.size.to_bits(),
                style.line_height.map(f32::to_bits),
                style.font.family,
                style.extra_letter_spacing.to_bits()
            ),
            wrap_bits: wrap_width.to_bits(),
            scale_bits: self.ctx.pixels_per_point().to_bits(),
            max_rows,
            break_anywhere,
            overflow,
            runs: runs
                .iter()
                .map(|(len, color)| (*len, color.map(|c| u32::from_le_bytes(c.to_array()))))
                .collect(),
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
        let format = |color: Color32| TextFormat {
            font_id: style.font.clone(),
            color,
            line_height: style.line_height,
            extra_letter_spacing: style.extra_letter_spacing,
            ..TextFormat::default()
        };
        let mut job = if runs.is_empty() {
            LayoutJob::single_section(req.text.to_owned(), format(Color32::PLACEHOLDER))
        } else {
            let mut job = LayoutJob {
                text: req.text.to_owned(),
                ..LayoutJob::default()
            };
            let mut at = 0usize;
            for (len, color) in runs {
                // Clamped, not trusted: acceptance guarantees the tiling for
                // an accepted tree, and a shaper handed a bad one draws a
                // short line rather than panicking inside a paint pass.
                let end = at.saturating_add(*len).min(req.text.len());
                if end > at {
                    job.sections.push(egui::text::LayoutSection {
                        leading_space: 0.0,
                        byte_range: egui::text::ByteIndex(at)..egui::text::ByteIndex(end),
                        format: format(color.unwrap_or(Color32::PLACEHOLDER)),
                    });
                }
                at = end;
            }
            if at < req.text.len() {
                job.sections.push(egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: egui::text::ByteIndex(at)..egui::text::ByteIndex(req.text.len()),
                    format: format(Color32::PLACEHOLDER),
                });
            }
            job
        };
        job.wrap.max_width = wrap_width;
        job.wrap.max_rows = max_rows;
        job.wrap.break_anywhere = break_anywhere;
        job.wrap.overflow_character = overflow;
        let galley = self.ctx.fonts_mut(|fonts| fonts.layout_job(job));
        self.galleys.insert(key, Arc::clone(&galley));
        galley
    }
}

/// The byte offset in `galley`'s own text nearest the local position `at`.
///
/// # This is the seam a text selection needs and nothing else provides
///
/// `contracts/view-tree.md`'s U-09 boundary puts shaping in the host and
/// keeps every egui type out of `gorgon-petra`, so [`ContentMeasure`] answers
/// only *how big* a run is. Nothing in the engine can turn a window position
/// into a position in a string: that mapping is per-glyph advances and line
/// breaks, which only the shaped run holds. So the mapping lives here, beside
/// the shaping, and the host is the one caller — a selection is host-derived
/// interaction state ([`gorgon_petra::input::TextSelection`]) for exactly the
/// same reason hover is.
///
/// `at` is relative to the galley's own origin, which for a Petra text node
/// is its placement's top-left ([`crate::paint`] paints at `rect.min`). A
/// position above the run answers 0 and one below it answers the end, which
/// is egui's own rule and is what lets a drag run off the top or bottom of a
/// code block and keep selecting.
///
/// **Byte offset, not char index.** egui counts characters and `str` slices
/// by bytes, and the two are the same number only until the first
/// multi-byte character. Converting here rather than at each caller is what
/// stops that difference reaching a `&text[..]` that panics.
#[must_use]
pub fn byte_offset_at(galley: &Galley, at: egui::Vec2) -> usize {
    char_to_byte(galley.text(), galley.cursor_from_pos(at).index.0)
}

/// The rectangles the byte range `range` covers in `galley`, one per row it
/// crosses, in coordinates local to the galley's origin.
///
/// Empty when the range is empty or falls outside the text. Rows the range
/// touches but covers no glyph of — the blank line between two paragraphs —
/// contribute nothing, because a zero-width fill is not a picture.
///
/// The row's own `pos` is added back on both ends: `Row::x_offset` is
/// measured from the row's start and `PlacedRow::pos` is where that start
/// sits in the galley, which is the same pair `Galley::cursor_from_pos` puts
/// together in the other direction.
#[must_use]
pub fn selection_rects(galley: &Galley, range: &std::ops::Range<usize>) -> Vec<egui::Rect> {
    if range.is_empty() {
        return Vec::new();
    }
    let text = galley.text();
    let (from, to) = (
        byte_to_char(text, range.start),
        byte_to_char(text, range.end),
    );
    let mut out = Vec::new();
    let mut row_start = 0usize;
    for row in &galley.rows {
        // The glyph count, which is what `x_offset` indexes. The newline is
        // counted for the *next* row's start but is not a glyph on this one.
        let glyphs = row.char_count_excluding_newline().0;
        let row_end = row_start + glyphs;
        let start = from.max(row_start);
        let end = to.min(row_end);
        if start < end {
            let x0 = row.pos.x + row.x_offset(egui::text::CharIndex(start - row_start));
            let x1 = row.pos.x + row.x_offset(egui::text::CharIndex(end - row_start));
            if x1 > x0 {
                out.push(egui::Rect::from_min_max(
                    egui::pos2(x0, row.min_y()),
                    egui::pos2(x1, row.max_y()),
                ));
            }
        }
        row_start += row.char_count_including_newline().0;
    }
    out
}

/// `runs` with the stretch `range` forced to `ink`, tiling the same string.
///
/// # Why the selected run takes its own ink at all
///
/// A highlight is a fill behind glyphs, and a fill dark enough to see moves
/// the ground those glyphs were measured against. Measured on the shipped
/// themes: the code snippet's keyword ink (`link-primary`) clears AA on
/// `surface.raised` in the light theme at 4.63:1, with 4.5 the floor — so
/// **any** selection ground darker than the well drops it below AA there.
/// Solving it by choosing a paler ground means choosing one nobody can see.
///
/// So the selected stretch takes the selection's own ink, which is what
/// `::selection` does in a browser and what most editors do: the highlight is
/// a ground *and* an ink, checked as a pair, and the syntax colouring inside
/// the selection is deliberately suspended for as long as it is selected.
///
/// Splitting the run list rather than re-measuring: colour is not a shaping
/// input (`GalleyShaper::galley_runs`), so the split galley has the geometry
/// the unsplit one had, which is what lets the highlight rectangles computed
/// from one be painted under the other.
#[must_use]
pub fn runs_with_selection(
    runs: &[(usize, Option<Color32>)],
    text_len: usize,
    range: &std::ops::Range<usize>,
    ink: Color32,
) -> Vec<(usize, Option<Color32>)> {
    // A node with no runs of its own still needs one, or there is nothing to
    // split and the whole string keeps the node's `foreground`.
    let base: Vec<(usize, Option<Color32>)> = if runs.is_empty() {
        vec![(text_len, None)]
    } else {
        runs.to_vec()
    };
    let mut out: Vec<(usize, Option<Color32>)> = Vec::with_capacity(base.len() + 2);
    let mut at = 0usize;
    for (len, colour) in base {
        let (start, end) = (at, at + len);
        at = end;
        // Three pieces, any of which may be empty: before the selection,
        // inside it, after it.
        for (from, to, colour) in [
            (start, end.min(range.start), colour),
            (start.max(range.start), end.min(range.end), Some(ink)),
            (start.max(range.end), end, colour),
        ] {
            if to > from {
                match out.last_mut() {
                    Some((last_len, last_ink)) if *last_ink == colour => *last_len += to - from,
                    _ => out.push((to - from, colour)),
                }
            }
        }
    }
    out
}

/// The byte offset of character `index`, or the string's length past its end.
fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(byte, _)| byte)
}

/// The character index of byte offset `at`, clamped to the string.
fn byte_to_char(text: &str, at: usize) -> usize {
    text[..at.min(text.len())].chars().count()
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

/// A host-registered measurer for one `Props.custom_kind` name.
///
/// Answers the same [`SizeProposal`] [`ContentMeasure::custom`] receives, so
/// a measurer can honour `Zero`, `Unbounded`, `Unspecified` and `Exact`
/// exactly as any other leaf does (`contracts/view-tree.md`) — a grid of
/// `cols` cells at `cell_width` each, clamped to whatever the offer allows,
/// say. This is the measurement mirror of
/// [`crate::paint::CustomPainterFn`]: that closure is handed the placement
/// this crate already resolved and paints *into* it; this one is handed the
/// offer and answers *with* the extent the painter will get.
pub type CustomMeasurerFn = dyn Fn(SizeProposal) -> Size;

/// Host-registered measurers, keyed by the name `Props.custom_kind` carries.
///
/// The measurement mirror of [`crate::paint::CustomPainters`]: empty by
/// default, so a tree with no host registration measures exactly what this
/// crate shipped before this type existed — every custom name lands in
/// [`GalleyShaper::unsupported_customs`] at [`Size::ZERO`]. Registration is
/// opt-in per name, and last registration wins, the same rule
/// [`crate::paint::CustomPainters::register`] and
/// [`crate::image::ImageSources::register`] both use.
#[derive(Default)]
pub struct CustomMeasurers {
    measurers: BTreeMap<String, Box<CustomMeasurerFn>>,
}

impl CustomMeasurers {
    /// An empty registry: every custom name falls back to the shaper's own
    /// answer, `Size::ZERO` recorded in `unsupported_customs`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `measurer` under `name`. Registering the same name twice
    /// replaces the previous measurer rather than keeping both.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        measurer: impl Fn(SizeProposal) -> Size + 'static,
    ) {
        self.measurers.insert(name.into(), Box::new(measurer));
    }

    /// The measurer registered for `name`, or `None` when nothing is.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&CustomMeasurerFn> {
        self.measurers.get(name).map(|measurer| measurer.as_ref())
    }
}

/// The [`ContentMeasure`] a running host measures through: text and images
/// go to `shaper`, and a `custom` name goes to `measurers` first, falling
/// back to `shaper` — which is where the pre-registry answer lives and stays
/// (`GalleyShaper::custom`'s `Size::ZERO`-and-record behaviour) — when
/// nothing is registered for it.
///
/// [`crate::paint`] keeps its shaper and its painter registry as two
/// arguments to one function, because [`crate::paint::PaintContent`] already
/// carries per-placement dispatch and a painter is reached only through
/// `paint_frame_with_hosts`. Layout instead asks its whole measurement
/// boundary through one [`ContentMeasure`] object, so [`Host::pass`] needs
/// the shaper and the registry combined into one — this type is that
/// combination, built fresh for each `petrify` call so it borrows nothing
/// longer than the pass that uses it.
///
/// [`Host::pass`]: crate::host::Host::pass
pub struct HostedContent<'a> {
    shaper: &'a mut GalleyShaper,
    measurers: &'a CustomMeasurers,
}

impl<'a> HostedContent<'a> {
    /// Measure through `shaper`, consulting `measurers` for `custom` names
    /// first.
    #[must_use]
    pub fn new(shaper: &'a mut GalleyShaper, measurers: &'a CustomMeasurers) -> Self {
        Self { shaper, measurers }
    }
}

impl ContentMeasure for HostedContent<'_> {
    fn text(&mut self, req: &TextRequest<'_>) -> TextMeasurement {
        self.shaper.text(req)
    }

    fn image(&mut self, source: &str, proposal: SizeProposal) -> Size {
        self.shaper.image(source, proposal)
    }

    fn custom(&mut self, name: &str, proposal: SizeProposal) -> Size {
        match self.measurers.get(name) {
            // A registered measurer answers instead of the shaper's
            // fallback — and does *not* touch `unsupported_customs`: that
            // set means "asked for and nothing answered", which is no
            // longer true for this name.
            Some(measurer) => measurer(proposal),
            None => self.shaper.custom(name, proposal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CustomMeasurers, ELLIPSIS, FontFaces, GalleyShaper, HostedContent, TextStyle, Typography,
    };
    use egui::{Color32, Context, FontFamily, FontId, Galley, RawInput};
    use gorgon_petra::layout::{ContentMeasure, SizeProposal, TextRequest};
    use gorgon_petra::token::{TypographyFamily, TypographyValue, TypographyWeight, dark, light};
    use gorgon_petra::tree::TextWrap;
    use std::sync::Arc;

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

    /// The hit test answers a byte offset, and answers it in bytes.
    ///
    /// egui counts characters and `str` slices by bytes, and the two are the
    /// same number only until the first multi-byte character. The emoji here
    /// is not decoration: with the conversion missing, a click past it hands
    /// back a char index that either slices inside a code point (a panic) or
    /// names the wrong glyph, and every ASCII test in this file would still
    /// pass.
    #[test]
    fn the_hit_test_answers_a_byte_offset_and_not_a_char_index() {
        let h = Headless::new();
        let mut shaper = h.shaper();
        let text = "aé😀bc";
        let galley = shaper.galley(&req(text, None, TextWrap::Clip));

        // Left of everything is the start; far right of everything is the end.
        assert_eq!(
            super::byte_offset_at(&galley, egui::vec2(-100.0, 8.0)),
            0,
            "a position left of the run is not the start of it"
        );
        let end = super::byte_offset_at(&galley, egui::vec2(10_000.0, 8.0));
        assert_eq!(end, text.len(), "a position past the run is not its end");

        // Every answer is a real char boundary, which is the whole point.
        let width = galley.rect.width();
        for step in 0..=20 {
            #[allow(clippy::cast_precision_loss)]
            let x = width * step as f32 / 20.0;
            let at = super::byte_offset_at(&galley, egui::vec2(x, 8.0));
            assert!(
                text.is_char_boundary(at),
                "offset {at} at x={x} is inside a code point of {text:?}"
            );
        }

        // And it is monotonic left to right, which a char index read as a
        // byte offset stops being at the first wide character.
        let mut last = 0;
        for step in 0..=20 {
            #[allow(clippy::cast_precision_loss)]
            let x = width * step as f32 / 20.0;
            let at = super::byte_offset_at(&galley, egui::vec2(x, 8.0));
            assert!(at >= last, "the offset went backwards: {last} then {at}");
            last = at;
        }
    }

    /// A range crossing a line break is one rectangle per row, not one box.
    ///
    /// One box from the first character to the last would cover the whole
    /// right margin of the first row and the whole left margin of the second,
    /// which is a highlight over text nobody selected.
    #[test]
    fn a_selection_across_rows_is_one_rectangle_per_row() {
        let h = Headless::new();
        let mut shaper = h.shaper();
        let text = "alpha\nbeta\ngamma";
        let galley = shaper.galley(&req(text, None, TextWrap::Wrap));

        // "pha\nbeta\nga" — part of the first row, all of the second, part of
        // the third.
        let rects = super::selection_rects(&galley, &(2..12));
        assert_eq!(rects.len(), 3, "expected one band per row, got {rects:?}");
        assert!(
            rects[0].min.x > rects[1].min.x,
            "the first row's band must start where the selection did, not at \
             the left margin: {rects:?}"
        );
        assert!(
            rects[2].max.x < rects[1].max.x,
            "the last row's band must stop where the selection did: {rects:?}"
        );
        // Each band sits on its own row, top to bottom.
        assert!(rects[0].max.y <= rects[1].min.y + 0.01);
        assert!(rects[1].max.y <= rects[2].min.y + 0.01);

        assert!(
            super::selection_rects(&galley, &(4..4)).is_empty(),
            "an empty range is not a picture"
        );
    }

    /// Splitting the run list forces the selected stretch's ink and tiles the
    /// same string.
    ///
    /// The tiling is what tree acceptance guarantees for an authored run list
    /// and what nothing guarantees for one this function builds, so it is
    /// checked here: a split that lost or duplicated a byte would shape a
    /// different string from the one that was measured.
    #[test]
    fn splitting_a_run_list_for_a_selection_keeps_it_tiling() {
        let ink = Color32::from_rgb(1, 2, 3);
        let blue = Color32::from_rgb(0, 0, 255);
        let runs = vec![(4, Some(blue)), (6, None)];

        let split = super::runs_with_selection(&runs, 10, &(2..7), ink);
        assert_eq!(split.iter().map(|(len, _)| *len).sum::<usize>(), 10);
        assert_eq!(
            split,
            vec![(2, Some(blue)), (5, Some(ink)), (3, None)],
            "the selected stretch takes the selection's ink and the rest keeps its own"
        );

        // A node with no runs of its own still gets one to split, or the
        // whole string would keep the node's `foreground` and the highlight
        // would sit under unchanged ink.
        assert_eq!(
            super::runs_with_selection(&[], 10, &(2..7), ink),
            vec![(2, None), (5, Some(ink)), (3, None)]
        );

        // Neighbours with the same ink merge, so a selection covering two
        // runs of one colour does not lengthen the list for nothing.
        let merged = super::runs_with_selection(&[(4, None), (6, None)], 10, &(0..10), ink);
        assert_eq!(merged, vec![(10, Some(ink))]);
    }

    /// The claim the whole colour-run design rests on: **colour is not a
    /// shaping input.**
    ///
    /// `Props::runs` reaches the frame without touching
    /// `ContentMeasure::text`, so a coloured line is measured uncoloured and
    /// painted coloured. That is only sound if the two galleys have the same
    /// geometry. Asserted glyph by glyph rather than on the bounding box: a
    /// box can match while a break moved inside it.
    ///
    /// Wrapped, not a single line, because a wrap is where a stray section
    /// boundary would show up first.
    #[test]
    fn a_coloured_galley_has_the_same_geometry_as_a_plain_one() {
        let h = Headless::new();
        let mut s = h.shaper();
        let text = "let fibers = kernel.spawn(count);";
        let request = req(text, Some(90.0), TextWrap::Wrap);

        let plain = s.galley(&request);
        let coloured = s.galley_runs(
            &request,
            &[
                (3, Some(Color32::from_rgb(0x78, 0xa9, 0xff))),
                (8, None),
                (1, Some(Color32::from_rgb(0xff, 0x83, 0x89))),
                (text.len() - 12, None),
            ],
        );

        assert!(
            !Arc::ptr_eq(&plain, &coloured),
            "the two share a galley, so this test is comparing one object \
             with itself and would pass on any implementation"
        );
        assert_eq!(plain.rect, coloured.rect, "the runs moved the box");
        assert_eq!(
            plain.rows.len(),
            coloured.rows.len(),
            "the runs moved a line break"
        );
        for (i, (a, b)) in plain.rows.iter().zip(&coloured.rows).enumerate() {
            let pos = |row: &_| {
                let row: &egui::epaint::text::PlacedRow = row;
                row.row
                    .glyphs
                    .iter()
                    .map(|g| (g.pos.x.to_bits(), g.pos.y.to_bits(), g.chr))
                    .collect::<Vec<_>>()
            };
            assert_eq!(pos(a), pos(b), "row {i}'s glyphs moved");
        }
    }

    /// Two inks over one string are two galleys, and the cache must not
    /// hand the second caller the first one's.
    ///
    /// The trap this exists for: `GalleyKey` was keyed on the text, the
    /// style, the wrap and the scale, and a colour run changes none of
    /// those. Without `runs` in the key, colouring a snippet would paint the
    /// previously cached uncoloured galley and every test asserting on the
    /// tree would still pass.
    #[test]
    fn two_inks_over_one_string_are_two_galleys() {
        let h = Headless::new();
        let mut s = h.shaper();
        let request = req("keyword rest", Some(400.0), TextWrap::Wrap);
        let blue = Color32::from_rgb(0x78, 0xa9, 0xff);
        let red = Color32::from_rgb(0xff, 0x83, 0x89);

        let plain = s.galley(&request);
        let a = s.galley_runs(&request, &[(7, Some(blue)), (5, None)]);
        let b = s.galley_runs(&request, &[(7, Some(red)), (5, None)]);
        let again = s.galley_runs(&request, &[(7, Some(blue)), (5, None)]);

        let ink = |g: &Arc<Galley>| {
            g.job
                .sections
                .iter()
                .map(|sec| sec.format.color)
                .collect::<Vec<_>>()
        };
        assert_eq!(ink(&plain), vec![Color32::PLACEHOLDER]);
        assert_eq!(ink(&a), vec![blue, Color32::PLACEHOLDER]);
        assert_eq!(ink(&b), vec![red, Color32::PLACEHOLDER]);
        assert!(
            Arc::ptr_eq(&a, &again),
            "the same runs re-shaped instead of hitting the cache"
        );
        assert!(!Arc::ptr_eq(&a, &b), "two inks shared one galley");
        assert!(
            !Arc::ptr_eq(&plain, &a),
            "coloured text reused the plain galley"
        );
    }

    /// An uncoloured stretch keeps `Color32::PLACEHOLDER`, which is what
    /// `egui::Painter::galley`'s fallback fills in — so a run naming no token
    /// takes the node's own `foreground` with no special case in the painter.
    #[test]
    fn an_uncoloured_run_keeps_the_placeholder_the_painter_fills_in() {
        let h = Headless::new();
        let mut s = h.shaper();
        let g = s.galley_runs(
            &req("abcdef", None, TextWrap::Wrap),
            &[(3, None), (3, None)],
        );
        assert_eq!(
            g.job
                .sections
                .iter()
                .map(|sec| sec.format.color)
                .collect::<Vec<_>>(),
            vec![Color32::PLACEHOLDER, Color32::PLACEHOLDER]
        );
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
        // **The run is chosen, not arbitrary.** Whether a given string
        // measures differently at two scales depends on where its accumulated
        // glyph advances happen to land relative to a whole pixel, so most
        // strings answer the same extent at 1.0 and 2.0 and would make the
        // premise below unprovable. This pangram does not: it measures
        // 110.1875 at scale 1.0 and 109.6875 at 2.0 under the shipped body
        // style. If a future ramp change makes the premise assertion fail,
        // that is what happened — pick another run, do not delete the
        // assertion, because without it this test passes whether or not the
        // key carries the scale.
        let r = req(
            "Sphinx of black quartz, judge my vow",
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
                    letter_spacing: 0.0,
                    family: TypographyFamily::Sans,
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
                    letter_spacing: 0.0,
                    family: TypographyFamily::Sans,
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

    /// `TypographyValue::letter_spacing` reaches the shaped run, and it is
    /// measured on the galley rather than read back off the style.
    ///
    /// **The field would be theatre without this.** A `letter_spacing` that
    /// round-trips through `TextStyle` and never reaches `TextFormat` is a
    /// token an author can set, a gate can check, and a reader can never see —
    /// which is exactly the shape of the defect this file's own `Typography`
    /// doc comment records, where an entire declared type ramp painted at 14
    /// units with every test green. So this measures a width difference on a
    /// laid-out run, at a tracking value taken from the shipped ramp.
    ///
    /// One line, no wrapping: tracking widens a run, and a wrapped run would
    /// answer the wrap width instead of the run's own extent.
    #[test]
    fn letter_spacing_reaches_the_shaped_run() {
        let faces = FontFaces::default();
        let mut t = Typography::new(TextStyle::new(FontId::new(14.0, FontFamily::Proportional)));
        for (step, tracking) in [("typography.tight", 0.0), ("typography.tracked", 0.32)] {
            t.set(
                step,
                TextStyle::from_token(
                    &TypographyValue {
                        size: 14.0,
                        line_height: 18.0,
                        weight: TypographyWeight::Regular,
                        letter_spacing: tracking,
                        family: TypographyFamily::Sans,
                    },
                    &faces,
                ),
            );
        }

        let h = Headless::new();
        let mut s = h.shaper_with(t);
        let text = "tracking";
        let tight = s.text(&TextRequest {
            style: Some("typography.tight"),
            ..req(text, None, TextWrap::Clip)
        });
        let tracked = s.text(&TextRequest {
            style: Some("typography.tracked"),
            ..req(text, None, TextWrap::Clip)
        });

        assert!(
            tracked.size.w > tight.size.w,
            "0.32 units of tracking over {} characters must widen the run: \
             {:?} against {:?}",
            text.chars().count(),
            tracked.size,
            tight.size
        );
        assert_eq!(
            tracked.size.h, tight.size.h,
            "tracking is horizontal; a height change means the line height \
             moved too and this measured two things"
        );
    }

    /// Two styles differing only in tracking are two galleys.
    ///
    /// The cache key is a string built by hand, so a field added to
    /// `TextStyle` and forgotten there is a silent bug of the worst kind: the
    /// painter is handed a galley measured for a *different* style, which
    /// breaks this module's one structural promise — that the run painted is
    /// byte-for-byte the run measured.
    #[test]
    fn tracking_is_part_of_the_galley_key() {
        let faces = FontFaces::default();
        let mut t = Typography::new(TextStyle::new(FontId::new(14.0, FontFamily::Proportional)));
        for (step, tracking) in [("typography.a", 0.0), ("typography.b", 0.6)] {
            t.set(
                step,
                TextStyle::from_token(
                    &TypographyValue {
                        size: 14.0,
                        line_height: 18.0,
                        weight: TypographyWeight::Regular,
                        letter_spacing: tracking,
                        family: TypographyFamily::Sans,
                    },
                    &faces,
                ),
            );
        }
        let h = Headless::new();
        let mut s = h.shaper_with(t);
        let a = s.galley(&TextRequest {
            style: Some("typography.a"),
            ..req("same text", None, TextWrap::Clip)
        });
        let b = s.galley(&TextRequest {
            style: Some("typography.b"),
            ..req("same text", None, TextWrap::Clip)
        });
        assert!(
            !std::sync::Arc::ptr_eq(&a, &b),
            "the same text at two tracking values was served one galley; the \
             painter would draw a run measured for the other style"
        );
    }

    /// The shipped ramp's `code` step draws through the monospace face, and
    /// nothing else does.
    ///
    /// The family field exists for exactly two of Carbon's fourteen
    /// productive roles, so the test that matters is the *negative* half: a
    /// `family` that silently applied to every step would be invisible in a
    /// screenshot of a code snippet and obvious in one of a heading.
    #[test]
    fn only_the_code_step_draws_through_the_mono_face() {
        let t = Typography::from_theme(&dark(), &FontFaces::default());
        assert_eq!(
            t.style("typography.code").unwrap().font.family,
            FontFamily::Monospace,
            "a code step that paints proportional is a code step in name only"
        );
        for proportional in [
            "typography.caption",
            "typography.label",
            "typography.body",
            "typography.body-compact",
            "typography.heading",
            "typography.heading-lg",
        ] {
            assert_eq!(
                t.style(proportional).unwrap().font.family,
                FontFamily::Proportional,
                "{proportional} must stay in the sans face"
            );
        }
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

        // Monospace stands in for an installed emphasis face: it is the
        // second family egui's defaults do have, so the assertion is about the
        // mapping and not about a font this test would have to ship. Both
        // emphasis classes are bound to it because the shipped ramp's only
        // emphasis step is `Medium` — Carbon's `semibold` — while a host theme
        // can still declare `Bold`, and the mapping has to carry either.
        let faces = FontFaces::new(
            FontFamily::Proportional,
            FontFamily::Monospace,
            FontFamily::Monospace,
        );
        assert!(!faces.distinguishes_weight(), "medium still shares bold");

        let heading = TextStyle::from_token(
            &TypographyValue {
                size: 20.0,
                line_height: 28.0,
                weight: TypographyWeight::Bold,
                letter_spacing: 0.0,
                family: TypographyFamily::Sans,
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
                letter_spacing: 0.0,
                family: TypographyFamily::Sans,
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

    /// Last registration for a name wins, the same rule
    /// [`crate::paint::CustomPainters`] uses.
    #[test]
    fn a_second_registration_replaces_the_first() {
        let mut measurers = CustomMeasurers::new();
        measurers.register("gauge", |_| gorgon_petra::geom::Size::new(1.0, 1.0));
        measurers.register("gauge", |_| gorgon_petra::geom::Size::new(2.0, 2.0));
        let size = measurers.get("gauge").expect("registered")(SizeProposal::unspecified());
        assert_eq!(size, gorgon_petra::geom::Size::new(2.0, 2.0));
    }

    /// A name nothing is registered for has no entry at all — not a
    /// zero-sized one — so a caller can tell "unregistered" from
    /// "registered and answered zero".
    #[test]
    fn an_unregistered_name_has_no_entry() {
        let measurers = CustomMeasurers::new();
        assert!(measurers.get("gauge").is_none());
    }

    /// [`HostedContent`] answers a registered `custom` name from the
    /// registry and leaves the shaper's own bookkeeping untouched for it.
    #[test]
    fn hosted_content_answers_a_registered_custom_name_from_the_registry() {
        let h = Headless::new();
        let mut s = h.shaper();
        let mut measurers = CustomMeasurers::new();
        measurers.register("gauge", |_| gorgon_petra::geom::Size::new(42.0, 24.0));
        let mut content = HostedContent::new(&mut s, &measurers);
        assert_eq!(
            content.custom("gauge", SizeProposal::unspecified()),
            gorgon_petra::geom::Size::new(42.0, 24.0)
        );
        assert!(
            s.unsupported_customs().is_empty(),
            "a registered name answered; it is not unsupported"
        );
    }

    /// The fallback [`GalleyShaper::custom`] takes over, unchanged, when
    /// [`HostedContent`] has no measurer for a name: registering *some*
    /// names must not make an unregistered one silently succeed.
    #[test]
    fn hosted_content_falls_back_to_the_shaper_for_an_unregistered_name() {
        let h = Headless::new();
        let mut s = h.shaper();
        let measurers = CustomMeasurers::new();
        let mut content = HostedContent::new(&mut s, &measurers);
        assert_eq!(
            content.custom("gauge", SizeProposal::unspecified()),
            gorgon_petra::geom::Size::ZERO
        );
        assert_eq!(s.unsupported_customs(), ["gauge"]);
    }
}
