//! Preparing a contribution, and regrowing the components whose bound
//! parameters a slot batch moved.
//!
//! A child of `host` so it reaches `Host`'s fields; split out because both
//! halves are one concern — what a contribution's expansion is, now and after
//! a commit — and `host.rs` is already far past the size a reader can hold
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).

use std::collections::BTreeSet;

use gorgon_petra::component::registry::ExpandError;
use gorgon_petra::component::registry::bound::{ExpandedTree, Regrown, expand_with};
use gorgon_petra::semantic::contribution_key;
use gorgon_petra::tree::{
    ApplyError, Registry, ResolveInputs, ResolvedTree, carries_bindings, validate,
};

use super::{App, Contribution, Host, Prepared};

impl<A: App> Host<A> {
    /// The acceptance this host judges every contribution by — read-only,
    /// so a shell-side runtime can expand and regrow against the same rules
    /// without marking the contributions for re-preparation the way
    /// [`Host::registry_mut`] must.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Expand one contribution against the held slot values, put it
    /// through stage-2 acceptance, and fold it.
    ///
    /// **Expand, then validate** — see [`Host::prepare_contributions`]. A
    /// bound component parameter folds during expansion, so a slot that has
    /// no value yet refuses here, as the same re-trying card a node-level
    /// fold refusal is (design §8's snapshot-first order); a defect no value
    /// repairs (an unknown component, a source on a parameter the component
    /// does not open) is a permanent refusal.
    pub(super) fn prepare_one(&self, contribution: &Contribution) -> Prepared {
        let refused = |reason: String| Prepared {
            id: contribution.id,
            slot: contribution.slot.clone(),
            node: contribution.tree.clone(),
            refused: Some(reason),
            fold_refused: None,
            binds: false,
            resolved: None,
            expanded: None,
        };
        let mut expanded = match expand_with(&contribution.tree, &self.slots) {
            Ok(expanded) => expanded,
            Err(ExpandError::Param(err)) => {
                return refused(format!("{}: {}", err.component, err.reason));
            }
            Err(ExpandError::Resolve(err)) => {
                return Prepared {
                    id: contribution.id,
                    slot: contribution.slot.clone(),
                    node: contribution.tree.clone(),
                    refused: None,
                    fold_refused: Some(err.to_string()),
                    binds: true,
                    resolved: None,
                    expanded: None,
                };
            }
        };
        if let Err(errors) = validate(expanded.tree(), &self.registry) {
            return refused(errors.to_string());
        }
        // The mount key, set before the fold rather than only at mount, so
        // the reverse index reports `ui:<id>/<path>` — the tail of the id the
        // frame will carry — instead of the declaration's own root key.
        // `keyed` sets this same key again at mount, where it also carries
        // the staleness flags; this is the same key, earlier.
        expanded.rekey_root(contribution_key(contribution.id));
        let node = expanded.tree().clone();
        let regrows = !expanded.units().is_empty();
        // A contribution with regrowable components always keeps a fold,
        // even when no node-level property binds: the retained tree is what a
        // regrown subtree is grafted onto.
        let binds = carries_bindings(&node) || regrows;
        let (resolved, fold_refused) = if binds {
            match ResolvedTree::resolve(&node, &self.slots) {
                Ok(resolved) => (Some(resolved), None),
                Err(err) => (None, Some(err.to_string())),
            }
        } else {
            (None, None)
        };
        Prepared {
            id: contribution.id,
            slot: contribution.slot.clone(),
            node,
            refused: None,
            fold_refused,
            binds,
            resolved,
            expanded: regrows.then_some(expanded),
        }
    }

    /// Re-expand the units of `expanded` that read a slot in `changed` and
    /// splice them into `next`, accepted against this host's registry — the
    /// shared [`ExpandedTree::regrow_into`], which owns the rules.
    pub(super) fn regrow(
        &self,
        expanded: &ExpandedTree,
        next: &mut ResolvedTree,
        changed: &BTreeSet<&str>,
        after: &ResolveInputs,
    ) -> Result<Option<Regrown>, ApplyError> {
        expanded.regrow_into(next, changed, after, &self.registry)
    }
}
