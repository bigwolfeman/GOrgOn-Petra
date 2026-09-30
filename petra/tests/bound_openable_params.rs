//! The tristate and visibility openable parameters, judged on the frame
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).
//!
//! `checkbox_tristate.state` drives the indeterminate box from a `Str` slot,
//! and `expanded` on an accordion item decides whether its body exists. Both
//! must fold to exactly the picture the literal reference draws (digest
//! parity), and a commit must graft by key: only the regrown item's nodes
//! and their ancestors move, and a sibling keeps its node and its hashes.

#[path = "bound_components/support.rs"]
mod support;

use std::sync::Arc;

use gorgon_petra::component::registry::{bound::expand_with, expand};
use gorgon_petra::tree::{NodeKind, ResolveInputs, SlotChange, SlotValue, ViewNode};
use serde_json::{Value, json};
use support::{ancestor_of, commit, flag, frame, hashes, moved, reference, seeded, within};

fn text(value: &str) -> SlotValue {
    SlotValue::Str(value.to_owned())
}

/// `name` keyed `key` with `params`, and `param` either literal (`Some`) or
/// bound to `slot` (`None`).
fn one(
    key: &str,
    name: &str,
    params: Value,
    param: &str,
    slot: &str,
    lit: Option<Value>,
) -> ViewNode {
    match lit {
        Some(value) => {
            let mut params = params;
            params[param] = value;
            reference(key, name, params, &[])
        }
        None => reference(key, name, params, &[(param, slot)]),
    }
}

fn tristates(lit: Option<&str>) -> ViewNode {
    let base = |key: &str| json!({ "key": key, "label": format!("Box {key}") });
    ViewNode::new(NodeKind::Stack, "page")
        .child(one(
            "mixed",
            "checkbox_tristate",
            base("mixed"),
            "state",
            "tri",
            lit.map(|s| json!(s)),
        ))
        .child(reference(
            "plain",
            "checkbox_tristate",
            json!({ "key": "plain", "label": "Box plain", "state": "checked" }),
            &[],
        ))
}

/// Every one of the three states folds to the literal picture, first
/// expansion and after a commit regrows it in place.
#[test]
fn a_bound_tristate_digests_the_literal_state_in_every_state() {
    let mut values: ResolveInputs = [("tri".into(), text("mixed"))].into_iter().collect();
    let (mut expanded, mut resolved) = seeded(&tristates(None), &values);
    let literal = |state: &str| expand(&tristates(Some(state))).expect("the literal expands");
    assert_eq!(
        frame(resolved.tree()).digest,
        frame(&literal("mixed")).digest,
        "a bound `mixed` must digest like the literal `mixed`"
    );
    for (version, state) in [(2, "checked"), (4, "unchecked"), (6, "mixed")] {
        commit(
            &mut expanded,
            &mut resolved,
            &mut values,
            &[SlotChange::new("tri", version, text(state))],
        );
        assert_eq!(
            frame(resolved.tree()).digest,
            frame(&literal(state)).digest,
            "after a commit to `{state}` the regrown box digests like the literal"
        );
        assert_eq!(
            resolved.tree(),
            expand_with(&tristates(None), &values)
                .expect("a full expansion")
                .tree(),
            "the regrown tree equals a from-scratch expansion at `{state}`"
        );
    }
}

/// Leaving `mixed` regrows only the bound box; its literal sibling keeps
/// its `Arc` and every hash.
#[test]
fn a_tristate_commit_grafts_only_its_own_box() {
    let mut values: ResolveInputs = [("tri".into(), text("mixed"))].into_iter().collect();
    let (mut expanded, mut resolved) = seeded(&tristates(None), &values);
    let before = frame(resolved.tree());
    let sibling = Arc::clone(&resolved.tree().children[1]);
    let named = commit(
        &mut expanded,
        &mut resolved,
        &mut values,
        &[SlotChange::new("tri", 2, text("checked"))],
    );
    let after = frame(resolved.tree());
    assert!(!named.is_empty(), "leaving mixed changes the box");
    assert!(
        named.iter().all(|id| within(id, "/page/mixed")),
        "only the bound box may be named, named {named:?}"
    );
    assert!(
        Arc::ptr_eq(&sibling, &resolved.tree().children[1]),
        "the literal sibling keeps its node"
    );
    for id in moved(&before, &after) {
        assert!(
            within(&id, "/page/mixed") || ancestor_of(&id, "/page/mixed"),
            "{id} moved, but only the bound box and its ancestors may"
        );
    }
}

fn items(lit: Option<bool>) -> ViewNode {
    let item =
        |key: &str| json!({ "key": key, "label": format!("Item {key}"), "body": "Body text." });
    // The literal sibling sits above the bound item, so the body the bound
    // item mounts cannot shift it: any hash it loses is a graft defect, not
    // layout.
    ViewNode::new(NodeKind::Stack, "page")
        .child(reference(
            "still",
            "accordion_item_spaced",
            json!({ "key": "still", "label": "Item still", "expanded": true, "body": "Stays." }),
            &[],
        ))
        .child(one(
            "open",
            "accordion_item_spaced",
            item("open"),
            "expanded",
            "shown",
            lit.map(|b| json!(b)),
        ))
}

/// Visibility: opening a bound item mounts its body inside the regrown
/// unit, and closing it again removes it; both states digest the literal,
/// and the open sibling keeps every hash throughout.
#[test]
fn a_bound_visibility_mounts_and_unmounts_the_body_by_key() {
    let mut values: ResolveInputs = [("shown".into(), flag(false))].into_iter().collect();
    let (mut expanded, mut resolved) = seeded(&items(None), &values);
    let literal = |open: bool| expand(&items(Some(open))).expect("the literal expands");
    let closed = frame(resolved.tree());
    assert_eq!(closed.digest, frame(&literal(false)).digest);
    assert!(
        !hashes(&closed)
            .keys()
            .any(|id| id.starts_with("/page/open/body")),
        "a closed item mounts no body"
    );

    let sibling = Arc::clone(&resolved.tree().children[0]);
    let named = commit(
        &mut expanded,
        &mut resolved,
        &mut values,
        &[SlotChange::new("shown", 2, flag(true))],
    );
    assert!(
        named.iter().all(|id| within(id, "/page/open")),
        "only the opened item may be named, named {named:?}"
    );
    assert!(
        Arc::ptr_eq(&sibling, &resolved.tree().children[0]),
        "the literal sibling keeps its node"
    );
    let open = frame(resolved.tree());
    assert_eq!(open.digest, frame(&literal(true)).digest);
    let appeared: Vec<String> = moved(&closed, &open).into_iter().collect();
    assert!(
        appeared.iter().any(|id| id.starts_with("/page/open/body")),
        "opening mounts the body, moved {appeared:?}"
    );
    for id in &appeared {
        assert!(
            within(id, "/page/open") || ancestor_of(id, "/page/open"),
            "{id} moved, but only the opened item and its ancestors may"
        );
    }
    let (b, a) = (hashes(&closed), hashes(&open));
    let still: Vec<&String> = b.keys().filter(|id| within(id, "/page/still")).collect();
    assert!(!still.is_empty(), "the sibling item is placed");
    for id in still {
        assert_eq!(b[id], a[id], "sibling placement {id} keeps its hash");
    }

    commit(
        &mut expanded,
        &mut resolved,
        &mut values,
        &[SlotChange::new("shown", 4, flag(false))],
    );
    let shut = frame(resolved.tree());
    assert_eq!(
        shut.digest, closed.digest,
        "closing again draws exactly the first closed picture"
    );
}
