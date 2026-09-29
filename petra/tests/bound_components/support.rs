//! Frame, hash and commit helpers for `bound_components.rs`.

use std::collections::{BTreeMap, BTreeSet};

use gorgon_petra::anim::{BUTTON_PRESS, TOGGLE_KNOB};
use gorgon_petra::component::registry::bound::{ExpandedTree, expand_with};
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::testing::{Harness, validated_with};
use gorgon_petra::token::{ThemeMode, standard_vocabulary};
use gorgon_petra::tree::{
    ComponentRef, NodeKind, PropVal, Registry, ResolveInputs, ResolvedTree, SlotChange, SlotKey,
    SlotValue, ViewNode,
};

pub fn frame(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    let mut registry = Registry::with_vocabulary(standard_vocabulary());
    registry.register_transition(TOGGLE_KNOB);
    registry.register_transition(BUTTON_PRESS);
    petrify(
        1,
        validated_with(tree, &registry),
        &mut harness.ctx(),
        Viewport::new(Size::new(640.0, 400.0), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

pub fn reference(
    key: &str,
    name: &str,
    params: serde_json::Value,
    bound: &[(&str, &str)],
) -> ViewNode {
    let mut node = ViewNode::new(NodeKind::Component, key);
    let mut component = ComponentRef::literal(name, params);
    for (param, slot) in bound {
        component
            .bound
            .insert((*param).to_owned(), PropVal::Bind(SlotKey::new(*slot)));
    }
    node.component = Some(component);
    node
}

pub fn flag(on: bool) -> SlotValue {
    SlotValue::Bool(on)
}

/// Placement id → subtree hash.
pub fn hashes(frame: &PetrifiedFrame) -> BTreeMap<String, [u8; 32]> {
    frame
        .placements
        .iter()
        .zip(&frame.subtree_hashes)
        .map(|(p, h)| (p.id.clone(), *h))
        .collect()
}

/// Every id whose subtree hash differs between the frames, or that only
/// one frame places.
pub fn moved(before: &PetrifiedFrame, after: &PetrifiedFrame) -> BTreeSet<String> {
    let (a, b) = (hashes(before), hashes(after));
    a.keys()
        .chain(b.keys())
        .filter(|id| a.get(*id) != b.get(*id))
        .cloned()
        .collect()
}

pub fn within(id: &str, root: &str) -> bool {
    id == root || id.starts_with(&format!("{root}/"))
}

pub fn ancestor_of(id: &str, root: &str) -> bool {
    root.starts_with(&format!("{id}/")) || id == "/"
}

/// What a host does with one batch: regrow the units that read it, apply
/// the values, splice each regrown unit. Answers the ids the splice named.
pub fn commit(
    expanded: &mut ExpandedTree,
    resolved: &mut ResolvedTree,
    values: &mut ResolveInputs,
    changes: &[SlotChange],
) -> Vec<String> {
    for change in changes {
        values.insert(change.slot.as_str().to_owned(), change.value.clone());
    }
    let slots: BTreeSet<&str> = changes.iter().map(|c| c.slot.as_str()).collect();
    let regrowths = expanded.regrow(&slots, values).expect("the units regrow");
    let candidate = expanded.with_regrowths(&regrowths);
    resolved
        .apply_slot_changes(changes)
        .expect("the values land");
    let mut named = Vec::new();
    for regrowth in &regrowths {
        named.extend(
            resolved
                .splice(expanded.at_of(regrowth), regrowth.node())
                .expect("the regrown unit folds"),
        );
    }
    expanded.commit(candidate);
    named
}

pub fn seeded(declared: &ViewNode, values: &ResolveInputs) -> (ExpandedTree, ResolvedTree) {
    let expanded = expand_with(declared, values).expect("the page expands");
    let resolved = ResolvedTree::resolve(expanded.tree(), values).expect("the page folds");
    (expanded, resolved)
}
