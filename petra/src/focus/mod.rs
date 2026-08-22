//! The focus tree: traversal order, programmatic moves, overlay scopes.
//!
//! # Traversal order is visual order
//!
//! A [`FocusTree`] is built from one frame's [`Placement`]s, which arrive in
//! tree pre-order (`crate::frame::placement::PlacementSink`'s contract) —
//! and tree pre-order is visual order, because every container in
//! `crate::layout` places its children in declaration order (the "shape
//! every container's `place` follows" pattern: push self, then place
//! children in `node.children` order). So the chain is: **declaration order
//! → placement (pre-order) → [`FocusTree::order`]**, with no step in
//! between that could reorder anything. `contracts/semantic-tree.md`'s
//! SC-010 audits exactly this claim ("focus order equals child order"), and
//! [`FocusTree::from_placements`] is the whole implementation of it: it does
//! not sort, it filters the placement slice in place.
//!
//! A placement is focusable when [`PlacementSemantics::actions`] contains
//! [`Interaction::Focus`] and [`PlacementSemantics::disabled`] is `false`
//! (data-model.md §7; disabled nodes are skipped, never removed from the
//! tree — they can become focusable again next frame without changing
//! identity).
//!
//! # Overlay scopes
//!
//! A `surface` placement declaring [`InputPolicy::Block`] (a modal) opens a
//! scope containing itself and its own subtree: while the focused node is
//! inside that scope, [`FocusTree::next`]/[`FocusTree::previous`] cycle only
//! among focusables in it, and [`FocusTree::focus`] refuses to move focus to
//! anything outside it (`FocusError::OutsideActiveScope`). The surface is in
//! its own scope, not merely above it — a modal that is itself focusable
//! would otherwise be the one node from which traversal walks straight out
//! of the trap.
//! `Passthrough` and `DismissOutside` surfaces declare no scope at all —
//! they are geometry and input routing only, and never trap focus.
//!
//! `Placement` carries no `input_policy` (it is a `surface`-only property);
//! [`crate::layout::overlay_surface::surface_scopes`] is where that gap is
//! bridged, and every constructor here takes its output as a second
//! argument. A test that only cares about scoping builds this map by hand
//! (see this module's own tests) — it never needs a real `ViewNode` tree.
//!
//! # The vanished-focus rule
//!
//! [`FocusTree::update`] is what a host calls between frames. When the
//! previously-focused node is no longer focusable in the new frame, focus
//! moves to **the next focusable in the previous frame's order that still
//! exists in the new frame; failing that, the nearest previous one that
//! still exists; failing that, the new frame's first focusable; failing
//! that, the none-state.** This is the one definition this module uses —
//! see [`successor`] — and `never_silently_empty_while_focusables_exist` is
//! the test that pins it down: the only way [`FocusTree::current`] reads
//! `None` is that the new frame has no focusable placement at all.
//!
//! That search runs **inside the blocking scope focus was trapped in**, when
//! that scope still holds a focusable node in the new frame (see
//! [`scoped_successor`]). Without that restriction a node vanishing from an
//! open modal hands focus to whatever comes next globally — outside the
//! modal, which is still blocking — and the trap becomes decorative exactly
//! when it matters. The single case where the trap yields is a surviving
//! modal with nothing focusable left inside it: trapping focus on nothing
//! would drop every keystroke, so the unrestricted rule applies there and
//! the never-silently-empty promise above holds unchanged.

use std::collections::BTreeMap;

use crate::frame::placement::Placement;
use crate::tree::{InputPolicy, Interaction};

/// Exactly one focused node, or the defined none-state; declared traversal
/// order; and the overlay scope that order is currently trapped inside, if
/// any (data-model.md §7).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusTree {
    /// Focusable placement ids, in visual order.
    order: Vec<String>,
    /// For each id in `order`, its blocking-surface ancestors, nearest
    /// first. Empty when the node is not inside any `Block` surface.
    scope_chain: BTreeMap<String, Vec<String>>,
    /// The one focused node, or `None`.
    current: Option<String>,
}

/// Why a programmatic focus move was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusError {
    /// `id` is not a focusable placement in this frame (absent, disabled,
    /// or declares no [`Interaction::Focus`]).
    UnknownId(String),
    /// `id` is focusable, but reaching it would leave the blocking scope
    /// `scope` currently trapping focus.
    OutsideActiveScope {
        /// The id that was requested.
        id: String,
        /// The blocking surface's id whose scope traversal cannot leave.
        scope: String,
    },
}

impl std::fmt::Display for FocusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "{id:?} is not a focusable node in the current frame"),
            Self::OutsideActiveScope { id, scope } => write!(
                f,
                "{id:?} is outside the blocking scope {scope:?} that currently traps focus"
            ),
        }
    }
}

impl std::error::Error for FocusError {}

impl FocusTree {
    /// Build a focus tree fresh, with no prior frame to succeed from.
    ///
    /// `current` starts on the first focusable in `placements`' order, or
    /// the none-state when there are none. `surface_scopes` is
    /// [`crate::layout::overlay_surface::surface_scopes`]'s output for the
    /// same tree that produced `placements`.
    #[must_use]
    pub fn from_placements(
        placements: &[Placement],
        surface_scopes: &BTreeMap<String, InputPolicy>,
    ) -> Self {
        let (order, scope_chain) = index(placements, surface_scopes);
        let current = order.first().cloned();
        Self {
            order,
            scope_chain,
            current,
        }
    }

    /// Rebuild against a new frame's placements, carrying `self.current`
    /// forward: unchanged if it is still focusable, or moved by the
    /// vanished-focus rule (see the module doc) if it is not.
    #[must_use]
    pub fn update(
        &self,
        placements: &[Placement],
        surface_scopes: &BTreeMap<String, InputPolicy>,
    ) -> Self {
        let (order, scope_chain) = index(placements, surface_scopes);
        let current = if self
            .current
            .as_deref()
            .is_some_and(|id| order.iter().any(|o| o == id))
        {
            self.current.clone()
        } else {
            scoped_successor(
                &self.order,
                &self.scope_chain,
                self.current.as_deref(),
                self.active_scope(),
                &order,
                &scope_chain,
            )
        };
        Self {
            order,
            scope_chain,
            current,
        }
    }

    /// The focused node's placement id, or `None` in the none-state.
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Every focusable placement id, in visual order.
    #[must_use]
    pub fn order(&self) -> &[String] {
        &self.order
    }

    /// The blocking surface `current` is trapped inside, or `None` when focus
    /// is not inside any `Block` surface.
    #[must_use]
    pub fn active_scope(&self) -> Option<&str> {
        self.scope_of(self.current.as_deref()?)
    }

    /// The nearest blocking surface `id` is inside, or `None` when it is
    /// inside none — or is not a focusable node this frame.
    ///
    /// [`FocusTree::active_scope`] is this question asked about `current`. A
    /// host needs it asked about other ids too: "is the frontmost open modal
    /// the one focus is in, and if not, which node inside it should focus
    /// move to" is not answerable from `current` alone.
    #[must_use]
    pub fn scope_of(&self, id: &str) -> Option<&str> {
        self.scope_chain.get(id)?.first().map(String::as_str)
    }

    /// Move focus to `id`.
    ///
    /// # Errors
    /// [`FocusError::UnknownId`] when `id` is not focusable this frame;
    /// [`FocusError::OutsideActiveScope`] when a blocking scope is active
    /// and `id` is not inside it — programmatic focus is trapped exactly
    /// like [`FocusTree::next`]/[`FocusTree::previous`], since letting a
    /// host route focus around the trap would make the trap decorative.
    /// Moving deeper into the active scope (including into a further-nested
    /// blocking surface within it) is always allowed; only leaving it is
    /// refused.
    pub fn focus(&mut self, id: &str) -> Result<(), FocusError> {
        if !self.order.iter().any(|o| o == id) {
            return Err(FocusError::UnknownId(id.to_owned()));
        }
        if let Some(scope) = self.active_scope() {
            let reaches = self
                .scope_chain
                .get(id)
                .is_some_and(|chain| chain.iter().any(|s| s == scope));
            if !reaches {
                return Err(FocusError::OutsideActiveScope {
                    id: id.to_owned(),
                    scope: scope.to_owned(),
                });
            }
        }
        self.current = Some(id.to_owned());
        Ok(())
    }

    /// Move to the next focusable in the active scope, wrapping at the end.
    /// A no-op in the none-state.
    pub fn next(&mut self) {
        self.step(1);
    }

    /// Move to the previous focusable in the active scope, wrapping at the
    /// start. A no-op in the none-state.
    pub fn previous(&mut self) {
        self.step(-1);
    }

    /// Move to the first focusable in the active scope. A no-op in the
    /// none-state.
    pub fn first(&mut self) {
        if let Some(id) = self.candidates().first() {
            self.current = Some((*id).to_owned());
        }
    }

    /// Move to the last focusable in the active scope. A no-op in the
    /// none-state.
    pub fn last(&mut self) {
        if let Some(id) = self.candidates().last() {
            self.current = Some((*id).to_owned());
        }
    }

    /// The ids `next`/`previous`/`first`/`last` cycle over: every focusable
    /// whose nearest blocking ancestor matches [`FocusTree::active_scope`]
    /// (both `None` when unscoped), in visual order. `current` is always a
    /// member of its own result when `current` is `Some`, since its scope is
    /// derived from itself.
    fn candidates(&self) -> Vec<&str> {
        let scope = self.active_scope();
        self.order
            .iter()
            .filter(|id| {
                let nearest = self
                    .scope_chain
                    .get(id.as_str())
                    .and_then(|c| c.first())
                    .map(String::as_str);
                nearest == scope
            })
            .map(String::as_str)
            .collect()
    }

    fn step(&mut self, dir: isize) {
        let candidates = self.candidates();
        if candidates.is_empty() {
            return;
        }
        let from = self
            .current
            .as_deref()
            .and_then(|c| candidates.iter().position(|x| *x == c));
        let len = candidates.len() as isize;
        let next = match from {
            Some(p) => (p as isize + dir).rem_euclid(len),
            None => 0,
        };
        self.current = Some(candidates[next as usize].to_owned());
    }
}

/// Build `(order, scope_chain)` from one frame's placements.
fn index(
    placements: &[Placement],
    surface_scopes: &BTreeMap<String, InputPolicy>,
) -> (Vec<String>, BTreeMap<String, Vec<String>>) {
    let mut order = Vec::new();
    let mut scope_chain = BTreeMap::new();
    for (idx, placement) in placements.iter().enumerate() {
        let focusable = placement.semantics.actions.contains(&Interaction::Focus)
            && !placement.semantics.disabled;
        if !focusable {
            continue;
        }
        order.push(placement.id.clone());
        scope_chain.insert(
            placement.id.clone(),
            blocking_scopes(idx, placements, surface_scopes),
        );
    }
    (order, scope_chain)
}

/// Every `Block`-policy surface scope `placements[idx]` is inside, nearest
/// first, found by walking `Placement::parent` — the same index chain
/// `crate::frame::placement::PlacementList` builds from `enter`/`leave`.
///
/// The walk starts at the node itself, not at its parent: a focusable
/// blocking surface is inside the scope it opens. Starting at the parent
/// left that one node reporting no active scope, so traversal from it
/// escaped the modal it *is*.
fn blocking_scopes(
    idx: usize,
    placements: &[Placement],
    surface_scopes: &BTreeMap<String, InputPolicy>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = Some(idx);
    while let Some(p) = cursor {
        let step = &placements[p];
        if surface_scopes.get(&step.id) == Some(&InputPolicy::Block) {
            out.push(step.id.clone());
        }
        cursor = step.parent;
    }
    out
}

/// The vanished-focus successor, restricted to the blocking `scope` focus was
/// trapped in whenever that scope still holds a focusable node in the new
/// frame. See the module doc's "vanished-focus rule".
///
/// Both orders are filtered by the same scope before [`successor`] runs, so
/// the rule itself keeps exactly one definition and this function only
/// decides which nodes it is allowed to choose between.
fn scoped_successor(
    previous_order: &[String],
    previous_chain: &BTreeMap<String, Vec<String>>,
    previous_current: Option<&str>,
    scope: Option<&str>,
    new_order: &[String],
    new_chain: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    let Some(scope) = scope else {
        return successor(previous_order, previous_current, new_order);
    };
    let inside = |chain: &BTreeMap<String, Vec<String>>, id: &String| {
        chain
            .get(id)
            .is_some_and(|c| c.iter().any(|s| s.as_str() == scope))
    };
    let new_inside: Vec<String> = new_order
        .iter()
        .filter(|id| inside(new_chain, id))
        .cloned()
        .collect();
    if new_inside.is_empty() {
        // The modal outlived every focusable it contained. Keeping focus
        // inside it would mean the none-state with focusables on screen and
        // every keystroke dropped; the unrestricted rule wins here.
        return successor(previous_order, previous_current, new_order);
    }
    let previous_inside: Vec<String> = previous_order
        .iter()
        .filter(|id| inside(previous_chain, id))
        .cloned()
        .collect();
    successor(&previous_inside, previous_current, &new_inside)
}

/// The vanished-focus successor: the next id in `previous_order` after
/// `previous_current` that still exists in `new_order`; failing that, the
/// nearest previous one that still exists; failing that, `new_order`'s
/// first; failing that, `None`. See the module doc.
fn successor(
    previous_order: &[String],
    previous_current: Option<&str>,
    new_order: &[String],
) -> Option<String> {
    let still_here = |id: &String| new_order.iter().any(|o| o == id);
    if let Some(cur) = previous_current
        && let Some(pos) = previous_order.iter().position(|o| o == cur)
    {
        if let Some(id) = previous_order[pos + 1..].iter().find(|id| still_here(id)) {
            return Some(id.clone());
        }
        if let Some(id) = previous_order[..pos].iter().rev().find(|id| still_here(id)) {
            return Some(id.clone());
        }
    }
    new_order.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::{FocusError, FocusTree};
    use crate::frame::placement::{PaintState, Placement, PlacementSemantics};
    use crate::geom::Rect;
    use crate::tree::{InputPolicy, Interaction, NodeKind};
    use std::collections::BTreeMap;

    /// A placement carrying enough to drive focus: id, parent, and whether
    /// it is focusable. This is the pattern `frame/placement.rs`'s own tests
    /// use — build `Placement` values directly rather than running layout.
    fn placement(id: &str, parent: Option<usize>, focusable: bool, disabled: bool) -> Placement {
        Placement {
            id: id.into(),
            kind: NodeKind::Text,
            rect: Rect::ZERO,
            z: 0,
            clip: Rect::ZERO,
            opacity: 1.0,
            paint: PaintState::default(),
            semantics: PlacementSemantics {
                actions: if focusable {
                    vec![Interaction::Focus]
                } else {
                    vec![]
                },
                disabled,
                ..PlacementSemantics::default()
            },
            parent,
        }
    }

    fn no_scopes() -> BTreeMap<String, InputPolicy> {
        BTreeMap::new()
    }

    #[test]
    fn traversal_order_equals_placement_order() {
        let placements = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, false),
            placement("/c", None, true, false),
        ];
        let tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(tree.order(), ["/a", "/b", "/c"]);
        assert_eq!(tree.current(), Some("/a"));
    }

    #[test]
    fn next_and_previous_wrap() {
        let placements = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, false),
            placement("/c", None, true, false),
        ];
        let mut tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(tree.current(), Some("/a"));
        tree.next();
        assert_eq!(tree.current(), Some("/b"));
        tree.next();
        assert_eq!(tree.current(), Some("/c"));
        tree.next();
        assert_eq!(
            tree.current(),
            Some("/a"),
            "next wraps past the last focusable"
        );
        tree.previous();
        assert_eq!(
            tree.current(),
            Some("/c"),
            "previous wraps past the first focusable"
        );
    }

    #[test]
    fn disabled_nodes_are_skipped() {
        let placements = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, true),
            placement("/c", None, true, false),
        ];
        let tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(
            tree.order(),
            ["/a", "/c"],
            "the disabled node never enters the order"
        );
    }

    #[test]
    fn non_interactive_nodes_are_not_focusable() {
        let placements = vec![
            placement("/a", None, true, false),
            placement("/label", None, false, false),
        ];
        let tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(tree.order(), ["/a"]);
    }

    /// A blocking modal (`/modal`) with two focusable children; one
    /// focusable sibling (`/outside`) lives outside it entirely.
    fn modal_scenario() -> (Vec<Placement>, BTreeMap<String, InputPolicy>) {
        let placements = vec![
            placement("/outside", None, true, false),         // 0
            placement("/modal", None, false, false), // 1 — the surface itself is not focusable
            placement("/modal/first", Some(1), true, false), // 2
            placement("/modal/second", Some(1), true, false), // 3
        ];
        let mut scopes = BTreeMap::new();
        scopes.insert("/modal".to_owned(), InputPolicy::Block);
        (placements, scopes)
    }

    #[test]
    fn a_blocking_scope_traps_traversal() {
        let (placements, scopes) = modal_scenario();
        let mut tree = FocusTree::from_placements(&placements, &scopes);
        tree.focus("/modal/first").unwrap();
        assert_eq!(tree.active_scope(), Some("/modal"));
        tree.next();
        assert_eq!(tree.current(), Some("/modal/second"));
        tree.next();
        assert_eq!(
            tree.current(),
            Some("/modal/first"),
            "traversal wraps inside the modal's own subtree, never reaching /outside"
        );
        tree.previous();
        assert_eq!(
            tree.current(),
            Some("/modal/second"),
            "previous wraps the same way"
        );
    }

    #[test]
    fn a_blocking_scope_refuses_programmatic_focus_leaving_it() {
        let (placements, scopes) = modal_scenario();
        let mut tree = FocusTree::from_placements(&placements, &scopes);
        tree.focus("/modal/first").unwrap();
        let err = tree.focus("/outside").unwrap_err();
        assert_eq!(
            err,
            FocusError::OutsideActiveScope {
                id: "/outside".into(),
                scope: "/modal".into(),
            }
        );
        assert_eq!(
            tree.current(),
            Some("/modal/first"),
            "a refused move leaves current unchanged"
        );
    }

    #[test]
    fn focus_can_enter_a_blocking_scope_from_outside_it() {
        let (placements, scopes) = modal_scenario();
        let mut tree = FocusTree::from_placements(&placements, &scopes);
        assert_eq!(tree.current(), Some("/outside"));
        tree.focus("/modal/first").unwrap();
        assert_eq!(tree.current(), Some("/modal/first"));
    }

    #[test]
    fn focus_of_an_unknown_id_is_refused() {
        let placements = vec![placement("/a", None, true, false)];
        let mut tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(
            tree.focus("/ghost").unwrap_err(),
            FocusError::UnknownId("/ghost".into())
        );
    }

    /// A passthrough surface's children are ordinary focusables: nothing
    /// traps traversal.
    #[test]
    fn a_passthrough_scope_does_not_trap_traversal() {
        let placements = vec![
            placement("/a", None, true, false),
            placement("/popup", None, false, false),
            placement("/popup/inner", Some(1), true, false),
            placement("/b", None, true, false),
        ];
        let mut scopes = BTreeMap::new();
        scopes.insert("/popup".to_owned(), InputPolicy::Passthrough);
        let mut tree = FocusTree::from_placements(&placements, &scopes);
        assert_eq!(tree.order(), ["/a", "/popup/inner", "/b"]);
        tree.next();
        tree.next();
        tree.next();
        assert_eq!(
            tree.current(),
            Some("/a"),
            "cycling reaches every focusable and wraps freely"
        );
    }

    // -- The vanished-focus rule. -------------------------------------------

    #[test]
    fn vanished_focus_moves_to_the_next_survivor_in_the_previous_order() {
        let before = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, false),
            placement("/c", None, true, false),
        ];
        let mut tree = FocusTree::from_placements(&before, &no_scopes());
        tree.focus("/b").unwrap();

        // /b is gone; /a survives before it in the old order, /c after.
        let after = vec![
            placement("/a", None, true, false),
            placement("/c", None, true, false),
        ];
        let tree = tree.update(&after, &no_scopes());
        assert_eq!(
            tree.current(),
            Some("/c"),
            "the next survivor in the previous order wins"
        );
    }

    #[test]
    fn vanished_focus_falls_back_to_the_previous_survivor_when_nothing_follows() {
        let before = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, false),
            placement("/c", None, true, false),
        ];
        let mut tree = FocusTree::from_placements(&before, &no_scopes());
        tree.focus("/c").unwrap();

        // /c is gone and nothing follows it; /a survives before it.
        let after = vec![placement("/a", None, true, false)];
        let tree = tree.update(&after, &no_scopes());
        assert_eq!(tree.current(), Some("/a"));
    }

    #[test]
    fn vanished_focus_falls_back_to_the_new_frames_first_focusable() {
        let before = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, false),
        ];
        let mut tree = FocusTree::from_placements(&before, &no_scopes());
        tree.focus("/b").unwrap();

        // Neither /a nor /b survives; an entirely new set replaces them.
        let after = vec![
            placement("/x", None, true, false),
            placement("/y", None, true, false),
        ];
        let tree = tree.update(&after, &no_scopes());
        assert_eq!(
            tree.current(),
            Some("/x"),
            "falls back to the new frame's first focusable"
        );
    }

    #[test]
    fn vanished_focus_reaches_the_none_state_only_when_no_focusables_remain() {
        let before = vec![placement("/a", None, true, false)];
        let mut tree = FocusTree::from_placements(&before, &no_scopes());
        tree.focus("/a").unwrap();

        let after: Vec<Placement> = vec![];
        let tree = tree.update(&after, &no_scopes());
        assert_eq!(tree.current(), None);
    }

    #[test]
    fn current_is_never_silently_empty_while_focusables_exist() {
        let before = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, false),
        ];
        let mut tree = FocusTree::from_placements(&before, &no_scopes());
        tree.focus("/a").unwrap();

        // /a vanishes but /b (and a newcomer) remain: current must be Some.
        let after = vec![
            placement("/b", None, true, false),
            placement("/c", None, true, false),
        ];
        let tree = tree.update(&after, &no_scopes());
        assert!(
            tree.current().is_some(),
            "focusables exist; the none-state is not allowed here"
        );
    }

    #[test]
    fn an_unchanged_current_survives_update_untouched() {
        let placements = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, false),
        ];
        let mut tree = FocusTree::from_placements(&placements, &no_scopes());
        tree.focus("/b").unwrap();
        let tree = tree.update(&placements, &no_scopes());
        assert_eq!(
            tree.current(),
            Some("/b"),
            "a still-focusable current is left alone by update"
        );
    }

    /// A modal that outlives one of its own focusables keeps focus inside
    /// itself. The scenario is built so the *global* rule would escape:
    /// `/outside` follows the modal in visual order, so "the next survivor in
    /// the previous order" is outside a modal that still blocks.
    #[test]
    fn a_vanished_focus_inside_an_open_modal_stays_inside_it() {
        let before = vec![
            placement("/modal", None, false, false),          // 0
            placement("/modal/first", Some(0), true, false),  // 1
            placement("/modal/second", Some(0), true, false), // 2
            placement("/outside", None, true, false),         // 3
        ];
        let mut scopes = BTreeMap::new();
        scopes.insert("/modal".to_owned(), InputPolicy::Block);
        let mut tree = FocusTree::from_placements(&before, &scopes);
        tree.focus("/modal/second").unwrap();

        let after = vec![
            placement("/modal", None, false, false),
            placement("/modal/first", Some(0), true, false),
            placement("/outside", None, true, false),
        ];
        let tree = tree.update(&after, &scopes);
        assert_eq!(
            tree.current(),
            Some("/modal/first"),
            "focus left a modal that is still blocking"
        );
        assert_eq!(tree.active_scope(), Some("/modal"));
    }

    /// The one case where the trap yields: the modal survives with nothing
    /// focusable left in it. Trapping focus on nothing would drop every
    /// keystroke, so the unrestricted rule applies and says so.
    #[test]
    fn a_modal_with_no_focusables_left_yields_rather_than_trapping_nothing() {
        let before = vec![
            placement("/modal", None, false, false),
            placement("/modal/only", Some(0), true, false),
            placement("/outside", None, true, false),
        ];
        let mut scopes = BTreeMap::new();
        scopes.insert("/modal".to_owned(), InputPolicy::Block);
        let mut tree = FocusTree::from_placements(&before, &scopes);
        tree.focus("/modal/only").unwrap();

        let after = vec![
            placement("/modal", None, false, false),
            placement("/outside", None, true, false),
        ];
        let tree = tree.update(&after, &scopes);
        assert_eq!(tree.current(), Some("/outside"));
    }

    /// A blocking surface that is itself focusable is inside its own scope,
    /// so traversal from it cycles within the modal instead of walking out.
    #[test]
    fn a_focusable_modal_surface_is_inside_its_own_scope() {
        let placements = vec![
            placement("/modal", None, true, false),
            placement("/modal/first", Some(0), true, false),
            placement("/outside", None, true, false),
        ];
        let mut scopes = BTreeMap::new();
        scopes.insert("/modal".to_owned(), InputPolicy::Block);
        let mut tree = FocusTree::from_placements(&placements, &scopes);
        tree.focus("/modal").unwrap();
        assert_eq!(
            tree.active_scope(),
            Some("/modal"),
            "the surface reported no scope, so it was outside the trap it opens"
        );
        tree.next();
        assert_eq!(tree.current(), Some("/modal/first"));
        tree.next();
        assert_eq!(
            tree.current(),
            Some("/modal"),
            "traversal wrapped out of the modal instead of back to it"
        );
        assert_eq!(
            tree.focus("/outside").unwrap_err(),
            FocusError::OutsideActiveScope {
                id: "/outside".into(),
                scope: "/modal".into(),
            }
        );
    }

    // -- Determinism. ---------------------------------------------------

    #[test]
    fn the_same_placements_produce_the_same_order_a_hundred_times() {
        let placements = vec![
            placement("/a", None, true, false),
            placement("/b", None, true, true),
            placement("/c", None, true, false),
            placement("/d", None, false, false),
        ];
        let first = FocusTree::from_placements(&placements, &no_scopes());
        for _ in 0..100 {
            let again = FocusTree::from_placements(&placements, &no_scopes());
            assert_eq!(again.order(), first.order());
            assert_eq!(again.current(), first.current());
        }
    }

    #[test]
    fn a_tree_with_no_focusables_starts_in_the_none_state() {
        let placements = vec![placement("/label", None, false, false)];
        let tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(tree.current(), None);
        assert!(tree.order().is_empty());
    }

    #[test]
    fn next_and_previous_are_no_ops_in_the_none_state() {
        let placements: Vec<Placement> = vec![];
        let mut tree = FocusTree::from_placements(&placements, &no_scopes());
        tree.next();
        tree.previous();
        tree.first();
        tree.last();
        assert_eq!(tree.current(), None);
    }
}
