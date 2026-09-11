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

use egui::{Color32, CornerRadius, Painter, Rgba, Stroke};
use gorgon_petra::frame::{PaintContent, PetrifiedFrame, Placement, round_rect};
use gorgon_petra::geom::{Rect as PetraRect, Scale};
use gorgon_petra::layout::TextRequest;
use gorgon_petra::token::value::CoverageValue;
use gorgon_petra::token::{
    DerivedState, FocusRing, InteractionRank, MotionValue, SHADOW_GEOMETRY, Silhouette,
    ThemeSnapshot, TokenName, TokenValue, TypographyValue, base_slot, resolve_slot, resolve_state,
};
use gorgon_petra::tree::{Edge, FocusFigure, FocusShownOn, NodeKind};

use crate::host::{COVERAGE_TOKEN, FALLBACK_COVERAGE, coverage_plan};
use crate::image::ImageSources;
use crate::text::GalleyShaper;

/// Token slot painted as a filled rect behind a node.
pub const BACKGROUND_SLOT: &str = "background";
/// Token slot painted as a one-unit outline around a node.
pub const BORDER_SLOT: &str = "border";
/// Token slot painted as a one-unit rule along the node's top edge only.
///
/// Carbon draws most of its rules on one edge — `border-block-end` under a
/// table row, `border-block-start` over a structured-list row,
/// `border-inline-start` beside a pagination button — and a four-sided
/// [`BORDER_SLOT`] on each of two adjacent rows drew every seam twice. The
/// four edge slots each stroke exactly one side of the same snapped rect
/// [`BORDER_SLOT`] would outline, at the same [`device_snapped_width`],
/// inset by half that width so the line sits inside the node the way an
/// inside-stroked outline does.
pub const BORDER_TOP_SLOT: &str = "border-top";
/// See [`BORDER_TOP_SLOT`]: the right edge only.
pub const BORDER_RIGHT_SLOT: &str = "border-right";
/// See [`BORDER_TOP_SLOT`]: the bottom edge only.
pub const BORDER_BOTTOM_SLOT: &str = "border-bottom";
/// See [`BORDER_TOP_SLOT`]: the left edge only.
pub const BORDER_LEFT_SLOT: &str = "border-left";
/// The four edge slots, in the order the painter walks them.
const EDGE_SLOTS: [(&str, Edge); 4] = [
    (BORDER_TOP_SLOT, Edge::Top),
    (BORDER_RIGHT_SLOT, Edge::Right),
    (BORDER_BOTTOM_SLOT, Edge::Bottom),
    (BORDER_LEFT_SLOT, Edge::Left),
];
/// Token slot used for a node's text.
pub const FOREGROUND_SLOT: &str = "foreground";
/// Token slot painted as a one-unit rule under each row of a node's text,
/// in the slot's own colour. Absent means no rule. Carbon's Link is
/// `text-decoration: underline` on hover, and always for its inline form;
/// this is that channel, and it takes `underline@hover` like any other slot.
pub const UNDERLINE_SLOT: &str = "underline";

/// Token slot painted as the corner radius of a node's background and
/// border. Resolves through a `shape.*` token (FR-053, C17); a node that
/// binds no `radius` paints square corners, the same default it had before
/// this slot existed.
pub const RADIUS_SLOT: &str = "radius";
/// Overrides [`RADIUS_SLOT`] for the node's top-left corner only. Absent,
/// [`RADIUS_SLOT`]'s shorthand applies — see [`resolve_corner_radius`].
pub const RADIUS_TOP_LEFT_SLOT: &str = "radius-top-left";
/// See [`RADIUS_TOP_LEFT_SLOT`]: the top-right corner.
pub const RADIUS_TOP_RIGHT_SLOT: &str = "radius-top-right";
/// See [`RADIUS_TOP_LEFT_SLOT`]: the bottom-right corner.
pub const RADIUS_BOTTOM_RIGHT_SLOT: &str = "radius-bottom-right";
/// See [`RADIUS_TOP_LEFT_SLOT`]: the bottom-left corner.
pub const RADIUS_BOTTOM_LEFT_SLOT: &str = "radius-bottom-left";
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

/// Token slot filled behind the stretch of a node's text the operator has
/// selected ([`gorgon_petra::frame::PaintContent::selection`]).
///
/// An **override**, not a requirement. A node that binds neither this nor
/// [`SELECTION_INK_SLOT`] takes [`DEFAULT_SELECTION_TOKEN`] and
/// [`DEFAULT_SELECTION_INK_TOKEN`]; a node that binds both takes what it
/// bound. Binding exactly one is a defect and is reported as an unresolved
/// slot rather than half-honoured, because a ground without its ink can sink
/// a coloured run below AA and an ink without its ground recolours text for
/// no visible reason.
pub const SELECTION_SLOT: &str = "selection";

/// The selection ground for every run that does not name its own.
///
/// # Why this is a default and not, like every other slot, a requirement
///
/// The painter invents no colours, and this does not: it names a token the
/// design system already ships, the way [`DEFAULT_TEXT_TOKEN`] does for ink
/// and [`crate::paint::focus`]'s ring does for the focus caret. What changed
/// is who has to say it. Selectability used to be declared per component, and
/// in the whole library exactly one component — the code snippet — ever
/// declared it. The operator, reading the catalog: *"a lot of these text
/// elements are not highlightable, like the lists. A lot are like this."*
///
/// # Why the accent and not a layer step
///
/// A selection has to be visible on `surface.base`, on all three layers, and
/// in both themes, and no single step off any one of those is a step off the
/// others — that is the whole reason the layer set alternates in light and
/// climbs in dark. A saturated fill is the only ground that reads against
/// every one of them, and it is what every desktop and every browser paints.
/// `text.on-accent` is defined *as* the ink for this fill, so the pair is
/// legible by construction rather than by measurement.
///
/// The code snippet keeps its own quieter pair: inside a well the highlight
/// is read against one known surface, and there a layer step is the better
/// picture. That is what the override is for.
pub const DEFAULT_SELECTION_TOKEN: &str = "accent.primary";

/// The ink a selected run takes over [`DEFAULT_SELECTION_TOKEN`].
pub const DEFAULT_SELECTION_INK_TOKEN: &str = "text.on-accent";
/// Token slot the selected stretch's glyphs are drawn in, over
/// [`SELECTION_SLOT`]'s fill.
///
/// A pair and not a single ground, because a ground dark enough to see is a
/// different ground from the one the text's contrast was measured against.
/// The code snippet's keyword ink clears AA on its own well by 0.13 in the
/// light theme, so every visible highlight would sink it; the selected run
/// takes this ink instead, exactly as `::selection { color }` does in a
/// browser. Both are resolved together and neither is used alone: a node
/// binding one and not the other gets no highlight and the missing slot is
/// reported. See [`DEFAULT_SELECTION_INK_TOKEN`] for what a node that binds
/// neither takes.
pub const SELECTION_INK_SLOT: &str = "selection-ink";

/// Every token slot this painter knows how to use. Anything else a node binds
/// lands in [`PaintReport::unknown_slots`] rather than being dropped on the
/// floor.
const KNOWN_SLOTS: &[&str] = &[
    BACKGROUND_SLOT,
    BORDER_SLOT,
    BORDER_TOP_SLOT,
    BORDER_RIGHT_SLOT,
    BORDER_BOTTOM_SLOT,
    BORDER_LEFT_SLOT,
    FOREGROUND_SLOT,
    UNDERLINE_SLOT,
    RADIUS_SLOT,
    RADIUS_TOP_LEFT_SLOT,
    RADIUS_TOP_RIGHT_SLOT,
    RADIUS_BOTTOM_RIGHT_SLOT,
    RADIUS_BOTTOM_LEFT_SLOT,
    SHADOW_SLOT,
    SILHOUETTE_SLOT,
    SELECTION_SLOT,
    SELECTION_INK_SLOT,
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

/// The horizontal inset between a placement's rect and the galley it paints.
///
/// Zero for everything but [`NodeKind::Input`], which is a leaf that refuses
/// padding: the chrome rect *is* the placement, so the text has to be moved
/// in by hand. Carbon's md field puts it `spacing-04` from each side.
///
/// A function rather than a literal in the paint path because the host has to
/// undo exactly this shift to turn a pointer position back into a byte
/// offset. Two copies of a number that must agree is one copy too many, and
/// the twelve units it is worth are exactly the width of a word.
#[must_use]
pub fn text_inset_x(kind: NodeKind, colors: &dyn TokenSource) -> f32 {
    if kind == NodeKind::Input {
        colors.spacing(FIELD_INSET_TOKEN).unwrap_or(FIELD_INSET)
    } else {
        0.0
    }
}

/// The token [`text_inset_x`] reads for a field's side inset.
pub const FIELD_INSET_TOKEN: &str = "spacing-04";

/// What [`text_inset_x`] falls back to for a source that does not name
/// [`FIELD_INSET_TOKEN`] — most of this module's own fixtures.
pub const FIELD_INSET: f32 = 12.0;

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

/// Resolves a node's full per-corner radius: each of the four corner slots
/// ([`RADIUS_TOP_LEFT_SLOT`] and its three siblings) overrides [`RADIUS_SLOT`]
/// for its own corner; a corner that binds nothing of its own takes the
/// shorthand, and a node that binds neither takes square — exactly the
/// picture a single `radius` binding always painted, which is why every one
/// of the 31 sites that bind only `radius` moves no pixel under this.
///
/// State-decorated (`radius@hover`, a corner slot's own `@hover`) resolves
/// through [`resolve_slot`]'s usual precedence chain before the shorthand
/// fallback is ever consulted, so a hovered node's per-corner override still
/// wins over its own resting shorthand.
fn resolve_corner_radius(
    tokens: &BTreeMap<String, String>,
    colors: &dyn TokenSource,
    state: DerivedState,
    report: &mut PaintReport,
) -> CornerRadius {
    let shorthand = resolve_slot(tokens, RADIUS_SLOT, state);
    let mut corner = |slot: &str| -> u8 {
        let token = resolve_slot(tokens, slot, state).or(shorthand);
        let radius = token.map_or(0.0, |token| resolve_radius_or_record(colors, token, report));
        radius.round().clamp(0.0, 255.0) as u8
    };
    CornerRadius {
        nw: corner(RADIUS_TOP_LEFT_SLOT),
        ne: corner(RADIUS_TOP_RIGHT_SLOT),
        se: corner(RADIUS_BOTTOM_RIGHT_SLOT),
        sw: corner(RADIUS_BOTTOM_LEFT_SLOT),
    }
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
        // Apex at the bottom edge's midpoint, base along the top edge.
        // A 2026-08-26 capture of the Fibers card read the up-pointing
        // triangle as a play/go mark sitting next to "Degraded". Pointing
        // it down is the warning-triangle orientation.
        Silhouette::Triangle => Some(vec![egui::pos2(l, t), egui::pos2(r, t), egui::pos2(cx, b)]),
        // One vertex at the midpoint of each edge.
        Silhouette::Diamond => Some(vec![
            egui::pos2(cx, t),
            egui::pos2(r, cy),
            egui::pos2(cx, b),
            egui::pos2(l, cy),
        ]),
        // Chamfered box, not a regular octagon. At 10×10 a regular
        // octagon (cut ≈ 0.29s) disagrees with a disc on only 8% of the
        // box, which is the same hole FR-015 closed when Circle and Square
        // used to share a rounded-rect. 0.22s of cut keeps the corners
        // empty (so it is not a square) and keeps the sides long (so it is
        // not a disc).
        Silhouette::Octagon => {
            let cut_x = (r - l) * 0.16;
            let cut_y = (b - t) * 0.16;
            Some(vec![
                egui::pos2(l + cut_x, t),
                egui::pos2(r - cut_x, t),
                egui::pos2(r, t + cut_y),
                egui::pos2(r, b - cut_y),
                egui::pos2(r - cut_x, b),
                egui::pos2(l + cut_x, b),
                egui::pos2(l, b - cut_y),
                egui::pos2(l, t + cut_y),
            ])
        }
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
    /// Canvases that emitted at least one shape from their draw list.
    pub canvases: usize,
    /// `Sprite` commands, across every canvas, whose asset did not resolve.
    ///
    /// Its own term rather than a line in [`PaintReport::undrawn`], because
    /// `contracts/draw-list.md` §10 asks for the count: a canvas drawing one
    /// rect and one missing sprite emits a shape, so it is `Drawn`, and
    /// without this number that half-drawn picture reads as a whole one.
    pub missing_assets: usize,
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
            && self.missing_assets == 0
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
    paint_frame_inner(painter, frame, shaper, colors, painters, images, None)
}

/// Draw `frame`, then the host's interpolated focus underline instead of a
/// jump onto `semantics.focused`.
///
/// [`crate::host::Host`] owns the caret. Paint tests that do not go through
/// the host keep calling [`paint_frame`], which still rings the focused
/// placement in one step.
pub(crate) fn paint_frame_with_caret(
    painter: &Painter,
    frame: &PetrifiedFrame,
    shaper: &mut GalleyShaper,
    colors: &dyn TokenSource,
    painters: &CustomPainters,
    images: &mut ImageSources,
    caret: Option<CaretOverlay>,
) -> PaintReport {
    paint_frame_inner(painter, frame, shaper, colors, painters, images, caret)
}

/// What the host wants drawn in place of a jump onto `semantics.focused`.
///
/// The two arms are the two states `focus_caret.rs` contracts: a settled
/// caret is the figure as it is designed, and a caret in flight is the four
/// sprung bands filled flat. Nothing moves between the two — the springs land
/// on the figure's own geometry — but the halo and the shadow are drawn only
/// when settled, because both answer questions about a control the operator
/// is looking at rather than about 160 ms of travel.
pub(crate) enum CaretPicture {
    /// Draw nothing. The bake pass uses this: the caret is drawn separately,
    /// after the scene is tessellated, so a hop is not frozen into the cache.
    Hidden,
    /// Settled on `node`, wearing `figure`.
    Settled {
        /// The rect the figure is drawn on, device-snapped.
        mark: egui::Rect,
        /// The shape drawn on it.
        figure: FocusFigure,
        /// That node's `radius` token, if any. Read only by
        /// [`FocusFigure::Border`].
        radius: Option<String>,
    },
    /// In flight: the four sprung bands, already interpolated, with any
    /// zero-extent ones dropped.
    Flying(Vec<egui::Rect>),
}

/// The host's caret this frame, and the clip it may not paint past.
pub(crate) struct CaretOverlay {
    /// What to draw.
    pub picture: CaretPicture,
    /// The clip the caret may not paint past.
    pub clip: egui::Rect,
}

fn paint_frame_inner(
    painter: &Painter,
    frame: &PetrifiedFrame,
    shaper: &mut GalleyShaper,
    colors: &dyn TokenSource,
    painters: &CustomPainters,
    images: &mut ImageSources,
    caret: Option<CaretOverlay>,
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
    }
    // The indicator is drawn after the loop rather than inside it because
    // it reaches outside its node's rect, and a later sibling's fill would
    // paint over one drawn in tree order. Either the host's in-flight bars,
    // or a jump onto the settled target: one target, resolved once, by
    // [`focused_caret_target`].
    let focused_off_screen = frame
        .placements
        .iter()
        .any(|p| p.semantics.focused && !p.is_visible());
    let painted = match caret {
        Some(overlay) => paint_caret_overlay(painter, &overlay, colors, scale, &mut report),
        None => focused_caret_target(frame).is_some_and(|target| {
            paint_focus_target(painter, &target, env.page, colors, scale, &mut report)
        }),
    };
    if painted {
        report.focus_rings += 1;
    } else if focused_off_screen
        || report
            .unresolved_tokens
            .contains(gorgon_petra::token::focus::RING_TOKEN)
    {
        report.blind_focus += 1;
    }
    report
}

/// Snap every `FocusRing` width to whole device pixels.
fn snapped_ring(scale: Scale) -> FocusRing {
    FocusRing {
        thickness: device_snapped_width(FocusRing::STANDARD.thickness, scale),
        gap: device_snapped_width(FocusRing::STANDARD.gap, scale),
        hug_gap: device_snapped_width(FocusRing::STANDARD.hug_gap, scale),
        stroke: device_snapped_width(FocusRing::STANDARD.stroke, scale),
        halo: device_snapped_width(FocusRing::STANDARD.halo, scale),
    }
}

fn petra_from_egui(node: egui::Rect) -> PetraRect {
    PetraRect {
        x: node.min.x,
        y: node.min.y,
        w: node.width(),
        h: node.height(),
    }
}

fn egui_from_petra(bar: PetraRect) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(bar.x, bar.y), egui::vec2(bar.w, bar.h))
}

/// How far the focus indicator sits outside the node (a bar under or a pair
/// of sides, plus the bar's shadow). A border reaches nowhere, so this is
/// the widest of the three and covers all of them.
fn indicator_outset(scale: Scale) -> f32 {
    let shadow = bar_shadow_geometry();
    device_snapped_width(FocusRing::STANDARD.overhang(), scale)
        + f32::from(shadow.spread)
        + f32::from(shadow.blur)
        + f32::from(shadow.offset[0].abs().max(shadow.offset[1].abs()))
}

/// Clip used to paint the caret.
///
/// Text buttons store `clip ∩ own rect`, so a settled underline would vanish
/// if we clipped to `clip` alone. Expand by the indicator's reach. While the
/// bar is in flight, use the page so the path is visible.
#[must_use]
pub(crate) fn caret_clip_limit(
    composer_clip: egui::Rect,
    moving: bool,
    page: egui::Rect,
    scale: Scale,
) -> egui::Rect {
    if moving {
        page
    } else {
        composer_clip.expand(indicator_outset(scale))
    }
}

/// The rects a **settled** `figure` fills on `mark`, for the two figures
/// that are filled outright.
///
/// `mark` is [`gorgon_petra::focus::marked_rect`]'s answer, not a
/// placement's rect: for a ring or a pair of brackets the two are the same,
/// and for a bar the horizontal extent is the control's content run.
///
/// [`FocusFigure::Border`] answers that rect itself, because its two
/// concentric strokes are struck *inside* it by [`paint_focus_figure`] and
/// there is no separate rect to fill.
#[must_use]
pub(crate) fn caret_bars(mark: egui::Rect, figure: FocusFigure, scale: Scale) -> Vec<egui::Rect> {
    let snapped = snapped_ring(scale);
    let pr = petra_from_egui(mark);
    match figure {
        FocusFigure::Border => vec![mark],
        FocusFigure::BarUnder => vec![egui_from_petra(snapped.bar(pr))],
        FocusFigure::BarInside => vec![egui_from_petra(snapped.bar_inside(pr))],
        FocusFigure::Sides => snapped.sides(pr).into_iter().map(egui_from_petra).collect(),
    }
}

/// The four bands the flying caret springs toward, in edge order: top,
/// right, bottom, left.
///
/// Every figure is a position of the same four rects, and a band a figure
/// does not use is not absent — it is **flat on the edge it belongs to**,
/// zero deep and full along. That is what makes each of the three pairs a
/// continuous morph rather than a swap: a band grows out of its own edge
/// instead of arriving from somewhere the eye can follow.
///
/// * `Border` — all four inside the node's edges, `stroke` deep. Carbon's
///   `outline-offset: -2px`.
/// * `BarUnder` — the bottom band is the bar below the node; the other three
///   are flat.
/// * `BarInside` — the same bottom band, on the node's own bottom edge.
/// * `Sides` — left and right stand outside the node; top and bottom are
///   flat.
///
/// `FocusCaret::bands` drops the flat ones before painting, so a bar under
/// is one filled rect and a border is four.
#[must_use]
pub(crate) fn caret_bands(
    mark: egui::Rect,
    figure: FocusFigure,
    scale: Scale,
) -> [egui::Rect; crate::focus_caret::BANDS] {
    let ring = snapped_ring(scale);
    let flat_top = egui::Rect::from_min_size(mark.min, egui::vec2(mark.width(), 0.0));
    let flat_bottom = egui::Rect::from_min_size(mark.left_bottom(), egui::vec2(mark.width(), 0.0));
    let flat_left = egui::Rect::from_min_size(mark.min, egui::vec2(0.0, mark.height()));
    let flat_right = egui::Rect::from_min_size(mark.right_top(), egui::vec2(0.0, mark.height()));
    match figure {
        // `FocusRing::border_edges` and not four rects written out here:
        // `component::tabs`' `a_tabs_ring_marks_edges_its_indicator_does_not`
        // measures which edges the ring marks, and a second copy of this
        // geometry would let the painter and that test drift apart.
        FocusFigure::Border => ring
            .border_edges(petra_from_egui(mark))
            .map(egui_from_petra),
        FocusFigure::BarUnder => [
            flat_top,
            flat_right,
            egui_from_petra(ring.bar(petra_from_egui(mark))),
            flat_left,
        ],
        // The same bottom band as `BarUnder`, seated on the node's own edge.
        // A flight between the two is therefore one band sliding five units,
        // which is the morph a person can follow — and the reason both are
        // the bottom band rather than one of them borrowing another edge.
        FocusFigure::BarInside => [
            flat_top,
            flat_right,
            egui_from_petra(ring.bar_inside(petra_from_egui(mark))),
            flat_left,
        ],
        FocusFigure::Sides => {
            let [left, right] = ring.sides(petra_from_egui(mark));
            [
                flat_top,
                egui_from_petra(right),
                flat_bottom,
                egui_from_petra(left),
            ]
        }
    }
}

/// Where the focus indicator goes this frame: the focused node's id, the
/// rect the figure is drawn around, the figure, and that rect's clip.
///
/// The rect is the focused placement's own, or — for a node that declared
/// [`FocusFigure::HugWell`] — the nearest ancestor that declared
/// [`FocusFigure::Hug`]: the well an `Input` leaf sits in. `id` stays the
/// focused node's, because that is what the spring keys its flight on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CaretTarget<'a> {
    /// The focused placement's id.
    pub id: &'a str,
    /// The rect the figure brackets, underlines or rings, device-snapped.
    pub mark: egui::Rect,
    /// The shape drawn on that rect.
    pub figure: FocusFigure,
    /// The `radius` token that placement paints its own fill with, if any.
    ///
    /// [`FocusFigure::Border`] takes the node's corner radius, so a ring on
    /// a pill is a pill and does not square off the control it marks. CSS
    /// says the same: an `outline` follows the box's `border-radius`, so
    /// Carbon's `focus-outline('outline')` is round on a rounded button
    /// without ever saying so. The other two figures ignore it.
    pub radius: Option<&'a str>,
    /// The composer clip of the placement `node` belongs to, device-snapped.
    pub clip: egui::Rect,
    /// That placement's cumulative opacity.
    pub opacity: f32,
}

/// The focused placement's caret target, if one is to be painted.
///
/// `None` when nothing is focused, when the focused node (or the well it
/// shows focus on) is off screen, or when a bar would cross a surface that
/// sits **above** the node — a `Layer::Popup` list opened flush under a
/// field, say. The indicator belongs to its node's own layer; painting it
/// over a surface stacked above that node would put the operator's eye on
/// a bar the node cannot own, so it is withheld instead. A surface the
/// focused node is *inside* is not above it and never hides it.
#[must_use]
pub(crate) fn focused_caret_target(frame: &PetrifiedFrame) -> Option<CaretTarget<'_>> {
    let scale = frame.viewport.scale;
    let placements = &frame.placements;
    let (index, focused) = placements
        .iter()
        .enumerate()
        .find(|(_, p)| p.semantics.focused)?;
    if !focused.is_visible() {
        return None;
    }
    let lineage: Vec<usize> = ancestors(placements, index).collect();
    // Two declarations show focus somewhere other than the focused node's
    // own rect, and they point opposite ways: `OnWell` up to the well the
    // node sits in, `OnHead` down to the head row it spans. Both fall back
    // to the focused node rather than going blind.
    let elsewhere = match focused.semantics.focus_shown_on {
        FocusShownOn::OnWell => lineage
            .iter()
            .copied()
            .find(|&i| placements[i].semantics.focus_shown_on == FocusShownOn::Well),
        FocusShownOn::OnHead => head_of(placements, index),
        _ => None,
    };
    let shown_index = elsewhere.unwrap_or(index);
    let shown_on = &placements[shown_index];
    if !shown_on.is_visible() {
        return None;
    }
    // The *shape* comes from the node the figure is drawn on, not from the
    // node holding focus: an `Input` leaf declaring `OnWell` hands the
    // question to the well, and the well is the node that knows it is a
    // well. The two halves of the declaration are read from two different
    // places on purpose.
    let figure = shown_on.semantics.focus_figure;
    // And the rect is not `shown_on.rect` either. A ring and a pair of
    // brackets mark the whole control; an underline marks its label, and on
    // a row that spans a panel those are 868 units and 44. `marked_rect` is
    // the engine's answer to which, and lives in `gorgon-petra` because a
    // second renderer has to reach the same picture (D-069).
    let mark = to_egui_snapped(
        gorgon_petra::focus::marked_rect(placements, shown_index, figure),
        scale,
    );
    let radius = frame.content.get(shown_index).and_then(|content| {
        resolve_slot(
            &content.tokens,
            RADIUS_SLOT,
            DerivedState::of(&shown_on.semantics),
        )
    });
    let bars = caret_bars(mark, figure, scale);
    let covered = placements.iter().enumerate().any(|(i, s)| {
        i != index
            && s.kind == NodeKind::Surface
            && s.z > shown_on.z
            && !lineage.contains(&i)
            && s.is_visible()
            && {
                let cover = to_egui_snapped(s.rect, scale);
                bars.iter().any(|bar| overlaps(*bar, cover))
            }
    });
    if covered {
        return None;
    }
    Some(CaretTarget {
        id: focused.id.as_str(),
        mark,
        figure,
        radius,
        clip: to_egui_snapped(shown_on.clip, scale),
        opacity: shown_on.opacity,
    })
}

/// The indices of `index`'s ancestors, nearest first, walking
/// `Placement::parent` the way `gorgon_petra::focus`'s `blocking_scopes`
/// does: bounded by the slice's length, so a cyclic or out-of-range chain
/// ends rather than spins.
fn ancestors(placements: &[Placement], index: usize) -> impl Iterator<Item = usize> + '_ {
    let mut cursor = placements.get(index).and_then(|p| p.parent);
    let mut steps = 0usize;
    std::iter::from_fn(move || {
        let i = cursor?;
        if steps >= placements.len() {
            return None;
        }
        steps += 1;
        cursor = placements.get(i).and_then(|p| p.parent);
        Some(i)
    })
}

/// The nearest descendant of `index` declaring [`FocusShownOn::Head`].
///
/// Nearest by depth, counted in steps up the parent chain — not by position
/// in the array. A tree item's own head row is one step down; the head row
/// of a child item nested under it is three or more, and must never win.
/// Carbon narrows the same way, with a direct-child selector:
/// `.cds--tree-node:focus > .cds--tree-node__label`.
///
/// Depth rather than array order because nothing in `PetrifiedFrame`'s
/// contract promises the placements arrive in tree order, and a rule that
/// silently leaned on it would break the day that stopped being true.
fn head_of(placements: &[Placement], index: usize) -> Option<usize> {
    placements
        .iter()
        .enumerate()
        .filter(|(_, p)| p.semantics.focus_shown_on == FocusShownOn::Head)
        .filter_map(|(i, _)| {
            ancestors(placements, i)
                .position(|a| a == index)
                .map(|steps| (steps, i))
        })
        .min()
        .map(|(_, i)| i)
}

/// Whether two rects share any area. Touching edges do not count: a list
/// flush under a field shares the field's bottom edge with a hug bar's
/// bottom edge, and that is not a cover.
fn overlaps(a: egui::Rect, b: egui::Rect) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

/// Draw the settled indicator for `target`, and report whether any of it
/// landed.
///
/// The geometry is [`FocusRing`]'s, in `gorgon-petra` (D-069). What lives
/// here is the drawing: bars filled in `focus.ring` (the accent), the
/// shadow that seats a bar under on the card, and the border's two
/// concentric strokes.
fn paint_focus_target(
    painter: &Painter,
    target: &CaretTarget<'_>,
    page: egui::Rect,
    colors: &dyn TokenSource,
    scale: Scale,
    report: &mut PaintReport,
) -> bool {
    let mut p = painter.clone();
    p.set_opacity(target.opacity.clamp(0.0, 1.0));
    let limit = caret_clip_limit(target.clip, false, page, scale);
    paint_focus_figure(
        &p,
        SettledFigure {
            mark: target.mark,
            figure: target.figure,
            radius: target.radius,
        },
        limit,
        colors,
        scale,
        report,
    )
}

/// Draw the host's caret. Used on a tessellated scene replay so a hop does
/// not re-emit every placement.
///
/// Settled goes to [`paint_focus_figure`], which draws the figure as it is
/// designed. In flight the four sprung bands are filled flat: no halo, no
/// shadow, no corner radius. See `focus_caret.rs`'s "Flight is not the
/// settled picture".
pub(crate) fn paint_caret_overlay(
    painter: &Painter,
    overlay: &CaretOverlay,
    colors: &dyn TokenSource,
    scale: Scale,
    report: &mut PaintReport,
) -> bool {
    match &overlay.picture {
        CaretPicture::Hidden => false,
        CaretPicture::Settled {
            mark,
            figure,
            radius,
        } => paint_focus_figure(
            painter,
            SettledFigure {
                mark: *mark,
                figure: *figure,
                radius: radius.as_deref(),
            },
            overlay.clip,
            colors,
            scale,
            report,
        ),
        CaretPicture::Flying(bands) => {
            paint_caret_bands(painter, bands, overlay.clip, colors, report)
        }
    }
}

/// Fill the sprung bands flat in `focus.ring`, never past `clip`.
///
/// No reach to add: every band of every figure is either inside the node or
/// one of the two outward figures' own bars, and `clip` already carries the
/// page while the caret is moving (`caret_clip_limit`).
fn paint_caret_bands(
    painter: &Painter,
    bands: &[egui::Rect],
    clip: egui::Rect,
    colors: &dyn TokenSource,
    report: &mut PaintReport,
) -> bool {
    let Some(color) = resolve_or_record(colors, gorgon_petra::token::focus::RING_TOKEN, report)
    else {
        return false;
    };
    let mut p = painter.clone();
    p.set_clip_rect(clip);
    let mut painted = false;
    for band in bands {
        if band.is_positive() {
            p.rect_filled(*band, 0.0, color);
            painted = true;
        }
    }
    painted
}

/// A settled indicator: which rect it is drawn on, which shape, and the
/// corner radius that shape follows.
///
/// One struct rather than three parameters because both callers — the jump
/// path and the host's settled caret — carry all three together, and the
/// painter reads them together.
#[derive(Clone, Copy)]
pub(crate) struct SettledFigure<'a> {
    /// The rect the figure is drawn on, device-snapped.
    pub mark: egui::Rect,
    /// The shape drawn on it.
    pub figure: FocusFigure,
    /// That rect's `radius` token, if any. Read only by
    /// [`FocusFigure::Border`].
    pub radius: Option<&'a str>,
}

/// Draw the settled `figure` on `node`, never past `limit`.
///
/// `limit` is the composer's clip for this node. A list row at the bottom
/// of a scroll must not paint its indicator onto the next card.
///
/// One arm per figure:
///
/// * [`FocusFigure::Border`] strikes two concentric bands *inside* `node`:
///   the accent on the node's own edge (Carbon's `outline: 2px solid $focus;
///   outline-offset: -2px`, `utilities/_focus-outline.scss:29`) and the
///   ground-coloured halo immediately inside it (Carbon's `inset 0 0 0
///   $button-border-width $background`, `components/button/_mixins.scss:134`).
///   Both follow the node's corner radius, the way a CSS `outline` follows
///   `border-radius`. The halo goes down first and the accent over it, so a
///   node too small to hold both loses the halo and keeps the indicator.
/// * Every remaining figure, [`FocusFigure::BarUnder`],
///   [`FocusFigure::Sides`] and [`FocusFigure::BarInside`], fills its bars and
///   casts [`gorgon_petra::token::focus::BAR_SHADOW_TOKEN`] behind each one,
///   which seats a 3-unit strip on whatever it lies over. One shadow for the
///   whole family, by the operator's ruling of 2026-09-06. See the block
///   comment on `shadow_color` for the one that was withheld and why the
///   reason for withholding it did not hold up.
///
///   The token is `shadow.raised`, not `shadow.overlay`: overlay drops four
///   units under a three-unit bar, so its own pixels never meet the bar's
///   and it stood as a second stripe below it (R6, light mode, 2026-09-05).
fn paint_focus_figure(
    painter: &Painter,
    settled: SettledFigure<'_>,
    limit: egui::Rect,
    colors: &dyn TokenSource,
    scale: Scale,
    report: &mut PaintReport,
) -> bool {
    let SettledFigure {
        mark,
        figure,
        radius,
    } = settled;
    if !mark.is_positive() {
        return false;
    }
    let Some(color) = resolve_or_record(colors, gorgon_petra::token::focus::RING_TOKEN, report)
    else {
        return false;
    };
    if figure == FocusFigure::Border {
        let clip = mark.intersect(limit);
        if !clip.is_positive() {
            return false;
        }
        let mut p = painter.clone();
        p.set_clip_rect(clip);
        let ring = snapped_ring(scale);
        let [(outer, stroke), (inner, halo)] = ring
            .bands(petra_from_egui(mark))
            .map(|(rect, width)| (egui_from_petra(rect), width));
        // The node's own radius, and the halo's is that minus the stroke it
        // sits inside — the way any nested rounded rect keeps a constant
        // band width round a corner. Negative clamps to square, which is
        // what a 1-unit radius under a 2-unit stroke should be.
        let outer_r = radius.map_or(0.0, |token| resolve_radius_or_record(colors, token, report));
        let inner_r = (outer_r - stroke).max(0.0);
        if let Some(ground) =
            resolve_or_record(colors, gorgon_petra::token::focus::HALO_TOKEN, report)
            && inner.is_positive()
        {
            p.rect_stroke(
                inner,
                inner_r,
                egui::Stroke::new(halo, ground),
                egui::StrokeKind::Inside,
            );
        }
        p.rect_stroke(
            outer,
            outer_r,
            egui::Stroke::new(stroke, color),
            egui::StrokeKind::Inside,
        );
        return true;
    }

    let shadow_geom = bar_shadow_geometry();
    let reach = device_snapped_width(FocusRing::STANDARD.overhang(), scale)
        + f32::from(shadow_geom.spread)
        + f32::from(shadow_geom.blur)
        + f32::from(shadow_geom.offset[0].abs().max(shadow_geom.offset[1].abs()));
    // **Every bar casts the same shadow.** The operator ruled on this on
    // 2026-09-06: *"not all the cursors have the same shadow. The underbar
    // (original one) has it right."*
    //
    // It was `BarUnder` alone before that, and the reason given for holding
    // the other two back does not survive checking. The first attempt at
    // uniformity turned `assert_hugs_well` red on seven pages. The ground
    // under a bracket's foot came back [27,27,27] against [34,34,34] beside
    // it, and the comment here then claimed that assertion existed
    // "because the smudge was reported". It does not. `git log -S` puts it
    // in 04637c7, wave F1, written the same day as the figure it guards and
    // by the same hand; the operator's only recorded shadow defect is R6
    // (`ROUND4-DEFECTS.md:20`), which is `BarUnder` casting `shadow.overlay`
    // and standing as a second stripe. The smudge assertion was a
    // prophylactic dressed as a report, and it outranked the operator once.
    // It has been rewritten to the property it can honestly claim: a
    // shadow under the foot rather than a bleed, neutral and fading with
    // distance, nowhere near the accent. That keeps its teeth against a bar
    // growing downward and gains teeth against the shadow being dropped.
    //
    // `Border` is still the exception, and it is the shape that makes it
    // one: a ring has no behind. It is struck on the node's own edge and
    // backed by a ground-coloured halo one band inside (`HALO_TOKEN`), and
    // that halo is already the separation a shadow would be asked for. A
    // rect shadow at `mark` would sit behind the *node*, not behind the
    // ring, and would double whatever elevation the node carries itself.
    // The `Border` arm returns above this line and never reaches it.
    let shadow_color =
        resolve_or_record(colors, gorgon_petra::token::focus::BAR_SHADOW_TOKEN, report);
    let mut painted = false;
    for bar in caret_bars(mark, figure, scale) {
        if !bar.is_positive() {
            continue;
        }
        let clip = bar.expand(reach).intersect(limit);
        if !clip.is_positive() {
            continue;
        }
        let mut cast = painter.clone();
        cast.set_clip_rect(clip);
        if let Some(shadow_color) = shadow_color {
            // `crate::shadow` and not `epaint::Shadow`, and its module doc
            // carries the measurement: epaint cannot draw a shadow on a bar
            // without leaving a diagonal seam at one corner, because it
            // clamps the blur to the bar's 3-unit thickness and then invents
            // a corner radius of half that. The operator found the seam on a
            // `Sides` bar on 2026-09-06, where a four-pixel diagonal is as
            // long as the bar is broad.
            cast.add(egui::Shape::Mesh(std::sync::Arc::new(
                crate::shadow::box_shadow(bar, shadow_geom, shadow_color),
            )));
        }
        cast.rect_filled(bar, 0.0, color);
        painted = true;
    }
    painted
}

/// The geometry keyed by [`gorgon_petra::token::focus::BAR_SHADOW_TOKEN`].
///
/// Looked up rather than indexed: `SHADOW_GEOMETRY[1]` was the old way and
/// it silently paired the bar with whatever row happened to sit second.
fn bar_shadow_geometry() -> gorgon_petra::token::ShadowGeometry {
    SHADOW_GEOMETRY
        .iter()
        .find(|(name, _)| *name == gorgon_petra::token::focus::BAR_SHADOW_TOKEN)
        .expect("BAR_SHADOW_TOKEN names one of SHADOW_GEOMETRY's rows")
        .1
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

/// The two corner radii that flank `edge`, as `(near rect.min, near
/// rect.max)` — the same min→max order [`edge_segment`] and
/// [`paint_rule_groove`] already walk that edge in, so a caller never has to
/// re-derive which corner is which.
fn corner_pair_for_edge(edge: Edge, corner_radius: CornerRadius) -> (f32, f32) {
    match edge {
        Edge::Top => (f32::from(corner_radius.nw), f32::from(corner_radius.ne)),
        Edge::Bottom => (f32::from(corner_radius.sw), f32::from(corner_radius.se)),
        Edge::Left => (f32::from(corner_radius.nw), f32::from(corner_radius.sw)),
        Edge::Right => (f32::from(corner_radius.ne), f32::from(corner_radius.se)),
    }
}

/// Shrinks the span `[min, max]` inward by `start` from `min` and `end` from
/// `max`, clamping at the midpoint rather than letting the two radii cross
/// and reverse the span — a node whose corner radii exceed half its own
/// extent has already gone through [`corner_for`]'s half-edge clause and
/// come out a pill; this guard is for the case that has not, an author
/// binding a per-corner radius the shorthand never had to answer for.
fn shrink_span(min: f32, max: f32, start: f32, end: f32) -> (f32, f32) {
    let mid = (min + max) / 2.0;
    ((min + start).min(mid), (max - end).max(mid))
}

/// The two ends of a one-edge rule of stroke `width` along `edge` of `rect`,
/// inset by half the width so the whole stroke lies inside the rect — the
/// same pixels `StrokeKind::Inside` would put that edge of a four-sided
/// outline on.
///
/// Shortened at each end by that end's own corner radius. Without this a
/// node with both a rounded corner and an edge slot bound there drew its
/// line straight into the arc the fill and the four-sided border already
/// curve away from — invisible while no component bound both (`field()`
/// binds a rule and no radius; every radius-bound node was `CornerRole::Tiled`,
/// itself always zero), and wrong the moment one did.
fn edge_segment(
    rect: egui::Rect,
    edge: Edge,
    width: f32,
    corner_radius: CornerRadius,
) -> [egui::Pos2; 2] {
    let half = width / 2.0;
    let (start, end) = corner_pair_for_edge(edge, corner_radius);
    match edge {
        Edge::Top => {
            let (x0, x1) = shrink_span(rect.min.x, rect.max.x, start, end);
            [
                egui::pos2(x0, rect.min.y + half),
                egui::pos2(x1, rect.min.y + half),
            ]
        }
        Edge::Bottom => {
            let (x0, x1) = shrink_span(rect.min.x, rect.max.x, start, end);
            [
                egui::pos2(x0, rect.max.y - half),
                egui::pos2(x1, rect.max.y - half),
            ]
        }
        Edge::Left => {
            let (y0, y1) = shrink_span(rect.min.y, rect.max.y, start, end);
            [
                egui::pos2(rect.min.x + half, y0),
                egui::pos2(rect.min.x + half, y1),
            ]
        }
        Edge::Right => {
            let (y0, y1) = shrink_span(rect.min.y, rect.max.y, start, end);
            [
                egui::pos2(rect.max.x - half, y0),
                egui::pos2(rect.max.x - half, y1),
            ]
        }
    }
}

/// One of the four corners a groove edge can end at, named so
/// [`corner_box`] and [`single_corner_radius`] can say which without
/// re-deriving it from `(Edge, bool)` twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Corner {
    Nw,
    Ne,
    Se,
    Sw,
}

/// The two corners flanking `edge`, as `(near rect.min, near rect.max)` —
/// the same order [`corner_pair_for_edge`] already answers as radii.
fn edge_corners(edge: Edge) -> (Corner, Corner) {
    match edge {
        Edge::Top => (Corner::Nw, Corner::Ne),
        Edge::Bottom => (Corner::Sw, Corner::Se),
        Edge::Left => (Corner::Nw, Corner::Sw),
        Edge::Right => (Corner::Ne, Corner::Se),
    }
}

/// The `radius`-sized square this one corner's arc is drawn inside — sized
/// so a stroke of `rect`'s real rounded-rect boundary, clipped to this box,
/// shows nothing but that corner's own quarter arc: the tangent point where
/// the arc meets each straight edge sits exactly on the box's far side, so
/// no straight run from a neighbouring edge crosses into it.
fn corner_box(rect: egui::Rect, corner: Corner, radius: f32) -> egui::Rect {
    match corner {
        Corner::Nw => egui::Rect::from_min_size(rect.min, egui::vec2(radius, radius)),
        Corner::Ne => egui::Rect::from_min_max(
            egui::pos2(rect.max.x - radius, rect.min.y),
            egui::pos2(rect.max.x, rect.min.y + radius),
        ),
        Corner::Se => egui::Rect::from_min_max(
            egui::pos2(rect.max.x - radius, rect.max.y - radius),
            rect.max,
        ),
        Corner::Sw => egui::Rect::from_min_max(
            egui::pos2(rect.min.x, rect.max.y - radius),
            egui::pos2(rect.min.x + radius, rect.max.y),
        ),
    }
}

/// A [`CornerRadius`] with only `corner` set to `radius`, the other three
/// square — the shape [`paint_groove_corner`] strokes `rect` with, so that,
/// clipped to [`corner_box`], only this one corner's arc can appear.
fn single_corner_radius(corner: Corner, radius: u8) -> CornerRadius {
    let mut cr = CornerRadius::ZERO;
    match corner {
        Corner::Nw => cr.nw = radius,
        Corner::Ne => cr.ne = radius,
        Corner::Se => cr.se = radius,
        Corner::Sw => cr.sw = radius,
    }
    cr
}

/// The groove's resolved thickness and colour for one edge, named by
/// position relative to the true edge rather than by which of shadow or
/// highlight they are — [`groove_bands`] already answers that per-edge (shadow
/// nearest the true edge on `Edge::Top`/`Edge::Left`, highlight nearest on
/// `Edge::Bottom`/`Edge::Right`, light from above-and-left), so
/// [`paint_rule_groove`] resolves the swap once into `near`/`far` and
/// [`paint_groove_corner`] never has to know which colour it is drawing,
/// only which ring. Bundled so `paint_groove_corner` fits under seven
/// arguments without folding four independently-named numbers into one bare
/// tuple.
struct GrooveStrokes {
    near_h: f32,
    far_h: f32,
    near: Color32,
    far: Color32,
}

/// Draws one corner's two concentric arcs of the groove — `strokes.near`
/// against `rect`'s own true boundary, `strokes.far` directly inside it —
/// clipped to [`corner_box`] so nothing of the straight walls elsewhere on
/// `rect` leaks in. Which of shadow or highlight is `near` for this edge was
/// already decided by the caller; see [`GrooveStrokes`].
///
/// **Why the two arcs stay parallel.** Both strokes are drawn with
/// `StrokeKind::Inside` against `rect`'s own real rounded-rect boundary (the
/// highlight ring against `rect` shrunk inward by `shadow_h`, radius reduced
/// the same amount) — the exact geometric relationship a rounded rect's
/// offset-inward curve already has to itself. Neither arc is hand-rolled, so
/// there is no second curve construction that could drift from the one the
/// fill and the four-sided border already draw.
///
/// Returns the number of shapes painted: 2 if `radius` is positive (one
/// stroke per band), 0 if the corner is square and there is nothing to
/// curve.
fn paint_groove_corner(
    painter: &Painter,
    rect: egui::Rect,
    corner: Corner,
    radius: f32,
    strokes: &GrooveStrokes,
) -> usize {
    if radius <= 0.0 {
        return 0;
    }
    let mut clipped = painter.clone();
    clipped.set_clip_rect(corner_box(rect, corner, radius).intersect(painter.clip_rect()));

    let radius_u8 = radius.round().clamp(0.0, 255.0) as u8;
    let near_h_u8 = strokes.near_h.round().clamp(0.0, 255.0) as u8;

    clipped.rect_stroke(
        rect,
        single_corner_radius(corner, radius_u8),
        Stroke::new(strokes.near_h, strokes.near),
        egui::StrokeKind::Inside,
    );
    clipped.rect_stroke(
        rect.shrink(strokes.near_h),
        single_corner_radius(corner, radius_u8.saturating_sub(near_h_u8)),
        Stroke::new(strokes.far_h, strokes.far),
        egui::StrokeKind::Inside,
    );
    2
}

/// The flat middle run of a groove along `edge`: the same two rects
/// [`paint_rule_groove`] always drew, shortened along the run axis by
/// `start_r`/`end_r` so [`paint_groove_corner`] owns the curved ends instead.
fn groove_bands(
    rect: egui::Rect,
    edge: Edge,
    shadow_h: f32,
    highlight_h: f32,
    start_r: f32,
    end_r: f32,
) -> (egui::Rect, egui::Rect) {
    match edge {
        Edge::Bottom => {
            let (x0, x1) = shrink_span(rect.min.x, rect.max.x, start_r, end_r);
            let highlight_rect = egui::Rect::from_min_max(
                egui::pos2(x0, rect.max.y - highlight_h),
                egui::pos2(x1, rect.max.y),
            );
            let shadow_rect = egui::Rect::from_min_max(
                egui::pos2(x0, rect.max.y - highlight_h - shadow_h),
                egui::pos2(x1, rect.max.y - highlight_h),
            );
            (shadow_rect, highlight_rect)
        }
        Edge::Top => {
            let (x0, x1) = shrink_span(rect.min.x, rect.max.x, start_r, end_r);
            let shadow_rect = egui::Rect::from_min_max(
                egui::pos2(x0, rect.min.y),
                egui::pos2(x1, rect.min.y + shadow_h),
            );
            let highlight_rect = egui::Rect::from_min_max(
                egui::pos2(x0, rect.min.y + shadow_h),
                egui::pos2(x1, rect.min.y + shadow_h + highlight_h),
            );
            (shadow_rect, highlight_rect)
        }
        Edge::Left => {
            let (y0, y1) = shrink_span(rect.min.y, rect.max.y, start_r, end_r);
            let shadow_rect = egui::Rect::from_min_max(
                egui::pos2(rect.min.x, y0),
                egui::pos2(rect.min.x + shadow_h, y1),
            );
            let highlight_rect = egui::Rect::from_min_max(
                egui::pos2(rect.min.x + shadow_h, y0),
                egui::pos2(rect.min.x + shadow_h + highlight_h, y1),
            );
            (shadow_rect, highlight_rect)
        }
        Edge::Right => {
            let (y0, y1) = shrink_span(rect.min.y, rect.max.y, start_r, end_r);
            let highlight_rect = egui::Rect::from_min_max(
                egui::pos2(rect.max.x - highlight_h, y0),
                egui::pos2(rect.max.x, y1),
            );
            let shadow_rect = egui::Rect::from_min_max(
                egui::pos2(rect.max.x - highlight_h - shadow_h, y0),
                egui::pos2(rect.max.x - highlight_h, y1),
            );
            (shadow_rect, highlight_rect)
        }
    }
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

/// Paints the rule material's two-stroke groove along `edge` of `rect`
/// instead of the flat single line every other edge slot draws.
///
/// `gorgon_petra::token::rule::SHADOW_UNITS` absolute device pixels of
/// shadow, directly followed by `HIGHLIGHT_UNITS` of highlight — **light
/// always comes from the top of the screen**, so the shadow band is the
/// physically higher one on both `Edge::Top` and `Edge::Bottom`, matching
/// the contract's "The construction". Both bands hug the inside of `edge`,
/// the same convention [`edge_segment`] uses for the flat line this
/// replaces.
///
/// Two filled rects sharing one exact boundary coordinate, not two strokes
/// centred on two coordinates: a centred stroke rounds its own half-width
/// independently of its neighbour and can leave a sub-pixel seam between
/// them, where two rects built from one shared `y` cannot.
///
/// **Shortened at both ends by the two corners `edge` touches**, with
/// [`paint_groove_corner`] drawing a matching curved patch at each corner
/// whose radius is positive. This was the photographed 2026-09-07 defect:
/// two flat rects with a hardcoded `0.0` corner radius, full width
/// regardless of what the fill and the four-sided border beside them curved
/// away to. `corner_radius` is the same value [`paint_one`] already resolved
/// for this node's fill and border, so a groove edge never disagrees with
/// the rect it grooves.
///
/// Returns the number of shapes painted, for [`paint_one`]'s own shape count
/// — 0 rather than a partial groove if either stroke's token fails to
/// resolve, because one stroke alone is not the material this document
/// specifies and `resolve_or_record` has already recorded the miss.
fn paint_rule_groove(
    painter: &Painter,
    rect: egui::Rect,
    edge: Edge,
    corner_radius: CornerRadius,
    env: &PaintEnv<'_>,
    report: &mut PaintReport,
) -> usize {
    use gorgon_petra::token::rule::{HIGHLIGHT_TOKEN, HIGHLIGHT_UNITS, SHADOW_TOKEN, SHADOW_UNITS};

    let Some(shadow) = resolve_or_record(env.colors, SHADOW_TOKEN, report) else {
        return 0;
    };
    let Some(highlight) = resolve_or_record(env.colors, HIGHLIGHT_TOKEN, report) else {
        return 0;
    };

    // Absolute device pixels, converted to the logical units the rest of
    // this file measures in — the same direction `device_snapped_width`
    // converts, but with nothing left to round: `SHADOW_UNITS` and
    // `HIGHLIGHT_UNITS` are already whole device-pixel counts, so dividing
    // by the scale factor lands exactly on a device pixel boundary whenever
    // `rect` itself does (it does: `rect` is `to_egui_snapped` before this
    // function is ever called).
    //
    // Light comes from **above and to the left** (operator decision,
    // 2026-09-09, from specimens), so shadow is always left of or above
    // highlight. Straight overhead was the original construction and it is
    // what excluded vertical rules: both walls of a vertical groove sit at
    // the same angle to an overhead light, which is a uniform darkening and
    // not a bevel. Moving the light costs the horizontal case nothing.
    let factor = env.scale.factor();
    let shadow_h = SHADOW_UNITS / factor;
    let highlight_h = HIGHLIGHT_UNITS / factor;

    let (start_r, end_r) = corner_pair_for_edge(edge, corner_radius);
    let (shadow_rect, highlight_rect) =
        groove_bands(rect, edge, shadow_h, highlight_h, start_r, end_r);

    painter.rect_filled(shadow_rect, 0.0, shadow);
    painter.rect_filled(highlight_rect, 0.0, highlight);
    // Two filled rects, counted as two. A groove that reported no fill would
    // be a node declaring paint and appearing to produce none, which is the
    // exact shape `every_built_page_paints_with_nothing_silent` exists to
    // catch — and it matters now that a separator's whole appearance is the
    // groove rather than a background rect beside it.
    report.fills += 2;
    let mut shapes = 2;

    // `groove_bands` already answers which colour sits nearest the true
    // edge for this `edge` (shadow for Top/Left, highlight for Bottom/Right
    // — light from above-and-left, 2026-09-09). Resolved once here so
    // `paint_groove_corner` draws its "near" ring against the same colour
    // `groove_bands` put nearest the edge, rather than assuming shadow is
    // always that ring.
    let (near_h, near, far_h, far) = match edge {
        Edge::Top | Edge::Left => (shadow_h, shadow, highlight_h, highlight),
        Edge::Bottom | Edge::Right => (highlight_h, highlight, shadow_h, shadow),
    };
    let strokes = GrooveStrokes {
        near_h,
        far_h,
        near,
        far,
    };
    let (start_corner, end_corner) = edge_corners(edge);
    shapes += paint_groove_corner(painter, rect, start_corner, start_r, &strokes);
    shapes += paint_groove_corner(painter, rect, end_corner, end_r, &strokes);
    shapes
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
    let corner_radius = resolve_corner_radius(&content.tokens, env.colors, state, report);
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
            // difference is not cosmetic: `petra-egui/examples/parity.rs`
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
    if let Some(token) = resolve_slot(&content.tokens, BACKGROUND_SLOT, state) {
        // A separator does not have a rule along one edge. It **is** the
        // rule, so its whole rect is the groove and its own fill slot is
        // what names the material — there is no edge slot to bind, because
        // there is nothing else in the node for an edge to belong to.
        //
        // This is the second of the two constructions
        // `gorgon_petra::token::rule`'s module doc describes, and it exists
        // because the first one cannot express a standalone divider. Seven
        // of them shipped as a childless stack with a `border.subtle` fill,
        // which painted a flat line while the same token grooved on a table
        // row, and the author of each one had to pick a thickness by hand.
        //
        // Orientation comes off the rect rather than off `props.axis`,
        // which the frame does not carry: a rule is thin across itself, so
        // the long side is the run. `Edge::Top` and `Edge::Left` are the
        // shadow-first arms, and since the rect is exactly the groove's
        // thickness the opposite arms would land on the same pixels. A
        // square separator is degenerate and takes the horizontal reading.
        let separator = placement.kind == gorgon_petra::tree::NodeKind::Separator;
        if separator && outline.is_none() && token == gorgon_petra::token::rule::MATERIAL_TOKEN {
            let edge = if rect.width() >= rect.height() {
                Edge::Top
            } else {
                Edge::Left
            };
            shapes += paint_rule_groove(painter, rect, edge, corner_radius, env, report);
        } else if let Some(color) = resolve_or_record(env.colors, token, report) {
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
    //
    // Through a widened clip, for the shadow's reason: the caret is the
    // other thing on this page that draws *outside* the node it belongs to
    // (`caret_of` puts the tip `h` past the surface's near edge, back at the
    // anchor), and every placement arrives here clipped to its own bounds.
    // Painted through the inherited clip the caret drew nothing: the report
    // counted a fill, `every_built_page_paints_with_nothing_silent` was
    // green, and rows 24 and 38 showed a bubble with no beak. Widened by
    // the caret's own depth and no further, and never past the page.
    if let Some(caret) = &content.caret {
        let fill = resolve_slot(&content.tokens, BACKGROUND_SLOT, state)
            .and_then(|token| resolve_or_record(env.colors, token, report));
        let mut cast = painter.clone();
        cast.set_clip_rect(painter.clip_rect().expand(caret.h).intersect(env.page));
        if crate::triangle::paint_caret(&cast, caret, fill, env.scale) {
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
    // One edge at a time. Each bound edge slot is one line segment down the
    // inside of that edge — the same placement `StrokeKind::Inside` gives
    // the four-sided outline above, so a node that swaps `border` for
    // `border-bottom` keeps its bottom rule on the same device pixels. A
    // silhouette has no edges to pick from, so on a non-rect figure the
    // slot is recorded undrawn rather than drawn on the wrong shape.
    //
    // Any edge bound to the rule material's trigger token —
    // `gorgon_petra::token::rule::MATERIAL_TOKEN`, the same string as
    // `border.subtle` — grooves instead of drawing the flat line every other
    // edge slot draws: see [`paint_rule_groove`]. All four edges, since the
    // light moved to the top-left on 2026-09-09; before that a vertical
    // groove had both walls at the same angle to an overhead light and
    // painted a uniform darkening rather than a bevel.
    //
    // Unless the bound edges would meet at a corner. While rules were
    // horizontal only, the contract's "never meet a corner" mandate cost
    // nothing to keep: no stroke could reach one. Vertical rules remove that,
    // so `corner_free` decides it explicitly, and a node binding a horizontal
    // and a vertical edge to the material draws two flat lines rather than
    // two walls of a bevelled box. Computed once, before the loop, because
    // the answer is about the *set* of bound edges and no single pass through
    // the loop can see it.
    let grooves = {
        let material = |slot: &str| {
            resolve_slot(&content.tokens, slot, state)
                .is_some_and(|token| token == gorgon_petra::token::rule::MATERIAL_TOKEN)
        };
        gorgon_petra::token::rule::corner_free(
            material(BORDER_TOP_SLOT),
            material(BORDER_RIGHT_SLOT),
            material(BORDER_BOTTOM_SLOT),
            material(BORDER_LEFT_SLOT),
        )
    };
    for (slot, edge) in EDGE_SLOTS {
        let Some(token) = resolve_slot(&content.tokens, slot, state) else {
            continue;
        };
        let Some(color) = resolve_or_record(env.colors, token, report) else {
            continue;
        };
        if outline.is_some() {
            report.undrawn.insert(slot.to_owned());
            continue;
        }
        if grooves && token == gorgon_petra::token::rule::MATERIAL_TOKEN {
            shapes += paint_rule_groove(painter, rect, edge, corner_radius, env, report);
            continue;
        }
        let width = device_snapped_width(1.0, env.scale);
        painter.line_segment(
            edge_segment(rect, edge, width, corner_radius),
            Stroke::new(width, color),
        );
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
        // An `Input` is a leaf: padding is refused on it, so the chrome
        // rect *is* the placement. The galley still has to sit inside that
        // rect — 12 units in from the sides (`spacing-04`) and centred on
        // the vertical, Carbon's md field. Painting at `rect.min` put the
        // first glyph on the border. Shaping against the full width, then
        // shifting in, would push the last glyph through the other border.
        let input = placement.kind == gorgon_petra::tree::NodeKind::Input;
        let inset_x = text_inset_x(placement.kind, env.colors);
        let inner_w = (placement.rect.w - 2.0 * inset_x).max(0.0);
        // Colour runs, resolved here because this is where the theme is. A
        // run naming no token keeps `None` and takes `color` below, the same
        // ink the whole line would have taken; a run naming one this theme
        // cannot resolve is reported and then also falls back, for
        // `Color32::PLACEHOLDER`'s reason above — visibly wrong beats
        // invisible, and the name is in the report either way.
        let runs: Vec<(usize, Option<Color32>)> = text
            .runs
            .iter()
            .map(|run| {
                let ink = run.foreground.as_deref().and_then(|name| {
                    let found = env.colors.color(name);
                    if found.is_none() {
                        report.unresolved_tokens.insert(name.to_owned());
                    }
                    found
                });
                (run.len, ink)
            })
            .collect();
        let request = TextRequest {
            text: &text.text,
            style: text.style.as_deref(),
            wrap: text.wrap,
            max_lines: text.max_lines,
            available_width: Some(inner_w),
        };
        // The selected stretch, resolved as a pair. Neither half is usable
        // alone: the fill without the ink can sink a coloured run below AA,
        // and the ink without the fill recolours text for no visible reason.
        // So a node binding one and not the other is refused rather than
        // half-honoured, and a node binding neither takes the shipped pair —
        // which is a token the design system names, not a colour this painter
        // chose (`DEFAULT_SELECTION_TOKEN`).
        let selection = content.selection.as_ref().and_then(|range| {
            let bound = content.tokens.contains_key(SELECTION_SLOT)
                || content.tokens.contains_key(SELECTION_INK_SLOT);
            let (ground, ink) = if bound {
                (
                    resolve_slot(&content.tokens, SELECTION_SLOT, state)?,
                    resolve_slot(&content.tokens, SELECTION_INK_SLOT, state)?,
                )
            } else {
                (DEFAULT_SELECTION_TOKEN, DEFAULT_SELECTION_INK_TOKEN)
            };
            let ground = resolve_or_record(env.colors, ground, report)?;
            let ink = resolve_or_record(env.colors, ink, report)?;
            Some((range, ground, ink))
        });
        let runs = match &selection {
            Some((range, _, ink)) => {
                crate::text::runs_with_selection(&runs, text.text.len(), range, *ink)
            }
            None => runs,
        };
        let galley = env.shaper.galley_runs(&request, &runs);
        let text_pos = if input {
            let galley_h = galley.rect.height();
            let inset_y = ((rect.height() - galley_h) * 0.5).max(0.0);
            egui::pos2(rect.min.x + inset_x, rect.min.y + inset_y)
        } else {
            rect.min
        };
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
        // Behind the glyphs, so the ink above is read against this fill and
        // not the other way round. One rectangle per row the selection
        // crosses, from the galley about to be painted — the same galley, so
        // the highlight and the letters cannot be measured differently.
        if let Some((range, ground, _)) = &selection {
            for band in crate::text::selection_rects(&galley, range) {
                painter.rect_filled(band.translate(text_pos.to_vec2()), 0.0, *ground);
                shapes += 1;
            }
        }
        let coverage = env
            .colors
            .coverage(COVERAGE_TOKEN)
            .unwrap_or(FALLBACK_COVERAGE);
        let plan = coverage_plan(coverage);
        for _ in 0..plan.repeats {
            painter.galley(text_pos, galley.clone(), color);
        }
        if plan.fraction > 0.0 {
            // ai-macs' fractional pass, ported verbatim: one further paint
            // at the colour's alpha scaled by the fractional remainder, not
            // the fraction silently dropped.
            painter.galley(
                text_pos,
                galley.clone(),
                color.gamma_multiply(plan.fraction),
            );
        }
        report.texts += 1;
        shapes += 1;
        // The underline is a strip one snapped unit deep under each row,
        // as wide as that row's glyphs, in the slot's own colour rather
        // than the text's: Carbon's link underline is `currentColor`, but a
        // slot that carried its own colour costs nothing more and is what
        // lets `underline@hover` name a tone. Drawn after the glyphs so a
        // descender crossing it stays legible.
        if let Some(token) = resolve_slot(&content.tokens, UNDERLINE_SLOT, state)
            && let Some(color) = resolve_or_record(env.colors, token, report)
        {
            let depth = device_snapped_width(1.0, env.scale);
            for row in &galley.rows {
                let run = row
                    .rect_without_leading_space()
                    .translate(text_pos.to_vec2());
                if run.width() <= 0.0 {
                    continue;
                }
                let strip = egui::Rect::from_min_size(
                    egui::pos2(run.min.x, run.max.y - depth),
                    egui::vec2(run.width(), depth),
                );
                painter.rect_filled(strip, 0.0, color);
                shapes += 1;
            }
        }
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
    if let Some(list) = &content.canvas {
        // A canvas is drawn by the engine, not by a host painter, which is
        // the whole reason it is a second node kind rather than a mode of
        // `custom`: every coordinate here is already in the digest, so what
        // reaches the screen is a function of the frame's identity.
        //
        // Assets are the one part the digest cannot see — it hashes the
        // asset's name and its rects, never decoded pixels — so a `Sprite`
        // that does not resolve is counted rather than quietly skipped.
        let ctx = painter.ctx().clone();
        let mut assets = crate::draw::HostImages::new(env.images, &ctx);
        let canvas = crate::draw::paint_canvas(
            painter,
            list,
            placement.rect,
            env.scale,
            env.colors,
            &mut assets,
        );
        report.missing_assets += canvas.missing_assets;
        for name in &canvas.undrawn {
            report.undrawn.insert(format!("canvas:{name}"));
        }
        if canvas.shapes > 0 {
            report.canvases += 1;
            shapes += canvas.shapes;
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

    /// Petrify one fixture at this module's standard 240 x 120 page.
    ///
    /// The registry is built the way `Host::new` builds one: the tree's own
    /// vocabulary **plus** every shipped transition name. Not
    /// `testing::validated`'s bare vocabulary, because the fixtures here are
    /// shipped components and a `button` names `anim::BUTTON_PRESS`, which
    /// acceptance refuses against a registry that was never told the name
    /// exists. Declaring the *shipped* set rather than whatever the tree
    /// happens to say keeps acceptance honest: a fixture that invents a
    /// transition name is still refused.
    fn frame_of(
        node: &ViewNode,
        h: &mut Harness<GalleyShaper, gorgon_petra::testing::NoRows>,
    ) -> gorgon_petra::frame::PetrifiedFrame {
        let mut registry = gorgon_petra::tree::Registry::with_vocabulary(
            gorgon_petra::testing::extended_vocabulary(node),
        );
        gorgon_petra::anim::shipped_registry().declare_into(&mut registry);
        frame_of_registered(node, h, &registry)
    }

    /// A canvas has to reach the screen, not merely reach the digest.
    ///
    /// # The bug this is the guard for
    ///
    /// `NodeKind::Canvas`, the six-command draw list, its refusals and its
    /// digest coverage all landed before anything in this pass read
    /// `PaintContent::canvas`. Every canvas test was green, every gate was
    /// green, and the shipped host painted a canvas's *background token* and
    /// none of its picture — the acceptance scene drew a coloured strip where
    /// a cat was supposed to walk. A draw list nothing draws is the exact
    /// shape of work that looks finished.
    ///
    /// So this asserts the count, not the absence of an error: `canvases`
    /// only moves when the interpreter emitted a shape.
    #[test]
    fn a_canvas_is_painted_by_the_pass_and_not_merely_carried_by_it() {
        use gorgon_petra::draw::{ColorRef, Command, Corners, DrawList, Paint};
        use gorgon_petra::geom::Rect as DrawRect;

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let list = DrawList::new(vec![Command::Rect {
            rect: DrawRect::new(2.0, 2.0, 40.0, 20.0),
            radius: Corners::default(),
            snap: false,
            paint: Paint::filled(ColorRef::Token("surface.raised".to_owned())),
        }])
        .expect("one small rect is inside every draw-list bound");

        let canvas = ViewNode::new(NodeKind::Canvas, "plot")
            .with_props(Props {
                canvas: Some(std::sync::Arc::new(list)),
                ..Props::default()
            })
            .with_constraints(gorgon_petra::tree::Constraints {
                horizontal: gorgon_petra::tree::AxisConstraint {
                    min: Some(60.0),
                    max: Some(60.0),
                    priority: 5,
                },
                vertical: gorgon_petra::tree::AxisConstraint {
                    min: Some(30.0),
                    max: Some(30.0),
                    priority: 5,
                },
            });
        let tree = ViewNode::new(NodeKind::Stack, "root").child(canvas);
        let frame = frame_of(&tree, &mut h);

        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(
            report.canvases, 1,
            "the pass must draw the list, not just carry it: {report:?}"
        );
        assert_eq!(report.missing_assets, 0, "{report:?}");
        assert_eq!(report.silent, 0, "{report:?}");
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
    /// notice. `petra-egui/examples/parity.rs` is not that: it pins
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

    /// A caret may leave its surface. It may not leave the page.
    ///
    /// # The bug this is the guard for
    ///
    /// `overlay_surface::caret_of` puts the caret's tip `h` units past the
    /// surface's near edge, back at the anchor, and the surface placement's
    /// clip is its own rect. Painted through that clip the caret was cut
    /// off entirely: `paint_caret` returned `true`, the report counted a
    /// fill, and the popover and tooltip pages rasterized a bubble with no
    /// beak. The pixels were the only thing that said so.
    ///
    /// # What is asserted
    ///
    /// The fixture is a shipped `popover` under a shipped `button`, so the
    /// caret is the engine's own. The caret is the one convex polygon the
    /// painter emits, and its clip must contain every one of its vertices,
    /// the tip included, while staying inside the page. Reading the clip
    /// rather than the pixels is deliberate: the clip is what the bug was.
    #[test]
    fn a_caret_may_leave_its_surface_but_never_leaves_the_page() {
        use gorgon_petra::component::{button, popover};

        const PAGE: Size = Size { w: 240.0, h: 120.0 };

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(button("anchor", "Anchor"))
            .child(popover("note", "Note", "anchor", "Hi"));
        let frame = frame_of(&tree, &mut h);
        let caret = frame
            .drawn()
            .find_map(|(_, content)| content.caret)
            .expect("the popover carries an engine caret");

        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");
        assert!(
            !report.undrawn.contains("caret"),
            "the caret must draw, or this test passes by drawing nothing: {report:?}"
        );

        let page = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(PAGE.w, PAGE.h));
        let out = host.0.run_ui(RawInput::default(), |_| {});
        let polygons: Vec<(egui::Rect, Vec<egui::Pos2>)> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Path(path) => Some((cs.clip_rect, path.points.clone())),
                _ => None,
            })
            .collect();
        out.drop_without_applying_deltas();

        let [(clip, points)] = polygons.as_slice() else {
            panic!("expected the caret to be the one polygon painted, got {polygons:?}");
        };
        assert!(
            page.contains_rect(*clip),
            "the caret's clip {clip:?} reaches outside the {PAGE:?} page"
        );
        let tip = egui::pos2(caret.tip_x, caret.tip_y);
        assert!(
            points.iter().any(|p| (*p - tip).length() < 1.0),
            "the polygon is not the caret: {points:?} has no vertex at the tip {tip:?}"
        );
        assert!(
            points.iter().all(|p| clip.contains(*p)),
            "the caret's clip {clip:?} cuts off its own vertices {points:?}: it \
             was painted through the surface's clip, which ends at the edge the \
             caret reaches out of"
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

    /// An edge slot paints one line, on the inside of its own edge, at the
    /// same snapped width the four-sided `border` uses — and paints no
    /// outline. This is the primitive the two Carbon tables lean on: a row
    /// that binds `border-bottom` draws its one rule and never a box.
    #[test]
    fn an_edge_slot_paints_one_inside_line_and_no_outline() {
        for (slot, edge) in super::EDGE_SLOTS {
            let scale = Scale::new(1.5).unwrap();
            let host = Headless::new();
            let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
            let mut props = Props::default();
            props.tokens.insert(slot.into(), tok("surface.raised"));
            let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
            let frame = petrify(
                1,
                validated(&node),
                &mut h.ctx(),
                Viewport::new(Size::new(240.0, 120.0), ThemeMode::Dark).with_scale(scale),
                TransitionActivity::default(),
            );
            let mut shaper = host.shaper();
            let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
            assert!(
                report.unknown_slots.is_empty(),
                "{slot}: the painter must know the slot, got {:?}",
                report.unknown_slots
            );
            let out = host.0.run_ui(RawInput::default(), |_| {});
            let strokes: Vec<_> = out
                .shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    Shape::LineSegment { points, stroke } => Some((*points, stroke.width)),
                    Shape::Rect(r) if r.stroke.width > 0.0 => {
                        panic!("{slot}: painted a four-sided outline {r:?}")
                    }
                    _ => None,
                })
                .collect();
            out.drop_without_applying_deltas();
            let [(points, width)] = strokes[..] else {
                panic!("{slot}: expected exactly one line, got {strokes:?}");
            };
            let expected = super::device_snapped_width(1.0, scale);
            assert_eq!(width, expected, "{slot}: width is not device-snapped");
            let rect = to_egui_snapped(frame.placements[0].rect, scale);
            assert_eq!(
                points,
                super::edge_segment(rect, edge, expected, CornerRadius::ZERO),
                "{slot}: the line is not on the inside of its own edge"
            );
            let half = expected / 2.0;
            let on_edge = match edge {
                super::Edge::Top => points[0].y == rect.min.y + half && points[1].y == points[0].y,
                super::Edge::Bottom => {
                    points[0].y == rect.max.y - half && points[1].y == points[0].y
                }
                super::Edge::Left => points[0].x == rect.min.x + half && points[1].x == points[0].x,
                super::Edge::Right => {
                    points[0].x == rect.max.x - half && points[1].x == points[0].x
                }
            };
            assert!(
                on_edge,
                "{slot}: {points:?} is not along {edge:?} of {rect:?}"
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
    /// Its sibling half — that the schema is exactly eleven named entries — is
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

    /// Every filled, unstroked rect egui received, in paint order. What a
    /// side rule and an underline are made of, and what a full `border`
    /// (a stroke) and a fill (a rect with a background token) are not.
    fn filled_rects(out: &egui::FullOutput) -> Vec<egui::Rect> {
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r) if r.stroke.width == 0.0 && r.fill != Color32::TRANSPARENT => {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect()
    }

    /// `border-bottom` is one strip along the bottom edge and nothing on the
    /// other three: no stroke, no top, no sides.
    ///
    /// Carbon's field is a fill with a bottom rule (slice-e "Text input"),
    /// and until this slot existed the only edge this painter could draw
    /// was a box, so every field in the catalog drew one. Read from the
    /// shapes egui received, at a fractional scale so the rule's depth has
    /// to be snapped to be one crisp row of device pixels.
    ///
    /// Fixture is `border-strong` (the control-boundary tone), not
    /// `border.subtle`: this test's whole point is the *generic* edge-slot
    /// mechanism, and since the rule material landed, `border.subtle`
    /// specifically grooves instead of drawing one strip —
    /// [`a_horizontal_rule_material_edge_paints_a_two_stroke_groove`] is
    /// that test. `border-strong` still exercises the plain path this one
    /// was written for: it is a state/control-boundary tone the contract
    /// keeps flat on purpose (checkbox, radio, toggle track, field rule),
    /// never a rule.
    #[test]
    fn a_bottom_border_is_one_strip_on_the_bottom_edge_and_no_box() {
        let scale = Scale::new(1.5).unwrap();
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props::default();
        props
            .tokens
            .insert("border-bottom".into(), tok("border-strong"));
        let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        let frame = petrify(
            1,
            validated(&node),
            &mut h.ctx(),
            Viewport::new(Size::new(240.0, 40.0), ThemeMode::Dark).with_scale(scale),
            TransitionActivity::default(),
        );
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(
            !report.unknown_slots.contains("border-bottom"),
            "the painter does not know the slot: {report:?}"
        );
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let stroked = out
            .shapes
            .iter()
            .any(|cs| matches!(&cs.shape, Shape::Rect(r) if r.stroke.width > 0.0));
        assert!(!stroked, "a bottom rule must not paint a box stroke");
        // A line segment, not a filled strip. Two waves wrote this slot the
        // same day and the landed painter is the one that strokes a segment
        // down the inside of the edge, which puts it on the same device
        // pixels `StrokeKind::Inside` gives the four-sided outline. This
        // test asserts the geometry either shape has to satisfy.
        let segments: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::LineSegment { points, stroke } => Some((*points, *stroke)),
                _ => None,
            })
            .collect();
        out.drop_without_applying_deltas();
        assert_eq!(segments.len(), 1, "exactly one strip: {segments:?}");
        let ([a, b], stroke) = segments[0];
        let node_rect = to_egui_snapped(frame.placements[0].rect, scale);
        let depth = device_snapped_width(1.0, scale);
        assert!(
            (a.y - (node_rect.max.y - depth / 2.0)).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4,
            "the strip sits on the bottom edge, inside it: {a:?} {b:?} in {node_rect:?}"
        );
        assert!(
            (a.x - node_rect.min.x).abs() < 1e-4 && (b.x - node_rect.max.x).abs() < 1e-4,
            "and spans the full width"
        );
        assert!(
            (stroke.width - depth).abs() < 1e-4,
            "one snapped unit deep: {} vs {depth}",
            stroke.width
        );
        let device = stroke.width * scale.factor();
        assert!(
            (device - device.round()).abs() < 1e-4,
            "the rule is {device} device pixels, not a whole number"
        );
    }

    /// `border-bottom` and `border-top`, each bound to `border.subtle`,
    /// paint the rule material's two-stroke groove: two filled bands, no
    /// stroke, shadow directly above highlight on both edges (light always
    /// comes from the top of the screen, never from "inside the node"), at
    /// exactly the theme's `rule.shadow` and `rule.highlight` colours and
    /// exactly [`gorgon_petra::token::rule::SHADOW_UNITS`] +
    /// [`gorgon_petra::token::rule::HIGHLIGHT_UNITS`] device pixels each.
    /// This is acceptance criterion 2 of
    /// `.agents/notes/proposed/architecture/2026-09-07-rule-material-two-stroke-groove.md`:
    /// "A rule renders as four units: two of shadow above two of
    /// highlight."
    #[test]
    fn a_horizontal_rule_material_edge_paints_a_two_stroke_groove() {
        let scale = Scale::new(1.0).unwrap();
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props::default();
        props
            .tokens
            .insert("border-bottom".into(), tok("border.subtle"));
        props
            .tokens
            .insert("border-top".into(), tok("border.subtle"));
        let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        let frame = petrify(
            1,
            validated(&node),
            &mut h.ctx(),
            Viewport::new(Size::new(240.0, 40.0), ThemeMode::Dark).with_scale(scale),
            TransitionActivity::default(),
        );
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let stroked = out
            .shapes
            .iter()
            .any(|cs| matches!(&cs.shape, Shape::Rect(r) if r.stroke.width > 0.0));
        assert!(!stroked, "a groove must not paint a box stroke");
        let segments = out
            .shapes
            .iter()
            .any(|cs| matches!(&cs.shape, Shape::LineSegment { .. }));
        assert!(
            !segments,
            "a grooved edge must not also paint the old flat line segment"
        );

        let bands: Vec<(egui::Rect, Color32)> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r) if r.stroke.width == 0.0 && r.fill != Color32::TRANSPARENT => {
                    Some((r.rect, r.fill))
                }
                _ => None,
            })
            .collect();
        let node_rect = to_egui_snapped(frame.placements[0].rect, scale);
        out.drop_without_applying_deltas();

        assert_eq!(
            bands.len(),
            4,
            "two bands per grooved edge, two edges: {bands:?}"
        );

        let shadow = snapshot()
            .color(gorgon_petra::token::rule::SHADOW_TOKEN)
            .expect("rule.shadow resolves in the shipped dark theme");
        let highlight = snapshot()
            .color(gorgon_petra::token::rule::HIGHLIGHT_TOKEN)
            .expect("rule.highlight resolves in the shipped dark theme");
        assert_ne!(
            shadow, highlight,
            "the two strokes must be distinguishable colours or this test \
             cannot tell them apart by fill alone"
        );

        let mut top: Vec<_> = bands
            .iter()
            .copied()
            .filter(|(r, _)| r.center().y < node_rect.center().y)
            .collect();
        let mut bottom: Vec<_> = bands
            .iter()
            .copied()
            .filter(|(r, _)| r.center().y >= node_rect.center().y)
            .collect();
        assert_eq!(top.len(), 2, "the top groove is two bands: {bands:?}");
        assert_eq!(bottom.len(), 2, "the bottom groove is two bands: {bands:?}");
        top.sort_by(|a, b| a.0.min.y.total_cmp(&b.0.min.y));
        bottom.sort_by(|a, b| a.0.min.y.total_cmp(&b.0.min.y));

        for (edge_name, [first, second]) in [
            ("top", [top[0], top[1]]),
            ("bottom", [bottom[0], bottom[1]]),
        ] {
            // Light always comes from the top of the screen: on both the
            // top edge and the bottom edge, the physically higher band
            // (smaller y) is the shadow and the lower one is the highlight.
            assert_eq!(
                first.1, shadow,
                "{edge_name}: the physically higher band must be the shadow"
            );
            assert_eq!(
                second.1, highlight,
                "{edge_name}: the physically lower band must be the highlight"
            );
            for (rect, _) in [first, second] {
                assert!(
                    (rect.min.x - node_rect.min.x).abs() < 1e-4
                        && (rect.max.x - node_rect.max.x).abs() < 1e-4,
                    "{edge_name}: a band must span the node's full width: \
                     {rect:?} in {node_rect:?}"
                );
            }
            let shadow_h = first.0.height();
            let highlight_h = second.0.height();
            assert!(
                (shadow_h - gorgon_petra::token::rule::SHADOW_UNITS / scale.factor()).abs() < 1e-4,
                "{edge_name}: shadow band is {shadow_h} logical units, not \
                 {} device pixels at this scale",
                gorgon_petra::token::rule::SHADOW_UNITS
            );
            assert!(
                (highlight_h - gorgon_petra::token::rule::HIGHLIGHT_UNITS / scale.factor()).abs()
                    < 1e-4,
                "{edge_name}: highlight band is {highlight_h} logical units, \
                 not {} device pixels at this scale",
                gorgon_petra::token::rule::HIGHLIGHT_UNITS
            );
            assert!(
                (first.0.max.y - second.0.min.y).abs() < 1e-4,
                "{edge_name}: the two bands must share one exact boundary, \
                 no gap and no overlap: {first:?} {second:?}"
            );
        }
        assert!(
            (top[0].0.min.y - node_rect.min.y).abs() < 1e-4,
            "the top groove hugs the node's own top edge: {top:?}"
        );
        assert!(
            (bottom[1].0.max.y - node_rect.max.y).abs() < 1e-4,
            "the bottom groove hugs the node's own bottom edge: {bottom:?}"
        );
    }

    /// A vertical pair grooves like a horizontal one. `border-left` with
    /// `border-right` is two parallel rules, not a corner, so both take the
    /// material: four filled bands and no flat line segments.
    ///
    /// This asserted the opposite until 2026-09-09, when the light moved to
    /// the top-left. Under an overhead light both walls of a vertical groove
    /// sit at the same angle and paint a uniform darkening rather than a
    /// bevel, which is what put vertical rules out of scope; from the
    /// top-left the left wall turns away and the right wall turns toward,
    /// and the horizontal case is unaffected because its own two walls
    /// already did.
    #[test]
    fn a_parallel_vertical_pair_grooves_like_a_horizontal_one() {
        let scale = Scale::new(1.0).unwrap();
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props::default();
        props
            .tokens
            .insert("border-left".into(), tok("border.subtle"));
        props
            .tokens
            .insert("border-right".into(), tok("border.subtle"));
        let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        let frame = petrify(
            1,
            validated(&node),
            &mut h.ctx(),
            Viewport::new(Size::new(240.0, 40.0), ThemeMode::Dark).with_scale(scale),
            TransitionActivity::default(),
        );
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let segments: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::LineSegment { points, .. } => Some(*points),
                _ => None,
            })
            .collect();
        let bands = out
            .shapes
            .iter()
            .filter(
                |cs| matches!(&cs.shape, Shape::Rect(r) if r.stroke.width == 0.0 && r.fill != Color32::TRANSPARENT),
            )
            .count();
        out.drop_without_applying_deltas();
        assert_eq!(
            segments.len(),
            0,
            "a material-bound vertical edge grooves, so it paints no flat \
             line segment: {segments:?}"
        );
        assert_eq!(bands, 4, "two vertical grooves, two filled bands each");
    }

    /// The corner guard. A node binding a horizontal edge and a vertical
    /// edge to the material would draw two walls of a bevelled box, which
    /// the contract refuses as the retro tell ("Never meet a corner"). That
    /// mandate used to cost nothing to keep, because rules were horizontal
    /// only and no stroke could reach a corner; vertical rules removed the
    /// guarantee, so `token::rule::corner_free` decides it and the painter
    /// falls back to two flat lines.
    ///
    /// Falling back rather than drawing nothing is deliberate: the author
    /// asked for a line on each edge and gets one. What they do not get is
    /// a bevel.
    #[test]
    fn perpendicular_material_edges_draw_flat_lines_instead_of_a_bevelled_corner() {
        let scale = Scale::new(1.0).unwrap();
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props::default();
        props
            .tokens
            .insert("border-top".into(), tok("border.subtle"));
        props
            .tokens
            .insert("border-left".into(), tok("border.subtle"));
        let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        let frame = petrify(
            1,
            validated(&node),
            &mut h.ctx(),
            Viewport::new(Size::new(240.0, 40.0), ThemeMode::Dark).with_scale(scale),
            TransitionActivity::default(),
        );
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let segments = out
            .shapes
            .iter()
            .filter(|cs| matches!(&cs.shape, Shape::LineSegment { .. }))
            .count();
        let bands = out
            .shapes
            .iter()
            .filter(
                |cs| matches!(&cs.shape, Shape::Rect(r) if r.stroke.width == 0.0 && r.fill != Color32::TRANSPARENT),
            )
            .count();
        out.drop_without_applying_deltas();
        assert_eq!(
            segments, 2,
            "one flat line per bound edge when the pair is a corner"
        );
        assert_eq!(
            bands, 0,
            "a corner must never groove: that is a bevelled box, which the \
             contract refuses"
        );
    }

    /// `underline` paints one strip under the text run, as wide as the
    /// glyphs and no wider than the node, and nothing when the slot is not
    /// bound.
    ///
    /// The link's accessible channel for an operator who cannot read the
    /// hue. Proved by difference: the same text with and without the slot,
    /// and the one extra shape has to be a strip that sits under the run.
    #[test]
    fn an_underline_is_one_strip_under_the_text_run() {
        let scale = Scale::new(2.0).unwrap();
        let paint = |underline: bool| -> (Vec<egui::Rect>, egui::Rect) {
            let host = Headless::new();
            let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
            let mut props = Props {
                text: Some("Open the spec".to_owned()),
                ..Props::default()
            };
            if underline {
                props
                    .tokens
                    .insert("underline".into(), tok("accent.primary"));
            }
            let node = ViewNode::new(NodeKind::Text, "root").with_props(props);
            let frame = petrify(
                1,
                validated(&node),
                &mut h.ctx(),
                Viewport::new(Size::new(240.0, 40.0), ThemeMode::Dark).with_scale(scale),
                TransitionActivity::default(),
            );
            let mut shaper = host.shaper();
            let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
            assert!(report.unknown_slots.is_empty(), "{report:?}");
            assert!(report.unresolved_tokens.is_empty(), "{report:?}");
            let out = host.0.run_ui(RawInput::default(), |_| {});
            let strips = filled_rects(&out);
            out.drop_without_applying_deltas();
            (strips, to_egui_snapped(frame.placements[0].rect, scale))
        };
        let (plain, _) = paint(false);
        assert!(plain.is_empty(), "plain text paints no strip: {plain:?}");
        let (strips, node_rect) = paint(true);
        assert_eq!(strips.len(), 1, "one line of text, one strip: {strips:?}");
        let strip = strips[0];
        let depth = device_snapped_width(1.0, scale);
        assert!(
            (strip.height() - depth).abs() < 1e-4,
            "one snapped unit deep: {}",
            strip.height()
        );
        assert!(
            strip.width() > 0.0 && strip.max.x <= node_rect.max.x + 1e-3,
            "as wide as the run and inside the node: {strip:?} in {node_rect:?}"
        );
        assert!(
            strip.max.y <= node_rect.max.y + 1e-3 && strip.min.y > node_rect.min.y,
            "under the glyphs, inside the node: {strip:?} in {node_rect:?}"
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

    /// A rounded button, focused, for the two drawing tests below.
    fn focused_button(figure: gorgon_petra::tree::FocusFigure) -> ViewNode {
        use gorgon_petra::tree::{Interaction, Role};

        let mut props = Props {
            text: Some("Run".into()),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        props.tokens.insert("radius".into(), tok("shape.corner-md"));
        let mut node = ViewNode::new(NodeKind::Text, "root")
            .with_props(props)
            .interactive(
                Role::Button,
                "Run",
                &[Interaction::Click, Interaction::Focus],
            );
        node.semantics.focus_figure = figure;
        node
    }

    /// `BarUnder` is a crisp strip below the node, not a rounded box around
    /// it and not a sandwich on the fill.
    ///
    /// Falsify by pointing `caret_bars`' `BarUnder` arm at `bands` or
    /// `sides`.
    #[test]
    fn the_focus_bar_is_a_crisp_strip_not_a_rounded_box() {
        let node = focused_button(gorgon_petra::tree::FocusFigure::BarUnder);
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = Some("/root".to_owned());
        let frame = frame_of(&node, &mut h);
        let node_rect = frame
            .placements
            .iter()
            .find(|p| p.id == "/root")
            .expect("the fixture places /root")
            .rect;
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(report.focus_rings, 1, "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let mut bars = Vec::new();
        for cs in &out.shapes {
            if let Shape::Rect(r) = &cs.shape
                && r.fill.is_opaque()
                && r.stroke.width == 0.0
                && r.rect.height() <= 3.5
            {
                bars.push((r.rect, r.corner_radius.nw));
            }
        }
        out.drop_without_applying_deltas();
        assert_eq!(bars.len(), 1, "one opaque underline: {bars:?}");
        let (bar, radius) = bars[0];
        assert_eq!(radius, 0, "the underline is flat");
        assert!(
            bar.top() >= node_rect.bottom() + 1.0,
            "the underline must sit below the node, not on its fill \
             (node bottom {}, bar top {})",
            node_rect.bottom(),
            bar.top()
        );
        // `focused_button` places one bare node with no text child, so
        // `marked_rect` takes its documented fallback and hands the bar the
        // node's own rect. The bar is then the whole of what it was handed,
        // which is the rule since 2026-09-06: no fraction anywhere.
        assert!(
            (bar.width() - node_rect.w).abs() < 0.5,
            "a textless control's bar spans it (node {}, bar {})",
            node_rect.w,
            bar.width()
        );
    }

    /// `Border` lies on the node's own edge and takes the node's own corner
    /// radius.
    ///
    /// CSS says the same thing without saying it: an `outline` follows the
    /// box's `border-radius`, so Carbon's `focus-outline('outline')`
    /// (`utilities/_focus-outline.scss:29`) is a pill on a pill button. A
    /// square ring on a `shape.corner-md` button fills in the corner notch
    /// and the control reads as a different shape from the one beside it.
    ///
    /// The radius is compared against the one the node's *own fill*
    /// rasterized with, not against a number typed here, so a change to
    /// `shape.corner-md` cannot make this pass on a stale expectation.
    ///
    /// Falsify by passing `0.0` instead of `outer_r` to `rect_stroke` in
    /// `paint_focus_bar`, or by pointing `caret_bars`' `Border` arm at
    /// `bar`.
    #[test]
    fn the_border_takes_the_nodes_own_corner_radius() {
        let ring = gorgon_petra::token::FocusRing::STANDARD;
        let node = focused_button(gorgon_petra::tree::FocusFigure::Border);
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = Some("/root".to_owned());
        let frame = frame_of(&node, &mut h);
        let node_rect = frame
            .placements
            .iter()
            .find(|p| p.id == "/root")
            .expect("the fixture places /root")
            .rect;
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(report.focus_rings, 1, "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let accent: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r)
                    if (r.stroke.width - ring.stroke).abs() < 0.01
                        && (r.rect.min.x - node_rect.x).abs() < 0.01
                        && (r.rect.min.y - node_rect.y).abs() < 0.01 =>
                {
                    Some((r.rect, r.corner_radius.nw))
                }
                _ => None,
            })
            .collect();
        let halo = out
            .shapes
            .iter()
            .filter(|cs| match &cs.shape {
                Shape::Rect(r) => (r.stroke.width - ring.halo).abs() < 0.01,
                _ => false,
            })
            .count();
        // The node's own fill, so the ring is compared against the radius
        // the node actually painted rather than against a number typed here.
        let fill_radius = out
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                Shape::Rect(r)
                    if r.stroke.width == 0.0
                        && r.fill.is_opaque()
                        && (r.rect.min.x - node_rect.x).abs() < 0.01
                        && (r.rect.width() - node_rect.w).abs() < 0.01 =>
                {
                    Some(f32::from(r.corner_radius.nw))
                }
                _ => None,
            })
            .expect("the fixture paints a rounded fill on the node");
        out.drop_without_applying_deltas();
        assert!(fill_radius > 0.0, "the fixture must round the node's fill");
        assert_eq!(accent.len(), 1, "one accent stroke on the node: {accent:?}");
        assert_eq!(halo, 1usize, "one halo band inside the accent");
        let (rect, radius) = accent[0];
        assert!(
            radius > 0 && f32::from(radius) == fill_radius,
            "the ring's corner {radius} is not the node's own {fill_radius}"
        );
        assert!(
            (rect.max.x - (node_rect.x + node_rect.w)).abs() < 0.01
                && (rect.max.y - (node_rect.y + node_rect.h)).abs() < 0.01,
            "the ring is on the node's own edge, not inside its padding \
             (node {node_rect:?}, ring {rect:?})"
        );
    }

    /// A bare text input, for the figure tests: `Role::TextInput`, and the
    /// focus declarations the caller makes.
    fn text_input(
        figure: gorgon_petra::tree::FocusFigure,
        shown_on: gorgon_petra::tree::FocusShownOn,
    ) -> ViewNode {
        use gorgon_petra::tree::{Interaction, Role};

        let mut props = Props {
            placeholder: Some("name".into()),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        props.tokens.insert("border".into(), tok("border.subtle"));
        let mut node = ViewNode::new(NodeKind::Input, "root")
            .with_props(props)
            .interactive(
                Role::TextInput,
                "name",
                &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
            );
        node.semantics.focus_figure = figure;
        node.semantics.focus_shown_on = shown_on;
        node
    }

    /// The figure is the node's declaration, never its role: a
    /// `Role::TextInput` carries whichever of the three it says it does.
    /// Falsify by putting `role == TextInput` back into
    /// `focused_caret_target`'s figure line.
    ///
    /// The shape and the target are read separately, so a node declaring a
    /// figure and pointing nowhere keeps that figure. `FocusShownOn::OnWell`
    /// with no `Well` ancestor also falls back here rather than going blind.
    #[test]
    fn the_figure_is_the_declaration_not_the_role() {
        use super::focused_caret_target;
        use gorgon_petra::tree::{FocusFigure, FocusShownOn};

        let host = Headless::new();
        for declared in [
            FocusFigure::Border,
            FocusFigure::BarUnder,
            FocusFigure::Sides,
        ] {
            for shown_on in [FocusShownOn::Own, FocusShownOn::Well, FocusShownOn::OnWell] {
                let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
                h.state.focused = Some("/root".to_owned());
                let frame = frame_of(&text_input(declared, shown_on), &mut h);
                let target = focused_caret_target(&frame).expect("focused and on screen");
                assert_eq!(
                    target.figure, declared,
                    "{declared:?} shown on {shown_on:?} lost its shape"
                );
                assert_eq!(target.id, "/root");
            }
        }
    }

    /// Each figure lands where `FocusRing` says it does, and only `Border`
    /// stays inside the node. Falsify by pointing any arm of `caret_bars`
    /// at another `FocusRing` method.
    #[test]
    fn each_figure_takes_its_own_geometry() {
        use super::{caret_bands, caret_bars, egui_from_petra};
        use gorgon_petra::geom::{Rect as PetraRect, Scale};
        use gorgon_petra::token::FocusRing;
        use gorgon_petra::tree::FocusFigure;

        let node = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(120.0, 40.0));
        let pr = PetraRect::new(20.0, 30.0, 120.0, 40.0);
        let ring = FocusRing::STANDARD;

        let border = caret_bars(node, FocusFigure::Border, Scale::ONE);
        assert_eq!(border, vec![node], "a border is struck inside the node");

        let bar = caret_bars(node, FocusFigure::BarUnder, Scale::ONE);
        assert_eq!(bar, vec![egui_from_petra(ring.bar(pr))]);
        assert!(
            bar[0].min.y >= node.max.y,
            "a bar under hangs below the node"
        );

        let sides = caret_bars(node, FocusFigure::Sides, Scale::ONE);
        assert_eq!(sides.len(), 2);
        assert!(
            sides[0].max.x <= node.min.x && sides[1].min.x >= node.max.x,
            "sides stand outside both edges: {sides:?} against {node:?}"
        );

        // And the four bands the spring flies, in edge order. Every figure
        // fills the same array; the ones it does not use are flat on their
        // own edge, which is what makes each pair a morph rather than a swap.
        let live = |figure| {
            caret_bands(node, figure, Scale::ONE)
                .into_iter()
                .filter(|b| b.width() > 0.0 && b.height() > 0.0)
                .count()
        };
        assert_eq!(live(FocusFigure::Border), 4, "a border is four bands");
        assert_eq!(live(FocusFigure::BarUnder), 1, "a bar under is one band");
        assert_eq!(live(FocusFigure::Sides), 2, "sides are two bands");
        assert_eq!(
            caret_bands(node, FocusFigure::BarUnder, Scale::ONE)[2],
            bar[0],
            "the bar under is the bottom band"
        );
        let side_bands = caret_bands(node, FocusFigure::Sides, Scale::ONE);
        assert_eq!(side_bands[3], sides[0], "the left bar is the left band");
        assert_eq!(side_bands[1], sides[1], "the right bar is the right band");
    }

    /// An `Input` leaf declaring `OnWell` shows its focus on the nearest
    /// ancestor declaring `Well` — the search well, not the leaf between the
    /// magnifier and the text. Falsify by returning `focused` instead of
    /// `shown_on` from `focused_caret_target`.
    #[test]
    fn a_hug_well_leaf_is_shown_on_the_well_around_it() {
        use super::focused_caret_target;
        use gorgon_petra::component::search;
        use gorgon_petra::geom::Axis;
        use gorgon_petra::tree::FocusFigure;

        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(search("q", "Filter"));
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = Some("/root/q/input".to_owned());
        let frame = frame_of(&root, &mut h);
        let well = frame.placement("/root/q").expect("the well is placed");
        let leaf = frame
            .placement("/root/q/input")
            .expect("the leaf is placed");
        assert!(
            leaf.rect.x > well.rect.x,
            "the fixture must put the leaf inside the well, past the glyph"
        );
        let target = focused_caret_target(&frame).expect("focused and on screen");
        assert_eq!(target.id, "/root/q/input", "the spring keys on the leaf");
        assert_eq!(target.figure, FocusFigure::Sides);
        assert_eq!(
            target.mark,
            to_egui_snapped(well.rect, frame.viewport.scale),
            "the bars bracket the well, not the leaf"
        );
    }

    /// An expanded tree item shows its focus on its own head row, not on
    /// the subtree its rect spans.
    ///
    /// The mirror of the well test above, pointing the other way. A tree
    /// item's rect covers its head row *and* every descendant it has
    /// expanded, so an underline on that rect lands beneath the last
    /// grandchild — feet away from the row the operator is standing on.
    /// Carbon narrows the same way, with a direct-child selector:
    /// `.cds--tree-node:focus > .cds--tree-node__label`
    /// (`_treeview.scss:59`).
    ///
    /// Falsify by dropping `OnHead`'s arm in `focused_caret_target`, or the
    /// `Head` declaration on `tree_view.rs`'s row.
    ///
    /// The figure is `BarInside` since 2026-09-06 and was `Border` before.
    /// Nothing this test asserts turned on which: the claim is *whose rect*
    /// the figure lands on, and a box that spanned the whole expanded
    /// subtree was wrong for the same reason an underline under it would
    /// be.
    ///
    /// R8's *layout* half rides here: `tree_item_sized`'s grid stretches its
    /// head row, so the row spans the same band `background@selected` fills
    /// and the stripe has the whole row to sit on.
    ///
    /// R8's *paint* half does not, and was retired on 2026-09-06. It read
    /// "the indicator's rect must share both edges with the selected band",
    /// which was the right shape while a bar's width came from its control.
    /// The operator's rule now is that a bar spans the control's label, so
    /// the stripe is deliberately shorter than the band and shares neither
    /// edge with it. What survives is that it stays inside the band and on
    /// the head row — see
    /// `.agents/notes/implemented/architecture/2026-09-06-a-contained-underline-and-the-four-regressions-it-ends.md`.
    #[test]
    fn an_expanded_tree_item_is_underlined_on_its_head_row() {
        use super::focused_caret_target;
        use gorgon_petra::component::{tree_item, tree_view};
        use gorgon_petra::tree::FocusFigure;

        let root = tree_view(
            "tree",
            vec![tree_item(
                "src",
                "src",
                true,
                false,
                vec![
                    tree_item("a", "kernel.rs", false, false, vec![]),
                    tree_item("b", "fiber.rs", false, false, vec![]),
                ],
            )],
        );
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = Some("/tree/src".to_owned());
        let frame = frame_of(&root, &mut h);

        let item = frame.placement("/tree/src").expect("the item is placed");
        let head = frame
            .placement("/tree/src/row")
            .expect("the head row is placed");
        let last = frame
            .placement("/tree/src/children/b")
            .expect("the last child is placed");
        assert!(
            item.rect.h > head.rect.h * 2.0,
            "the fixture must be expanded: item {} high, head row {} high",
            item.rect.h,
            head.rect.h
        );

        // The two child items carry head rows of their own, so this fixture
        // exercises the "nearest" rule and not merely "any descendant".
        assert!(
            frame.placement("/tree/src/children/a/row").is_some(),
            "the fixture must nest head rows under the focused item's own"
        );

        let target = focused_caret_target(&frame).expect("focused and on screen");
        assert_eq!(target.id, "/tree/src", "the spring keys on the item");
        assert_eq!(target.figure, FocusFigure::BarInside);
        let head_rect = to_egui_snapped(head.rect, frame.viewport.scale);
        assert_eq!(
            (target.mark.min.y, target.mark.max.y),
            (head_rect.min.y, head_rect.max.y),
            "the stripe goes on the head row, not on the whole subtree"
        );
        let label = to_egui_snapped(
            frame
                .placement("/tree/src/row/label")
                .expect("the head row's label is placed")
                .rect,
            frame.viewport.scale,
        );
        // The row's content, which ends at the word. It starts earlier than
        // the word does, at the indent and caret the row reserves — those are
        // the row's own parts and `marked_rect` counts them, which is what
        // keeps the run from moving when a branch gains or loses its caret.
        assert_eq!(
            target.mark.max.x, label.max.x,
            "the run ends at the row's word: {:?} against {label:?}",
            target.mark
        );
        assert!(
            target.mark.min.x <= label.min.x,
            "and reaches back over the row's own indent: {:?}",
            target.mark
        );
        // Inside the selected band, not equal to it. The fill is bound on
        // the item and spans the tree; the stripe marks the word.
        let band = to_egui_snapped(item.rect, frame.viewport.scale);
        assert!(
            target.mark.min.x >= band.min.x && target.mark.max.x <= band.max.x,
            "the indicator left the selected band it sits on: {:?} in {band:?}",
            target.mark
        );
        assert!(
            target.mark.width() < band.width(),
            "a stripe as wide as the whole band is the picture the label rule \
             replaced"
        );
        assert!(
            target.mark.bottom() < last.rect.y,
            "the bar landed at or below the last child, which is the defect \
             this test exists for (bar bottom {}, last child top {})",
            target.mark.bottom(),
            last.rect.y
        );
    }

    /// A figure that would cross a surface stacked above the focused node is
    /// withheld: the bar under a menu's trigger lands on the open menu's
    /// first row otherwise. Falsify by dropping the `covered` check.
    ///
    /// Only the figures that reach outside the node can be covered, so this
    /// drives `BarUnder` on the trigger. The second half is the reason that
    /// matters: `Border` never leaves the node's own rect, so the same open
    /// menu covers nothing and the indicator is drawn rather than withheld.
    /// Withholding is a cost the reaching figures pay and the contained ones
    /// — `Border` and, since 2026-09-06, `BarInside` — do not.
    #[test]
    fn a_caret_crossing_a_surface_above_its_node_is_withheld() {
        use super::focused_caret_target;
        use gorgon_petra::component::{button, menu, menu_item};
        use gorgon_petra::geom::Axis;
        use gorgon_petra::tree::FocusFigure;

        let pair = |open: bool, figure: FocusFigure| {
            let mut trigger = button("trigger", "Actions");
            trigger.semantics.focus_figure = figure;
            let mut children = vec![trigger];
            if open {
                children.push(menu("menu", "Actions", vec![menu_item("mn-0", "Rename")]));
            }
            ViewNode::new(NodeKind::Stack, "root")
                .with_props(Props {
                    axis: Some(Axis::Vertical),
                    ..Props::default()
                })
                .with_children(children)
        };
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = Some("/root/trigger".to_owned());
        let shut = frame_of(&pair(false, FocusFigure::BarUnder), &mut h);
        assert!(
            focused_caret_target(&shut).is_some(),
            "with the menu shut the trigger's bar is painted"
        );
        let open = frame_of(&pair(true, FocusFigure::BarUnder), &mut h);
        let menu_rect = open
            .placement("/root/menu")
            .expect("the menu is placed")
            .rect;
        let trigger_rect = open.placement("/root/trigger").expect("placed").rect;
        assert!(
            menu_rect.y
                <= trigger_rect.bottom() + gorgon_petra::token::FocusRing::STANDARD.overhang(),
            "the fixture must open the menu where the underline would go: \
             menu {menu_rect:?}, trigger {trigger_rect:?}"
        );
        assert_eq!(
            focused_caret_target(&open),
            None,
            "the bar would cross the open menu, which is above the trigger"
        );

        let ringed = frame_of(&pair(true, FocusFigure::Border), &mut h);
        let target = focused_caret_target(&ringed)
            .expect("a border never leaves the node, so nothing above it can cover it");
        assert_eq!(target.id, "/root/trigger");
        assert_eq!(target.figure, FocusFigure::Border);
    }

    /// A focused text field declaring `Hug` is hugged on the left and
    /// right, not underlined.
    #[test]
    fn a_focused_field_is_hugged_on_the_left_and_right() {
        let node = text_input(
            gorgon_petra::tree::FocusFigure::Sides,
            gorgon_petra::tree::FocusShownOn::Well,
        );

        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        h.state.focused = Some("/root".to_owned());
        let frame = frame_of(&node, &mut h);
        let node_rect = frame
            .placements
            .iter()
            .find(|p| p.id == "/root")
            .expect("the fixture places /root")
            .rect;
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(report.focus_rings, 1, "{report:?}");

        let out = host.0.run_ui(RawInput::default(), |_| {});
        let mut bars = Vec::new();
        for cs in &out.shapes {
            if let Shape::Rect(r) = &cs.shape
                && r.fill.is_opaque()
                && r.stroke.width == 0.0
                && r.rect.width() <= 3.5
                && r.rect.height() >= node_rect.h - 1.0
            {
                bars.push(r.rect);
            }
        }
        out.drop_without_applying_deltas();
        assert_eq!(bars.len(), 2, "left and right hugs: {bars:?}");
        bars.sort_by(|a, b| a.min.x.partial_cmp(&b.min.x).unwrap());
        assert!(
            bars[0].max.x <= node_rect.x + 0.5,
            "left hug sits outside the field (field x {}, hug right {})",
            node_rect.x,
            bars[0].max.x
        );
        assert!(
            bars[1].min.x >= node_rect.x + node_rect.w - 0.5,
            "right hug sits outside the field (field right {}, hug left {})",
            node_rect.x + node_rect.w,
            bars[1].min.x
        );
    }

    /// Text buttons clip to their own rect. A bar under sits *below* that
    /// rect. The paint clip must expand by the indicator's reach or the bar
    /// lands, then vanishes.
    #[test]
    fn the_settled_underline_survives_a_self_clip() {
        use super::{caret_bars, caret_clip_limit};
        use gorgon_petra::geom::Scale;
        use gorgon_petra::tree::FocusFigure;

        let node = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(90.0, 40.0));
        let page = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));
        let limit = caret_clip_limit(node, false, page, Scale::ONE);
        let bars = caret_bars(node, FocusFigure::BarUnder, Scale::ONE);
        assert_eq!(bars.len(), 1);
        assert!(
            bars[0].intersects(limit),
            "settled underline {:?} must sit inside the expanded clip {limit:?}",
            bars[0]
        );
        assert!(
            !node.contains(bars[0].center()),
            "the underline is still below the node, not pulled inside it"
        );
    }

    /// The headline claim: focusing a node changes the picture by extra
    /// geometry that was not there before (the underline, and the shadow
    /// that seats it).
    #[test]
    fn a_focused_node_paints_a_ring_an_unfocused_one_does_not() {
        let (blind, unfocused) = paint_focused(&button(), None, &snapshot());
        let (ringed, focused) = paint_focused(&button(), Some("/root"), &snapshot());

        assert!(blind > 0, "the fixture must paint something at all");
        assert!(
            ringed > blind,
            "the underline must reach egui: {unfocused:?} then {focused:?}"
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
            assert!(
                ringed > blind,
                "{what}: the underline must be drawn on top of whatever the \
                 node draws, not instead of it: {report:?}"
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
    ///
    /// Both maps must already be the same shape. Two markers with the same
    /// nominal box size (the four `StatusShape`s below) satisfy that as-is;
    /// two with different box sizes (checkbox 16px against radio 18px, per
    /// Carbon's own SCSS — T070, `_checkbox.scss` vs `_radio-button.scss`)
    /// have to go through [`normalize_square`] first.
    fn disagreement(a: &[bool], b: &[bool]) -> f64 {
        assert_eq!(a.len(), b.len(), "coverage maps must be the same shape");
        let differing = a.iter().zip(b).filter(|(x, y)| x != y).count();
        differing as f64 / a.len() as f64
    }

    /// Recover the side length of a square coverage grid.
    ///
    /// [`marker_coverage`] samples a marker's own (always square, for every
    /// caller in this file) box at [`SAMPLE_PITCH`] in both dimensions, so
    /// `map.len()` is always a perfect square; this is just that inverse,
    /// so a flat map can be re-indexed as `[row * side + col]`.
    fn side_of(map: &[bool]) -> usize {
        #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
        let side = (map.len() as f64).sqrt().round() as usize;
        assert_eq!(
            side * side,
            map.len(),
            "coverage map of {} samples is not a square grid",
            map.len()
        );
        side
    }

    /// The common grid two differently-sized markers are normalized onto
    /// before [`disagreement`] compares them. Picked below the smaller of
    /// the two source grids this file ever produces (checkbox, 32×32 at
    /// [`SAMPLE_PITCH`]) so every target cell downsamples rather than
    /// repeating a source sample.
    const NORMALIZED_GRID: usize = 24;

    /// Resample a `side × side` boolean coverage grid onto a fixed
    /// [`NORMALIZED_GRID`] `× NORMALIZED_GRID` grid.
    ///
    /// This is what makes [`disagreement`] size-independent: a checkbox's
    /// 16px box and a radio's 18px box paint different pixel counts, but a
    /// mark's *shape* — square corners against a round silhouette — is a
    /// property of where coverage sits relative to the box's own bounds,
    /// not of how many device pixels that box happens to span. Normalizing
    /// both marks onto the same grid before diffing measures exactly that:
    /// each target cell samples the source cell its own centre maps to
    /// under proportional `[0, 1)` scaling (nearest-neighbour, not
    /// area-averaged — acceptable here because the shapes under test are
    /// coarse corner-vs-centre silhouettes, not fine detail, so resampling
    /// aliasing does not change which cells two such figures disagree on).
    ///
    /// A square-vs-round diff survives normalization because it is scale
    /// invariant: a circle's corners are empty and a square's are filled at
    /// every scale, so shrinking or growing the sampling grid moves samples
    /// but not which region of the box they fall in.
    fn normalize_square(map: &[bool], side: usize) -> Vec<bool> {
        let mut out = Vec::with_capacity(NORMALIZED_GRID * NORMALIZED_GRID);
        for row in 0..NORMALIZED_GRID {
            for col in 0..NORMALIZED_GRID {
                #[allow(clippy::cast_precision_loss)]
                let u = (col as f64 + 0.5) / NORMALIZED_GRID as f64;
                #[allow(clippy::cast_precision_loss)]
                let v = (row as f64 + 0.5) / NORMALIZED_GRID as f64;
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let src_col = ((u * side as f64) as usize).min(side - 1);
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let src_row = ((v * side as f64) as usize).min(side - 1);
                out.push(map[src_row * side + src_col]);
            }
        }
        out
    }

    /// The shipped status markers, plus diamond, paint distinguishable pictures.
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

        // Square is still in the enum for a box, but shipped Down is an
        // octagon and at 10×10 a chamfered box vs a sharp box is a 6%
        // corner cut — below this bar. The pairs that have to clear it are
        // the ones on the Fibers card: disc, triangle, octagon, and diamond.
        let cases = [
            ("status.ok", StatusShape::Circle),
            ("status.degraded", StatusShape::Triangle),
            ("status.down", StatusShape::Octagon),
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

    /// Degraded's triangle points down: base along the top, tip on the
    /// bottom. An up-pointing triangle sat next to "Degraded" on the Fibers
    /// card and read as a play mark.
    #[test]
    fn the_degraded_triangle_points_down() {
        use gorgon_petra::component::status;
        use gorgon_petra::token::{StatusShape, StatusToken};

        let st = StatusToken::new(tok("status.degraded"), StatusShape::Triangle, "Degraded")
            .expect("the fixture text is non-empty");
        let map = marker_coverage(status("s", &st), "/s/dot");
        let cols = (10.0 / SAMPLE_PITCH).round() as usize;
        let rows = cols;
        assert_eq!(map.len(), cols * rows, "10×10 marker at {SAMPLE_PITCH}");
        let at = |row: usize, col: usize| map[row * cols + col];
        // Base is the top edge: the top-left interior is inside the figure.
        assert!(at(1, 1), "top-left of a down-pointing triangle is filled");
        // Tip is the bottom midpoint: the bottom-left is outside, the
        // bottom-centre is inside.
        assert!(
            !at(rows - 2, 1),
            "bottom-left of a down-pointing triangle is empty"
        );
        assert!(
            at(rows - 2, cols / 2),
            "bottom-centre of a down-pointing triangle is the tip"
        );
    }

    /// Down's marker is an octagon: the box corners are empty, the edge
    /// midpoints are filled. A square would fill the corners.
    #[test]
    fn the_down_marker_is_an_octagon() {
        use gorgon_petra::component::status;
        use gorgon_petra::token::{StatusShape, StatusToken};

        let st = StatusToken::new(tok("status.down"), StatusShape::Octagon, "Down")
            .expect("the fixture text is non-empty");
        let map = marker_coverage(status("s", &st), "/s/dot");
        let cols = (10.0 / SAMPLE_PITCH).round() as usize;
        let rows = cols;
        let at = |row: usize, col: usize| map[row * cols + col];
        assert!(
            !at(0, 0),
            "top-left corner of an octagon is empty, a square would fill it"
        );
        assert!(!at(0, cols - 1), "top-right corner of an octagon is empty");
        assert!(at(0, cols / 2), "top flat of an octagon is filled");
        assert!(at(rows / 2, 0), "left flat of an octagon is filled");
        assert!(at(rows / 2, cols / 2), "the octagon's interior is filled");
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
        // Checkbox (16px) and radio (18px) are different box sizes in
        // Carbon's own SCSS (T070) — normalize both onto a common grid
        // before diffing, or `disagreement`'s equal-length assertion
        // fires on the box-size difference instead of measuring shape.
        let cb_norm = normalize_square(&cb, side_of(&cb));
        let rb_norm = normalize_square(&rb, side_of(&rb));
        let differing = disagreement(&cb_norm, &rb_norm);
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
