//! Tests for the semantic projection.
//!
//! Four claims carry the module and each has a test that fails without it:
//! ids are key-paths and are never recycled; a repeated query is byte-identical;
//! a virtualized collection counts what it did not materialize instead of
//! inventing it; and the contract's audit obligations are enforced by a
//! function, not by prose.

use std::collections::BTreeMap;

use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify, round_rect};
use crate::geom::{Scale, Size};
use crate::semantic::{AuditRule, StateFlag, TreeQuery, audit, project};
use crate::testing::{GeneratedRows, Harness, MonoContent, validated};
use crate::token::ThemeMode;
use crate::tree::{
    FocusFigure, FocusShownOn, Interaction, NodeKind, Props, Role, Semantics, ViewNode,
};

const VIEWPORT: Size = Size { w: 200.0, h: 120.0 };

/// Petrify `tree` at `seq` with the default doubles.
fn frame_of(seq: u64, tree: &ViewNode) -> PetrifiedFrame {
    frame_in(seq, tree, Viewport::new(VIEWPORT, ThemeMode::Dark))
}

/// Petrify `tree` at `seq` against an explicit viewport.
fn frame_in(seq: u64, tree: &ViewNode, viewport: Viewport) -> PetrifiedFrame {
    let mut harness = Harness::new();
    harness.scale = viewport.scale;
    petrify(
        seq,
        validated(tree),
        &mut harness.ctx(),
        viewport,
        TransitionActivity::default(),
    )
}

/// A three-pane fixture with a focusable button, a status readout and a label.
fn app(child_keys: &[&str]) -> ViewNode {
    let mut root = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
        axis: Some(crate::geom::Axis::Vertical),
        ..Props::default()
    });
    for key in child_keys {
        root = root.child(ViewNode::new(NodeKind::Text, *key).with_props(Props {
            text: Some((*key).to_owned()),
            ..Props::default()
        }));
    }
    root
}

fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("a semantic tree serializes")
}

// ---------------------------------------------------------------- projection

/// Every placement becomes exactly one node, and pre-order survives the
/// rebuild through `Placement::parent`.
#[test]
fn the_tree_holds_one_node_per_placement_in_placement_order() {
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .child(
            ViewNode::new(NodeKind::Stack, "left")
                .child(ViewNode::new(NodeKind::Text, "a"))
                .child(ViewNode::new(NodeKind::Text, "b")),
        )
        .child(ViewNode::new(NodeKind::Text, "right"));
    let frame = frame_of(1, &tree);
    let projected = project(&frame).expect("a petrified frame has a root");

    let placed: Vec<&str> = frame.placements.iter().map(|p| p.id.as_str()).collect();
    let walked: Vec<&str> = projected.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        walked, placed,
        "pre-order of the projection must be placement order"
    );
    assert_eq!(
        walked,
        [
            "/root",
            "/root/left",
            "/root/left/a",
            "/root/left/b",
            "/root/right"
        ]
    );
    assert_eq!(projected.len(), frame.placements.len());
}

/// `bounds` come from `frame::round_rect`, the rule the renderer and the frame
/// digest share. A second rounding here would put the driver's click a pixel
/// away from the thing it drew.
#[test]
fn bounds_are_device_pixels_from_the_one_rounding_rule() {
    let scale = Scale::new(1.5).expect("1.5 is a scale");
    let viewport = Viewport::new(Size::new(101.0, 51.0), ThemeMode::Light).with_scale(scale);
    let frame = frame_in(4, &app(&["one", "two"]), viewport);
    let projected = project(&frame).expect("root");

    for placement in &frame.placements {
        let node = projected.find(&placement.id).expect("every placement");
        let want = round_rect(placement.rect, scale);
        assert_eq!(
            (node.bounds.x, node.bounds.y, node.bounds.w, node.bounds.h),
            (want.x, want.y, want.w, want.h),
            "{} rounded twice",
            placement.id
        );
    }
    // Non-vacuity: at 1.5 scale the device rect must not simply be the logical
    // one, or this test would pass against a projection that never rounded.
    let root = projected.root();
    assert_eq!(
        (root.bounds.w, root.bounds.h),
        (152, 77),
        "101 x 51 logical units at 1.5 scale"
    );
}

/// The declared field names, their order, and the absent-means-default rule,
/// pinned against the contract.
#[test]
fn the_wire_shape_is_the_contract_shape() {
    let node = ViewNode::new(NodeKind::Input, "field")
        .with_semantics(Semantics {
            role: Some(Role::TextInput),
            label: Some("Filter".into()),
            value: Some("fib".into()),
            disabled: false,
            read_only: false,
            skeleton: false,
            selected: true,
            expanded: Some(true),
            stale: true,
            // Held out of the projection on purpose: what the focus figure
            // is and where it is drawn are paint facts. Non-default here so
            // the assertion below proves the hold-out rather than the default.
            focus_figure: FocusFigure::Sides,
            focus_shown_on: FocusShownOn::Well,
            focus_run: true,
            owns_its_text: true,
        })
        .with_ambient(true);
    let node = ViewNode {
        interactions: vec![
            Interaction::TextEdit,
            Interaction::Click,
            Interaction::Focus,
        ],
        ..node
    };
    let frame = frame_in(
        3,
        &node,
        Viewport::new(Size::new(120.0, 40.0), ThemeMode::Dark),
    );
    let projected = project(&frame).expect("root");
    assert_eq!(
        json(&projected),
        r#"{"id":"/field","role":"textinput","label":"Filter","value":"fib","state":{"selected":true,"expanded":true,"stale":true,"ambient":true},"bounds":{"x":0,"y":0,"w":120,"h":40},"frame_seq":3,"actions":["click","focus","text-edit"],"children":[]}"#
    );
}

/// A node that declares nothing still carries the always-present fields, and
/// carries no key for a flag that is not in force.
#[test]
fn absent_state_flags_and_absent_values_stay_off_the_wire() {
    let frame = frame_in(
        7,
        &ViewNode::new(NodeKind::Stack, "root"),
        Viewport::new(Size::new(100.0, 50.0), ThemeMode::Dark),
    );
    let projected = project(&frame).expect("root");
    assert_eq!(
        json(&projected),
        r#"{"id":"/root","role":"pane","label":"","state":{},"bounds":{"x":0,"y":0,"w":100,"h":50},"frame_seq":7,"actions":[],"children":[]}"#
    );
}

/// `actions` is a set on the wire: authoring order must not reach it, or two
/// nodes accepting the same kinds would serialize differently.
#[test]
fn actions_are_sorted_and_deduplicated() {
    // `.interactive` also sets `semantics.role` and `.label`: acceptance
    // refuses an interactive node that carries neither, and this fixture's
    // whole point is the `interactions` list, not a violation of a rule
    // this file's `validate.rs` tests already cover.
    let node = ViewNode::new(NodeKind::Stack, "root").interactive(
        Role::Button,
        "Actions",
        &[
            Interaction::Scroll,
            Interaction::Click,
            Interaction::Click,
            Interaction::Focus,
        ],
    );
    let frame = frame_of(1, &node);
    let projected = project(&frame).expect("root");
    assert_eq!(
        projected.root().actions,
        [Interaction::Click, Interaction::Focus, Interaction::Scroll]
    );
}

/// A `spacer` has no role and the projection does not invent one; the node is
/// still there, so a driver can still see what the frame placed.
#[test]
fn a_node_with_no_role_keeps_its_place_without_being_given_one() {
    let tree = ViewNode::new(NodeKind::Stack, "root").child(ViewNode::new(NodeKind::Spacer, "gap"));
    let projected = project(&frame_of(1, &tree)).expect("root");
    let gap = projected
        .find("/root/gap")
        .expect("the spacer is projected");
    assert_eq!(gap.role, None);
    assert!(
        !json(gap).contains("role"),
        "an absent role must not reach the wire: {}",
        json(gap)
    );
}

/// A frame with no placements has no tree, rather than an invented root.
#[test]
fn a_frame_with_no_placements_projects_no_tree() {
    let mut frame = frame_of(1, &ViewNode::new(NodeKind::Stack, "root"));
    frame.placements.clear();
    frame.content.clear();
    assert!(project(&frame).is_none());
}

// ------------------------------------------------------------------ stable ids

/// Ids are key-paths, so scrolling a list — which changes which rows exist and
/// where every one of them sits — never renames a row that stayed.
#[test]
fn a_row_keeps_its_id_across_a_scroll() {
    let ids_at = |offset: f32| -> Vec<String> {
        let mut harness =
            Harness::with(MonoContent::new(), GeneratedRows::new("fibers", TOTAL_ROWS));
        harness.set_scroll("/list", offset);
        let frame = petrify(
            1,
            validated(&virtual_list()),
            &mut harness.ctx(),
            Viewport::new(VIEWPORT, ThemeMode::Dark),
            TransitionActivity::default(),
        );
        project(&frame)
            .expect("root")
            .iter()
            .map(|n| n.id.clone())
            .collect()
    };
    let top = ids_at(0.0);
    let down = ids_at(48.0);
    let is_row = |id: &&String| id.starts_with("/list/rows/row-");
    let shared: Vec<&String> = top
        .iter()
        .filter(is_row)
        .filter(|id| down.contains(id))
        .collect();
    assert!(
        shared.len() > 3,
        "the two row windows must overlap or this proves nothing: {top:?} vs {down:?}"
    );
    assert!(
        top.contains(&"/list/rows/row-3".to_owned())
            && down.contains(&"/list/rows/row-3".to_owned()),
        "row 3 is on screen at both offsets under one id: {top:?} / {down:?}"
    );
    assert!(
        !top.contains(&"/list/rows/row-11".to_owned())
            && down.contains(&"/list/rows/row-11".to_owned()),
        "the window really moved, so the shared ids are not the whole list"
    );
}

/// A theme change moves every token and no id.
#[test]
fn a_theme_change_moves_no_id() {
    let tree = app(&["a", "b"]);
    let dark = project(&frame_in(
        1,
        &tree,
        Viewport::new(VIEWPORT, ThemeMode::Dark),
    ))
    .expect("root");
    let light = project(&frame_in(
        2,
        &tree,
        Viewport::new(VIEWPORT, ThemeMode::Light).with_theme_rev(9),
    ))
    .expect("root");
    let dark_ids: Vec<&str> = dark.iter().map(|n| n.id.as_str()).collect();
    let light_ids: Vec<&str> = light.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(dark_ids, light_ids);
}

/// "Re-parents that preserve the path": the container above a node changes
/// kind and gains siblings before it, so the node's *index* moves and its
/// key-path does not. An id allocated from position would fail this.
#[test]
fn an_id_follows_the_key_path_not_the_position() {
    let before = ViewNode::new(NodeKind::Stack, "root")
        .child(ViewNode::new(NodeKind::Stack, "panel").child(ViewNode::new(NodeKind::Text, "ok")));
    let after = ViewNode::new(NodeKind::Stack, "root")
        .child(ViewNode::new(NodeKind::Text, "banner"))
        .child(
            ViewNode::new(NodeKind::Overlay, "panel").child(ViewNode::new(NodeKind::Text, "ok")),
        );

    let first = project(&frame_of(1, &before)).expect("root");
    let second = project(&frame_of(2, &after)).expect("root");
    assert!(first.find("/root/panel/ok").is_some());
    assert!(
        second.find("/root/panel/ok").is_some(),
        "the key-path is unchanged, so the id must be: {:?}",
        second.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
}

/// Never recycled: a node that leaves takes its id with it, and the node that
/// later occupies its position gets its own. Index-derived ids would hand
/// `delta` the id `beta` used to answer to, and FR-039's `stale-node` error
/// would start naming the wrong node.
#[test]
fn an_absent_key_path_never_hands_its_id_to_a_new_node() {
    let full = project(&frame_of(1, &app(&["alpha", "beta", "gamma"]))).expect("root");
    let shrunk = project(&frame_of(2, &app(&["alpha", "gamma"]))).expect("root");
    let regrown = project(&frame_of(3, &app(&["alpha", "delta", "gamma"]))).expect("root");

    assert!(full.find("/root/beta").is_some());
    assert!(
        shrunk.find("/root/beta").is_none(),
        "the removed node's id must not resolve"
    );

    let retired: Vec<&str> = full
        .iter()
        .map(|n| n.id.as_str())
        .filter(|id| shrunk.find(id).is_none())
        .collect();
    assert_eq!(retired, ["/root/beta"]);

    let fresh: Vec<&str> = regrown
        .iter()
        .map(|n| n.id.as_str())
        .filter(|id| shrunk.find(id).is_none())
        .collect();
    assert_eq!(fresh, ["/root/delta"]);
    for id in &fresh {
        assert!(
            !retired.contains(id),
            "{id} was retired and has been handed to a different node"
        );
    }
    // The new node stands where the old one did, which is what makes this a
    // recycling hazard rather than an append.
    let order: Vec<&str> = regrown
        .root()
        .children
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(order, ["/root/alpha", "/root/delta", "/root/gamma"]);
}

// ------------------------------------------------------------ byte identity

/// FR-036: repeated queries of an unchanged UI return identical trees.
#[test]
fn two_projections_of_one_frame_serialize_to_identical_bytes() {
    let frame = frame_of(11, &app(&["alpha", "beta", "gamma"]));
    let once = json(&project(&frame).expect("root"));
    let again = json(&project(&frame).expect("root"));
    assert_eq!(once.as_bytes(), again.as_bytes());
    for _ in 0..64 {
        assert_eq!(
            json(&project(&frame).expect("root")).as_bytes(),
            once.as_bytes()
        );
    }
}

/// The same input petrified twice must project the same bytes too: identity
/// must not depend on which `PetrifiedFrame` value the query happened to hold.
#[test]
fn two_petrifies_of_one_tree_project_to_identical_bytes() {
    let tree = app(&["alpha", "beta", "gamma"]);
    let once = json(&project(&frame_of(11, &tree)).expect("root"));
    let again = json(&project(&frame_of(11, &tree)).expect("root"));
    assert_eq!(once, again);
}

/// The guard that keeps the two tests above from passing vacuously: a changed
/// UI must change the bytes.
#[test]
fn a_changed_ui_changes_the_bytes() {
    let base = json(&project(&frame_of(11, &app(&["alpha", "beta"]))).expect("root"));
    let renamed = json(&project(&frame_of(11, &app(&["alpha", "zeta"]))).expect("root"));
    let resequenced = json(&project(&frame_of(12, &app(&["alpha", "beta"]))).expect("root"));
    assert_ne!(base, renamed);
    assert_ne!(base, resequenced);
}

// ---------------------------------------------------------- virtualization

const TOTAL_ROWS: usize = 100_000;

/// A `scroll` over a virtualized `collection` of `TOTAL_ROWS` rows.
fn virtual_list() -> ViewNode {
    ViewNode::new(NodeKind::Scroll, "list")
        .with_props(Props {
            overscan: Some(24.0),
            ..Props::default()
        })
        .child(
            ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
                total_count: Some(TOTAL_ROWS),
                source: Some("fibers".into()),
                estimated_extent: Some(16.0),
                ..Props::default()
            }),
        )
}

/// FR-009 honesty: the tree names the rows the frame materialized, reports
/// `total_count` for the rest, and fabricates nothing.
#[test]
fn a_virtualized_collection_counts_what_it_did_not_materialize() {
    let mut harness = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", TOTAL_ROWS));
    harness.set_scroll("/list", 480.0);
    let frame = petrify(
        1,
        validated(&virtual_list()),
        &mut harness.ctx(),
        Viewport::new(VIEWPORT, ThemeMode::Dark),
        TransitionActivity::default(),
    );
    let projected = project(&frame).expect("root");

    let collection = projected.find("/list/rows").expect("the collection node");
    assert_eq!(collection.total_count, Some(TOTAL_ROWS));
    assert_eq!(collection.role, Some(Role::List));

    let rows = collection.children.len();
    assert!(
        (8..=40).contains(&rows),
        "expected a windowed row count, got {rows} nodes for {TOTAL_ROWS} rows"
    );
    assert_eq!(
        projected.len(),
        frame.placements.len(),
        "a node per placement and not one more"
    );

    // Nothing outside the window exists as a node, however plausible its id.
    for id in [
        "/list/rows/row-0",
        "/list/rows/row-99999",
        "/list/rows/row-500",
    ] {
        let materialized = frame.placements.iter().any(|p| p.id == id);
        assert_eq!(
            projected.find(id).is_some(),
            materialized,
            "{id}: the tree must agree with the frame about whether this row exists"
        );
    }
    assert!(
        projected.find("/list/rows/row-99999").is_none(),
        "the last row of a 100 000-row list is not on a 120-unit screen"
    );
    let violations = audit(&projected, &frame);
    assert!(violations.is_empty(), "{violations:?}");
}

// ------------------------------------------------------------------- audit

/// A frame built from well-formed input audits clean. Without this the
/// violation tests below could all pass against an audit that reports
/// everything.
#[test]
fn a_well_formed_frame_audits_clean() {
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(crate::geom::Axis::Vertical),
            ..Props::default()
        })
        .child(
            ViewNode::new(NodeKind::Text, "reload")
                .with_props(Props {
                    text: Some("Reload".into()),
                    ..Props::default()
                })
                .interactive(
                    Role::Button,
                    "Reload",
                    &[Interaction::Click, Interaction::Focus],
                ),
        )
        .child(
            ViewNode::new(NodeKind::Text, "state").with_semantics(Semantics {
                role: Some(Role::Status),
                label: Some("Kernel up".into()),
                ..Semantics::default()
            }),
        )
        .child(ViewNode::new(NodeKind::Input, "filter").interactive(
            Role::TextInput,
            "Filter",
            &[Interaction::Focus, Interaction::TextEdit],
        ));
    let frame = frame_of(5, &tree);
    let projected = project(&frame).expect("root");
    assert_eq!(audit(&projected, &frame), Vec::new());
}

/// The fixture the violation tests mutate.
fn audit_fixture() -> (crate::semantic::SemanticTree, PetrifiedFrame) {
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(crate::geom::Axis::Vertical),
            ..Props::default()
        })
        .child(
            ViewNode::new(NodeKind::Text, "reload")
                .with_props(Props {
                    text: Some("Reload".into()),
                    ..Props::default()
                })
                .interactive(
                    Role::Button,
                    "Reload",
                    &[Interaction::Click, Interaction::Focus],
                ),
        )
        .child(ViewNode::new(NodeKind::Input, "filter").interactive(
            Role::TextInput,
            "Filter",
            &[Interaction::Focus, Interaction::TextEdit],
        ))
        .child(ViewNode::new(NodeKind::Spacer, "gap"));
    let frame = frame_of(5, &tree);
    let projected = project(&frame).expect("root");
    (projected, frame)
}

/// Every rule reported for `id`.
fn rules_for(
    tree: &crate::semantic::SemanticTree,
    frame: &PetrifiedFrame,
    id: &str,
) -> Vec<AuditRule> {
    audit(tree, frame)
        .into_iter()
        .filter(|v| v.node_id == id)
        .map(|v| v.rule)
        .collect()
}

/// Obligation 1, the role half.
#[test]
fn an_actionable_node_without_a_role_is_reported() {
    let (mut tree, frame) = audit_fixture();
    let mut root = tree.root().clone();
    root.children[0].role = None;
    tree = crate::semantic::SemanticTree::new(root);
    assert_eq!(
        rules_for(&tree, &frame, "/root/reload"),
        [AuditRule::ActionableNeedsRoleAndLabel]
    );
}

/// Obligation 1, the label half. A label of spaces is no label.
#[test]
fn an_actionable_node_without_a_label_is_reported() {
    let (_, frame) = audit_fixture();
    for blank in ["", "   "] {
        let mut root = project(&frame).expect("root").root().clone();
        root.children[0].label = blank.to_owned();
        let tree = crate::semantic::SemanticTree::new(root);
        assert_eq!(
            rules_for(&tree, &frame, "/root/reload"),
            [AuditRule::ActionableNeedsRoleAndLabel],
            "label {blank:?}"
        );
    }
}

/// Obligation 2: the shape-plus-text rule. A status with no label leaves
/// colour as its only channel.
#[test]
fn a_status_without_a_label_is_reported() {
    // Acceptance refuses a status role with no label
    // (`Violation::StatusRoleWithoutLabel`), so the fixture starts labelled
    // and the label is stripped from the *projection* afterward — the same
    // move `an_actionable_node_without_a_role_is_reported` makes for
    // Obligation 1 — to reach the state this audit rule exists to catch.
    let tree = ViewNode::new(NodeKind::Stack, "root").child(
        ViewNode::new(NodeKind::Text, "state").with_semantics(Semantics {
            role: Some(Role::Status),
            label: Some("placeholder".into()),
            ..Semantics::default()
        }),
    );
    let frame = frame_of(5, &tree);
    let mut root = project(&frame).expect("root").root().clone();
    root.children[0].label = String::new();
    let projected = crate::semantic::SemanticTree::new(root);
    assert_eq!(
        rules_for(&projected, &frame, "/root/state"),
        [AuditRule::StatusNeedsLabel]
    );
}

/// Obligation 3 (SC-010): reading order and focus order are the same list.
/// Swapping two children of the projection breaks exactly that and nothing
/// else the audit can see.
#[test]
fn a_projection_that_reorders_children_breaks_focus_order() {
    let (tree, frame) = audit_fixture();
    assert!(audit(&tree, &frame).is_empty());

    let mut root = tree.root().clone();
    root.children.swap(0, 1);
    let scrambled = crate::semantic::SemanticTree::new(root);
    let rules: Vec<AuditRule> = audit(&scrambled, &frame)
        .into_iter()
        .map(|v| v.rule)
        .collect();
    assert_eq!(rules, [AuditRule::FocusOrderIsChildOrder]);
}

/// Obligation 4, the "flag matches the frame" half.
#[test]
fn a_truncated_flag_the_frame_does_not_carry_is_reported() {
    let (tree, frame) = audit_fixture();
    let mut root = tree.root().clone();
    root.children[0].state.truncated = true;
    let lying = crate::semantic::SemanticTree::new(root);
    assert_eq!(
        rules_for(&lying, &frame, "/root/reload"),
        [AuditRule::TruncationIsReal]
    );

    // And the other direction: a real truncation the tree dropped.
    let mut frame_truncated = frame.clone();
    let index = frame_truncated
        .placements
        .iter()
        .position(|p| p.id == "/root/reload")
        .expect("the button is placed");
    frame_truncated.placements[index].paint.truncated = true;
    assert_eq!(
        rules_for(&tree, &frame_truncated, "/root/reload"),
        [AuditRule::TruncationIsReal]
    );
}

/// Obligation 4, the "there was something to hide" half: a spacer draws no
/// text and holds no children, so it cannot have truncated anything.
#[test]
fn a_truncation_with_nothing_to_hide_is_reported() {
    let (_, frame) = audit_fixture();
    let mut frame = frame;
    let index = frame
        .placements
        .iter()
        .position(|p| p.id == "/root/gap")
        .expect("the spacer is placed");
    frame.placements[index].paint.truncated = true;
    let tree = project(&frame).expect("root");
    assert_eq!(
        rules_for(&tree, &frame, "/root/gap"),
        [AuditRule::TruncationIsReal]
    );

    // A container that truncated its children is a real truncation and must
    // not be reported, or the rule would fire on every clipped pane.
    let tall = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(crate::geom::Axis::Vertical),
            ..Props::default()
        })
        .child(
            ViewNode::new(NodeKind::Spacer, "a").with_constraints(crate::tree::Constraints {
                vertical: crate::tree::AxisConstraint {
                    min: Some(400.0),
                    ..crate::tree::AxisConstraint::default()
                },
                ..crate::tree::Constraints::default()
            }),
        );
    let squeezed = frame_of(6, &tall);
    assert!(
        squeezed.placements[0].paint.truncated,
        "400 units of minimum in a 120-unit window is a truncation"
    );
    let projected = project(&squeezed).expect("root");
    assert_eq!(
        rules_for(&projected, &squeezed, "/root"),
        Vec::new(),
        "a container that could not fit its children truncated something real"
    );
}

/// The anti-fabrication rule: a node whose id names no placement.
#[test]
fn a_fabricated_node_is_reported() {
    let (tree, frame) = audit_fixture();
    let mut root = tree.root().clone();
    let mut ghost = root.children[0].clone();
    ghost.id = "/root/rows/row-99999".into();
    ghost.actions.clear();
    root.children.push(ghost);
    let padded = crate::semantic::SemanticTree::new(root);
    assert_eq!(
        rules_for(&padded, &frame, "/root/rows/row-99999"),
        [AuditRule::NodeIsNotAPlacement]
    );
}

/// The opposite dishonesty: a placement the tree quietly dropped.
#[test]
fn a_dropped_placement_is_reported() {
    let (tree, frame) = audit_fixture();
    let mut root = tree.root().clone();
    root.children.remove(2);
    let thinned = crate::semantic::SemanticTree::new(root);
    assert_eq!(
        rules_for(&thinned, &frame, "/root/gap"),
        [AuditRule::PlacementIsNotANode]
    );
}

/// FR-039 needs an id to name one node.
#[test]
fn a_duplicate_id_is_reported() {
    let (tree, frame) = audit_fixture();
    let mut root = tree.root().clone();
    let twin = root.children[2].clone();
    root.children.push(twin);
    let doubled = crate::semantic::SemanticTree::new(root);
    assert_eq!(
        rules_for(&doubled, &frame, "/root/gap"),
        [AuditRule::DuplicateNodeId]
    );
}

/// A tree paired with the wrong frame is the failure a screenshot-plus-tree
/// consumer would otherwise see as a mystery.
#[test]
fn a_tree_from_another_frame_is_reported() {
    let (_, frame) = audit_fixture();
    let older = project(&frame_of(4, &ViewNode::new(NodeKind::Stack, "root"))).expect("root");
    let rules: Vec<AuditRule> = audit(&older, &frame).into_iter().map(|v| v.rule).collect();
    assert!(
        rules.contains(&AuditRule::FrameSeqMismatch),
        "expected a frame-seq mismatch, got {rules:?}"
    );
}

/// A malformed frame must not panic the audit: a gate that crashes reports
/// nothing.
#[test]
fn a_frame_with_a_forward_parent_link_is_reported_not_panicked() {
    let (tree, frame) = audit_fixture();
    let mut broken = frame.clone();
    broken.placements[1].parent = Some(9);
    let rules: Vec<AuditRule> = audit(&tree, &broken).into_iter().map(|v| v.rule).collect();
    assert!(
        rules.is_empty()
            || rules
                .iter()
                .all(|r| *r != AuditRule::FocusOrderIsChildOrder)
    );
    // And the projection of such a frame leaves the unhangable node out, which
    // the audit then names.
    let reprojected = project(&broken).expect("root");
    let dropped: Vec<AuditRule> = audit(&reprojected, &broken)
        .into_iter()
        .filter(|v| v.node_id == "/root/reload")
        .map(|v| v.rule)
        .collect();
    assert_eq!(dropped, [AuditRule::PlacementIsNotANode]);
}

/// Every rule has a distinct stable name; gate output and driver errors quote
/// these.
#[test]
fn rule_names_are_distinct_and_stable() {
    let names: BTreeMap<&str, AuditRule> = crate::semantic::AUDIT_RULES
        .iter()
        .map(|rule| (rule.as_str(), *rule))
        .collect();
    assert_eq!(names.len(), crate::semantic::AUDIT_RULES.len());
    assert_eq!(
        AuditRule::FocusOrderIsChildOrder.to_string(),
        "focus-order-is-child-order"
    );
}

// ------------------------------------------------------------------- query

#[test]
fn filters_select_by_role_label_and_state() {
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .child(ViewNode::new(NodeKind::Text, "reload").interactive(
            Role::Button,
            "Reload kernel",
            &[Interaction::Click, Interaction::Focus],
        ))
        .child(ViewNode::new(NodeKind::Text, "quit").interactive(
            Role::Button,
            "Quit",
            &[Interaction::Click, Interaction::Focus],
        ))
        .child(
            ViewNode::new(NodeKind::Text, "old").with_semantics(Semantics {
                role: Some(Role::Status),
                label: Some("Reload pending".into()),
                stale: true,
                ..Semantics::default()
            }),
        );
    let frame = frame_of(1, &tree);
    let projected = project(&frame).expect("root");

    let by_role: Vec<&str> = projected
        .find_all(&TreeQuery::new().with_role("button"))
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(by_role, ["/root/reload", "/root/quit"]);

    let by_label: Vec<&str> = projected
        .find_all(&TreeQuery::new().with_label_contains("Reload"))
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(by_label, ["/root/reload", "/root/old"]);

    let both: Vec<&str> = projected
        .find_all(
            &TreeQuery::new()
                .with_role("button")
                .with_label_contains("Reload"),
        )
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(both, ["/root/reload"]);

    let stale: Vec<&str> = projected
        .find_all(&TreeQuery::new().with_state(StateFlag::Stale))
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(stale, ["/root/old"]);

    assert_eq!(projected.find_all(&TreeQuery::new()).len(), projected.len());
    assert!(
        projected
            .find_all(&TreeQuery::new().with_role("custom:nothing"))
            .is_empty()
    );
}

/// A subtree answers with the same shape as a whole tree, frame number and
/// all, so a consumer cannot tell the two apart by anything but the root.
#[test]
fn a_subtree_is_a_tree() {
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .child(ViewNode::new(NodeKind::Stack, "panel").child(ViewNode::new(NodeKind::Text, "ok")));
    let frame = frame_of(21, &tree);
    let projected = project(&frame).expect("root");
    let panel = projected.subtree("/root/panel").expect("the panel");
    assert_eq!(panel.frame_seq(), 21);
    assert_eq!(panel.root().id, "/root/panel");
    assert_eq!(panel.len(), 2);
    assert_eq!(json(&panel), json(projected.find("/root/panel").unwrap()));
    assert!(projected.subtree("/root/nowhere").is_none());
}

// ------------------------------------------------------------------- focused

/// `state.focused` reaches the tree from the placement, and names exactly one
/// node.
///
/// This is the seam between two changes made in different branches: the focus
/// ring is painted from `PlacementSemantics::focused`, and the tree reports
/// the same flag. `contracts/semantic-tree.md` puts `focused` in `state`, and
/// FR-027 makes a disagreement between what a screen reader hears and what a
/// test asserts a bug by definition — so the projection must read the flag the
/// painter reads, never a second source.
#[test]
fn the_focused_flag_reaches_the_tree_and_names_one_node() {
    let tree = app(&["a", "b", "c"]);
    let mut harness = Harness::new();
    harness.state.focused = Some("/root/b".into());
    let frame = petrify(
        1,
        validated(&tree),
        &mut harness.ctx(),
        Viewport::new(VIEWPORT, ThemeMode::Dark),
        TransitionActivity::default(),
    );
    let projected = project(&frame).expect("the fixture places a root");

    let focused: Vec<&str> = projected
        .iter()
        .filter(|node| node.state.focused)
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(
        focused,
        ["/root/b"],
        "exactly the node the layout state names carries the flag"
    );
}

/// A consumer can select the focused node by filter, which is how a driver
/// answers "what has focus" without walking the whole tree itself.
#[test]
fn the_focused_flag_is_queryable() {
    let tree = app(&["a", "b", "c"]);
    let mut harness = Harness::new();
    harness.state.focused = Some("/root/c".into());
    let frame = petrify(
        1,
        validated(&tree),
        &mut harness.ctx(),
        Viewport::new(VIEWPORT, ThemeMode::Dark),
        TransitionActivity::default(),
    );
    let projected = project(&frame).expect("the fixture places a root");

    let query = TreeQuery {
        state: Some(StateFlag::Focused),
        ..TreeQuery::new()
    };
    let hits: Vec<&str> = projected
        .iter()
        .filter(|node| query.matches(node))
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(hits, ["/root/c"]);
}

/// A frame with nothing focused says so, rather than defaulting to its first
/// node or to every node.
#[test]
fn a_frame_with_no_focus_marks_nothing() {
    let frame = frame_of(1, &app(&["a", "b"]));
    let projected = project(&frame).expect("the fixture places a root");
    assert!(
        projected.iter().all(|node| !node.state.focused),
        "no layout state means no focused node"
    );
}

/// The action names the tree advertises are the contract's closed set, spelled
/// exactly as `contracts/semantic-tree.md` prints them.
///
/// This is the FR-027 seam in one assertion. A driver reads the contract and
/// sends `text-edit`; the tree advertises whatever `Interaction` serializes to.
/// If those two ever drift — someone renames the variant, or the contract is
/// tidied to `text` — a driver would ask for an action the tree says it
/// accepts and be refused. The contract's list is duplicated here on purpose:
/// that is what makes this a test rather than a tautology over one source.
#[test]
fn the_action_wire_names_are_the_contract_set() {
    let all = [
        Interaction::Click,
        Interaction::Drag,
        Interaction::Hover,
        Interaction::Focus,
        Interaction::TextEdit,
        Interaction::Scroll,
        Interaction::Key,
    ];
    let printed: Vec<String> = all
        .iter()
        .map(|action| {
            let wire = json(action);
            wire.trim_matches('"').to_owned()
        })
        .collect();
    assert_eq!(
        printed,
        [
            "click",
            "drag",
            "hover",
            "focus",
            "text-edit",
            "scroll",
            "key"
        ],
        "the contract's action vocabulary and Interaction's wire form must agree"
    );
}
