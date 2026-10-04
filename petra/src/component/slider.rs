//! Carbon Slider (slice-e). Track 2px T070. Two-handle omitted.
//!
//! Anatomy of the default (single-handle) slider, style page +
//! `_slider.scss`, checked against `30-slider.png` on 2026-09-04:
//! 1. Label — required, above the rail ([`Role::Button`] lives on the
//!    handle, FR-058).
//! 2. Track — unfilled rail, [`BORDER_SUBTLE`], height **2px** (SCSS;
//!    style-page 4px is design intent, T070 prefers SCSS). Accepts a
//!    press: Carbon jumps the value to wherever the track is clicked and
//!    drags from there, so the rail declares [`Interaction::Drag`] too.
//! 3. Min value — leftmost range label (`"0"`).
//! 4. Handle — 14×14 circle (`shape.corner-full`, via
//!    [`crate::token::CornerRole::Pill`]) in
//!    [`LAYER_SELECTED_INVERSE`]
//!    (SCSS `background: $layer-selected-inverse`; the style page's
//!    `$icon-primary` is the documentation side of a T070 disagreement),
//!    [`Interaction::Drag`] plus Focus, Click and Hover. The filled track
//!    is the same tone.
//! 5. Max value — rightmost range label (`"100"`).
//! 6. Number input — 64 wide, 40 tall, showing the value as an integer on
//!    the `0`–`100` range the labels name, in Carbon's field chrome
//!    (`_slider.scss` `.cds--slider-text-input`, style page "number input
//!    40px tall × 64px wide"). Editable: the page that owns the slider hears
//!    its keystrokes the way it hears a text field's.
//!
//! Range (two-handle) is omitted. Read-only collapses the handle to zero
//! size, not a dim, and drops the number input's editing intents.
//!
//! # Why the fill is not the accent
//!
//! It was, until 2026-09-04, because the accent is the one blue this library
//! spends and a filled track looked like a place to spend it. Carbon's g100
//! filled track and handle are white (`$layer-selected-inverse`), its white
//! theme's are near-black, and the accent appears only under focus
//! (`.cds--slider__thumb:focus ~ .cds--slider__filled-track {
//! background: $border-interactive }`). The inverse tone is a luminance
//! step against the rail nobody can miss, which is the property a
//! colour-blind reader needs and a blue-on-grey track has less of.

use super::field::{EDITABLE_TEXT_INTENTS, bind_field_chrome};
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_SELECTED_INVERSE, SIZE_MD, SPACING_03, SPACING_05, TEXT_MUTED,
    TEXT_PRIMARY, TYPOGRAPHY_BODY, t,
};
use crate::frame::PetrifiedFrame;
use crate::geom::{Align, Axis, Point};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    AxisConstraint, Behaviour, Constraints, FocusFigure, Intent, Interaction, Key, NodeKind, Phase,
    Props, Role, TrackSize, ViewNode,
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

/// SOURCED style page: the paired number input is 64 wide × 40 tall.
const INPUT_WIDTH: f32 = 64.0;

const _: () = assert!(TRACK_HEIGHT == 2.0);
const _: () = assert!(HANDLE == 14.0);
const _: () = assert!(MIN_TRACK == 200.0);
const _: () = assert!(MAX_TRACK == 640.0);
const _: () = assert!(INPUT_WIDTH == 64.0);
const _: () = assert!(SIZE_MD == 40.0);

const HANDLE_INTENTS: &[Interaction] = &[
    Interaction::Drag,
    Interaction::Focus,
    Interaction::Click,
    Interaction::Hover,
];

/// What the rail answers to: a press anywhere on the track starts a drag
/// from that point (Carbon: "clicking anywhere on the track jumps the value
/// to that point"). No `Hover`: the rail has no hover picture, and a rail
/// that were hovered would be reported to the page as a pointer position
/// it never asked for.
const RAIL_INTENTS: &[Interaction] = &[Interaction::Drag, Interaction::Click];

/// Track press jumps the value. Spec 010 names this the whole of `OnPress`.
const ADJUSTS_ON_PRESS: Behaviour = Behaviour {
    intent: Intent::Adjust,
    phase: Phase::OnPress,
};

/// Live drag. Spec 010 names this the whole of `OnChange`.
const ADJUSTS_ON_CHANGE: Behaviour = Behaviour {
    intent: Intent::Adjust,
    phase: Phase::OnChange,
};

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

/// The integer the number input shows for `done`, on the `0`–`100` range
/// the min and max labels name.
#[must_use]
pub fn slider_input_text(value: f32) -> String {
    format!("{:.0}", normalise(value) * 100.0)
}

/// The normalised value an integer typed into the number input names:
/// `"40"` is `0.4`. Digits only; anything else, or nothing, is `None`, so a
/// page can leave the value where it was rather than jump to zero on a
/// stray keystroke.
#[must_use]
pub fn slider_value_of_input(text: &str) -> Option<f32> {
    let trimmed = text.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = trimmed.parse().ok()?;
    #[allow(clippy::cast_precision_loss)]
    Some(normalise(n as f32 / 100.0))
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
            Some(LAYER_SELECTED_INVERSE)
        } else {
            None
        },
        None,
        // FR-022: the handle is a disc at whatever size it ships, so it
        // says `CornerRole::Pill` rather than leaning on the half-edge
        // clause, which `Floating`'s 8 clears at today's 14 units and would
        // stop clearing the day the handle grew past 16.
        if size > 0.0 {
            Some(corner_for(CornerRole::Pill, size))
        } else {
            None
        },
    );
    handle = handle.with_constraints(pin_extent(size, size));
    if live {
        handle = handle
            .interactive(Role::Button, label, HANDLE_INTENTS)
            .with_behaviour(ADJUSTS_ON_CHANGE)
            .owning_its_text();
        handle.semantics.value = Some(value);
    }
    handle
}

/// The paired number input: Carbon's field chrome, 64 × 40, showing the
/// value as an integer. Editable unless the slider is read-only.
fn input_node(label: &str, value: f32, read_only: bool) -> ViewNode {
    let mut props = Props {
        text: Some(slider_input_text(value)),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_PRIMARY));
    bind_field_chrome(&mut props);
    let intents: &[Interaction] = if read_only {
        &[Interaction::Focus]
    } else {
        EDITABLE_TEXT_INTENTS
    };
    let mut node = ViewNode::new(NodeKind::Input, "input")
        .with_props(props)
        .with_constraints(pin_extent(INPUT_WIDTH, SIZE_MD))
        .interactive(Role::TextInput, format!("{label} value"), intents)
        // A well a person types into: `Sides`, the same as `component::field`.
        .with_focus_figure(FocusFigure::Sides);
    node.semantics.read_only = read_only;
    node
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

    let fill = rail_cell("fill", LAYER_SELECTED_INVERSE);
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
    if !read_only {
        // A press on the track, not only on the 14-unit handle, starts the
        // gesture. The handle is painted after the rail, so a press on the
        // handle still names the handle (`input::hit_test` walks paint
        // order back to front); the rail catches the rest of the track.
        rail = rail
            .interactive(Role::Pane, format!("{label} track"), RAIL_INTENTS)
            .with_behaviour(ADJUSTS_ON_PRESS);
    }

    // SOURCED style page: range labels carry `margin-right: 16px`, and the
    // Carbon shot has 16 between "0" and the rail and between "100" and the
    // number input.
    let row = stack(
        "row",
        Axis::Horizontal,
        Some(SPACING_05),
        vec![
            range_label("min", "0"),
            rail,
            range_label("max", "100"),
            input_node(&label, done, read_only),
        ],
    );

    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), vec![caption, row]);
    node.semantics.label = Some(label.clone());
    node.semantics.value = Some(reported.clone());
    if read_only {
        // Handle is zero size, so focus lives on the control itself. Drag
        // and Click are the interactions a read-only slider will not honour.
        node = node
            .interactive(
                Role::Button,
                label,
                &[Interaction::Focus, Interaction::Hover],
            )
            .owning_its_text();
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

/// The value a pointer at `pos` names on the slider whose placement `node`
/// belongs to, in `[0, 1]` — or `None` when `node` is not inside a slider
/// this frame placed.
///
/// This is the half of a drag the engine cannot do for an application. A
/// routed `PointerMoved` carries a window position and the id of the node
/// holding the capture (the handle); turning that into a value needs the
/// *rail's* rect, which only the frame has and only this module knows how
/// to find from a handle's id. The handle's centre travels from the rail's
/// left edge plus half a handle to its right edge minus half a handle, so
/// the mapping puts the handle's centre under the pointer at every value
/// rather than only at the ends.
///
/// `node` may be any placement id inside the slider — the handle, the rail,
/// a track cell — since a press can land on any of them.
#[must_use]
pub fn slider_value_at(frame: &PetrifiedFrame, node: &str, pos: Point) -> Option<f32> {
    let rail_id = rail_of(node)?;
    let rail = frame.placement(rail_id)?.rect;
    let travel = rail.w - HANDLE;
    if travel <= 0.0 {
        return None;
    }
    Some(normalise((pos.x - rail.x - HANDLE / 2.0) / travel))
}

/// The canonical id of the rail enclosing `node`, or `None` when `node` is
/// not under a `row/rail` this module built.
fn rail_of(node: &str) -> Option<&str> {
    const RAIL: &str = "/row/rail";
    let end = node.find(RAIL)? + RAIL.len();
    if node.len() == end || node.as_bytes()[end] == b'/' {
        Some(&node[..end])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HANDLE, INPUT_WIDTH, MAX_TRACK, MIN_TRACK, TRACK_HEIGHT, slider, slider_input_text,
        slider_readonly, slider_value_at, slider_value_of_input,
    };
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::{
        ACCENT_PRIMARY, BORDER_STRONG, BORDER_SUBTLE, LAYER_SELECTED_INVERSE, SIZE_MD, SPACING_05,
        SURFACE_RAISED,
    };

    use crate::geom::Point;

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::token::{CornerRole, corner_for};
    use crate::tree::{Interaction, NodeKind, Role, TrackSize, ViewNode};

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
        // SCSS: `.cds--slider__filled-track { background:
        // $layer-selected-inverse }`. The accent is the *focused* fill
        // (`$border-interactive`) and was wrong at rest; asserted away by
        // name so it cannot come back as a "fix".
        assert_eq!(token(fill, "background"), Some(LAYER_SELECTED_INVERSE));
        assert_ne!(token(fill, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(token(track, "background"), Some(BORDER_SUBTLE));
    }

    /// The rail accepts a press: Carbon jumps the value to wherever the
    /// track is clicked and drags from there. Without `Drag` on the rail a
    /// press on the track routed nowhere and only the 14-unit handle was
    /// a place to start.
    #[test]
    fn the_rail_accepts_a_press_so_a_click_on_the_track_starts_a_drag() {
        let node = slider("vol", "Volume", 0.4);
        let rail = named(&node, "rail");
        assert!(rail.interactions.contains(&Interaction::Drag));
        assert!(rail.interactions.contains(&Interaction::Click));
        assert!(
            !rail.interactions.contains(&Interaction::Hover),
            "a hovered rail would be reported to the page as a position it \
             never asked for"
        );
        assert_eq!(rail.semantics.label.as_deref(), Some("Volume track"));
        let readonly = slider_readonly("vol", "Volume", 0.4);
        assert!(
            named(&readonly, "rail").interactions.is_empty(),
            "a read-only slider's track does not start a drag"
        );
    }

    /// The paired number input: 64 × 40, Carbon's field chrome, the value
    /// as an integer on the labels' 0–100 range, editable, and 16 from the
    /// max label (the row's spacing, style page `margin-right: 16px`).
    #[test]
    fn the_number_input_shows_the_value_as_an_integer_in_a_carbon_well() {
        let node = slider("vol", "Volume", 0.4);
        let row = named(&node, "row");
        let keys: Vec<&str> = row.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["min", "rail", "max", "input"]);
        assert_eq!(
            row.props.spacing.as_ref().map(TokenName::as_str),
            Some(SPACING_05)
        );
        let input = named(&node, "input");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.props.text.as_deref(), Some("40"));
        assert_eq!(input.constraints.horizontal.min, Some(INPUT_WIDTH));
        assert_eq!(input.constraints.horizontal.max, Some(INPUT_WIDTH));
        assert_eq!(input.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(input.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(token(input, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(input, "border-bottom"), Some(BORDER_STRONG));
        assert_eq!(token(input, "border"), None);
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.semantics.label.as_deref(), Some("Volume value"));
        assert!(input.interactions.contains(&Interaction::TextEdit));
        assert!(input.interactions.contains(&Interaction::Click));
        assert!(!input.semantics.read_only);

        let readonly_slider = slider_readonly("vol", "Volume", 0.4);
        let readonly = named(&readonly_slider, "input");
        assert!(readonly.semantics.read_only);
        assert!(!readonly.interactions.contains(&Interaction::TextEdit));
        assert_eq!(readonly.props.text.as_deref(), Some("40"));
    }

    /// The integer the input shows and the value it names round-trip, and
    /// a non-number names nothing rather than zero.
    #[test]
    fn the_input_text_and_the_value_round_trip() {
        assert_eq!(slider_input_text(0.4), "40");
        assert_eq!(slider_input_text(0.0), "0");
        assert_eq!(slider_input_text(1.0), "100");
        assert_eq!(slider_input_text(f32::NAN), "0");
        assert_eq!(slider_value_of_input("40"), Some(0.4));
        assert_eq!(slider_value_of_input("0"), Some(0.0));
        assert_eq!(
            slider_value_of_input("250"),
            Some(1.0),
            "clamped to the range"
        );
        assert_eq!(
            slider_value_of_input(""),
            None,
            "nothing typed names nothing"
        );
        assert_eq!(slider_value_of_input("4x"), None);
        assert_eq!(slider_value_of_input("-4"), None);
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
        // SCSS: `.cds--slider__thumb { background: $layer-selected-inverse }`.
        assert_eq!(token(handle, "background"), Some(LAYER_SELECTED_INVERSE));
        assert_eq!(
            token(handle, "radius"),
            Some(corner_for(CornerRole::Pill, HANDLE))
        );
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
                    assert_fits_parent(label, p, &frame.placements[parent_idx]);
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

    /// A drag is a window position and a captured node id; the value is
    /// the pointer's position along the rail, handle-centred, from any
    /// node inside the slider.
    #[test]
    fn a_pointer_position_maps_to_a_value_along_the_rail_from_any_node_inside() {
        let frame = petrify_lone(slider("vol", "Volume", 0.4));
        let rail_placement = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/vol/row/rail"))
            .expect("the rail is placed");
        let rail = rail_placement.rect;
        let rail_id = rail_placement.id.clone();
        let handle = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/rail/handle"))
            .expect("the handle is placed")
            .id
            .clone();
        let at = |x: f32| slider_value_at(&frame, &handle, Point::new(x, rail.y));
        assert_eq!(at(rail.x + HANDLE / 2.0), Some(0.0), "left end");
        assert_eq!(at(rail.right() - HANDLE / 2.0), Some(1.0), "right end");
        let mid = at(rail.x + rail.w / 2.0).unwrap();
        assert!((mid - 0.5).abs() < 1e-3, "middle is 0.5, got {mid}");
        assert_eq!(at(rail.x - 100.0), Some(0.0), "clamped below");
        assert_eq!(at(rail.right() + 100.0), Some(1.0), "clamped above");
        assert!(
            at(rail.right() - HANDLE / 2.0) > at(rail.x + rail.w / 2.0),
            "further right is a larger value"
        );
        // The rail itself names the same slider.
        assert_eq!(
            slider_value_at(&frame, &rail_id, Point::new(rail.x + rail.w / 2.0, 0.0)),
            at(rail.x + rail.w / 2.0)
        );
        assert_eq!(
            slider_value_at(&frame, "/root/vol/label", Point::ZERO),
            None,
            "the caption is not inside the rail"
        );
        assert_eq!(
            slider_value_at(&frame, "/root/vol/row/railway", Point::ZERO),
            None,
            "a key that merely starts with `rail` is not the rail"
        );
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
