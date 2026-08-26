//! Painting a petrified frame through `egui::Painter`.
//!
//! Petra decides where everything goes and in what order; this module only
//! draws. One egui layer holds the whole frame and shapes are added in Petra's
//! own paint order, so egui's z-ordering never gets a say — `Ui` is not used,
//! and neither is egui's layer ordering beyond the single layer this paints
//! into.
//!
//! # Hosted content (FR-059, T080/T081)
//!
//! Two of Petra's twelve node kinds declare content this crate cannot draw on
//! its own — an image names a source, a custom node names a registered
//! painter — and both were, until now, unconditionally reported in
//! [`PaintReport::undrawn`]. [`CustomPainters`] and
//! [`crate::image::ImageSources`] are how a host closes that gap: registering
//! a painter or a source makes the matching content draw into the *same*
//! layer, in the *same* Petra-decided paint order, as everything else in this
//! file — a registered painter draws through the `&Painter` this module
//! hands it, never a layer or a `Ui` of its own. What does not change is the
//! honesty: a name with nothing registered, and a registered painter that
//! draws nothing, land in `undrawn` exactly alike. See
//! [`paint_frame_with_hosts`].

use std::collections::{BTreeMap, BTreeSet};

use egui::{Color32, Painter, Rgba, Stroke};
use gorgon_petra::frame::{PaintContent, PetrifiedFrame, Placement, round_rect};
use gorgon_petra::geom::{Rect as PetraRect, Scale};
use gorgon_petra::layout::TextRequest;
use gorgon_petra::token::value::CoverageValue;
use gorgon_petra::token::{
    DerivedState, FocusRing, InteractionRank, MotionValue, SHADOW_GEOMETRY, Silhouette,
    ThemeSnapshot, TokenName, TokenValue, TypographyValue, base_slot, resolve_slot, resolve_state,
};

use crate::host::{COVERAGE_TOKEN, FALLBACK_COVERAGE, coverage_plan};
use crate::image::ImageSources;
use crate::text::GalleyShaper;

/// Token slot painted as a filled rect behind a node.
pub const BACKGROUND_SLOT: &str = "background";
/// Token slot painted as a one-unit outline around a node.
pub const BORDER_SLOT: &str = "border";
/// Token slot used for a node's text.
pub const FOREGROUND_SLOT: &str = "foreground";
/// Token slot painted as the corner radius of a node's background and
/// border. Resolves through a `shape.*` token (FR-053, C17); a node that
/// binds no `radius` paints square corners, the same default it had before
/// this slot existed.
pub const RADIUS_SLOT: &str = "radius";
/// Token slot naming the outline family a node's background and border are
/// drawn as. Resolves through a `shape.silhouette-*` token
/// ([`gorgon_petra::token::Silhouette`]); a node that binds no `silhouette`
/// paints the rect it always did.
///
/// This is the slot that makes [`RADIUS_SLOT`] finite. A corner radius on a
/// square box spans a sharp square to a full disc and stops there, so a
/// component with four figures to express — `component::status`, whose
/// `StatusShape` has exactly four variants — had two pictures for four
/// meanings, and at the radii the library actually bound, the two differed
/// by 0.414 logical units: under one device pixel at scale 1.0. FR-015 says
/// meaning must never rest on colour alone, and that shape channel was not
/// reaching the screen.
pub const SILHOUETTE_SLOT: &str = "silhouette";

/// Token consulted for the elevation shadow cast behind a node's fill.
///
/// Bound to one of the two names [`gorgon_petra::token::SHADOW_GEOMETRY`]
/// carries. The token supplies only the *colour*; the offset, blur and spread
/// come from that table, because they do not change between themes and a
/// theme is the wrong place to keep a fact that is the same in both.
pub const SHADOW_SLOT: &str = "shadow";

/// Every token slot this painter knows how to use. Anything else a node binds
/// lands in [`PaintReport::unknown_slots`] rather than being dropped on the
/// floor.
const KNOWN_SLOTS: &[&str] = &[
    BACKGROUND_SLOT,
    BORDER_SLOT,
    FOREGROUND_SLOT,
    RADIUS_SLOT,
    SHADOW_SLOT,
    SILHOUETTE_SLOT,
];
/// Token consulted for text with no declared `foreground`.
pub const DEFAULT_TEXT_TOKEN: &str = "text.primary";

/// Resolves a token name to the value a theme assigns it.
///
/// A trait rather than a concrete theme so the painter has no opinion about
/// where tokens come from: the shipped implementation reads a
/// [`ThemeSnapshot`], and a test can supply four colours and no theme at all.
///
/// [`TokenSource::value`] is the whole vocabulary — every kind a theme can
/// assign — and every other method here is one typed reading of it. That
/// breadth is what FR-059 means by "the painter receives the theme
/// snapshot": a hosted painter handed one of these reads `spacing.*`,
/// `typography.*` and `motion.*` off the same theme the rest of the frame
/// painted with, instead of inventing a second set of numbers beside the
/// design system. Two slices of the snapshot — colour and shape — is what
/// this trait used to be, and a sparkline that wants to step its gridlines
/// on the 4-unit ramp could not reach the ramp.
///
/// `value` is defaulted rather than required, so a fixture that only ever
/// answered [`TokenSource::color`] keeps compiling and keeps answering
/// exactly what it answered before: `None` to everything else.
pub trait TokenSource {
    /// The colour for `token`, or `None` when the theme has no such colour.
    ///
    /// The one required method, and the only one that is not a plain reading
    /// of [`TokenSource::value`]: a colour crosses out of Petra's linear
    /// light into egui's gamma-encoded [`Color32`], and where that
    /// conversion happens is a decision the source gets to make.
    fn color(&self, token: &str) -> Option<Color32>;

    /// The raw value `token` resolves to, whatever kind it is, or `None`
    /// when this source does not define it.
    ///
    /// Defaulted to `None` so every implementor that predates the wider
    /// vocabulary keeps compiling. Overriding this one method is what lights
    /// up every typed reader below, which is why [`ThemeSnapshot`] overrides
    /// it and a four-colour fixture does not have to.
    fn value(&self, _token: &str) -> Option<&TokenValue> {
        None
    }

    /// The corner radius, in logical units, `token` resolves to, or `None`
    /// when the source has no such shape.
    fn radius(&self, token: &str) -> Option<f32> {
        match self.value(token)? {
            TokenValue::Shape(shape) => Some(shape.corner_radius),
            _ => None,
        }
    }

    /// The outline family `token` resolves to, or `None` when the source has
    /// no such figure.
    fn silhouette(&self, token: &str) -> Option<Silhouette> {
        match self.value(token)? {
            TokenValue::Silhouette(figure) => Some(*figure),
            _ => None,
        }
    }

    /// The gap, in logical units, `token` resolves to, or `None` when the
    /// source has no such spacing.
    ///
    /// This is the reader the old narrowing cost. A hosted painter that
    /// wants its own geometry to sit on the same ramp as the frame around it
    /// had no way to read the ramp, so it spelled literals — inside
    /// `petra-egui/src`, which the `literal-style` lane does not read.
    fn spacing(&self, token: &str) -> Option<f32> {
        match self.value(token)? {
            TokenValue::Spacing(units) => Some(*units),
            _ => None,
        }
    }

    /// The text style `token` resolves to, or `None` when the source has no
    /// such style. A hosted painter that labels its own axis picks its size
    /// off the type ramp here rather than off a number of its own.
    fn typography(&self, token: &str) -> Option<TypographyValue> {
        match self.value(token)? {
            TokenValue::Typography(style) => Some(*style),
            _ => None,
        }
    }

    /// The transition timing `token` resolves to, or `None` when the source
    /// has no such motion.
    fn motion(&self, token: &str) -> Option<MotionValue> {
        match self.value(token)? {
            TokenValue::Motion(timing) => Some(*timing),
            _ => None,
        }
    }

    /// The glyph coverage curve `token` resolves to, or `None` when the
    /// source has no such value — a `TokenSource` whose vocabulary predates
    /// [`crate::host::COVERAGE_TOKEN`], or a fixture (most of this module's
    /// own tests) that only ever implements [`TokenSource::color`]. The text
    /// paint path (below) falls back to
    /// [`crate::host::FALLBACK_COVERAGE`] rather than treating `None` here
    /// as "paint once and stop asking" — the same defensive posture
    /// [`crate::host::bind_glyph_coverage`] takes for the atlas half of this
    /// same token.
    fn coverage(&self, token: &str) -> Option<CoverageValue> {
        match self.value(token)? {
            TokenValue::Coverage(curve) => Some(*curve),
            _ => None,
        }
    }
}

impl TokenSource for ThemeSnapshot {
    fn color(&self, token: &str) -> Option<Color32> {
        let name = TokenName::new(token).ok()?;
        match self.value(&name)? {
            // Petra keeps colours in linear light so animation can interpolate
            // them correctly; egui's `Rgba` is the same space, and the
            // conversion to `Color32` is egui's own gamma encode rather than a
            // second transfer function written here that could disagree.
            TokenValue::Color(c) => Some(Color32::from(Rgba::from_rgba_unmultiplied(
                c.r, c.g, c.b, c.a,
            ))),
            _ => None,
        }
    }

    // `value` is the whole of what this impl has to say; every typed reader
    // the trait defines could ride on the default bodies from here. The
    // three below still delegate instead, because `gorgon-petra` owns an
    // accessor for each and its doc is explicit that the kind match is
    // written once, there. The defaults exist for sources that are not a
    // snapshot at all and have no petra-side accessor to borrow.
    fn value(&self, token: &str) -> Option<&TokenValue> {
        let name = TokenName::new(token).ok()?;
        ThemeSnapshot::value(self, &name)
    }

    fn radius(&self, token: &str) -> Option<f32> {
        let name = TokenName::new(token).ok()?;
        self.corner(&name)
    }

    fn silhouette(&self, token: &str) -> Option<Silhouette> {
        let name = TokenName::new(token).ok()?;
        ThemeSnapshot::silhouette(self, &name)
    }

    fn spacing(&self, token: &str) -> Option<f32> {
        let name = TokenName::new(token).ok()?;
        ThemeSnapshot::spacing(self, &name)
    }
}

/// Resolve `token` to a colour, or record it unresolved and return `None`.
///
/// This is the "look up a token, or note that it did not resolve" idiom
/// shared by the focus-ring bands and the background/border fills: all three
/// skip drawing on a miss and none of them guess. Text does not use this —
/// it draws [`Color32::PLACEHOLDER`] and keeps going, because a blank paint
/// is worse than a visibly wrong one for text, and folding that fourth site
/// in here would erase the difference on purpose.
fn resolve_or_record(
    colors: &dyn TokenSource,
    token: &str,
    report: &mut PaintReport,
) -> Option<Color32> {
    let color = colors.color(token);
    if color.is_none() {
        report.unresolved_tokens.insert(token.to_owned());
    }
    color
}

/// Resolve `token` to a corner radius, recording it unresolved on a miss.
///
/// Falls back to `0.0` — square corners — on a miss rather than returning
/// `None` for the caller to fall back on: unlike a fill or a stroke, which
/// can simply not be drawn, a corner radius is always applied to a rect that
/// is going to be painted regardless. `0.0` is also the exact corner every
/// node had before this slot existed, so an unresolved `radius` degrades to
/// the old picture rather than to a new failure mode.
fn resolve_radius_or_record(
    colors: &dyn TokenSource,
    token: &str,
    report: &mut PaintReport,
) -> f32 {
    let radius = colors.radius(token);
    if radius.is_none() {
        report.unresolved_tokens.insert(token.to_owned());
    }
    radius.unwrap_or(0.0)
}

/// Resolve `token` to an outline family, recording it unresolved on a miss.
///
/// Falls back to [`Silhouette::Rect`] for the same reason
/// [`resolve_radius_or_record`] falls back to `0.0`: the figure is always
/// applied to a node that is going to be painted regardless, and the rect is
/// the exact figure every node drew before this slot existed. The miss is in
/// the report either way, which is the part that stops a mistyped silhouette
/// from quietly becoming a square — the failure mode this whole slot was
/// added to close.
fn resolve_silhouette_or_record(
    colors: &dyn TokenSource,
    token: &str,
    report: &mut PaintReport,
) -> Silhouette {
    let figure = colors.silhouette(token);
    if figure.is_none() {
        report.unresolved_tokens.insert(token.to_owned());
    }
    figure.unwrap_or_default()
}

/// The closed outline `figure` traces inside `rect`, or `None` for
/// [`Silhouette::Rect`] — which is not a polygon here but egui's own rounded
/// rect, so that a corner radius keeps working and a square box at
/// `shape.corner-full` keeps painting the disc it always did.
///
/// The engine names the figure and this function is the only place its
/// geometry is written down. Both polygons are inscribed in the node's rect
/// and wound clockwise in egui's y-down space.
///
/// Why these two and not, say, a pentagon: a triangle and a diamond are the
/// figures a corner radius cannot reach, they stay separable from each other
/// and from a box at the 10x10 the status marker actually paints at, and
/// they are the silhouettes `gorgon_petra::token::StatusShape` already
/// names. A figure whose outline is close to its bounding box would add a
/// vocabulary entry without adding a channel.
fn silhouette_points(figure: Silhouette, rect: egui::Rect) -> Option<Vec<egui::Pos2>> {
    let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
    let (cx, cy) = (rect.center().x, rect.center().y);
    match figure {
        Silhouette::Rect => None,
        // Apex at the top edge's midpoint, base along the bottom edge.
        Silhouette::Triangle => Some(vec![egui::pos2(cx, t), egui::pos2(r, b), egui::pos2(l, b)]),
        // One vertex at the midpoint of each edge.
        Silhouette::Diamond => Some(vec![
            egui::pos2(cx, t),
            egui::pos2(r, cy),
            egui::pos2(cx, b),
            egui::pos2(l, cy),
        ]),
    }
}

/// What one paint pass did, and what it could not do.
///
/// The counts are the accounting that makes a missing panel visible. The
/// digest hashes placements, not pixels, so a placement that never reached a
/// painter leaves every gate green and the picture wrong; this report is how
/// the host notices.
///
/// The first version of this struct counted `visited`, incremented
/// unconditionally at the top of the paint loop over a list zipped from two
/// vectors that are always pushed together — so `is_complete()` returned
/// `true` in every reachable pass, while this doc comment, the crate's
/// invariant companion, and an Agent Note all said it detected an undrawn
/// panel. The categories below exist so the question "did this placement
/// actually produce anything?" has an answer that can be *no*:
///
/// * `drawn` — emitted at least one shape.
/// * `skipped_clipped` — clipped to nothing, which is the correct outcome for
///   a node scrolled out of view or behind a closed surface.
/// * `empty` — declared no paint content at all. A bare `Stack` is a position
///   for its children and nothing else; it is not a failure.
/// * `silent` — **declared content and emitted nothing**. This is the failure
///   the accounting is for: an image with no loader, a custom kind with no
///   painter, or a fill whose token did not resolve.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaintReport {
    /// Placements in the frame.
    pub placements: usize,
    /// Placements that emitted at least one shape.
    pub drawn: usize,
    /// Placements clipped to nothing, deliberately not drawn.
    pub skipped_clipped: usize,
    /// Placements that declared nothing to paint.
    pub empty: usize,
    /// Placements that declared content and emitted nothing.
    pub silent: usize,
    /// Filled rects drawn.
    pub fills: usize,
    /// Text runs drawn.
    pub texts: usize,
    /// Images drawn through a resolved [`crate::image::ImageSources`] entry.
    pub images: usize,
    /// Custom nodes drawn through a registered [`CustomPainters`] entry.
    pub customs: usize,
    /// Focus rings drawn: focused placements that got at least one band.
    pub focus_rings: usize,
    /// **Focused placements that got no ring at all.** Keyboard operation
    /// nobody can see is the defect the ring exists to close, so this is a
    /// failed pass and not a note in a set: it is the one way a frame can be
    /// fully drawn and still leave the operator blind.
    pub blind_focus: usize,
    /// Token names this pass asked for and could not resolve, sorted: a
    /// colour, a corner radius, or a typography style. Not only colours —
    /// the typography half was added because a style name that resolved to
    /// nothing used to fall back to the default size in silence, which is
    /// how a whole declared type ramp painted at one size with this set
    /// empty and every gate green.
    pub unresolved_tokens: BTreeSet<String>,
    /// Content kinds this pass has no painter for, sorted.
    pub undrawn: BTreeSet<String>,
    /// Token slots this pass does not know how to use, sorted. `radius` is
    /// no longer one of them (FR-053, C17) — a node binding it gets a real
    /// corner, and a genuinely unrecognised slot name still lands here
    /// rather than being dropped silently.
    pub unknown_slots: BTreeSet<String>,
    /// The frame's placement and content arrays disagreed in length. They are
    /// public fields on [`gorgon_petra::frame::PetrifiedFrame`] and are zipped
    /// to pair them, and a zip silently truncates to the shorter of the two —
    /// so the tail would vanish from the picture with nothing to show for it.
    pub desynced: bool,
}

impl PaintReport {
    /// Whether the pass accounted for every placement and left none silent.
    ///
    /// Note what this deliberately is *not*: a count of placements the loop
    /// touched. That number is a function of the loop, not of the drawing, and
    /// asserting on it proved exactly nothing.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.desynced
            && self.silent == 0
            && self.blind_focus == 0
            && self.drawn + self.skipped_clipped + self.empty == self.placements
    }
}

/// What one placement produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// Emitted at least one shape.
    Drawn,
    /// Declared nothing to paint.
    Empty,
    /// Declared content and emitted nothing.
    Silent,
}

/// What a registered custom painter is given to draw with.
///
/// Petra has already resolved position, clipping, fade and scale for this
/// placement by the time a custom painter runs, so this is everything a
/// painter needs to draw *content* and nothing it would need to negotiate
/// *layout* — the same division of labour [`crate::image`] draws for images.
///
/// `clip` and `opacity` are already in force on the `&Painter` a painter is
/// called with (`Painter::clip_rect`, `Painter::opacity`) — every shape drawn
/// through it is clipped and faded automatically. They are repeated here so
/// a painter that needs the *number* — to skip expensive work at zero
/// opacity, say, rather than pay for it and have it faded away — does not
/// have to reach back into the painter to get it.
pub struct CustomPaintCtx<'a> {
    /// This placement's rect, already snapped to the device-pixel grid.
    pub rect: egui::Rect,
    /// The clip rect already in force on the painter this ctx accompanies.
    pub clip: egui::Rect,
    /// This placement's opacity, already clamped to `0.0..=1.0` and already
    /// in force on the painter this ctx accompanies.
    pub opacity: f32,
    /// The device scale this frame was placed at.
    pub scale: Scale,
    /// The theme snapshot this pass is painting with, read through
    /// [`TokenSource`] so a painter has no opinion about where the theme
    /// comes from.
    ///
    /// FR-059's fifth item, and the whole snapshot rather than a slice of
    /// it: colour, shape, spacing, typography and motion all resolve here,
    /// so a hosted painter spaces and sizes itself on the same design system
    /// as the frame around it instead of carrying literals of its own.
    pub tokens: &'a dyn TokenSource,
}

/// A host-registered painter for one `PaintContent.custom` name.
///
/// Returns whether it drew anything. **A painter that draws nothing must
/// return `false`** — a sparkline given no data points, say — so its name
/// still lands in [`PaintReport::undrawn`] rather than the frame reading as
/// complete over a placement nothing actually painted. This is FR-059's
/// "registered-but-unpainted stays undrawn" made structural: the return
/// value is the only signal [`paint_one`] has, so a painter that lies about
/// it is the one way this contract can still be broken, and it is a
/// one-line honesty obligation on whoever writes the painter rather than
/// something this crate can enforce further.
pub type CustomPainterFn = dyn Fn(&Painter, &CustomPaintCtx<'_>) -> bool;

/// Host-registered painters, keyed by the name `PaintContent.custom` carries.
///
/// Empty by default, so a frame with no host registration paints exactly
/// what this crate shipped before T080 existed: every custom name lands in
/// `undrawn`. Registration is opt-in per name — an unregistered name and a
/// registered painter that chose not to draw are the same outcome from the
/// frame's point of view, and [`paint_one`] treats them identically.
#[derive(Default)]
pub struct CustomPainters {
    painters: BTreeMap<String, Box<CustomPainterFn>>,
}

impl CustomPainters {
    /// An empty registry: every custom name lands in `undrawn`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `painter` under `name`. Registering the same name twice
    /// replaces the previous painter rather than keeping both — there is
    /// exactly one painter per name, the same "last registration wins" rule
    /// [`crate::image::ImageSources::register`] uses for sources.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        painter: impl Fn(&Painter, &CustomPaintCtx<'_>) -> bool + 'static,
    ) {
        self.painters.insert(name.into(), Box::new(painter));
    }

    /// The painter registered for `name`, or `None` when nothing is.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&CustomPainterFn> {
        self.painters.get(name).map(|painter| painter.as_ref())
    }
}

/// Draw `frame` into `painter`, with no host-registered painters or image
/// sources.
///
/// This is the entry point every existing caller keeps using unchanged: a
/// frame painted this way behaves exactly as it did before T080/T081 — every
/// `image` and every `custom` node lands in [`PaintReport::undrawn`], named.
/// A host that wants either drawn calls [`paint_frame_with_hosts`] instead.
///
/// The painter is expected to be a full-window layer painter
/// (`egui::Context::layer_painter`); each placement gets a clipped, faded clone
/// of it rather than a nested `Ui`.
pub fn paint_frame(
    painter: &Painter,
    frame: &PetrifiedFrame,
    shaper: &mut GalleyShaper,
    colors: &dyn TokenSource,
) -> PaintReport {
    paint_frame_with_hosts(
        painter,
        frame,
        shaper,
        colors,
        &CustomPainters::default(),
        &mut ImageSources::default(),
    )
}

/// Draw `frame` into `painter`, dispatching `PaintContent.custom` through
/// `painters` and `PaintContent.image` through `images`.
///
/// Everything [`paint_frame`] documents holds here too; the only difference
/// is that a name either registry answers for is drawn instead of merely
/// reported. Kept as a second function rather than growing `paint_frame`'s
/// parameter list so every caller written against the four-argument form —
/// `gorgon-petra-egui`'s own `Host::pass` included — keeps compiling
/// unchanged; wiring a running host up to real registries is call-site work
/// for whoever owns that host loop, not a signature this function should
/// force on every caller that does not need it yet.
pub fn paint_frame_with_hosts(
    painter: &Painter,
    frame: &PetrifiedFrame,
    shaper: &mut GalleyShaper,
    colors: &dyn TokenSource,
    painters: &CustomPainters,
    images: &mut ImageSources,
) -> PaintReport {
    let mut report = PaintReport {
        placements: frame.placements.len(),
        // `paint_pairs` zips, and a zip truncates rather than complaining, so
        // the mismatch has to be caught here or it is never caught at all.
        desynced: frame.placements.len() != frame.content.len(),
        ..PaintReport::default()
    };
    let scale = frame.viewport.scale;
    // At most one node holds focus, so this stays empty on almost every
    // frame and allocates nothing. The rings are drawn after the loop rather
    // than inside it because a ring reaches outside its node's rect, and a
    // later sibling's fill would paint over one drawn in tree order.
    let mut env = PaintEnv {
        shaper,
        colors,
        scale,
        page: to_egui_snapped(
            PetraRect {
                x: 0.0,
                y: 0.0,
                w: frame.viewport.size.w,
                h: frame.viewport.size.h,
            },
            scale,
        ),
        painters,
        images,
    };
    // The focused placement *and* the corner radius it asked for. The ring
    // tracks the node's own rounding, so it needs the `radius` slot the main
    // loop already resolved — carried here rather than re-derived, so a ring
    // can never round differently from the shape it is ringing.
    let mut focused: Vec<(&Placement, f32)> = Vec::new();
    for (placement, content) in frame.paint_pairs() {
        let clip = to_egui_snapped(placement.clip, scale);
        if !clip.is_positive() {
            // Clipped to nothing: the node is scrolled out or behind a closed
            // surface. Skipping the draw is the correct outcome here, not a
            // lost placement — but it is counted in its own bucket rather than
            // folded in with the drawn ones, so "nothing was painted this
            // frame because everything was clipped" stays visible.
            report.skipped_clipped += 1;
            continue;
        }
        let mut p = painter.with_clip_rect(clip);
        p.set_opacity(placement.opacity.clamp(0.0, 1.0));
        match paint_one(&p, placement, content, &mut env, &mut report) {
            Outcome::Drawn => report.drawn += 1,
            Outcome::Empty => report.empty += 1,
            Outcome::Silent => report.silent += 1,
        }
        if placement.semantics.focused {
            // The same chain `paint_one` walked, so a ring can never round
            // differently from the shape it is ringing when a state rebinds
            // `radius` (`gorgon_petra::token::state`).
            let corner_radius = resolve_slot(
                &content.tokens,
                RADIUS_SLOT,
                DerivedState::of(&placement.semantics),
            )
            .map_or(0.0, |token| {
                resolve_radius_or_record(env.colors, token, &mut report)
            });
            focused.push((placement, corner_radius));
        }
    }
    for (placement, corner_radius) in focused {
        let mut p = painter.with_clip_rect(to_egui_snapped(placement.clip, scale));
        p.set_opacity(placement.opacity.clamp(0.0, 1.0));
        if paint_focus_ring(&p, placement, corner_radius, colors, scale, &mut report) {
            report.focus_rings += 1;
        } else {
            report.blind_focus += 1;
        }
    }
    report
}

/// Draw the focus ring around one focused placement, and report whether any
/// of it landed.
///
/// The geometry is [`FocusRing`]'s, in `gorgon-petra`: how thick the ring is
/// and where it sits relative to the node is a design decision a second
/// renderer has to reproduce, so it does not live in this crate (D-069). What
/// lives here is three strokes.
///
/// The ring is clipped exactly as its node is. A node whose clip is its own
/// rect — a frame root, or a child filling its scroll viewport — therefore
/// keeps only the half of the ring inside its edge, which is why the core
/// band straddles that edge rather than sitting outside it.
fn paint_focus_ring(
    painter: &Painter,
    placement: &Placement,
    corner_radius: f32,
    colors: &dyn TokenSource,
    scale: Scale,
    report: &mut PaintReport,
) -> bool {
    // Snap **once**, then build the bands from what came back.
    //
    // The previous version snapped each band's own rect, and that quietly
    // destroyed the contiguity `FocusRing::bands` guarantees. The two halo
    // centrelines sit on half-units by construction, so rounding them
    // separately walked the outer band out by half a pixel and the inner band
    // in by half — which opened a one-pixel gap and let the button's own fill
    // show through the middle of its focus ring. Measured on a real capture
    // across a straight edge: card, core, **fill**, halo, fill.
    //
    // Snapping the widths into a `FocusRing` of their own, rather than
    // snapping them at the stroke, is what makes this right at fractional
    // scale too. At 1.25x the core rounds to 3 device pixels and each halo to
    // 1, so the flank is 2 device pixels and not the 1.875 the unsnapped
    // measurements would give — deriving the offsets from the same numbers the
    // strokes use is the only way those two agree.
    let snapped = FocusRing {
        core: device_snapped_width(FocusRing::STANDARD.core, scale),
        halo: device_snapped_width(FocusRing::STANDARD.halo, scale),
    };
    let node = to_egui_snapped(placement.rect, scale);

    let mut drawn = false;
    for band in snapped.bands(PetraRect {
        x: node.min.x,
        y: node.min.y,
        w: node.width(),
        h: node.height(),
    }) {
        let Some(color) = resolve_or_record(colors, band.token, report) else {
            continue;
        };
        // Already on the device grid: `node` is snapped and `band.offset` is
        // a whole number of device pixels away from it. Re-snapping here is
        // the bug above.
        let rect = egui::Rect::from_min_size(
            egui::pos2(band.rect.x, band.rect.y),
            egui::vec2(band.rect.w, band.rect.h),
        );
        if !rect.is_positive() {
            // The inner halo collapses on a node thinner than the ring. The
            // outer bands still draw, so this is not a blind focus.
            continue;
        }
        let width = band.width;
        // Concentric with the node, not square around it. This passed a
        // hardcoded `0.0` until 2026-08-25, so a focused `button` — eight
        // units of `shape.corner-md` — wore a hard rectangle while the two
        // unfocused buttons beside it kept their corners. It did not read as
        // one component with focus on it; it read as a different component.
        // `FocusBand::offset` carries it, because a band offset outward by
        // `d` gains exactly `d` of radius — one number for both, so the two
        // cannot disagree.
        let radius = (corner_radius + band.offset).max(0.0);
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(width, color),
            egui::StrokeKind::Middle,
        );
        drawn = true;
    }
    drawn
}

/// A stroke width snapped to a whole number of device pixels, then converted
/// back to logical units.
///
/// A stroke's width is a separate number from the rect it strokes, and
/// `to_egui_snapped` only snaps the rect. At any fractional device scale — a
/// panel border at 1.25x, say — a logical-unit width lands on a non-integer
/// number of device pixels, and egui's tessellator anti-aliases the leftover
/// half-pixel into a soft grey band instead of a crisp line: exactly the
/// artefact CSSWG issue #3720 describes for sub-pixel hairlines. The focus
/// ring worked this out first; this is that fix, named and shared so a third
/// stroke does not have to rediscover it.
fn device_snapped_width(width: f32, scale: Scale) -> f32 {
    let factor = scale.factor();
    (width * factor).round().max(1.0) / factor
}

/// Everything one paint pass shares across every placement it visits, beyond
/// the placement and content that change per call.
///
/// Grouped into one struct rather than threading five parameters through
/// [`paint_one`] individually — `shaper` and `images` need `&mut` and the
/// other three do not, which is exactly the split a hand-written struct
/// keeps honest that a long parameter list would not.
struct PaintEnv<'a> {
    shaper: &'a mut GalleyShaper,
    colors: &'a dyn TokenSource,
    scale: Scale,
    /// The whole drawable surface, device-snapped: `frame.viewport.size` at
    /// the origin.
    ///
    /// Only the shadow block reads it, and only to stop an elevation from
    /// leaving the page — see the comment there for why that boundary is
    /// load-bearing rather than tidy. Taken from Petra's own viewport rather
    /// than from `egui::Context`, which in 0.36.1 has no accessor for it and
    /// would in any case be answering about the window rather than about the
    /// surface Petra laid the frame out for.
    page: egui::Rect,
    painters: &'a CustomPainters,
    images: &'a mut ImageSources,
}

fn paint_one(
    painter: &Painter,
    placement: &Placement,
    content: &PaintContent,
    env: &mut PaintEnv<'_>,
    report: &mut PaintReport,
) -> Outcome {
    let rect = to_egui_snapped(placement.rect, env.scale);
    let mut shapes = 0_usize;

    // Every slot this painter does not understand is recorded by name,
    // matched on the *base* slot: `background@hover` is the `background`
    // slot bound for one state (`gorgon_petra::token::state`), not a seventh
    // slot this painter has never heard of.
    for slot in content.tokens.keys() {
        if !KNOWN_SLOTS.contains(&base_slot(slot)) {
            report.unknown_slots.insert(slot.clone());
        }
    }

    // Which token family every slot below resolves through. Read once, here,
    // rather than at each of the six lookups: the rank is a property of the
    // node, and six copies of `resolve_state` is six chances for the border
    // to think it is hovered while the fill thinks it is pressed.
    let state = DerivedState::of(&placement.semantics);
    let rank = resolve_state(state);

    // `radius` and `silhouette` shape both the fill and the stroke below
    // them, so both are resolved once, ahead of either, rather than
    // duplicated into two arms that could drift apart.
    let corner_radius = resolve_slot(&content.tokens, RADIUS_SLOT, state).map_or(0.0, |token| {
        resolve_radius_or_record(env.colors, token, report)
    });
    let figure = resolve_slot(&content.tokens, SILHOUETTE_SLOT, state)
        .map_or(Silhouette::Rect, |token| {
            resolve_silhouette_or_record(env.colors, token, report)
        });
    let outline = silhouette_points(figure, rect);

    // Elevation goes down first, because a shadow is behind the thing that
    // casts it. Drawn after the fill it would sit *on* the card, which is not
    // a subtle mistake -- it is an obviously wrong picture, and that is the
    // reason this block is here rather than next to the border below.
    if let Some(token) = resolve_slot(&content.tokens, SHADOW_SLOT, state) {
        // A rectangular shadow under a triangle is worse than no shadow, and
        // `epaint::Shadow::as_shape` can only make a `RectShape`. A node that
        // asked for both gets its silhouette honoured and its elevation
        // dropped, and nothing in `report` claims a shadow was drawn.
        //
        // A **disabled** node casts no shadow either, and that is FR-010
        // rather than a style preference. Depth is this painter's "you can
        // press this" channel, so a control that cannot be pressed must not
        // have it: a button lying flat beside two that are lifted reads as
        // unavailable *before* any of its colours do, and it survives a
        // reader who cannot separate the colours at all. The disabled colour
        // family is the second channel, not the only one — a colour-only
        // disabled state is exactly what FR-010 forbids.
        if outline.is_none()
            && rank != InteractionRank::Disabled
            && let Some(color) = resolve_or_record(env.colors, token, report)
            && let Some((_, geometry)) = SHADOW_GEOMETRY.iter().find(|(name, _)| *name == token)
        {
            let shadow = egui::epaint::Shadow {
                offset: geometry.offset,
                blur: geometry.blur,
                spread: geometry.spread,
                color,
            };
            // A shadow is the one thing on this page that has to draw
            // *outside* the node casting it, and every placement arrives here
            // already clipped to its own bounds. Painting it through the
            // inherited clip drew nothing at all: the token resolved, the
            // report counted a fill, and the window was pixel-identical --
            // the exact silent failure the design contract predicted for this
            // block, arriving through a mechanism the contract did not.
            //
            // The clip is widened by the shadow's own reach and no further:
            // `spread` grows the rect, `blur` feathers past that, and
            // `offset` displaces the whole thing. A shadow cannot escape by
            // more than it was declared to extend.
            //
            // And it may not leave the surface at all, which is what the
            // intersection with `screen_rect` is for. A shadow is allowed
            // outside its *node*; it is not allowed outside the *page*. The
            // difference is not cosmetic: `gorgon/petra-egui/examples/parity.rs`
            // pins `RawInput::screen_rect` to a fixed rectangle so the two
            // targets lay out identically whatever size a window manager
            // grants, and `petra-parity` refuses the capture if anything is
            // painted beyond that pin -- which is exactly what a card near the
            // bottom edge did once elevation landed, because widening a clip
            // by 13 units walks straight through a boundary nothing else in
            // this painter can reach. `PaintEnv::page` is Petra's own
            // viewport, which is the pinned rectangle on that page and the
            // real surface everywhere else, so one intersection is correct in
            // both cases.
            let reach = f32::from(geometry.spread)
                + f32::from(geometry.blur)
                + f32::from(geometry.offset[0].abs().max(geometry.offset[1].abs()));
            // `Painter::with_clip_rect` INTERSECTS -- `rect.intersect(self.clip_rect)`
            // in egui-0.36.1's `painter.rs:73`. It can only ever narrow, so
            // widening through it is a silent no-op, and that is exactly how
            // the first attempt at this block failed: the shadow drew, and
            // survived only in the corner cut-outs the card's own rounding
            // left behind. `set_clip_rect` is the one that replaces.
            let mut cast = painter.clone();
            cast.set_clip_rect(painter.clip_rect().expand(reach).intersect(env.page));
            cast.add(egui::Shape::Rect(shadow.as_shape(rect, corner_radius)));
            report.fills += 1;
            shapes += 1;
        }
    }
    if let Some(token) = resolve_slot(&content.tokens, BACKGROUND_SLOT, state)
        && let Some(color) = resolve_or_record(env.colors, token, report)
    {
        match &outline {
            None => {
                painter.rect_filled(rect, corner_radius, color);
            }
            Some(points) => {
                painter.add(egui::Shape::convex_polygon(
                    points.clone(),
                    color,
                    Stroke::NONE,
                ));
            }
        }
        report.fills += 1;
        shapes += 1;
    }
    // An anchored surface's caret, drawn with the fill it continues and
    // therefore immediately after it: `crate::triangle` owns the shape, the
    // engine owns the geometry (`contracts/anchored-placement.md` §5), and a
    // surface that binds no background has no colour to draw one in.
    //
    // The fill goes through `resolve_slot`, not a bare map lookup, for the
    // same reason the background above does: a hovered surface binding
    // `background@hover` would otherwise grow a caret in its resting colour,
    // and a caret that disagrees with the shape it continues is worse than no
    // caret at all.
    if let Some(caret) = &content.caret {
        let fill = resolve_slot(&content.tokens, BACKGROUND_SLOT, state)
            .and_then(|token| resolve_or_record(env.colors, token, report));
        if crate::triangle::paint_caret(painter, caret, fill, env.scale) {
            report.fills += 1;
            shapes += 1;
        } else {
            report.undrawn.insert("caret".to_owned());
        }
    }
    if let Some(token) = resolve_slot(&content.tokens, BORDER_SLOT, state)
        && let Some(color) = resolve_or_record(env.colors, token, report)
    {
        let width = device_snapped_width(1.0, env.scale);
        match &outline {
            None => {
                painter.rect_stroke(
                    rect,
                    corner_radius,
                    Stroke::new(width, color),
                    egui::StrokeKind::Inside,
                );
            }
            // A polygon stroke is centred on its path rather than inset the
            // way `StrokeKind::Inside` insets a rect's. At the one-unit
            // width this painter draws, that is half a logical unit of
            // overhang on a marker whose whole job is to be a recognisable
            // outline; correcting it would mean insetting the polygon, and
            // an inset that shrinks a triangle is not the same operation on
            // every figure. Left centred, and named here rather than
            // silently different.
            Some(points) => {
                painter.add(egui::Shape::convex_polygon(
                    points.clone(),
                    Color32::TRANSPARENT,
                    Stroke::new(width, color),
                ));
            }
        }
        shapes += 1;
    }

    if let Some(text) = &content.text {
        let token =
            resolve_slot(&content.tokens, FOREGROUND_SLOT, state).unwrap_or(DEFAULT_TEXT_TOKEN);
        let color = env.colors.color(token).unwrap_or_else(|| {
            report.unresolved_tokens.insert(token.to_owned());
            // Not a guess at the theme's intent: a visibly wrong colour is
            // better than invisible text, and the unresolved token is in the
            // report either way.
            Color32::PLACEHOLDER
        });
        // A style token the shaper has no binding for still shapes — at the
        // theme's body style, because blank text is worse on screen than
        // text at the wrong size — so the only way it can be noticed is for
        // the name to be reported here, the same as an unresolved colour.
        if let Some(unresolved) = env
            .shaper
            .typography()
            .resolve(text.style.as_deref())
            .1
            .map(str::to_owned)
        {
            report.unresolved_tokens.insert(unresolved);
        }
        let galley = env.shaper.galley(&TextRequest {
            text: &text.text,
            style: text.style.as_deref(),
            wrap: text.wrap,
            max_lines: text.max_lines,
            available_width: Some(placement.rect.w),
        });
        // The atlas curve alone cannot reach every `passes` value the
        // `text.coverage-curve` token allows (SPEC.md §2.3: no
        // `FontColorTransferFunction` variant exists past two-pass
        // compositing), so the rest is spent here, by painting the same
        // galley `repeats` times under egui-wgpu's premultiplied
        // source-over — which composites to `1 - (1-a)^repeats`, the same
        // identity the atlas curve itself rests on
        // (`crate::host::coverage_plan`'s doc comment names the trap this
        // is the other half of). `report.texts` and `shapes` count the
        // logical run once regardless of how many physical paints it took —
        // see this module's doc comment and `PaintReport`'s.
        let coverage = env
            .colors
            .coverage(COVERAGE_TOKEN)
            .unwrap_or(FALLBACK_COVERAGE);
        let plan = coverage_plan(coverage);
        for _ in 0..plan.repeats {
            painter.galley(rect.min, galley.clone(), color);
        }
        if plan.fraction > 0.0 {
            // ai-macs' fractional pass, ported verbatim: one further paint
            // at the colour's alpha scaled by the fractional remainder, not
            // the fraction silently dropped.
            painter.galley(
                rect.min,
                galley.clone(),
                color.gamma_multiply(plan.fraction),
            );
        }
        report.texts += 1;
        shapes += 1;
    }

    if let Some(source) = &content.image {
        // `resolve` is the whole of "decode/upload behind a source-keyed
        // cache" (T081): a source this pass has drawn before comes back
        // from the cache, a new one is decoded and uploaded once. Either
        // way what comes back is a texture with its own natural size, and
        // `image::contain` is the aspect handling — fit that size inside
        // `rect`, the space Petra offered this node, without stretching it.
        if let Some(handle) = env.images.resolve(painter.ctx(), source) {
            let target = crate::image::contain(rect, handle.size_vec2());
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(handle.id(), target, uv, Color32::WHITE);
            report.images += 1;
            shapes += 1;
        } else {
            // Not registered, or registered with pixels that failed to
            // decode — either way this is the source that could not be
            // drawn, named so the operator knows which picture is missing
            // rather than that "an image" is.
            report.undrawn.insert(format!("image:{source}"));
        }
    }
    if let Some(name) = &content.custom {
        let ctx = CustomPaintCtx {
            rect,
            clip: painter.clip_rect(),
            opacity: painter.opacity(),
            scale: env.scale,
            tokens: env.colors,
        };
        // `is_some_and` rather than an `if let` that ignores the bool: a
        // registered painter that draws nothing must be treated exactly
        // like no painter at all, and this is the one line where that
        // equivalence either holds or quietly stops holding.
        if env
            .painters
            .get(name)
            .is_some_and(|paint| paint(painter, &ctx))
        {
            report.customs += 1;
            shapes += 1;
        } else {
            report.undrawn.insert(format!("custom:{name}"));
        }
    }

    if content.is_empty() {
        // Nothing was declared, so nothing missing. A `Stack` is a position
        // for its children; it is not supposed to paint.
        Outcome::Empty
    } else if shapes > 0 {
        Outcome::Drawn
    } else {
        // Content was declared and the pass emitted nothing for it. This is
        // the state the accounting exists to name.
        Outcome::Silent
    }
}

/// A logical rect snapped to the same device-pixel grid the digest hashes.
///
/// This is the function that makes `contracts/frame-identity.md`'s "the SAME
/// rounding the renderer uses" true. It was not true before: the digest called
/// [`round_rect`] and the painter handed egui the raw logical rect, letting
/// egui's tessellator round it its own way at its own time. The two agree at
/// scale 1.0 and were never checked anywhere else, so the digest described a
/// device-pixel frame that nothing had painted.
///
/// The snap happens here rather than in the engine because the engine has no
/// business knowing about pixels: `Placement::rect` stays logical, one rounding
/// rule is applied to it in exactly two places, and both places call the same
/// function. Converting back to logical (`device / scale`) hands egui a value
/// that is already on the grid, so its own rounding is a no-op rather than a
/// second, different opinion.
fn to_egui_snapped(rect: PetraRect, scale: Scale) -> egui::Rect {
    let d = round_rect(rect, scale);
    let f = scale.factor();
    #[allow(clippy::cast_precision_loss)]
    egui::Rect::from_min_size(
        egui::pos2(d.x as f32 / f, d.y as f32 / f),
        egui::vec2(d.w as f32 / f, d.h as f32 / f),
    )
}

/// Prove the paint accounting can detect a placement that painted nothing.
///
/// Called by this crate's invariant companion. This runs a **real**
/// [`paint_frame`] over a real petrified frame, which is the whole point: the
/// version this replaced asserted on two hand-written `PaintReport` literals,
/// so it proved that `2 != 3` and nothing whatsoever about the paint pass. The
/// accounting only earns its keep if a pass that drops content reports it, so
/// that is what is exercised here.
///
/// # Panics
/// Panics when a frame whose only content is an image — which this crate has
/// no loader for — still reports as a complete pass.
pub fn verify_paint_accounting() {
    use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
    use gorgon_petra::geom::Size;
    use gorgon_petra::testing::{Harness, NoRows, validated};
    use gorgon_petra::token::{ThemeMode, ThemeSnapshot, dark};
    use gorgon_petra::tree::{NodeKind, Props, ViewNode};

    let ctx = egui::Context::default();
    // egui accumulates font-atlas deltas that something is expected to
    // consume. Nothing here presents a frame, so each pass is explicitly
    // dropped without applying them; otherwise egui panics on teardown.
    ctx.run_ui(egui::RawInput::default(), |_| {})
        .drop_without_applying_deltas();

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("petra-invariant"),
    ));
    let viewport = Viewport::new(Size::new(120.0, 60.0), ThemeMode::Dark);
    let theme = ThemeSnapshot::new(dark(), 1);

    // An image node declares content and this crate has no image loader, so
    // the pass must emit nothing for it and must say so.
    let tree = ViewNode::new(NodeKind::Image, "logo").with_props(Props {
        image: Some("logo.png".into()),
        ..Props::default()
    });
    let mut harness = Harness::with(GalleyShaper::new(ctx.clone()), NoRows);
    let frame = petrify(
        1,
        validated(&tree),
        &mut harness.ctx(),
        viewport,
        TransitionActivity::default(),
    );
    let mut shaper = GalleyShaper::new(ctx.clone());
    let report = paint_frame(&painter, &frame, &mut shaper, &theme);

    assert_eq!(
        report.silent, 1,
        "gorgon-petra-egui: a placement that declared content and painted \
         nothing must be counted as silent, not as visited: {report:?}"
    );
    assert!(
        !report.is_complete(),
        "gorgon-petra-egui: a pass that dropped a placement must not read as \
         complete; the digest cannot see a missing panel, so this accounting \
         is what does: {report:?}"
    );

    // And the other direction, so the accounting cannot earn a pass by
    // calling every frame incomplete.
    let drawn = ViewNode::new(NodeKind::Text, "label").with_props(Props {
        text: Some("ok".into()),
        ..Props::default()
    });
    let frame = petrify(
        2,
        validated(&drawn),
        &mut harness.ctx(),
        viewport,
        TransitionActivity::default(),
    );
    let report = paint_frame(&painter, &frame, &mut shaper, &theme);
    assert!(
        report.is_complete() && report.drawn == 1,
        "gorgon-petra-egui: a frame whose text was painted must read as \
         complete: {report:?}"
    );

    ctx.run_ui(egui::RawInput::default(), |_| {})
        .drop_without_applying_deltas();
}

#[cfg(test)]
mod tests {
    use super::{
        CustomPaintCtx, CustomPainters, TokenSource, device_snapped_width, paint_frame,
        paint_frame_with_hosts, to_egui_snapped, verify_paint_accounting,
    };
    use egui::{Color32, Context, CornerRadius, Id, LayerId, Order, RawInput, Shape};
    use gorgon_petra::frame::round_rect;
    use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
    use gorgon_petra::geom::Size;
    use gorgon_petra::geom::{Rect as PetraRect, Scale};
    use gorgon_petra::testing::{Harness, validated};
    use gorgon_petra::token::value::CoverageValue;
    use gorgon_petra::token::{
        Theme, ThemeMode, ThemeSnapshot, TokenName, TokenValue, dark, standard_vocabulary,
    };
    use gorgon_petra::tree::{NodeKind, Props, ViewNode};

    use crate::image::{ImagePixels, ImageSources};
    use crate::text::GalleyShaper;

    struct Headless(Context);

    impl Headless {
        fn new() -> Self {
            let ctx = Context::default();
            pass(&ctx);
            Self(ctx)
        }
        fn painter(&self) -> egui::Painter {
            self.0
                .layer_painter(LayerId::new(Order::Background, Id::new("petra-test")))
        }
        fn shaper(&self) -> GalleyShaper {
            GalleyShaper::new(self.0.clone())
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

    fn snapshot() -> ThemeSnapshot {
        ThemeSnapshot::new(dark(), 1)
    }

    /// `dark()` with its `text.coverage-curve` overridden — the fixture the
    /// coverage-repeat tests below use to reach `passes` values other than
    /// dark's own shipped `3.0`.
    fn coverage_variant(passes: f32, snap: bool) -> ThemeSnapshot {
        let mut values = dark().values().clone();
        values.insert(
            TokenName::new("text.coverage-curve")
                .expect("\"text.coverage-curve\" is a well-formed token name"),
            TokenValue::Coverage(CoverageValue { passes, snap }),
        );
        let theme = Theme::build(ThemeMode::Dark, &standard_vocabulary(), values).expect(
            "overriding an already-declared token's value at its declared kind \
             keeps the theme complete",
        );
        ThemeSnapshot::new(theme, 1)
    }

    /// A token reference, for the fixtures below — `Props.tokens`' value
    /// type is [`TokenName`], not a plain string (C15).
    fn tok(name: &str) -> TokenName {
        TokenName::new(name).expect("test token names are well-formed")
    }

    fn tree() -> ViewNode {
        let mut panel = Props::default();
        panel
            .tokens
            .insert("background".into(), tok("surface.base"));
        panel.tokens.insert("border".into(), tok("surface.raised"));
        ViewNode::new(NodeKind::Stack, "root")
            .with_props(panel)
            .child(ViewNode::new(NodeKind::Text, "title").with_props(Props {
                text: Some("Fibers".into()),
                ..Props::default()
            }))
            .child(ViewNode::new(NodeKind::Spacer, "gap"))
    }

    fn frame_of(
        node: &ViewNode,
        h: &mut Harness<GalleyShaper, gorgon_petra::testing::NoRows>,
    ) -> gorgon_petra::frame::PetrifiedFrame {
        petrify(
            1,
            validated(node),
            &mut h.ctx(),
            Viewport::new(Size::new(240.0, 120.0), ThemeMode::Dark),
            TransitionActivity::default(),
        )
    }

    /// An elevation may leave its own node. It may not leave the page.
    ///
    /// # The bug this is the guard for
    ///
    /// A shadow is the one thing in this painter that deliberately draws
    /// outside the rect it belongs to, so the shadow block widens its clip by
    /// the declared reach. Nothing capped that, and a card near the bottom of
    /// the surface therefore painted *past the end of the page* — up to
    /// `spread + blur + max(|offset|)` units of blurred black onto whatever
    /// was outside.
    ///
    /// On a window that is only ever as large as the frame, nobody would
    /// notice. `gorgon/petra-egui/examples/parity.rs` is not that: it pins
    /// `RawInput::screen_rect` to a fixed rectangle so the native and web
    /// targets lay out identically whatever size a window manager grants, and
    /// the `petra-parity` lane refuses the capture outright if anything is
    /// painted outside the pin. It did refuse it, naming "5 or more distinct
    /// colour(s) outside that rectangle" against a budget of 4 — which is what
    /// a blurred shadow gradient looks like when it lands in a margin that is
    /// supposed to hold one flat colour.
    ///
    /// # What is asserted
    ///
    /// The fixture is a node bound flush to the bottom-right of a small
    /// viewport, carrying `shadow.overlay` — the deeper of the two, reach 13,
    /// so an unclamped clip would escape by a wide margin. Every clip rect
    /// egui received must sit inside the page. Reading the clip rather than
    /// the pixels is deliberate: the clip is what the bug was, and a pixel
    /// assertion would additionally depend on the shadow's alpha being high
    /// enough to change a byte, which is a different claim.
    #[test]
    fn an_elevation_may_leave_its_node_but_never_leaves_the_page() {
        const PAGE: Size = Size { w: 240.0, h: 120.0 };

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);

        // Flush to the far corner, so the shadow has somewhere to escape to.
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        props.tokens.insert("shadow".into(), tok("shadow.overlay"));
        let card = ViewNode::new(NodeKind::Stack, "card")
            .with_props(props)
            .with_constraints(gorgon_petra::tree::Constraints {
                horizontal: gorgon_petra::tree::AxisConstraint {
                    min: Some(PAGE.w),
                    max: Some(PAGE.w),
                    priority: 0,
                },
                vertical: gorgon_petra::tree::AxisConstraint {
                    min: Some(PAGE.h),
                    max: Some(PAGE.h),
                    priority: 0,
                },
            });
        let tree = ViewNode::new(NodeKind::Stack, "root").child(card);
        let frame = frame_of(&tree, &mut h);

        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");
        assert!(
            report.fills >= 2,
            "the fixture must actually draw a shadow and a fill, or this test \
             passes by drawing nothing: {report:?}"
        );

        let page = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(PAGE.w, PAGE.h));
        let out = host.0.run_ui(RawInput::default(), |_| {});
        let escaped: Vec<egui::Rect> = out
            .shapes
            .iter()
            .map(|cs| cs.clip_rect)
            .filter(|clip| !page.contains_rect(*clip))
            .collect();
        out.drop_without_applying_deltas();

        assert!(
            escaped.is_empty(),
            "{} clip rect(s) reach outside the {PAGE:?} page: {escaped:?}. A \
             shadow is allowed outside its node and is not allowed outside the \
             surface -- see the shadow block's own comment for the lane that \
             catches this when the assertion does not.",
            escaped.len()
        );
    }

    /// The declared type ramp has to survive all the way to the screen.
    ///
    /// `component::heading` binds `typography.heading` (20 units) and
    /// `component::text` binds `typography.body` (14). The size is read off
    /// the shapes egui actually received, because that is the only place the
    /// failure this guards was visible: the shaper's style map was keyed on
    /// a private vocabulary (`body`, `heading`, `small`, `mono`) that no
    /// theme and no component ever used, so every lookup missed, every miss
    /// fell back to one size, and every gate stayed green.
    #[test]
    fn a_heading_paints_larger_than_body_text() {
        use gorgon_petra::component::{heading, text as body_text};

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(heading("h", "Heading"))
            .child(body_text("b", "Body"));
        let frame = frame_of(&tree, &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(
            &host.painter(),
            &frame,
            &mut shaper,
            &coverage_variant(3.0, false),
        );
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");
        assert_eq!(report.texts, 2, "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let mut sizes: Vec<f32> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Text(t) => Some(t.galley.job.sections[0].format.font_id.size),
                _ => None,
            })
            .collect();
        out.drop_without_applying_deltas();

        // The fixture pins `passes: 3.0` rather than reading whatever the
        // shipped theme carries, so each logical run paints three times --
        // three physical `Shape::Text` entries per run, six total for two
        // runs -- before this test's own concern (the size ramp) ever gets a
        // look. A binder that dropped the repeat count silently would leave
        // this at 2, not 6, and this assertion would be the one to catch it.
        //
        // Reading `dark()` here instead would make the assertion a function
        // of a tuning decision: the shipped value moved to `2.0` (one paint)
        // when the muted-tone fix landed, and this test would have gone
        // vacuously green while still claiming to guard the repeat count.
        assert_eq!(
            sizes.len(),
            6,
            "two runs at three paints each (passes: 3.0) must all reach \
             egui: {sizes:?}"
        );
        sizes.dedup();
        assert_eq!(
            sizes.len(),
            2,
            "one heading run and one body run, once the three identical \
             coverage-repeat paints per run are collapsed: {sizes:?}"
        );
        assert!(
            sizes[0] > sizes[1],
            "typography.heading painted at {} and typography.body at {} — the \
             declared ramp collapsed to one size on the way to the screen",
            sizes[0],
            sizes[1]
        );
        assert_eq!(
            sizes,
            vec![20.0, 14.0],
            "and at the sizes the shipped ramp declares: {sizes:?}"
        );
    }

    /// `report.texts` is `PaintReport`'s honesty counter and the gallery
    /// prints it — painting one run three times for `passes: 3.0` must
    /// still report one text, not three, or the counter starts lying about
    /// the thing it exists to be honest about.
    ///
    /// `passes` comes from [`coverage_variant`], not from `dark()`. The
    /// property under test is "a repeated paint reports once", and a test
    /// that sourced the repeat count from the shipped theme would stop
    /// testing it the moment the shipped value became `1` physical paint —
    /// silently, and while still passing.
    #[test]
    fn painting_a_run_several_times_for_coverage_still_reports_one_text() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let tree = ViewNode::new(NodeKind::Text, "label").with_props(Props {
            text: Some("Fibers".into()),
            ..Props::default()
        });
        let frame = frame_of(&tree, &mut h);
        let mut shaper = host.shaper();
        // `passes: 3.0` -- three physical paints for this one logical run.
        let report = paint_frame(
            &host.painter(),
            &frame,
            &mut shaper,
            &coverage_variant(3.0, false),
        );
        assert_eq!(report.texts, 1, "{report:?}");
        assert_eq!(report.drawn, 1, "{report:?}");
        assert_eq!(report.placements, 1);

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let text_shapes = out
            .shapes
            .iter()
            .filter(|cs| matches!(cs.shape, Shape::Text(_)))
            .count();
        out.drop_without_applying_deltas();
        assert_eq!(
            text_shapes, 3,
            "passes: 3.0 must still paint the glyph three times even though \
             the report counts one text run"
        );
    }

    /// A non-integer `passes` is not dropped to its floor: ai-macs' own
    /// fractional-pass model — one further paint at the colour's alpha
    /// scaled by the fractional remainder — ports verbatim (SPEC.md §1.4,
    /// §2.3).
    #[test]
    fn a_fractional_passes_value_paints_one_further_faded_pass() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let tree = ViewNode::new(NodeKind::Text, "label").with_props(Props {
            text: Some("Fibers".into()),
            ..Props::default()
        });
        let frame = frame_of(&tree, &mut h);
        let mut shaper = host.shaper();
        // passes: 2.5 is not one of the two anchor points `coverage_plan`
        // spends the atlas curve on, so it decomposes as Off, two whole
        // paints, plus one further pass at half the text colour's alpha.
        let report = paint_frame(
            &host.painter(),
            &frame,
            &mut shaper,
            &coverage_variant(2.5, false),
        );
        assert_eq!(report.texts, 1, "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let fallback_colors: Vec<Color32> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Text(t) => Some(t.fallback_color),
                _ => None,
            })
            .collect();
        out.drop_without_applying_deltas();

        assert_eq!(
            fallback_colors.len(),
            3,
            "two whole passes plus one fractional pass: {fallback_colors:?}"
        );
        assert_eq!(
            fallback_colors[0], fallback_colors[1],
            "the two whole passes must paint at full, identical alpha: \
             {fallback_colors:?}"
        );
        assert!(
            fallback_colors[2].a() < fallback_colors[0].a(),
            "the fractional pass must be faded, not painted at full alpha: \
             {fallback_colors:?}"
        );
        assert_ne!(
            fallback_colors[2].a(),
            0,
            "faded is not the same as invisible: {fallback_colors:?}"
        );
    }

    /// A style token nothing is bound to must be named in the report. The
    /// run still paints — blank text is worse on screen than text at the
    /// wrong size — so the report is the only place the miss can show.
    #[test]
    fn an_unresolved_style_token_is_reported_rather_than_shaped_in_silence() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let tree = ViewNode::new(NodeKind::Text, "label").with_props(Props {
            text: Some("Fibers".into()),
            style: Some(tok("typography.heading")),
            ..Props::default()
        });
        let frame = frame_of(&tree, &mut h);

        // A shaper whose map knows nothing, standing in for a host that
        // never bound the theme to it.
        let mut blind = crate::text::GalleyShaper::with_typography(
            host.0.clone(),
            crate::text::Typography::new(crate::text::TextStyle::new(egui::FontId::new(
                14.0,
                egui::FontFamily::Proportional,
            ))),
        );
        let report = paint_frame(&host.painter(), &frame, &mut blind, &snapshot());
        assert_eq!(report.texts, 1, "the run still paints: {report:?}");
        assert!(
            report.unresolved_tokens.contains("typography.heading"),
            "the style token that resolved to nothing must be named: {report:?}"
        );

        // And the opposite direction, so the report cannot earn a pass by
        // naming every style token: a bound one is not reported.
        let mut bound = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut bound, &snapshot());
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");
    }

    #[test]
    fn every_placement_is_visited_and_content_is_drawn() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of(&tree(), &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());

        assert_eq!(report.placements, 3);
        assert!(report.is_complete(), "{report:?}");
        assert_eq!(report.texts, 1, "the title is the only text run");
        assert_eq!(report.fills, 1, "only the panel declares a background");
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");
        assert!(report.undrawn.is_empty(), "{report:?}");
    }

    /// A token neither shipped nor declared to the registry used to reach
    /// paint and come back as `report.unresolved_tokens` — this test's
    /// assertion, before `gorgon-petra`'s tree-acceptance vocabulary check
    /// (FR-056, contract C5) existed. It cannot any more: `frame_of` mints
    /// its `ValidatedTree` through `gorgon_petra::testing::validated`, which
    /// now refuses a tree naming an undeclared token before paint ever sees
    /// it (`tree::validate::Violation::UnknownTokenRef`) — so the scenario
    /// this test used to construct is unreachable through the front door,
    /// the same way an out-of-vocabulary spacing name can no longer reach
    /// `props::resolve_spacing`'s `panic!`. `PaintReport::unresolved_tokens`
    /// stays as the same kind of defense-in-depth that panic is, and is
    /// still exercised where it is still reachable — a themed snapshot
    /// legitimately missing a *focus-ring* value, below — but the case this
    /// test named is now provable at the earlier boundary, and that is what
    /// it proves instead.
    #[test]
    fn an_undeclared_token_is_refused_before_paint_ever_sees_it() {
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("surface.invented"));
        let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        let errors = gorgon_petra::tree::validate(&node, &gorgon_petra::tree::Registry::new())
            .expect_err("surface.invented is not declared to any vocabulary");
        assert!(errors.to_string().contains("surface.invented"), "{errors}");
    }

    /// Content this crate cannot draw yet is named rather than left as an
    /// unexplained blank.
    ///
    /// This test used to assert `report.is_complete()` on exactly this frame —
    /// it encoded the bug it was standing next to, since a frame whose only
    /// content could not be drawn is the definition of an incomplete pass. The
    /// assertion is inverted now; `undrawn` still names what was missing, which
    /// was the half it always got right.
    #[test]
    fn undrawable_content_is_named() {
        let host = Headless::new();
        let node = ViewNode::new(NodeKind::Stack, "root").child(
            ViewNode::new(NodeKind::Image, "logo").with_props(Props {
                image: Some("logo.png".into()),
                ..Props::default()
            }),
        );
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of(&node, &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(
            !report.is_complete(),
            "a frame whose only content could not be drawn is not a complete \
             pass: {report:?}"
        );
        assert_eq!(
            report
                .undrawn
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["image:logo.png"],
            "T081: an unresolvable source is named by itself, not by the bare kind"
        );
    }

    /// The shipped dark theme resolves the slots the painter reads.
    #[test]
    fn the_shipped_theme_answers_the_slots_the_painter_uses() {
        let snap = snapshot();
        for token in [
            "surface.base",
            "surface.raised",
            "text.primary",
            "text.muted",
        ] {
            assert!(snap.color(token).is_some(), "{token} has no colour");
        }
        assert_eq!(
            snap.color("spacing-04"),
            None,
            "a spacing token is not a colour"
        );
        assert_eq!(snap.color("not.a.token"), None);
        assert_ne!(snap.color("surface.base"), Some(Color32::PLACEHOLDER));
    }

    /// The claim `contracts/frame-identity.md` makes about rounding, checked
    /// rather than asserted: what the painter hands egui must land on exactly
    /// the device-pixel grid the digest hashed. Before this, the digest called
    /// `round_rect` and the painter did not call anything — the two agreed at
    /// scale 1.0 and were unverified everywhere else.
    #[test]
    fn the_painter_lands_on_the_same_device_grid_the_digest_hashes() {
        for factor in [1.0_f32, 1.25, 1.5, 2.0] {
            let scale = Scale::new(factor).unwrap();
            for rect in [
                PetraRect::new(0.0, 0.0, 100.0, 24.0),
                PetraRect::new(0.4, 12.3, 33.7, 9.9),
                PetraRect::new(-8.5, -0.5, 17.0, 1.0),
            ] {
                let device = round_rect(rect, scale);
                let painted = to_egui_snapped(rect, scale);
                assert_eq!(
                    (
                        (painted.min.x * factor).round() as i32,
                        (painted.min.y * factor).round() as i32,
                        (painted.width() * factor).round() as i32,
                        (painted.height() * factor).round() as i32,
                    ),
                    (device.x, device.y, device.w, device.h),
                    "scale {factor} rect {rect:?}"
                );
            }
        }
    }

    /// Adjacent rows must still tile after the painter's conversion back to
    /// logical units. This is the property edge-rounding exists for, checked on
    /// the values the painter actually emits rather than on `round_rect` alone.
    #[test]
    fn adjacent_painted_rows_share_an_edge_at_fractional_scale() {
        for factor in [1.25_f32, 1.5, 1.75] {
            let scale = Scale::new(factor).unwrap();
            let mut previous_bottom: Option<f32> = None;
            for i in 0..8_u8 {
                let row = PetraRect::new(0.0, f32::from(i) * 20.5, 80.0, 20.5);
                let painted = to_egui_snapped(row, scale);
                if let Some(bottom) = previous_bottom {
                    assert!(
                        (painted.min.y - bottom).abs() < 1e-4,
                        "row {i} at scale {factor} starts at {} but the row \
                         above ended at {bottom}: that gap is a visible seam",
                        painted.min.y
                    );
                }
                previous_bottom = Some(painted.max.y);
            }
        }
    }

    /// The border stroke's *width*, not just the rect it strokes, must land
    /// on a whole number of device pixels — otherwise the leftover
    /// half-pixel anti-aliases into a grey smear at any scale that is not a
    /// whole number, per `canon/type-and-render-craft.md` §6. The focus
    /// ring's band width already did this correction; the border slot did
    /// not, until now.
    #[test]
    fn a_border_stroke_is_a_whole_number_of_device_pixels() {
        for factor in [1.25_f32, 1.5] {
            let scale = Scale::new(factor).unwrap();

            // The helper in isolation: the snapped width, scaled back up to
            // device pixels, must be within rounding error of a whole
            // number. At scale 1.0 this holds trivially for any width, which
            // is exactly why the interesting scales are fractional ones.
            let width = device_snapped_width(1.0, scale);
            let device = width * factor;
            assert!(
                (device - device.round()).abs() < 1e-4,
                "scale {factor}: snapped width {width} is {device} device \
                 pixels, not a whole number"
            );

            // And the painter actually uses it: paint a bordered node and
            // read the stroke width egui received, not just what the helper
            // returns in isolation.
            let host = Headless::new();
            let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
            let mut props = Props::default();
            props.tokens.insert("border".into(), tok("surface.raised"));
            let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
            let frame = petrify(
                1,
                validated(&node),
                &mut h.ctx(),
                Viewport::new(Size::new(240.0, 120.0), ThemeMode::Dark).with_scale(scale),
                TransitionActivity::default(),
            );
            let mut shaper = host.shaper();
            paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
            let out = host.0.run_ui(RawInput::default(), |_| {});
            let painted_width = out
                .shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    Shape::Rect(r) if r.stroke.width > 0.0 => Some(r.stroke.width),
                    _ => None,
                })
                .expect("the border must have painted a stroke");
            out.drop_without_applying_deltas();

            assert_eq!(
                painted_width, width,
                "scale {factor}: the painter must use the same snapped \
                 width the helper computes"
            );
            let painted_device = painted_width * factor;
            assert!(
                (painted_device - painted_device.round()).abs() < 1e-4,
                "scale {factor}: the painted border width {painted_width} \
                 is {painted_device} device pixels, not a whole number"
            );
        }
    }

    #[test]
    fn the_accounting_detects_a_dropped_placement() {
        verify_paint_accounting();
    }

    /// A node that declares content this crate cannot draw must be counted as
    /// silent, and the frame must read as incomplete. Before this, an image
    /// landed in `undrawn` while the pass still reported every placement
    /// visited and complete.
    #[test]
    fn a_placement_that_declares_content_and_paints_nothing_is_not_complete() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let tree = ViewNode::new(NodeKind::Stack, "root").child(
            ViewNode::new(NodeKind::Image, "logo").with_props(Props {
                image: Some("logo.png".into()),
                ..Props::default()
            }),
        );
        let frame = frame_of(&tree, &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());

        assert_eq!(report.silent, 1, "the image painted nothing: {report:?}");
        assert_eq!(report.empty, 1, "the bare stack declares nothing to paint");
        assert!(!report.is_complete(), "{report:?}");
        assert!(report.undrawn.contains("image:logo.png"), "{report:?}");
    }

    /// A state-decorated slot is the slot it decorates, in three places at
    /// once: the schema, the unknown-slot report, and the colour actually
    /// drawn.
    ///
    /// The unknown-slot half is the one that would fail loudest. Every state
    /// binding on every component would otherwise land in
    /// `PaintReport::unknown_slots`, so the report that exists to name the
    /// *one* slot a host invented would name six per button and stop being
    /// readable at all.
    ///
    /// The drawn half is what makes the other two matter: hovering the node
    /// has to change the fill. Asserted through `report.fills` plus the
    /// resolved token name rather than through pixels — this crate has no
    /// pixel readback — but the token name is what `paint_one` hands to
    /// `resolve_or_record`, so a chain that resolved the wrong name would
    /// report the wrong colour as unresolved.
    #[test]
    fn a_state_decorated_slot_is_drawn_as_the_slot_it_decorates() {
        use super::BACKGROUND_SLOT;
        use gorgon_petra::token::{DerivedState, resolve_slot, standard_slots};

        assert!(
            standard_slots().contains("background@hover"),
            "the schema must resolve a decorated key to its base slot"
        );
        assert!(
            super::KNOWN_SLOTS.contains(&super::base_slot("background@hover")),
            "and this painter must recognise it as `background`"
        );

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        props
            .tokens
            .insert("background@hover".into(), tok("layer-hover"));
        let mut frame = frame_of(
            &ViewNode::new(NodeKind::Stack, "card").with_props(props),
            &mut h,
        );

        let mut shaper = host.shaper();
        let resting = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(resting.unknown_slots.is_empty(), "{resting:?}");
        assert_eq!(resting.fills, 1);
        assert_eq!(
            resolve_slot(
                &frame.content[0].tokens,
                BACKGROUND_SLOT,
                DerivedState::default()
            ),
            Some("surface.raised")
        );

        frame.placements[0].semantics.hovered = true;
        let lit = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(lit.unknown_slots.is_empty(), "{lit:?}");
        assert_eq!(lit.fills, 1, "still exactly one fill, in a different tone");
        assert!(lit.unresolved_tokens.is_empty(), "{lit:?}");
        assert_eq!(
            resolve_slot(
                &frame.content[0].tokens,
                BACKGROUND_SLOT,
                DerivedState::of(&frame.placements[0].semantics)
            ),
            Some("layer-hover"),
            "hovering the card did not move it to its hover surface"
        );
    }

    /// A disabled node casts no shadow (FR-010).
    ///
    /// Depth is this painter's "you can press this" channel, and it is the
    /// one channel that survives a reader who cannot separate the colours at
    /// all — which is why "disabled" may not be carried by a colour family
    /// alone. The same node enabled draws two fills (shadow plus background)
    /// and disabled draws one.
    #[test]
    fn a_disabled_node_casts_no_shadow() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        props.tokens.insert("shadow".into(), tok("shadow.raised"));
        let mut frame = frame_of(
            &ViewNode::new(NodeKind::Stack, "card").with_props(props),
            &mut h,
        );

        let mut shaper = host.shaper();
        let lifted = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(lifted.fills, 2, "an enabled card is lifted: {lifted:?}");

        frame.placements[0].semantics.disabled = true;
        let flat = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(
            flat.fills, 1,
            "a disabled card kept its elevation, so the only thing separating \
             it from an available one is colour: {flat:?}"
        );
    }

    /// A token slot this painter does not consume is still named rather than
    /// dropped.
    ///
    /// **There used to be two routes into `unknown_slots` and now there is
    /// one.** The other was a slot `standard_slots()` declared and this
    /// painter did not draw, and `highlight` was the fixture for it. FR-025
    /// retired all five such slots on 2026-08-25, so that route is not merely
    /// untested — it is empty by construction, and
    /// [`the_painter_draws_every_slot_the_schema_declares`] is what holds it
    /// empty. Deleting this test with it would have dropped the surviving
    /// route, which is the one a host painter's invented slot name actually
    /// takes.
    ///
    /// `glow` is the fixture: declared by nobody, so tree acceptance checks
    /// only that the *token* exists, and the painter reports the slot rather
    /// than dropping it on the floor.
    ///
    /// The guard is the two assertions below, not this comment. This fixture
    /// was `shadow` until the painter learned to draw one, then `highlight`
    /// until the slot was retired; both times a green test silently stopped
    /// testing the route its own doc claimed, and both times it was an
    /// assertion about the schema that caught it.
    #[test]
    fn a_genuinely_unknown_token_slot_is_recorded() {
        use gorgon_petra::token::standard_slots;
        assert!(
            !standard_slots().contains("glow"),
            "this fixture needs a slot nothing declares; if `glow` is ever \
             added to the schema, pick another invented name"
        );
        assert!(
            !super::KNOWN_SLOTS.contains(&"glow"),
            "and one this painter does not draw"
        );

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props {
            text: Some("hi".into()),
            ..Props::default()
        };
        props.tokens.insert("glow".into(), tok("text.muted"));
        let frame = frame_of(
            &ViewNode::new(NodeKind::Text, "t").with_props(props),
            &mut h,
        );
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());

        assert!(report.unknown_slots.contains("glow"), "{report:?}");
        assert!(
            !report.unknown_slots.contains("radius"),
            "radius is a known slot now (C17): {report:?}"
        );
        // The text still painted, so the placement is drawn, not silent — the
        // unknown slot is a gap in the painter, not a lost placement.
        assert_eq!(report.drawn, 1, "{report:?}");
    }

    /// The schema and the painter agree, exactly, in both directions.
    ///
    /// **This is the invariant FR-025 bought and the reason the five retired
    /// slots cannot quietly come back.** `token::slot` declared eleven slots
    /// against this painter's six for as long as nothing compared the two
    /// sets, and each of the five extras read as a considered commitment
    /// rather than as a gap — a schema entry is indistinguishable from a
    /// promise until something asks the painter whether it can keep it.
    ///
    /// Both directions matter and they fail for different reasons. A slot
    /// declared and undrawn is a promise to an author that lands in
    /// `PaintReport::unknown_slots` at runtime; a slot drawn and undeclared is
    /// a painter feature that tree acceptance will not let anybody reach.
    ///
    /// This assertion has to live in `gorgon-petra-egui`, because
    /// `gorgon-petra` does not depend on it and so cannot see `KNOWN_SLOTS`.
    /// Its sibling half — that the schema is exactly six named entries — is
    /// `token::slot::tests::the_shipped_schema_is_exactly_what_the_painter_draws`.
    #[test]
    fn the_painter_draws_every_slot_the_schema_declares() {
        use gorgon_petra::token::{SlotSpec, standard_slots};
        let schema = standard_slots();

        let mut declared: Vec<&str> = schema.slots().map(SlotSpec::name).collect();
        let mut drawn: Vec<&str> = super::KNOWN_SLOTS.to_vec();
        declared.sort_unstable();
        drawn.sort_unstable();
        assert_eq!(
            declared, drawn,
            "the shipped slot schema and this painter's KNOWN_SLOTS have \
             drifted apart. A slot only one side knows is either a promise to \
             an author the painter cannot keep, or a painter feature tree \
             acceptance will not let anybody bind."
        );
    }

    /// C17's proof. A node binding `radius` must get the token's own corner,    /// C17's proof. A node binding `radius` must get the token's own corner,
    /// not a square one — read from the shape egui actually received, the
    /// same standard `a_border_stroke_is_a_whole_number_of_device_pixels`
    /// and the focus-ring tests hold themselves to, rather than trusted from
    /// the report alone.
    #[test]
    fn a_bound_radius_paints_the_named_corner() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("surface.base"));
        props.tokens.insert("radius".into(), tok("shape.corner-lg"));
        let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        let frame = frame_of(&node, &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());

        assert!(!report.unknown_slots.contains("radius"), "{report:?}");
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");
        assert_eq!(report.fills, 1, "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let rect_shape = out
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                Shape::Rect(r) => Some(r.clone()),
                _ => None,
            })
            .expect("the fill must reach egui as a Shape::Rect");
        out.drop_without_applying_deltas();

        // shape.corner-lg is 12.0 in both shipped themes (corner radius is
        // geometry, not colour, so it does not vary by mode) and
        // `CornerRadius::from(f32)` rounds to `u8`.
        assert_eq!(
            rect_shape.corner_radius,
            CornerRadius::from(12.0_f32),
            "the painted rect did not carry the token's own radius"
        );
    }

    // --- Focus ------------------------------------------------------------
    //
    // Keyboard focus that cannot be seen is not usable, and until this ring
    // existed a focused node painted exactly the shapes an unfocused one did:
    // `scratch_a_focused_node_paints_the_same_shapes` asserted that equality
    // and passed. The tests below are that test inverted, and they count the
    // shapes egui actually received rather than trusting the report.

    /// A focusable node, so the state's focused id names something real.
    fn button() -> ViewNode {
        use gorgon_petra::tree::{Interaction, Role};
        ViewNode::new(NodeKind::Text, "root")
            .with_props(Props {
                text: Some("Run".into()),
                ..Props::default()
            })
            .interactive(
                Role::Button,
                "Run",
                &[Interaction::Click, Interaction::Focus],
            )
    }

    /// Paint `node` with `focused` in force, and report both what egui
    /// received and what the pass said about itself.
    fn paint_focused(
        node: &ViewNode,
        focused: Option<&str>,
        colors: &dyn TokenSource,
    ) -> (usize, super::PaintReport) {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = focused.map(str::to_owned);
        let frame = frame_of(node, &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, colors);
        let out = host.0.run_ui(RawInput::default(), |_| {});
        let shapes = out.shapes.len();
        out.drop_without_applying_deltas();
        (shapes, report)
    }

    /// The three bands tile the ring with no seam: no pixel of the node's own
    /// fill survives between them.
    ///
    /// # The bug, and why it was visible before it was findable
    ///
    /// `FocusRing::bands` returns three contiguous spans — `[+1,+2]`,
    /// `[-1,+1]`, `[-2,-1]` around the edge. The painter snapped each band's
    /// rect to the device grid *separately*, and the halo centrelines sit on
    /// half-units by construction, so rounding walked the outer band out by
    /// half a pixel and the inner band in by half. That opened a one-pixel
    /// gap on the inside, and the button's own fill showed through the middle
    /// of its focus ring.
    ///
    /// It was reported as the button "bleeding out of its border" and
    /// confirmed by reading a real capture down a straight edge: card, core,
    /// **fill**, halo, fill — where the ring should be halo, core, halo with
    /// nothing between.
    ///
    /// # What is asserted
    ///
    /// The bands' device-space spans, sorted, must join end to end. That is
    /// the property `FocusRing::bands` guarantees and the painter has to
    /// preserve; a gap anywhere in it is the defect, whatever caused it.
    /// Checking spans rather than pixels is deliberate — a pixel assertion
    /// would also depend on the fill colour differing from both ring tokens,
    /// which is a different claim and true only by luck.
    #[test]
    fn the_focus_rings_three_bands_leave_no_gap_for_the_fill_to_show_through() {
        use gorgon_petra::tree::{Interaction, Role};

        let mut props = Props {
            text: Some("Run".into()),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        let node = ViewNode::new(NodeKind::Text, "root")
            .with_props(props)
            .interactive(
                Role::Button,
                "Run",
                &[Interaction::Click, Interaction::Focus],
            );

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = Some("/root".to_owned());
        let frame = frame_of(&node, &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(report.focus_rings, 1, "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        // Each band's top stroke spans [top - w/2, top + w/2] vertically.
        let mut spans: Vec<(f32, f32)> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r) if r.stroke.width > 0.0 => {
                    let half = r.stroke.width / 2.0;
                    Some((r.rect.top() - half, r.rect.top() + half))
                }
                _ => None,
            })
            .collect();
        out.drop_without_applying_deltas();
        assert_eq!(spans.len(), 3, "three bands: {spans:?}");
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));

        for pair in spans.windows(2) {
            let (_, prev_end) = pair[0];
            let (next_start, _) = pair[1];
            assert!(
                (next_start - prev_end).abs() < f32::EPSILON,
                "the ring has a {:.2}-unit seam between {:?} and {:?} (all \
                 spans: {spans:?}). A gap here is a stripe of the node's own \
                 fill running through the middle of its focus ring.",
                next_start - prev_end,
                pair[0],
                pair[1],
            );
        }
    }

    /// The ring is concentric with the node's own corners, not a square
    /// drawn around them.
    ///
    /// # What this looked like when it was wrong
    ///
    /// `paint_focus_ring` passed a hardcoded `0.0` corner radius until
    /// 2026-08-25. A focused `component::button` — `shape.corner-md`, eight
    /// units of rounding — therefore wore a hard rectangle, while the two
    /// unfocused buttons beside it kept their corners. The reported symptom
    /// was not "the focus ring is wrong"; it was that the primary button
    /// looked like a different component from its own neighbours.
    ///
    /// It survived because every existing ring test counts bands, checks
    /// colours or checks the report, and a square ring has exactly as many
    /// bands in exactly the right colours as a rounded one.
    ///
    /// # What is asserted
    ///
    /// The three radii egui received, against the three the geometry
    /// defines: the node's own radius plus `FocusBand::radius_delta`, which
    /// is `-1.5`, `0.0`, `+1.5` for the shipped ring. Checking all three
    /// rather than "not zero" is what catches a ring that rounds but is not
    /// *concentric* — one that bulges at the corners because every band took
    /// the node's radius unchanged.
    #[test]
    fn the_focus_ring_follows_the_corners_of_the_node_it_rings() {
        use gorgon_petra::token::focus::FocusRing;
        use gorgon_petra::tree::{Interaction, Role};

        let radius = snapshot()
            .radius("shape.corner-md")
            .expect("the shipped shape ramp declares shape.corner-md");
        let mut props = Props {
            text: Some("Run".into()),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        props.tokens.insert("radius".into(), tok("shape.corner-md"));
        let node = ViewNode::new(NodeKind::Text, "root")
            .with_props(props)
            .interactive(
                Role::Button,
                "Run",
                &[Interaction::Click, Interaction::Focus],
            );

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = Some("/root".to_owned());
        let frame = frame_of(&node, &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(report.focus_rings, 1, "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        // The three stroked rects are the ring; the node's own fill is
        // filled, not stroked, so `stroke.width > 0.0` selects the bands.
        let mut ring: Vec<u8> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r) if r.stroke.width > 0.0 => Some(r.corner_radius.nw),
                _ => None,
            })
            .collect();
        out.drop_without_applying_deltas();
        ring.sort_unstable();

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mut want: Vec<u8> = FocusRing::STANDARD
            .bands(gorgon_petra::geom::Rect::new(0.0, 0.0, 80.0, 30.0))
            .iter()
            .map(|band| (radius + band.offset).max(0.0).round() as u8)
            .collect();
        want.sort_unstable();

        assert_eq!(
            ring, want,
            "the ring's three bands must be concentric with the node's own \
             {radius}-unit corners. All-zero is the square-ring bug; all-equal \
             is a ring that rounds without being concentric and bulges at the \
             corners."
        );
    }

    /// The headline claim: focusing a node changes the picture, by three
    /// strokes that were not there before.
    #[test]
    fn a_focused_node_paints_a_ring_an_unfocused_one_does_not() {
        let (blind, unfocused) = paint_focused(&button(), None, &snapshot());
        let (ringed, focused) = paint_focused(&button(), Some("/root"), &snapshot());

        assert!(blind > 0, "the fixture must paint something at all");
        assert_eq!(
            ringed,
            blind + 3,
            "the ring is three bands — halo, core, halo — and every one of \
             them must reach egui: {unfocused:?} then {focused:?}"
        );
        assert_eq!(focused.focus_rings, 1, "{focused:?}");
        assert_eq!(focused.blind_focus, 0, "{focused:?}");
        assert_eq!(unfocused.focus_rings, 0, "{unfocused:?}");
        assert!(focused.is_complete(), "{focused:?}");
        assert!(
            focused.unresolved_tokens.is_empty(),
            "the shipped theme defines the ring: {focused:?}"
        );
    }

    /// FR-015: the indicator must not be a colour swap. Whatever the node
    /// binds — including nothing at all — the ring is extra geometry on top,
    /// so the two states differ in shape and not only in hue.
    #[test]
    fn the_ring_lands_on_a_node_whatever_background_it_binds() {
        let plain = ViewNode::new(NodeKind::Text, "root").with_props(Props {
            text: Some("Run".into()),
            ..Props::default()
        });
        let mut dark_bg = Props {
            text: Some("Run".into()),
            ..Props::default()
        };
        dark_bg
            .tokens
            .insert("background".into(), tok("surface.base"));
        let mut light_bg = Props {
            text: Some("Run".into()),
            ..Props::default()
        };
        light_bg
            .tokens
            .insert("background".into(), tok("status.degraded"));

        for (what, node) in [
            ("no background token", plain),
            (
                "a dark background",
                ViewNode::new(NodeKind::Text, "root").with_props(dark_bg),
            ),
            (
                "a light background",
                ViewNode::new(NodeKind::Text, "root").with_props(light_bg),
            ),
        ] {
            let (blind, _) = paint_focused(&node, None, &snapshot());
            let (ringed, report) = paint_focused(&node, Some("/root"), &snapshot());
            assert_eq!(
                ringed,
                blind + 3,
                "{what}: the ring must be drawn on top of whatever the node \
                 draws, not instead of it: {report:?}"
            );
            assert_eq!(report.focus_rings, 1, "{what}: {report:?}");
        }
    }

    /// Only the focused node is ringed. A pass that ringed every placement
    /// would pass the test above and box the whole window.
    #[test]
    fn exactly_one_ring_is_drawn_for_one_focused_node() {
        let node = ViewNode::new(NodeKind::Stack, "root")
            .child(button())
            .child(ViewNode::new(NodeKind::Text, "other").with_props(Props {
                text: Some("Stop".into()),
                ..Props::default()
            }));
        let (_, report) = paint_focused(&node, Some("/root/root"), &snapshot());
        assert_eq!(report.placements, 3);
        assert_eq!(report.focus_rings, 1, "{report:?}");
    }

    /// A theme with no ring colours leaves keyboard operation blind, and that
    /// is a failed pass rather than a note in a set. Without this the ring
    /// could silently stop being drawn and every gate would stay green.
    #[test]
    fn a_theme_without_the_ring_tokens_reports_a_blind_focus() {
        struct NoFocusColors;
        impl TokenSource for NoFocusColors {
            fn color(&self, token: &str) -> Option<Color32> {
                if token.starts_with("focus.") {
                    return None;
                }
                Some(Color32::from_rgb(0x80, 0x80, 0x80))
            }
        }

        let (shapes, report) = paint_focused(&button(), Some("/root"), &NoFocusColors);
        let (blind, _) = paint_focused(&button(), None, &NoFocusColors);
        assert_eq!(shapes, blind, "no band was drawn: {report:?}");
        assert_eq!(report.blind_focus, 1, "{report:?}");
        assert_eq!(report.focus_rings, 0, "{report:?}");
        assert!(
            !report.is_complete(),
            "a focused node with no ring is not a complete pass: {report:?}"
        );
        assert!(
            report.unresolved_tokens.contains("focus.ring"),
            "{report:?}"
        );
        assert!(
            report.unresolved_tokens.contains("focus.ring-halo"),
            "{report:?}"
        );
    }

    /// The two arrays are public fields on `PetrifiedFrame`, they are zipped
    /// to pair them, and a zip truncates in silence. A frame whose tail has no
    /// content would simply stop being painted.
    #[test]
    fn a_frame_whose_arrays_disagree_in_length_is_not_complete() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut frame = frame_of(&tree(), &mut h);
        frame.content.pop();
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());

        assert!(report.desynced, "{report:?}");
        assert!(!report.is_complete(), "{report:?}");
    }

    // --- Hosted content: custom painter dispatch (T080) --------------------

    fn custom_node(kind: &str) -> ViewNode {
        ViewNode::new(NodeKind::Stack, "root").child(
            ViewNode::new(NodeKind::Custom, "chart").with_props(Props {
                custom_kind: Some(kind.into()),
                ..Props::default()
            }),
        )
    }

    /// A [`gorgon_petra::tree::Registry`] that accepts `kind` as a valid
    /// custom node — tree acceptance, the thing `gallery.rs` calls
    /// `register_custom_kind` for. Deliberately a different registration
    /// from [`CustomPainters`]: a tree can accept a kind no painter answers
    /// for, which is exactly the case `a_registered_kind_with_no_painter_is_still_undrawn`
    /// below exercises.
    fn registry_with(kind: &str) -> gorgon_petra::tree::Registry {
        let mut registry = gorgon_petra::tree::Registry::new();
        registry.register_custom_kind(kind);
        registry
    }

    fn frame_of_registered(
        node: &ViewNode,
        h: &mut Harness<GalleyShaper, gorgon_petra::testing::NoRows>,
        registry: &gorgon_petra::tree::Registry,
    ) -> gorgon_petra::frame::PetrifiedFrame {
        petrify(
            1,
            gorgon_petra::testing::validated_with(node, registry),
            &mut h.ctx(),
            Viewport::new(Size::new(240.0, 120.0), ThemeMode::Dark),
            TransitionActivity::default(),
        )
    }

    /// What the recording painter in the test below captured from its
    /// [`CustomPaintCtx`]: rect, clip, opacity, scale, and whether the theme
    /// reached it. Named rather than spelled inline because `clippy::type_complexity`
    /// refuses the nested form, and rightly — the tuple says nothing about itself.
    type SeenCtx =
        std::rc::Rc<std::cell::RefCell<Option<(egui::Rect, egui::Rect, f32, Scale, bool)>>>;
    /// FR-059's headline claim: a host-registered painter draws into the
    /// same pass, at the same layer, and the placement it draws is counted
    /// as drawn — not undrawn, not silent.
    #[test]
    fn a_registered_custom_painter_draws_into_the_frame() {
        let host = Headless::new();
        let registry = registry_with("sparkline");
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of_registered(&custom_node("sparkline"), &mut h, &registry);
        let mut shaper = host.shaper();

        let mut painters = CustomPainters::new();
        painters.register(
            "sparkline",
            |painter: &egui::Painter, ctx: &CustomPaintCtx<'_>| {
                painter.rect_filled(ctx.rect, 0.0, Color32::WHITE);
                true
            },
        );
        let mut images = ImageSources::new();
        let report = paint_frame_with_hosts(
            &host.painter(),
            &frame,
            &mut shaper,
            &snapshot(),
            &painters,
            &mut images,
        );

        assert_eq!(report.customs, 1, "{report:?}");
        assert_eq!(report.drawn, 1, "{report:?}");
        assert!(report.undrawn.is_empty(), "{report:?}");
        assert!(report.is_complete(), "{report:?}");
    }

    /// FR-059's fifth item, and the one that did not land beside the other
    /// four: the painter is handed **the theme snapshot**, not two slices of
    /// it. A painter that reads a `spacing.*` and a `typography.*` token off
    /// the context it is given must get the values the theme actually holds.
    /// Without this, a hosted region cannot sit on the design system's ramp
    /// and has to spell literals instead — inside `petra-egui/src`, which
    /// the `literal-style` lane does not read, so nothing would catch them.
    #[test]
    fn a_custom_painter_reads_spacing_and_typography_off_the_theme() {
        /// What the recording painter took off its context. Named for the
        /// same reason [`SeenCtx`] is: `clippy::type_complexity` refuses the
        /// nested form spelled inline.
        type SeenTokens = std::rc::Rc<
            std::cell::RefCell<Option<(Option<f32>, Option<gorgon_petra::token::TypographyValue>)>>,
        >;

        let host = Headless::new();
        let registry = registry_with("sparkline");
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of_registered(&custom_node("sparkline"), &mut h, &registry);
        let mut shaper = host.shaper();

        let seen: SeenTokens = std::rc::Rc::new(std::cell::RefCell::new(None));
        let sink = std::rc::Rc::clone(&seen);
        let mut painters = CustomPainters::new();
        painters.register(
            "sparkline",
            move |painter: &egui::Painter, ctx: &CustomPaintCtx<'_>| {
                *sink.borrow_mut() = Some((
                    ctx.tokens.spacing("spacing-02"),
                    ctx.tokens.typography("typography.body"),
                ));
                painter.rect_filled(ctx.rect, 0.0, Color32::WHITE);
                true
            },
        );

        let mut images = ImageSources::new();
        let theme = snapshot();
        let report = paint_frame_with_hosts(
            &host.painter(),
            &frame,
            &mut shaper,
            &theme,
            &painters,
            &mut images,
        );
        assert_eq!(report.customs, 1, "the painter must have run: {report:?}");

        let (gap, body) = seen.borrow().expect("the painter ran, so it recorded");

        // The oracle is the snapshot's own accessors rather than a number
        // copied out of `shipped.rs`: this asserts the painter reads *the
        // theme*, and keeps telling the truth if the ramp is re-tuned.
        let expected_gap = ThemeSnapshot::spacing(&theme, &tok("spacing-02"));
        assert_eq!(
            gap, expected_gap,
            "the painter must read the theme's spacing ramp, not a number of its own"
        );
        assert!(
            gap.is_some_and(|g| g > 0.0),
            "the shipped theme defines spacing-02, so a `None` here means the \
             context never carried the ramp at all: {gap:?}"
        );

        let expected_body = match ThemeSnapshot::value(&theme, &tok("typography.body")) {
            Some(gorgon_petra::token::TokenValue::Typography(style)) => Some(*style),
            _ => None,
        };
        assert_eq!(
            body, expected_body,
            "the painter must read the theme's type ramp"
        );
        assert!(
            body.is_some_and(|b| b.size > 0.0),
            "the shipped theme defines typography.body: {body:?}"
        );
    }

    /// FR-059: a custom kind the tree accepts (`register_custom_kind`) but
    /// no host painter answers for is still `undrawn`, never a silent
    /// blank. This is exactly `petra-egui/examples/gallery.rs`'s
    /// `sparkline` section today: accepted by the tree, unregistered in the
    /// paint dispatch.
    #[test]
    fn a_registered_kind_with_no_painter_is_still_undrawn() {
        let host = Headless::new();
        let registry = registry_with("sparkline");
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of_registered(&custom_node("sparkline"), &mut h, &registry);
        let mut shaper = host.shaper();

        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());

        assert!(report.undrawn.contains("custom:sparkline"), "{report:?}");
        assert_eq!(report.silent, 1, "{report:?}");
        assert!(!report.is_complete(), "{report:?}");
    }

    /// The other half of "registered-but-unpainted stays undrawn": a
    /// painter that *is* registered but draws nothing — a sparkline handed
    /// zero data points, say — must be treated exactly like no painter at
    /// all, not as a pass. FR-059 makes this required behaviour, and this
    /// is the test A1-7's sabotage targets: an `if let Some(paint) = ...`
    /// that ignores the returned `bool` would make this frame read as
    /// complete over content nothing actually drew.
    #[test]
    fn a_painter_that_draws_nothing_is_still_undrawn() {
        let host = Headless::new();
        let registry = registry_with("sparkline");
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of_registered(&custom_node("sparkline"), &mut h, &registry);
        let mut shaper = host.shaper();

        let mut painters = CustomPainters::new();
        painters.register(
            "sparkline",
            |_painter: &egui::Painter, _ctx: &CustomPaintCtx<'_>| {
                // Registered, ran, and had nothing to draw this frame.
                false
            },
        );
        let mut images = ImageSources::new();
        let report = paint_frame_with_hosts(
            &host.painter(),
            &frame,
            &mut shaper,
            &snapshot(),
            &painters,
            &mut images,
        );

        assert!(report.undrawn.contains("custom:sparkline"), "{report:?}");
        assert_eq!(report.customs, 0, "{report:?}");
        assert_eq!(report.silent, 1, "{report:?}");
        assert!(!report.is_complete(), "{report:?}");
    }

    /// FR-059's field list, checked rather than assumed: a custom painter
    /// receives the resolved rect, the clip, the opacity, the device scale,
    /// and a colour source that resolves the same theme the rest of the
    /// frame painted with.
    #[test]
    fn a_custom_painter_receives_the_resolved_geometry_and_theme() {
        let host = Headless::new();
        let registry = registry_with("sparkline");
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of_registered(&custom_node("sparkline"), &mut h, &registry);
        let mut shaper = host.shaper();
        let expected = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/chart"))
            .expect("the custom node placed")
            .rect;
        let expected_rect = to_egui_snapped(expected, frame.viewport.scale);

        let seen: SeenCtx = std::rc::Rc::default();
        let recorder = std::rc::Rc::clone(&seen);
        let mut painters = CustomPainters::new();
        painters.register(
            "sparkline",
            move |_painter: &egui::Painter, ctx: &CustomPaintCtx<'_>| {
                *recorder.borrow_mut() = Some((
                    ctx.rect,
                    ctx.clip,
                    ctx.opacity,
                    ctx.scale,
                    ctx.tokens.color("surface.base").is_some(),
                ));
                true
            },
        );
        let mut images = ImageSources::new();
        let report = paint_frame_with_hosts(
            &host.painter(),
            &frame,
            &mut shaper,
            &snapshot(),
            &painters,
            &mut images,
        );
        assert_eq!(report.customs, 1, "{report:?}");

        let (rect, clip, opacity, scale, saw_theme_color) =
            seen.borrow().expect("the painter must have run");
        assert_eq!(
            rect, expected_rect,
            "the resolved rect must match the placement"
        );
        assert!(
            clip.is_positive(),
            "a node nothing has clipped away must still carry a positive clip: {clip:?}"
        );
        assert!(
            clip.contains_rect(rect),
            "the clip in force must cover the rect it accompanies: clip {clip:?}, rect {rect:?}"
        );
        assert!((opacity - 1.0).abs() < 1e-6, "opacity {opacity}");
        assert_eq!(
            scale, frame.viewport.scale,
            "the device scale must be the frame's"
        );
        assert!(
            saw_theme_color,
            "the painter must be able to resolve a real theme colour, not a fake one"
        );
    }

    // --- Hosted content: image dispatch (T081) ------------------------------

    fn image_node(source: &str) -> ViewNode {
        ViewNode::new(NodeKind::Stack, "root").child(
            ViewNode::new(NodeKind::Image, "logo").with_props(Props {
                image: Some(source.into()),
                ..Props::default()
            }),
        )
    }

    /// T081's headline claim: a source resolved through a registered
    /// `ImageSources` draws, and the placement is counted as drawn.
    #[test]
    fn a_registered_image_source_draws_and_is_not_undrawn() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of(&image_node("gallery/logo"), &mut h);
        let mut shaper = host.shaper();

        let mut images = ImageSources::new();
        images.register(
            "gallery/logo",
            ImagePixels::new(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]),
        );
        let painters = CustomPainters::new();
        let report = paint_frame_with_hosts(
            &host.painter(),
            &frame,
            &mut shaper,
            &snapshot(),
            &painters,
            &mut images,
        );

        assert_eq!(report.images, 1, "{report:?}");
        assert!(report.undrawn.is_empty(), "{report:?}");
        assert!(report.is_complete(), "{report:?}");
    }

    /// T081: a source with nothing registered for it is named by itself in
    /// `undrawn` — not by the bare "image" kind, so an operator can tell
    /// *which* picture failed to load.
    #[test]
    fn an_unresolvable_image_source_is_named_by_itself() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of(&image_node("gallery/missing"), &mut h);
        let mut shaper = host.shaper();

        // A registry that has *something* registered, just not this name —
        // proves the miss is per-source, not "the registry is empty".
        let mut images = ImageSources::new();
        images.register("gallery/logo", ImagePixels::new(1, 1, vec![0, 0, 0, 255]));
        let painters = CustomPainters::new();
        let report = paint_frame_with_hosts(
            &host.painter(),
            &frame,
            &mut shaper,
            &snapshot(),
            &painters,
            &mut images,
        );

        assert_eq!(report.images, 0, "{report:?}");
        assert!(
            report.undrawn.contains("image:gallery/missing"),
            "{report:?}"
        );
        assert!(!report.is_complete(), "{report:?}");
    }

    /// A source registered with pixels that do not decode (wrong byte
    /// count for the declared dimensions) fails to resolve exactly like an
    /// unregistered one — named, not silently blank, not a panic.
    #[test]
    fn a_source_that_fails_to_decode_is_named_in_undrawn() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of(&image_node("gallery/broken"), &mut h);
        let mut shaper = host.shaper();

        let mut images = ImageSources::new();
        images.register("gallery/broken", ImagePixels::new(4, 4, vec![1, 2, 3]));
        let painters = CustomPainters::new();
        let report = paint_frame_with_hosts(
            &host.painter(),
            &frame,
            &mut shaper,
            &snapshot(),
            &painters,
            &mut images,
        );

        assert!(
            report.undrawn.contains("image:gallery/broken"),
            "{report:?}"
        );
        assert!(!report.is_complete(), "{report:?}");
    }

    // ---------------------------------------------------------------
    // FR-015: the shape channel has to reach the screen.
    //
    // The tests below measure *coverage*, not declarations. Every earlier
    // check on this channel read a token name off a `ViewNode` or a corner
    // radius off a `Shape::Rect`, which is exactly how four `StatusShape`
    // variants shipped as two pictures — and how those two differed by
    // 0.414 logical units, under one device pixel at scale 1.0. So these
    // run the real paint pass, hand the emitted shapes to egui's own
    // tessellator, and compare the triangles it produces.
    // ---------------------------------------------------------------

    /// Half a logical unit per sample: four samples per device pixel at
    /// scale 1.0, fine enough that the measured fractions below sit within
    /// a point or two of the figures' analytic areas.
    const SAMPLE_PITCH: f32 = 0.5;

    /// The fraction of a marker's box on which two markers must disagree to
    /// count as different silhouettes.
    ///
    /// Calibrated against the defect, not against the fix. A corner radius
    /// is the only channel this library had before the `silhouette` slot
    /// existed, and on the two markers here the largest difference a corner
    /// radius alone can produce is:
    ///
    /// * 0.060 of the box — a 10x10 status dot at `shape.corner-sm` against
    ///   the same dot at `shape.corner-full`;
    /// * 0.104 of the box — a 12x12 checkbox at `shape.corner-sm` against a
    ///   12x12 radio at `shape.corner-full`.
    ///
    /// Both figures are measured, not estimated: reverting this leaf's two
    /// component files to their previous bindings and re-running the two
    /// tests below prints exactly those numbers, and both tests fail.
    ///
    /// Both of those are pictures the review measured as differing by under
    /// one device pixel of outline deviation (0.414 and 0.828 logical units)
    /// — that is, indistinguishable. The bar sits above both, so a pair that
    /// separates only by rounding its corners fails here. Every pair this
    /// library actually paints clears 0.21, so the bar is not tight against
    /// the fix either; both margins are printed by the tests below.
    const MIN_DISTINCT_FRACTION: f64 = 0.15;

    /// Whether `p` is inside the triangle `(a, b, c)`, winding-agnostic.
    fn in_triangle(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> bool {
        let side =
            |u: egui::Pos2, v: egui::Pos2| (v.x - u.x) * (p.y - u.y) - (v.y - u.y) * (p.x - u.x);
        let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(neg && pos)
    }

    /// Paint `node` and return which samples inside the placement whose id
    /// ends with `marker`, sampled at [`SAMPLE_PITCH`], the renderer
    /// actually covered.
    ///
    /// The coverage comes from `Context::tessellate` — the same call an
    /// eframe backend makes on its way to the GPU — with feathering turned
    /// off, so an anti-aliasing fringe cannot inflate a difference between
    /// two silhouettes. Sampling is confined to the marker's own rect, so
    /// the label beside it contributes nothing.
    fn marker_coverage(node: ViewNode, marker: &str) -> Vec<bool> {
        let host = Headless::new();
        host.0
            .tessellation_options_mut(|options| options.feathering = false);
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let tree = ViewNode::new(NodeKind::Stack, "root").child(node);
        let frame = frame_of(&tree, &mut h);
        let placement = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(marker))
            .unwrap_or_else(|| panic!("no placement id ends with {marker:?}"));
        let box_rect = to_egui_snapped(placement.rect, Scale::new(1.0).expect("scale 1 is legal"));

        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(
            report.unresolved_tokens.is_empty(),
            "every token this marker binds must resolve: {report:?}"
        );

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let ppp = out.pixels_per_point;
        let primitives = host.0.tessellate(out.shapes.clone(), ppp);
        out.drop_without_applying_deltas();

        let mut triangles: Vec<[egui::Pos2; 3]> = Vec::new();
        for clipped in &primitives {
            if let egui::epaint::Primitive::Mesh(mesh) = &clipped.primitive {
                for tri in mesh.indices.chunks_exact(3) {
                    triangles.push([
                        mesh.vertices[tri[0] as usize].pos,
                        mesh.vertices[tri[1] as usize].pos,
                        mesh.vertices[tri[2] as usize].pos,
                    ]);
                }
            }
        }
        assert!(
            !triangles.is_empty(),
            "the pass emitted no geometry at all for {marker:?}"
        );

        let cols = (box_rect.width() / SAMPLE_PITCH).round() as usize;
        let rows = (box_rect.height() / SAMPLE_PITCH).round() as usize;
        let mut covered = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                #[allow(clippy::cast_precision_loss)]
                let p = egui::pos2(
                    box_rect.left() + (col as f32 + 0.5) * SAMPLE_PITCH,
                    box_rect.top() + (row as f32 + 0.5) * SAMPLE_PITCH,
                );
                covered.push(triangles.iter().any(|t| in_triangle(p, t[0], t[1], t[2])));
            }
        }
        covered
    }

    /// The fraction of a marker's box on which two coverage maps disagree.
    fn disagreement(a: &[bool], b: &[bool]) -> f64 {
        assert_eq!(a.len(), b.len(), "coverage maps must be the same shape");
        let differing = a.iter().zip(b).filter(|(x, y)| x != y).count();
        differing as f64 / a.len() as f64
    }

    /// All four `StatusShape` variants paint four different pictures.
    ///
    /// This is FR-015's non-colour channel measured where it has to be true:
    /// on the tessellated geometry. `Circle` and `Square` are two ends of the
    /// rect family (`shape.corner-full` and `shape.corner-none`); `Triangle`
    /// and `Diamond` are figures no corner radius can reach, and they arrive
    /// through the `silhouette` slot.
    ///
    /// The pair that matters most is `Triangle` against `Square`: the shipped
    /// light theme paints `status.degraded` an orange-red and `status.down` a
    /// dark red, so those two markers are the ones a red-green colourblind
    /// operator has to tell apart without hue.
    #[test]
    fn every_status_shape_paints_a_distinguishable_marker() {
        use gorgon_petra::component::status;
        use gorgon_petra::token::{StatusShape, StatusToken};

        let cases = [
            ("status.ok", StatusShape::Circle),
            ("status.degraded", StatusShape::Triangle),
            ("status.down", StatusShape::Square),
            ("status.ok", StatusShape::Diamond),
        ];
        let maps: Vec<(StatusShape, Vec<bool>)> = cases
            .iter()
            .map(|(token, shape)| {
                let st = StatusToken::new(tok(token), *shape, "S")
                    .expect("the fixture text is non-empty");
                (*shape, marker_coverage(status("s", &st), "/s/dot"))
            })
            .collect();

        for (shape, map) in &maps {
            #[allow(clippy::cast_precision_loss)]
            let filled = map.iter().filter(|c| **c).count() as f64 / map.len() as f64;
            println!("{shape:?}: covers {filled:.3} of its box");
        }
        for i in 0..maps.len() {
            for j in (i + 1)..maps.len() {
                let differing = disagreement(&maps[i].1, &maps[j].1);
                println!(
                    "{:?} vs {:?}: {differing:.3} of the box differs (bar {MIN_DISTINCT_FRACTION})",
                    maps[i].0, maps[j].0,
                );
                assert!(
                    differing >= MIN_DISTINCT_FRACTION,
                    "StatusShape::{:?} and StatusShape::{:?} paint the same marker: only \
                     {differing:.3} of the marker's box differs, at or below what a corner \
                     radius alone already produced, so the FR-015 shape channel does not \
                     survive to the screen for this pair",
                    maps[i].0,
                    maps[j].0,
                );
            }
        }
    }

    /// A checkbox and a radio paint two different boxes.
    ///
    /// `controls`'s module doc claims "a round control reads as 'one choice
    /// among several' the way a square one does not". Both boxes are 12x12,
    /// and until the checkbox lost its `shape.corner-sm` rounding the two
    /// outlines were 0.828 logical units apart at their widest — under one
    /// device pixel. Measured on coverage rather than on the corner radius,
    /// because the corner radius is the number that was already right.
    #[test]
    fn a_checkbox_and_a_radio_paint_distinguishable_boxes() {
        use gorgon_petra::component::{checkbox, radio};

        let cb = marker_coverage(checkbox("c", "Restart", true), "/c/box");
        let rb = marker_coverage(radio("r", "Restart", true), "/r/box");
        let differing = disagreement(&cb, &rb);
        #[allow(clippy::cast_precision_loss)]
        let (cb_fill, rb_fill) = (
            cb.iter().filter(|c| **c).count() as f64 / cb.len() as f64,
            rb.iter().filter(|c| **c).count() as f64 / rb.len() as f64,
        );
        println!(
            "checkbox covers {cb_fill:.3}, radio covers {rb_fill:.3}, {differing:.3} of the \
             box differs (bar {MIN_DISTINCT_FRACTION})"
        );
        assert!(
            differing >= MIN_DISTINCT_FRACTION,
            "checkbox and radio paint the same box: only {differing:.3} of it differs, at \
             or below what a corner radius alone already produced"
        );
    }

    /// A `silhouette` token the source cannot resolve is reported, and the
    /// node falls back to the rect every node drew before the slot existed.
    ///
    /// The fallback is the safe half; the report is the half that matters.
    /// A mistyped figure that silently became a square is the same defect
    /// this slot was added to close, one level down.
    #[test]
    fn an_unresolved_silhouette_is_reported_and_falls_back_to_a_rect() {
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("surface.base"));
        props
            .tokens
            .insert("silhouette".into(), tok("shape.silhouette-hexagon"));
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        let frame = petrify(
            1,
            gorgon_petra::testing::validated_with(
                &node,
                &gorgon_petra::tree::Registry::with_vocabulary({
                    let mut v = gorgon_petra::token::standard_vocabulary();
                    v.declare(gorgon_petra::token::DesignToken::new(
                        tok("shape.silhouette-hexagon"),
                        gorgon_petra::token::TokenKind::Silhouette,
                    ));
                    v
                }),
            ),
            &mut h.ctx(),
            Viewport::new(Size::new(240.0, 120.0), ThemeMode::Dark),
            TransitionActivity::default(),
        );
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());

        assert!(
            report
                .unresolved_tokens
                .contains("shape.silhouette-hexagon"),
            "an unresolvable figure must be named in the report: {report:?}"
        );
        assert_eq!(report.fills, 1, "the fallback still paints: {report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let rects = out
            .shapes
            .iter()
            .filter(|cs| matches!(cs.shape, Shape::Rect(_)))
            .count();
        out.drop_without_applying_deltas();
        assert_eq!(rects, 1, "the fallback figure is a rect, not a polygon");
    }
}
