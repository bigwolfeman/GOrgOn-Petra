//! Bound component parameters: expand with slot values, and re-expand one
//! component's subtree when a slot it reads commits.
//!
//! A component reference may bind the parameters its row in
//! [`super::openable`] declares. [`expand_with`] folds each source to a
//! literal and runs the ordinary constructor, and records a [`Unit`] for
//! every *outermost* reference whose parameter tree binds anything — a
//! toggle on its own, or a whole `data_table_zebra_sized` when one of its
//! rows binds `selected`, because the table's constructor wraps and weights
//! the rows it is handed and only re-running it reproduces that. A slot
//! commit then re-runs exactly the units that read it
//! ([`ExpandedTree::regrow`]); nothing else in the tree is expanded again
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).
//!
//! Units never nest: a constructor builds primitives only, so the walk never
//! meets a reference inside a unit's output, and two units are never
//! ancestor and descendant.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::MAX_DEPTH;
use super::expand::{ExpandError, Fold, expand_node, too_deep};
use crate::tree::{Key, KeyPath, NodeKind, ResolveInputs, ViewNode};

/// One outermost component reference whose parameters read slots.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    /// Child-index path of its expansion in [`ExpandedTree::tree`].
    at: Vec<usize>,
    /// The reference as declared: bound sources intact, nested references
    /// in `params` unexpanded.
    reference: ViewNode,
    /// Every slot a bound parameter anywhere in its parameter tree reads.
    slots: BTreeSet<String>,
}

impl Unit {
    /// Child-index path of this unit's expansion.
    #[must_use]
    pub fn at(&self) -> &[usize] {
        &self.at
    }

    /// Every slot this unit re-expands on.
    #[must_use]
    pub fn slots(&self) -> &BTreeSet<String> {
        &self.slots
    }

    /// The reference as declared, bound sources intact — what a host reads
    /// to learn which parameter a slot drives.
    #[must_use]
    pub fn reference(&self) -> &ViewNode {
        &self.reference
    }
}

/// An expanded tree that remembers which subtrees re-expand on which slots.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpandedTree {
    tree: ViewNode,
    units: Vec<Unit>,
}

/// One unit re-expanded against new values, not yet committed.
#[derive(Clone, Debug)]
pub struct Regrowth {
    unit: usize,
    node: ViewNode,
}

impl Regrowth {
    /// The regrown subtree (a declaration: node-level sources still
    /// unfolded).
    #[must_use]
    pub fn node(&self) -> &ViewNode {
        &self.node
    }
}

/// Expand every component reference in `declared`, folding each bound
/// parameter against `inputs` first.
///
/// # Errors
/// [`ExpandError::Param`] for a reference the constructor refuses, a source
/// on a parameter its component does not open, or a parameter declared both
/// literally and bound — defects no slot value repairs.
/// [`ExpandError::Resolve`] for a source that did not evaluate: a slot with
/// no value yet, or a value of a type the parameter does not hold.
pub fn expand_with(
    declared: &ViewNode,
    inputs: &ResolveInputs,
) -> Result<ExpandedTree, ExpandError> {
    let mut units = Vec::new();
    let tree = walk(declared, &mut Vec::new(), inputs, &mut units, 0)?;
    Ok(ExpandedTree { tree, units })
}

fn walk(
    node: &ViewNode,
    at: &mut Vec<usize>,
    inputs: &ResolveInputs,
    units: &mut Vec<Unit>,
    depth: usize,
) -> Result<ViewNode, ExpandError> {
    if depth >= MAX_DEPTH {
        return Err(too_deep(node.key.as_str()).into());
    }
    if node.kind == NodeKind::Component {
        return unit(node, at, inputs, units, depth);
    }
    // Recursion stays in this small frame and the closure's: a `ViewNode`
    // temporary held across the recursive call multiplies a debug frame by
    // the depth, and a tree just under `MAX_DEPTH` then overflows the stack
    // before the depth cap can refuse it.
    let children = node
        .children
        .iter()
        .enumerate()
        .map(|(ix, child)| {
            at.push(ix);
            let grown = walk(child, at, inputs, units, depth + 1).map(Arc::new);
            at.pop();
            grown
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = node.clone();
    out.children = children;
    Ok(out)
}

#[inline(never)]
fn unit(
    node: &ViewNode,
    at: &[usize],
    inputs: &ResolveInputs,
    units: &mut Vec<Unit>,
    depth: usize,
) -> Result<ViewNode, ExpandError> {
    let mut fold = Fold::with(inputs);
    let built = expand_node(node, depth, &mut fold)?;
    if !fold.read.is_empty() {
        units.push(Unit {
            at: at.to_vec(),
            reference: node.clone(),
            slots: fold.read,
        });
    }
    Ok(built)
}

impl ExpandedTree {
    /// The expansion: primitives only, node-level sources still declared.
    #[must_use]
    pub fn tree(&self) -> &ViewNode {
        &self.tree
    }

    /// Every unit, in tree pre-order.
    #[must_use]
    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    /// Give the root a different key — the mount key a host re-roots a
    /// contribution under. Kept here so a unit at the root regrows under
    /// the same key.
    pub fn rekey_root(&mut self, key: Key) {
        self.tree.key = key;
    }

    /// Canonical id of `unit`'s expansion, in this tree's own id space.
    #[must_use]
    pub fn unit_id(&self, unit: &Unit) -> String {
        let mut path = KeyPath::root();
        let mut node = &self.tree;
        path.push(node.key.clone());
        for &ix in &unit.at {
            node = node.children.get(ix).unwrap_or_else(|| {
                panic!(
                    "unit path {:?} names no node: units and the tree are built and spliced \
                     together, so this is an engine bug",
                    unit.at
                )
            });
            path.push(node.key.clone());
        }
        path.id()
    }

    /// Re-expand every unit that reads any slot in `changed`, against
    /// `inputs` — the values the batch leaves behind. Nothing is
    /// committed; [`Self::commit`] lands the result.
    ///
    /// # Errors
    /// The first unit that refuses, with its canonical id.
    pub fn regrow(
        &self,
        changed: &BTreeSet<&str>,
        inputs: &ResolveInputs,
    ) -> Result<Vec<Regrowth>, (String, ExpandError)> {
        let mut out = Vec::new();
        for (ix, unit) in self.units.iter().enumerate() {
            if !unit
                .slots
                .iter()
                .any(|slot| changed.contains(slot.as_str()))
            {
                continue;
            }
            let mut fold = Fold::with(inputs);
            let mut node = expand_node(&unit.reference, unit.at.len(), &mut fold)
                .map_err(|err| (self.unit_id(unit), err))?;
            if unit.at.is_empty() {
                node.key = self.tree.key.clone();
            }
            out.push(Regrowth { unit: ix, node });
        }
        Ok(out)
    }

    /// The expansion with `regrowths` spliced in: what acceptance judges
    /// before anything is committed.
    #[must_use]
    pub fn with_regrowths(&self, regrowths: &[Regrowth]) -> ViewNode {
        let mut tree = self.tree.clone();
        for regrowth in regrowths {
            place(
                &mut tree,
                &self.units[regrowth.unit].at,
                regrowth.node.clone(),
            );
        }
        tree
    }

    /// The child-index path `regrowth` lands at.
    #[must_use]
    pub fn at_of(&self, regrowth: &Regrowth) -> &[usize] {
        &self.units[regrowth.unit].at
    }

    /// Canonical id of the unit `regrowth` re-expanded.
    #[must_use]
    pub fn id_of(&self, regrowth: &Regrowth) -> String {
        self.unit_id(&self.units[regrowth.unit])
    }

    /// Land `tree` — [`Self::with_regrowths`]'s answer, once accepted.
    pub fn commit(&mut self, tree: ViewNode) {
        self.tree = tree;
    }
}

fn place(root: &mut ViewNode, at: &[usize], node: ViewNode) {
    match at.split_first() {
        None => *root = node,
        Some((&ix, rest)) => {
            let child = root.children.get_mut(ix).unwrap_or_else(|| {
                panic!(
                    "unit path {at:?} names no node: units and the tree are built and spliced \
                     together, so this is an engine bug"
                )
            });
            place(Arc::make_mut(child), rest, node);
        }
    }
}

mod regrow;
pub use regrow::{GrownUnit, Regrown};

#[cfg(test)]
mod tests;
