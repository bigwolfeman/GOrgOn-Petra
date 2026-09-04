//! Carbon Progress indicator (slice-d).
//!
//! Horizontal row of connected steps. Distinct from [`super::progress`]
//! (a system-driven bar). Anatomy of one horizontal step
//! (`_progress-indicator.scss` + `26-progress-indicator.png`):
//! 1. Step line — 2px, **the step's own top rule**, spanning the step's
//!    128 width (`.cds--progress-line`, `:59-60`); [`ACCENT_PRIMARY`] when
//!    complete or current, else [`BORDER_SUBTLE`]. Adjacent steps abut, so
//!    the rules join into one rail that ends at the last step's edge.
//! 2. Status indicator, 16×16, 10 below the step's top (`margin-block-
//!    start: 10px`; T070: SCSS wins over the style page's 16) — complete:
//!    [`IconMark::Check`] on an accent disc; current: Carbon's `Incomplete`
//!    glyph, an accent ring with its left half filled; not started: an
//!    empty ring. Three different silhouettes, so no two states differ by
//!    hue alone.
//! 3. Label, inline to the right of the icon, `$spacing-03` off it.
//!
//! Complete is `Semantics.value = "complete"`; current is
//! `Semantics.selected`. Never colour alone.
//!
//! # What the line is not
//!
//! Until 2026-09-04 the line was drawn *beside* the icon as a connector to
//! the next step, with `max: None` so it grew to fill the row, and every
//! step drew one — including the last, which hung a 265-logical-unit rule
//! off the last circle into nothing. That is the picture the operator
//! called "the gap between circles looks bad". Carbon's line is not a link
//! between two circles; it is each step's own top edge, and the last
//! step's ends where the step does.

use std::sync::Arc;

use super::icon::{IconMark, icon};
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, SHAPE_FULL, SPACING_03, TEXT_PRIMARY, t};
use crate::draw::{ColorRef, Command, DrawList, Paint, PathVerb, Stroke, Width};
use crate::geom::{Align, Axis, Point, Size};
use crate::tree::{AxisConstraint, Constraints, Key, NodeKind, Props, Role, Semantics, ViewNode};

/// Carbon step `inline-size` (`convert.to-rem(128px)`, `:43-44`). Fixed,
/// not a floor: the default indicator does not grow its steps
/// (`--space-equal` is the variant that does, and it is not built here).
const STEP_WIDTH: f32 = 128.0;
/// Carbon icon 16×16 (`$spacing-05`).
const ICON: f32 = 16.0;
/// Carbon `.cds--progress-line` height.
const LINE: f32 = 2.0;
/// T070: SCSS `margin-block-start: 10px`, not the style-page 16. Composed
/// here as the 2-unit line plus the `$spacing-03` gap under it.
const ICON_TOP: f32 = 10.0;
/// The `Incomplete` glyph's ring stroke.
const RING_STROKE: f32 = 1.0;

const _: () = assert!(STEP_WIDTH == 128.0);
const _: () = assert!(ICON == 16.0);
const _: () = assert!(LINE == 2.0);
const _: () = assert!(ICON_TOP == LINE + 8.0);

/// Horizontal progress indicator. `Role::List`, no interactions. Steps
/// abut: the spacing is `None` so each step's top rule meets the next.
pub fn progress_indicator(key: impl Into<Key>, steps: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, steps);
    node.semantics = Semantics {
        role: Some(Role::List),
        ..Semantics::default()
    };
    node
}

/// One step: its own top rule, then icon + label inline. Exactly 128 wide.
///
/// `complete` writes [`Semantics.value`] `"complete"` and draws
/// [`IconMark::Check`]. `current` writes [`Semantics.selected`]. A step
/// may be both (the facts are independent). Not-started is an empty
/// circle.
pub fn progress_step(
    key: impl Into<Key>,
    label: impl Into<String>,
    complete: bool,
    current: bool,
) -> ViewNode {
    let label = label.into();
    let line_fill = if complete || current {
        ACCENT_PRIMARY
    } else {
        BORDER_SUBTLE
    };

    // A childless `Stack`, not a `swatch`, so `align_self: Stretch` can
    // span the step's width at place time without the spacer's "whatever
    // is offered" measure — `pagination::nav_divider` documents why.
    let mut line = stack("line", Axis::Horizontal, None, vec![]);
    line.props.tokens.insert("background".into(), t(line_fill));
    line.props.align_self = Some(Align::Stretch);
    line.constraints.vertical = AxisConstraint {
        min: Some(LINE),
        max: Some(LINE),
        priority: 0,
    };

    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let mut row = stack(
        "row",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![status_icon(complete, current), caption],
    );
    row.props.align = Some(Align::Center);

    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), vec![line, row]);
    node.constraints.horizontal = AxisConstraint {
        min: Some(STEP_WIDTH),
        max: Some(STEP_WIDTH),
        priority: 0,
    };
    node.semantics = Semantics {
        role: Some(Role::ListItem),
        label: Some(label),
        value: complete.then(|| "complete".to_string()),
        selected: current,
        ..Semantics::default()
    };
    node
}

fn status_icon(complete: bool, current: bool) -> ViewNode {
    if complete {
        complete_mark()
    } else if current {
        current_mark()
    } else {
        swatch(
            "icon",
            ICON,
            ICON,
            None,
            Some(BORDER_SUBTLE),
            Some(SHAPE_FULL),
        )
    }
}

fn square(extent: f32) -> Constraints {
    Constraints {
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
    }
}

/// Check on a 16×16 accent circle so [`TEXT_ON_ACCENT`] ink has a ground.
fn complete_mark() -> ViewNode {
    let tick = icon("mark", IconMark::Check);
    let inset = ((ICON - tick.constraints.horizontal.min.unwrap_or(0.0)) * 0.5).max(0.0);
    let mut node = stack(
        "icon",
        Axis::Horizontal,
        None,
        vec![
            swatch("inset-start", inset, ICON, None, None, None),
            tick,
            swatch("inset-end", inset, ICON, None, None, None),
        ],
    );
    node.props.align = Some(Align::Center);
    node.props
        .tokens
        .insert("background".into(), t(ACCENT_PRIMARY));
    node.props.tokens.insert("radius".into(), t(SHAPE_FULL));
    node.with_constraints(square(ICON))
}

/// Carbon's `Incomplete` glyph for the current step: an accent ring with
/// its left half filled. A canvas rather than a swatch because a swatch is
/// a whole rect and this is half a disc.
///
/// The half-disc is a twelve-segment polygon on the ring's own radius —
/// every vertex on one circle, so it is convex and [`DrawList::new`]
/// accepts the fill (`contracts/draw-list.md` §5). The ring is stroked one
/// unit wide on a radius one unit inside the box, so the outer edge of the
/// stroke lands on the box's edge and nothing is clipped.
///
/// # Panics
/// Never in practice: one ellipse and one thirteen-verb convex path are
/// well inside every draw-list bound. A panic here means an edit broke
/// convexity, which is a defect and not a runtime condition to handle.
fn current_mark() -> ViewNode {
    const SEGMENTS: usize = 12;
    let half = ICON / 2.0;
    let radius = half - RING_STROKE;
    let accent = ColorRef::Token(t(ACCENT_PRIMARY).as_str().to_owned());
    let ring = Command::Ellipse {
        center: Point::new(half, half),
        radii: Size::new(radius, radius),
        paint: Paint::stroked(Stroke {
            width: Width::Logical(RING_STROKE),
            color: accent.clone(),
        }),
    };
    // Top of the ring, round the left side, to the bottom; the close is
    // the vertical chord. Screen y grows downward, hence the minus.
    let verbs: Vec<PathVerb> = (0..=SEGMENTS)
        .map(|k| {
            let theta =
                std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (k as f32) / (SEGMENTS as f32);
            let point = Point::new(half + radius * theta.cos(), half - radius * theta.sin());
            if k == 0 {
                PathVerb::MoveTo(point)
            } else {
                PathVerb::LineTo(point)
            }
        })
        .collect();
    let half_disc = Command::Path {
        verbs,
        closed: true,
        paint: Paint::filled(accent),
    };
    let list = DrawList::new(vec![ring, half_disc])
        .unwrap_or_else(|err| panic!("Incomplete mark draw list refused: {err}"));
    ViewNode::new(NodeKind::Canvas, "icon")
        .with_props(Props {
            canvas: Some(Arc::new(list)),
            ..Props::default()
        })
        .with_constraints(square(ICON))
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT_PRIMARY, BORDER_SUBTLE, ICON, ICON_TOP, LINE, STEP_WIDTH, progress_indicator,
        progress_step,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Rect, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{NodeKind, Props, Registry, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn has_canvas(node: &ViewNode) -> bool {
        node.kind == NodeKind::Canvas || node.children.iter().any(|child| has_canvas(child))
    }

    fn three() -> ViewNode {
        progress_indicator(
            "pi",
            vec![
                progress_step("a", "Choose", true, false),
                progress_step("b", "Configure", false, true),
                progress_step("c", "Review", false, false),
            ],
        )
    }

    #[test]
    fn progress_indicator_is_a_horizontal_list() {
        let node = three();
        assert_eq!(node.semantics.role, Some(Role::List));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert_eq!(node.children.len(), 3);
        assert_eq!(
            node.props.spacing, None,
            "steps abut so their top rules join into one rail"
        );
    }

    #[test]
    fn progress_step_is_exactly_128_wide() {
        let node = progress_step("a", "Choose", false, false);
        assert_eq!(node.constraints.horizontal.min, Some(STEP_WIDTH));
        assert_eq!(
            node.constraints.horizontal.max,
            Some(STEP_WIDTH),
            "the default indicator's steps are fixed, not space-equal"
        );
        assert_eq!(STEP_WIDTH, 128.0);
        assert_eq!(node.semantics.role, Some(Role::ListItem));
        assert_eq!(node.semantics.label.as_deref(), Some("Choose"));
        assert!(node.interactions.is_empty());
    }

    #[test]
    fn progress_step_complete_is_check_plus_value() {
        let node = progress_step("a", "Choose", true, false);
        assert_eq!(node.semantics.value.as_deref(), Some("complete"));
        assert!(!node.semantics.selected);
        assert!(has_canvas(&node), "complete draws IconMark::Check");
        assert_eq!(
            token(named(&node, "icon"), "background"),
            Some(ACCENT_PRIMARY)
        );
        let line = named(&node, "line");
        assert_eq!(token(line, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(line.constraints.vertical.min, Some(LINE));
        assert_eq!(LINE, 2.0);
    }

    /// Current is a different *shape* from complete, not a different
    /// tint: a ring with half a disc in it, against a full disc with a
    /// check. Before this the two were both solid accent discs and only a
    /// 5-unit check told them apart.
    #[test]
    fn progress_step_current_is_a_half_filled_ring_not_colour_alone() {
        let node = progress_step("b", "Configure", false, true);
        assert!(node.semantics.selected);
        assert!(node.semantics.value.is_none());
        let icon = named(&node, "icon");
        assert_eq!(
            icon.kind,
            NodeKind::Canvas,
            "current draws the Incomplete glyph"
        );
        assert_eq!(
            token(icon, "background"),
            None,
            "the glyph is drawn, not a filled swatch"
        );
        let complete = progress_step("a", "Choose", true, false);
        assert_ne!(
            icon.props.canvas,
            named(&complete, "mark").props.canvas,
            "current and complete must draw different marks"
        );
        assert_eq!(icon.constraints.horizontal.min, Some(ICON));
        assert_eq!(icon.constraints.vertical.max, Some(ICON));
        assert_eq!(
            token(named(&node, "line"), "background"),
            Some(ACCENT_PRIMARY)
        );
    }

    #[test]
    fn progress_step_not_started_is_an_empty_circle() {
        let node = progress_step("c", "Review", false, false);
        assert!(!node.semantics.selected);
        assert!(node.semantics.value.is_none());
        assert!(!has_canvas(&node));
        let icon = named(&node, "icon");
        assert_eq!(token(icon, "background"), None);
        assert_eq!(token(icon, "border"), Some(BORDER_SUBTLE));
        assert_eq!(token(icon, "radius"), Some(super::SHAPE_FULL));
        assert_eq!(
            token(named(&node, "line"), "background"),
            Some(BORDER_SUBTLE)
        );
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
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

    fn rect_of(frame: &PetrifiedFrame, suffix: &str) -> Rect {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("no placement ending in {suffix}"))
            .rect
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Row 26's defect, pinned on the placed frame: the line is each step's
    /// own top rule — flush with the step's left edge, exactly the step's
    /// width, at the step's top — the steps abut at a 128 pitch, and the
    /// rail ends where the last step does. Nothing trails past it. The
    /// icon row starts 10 below the step's top (T070).
    #[test]
    fn the_line_is_each_steps_own_top_rule_and_nothing_trails_the_last_step() {
        let frame = petrify_lone(three());
        let indicator = rect_of(&frame, "/pi");
        let steps = ["/pi/a", "/pi/b", "/pi/c"].map(|s| rect_of(&frame, s));
        let lines = ["/pi/a/line", "/pi/b/line", "/pi/c/line"].map(|s| rect_of(&frame, s));
        for (step, line) in steps.iter().zip(&lines) {
            assert_eq!(step.w, STEP_WIDTH, "a step is exactly 128 wide");
            assert_eq!(
                line.x, step.x,
                "the rule is flush with its step's left edge"
            );
            assert_eq!(line.w, step.w, "the rule spans exactly its own step");
            assert_eq!(line.y, step.y, "the rule is the step's top edge");
            assert_eq!(line.h, LINE);
        }
        for pair in steps.windows(2) {
            assert_eq!(
                pair[1].x,
                pair[0].x + STEP_WIDTH,
                "steps abut so the rules join into one rail"
            );
        }
        assert_eq!(
            indicator.w,
            3.0 * STEP_WIDTH,
            "the indicator is its steps and nothing more"
        );
        let last_line = lines[2];
        assert_eq!(
            last_line.x + last_line.w,
            indicator.x + indicator.w,
            "the rail ends at the last step's edge; nothing trails past it"
        );
        let row = rect_of(&frame, "/pi/a/row");
        assert_eq!(
            row.y - steps[0].y,
            ICON_TOP,
            "icon row starts 10 below the top"
        );
        assert_eq!(ICON_TOP, 10.0);
    }

    /// Check C/D: every step state places its line and icon with a real
    /// rect, nothing overflows, nothing leaves its parent.
    #[test]
    fn line_and_icon_place_with_a_real_nonzero_rect_in_every_state() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("complete", progress_step("a", "Choose", true, false)),
            ("current", progress_step("b", "Configure", false, true)),
            ("not-started", progress_step("c", "Review", false, false)),
            (
                "disabled",
                crate::component::disabled(progress_step("d", "Review", false, false)),
            ),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            let line = rect_of(&frame, "/line");
            assert!(
                line.w > 0.0 && line.h > 0.0,
                "{label}: line placed with a degenerate rect {line:?}"
            );
            let icon = rect_of(&frame, "/icon");
            assert!(
                icon.w > 0.0 && icon.h > 0.0,
                "{label}: icon placed with a degenerate rect {icon:?}"
            );
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
                    p.id
                );
                assert!(
                    !p.paint.truncated,
                    "{label}: {} was truncated to fit its parent",
                    p.id
                );
                if let Some(parent_idx) = p.parent {
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check E: the label against the page ground the step sits on (the
    /// step itself binds no `background`), in both themes. Steps declare
    /// no `Interaction` at all, so there is no Check F focus case here.
    #[test]
    fn step_label_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            for node in [
                progress_step("a", "Choose", true, false),
                progress_step("b", "Configure", false, true),
                progress_step("c", "Review", false, false),
            ] {
                let label = named(&node, "label");
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "label at {ratio:.2}:1 against page ground fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }
}
