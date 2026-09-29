//! Regrow the units a slot batch reaches, accept the result, and graft it
//! onto a retained fold.
//!
//! One implementation for every retained-tree host: the egui `Host`'s
//! `apply_slot_changes` and the shell-side view runtime
//! (`gorgon-view-fiber`) both land a batch through
//! [`ExpandedTree::regrow_into`], so the two cannot disagree about what a
//! regrown component looks like
//! (`.agents/notes/implemented/architecture/2026-09-28-gallery-shell-slot-client.md`).

use std::collections::BTreeSet;

use super::ExpandedTree;
use crate::tree::{ApplyError, Registry, ResolveInputs, ResolvedTree, ViewNode, validate};

/// One component that re-expanded.
#[derive(Clone, Debug, PartialEq)]
pub struct GrownUnit {
    /// Canonical id of the component's expansion, in the tree's own id space.
    pub id: String,
    /// Child-index path the regrown subtree landed at.
    pub at: Vec<usize>,
    /// Canonical ids of the nodes inside it whose content changed.
    pub named: Vec<String>,
}

/// What one tree's regrowth produced, before the caller commits it.
#[derive(Clone, Debug)]
pub struct Regrown {
    /// The expansion with every regrown unit spliced in (a declaration) —
    /// what [`ExpandedTree::commit`] takes once every tree accepted.
    pub expansion: ViewNode,
    /// Every component that re-expanded, in tree pre-order.
    pub units: Vec<GrownUnit>,
}

impl Regrown {
    /// Every changed node across every unit.
    pub fn named(&self) -> impl Iterator<Item = &str> {
        self.units
            .iter()
            .flat_map(|unit| unit.named.iter().map(String::as_str))
    }
}

impl ExpandedTree {
    /// Whether any unit re-expands on a slot in `changed` — the cheap probe
    /// a caller runs before copying the values a regrowth expands against.
    #[must_use]
    pub fn reads_any(&self, changed: &BTreeSet<&str>) -> bool {
        self.units()
            .iter()
            .any(|unit| unit.slots().iter().any(|s| changed.contains(s.as_str())))
    }

    /// Re-expand the units that read a slot in `changed`, against `after`
    /// (the values the batch leaves behind), accept the whole expansion
    /// against `registry`, and splice each regrown subtree into `next`.
    ///
    /// Nothing but `next` is touched: the caller commits
    /// [`Regrown::expansion`] only when every retained tree accepted the
    /// batch. Acceptance runs over the whole tree, because a regrown subtree
    /// can break a rule whose referent lives outside it (an anchor, a sibling
    /// key). `Ok(None)` when no unit reads a changed slot.
    ///
    /// # Errors
    /// [`ApplyError::Regrow`] when a unit's constructor refuses the new
    /// values or the regrown tree fails acceptance; the splice's own
    /// [`ApplyError`] when a regrown subtree does not fold.
    pub fn regrow_into(
        &self,
        next: &mut ResolvedTree,
        changed: &BTreeSet<&str>,
        after: &ResolveInputs,
        registry: &Registry,
    ) -> Result<Option<Regrown>, ApplyError> {
        let regrowths =
            self.regrow(changed, after)
                .map_err(|(component, err)| ApplyError::Regrow {
                    component,
                    reason: err.to_string(),
                })?;
        if regrowths.is_empty() {
            return Ok(None);
        }
        let expansion = self.with_regrowths(&regrowths);
        validate(&expansion, registry).map_err(|errors| ApplyError::Regrow {
            component: regrowths
                .iter()
                .map(|r| self.id_of(r))
                .collect::<Vec<_>>()
                .join(", "),
            reason: errors.to_string(),
        })?;
        let mut units = Vec::with_capacity(regrowths.len());
        for regrowth in &regrowths {
            let at = self.at_of(regrowth).to_vec();
            let named = next.splice(&at, regrowth.node())?;
            units.push(GrownUnit {
                id: self.id_of(regrowth),
                at,
                named,
            });
        }
        Ok(Some(Regrown { expansion, units }))
    }
}
