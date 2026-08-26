//! The acceptance scene for the draw list: a cat walking a full-width strip.
//!
//! `specs/005-petra-carbon-authoring/contracts/draw-list.md` T133. It is one
//! scene and it is doing four jobs at once, which is why it is a file rather
//! than a fixture inside a test:
//!
//! 1. **It is a real picture drawn from the six commands.** Ellipses for the
//!    body and head, convex filled paths for the ears, a stroked cubic for the
//!    tail, rects for the legs. Nothing here is a coloured box standing in for
//!    a cat.
//! 2. **It is the divergence case.** The cat is drawn from *geometry only* —
//!    there is no `Sprite` in the list — so it is digest-visible and therefore
//!    never `is_hosted`, and it rebuilds and repaints itself and therefore
//!    always `repaints_itself`. `contracts/draw-list.md` §6 binds those two
//!    predicates apart, and this is the one payload shape that tells them
//!    apart.
//! 3. **It is the engine's ambient loop, end to end.** The list is static per
//!    frame; what moves is the canvas's *placement*, driven by an ambient
//!    keyframe track the engine loops on `t mod duration` (§8). No host
//!    re-triggers anything, and the cat does not walk off the right-hand side
//!    and stop.
//! 4. **It is what the `idle-audit` lane fails on.** The scene declares
//!    `ambient`, which is what accounts for a surface that repaints forever.
//!    Take [`Scene::AMBIENT`] away and the lane goes red naming `/root/strip/
//!    cat` — that is the whole point of the lane, and `tests/cat_idle.rs` is
//!    where both directions are held.
//!
//! # Why the cat is not a sprite
//!
//! A sprite would be the obvious way to draw a cat, and it would make this
//! scene test the *wrong* thing. A canvas carrying a `Sprite` is hosted: the
//! digest sees `host/cat.png` and the rects, never the pixels. Drawn from
//! geometry, every coordinate of the cat is in the frame digest — so the
//! scene proves the guarantee a canvas exists to give, and it lands on the one
//! side of §6 where the two predicates disagree.

// Two targets compile this file and each uses only the part it needs: the
// gallery binary draws the cat and prints its cost, and
// `tests/cat_idle.rs` — which reaches a `src/bin/` module the only way Cargo
// offers, `#[path]` — drives the whole scene through the shipped `Host`. So
// items the *other* target uses read as dead here. Same arrangement, and same
// reason, as `tests/support/mod.rs`.
#![allow(dead_code)]

use std::sync::Arc;

use gorgon_petra::anim::curve::{CubicBezier, Keyframe, KeyframeTrack};
use gorgon_petra::anim::registry::{Timing, Track, TransitionDef, TransitionRegistry};
use gorgon_petra::anim::value::{AnimVector, PropertyKind};
use gorgon_petra::draw::{ColorRef, Command, Corners, DrawList, Paint, PathVerb, Stroke, Width};
use gorgon_petra::geom::{Axis, Point, Rect, Size};
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};

/// Everything about the scene a test or a gate needs to name.
pub struct Scene;

impl Scene {
    /// The canvas node's key, and the tail of its placement id.
    pub const CAT: &'static str = "cat";
    /// The strip the cat walks along.
    pub const STRIP: &'static str = "strip";
    /// The canvas placement's full id, which is what the idle audit names when
    /// the declaration is missing.
    pub const CAT_ID: &'static str = "/root/strip/cat";
    /// The transition definition the scene names.
    pub const TRANSITION: &'static str = "cat-walk";
    /// Whether the scene declares itself ambient.
    ///
    /// A named constant rather than a literal at the call site because it is
    /// the one line the `idle-audit` lane is a test of. Setting it to `false`
    /// is the sabotage `tests/cat_idle.rs` performs from the other side, and
    /// it must turn the lane red.
    pub const AMBIENT: bool = true;
    /// How wide the cat's own box is, logical units.
    pub const WIDTH: f32 = 48.0;
    /// How tall it is.
    pub const HEIGHT: f32 = 32.0;
    /// How tall the strip it walks along is. Taller than the cat, so the walk
    /// has ground under it and the canvas is visibly a box inside a band.
    pub const STRIP_HEIGHT: f32 = 40.0;
    /// One lap, seconds. The engine wraps the track on this
    /// (`Timing::looped_time`), so the cat re-enters from the left rather than
    /// stopping at the right.
    pub const LAP_SECONDS: f64 = 6.0;
}

/// The cat, as a draw list in canvas-local logical units.
///
/// Every filled closed path here is convex, which is not a coincidence:
/// [`DrawList::new`] refuses a non-convex one, so a cat drawn as one outline
/// with four legs and two ears would not construct. Decomposing it into convex
/// pieces is the author's job, and this is what that looks like — the ears are
/// two triangles rather than two notches in the head.
///
/// # Panics
/// Never in practice: the list is a fixed twelve commands, well inside
/// `MAX_DRAW_COMMANDS`, and every filled path below is a convex triangle. A
/// panic here would mean an edit broke one of those, which is a defect and not
/// a runtime condition to handle.
#[must_use]
pub fn drawing() -> DrawList {
    let fur = || ColorRef::Token("text.primary".to_owned());
    let eye = || ColorRef::Token("accent.primary".to_owned());
    let outline = Stroke {
        width: Width::Device(1.0),
        color: ColorRef::Token("border.subtle".to_owned()),
    };
    let leg = |x: f32| Command::Rect {
        rect: Rect::new(x, 24.5, 4.5, 7.0),
        radius: Corners::all(1.5),
        snap: false,
        paint: Paint::filled(fur()),
    };
    let ear = |x: f32| Command::Path {
        verbs: vec![
            PathVerb::MoveTo(Point::new(x, 8.5)),
            PathVerb::LineTo(Point::new(x + 3.0, 1.5)),
            PathVerb::LineTo(Point::new(x + 6.0, 8.5)),
        ],
        closed: true,
        paint: Paint::filled(fur()),
    };

    DrawList::new(vec![
        // Tail first, so the body draws over where it joins.
        Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(7.5, 19.0)),
                PathVerb::CubicTo {
                    c1: Point::new(0.5, 15.5),
                    c2: Point::new(1.5, 4.5),
                    to: Point::new(8.0, 3.5),
                },
            ],
            closed: false,
            paint: Paint::stroked(Stroke {
                width: Width::Logical(2.5),
                color: ColorRef::Token("text.primary".to_owned()),
            }),
        },
        leg(12.0),
        leg(20.0),
        leg(28.0),
        Command::Ellipse {
            center: Point::new(22.0, 20.0),
            radii: Size::new(15.0, 8.5),
            paint: Paint {
                fill: Some(fur()),
                stroke: Some(outline.clone()),
            },
        },
        ear(31.0),
        ear(39.0),
        Command::Ellipse {
            center: Point::new(38.0, 13.5),
            radii: Size::new(8.0, 7.0),
            paint: Paint {
                fill: Some(fur()),
                stroke: Some(outline),
            },
        },
        Command::Ellipse {
            center: Point::new(35.5, 12.0),
            radii: Size::new(1.25, 1.75),
            paint: Paint::filled(eye()),
        },
        Command::Ellipse {
            center: Point::new(41.0, 12.0),
            radii: Size::new(1.25, 1.75),
            paint: Paint::filled(eye()),
        },
        // Whiskers, under one Push so the pair share a fade without either
        // command carrying an opacity of its own.
        Command::Push {
            transform: None,
            clip: None,
            opacity: Some(0.6),
        },
        Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(41.0, 15.5)),
                PathVerb::LineTo(Point::new(47.5, 14.0)),
                PathVerb::MoveTo(Point::new(41.0, 16.5)),
                PathVerb::LineTo(Point::new(47.5, 18.0)),
            ],
            closed: false,
            paint: Paint::stroked(Stroke {
                width: Width::Device(1.0),
                color: ColorRef::Token("text.muted".to_owned()),
            }),
        },
        Command::Pop,
    ])
    .expect("the cat is twelve convex commands, well inside every draw-list bound")
}

/// The scene: a full-width strip along the bottom of the window with the cat
/// canvas in it.
///
/// The strip is an `overlay` rather than a `stack` so the canvas's laid-out
/// position is the strip's own top-left corner, which is where the walk starts
/// from. A stack would centre or distribute it, and the keyframe track's
/// absolute values would then fight the layout instead of replacing it.
#[must_use]
pub fn scene() -> ViewNode {
    let cat = ViewNode::new(NodeKind::Canvas, Scene::CAT)
        .with_props(Props {
            canvas: Some(Arc::new(drawing())),
            // The canvas's own backing plate, so the cat walks on a surface
            // rather than on whatever happens to be behind the strip.
            //
            // It is also, today, what keeps the shipped paint pass from
            // reporting this placement **silent**: `petra-egui/src/paint.rs`
            // does not yet read `PaintContent::canvas` — that is T135, and it
            // is not this lane's file — so a canvas that bound no token at all
            // would declare content and emit nothing. Stated here rather than
            // left to be discovered: until T135 lands, the shipped host paints
            // this rect and not the cat, and
            // `gorgon_petra_egui::draw::paint_canvas` is the only thing that
            // draws the cat.
            tokens: [(
                "background".to_owned(),
                TokenName::new("surface.base").expect("a shipped token name"),
            )]
            .into_iter()
            .collect(),
            ..Props::default()
        })
        .with_constraints(fixed(Scene::WIDTH, Scene::HEIGHT))
        .with_transition(Scene::TRANSITION)
        // The line the `idle-audit` lane is a test of. Without it the cat is
        // an undeclared continuous repaint and the lane goes red naming
        // `Scene::CAT_ID`.
        .with_ambient(Scene::AMBIENT);

    ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(ViewNode::new(NodeKind::Spacer, "sky"))
        .child(
            ViewNode::new(NodeKind::Overlay, Scene::STRIP)
                .with_props(Props {
                    tokens: [(
                        "background".to_owned(),
                        TokenName::new("surface.layer-one").expect("a shipped token name"),
                    )]
                    .into_iter()
                    .collect(),
                    ..Props::default()
                })
                .with_constraints(Constraints {
                    horizontal: AxisConstraint::default(),
                    vertical: AxisConstraint {
                        min: Some(Scene::STRIP_HEIGHT),
                        max: Some(Scene::STRIP_HEIGHT),
                        priority: 10,
                    },
                })
                .child(cat),
        )
}

/// The ambient walk: a keyframe track over `Position`, looped by the engine.
///
/// Four keyframes over [`Scene::LAP_SECONDS`], from the left edge of the
/// window to one cat-width past the right edge, at the strip's own `y`. The
/// track carries **absolute** values, which is what a keyframe track means
/// here — while it runs it replaces the laid-out position rather than
/// interpolating toward it.
///
/// It declares an `enter` track because that is what starts a trajectory for a
/// node whose laid-out position never changes: without one the engine has no
/// reason to animate a node that was placed where it belongs, and the cat
/// would sit still under a declaration that says it never stops.
///
/// # Errors
/// If the keyframe times are not increasing or the track's width does not
/// match `Position` — both of which are properties of the literals below, so
/// an error here is an edit that broke them.
pub fn walk(window: Size) -> Result<TransitionDef, String> {
    let y = f64::from(window.h - Scene::STRIP_HEIGHT + (Scene::STRIP_HEIGHT - Scene::HEIGHT) / 2.0);
    let start = -f64::from(Scene::WIDTH);
    let end = f64::from(window.w);
    let at = |time: f64, x: f64| Keyframe {
        time,
        value: AnimVector::new([x, y, 0.0, 0.0], 2),
        easing_in: CubicBezier::LINEAR,
    };
    let track = KeyframeTrack::new(vec![
        at(0.0, start),
        at(Scene::LAP_SECONDS / 3.0, start + (end - start) / 3.0),
        at(
            Scene::LAP_SECONDS * 2.0 / 3.0,
            start + (end - start) * 2.0 / 3.0,
        ),
        at(Scene::LAP_SECONDS, end),
    ])
    .map_err(|err| format!("the cat's walk track: {err}"))?;

    TransitionDef::builder()
        .drive(PropertyKind::Position, Timing::Keyframes(track))
        .enter(
            Track::new(
                PropertyKind::Position,
                AnimVector::new([start, y, 0.0, 0.0], 2),
            )
            .map_err(|err| format!("the cat's enter track: {err}"))?,
        )
        .ambient(true)
        .build()
}

/// A registry holding just the cat's walk, ready for
/// `gorgon_petra_egui::host::Host::set_transitions`.
///
/// # Errors
/// Whatever [`walk`] reports.
pub fn transitions(window: Size) -> Result<TransitionRegistry, String> {
    let mut registry = TransitionRegistry::new();
    registry.register(Scene::TRANSITION, walk(window)?);
    Ok(registry)
}

/// A both-axes-pinned constraint, the way a canvas asks for room.
///
/// A canvas answers an unbounded probe with nothing
/// (`gorgon_petra::layout::leaf`), so this is not decoration: without it the
/// cat would be placed at zero size and the scene would be an empty strip.
fn fixed(w: f32, h: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(w),
            max: Some(w),
            priority: 10,
        },
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 10,
        },
    }
}
