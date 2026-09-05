//! Carbon Progress indicator (slice-d).
//!
//! Horizontal row of connected steps. Distinct from [`super::progress`]
//! (a system-driven bar). Anatomy of one horizontal step, measured on
//! `26-progress-indicator.png` against `_progress-indicator.scss`:
//! 1. Step line — 2px, **the step's own top rule**, spanning the step's
//!    128 width (`.cds--progress-line`, `position: absolute` at the top);
//!    [`ACCENT_PRIMARY`] when complete or current, else the recessed
//!    [`LAYER_ACCENT`] (Carbon spends `$border-subtle`, which in g100 is the
//!    same tone as `$layer-accent-01`; Petra's `border.subtle` is a raised
//!    hairline tone that read as a bright second rail — the row 25 finding).
//!    Adjacent steps abut, so the rules join into one rail that ends at the
//!    last step's edge.
//! 2. Status glyph, 16×16, its top **10** below the step's top
//!    (`svg { margin-block-start: 10px }`; T070: SCSS wins over the style
//!    page's 16) — complete: [`IconMark::CheckmarkOutline`] in the accent;
//!    current: [`IconMark::Incomplete`] in the accent; not started:
//!    [`IconMark::CircleDash`] in `icon-primary`. Three different
//!    silhouettes, so no two states differ by hue alone.
//! 3. Label, inline to the right of the glyph, `$spacing-03` off it
//!    (`margin-inline-end`), `body-compact-01`, centred on the glyph. In
//!    Carbon that centring is arithmetic — the label's line box starts
//!    `$spacing-03` below the step's top and is 20.3 tall (`line-height:
//!    1.45`), so its centre is 18.15 against the glyph's 18. Petra's body
//!    line box is 16 tall, the glyph's own height, so here the row is
//!    centred on its cross axis and starts 10 from the top, and the two
//!    facts the Carbon capture shows — glyph top at 10, label centred on the
//!    glyph — both hold.
//!
//! Complete is `Semantics.value = "complete"`; current is
//! `Semantics.selected`. Never colour alone.
//!
//! # What changed on 2026-09-04, twice
//!
//! Until the round-1 fix the line was drawn *beside* the glyph as a
//! connector, growing to fill the row, and the last step hung a rule into
//! nothing. Round 1 moved it to the top edge, where Carbon's is, and drew
//! the glyphs as heavy filled discs — a solid accent disc with a check, a
//! half-filled disc, a solid grey ring — which the operator liked less than
//! the connector. Carbon's glyphs are thin: a 1-unit outline ring with a
//! check inside, a half disc with a dashed right half, a dashed ring. Those
//! are what this draws now, off Carbon's own paths.

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{ACCENT_PRIMARY, LAYER_ACCENT, SPACING_03, TEXT_PRIMARY, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Key, Role, Semantics, ViewNode};

/// Carbon step `inline-size` (`convert.to-rem(128px)`, `:43-44`). Fixed,
/// not a floor: the default indicator does not grow its steps
/// (`--space-equal` is the variant that does, and it is not built here).
const STEP_WIDTH: f32 = 128.0;
/// Carbon `.cds--progress-line` height.
const LINE: f32 = 2.0;
/// T070: SCSS `margin-block-start: 10px` on the glyph, not the style-page
/// 16. The glyph-and-label row starts here. The line is out of flow in
/// Carbon (`position: absolute`), so this is measured from the step's top
/// and the in-flow gap under the line is this less the line.
const ICON_TOP: f32 = 10.0;

const _: () = assert!(STEP_WIDTH == 128.0);
const _: () = assert!(LINE == 2.0);
const _: () = assert!(ICON_TOP == 10.0);

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

/// One step: its own top rule, then glyph + label inline. Exactly 128 wide.
///
/// `complete` writes [`Semantics.value`] `"complete"` and draws
/// [`IconMark::CheckmarkOutline`]. `current` writes [`Semantics.selected`]
/// and draws [`IconMark::Incomplete`]. A step may be both (the facts are
/// independent; complete wins the glyph). Not-started is
/// [`IconMark::CircleDash`].
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
        LAYER_ACCENT
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

    let mut node = stack(
        key,
        Axis::Vertical,
        None,
        vec![
            line,
            swatch("gap", 0.0, ICON_TOP - LINE, None, None, None),
            row,
        ],
    );
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
    let (mark, tone) = if complete {
        (IconMark::CheckmarkOutline, IconTone::Accent)
    } else if current {
        (IconMark::Incomplete, IconTone::Accent)
    } else {
        (IconMark::CircleDash, IconTone::Primary)
    };
    icon_toned("icon", mark, tone)
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT_PRIMARY, ICON_TOP, LAYER_ACCENT, LINE, STEP_WIDTH, progress_indicator, progress_step,
    };
    use crate::component::tokens::ICON_PRIMARY;
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

    /// Every colour the glyph's draw list paints with.
    fn glyph_tokens(node: &ViewNode) -> Vec<String> {
        let icon = named(node, "icon");
        assert_eq!(icon.kind, NodeKind::Canvas, "every step glyph is drawn");
        icon.props
            .canvas
            .as_ref()
            .expect("a canvas")
            .color_tokens()
            .map(str::to_owned)
            .collect()
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

    /// Row 26: the three glyphs are Carbon's three — an outlined ring with
    /// a check, a half disc with a dashed half, a dashed ring — in Carbon's
    /// two tones, and no two are the same picture. A solid disc is not one
    /// of them.
    #[test]
    fn the_three_states_are_carbons_three_thin_glyphs() {
        let complete = progress_step("a", "Choose", true, false);
        let current = progress_step("b", "Configure", false, true);
        let pending = progress_step("c", "Review", false, false);
        assert_eq!(complete.semantics.value.as_deref(), Some("complete"));
        assert!(!complete.semantics.selected);
        assert!(current.semantics.selected);
        assert!(current.semantics.value.is_none());
        assert!(!pending.semantics.selected);

        for name in glyph_tokens(&complete)
            .iter()
            .chain(glyph_tokens(&current).iter())
        {
            assert_eq!(
                name, ACCENT_PRIMARY,
                "complete and current are $interactive"
            );
        }
        for name in glyph_tokens(&pending) {
            assert_eq!(name, ICON_PRIMARY, "not started is $icon-primary");
        }
        let pictures =
            [&complete, &current, &pending].map(|n| named(n, "icon").props.canvas.clone());
        assert_ne!(pictures[0], pictures[1]);
        assert_ne!(pictures[1], pictures[2]);
        assert_ne!(pictures[0], pictures[2]);
        for step in [&complete, &current, &pending] {
            assert!(
                !named(step, "icon").props.tokens.contains_key("background"),
                "a step glyph is drawn, never a filled swatch"
            );
        }
        assert_eq!(
            token(named(&complete, "line"), "background"),
            Some(ACCENT_PRIMARY)
        );
        assert_eq!(
            token(named(&current, "line"), "background"),
            Some(ACCENT_PRIMARY)
        );
        assert_eq!(
            token(named(&pending, "line"), "background"),
            Some(LAYER_ACCENT),
            "the incomplete rail recedes; border.subtle read as a bright second rail"
        );
        assert_eq!(
            named(&complete, "line").constraints.vertical.min,
            Some(LINE)
        );
        assert_eq!(LINE, 2.0);
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

    /// Row 26's geometry, pinned on the placed frame against the Carbon
    /// capture: the line is each step's own top rule — flush with the
    /// step's left edge, exactly the step's width, at the step's top — the
    /// steps abut at a 128 pitch, the rail ends where the last step does,
    /// and the glyph's top is 10 below the step's top (T070), which in the
    /// Carbon shot is 20 device pixels under the rule's top edge at 2x.
    #[test]
    fn the_line_is_each_steps_own_top_rule_and_the_glyph_sits_ten_below_it() {
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
        for (step, key) in steps.iter().zip(["/pi/a", "/pi/b", "/pi/c"]) {
            let icon = rect_of(&frame, &format!("{key}/row/icon"));
            assert_eq!(icon.w, 16.0);
            assert_eq!(icon.h, 16.0);
            assert!(
                (icon.y - step.y - ICON_TOP).abs() < 0.01,
                "{key}: the glyph's top is {} below the step's top, Carbon's is {ICON_TOP}",
                icon.y - step.y
            );
            assert_eq!(
                icon.x, step.x,
                "the glyph is flush with the step's left edge"
            );
            let label = rect_of(&frame, &format!("{key}/row/label"));
            let off = (label.y + label.h / 2.0) - (icon.y + icon.h / 2.0);
            assert!(
                off.abs() <= 0.5,
                "{key}: the label's centre is {off} off the glyph's: {label:?} beside {icon:?}"
            );
            assert!(
                (label.x - (icon.x + icon.w + 8.0)).abs() < 0.01,
                "{key}: the label starts $spacing-03 after the glyph, got {label:?}"
            );
        }
        assert_eq!(ICON_TOP, 10.0);
    }

    /// Check C/D: every step state places its line and glyph with a real
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
                // The gap spacer is a zero-width, six-tall spacer by
                // design; every other placement has area.
                if !p.id.ends_with("/gap") {
                    assert!(
                        p.rect.w > 0.0 && p.rect.h > 0.0,
                        "{label}: {} placed with a degenerate rect {:?}",
                        p.id,
                        p.rect
                    );
                }
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
    /// step itself binds no `background`), in both themes; and the glyph
    /// inks against the same ground at the 3:1 a non-text mark needs.
    /// Steps declare no `Interaction` at all, so there is no Check F focus
    /// case here.
    #[test]
    fn step_label_and_glyph_clear_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        const MIN_GLYPH_CONTRAST: f32 = 3.0;
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
                for ink in glyph_tokens(&node) {
                    let ratio = color(&theme, &ink).contrast_ratio(bg);
                    assert!(
                        ratio >= MIN_GLYPH_CONTRAST,
                        "{ink} glyph at {ratio:.2}:1 against page ground fails {MIN_GLYPH_CONTRAST}:1"
                    );
                }
            }
        }
    }
}
