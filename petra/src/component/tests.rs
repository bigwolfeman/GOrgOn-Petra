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

use super::{
    button, checkbox, field, heading, list_row, progress, radio, section, status, tab, tab_bar,
    text, toggle,
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
            button("primary", "Save"),
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
    // The component library binds real design-token names (`spacing.md`,
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
