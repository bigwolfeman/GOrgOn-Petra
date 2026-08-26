//! Petra's fourth primitive: the triangle, and the one thing that rides it.
//!
//! Rects, outlines and text are the three shapes this painter drew before
//! this module existed (`contracts/view-tree.md` §Hosted content). A caret —
//! the pointer a popover draws back at the control that opened it — is none
//! of the three, and it could not be smuggled in as either of the two hosted
//! payloads: [`gorgon_petra::frame::PaintContent::is_hosted`] makes `custom`
//! digest-blind, so a caret riding it would move from one side of a popover
//! to the other without moving the frame digest. So the shape is drawn here,
//! by the engine's own painter, from geometry the engine resolved and hashed
//! (`contracts/anchored-placement.md` §5).
//!
//! Its own file rather than another block in `paint.rs` because a primitive
//! is a thing with an outline, a fill rule and a device-grid story of its
//! own, and `paint.rs` is already the longest module in this crate.

use egui::{Color32, Painter, Pos2, Stroke};
use gorgon_petra::frame::placement::CaretPaint;
use gorgon_petra::geom::Scale;
use gorgon_petra::tree::Edge;

/// The three corners of `caret`, in logical units: the tip that touches the
/// anchor, then the two ends of its base on the surface's near edge.
///
/// Split out from [`paint_caret`] because it is the whole of the shape's
/// definition and it needs no painter to check: a test can compare these
/// three points against the rect the surface was placed at, which is what
/// "the caret grows out of the near edge" actually means.
#[must_use]
pub fn caret_points(caret: &CaretPaint) -> [Pos2; 3] {
    let half = caret.w / 2.0;
    let tip = Pos2::new(caret.tip_x, caret.tip_y);
    // The base runs along the surface's near edge, which is `h` back from the
    // tip in whichever direction the caret points.
    match caret.side {
        // The surface is below its anchor, so the caret points up: the base
        // is `h` below the tip, running horizontally.
        Edge::Bottom => [
            tip,
            Pos2::new(caret.tip_x - half, caret.tip_y + caret.h),
            Pos2::new(caret.tip_x + half, caret.tip_y + caret.h),
        ],
        Edge::Top => [
            tip,
            Pos2::new(caret.tip_x + half, caret.tip_y - caret.h),
            Pos2::new(caret.tip_x - half, caret.tip_y - caret.h),
        ],
        Edge::Right => [
            tip,
            Pos2::new(caret.tip_x + caret.h, caret.tip_y + half),
            Pos2::new(caret.tip_x + caret.h, caret.tip_y - half),
        ],
        Edge::Left => [
            tip,
            Pos2::new(caret.tip_x - caret.h, caret.tip_y - half),
            Pos2::new(caret.tip_x - caret.h, caret.tip_y + half),
        ],
    }
}

/// Draw `caret` in `fill`, and report whether anything was drawn.
///
/// `None` for `fill` is a surface that binds no `background` token: a caret
/// is a continuation of the surface's own fill past its edge, so there is no
/// colour to draw it in and nothing is drawn. The caller records that in the
/// paint report rather than this function inventing a colour — a visibly
/// wrong caret hanging off a popover is worse than none, and unlike text (see
/// `paint.rs`'s `Color32::PLACEHOLDER` on an unresolved foreground) a missing
/// caret costs the reader no information the surface itself does not already
/// carry.
///
/// `scale` is taken for the same reason every other shape in this crate takes
/// it: the tip is snapped to the device grid so a caret at a fractional
/// scale lands where the rest of the frame does. The base corners are snapped
/// with it, so the shape stays symmetric about the tip.
pub fn paint_caret(
    painter: &Painter,
    caret: &CaretPaint,
    fill: Option<Color32>,
    scale: Scale,
) -> bool {
    let Some(color) = fill else {
        return false;
    };
    if caret.w <= 0.0 || caret.h <= 0.0 {
        return false;
    }
    let points = caret_points(caret)
        .into_iter()
        .map(|p| snap(p, scale))
        .collect::<Vec<_>>();
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    true
}

/// One point on the same device grid `gorgon_petra::frame::round_rect` puts
/// every rect on, returned in logical units so egui's own rounding is a
/// no-op rather than a second opinion.
fn snap(point: Pos2, scale: Scale) -> Pos2 {
    let f = scale.factor();
    Pos2::new((point.x * f).round() / f, (point.y * f).round() / f)
}

#[cfg(test)]
mod tests {
    use super::{caret_points, paint_caret};
    use gorgon_petra::frame::placement::CaretPaint;
    use gorgon_petra::geom::Scale;
    use gorgon_petra::tree::Edge;

    fn caret(side: Edge) -> CaretPaint {
        CaretPaint {
            side,
            tip_x: 100.0,
            tip_y: 200.0,
            w: 12.0,
            h: 6.0,
        }
    }

    /// The tip is where the engine put it and the base is a full `w` wide,
    /// `h` back from the tip, on every side.
    ///
    /// Hand-computed rather than derived from the same expressions the code
    /// uses: a test that recomputed `tip_y + h` would agree with a sign error
    /// as readily as with the truth.
    #[test]
    fn the_caret_points_away_from_the_surface_on_every_side() {
        let up = caret_points(&caret(Edge::Bottom));
        assert_eq!((up[0].x, up[0].y), (100.0, 200.0));
        assert_eq!((up[1].x, up[1].y), (94.0, 206.0));
        assert_eq!((up[2].x, up[2].y), (106.0, 206.0));

        let down = caret_points(&caret(Edge::Top));
        assert_eq!((down[1].x, down[1].y), (106.0, 194.0));
        assert_eq!((down[2].x, down[2].y), (94.0, 194.0));

        let left = caret_points(&caret(Edge::Right));
        assert_eq!((left[1].x, left[1].y), (106.0, 206.0));
        assert_eq!((left[2].x, left[2].y), (106.0, 194.0));

        let right = caret_points(&caret(Edge::Left));
        assert_eq!((right[1].x, right[1].y), (94.0, 194.0));
        assert_eq!((right[2].x, right[2].y), (94.0, 206.0));
    }

    /// Every side's base is exactly `w` long and exactly `h` from the tip,
    /// which is the property the four hand-computed cases above are instances
    /// of.
    #[test]
    fn every_side_puts_the_base_a_full_depth_behind_the_tip() {
        for side in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            let c = caret(side);
            let [tip, a, b] = caret_points(&c);
            assert!(
                (a.distance(b) - c.w).abs() < 0.001,
                "{side:?}: base is {} long, want {}",
                a.distance(b),
                c.w
            );
            let mid = egui::Pos2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            assert!(
                (tip.distance(mid) - c.h).abs() < 0.001,
                "{side:?}: tip is {} from the base midpoint, want {}",
                tip.distance(mid),
                c.h
            );
        }
    }

    /// A headless egui context that hands back its texture deltas on drop.
    ///
    /// `epaint` panics on a `TexturesDelta` dropped with unapplied deltas, so
    /// a test that opens a context and never runs a pass over it fails for a
    /// reason that has nothing to do with the code under test. Same shape as
    /// `paint.rs`'s own test harness.
    struct Headless(egui::Context);

    impl Headless {
        fn new() -> Self {
            let ctx = egui::Context::default();
            Self::pass(&ctx);
            Self(ctx)
        }

        fn painter(&self) -> egui::Painter {
            self.0.layer_painter(egui::LayerId::new(
                egui::Order::Background,
                egui::Id::new("petra-caret-test"),
            ))
        }

        fn pass(ctx: &egui::Context) {
            ctx.run_ui(egui::RawInput::default(), |_| {})
                .drop_without_applying_deltas();
        }
    }

    impl Drop for Headless {
        fn drop(&mut self) {
            Self::pass(&self.0);
        }
    }

    /// A surface that binds no background has no colour to draw a caret in,
    /// and nothing is drawn rather than something guessed.
    #[test]
    fn a_caret_with_no_fill_draws_nothing() {
        let headless = Headless::new();
        let painter = headless.painter();
        assert!(!paint_caret(
            &painter,
            &caret(Edge::Bottom),
            None,
            Scale::ONE
        ));
        assert!(
            paint_caret(
                &painter,
                &caret(Edge::Bottom),
                Some(egui::Color32::RED),
                Scale::ONE
            ),
            "the same caret with a colour does draw, so the negative above \
             is about the fill and not about the caret"
        );
    }

    /// A degenerate caret is not drawn: a zero-width or zero-depth triangle
    /// is three collinear points, which tessellates to nothing while still
    /// being counted as a shape by a caller that trusted the return value.
    #[test]
    fn a_degenerate_caret_draws_nothing() {
        let headless = Headless::new();
        let painter = headless.painter();
        for (w, h) in [(0.0, 6.0), (12.0, 0.0), (0.0, 0.0)] {
            let flat = CaretPaint {
                w,
                h,
                ..caret(Edge::Bottom)
            };
            assert!(
                !paint_caret(&painter, &flat, Some(egui::Color32::RED), Scale::ONE),
                "w={w} h={h} must not report a drawn shape"
            );
        }
    }

    /// The call site in `paint.rs` is live: a real petrified frame carrying a
    /// real anchored surface draws one more shape than the same frame without
    /// the caret, and reports it.
    ///
    /// The unit tests above prove the shape is right. This proves the shape
    /// is *reached* — which is the half that a primitive nothing calls would
    /// still pass.
    #[test]
    fn a_petrified_anchored_surface_paints_its_caret_through_the_frame_pass() {
        use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
        use gorgon_petra::geom::Size;
        use gorgon_petra::testing::{Harness, validated};
        use gorgon_petra::token::{ThemeMode, TokenName, dark};
        use gorgon_petra::tree::{Align, Anchor, Layer, NodeKind, Props, ViewNode};

        let mut body = ViewNode::new(NodeKind::Spacer, "body");
        body.constraints.horizontal.min = Some(80.0);
        body.constraints.horizontal.max = Some(80.0);
        body.constraints.vertical.min = Some(40.0);
        body.constraints.vertical.max = Some(40.0);
        let mut button = ViewNode::new(NodeKind::Spacer, "button");
        button.constraints.horizontal.min = Some(120.0);
        button.constraints.horizontal.max = Some(120.0);
        button.constraints.vertical.min = Some(40.0);
        button.constraints.vertical.max = Some(40.0);

        let with_background = |bound: bool| {
            let tokens = if bound {
                [(
                    "background".to_owned(),
                    TokenName::new("surface.raised").unwrap(),
                )]
                .into_iter()
                .collect()
            } else {
                std::collections::BTreeMap::new()
            };
            ViewNode::new(NodeKind::Overlay, "root")
                .child(
                    ViewNode::new(NodeKind::Stack, "bar")
                        .with_props(Props {
                            axis: Some(gorgon_petra::geom::Axis::Vertical),
                            align: Some(gorgon_petra::geom::Align::Start),
                            ..Props::default()
                        })
                        .child(button.clone()),
                )
                .child(
                    ViewNode::new(NodeKind::Surface, "popup")
                        .with_props(Props {
                            layer: Some(Layer::Popup),
                            anchor: Some(Anchor::Node {
                                id: "/root/bar/button".into(),
                                edge: Edge::Bottom,
                                align: Align::Center,
                                offset: None,
                            }),
                            tokens,
                            ..Props::default()
                        })
                        .child(body.clone()),
                )
        };

        let report = |bound: bool| {
            let tree = with_background(bound);
            let mut engine = Harness::new();
            let frame = petrify(
                1,
                validated(&tree),
                &mut engine.ctx(),
                Viewport::new(Size::new(600.0, 400.0), ThemeMode::Dark),
                TransitionActivity::default(),
            );
            assert!(
                frame.content.iter().any(|c| c.caret.is_some()),
                "the engine must have produced a caret for this frame"
            );
            let headless = Headless::new();
            let painter = headless.painter();
            let mut shaper = crate::text::GalleyShaper::new(headless.0.clone());
            let theme = gorgon_petra::token::ThemeSnapshot::new(dark(), 1);
            crate::paint::paint_frame(&painter, &frame, &mut shaper, &theme)
        };

        let drawn = report(true);
        assert!(
            !drawn.undrawn.contains("caret"),
            "a surface that binds a background draws its caret: {:?}",
            drawn.undrawn
        );
        // Two fills: the surface's own background rect, and the caret.
        assert_eq!(drawn.fills, 2, "{drawn:?}");

        let blind = report(false);
        assert!(
            blind.undrawn.contains("caret"),
            "a surface with no background has no colour for a caret, and the \
             pass must say so rather than draw nothing in silence: {:?}",
            blind.undrawn
        );
        assert_eq!(blind.fills, 0, "{blind:?}");
    }
}
