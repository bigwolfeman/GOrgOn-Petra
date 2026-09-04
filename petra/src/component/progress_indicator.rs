//! Carbon Progress indicator (slice-d).
//!
//! Horizontal row of connected steps. Distinct from [`super::progress`]
//! (a system-driven bar). Anatomy (`_progress-indicator.scss`):
//! 1. Status indicator — complete: [`IconMark::Check`] on an accent
//!    circle; current: filled circle; not started: empty circle.
//! 2. Step line — 2px, [`ACCENT_PRIMARY`] when complete/current else
//!    [`BORDER_SUBTLE`].
//! 3. Label.
//!
//! Step min-width 128. Icon top-margin **10px** (T070: SCSS wins; style
//! page 16px is design intent). Complete is `Semantics.value = "complete"`;
//! current is `Semantics.selected`. Never colour alone.

use super::icon::{IconMark, icon};
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, SHAPE_FULL, SPACING_03, TEXT_PRIMARY, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Key, Role, Semantics, ViewNode};

/// Carbon step `min-inline-size` (`convert.to-rem(128px)`).
const STEP_MIN: f32 = 128.0;
/// Carbon icon 16×16 (`$spacing-05`).
const ICON: f32 = 16.0;
/// T070: SCSS `margin-block-start: 10px`, not the style-page 16.
const ICON_TOP: f32 = 10.0;
/// Carbon `.cds--progress-line` height.
const LINE: f32 = 2.0;
/// Icon end-margin `$spacing-03` = 12, so the line keeps the 128 floor.
const LINE_MIN: f32 = STEP_MIN - ICON - 12.0;

const _: () = assert!(STEP_MIN == 128.0);
const _: () = assert!(ICON == 16.0);
const _: () = assert!(ICON_TOP == 10.0);
const _: () = assert!(LINE == 2.0);
const _: () = assert!(LINE_MIN == 100.0);

/// Horizontal progress indicator. `Role::List`, no interactions.
pub fn progress_indicator(key: impl Into<Key>, steps: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, steps);
    node.semantics = Semantics {
        role: Some(Role::List),
        ..Semantics::default()
    };
    node
}

/// One step: icon + label. Min-width 128.
///
/// `complete` writes [`Semantics.value`] `"complete"` and draws
/// [`IconMark::Check`]. `current` writes [`Semantics.selected`]. A step
/// may be both (the facts are independent). Not-started is an empty
/// circle; there is no Incomplete mark in [`IconMark`].
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

    let mut icon_top = stack("icon-top", Axis::Horizontal, None, vec![]);
    icon_top.constraints.vertical = AxisConstraint {
        min: Some(ICON_TOP),
        max: Some(ICON_TOP),
        priority: 0,
    };

    let mut line = swatch("line", 0.0, LINE, Some(line_fill), None, None);
    line.constraints.horizontal = AxisConstraint {
        min: Some(LINE_MIN),
        max: None,
        priority: 0,
    };

    let mut icon_row = stack(
        "icon-row",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![status_icon(complete, current), line],
    );
    icon_row.props.align = Some(Align::Center);

    let head = stack("head", Axis::Vertical, None, vec![icon_top, icon_row]);

    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), vec![head, caption]);
    node.constraints.horizontal.min = Some(STEP_MIN);
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
        swatch(
            "icon",
            ICON,
            ICON,
            Some(ACCENT_PRIMARY),
            Some(BORDER_SUBTLE),
            Some(SHAPE_FULL),
        )
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
    node.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(ICON),
            max: Some(ICON),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(ICON),
            max: Some(ICON),
            priority: 0,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT_PRIMARY, BORDER_SUBTLE, ICON_TOP, LINE, STEP_MIN, progress_indicator, progress_step,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
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

    #[test]
    fn progress_indicator_is_a_horizontal_list() {
        let node = progress_indicator(
            "steps",
            vec![
                progress_step("a", "Choose", true, false),
                progress_step("b", "Configure", false, true),
                progress_step("c", "Review", false, false),
            ],
        );
        assert_eq!(node.semantics.role, Some(Role::List));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.props.axis, Some(crate::geom::Axis::Horizontal));
        assert_eq!(node.children.len(), 3);
    }

    #[test]
    fn progress_step_min_width_is_128() {
        let node = progress_step("a", "Choose", false, false);
        assert_eq!(node.constraints.horizontal.min, Some(STEP_MIN));
        assert_eq!(STEP_MIN, 128.0);
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
        let line = named(&node, "line");
        assert_eq!(token(line, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(line.constraints.vertical.min, Some(LINE));
        assert_eq!(LINE, 2.0);
    }

    #[test]
    fn progress_step_current_is_selected_not_colour_alone() {
        let node = progress_step("b", "Configure", false, true);
        assert!(node.semantics.selected);
        assert!(node.semantics.value.is_none());
        assert!(!has_canvas(&node), "current is a filled circle, not Check");
        let icon = named(&node, "icon");
        assert_eq!(token(icon, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(token(icon, "radius"), Some(super::SHAPE_FULL));
        let line = named(&node, "line");
        assert_eq!(token(line, "background"), Some(ACCENT_PRIMARY));
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
        let line = named(&node, "line");
        assert_eq!(token(line, "background"), Some(BORDER_SUBTLE));
        let top = named(&node, "icon-top");
        assert_eq!(top.constraints.vertical.min, Some(ICON_TOP));
        assert_eq!(ICON_TOP, 10.0);
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

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Class 2's exact suspect: `line` is a childless swatch (this
    /// module's own doc: "Step line — 2px"), the same shape as the
    /// Accordion divider that petrified 0px wide. Unlike that divider,
    /// `line` already carries an explicit `constraints.horizontal.min`
    /// ([`LINE_MIN`]) rather than relying on the stack's natural
    /// (zero) width — this is the frame-level check the audit plan
    /// requires, not a substitute for reading the code, across complete,
    /// current, not-started, and a disabled step.
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
            let line = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/line"))
                .unwrap_or_else(|| panic!("{label}: no placement ending /line"));
            assert!(
                line.rect.w > 0.0 && line.rect.h > 0.0,
                "{label}: line placed with a degenerate rect {:?}",
                line.rect
            );
            let icon = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/icon"))
                .unwrap_or_else(|| panic!("{label}: no placement ending /icon"));
            assert!(
                icon.rect.w > 0.0 && icon.rect.h > 0.0,
                "{label}: icon placed with a degenerate rect {:?}",
                icon.rect
            );
            for p in &frame.placements {
                // `icon-top` is a pure vertical-margin spacer (Carbon's
                // `margin-block-start: 10px` on the icon row): it binds no
                // token, draws no text, and has no children, so
                // `paint.paint_hash` is 0 ("zero when the node draws
                // nothing of its own", `PaintState`'s own doc). Its
                // horizontal extent petrifies to 0 the same way the
                // Accordion divider's did, but unlike that divider it
                // declares no paint content to cover, so a 0px-wide rect
                // here is inert, not a missing pixel — Check C's own
                // wording is "no placement *that declares content*". Every
                // content-bearing placement (`line`, `icon`, `label`) still
                // gets the strict check above and below.
                if p.paint.paint_hash != 0 {
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
    /// no `Interaction` at all (this module's own tests, `assert!(node
    /// .interactions.is_empty())`), so there is no Check F focus case here
    /// — see this module's own doc on the interactive variant being a
    /// scope gap, not audited here.
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
