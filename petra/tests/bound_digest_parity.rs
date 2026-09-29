//! Digest parity for the binding fold (design §5 of
//! `.agents/notes/proposed/architecture/2026-09-27-bound-slot-table-ui-model.md`).
//!
//! `contracts/frame-identity.md` pins the v4 Merkle digest over the frame,
//! and every promise about incremental frames, damage and snapshots sits on
//! it. A tree whose properties are `Bind`/`Derive` declarations folds to the
//! literal tree it draws, so it must digest exactly like the same tree
//! authored literally — the declaration is one source of the same picture,
//! never a second picture.

use gorgon_petra::frame::{FrameDigest, PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::testing::{Harness, validated_with};
use gorgon_petra::token::{ThemeMode, standard_vocabulary};
use gorgon_petra::tree::{
    NodeKind, PropKey, PropVal, Registry, ResolveInputs, ResolvedTree, SlotKey, SlotValue,
    ViewNode, carries_bindings,
};

fn frame(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        1,
        validated_with(tree, &Registry::with_vocabulary(standard_vocabulary())),
        &mut harness.ctx(),
        Viewport::new(Size::new(400.0, 200.0), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn dig(tree: &ViewNode) -> FrameDigest {
    frame(tree).digest
}

fn str_value(text: &str) -> SlotValue {
    SlotValue::Str(text.to_owned())
}

/// One text node, `label` from `source` or (when `None`) from the literal
/// `label`. Everything else about the two is identical, so a digest that
/// separates them separates them over the property's origin alone.
fn labelled(key: &str, label: &str, source: Option<PropVal>) -> ViewNode {
    let mut node = ViewNode::new(NodeKind::Text, key);
    node.props.text = Some(format!("{key} body"));
    match source {
        Some(source) => node.bound.set(PropKey::Label, source),
        None => node.semantics.label = Some(label.to_owned()),
    }
    node
}

#[test]
fn a_bound_tree_folded_to_a_value_digests_the_literal_tree() {
    let declared = labelled("t", "Fibers", Some(PropVal::Bind(SlotKey::new("title"))));
    let literal = labelled("t", "Fibers", None);
    let mut inputs = ResolveInputs::new();
    inputs.insert("title".into(), str_value("Fibers"));

    let folded = ResolvedTree::resolve(&declared, &inputs).expect("the fold accepts the binding");
    assert!(carries_bindings(&declared), "the declared tree is bound");
    assert_eq!(
        folded.tree(),
        &literal,
        "the fold must land exactly the literal tree it draws"
    );
    assert_eq!(
        dig(folded.tree()),
        dig(&literal),
        "a bound tree resolved to V must frame-digest like the literal tree \
         carrying V; frame identity may not depend on where a value came from"
    );
}

/// The library's own transitions registered, so a toggle's knob accepts.
fn library_dig(tree: &ViewNode) -> FrameDigest {
    let mut harness = Harness::new();
    let mut registry = Registry::with_vocabulary(standard_vocabulary());
    registry.register_transition(gorgon_petra::anim::TOGGLE_KNOB);
    petrify(
        1,
        validated_with(tree, &registry),
        &mut harness.ctx(),
        Viewport::new(Size::new(400.0, 200.0), ThemeMode::Dark),
        TransitionActivity::default(),
    )
    .digest
}

/// A component whose openable parameter is bound (`ComponentRef::bound`)
/// digests like the same reference written with the literal, both when it
/// first expands and after a commit re-expands it in place — the regrown
/// subtree is the picture a fresh literal expansion draws, not an
/// approximation of it.
#[test]
fn a_bound_component_parameter_digests_the_literal_reference() {
    use gorgon_petra::component::registry::{bound::expand_with, expand};
    use gorgon_petra::tree::{ComponentRef, SlotChange};
    use std::collections::BTreeSet;

    let toggle = |selected: Option<bool>| {
        let mut params = serde_json::json!({ "key": "tg", "label": "Auto-reload" });
        let mut component = ComponentRef::literal("toggle", serde_json::Value::Null);
        match selected {
            Some(on) => params["selected"] = serde_json::json!(on),
            None => {
                component
                    .bound
                    .insert("selected".into(), PropVal::Bind(SlotKey::new("on")));
            }
        }
        component.params = params;
        let mut node = ViewNode::new(NodeKind::Component, "tg");
        node.component = Some(component);
        ViewNode::new(NodeKind::Stack, "page").child(node)
    };
    let literal = |on: bool| expand(&toggle(Some(on))).expect("the literal toggle expands");

    let mut inputs = ResolveInputs::new();
    inputs.insert("on".into(), SlotValue::Bool(false));
    let mut expanded = expand_with(&toggle(None), &inputs).expect("the bound toggle expands");
    let mut resolved = ResolvedTree::resolve(expanded.tree(), &inputs).expect("it folds");
    assert_eq!(
        library_dig(resolved.tree()),
        library_dig(&literal(false)),
        "a bound parameter resolved to V must frame-digest like the literal V"
    );

    inputs.insert("on".into(), SlotValue::Bool(true));
    let regrowths = expanded
        .regrow(&BTreeSet::from(["on"]), &inputs)
        .expect("the toggle regrows");
    resolved
        .apply_slot_changes(&[SlotChange::new("on", 1, SlotValue::Bool(true))])
        .expect("the value lands");
    for regrowth in &regrowths {
        resolved
            .splice(expanded.at_of(regrowth), regrowth.node())
            .expect("the regrown toggle folds");
    }
    expanded.commit(expanded.with_regrowths(&regrowths));
    assert_eq!(
        library_dig(resolved.tree()),
        library_dig(&literal(true)),
        "after a commit the regrown subtree must digest like the literal it now says"
    );
}

#[test]
fn a_derived_value_digests_the_literal_tree_that_says_the_same_thing() {
    use gorgon_petra::tree::DeriveExpr;
    use std::collections::BTreeMap;

    let mut args = BTreeMap::new();
    args.insert("who".into(), PropVal::Bind(SlotKey::new("who")));
    let declared = labelled(
        "t",
        "",
        Some(PropVal::Derive(DeriveExpr::Fmt {
            format: "{who} has 3 rows".to_owned(),
            args,
        })),
    );
    let literal = labelled("t", "Fen has 3 rows", None);
    let mut inputs = ResolveInputs::new();
    inputs.insert("who".into(), str_value("Fen"));

    let folded = ResolvedTree::resolve(&declared, &inputs).expect("the fold accepts the derive");
    assert_eq!(folded.tree(), &literal);
    assert_eq!(
        dig(folded.tree()),
        dig(&literal),
        "a `fmt` derivation must frame-digest like the literal string it renders"
    );
}
