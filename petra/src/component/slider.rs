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
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, TrackSize, ViewNode};

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

    /// Check C/D: the top class-two suspects in this group's brief. `fill`
    /// and `track` are exactly the childless swatch shape that petrified
    /// 0px wide for the Accordion divider — a `NodeKind::Spacer` with no
    /// children, binding a `background`, sized by nothing but a Grid
    /// `TrackSize::Weight`. Unlike the Accordion divider, `rail_cell`
    /// resets only the *horizontal* constraint to let the Grid weight
    /// drive width, while `TRACK_HEIGHT` stays pinned via an explicit
    /// `Constraints.vertical` (`min == max == 2.0`) rather than relying on
    /// `Align::Stretch` — this is the frame-level check that proves it
    /// petrifies non-zero on both axes at the min, mid and max positions,
    /// not a substitute for reading the code.
    #[test]
    fn fill_and_track_place_with_a_real_nonzero_rect_at_every_value() {
        for (label, value) in [("min", 0.0_f32), ("mid", 0.5), ("max", 1.0)] {
            let frame = petrify_lone(slider("vol", "Volume", value));
            for suffix in ["/fill", "/track", "/handle"] {
                let p = frame
                    .placements
                    .iter()
                    .find(|p| p.id.ends_with(suffix))
                    .unwrap_or_else(|| panic!("{label}: no placement ending {suffix}"));
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {suffix} placed with a degenerate rect {:?}",
                    p.rect
                );
            }
        }
    }

    /// Check C/D across every placement, min/mid/max and read-only. The
    /// read-only handle is the one placement that legitimately petrifies
    /// to a zero rect on purpose (Carbon: "read-only removes the visible
    /// handle entirely rather than merely disabling it") — `handle_node`
    /// binds it no token at all when `size == 0.0`, so `paint.paint_hash`
    /// is 0 and it declares no content to cover, the same "zero when the
    /// node draws nothing of its own" exemption `progress_indicator.rs`'s
    /// `icon-top` spacer already established. Every content-bearing
    /// placement still gets the strict check.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("min", slider("vol", "Volume", 0.0)),
            ("mid", slider("vol", "Volume", 0.5)),
            ("max", slider("vol", "Volume", 1.0)),
            ("readonly", slider_readonly("vol", "Volume", 0.5)),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            for p in &frame.placements {
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

    /// Check F: the handle declares `Focus` and is reachable on a live
    /// slider. On a read-only slider the handle carries no interactions at
    /// all (collapsed to zero, see the geometry test above), so focus
    /// moves to the control itself instead.
    #[test]
    fn handle_or_control_is_reachable_by_focus_state() {
        for (label, node, suffix, should_be_focusable) in [
            (
                "enabled handle",
                slider("vol", "Volume", 0.5),
                "/handle",
                true,
            ),
            (
                "readonly control",
                slider_readonly("vol", "Volume", 0.5),
                "/vol",
                true,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{label}: no placement ending {suffix}"));
            let reachable = focus.order().iter().any(|id| id == &placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the caption label and the min/max range labels against the
    /// page ground the slider sits on (it binds no fill of its own — see
    /// `checkbox_text_clears_aa_contrast_on_the_page_ground` in
    /// `controls.rs` for the same shape), in both themes.
    #[test]
    fn slider_text_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            let node = slider("vol", "Volume", 0.5);
            for key in ["label", "min", "max"] {
                let text_node = named(&node, key);
                let fg_name = text_node
                    .props
                    .tokens
                    .get("foreground")
                    .unwrap_or_else(|| panic!("{key} binds a foreground"));
                let opacity = text_node.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{key} at {ratio:.2}:1 against page ground fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }
}
