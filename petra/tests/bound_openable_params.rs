//! The tristate and visibility openable parameters, judged on the frame
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).
//!
//! `checkbox_tristate.state` drives the indeterminate box from a `Str` slot,
//! and the disclosure rows — `expanded` on an accordion item or a left-panel
//! icon item, `open` on a menu button, a docked panel or a toggletip,
//! `selected` on an AI label — decide whether their body, menu, tip or
//! explainability panel exists at all. Each
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

/// The `ui_shell_left_panel_icon_item` params of a row keyed `key`, holding
/// one literal subitem child — the left-panel page's Kernel row shape.
fn icon_params(key: &str) -> Value {
    let kid_key = format!("{key}-kid");
    let kid_params = json!({ "key": kid_key.clone(), "label": "Fibers", "selected": false });
    let kid = serde_json::to_value(reference(
        &kid_key,
        "ui_shell_left_panel_icon_subitem",
        kid_params,
        &[],
    ))
    .expect("a subitem serializes");
    json!({
        "key": key,
        "label": "Kernel",
        "mark": "switcher",
        "selected": false,
        "children": [kid],
    })
}

fn icon_items(lit: Option<bool>) -> ViewNode {
    // The literal sibling sits above the bound item, so the nested children
    // the bound item mounts cannot shift it: any hash it loses is a graft
    // defect, not layout.
    let mut still = icon_params("still");
    still["expanded"] = json!(true);
    ViewNode::new(NodeKind::Stack, "page")
        .child(reference(
            "still",
            "ui_shell_left_panel_icon_item",
            still,
            &[],
        ))
        .child(one(
            "open",
            "ui_shell_left_panel_icon_item",
            icon_params("open"),
            "expanded",
            "shown",
            lit.map(|b| json!(b)),
        ))
}

/// Visibility on the left-panel icon item: `expanded` decides whether the
/// nested sub-menu exists at all (`left_panel_item` mounts its `children`
/// stack only while expanded and non-empty), so a commit adds or removes
/// that subtree by key inside the regrown unit; both states digest the
/// literal, and the open sibling keeps every hash throughout.
/// Falsified 2026-10-07 by removing the `("ui_shell_left_panel_icon_item",
/// &[("expanded", ...), ("selected", ...)])` row from `OPENABLE`, which is
/// what that table shipped before the row joined:
///
/// ```text
/// thread 'a_bound_left_panel_visibility_mounts_and_unmounts_the_nested_children_by_key' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "ui_shell_left_panel_icon_item", reason: "parameter `expanded` is not openable: a component expands from literal parameters, and `ui_shell_left_panel_icon_item` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_left_panel_visibility_mounts_and_unmounts_the_nested_children_by_key() {
    let mut values: ResolveInputs = [("shown".into(), flag(false))].into_iter().collect();
    let (mut expanded, mut resolved) = seeded(&icon_items(None), &values);
    let literal = |open: bool| expand(&icon_items(Some(open))).expect("the literal expands");
    let closed = frame(resolved.tree());
    assert_eq!(closed.digest, frame(&literal(false)).digest);
    assert!(
        !hashes(&closed)
            .keys()
            .any(|id| id.starts_with("/page/open/children")),
        "a closed item mounts no nested children"
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
        appeared
            .iter()
            .any(|id| id.starts_with("/page/open/children")),
        "opening mounts the nested children, moved {appeared:?}"
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

/// The property every mount-gate row owes (`name.param` bound to `shown`),
/// judged the way the two disclosure tests above judge theirs: opening
/// mounts the `mounted` subtree by key inside the regrown unit and closing
/// removes it, both states digest the literal, a commit names only the
/// bound row, and the literal open sibling above it keeps its node and
/// every hash throughout. Each caller is its own test with its own
/// falsification; this is the checks, not a sweep.
fn visibility_reexpansion(
    name: &str,
    param: &str,
    mounted: &str,
    rows: fn(Option<bool>) -> ViewNode,
) {
    let mounted = format!("/page/open/{mounted}");
    let mut values: ResolveInputs = [("shown".into(), flag(false))].into_iter().collect();
    let (mut expanded, mut resolved) = seeded(&rows(None), &values);
    let literal = |open: bool| expand(&rows(Some(open))).expect("the literal expands");
    let closed = frame(resolved.tree());
    assert_eq!(
        closed.digest,
        frame(&literal(false)).digest,
        "`{name}.{param}` bound shut digests like the literal shut"
    );
    assert!(
        !hashes(&closed).keys().any(|id| within(id, &mounted)),
        "a shut `{name}` mounts no `{mounted}` subtree"
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
        "only the bound row may be named, named {named:?}"
    );
    assert!(
        Arc::ptr_eq(&sibling, &resolved.tree().children[0]),
        "the literal sibling keeps its node"
    );
    let open = frame(resolved.tree());
    assert_eq!(
        open.digest,
        frame(&literal(true)).digest,
        "`{name}.{param}` bound open digests like the literal open"
    );
    let appeared: Vec<String> = moved(&closed, &open).into_iter().collect();
    assert!(
        appeared.iter().any(|id| within(id, &mounted)),
        "opening mounts the `{mounted}` subtree, moved {appeared:?}"
    );
    for id in &appeared {
        assert!(
            within(id, "/page/open") || ancestor_of(id, "/page/open"),
            "{id} moved, but only the bound row and its ancestors may"
        );
    }
    let (b, a) = (hashes(&closed), hashes(&open));
    let still: Vec<&String> = b.keys().filter(|id| within(id, "/page/still")).collect();
    assert!(!still.is_empty(), "the sibling row is placed");
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
        "closing again draws exactly the first shut picture"
    );
}

/// The `menu_button` params of a row keyed `key`, holding one literal
/// `menu_item` child — the menu-buttons page's Duplicate row shape.
fn menu_params(key: &str) -> Value {
    let kid_key = format!("{key}-kid");
    let kid = serde_json::to_value(reference(
        &kid_key,
        "menu_item",
        json!({ "key": kid_key.clone(), "label": "Duplicate" }),
        &[],
    ))
    .expect("a menu item serializes");
    json!({ "key": key, "label": "More", "children": [kid] })
}

fn menu_rows(lit: Option<bool>) -> ViewNode {
    // The literal sibling sits above the bound row, so the menu the bound
    // row mounts cannot shift it: any hash it loses is a graft defect, not
    // layout.
    let mut still = menu_params("still");
    still["open"] = json!(true);
    ViewNode::new(NodeKind::Stack, "page")
        .child(reference("still", "menu_button", still, &[]))
        .child(one(
            "open",
            "menu_button",
            menu_params("open"),
            "open",
            "shown",
            lit.map(|b| json!(b)),
        ))
}

/// The `docked` params of a row keyed `key`, holding one literal button as
/// its `body` — the drawer page's panel body shape (its policy, edge and
/// body are always declared; only `open` moves).
fn docked_params(key: &str) -> Value {
    let kid_key = format!("{key}-kid");
    let body = serde_json::to_value(reference(
        &kid_key,
        "button",
        json!({ "key": kid_key.clone(), "label": "Close" }),
        &[],
    ))
    .expect("a body serializes");
    json!({
        "key": key,
        "edge": "right",
        "body": body,
        "policy": "passthrough",
    })
}

fn docked_rows(lit: Option<bool>) -> ViewNode {
    // The literal sibling sits above the bound panel; see `menu_rows`.
    let mut still = docked_params("still");
    still["open"] = json!(true);
    ViewNode::new(NodeKind::Stack, "page")
        .child(reference("still", "docked", still, &[]))
        .child(one(
            "open",
            "docked",
            docked_params("open"),
            "open",
            "shown",
            lit.map(|b| json!(b)),
        ))
}

/// The `toggletip_with` params of a row keyed `key`, holding one literal
/// link child — the toggletip page's "Open the trace" action shape.
fn tip_params(key: &str) -> Value {
    let kid_key = format!("{key}-kid");
    let kid = serde_json::to_value(reference(
        &kid_key,
        "link",
        json!({ "key": kid_key.clone(), "label": "Open the trace" }),
        &[],
    ))
    .expect("a link serializes");
    json!({ "key": key, "label": "Why", "children": [kid] })
}

fn tip_rows(lit: Option<bool>) -> ViewNode {
    // The literal sibling sits above the bound tip; see `menu_rows`.
    let mut still = tip_params("still");
    still["open"] = json!(true);
    ViewNode::new(NodeKind::Stack, "page")
        .child(reference("still", "toggletip_with", still, &[]))
        .child(one(
            "open",
            "toggletip_with",
            tip_params("open"),
            "open",
            "shown",
            lit.map(|b| json!(b)),
        ))
}

/// The `ai_label` params of a row keyed `key` — the ai-label page's live
/// row shape (`value` is its explanation, always declared).
fn ai_params(key: &str) -> Value {
    json!({ "key": key, "label": "Confidence score", "value": "Trained on ticket history." })
}

fn ai_rows(lit: Option<bool>) -> ViewNode {
    // The literal sibling sits above the bound label; see `menu_rows`.
    let mut still = ai_params("still");
    still["selected"] = json!(true);
    ViewNode::new(NodeKind::Stack, "page")
        .child(reference("still", "ai_label", still, &[]))
        .child(one(
            "open",
            "ai_label",
            ai_params("open"),
            "selected",
            "shown",
            lit.map(|b| json!(b)),
        ))
}

/// `menu_button.open` is the menu-buttons page's own gate:
/// `menu_button` mounts its `menu` list box — the caller's `children`
/// inside it — only while `open` (`menu_button.rs`: `let mut children =
/// vec![trigger]; if open { children.push(menu("menu", label, items)); }`),
/// so this is a disclosure row and the commit mounts the menu by key.
/// Falsified 2026-10-07 by removing the `("menu_button", &[("open",
/// PropType::Bool)])` row from `OPENABLE`, which is what that table shipped
/// before the row joined:
///
/// ```text
/// thread 'a_bound_menu_button_open_mounts_and_unmounts_the_menu_by_key' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "menu_button", reason: "parameter `open` is not openable: a component expands from literal parameters, and `menu_button` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_menu_button_open_mounts_and_unmounts_the_menu_by_key() {
    visibility_reexpansion("menu_button", "open", "menu", menu_rows);
}

/// `docked.open` is the drawer page's panel gate: `docked` returns a
/// zero-size spacer that mounts no body when shut (`drawer.rs`: `if !open
/// { return closed(key); }`, whose test says "a closed drawer mounts no
/// body"), so the commit adds or removes the panel's `content` subtree by
/// key.
/// Falsified 2026-10-07 by removing the `("docked", &[("open",
/// PropType::Bool)])` row from `OPENABLE`, which is what that table shipped
/// before the row joined:
///
/// ```text
/// thread 'a_bound_docked_open_mounts_and_unmounts_the_body_by_key' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "docked", reason: "parameter `open` is not openable: a component expands from literal parameters, and `docked` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_docked_open_mounts_and_unmounts_the_body_by_key() {
    visibility_reexpansion("docked", "open", "content", docked_rows);
}

/// `toggletip_with.open` is the toggletip page's tip gate:
/// `toggletip_with` mounts its `tip` bubble — the caller's `children`
/// inside it — only while `open` (`toggletip.rs`: `let mut nodes =
/// vec![trigger]; if open { nodes.push(bubble(label, children)); }`), so
/// the commit mounts and unmounts the tip by key.
/// Falsified 2026-10-07 by removing the `("toggletip_with", &[("open",
/// PropType::Bool)])` row from `OPENABLE`, which is what that table shipped
/// before the row joined:
///
/// ```text
/// thread 'a_bound_toggletip_open_mounts_and_unmounts_the_tip_by_key' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "toggletip_with", reason: "parameter `open` is not openable: a component expands from literal parameters, and `toggletip_with` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_toggletip_open_mounts_and_unmounts_the_tip_by_key() {
    visibility_reexpansion("toggletip_with", "open", "tip", tip_rows);
}

/// `ai_label.selected` is the ai-label page's explainability gate: the
/// wire `selected` is the constructor's `open`
/// (`feedback.rs`: `lib::ai_label(p.key, p.label, p.selected, p.value)`),
/// and `default_sized` mounts the `panel` popover only while it is true
/// (`ai_label.rs`: `if open { children.push(explainability_panel(...)) }`).
/// Falsified 2026-10-07 by removing the `("ai_label", &[("selected",
/// PropType::Bool)])` row from `OPENABLE`, which is what that table shipped
/// before the row joined:
///
/// ```text
/// thread 'a_bound_ai_label_selection_mounts_and_unmounts_the_panel_by_key' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "ai_label", reason: "parameter `selected` is not openable: a component expands from literal parameters, and `ai_label` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_ai_label_selection_mounts_and_unmounts_the_panel_by_key() {
    visibility_reexpansion("ai_label", "selected", "panel", ai_rows);
}
