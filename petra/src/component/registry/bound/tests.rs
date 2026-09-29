use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::*;
use crate::component::registry::{build, expand};
use crate::tree::{BindTarget, ComponentRef, PropVal, ResolveError, SlotKey, SlotValue};

fn reference(
    key: &str,
    name: &str,
    params: serde_json::Value,
    bound: &[(&str, PropVal)],
) -> ViewNode {
    let mut node = ViewNode::new(NodeKind::Component, key);
    let mut component = ComponentRef::literal(name, params);
    component.bound = bound
        .iter()
        .map(|(param, source)| ((*param).to_owned(), source.clone()))
        .collect::<BTreeMap<_, _>>();
    node.component = Some(component);
    node
}

fn bind(slot: &str) -> PropVal {
    PropVal::Bind(SlotKey::new(slot))
}

fn inputs(pairs: &[(&str, SlotValue)]) -> ResolveInputs {
    pairs
        .iter()
        .map(|(slot, value)| ((*slot).to_owned(), value.clone()))
        .collect()
}

fn bound_toggle(key: &str, slot: &str) -> ViewNode {
    reference(
        key,
        "toggle",
        json!({ "key": key, "label": "Auto-reload" }),
        &[("selected", bind(slot))],
    )
}

#[test]
fn a_bound_parameter_expands_exactly_like_the_literal_it_resolves_to() {
    for on in [false, true] {
        let grown = expand_with(
            &bound_toggle("tg", "on"),
            &inputs(&[("on", SlotValue::Bool(on))]),
        )
        .expect("an openable parameter with a value expands");
        let literal = build(
            "toggle",
            &json!({ "key": "tg", "label": "Auto-reload", "selected": on }),
        )
        .expect("the literal toggle builds");
        assert_eq!(grown.tree(), &literal, "selected = {on}");
        assert_eq!(grown.units().len(), 1);
        assert_eq!(grown.units()[0].slots(), &BTreeSet::from(["on".to_owned()]));
    }
}

#[test]
fn a_source_on_a_parameter_the_component_does_not_open_is_refused_by_name() {
    let node = reference(
        "tg",
        "toggle",
        json!({ "key": "tg", "selected": false }),
        &[("label", bind("title"))],
    );
    let err = expand_with(&node, &inputs(&[("title", SlotValue::Str("x".into()))]))
        .expect_err("`label` is not openable on a toggle");
    let ExpandError::Param(err) = err else {
        panic!("a non-openable source is a parameter defect, got {err}");
    };
    assert_eq!(err.component, "toggle");
    assert!(
        err.reason.contains("parameter `label` is not openable")
            && err
                .reason
                .contains("a component expands from literal parameters")
            && err.reason.contains("[selected]"),
        "the refusal must name the parameter and what is open, got: {}",
        err.reason
    );
}

#[test]
fn a_parameter_with_a_literal_and_a_source_is_refused() {
    let node = reference(
        "tg",
        "toggle",
        json!({ "key": "tg", "label": "L", "selected": true }),
        &[("selected", bind("on"))],
    );
    let err = expand_with(&node, &inputs(&[("on", SlotValue::Bool(false))]))
        .expect_err("two sources for one parameter");
    assert!(err.to_string().contains("exactly one source"), "got: {err}");
}

#[test]
fn a_missing_value_is_a_resolve_refusal_naming_the_slot_and_the_parameter() {
    let err = expand_with(&bound_toggle("tg", "on"), &ResolveInputs::new())
        .expect_err("no value for `on`");
    match &err {
        ExpandError::Resolve(ResolveError::UnknownSlot { slot, prop, .. }) => {
            assert_eq!(slot, "on");
            assert_eq!(*prop, BindTarget::Param("selected"));
        }
        other => panic!("expected UnknownSlot, got {other}"),
    }
    assert!(err.to_string().contains("tg.params.selected"), "{err}");
}

#[test]
fn a_value_of_the_wrong_type_is_refused_before_the_constructor_runs() {
    let err = expand_with(
        &bound_toggle("tg", "on"),
        &inputs(&[("on", SlotValue::Str("yes".into()))]),
    )
    .expect_err("a string cannot fill a boolean parameter");
    assert!(
        matches!(err, ExpandError::Resolve(ResolveError::TypeMismatch { .. })),
        "{err}"
    );
}

#[test]
fn the_literal_expansion_refuses_a_bound_reference_and_names_the_way_through() {
    let err = expand(&bound_toggle("tg", "on")).expect_err("no values to fold against");
    assert!(
        err.reason.contains("expand_with") && err.reason.contains("selected"),
        "got: {}",
        err.reason
    );
}

/// A row binding `selected` inside a table: the unit is the table, because
/// the table's constructor is what wraps and weights the rows it is handed.
#[test]
fn the_unit_is_the_outermost_reference_whose_parameters_bind() {
    let row = |key: &str, slot: Option<&str>| {
        let cell = json!({ "kind": "text", "key": format!("{key}-c"), "props": { "text": key } });
        let mut params = json!({ "key": key, "children": [cell] });
        if slot.is_none() {
            params["selected"] = json!(false);
        }
        let bound: Vec<(&str, PropVal)> = slot.map(|s| ("selected", bind(s))).into_iter().collect();
        serde_json::to_value(reference(key, "data_table_row_sm", params, &bound))
            .expect("a node serializes")
    };
    let header = json!({ "kind": "text", "key": "h", "props": { "text": "Name" } });
    let table = reference(
        "table",
        "data_table_zebra_sized",
        json!({
            "key": "table",
            "header": [header],
            "rows": [row("r0", Some("sel-0")), row("r1", None)],
            "weights": [1.0],
            "dividers": false,
            "reorderable": false,
        }),
        &[],
    );
    let root = ViewNode::new(NodeKind::Stack, "page")
        .child(ViewNode::new(NodeKind::Text, "title"))
        .child(table);
    let grown = expand_with(&root, &inputs(&[("sel-0", SlotValue::Bool(true))]))
        .expect("the table expands with its bound row");
    assert_eq!(grown.units().len(), 1);
    assert_eq!(grown.units()[0].at(), [1]);
    assert_eq!(grown.unit_id(&grown.units()[0]), "/page/table");
}

/// The depth cap refuses before the stack gives out, and a tree just under
/// it expands, on a pinned 2 MiB stack (the test harness default, which
/// `RUST_MIN_STACK` would otherwise move). A `ViewNode` temporary held across
/// `walk`'s recursion once overflowed the stack under `petra-egui`'s
/// `a_tree_that_nests_too_deep_is_refused_not_expanded` and aborted the
/// whole test process instead of refusing.
#[test]
fn the_depth_cap_refuses_before_the_stack_overflows() {
    let chain = |depth: usize| {
        let mut node = ViewNode::new(NodeKind::Stack, "leaf");
        for i in 0..depth {
            node = ViewNode::new(NodeKind::Stack, format!("n{i}")).child(node);
        }
        node
    };
    let run = move || {
        let deep = expand_with(&chain(MAX_DEPTH + 72), &ResolveInputs::new())
            .expect_err("deeper than MAX_DEPTH");
        assert!(deep.to_string().contains("nests deeper"), "{deep}");
        expand_with(&chain(MAX_DEPTH - 1), &ResolveInputs::new())
            .expect("a tree just under MAX_DEPTH expands");
    };
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(run)
        .expect("spawn the pinned-stack thread")
        .join()
        .expect("the walk must refuse, not overflow");
}

#[test]
fn a_commit_regrows_only_the_units_that_read_it() {
    let root = ViewNode::new(NodeKind::Stack, "page")
        .child(bound_toggle("a", "on-a"))
        .child(bound_toggle("b", "on-b"));
    let before = inputs(&[
        ("on-a", SlotValue::Bool(false)),
        ("on-b", SlotValue::Bool(false)),
    ]);
    let grown = expand_with(&root, &before).expect("both toggles expand");
    let after = inputs(&[
        ("on-a", SlotValue::Bool(true)),
        ("on-b", SlotValue::Bool(false)),
    ]);
    let regrowths = grown
        .regrow(&BTreeSet::from(["on-a"]), &after)
        .expect("the regrowth expands");
    assert_eq!(regrowths.len(), 1, "only `a` reads `on-a`");
    assert_eq!(grown.id_of(&regrowths[0]), "/page/a");
    let spliced = grown.with_regrowths(&regrowths);
    let whole = expand_with(&root, &after).expect("a from-scratch expansion");
    assert_eq!(
        &spliced,
        whole.tree(),
        "splicing the regrown unit must land exactly the tree a full expansion draws"
    );
}
