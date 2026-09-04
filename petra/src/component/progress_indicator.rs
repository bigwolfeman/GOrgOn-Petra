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
    use crate::tree::{NodeKind, Role, ViewNode};

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
}
