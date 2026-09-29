//! Bound component parameters, judged on the frame
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).
//!
//! A slot commit re-expands the one component whose parameter reads it and
//! nothing else. What proves "nothing else" is the frame's own Merkle
//! subtree hashes: a placement outside the regrown component keeps the hash
//! it had, and inside it only the nodes whose content moved (and their
//! ancestors, whose hash folds a changed child) change.

#[path = "bound_components/support.rs"]
mod support;

use std::sync::Arc;

use gorgon_petra::component::registry::bound::expand_with;
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::tree::{NodeKind, ResolveInputs, SlotChange, ViewNode};
use serde_json::json;
use support::{ancestor_of, commit, flag, frame, hashes, moved, reference, seeded, within};

fn toggles() -> ViewNode {
    let toggle = |key: &str, slot: &str| {
        reference(
            key,
            "toggle",
            json!({ "key": key, "label": format!("Toggle {key}") }),
            &[("selected", slot)],
        )
    };
    ViewNode::new(NodeKind::Stack, "page")
        .child(toggle("a", "on-a"))
        .child(toggle("b", "on-b"))
}

#[test]
fn a_bound_toggle_commit_re_expands_only_its_own_subtree() {
    let mut values: ResolveInputs = [("on-a".into(), flag(false)), ("on-b".into(), flag(false))]
        .into_iter()
        .collect();
    let (mut expanded, mut resolved) = seeded(&toggles(), &values);
    let before = frame(resolved.tree());
    let sibling = Arc::clone(&resolved.tree().children[1]);

    let named = commit(
        &mut expanded,
        &mut resolved,
        &mut values,
        &[SlotChange::new("on-a", 1, flag(true))],
    );
    let after = frame(resolved.tree());

    assert!(
        !named.is_empty(),
        "flipping the toggle must change something"
    );
    assert!(
        named.iter().all(|id| within(id, "/page/a")),
        "the splice may only name nodes of the regrown toggle, named {named:?}"
    );
    assert!(
        Arc::ptr_eq(&sibling, &resolved.tree().children[1]),
        "the sibling toggle keeps its node, pointer for pointer"
    );
    let moved = moved(&before, &after);
    assert!(
        moved.iter().any(|id| within(id, "/page/a")),
        "the flipped toggle's hashes must move, moved {moved:?}"
    );
    for id in &moved {
        assert!(
            within(id, "/page/a") || ancestor_of(id, "/page/a"),
            "{id} moved, but it is neither the regrown toggle nor an ancestor of it"
        );
    }
    let b = hashes(&before);
    let a = hashes(&after);
    let siblings: Vec<&String> = b.keys().filter(|id| within(id, "/page/b")).collect();
    assert!(!siblings.is_empty(), "the sibling toggle is placed");
    for id in siblings {
        assert_eq!(b[id], a[id], "sibling placement {id} must keep its hash");
    }
}

fn table() -> ViewNode {
    let cell =
        |key: &str, text: &str| json!({ "kind": "text", "key": key, "props": { "text": text } });
    let row = |key: &str, slot: &str| {
        serde_json::to_value(reference(
            key,
            "data_table_row_sm",
            json!({ "key": key, "children": [cell(&format!("{key}-name"), key)] }),
            &[("selected", slot)],
        ))
        .expect("a row serializes")
    };
    let header = serde_json::to_value(reference(
        "sort",
        "data_table_sort_header",
        json!({ "key": "sort", "label": "Name" }),
        &[("selected", "ascending")],
    ))
    .expect("a header serializes");
    let table = reference(
        "table",
        "data_table_zebra_sized",
        json!({
            "key": "table",
            "header": [header],
            "rows": [row("r0", "sel-0"), row("r1", "sel-1"), row("r2", "sel-2")],
            "weights": [1.0],
            "dividers": false,
            "reorderable": false,
        }),
        &[],
    );
    ViewNode::new(NodeKind::Stack, "page")
        .child(reference(
            "intro",
            "toggle",
            json!({ "key": "intro", "label": "Unrelated", "selected": false }),
            &[],
        ))
        .child(table)
}

fn table_values() -> ResolveInputs {
    [
        ("sel-0".into(), flag(false)),
        ("sel-1".into(), flag(false)),
        ("sel-2".into(), flag(false)),
        ("ascending".into(), flag(true)),
    ]
    .into_iter()
    .collect()
}

/// The frame id of the outermost placement whose id ends in `/{key}`.
fn id_of(frame: &PetrifiedFrame, key: &str) -> String {
    let tail = format!("/{key}");
    frame
        .placements
        .iter()
        .map(|p| p.id.as_str())
        .filter(|id| id.ends_with(&tail))
        .min_by_key(|id| id.len())
        .unwrap_or_else(|| panic!("no placement ends in {tail}"))
        .to_owned()
}

#[test]
fn a_bound_row_selection_regrows_the_table_and_moves_only_that_row() {
    let mut values = table_values();
    let (mut expanded, mut resolved) = seeded(&table(), &values);
    assert_eq!(expanded.units().len(), 1, "the table is the one unit");
    let before = frame(resolved.tree());
    let row = id_of(&before, "r1");

    let named = commit(
        &mut expanded,
        &mut resolved,
        &mut values,
        &[SlotChange::new("sel-1", 1, flag(true))],
    );
    let after = frame(resolved.tree());

    // The table's constructor derives the header's select-all box from its
    // rows (one selected row makes it indeterminate), which is exactly why
    // the unit is the whole table and not the row: re-running the row alone
    // would leave that box lying.
    let select_all = id_of(&before, "select-all");
    let allowed = [row.as_str(), select_all.as_str()];
    assert!(!named.is_empty());
    assert!(
        named
            .iter()
            .all(|id| allowed.iter().any(|root| within(id, root))),
        "only the selected row and the select-all box may be named, named {named:?}"
    );
    assert!(
        named.iter().any(|id| within(id, &select_all)),
        "the select-all box must follow the selection, named {named:?}"
    );
    let moved = moved(&before, &after);
    assert!(moved.contains(&row), "the row's own hash moves");
    for id in &moved {
        assert!(
            allowed
                .iter()
                .any(|root| within(id, root) || ancestor_of(id, root)),
            "{id} moved, but only row {row}, the select-all box and their ancestors may"
        );
    }
    for key in ["r0", "r2", "intro"] {
        let untouched = id_of(&after, key);
        assert!(!moved.contains(&untouched), "{untouched} keeps its hash");
    }
    assert_eq!(
        resolved.tree(),
        expand_with(&table(), &values)
            .expect("a full expansion")
            .tree(),
        "the regrown table must equal a from-scratch expansion at the same values"
    );
}

#[test]
fn a_bound_sort_direction_regrows_the_header_and_leaves_every_row() {
    let mut values = table_values();
    let (mut expanded, mut resolved) = seeded(&table(), &values);
    let before = frame(resolved.tree());
    let header = id_of(&before, "sort");

    commit(
        &mut expanded,
        &mut resolved,
        &mut values,
        &[SlotChange::new("ascending", 1, flag(false))],
    );
    let after = frame(resolved.tree());

    let moved = moved(&before, &after);
    assert!(moved.contains(&header), "the header's hash moves");
    for id in &moved {
        assert!(
            within(id, &header) || ancestor_of(id, &header),
            "{id} moved, but only the sort header {header} and its ancestors may"
        );
    }
    for key in ["r0", "r1", "r2"] {
        let row = id_of(&after, key);
        assert!(!moved.contains(&row), "row {row} keeps its hash");
    }
}
