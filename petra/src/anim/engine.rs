//! The transition engine: key-diff, retargeting, exits, and the exact snap.
//!
//! # Where animation sits
//!
//! Layout negotiates *targets*; this engine interpolates the *result*. It runs
//! after [`crate::frame::petrify`] and rewrites placements in the finished
//! frame, then recomputes the frame's Merkle hashes and digest from what it
//! wrote. Two consequences, both deliberate and both worth stating:
//!
//! * A node animating its rect does **not** re-negotiate its siblings. A
//!   sliding panel slides over the layout, it does not push its neighbours
//!   along. That is what keeps FR-006's determinism — layout is a pure
//!   function of the tree and the viewport, and motion cannot feed back into
//!   it.
//! * The digest of an animating frame is a real digest of what is on screen,
//!   not of the target. A driver comparing digests mid-transition sees the
//!   mid-transition picture, which is what `contracts/driver-protocol.md`'s
//!   screenshot verification needs.
//!
//! # The settle rule, and why the settled digest has no residue
//!
//! `contracts/animation.md`: *"A transition settles when |x − target| and |v|
//! are both under declared thresholds ... it then snaps exactly to target (the
//! digest must not carry near-target residue)."*
//!
//! The implementation is stronger than a snap: when nothing is running and no
//! exit is in flight, this engine does not touch the frame **at all** — not
//! the placements, not the hashes, not the digest. A settled frame is
//! byte-identical to the frame `petrify` produced, so its digest is the digest
//! of a build straight to the target, with no engine in the picture.
//! `gorgon/petra/tests/animation_frames.rs` asserts exactly that equality.
//!
//! # What this engine cannot see
//!
//! Declarations are read off the [`ViewNode`] tree by
//! [`Declarations::collect`], so a node the tree does not contain has no
//! declaration. Rows materialized by a `collection` through
//! [`crate::layout::RowSource`] are exactly that case: they are placed, they
//! have ids, and they do not appear in the tree walk. They therefore do not
//! animate. This is a limitation of reading declarations from the tree, it is
//! not a bug to be found later, and closing it means carrying the declaration
//! on the placement — which is a digest change (`Placement` is destructured
//! with no rest pattern in `crate::frame::digest::leaf_hash`, deliberately, so
//! that adding a field asks).

use std::collections::{BTreeMap, BTreeSet};

use super::policy::MotionPolicy;
use super::registry::{ExitRule, Timing, TransitionDef, TransitionRegistry};
use super::value::{AnimVector, PropertyKind};

/// Stuck-clock and idle-gap stand-in. Same numbers as the flying caret
/// (`gorgon-petra-egui` `focus_caret.rs`): bigger than a 30 Hz vsync,
/// smaller than "we were asleep". SC-002 paints nothing at idle, so the
/// next click can carry seconds of wall time; counting that gap as curve
/// time finishes a 140 ms slide in one frame.
const FRAME_DT: f64 = 1.0 / 60.0;
const IDLE_GAP: f64 = 0.04;

/// The clock a new or retargeted trajectory starts from.
///
/// Vsync-spaced frames keep `last` so local time matches wall time.
/// An idle gap (or a stuck clock) is replaced with one frame of
/// progress, not the whole nap.
fn motion_clock(last: f64, now: f64) -> f64 {
    let gap = now - last;
    if gap <= 0.0 || gap > IDLE_GAP {
        now - FRAME_DT
    } else {
        last
    }
}
use crate::frame::{PaintContent, PetrifiedFrame, Placement, TransitionActivity, digest};
use crate::geom::Rect;
use crate::layout::Slot;
use crate::tree::{KeyPath, ViewNode};

/// The properties this engine drives on a [`Placement`], in evaluation order.
///
/// Exactly the animatable fields a placement carries.
/// [`PropertyKind::Color`] is absent because no placement field holds a
/// resolved colour — token bindings reach the painter through
/// [`PaintContent`] and resolve at paint time. `crate::anim::value`'s
/// `color_is_interpolable_but_not_yet_driven_by_the_engine` test pins that
/// gap so it cannot close or widen silently.
pub const DRIVEN_PROPERTIES: [PropertyKind; 3] = [
    PropertyKind::Position,
    PropertyKind::Size,
    PropertyKind::Opacity,
];

/// Which transition definition each node in a tree named.
///
/// Built by walking the tree along the same [`KeyPath`] the layout walk uses,
/// so the keys here are exactly the `Placement::id` values the frame will
/// carry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Declarations {
    by_id: BTreeMap<String, String>,
    ambient_ids: BTreeSet<String>,
}

impl Declarations {
    /// Walk `tree` and record every node that names a transition, and every
    /// node that declares itself ambient.
    #[must_use]
    pub fn collect(tree: &ViewNode) -> Self {
        let mut out = Self::default();
        let mut path = KeyPath::root();
        out.walk(tree, &mut path);
        out
    }

    fn walk(&mut self, node: &ViewNode, path: &mut KeyPath) {
        path.push(node.key.clone());
        if node.transition.is_some() || node.ambient {
            let id = path.id();
            if let Some(reference) = node.transition.as_ref() {
                self.by_id.insert(id.clone(), reference.name().to_owned());
            }
            if node.ambient {
                self.ambient_ids.insert(id);
            }
        }
        for child in &node.children {
            self.walk(child, path);
        }
        path.pop();
    }

    /// The transition name node `id` declared.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&str> {
        self.by_id.get(id).map(String::as_str)
    }

    /// Whether node `id` declared itself ambient.
    #[must_use]
    pub fn is_ambient(&self, id: &str) -> bool {
        self.ambient_ids.contains(id)
    }

    /// How many nodes name a transition.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether no node names a transition.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// One property's live trajectory.
#[derive(Clone, Debug)]
struct TrackState {
    timing: Timing,
    property: PropertyKind,
    /// Where this trajectory started, and how fast. On a retarget these
    /// become the *current* value and velocity — that is the whole of the
    /// interruption rule, and `SC-007` is the test of it.
    origin: AnimVector,
    velocity0: AnimVector,
    target: AnimVector,
    /// Host clock at trajectory start. The trajectory's local `t` is
    /// `now - start`.
    start: f64,
    /// The clock this track was last advanced to.
    ///
    /// A trajectory that is retargeted rebases onto *this*, not onto the
    /// current frame's clock: the state it carries (`value`, `velocity`) is
    /// the state at the last frame, and starting the new trajectory's clock
    /// at the current frame would replay that state for one frame and stall
    /// the motion visibly at every interruption.
    last_now: f64,
    /// State at `last_now`: where it was and how fast.
    value: AnimVector,
    velocity: AnimVector,
    settled: bool,
    /// Whether this trajectory's local clock wraps on the track's own
    /// duration.
    ///
    /// Copied from [`TransitionDef::is_ambient`] when the trajectory starts,
    /// so an ambient keyframe track runs forever without a host re-triggering
    /// it (`contracts/draw-list.md` §8). The rule itself is one expression in
    /// [`Timing::looped_time`]; this flag only says which trajectories it
    /// applies to.
    looping: bool,
}

impl TrackState {
    /// Advance to `now` and answer where the property is.
    fn advance(&mut self, now: f64) -> AnimVector {
        if self.settled {
            self.last_now = now;
            return self.target;
        }
        let elapsed = (now - self.start).max(0.0);
        // An ambient track wraps: `t mod duration` never reaches the `t >=
        // duration` branch in `Timing::sample`, so it never settles and never
        // snaps to the layout target. Everything else passes through.
        let elapsed = if self.looping {
            self.timing.looped_time(elapsed)
        } else {
            elapsed
        };
        let sample = self.timing.sample(
            self.origin,
            self.velocity0,
            self.target,
            elapsed,
            self.property.thresholds(),
        );
        if sample.settled || !sample.value.is_finite() {
            // The exact target, not the sample: no near-target residue.
            self.settled = true;
            self.value = self.target;
            self.velocity = AnimVector::zeros(self.target.len());
        } else {
            self.value = sample.value;
            self.velocity = sample.velocity;
        }
        self.last_now = now;
        self.value
    }

    /// Aim at a new target from wherever this trajectory is right now.
    ///
    /// `contracts/animation.md`, the interruption rule: *"evaluate the CURRENT
    /// trajectory's x(t) and v(t) in closed form, then start the new
    /// trajectory with x₀ = x(t), v₀ = v(t) toward the new target. Position
    /// and velocity are both continuous."* The two lines below are that
    /// sentence; the `velocity0` line is what SC-007 measures and what T055
    /// sabotages. `self.velocity` is the closed form's own answer at
    /// `last_now`, never a difference across two frames.
    fn retarget(&mut self, target: AnimVector, now: f64) {
        self.origin = self.value;
        self.velocity0 = self.velocity;
        self.target = target;
        self.start = motion_clock(self.last_now, now);
        self.last_now = self.start;
        self.settled = false;
    }
}

/// A subtree that left the tree and is running its exit.
#[derive(Clone, Debug)]
struct ExitGroup {
    /// The subtree as last rendered, in pre-order. `parents` are indices
    /// within this vector, `None` for the group root.
    items: Vec<ExitItem>,
    parents: Vec<Option<usize>>,
    /// Id of the parent to re-insert under. `None` means the group root was
    /// the frame root, which cannot be re-inserted anywhere.
    parent_id: Option<String>,
    /// The one track driving the exit.
    track: TrackState,
    /// The root's value for the driven property as recorded, which the
    /// descendants' offsets are relative to.
    recorded: AnimVector,
}

#[derive(Clone, Debug)]
struct ExitItem {
    placement: Placement,
    content: PaintContent,
    slot: Slot,
}

/// The last frame's raw, pre-animation values, keyed by placement id.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RawTarget {
    rect: Rect,
    opacity: f32,
}

/// Drives every declared transition in an application.
///
/// One engine per host. It holds the trajectories, the exits in flight, and
/// the policy; it does not hold a clock — the host passes its own timestamp
/// in, so a test can drive sixty simulated seconds without sleeping and a
/// window can pass `egui`'s frame time.
#[derive(Debug)]
pub struct TransitionEngine {
    registry: TransitionRegistry,
    policy: MotionPolicy,
    nodes: BTreeMap<String, BTreeMap<PropertyKind, TrackState>>,
    exiting: Vec<ExitGroup>,
    /// Raw layout values of the previous frame, per placement id. A change
    /// here is what starts or retargets a trajectory.
    previous: Option<BTreeMap<String, RawTarget>>,
    /// The last frame as rendered, for an exiting subtree to be built from.
    rendered: BTreeMap<String, ExitItem>,
    /// The transition each rendered node named, kept beside the render.
    ///
    /// An exiting node is by definition absent from the *current* tree, so
    /// the current `Declarations` cannot say what its exit rule is. Reading
    /// the name off the frame it was last on is the only place the answer
    /// still exists.
    declared: BTreeMap<String, String>,
    /// The host clock of the previous frame. A trajectory starting this frame
    /// began at the previous one — that is when the node was still at its old
    /// value — so this is the clock its local time is measured from.
    previous_now: Option<f64>,
    /// Names a node declared that the registry could not resolve.
    ///
    /// Unreachable through `crate::tree::validate`, which refuses such a tree
    /// — but only when the host declared the registry's names into the tree
    /// registry ([`TransitionRegistry::declare_into`]). A host that did not is
    /// a wiring bug, and it is recorded and answerable
    /// ([`TransitionEngine::unresolved`]) rather than showing up as a node
    /// that mysteriously never moves.
    unresolved: BTreeMap<String, String>,
}

impl TransitionEngine {
    /// An engine over `registry`, at full motion.
    #[must_use]
    pub fn new(registry: TransitionRegistry) -> Self {
        Self {
            registry,
            policy: MotionPolicy::new(),
            nodes: BTreeMap::new(),
            exiting: Vec::new(),
            previous: None,
            rendered: BTreeMap::new(),
            declared: BTreeMap::new(),
            previous_now: None,
            unresolved: BTreeMap::new(),
        }
    }

    /// The definitions this engine resolves names against.
    #[must_use]
    pub fn registry(&self) -> &TransitionRegistry {
        &self.registry
    }

    /// The motion policy in force.
    #[must_use]
    pub fn policy(&self) -> MotionPolicy {
        self.policy
    }

    /// Turn reduced motion on or off.
    ///
    /// A real change completes every running movement transition instantly at
    /// its target, per `contracts/animation.md`: *"Toggling reduced motion
    /// mid-flight completes running transitions instantly to their targets."*
    /// Turning it *off* mid-flight does not restart anything — there is
    /// nothing to restart, since the transitions it would have restarted are
    /// already at their targets.
    pub fn set_reduced_motion(&mut self, on: bool) {
        if !self.policy.set_reduced_motion(on) {
            return;
        }
        if !on {
            return;
        }
        for tracks in self.nodes.values_mut() {
            tracks.retain(|property, track| {
                if self.policy.animates(*property) {
                    return true;
                }
                track.settled = true;
                track.value = track.target;
                track.velocity = AnimVector::zeros(track.target.len());
                true
            });
        }
        self.exiting.retain_mut(|group| {
            if self.policy.animates(group.track.property) {
                return true;
            }
            group.track.settled = true;
            group.track.value = group.track.target;
            group.track.velocity = AnimVector::zeros(group.track.target.len());
            // A movement-driven exit under reduced motion ends now: the node
            // is gone, and its end state is "not on screen", which is where
            // completing instantly lands.
            false
        });
    }

    /// Node ids whose declared transition name did not resolve, and the name.
    #[must_use]
    pub fn unresolved(&self) -> &BTreeMap<String, String> {
        &self.unresolved
    }

    /// Forget every trajectory and every exit.
    ///
    /// For a host that has swapped the whole application out from under the
    /// engine: continuing to retarget from a trajectory belonging to a tree
    /// that no longer exists would animate from a stale position.
    pub fn reset(&mut self) {
        self.nodes.clear();
        self.exiting.clear();
        self.previous = None;
        self.rendered.clear();
        self.declared.clear();
        self.previous_now = None;
        self.unresolved.clear();
    }

    /// Advance every transition to `now` and rewrite `frame` to what is
    /// actually on screen, answering the motion the frame carries.
    ///
    /// `now` is the host's clock in seconds, monotone within a run. Time going
    /// backwards is clamped to zero elapsed rather than evaluated backwards,
    /// which would diverge.
    ///
    /// When nothing is moving the frame is left untouched — see the settle
    /// section of this module's doc.
    pub fn animate(
        &mut self,
        frame: &mut PetrifiedFrame,
        declarations: &Declarations,
        now: f64,
    ) -> TransitionActivity {
        let raw: BTreeMap<String, RawTarget> = frame
            .placements
            .iter()
            .map(|p| {
                (
                    p.id.clone(),
                    RawTarget {
                        rect: p.rect,
                        opacity: p.opacity,
                    },
                )
            })
            .collect();
        let previous = self.previous.take();
        let first_frame = previous.is_none();
        let previous = previous.unwrap_or_default();
        let previous_now = self.previous_now.unwrap_or(now);

        let mut changed = false;
        if !first_frame {
            changed |=
                self.start_and_advance(frame, declarations, &previous, &raw, now, previous_now);
            changed |= self.collect_exits(&previous, &raw, now, previous_now);
        }
        changed |= self.apply_exits(frame, now);
        if changed {
            rebuild_identity(frame);
        }

        // The rendered snapshot is what an exiting subtree is rebuilt from, so
        // it is taken after the rewrite: an exit continues from where the node
        // actually was, not from where layout wanted it.
        self.snapshot_rendered(frame, declarations);
        self.previous = Some(raw);
        self.previous_now = Some(now);

        let activity = self.activity(frame, declarations);
        frame.transitions = activity;
        activity
    }

    /// Start, retarget and advance every declared node's tracks. Answers
    /// whether any placement was rewritten.
    fn start_and_advance(
        &mut self,
        frame: &mut PetrifiedFrame,
        declarations: &Declarations,
        previous: &BTreeMap<String, RawTarget>,
        raw: &BTreeMap<String, RawTarget>,
        now: f64,
        previous_now: f64,
    ) -> bool {
        let mut changed = false;
        let live: BTreeSet<&str> = raw.keys().map(String::as_str).collect();
        self.nodes.retain(|id, _| live.contains(id.as_str()));

        for index in 0..frame.placements.len() {
            let id = frame.placements[index].id.clone();
            let Some(name) = declarations.get(&id) else {
                continue;
            };
            let Some(def) = self.registry.get(name) else {
                self.unresolved.insert(id, name.to_owned());
                continue;
            };
            let def = def.clone();
            let created = !previous.contains_key(&id);
            for property in DRIVEN_PROPERTIES {
                if def.timing(property).is_none() {
                    continue;
                }
                changed |= self.advance_property(
                    frame,
                    index,
                    &id,
                    &def,
                    property,
                    created,
                    previous,
                    now,
                    previous_now,
                );
            }
        }
        changed
    }

    #[allow(clippy::too_many_arguments)]
    fn advance_property(
        &mut self,
        frame: &mut PetrifiedFrame,
        index: usize,
        id: &str,
        def: &TransitionDef,
        property: PropertyKind,
        created: bool,
        previous: &BTreeMap<String, RawTarget>,
        now: f64,
        previous_now: f64,
    ) -> bool {
        let target = read_property(&frame.placements[index], property);
        // Reduced motion: the end state is identical and it is reached now.
        // Any trajectory already running for this property was completed by
        // `set_reduced_motion`, so there is nothing here to stop.
        if !self.policy.animates(property) {
            self.nodes.get_mut(id).map(|t| t.remove(&property));
            return false;
        }
        let timing = def
            .timing(property)
            .expect("caller checked the definition drives this property")
            .clone();

        let tracks = self.nodes.entry(id.to_owned()).or_default();
        match tracks.get_mut(&property) {
            Some(track) => {
                if track.target != target {
                    track.retarget(target, now);
                }
            }
            None => {
                // A trajectory starts from where the node was, which is the
                // previous frame's value — or, for a node that has just been
                // created, from its declared enter track, and if it declares
                // none it simply appears at its target.
                //
                // An **ambient** definition is the exception, and it is the
                // other half of `contracts/draw-list.md` §8. Every arm below
                // asks "what changed", and an ambient keyframe loop is not a
                // response to a change: it is declared, endless motion whose
                // track carries absolute values. A canvas placed once and
                // never moved by layout would otherwise never start its walk
                // and would sit still forever under a declaration that says it
                // never stops — which is exactly the shape of hole
                // `Timing::looped_time` was widened to close at the other end.
                // `target` is the origin because a keyframe track replaces the
                // laid-out value rather than interpolating toward it, so the
                // origin only decides where the property sits if the track
                // ends, and a looped one does not.
                let origin = if def.is_ambient() {
                    match def.enter() {
                        Some(track) if created && track.property == property => track.value,
                        _ => target,
                    }
                } else if created {
                    match def.enter() {
                        Some(track) if track.property == property => track.value,
                        _ => return false,
                    }
                } else {
                    let Some(before) = previous.get(id) else {
                        return false;
                    };
                    let was = read_raw(*before, property);
                    if was == target {
                        return false;
                    }
                    was
                };
                let clock = motion_clock(previous_now, now);
                tracks.insert(
                    property,
                    TrackState {
                        timing,
                        property,
                        origin,
                        velocity0: AnimVector::zeros(origin.len()),
                        target,
                        start: clock,
                        last_now: clock,
                        value: origin,
                        velocity: AnimVector::zeros(origin.len()),
                        settled: false,
                        looping: def.is_ambient(),
                    },
                );
            }
        }

        let track = tracks
            .get_mut(&property)
            .expect("just inserted or already present");
        let value = track.advance(now);
        if value == target {
            // Settled, or a trajectory that happens to be exactly there. The
            // placement already carries the target, so writing it would be a
            // no-op and marking the frame changed would cost a rehash.
            return false;
        }
        write_property(&mut frame.placements[index], property, value);
        true
    }

    /// Turn nodes that left the tree into exit groups. Answers whether any
    /// group was created.
    fn collect_exits(
        &mut self,
        previous: &BTreeMap<String, RawTarget>,
        raw: &BTreeMap<String, RawTarget>,
        now: f64,
        previous_now: f64,
    ) -> bool {
        let gone: Vec<&str> = previous
            .keys()
            .map(String::as_str)
            .filter(|id| !raw.contains_key(*id))
            .collect();
        if gone.is_empty() {
            return false;
        }
        let gone_set: BTreeSet<&str> = gone.iter().copied().collect();
        let mut created = false;
        for id in &gone {
            let Some(item) = self.rendered.get(*id) else {
                continue;
            };
            // Only the topmost node of a departing subtree forms a group; its
            // descendants come with it.
            if let Some(parent_id) = self.rendered_parent(id)
                && gone_set.contains(parent_id.as_str())
            {
                continue;
            }
            let Some(name) = self.declared.get(*id).cloned() else {
                continue;
            };
            let Some(def) = self.registry.get(&name) else {
                continue;
            };
            let ExitRule::RunExitTrack(exit) = def.exit().clone() else {
                continue;
            };
            let Some(timing) = def.timing(exit.property).cloned() else {
                continue;
            };
            if !self.policy.animates(exit.property) {
                continue;
            }
            let parent_id = self.rendered_parent(id);
            // Nowhere to re-insert: no parent, or a parent that left too and
            // is not itself running an exit. Stated in `ExitRule`'s doc as the
            // one degradation, rather than dangling a placement with no parent
            // and breaking the frame's Merkle root.
            let Some(parent_id) = parent_id else {
                continue;
            };
            if !raw.contains_key(&parent_id) {
                continue;
            }
            let (items, parents) = self.rendered_subtree(id);
            if items.is_empty() {
                continue;
            }
            let recorded = read_property(&item.placement, exit.property);
            let clock = motion_clock(previous_now, now);
            // The exit continues from the departing node's own motion: if it
            // was already moving under this property, the exit carries that
            // velocity, by the same rule a retarget does.
            let carried = self
                .nodes
                .get(*id)
                .and_then(|t| t.get(&exit.property))
                .map_or_else(|| AnimVector::zeros(recorded.len()), |t| t.velocity);
            let track = TrackState {
                timing,
                property: exit.property,
                origin: recorded,
                velocity0: carried,
                target: exit.value,
                start: clock,
                last_now: clock,
                value: recorded,
                velocity: carried,
                settled: false,
                // An exit runs once and the node leaves. Looping one would
                // keep a departed subtree spliced into every later frame.
                looping: false,
            };
            self.nodes.remove(*id);
            self.exiting.push(ExitGroup {
                items,
                parents,
                parent_id: Some(parent_id),
                track,
                recorded,
            });
            created = true;
        }
        created
    }

    /// Advance every exit group and splice the unsettled ones back into the
    /// frame. Answers whether the frame was rewritten.
    fn apply_exits(&mut self, frame: &mut PetrifiedFrame, now: f64) -> bool {
        if self.exiting.is_empty() {
            return false;
        }
        let mut groups = std::mem::take(&mut self.exiting);
        let mut changed = false;
        for group in &mut groups {
            let value = group.track.advance(now);
            if group.track.settled {
                continue;
            }
            let Some(parent_id) = group.parent_id.clone() else {
                continue;
            };
            let Some(parent_index) = frame.placements.iter().position(|p| p.id == parent_id) else {
                // The parent left between frames. There is nothing to hang
                // the exiting subtree from, so it completes now.
                group.track.settled = true;
                continue;
            };
            splice_group(frame, group, parent_index, value);
            changed = true;
        }
        groups.retain(|g| !g.track.settled);
        self.exiting = groups;
        changed
    }

    fn rendered_parent(&self, id: &str) -> Option<String> {
        let ancestors = KeyPath::ancestor_ids(id);
        ancestors
            .into_iter()
            .find(|candidate| self.rendered.contains_key(candidate))
    }

    /// The recorded subtree rooted at `id`, in pre-order, with parents
    /// re-based to indices within the returned vector.
    fn rendered_subtree(&self, id: &str) -> (Vec<ExitItem>, Vec<Option<usize>>) {
        let prefix = format!("{id}/");
        let mut ids: Vec<&String> = self
            .rendered
            .keys()
            .filter(|k| k.as_str() == id || k.starts_with(&prefix))
            .collect();
        // `rendered` is a BTreeMap, so this is already the lexicographic
        // order of the ids. Pre-order is what the splice needs, and for ids
        // that share a prefix the two agree: a parent's id is a proper prefix
        // of every descendant's and therefore sorts before all of them.
        ids.sort_unstable();
        let local: BTreeMap<&str, usize> = ids
            .iter()
            .enumerate()
            .map(|(i, k)| (k.as_str(), i))
            .collect();
        let mut items = Vec::with_capacity(ids.len());
        let mut parents = Vec::with_capacity(ids.len());
        for key in &ids {
            items.push(self.rendered[*key].clone());
            let parent = KeyPath::ancestor_ids(key)
                .into_iter()
                .find_map(|ancestor| local.get(ancestor.as_str()).copied());
            parents.push(parent);
        }
        (items, parents)
    }

    fn snapshot_rendered(&mut self, frame: &PetrifiedFrame, declarations: &Declarations) {
        self.rendered.clear();
        self.declared.clear();
        for (index, placement) in frame.placements.iter().enumerate() {
            if let Some(name) = declarations.get(&placement.id) {
                self.declared.insert(placement.id.clone(), name.to_owned());
            }
            self.rendered.insert(
                placement.id.clone(),
                ExitItem {
                    placement: placement.clone(),
                    content: frame.content[index].clone(),
                    slot: frame.slots[index],
                },
            );
        }
    }

    /// The activity a frame carries: how many trajectories are still moving,
    /// and how many declared-endless animations exist.
    ///
    /// Ambient is a set of ids, not a sum of two counts: a node that both
    /// declares `ambient` on the tree and names an ambient transition is one
    /// endless animation, not two.
    fn activity(&self, frame: &PetrifiedFrame, declarations: &Declarations) -> TransitionActivity {
        let running = self
            .nodes
            .values()
            .flat_map(BTreeMap::values)
            // A looping track is excluded, not because it is finished but
            // because it never will be. `TransitionActivity::running` is
            // "transitions still moving *toward a target*", and an ambient
            // keyframe loop has no target it is heading for — it replaces the
            // laid-out value forever. Counting one here would make
            // `is_settled` permanently false and a driver's settle wait
            // unreachable, which is the same contradiction
            // `TransitionBuilder::build` refuses when an ambient definition
            // drives a spring. `TransitionActivity::ambient` is where an
            // endless animation is counted, and `wants_frame` reads that.
            .filter(|track| !track.settled && !track.looping)
            .count()
            + self
                .exiting
                .iter()
                .filter(|group| !group.track.settled)
                .count();
        let mut ambient: BTreeSet<&str> = BTreeSet::new();
        for placement in &frame.placements {
            let id = placement.id.as_str();
            if placement.semantics.ambient || declarations.is_ambient(id) {
                ambient.insert(id);
                continue;
            }
            if let Some(name) = declarations.get(id)
                && self
                    .registry
                    .get(name)
                    .is_some_and(TransitionDef::is_ambient)
            {
                ambient.insert(id);
            }
        }
        TransitionActivity {
            running,
            ambient: ambient.len(),
        }
    }
}

/// Read one animatable property off a placement.
fn read_property(placement: &Placement, property: PropertyKind) -> AnimVector {
    match property {
        PropertyKind::Position => AnimVector::new(
            [
                f64::from(placement.rect.x),
                f64::from(placement.rect.y),
                0.0,
                0.0,
            ],
            2,
        ),
        PropertyKind::Size => AnimVector::new(
            [
                f64::from(placement.rect.w),
                f64::from(placement.rect.h),
                0.0,
                0.0,
            ],
            2,
        ),
        PropertyKind::Opacity => AnimVector::scalar(f64::from(placement.opacity)),
        PropertyKind::Color => unreachable!("DRIVEN_PROPERTIES excludes Color"),
    }
}

fn read_raw(raw: RawTarget, property: PropertyKind) -> AnimVector {
    match property {
        PropertyKind::Position => {
            AnimVector::new([f64::from(raw.rect.x), f64::from(raw.rect.y), 0.0, 0.0], 2)
        }
        PropertyKind::Size => {
            AnimVector::new([f64::from(raw.rect.w), f64::from(raw.rect.h), 0.0, 0.0], 2)
        }
        PropertyKind::Opacity => AnimVector::scalar(f64::from(raw.opacity)),
        PropertyKind::Color => unreachable!("DRIVEN_PROPERTIES excludes Color"),
    }
}

/// Write one animatable property onto a placement.
///
/// Opacity is clamped into `[0, 1]` and extents to at least zero, because
/// [`Placement`]'s own contract states those ranges and an underdamped spring
/// overshoots by design. The clamp bites only mid-flight: a settled track
/// writes the exact target, which was already in range because layout put it
/// there.
fn write_property(placement: &mut Placement, property: PropertyKind, value: AnimVector) {
    match property {
        PropertyKind::Position => {
            placement.rect.x = value.get(0) as f32;
            placement.rect.y = value.get(1) as f32;
        }
        PropertyKind::Size => {
            placement.rect.w = (value.get(0) as f32).max(0.0);
            placement.rect.h = (value.get(1) as f32).max(0.0);
        }
        PropertyKind::Opacity => {
            placement.opacity = (value.get(0) as f32).clamp(0.0, 1.0);
        }
        PropertyKind::Color => unreachable!("DRIVEN_PROPERTIES excludes Color"),
    }
}

/// Every placement's subtree size, from the parent chain alone.
///
/// Walks last to first: a parent always sits at an earlier index than its
/// children in pre-order, so by the time index `i` is read every descendant
/// has already been folded into it.
fn subtree_lens(parents: &[Option<usize>]) -> Vec<usize> {
    let mut lens = vec![1_usize; parents.len()];
    for i in (0..parents.len()).rev() {
        if let Some(parent) = parents[i] {
            lens[parent] += lens[i];
        }
    }
    lens
}

/// Splice one exit group's recorded subtree back into `frame` as the last
/// child of `parent_index`, moved to `value`.
///
/// Inserting a contiguous pre-order subtree immediately after its parent's own
/// subtree keeps the whole array in pre-order, which is what
/// [`crate::frame::digest::subtree_hashes`] requires.
fn splice_group(
    frame: &mut PetrifiedFrame,
    group: &ExitGroup,
    parent_index: usize,
    value: AnimVector,
) {
    let parents: Vec<Option<usize>> = frame.placements.iter().map(|p| p.parent).collect();
    let lens = subtree_lens(&parents);
    let at = parent_index + lens[parent_index];
    let n = group.items.len();

    for placement in &mut frame.placements {
        if let Some(parent) = placement.parent.as_mut()
            && *parent >= at
        {
            *parent += n;
        }
    }

    let mut placements = Vec::with_capacity(n);
    let mut content = Vec::with_capacity(n);
    let mut slots = Vec::with_capacity(n);
    for (local, item) in group.items.iter().enumerate() {
        let mut placement = item.placement.clone();
        placement.parent = match group.parents[local] {
            Some(p) => Some(at + p),
            None => Some(parent_index),
        };
        apply_exit_value(&mut placement, group, value);
        placements.push(placement);
        content.push(item.content.clone());
        slots.push(item.slot);
    }

    splice_at(&mut frame.placements, at, placements);
    splice_at(&mut frame.content, at, content);
    splice_at(&mut frame.slots, at, slots);
}

/// Apply the exiting root's animated value to one member of its subtree.
///
/// Opacity propagates as a ratio, because [`Placement::opacity`] is
/// *cumulative*: fading the root to a third of what it was must fade every
/// descendant by the same third, or a child would outlive its parent on
/// screen. Position propagates as a translation, applied to the clip as well
/// as the rect so the subtree moves rather than shearing out of its own clip.
fn apply_exit_value(placement: &mut Placement, group: &ExitGroup, value: AnimVector) {
    match group.track.property {
        PropertyKind::Opacity => {
            let was = group.recorded.get(0);
            let ratio = if was.abs() < f64::EPSILON {
                0.0
            } else {
                value.get(0) / was
            };
            placement.opacity = (placement.opacity * ratio as f32).clamp(0.0, 1.0);
        }
        PropertyKind::Position => {
            let dx = (value.get(0) - group.recorded.get(0)) as f32;
            let dy = (value.get(1) - group.recorded.get(1)) as f32;
            placement.rect.x += dx;
            placement.rect.y += dy;
            placement.clip.x += dx;
            placement.clip.y += dy;
        }
        PropertyKind::Size | PropertyKind::Color => {
            unreachable!("an exit track drives Opacity or Position; the builder refuses the rest")
        }
    }
}

fn splice_at<T>(target: &mut Vec<T>, at: usize, items: Vec<T>) {
    let tail = target.split_off(at);
    target.extend(items);
    target.extend(tail);
}

/// Recompute everything downstream of the placements: subtree sizes, subtree
/// hashes, and the digest.
///
/// A full walk, no reuse: a transition typically moves a whole subtree, and a
/// partially-known hash array would have to be proven correct against a frame
/// this function just rewrote. `subtree_hashes` is linear in the placement
/// count and runs once per animating frame.
fn rebuild_identity(frame: &mut PetrifiedFrame) {
    let parents: Vec<Option<usize>> = frame.placements.iter().map(|p| p.parent).collect();
    frame.subtree_len = subtree_lens(&parents);
    frame.subtree_hashes = digest::subtree_hashes(frame.viewport.scale, &frame.placements);
    frame.digest = digest::digest_from_root(
        &frame.viewport,
        digest::root_hash_from(&frame.subtree_hashes),
    );
    debug_assert_eq!(
        frame.digest,
        digest::digest(&frame.viewport, &frame.placements),
        "the animated frame's digest must be the digest of its own placements"
    );
    debug_assert!(
        frame.paint_hashes_agree(),
        "an animated frame lost the agreement between its placements' paint \
         hashes and its paint payloads"
    );
}

#[cfg(test)]
mod tests {
    use super::{Declarations, TransitionEngine, subtree_lens};
    use crate::anim::fixtures;
    use crate::anim::registry::TransitionRegistry;
    use crate::tree::{NodeKind, ViewNode};

    use fixtures::definitions as slide_registry;

    #[test]
    fn declarations_key_on_the_same_ids_the_frame_carries() {
        let tree = ViewNode::new(NodeKind::Stack, "app")
            .child(ViewNode::new(NodeKind::Text, "title").with_transition("slide"))
            .child(ViewNode::new(NodeKind::Text, "body").with_ambient(true));
        let declarations = Declarations::collect(&tree);
        assert_eq!(declarations.get("/app/title"), Some("slide"));
        assert!(declarations.is_ambient("/app/body"));
        assert_eq!(declarations.len(), 1);

        let frame = fixtures::frame(&tree, 1, 200.0, 100.0);
        let ids: Vec<&str> = frame.placements.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"/app/title"), "{ids:?}");
        assert!(ids.contains(&"/app/body"), "{ids:?}");
    }

    /// An ambient keyframe loop starts on its own and never stops, on a node
    /// whose laid-out position never moves.
    ///
    /// The other half of `contracts/draw-list.md` §8, and the half that is
    /// invisible from `registry.rs`: `Timing::looped_time` makes a running
    /// ambient track wrap, and this makes one *run*. Every other trajectory in
    /// this engine starts because something changed; an ambient one is
    /// declared motion, and a canvas placed once and never moved by layout
    /// would otherwise sit still forever under a declaration that says it
    /// never stops.
    ///
    /// Two assertions, and the second is what makes the first mean anything:
    /// the placement is somewhere other than where layout put it, and it is
    /// somewhere *different* one second later.
    #[test]
    fn an_ambient_keyframe_loop_starts_itself_and_keeps_going() {
        use crate::anim::curve::{CubicBezier, Keyframe, KeyframeTrack};
        use crate::anim::registry::{Timing, TransitionDef};
        use crate::anim::value::{AnimVector, PropertyKind};

        let track = KeyframeTrack::new(vec![
            Keyframe {
                time: 0.0,
                value: AnimVector::new([0.0, 0.0, 0.0, 0.0], 2),
                easing_in: CubicBezier::LINEAR,
            },
            Keyframe {
                time: 2.0,
                value: AnimVector::new([80.0, 0.0, 0.0, 0.0], 2),
                easing_in: CubicBezier::LINEAR,
            },
        ])
        .unwrap();
        let mut registry = TransitionRegistry::new();
        registry.register(
            "drift",
            TransitionDef::builder()
                .drive(PropertyKind::Position, Timing::Keyframes(track))
                .ambient(true)
                .build()
                .unwrap(),
        );

        // The tree never changes: same offset, same text, every frame. Under
        // the pre-005 rule nothing here would ever start a trajectory.
        let tree = ViewNode::new(NodeKind::Stack, "app").child(
            ViewNode::new(NodeKind::Text, "mover")
                .with_props(crate::tree::Props {
                    text: Some("mover".into()),
                    ..crate::tree::Props::default()
                })
                .with_transition("drift")
                .with_ambient(true),
        );

        let mut engine = TransitionEngine::new(registry);
        let declarations = Declarations::collect(&tree);
        let settled = fixtures::frame(&tree, 1, 200.0, 100.0);
        let laid_out = settled
            .placement("/app/mover")
            .expect("the fixture places a mover")
            .rect
            .x;

        let mut xs = Vec::new();
        for step in 0..12_u32 {
            let mut frame = fixtures::frame(&tree, u64::from(step) + 1, 200.0, 100.0);
            let activity = engine.animate(&mut frame, &declarations, f64::from(step) * 0.25);
            xs.push(frame.placement("/app/mover").expect("placed").rect.x);
            assert_eq!(
                activity.ambient, 1,
                "step {step}: the loop is counted as an endless animation"
            );
            assert!(
                activity.is_settled(),
                "step {step}: an ambient loop must never block settle, however \
                 much it is moving (contracts/animation.md)"
            );
            assert!(
                crate::anim::wants_frame(activity),
                "step {step}: and yet frames must keep coming"
            );
        }

        assert!(
            xs.iter().any(|x| (*x - laid_out).abs() > 1.0),
            "the ambient track never moved the placement off its laid-out x              ({laid_out}); saw {xs:?}"
        );
        assert!(
            xs.windows(2).any(|w| (w[1] - w[0]).abs() > 0.5),
            "the ambient track started and then stopped; saw {xs:?}"
        );
    }

    #[test]
    fn subtree_lens_counts_a_whole_subtree_including_itself() {
        // 0 root, 1 child of 0, 2 child of 1, 3 child of 0.
        let parents = [None, Some(0), Some(1), Some(0)];
        assert_eq!(subtree_lens(&parents), vec![4, 2, 1, 1]);
    }

    /// The first frame an engine ever sees has nothing to animate from, so it
    /// must not move anything and must not change the frame's identity.
    #[test]
    fn the_first_frame_is_left_exactly_as_petrified() {
        let tree = fixtures::two_panels(0.0);
        let mut engine = TransitionEngine::new(slide_registry());
        let declarations = Declarations::collect(&tree);
        let reference = fixtures::frame(&tree, 1, 300.0, 120.0);
        let mut frame = fixtures::frame(&tree, 1, 300.0, 120.0);
        let activity = engine.animate(&mut frame, &declarations, 0.0);
        assert_eq!(frame.digest, reference.digest);
        assert_eq!(frame.placements, reference.placements);
        assert_eq!(activity.running, 0);
        assert!(activity.is_settled());
    }

    /// A target change starts a trajectory, the frame carries the
    /// mid-transition picture, and running counts it.
    #[test]
    fn a_moved_node_animates_and_reports_itself_running() {
        let mut engine = TransitionEngine::new(slide_registry());
        let at_zero = fixtures::two_panels(0.0);
        let mut frame = fixtures::frame(&at_zero, 1, 300.0, 120.0);
        engine.animate(&mut frame, &Declarations::collect(&at_zero), 0.0);

        let moved = fixtures::two_panels(60.0);
        let target = fixtures::frame(&moved, 2, 300.0, 120.0);
        let mut frame = fixtures::frame(&moved, 2, 300.0, 120.0);
        let activity = engine.animate(&mut frame, &Declarations::collect(&moved), 0.016);
        assert_eq!(activity.running, 1, "one position trajectory");
        assert!(!activity.is_settled());
        let animated = frame.placement("/app/mover").unwrap().rect.x;
        let wanted = target.placement("/app/mover").unwrap().rect.x;
        assert!(
            animated < wanted,
            "16 ms in, the node is behind its target: {animated} vs {wanted}"
        );
        assert_ne!(frame.digest, target.digest, "the picture really moved");
    }

    /// SC-002 paints nothing at idle, so the next click can land a second
    /// later. That gap is not curve time: a 140 ms slide must still
    /// interpolate, not finish on the first frame.
    #[test]
    fn a_second_move_after_idle_still_interpolates() {
        let mut engine = TransitionEngine::new(slide_registry());
        let at_zero = fixtures::two_panels(0.0);
        let moved = fixtures::two_panels(60.0);
        let declarations_zero = Declarations::collect(&at_zero);
        let declarations_moved = Declarations::collect(&moved);

        let mut frame = fixtures::frame(&at_zero, 1, 300.0, 120.0);
        engine.animate(&mut frame, &declarations_zero, 0.0);

        let mut now = 0.016;
        loop {
            let mut frame = fixtures::frame(&moved, 2, 300.0, 120.0);
            let activity = engine.animate(&mut frame, &declarations_moved, now);
            if activity.is_settled() {
                break;
            }
            now += 0.016;
            assert!(now < 2.0, "the first slide never settled");
        }

        let idle_now = now + 1.0;
        let target = fixtures::frame(&at_zero, 3, 300.0, 120.0)
            .placement("/app/mover")
            .unwrap()
            .rect
            .x;
        let mut frame = fixtures::frame(&at_zero, 3, 300.0, 120.0);
        let activity = engine.animate(&mut frame, &declarations_zero, idle_now);
        assert_eq!(activity.running, 1, "idle must not eat the second slide");
        assert!(!activity.is_settled());
        let animated = frame.placement("/app/mover").unwrap().rect.x;
        assert!(
            (animated - target).abs() > 1.0,
            "after 1 s idle the second slide finished in one frame: at {animated}, target {target}"
        );
    }

    /// The settle-then-snap rule, end to end: run to settle and the frame is
    /// byte-identical to a build straight to the target.
    #[test]
    fn a_settled_frame_has_no_near_target_residue() {
        let mut engine = TransitionEngine::new(slide_registry());
        let at_zero = fixtures::two_panels(0.0);
        let mut frame = fixtures::frame(&at_zero, 1, 300.0, 120.0);
        engine.animate(&mut frame, &Declarations::collect(&at_zero), 0.0);

        let moved = fixtures::two_panels(60.0);
        let declarations = Declarations::collect(&moved);
        let reference = fixtures::frame(&moved, 99, 300.0, 120.0);
        let mut last = TransitionEngineFrame::default();
        for step in 1..200 {
            let mut frame = fixtures::frame(&moved, 2, 300.0, 120.0);
            let activity = engine.animate(&mut frame, &declarations, f64::from(step) * 0.016);
            last = TransitionEngineFrame {
                settled: activity.is_settled(),
                digest_matches: frame.digest == reference.digest,
                placements_match: frame.placements == reference.placements,
            };
            if last.settled {
                break;
            }
        }
        assert!(last.settled, "the transition never settled");
        assert!(last.placements_match, "settled placements carry residue");
        assert!(
            last.digest_matches,
            "the settled digest is not the target's"
        );
    }

    #[derive(Default)]
    struct TransitionEngineFrame {
        settled: bool,
        digest_matches: bool,
        placements_match: bool,
    }

    /// An unresolvable name is recorded and answerable, never a node that
    /// silently refuses to move.
    #[test]
    fn an_unresolvable_name_is_recorded() {
        let tree = ViewNode::new(NodeKind::Stack, "app")
            .child(ViewNode::new(NodeKind::Text, "t").with_transition("nope"));
        let declarations = Declarations::collect(&tree);
        let mut engine = TransitionEngine::new(TransitionRegistry::new());
        let mut frame = fixtures::frame(&tree, 1, 200.0, 80.0);
        engine.animate(&mut frame, &declarations, 0.0);
        let mut frame = fixtures::frame(&tree, 2, 200.0, 80.0);
        engine.animate(&mut frame, &declarations, 0.016);
        assert_eq!(
            engine.unresolved().get("/app/t").map(String::as_str),
            Some("nope")
        );
    }
}
