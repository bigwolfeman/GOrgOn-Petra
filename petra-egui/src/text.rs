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

use crate::image::ImageSources;
use egui::emath::{GUI_ROUNDING, GuiRounding};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Context, FontFamily, FontId, Galley};
use gorgon_petra::cache::LruCache;
use gorgon_petra::geom::{Rect, Size};
use gorgon_petra::layout::{
    ContentMeasure, InkOverflow, RunFace, SizeProposal, TextMeasurement, TextRequest,
};
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
    /// The italic family per weight class, `Regular`, `Medium`, `Bold`.
    /// The upright families until [`FontFaces::with_italic`] binds faces:
    /// a stack with no italic draws an italic run upright, and
    /// [`FontFaces::distinguishes_slant`] says so.
    italic: [FontFamily; 3],
    mono: FontFamily,
}

impl Default for FontFaces {
    fn default() -> Self {
        Self::new(
            FontFamily::Proportional,
            FontFamily::Proportional,
            FontFamily::Proportional,
        )
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
            italic: [regular.clone(), medium.clone(), bold.clone()],
            regular,
            medium,
            bold,
            mono: FontFamily::Monospace,
        }
    }

    /// The same faces, drawing an italic run ([`gorgon_petra::tree::TextRun::italic`])
    /// at each weight class through these families.
    #[must_use]
    pub fn with_italic(
        mut self,
        regular: FontFamily,
        medium: FontFamily,
        bold: FontFamily,
    ) -> Self {
        self.italic = [regular, medium, bold];
        self
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

    /// The italic family a sans run at `weight` draws through.
    #[must_use]
    pub fn italic(&self, weight: TypographyWeight) -> &FontFamily {
        &self.italic[weight_index(weight)]
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

    /// Whether an italic run can paint differently from an upright one at
    /// every weight class: `false` under [`FontFaces::default`], whose stack
    /// has no italic face.
    #[must_use]
    pub fn distinguishes_slant(&self) -> bool {
        let upright = [&self.regular, &self.medium, &self.bold];
        self.italic
            .iter()
            .all(|italic| upright.iter().all(|up| *up != italic))
    }
}

/// `Regular`, `Medium`, `Bold` as 0, 1, 2: the order every per-weight array
/// in this module is in.
fn weight_index(weight: TypographyWeight) -> usize {
    match weight {
        TypographyWeight::Regular => 0,
        TypographyWeight::Medium => 1,
        TypographyWeight::Bold => 2,
    }
}

/// The faces a sans style's runs can switch to: its own weight, and the
/// upright and italic family of each weight class.
#[derive(Clone, Debug, PartialEq)]
pub struct RunFamilies {
    /// The style's own weight, which a run with no weight of its own keeps.
    pub own: TypographyWeight,
    /// Upright families, `Regular`, `Medium`, `Bold`.
    pub upright: [FontFamily; 3],
    /// Italic families, same order.
    pub italic: [FontFamily; 3],
    /// The family an inline-code run ([`gorgon_petra::tree::TextRun::code`])
    /// draws through: the theme's monospace face.
    pub mono: FontFamily,
    /// An inline-code run's size over this style's size: the theme's
    /// `typography.code` over its `typography.body`
    /// ([`Typography::from_theme`]), so code in a heading is as much smaller
    /// than the heading as code in a paragraph is than the paragraph.
    pub code_scale: f32,
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
/// The token's own weight is spent at construction, choosing the family in
/// `font` through [`FontFaces`] — see that type for what the default font
/// stack can and cannot express. [`TextStyle::faces`] keeps the rest, so a
/// run that names a weight or a slant ([`gorgon_petra::layout::RunFace`])
/// draws in the same size, leading and tracking through its own face.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Size and family.
    pub font: FontId,
    /// The families a run's face draws through at this style, or `None` when
    /// a run's weight and slant change nothing: a monospace style (the mono
    /// family has one face) or a style built from a bare [`FontId`].
    pub faces: Option<RunFamilies>,
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
            faces: None,
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
        let at = |weight| faces.family(value.family, weight).clone();
        Self {
            font: FontId::new(value.size, at(value.weight)),
            faces: (value.family == TypographyFamily::Sans).then(|| RunFamilies {
                own: value.weight,
                upright: [
                    at(TypographyWeight::Regular),
                    at(TypographyWeight::Medium),
                    at(TypographyWeight::Bold),
                ],
                italic: [
                    faces.italic(TypographyWeight::Regular).clone(),
                    faces.italic(TypographyWeight::Medium).clone(),
                    faces.italic(TypographyWeight::Bold).clone(),
                ],
                mono: faces
                    .family(TypographyFamily::Mono, TypographyWeight::Regular)
                    .clone(),
                code_scale: 1.0,
            }),
            line_height: Some(value.line_height),
            extra_letter_spacing: value.letter_spacing,
        }
    }

    /// This style at `factor` times its size: the font, the line height and
    /// the tracking all scale together, so a zoomed run keeps the
    /// proportions the token declares
    /// ([`gorgon_petra::tree::Props::text_scale`]). A factor of exactly 1
    /// is the style itself.
    #[must_use]
    pub fn scaled(&self, factor: f32) -> Self {
        if factor == 1.0 {
            return self.clone();
        }
        Self {
            font: FontId::new(self.font.size * factor, self.font.family.clone()),
            faces: self.faces.clone(),
            line_height: self.line_height.map(|h| h * factor),
            extra_letter_spacing: self.extra_letter_spacing * factor,
        }
    }

    /// The font a run set in `face` draws with: this style's own font for
    /// the plain face or a style that has no run families.
    #[must_use]
    pub fn font_at(&self, face: RunFace) -> FontId {
        match &self.faces {
            Some(families) if face.code => {
                FontId::new(self.font.size * families.code_scale, families.mono.clone())
            }
            Some(families) if !face.is_plain() => {
                let at = weight_index(face.weight.unwrap_or(families.own));
                let family = if face.italic {
                    &families.italic[at]
                } else {
                    &families.upright[at]
                };
                FontId::new(self.font.size, family.clone())
            }
            _ => self.font.clone(),
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

    /// The token code inside running text is set at, whose size over
    /// [`Typography::BODY`]'s is an inline-code run's scale.
    pub const CODE: &'static str = "typography.code";

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
        // Inline code is the code step's share of the body step, applied to
        // whatever style the code sits in (`RunFamilies::code_scale`). A
        // theme missing either step sets code at its style's own size.
        let size_of = |name: &str| styles.get(name).map(|style: &TextStyle| style.font.size);
        if let (Some(code), Some(body)) = (size_of(Self::CODE), size_of(Self::BODY))
            && body > 0.0
        {
            for style in styles.values_mut() {
                if let Some(families) = style.faces.as_mut() {
                    families.code_scale = code / body;
                }
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

/// The parts of a resolved [`TextStyle`] that change a shaped run.
///
/// Fields rather than one formatted string: the key is built on every
/// measurement and every paint of every run, cache hit or not.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct StyleKey {
    size_bits: u32,
    line_height_bits: Option<u32>,
    family: FontFamily,
    letter_spacing_bits: u32,
}

/// Everything that decides what a galley looks like.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct GalleyKey {
    text: String,
    /// Size, line height, family and tracking: two runs that differ in any
    /// of them are two galleys. Bit patterns, not values, for the same
    /// reason `wrap_bits` is.
    style: StyleKey,
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
    runs: Vec<(usize, Option<u32>, bool, Option<u32>)>,
    /// Face runs over the text, from [`TextRequest::runs`], or empty when
    /// every run is in the node's own face. A shaping input: the same string
    /// with a `strong` or an italic stretch has other advances, so it is its
    /// own galley for measuring and for painting alike, and both read it off
    /// the request.
    faces: Vec<(usize, RunFace)>,
}

/// One painted run of a text: its byte length, its ink (`None` for the
/// painter's fallback), and whether it is underlined
/// ([`gorgon_petra::tree::TextRun::underline`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InkRun {
    /// Byte length.
    pub len: usize,
    /// Ink, or `None` for the fallback.
    pub color: Option<Color32>,
    /// Underlined.
    pub underline: bool,
    /// The band behind the run, or `None`.
    pub background: Option<Color32>,
}

impl InkRun {
    /// A run of `len` bytes in `color`, not underlined.
    #[must_use]
    pub fn plain(len: usize, color: Option<Color32>) -> Self {
        Self {
            len,
            color,
            underline: false,
            background: None,
        }
    }
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
    /// because a galley carries its glyph rows and is the heavier object.
    ///
    /// It is **not** a bound on one screen. A key is a (text, wrap width)
    /// pair and layout asks for one text at several widths, so a dense screen
    /// asks for far more keys than it has strings: the GOrgOn shell asked for
    /// 2 072 keys over 336 strings in one frame. Under a plain cap that
    /// thrashed (see [`gorgon_petra::cache`]); a host that calls
    /// [`GalleyShaper::begin_pass`] per frame, as [`crate::host::Host`]
    /// does, never evicts a galley its current or previous frame used, and
    /// this number bounds only the galleys kept beyond that for reuse.
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

    /// Mark the start of a frame: galleys this frame or the last one used are
    /// never evicted, whatever the bound ([`gorgon_petra::cache::LruCache::begin_pass`]).
    pub fn begin_pass(&mut self) {
        self.galleys.begin_pass();
    }

    /// Galleys held now.
    #[must_use]
    pub fn len(&self) -> usize {
        self.galleys.len()
    }

    /// Whether no galley is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.galleys.is_empty()
    }

    /// Galleys dropped to stay inside the bound since construction. Under
    /// [`GalleyShaper::begin_pass`] only galleys no recent frame used are
    /// dropped. Without it, a number that climbs every frame means one screen
    /// holds more distinct runs than the bound, and the shaper is re-shaping
    /// text it just had.
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

    /// The galley `key`'s request shapes to, built from the unbounded galley
    /// already shaped for the same run when the request's `wrap_width` cannot
    /// break it. `None` when it can, or when no unbounded galley is held.
    ///
    /// Layout asks for one string at its open width and then at the width
    /// it was given, which is usually at least that wide; this answers the
    /// second ask without shaping the string again. It follows egui 0.36's
    /// own layout of a wrapped job step for step, and
    /// `a_width_that_holds_the_open_run_draws_what_shaping_at_it_draws`
    /// holds the two equal over a sweep of widths, so an egui upgrade that
    /// changes either step fails there.
    ///
    /// 1. egui lays a job out at its wrap width rounded to a whole point
    ///    (`GalleyCache::layout_internal`), and breaks a paragraph only when
    ///    it is wider than that plus half a point when the job rounds its
    ///    output (`LayoutJob::effective_wrap_width`). A run whose widest
    ///    paragraph fits keeps every row, glyph, elision and mesh of its
    ///    unbounded layout. Every job this shaper builds is left-aligned and
    ///    unjustified, so nothing else in the layout reads the width.
    /// 2. Rounding the output then caps the galley's box at the wrap width
    ///    (`Galley::round_output_to_gui`). An unbounded box already inside
    ///    the width is the answer as it stands; a wider one is the same
    ///    galley with its right edge pulled in, exactly as egui pulls it.
    ///
    /// A galley answered as it stands keeps its own copy of its job, wrap
    /// width included. Nothing in this crate reads that width back.
    fn open_galley_that_fits(&mut self, key: &GalleyKey, wrap_width: f32) -> Option<Arc<Galley>> {
        if !wrap_width.is_finite() {
            return None;
        }
        let open = GalleyKey {
            wrap_bits: f32::INFINITY.to_bits(),
            ..key.clone()
        };
        let galley = Arc::clone(self.galleys.get(&open)?);
        let width = wrap_width.round();
        let rounds = galley.job.round_output_to_gui;
        // `intrinsic_size()` is itself rounded to 1/32 of a point when the
        // job rounds its output; the slack covers that rounding.
        let breaks_past = if rounds {
            width + 0.5 - GUI_ROUNDING
        } else {
            width
        };
        if galley.intrinsic_size().x > breaks_past {
            return None;
        }
        if galley.rect.width() <= width {
            return Some(galley);
        }
        if !rounds || galley.rect.width() > width + 1.0 {
            // Neither can happen to a run that fits; shape rather than guess.
            return None;
        }
        let mut capped = (*galley).clone();
        capped.rect.max.x = capped.rect.max.x.min(capped.rect.min.x + width).floor_ui();
        let mut job = (*capped.job).clone();
        job.wrap.max_width = width;
        capped.job = Arc::new(job);
        Some(Arc::new(capped))
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
    pub fn galley_runs(&mut self, req: &TextRequest<'_>, runs: &[InkRun]) -> Arc<Galley> {
        // Cloned rather than borrowed: the shaping below needs `&mut self`
        // for the cache and the counters, and a `TextStyle` is a size, a
        // family handle and an optional float. Scaled before the key is
        // built, so a zoomed run and the theme-sized one never share a
        // galley: the key already holds the size, leading and tracking.
        let style = self
            .typography
            .resolve(req.style)
            .0
            .scaled(req.text_scale.factor());
        let faces = req.runs.faces();
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
            style: StyleKey {
                size_bits: style.font.size.to_bits(),
                line_height_bits: style.line_height.map(f32::to_bits),
                family: style.font.family.clone(),
                letter_spacing_bits: style.extra_letter_spacing.to_bits(),
            },
            wrap_bits: wrap_width.to_bits(),
            scale_bits: self.ctx.pixels_per_point().to_bits(),
            max_rows,
            break_anywhere,
            overflow,
            runs: runs
                .iter()
                .map(|run| {
                    (
                        run.len,
                        run.color.map(|c| u32::from_le_bytes(c.to_array())),
                        run.underline,
                        run.background.map(|c| u32::from_le_bytes(c.to_array())),
                    )
                })
                .collect(),
            faces: faces.clone(),
        };
        if let Some(hit) = self.galleys.get(&key) {
            self.reused += 1;
            return Arc::clone(hit);
        }
        if let Some(open) = self.open_galley_that_fits(&key, wrap_width) {
            self.reused += 1;
            self.galleys.insert(key, Arc::clone(&open));
            return open;
        }
        self.shaped += 1;
        // Built section-by-hand rather than through `LayoutJob::simple`,
        // which takes a `FontId` and has nowhere to put a line height.
        // `single_section` leaves `wrap` at its default, so the wrap width
        // `simple` would have set is set here.
        let code_line_height = faces
            .iter()
            .any(|(_, face)| face.code)
            .then(|| self.code_line_height(&style));
        let format = |face: RunFace,
                      color: Color32,
                      underline: bool,
                      background: Option<Color32>| TextFormat {
            font_id: style.font_at(face),
            color,
            background: background.unwrap_or(Color32::TRANSPARENT),
            line_height: if face.code {
                code_line_height.or(style.line_height)
            } else {
                style.line_height
            },
            extra_letter_spacing: style.extra_letter_spacing,
            // One unit in the run's own ink, which egui draws under the
            // baseline and which moves no glyph.
            underline: if underline {
                egui::Stroke::new(1.0, color)
            } else {
                egui::Stroke::NONE
            },
            ..TextFormat::default()
        };
        let mut job = if runs.is_empty() && faces.is_empty() {
            LayoutJob::single_section(
                req.text.to_owned(),
                format(RunFace::default(), Color32::PLACEHOLDER, false, None),
            )
        } else {
            let mut job = LayoutJob {
                text: req.text.to_owned(),
                ..LayoutJob::default()
            };
            // One section per stretch over which both the ink run and the
            // face run hold still: the two lists tile the same string at
            // their own boundaries, and a section is where neither changes.
            // Clamped, not trusted: acceptance guarantees the tiling for an
            // accepted tree, and a shaper handed a bad one draws a short line
            // rather than panicking inside a paint pass.
            // Inline code stands clear of its neighbours: a space before the
            // section that opens a code run and before the one that follows
            // it (`TextRun::code`). Space, not glyphs, so the run's bytes and
            // its copy are the author's.
            let clear = code_clearance(&style);
            let mut was_code = false;
            for (range, face, ink) in sections(req.text.len(), &faces, runs) {
                let leading_space = if face.code == was_code { 0.0 } else { clear };
                was_code = face.code;
                let (color, underline, background) =
                    ink.map_or((Color32::PLACEHOLDER, false, None), |run| {
                        (
                            run.color.unwrap_or(Color32::PLACEHOLDER),
                            run.underline,
                            run.background,
                        )
                    });
                job.sections.push(egui::text::LayoutSection {
                    leading_space,
                    byte_range: egui::text::ByteIndex(range.start)
                        ..egui::text::ByteIndex(range.end),
                    format: format(face, color, underline, background),
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

    /// The line height an inline-code section of `style` lays out with, so
    /// its glyphs share the words' baseline.
    ///
    /// egui puts a glyph's baseline at its own face's ascent below the row
    /// top, plus `valign` times the row's height less the section's line
    /// height (`epaint` `galley_from_rows`). With one line height on every
    /// section, that leaves the code face's baseline at the code face's
    /// ascent: the mono face at the code step's smaller size has the
    /// smaller ascent, so inline code rode above the words around it. A
    /// code section shorter than the row by the difference in ascents
    /// takes that difference back under the default bottom `valign`, which
    /// lands both baselines on one pixel row. A code face with the larger
    /// ascent gets the longer line instead: the row grows by the
    /// difference and the words move down to the code's baseline.
    ///
    /// A row that wraps to inline code alone is that difference shorter
    /// than a row of words (about a pixel and a half on the shipped body
    /// step): egui sizes a row from the glyphs on it, and a code section
    /// cannot ask for a taller row without lifting its own baseline.
    fn code_line_height(&self, style: &TextStyle) -> f32 {
        let ppp = self.ctx.pixels_per_point();
        let code = style.font_at(RunFace {
            code: true,
            ..RunFace::default()
        });
        let coords = egui::epaint::text::VariationCoords::default();
        let (words, code) = self.ctx.fonts_mut(|fonts| {
            let mut metrics = |font: &FontId| {
                fonts
                    .fonts
                    .font(&font.family)
                    .styled_metrics(ppp, font.size, &coords)
            };
            (metrics(&style.font), metrics(&code))
        });
        let row = style.line_height.unwrap_or(words.row_height);
        row - (words.ascent - code.ascent)
    }
}

/// The space an inline-code run ([`gorgon_petra::tree::TextRun::code`])
/// keeps clear at each end in `style`: a share of the code face's size, so
/// it scales with the words. Its chip ([`code_chip`]) reaches most of the
/// way into it, and the hair left over keeps the chip off the next glyph.
#[must_use]
pub fn code_clearance(style: &TextStyle) -> f32 {
    style
        .font_at(RunFace {
            code: true,
            ..RunFace::default()
        })
        .size
        * CODE_CLEARANCE
}

/// [`code_clearance`] as a share of the code face's size: the chip's reach
/// and a twentieth of an em more, so punctuation written against the code
/// (`(`code`,`) sits against the chip, as it does in a GitHub preview,
/// while the chip still stops short of the glyph beside it.
const CODE_CLEARANCE: f32 = 0.30;

/// How far a chip reaches into its clear space, as a share of the code
/// face's size: GitHub's `code` pads a quarter em or so at each side.
const CODE_CHIP_REACH: f32 = 0.25;

/// The chip behind an inline-code run on one row: `row_band` (the run's
/// glyphs on the row, the full line height, from [`glyph_rects`])
/// reaching into its clear space at each end, and one and a half times the
/// code face's size tall, centred on the row. The
/// line's leading stays out of it, so chips on two lines never touch.
#[must_use]
pub fn code_chip(style: &TextStyle, row_band: egui::Rect) -> egui::Rect {
    let size = style
        .font_at(RunFace {
            code: true,
            ..RunFace::default()
        })
        .size;
    let reach = size * CODE_CHIP_REACH;
    let height = (size * 1.5).min(row_band.height());
    let centre = row_band.center().y;
    egui::Rect::from_min_max(
        egui::pos2(row_band.min.x - reach, centre - height / 2.0),
        egui::pos2(row_band.max.x + reach, centre + height / 2.0),
    )
}

/// The glyph boxes of byte `range` of `galley`, one per row it crosses, in
/// galley-local coordinates: from the first glyph's left edge to the last
/// glyph's right edge, the full height of the row.
///
/// Unlike [`selection_rects`], which ends a stretch where the next glyph
/// starts (a selection is a caret span), this ends at the last glyph's own
/// advance, so space laid out after the run is not part of its box.
#[must_use]
pub fn glyph_rects(galley: &Galley, range: &std::ops::Range<usize>) -> Vec<egui::Rect> {
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
        let mut span: Option<(f32, f32)> = None;
        for (i, glyph) in row.glyphs.iter().enumerate() {
            let at = row_start + i;
            if at >= from && at < to {
                let (x0, x1) = (glyph.pos.x, glyph.pos.x + glyph.advance_width);
                span = Some(span.map_or((x0, x1), |(a, b)| (a.min(x0), b.max(x1))));
            }
        }
        if let Some((x0, x1)) = span
            && x1 > x0
        {
            out.push(egui::Rect::from_min_max(
                egui::pos2(row.pos.x + x0, row.min_y()),
                egui::pos2(row.pos.x + x1, row.max_y()),
            ));
        }
        row_start += row.char_count_including_newline().0;
    }
    out
}

/// The stretches of a `len`-byte string over which neither the face runs
/// nor the ink runs change, in order, tiling `0..len`.
///
/// Each list tiles the string at its own boundaries (either may be empty,
/// meaning one stretch); a boundary past `len` is clamped and a list that
/// stops short leaves the tail in the plain face and no ink, the placeholder the
/// painter fills with the node's own `foreground`.
fn sections<'r>(
    len: usize,
    faces: &[(usize, RunFace)],
    inks: &'r [InkRun],
) -> Vec<(std::ops::Range<usize>, RunFace, Option<&'r InkRun>)> {
    let mut cuts: Vec<usize> = Vec::with_capacity(faces.len() + inks.len() + 1);
    let mut at = 0usize;
    for (n, _) in faces {
        at = at.saturating_add(*n).min(len);
        cuts.push(at);
    }
    at = 0;
    for run in inks {
        at = at.saturating_add(run.len).min(len);
        cuts.push(at);
    }
    cuts.push(len);
    cuts.sort_unstable();
    cuts.dedup();
    // The run covering byte `byte` of a list, by walking its cumulative ends.
    fn covering<T>(list: &[T], len_of: impl Fn(&T) -> usize, byte: usize) -> Option<&T> {
        let mut end = 0usize;
        list.iter().find(|item| {
            end = end.saturating_add(len_of(item));
            byte < end
        })
    }
    let mut out = Vec::with_capacity(cuts.len());
    let mut start = 0usize;
    for cut in cuts {
        if cut > start {
            let face = covering(faces, |f| f.0, start).map_or_else(RunFace::default, |f| f.1);
            let ink = covering(inks, |r| r.len, start);
            out.push((start..cut, face, ink));
            start = cut;
        }
    }
    out
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

/// The zero-width rectangle a caret bar stands on before byte offset `at` in
/// `galley`, in coordinates local to the galley's origin: the bar's x, and
/// the top and bottom of the row it sits in.
///
/// egui's own cursor geometry, so the bar lands exactly where
/// [`byte_offset_at`] would read the same position back. An offset past the
/// end answers the end of the last row.
#[must_use]
pub fn caret_rect(galley: &Galley, at: usize) -> egui::Rect {
    let index = byte_to_char(galley.text(), at);
    galley.pos_from_cursor(egui::text::CCursor::new(index))
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
    runs: &[InkRun],
    text_len: usize,
    range: &std::ops::Range<usize>,
    ink: Color32,
) -> Vec<InkRun> {
    // A node with no runs of its own still needs one, or there is nothing to
    // split and the whole string keeps the node's `foreground`.
    let base: Vec<InkRun> = if runs.is_empty() {
        vec![InkRun::plain(text_len, None)]
    } else {
        runs.to_vec()
    };
    let mut out: Vec<InkRun> = Vec::with_capacity(base.len() + 2);
    let mut at = 0usize;
    for run in base {
        let (start, end) = (at, at + run.len);
        at = end;
        // Three pieces, any of which may be empty: before the selection,
        // inside it, after it. The underline survives the selection: it is
        // a shape, and the selection changes only the ink.
        for (from, to, colour) in [
            (start, end.min(range.start), run.color),
            (start.max(range.start), end.min(range.end), Some(ink)),
            (start.max(range.end), end, run.color),
        ] {
            if to > from {
                match out.last_mut() {
                    Some(last)
                        if last.color == colour
                            && last.underline == run.underline
                            && last.background == run.background =>
                    {
                        last.len += to - from;
                    }
                    _ => out.push(InkRun {
                        len: to - from,
                        color: colour,
                        underline: run.underline,
                        background: run.background,
                    }),
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
        // `WrapOrHide` shapes as `Wrap` does; layout shows none of it in a
        // box too narrow for its shortest word.
        TextWrap::Wrap | TextWrap::WrapOrHide => (
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

/// The widest whitespace-free stretch of glyphs on any row of `galley`,
/// from the first glyph's left edge to the last one's advance.
fn widest_word(galley: &Galley) -> f32 {
    let mut widest = 0.0f32;
    for row in &galley.rows {
        let mut start: Option<f32> = None;
        let mut end = 0.0f32;
        for glyph in &row.row.glyphs {
            if glyph.chr.is_whitespace() {
                if let Some(from) = start.take() {
                    widest = widest.max(end - from);
                }
                continue;
            }
            start.get_or_insert(glyph.pos.x);
            end = glyph.pos.x + glyph.advance_width;
        }
        if let Some(from) = start {
            widest = widest.max(end - from);
        }
    }
    widest
}

/// How far `galley`'s glyph meshes reach past its box, above and below.
///
/// `mesh_bounds` is the tight box around every glyph quad epaint will draw;
/// `rect` is the line boxes layout sized the node from. An empty run has no
/// mesh (`Rect::NOTHING`) and no overflow.
fn ink_overflow(galley: &Galley) -> InkOverflow {
    let ink = galley.mesh_bounds;
    if !ink.is_positive() {
        return InkOverflow::default();
    }
    InkOverflow {
        above: (galley.rect.top() - ink.top()).max(0.0),
        below: (ink.bottom() - galley.rect.bottom()).max(0.0),
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
            ink: ink_overflow(&galley),
        }
    }

    /// Read off the open probe's galley, so each word is measured in the
    /// face its runs set it in and with the shaping the paint will use.
    fn min_content_width(&mut self, req: &TextRequest<'_>) -> f32 {
        let open = TextRequest {
            available_width: None,
            ..*req
        };
        widest_word(&self.galley(&open))
    }

    fn fragments(&mut self, req: &TextRequest<'_>, range: std::ops::Range<usize>) -> Vec<Rect> {
        // The galley `text` measured, so the rectangles are the glyphs that
        // will be painted: the painter shapes the same request with its ink
        // and ink moves no glyph.
        let galley = self.galley(req);
        selection_rects(&galley, &range)
            .into_iter()
            .map(|r| Rect::new(r.min.x, r.min.y, r.width(), r.height()))
            .collect()
    }

    fn image(&mut self, source: &str, _proposal: SizeProposal) -> Size {
        // No image loader is wired yet. Returning zero is the honest answer —
        // an invented size would reserve space for a picture that will never
        // appear — and the source is recorded so the host can say which images
        // it could not draw instead of the panel just looking wrong.
        self.unsupported_images.insert(source.to_owned());
        Size::ZERO
    }

    fn custom(&mut self, name: &str, _key: Option<&str>, _proposal: SizeProposal) -> Size {
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
pub type CustomMeasurerFn = dyn Fn(&CustomMeasureCtx<'_>) -> Size;

/// What a registered custom measurer is asked: the offer, and which node.
///
/// The measurement mirror of [`crate::paint::CustomPaintCtx`]. A struct, not
/// two arguments, so the next fact a measurer needs is a field and not a
/// change to every registered closure.
#[derive(Clone, Copy, Debug)]
pub struct CustomMeasureCtx<'a> {
    /// The offer, honoured the way any leaf honours one.
    pub proposal: SizeProposal,
    /// The node's `Props::custom_key`, or `None` when it set none.
    pub key: Option<&'a str>,
}

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
        measurer: impl Fn(&CustomMeasureCtx<'_>) -> Size + 'static,
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
    images: &'a ImageSources,
}

impl<'a> HostedContent<'a> {
    /// Measure through `shaper`, consulting `measurers` for `custom` names
    /// first and `images` for image sources.
    #[must_use]
    pub fn new(
        shaper: &'a mut GalleyShaper,
        measurers: &'a CustomMeasurers,
        images: &'a ImageSources,
    ) -> Self {
        Self {
            shaper,
            measurers,
            images,
        }
    }
}

impl ContentMeasure for HostedContent<'_> {
    fn text(&mut self, req: &TextRequest<'_>) -> TextMeasurement {
        self.shaper.text(req)
    }

    fn min_content_width(&mut self, req: &TextRequest<'_>) -> f32 {
        self.shaper.min_content_width(req)
    }

    fn fragments(&mut self, req: &TextRequest<'_>, range: std::ops::Range<usize>) -> Vec<Rect> {
        self.shaper.fragments(req, range)
    }

    fn image(&mut self, source: &str, proposal: SizeProposal) -> Size {
        // A registered picture takes the room its pixels take, shrunk to
        // the offer ([`crate::image::measure`]). An unregistered source
        // keeps the shaper's answer: no room, and the name recorded.
        match self.images.size(source) {
            Some(natural) => crate::image::measure(natural, proposal),
            None => self.shaper.image(source, proposal),
        }
    }

    fn custom(&mut self, name: &str, key: Option<&str>, proposal: SizeProposal) -> Size {
        match self.measurers.get(name) {
            // A registered measurer answers instead of the shaper's
            // fallback — and does *not* touch `unsupported_customs`: that
            // set means "asked for and nothing answered", which is no
            // longer true for this name.
            Some(measurer) => measurer(&CustomMeasureCtx { proposal, key }),
            None => self.shaper.custom(name, key, proposal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CustomMeasureCtx, CustomMeasurers, ELLIPSIS, FontFaces, GalleyShaper, HostedContent,
        ImageSources, InkRun, TextStyle, Typography, code_chip, code_clearance, glyph_rects,
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
            runs: gorgon_petra::layout::TextRuns::Plain,
            text_scale: gorgon_petra::geom::Scale::ONE,
        }
    }

    /// A text scale reaches the glyphs: the same run under `text_scale` 2
    /// shapes twice as tall and wide as at 1, as one galley of its own (the
    /// cache key holds the scaled size), and a run under scale 1 is exactly
    /// the theme's style.
    ///
    /// Falsify by dropping the `.scaled(..)` call in `galley_runs`: both
    /// requests answer the same galley.
    #[test]
    fn a_text_scale_shapes_the_run_at_that_multiple_of_its_style() {
        let h = Headless::new();
        let mut s = h.shaper();
        let at = |s: &mut GalleyShaper, k: f32| {
            s.galley(&TextRequest {
                style: Some("typography.body"),
                text_scale: gorgon_petra::geom::Scale::new(k).unwrap(),
                ..req("Zoomed words", None, TextWrap::Wrap)
            })
        };
        let one = at(&mut s, 1.0);
        let two = at(&mut s, 2.0);
        assert!(
            !Arc::ptr_eq(&one, &two),
            "a zoomed run shared the plain galley"
        );
        let ratio_h = two.rect.height() / one.rect.height();
        let ratio_w = two.rect.width() / one.rect.width();
        assert!((ratio_h - 2.0).abs() < 0.05, "height ratio {ratio_h}");
        assert!((ratio_w - 2.0).abs() < 0.1, "width ratio {ratio_w}");
        let style = s.typography().style("typography.body").unwrap().clone();
        assert_eq!(style.scaled(1.0), style);
        let doubled = style.scaled(2.0);
        assert_eq!(doubled.font.size, style.font.size * 2.0);
        assert_eq!(doubled.line_height, style.line_height.map(|h| h * 2.0));
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

    /// The caret stands where the hit test reads the same offset back, in
    /// bytes: after the two-byte `é` the bar is right of the `a` and left of
    /// the emoji, and an offset past the end stands at the end.
    #[test]
    fn a_caret_stands_at_its_byte_offset_on_its_row() {
        let h = Headless::new();
        let mut shaper = h.shaper();
        let text = "aé😀bc";
        let galley = shaper.galley(&req(text, None, TextWrap::Clip));
        let after_e = 'a'.len_utf8() + 'é'.len_utf8();
        let bar = super::caret_rect(&galley, after_e);
        assert_eq!(bar.width(), 0.0);
        assert!(bar.height() > 0.0, "the bar spans its row: {bar:?}");
        assert_eq!(
            super::byte_offset_at(&galley, egui::vec2(bar.min.x + 0.1, bar.center().y)),
            after_e,
            "the bar is not where the hit test reads its offset back"
        );
        let start = super::caret_rect(&galley, 0);
        let end = super::caret_rect(&galley, 99);
        assert!(start.min.x < bar.min.x && bar.min.x < end.min.x);
        assert!((end.min.x - galley.rect.max.x).abs() < 0.5, "{end:?}");
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
        let runs = vec![InkRun::plain(4, Some(blue)), InkRun::plain(6, None)];

        let split = super::runs_with_selection(&runs, 10, &(2..7), ink);
        assert_eq!(split.iter().map(|run| run.len).sum::<usize>(), 10);
        assert_eq!(
            split,
            vec![
                InkRun::plain(2, Some(blue)),
                InkRun::plain(5, Some(ink)),
                InkRun::plain(3, None)
            ],
            "the selected stretch takes the selection's ink and the rest keeps its own"
        );

        // A node with no runs of its own still gets one to split, or the
        // whole string would keep the node's `foreground` and the highlight
        // would sit under unchanged ink.
        assert_eq!(
            super::runs_with_selection(&[], 10, &(2..7), ink),
            vec![
                InkRun::plain(2, None),
                InkRun::plain(5, Some(ink)),
                InkRun::plain(3, None)
            ]
        );

        // Neighbours with the same ink merge, so a selection covering two
        // runs of one colour does not lengthen the list for nothing.
        let merged = super::runs_with_selection(
            &[InkRun::plain(4, None), InkRun::plain(6, None)],
            10,
            &(0..10),
            ink,
        );
        assert_eq!(merged, vec![InkRun::plain(10, Some(ink))]);

        // An underline is a shape and outlives the selection's ink, and two
        // neighbours that differ only in it do not merge.
        let marked = InkRun {
            underline: true,
            ..InkRun::plain(4, None)
        };
        let split = super::runs_with_selection(&[marked, InkRun::plain(6, None)], 10, &(2..7), ink);
        assert_eq!(
            split,
            vec![
                InkRun {
                    underline: true,
                    ..InkRun::plain(2, None)
                },
                InkRun {
                    underline: true,
                    ..InkRun::plain(2, Some(ink))
                },
                InkRun::plain(3, Some(ink)),
                InkRun::plain(3, None),
            ]
        );
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
                InkRun::plain(3, Some(Color32::from_rgb(0x78, 0xa9, 0xff))),
                InkRun::plain(8, None),
                InkRun::plain(1, Some(Color32::from_rgb(0xff, 0x83, 0x89))),
                InkRun::plain(text.len() - 12, None),
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
        let a = s.galley_runs(
            &request,
            &[InkRun::plain(7, Some(blue)), InkRun::plain(5, None)],
        );
        let b = s.galley_runs(
            &request,
            &[InkRun::plain(7, Some(red)), InkRun::plain(5, None)],
        );
        let again = s.galley_runs(
            &request,
            &[InkRun::plain(7, Some(blue)), InkRun::plain(5, None)],
        );

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
            &[InkRun::plain(3, None), InkRun::plain(3, None)],
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

    /// What a galley draws: its box, its ink, its elision, its vertex count,
    /// and each row's place, size, line break and glyphs.
    #[derive(Debug, PartialEq)]
    struct Drawn {
        rect: egui::Rect,
        ink: egui::Rect,
        elided: bool,
        vertices: usize,
        rows: Vec<DrawnRow>,
    }

    /// One row: where it sits, its size, whether it ends a line, its glyphs.
    type DrawnRow = (egui::Pos2, egui::Vec2, bool, Vec<(char, egui::Pos2)>);

    fn drawn(galley: &Galley) -> Drawn {
        Drawn {
            rect: galley.rect,
            ink: galley.mesh_bounds,
            elided: galley.elided,
            vertices: galley.num_vertices,
            rows: galley
                .rows
                .iter()
                .map(|row| {
                    let glyphs = row.row.glyphs.iter().map(|g| (g.chr, g.pos)).collect();
                    (row.pos, row.row.size, row.ends_with_newline, glyphs)
                })
                .collect(),
        }
    }

    /// Layout asks for a run at its open width and then at the width it was
    /// given. When the given width cannot break the run, the answer is built
    /// from the open galley without shaping again, and it draws exactly what
    /// shaping at that width draws: same box, rows, glyphs, elision and ink.
    #[test]
    fn a_width_that_holds_the_open_run_draws_what_shaping_at_it_draws() {
        let h = Headless::new();
        for (text, wrap) in [
            ("line 811 as it was", TextWrap::Clip),
            ("line 812 rewritten now", TextWrap::Clip),
            ("line 811 as it was", TextWrap::Ellipsis),
            ("a wrapping run of words", TextWrap::Wrap),
            ("two\nparagraphs here", TextWrap::Wrap),
        ] {
            let mut s = h.shaper();
            let open = s.galley(&req(text, None, wrap));
            let natural = open.intrinsic_size().x;
            // A wide offer, the common case, shapes nothing.
            let wide = s.galley(&req(text, Some(natural + 40.0), wrap));
            assert!(Arc::ptr_eq(&open, &wide), "{text:?} {wrap:?}");
            assert_eq!(s.stats().0, 1, "{text:?} {wrap:?}");
            for step in 0..64 {
                // Every 1/32 of a point from just under the natural width to a
                // point past it, then a much wider offer.
                let width = if step == 63 {
                    natural + 40.0
                } else {
                    natural - 0.25 + step as f32 / 32.0
                };
                let at = req(text, Some(width), wrap);
                let given = s.galley(&at);
                // A shaper that never saw the open run shapes at the width.
                let fresh = h.shaper().galley(&at);
                assert_eq!(drawn(&given), drawn(&fresh), "{text:?} {wrap:?} at {width}");
            }
            let (shaped, _) = s.stats();
            assert!(
                shaped < 32,
                "{text:?} {wrap:?}: {shaped} of 64 widths shaped"
            );
        }
    }

    /// A width narrower than the open run breaks or cuts it, so it is shaped
    /// at that width and draws differently.
    #[test]
    fn a_width_short_of_the_open_run_is_shaped_at_that_width() {
        let h = Headless::new();
        for wrap in [TextWrap::Clip, TextWrap::Ellipsis, TextWrap::Wrap] {
            let mut s = h.shaper();
            let text = "a run of several words";
            let open = s.galley(&req(text, None, wrap));
            let narrow = s.galley(&req(text, Some(open.intrinsic_size().x / 2.0), wrap));
            assert!(!Arc::ptr_eq(&open, &narrow), "{wrap:?}");
            assert_ne!(drawn(&open), drawn(&narrow), "{wrap:?}");
            assert_eq!(s.stats().0, 2, "{wrap:?}");
        }
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
    /// A label set in `heading-sm` (Medium) and one in `body-compact`
    /// (Regular) share a baseline in a box of one height: the two faces
    /// have identical vertical metrics, so a selected tab's bolder label
    /// must not drop below its resting neighbours.
    #[test]
    fn the_medium_and_regular_steps_share_a_baseline() {
        let h = Headless::new();
        let t = Typography::from_theme(&dark(), &FontFaces::default());
        let mut s = h.shaper_with(t);
        let baseline = |s: &mut GalleyShaper, style: &str| {
            let request = TextRequest {
                style: Some(style),
                ..req("find", None, TextWrap::Wrap)
            };
            let g = s.galley(&request);
            let row = &g.rows[0];
            (row.pos.y, row.row.glyphs[0].pos.y, g.rect.height())
        };
        let regular = baseline(&mut s, "typography.body-compact");
        let medium = baseline(&mut s, "typography.heading-sm");
        assert_eq!(regular, medium, "regular {regular:?} medium {medium:?}");
    }

    /// A `strong` run: the measured width and the painted width agree, and
    /// both are the emphasis face's, wider than the same words at regular.
    ///
    /// Layout measures through `ContentMeasure::text` with the tree's runs
    /// and the painter shapes through `galley_runs` with the frame's runs
    /// plus its inks. Falsify by dropping `weights` from the galley's
    /// sections: the bold measurement equals the regular one.
    #[test]
    fn a_bold_run_measures_what_it_paints_and_is_wider_than_regular() {
        use gorgon_petra::frame::placement::TextRunPaint;
        use gorgon_petra::layout::TextRuns;
        use gorgon_petra::tree::TextRun;
        let h = Headless::new();
        crate::fonts::install_design_system(&h.0);
        pass(&h.0);
        let mut s = h.shaper_with(Typography::from_theme(
            &dark(),
            &crate::fonts::design_system_faces(),
        ));
        let text = "plain strong plain";
        let tree = [
            TextRun {
                len: 6,
                ..TextRun::default()
            },
            TextRun {
                len: 6,
                weight: Some(TypographyWeight::Medium),
                ..TextRun::default()
            },
            TextRun {
                len: 6,
                ..TextRun::default()
            },
        ];
        let frame: Vec<TextRunPaint> = tree
            .iter()
            .map(|r| TextRunPaint {
                len: r.len,
                foreground: None,
                underline: false,
                background: None,
                weight: r.weight,
                italic: r.italic,
                code: r.code,
            })
            .collect();
        let at = |runs| TextRequest {
            style: Some("typography.body"),
            runs,
            ..req(text, None, TextWrap::Wrap)
        };
        let regular = s.text(&at(TextRuns::Plain)).size.w;
        let measured = s.text(&at(TextRuns::Tree(&tree))).size.w;
        let inks = [
            InkRun::plain(6, None),
            InkRun::plain(6, Some(Color32::RED)),
            InkRun::plain(6, None),
        ];
        let painted = s
            .galley_runs(&at(TextRuns::Frame(&frame)), &inks)
            .rect
            .width();
        assert!(
            (measured - painted).abs() < 1e-3,
            "measured {measured} and painted {painted} must be one width"
        );
        assert!(
            measured > regular + 0.5,
            "the strong stretch must be set in its own face: {measured} vs regular {regular}"
        );
    }

    /// An italic run: measured and painted are one width, and the painted
    /// stretch draws through the italic family, at the style's own weight
    /// when the run names none and at Medium Italic when it is also strong.
    ///
    /// Falsify by dropping `italic` from `RunFace` in `TextRuns::faces`: the
    /// stretch draws upright and the family assertions fail.
    #[test]
    fn an_italic_run_measures_what_it_paints_in_the_italic_face() {
        use gorgon_petra::frame::placement::TextRunPaint;
        use gorgon_petra::layout::TextRuns;
        use gorgon_petra::tree::TextRun;
        let h = Headless::new();
        crate::fonts::install_design_system(&h.0);
        pass(&h.0);
        let mut s = h.shaper_with(Typography::from_theme(
            &dark(),
            &crate::fonts::design_system_faces(),
        ));
        let text = "plain emphasis both plain";
        let tree = [
            TextRun {
                len: 6,
                ..TextRun::default()
            },
            TextRun {
                len: 9,
                italic: true,
                ..TextRun::default()
            },
            TextRun {
                len: 5,
                italic: true,
                weight: Some(TypographyWeight::Medium),
                ..TextRun::default()
            },
            TextRun {
                len: 5,
                ..TextRun::default()
            },
        ];
        let frame: Vec<TextRunPaint> = tree
            .iter()
            .map(|r| TextRunPaint {
                len: r.len,
                foreground: None,
                underline: false,
                background: None,
                weight: r.weight,
                italic: r.italic,
                code: r.code,
            })
            .collect();
        let at = |runs| TextRequest {
            style: Some("typography.body"),
            runs,
            ..req(text, None, TextWrap::Wrap)
        };
        let upright = s.text(&at(TextRuns::Plain)).size.w;
        let measured = s.text(&at(TextRuns::Tree(&tree))).size.w;
        let galley = s.galley_runs(&at(TextRuns::Frame(&frame)), &[]);
        let painted = galley.rect.width();
        assert!(
            (measured - painted).abs() < 1e-3,
            "measured {measured} and painted {painted} must be one width"
        );
        assert!(
            (measured - upright).abs() > 0.5,
            "the italic stretches must have the italic face's advances: {measured} vs {upright}"
        );
        let families: Vec<String> = galley
            .job
            .sections
            .iter()
            .map(|section| format!("{:?}", section.format.font_id.family))
            .collect();
        assert!(
            families[1].contains(crate::fonts::SANS_ITALIC_FAMILY),
            "{families:?}"
        );
        assert!(
            families[2].contains(crate::fonts::SANS_MEDIUM_ITALIC_FAMILY),
            "{families:?}"
        );
        assert!(
            families[3].contains(crate::fonts::SANS_REGULAR_FAMILY),
            "{families:?}"
        );
    }

    /// An inline-code run: measured and painted are one width; the stretch
    /// draws through the mono family at the code step's share of the body
    /// size (12 over 14 in the shipped ramp); clear space opens before it
    /// and after it, and its chip covers its letters and reaches into that
    /// space without touching the glyphs either side.
    ///
    /// Falsify by dropping `code` from `RunFace` in `TextRuns::faces` (the
    /// stretch draws in the sans face), or the `leading_space` (the chip
    /// overlaps its neighbours).
    #[test]
    fn an_inline_code_run_measures_what_it_paints_in_the_mono_face_with_clear_space() {
        use gorgon_petra::frame::placement::TextRunPaint;
        use gorgon_petra::layout::TextRuns;
        use gorgon_petra::tree::TextRun;
        let h = Headless::new();
        crate::fonts::install_design_system(&h.0);
        pass(&h.0);
        let typography = Typography::from_theme(&dark(), &crate::fonts::design_system_faces());
        let style = typography.resolve(Some("typography.body")).0.clone();
        let mut s = h.shaper_with(typography);
        let text = "see code, then";
        let tree = [
            TextRun {
                len: 4,
                ..TextRun::default()
            },
            TextRun {
                len: 4,
                code: true,
                ..TextRun::default()
            },
            TextRun {
                len: 6,
                ..TextRun::default()
            },
        ];
        let frame: Vec<TextRunPaint> = tree
            .iter()
            .map(|r| TextRunPaint {
                len: r.len,
                foreground: None,
                underline: false,
                background: None,
                weight: r.weight,
                italic: r.italic,
                code: r.code,
            })
            .collect();
        let at = |runs| TextRequest {
            style: Some("typography.body"),
            runs,
            ..req(text, None, TextWrap::Wrap)
        };
        let measured = s.text(&at(TextRuns::Tree(&tree))).size.w;
        let galley = s.galley_runs(&at(TextRuns::Frame(&frame)), &[]);
        assert!(
            (measured - galley.rect.width()).abs() < 1e-3,
            "measured {measured} and painted {} must be one width",
            galley.rect.width()
        );
        let code = &galley.job.sections[1].format.font_id;
        assert!(
            (code.size - 14.0 * 12.0 / 14.0).abs() < 1e-3,
            "code is the code step's share of the body: {}",
            code.size
        );
        assert_eq!(
            code.family,
            crate::fonts::design_system_faces()
                .family(TypographyFamily::Mono, TypographyWeight::Regular,)
                .clone()
        );
        let clear = code_clearance(&style);
        assert!(clear > 0.0);
        let spaces: Vec<f32> = galley
            .job
            .sections
            .iter()
            .map(|s| s.leading_space)
            .collect();
        assert_eq!(
            spaces,
            [0.0, clear, clear],
            "space before and after the run"
        );
        let letters = glyph_rects(&galley, &(4..8));
        assert_eq!(letters.len(), 1);
        let chip = code_chip(&style, letters[0]);
        let before = glyph_rects(&galley, &(0..4))[0];
        let after = glyph_rects(&galley, &(8..9))[0];
        assert!(
            chip.min.x > before.max.x - 0.01,
            "{chip:?} clears {before:?}"
        );
        assert!(chip.max.x < after.min.x + 0.01, "{chip:?} clears {after:?}");
        assert!(chip.min.x < letters[0].min.x && chip.max.x > letters[0].max.x);
        assert!(
            chip.height() < letters[0].height(),
            "the line's leading stays out"
        );
    }

    /// Inline code shares the words' baseline on a small, the body and a
    /// heading step, and the row keeps the words' height.
    ///
    /// Falsify by giving code sections the style's own line height again
    /// (`GalleyShaper::code_line_height`): the smaller mono face's baseline
    /// rides its own ascent, above the words.
    #[test]
    fn inline_code_sits_on_the_words_baseline() {
        use gorgon_petra::layout::TextRuns;
        use gorgon_petra::tree::TextRun;
        let h = Headless::new();
        crate::fonts::install_design_system(&h.0);
        pass(&h.0);
        let typography = Typography::from_theme(&dark(), &crate::fonts::design_system_faces());
        let text = "see code, then";
        let tree = [
            TextRun {
                len: 4,
                ..TextRun::default()
            },
            TextRun {
                len: 4,
                code: true,
                ..TextRun::default()
            },
            TextRun {
                len: 6,
                ..TextRun::default()
            },
        ];
        for step in [
            "typography.caption",
            "typography.body",
            "typography.heading-lg",
        ] {
            let mut s = h.shaper_with(typography.clone());
            let plain = s.galley(&TextRequest {
                style: Some(step),
                ..req(text, None, TextWrap::Wrap)
            });
            let galley = s.galley(&TextRequest {
                style: Some(step),
                runs: TextRuns::Tree(&tree),
                ..req(text, None, TextWrap::Wrap)
            });
            assert_eq!(galley.rows.len(), 1, "{step}");
            let row = &galley.rows[0];
            let baselines: Vec<f32> = row.glyphs.iter().map(|g| g.pos.y).collect();
            let words = baselines[0];
            assert!(
                baselines.iter().all(|y| (y - words).abs() < 1e-3),
                "{step}: every glyph on one baseline: {baselines:?}"
            );
            assert!(
                (row.height() - plain.rows[0].height()).abs() < 1e-3,
                "{step}: the row keeps the words' height: {} against {}",
                row.height(),
                plain.rows[0].height()
            );
        }
    }

    /// A run list that names no weight shapes the plain string: same key,
    /// same galley, so no rect on any existing page moves.
    #[test]
    fn a_run_list_without_weight_shapes_the_plain_string() {
        use gorgon_petra::layout::TextRuns;
        use gorgon_petra::tree::TextRun;
        let h = Headless::new();
        let mut s = h.shaper();
        let tree = [TextRun {
            len: 4,
            underline: true,
            ..TextRun::default()
        }];
        let plain = s.galley(&req("text", None, TextWrap::Wrap));
        let ruled = s.galley(&TextRequest {
            runs: TextRuns::Tree(&tree),
            ..req("text", None, TextWrap::Wrap)
        });
        assert!(Arc::ptr_eq(&plain, &ruled), "one key, one galley");
    }

    /// A link's fragments are the painted glyphs' rows: what
    /// `selection_rects` reads off the galley the painter draws.
    #[test]
    fn fragments_are_the_painted_rows_of_the_range() {
        let h = Headless::new();
        let mut s = h.shaper();
        let request = req("alpha beta gamma delta", Some(60.0), TextWrap::Wrap);
        let rows = s.text(&request).lines;
        assert!(rows > 1, "the request must wrap for this to mean anything");
        let range = 6..16;
        let fragments = s.fragments(&request, range.clone());
        let galley = s.galley(&request);
        let want: Vec<gorgon_petra::geom::Rect> = super::selection_rects(&galley, &range)
            .into_iter()
            .map(|r| gorgon_petra::geom::Rect::new(r.min.x, r.min.y, r.width(), r.height()))
            .collect();
        assert_eq!(fragments, want);
        assert!(fragments.len() >= 2, "`beta gamma` wraps: {fragments:?}");
    }

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
            s.custom("gauge", None, SizeProposal::unspecified()),
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
        let size = measurers.get("gauge").expect("registered")(&CustomMeasureCtx {
            proposal: SizeProposal::unspecified(),
            key: None,
        });
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
        let images = ImageSources::new();
        measurers.register("gauge", |_| gorgon_petra::geom::Size::new(42.0, 24.0));
        let mut content = HostedContent::new(&mut s, &measurers, &images);
        assert_eq!(
            content.custom("gauge", None, SizeProposal::unspecified()),
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
        let images = ImageSources::new();
        let mut content = HostedContent::new(&mut s, &measurers, &images);
        assert_eq!(
            content.custom("gauge", None, SizeProposal::unspecified()),
            gorgon_petra::geom::Size::ZERO
        );
        assert_eq!(s.unsupported_customs(), ["gauge"]);
    }

    /// A registered picture takes its pixels' room; one nobody registered
    /// takes none and is named.
    #[test]
    fn hosted_content_measures_a_registered_image_and_names_an_unregistered_one() {
        use crate::image::ImagePixels;
        let h = Headless::new();
        let mut s = h.shaper();
        let measurers = CustomMeasurers::new();
        let mut images = ImageSources::new();
        images
            .register("logo", ImagePixels::try_new(4, 2, vec![0; 32]).unwrap())
            .unwrap();
        let mut content = HostedContent::new(&mut s, &measurers, &images);
        assert_eq!(
            content.image("logo", SizeProposal::unspecified()),
            gorgon_petra::geom::Size::new(4.0, 2.0)
        );
        assert_eq!(
            content.image("gone", SizeProposal::unspecified()),
            gorgon_petra::geom::Size::ZERO
        );
        assert_eq!(s.unsupported_images(), ["gone"]);
    }
}
