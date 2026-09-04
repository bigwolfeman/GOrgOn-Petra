//! Carbon Slider (slice-e). Track 2px T070. Two-handle omitted.
//!
//! Anatomy of the default (single-handle) slider, style page +
//! `_slider.scss`:
//! 1. Label — required, above the rail ([`Role::Button`] lives on the
//!    handle, FR-058).
//! 2. Track — unfilled rail, [`BORDER_SUBTLE`], height **2px** (SCSS;
//!    style-page 4px is design intent, T070 prefers SCSS).
//! 3. Min value — leftmost range label (`"0"`).
//! 4. Handle — 14×14 circle ([`SHAPE_FULL`]), [`Interaction::Drag`] plus
//!    Focus and Click.
//! 5. Max value — rightmost range label (`"100"`).
//!
//! Number input is omitted (constructor has no field). Range (two-handle)
//! is omitted. Read-only collapses the handle to zero size, not a dim.

use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, SHAPE_FULL, SPACING_03, TEXT_MUTED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, Interaction, Key, NodeKind, Props, Role, TrackSize, ViewNode,
};

/// MEASURED `_slider.scss` track height. Style page lists 4px; T070 is 2.
const TRACK_HEIGHT: f32 = 2.0;
/// MEASURED default-slider handle at rest (`convert.to-rem(14px)`).
const HANDLE: f32 = 14.0;
/// MEASURED `min-inline-size` of the rail.
const MIN_TRACK: f32 = 200.0;
/// MEASURED `max-inline-size` of the rail.
const MAX_TRACK: f32 = 640.0;
/// Smallest weight either rail track may carry (tree acceptance refuses 0).
const MIN_WEIGHT: f32 = 0.001;

const _: () = assert!(TRACK_HEIGHT == 2.0);
const _: () = assert!(HANDLE == 14.0);
const _: () = assert!(MIN_TRACK == 200.0);
const _: () = assert!(MAX_TRACK == 640.0);

const HANDLE_INTENTS: &[Interaction] = &[
    Interaction::Drag,
    Interaction::Focus,
    Interaction::Click,
    Interaction::Hover,
];

fn normalise(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn percent(done: f32) -> String {
    format!("{:.0}%", done * 100.0)
}

fn pin_extent(w: f32, h: f32) -> Constraints {
    Constraints {
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
    }
}

fn range_label(key: &'static str, caption: &str) -> ViewNode {
    let mut node = text(key, caption);
    node.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
    node
}

fn rail_cell(key: &'static str, background: &str) -> ViewNode {
    let mut cell = swatch(key, 0.0, TRACK_HEIGHT, Some(background), None, None);
    cell.constraints.horizontal = AxisConstraint::default();
    cell
}

fn handle_node(label: String, value: String, size: f32, live: bool) -> ViewNode {
    let mut handle = swatch(
        "handle",
        size,
        size,
        if size > 0.0 {
            Some(ACCENT_PRIMARY)
        } else {
            None
        },
        None,
        if size > 0.0 { Some(SHAPE_FULL) } else { None },
    );
    handle = handle.with_constraints(pin_extent(size, size));
    if live {
        handle = handle.interactive(Role::Button, label, HANDLE_INTENTS);
        handle.semantics.value = Some(value);
    }
    handle
}

fn slider_built(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: f32,
    read_only: bool,
) -> ViewNode {
    let label = label.into();
    let done = normalise(value);
    let rest = (1.0 - done).max(MIN_WEIGHT);
    let fill_w = done.max(MIN_WEIGHT);
    let reported = percent(done);
    let handle_size = if read_only { 0.0 } else { HANDLE };

    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let fill = rail_cell("fill", ACCENT_PRIMARY);
    let track = rail_cell("track", BORDER_SUBTLE);
    let handle = handle_node(label.clone(), reported.clone(), handle_size, !read_only);

    let mut rail = ViewNode::new(NodeKind::Grid, "rail").with_props(Props {
        columns: vec![
            TrackSize::Weight { weight: fill_w },
            TrackSize::Fixed { value: handle_size },
            TrackSize::Weight { weight: rest },
        ],
        align: Some(Align::Center),
        ..Props::default()
    });
    rail = rail.with_children(vec![fill, handle, track]);
    rail.constraints.horizontal = AxisConstraint {
        min: Some(MIN_TRACK),
        max: Some(MAX_TRACK),
        priority: 0,
    };

    let row = stack(
        "row",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![range_label("min", "0"), rail, range_label("max", "100")],
    );

    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), vec![caption, row]);
    node.semantics.label = Some(label.clone());
    node.semantics.value = Some(reported.clone());
    if read_only {
        // Handle is zero size, so focus lives on the control itself. Drag
        // and Click are the interactions a read-only slider will not honour.
        node = node.interactive(
            Role::Button,
            label,
            &[Interaction::Focus, Interaction::Hover],
        );
        node.semantics.read_only = true;
        node.semantics.value = Some(reported);
    }
    node
}

/// Default slider. `value` is normalised into `[0, 1]`. `label` is required.
pub fn slider(key: impl Into<Key>, label: impl Into<String>, value: f32) -> ViewNode {
    slider_built(key, label, value, false)
}

/// Read-only slider: handle collapsed to zero size (Carbon), not a dim.
pub fn slider_readonly(key: impl Into<Key>, label: impl Into<String>, value: f32) -> ViewNode {
    slider_built(key, label, value, true)
}

#[cfg(test)]
mod tests {
    use super::{HANDLE, MAX_TRACK, MIN_TRACK, TRACK_HEIGHT, slider, slider_readonly};
    use crate::component::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, SHAPE_FULL};
    use crate::tree::{Interaction, Role, TrackSize, ViewNode};

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

    #[test]
    fn track_is_two_px_scss_not_style_page_four() {
        let node = slider("vol", "Volume", 0.4);
        let fill = named(&node, "fill");
        let track = named(&node, "track");
        assert_eq!(fill.constraints.vertical.min, Some(TRACK_HEIGHT));
        assert_eq!(fill.constraints.vertical.max, Some(TRACK_HEIGHT));
        assert_eq!(track.constraints.vertical.min, Some(TRACK_HEIGHT));
        assert_eq!(track.constraints.vertical.max, Some(TRACK_HEIGHT));
        assert_eq!(TRACK_HEIGHT, 2.0);
        assert_ne!(TRACK_HEIGHT, 4.0);
        assert_eq!(token(fill, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(token(track, "background"), Some(BORDER_SUBTLE));
    }

    #[test]
    fn handle_is_a_button_that_declares_drag() {
        let node = slider("vol", "Volume", 0.4);
        let handle = named(&node, "handle");
        assert_eq!(handle.semantics.role, Some(Role::Button));
        assert_eq!(handle.semantics.label.as_deref(), Some("Volume"));
        assert_eq!(handle.semantics.value.as_deref(), Some("40%"));
        assert!(handle.interactions.contains(&Interaction::Drag));
        assert!(handle.interactions.contains(&Interaction::Focus));
        assert!(handle.interactions.contains(&Interaction::Click));
        assert_eq!(handle.constraints.horizontal.min, Some(HANDLE));
        assert_eq!(handle.constraints.vertical.min, Some(HANDLE));
        assert_eq!(HANDLE, 14.0);
        assert_eq!(token(handle, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(token(handle, "radius"), Some(SHAPE_FULL));
        assert_eq!(named(&node, "label").props.text.as_deref(), Some("Volume"));
        assert_eq!(named(&node, "min").props.text.as_deref(), Some("0"));
        assert_eq!(named(&node, "max").props.text.as_deref(), Some("100"));
    }

    #[test]
    fn rail_is_fluid_between_200_and_640() {
        let node = slider("vol", "Volume", 0.4);
        let rail = named(&node, "rail");
        assert_eq!(rail.constraints.horizontal.min, Some(MIN_TRACK));
        assert_eq!(rail.constraints.horizontal.max, Some(MAX_TRACK));
        assert_eq!(MIN_TRACK, 200.0);
        assert_eq!(MAX_TRACK, 640.0);
        match &rail.props.columns[..] {
            [
                TrackSize::Weight { weight: a },
                TrackSize::Fixed { value: h },
                TrackSize::Weight { weight: b },
            ] => {
                assert!((*a - 0.4).abs() < f32::EPSILON);
                assert_eq!(*h, HANDLE);
                assert!((*b - 0.6).abs() < f32::EPSILON);
            }
            other => panic!("unexpected rail columns: {other:?}"),
        }
    }

    #[test]
    fn readonly_collapses_the_handle_to_zero_and_drops_drag() {
        let node = slider_readonly("vol", "Volume", 0.5);
        assert!(node.semantics.read_only);
        assert!(!node.semantics.disabled);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Volume"));
        assert_eq!(node.semantics.value.as_deref(), Some("50%"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(!node.interactions.contains(&Interaction::Drag));
        assert!(!node.interactions.contains(&Interaction::Click));
        let handle = named(&node, "handle");
        assert_eq!(handle.constraints.horizontal.min, Some(0.0));
        assert_eq!(handle.constraints.horizontal.max, Some(0.0));
        assert_eq!(handle.constraints.vertical.min, Some(0.0));
        assert_eq!(handle.constraints.vertical.max, Some(0.0));
        assert!(handle.interactions.is_empty());
        assert_eq!(named(&node, "fill").constraints.vertical.min, Some(2.0));
    }

    #[test]
    fn value_normalises_and_is_a_percentage() {
        assert_eq!(
            slider("vol", "Volume", f32::NAN).semantics.value.as_deref(),
            Some("0%")
        );
        assert_eq!(
            slider("vol", "Volume", -1.0).semantics.value.as_deref(),
            Some("0%")
        );
        assert_eq!(
            slider("vol", "Volume", 2.0).semantics.value.as_deref(),
            Some("100%")
        );
        let full = slider("vol", "Volume", 1.0);
        assert_eq!(
            named(&full, "handle").semantics.value.as_deref(),
            Some("100%")
        );
    }
}
