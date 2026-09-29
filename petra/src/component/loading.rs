//! `loading` — Carbon Loading (slice-c): a circular in-progress indicator.
//!
//! # What Carbon draws
//!
//! One SVG, `viewBox 0 0 100 100`, rotated by CSS: `@keyframes rotate` from
//! `0deg` to `360deg`, `690ms linear infinite` (`_animation.scss`, mixin
//! `spin`). In it, a `<circle r=44>` stroked 10 wide in `$interactive`,
//! dashed so that 81% of its circumference is ink and the rest is gap
//! (`loading-progress($circumference, 81)`, `_loading.scss`), `stroke-linecap:
//! butt`. The `--small` form is 16px, `r=42` (`Loading.tsx`), stroked 16 in
//! the viewBox, 48% ink, and — only the small form — a second full circle
//! behind it in `$layer-accent`, the track. The large form has no track:
//! `Loading.tsx` renders `.cds--loading__background` when `small` and not
//! otherwise, and `17-loading.png` shows exactly that.
//!
//! The `description` is the SVG's `<title>`, read by assistive technology
//! and never drawn. So this component draws no caption either; the label
//! goes in [`Semantics`] and nowhere else.
//!
//! # How it turns
//!
//! The picture is a stroked arc of cubics ([`arc_verbs`]) rebuilt for each
//! frame at the phase `now` names, on a canvas that declares `ambient`.
//! That is `contracts/draw-list.md` §8's rule for a canvas whose content
//! changes every frame: *"rebuild and resubmit a new `DrawList`, paying one
//! digest change per frame, and declare `ambient`"*. The draw list itself
//! takes no clock — `now` is the host's, the same seconds the animation
//! scheduler advances on — and the ambient declaration is what keeps
//! frames coming ([`crate::anim::wants_frame`]).
//!
//! Rotation is not a property the transition engine drives, and the draw
//! list's transform carries no rotation on purpose (`crate::draw::Affine`),
//! so the angle cannot be an engine-side keyframe. Why this shape and not
//! an engine rotation is recorded in
//! `.agents/notes/implemented/architecture/2026-09-04-a-spinner-turns-by-resubmitting-its-list.md`.
//!
//! Carbon keeps spinning under `prefers-reduced-motion`: the `spin` mixin's
//! reduced-motion branch stops only the 10ms stroke-init keyframe, not the
//! rotation. So does this.
//!
//! # Carbon runs one motion, not two, and so does this
//!
//! `_animation.scss`'s `spin` mixin declares two animations and only one of
//! them is continuous:
//!
//! ```scss
//! animation-duration: 690ms;              // --rotate, infinite, linear
//! svg circle {
//!   animation-duration: 10ms;             // --init-stroke
//!   animation-name: #{$prefix}--init-stroke;
//! }
//! ```
//!
//! `--init-stroke` carries no `animation-iteration-count`, so it defaults to
//! `1`: it runs for **ten milliseconds, once, at mount**, walking
//! `stroke-dashoffset` from `loading-progress($circumference, 0)` to
//! `loading-progress($circumference, 81)`. That is the arc growing in as the
//! spinner appears. From 10 ms onward the dash offset is the static value
//! `.cds--loading__stroke` binds, and the only thing moving is the container's
//! rotation.
//!
//! So the ink fraction is a constant, not an animation, and Carbon's steady
//! state is one rigid picture turning — exactly this file's shape. The
//! suspicion that we ran one motion where Carbon ran two is **refuted**;
//! there is no second continuous animation to build. The ink fractions match
//! too: `loading-progress($c, 81) = $c - 0.81 * $c`, and with
//! `stroke-dasharray: $c $c` an offset of `0.19 * $c` leaves `0.81 * $c` of
//! ink — [`INK_LG`], and [`INK_SM`] by the same arithmetic at 48.
//!
//! # The turn is slower than Carbon's, on purpose
//!
//! [`TURN_SECONDS`] is **not** Carbon's number. See its doc.

use std::sync::Arc;

use super::tokens::{ACCENT_PRIMARY, LAYER_ACCENT, t};
use crate::draw::{ColorRef, Command, DrawList, Paint, Stroke, Width, arc_verbs};
use crate::geom::{Point, Size};
use crate::tree::{AxisConstraint, Constraints, Key, NodeKind, Props, Role, Semantics, ViewNode};

/// Carbon default / large spinner. MEASURED `$loading-size: 5.5rem`.
const SIZE_LG: f32 = 88.0;
/// Carbon `--small`. MEASURED `_loading.scss` `convert.to-rem(16px)`.
const SIZE_SM: f32 = 16.0;
/// The SVG viewBox both forms are drawn in. MEASURED `Loading.tsx`.
const VIEWBOX: f32 = 100.0;
/// Large circle radius in the viewBox. MEASURED `_vars.scss` `$radius: 44`.
const RADIUS_LG: f32 = 44.0;
/// Small circle radius in the viewBox. MEASURED `Loading.tsx` `r: small ? "42" : "44"`.
const RADIUS_SM: f32 = 42.0;
/// Large stroke in the viewBox. MEASURED `_loading.scss` `circle { stroke-width: 10 }`.
const STROKE_LG: f32 = 10.0;
/// Small stroke in the viewBox. MEASURED `_loading.scss` `--small circle { stroke-width: 16 }`.
const STROKE_SM: f32 = 16.0;
/// Fraction of the circumference that is ink. MEASURED `loading-progress($circumference, 81)`.
const INK_LG: f32 = 0.81;
/// MEASURED `--small .cds--loading__stroke`: `loading-progress($circumference, 48)`.
const INK_SM: f32 = 0.48;
/// Carbon's own turn. MEASURED `_animation.scss` `animation-duration: 690ms`
/// on the `spin` mixin, which is `linear` and `infinite`.
///
/// Kept as a named constant even though nothing draws at this rate, because
/// it is the conformance target and [`TURN_SECONDS`] is stated as a multiple
/// of it: a reader can see the size of the departure, and undoing it is
/// changing the multiplier back to 1.
const CARBON_TURN_SECONDS: f64 = 0.69;

/// DEPARTURE FROM CARBON, operator decision 2026-09-05: one full turn takes
/// **twice** Carbon's 690 ms, so 1.38 s.
///
/// This is a preference, not a correction. The 690 ms above is measured, we
/// drew at it, and the module doc records that we also match Carbon on the
/// ink fraction and on running one continuous motion rather than two — there
/// was no second animation missing and nothing else to blame for the rate.
/// The operator walked the catalog three times and asked twice for the
/// spinner to slow down; put the choice between "keep Carbon's rate" and
/// "slow it anyway", he chose to slow it.
///
/// Two, and not some other factor, because a plain doubling is the only
/// multiplier that stays legible: 1.45 turns a second reads as alarm, 0.72
/// reads as work in progress, and anyone who wants Carbon's rate back
/// changes one `2.0` rather than re-deriving a number. Row 14's inline
/// spinner turns on this same constant, which is the operator's other
/// "spinning too fast" line.
const TURN_SECONDS: f64 = CARBON_TURN_SECONDS * 2.0;

const _: () = assert!(SIZE_LG == 88.0);
const _: () = assert!(SIZE_SM == 16.0);
/// Carbon's measurement stays pinned; the departure is the multiplier alone.
const _: () = assert!(CARBON_TURN_SECONDS == 0.69);
const _: () = assert!(TURN_SECONDS == 1.38);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadingSize {
    Large,
    Small,
}

impl LoadingSize {
    fn extent(self) -> f32 {
        match self {
            Self::Large => SIZE_LG,
            Self::Small => SIZE_SM,
        }
    }

    /// Stroke centreline radius, in logical units.
    fn radius(self) -> f32 {
        let viewbox = match self {
            Self::Large => RADIUS_LG,
            Self::Small => RADIUS_SM,
        };
        viewbox / VIEWBOX * self.extent()
    }

    /// Stroke width, in logical units.
    fn stroke(self) -> f32 {
        let viewbox = match self {
            Self::Large => STROKE_LG,
            Self::Small => STROKE_SM,
        };
        viewbox / VIEWBOX * self.extent()
    }

    fn ink(self) -> f32 {
        match self {
            Self::Large => INK_LG,
            Self::Small => INK_SM,
        }
    }

    /// Only the small form draws the track behind its arc.
    fn has_track(self) -> bool {
        self == Self::Small
    }
}

/// Where in its turn the spinner is at `now` seconds: `[0, 1)`.
///
/// Public so a page holding the host's clock can read the same phase the
/// component drew, and so a test can drive two frames apart by a known
/// fraction of a turn.
#[must_use]
pub fn spinner_phase(now: f64) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let phase = (now / TURN_SECONDS).rem_euclid(1.0) as f32;
    phase
}

/// Rebuild every ambient loading-spinner canvas under `root` at host clock
/// `now`.
///
/// A contributed tree is an owned snapshot: without this walk, an ambient
/// canvas keeps asking for frames ([`crate::anim::wants_frame`]) but paints
/// the phase it was published with. The host is the only clock that may
/// drive these canvases (`contracts/draw-list.md` §8); Lua must not tick a
/// spinner by republishing.
///
/// Matches canvases under a node whose [`Semantics::value`] is `"loading"`
/// (the [`loading`] / [`loading_sm`] / [`super::inline_loading`] mark) and
/// sized to Carbon's large or small spinner extent.
pub fn advance_ambient_spinners(root: &mut ViewNode, now: f64) {
    advance_walk(root, now, false);
}

fn advance_walk(node: &mut ViewNode, now: f64, under_loading: bool) {
    let loading_here = under_loading || node.semantics.value.as_deref() == Some("loading");
    if node.kind == NodeKind::Canvas
        && node.ambient
        && loading_here
        && let Some(size) = spinner_size_of(node)
    {
        node.props.canvas = Some(Arc::new(spinner_list(size, now)));
    }
    for child in &mut node.children {
        advance_walk(Arc::make_mut(child), now, loading_here);
    }
}

fn spinner_size_of(node: &ViewNode) -> Option<LoadingSize> {
    let extent = node
        .constraints
        .horizontal
        .max
        .or(node.constraints.horizontal.min)?;
    if (extent - SIZE_LG).abs() < 0.5 {
        Some(LoadingSize::Large)
    } else if (extent - SIZE_SM).abs() < 0.5 {
        Some(LoadingSize::Small)
    } else {
        None
    }
}

fn spinner_list(size: LoadingSize, now: f64) -> DrawList {
    let extent = size.extent();
    let half = extent / 2.0;
    let center = Point::new(half, half);
    let radius = size.radius();
    let stroke = size.stroke();
    let mut commands = Vec::with_capacity(2);
    if size.has_track() {
        commands.push(Command::Ellipse {
            center,
            radii: Size::new(radius, radius),
            paint: Paint::stroked(Stroke {
                width: Width::Logical(stroke),
                color: ColorRef::Token(t(LAYER_ACCENT).as_str().to_owned()),
            }),
        });
    }
    let start = spinner_phase(now) * std::f32::consts::TAU;
    commands.push(Command::Path {
        verbs: arc_verbs(center, radius, start, size.ink() * std::f32::consts::TAU),
        closed: false,
        paint: Paint::stroked(Stroke {
            width: Width::Logical(stroke),
            color: ColorRef::Token(t(ACCENT_PRIMARY).as_str().to_owned()),
        }),
    });
    DrawList::new(commands).unwrap_or_else(|err| panic!("spinner draw list refused: {err}"))
}

/// Circular in-progress indicator, Carbon large (88), at the host clock
/// `now` (seconds).
///
/// `label` is required: [`Role::Progress`] refuses an empty label. It is
/// the accessible name and is not drawn, as Carbon's `description` is not.
pub fn loading(key: impl Into<Key>, label: impl Into<String>, now: f64) -> ViewNode {
    loading_sized(key, label, LoadingSize::Large, now)
}

/// Circular in-progress indicator, Carbon small (16), at the host clock
/// `now` (seconds).
pub fn loading_sm(key: impl Into<Key>, label: impl Into<String>, now: f64) -> ViewNode {
    loading_sized(key, label, LoadingSize::Small, now)
}

fn loading_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    size: LoadingSize,
    now: f64,
) -> ViewNode {
    let label = label.into();
    let mut node = spinner(key, size, now);
    node.semantics = Semantics {
        role: Some(Role::Progress),
        label: Some(label),
        value: Some("loading".into()),
        ..Semantics::default()
    };
    node
}

/// The small spinner alone, for a component that composes it under its
/// own semantics: Inline loading's active state.
pub(crate) fn spinner_small(key: impl Into<Key>, now: f64) -> ViewNode {
    spinner(key, LoadingSize::Small, now)
}

/// The arc at the phase `now` names, on an ambient canvas.
fn spinner(key: impl Into<Key>, size: LoadingSize, now: f64) -> ViewNode {
    let extent = size.extent();
    // The dash starts where an SVG circle's stroke starts, three o'clock,
    // and the whole picture turns clockwise with the phase.
    let list = spinner_list(size, now);
    ViewNode::new(NodeKind::Canvas, key)
        .with_props(Props {
            canvas: Some(Arc::new(list)),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(extent),
                max: Some(extent),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(extent),
                max: Some(extent),
                priority: 0,
            },
        })
        .with_ambient(true)
}

#[cfg(test)]
mod tests {
    use super::{
        CARBON_TURN_SECONDS, INK_LG, INK_SM, SIZE_LG, SIZE_SM, TURN_SECONDS,
        advance_ambient_spinners, loading, loading_sm, spinner_phase,
    };
    use crate::component::tokens::{ACCENT_PRIMARY, LAYER_ACCENT};
    use crate::draw::{ColorRef, Command, DrawList, PathVerb};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Point, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ThemeMode, standard_vocabulary};
    use crate::tree::{NodeKind, Props, Registry, Role, ViewNode};

    fn list(node: &ViewNode) -> &DrawList {
        node.props.canvas.as_ref().expect("the spinner is a canvas")
    }

    /// The arc command, and the angle its first point sits at.
    fn arc(node: &ViewNode) -> (&Command, f32) {
        let extent = node.constraints.horizontal.min.unwrap();
        let half = extent / 2.0;
        let command = list(node)
            .commands()
            .iter()
            .find(|c| matches!(c, Command::Path { .. }))
            .expect("the spinner draws an arc path");
        let Command::Path { verbs, closed, .. } = command else {
            unreachable!()
        };
        assert!(!closed, "an arc with a gap is an open path");
        let PathVerb::MoveTo(start) = verbs[0] else {
            panic!("an arc starts with MoveTo: {verbs:?}")
        };
        (command, (start.y - half).atan2(start.x - half))
    }

    /// Row 17: one stroked arc, not two closed rings. The large form has no
    /// track, exactly as Carbon's `Loading.tsx` renders none, and every
    /// circle-ish thing on it is the accent arc.
    #[test]
    fn loading_is_progress_drawing_one_open_accent_arc_and_no_track() {
        let node = loading("wait", "Loading data", 0.0);
        assert_eq!(node.semantics.role, Some(Role::Progress));
        assert_eq!(node.semantics.label.as_deref(), Some("Loading data"));
        assert_eq!(node.semantics.value.as_deref(), Some("loading"));
        assert!(!node.is_interactive());
        assert_eq!(node.kind, NodeKind::Canvas);
        assert_eq!(node.constraints.horizontal.min, Some(SIZE_LG));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_LG));
        assert!(
            node.children.is_empty(),
            "Carbon's description is a <title>, not a caption; nothing is drawn under the ring"
        );
        let commands = list(&node).commands();
        assert_eq!(commands.len(), 1, "{commands:?}");
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, Command::Ellipse { .. })),
            "the large form has no track ring"
        );
        let (command, _) = arc(&node);
        let Command::Path { verbs, paint, .. } = command else {
            unreachable!()
        };
        assert_eq!(verbs.len(), 5, "81% of a turn is four quarter-turn cubics");
        assert_eq!(
            paint.stroke.as_ref().map(|s| &s.color),
            Some(&ColorRef::Token(ACCENT_PRIMARY.to_owned()))
        );
        assert!(paint.fill.is_none());
        // Carbon's numbers, scaled out of the 100 viewBox: r = 44 → 38.72,
        // stroke 10 → 8.8, so the outer edge sits at 43.12, inside 44.
        let PathVerb::MoveTo(start) = verbs[0] else {
            unreachable!()
        };
        assert!((start.x - (44.0 + 38.72)).abs() < 1e-3, "{start:?}");
        assert!((start.y - 44.0).abs() < 1e-3, "{start:?}");
        match paint.stroke.as_ref().unwrap().width {
            crate::draw::Width::Logical(w) => assert!((w - 8.8).abs() < 1e-3, "{w}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(INK_LG, 0.81);
    }

    /// The small form is the one with a track: a full `layer-accent` circle
    /// under a 48% accent arc, both 2.56 wide, both on r = 6.72 so the
    /// outer edge lands exactly on the 16 box.
    #[test]
    fn loading_sm_is_16_with_a_recessed_track_under_a_half_turn_arc() {
        let node = loading_sm("wait", "Loading", 0.0);
        assert_eq!(node.semantics.role, Some(Role::Progress));
        assert_eq!(node.constraints.horizontal.min, Some(SIZE_SM));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_SM));
        let commands = list(&node).commands();
        assert_eq!(commands.len(), 2, "{commands:?}");
        let Command::Ellipse {
            center,
            radii,
            paint,
        } = &commands[0]
        else {
            panic!("the track is drawn first, under the arc: {commands:?}")
        };
        assert_eq!(*center, Point::new(8.0, 8.0));
        assert!((radii.w - 6.72).abs() < 1e-3 && (radii.h - 6.72).abs() < 1e-3);
        assert_eq!(
            paint.stroke.as_ref().map(|s| &s.color),
            Some(&ColorRef::Token(LAYER_ACCENT.to_owned())),
            "the track recedes; border.subtle is a hairline tone and read as a second ring"
        );
        let (command, _) = arc(&node);
        let Command::Path { verbs, .. } = command else {
            unreachable!()
        };
        assert_eq!(verbs.len(), 3, "48% of a turn is two cubics");
        assert_eq!(INK_SM, 0.48);
    }

    /// The whole defect: a spinner that does not turn. Two clocks a tenth
    /// of a turn apart draw the arc from two different angles, a full turn
    /// apart draw it from the same one, and the canvas says it is ambient
    /// so the host keeps asking for frames.
    #[test]
    fn the_arc_turns_with_the_clock_and_repeats_every_turn() {
        let at = |now: f64| arc(&loading("wait", "Loading", now)).1;
        assert!(spinner_phase(0.0).abs() < 1e-6);
        assert!((spinner_phase(TURN_SECONDS / 4.0) - 0.25).abs() < 1e-6);
        assert!((spinner_phase(TURN_SECONDS * 3.0) - 0.0).abs() < 1e-5);
        let a = at(0.0);
        let b = at(TURN_SECONDS / 10.0);
        let c = at(TURN_SECONDS);
        assert!(
            (b - a - std::f32::consts::TAU / 10.0).abs() < 1e-4,
            "{a} -> {b}"
        );
        assert!(
            (c - a).abs() < 1e-4,
            "one full turn later the picture repeats: {a} vs {c}"
        );
        // Clockwise on screen: a later clock puts the start lower on the
        // right side (y grows downward).
        assert!(b > a);
        for node in [loading("w", "l", 0.3), loading_sm("w", "l", 0.3)] {
            assert!(
                node.ambient,
                "a canvas rebuilt every frame must declare ambient, or idle scheduling stops it"
            );
            assert!(node.transition.is_none());
        }
        assert_ne!(
            list(&loading("w", "l", 0.0)),
            list(&loading("w", "l", 0.1)),
            "two phases are two different lists, and so two digests"
        );
    }

    /// A frozen contributed spinner must turn when the host advances it —
    /// without replacing the node or asking Lua to republish.
    #[test]
    fn advance_ambient_spinners_rewrites_a_frozen_loading_canvas() {
        let mut tree = loading("load-lg", "Working", 0.0);
        let before = list(&tree).clone();
        advance_ambient_spinners(&mut tree, TURN_SECONDS / 4.0);
        let after = list(&tree);
        assert_ne!(before, *after, "host clock must rebuild the ambient canvas");
        assert_eq!(
            *after,
            *list(&loading("load-lg", "Working", TURN_SECONDS / 4.0)),
            "rewritten phase must match a freshly authored spinner"
        );
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = Registry::with_vocabulary(standard_vocabulary());
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    /// Check C/D: both spinners place with a real square rect, nothing
    /// outside the parent, and the frame counts the ambient canvas.
    #[test]
    fn frame_geometry_is_a_real_square_and_the_frame_counts_it_ambient() {
        for (node, extent) in [
            (loading("wait", "Loading data", 0.0), SIZE_LG),
            (loading_sm("wait", "Loading", 0.0), SIZE_SM),
        ] {
            let frame = petrify_lone(node);
            let spinner = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/wait"))
                .expect("the spinner is placed");
            assert_eq!(spinner.rect.w, extent);
            assert_eq!(spinner.rect.h, extent);
            assert!(!spinner.paint.overflowed);
            assert!(
                spinner.semantics.ambient,
                "the placement carries the declaration"
            );
            for p in &frame.placements {
                if let Some(parent_idx) = p.parent {
                    let parent = &frame.placements[parent_idx];
                    assert!(
                        p.rect.x >= parent.rect.x - 0.01
                            && p.rect.y >= parent.rect.y - 0.01
                            && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                            && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01,
                        "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id,
                        p.rect,
                        parent.id,
                        parent.rect
                    );
                }
            }
        }
    }

    /// The operator's rate, pinned against the wall clock rather than
    /// against itself.
    ///
    /// Every other motion assertion in this file is relative — "a tenth of a
    /// turn later" — so all of them stay green at any `TURN_SECONDS`,
    /// including the 690 ms this row departed from. This one names real
    /// seconds: at Carbon's 690 ms the spinner is exactly **half** way round,
    /// and a full second is a little over two thirds. Put 0.69 back and both
    /// go red.
    #[test]
    fn one_turn_takes_twice_carbon_s_690ms_of_real_time() {
        assert!(
            (spinner_phase(CARBON_TURN_SECONDS) - 0.5).abs() < 1e-6,
            "Carbon's own turn must leave this spinner half way round, got {}",
            spinner_phase(CARBON_TURN_SECONDS)
        );
        assert!(
            (spinner_phase(1.0) - (1.0 / 1.38)).abs() < 1e-6,
            "one wall-clock second is {} of a turn",
            spinner_phase(1.0)
        );
        // Turns per second, the number the operator was reacting to.
        let rate = 1.0 / TURN_SECONDS;
        assert!(
            (0.7..0.75).contains(&rate),
            "the spinner turns {rate} times a second"
        );
        assert_eq!(TURN_SECONDS, 1.38);
        assert_eq!(CARBON_TURN_SECONDS, 0.69);
    }

    /// Check F is vacuous here: slice-c states Loading is "non-interactive
    /// and not focusable" — confirmed rather than assumed.
    #[test]
    fn loading_declares_no_interaction() {
        assert!(!loading("wait", "Loading data", 0.0).is_interactive());
        assert!(!loading_sm("wait", "Loading", 0.0).is_interactive());
    }
}
