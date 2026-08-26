//! A canvas's picture is in the frame digest, exactly once and always the
//! same way.
//!
//! Two claims, and the second is the one that makes the first worth having:
//!
//! 1. **Stability.** A hundred petrifications of one canvas tree produce one
//!    digest. `contracts/frame-identity.md`'s determinism claim, at the node
//!    kind that carries the most floats per placement of any kind Petra has.
//! 2. **Sensitivity.** Change one number inside one command and the digest
//!    moves. A digest that were stable and insensitive would be a constant,
//!    and a constant passes claim 1 perfectly.
//!
//! This runs through `petrify`, not through `hash_draw_list`: the unit tests
//! in `frame::digest` already hold the stream itself, and what is untested
//! without this file is whether a canvas's payload ever *reaches* the frame —
//! whether `paint_content_of` reads `props.canvas`, whether `attach` hashes
//! it, and whether the leaf hash carries the result. A canvas that drew
//! beautifully and hashed to zero would pass every unit test in the crate.

use std::sync::Arc;

use gorgon_petra::draw::{
    Affine, AssetRef, ColorRef, Command, Corners, DrawList, Fit, Paint, PathVerb, Stroke, Width,
};
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Point, Rect, Size};
use gorgon_petra::testing::{Harness, gap, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};

/// One of every shape command, on coordinates that are not round.
///
/// Not round for the same reason the parity vectors are not: a fixture laid
/// out on integer boundaries would still be stable under a digest that
/// truncated every float to an integer, and truncation is exactly the kind of
/// "harmless" change `contracts/draw-list.md` §4 forbids.
fn a_picture() -> Vec<Command> {
    vec![
        Command::Push {
            transform: Some(Affine {
                tx: 2.375,
                ty: 1.125,
                sx: 1.25,
                sy: 0.75,
            }),
            clip: Some(Rect::new(0.5, 0.25, 180.75, 70.125)),
            opacity: Some(0.8125),
        },
        Command::Rect {
            rect: Rect::new(3.375, 4.625, 40.75, 22.125),
            radius: Corners::all(2.25),
            snap: true,
            paint: Paint::filled(ColorRef::Token("surface.raised".to_owned())),
        },
        Command::Ellipse {
            center: Point::new(24.3125, 15.6875),
            radii: Size::new(6.125, 3.375),
            paint: Paint {
                fill: Some(ColorRef::Rgba([31, 63, 127, 255])),
                stroke: Some(Stroke {
                    width: Width::Device(1.5),
                    color: ColorRef::Token("border.subtle".to_owned()),
                }),
            },
        },
        Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(1.3125, 30.1875)),
                PathVerb::CubicTo {
                    c1: Point::new(12.5625, 12.0625),
                    c2: Point::new(30.1875, 20.3125),
                    to: Point::new(44.9375, 34.8125),
                },
            ],
            closed: false,
            paint: Paint::stroked(Stroke {
                width: Width::Logical(1.8125),
                color: ColorRef::Token("text.primary".to_owned()),
            }),
        },
        Command::Sprite {
            asset: AssetRef::host("cat.png"),
            dst: Rect::new(2.75, 3.25, 18.5, 11.125),
            src: Some(Rect::new(0.25, 0.5, 9.75, 5.25)),
            fit: Fit::Contain,
            tint: None,
        },
        Command::Pop,
    ]
}

/// A canvas of a fixed size beside a label, so the canvas is not the whole
/// frame and its own rect is the thing that moved if a rect moves.
fn tree(commands: Vec<Command>) -> ViewNode {
    let list = DrawList::new(commands).expect("the fixture is inside every draw-list bound");
    ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            spacing: gap(2.0),
            ..Props::default()
        })
        .child(ViewNode::new(NodeKind::Text, "caption").with_props(Props {
            text: Some("a canvas".to_owned()),
            ..Props::default()
        }))
        .child(
            ViewNode::new(NodeKind::Canvas, "plot")
                .with_props(Props {
                    canvas: Some(Arc::new(list)),
                    ..Props::default()
                })
                .with_constraints(fixed(190.5, 84.25)),
        )
}

fn fixed(w: f32, h: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(w),
            max: Some(w),
            priority: 5,
        },
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 5,
        },
    }
}

fn frame_of(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(400.5, 300.25), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

/// The canvas's placement and its payload, or a panic naming what is missing.
fn canvas_payload(frame: &PetrifiedFrame) -> (u64, &Arc<DrawList>) {
    let (placement, content) = frame
        .drawn()
        .find(|(p, _)| p.id == "/root/plot")
        .expect("the fixture places a canvas at /root/plot");
    assert_eq!(placement.kind, NodeKind::Canvas);
    let list = content
        .canvas
        .as_ref()
        .expect("a canvas placement must carry the list its props declared");
    (placement.paint.paint_hash, list)
}

/// Claim 1: a hundred runs, one digest — and one *frame*, since two frames can
/// share a digest and still disagree about something the digest does not
/// distinguish.
#[test]
fn a_hundred_petrifications_of_a_canvas_give_one_digest() {
    let tree = tree(a_picture());
    let first = frame_of(&tree);
    let (first_hash, first_list) = canvas_payload(&first);

    assert_ne!(
        first_hash, 0,
        "a canvas payload that hashed to zero would be a picture the digest \
         cannot see at all — the exact failure `PaintContent::is_empty` has a \
         canvas arm for"
    );
    assert_eq!(first_list.len(), 6, "every command reached the frame");
    assert!(
        first.paint_hashes_agree(),
        "the placement's paint hash must describe the payload beside it"
    );

    for run in 0..100 {
        let again = frame_of(&tree);
        assert_eq!(again.digest, first.digest, "run {run}: the digest moved");
        assert_eq!(again.placements, first.placements, "run {run}");
        assert_eq!(again.content, first.content, "run {run}");
        assert_eq!(canvas_payload(&again).0, first_hash, "run {run}");
    }
}

/// Claim 2, the sensitivity half: change exactly one thing inside one command
/// and both the payload hash and the whole frame digest move.
///
/// A table rather than one case, because the stream writes a different field
/// list per command: a mutation the stream missed would be invisible with any
/// single row, and every row here names a picture a reader would see change.
#[test]
fn changing_one_command_changes_the_digest() {
    let base_tree = tree(a_picture());
    let base = frame_of(&base_tree);
    let (base_hash, _) = canvas_payload(&base);

    type Mutate = fn(&mut Vec<Command>);
    let table: &[(&str, Mutate)] = &[
        ("the whole list emptied", |commands| commands.clear()),
        ("one command removed", |commands| {
            commands.remove(2);
        }),
        ("two commands swapped", |commands| commands.swap(1, 2)),
        ("a rect edge moved a sixteenth of a unit", |commands| {
            if let Command::Rect { rect, .. } = &mut commands[1] {
                rect.x += 0.0625;
            }
        }),
        ("a corner radius", |commands| {
            if let Command::Rect { radius, .. } = &mut commands[1] {
                radius.top_left += 0.5;
            }
        }),
        ("the snap flag", |commands| {
            if let Command::Rect { snap, .. } = &mut commands[1] {
                *snap = !*snap;
            }
        }),
        ("an ellipse radius", |commands| {
            if let Command::Ellipse { radii, .. } = &mut commands[2] {
                radii.w += 0.0625;
            }
        }),
        ("a stroke width", |commands| {
            if let Command::Ellipse { paint, .. } = &mut commands[2]
                && let Some(stroke) = paint.stroke.as_mut()
            {
                stroke.width = Width::Device(2.5);
            }
        }),
        ("logical against device width, same number", |commands| {
            if let Command::Ellipse { paint, .. } = &mut commands[2]
                && let Some(stroke) = paint.stroke.as_mut()
            {
                stroke.width = Width::Logical(1.5);
            }
        }),
        ("a bezier control point", |commands| {
            if let Command::Path { verbs, .. } = &mut commands[3]
                && let PathVerb::CubicTo { c2, .. } = &mut verbs[1]
            {
                c2.x += 0.0625;
            }
        }),
        ("a rebound colour token", |commands| {
            if let Command::Rect { paint, .. } = &mut commands[1] {
                paint.fill = Some(ColorRef::Token("status.down".to_owned()));
            }
        }),
        ("a literal colour channel", |commands| {
            if let Command::Ellipse { paint, .. } = &mut commands[2] {
                paint.fill = Some(ColorRef::Rgba([31, 63, 128, 255]));
            }
        }),
        ("the sprite's asset", |commands| {
            if let Command::Sprite { asset, .. } = &mut commands[4] {
                *asset = AssetRef::host("dog.png");
            }
        }),
        ("the sprite's owner", |commands| {
            if let Command::Sprite { asset, .. } = &mut commands[4] {
                *asset = AssetRef::new("plugin.weather", "cat.png");
            }
        }),
        ("the sprite's fit", |commands| {
            if let Command::Sprite { fit, .. } = &mut commands[4] {
                *fit = Fit::Fill;
            }
        }),
        ("the sprite's tint appearing", |commands| {
            if let Command::Sprite { tint, .. } = &mut commands[4] {
                *tint = Some(ColorRef::Rgba([255, 255, 255, 128]));
            }
        }),
        ("the pushed opacity", |commands| {
            if let Command::Push { opacity, .. } = &mut commands[0] {
                *opacity = Some(0.75);
            }
        }),
        ("the pushed transform", |commands| {
            if let Command::Push { transform, .. } = &mut commands[0] {
                *transform = Some(Affine::translate(2.375, 1.125));
            }
        }),
        ("the pushed clip", |commands| {
            if let Command::Push { clip, .. } = &mut commands[0] {
                *clip = None;
            }
        }),
    ];

    for (what, mutate) in table {
        let mut commands = a_picture();
        mutate(&mut commands);
        assert_ne!(
            commands,
            a_picture(),
            "{what}: the mutation changed nothing"
        );
        let moved = frame_of(&tree(commands));
        let (moved_hash, _) = canvas_payload(&moved);
        assert_ne!(
            moved_hash, base_hash,
            "{what} decides the picture and the paint hash cannot see it"
        );
        assert_ne!(
            moved.digest, base.digest,
            "{what} moved the paint hash and the frame digest did not follow"
        );
    }
}

/// Two canvases drawing the same picture in the same box are the same frame,
/// and the same list handed back through a shared `Arc` is too.
///
/// The lower bound on sensitivity. A digest that moved for a re-built but
/// identical list would make every canvas frame a "changed" frame and would
/// undo what the `Arc` in the payload is for.
#[test]
fn an_identical_list_rebuilt_from_scratch_is_the_same_frame() {
    let shared = Arc::new(DrawList::new(a_picture()).unwrap());
    let with_shared = |list: Arc<DrawList>| {
        ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "caption").with_props(Props {
                text: Some("a canvas".to_owned()),
                ..Props::default()
            }))
            .child(
                ViewNode::new(NodeKind::Canvas, "plot")
                    .with_props(Props {
                        canvas: Some(list),
                        ..Props::default()
                    })
                    .with_constraints(fixed(190.5, 84.25)),
            )
    };
    let rebuilt = Arc::new(DrawList::new(a_picture()).unwrap());
    assert!(
        !Arc::ptr_eq(&shared, &rebuilt),
        "the two must be different allocations for this test to mean anything"
    );
    assert_eq!(
        frame_of(&with_shared(shared)).digest,
        frame_of(&with_shared(rebuilt)).digest
    );
}

/// The split predicate, through a real frame rather than through a payload
/// literal (`contracts/draw-list.md` §6).
///
/// A geometry-only canvas is **not** hosted — every coordinate it draws is
/// hashed, so digest equality is picture equality over its rect — and is still
/// reported as able to repaint itself, because it rebuilds its list to change
/// anything. Adding one `Sprite` moves it across the first line and not the
/// second.
#[test]
fn a_geometry_only_canvas_is_unhosted_and_still_self_repainting() {
    let geometry: Vec<Command> = a_picture()
        .into_iter()
        .filter(|c| !matches!(c, Command::Sprite { .. }))
        .collect();
    let frame = frame_of(&tree(geometry));
    assert!(
        !frame.hosted(),
        "a canvas with no Sprite has no pixels the digest cannot see"
    );
    assert_eq!(frame.hosted_placements().count(), 0);
    let repainting: Vec<&str> = frame
        .self_repainting_placements()
        .map(|(p, _)| p.id.as_str())
        .collect();
    assert_eq!(
        repainting,
        ["/root/plot"],
        "the canvas is still a surface FR-030 must attribute a repaint to"
    );

    let sprited = frame_of(&tree(a_picture()));
    assert!(
        sprited.hosted(),
        "one Sprite is enough: the digest sees the asset's name, never its pixels"
    );
    let hosted: Vec<&str> = sprited
        .hosted_placements()
        .map(|(p, _)| p.id.as_str())
        .collect();
    assert_eq!(hosted, ["/root/plot"]);
}

/// A canvas takes the room its constraints ask for and nothing else
/// (`contracts/draw-list.md` §7), and it never asks the measurement registry.
///
/// The second half is what makes `canvas` a second kind rather than a mode of
/// `custom`: a `custom` node with no registered measurer is a tree-acceptance
/// error, and a canvas with a list of ten thousand commands is still a node
/// whose size its parent decided.
#[test]
fn a_canvas_sizes_from_its_constraints_and_never_from_its_list() {
    let small = frame_of(&tree(a_picture()));
    let big_picture: Vec<Command> = {
        let mut commands = a_picture();
        // A shape far outside the canvas's own box. A kind that measured its
        // list would grow; a canvas does not.
        commands.insert(
            1,
            Command::Rect {
                rect: Rect::new(0.0, 0.0, 9000.0, 9000.0),
                radius: Corners::SQUARE,
                snap: false,
                paint: Paint::filled(ColorRef::Token("surface.base".to_owned())),
            },
        );
        commands
    };
    let big = frame_of(&tree(big_picture));

    let rect = |frame: &PetrifiedFrame| {
        frame
            .placement("/root/plot")
            .expect("the canvas is placed")
            .rect
    };
    assert_eq!(rect(&small).w, 190.5);
    assert_eq!(rect(&small).h, 84.25);
    assert_eq!(
        rect(&big),
        rect(&small),
        "a draw list has no intrinsic size, so a bigger picture is the same box"
    );
    assert_ne!(
        big.digest, small.digest,
        "the same box drawing a different picture is still a different frame"
    );
}

/// A canvas that declares no list draws nothing and hashes to nothing, rather
/// than being a canvas-shaped hole with a nonzero payload.
#[test]
fn a_canvas_with_no_list_carries_no_payload() {
    let bare = ViewNode::new(NodeKind::Stack, "root")
        .child(ViewNode::new(NodeKind::Canvas, "plot").with_constraints(fixed(190.5, 84.25)));
    let frame = frame_of(&bare);
    let (_, content) = frame
        .drawn()
        .find(|(p, _)| p.id == "/root/plot")
        .expect("the canvas is placed");
    assert!(content.canvas.is_none());
    assert!(content.is_empty(), "nothing declared is nothing to draw");
    assert!(!frame.hosted());
    assert_eq!(
        frame.self_repainting_placements().count(),
        0,
        "a canvas with no list has nothing to rebuild, so it repaints nothing"
    );
}
