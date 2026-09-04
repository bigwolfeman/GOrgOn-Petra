//! Carbon Date picker (slice-b). Closed field plus a one-month calendar
//! popover. No date library.
//!
//! Anatomy (`_date-picker.scss` + `_flatpickr.scss`):
//! 1. Field — [`SURFACE_RAISED`] + [`BORDER_SUBTLE`], height md 40.
//! 2. Current value (visible text).
//! 3. Calendar mark as the word `"calendar"` — never an icon-only glyph
//!    (FR-026).
//! 4. Open calendar — [`super::popover::popover_with`] hosting a 7-column
//!    weekday-initial row plus day buttons `1..=28`.
//!
//! Calendar menu is a fixed 288×336 box (SCSS), independent of field size.
//! Day cells are 40×40 ([`SIZE_MD`]). Previous/next month and a real
//! Gregorian grid are omitted: 28 days is enough to prove the anatomy.

use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, SHAPE_SM, SIZE_MD, SPACING_03, SPACING_05, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, Interaction, Key, NodeKind, Props, Role, TrackSize, ViewNode,
};

/// Carbon calendar menu width (`18rem`). Independent of field size.
const CALENDAR_W: f32 = 288.0;
/// Carbon calendar menu height (`21rem`). Style-page and SCSS agree.
const CALENDAR_H: f32 = 336.0;
/// Days shown to prove the grid. Four weeks; not a real month length.
const VISIBLE_DAYS: u32 = 28;

const WEEKDAYS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(CALENDAR_W == 288.0);
const _: () = assert!(CALENDAR_H == 336.0);
const _: () = assert!(VISIBLE_DAYS == 28);

const FIELD_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Closed date field at Carbon md (40). `label` is the accessible name;
/// `value` is the visible date text.
pub fn date_picker(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    closed_field(key, label, value, None)
}

/// Open calendar: the closed field plus a popover with weekday initials
/// and day buttons `1..=28`.
///
/// The field child is keyed `"field"`; the popover is keyed `"calendar"`
/// and anchored to `"field"`.
pub fn date_picker_open(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let field = closed_field("field", label.clone(), value, Some(true));
    let calendar = popover_with("calendar", label, "field", vec![month_grid()]);
    let mut node = stack(key, Axis::Vertical, None, vec![field, calendar]);
    node.semantics.expanded = Some(true);
    node
}

fn closed_field(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    expanded: Option<bool>,
) -> ViewNode {
    let label = label.into();
    let mut value_node = text("value", value.into());
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut mark = text("calendar-mark", "calendar");
    mark.props.tokens.insert("foreground".into(), t(TEXT_MUTED));

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![value_node, mark],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node =
        node.with_constraints(pin_height(SIZE_MD))
            .interactive(Role::Button, label, FIELD_INTENTS);
    node.semantics.expanded = expanded;
    node
}

fn month_grid() -> ViewNode {
    let mut children = Vec::with_capacity(7 + VISIBLE_DAYS as usize);
    for (i, name) in WEEKDAYS.iter().enumerate() {
        let mut cell = text(format!("dow-{i}"), (*name).to_owned());
        cell.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
        children.push(cell);
    }
    for day in 1..=VISIBLE_DAYS {
        children.push(day_button(day));
    }
    let mut grid = ViewNode::new(NodeKind::Grid, "days").with_props(Props {
        columns: vec![TrackSize::Fixed { value: SIZE_MD }; 7],
        rows: vec![TrackSize::Fixed { value: SIZE_MD }; 5],
        ..Props::default()
    });
    grid.constraints = Constraints {
        horizontal: AxisConstraint {
            min: Some(CALENDAR_W),
            max: Some(CALENDAR_W),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(CALENDAR_H),
            max: Some(CALENDAR_H),
            priority: 0,
        },
    };
    grid.with_children(children)
}

fn day_button(day: u32) -> ViewNode {
    let label = day.to_string();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(format!("day-{day}"), Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(pin_height(SIZE_MD))
        .interactive(Role::Button, label, FIELD_INTENTS)
}

fn pin_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CALENDAR_H, CALENDAR_W, SIZE_MD, VISIBLE_DAYS, WEEKDAYS, date_picker, date_picker_open,
    };
    use crate::component::tokens::{BORDER_SUBTLE, SURFACE_RAISED};
    use crate::tree::{Anchor, Interaction, NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn date_picker_is_a_closed_button_at_height_40() {
        let node = date_picker("due", "Due date", "2026-08-30");
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Due date"));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert!(node.interactions.contains(&Interaction::Click));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert_eq!(
            child(&node, "value").props.text.as_deref(),
            Some("2026-08-30")
        );
        assert_eq!(
            child(&node, "calendar-mark").props.text.as_deref(),
            Some("calendar")
        );
        assert!(child(&node, "calendar-mark").semantics.role.is_none());
        assert_eq!(node.semantics.expanded, None);
        assert_ne!(node.semantics.role, Some(Role::Overlay));
    }

    #[test]
    fn date_picker_open_hosts_weekdays_and_day_buttons_in_a_popover() {
        let node = date_picker_open("due", "Due date", "2026-08-30");
        assert_eq!(node.semantics.expanded, Some(true));
        let field = child(&node, "field");
        assert_eq!(field.semantics.role, Some(Role::Button));
        assert_eq!(field.semantics.expanded, Some(true));

        let calendar = child(&node, "calendar");
        assert_eq!(calendar.kind, NodeKind::Surface);
        assert_eq!(calendar.semantics.role, Some(Role::Overlay));
        match &calendar.props.anchor {
            Some(Anchor::Node { id, .. }) => assert_eq!(id, "field"),
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        let content = child(calendar, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        let days = child(content, "days");
        assert_eq!(days.kind, NodeKind::Grid);
        assert_eq!(days.props.columns.len(), 7);
        assert_eq!(days.constraints.horizontal.max, Some(CALENDAR_W));
        assert_eq!(days.constraints.vertical.max, Some(CALENDAR_H));
        assert_eq!(CALENDAR_W, 288.0);
        assert_eq!(CALENDAR_H, 336.0);

        for (i, name) in WEEKDAYS.iter().enumerate() {
            let cell = child(days, &format!("dow-{i}"));
            assert_eq!(cell.props.text.as_deref(), Some(*name));
            assert!(cell.semantics.role.is_none());
            assert!(cell.interactions.is_empty());
        }
        assert_eq!(VISIBLE_DAYS, 28);
        for day in 1..=VISIBLE_DAYS {
            let cell = child(days, &format!("day-{day}"));
            assert_eq!(cell.semantics.role, Some(Role::Button));
            assert_eq!(
                cell.semantics.label.as_deref(),
                Some(day.to_string().as_str())
            );
            assert!(cell.interactions.contains(&Interaction::Click));
        }
        assert!(
            days.children
                .iter()
                .all(|c| c.key.as_str() != "day-29" && c.key.as_str() != "day-31"),
            "anatomy grid stops at 28; this is not a date library"
        );
    }
}
