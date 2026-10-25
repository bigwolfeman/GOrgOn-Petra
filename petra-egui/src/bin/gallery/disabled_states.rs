//! Spec 008 US5: the disabled treatment, read back out of the built gallery
//! cells (T026–T028).
//!
//! One question home, the seam [`Catalog::disabled_cells`] is
//! (`catalog.rs`): `page_body` places those cells, `lua_parity` seeds its
//! mirror from the same call, the shots photograph them, and this module
//! asserts the treatment on them — so a page, a mirror, a picture and a
//! test cannot disagree about which component has a disabled state or what
//! it contains.
//!
//! # What "the treatment" is, and why every assertion below has teeth
//!
//! Three channels, and a test that only asserts the node exists proves none
//! of them (`tasks.md` T026: "a test that only asserts the node exists is
//! the kind this spec was written to prevent"):
//!
//! 1. **the declaration** — every node under the wrapper says
//!    `Semantics.disabled` and declares no interaction at all, so no hover,
//!    press, capture or focus can land on it;
//! 2. **the ink** — every label resolves `foreground@disabled` to
//!    `text.muted`, the binding `contracts/disabled.md` records. Delete the
//!    binding and the label falls back to live `text.primary` (the test
//!    fails); re-point it at Carbon's `$text-disabled` and the test fails
//!    too — that token is 1.31:1 and T027 measured it out of the label
//!    channel for exactly that reason;
//! 3. **the chrome** — every `*@disabled` binding in the subtree is one of
//!    the four register tokens, each with its resting half (the resting-
//!    binding invariant, per node).
//!
//! The per-state cases live in `disabled_states/slice_a.rs` through
//! `slice_f.rs` — one test per manifest row
//! (`.agents/research/08-25-2026/Carbon-Component-Inventory/VARIANTS.md`'s
//! `| state | disabled |` rows, plus Form's `| state | disabled-fieldset |`),
//! naming the component's own anatomy rather than sharing one generic
//! assertion over every page: which cells a component exhibits its state
//! through is part of what the test protects. `disabled_states/probes.rs`
//! holds T027's measurement across all of them and T028's interaction and
//! focus assertions.

mod probes;
mod slice_a;
mod slice_b;
mod slice_c;
mod slice_d;
mod slice_e;
mod slice_f;

use gorgon_petra::token::measure::slot_map;
use gorgon_petra::token::{DerivedState, InteractionRank, resolve_slot, resolve_state};
use gorgon_petra::tree::{NodeKind, ViewNode};

use crate::catalog::Catalog;

/// The register's disabled tokens (`contracts/disabled.md` §Binding
/// register): the three chrome families, plus the label ink.
const REGISTER: [&str; 4] = [
    "text.muted",
    "icon-disabled",
    "icon-on-color-disabled",
    "button-disabled",
];

/// WCAG 2.1 SC 1.4.3 at AA for body text — the floor T027 measures against.
const FLOOR: f32 = 4.5;

/// The page's built disabled cell(s), one per state it exhibits.
///
/// # Panics
/// If the page exhibits none — a manifest row with no gallery cell is a T025
/// gap, and drawing nothing in its place would hide it.
fn cells(component: &str) -> Vec<ViewNode> {
    let cells = Catalog::on_page(component).disabled_cells();
    assert!(
        !cells.is_empty(),
        "{component}: the page exhibits no disabled cell, so the state the \
         manifest documents has no gallery cell (T025)"
    );
    cells
}

/// Assert the disabled treatment on `root` and everything under it — the
/// whole cell is the disabled state.
fn assert_disabled_state(what: &str, root: &ViewNode) {
    assert!(
        root.semantics.disabled,
        "{what}: the root is not declared disabled — `component::disabled()` \
         is no longer applied to this cell"
    );
    let mut labels = 0usize;
    assert_treatment(what, root, &mut labels);
    assert!(
        labels > 0,
        "{what}: the cell draws no label, so the label-ink assertions \
         measured nothing"
    );
}

/// Assert the treatment on the one disabled subtree inside `root`, and that
/// nothing outside it claims the state — Form's disabled fieldset, where a
/// disabled field sits inside a live form (manifest row 223).
///
/// # Panics
/// If the cell holds no disabled subtree (the field lost its wrapper), if it
/// holds more than one (the cell should exhibit one fieldset, not a
/// scatter), or if anything outside them claims the state.
fn assert_disabled_state_within(what: &str, root: &ViewNode) {
    let disabled_roots = maximal_disabled_roots(root);
    assert_eq!(
        disabled_roots.len(),
        1,
        "{what}: expected exactly one disabled subtree, found {}",
        disabled_roots.len()
    );
    let mut labels = 0usize;
    assert_treatment(what, disabled_roots[0], &mut labels);
    assert!(labels > 0, "{what}: the disabled subtree draws no label");
    assert_live_outside(what, root);
}

/// Every node under `node` that is disabled while its parent is not.
fn maximal_disabled_roots(node: &ViewNode) -> Vec<&ViewNode> {
    fn walk<'n>(node: &'n ViewNode, inside: bool, out: &mut Vec<&'n ViewNode>) {
        let now = inside || node.semantics.disabled;
        if node.semantics.disabled && !inside {
            out.push(node);
        }
        for child in &node.children {
            walk(child, now, out);
        }
    }
    let mut out = Vec::new();
    walk(node, false, &mut out);
    out
}

/// Nothing outside a disabled subtree claims the state, and nothing outside
/// one carries half a disabled binding.
fn assert_live_outside(what: &str, node: &ViewNode) {
    if !node.semantics.disabled {
        assert_no_half_bindings(what, node);
    }
    for child in &node.children {
        if !child.semantics.disabled {
            assert_live_outside(what, child);
        }
    }
}

/// The declaration channel plus the token channels, one node.
fn assert_treatment(what: &str, node: &ViewNode, labels: &mut usize) {
    assert!(
        node.semantics.disabled,
        "{what}: `{}` is under the disabled wrapper but does not declare it — \
         `Semantics.disabled` does not inherit, so every node must say so",
        node.key.as_str()
    );
    assert_eq!(
        resolve_state(state_of(node)),
        InteractionRank::Disabled,
        "{what}: `{}` does not resolve the Disabled rank",
        node.key.as_str()
    );
    assert!(
        node.interactions.is_empty(),
        "{what}: `{}` is disabled but still declares {:?} — a disabled node \
         takes no interaction state and is not focusable (T028)",
        node.key.as_str(),
        node.interactions
    );
    assert_no_half_bindings(what, node);

    let slots = slot_map(&node.props.tokens);
    let ink = resolve_slot(&slots, "foreground", state_of(node));
    if node.kind == NodeKind::Text {
        assert_eq!(
            ink,
            Some("text.muted"),
            "{what}: `{}` draws text with disabled ink {ink:?}, not the \
             register's `text.muted` (T024, T027)",
            node.key.as_str()
        );
        *labels += 1;
    } else if let Some(token) = ink {
        assert_eq!(
            token,
            "text.muted",
            "{what}: `{}` resolves disabled ink `{token}`, not the register's \
             `text.muted` (T024, T027)",
            node.key.as_str()
        );
        *labels += 1;
    }
    for child in &node.children {
        assert_treatment(what, child, labels);
    }
}

/// One node's declared state as the token layer sees it.
///
/// [`DerivedState::of`] reads a *placement*'s semantics (the host copies
/// them across at layout); a built cell is still a tree, so this is the
/// same projection over `gorgon_petra::tree::Semantics`.
fn state_of(node: &ViewNode) -> DerivedState {
    DerivedState {
        skeleton: node.semantics.skeleton,
        disabled: node.semantics.disabled,
        selected: node.semantics.selected,
        read_only: node.semantics.read_only,
        ..DerivedState::default()
    }
}

/// Every `@disabled` binding on `node` is a register token and carries its
/// resting half — the invariant `component::tests` holds repo-wide, named
/// per component here so the failure says which cell lost it.
fn assert_no_half_bindings(what: &str, node: &ViewNode) {
    for (slot, token) in &node.props.tokens {
        let Some(resting) = slot.strip_suffix("@disabled") else {
            continue;
        };
        assert!(
            REGISTER.contains(&token.as_str()),
            "{what}: `{}` binds {slot} -> {token}, outside the register \
             {REGISTER:?}",
            node.key.as_str()
        );
        assert!(
            node.props.tokens.contains_key(resting),
            "{what}: `{}` binds {slot} without its resting `{resting}`",
            node.key.as_str()
        );
    }
}
