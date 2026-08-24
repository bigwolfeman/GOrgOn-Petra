//! The font stack, and the missing-glyph detector SC-009 is written about.
//!
//! Two jobs live here, and they are separate on purpose.
//!
//! # 1. A font stack that can draw more than Latin
//!
//! egui's built-in font definitions install four faces — `Ubuntu-Light`,
//! `Hack`, `NotoEmoji-Regular` and `emoji-icon-font` — and none of them
//! carries CJK, Arabic, Devanagari or Hebrew. Text in those scripts is not
//! *badly* drawn under the default stack; it is drawn as a row of replacement
//! boxes, one per codepoint. [`install_desktop_fallbacks`] adds the faces the
//! host machine already has, so the boxes stop.
//!
//! It is deliberately **opt-in**: it reads files, which is not something
//! [`crate::host::Host::new`] should do behind an application's back, and the
//! set of faces a product ships is a product decision. What this module
//! guarantees is that the decision is *takeable*, and that whatever it took
//! is reported rather than guessed at — see [`FontStackReport`].
//!
//! # 2. A detector that can tell a drawn glyph from a drawn box
//!
//! [`GlyphProbe`] answers, for one codepoint, which of four things the
//! shaper actually did:
//!
//! * [`GlyphOutcome::Rendered`] — a real, inked glyph;
//! * [`GlyphOutcome::Tofu`] — the replacement box, because no face has it;
//! * [`GlyphOutcome::Blank`] — a face claims the codepoint and draws nothing;
//! * [`GlyphOutcome::Dropped`] — the shaper emitted no glyph at all.
//!
//! The last three are all gaps. Only the second is what people mean by
//! "tofu", but a product that draws nothing where a letter belongs has the
//! same defect wearing a quieter coat, so all three are named.
//!
//! ## Why the detector does not simply ask `Fonts::has_glyph`
//!
//! `epaint::Fonts::has_glyph` compares the face a codepoint resolves to
//! against the family's replacement face. That is a *metadata* answer: it is
//! true for a face that has the codepoint in its `cmap` and no outline for
//! it, which is exactly what a colour-bitmap emoji face looks like to
//! `skrifa`'s outline reader. [`GlyphProbe`] instead lays the codepoint out
//! through the same `layout_job` path [`crate::text::GalleyShaper`] paints
//! from, and compares the *atlas region* the glyph was rasterised into
//! against the atlas region the replacement box occupies. Same texels means
//! the same picture on screen, whatever the metadata said.
//!
//! ## Why the probe validates itself
//!
//! A detector keyed on "is this the replacement box" is worthless if the
//! codepoint it uses to *learn* what the replacement box looks like turns
//! out to be covered by some installed face. [`GlyphProbe::new`] therefore
//! walks [`PROBE_CANDIDATES`] until it finds one whose ink matches the ink of
//! [`REPLACEMENT_BOX`], and refuses to build if none does. It also refuses if
//! `'A'` comes back as the replacement box, which would mean the whole font
//! stack is broken and every later answer meaningless.
//!
//! # The seam this module is shaped for
//!
//! SC-009 covers desktop *and* web. Everything in this module except
//! [`install_desktop_fallbacks`] is target-agnostic: the wasm parity lane
//! builds its own `egui::Context`, installs whatever faces the browser gives
//! it, and drives the same [`GlyphProbe`] over the same
//! [`script_samples`]. The desktop half is proven by
//! `tests/text_scripts.rs`; the web half is not proven by anything in this
//! crate today.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Galley, TextFormat};

// ---------------------------------------------------------------------------
// The font stack
// ---------------------------------------------------------------------------

/// The built-in faces that carry emoji, in the order egui installs them.
///
/// [`install_desktop_fallbacks`] splices its own faces in *ahead* of these
/// rather than appending to the end of the family. `emoji-icon-font` is an
/// experimental face that claims large ranges of the private-use planes and
/// some symbol blocks; letting it sit ahead of a real script face would let
/// it answer for codepoints it draws an icon for. Script faces first, emoji
/// last, is the order that makes the fallback chain explainable.
const BUILTIN_EMOJI_FACES: &[&str] = &["NotoEmoji-Regular", "emoji-icon-font"];

/// A face this crate will install if the host machine has it.
///
/// `candidates` is a preference-ordered list of absolute paths, not a
/// directory scan: a scan of `/usr/share/fonts` on a developer machine walks
/// several thousand files, and the answer it would give — "some face,
/// somewhere, covers this" — is worse than a named face, because it changes
/// with whatever the operator installed last.
#[derive(Clone, Copy, Debug)]
pub struct FallbackFace {
    /// The name the face is installed under in [`egui::FontDefinitions`].
    pub name: &'static str,

    /// Absolute paths to try, in order. The first that exists and carries an
    /// sfnt signature wins.
    pub candidates: &'static [&'static str],

    /// Which face inside a collection (`.ttc`) to take. `0` for a plain
    /// `.ttf`/`.otf`.
    pub index: u32,

    /// The [`script_samples`] keys this face is here to cover. Read by tests
    /// to say *which* script a missing face costs, and by
    /// [`FontStackReport::summary`].
    pub scripts: &'static [&'static str],

    /// A host without this face cannot draw its scripts at all, so its
    /// absence is a failure rather than a note.
    ///
    /// `false` marks a face that *improves* a script the built-in stack
    /// already draws something for. The emoji entry is the only one: egui
    /// bundles a monochrome emoji face, so emoji are legible without it —
    /// just not the ones added to Unicode after that bundled face was cut.
    /// See [`RECORDED_EMOJI_GAPS`].
    pub required: bool,
}

/// The desktop fallback faces, in fallback-chain order.
///
/// # Why absolute paths and not an embedded font
///
/// `include_bytes!` of the Noto faces these entries name is about 32 MB
/// (Noto Sans CJK alone is 16 MB), in a repository that ships no binary font
/// assets at all today. Reading the host's copy costs nothing at rest, works
/// for every product built on this crate, and — because
/// [`FontStackReport::absent`] names what was not found — fails loudly on a
/// machine that lacks them instead of quietly reverting to boxes. The trade
/// is portability: this list knows the layouts of a handful of platforms and
/// nothing about the rest, and it cannot work on wasm at all. See the agent
/// note for the alternatives weighed.
///
/// # Why the emoji entry names a monochrome face
///
/// Deliberate, and load-bearing. `NotoColorEmoji.ttf` is a CBDT/CBLC
/// colour-bitmap face and Segoe UI Emoji is a COLR one; egui 0.36.1
/// rasterises glyphs through `skrifa`'s *outline* reader, which finds no
/// outlines in either, so installing one would move emoji from *monochrome
/// and legible* to *claimed and invisible* — a [`GlyphOutcome::Blank`],
/// strictly worse than the box it was meant to replace. Colour emoji are
/// research item R4 of spec 003, they land in the overlay fork's
/// custom-glyph registry, and that fork does not exist in this workspace.
/// [`no_colour_emoji_face_is_named`] keeps a colour face out of this table
/// until it does.
///
/// [`no_colour_emoji_face_is_named`]: #
pub const DESKTOP_FALLBACKS: &[FallbackFace] = &[
    FallbackFace {
        name: "gorgon-fallback-cjk",
        candidates: &[
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            r"C:\Windows\Fonts\msgothic.ttc",
        ],
        index: 0,
        scripts: &["cjk"],
        required: true,
    },
    FallbackFace {
        name: "gorgon-fallback-arabic",
        candidates: &[
            "/usr/share/fonts/noto/NotoSansArabic-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSansArabic-Regular.ttf",
            "/System/Library/Fonts/Supplemental/GeezaPro.ttc",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        index: 0,
        scripts: &["arabic"],
        required: true,
    },
    FallbackFace {
        name: "gorgon-fallback-devanagari",
        candidates: &[
            "/usr/share/fonts/noto/NotoSansDevanagari-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSansDevanagari-Regular.ttf",
            "/System/Library/Fonts/Supplemental/DevanagariMT.ttc",
            r"C:\Windows\Fonts\Nirmala.ttf",
        ],
        index: 0,
        scripts: &["devanagari"],
        required: true,
    },
    FallbackFace {
        name: "gorgon-fallback-hebrew",
        candidates: &[
            "/usr/share/fonts/noto/NotoSansHebrew-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSansHebrew-Regular.ttf",
            "/System/Library/Fonts/Supplemental/Arial Hebrew.ttc",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        index: 0,
        scripts: &["hebrew"],
        required: true,
    },
    FallbackFace {
        name: "gorgon-fallback-emoji",
        candidates: &[
            "/usr/share/fonts/noto/NotoEmoji-Regular.ttf",
            "/usr/share/fonts/noto-emoji/NotoEmoji-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf",
            "/usr/share/fonts/opentype/noto/NotoEmoji-Regular.ttf",
            "/usr/share/fonts/google-noto-emoji/NotoEmoji-Regular.ttf",
            "/usr/share/fonts/TTF/NotoEmoji-Regular.ttf",
        ],
        index: 0,
        scripts: &["emoji"],
        required: false,
    },
];

/// Emoji the bundled monochrome face does not carry, measured — not guessed.
///
/// `epaint_default_fonts` bundles a build of Noto Emoji whose format-12
/// `cmap` holds 887 codepoints, which is roughly the Unicode 6.1 emoji set.
/// Anything standardised after that draws the replacement box, and there is
/// no way to fix it from inside this crate: egui 0.36.1 rasterises through
/// `skrifa`'s *outline* reader, and the colour-bitmap faces a desktop
/// actually ships (`NotoColorEmoji.ttf` is CBDT, Segoe UI Emoji is COLR)
/// carry no outlines to read.
///
/// So this list is a recorded gap, and `tests/text_scripts.rs` treats it as
/// a ceiling rather than a floor: a gap outside this list turns the suite
/// red, and installing a modern monochrome Noto Emoji — which
/// `gorgon-fallback-emoji` will pick up with no code change — is required to
/// close every one of them at once.
///
/// The codepoints, and why each is here rather than being a colour-emoji
/// question: U+1F642 (Unicode 7.0), U+1F914 (Unicode 8.0), U+1FAE0 (Unicode
/// 14.0). All three are ordinary monochrome-renderable pictographs. None of
/// them needs R4.
pub const RECORDED_EMOJI_GAPS: &[char] = &['\u{1F642}', '\u{1F914}', '\u{1FAE0}'];

/// Emoji from both sides of the bundled face's coverage boundary.
///
/// Used by the test that keeps [`RECORDED_EMOJI_GAPS`] honest. Deliberately
/// *not* part of [`script_samples`]: the samples are the corpus the zero-box
/// acceptance is measured over, and mixing a known gap into it would make
/// that acceptance meaningless.
#[must_use]
pub fn emoji_boundary_sample() -> &'static str {
    "🚀🎉✅🔥❤🙂🤔🫠"
}

/// What a font-stack install actually achieved.
///
/// Nothing here is inferred. `installed` names the file each face came from,
/// `absent` names the faces whose every candidate was missing, and
/// `unreadable` names files that existed and could not be used. A caller
/// that ignores this type gets the same font stack; a caller that reads it
/// can say *why* a script is going to draw boxes before it draws any.
#[derive(Clone, Debug, Default)]
pub struct FontStackReport {
    installed: Vec<(&'static str, PathBuf)>,
    absent: Vec<&'static FallbackFace>,
    unreadable: Vec<(PathBuf, String)>,
}

impl FontStackReport {
    /// The faces installed, each with the file it was read from.
    #[must_use]
    pub fn installed(&self) -> &[(&'static str, PathBuf)] {
        &self.installed
    }

    /// The faces for which no candidate path existed on this machine,
    /// required and optional alike.
    #[must_use]
    pub fn absent(&self) -> &[&'static FallbackFace] {
        &self.absent
    }

    /// The absent faces whose scripts nothing else can draw.
    #[must_use]
    pub fn absent_required(&self) -> Vec<&'static FallbackFace> {
        self.absent
            .iter()
            .copied()
            .filter(|face| face.required)
            .collect()
    }

    /// This face was installed.
    #[must_use]
    pub fn installed_face(&self, name: &str) -> bool {
        self.installed.iter().any(|(known, _)| *known == name)
    }

    /// Files that existed but could not be used, each with the reason.
    #[must_use]
    pub fn unreadable(&self) -> &[(PathBuf, String)] {
        &self.unreadable
    }

    /// Every *required* face was installed and nothing was unreadable.
    ///
    /// An absent optional face is a recorded note, not a failure — see
    /// [`FallbackFace::required`] — so it is listed by [`Self::summary`] and
    /// ignored here.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.absent_required().is_empty() && self.unreadable.is_empty()
    }

    /// A multi-line human summary, one line per face and one per problem.
    ///
    /// Written for a test failure message and for a startup log, which want
    /// the same thing: the file that was chosen, not just the count.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut out = String::new();
        for (name, path) in &self.installed {
            out.push_str(&format!("  installed {name} from {}\n", path.display()));
        }
        for face in &self.absent {
            out.push_str(&format!(
                "  ABSENT{} {} (covers {}): none of {} candidate path(s) exist:\n",
                if face.required { "" } else { " (optional)" },
                face.name,
                face.scripts.join(", "),
                face.candidates.len(),
            ));
            for candidate in face.candidates {
                out.push_str(&format!("    - {candidate}\n"));
            }
        }
        for (path, why) in &self.unreadable {
            out.push_str(&format!("  UNREADABLE {}: {why}\n", path.display()));
        }
        out
    }
}

/// The definitions [`install_desktop_fallbacks`] would install, and the
/// report describing them.
///
/// Split out from the install so the file-system half can be exercised
/// without an [`egui::Context`], and so a host that already builds its own
/// [`egui::FontDefinitions`] can take the fallback chain without taking
/// egui's defaults wholesale.
#[must_use]
pub fn desktop_fallback_definitions() -> (FontDefinitions, FontStackReport) {
    let mut definitions = FontDefinitions::default();
    let mut report = FontStackReport::default();

    for face in DESKTOP_FALLBACKS {
        match load_face(face) {
            Some((path, bytes)) => {
                let data = FontData {
                    font: std::borrow::Cow::Owned(bytes),
                    index: face.index,
                    tweak: egui::FontTweak::default(),
                };
                definitions
                    .font_data
                    .insert(face.name.to_owned(), Arc::new(data));
                report.installed.push((face.name, path));
            }
            None => report.absent.push(face),
        }
    }

    let names: Vec<String> = report
        .installed
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let chain = definitions.families.entry(family).or_default();
        let at = chain
            .iter()
            .position(|name| BUILTIN_EMOJI_FACES.contains(&name.as_str()))
            .unwrap_or(chain.len());
        for (offset, name) in names.iter().enumerate() {
            chain.insert(at + offset, name.clone());
        }
    }

    (definitions, report)
}

/// Install [`DESKTOP_FALLBACKS`] on top of egui's built-in faces.
///
/// egui applies new font definitions at the start of the next pass, so the
/// caller must drive one before the new faces answer anything — see
/// `egui::Context::set_fonts`. A shaper holding cached galleys must be
/// cleared too ([`crate::text::GalleyShaper::clear`]): a galley shaped
/// against the old stack is still a galley full of boxes.
pub fn install_desktop_fallbacks(ctx: &Context) -> FontStackReport {
    let (definitions, report) = desktop_fallback_definitions();
    ctx.set_fonts(definitions);
    report
}

/// Read one fallback face's first usable candidate.
///
/// Rejects anything without an sfnt signature *before* handing it to egui,
/// because `epaint::FontsImpl::new` panics on a face `skrifa` cannot parse —
/// a font stack that aborts the process because the operator has a stale
/// symlink in `/usr/share/fonts` is not a font stack.
fn load_face(face: &'static FallbackFace) -> Option<(PathBuf, Vec<u8>)> {
    for candidate in face.candidates {
        let path = Path::new(candidate);
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if !is_sfnt(&bytes) {
            continue;
        }
        return Some((path.to_path_buf(), bytes));
    }
    None
}

/// The file starts with one of the four signatures an sfnt container uses.
///
/// `0x00010000` (TrueType outlines), `OTTO` (CFF outlines), `true` (legacy
/// Apple TrueType), `ttcf` (a collection).
fn is_sfnt(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some(b"\x00\x01\x00\x00" | b"OTTO" | b"true" | b"ttcf")
    )
}

// ---------------------------------------------------------------------------
// The detector
// ---------------------------------------------------------------------------

/// The character epaint draws where a codepoint has no face: WHITE MEDIUM
/// SQUARE. Laying this out is how [`GlyphProbe`] learns what a box looks
/// like, and it works even on a stack where epaint fell back to its
/// second-choice replacement character, because the fallback happens inside
/// the same lookup.
pub const REPLACEMENT_BOX: char = '◻';

/// Codepoints tried, in order, to confirm the box the detector learned is
/// really what an *uncovered* codepoint produces.
///
/// All four are unassigned in Unicode 16 and outside every private-use area,
/// so no font is entitled to map them — but "entitled" is not "does", which
/// is why [`GlyphProbe::new`] checks rather than assumes, and why there is
/// more than one.
pub const PROBE_CANDIDATES: &[char] = &['\u{0378}', '\u{05FF}', '\u{2FE0}', '\u{1CFB}'];

/// What the shaper did with one codepoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphOutcome {
    /// A real glyph with ink in it.
    Rendered,

    /// The replacement box — no installed face has this codepoint.
    Tofu,

    /// A glyph was produced and it has no ink. Whitespace is reported as
    /// [`Self::Rendered`]; this variant means a *visible* character drew
    /// nothing, which is what a colour-bitmap face looks like to an outline
    /// rasteriser.
    Blank,

    /// The shaper emitted no glyph at all for this codepoint.
    Dropped,
}

impl GlyphOutcome {
    /// Anything other than [`Self::Rendered`].
    #[must_use]
    pub fn is_gap(self) -> bool {
        !matches!(self, Self::Rendered)
    }

    /// A short phrase naming the outcome, for a failure message.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rendered => "rendered",
            Self::Tofu => "tofu (the shaper drew the replacement box)",
            Self::Blank => "blank (a face claims it and draws no ink)",
            Self::Dropped => "dropped (the shaper emitted no glyph)",
        }
    }
}

/// The atlas region a glyph was rasterised into, in texels.
///
/// Two glyphs with the same region are literally the same picture: epaint
/// caches one allocation per (face, glyph id, metrics, subpixel bin) and
/// hands the same rectangle back. `min == max` means the glyph has no ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ink {
    min: [u16; 2],
    max: [u16; 2],
}

impl Ink {
    fn is_empty(self) -> bool {
        self.min == self.max
    }
}

/// Answers, for one codepoint under one [`FontId`], what the shaper drew.
///
/// Build one per (context, font) pair and reuse it: construction lays out
/// several probe strings, and every answer after that is one cached galley
/// lookup.
#[derive(Clone, Debug)]
pub struct GlyphProbe {
    font: FontId,
    tofu: Ink,
    learned_from: char,
}

impl GlyphProbe {
    /// Learn what a replacement box looks like under `font`, and prove the
    /// answer is usable.
    ///
    /// # Errors
    ///
    /// * The context has no fonts yet — drive one `Context::run`/`run_ui`
    ///   first.
    /// * [`REPLACEMENT_BOX`] produced no glyph, so there is nothing to
    ///   compare against.
    /// * No codepoint in [`PROBE_CANDIDATES`] came back as the box, which
    ///   means either the box is not what an uncovered codepoint produces on
    ///   this stack, or every probe candidate is covered. Either way the
    ///   detector would be blind, so it refuses to exist.
    /// * `'A'` came back as the box, which means the font stack is broken
    ///   and no later answer would mean anything.
    pub fn new(ctx: &Context, font: FontId) -> Result<Self, String> {
        let tofu = ink(ctx, &font, REPLACEMENT_BOX).ok_or_else(|| {
            format!("the replacement box {REPLACEMENT_BOX:?} shaped to no glyph under {font:?}")
        })?;
        if tofu.is_empty() {
            return Err(format!(
                "the replacement box {REPLACEMENT_BOX:?} shaped to an inkless glyph under \
                 {font:?}; there is no box to recognise"
            ));
        }
        if ink(ctx, &font, 'A') == Some(tofu) {
            return Err(format!(
                "'A' shaped to the replacement box under {font:?}: the font stack has no Latin \
                 face and every answer from this probe would be meaningless"
            ));
        }
        let learned_from = PROBE_CANDIDATES
            .iter()
            .copied()
            .find(|c| ink(ctx, &font, *c) == Some(tofu))
            .ok_or_else(|| {
                let tried: Vec<String> = PROBE_CANDIDATES
                    .iter()
                    .map(|c| format!("U+{:04X}", *c as u32))
                    .collect();
                format!(
                    "no probe codepoint ({}) shaped to the replacement box under {font:?}: this \
                     detector cannot tell a box from a glyph and must not be used",
                    tried.join(", ")
                )
            })?;
        Ok(Self {
            font,
            tofu,
            learned_from,
        })
    }

    /// The font this probe answers for.
    #[must_use]
    pub fn font(&self) -> &FontId {
        &self.font
    }

    /// The unassigned codepoint whose ink taught this probe what a box is.
    #[must_use]
    pub fn learned_from(&self) -> char {
        self.learned_from
    }

    /// What the shaper does with `c` on its own.
    ///
    /// Laid out alone rather than read out of a longer run: epaint bins glyph
    /// rasterisation into four subpixel positions, so the *same* box drawn at
    /// two different x offsets occupies two different atlas regions. A run of
    /// one always starts at x = 0 and therefore always lands in bin zero,
    /// which is what makes the comparison in here exact instead of
    /// approximate.
    #[must_use]
    pub fn outcome(&self, ctx: &Context, c: char) -> GlyphOutcome {
        match ink(ctx, &self.font, c) {
            None => {
                if c.is_whitespace() {
                    GlyphOutcome::Rendered
                } else {
                    GlyphOutcome::Dropped
                }
            }
            Some(found) if found == self.tofu => GlyphOutcome::Tofu,
            Some(found) if found.is_empty() => {
                if c.is_whitespace() {
                    GlyphOutcome::Rendered
                } else {
                    GlyphOutcome::Blank
                }
            }
            Some(_) => GlyphOutcome::Rendered,
        }
    }

    /// Classify every non-whitespace codepoint of `sample`.
    #[must_use]
    pub fn coverage(&self, ctx: &Context, script: &str, sample: &str) -> ScriptCoverage {
        let mut outcomes = Vec::new();
        let mut seen = Vec::new();
        for c in sample.chars().filter(|c| !c.is_whitespace()) {
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            outcomes.push((c, self.outcome(ctx, c)));
        }
        ScriptCoverage {
            script: script.to_owned(),
            outcomes,
        }
    }

    /// Classify a galley that was already shaped by production code.
    ///
    /// Two independent readings, unioned, because they fail differently:
    ///
    /// 1. every codepoint of the galley's own text is re-probed through
    ///    [`Self::outcome`], which catches a box at any subpixel position;
    /// 2. every glyph the galley actually holds is compared to the box's
    ///    atlas region directly, which catches a box the per-codepoint
    ///    reading would have called rendered — a disagreement that would mean
    ///    a defect in this module.
    ///
    /// A codepoint that produced no glyph *of its own* is **not** a gap
    /// here. Harfrust attributes every glyph of a shaped cluster to the
    /// cluster's first character, so a Devanagari vowel sign is drawn and
    /// then reported under the consonant it hangs off. Reading absence as a
    /// dropped glyph flagged four correctly-drawn marks in
    /// `देवनागरी`. Reading 1 above already catches a mark the stack really
    /// cannot draw, because a lone uncovered combining mark laid out on its
    /// own yields no glyph at all — [`GlyphOutcome::Dropped`].
    #[must_use]
    pub fn galley_outcomes(&self, ctx: &Context, galley: &Galley) -> Vec<(char, GlyphOutcome)> {
        let mut drawn: Vec<(char, Ink)> = Vec::new();
        for row in &galley.rows {
            for glyph in &row.row.glyphs {
                drawn.push((
                    glyph.chr,
                    Ink {
                        min: glyph.uv_rect.min,
                        max: glyph.uv_rect.max,
                    },
                ));
            }
        }

        let mut out: Vec<(char, GlyphOutcome)> = Vec::new();
        let mut seen: Vec<char> = Vec::new();
        for c in galley.job.text.chars().filter(|c| !c.is_whitespace()) {
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            let mut outcome = self.outcome(ctx, c);
            for (chr, found) in &drawn {
                if *chr == c && *found == self.tofu {
                    outcome = GlyphOutcome::Tofu;
                }
            }
            out.push((c, outcome));
        }
        out
    }
}

/// The atlas region `c` rasterises into when laid out on its own, or `None`
/// if the shaper produced no glyph for it.
fn ink(ctx: &Context, font: &FontId, c: char) -> Option<Ink> {
    let job = LayoutJob::single_section(
        c.to_string(),
        TextFormat {
            font_id: font.clone(),
            color: Color32::PLACEHOLDER,
            ..TextFormat::default()
        },
    );
    let galley = ctx.fonts_mut(|fonts| fonts.layout_job(job));
    let glyph = galley.rows.iter().find_map(|row| row.row.glyphs.first())?;
    Some(Ink {
        min: glyph.uv_rect.min,
        max: glyph.uv_rect.max,
    })
}

/// One script's coverage under one font stack.
#[derive(Clone, Debug)]
pub struct ScriptCoverage {
    script: String,
    outcomes: Vec<(char, GlyphOutcome)>,
}

impl ScriptCoverage {
    /// The script name this coverage was measured for.
    #[must_use]
    pub fn script(&self) -> &str {
        &self.script
    }

    /// Distinct non-whitespace codepoints measured.
    #[must_use]
    pub fn total(&self) -> usize {
        self.outcomes.len()
    }

    /// Codepoints that drew a real glyph.
    #[must_use]
    pub fn covered(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|(_, outcome)| !outcome.is_gap())
            .count()
    }

    /// Every codepoint that did not draw a real glyph, with what happened.
    #[must_use]
    pub fn gaps(&self) -> Vec<(char, GlyphOutcome)> {
        self.outcomes
            .iter()
            .copied()
            .filter(|(_, outcome)| outcome.is_gap())
            .collect()
    }

    /// The machine-readable line this measurement exists to print:
    /// `GLYPH <script>=<covered>/<total>`.
    #[must_use]
    pub fn report_line(&self) -> String {
        format!("GLYPH {}={}/{}", self.script, self.covered(), self.total())
    }

    /// A failure message naming the script and every uncovered codepoint.
    ///
    /// Empty when there is nothing to report, so a caller can join several
    /// scripts' messages and test the result for emptiness.
    #[must_use]
    pub fn gap_report(&self) -> String {
        let gaps = self.gaps();
        if gaps.is_empty() {
            return String::new();
        }
        let mut out = format!(
            "script {:?}: {} of {} codepoint(s) did not render:\n",
            self.script,
            gaps.len(),
            self.total(),
        );
        for (c, outcome) in gaps {
            out.push_str(&format!(
                "  - U+{:04X} {:?} -> {}\n",
                c as u32,
                c,
                outcome.as_str()
            ));
        }
        out
    }
}

/// The scripts SC-009 is written about, and the run that proves each.
///
/// Shared by the desktop test and the wasm parity lane so both halves of
/// SC-009 measure the same corpus. Whitespace is present because these are
/// meant to be laid out as real runs; [`GlyphProbe::coverage`] skips it when
/// counting, since an inkless space is correct rather than missing.
///
/// The Devanagari sample carries combining vowel signs on purpose: they are
/// the class epaint's layout drops rather than boxes when no face has them,
/// which is a gap that a tofu-only detector would never see.
///
/// The emoji sample stays inside the bundled monochrome face's coverage. That
/// is not a convenience: this corpus is what the zero-box acceptance is
/// measured over, so putting a codepoint in it that the shipped stack is
/// known to lack would turn the acceptance into a permanently failing test
/// or, worse, into a licensed exception. The known-missing emoji are
/// enumerated in [`RECORDED_EMOJI_GAPS`] and measured through
/// [`emoji_boundary_sample`] instead.
#[must_use]
pub fn script_samples() -> &'static [(&'static str, &'static str)] {
    &[
        ("latin", "Ünïcödé quartz jugs — ABC xyz 0123"),
        ("cjk", "日本語のテキスト 中文字体 한국어"),
        ("arabic", "نص عربي للاختبار"),
        ("devanagari", "देवनागरी लिपि"),
        ("hebrew", "טקסט עברי"),
        ("emoji", "🚀🎉✅🔥❤"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context that has been through one pass, so its fonts exist.
    fn headless() -> Context {
        let ctx = Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        ctx
    }

    #[test]
    fn the_signature_check_accepts_sfnt_and_rejects_everything_else() {
        assert!(is_sfnt(b"\x00\x01\x00\x00rest"));
        assert!(is_sfnt(b"OTTOrest"));
        assert!(is_sfnt(b"ttcfrest"));
        assert!(is_sfnt(b"truerest"));
        assert!(!is_sfnt(b"<!DOCTYPE html>"));
        assert!(!is_sfnt(b"\x7fELF"));
        assert!(!is_sfnt(b"abc"));
        assert!(!is_sfnt(b""));
    }

    /// The fallback faces go in ahead of the emoji faces in both families, so
    /// `emoji-icon-font` can never answer for a script codepoint.
    #[test]
    fn fallbacks_are_spliced_in_ahead_of_the_emoji_faces() {
        let (definitions, report) = desktop_fallback_definitions();
        let installed: Vec<&str> = report.installed().iter().map(|(n, _)| *n).collect();
        if installed.is_empty() {
            // Nothing to order. The install test in `tests/text_scripts.rs`
            // is the one that refuses an incomplete stack; this unit test
            // only has something to say when a face was found.
            return;
        }
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let chain = &definitions.families[&family];
            let first_emoji = chain
                .iter()
                .position(|n| BUILTIN_EMOJI_FACES.contains(&n.as_str()))
                .expect("egui's defaults install the emoji faces");
            for name in &installed {
                let at = chain
                    .iter()
                    .position(|n| n == name)
                    .unwrap_or_else(|| panic!("{name} missing from {family:?}: {chain:?}"));
                assert!(
                    at < first_emoji,
                    "{name} must precede the emoji faces in {family:?}: {chain:?}"
                );
            }
        }
    }

    /// No entry names a colour-emoji face. See [`DESKTOP_FALLBACKS`] for
    /// why: CBDT/COLR faces carry no outlines, egui 0.36.1 reads only
    /// outlines, and the result is an invisible glyph — worse than the box.
    /// Colour emoji wait on spec 003's R4 and the overlay fork.
    #[test]
    fn no_colour_emoji_face_is_named() {
        const COLOUR_FACES: &[&str] = &[
            "NotoColorEmoji",
            "AppleColorEmoji",
            "Apple Color Emoji",
            "seguiemj",
            "TwemojiMozilla",
        ];
        for face in DESKTOP_FALLBACKS {
            for candidate in face.candidates {
                for colour in COLOUR_FACES {
                    assert!(
                        !candidate.contains(colour),
                        "{} names the colour face {colour} at {candidate}. egui 0.36.1 \
                         rasterises outlines only, so that face draws nothing at all. \
                         Colour emoji are R4 (overlay fork), which does not exist here.",
                        face.name
                    );
                }
            }
        }
    }

    /// Exactly one optional face, it is the emoji one, and every other face
    /// is required. An optional face is the only kind whose absence this
    /// crate tolerates, so the set has to stay small and named.
    #[test]
    fn the_emoji_face_is_the_only_optional_one() {
        let optional: Vec<&str> = DESKTOP_FALLBACKS
            .iter()
            .filter(|face| !face.required)
            .map(|face| face.name)
            .collect();
        assert_eq!(optional, ["gorgon-fallback-emoji"]);
    }

    /// The recorded emoji gaps are recorded *against* the boundary sample,
    /// so neither can drift away from the other unnoticed.
    #[test]
    fn every_recorded_emoji_gap_is_in_the_boundary_sample() {
        for c in RECORDED_EMOJI_GAPS {
            assert!(
                emoji_boundary_sample().contains(*c),
                "U+{:04X} is recorded as a gap but nothing measures it",
                *c as u32
            );
        }
        let covered = emoji_boundary_sample()
            .chars()
            .filter(|c| !RECORDED_EMOJI_GAPS.contains(c))
            .count();
        assert!(
            covered >= RECORDED_EMOJI_GAPS.len(),
            "the boundary sample must exercise both sides of the boundary"
        );
    }

    /// The probe refuses to exist rather than answer blindly.
    #[test]
    fn the_probe_learns_a_box_from_an_unassigned_codepoint() {
        let ctx = headless();
        let probe = GlyphProbe::new(&ctx, FontId::proportional(14.0))
            .expect("egui's default stack has a replacement box and Latin letters");
        assert!(
            PROBE_CANDIDATES.contains(&probe.learned_from()),
            "the probe must learn from its own candidate list"
        );
    }

    /// The detector fires. A codepoint no built-in face carries must come
    /// back as a box, or every green run this module produces is worthless.
    #[test]
    fn the_probe_calls_an_uncovered_codepoint_tofu() {
        let ctx = headless();
        let probe =
            GlyphProbe::new(&ctx, FontId::proportional(14.0)).expect("probe under default fonts");
        assert_eq!(
            probe.outcome(&ctx, '\u{13000}'),
            GlyphOutcome::Tofu,
            "U+13000 EGYPTIAN HIEROGLYPH A001 is in none of egui's built-in faces"
        );
        assert_eq!(probe.outcome(&ctx, 'A'), GlyphOutcome::Rendered);
        assert_eq!(probe.outcome(&ctx, ' '), GlyphOutcome::Rendered);
    }

    /// The corpus covers what SC-009 asks for, and names the scripts the
    /// fallback table claims to serve.
    #[test]
    fn the_corpus_covers_the_scripts_the_fallback_table_serves() {
        let names: Vec<&str> = script_samples().iter().map(|(name, _)| *name).collect();
        for wanted in ["latin", "cjk", "arabic", "devanagari", "hebrew", "emoji"] {
            assert!(names.contains(&wanted), "{wanted} missing from {names:?}");
        }
        for face in DESKTOP_FALLBACKS {
            for script in face.scripts {
                assert!(
                    names.contains(script),
                    "{} covers {script:?}, which no sample exercises",
                    face.name
                );
            }
        }
    }

    /// `GLYPH <script>=<covered>/<total>` — the shape the gate greps for.
    #[test]
    fn the_report_line_is_the_shape_the_gate_reads() {
        let coverage = ScriptCoverage {
            script: "cjk".to_owned(),
            outcomes: vec![
                ('日', GlyphOutcome::Rendered),
                ('本', GlyphOutcome::Rendered),
                ('語', GlyphOutcome::Tofu),
            ],
        };
        assert_eq!(coverage.report_line(), "GLYPH cjk=2/3");
        assert_eq!(coverage.total(), 3);
        let report = coverage.gap_report();
        assert!(report.contains("cjk"), "{report}");
        assert!(report.contains("U+8A9E"), "{report}");
        assert!(report.contains("tofu"), "{report}");
    }

    /// A clean measurement produces no gap report at all, so several scripts'
    /// reports can be joined and tested for emptiness.
    #[test]
    fn a_clean_measurement_reports_nothing() {
        let coverage = ScriptCoverage {
            script: "latin".to_owned(),
            outcomes: vec![('A', GlyphOutcome::Rendered)],
        };
        assert_eq!(coverage.report_line(), "GLYPH latin=1/1");
        assert!(coverage.gap_report().is_empty());
        assert!(coverage.gaps().is_empty());
    }
}
