//! Gate C1-5, the real acceptance: a tree built from every shipped
//! component passes `semantic::audit` with zero findings.
//!
//! This is not a smoke test that the tree petrifies — every layout test in
//! the workspace already proves that much for a plain `ViewNode` tree. It
//! is the specific claim FR-058 makes: that a surface an author composes
//! entirely out of this module's thirteen names cannot end up with an
//! unlabelled interactive node or a colour-only status, because the audit
//! that would catch either one finds nothing to report.

use crate::frame::{TransitionActivity, Viewport, petrify};
use crate::geom::{Axis, Size};
use crate::semantic::{audit, project};
use crate::testing::{Harness, validated_with};
use crate::token::{StatusShape, StatusToken, ThemeMode, TokenName, standard_vocabulary};
use crate::tree::{NodeKind, Props, Registry, ViewNode};

use super::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, TEXT_ON_ACCENT};
use super::{
    MAX_LAYER_DEPTH, button, checkbox, field, heading, layer_tokens, list_row, on_layer,
    primary_button, progress, radio, section, status, tab, tab_bar, text, toggle,
};

const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

/// One tree exercising all thirteen shipped components, each at least once,
/// nested under a plain `Stack` root (a primitive, per gate C1-10 — the
/// library composes from the primitives, it does not replace the root).
fn full_gallery() -> ViewNode {
    let ok = StatusToken::new(
        TokenName::new("status.ok").unwrap(),
        StatusShape::Circle,
        "OK",
    )
    .unwrap();
    let degraded = StatusToken::new(
        TokenName::new("status.degraded").unwrap(),
        StatusShape::Triangle,
        "Degraded",
    )
    .unwrap();

    let controls = section(
        "controls",
        "Controls",
        vec![
            checkbox("check", "Checked", true),
            radio("radio", "Chosen", false),
            toggle("toggle", "On", true),
            primary_button("primary", "Save"),
            button("secondary", "Cancel"),
            field("name", "Fiber name"),
        ],
    );

    let tabs = section(
        "tabs",
        "Tabs",
        vec![tab_bar(
            "strip",
            vec![
                tab("t-fibers", "Fibers", true),
                tab("t-trace", "Trace", false),
            ],
        )],
    );

    let readouts = section(
        "readouts",
        "Readouts",
        vec![
            progress("rebuild", "Rebuild", 0.62),
            status("s-ok", &ok),
            status("s-degraded", &degraded),
        ],
    );

    let list = section(
        "list",
        "List",
        vec![
            list_row("row-0", "row 0", true),
            list_row("row-1", "row 1", false),
        ],
    );

    let mut root = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
        axis: Some(Axis::Vertical),
        ..Props::default()
    });
    root = root
        .child(heading("title", "Component gallery"))
        .child(text("intro", "every shipped component, once"))
        .child(controls)
        .child(tabs)
        .child(readouts)
        .child(list);
    root
}

#[test]
fn every_component_in_one_tree_passes_the_audit_with_zero_findings() {
    let tree = full_gallery();
    // The component library binds real design-token names (`spacing-04`,
    // `text.primary`, ...), so — unlike most layout fixtures, which name no
    // token at all — this tree needs a `Registry` that actually declares the
    // shipped vocabulary; `crate::testing::validated`'s empty `Registry::new()`
    // would refuse every one of them as unknown (`Violation::UnknownTokenRef`).
    let registry = Registry::with_vocabulary(standard_vocabulary());
    let mut harness = Harness::new();
    let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
    harness.scale = viewport.scale;
    let frame = petrify(
        1,
        validated_with(&tree, &registry),
        &mut harness.ctx(),
        viewport,
        TransitionActivity::default(),
    );
    let projected = project(&frame).expect("a petrified frame with a root has a projection");
    let violations = audit(&projected, &frame);
    assert!(
        violations.is_empty(),
        "a tree built only from the component library must pass the audit clean, found: {violations:#?}"
    );
}

/// [`button`], [`checkbox`], [`radio`], [`toggle`], [`tab`], [`field`], and
/// [`list_row`] all declare at least one interaction; every one of them
/// must carry a role and a non-empty label; this is FR-058 measured through
/// the same audit rule the sabotage run (gate C1-12) targets.
#[test]
fn every_interactive_component_declares_a_role_and_a_label() {
    let nodes = [
        button("b", "Save"),
        checkbox("c", "Checked", true),
        radio("r", "Chosen", false),
        toggle("t", "On", true),
        tab("tb", "Fibers", true),
        field("f", "Fiber name"),
        list_row("l", "row", false),
    ];
    for node in nodes {
        assert!(
            node.is_interactive(),
            "{:?} declares no interaction",
            node.key
        );
        assert!(
            node.semantics.role.is_some(),
            "{:?} is interactive but carries no role",
            node.key
        );
        assert!(
            node.semantics
                .label
                .as_ref()
                .is_some_and(|label| !label.trim().is_empty()),
            "{:?} is interactive but carries no non-empty label",
            node.key
        );
    }
}

/// [`status`] cannot be built without a shape and a non-empty text channel,
/// because its only parameter is a [`StatusToken`] and `StatusToken::new`
/// already refuses one with empty text (gate C1-6). This test measures the
/// component's own output rather than `StatusToken` a second time: the
/// projected node's role and label are what the audit actually reads.
#[test]
fn status_always_carries_role_and_label_from_its_status_token() {
    let token = StatusToken::new(
        TokenName::new("status.down").unwrap(),
        StatusShape::Square,
        "Down",
    )
    .unwrap();
    let node = status("s", &token);
    assert_eq!(node.semantics.role, Some(crate::tree::Role::Status));
    assert_eq!(node.semantics.label.as_deref(), Some("Down"));
}

/// A control's `selected` state is declared in `Semantics`, never carried by
/// its fill colour alone (gate C1-6's sibling claim, for the binary
/// controls rather than `status`).
#[test]
fn selection_is_declared_state_not_only_a_fill_colour() {
    assert!(checkbox("c", "Checked", true).semantics.selected);
    assert!(!checkbox("c", "Unchecked", false).semantics.selected);
    assert!(toggle("t", "On", true).semantics.selected);
    assert!(tab("tb", "Fibers", true).semantics.selected);
    assert!(list_row("l", "row", true).semantics.selected);
}

#[test]
fn marker_and_label_share_a_midline() {
    use crate::geom::Align;
    assert_eq!(
        checkbox("c", "Checked", false).props.align,
        Some(Align::Center)
    );
    assert_eq!(radio("r", "Chosen", false).props.align, Some(Align::Center));
    assert_eq!(toggle("t", "On", false).props.align, Some(Align::Center));
    let down = StatusToken::new(
        TokenName::new("status.down").unwrap(),
        StatusShape::Square,
        "Down",
    )
    .unwrap();
    assert_eq!(status("s", &down).props.align, Some(Align::Center));

    // Props.align is the declaration. The row is 12-vs-20 (10-vs-20 for
    // status); Start would place the marker ~4 units above the words.
    // Petrify and compare the placed midlines so a layout that ignores
    // align cannot stay green.
    let mid = |suffix: &str, frame: &crate::frame::PetrifiedFrame| {
        let rect = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("{suffix} is missing from the petrified frame"))
            .rect;
        rect.y + rect.h * 0.5
    };
    let cases: &[(&str, ViewNode, &str, &str)] = &[
        (
            "checkbox",
            checkbox("c", "Checked", false),
            "/root/c/box",
            "/root/c/label",
        ),
        (
            "radio",
            radio("r", "Chosen", false),
            "/root/r/box",
            "/root/r/label",
        ),
        (
            "toggle",
            toggle("t", "On", false),
            "/root/t/track",
            "/root/t/label",
        ),
        ("status", status("s", &down), "/root/s/dot", "/root/s/label"),
    ];
    for (name, node, marker, label) in cases {
        let frame = petrify_lone(node.clone());
        let marker_mid = mid(marker, &frame);
        let label_mid = mid(label, &frame);
        assert!(
            (marker_mid - label_mid).abs() < 0.5,
            "{name}: marker midline {marker_mid} vs label midline {label_mid}"
        );
    }
}

/// `size-md` is 40 in the ramp; a field that spelled 40.0 at the call site
/// would drift the first time the ramp moved. The constraint is the
/// declaration; the placed rect is the claim.
#[test]
fn a_field_is_as_tall_as_size_md() {
    assert_eq!(super::tokens::SIZE_MD, 40.0);
    let node = field("name", "Fiber name");
    assert_eq!(node.constraints.vertical.min, Some(super::tokens::SIZE_MD));
    let theme = crate::token::light();
    let value = theme
        .value(&TokenName::new("size-md").unwrap())
        .expect("size-md is in the vocabulary");
    match value {
        crate::token::TokenValue::Spacing(units) => {
            assert_eq!(*units, super::tokens::SIZE_MD);
        }
        other => panic!("size-md should be a spacing value, got {other:?}"),
    }
    let frame = petrify_lone(node);
    let placed = frame
        .placements
        .iter()
        .find(|p| p.id.ends_with("/root/name"))
        .expect("the field is missing from the petrified frame");
    assert_eq!(placed.rect.h, super::tokens::SIZE_MD);
}

/// A component with more than one visual part composes it from a primitive
/// container (`NodeKind::Stack` or `NodeKind::Grid`), never by inventing a
/// new node kind — gate C1-10, checked structurally rather than by reading
/// the source.
#[test]
fn multi_part_components_are_built_from_primitive_container_kinds() {
    assert_eq!(button("b", "Save").kind, NodeKind::Stack);
    assert_eq!(section("s", "T", vec![]).kind, NodeKind::Stack);
    assert_eq!(progress("p", "Rebuild", 0.5).kind, NodeKind::Grid);
    assert_eq!(tab_bar("tb", vec![]).kind, NodeKind::Stack);
}

/// Walk `node` and everything under it, handing `visit` each node's key path
/// and its bound tokens.
fn walk(node: &ViewNode, path: &str, visit: &mut impl FnMut(&str, &crate::tree::Props)) {
    let here = if path.is_empty() {
        node.key.as_str().to_owned()
    } else {
        format!("{path}/{}", node.key.as_str())
    };
    visit(&here, &node.props);
    for child in &node.children {
        walk(child, &here, visit);
    }
}

/// **The regression guard for the 2026-08-25 design pass.** Exactly the set
/// of nodes below draws an edge, every one of them in the border tone, and
/// nothing else in the library draws one at all.
///
/// # The rule, stated once
///
/// **Containers get a tone. Controls get an edge.**
///
/// A card, a well, a progress rail, an image frame and a list strip are
/// separated from what is behind them by a *fill* — one layer of the shipped
/// set sitting on another, or an elevation shadow. Every one of those also
/// drew a `text.muted` outline before this pass: 10.73:1 against the fill it
/// was separating, decoration over a shape that already had a boundary, and
/// the reason the page read as a wireframe. Those are gone.
///
/// A `button`, a `field`, a checkbox box, a radio box and a toggle track
/// keep one, for two different reasons that both come down to a
/// measurement:
///
/// - The binary controls' marks have **no fill at all** when they are off.
///   `box_control` passes `None` as the background of an unchecked box, so
///   taking the outline away does not quieten the control, it deletes it.
/// - `field` has a fill and it is not enough. [`on_layer`] steps it one
///   layer ahead of the card it sits on, which measures **1.26:1 in dark and
///   1.12:1 in light** against WCAG 2.1 SC 1.4.11's 3:1 floor. A field is a
///   place to *put* something rather than a thing to press, and it carries
///   no label of its own until somebody types one — an empty well with no
///   boundary does not read as an input at all.
///
/// **`button` and `primary_button` were on this list and came off it.** They
/// are elevated instead: every button casts `shadow.raised`, which is a
/// weaker boundary by measurement (~1.6:1 in light against the border's
/// 3.34:1) and is the call Material 3 and Apple's HIG both make for a filled
/// button. `labelled`'s doc carries the full table and the reasoning; this
/// test is only the pin. If buttons ever draw an edge again, this fails, and
/// that is the intended behaviour rather than an inconvenience.
///
/// `list_row` and `tab` are the deliberate omission: each is one segment of
/// a strip rather than a free-standing control, the strip is what identifies
/// it, and a tab bar of five outlined boxes is a wireframe again.
///
/// # Why one test and not two
///
/// Each half alone is defeatable. A future edit that puts a border back on
/// the card would bind the right *token* and pass a tone check. An edit that
/// repaints the checkbox in `text.muted` would leave the same *set* of nodes
/// bordered and pass a membership check. Both halves are asserted here,
/// against an exact set rather than an allow-list, so adding a border
/// anywhere fails just as loudly as removing one.
///
/// # What it cannot reach
///
/// Only what [`full_gallery`] composes, which is why that fixture is the
/// all-thirteen tree rather than a hand-picked subset. And contrast is not
/// the only thing that makes an edge shout: stroke width lives in the
/// painter (`gorgon-petra-egui`'s `device_snapped_width`), not here. A
/// capture owns that.
#[test]
fn containers_take_a_tone_and_controls_take_an_edge() {
    /// Every node in [`full_gallery`] that may draw a border, by the key
    /// path it appears at. An exact set: a node missing from here that draws
    /// one fails, and a node listed here that stops drawing one fails too.
    const DRAWS_AN_EDGE: [&str; 4] = [
        // `field`: an empty well with no boundary does not read as a place
        // to type. See `field`'s own doc for why it keeps one when `button`
        // does not.
        "root/controls/name",
        // The binary controls' marks: no fill at all when they are off.
        "root/controls/check/box",
        "root/controls/radio/box",
        "root/controls/toggle/track",
    ];

    // `status` is not on that list, and the omission is measured rather than
    // an oversight: a status mark carries a `silhouette` and a filled
    // status colour, so its shape *is* its boundary and every one of the
    // three shipped colours already clears 3:1 on the layers it can be
    // painted on (`shipped.rs`'s own status gates). Adding an outline would
    // put a grey ring around the one channel that is deliberately not grey.
    // `primary_button` is absent for the reason in this test's doc; its
    // label child is a `Text` node and draws no box at all.

    let mut bordered: Vec<String> = Vec::new();
    walk(&full_gallery(), "", &mut |path, props| {
        let Some(token) = props.tokens.get("border") else {
            return;
        };
        assert_eq!(
            token.as_str(),
            BORDER_SUBTLE,
            "{path} draws its edge in `{}`. A border binds the border tone; \
             binding a text tone is how this library came to look like a \
             wireframe, and it fails no contrast floor on the way.",
            token.as_str()
        );
        bordered.push(path.to_owned());
    });
    bordered.sort();

    let mut expected: Vec<String> = DRAWS_AN_EDGE.iter().map(|s| (*s).to_owned()).collect();
    expected.sort();
    assert_eq!(
        bordered, expected,
        "the set of nodes drawing an edge changed. Containers take a tone \
         and controls take an edge -- read this test's doc before widening \
         the list, and if a node genuinely needs an edge, say which of the \
         two measured reasons applies to it."
    );
}

/// [`on_layer`] seats a control one tone ahead of the ground it is placed
/// on, and a control that means to disappear takes the ground's own tone.
///
/// Both halves are checked against the *shipped colours*, not only against
/// the token names: two names that resolved to one grey would pass a name
/// comparison and paint an invisible control, which is precisely the failure
/// the four-layer set was extended to fix.
#[test]
fn on_layer_steps_a_control_one_tone_ahead_of_its_ground() {
    use crate::token::{LAYER_TOKENS, TokenValue, dark, light};

    for depth in 0..=MAX_LAYER_DEPTH {
        let raised = on_layer(button("b", "Save"), depth);
        let flush = on_layer(tab("t", "Trace", false), depth);

        assert_eq!(
            raised.props.tokens.get("background").map(TokenName::as_str),
            Some(LAYER_TOKENS[depth + 1]),
            "a raised control seated on layer {depth} must take the tone one \
             step ahead of it"
        );
        assert_eq!(
            flush.props.tokens.get("background").map(TokenName::as_str),
            Some(LAYER_TOKENS[depth]),
            "a control that means to sit flush with its ground must take the \
             ground's own tone"
        );

        for (label, theme) in [("light", light()), ("dark", dark())] {
            let colour = |token: &str| match theme.value(&TokenName::new(token).unwrap()) {
                Some(TokenValue::Color(c)) => *c,
                other => panic!("{token} is not a colour: {other:?}"),
            };
            assert_ne!(
                colour(LAYER_TOKENS[depth]),
                colour(LAYER_TOKENS[depth + 1]),
                "{label}: layers {depth} and {} resolve to the same colour, \
                 so a control seated here has no edge at all — the tonal cue \
                 is the whole depth cue now that the borders are gone",
                depth + 1
            );
        }
    }
}

/// A seat deeper than the layer set can express is clamped rather than
/// allowed to run off the end of it.
///
/// The end of `LAYER_TOKENS` is `surface.layer-three`, so an unclamped
/// `depth + 1` would either panic on the index or — worse, if someone
/// "fixed" it with a saturating index — resolve the control and its ground
/// to the same grey. That second failure is silent: the token resolves, the
/// painter reports a fill, and the control is invisible.
#[test]
fn a_seat_deeper_than_the_ramp_is_clamped_and_still_has_a_step_in_it() {
    use crate::token::LAYER_TOKENS;

    let deep = on_layer(button("b", "Save"), MAX_LAYER_DEPTH + 40);
    assert_eq!(
        deep.props.tokens.get("background").map(TokenName::as_str),
        Some(LAYER_TOKENS[MAX_LAYER_DEPTH + 1]),
        "an over-deep seat must land on the deepest step the ramp can \
         express, not past the end of it"
    );
}

/// [`on_layer`] rewrites the two tones it is about and nothing else, and it
/// never adds an edge.
///
/// The accent case is the one with a measurement behind it: `shipped.rs`'s
/// `DEEPEST_ACCENT_LAYER` records that dark's accent is under the 3:1 fill
/// floor on `surface.layer-three`, so a re-seating pass that treated an
/// accent fill as "a background, therefore mine to move" could walk a
/// primary button onto a ground its own colour cannot carry.
#[test]
fn on_layer_leaves_every_other_fill_alone_and_never_touches_an_edge() {
    for depth in 0..=MAX_LAYER_DEPTH {
        let primary = on_layer(primary_button("p", "Save"), depth);
        assert_eq!(
            primary
                .props
                .tokens
                .get("background")
                .map(TokenName::as_str),
            Some(ACCENT_PRIMARY),
            "an accent fill is a deliberate choice by whoever bound it, not a \
             surface tone for this pass to step"
        );

        for (before, after) in [
            (button("b", "Save"), on_layer(button("b", "Save"), depth)),
            (
                field("f", "Fiber name"),
                on_layer(field("f", "Fiber name"), depth),
            ),
            (
                tab("t", "Trace", false),
                on_layer(tab("t", "Trace", false), depth),
            ),
            (
                list_row("l", "row", true),
                on_layer(list_row("l", "row", true), depth),
            ),
            (primary_button("p", "Save"), primary),
        ] {
            assert_eq!(
                before.props.tokens.get("border"),
                after.props.tokens.get("border"),
                "{:?}'s edge changed when it was re-seated. Which components \
                 draw an edge, and in what tone, is decided once by the \
                 component (see `containers_take_a_tone_and_controls_take_an_edge`) \
                 and is not a depth question. An operator that added one here \
                 would put an outline on a card; one that removed it would take \
                 the boundary off every control on a card.",
                after.key
            );
        }
    }
}

/// The primary button spends the accent, and its label is the ink that goes
/// with it.
///
/// `text.on-accent` is not a stylistic pick: `shipped.rs` measures both
/// shipped text tones on both accents at 1.95:1 to 3.48:1, all four under
/// the 4.5:1 AA floor. A caller — or a later edit — that "simplifies" the
/// label back to `text.primary` fails here.
#[test]
fn the_primary_button_spends_the_accent_and_the_ink_that_goes_with_it() {
    let node = primary_button("p", "Save");
    assert_eq!(
        node.props.tokens.get("background").map(TokenName::as_str),
        Some(ACCENT_PRIMARY)
    );
    assert!(
        node.props.tokens.contains_key("shadow"),
        "a button sits *on* the surface, and depth is what says so"
    );
    assert!(
        !node.props.tokens.contains_key("border"),
        "the accent is the emphasis; an edge on top of it is the wireframe again"
    );

    let label = node
        .children
        .first()
        .expect("primary_button carries its label as a child node");
    assert_eq!(
        label.props.tokens.get("foreground").map(TokenName::as_str),
        Some(TEXT_ON_ACCENT),
        "neither shipped text tone clears AA on either accent — see \
         `ON_ACCENT_TOKEN`'s own measurements"
    );

    // The plain button is elevated too -- the two are told apart by their
    // fill, not by their depth, because that is where the contrast is. What
    // it must not do is pick up the accent.
    let plain = button("b", "Cancel");
    assert!(
        plain.props.tokens.contains_key("shadow"),
        "every button lifts off the surface; only the fill ranks them"
    );
    assert_ne!(
        plain.props.tokens.get("background").map(TokenName::as_str),
        Some(ACCENT_PRIMARY),
        "an accent that appears on every button is not an accent"
    );
    assert!(
        !plain.props.tokens.contains_key("border"),
        "a default button is separated by tone and depth, not by an outline"
    );
}

/// Keeps [`full_gallery`]'s two status calls pinned to distinct shapes, so
/// the acceptance test above is not silently exercising `Circle` twice.
#[test]
fn the_fixture_status_tokens_are_shaped_differently() {
    let a = StatusToken::new(
        TokenName::new("status.ok").unwrap(),
        StatusShape::Circle,
        "OK",
    )
    .unwrap();
    let b = StatusToken::new(
        TokenName::new("status.degraded").unwrap(),
        StatusShape::Triangle,
        "Degraded",
    )
    .unwrap();
    assert_ne!(a.shape(), b.shape());
}

/// Petrify a single component under a vertical stack root.
fn petrify_lone(child: ViewNode) -> crate::frame::PetrifiedFrame {
    let root = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(child);
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

/// Petrify a lone [`progress`] at `value` and return the widths the frame
/// actually drew: `(bar, fill, track)`.
///
/// The tests above read `ViewNode` props, which is exactly how a zero-width
/// fill shipped: the column weights were right, the two cells were pinned to
/// zero extent, and nothing in this file looked at a rect. This helper is the
/// geometry channel — it runs the same `petrify` the acceptance test above
/// runs and reports placed rects, not declarations.
fn bar_widths(value: f32) -> (f32, f32, f32) {
    let root = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(progress("bar", "Rebuild", value));
    let registry = Registry::with_vocabulary(standard_vocabulary());
    let mut harness = Harness::new();
    let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
    harness.scale = viewport.scale;
    let frame = petrify(
        1,
        validated_with(&root, &registry),
        &mut harness.ctx(),
        viewport,
        TransitionActivity::default(),
    );
    let width_of = |suffix: &str| {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("{suffix} is missing from the petrified frame"))
            .rect
            .w
    };
    (
        width_of("/root/bar"),
        width_of("/root/bar/fill"),
        width_of("/root/bar/track"),
    )
}

/// The percentage [`progress`] announces to a screen reader, as a number.
fn announced_percent(value: f32) -> f32 {
    let reported = progress("bar", "Rebuild", value)
        .semantics
        .value
        .expect("progress always reports a value");
    reported
        .strip_suffix('%')
        .unwrap_or_else(|| panic!("{reported:?} is not a percentage"))
        .parse()
        .unwrap_or_else(|e| panic!("{reported:?} does not parse as a percentage: {e}"))
}

/// The fraction of the bar the fill actually covers, in percent.
fn drawn_percent(value: f32) -> f32 {
    let (bar, fill, _) = bar_widths(value);
    assert!(bar > 0.0, "the bar itself was drawn at zero width");
    fill / bar * 100.0
}

/// A 62% bar is drawn 62% full, and the two cells tile the bar exactly.
///
/// Both halves matter. The weights alone were already right when the fill
/// was zero pixels wide, so the claim under test is the placed rect: the
/// fill occupies its track rather than declining it.
#[test]
fn the_fill_is_drawn_at_the_width_its_value_asks_for() {
    let (bar, fill, track) = bar_widths(0.62);
    assert!(fill > 0.0, "the fill of a 62% bar was drawn {fill} wide");
    assert!(
        (fill / bar - 0.62).abs() < 0.005,
        "a 0.62 value drew {fill} of {bar} ({:.1}%), not ~62%",
        fill / bar * 100.0
    );
    assert!(
        (fill + track - bar).abs() < 0.5,
        "fill {fill} + track {track} does not tile the {bar}-wide bar"
    );
}

/// The ends: an empty bar draws no meaningful fill, a complete one draws
/// almost nothing but fill, and the fill grows with the value in between.
///
/// Neither end lands on exactly 0 or exactly `bar`: tree acceptance refuses
/// a `Weight` track of zero, so the empty side of the bar carries
/// `MIN_WEIGHT` (0.001) instead — a tenth of a percent, which rounds away on
/// screen and in the announced percentage alike.
#[test]
fn an_empty_bar_and_a_complete_bar_are_both_drawn() {
    let (bar_0, fill_0, track_0) = bar_widths(0.0);
    assert!(
        fill_0 / bar_0 < 0.01,
        "a 0.0 value drew {fill_0} of {bar_0} — an empty bar must read empty"
    );
    assert!(
        track_0 / bar_0 > 0.99,
        "a 0.0 value left only {track_0} of {bar_0} for the track"
    );

    let (bar_1, fill_1, track_1) = bar_widths(1.0);
    assert!(
        fill_1 / bar_1 > 0.99,
        "a 1.0 value drew {fill_1} of {bar_1} — a complete bar must read full"
    );
    assert!(
        track_1 / bar_1 < 0.01,
        "a 1.0 value left {track_1} of {bar_1} still unfilled"
    );

    let fill_62 = bar_widths(0.62).1;
    assert!(
        fill_0 < fill_62 && fill_62 < fill_1,
        "fill is not monotone in value: {fill_0} / {fill_62} / {fill_1}"
    );
}

/// The label and the bar are two readers of one number, so they must never
/// disagree — including on the inputs that are not numbers.
///
/// `f32::clamp` propagates `NaN` and `f32::max` scrubs it, so before the
/// single normalisation in `progress` a `NaN` announced "NaN%" over two
/// equal weights: a bar drawn half full. Out-of-range and non-finite input
/// is normalised once, ahead of both channels; the announced percentage and
/// the drawn percentage are compared here against each other, not against a
/// hardcoded pair, so neither channel can be fixed alone.
#[test]
fn the_announced_percentage_and_the_drawn_fill_always_agree() {
    for value in [
        0.0,
        0.62,
        1.0,
        -0.5,
        1.5,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        let announced = announced_percent(value);
        let drawn = drawn_percent(value);
        assert!(
            (announced - drawn).abs() < 0.5,
            "value {value:?} announces {announced}% but draws {drawn:.2}%"
        );
    }
    assert_eq!(
        announced_percent(f32::NAN),
        0.0,
        "a value nobody could compute must not announce a finished job"
    );
}

/// FR-004a's two layering rules, read off every seat the ramp can express.
///
/// **The border rule is the one this exists for.** Carbon states it as
/// *"border tokens pair with its same number, for example `$field-03` pairs
/// with `$border-strong-03`"* — the field's number, not the background's — and
/// stated in prose it is the kind of rule that survives one implementation and
/// then quietly becomes "the border pairs with the background" in the next.
/// Stated as arithmetic it is two ordinals that either match or do not.
///
/// The ordinals are parsed back out of the token names rather than compared to
/// a table written here, so this fails if `layer_tokens` starts returning a
/// consistent-looking triple that has slipped a step — which a table would
/// have to be edited to notice.
#[test]
fn the_layer_triple_puts_the_field_one_ahead_and_the_border_beside_it() {
    /// The trailing two-digit ordinal of a Carbon-spelled name, if it has one.
    fn ordinal(token: &str) -> Option<usize> {
        token.rsplit('-').next()?.parse().ok()
    }

    for depth in 0..=MAX_LAYER_DEPTH {
        let seat = layer_tokens(depth);

        // The background carries no ordinal of its own — the layer set spells
        // its steps as words — so the seat number is the source of truth for
        // it, and the other two are checked against that.
        let field = ordinal(seat.field)
            .unwrap_or_else(|| panic!("`{}` carries no ordinal to pair a border with", seat.field));
        let border =
            ordinal(seat.border).unwrap_or_else(|| panic!("`{}` carries no ordinal", seat.border));

        assert_eq!(
            field,
            depth + 1,
            "a field on {} must be field-0{}, one layer ahead, not `{}`",
            seat.background,
            depth + 1,
            seat.field
        );
        assert_eq!(
            border, field,
            "`{}` pairs with `{}`: a border takes the *field's* number, not \
             the background's. This is the rule that gets lost.",
            seat.border, seat.field
        );
    }

    // Past the end of the ramp the seat clamps rather than running off it,
    // for `MAX_LAYER_DEPTH`'s reason: the alternative resolves a node and its
    // ground to the same colour and reports success.
    assert_eq!(
        layer_tokens(MAX_LAYER_DEPTH + 7),
        layer_tokens(MAX_LAYER_DEPTH),
        "a deeper seat than the ramp can express must clamp, not wrap or panic"
    );
}

/// Every name `layer_tokens` can emit is one the shipped vocabulary declares.
///
/// Without this the operators would be a well-formed arithmetic over names
/// that resolve to nothing: a tree built from them is refused at acceptance,
/// at runtime, in whichever component reaches the deepest seat first.
#[test]
fn every_layering_operator_name_is_in_the_standard_vocabulary() {
    let vocab = standard_vocabulary();
    for depth in 0..=MAX_LAYER_DEPTH {
        let seat = layer_tokens(depth);
        for token in [seat.background, seat.field, seat.border] {
            let name = TokenName::new(token)
                .unwrap_or_else(|err| panic!("`{token}` is not a well-formed token name: {err}"));
            assert!(
                vocab.contains(&name),
                "seat {depth} emits `{token}`, which standard_vocabulary() does \
                 not declare"
            );
        }
    }
}
