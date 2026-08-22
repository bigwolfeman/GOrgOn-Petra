//! Painting a petrified frame through `egui::Painter`.
//!
//! Petra decides where everything goes and in what order; this module only
//! draws. One egui layer holds the whole frame and shapes are added in Petra's
//! own paint order, so egui's z-ordering never gets a say — `Ui` is not used,
//! and neither is egui's layer ordering beyond the single layer this paints
//! into.

use std::collections::BTreeSet;

use egui::{Color32, Painter, Rgba, Stroke};
use gorgon_petra::frame::{PaintContent, PetrifiedFrame, Placement};
use gorgon_petra::geom::Rect as PetraRect;
use gorgon_petra::layout::TextRequest;
use gorgon_petra::token::{ThemeSnapshot, TokenName, TokenValue};

use crate::text::GalleyShaper;

/// Token slot painted as a filled rect behind a node.
pub const BACKGROUND_SLOT: &str = "background";
/// Token slot painted as a one-unit outline around a node.
pub const BORDER_SLOT: &str = "border";
/// Token slot used for a node's text.
pub const FOREGROUND_SLOT: &str = "foreground";
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

/// What one paint pass did, and what it could not do.
///
/// The counts are the accounting that makes a missing panel visible. The
/// digest hashes placements, not pixels, so a placement that never reached a
/// painter leaves every gate green and the picture wrong; this report is how
/// the host notices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaintReport {
    /// Placements in the frame.
    pub placements: usize,
    /// Placements the pass visited.
    pub visited: usize,
    /// Filled rects drawn.
    pub fills: usize,
    /// Text runs drawn.
    pub texts: usize,
    /// Token names no colour could be found for, sorted.
    pub unresolved_tokens: BTreeSet<String>,
    /// Content kinds this pass has no painter for, sorted.
    pub undrawn: BTreeSet<String>,
}

impl PaintReport {
    /// Whether every placement in the frame was visited.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.visited == self.placements
    }
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
        ..PaintReport::default()
    };
    for (placement, content) in frame.paint_pairs() {
        report.visited += 1;
        let clip = to_egui(placement.clip);
        if !clip.is_positive() {
            // Clipped to nothing: the node is scrolled out or behind a closed
            // surface. Counted as visited — it was considered, and skipping the
            // draw is the correct outcome, not a lost placement.
            continue;
        }
        let mut p = painter.with_clip_rect(clip);
        p.set_opacity(placement.opacity.clamp(0.0, 1.0));
        paint_one(&p, placement, content, shaper, colors, &mut report);
    }
    report
}

fn paint_one(
    painter: &Painter,
    placement: &Placement,
    content: &PaintContent,
    shaper: &mut GalleyShaper,
    colors: &dyn ColorSource,
    report: &mut PaintReport,
) {
    let rect = to_egui(placement.rect);

    if let Some(token) = content.tokens.get(BACKGROUND_SLOT) {
        match colors.color(token) {
            Some(color) => {
                painter.rect_filled(rect, 0.0, color);
                report.fills += 1;
            }
            None => {
                report.unresolved_tokens.insert(token.clone());
            }
        }
    }
    if let Some(token) = content.tokens.get(BORDER_SLOT) {
        match colors.color(token) {
            Some(color) => {
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, color), egui::StrokeKind::Inside);
            }
            None => {
                report.unresolved_tokens.insert(token.clone());
            }
        }
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
    }

    if content.image.is_some() {
        report.undrawn.insert("image".into());
    }
    if let Some(name) = &content.custom {
        report.undrawn.insert(format!("custom:{name}"));
    }
}

fn to_egui(rect: PetraRect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.x, rect.y),
        egui::vec2(rect.w.max(0.0), rect.h.max(0.0)),
    )
}

/// Prove the paint accounting can detect a skipped placement.
///
/// Called by this crate's invariant companion. The accounting only earns its
/// keep if an incomplete pass reports incomplete, so that is what is asserted
/// rather than that a complete one reports complete.
pub fn verify_paint_accounting() {
    let complete = PaintReport {
        placements: 3,
        visited: 3,
        ..PaintReport::default()
    };
    assert!(
        complete.is_complete(),
        "gorgon-petra-egui: a pass that visited every placement must read as complete"
    );
    let dropped = PaintReport {
        placements: 3,
        visited: 2,
        ..PaintReport::default()
    };
    assert!(
        !dropped.is_complete(),
        "gorgon-petra-egui: a pass that dropped a placement must not read as complete; \
         the digest cannot see a missing panel, so this accounting is what does"
    );
}

#[cfg(test)]
mod tests {
    use super::{ColorSource, PaintReport, paint_frame, verify_paint_accounting};
    use egui::{Color32, Context, Id, LayerId, Order, RawInput};
    use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
    use gorgon_petra::geom::Size;
    use gorgon_petra::testing::Harness;
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
            node,
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
        assert!(report.is_complete());
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

    #[test]
    fn the_accounting_detects_a_dropped_placement() {
        verify_paint_accounting();
        assert!(
            !PaintReport {
                placements: 1,
                visited: 0,
                ..PaintReport::default()
            }
            .is_complete()
        );
    }
}
