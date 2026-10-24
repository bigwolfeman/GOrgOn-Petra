//! One shared skeleton shape: the placeholder a component shows while content
//! that has not arrived is still loading.
//!
//! `specs/008-petra-carbon-variants/contracts/skeleton.md` binds this file:
//! one parameterized shape (FR-012) — [`skeleton`] — sized from a [`SizeStep`]
//! and the [`SkeletonAnatomy`] it stands in for (FR-016), pulsing with
//! Carbon's own `--skeleton` shimmer and declaring that pulse ambient through
//! [`ViewNode::with_ambient`] (FR-017). There is no per-component drawing
//! routine anywhere in the library: a date picker's skeleton is three calls
//! into this shape, a button's is one.
//!
//! Carbon's measurements, kept where they came from rather than paraphrased:
//! `packages/styles/scss/utilities/_skeleton.scss` (`@mixin skeleton` — the
//! ground plus an animated `::before` fill layer), `utilities/_keyframes.scss`
//! (`@keyframes --skeleton` — the curve [`pulse_fill`] transcribes), and the
//! per-component block sizes in `packages/styles/scss/components/` (the table
//! on [`SkeletonAnatomy`]). The two tones bind `surface.layer-two` (ground)
//! and `surface.layer-three` (fill) — M-Carbon's one-step layer pair, which is
//! Carbon's `$skeleton-background`/`$skeleton-element` relationship under
//! FR-031's ban on new token names.

use std::sync::Arc;

use crate::anim::curve::CubicBezier;
use crate::component::tokens::{SURFACE_LAYER_THREE, SURFACE_LAYER_TWO, t};
use crate::draw::{ColorRef, Command, Corners, DrawList, Paint, PathVerb};
use crate::geom::{Point, Rect, Size};
use crate::tree::{AxisConstraint, Constraints, Key, NodeKind, Props, Semantics, ViewNode};

/// One full pulse cycle, seconds: Carbon's `3000ms` animation.
pub(crate) const PULSE_SECONDS: f64 = 3.0;

/// Carbon's published size-step ramp, in logical units.
///
/// The heights are Carbon's block heights (`--2xs` … `--2xl` in the component
/// size tables), one ramp for every stepped anatomy. The three Petra already
/// ships as its own control sizes are pinned below so the two tables cannot
/// drift apart silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeStep {
    /// `xs` — 24.
    Xs,
    /// `sm` — 32.
    Sm,
    /// `md` — 40.
    Md,
    /// `lg` — 48.
    Lg,
    /// `xl` — 64.
    Xl,
    /// `2xl` — 80.
    Xxl,
}

impl SizeStep {
    /// The block height this step carries, logical units.
    #[must_use]
    pub fn height(self) -> f32 {
        match self {
            Self::Xs => 24.0,
            Self::Sm => 32.0,
            Self::Md => 40.0,
            Self::Lg => 48.0,
            Self::Xl => 64.0,
            Self::Xxl => 80.0,
        }
    }

    /// The wire name (`"xs"` … `"2xl"`), for a registry parameter.
    #[must_use]
    pub fn wire(self) -> &'static str {
        match self {
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
            Self::Xl => "xl",
            Self::Xxl => "2xl",
        }
    }

    /// The step a wire name names, or `None` — never a silent default.
    #[must_use]
    pub fn from_wire(name: &str) -> Option<Self> {
        match name {
            "xs" => Some(Self::Xs),
            "sm" => Some(Self::Sm),
            "md" => Some(Self::Md),
            "lg" => Some(Self::Lg),
            "xl" => Some(Self::Xl),
            "2xl" => Some(Self::Xxl),
            _ => None,
        }
    }
}

const _: () = assert!(crate::component::tokens::SIZE_XS == 24.0);
const _: () = assert!(crate::component::tokens::SIZE_MD == 40.0);
const _: () = assert!(crate::component::tokens::SIZE_LG == 48.0);

/// The block shapes Carbon publishes skeleton dimensions for.
///
/// | Arm | Block | Carbon source |
/// |---|---|---|
/// | `Text` | 16 high | one-line text bars; data-table cells 16×64, header description 16×160 |
/// | `Heading` | 24 high | data-table header title 24×120 |
/// | `Label` | 18 high | radio-button label 18×100 |
/// | `Caption` | 14 high | field labels 14×75, progress-indicator label 14×40, tab nav-link 14×160 |
/// | `Micro` | 12 high | slider range label 12×20 |
/// | `Sliver` | 8 high | toggle's rectangle (24×8) |
/// | `Icon` | 16×16, `inline` ignored | accordion chevron slot, date-picker input icon |
/// | `Knob` | 18×18 circle, `inline` ignored | toggle knob |
/// | `Control` | step height | buttons 150 wide at sm/md/lg; field blocks 40 high (32 sm, 48 lg) |
/// | `Tag` | 18/24/32 at `Sm`/`Md`/`Lg` | tag 60 wide, 18/24/32 by size |
///
/// A (anatomy, step) pair Carbon does not publish — a `Tag` at `Xxl`, say —
/// is a misconfiguration and fails loudly ([`SkeletonAnatomy::block`]
/// panics naming the pair). A caller reaches only for the steps its component
/// actually has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkeletonAnatomy {
    /// A one-line text bar: 16 high.
    Text,
    /// A heading bar: 24 high.
    Heading,
    /// A label bar: 18 high.
    Label,
    /// A caption bar: 14 high.
    Caption,
    /// A micro bar: 12 high.
    Micro,
    /// A sliver bar: 8 high.
    Sliver,
    /// A square icon slot: 16×16.
    Icon,
    /// A circular knob: 18×18.
    Knob,
    /// A control block: the step's height.
    Control,
    /// A tag block: 18/24/32 at sm/md/lg.
    Tag,
}

impl SkeletonAnatomy {
    /// The block this anatomy stands in for: `(inline, block)` logical units.
    ///
    /// # Panics
    /// On a (anatomy, step) pair Carbon does not publish, and on a non-positive
    /// `inline` for an arm that takes one. Both are authoring mistakes; the
    /// standing order is that misconfiguration fails loud rather than drawing
    /// a skeleton that lies about its size.
    #[must_use]
    pub fn block(self, step: SizeStep, inline: f32) -> (f32, f32) {
        match self {
            Self::Text => (inline_extent(self, inline), 16.0),
            Self::Heading => (inline_extent(self, inline), 24.0),
            Self::Label => (inline_extent(self, inline), 18.0),
            Self::Caption => (inline_extent(self, inline), 14.0),
            Self::Micro => (inline_extent(self, inline), 12.0),
            Self::Sliver => (inline_extent(self, inline), 8.0),
            Self::Icon => (16.0, 16.0),
            Self::Knob => (18.0, 18.0),
            Self::Control => (inline_extent(self, inline), step.height()),
            Self::Tag => {
                let h = match step {
                    SizeStep::Sm => 18.0,
                    SizeStep::Md => 24.0,
                    SizeStep::Lg => 32.0,
                    SizeStep::Xs | SizeStep::Xl | SizeStep::Xxl => panic!(
                        "Carbon publishes no tag at the {} step; a tag skeleton runs sm/md/lg",
                        step.wire()
                    ),
                };
                (inline_extent(self, inline), h)
            }
        }
    }

    /// Whether this block is the circular one (Carbon's `circular-skeleton`
    /// mixin: the toggle knob), which pulses under a 200 %-wide fill clipped
    /// to the circle rather than a 100 %-wide one.
    #[must_use]
    pub fn is_circular(self) -> bool {
        self == Self::Knob
    }

    /// The wire name, for a registry parameter and the semantic tree.
    #[must_use]
    pub fn wire(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Heading => "heading",
            Self::Label => "label",
            Self::Caption => "caption",
            Self::Micro => "micro",
            Self::Sliver => "sliver",
            Self::Icon => "icon",
            Self::Knob => "knob",
            Self::Control => "control",
            Self::Tag => "tag",
        }
    }

    /// The anatomy a wire name names, or `None` — never a silent default.
    #[must_use]
    pub fn from_wire(name: &str) -> Option<Self> {
        match name {
            "text" => Some(Self::Text),
            "heading" => Some(Self::Heading),
            "label" => Some(Self::Label),
            "caption" => Some(Self::Caption),
            "micro" => Some(Self::Micro),
            "sliver" => Some(Self::Sliver),
            "icon" => Some(Self::Icon),
            "knob" => Some(Self::Knob),
            "control" => Some(Self::Control),
            "tag" => Some(Self::Tag),
            _ => None,
        }
    }
}

fn inline_extent(anatomy: SkeletonAnatomy, inline: f32) -> f32 {
    assert!(
        inline > 0.0,
        "a {} skeleton with inline extent {} is a misconfiguration: the block has no width",
        anatomy.wire(),
        inline
    );
    inline
}

/// Where in its pulse cycle the skeleton is at `now` seconds: `[0, 1)`.
///
/// Public so a page holding the host's clock can read the same phase the shape
/// drew, exactly as [`super::loading::spinner_phase`] serves the spinner.
#[must_use]
pub fn skeleton_pulse_phase(now: f64) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let phase = (now / PULSE_SECONDS).rem_euclid(1.0) as f32;
    phase
}

/// One shared skeleton block at phase `now`: ground plus its pulsing fill,
/// on an ambient canvas.
///
/// The node declares no interaction and no role (FR-014); it declares
/// `Semantics.skeleton` (FR-015) and ambient (FR-017) and nothing else. The
/// fill is Carbon's `::before` layer — the `--skeleton` keyframe transcribed
/// in [`pulse_fill`] — and it moves because the node asked to move, never
/// because something repainted it.
#[must_use]
pub fn skeleton(
    key: impl Into<Key>,
    anatomy: SkeletonAnatomy,
    step: SizeStep,
    inline: f32,
    now: f64,
) -> ViewNode {
    let (w, h) = anatomy.block(step, inline);
    let fill_w = if anatomy.is_circular() { w * 2.0 } else { w };
    let list = block_list(
        anatomy,
        w,
        h,
        pulse_fill(w, fill_w, skeleton_pulse_phase(now)),
    );
    ViewNode::new(NodeKind::Canvas, key)
        .with_props(Props {
            canvas: Some(Arc::new(list)),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(w),
                max: Some(w),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(h),
                max: Some(h),
                priority: 0,
            },
        })
        .with_ambient(true)
        .with_semantics(Semantics {
            skeleton: true,
            ..Semantics::default()
        })
}

/// Rebuild every ambient skeleton canvas under `root` at host clock `now`,
/// or rest it under `reduced`.
///
/// The mirror of [`super::loading::advance_ambient_spinners`]: a contributed
/// tree is an owned snapshot, so without this walk an ambient skeleton paints
/// the phase it was published with. The host is the only clock that may drive
/// these canvases (`contracts/draw-list.md` §8); Lua must not tick a skeleton
/// by republishing.
///
/// Under reduced motion the skeleton draws Carbon's own `animation: none`
/// resting state — the fill at rest, full and opaque — and stops asking for
/// frames, which is what the node un-declaring `ambient` says. A reduced
/// skeleton is what Carbon serves, not a static divergence from it.
pub fn advance_ambient_skeletons(root: &mut ViewNode, now: f64, reduced: bool) {
    let phase = skeleton_pulse_phase(now);
    advance_walk(root, phase, reduced);
}

fn advance_walk(node: &mut ViewNode, phase: f32, reduced: bool) {
    if node.kind == NodeKind::Canvas
        && node.ambient
        && node.semantics.skeleton
        && let Some((shape, w, h)) = block_of(node)
    {
        let fill_w = if shape.is_circular() { w * 2.0 } else { w };
        let fill = if reduced {
            rest_fill(w)
        } else {
            pulse_fill(w, fill_w, phase)
        };
        node.props.canvas = Some(Arc::new(block_list(shape, w, h, fill)));
        if reduced {
            node.ambient = false;
        }
    }
    for child in &mut node.children {
        advance_walk(Arc::make_mut(child), phase, reduced);
    }
}

/// The block a skeleton canvas draws, re-derived from the list it published:
/// the ground command says the shape (a rect bar or the circular knob), the
/// pinned constraints say the extent. Nothing is smuggled into the semantic
/// tree to make a repaint possible.
fn block_of(node: &ViewNode) -> Option<(SkeletonAnatomy, f32, f32)> {
    let list = node.props.canvas.as_ref()?;
    let w = node
        .constraints
        .horizontal
        .max
        .or(node.constraints.horizontal.min)?;
    let h = node
        .constraints
        .vertical
        .max
        .or(node.constraints.vertical.min)?;
    let shape = match list.commands().first()? {
        Command::Ellipse { .. } => SkeletonAnatomy::Knob,
        Command::Rect { .. } => SkeletonAnatomy::Text,
        _ => return None,
    };
    Some((shape, w, h))
}

/// The fill layer at `phase`: where its span sits inside the block and how
/// opaque it is.
///
/// This is Carbon's `@keyframes --skeleton` (`utilities/_keyframes.scss`),
/// transcribed segment by segment: `scaleX` 0→1 pivot left (0–20 %), hold
/// (20–28 %), 1→0 pivot right (28–51 %), rest (51–58 %), 0→1 pivot right
/// (58–82 %), hold (82–83 %), 1→0 pivot left (83–96 %), rest (96–100 %),
/// each segment eased `ease-in-out`. Opacity is a stop-holding property as
/// CSS reads one: 0.3→1 across 0–20 %, 1→0.3 across 20–100 %.
///
/// Carbon animates a `::before` **box** and clips it to the block; the span
/// here is that box intersected with the block. For the circular knob the box
/// is 200 % wide (`@mixin circular-skeleton`), so the knob's sweep saturates
/// at half scale and its box visibly leaves the circle during the second half
/// of a collapse — both Carbon's own behaviour, kept rather than smoothed.
pub(crate) fn pulse_fill(block_w: f32, fill_w: f32, phase: f32) -> Fill {
    let (scale, pivot_left) = pulse_scale(phase);
    let width = fill_w * scale;
    let x0 = if pivot_left { 0.0 } else { fill_w - width };
    Fill {
        x0: x0.clamp(0.0, block_w),
        x1: (x0 + width).clamp(0.0, block_w),
        opacity: pulse_opacity(phase),
    }
}

/// Carbon's reduced-motion state (`animation: none`): the fill at rest,
/// full-size and opaque, over the ground.
pub(crate) fn rest_fill(block_w: f32) -> Fill {
    Fill {
        x0: 0.0,
        x1: block_w,
        opacity: 1.0,
    }
}

/// The fill layer's span and opacity.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fill {
    x0: f32,
    x1: f32,
    opacity: f32,
}

fn pulse_scale(phase: f32) -> (f32, bool) {
    if phase < 0.20 {
        (ease(phase / 0.20), true)
    } else if phase < 0.28 {
        (1.0, true)
    } else if phase < 0.51 {
        (1.0 - ease((phase - 0.28) / 0.23), false)
    } else if phase < 0.58 {
        (0.0, false)
    } else if phase < 0.82 {
        (ease((phase - 0.58) / 0.24), false)
    } else if phase < 0.83 {
        (1.0, false)
    } else if phase < 0.96 {
        (1.0 - ease((phase - 0.83) / 0.13), true)
    } else {
        (0.0, true)
    }
}

fn pulse_opacity(phase: f32) -> f32 {
    if phase < 0.20 {
        0.3 + 0.7 * ease(phase / 0.20)
    } else {
        1.0 - 0.7 * ease((phase - 0.20) / 0.80)
    }
}

/// CSS `ease-in-out` — `cubic-bezier(0.42, 0, 0.58, 1)` — on `[0, 1]`.
fn ease(t: f32) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let eased = CubicBezier::EASE_IN_OUT.eval(f64::from(t.clamp(0.0, 1.0)));
    #[allow(clippy::cast_possible_truncation)]
    let eased = eased as f32;
    eased
}

/// The smallest fill span worth drawing, logical units.
///
/// The sweep's rests land the fill box on the block edge with a span of a few
/// millionths — a zero-area sliver whose chords coincide, which the draw
/// list's convexity test rightly refuses as a polygon that turns back on
/// itself. Below this threshold the band carries no visible information (it
/// is far under one device pixel at every scale the host runs), so the block
/// draws its ground alone — exactly what the empty fill at the rests does.
const MIN_VISIBLE_FILL: f32 = 0.01;

/// Ground plus fill at `fill`, as one block's draw list.
fn block_list(anatomy: SkeletonAnatomy, w: f32, h: f32, fill: Fill) -> DrawList {
    let mut commands = vec![ground(anatomy, w, h)];
    let span = fill.x1 - fill.x0;
    if span > MIN_VISIBLE_FILL && fill.opacity > 0.0 {
        commands.push(Command::Push {
            transform: None,
            clip: None,
            opacity: Some(fill.opacity),
        });
        commands.push(fill_op(anatomy, w, h, fill));
        commands.push(Command::Pop);
    }
    DrawList::new(commands).unwrap_or_else(|err| panic!("skeleton draw list refused: {err}"))
}

fn ground(anatomy: SkeletonAnatomy, w: f32, h: f32) -> Command {
    if anatomy.is_circular() {
        Command::Ellipse {
            center: Point::new(w / 2.0, h / 2.0),
            radii: Size::new(w / 2.0, h / 2.0),
            paint: Paint::filled(ColorRef::Token(t(SURFACE_LAYER_TWO).as_str().to_owned())),
        }
    } else {
        Command::Rect {
            rect: Rect::new(0.0, 0.0, w, h),
            radius: Corners::all(0.0),
            snap: false,
            paint: Paint::filled(ColorRef::Token(t(SURFACE_LAYER_TWO).as_str().to_owned())),
        }
    }
}

fn fill_op(anatomy: SkeletonAnatomy, w: f32, h: f32, fill: Fill) -> Command {
    let paint = Paint::filled(ColorRef::Token(t(SURFACE_LAYER_THREE).as_str().to_owned()));
    if anatomy.is_circular() {
        let span = fill.x1 - fill.x0;
        if (span - w).abs() < f32::EPSILON {
            // Full circle at rest or at scale saturation: the ellipse itself.
            Command::Ellipse {
                center: Point::new(w / 2.0, h / 2.0),
                radii: Size::new(w / 2.0, h / 2.0),
                paint,
            }
        } else {
            Command::Path {
                verbs: circle_band(Point::new(w / 2.0, h / 2.0), w / 2.0, fill.x0, fill.x1),
                closed: true,
                paint,
            }
        }
    } else {
        Command::Rect {
            rect: Rect::new(fill.x0, 0.0, fill.x1 - fill.x0, h),
            radius: Corners::all(0.0),
            snap: false,
            paint,
        }
    }
}

/// The vertical band of a circle between `x0` and `x1`, as one closed convex
/// path: two chords and two arcs, each arc split at the crown so no cubic
/// spans more than a quarter circle.
///
/// This is what Carbon's circular fill looks like — a 200 %-wide `::before`
/// clipped by `border-radius: 50%` — and the draw list's own convexity test
/// is the check that the construction stayed convex.
fn circle_band(center: Point, radius: f32, x0: f32, x1: f32) -> Vec<PathVerb> {
    let rise = |x: f32| {
        let dx = x - center.x;
        (radius * radius - dx * dx).max(0.0).sqrt()
    };
    let angle = |x: f32| ((x - center.x) / radius).clamp(-1.0, 1.0).asin();
    let at = |theta: f32| {
        Point::new(
            center.x + radius * theta.sin(),
            center.y - radius * theta.cos(),
        )
    };
    let pi = std::f32::consts::PI;
    let a0 = angle(x0);
    let a1 = angle(x1);
    // Angles are `at`'s convention: `0` is the top crown, `pi` the bottom
    // one, and a block `x` is `asin((x - cx) / r)` on the top arc and
    // `pi - asin((x - cx) / r)` on the bottom one. The band is walked over
    // the top crown and back under the bottom one; a crown is a split point
    // only when the arc actually crosses it, because a cubic that reverses
    // over a crown it never reaches is a polygon that turns back on itself —
    // which the draw list's convexity test refuses, and did, at every phase
    // before the sweep saturated.
    let mut verbs = vec![PathVerb::MoveTo(Point::new(x0, center.y - rise(x0)))];
    push_arc_split(&mut verbs, &at, a0, a1, 0.0);
    verbs.push(PathVerb::LineTo(Point::new(x1, center.y + rise(x1))));
    push_arc_split(&mut verbs, &at, pi - a1, pi - a0, pi);
    verbs.push(PathVerb::Close);
    verbs
}

/// One arc `from` → `to`, split at `crown` only when the range crosses it.
///
/// Both halves then stay within a quarter circle of their crown, and a range
/// that never reaches the crown is one cubic — never two that reverse over
/// it. The halves pass [`push_arc`] a reference so one closure serves three
/// calls.
fn push_arc_split(
    verbs: &mut Vec<PathVerb>,
    at: &impl Fn(f32) -> Point,
    from: f32,
    to: f32,
    crown: f32,
) {
    if (from < crown && crown < to) || (to < crown && crown < from) {
        push_arc(verbs, at, from, crown);
        push_arc(verbs, at, crown, to);
    } else {
        push_arc(verbs, at, from, to);
    }
}

/// One cubic approximating the circle arc from `from` to `to` (radians on
/// [`circle_band`]'s parameterization), skipped when the span is empty.
fn push_arc(verbs: &mut Vec<PathVerb>, at: impl Fn(f32) -> Point, from: f32, to: f32) {
    let span = to - from;
    if span <= f32::EPSILON {
        return;
    }
    let handle = (4.0 / 3.0) * (span / 4.0).tan();
    let tangent = |theta: f32| {
        let p = at(theta);
        (p, Point::new(theta.cos(), theta.sin()))
    };
    let (start, t0) = tangent(from);
    let (end, t1) = tangent(to);
    verbs.push(PathVerb::CubicTo {
        c1: Point::new(start.x + handle * t0.x, start.y + handle * t0.y),
        c2: Point::new(end.x - handle * t1.x, end.y - handle * t1.y),
        to: end,
    });
}

#[cfg(test)]
mod tests {
    use super::{
        Fill, PULSE_SECONDS, SizeStep, SkeletonAnatomy, advance_ambient_skeletons, block_list,
        pulse_fill, pulse_opacity, pulse_scale, rest_fill, skeleton, skeleton_pulse_phase,
    };
    use crate::component::tests::petrify_lone;
    use crate::draw::Command;
    use crate::focus::FocusTree;
    use crate::semantic::{StateFlag, TreeQuery, project};
    use crate::tree::{Interaction, NodeKind, ViewNode};

    use std::collections::BTreeMap;

    /// Carbon's published block sizes, asserted rather than trusted.
    #[test]
    fn the_shape_is_sized_from_the_step_and_the_anatomy() {
        for (anatomy, step, inline, want) in [
            (SkeletonAnatomy::Text, SizeStep::Md, 64.0, (64.0, 16.0)),
            (SkeletonAnatomy::Text, SizeStep::Md, 160.0, (160.0, 16.0)),
            (SkeletonAnatomy::Heading, SizeStep::Md, 120.0, (120.0, 24.0)),
            (SkeletonAnatomy::Label, SizeStep::Md, 100.0, (100.0, 18.0)),
            (SkeletonAnatomy::Caption, SizeStep::Md, 75.0, (75.0, 14.0)),
            (SkeletonAnatomy::Caption, SizeStep::Md, 160.0, (160.0, 14.0)),
            (SkeletonAnatomy::Micro, SizeStep::Md, 20.0, (20.0, 12.0)),
            (SkeletonAnatomy::Sliver, SizeStep::Md, 24.0, (24.0, 8.0)),
            (SkeletonAnatomy::Icon, SizeStep::Md, 999.0, (16.0, 16.0)),
            (SkeletonAnatomy::Knob, SizeStep::Md, 999.0, (18.0, 18.0)),
            (SkeletonAnatomy::Control, SizeStep::Xs, 150.0, (150.0, 24.0)),
            (SkeletonAnatomy::Control, SizeStep::Sm, 150.0, (150.0, 32.0)),
            (SkeletonAnatomy::Control, SizeStep::Md, 150.0, (150.0, 40.0)),
            (SkeletonAnatomy::Control, SizeStep::Lg, 150.0, (150.0, 48.0)),
            (SkeletonAnatomy::Control, SizeStep::Xl, 150.0, (150.0, 64.0)),
            (
                SkeletonAnatomy::Control,
                SizeStep::Xxl,
                150.0,
                (150.0, 80.0),
            ),
            (SkeletonAnatomy::Tag, SizeStep::Sm, 60.0, (60.0, 18.0)),
            (SkeletonAnatomy::Tag, SizeStep::Md, 60.0, (60.0, 24.0)),
            (SkeletonAnatomy::Tag, SizeStep::Lg, 60.0, (60.0, 32.0)),
        ] {
            assert_eq!(
                anatomy.block(step, inline),
                want,
                "{} at {} is Carbon's published block, not an approximation",
                anatomy.wire(),
                step.wire()
            );
        }
    }

    /// FR-013: every manifest row `| state | skeleton |` produces its
    /// Carbon-published blocks from the one shared shape — the recipe table
    /// (VARIANTS.md's twenty rows) kept beside the numbers it pins. The
    /// recipe is the call-site knowledge (contract: "the recipe belongs at
    /// the call site and in the test that pins the numbers"); the shape is
    /// the only thing that draws. `inline` is the call site's width where
    /// Carbon publishes a percentage, and Carbon's pixel value where it
    /// publishes one.
    #[test]
    fn every_manifest_component_produces_its_published_skeleton() {
        // One Carbon-published block: anatomy, size step, call-site inline
        // extent, and the (width, height) the block must come out as.
        type Recipe = (SkeletonAnatomy, SizeStep, f32, (f32, f32));
        type ComponentRecipe = (&'static str, &'static [Recipe]);
        let recipes: &[ComponentRecipe] = &[
            // Title bars `--skeleton__text` 16 × 100 %; the arrow stays a real icon.
            (
                "#12 accordion",
                &[(SkeletonAnatomy::Text, SizeStep::Md, 160.0, (160.0, 16.0))],
            ),
            // Link bars 1rem × 100px.
            (
                "#43 breadcrumb",
                &[(SkeletonAnatomy::Text, SizeStep::Md, 100.0, (100.0, 16.0))],
            ),
            // 150 wide at the step's own height (32/40/48).
            (
                "#64 button",
                &[
                    (SkeletonAnatomy::Control, SizeStep::Sm, 150.0, (150.0, 32.0)),
                    (SkeletonAnatomy::Control, SizeStep::Md, 150.0, (150.0, 40.0)),
                    (SkeletonAnatomy::Control, SizeStep::Lg, 150.0, (150.0, 48.0)),
                ],
            ),
            // Label text $spacing-05 (16px) × 100px.
            (
                "#78 checkbox",
                &[(SkeletonAnatomy::Text, SizeStep::Md, 100.0, (100.0, 16.0))],
            ),
            // Snippet bars 1rem × 100 % (single container 56, multi 98).
            (
                "#88 code snippet",
                &[(SkeletonAnatomy::Text, SizeStep::Md, 320.0, (320.0, 16.0))],
            ),
            // Header title 24×120, cells 16×64, header description 16×160.
            (
                "#150 data table",
                &[
                    (SkeletonAnatomy::Heading, SizeStep::Md, 120.0, (120.0, 24.0)),
                    (SkeletonAnatomy::Text, SizeStep::Md, 64.0, (64.0, 16.0)),
                    (SkeletonAnatomy::Text, SizeStep::Md, 160.0, (160.0, 16.0)),
                ],
            ),
            // Input skeletons 14×75 (range inputs 143.5 wide).
            (
                "#167 date picker",
                &[
                    (SkeletonAnatomy::Caption, SizeStep::Md, 75.0, (75.0, 14.0)),
                    (SkeletonAnatomy::Caption, SizeStep::Md, 143.5, (143.5, 14.0)),
                ],
            ),
            // Field block at the md step (40 high), inline 100 %.
            (
                "#192 dropdown",
                &[(SkeletonAnatomy::Control, SizeStep::Md, 320.0, (320.0, 40.0))],
            ),
            // Field block sized to the input: 32/40/48 by step.
            (
                "#342 number input",
                &[
                    (SkeletonAnatomy::Control, SizeStep::Sm, 200.0, (200.0, 32.0)),
                    (SkeletonAnatomy::Control, SizeStep::Md, 200.0, (200.0, 40.0)),
                    (SkeletonAnatomy::Control, SizeStep::Lg, 200.0, (200.0, 48.0)),
                ],
            ),
            // `--skeleton__text` placeholder bars 16 × 100 %.
            (
                "#354 pagination",
                &[(SkeletonAnatomy::Text, SizeStep::Md, 160.0, (160.0, 16.0))],
            ),
            // The label bar only, 14×40; the step icon/line stays real.
            (
                "#387 progress indicator",
                &[(SkeletonAnatomy::Caption, SizeStep::Md, 40.0, (40.0, 14.0))],
            ),
            // Label 18×100; the appearance slot is hidden, not skeletonized.
            (
                "#401 radio button",
                &[(SkeletonAnatomy::Label, SizeStep::Md, 100.0, (100.0, 18.0))],
            ),
            // Input block, inline 100 %, 32/40/48 by step.
            (
                "#414 search",
                &[
                    (SkeletonAnatomy::Control, SizeStep::Sm, 320.0, (320.0, 32.0)),
                    (SkeletonAnatomy::Control, SizeStep::Md, 320.0, (320.0, 40.0)),
                    (SkeletonAnatomy::Control, SizeStep::Lg, 320.0, (320.0, 48.0)),
                ],
            ),
            // Select input block 2.5rem (40) high, inline 100 %.
            (
                "#432 select",
                &[(SkeletonAnatomy::Control, SizeStep::Md, 320.0, (320.0, 40.0))],
            ),
            // Range labels 12×20; the track keeps cursor/default, no fill change.
            (
                "#447 slider",
                &[(SkeletonAnatomy::Micro, SizeStep::Md, 20.0, (20.0, 12.0))],
            ),
            // Cell spans 1rem high × 75 % of the cell inline.
            (
                "#457 structured list",
                &[(SkeletonAnatomy::Text, SizeStep::Md, 240.0, (240.0, 16.0))],
            ),
            // Nav-link span 14 high × 100 % of the 10rem link slot.
            (
                "#480 tabs",
                &[(SkeletonAnatomy::Caption, SizeStep::Md, 160.0, (160.0, 14.0))],
            ),
            // 60 wide, 18/24/32 at sm/md/lg.
            (
                "#507 tag",
                &[
                    (SkeletonAnatomy::Tag, SizeStep::Sm, 60.0, (60.0, 18.0)),
                    (SkeletonAnatomy::Tag, SizeStep::Md, 60.0, (60.0, 24.0)),
                    (SkeletonAnatomy::Tag, SizeStep::Lg, 60.0, (60.0, 32.0)),
                ],
            ),
            // Label 14×75 above the field block, 32/40/48 by step.
            (
                "#522 text input",
                &[
                    (SkeletonAnatomy::Caption, SizeStep::Md, 75.0, (75.0, 14.0)),
                    (SkeletonAnatomy::Control, SizeStep::Sm, 320.0, (320.0, 32.0)),
                    (SkeletonAnatomy::Control, SizeStep::Md, 320.0, (320.0, 40.0)),
                    (SkeletonAnatomy::Control, SizeStep::Lg, 320.0, (320.0, 48.0)),
                ],
            ),
            // Knob 18×18 beside the 24×8 rectangle.
            (
                "#549 toggle",
                &[
                    (SkeletonAnatomy::Knob, SizeStep::Md, 999.0, (18.0, 18.0)),
                    (SkeletonAnatomy::Sliver, SizeStep::Md, 24.0, (24.0, 8.0)),
                ],
            ),
        ];
        for (component, recipe) in recipes {
            for &(anatomy, step, inline, want) in recipe.iter() {
                let node = skeleton("s", anatomy, step, inline, 0.0);
                assert_eq!(
                    (
                        node.constraints.horizontal.min.unwrap_or_default(),
                        node.constraints.vertical.min.unwrap_or_default()
                    ),
                    want,
                    "{component}: a {} block at {} is {}×{}, not an approximation",
                    anatomy.wire(),
                    step.wire(),
                    want.0,
                    want.1
                );
                assert!(
                    node.semantics.skeleton && node.ambient && node.interactions.is_empty(),
                    "{component}: the block declares skeleton + ambient and no interactions"
                );
            }
        }
    }

    /// A pair Carbon does not publish is a misconfiguration and says so.
    #[test]
    #[should_panic(expected = "Carbon publishes no tag at the 2xl step")]
    fn an_unpublished_step_fails_loudly() {
        let _ = SkeletonAnatomy::Tag.block(SizeStep::Xxl, 60.0);
    }

    #[test]
    #[should_panic(expected = "is a misconfiguration")]
    fn a_block_with_no_inline_extent_fails_loudly() {
        let _ = SkeletonAnatomy::Text.block(SizeStep::Md, 0.0);
    }

    /// FR-014: no interactions, and nothing skeleton-shaped in the focus
    /// order beside a real control that is.
    #[test]
    fn a_skeleton_is_absent_from_the_focus_order() {
        let real = crate::component::button::button("real", "Save");
        let node = crate::component::kit::stack(
            "page",
            crate::geom::Axis::Vertical,
            None,
            vec![
                real,
                skeleton("bar", SkeletonAnatomy::Control, SizeStep::Md, 150.0, 0.0),
                skeleton("knob", SkeletonAnatomy::Knob, SizeStep::Md, 18.0, 0.0),
            ],
        );
        assert_skeletons_declare_nothing(&node);

        let frame = petrify_lone(node);
        let focus = FocusTree::from_placements(&frame.placements, &BTreeMap::new());
        let order = focus.order();
        assert!(
            order.iter().any(|id| id.ends_with("/real")),
            "the control that should be reachable is: {order:?}"
        );
        for p in &frame.placements {
            if p.id.ends_with("/bar") || p.id.ends_with("/knob") {
                assert!(
                    !order.contains(&p.id),
                    "a skeleton is not in the focus order: {}",
                    p.id
                );
            }
        }
    }

    fn assert_skeletons_declare_nothing(node: &ViewNode) {
        if node.semantics.skeleton {
            assert!(
                node.interactions.is_empty(),
                "a skeleton declares no interaction: {:?}",
                node.interactions
            );
            assert!(
                !node.interactions.contains(&Interaction::Focus),
                "a skeleton is not focusable"
            );
            assert!(
                node.semantics.role.is_none() && node.semantics.label.is_none(),
                "a skeleton carries no role and no label"
            );
        }
        for child in &node.children {
            assert_skeletons_declare_nothing(child);
        }
    }

    /// FR-015: a driver tells loading from empty without pixels — the loading
    /// tree has skeleton nodes, the empty tree has none.
    #[test]
    fn a_driver_distinguishes_loading_from_empty_without_pixels() {
        let loading = petrify_lone(skeleton(
            "bar",
            SkeletonAnatomy::Caption,
            SizeStep::Md,
            75.0,
            0.0,
        ));
        let empty = petrify_lone(ViewNode::new(NodeKind::Stack, "empty"));

        let query = TreeQuery::new().with_state(StateFlag::Skeleton);
        let loading_tree = project(&loading).expect("a petrified frame has a root");
        let empty_tree = project(&empty).expect("a petrified frame has a root");
        let loading_hits = loading_tree.iter().filter(|n| query.matches(n)).count();
        let empty_hits = empty_tree.iter().filter(|n| query.matches(n)).count();
        assert_eq!(loading_hits, 1, "the loading tree names its skeleton");
        assert_eq!(empty_hits, 0, "the empty tree has none");
    }

    /// The pulse is the transcribed keyframe: scale and pivot at Carbon's
    /// nine stops, and opacity at the three stops that declare one. The
    /// in-between opacity is the eased interpolation those stops describe —
    /// asserting it by a hand-computed number would pin the test to this
    /// test's arithmetic rather than to Carbon.
    #[test]
    fn the_pulse_is_the_keyframe_transcribed() {
        for (phase, want_scale, want_left) in [
            (0.00_f32, 0.0, true),
            (0.20, 1.0, true),
            (0.28, 1.0, false),
            (0.51, 0.0, false),
            (0.58, 0.0, false),
            (0.82, 1.0, false),
            (0.83, 1.0, true),
            (0.96, 0.0, true),
            (1.00, 0.0, true),
        ] {
            let (scale, left) = pulse_scale(phase);
            assert!(
                (scale - want_scale).abs() < 0.001,
                "scale at {phase}: {scale}"
            );
            assert_eq!(left, want_left, "pivot at {phase}");
        }
        for (phase, want) in [(0.00_f32, 0.3_f32), (0.20, 1.0), (1.00, 0.3)] {
            let opacity = pulse_opacity(phase);
            assert!(
                (opacity - want).abs() < 0.001,
                "opacity at {phase}: {opacity}"
            );
        }
        let mid = pulse_opacity(0.60);
        assert!(
            mid > 0.3 && mid < 1.0,
            "the undeclared stretch interpolates down from 1 to 0.3: {mid}"
        );
    }

    /// Carbon animates a clipped box: the knob's 200 %-wide fill saturates at
    /// half scale, and its box leaves the circle late in a collapse.
    #[test]
    fn the_circular_fill_saturates_at_half_scale() {
        let full = pulse_fill(18.0, 36.0, 0.10);
        assert_eq!(full.x0, 0.0);
        assert!(
            (full.x1 - 18.0).abs() < 0.001,
            "saturated at 0.10: {full:?}"
        );

        let late_collapse = pulse_fill(18.0, 36.0, 0.48);
        assert!(
            late_collapse.x1 <= late_collapse.x0,
            "the box has left the circle at 0.48: {late_collapse:?}"
        );
    }

    /// Every band of the circular fill at every phase draws — the ground plus
    /// one convex band — because the swept box is clipped to the circle at
    /// interior spans too, not only at the saturating one. Before the arc
    /// split was fixed to walk the crowns the band actually crosses, the
    /// draw list refused every phase between the rests (the polygon turned
    /// back on itself), which only a full-cycle sweep finds.
    #[test]
    fn a_circular_band_draws_at_every_phase() {
        for step in 0..200_usize {
            // 0..200 is exactly representable in f32; the cast is the map.
            #[allow(clippy::cast_precision_loss)]
            let phase = step as f32 / 200.0;
            let now = f64::from(phase) * PULSE_SECONDS;
            let mut knob = skeleton("knob", SkeletonAnatomy::Knob, SizeStep::Md, 18.0, now);
            // Through the walker too: it re-derives the block from the list
            // and rebuilds it, the same refusal would fire there.
            advance_ambient_skeletons(&mut knob, now, false);
            let list = list_of(&knob);
            let commands = list.commands().len();
            assert!(
                commands == 1 || commands == 4,
                "phase {phase}: ground alone or ground + one band, saw {commands} command(s)"
            );
        }
    }

    /// FR-017's declaration, on the node itself.
    #[test]
    fn the_shape_is_a_canvas_that_declares_ambient() {
        let node = skeleton("bar", SkeletonAnatomy::Text, SizeStep::Md, 64.0, 0.0);
        assert_eq!(node.kind, NodeKind::Canvas, "the pulse lives on a canvas");
        assert!(node.ambient, "the pulse is declared ambient, never foreign");
        assert!(node.semantics.skeleton, "FR-015");
        assert!(
            node.props.canvas.is_some(),
            "the shape publishes a draw list"
        );
    }

    /// The host's clock moves the fill; reduced motion rests it and the node
    /// stops asking for frames.
    #[test]
    fn the_host_clock_moves_the_fill_and_reduced_motion_rests_it() {
        let mut node = crate::component::kit::stack(
            "root",
            crate::geom::Axis::Vertical,
            None,
            vec![skeleton(
                "bar",
                SkeletonAnatomy::Text,
                SizeStep::Md,
                64.0,
                0.0,
            )],
        );
        // Phase 0 publishes the empty starting fill: ground only.
        assert_eq!(list_of(&node.children[0]).commands().len(), 1);

        advance_ambient_skeletons(&mut node, PULSE_SECONDS * 0.1, false);
        let bar = &node.children[0];
        assert!(bar.ambient, "a pulsing skeleton keeps its declaration");
        let grown = list_of(bar).commands().len();
        assert_eq!(
            grown,
            4,
            "ground plus a fill layer: {:?}",
            list_of(bar).commands()
        );

        advance_ambient_skeletons(&mut node, PULSE_SECONDS * 0.35, false);
        let later = fill_span_of(&node.children[0]);
        let earlier = {
            advance_ambient_skeletons(&mut node, PULSE_SECONDS * 0.1, false);
            fill_span_of(&node.children[0])
        };
        assert!(
            (later.0 - earlier.0).abs() > 0.5 || (later.1 - earlier.1).abs() > 0.5,
            "the walk repaints the phase it is given: {earlier:?} vs {later:?}"
        );

        advance_ambient_skeletons(&mut node, 0.0, true);
        let bar = &node.children[0];
        assert!(!bar.ambient, "a reduced skeleton stops asking for frames");
        assert_eq!(
            list_of(bar).commands().len(),
            4,
            "Carbon's animation: none rests a full fill over the ground"
        );
        let rest = rest_fill(64.0);
        assert_eq!((rest.x0, rest.x1), (0.0, 64.0));
        assert_eq!(rest.opacity, 1.0);
    }

    #[test]
    fn the_pulse_wraps_on_its_own_cycle() {
        assert_eq!(skeleton_pulse_phase(0.0), 0.0);
        let cycle = skeleton_pulse_phase(PULSE_SECONDS + 0.3);
        let base = skeleton_pulse_phase(0.3);
        assert!(
            (cycle - base).abs() < 1e-6,
            "one lap later is the same phase"
        );
    }

    fn list_of(node: &ViewNode) -> &crate::draw::DrawList {
        node.props.canvas.as_ref().expect("a skeleton is a canvas")
    }

    /// The fill rect's `(x, width)` out of a published block list — the rect
    /// under the fill layer's `Push`, never the ground.
    fn fill_span_of(node: &ViewNode) -> (f32, f32) {
        for command in list_of(node).commands().iter().skip(1) {
            if let Command::Rect { rect, .. } = command {
                return (rect.x, rect.w);
            }
        }
        panic!(
            "the block published no fill layer: {:?}",
            list_of(node).commands()
        );
    }

    /// The empty-fill case draws no fill layer at all rather than an empty
    /// rect with an opacity on it.
    #[test]
    fn an_empty_pulse_draws_ground_only() {
        let list = block_list(
            SkeletonAnatomy::Text,
            64.0,
            16.0,
            Fill {
                x0: 0.0,
                x1: 0.0,
                opacity: 0.3,
            },
        );
        assert_eq!(
            list.commands().len(),
            1,
            "ground only: {:?}",
            list.commands()
        );
        assert!(matches!(
            list.commands()[0],
            Command::Rect { .. } | Command::Ellipse { .. }
        ));
    }
}
