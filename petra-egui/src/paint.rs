//! Painting a petrified frame through `egui::Painter`.
//!
//! Petra decides where everything goes and in what order; this module only
//! draws. One egui layer holds the whole frame and shapes are added in Petra's
//! own paint order, so egui's z-ordering never gets a say — `Ui` is not used,
//! and neither is egui's layer ordering beyond the single layer this paints
//! into.

use std::collections::BTreeSet;

use egui::{Color32, Painter, Rgba, Stroke};
use gorgon_petra::frame::{PaintContent, PetrifiedFrame, Placement, round_rect};
use gorgon_petra::geom::{Rect as PetraRect, Scale};
use gorgon_petra::layout::TextRequest;
use gorgon_petra::token::{FocusRing, ThemeSnapshot, TokenName, TokenValue};

use crate::text::GalleyShaper;

/// Token slot painted as a filled rect behind a node.
pub const BACKGROUND_SLOT: &str = "background";
/// Token slot painted as a one-unit outline around a node.
pub const BORDER_SLOT: &str = "border";
/// Token slot used for a node's text.
pub const FOREGROUND_SLOT: &str = "foreground";

/// Every token slot this painter knows how to use. Anything else a node binds
/// lands in [`PaintReport::unknown_slots`] rather than being dropped on the
/// floor — the shipped vocabulary already declares `shape.*` tokens that
/// nothing here consumes.
const KNOWN_SLOTS: &[&str] = &[BACKGROUND_SLOT, BORDER_SLOT, FOREGROUND_SLOT];
/// Token consulted for text with no declared `foreground`.
pub const DEFAULT_TEXT_TOKEN: &str = "text.primary";

/// Resolves a token name to a colour.
///
/// A trait rather than a concrete theme so the painter has no opinion about
/// where colours come from: the shipped implementation reads a
/// [`ThemeSnapshot`], and a test can supply four colours and no theme at all.
pub trait ColorSource {
    /// The colour for `token`, or `None` when the theme has no such colour.
    fn color(&self, token: &str) -> Option<Color32>;
}

impl ColorSource for ThemeSnapshot {
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
    colors: &dyn ColorSource,
    token: &str,
    report: &mut PaintReport,
) -> Option<Color32> {
    let color = colors.color(token);
    if color.is_none() {
        report.unresolved_tokens.insert(token.to_owned());
    }
    color
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
    /// Focus rings drawn: focused placements that got at least one band.
    pub focus_rings: usize,
    /// **Focused placements that got no ring at all.** Keyboard operation
    /// nobody can see is the defect the ring exists to close, so this is a
    /// failed pass and not a note in a set: it is the one way a frame can be
    /// fully drawn and still leave the operator blind.
    pub blind_focus: usize,
    /// Token names no colour could be found for, sorted.
    pub unresolved_tokens: BTreeSet<String>,
    /// Content kinds this pass has no painter for, sorted.
    pub undrawn: BTreeSet<String>,
    /// Token slots this pass does not know how to use, sorted. A node binding
    /// `radius` gets square corners today; without this set it would get them
    /// silently, and the report would read clean.
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

/// Draw `frame` into `painter`.
///
/// The painter is expected to be a full-window layer painter
/// (`egui::Context::layer_painter`); each placement gets a clipped, faded clone
/// of it rather than a nested `Ui`.
pub fn paint_frame(
    painter: &Painter,
    frame: &PetrifiedFrame,
    shaper: &mut GalleyShaper,
    colors: &dyn ColorSource,
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
    let mut focused: Vec<&Placement> = Vec::new();
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
        match paint_one(&p, placement, content, shaper, colors, scale, &mut report) {
            Outcome::Drawn => report.drawn += 1,
            Outcome::Empty => report.empty += 1,
            Outcome::Silent => report.silent += 1,
        }
        if placement.semantics.focused {
            focused.push(placement);
        }
    }
    for placement in focused {
        let mut p = painter.with_clip_rect(to_egui_snapped(placement.clip, scale));
        p.set_opacity(placement.opacity.clamp(0.0, 1.0));
        if paint_focus_ring(&p, placement, colors, scale, &mut report) {
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
    colors: &dyn ColorSource,
    scale: Scale,
    report: &mut PaintReport,
) -> bool {
    let mut drawn = false;
    for band in FocusRing::STANDARD.bands(placement.rect) {
        let Some(color) = resolve_or_record(colors, band.token, report) else {
            continue;
        };
        let rect = to_egui_snapped(band.rect, scale);
        if !rect.is_positive() {
            // The inner halo collapses on a node thinner than the ring. The
            // outer bands still draw, so this is not a blind focus.
            continue;
        }
        let width = device_snapped_width(band.width, scale);
        painter.rect_stroke(
            rect,
            0.0,
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

fn paint_one(
    painter: &Painter,
    placement: &Placement,
    content: &PaintContent,
    shaper: &mut GalleyShaper,
    colors: &dyn ColorSource,
    scale: Scale,
    report: &mut PaintReport,
) -> Outcome {
    let rect = to_egui_snapped(placement.rect, scale);
    let mut shapes = 0_usize;

    // Every slot this painter does not understand is recorded by name. A node
    // binding `radius` to `shape.corner-lg` gets square corners today; the
    // point of the set is that it does not get them silently.
    for slot in content.tokens.keys() {
        if !KNOWN_SLOTS.contains(&slot.as_str()) {
            report.unknown_slots.insert(slot.clone());
        }
    }

    if let Some(token) = content.tokens.get(BACKGROUND_SLOT)
        && let Some(color) = resolve_or_record(colors, token, report)
    {
        painter.rect_filled(rect, 0.0, color);
        report.fills += 1;
        shapes += 1;
    }
    if let Some(token) = content.tokens.get(BORDER_SLOT)
        && let Some(color) = resolve_or_record(colors, token, report)
    {
        let width = device_snapped_width(1.0, scale);
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(width, color),
            egui::StrokeKind::Inside,
        );
        shapes += 1;
    }

    if let Some(text) = &content.text {
        let token = content
            .tokens
            .get(FOREGROUND_SLOT)
            .map_or(DEFAULT_TEXT_TOKEN, String::as_str);
        let color = colors.color(token).unwrap_or_else(|| {
            report.unresolved_tokens.insert(token.to_owned());
            // Not a guess at the theme's intent: a visibly wrong colour is
            // better than invisible text, and the unresolved token is in the
            // report either way.
            Color32::PLACEHOLDER
        });
        let galley = shaper.galley(&TextRequest {
            text: &text.text,
            style: text.style.as_deref(),
            wrap: text.wrap,
            max_lines: text.max_lines,
            available_width: Some(placement.rect.w),
        });
        painter.galley(rect.min, galley, color);
        report.texts += 1;
        shapes += 1;
    }

    if content.image.is_some() {
        report.undrawn.insert("image".into());
    }
    if let Some(name) = &content.custom {
        report.undrawn.insert(format!("custom:{name}"));
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
        ColorSource, device_snapped_width, paint_frame, to_egui_snapped, verify_paint_accounting,
    };
    use egui::{Color32, Context, Id, LayerId, Order, RawInput, Shape};
    use gorgon_petra::frame::round_rect;
    use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
    use gorgon_petra::geom::Size;
    use gorgon_petra::geom::{Rect as PetraRect, Scale};
    use gorgon_petra::testing::{Harness, validated};
    use gorgon_petra::token::{ThemeMode, ThemeSnapshot, dark};
    use gorgon_petra::tree::{NodeKind, Props, ViewNode};

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

    fn tree() -> ViewNode {
        let mut panel = Props::default();
        panel
            .tokens
            .insert("background".into(), "surface.base".into());
        panel
            .tokens
            .insert("border".into(), "surface.raised".into());
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

    /// A token the theme does not define is reported, not silently skipped and
    /// not guessed at.
    #[test]
    fn an_unresolved_token_is_named() {
        let host = Headless::new();
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), "surface.invented".into());
        let node = ViewNode::new(NodeKind::Stack, "root").with_props(props);
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let frame = frame_of(&node, &mut h);
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());
        assert_eq!(report.fills, 0);
        assert!(
            report.unresolved_tokens.contains("surface.invented"),
            "{report:?}"
        );
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
            ["image"]
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
            snap.color("spacing.md"),
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
            props
                .tokens
                .insert("border".into(), "surface.raised".into());
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
        assert!(report.undrawn.contains("image"), "{report:?}");
    }

    /// A token slot no painter consumes is named rather than dropped. The
    /// shipped vocabulary declares `shape.*` tokens and nothing here reads
    /// them, so a node asking for rounded corners gets square ones — that is
    /// allowed to be true, and not allowed to be silent.
    #[test]
    fn a_token_slot_this_painter_does_not_know_is_recorded() {
        let host = Headless::new();
        let mut h = Harness::with(host.shaper(), gorgon_petra::testing::NoRows);
        let mut props = Props {
            text: Some("hi".into()),
            ..Props::default()
        };
        props
            .tokens
            .insert("radius".into(), "shape.corner-lg".into());
        let frame = frame_of(
            &ViewNode::new(NodeKind::Text, "t").with_props(props),
            &mut h,
        );
        let mut shaper = host.shaper();
        let report = paint_frame(&host.painter(), &frame, &mut shaper, &snapshot());

        assert!(report.unknown_slots.contains("radius"), "{report:?}");
        // The text still painted, so the placement is drawn, not silent — the
        // unknown slot is a gap in the painter, not a lost placement.
        assert_eq!(report.drawn, 1, "{report:?}");
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
        colors: &dyn ColorSource,
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
            .insert("background".into(), "surface.base".into());
        let mut light_bg = Props {
            text: Some("Run".into()),
            ..Props::default()
        };
        light_bg
            .tokens
            .insert("background".into(), "status.degraded".into());

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
        impl ColorSource for NoFocusColors {
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
}
