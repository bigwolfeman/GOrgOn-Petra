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
//! [`Interaction::Focus`], the node is not disabled, and
//! [`Placement::is_visible`] — the same clip the pointer already honours
//! (`crate::input::hit_test`). An overscan row is placed so scrolling is
//! smooth; it is not Tab-reachable. Disabled nodes are skipped, never
//! removed from the tree — they can become focusable again next frame
//! without changing identity (data-model.md §7).
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
use crate::geom::Rect;
use crate::tree::{FocusFigure, InputPolicy, Interaction, NodeKind};

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

    /// The next (`dir = 1`) or previous (`dir = -1`) focusable id after
    /// `current`, from `placements`' pre-order — restricted to the active
    /// blocking scope exactly as [`FocusTree::next`]/[`FocusTree::previous`]
    /// are, but **not** filtered by [`Placement::is_visible`].
    ///
    /// [`FocusTree::order`] only ever names what a frame actually placed on
    /// screen (the module doc's "Traversal order is visual order"), because
    /// [`crate::input::hit_test`] and Tab reachability have to agree. A host
    /// that wants Tab to be able to *scroll* something into view needs the
    /// wider question answered first: what is the very next focusable node
    /// in document order, on screen or not. This is that answer; a host
    /// still decides what to do with a target that is not in `order` yet —
    /// see `gorgon-petra-egui`'s `Host::step_focus`.
    ///
    /// Wraps exactly as `next`/`previous` do: the last candidate's successor
    /// is the first. `None` when nothing in scope is focusable at all.
    #[must_use]
    pub fn reachable(
        &self,
        placements: &[Placement],
        surface_scopes: &BTreeMap<String, InputPolicy>,
        dir: isize,
    ) -> Option<String> {
        let scope = self.active_scope();
        let candidates: Vec<&str> = placements
            .iter()
            .enumerate()
            .filter(|(_, p)| is_focusable(p))
            .filter(|(idx, _)| {
                blocking_scopes(*idx, placements, surface_scopes)
                    .first()
                    .map(String::as_str)
                    == scope
            })
            .map(|(_, p)| p.id.as_str())
            .collect();
        step_over(&candidates, self.current.as_deref(), dir)
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
        self.current = step_over(&candidates, self.current.as_deref(), dir);
    }
}

/// Whether `placement` is a legal focus target on the terms
/// [`Interaction::Focus`] and [`PlacementSemantics::disabled`] alone —
/// [`Placement::is_visible`] is a separate question, deliberately not asked
/// here: [`index`] asks both to build the on-screen order,
/// [`FocusTree::reachable`] asks only this one to answer "is there a
/// focusable node further out, on screen or not".
fn is_focusable(placement: &Placement) -> bool {
    placement.semantics.actions.contains(&Interaction::Focus) && !placement.semantics.disabled
}

/// The id at `current`'s position in `candidates`, stepped by `dir` and
/// wrapped — [`FocusTree::next`]/[`FocusTree::previous`]'s arithmetic,
/// shared with [`FocusTree::reachable`] so the two can never disagree about
/// what "the next one, wrapping" means. `current` absent from `candidates`
/// (including the none-state) starts at the first. `None` only when
/// `candidates` itself is empty.
fn step_over(candidates: &[&str], current: Option<&str>, dir: isize) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let from = current.and_then(|c| candidates.iter().position(|x| *x == c));
    let len = candidates.len() as isize;
    let next = match from {
        Some(p) => (p as isize + dir).rem_euclid(len),
        None => 0,
    };
    Some(candidates[next as usize].to_owned())
}

/// Build `(order, scope_chain)` from one frame's placements.
fn index(
    placements: &[Placement],
    surface_scopes: &BTreeMap<String, InputPolicy>,
) -> (Vec<String>, BTreeMap<String, Vec<String>>) {
    let mut order = Vec::new();
    let mut scope_chain = BTreeMap::new();
    for (idx, placement) in placements.iter().enumerate() {
        if !is_focusable(placement) || !placement.is_visible() {
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
///
/// `Placement` is a public struct with public fields and no sealed
/// constructor, so `parent` cannot be trusted to be well-formed: this walks
/// defensively rather than indexing the slice directly by an unchecked
/// `parent` value, and stops with whatever scope chain it has found so far
/// on either kind of malformed input, the same "refuse gracefully, do not
/// guess" choice
/// `crate::semantic::project::project` makes for the identical class of
/// problem (an index that cannot be hung anywhere sound is left out rather
/// than trusted). A `parent` pointing past the end of `placements` stops the
/// walk immediately; a `parent` chain that cycles back on itself — which
/// would otherwise spin this loop forever — is bounded by the list's own
/// length, since no well-formed chain can be longer than that.
fn blocking_scopes(
    idx: usize,
    placements: &[Placement],
    surface_scopes: &BTreeMap<String, InputPolicy>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = Some(idx);
    let mut steps = 0usize;
    while let Some(p) = cursor {
        let Some(step) = placements.get(p) else {
            break;
        };
        if surface_scopes.get(&step.id) == Some(&InputPolicy::Block) {
            out.push(step.id.clone());
        }
        cursor = step.parent;
        steps += 1;
        if steps > placements.len() {
            break;
        }
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

/// The rect a focus figure is measured against for the placement at `shown`.
///
/// For [`FocusFigure::Border`] and [`FocusFigure::Sides`] this is the
/// placement's own rect: a ring and a pair of brackets mark the whole
/// control. For the two bar figures it is the control's **content run** —
/// horizontally the extent of what the control actually shows, vertically
/// the control itself — so the stripe underlines the content and the bar
/// still seats on the control's own bottom edge.
///
/// # Why the content and not the control's width
///
/// The bars took `FocusRing::WIDTH_FRACTION` — two thirds of the control,
/// centred — until 2026-09-06. That fraction is a *proxy* for "about as wide
/// as what the control shows", and it holds only while a control hugs its
/// own content. A menu trigger is 80 wide around a 48-wide label, and two
/// thirds of it is 53: near enough that nobody looked twice for a year.
///
/// A row that spans a panel breaks the proxy. A tree row is 868 wide around
/// a 44-wide word at x 332, so two thirds centred is a 579-unit stripe
/// starting at x 434 — a hundred units past the end of the word, floating in
/// the row's empty half and touching nothing. Widening it to the full 868
/// does not fix that; it runs the stripe *further* past the word and reads
/// as the container's own rule. Both are the same missing idea, which is
/// that the bar was never about the control's width.
///
/// # What counts as content
///
/// Every visible descendant that lays out no children of its own
/// ([`NodeKind::is_container`]), except an [`NodeKind::Input`]. So: text,
/// canvases, images, and the spacers a control reserves for its own parts.
///
/// **Containers are excluded because a childless one is decoration.** A tab's
/// selection indicator, a side-nav row's accent bar and an icon's trailing
/// gap are all childless `stack`s that paint a fill and hold nothing, and a
/// contained tab's indicator spans the tab's full width — counting it would
/// put the bar back at the control's width, which is the thing this rule
/// exists to stop.
///
/// **An `input` is excluded because it is its own focus target**, with its
/// own figure. A data table row holding an editable cell is the case: the
/// cell is a control inside a control, not part of the row's mark.
///
/// **Spacers are included, and that is the load-bearing half.** A checkbox
/// draws its box as a bare `spacer` when it is empty and as an inset canvas
/// tick when it is not, so a rule that counted only the things a person can
/// see would move the bar 24 units the moment the box was ticked. The run
/// has to be stable under state, and a control's own reserved space is part
/// of the control.
///
/// The consequence is that **a row with a trailing affordance must narrow
/// itself explicitly**, because its trailing spacer and chevron are content
/// by this rule and reach the far edge. An accordion header is 868 wide with
/// a 79-wide title at x 308, a 725-wide spacer, and a chevron at x 1128;
/// nothing geometric separates that chevron from a tag's dismiss cross,
/// which *is* part of its control. The component knows and the engine cannot,
/// so the component says so with [`crate::tree::FocusShownOn`] — the same
/// mechanism that already answers *which placement*, pointed one level
/// further in.
///
/// A control with no content at all falls back to its own rect.
/// `every_bar_figure_finds_a_label` is the gate that names any control where
/// that fallback lands on something too wide to read as an underline.
#[must_use]
pub fn marked_rect(placements: &[Placement], shown: usize, figure: FocusFigure) -> Rect {
    let Some(node) = placements.get(shown) else {
        return Rect::ZERO;
    };
    if !figure.marks_the_label() {
        return node.rect;
    }
    let run = declared_run(placements, shown).or_else(|| content_run(placements, shown));
    run.map_or(node.rect, |(l, r)| {
        Rect::new(l, node.rect.y, r - l, node.rect.h)
    })
}

/// The run a descendant declared with [`crate::tree::Semantics::focus_run`],
/// nearest first.
///
/// Nearest by depth in steps up the parent chain, the same way
/// `gorgon-petra-egui`'s `head_of` picks a head row: a row nested inside a
/// row must not answer for the row above it.
fn declared_run(placements: &[Placement], shown: usize) -> Option<(f32, f32)> {
    placements
        .iter()
        .enumerate()
        .filter(|&(i, p)| {
            p.semantics.focus_run
                && i != shown
                && p.rect.w > 0.0
                && p.is_visible()
                && descends_from(placements, i, shown)
        })
        .min_by_key(|&(i, _)| depth_below(placements, i, shown))
        .map(|(_, p)| (p.rect.x, p.rect.x + p.rect.w))
}

/// How many parent steps `index` is below `ancestor`, saturating at the
/// slice length when the chain does not reach it.
fn depth_below(placements: &[Placement], index: usize, ancestor: usize) -> usize {
    let mut cursor = placements.get(index).and_then(|p| p.parent);
    for step in 0..placements.len() {
        match cursor {
            Some(i) if i == ancestor => return step,
            Some(i) => cursor = placements.get(i).and_then(|p| p.parent),
            None => break,
        }
    }
    placements.len()
}

/// The horizontal extent of `shown`'s content, by the rule [`marked_rect`]
/// documents, as `(left, right)`.
fn content_run(placements: &[Placement], shown: usize) -> Option<(f32, f32)> {
    let mut span: Option<(f32, f32)> = None;
    for (i, p) in placements.iter().enumerate() {
        if i == shown
            || p.kind.is_container()
            || p.kind == NodeKind::Input
            || p.rect.w <= 0.0
            || !p.is_visible()
            || !descends_from(placements, i, shown)
        {
            continue;
        }
        let (l, r) = (p.rect.x, p.rect.x + p.rect.w);
        span = Some(match span {
            Some((sl, sr)) => (sl.min(l), sr.max(r)),
            None => (l, r),
        });
    }
    span
}

/// Whether `index` is inside `ancestor`'s subtree.
///
/// Walks `Placement::parent`, bounded by the slice length exactly as
/// [`FocusTree`]'s own scope walk is: a cyclic or out-of-range chain ends
/// rather than spins. Deliberately not "the contiguous run after `ancestor`",
/// which pre-order would allow — `gorgon-petra-egui`'s caret already declined
/// to lean on placement order for the same question, and one rule that holds
/// whatever the order is beats two that agree only while it does.
fn descends_from(placements: &[Placement], index: usize, ancestor: usize) -> bool {
    let mut cursor = placements.get(index).and_then(|p| p.parent);
    for _ in 0..placements.len() {
        match cursor {
            Some(i) if i == ancestor => return true,
            Some(i) => cursor = placements.get(i).and_then(|p| p.parent),
            None => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{FocusError, FocusTree, marked_rect};
    use crate::frame::placement::{PaintState, Placement, PlacementSemantics};
    use crate::geom::Rect;
    use crate::tree::{FocusFigure, InputPolicy, Interaction, NodeKind};
    use std::collections::BTreeMap;

    /// A placement carrying enough to drive focus: id, parent, and whether
    /// it is focusable. This is the pattern `frame/placement.rs`'s own tests
    /// use — build `Placement` values directly rather than running layout.
    fn placement(id: &str, parent: Option<usize>, focusable: bool, disabled: bool) -> Placement {
        Placement {
            id: id.into(),
            kind: NodeKind::Text,
            // A real compositor clip: the node is on screen. `Rect::ZERO`
            // against `Rect::ZERO` does not overlap, and would drop every
            // fixture from the order.
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            z: 0,
            clip: Rect::new(0.0, 0.0, 100.0, 100.0),
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

    /// A row 868 wide holding a caret at x 308 and a 44-wide word at 332:
    /// `component::tree_view`'s own numbers, measured on 2026-09-06.
    fn wide_row() -> Vec<Placement> {
        let mut row = placement("/row", None, true, false);
        row.kind = NodeKind::Stack;
        row.rect = Rect::new(292.0, 288.0, 868.0, 32.0);
        row.clip = Rect::new(0.0, 0.0, 2000.0, 2000.0);

        let mut caret = placement("/row/caret", Some(0), false, false);
        caret.kind = NodeKind::Canvas;
        caret.rect = Rect::new(308.0, 296.0, 16.0, 16.0);
        caret.clip = row.clip;

        let mut label = placement("/row/label", Some(0), false, false);
        label.rect = Rect::new(332.0, 294.0, 43.9, 20.0);
        label.clip = row.clip;

        vec![row, caret, label]
    }

    /// The bar spans the row's content, not the row. This is the whole rule.
    #[test]
    fn a_bar_is_measured_against_the_controls_content() {
        let places = wide_row();
        let run = marked_rect(&places, 0, FocusFigure::BarInside);

        assert_eq!(run.x, 308.0, "the run starts at the caret: {run:?}");
        assert!(
            (run.right() - 375.9).abs() < 1e-3,
            "and ends at the word: {run:?}"
        );
        assert_eq!(
            (run.y, run.h),
            (288.0, 32.0),
            "vertically the row, so the bar still seats on the row's own edge"
        );
        assert_eq!(
            run,
            marked_rect(&places, 0, FocusFigure::BarUnder),
            "both bars ask the same question"
        );
    }

    /// A ring and a pair of brackets mark the control, so they are handed
    /// the control. Falsify by dropping the `marks_the_label` guard: the
    /// brackets would then hug the content and leave the row unmarked.
    #[test]
    fn a_ring_and_a_bracket_are_measured_against_the_whole_control() {
        let places = wide_row();
        for figure in [FocusFigure::Border, FocusFigure::Sides] {
            assert_eq!(
                marked_rect(&places, 0, figure),
                places[0].rect,
                "{figure:?} marks the control"
            );
        }
    }

    /// A childless container is decoration and never widens the run.
    ///
    /// A contained tab's selection indicator spans the tab's whole width, so
    /// counting it would put the bar back at the control's width — the exact
    /// picture the content rule replaced. Falsify by dropping the
    /// `is_container` test in `content_run`.
    #[test]
    fn an_indicator_is_not_content() {
        let mut places = wide_row();
        let mut indicator = placement("/row/indicator", Some(0), false, false);
        indicator.kind = NodeKind::Stack;
        indicator.rect = Rect::new(292.0, 317.0, 868.0, 3.0);
        indicator.clip = places[0].clip;
        places.push(indicator);

        let run = marked_rect(&places, 0, FocusFigure::BarInside);
        assert_eq!(run.x, 308.0, "{run:?}");
        assert!((run.right() - 375.9).abs() < 1e-3, "{run:?}");
    }

    /// A nested `input` is its own focus target, not its row's mark. A data
    /// table row holding an editable cell is the case.
    #[test]
    fn a_nested_input_is_not_content() {
        let mut places = wide_row();
        let mut cell = placement("/row/cell", Some(0), true, false);
        cell.kind = NodeKind::Input;
        cell.rect = Rect::new(762.0, 296.0, 382.0, 16.0);
        cell.clip = places[0].clip;
        places.push(cell);

        let run = marked_rect(&places, 0, FocusFigure::BarInside);
        assert_eq!(run.x, 308.0, "{run:?}");
        assert!((run.right() - 375.9).abs() < 1e-3, "{run:?}");
    }

    /// The run does not move when the control changes state.
    ///
    /// A checkbox draws its box as a bare `spacer` when empty and as an inset
    /// canvas tick when ticked. Measured on the catalog: `check-b` is a
    /// 16-wide spacer at x 296, `check-a` is a 10-wide tick at x 299 between
    /// two 3-wide insets. A rule that counted only what a person can see
    /// would start the bar at 320 in one state and 299 in the other, so the
    /// stripe would jump 24 units on a tick. Falsify by excluding spacers.
    #[test]
    fn the_run_does_not_move_when_a_box_is_ticked() {
        let row = |ticked: bool| {
            let mut node = placement("/row", None, true, false);
            node.kind = NodeKind::Stack;
            node.rect = Rect::new(292.0, 284.0, 68.0, 24.0);
            node.clip = Rect::new(0.0, 0.0, 2000.0, 2000.0);

            let mut label = placement("/row/label", Some(0), false, false);
            label.rect = Rect::new(320.0, 286.0, 36.0, 20.0);
            label.clip = node.clip;

            let mut places = vec![node, label];
            if ticked {
                for (name, kind, x, w) in [
                    ("/row/inset-start", NodeKind::Spacer, 296.0, 3.0),
                    ("/row/tick", NodeKind::Canvas, 299.0, 10.0),
                    ("/row/inset-end", NodeKind::Spacer, 309.0, 3.0),
                ] {
                    let mut part = placement(name, Some(0), false, false);
                    part.kind = kind;
                    part.rect = Rect::new(x, 288.0, w, 16.0);
                    part.clip = places[0].clip;
                    places.push(part);
                }
            } else {
                let mut boxed = placement("/row/box", Some(0), false, false);
                boxed.kind = NodeKind::Spacer;
                boxed.rect = Rect::new(296.0, 288.0, 16.0, 16.0);
                boxed.clip = places[0].clip;
                places.push(boxed);
            }
            marked_rect(&places, 0, FocusFigure::BarInside)
        };

        assert_eq!(row(true), row(false), "the stripe moved on a tick");
        assert_eq!(
            row(false).x,
            296.0,
            "and it starts at the box, not at the word"
        );
    }

    /// A declared run beats the content rule, and narrows only the width.
    ///
    /// The accordion header is the case: a title, a wide spacer and a
    /// chevron pinned at the trailing edge, whose content run is the whole
    /// row. Falsify by dropping the `declared_run` call in `marked_rect`.
    #[test]
    fn a_declared_run_narrows_the_bar_without_moving_it() {
        let mut places = wide_row();
        let mut chevron = placement("/row/chevron", Some(0), false, false);
        chevron.kind = NodeKind::Canvas;
        chevron.rect = Rect::new(1128.0, 296.0, 16.0, 16.0);
        chevron.clip = places[0].clip;
        places.push(chevron);

        let wide = marked_rect(&places, 0, FocusFigure::BarInside);
        assert!(
            (wide.right() - 1144.0).abs() < 1e-3,
            "the fixture must reach the chevron first: {wide:?}"
        );

        places[2].semantics.focus_run = true;
        let run = marked_rect(&places, 0, FocusFigure::BarInside);
        assert_eq!(run.x, places[2].rect.x, "{run:?}");
        assert!(
            (run.w - places[2].rect.w).abs() < 1e-3,
            "the declared node decides the width: {run:?}"
        );
        assert_eq!(
            (run.y, run.h),
            (places[0].rect.y, places[0].rect.h),
            "and the row still decides where the bar seats"
        );
    }

    /// The nearest declaration wins, so a row nested inside a row does not
    /// answer for the row above it.
    #[test]
    fn the_nearest_declared_run_wins() {
        let mut places = wide_row();
        places[2].semantics.focus_run = true;

        let mut nested = placement("/row/sub", Some(0), false, false);
        nested.kind = NodeKind::Stack;
        nested.rect = Rect::new(292.0, 288.0, 868.0, 32.0);
        nested.clip = places[0].clip;
        places.push(nested);
        let sub = places.len() - 1;

        let mut deep = placement("/row/sub/label", Some(sub), false, false);
        deep.rect = Rect::new(700.0, 294.0, 50.0, 20.0);
        deep.clip = places[0].clip;
        deep.semantics.focus_run = true;
        places.push(deep);

        let run = marked_rect(&places, 0, FocusFigure::BarInside);
        assert_eq!(run.x, 332.0, "the deeper declaration won: {run:?}");
    }

    /// Content is a union, so placement order cannot change the answer.
    #[test]
    fn the_run_is_the_same_whatever_order_the_parts_arrive_in() {
        let mut places = wide_row();
        let forward = marked_rect(&places, 0, FocusFigure::BarInside);
        places.swap(1, 2);
        for (i, p) in places.iter_mut().enumerate() {
            if i != 0 {
                p.parent = Some(0);
            }
        }
        assert_eq!(marked_rect(&places, 0, FocusFigure::BarInside), forward);
    }

    /// No content at all: the control's own rect, which is the documented
    /// fallback for an icon-only button.
    #[test]
    fn a_control_with_no_content_falls_back_to_its_own_rect() {
        let mut places = wide_row();
        places.truncate(1);
        assert_eq!(
            marked_rect(&places, 0, FocusFigure::BarInside),
            places[0].rect
        );
    }

    /// Content scrolled out of the clip is not content. It is in the frame so
    /// scrolling stays smooth, exactly as
    /// `a_clipped_away_node_is_not_in_the_tab_order` says, and a bar under a
    /// word nobody can see is not an underline.
    #[test]
    fn offscreen_content_does_not_widen_the_run() {
        let mut places = wide_row();
        for p in places.iter_mut().skip(1) {
            p.clip = Rect::new(0.0, 0.0, 100.0, 100.0);
        }
        let run = marked_rect(&places, 0, FocusFigure::BarInside);
        assert_eq!(run, places[0].rect, "{run:?}");
    }

    /// A grandchild counts. `component::ui_shell`'s nav label sits three
    /// levels under the row it marks (`row/body/lead/label`), so a rule that
    /// only looked at direct children would find nothing there.
    #[test]
    fn content_nested_under_a_wrapper_is_still_found() {
        let mut places = wide_row();
        let mut wrapper = placement("/row/body", Some(0), false, false);
        wrapper.kind = NodeKind::Stack;
        wrapper.rect = places[0].rect;
        wrapper.clip = places[0].clip;
        places.push(wrapper);
        let wrapper_at = places.len() - 1;
        places[2].parent = Some(wrapper_at);

        let run = marked_rect(&places, 0, FocusFigure::BarInside);
        assert_eq!(run.x, 308.0, "{run:?}");
    }

    fn no_scopes() -> BTreeMap<String, InputPolicy> {
        BTreeMap::new()
    }

    /// A node the composer placed in overscan (rect outside the clip) is
    /// in the frame so scrolling stays smooth. It is not in the Tab order:
    /// the pointer already refuses it (`hit_test`), and keyboard reach
    /// must match.
    #[test]
    fn a_clipped_away_node_is_not_in_the_tab_order() {
        let mut hidden = placement("/hidden", None, true, false);
        hidden.rect = Rect::new(0.0, 300.0, 10.0, 10.0);
        hidden.clip = Rect::new(0.0, 0.0, 200.0, 200.0);
        let placements = vec![
            placement("/a", None, true, false),
            hidden,
            placement("/b", None, true, false),
        ];
        let tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(tree.order(), ["/a", "/b"]);
        assert_eq!(tree.current(), Some("/a"));
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

    /// A malformed `parent` pointing past the end of the placement list (the
    /// case a caller who hand-builds `Placement` values can trigger) must not
    /// panic: `blocking_scopes` stops the walk rather than indexing out of
    /// bounds. Deleting the bounds check turns this into a panic.
    #[test]
    fn a_parent_index_past_the_end_of_the_list_does_not_panic() {
        let placements = vec![placement("/a", Some(99), true, false)];
        let tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(tree.order(), ["/a"]);
    }

    /// A `parent` chain that cycles back on itself must not hang: without
    /// the step bound, `blocking_scopes`'s `while let` loop spins forever on
    /// this input. This test relies on the surrounding test binary's own
    /// timeout to turn a reintroduced hang into a visible failure.
    #[test]
    fn a_cyclic_parent_chain_does_not_hang() {
        let placements = vec![placement("/a", Some(0), true, false)];
        let tree = FocusTree::from_placements(&placements, &no_scopes());
        assert_eq!(tree.order(), ["/a"]);
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
