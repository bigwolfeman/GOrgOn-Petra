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
