//! The proposal/response vocabulary and the measurement cache.
//!
//! Four proposal values, per axis (`contracts/view-tree.md`). A parent may
//! probe a child with several of them before it commits one; the cache is what
//! makes that affordable, which is why it is part of the vocabulary module
//! rather than an optimisation bolted on later.

use std::collections::{BTreeSet, HashSet};
use std::hash::{Hash, Hasher};

use crate::cache::{CacheMap, LruCache, reserve_index};
use crate::geom::{Axis, Scale, Size};
use crate::tree::KeyPath;

/// One axis of a size offer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Proposal {
    /// The parent offers exactly this extent. The child answers with what it
    /// takes for that offer, which may differ: a parent places, it does not
    /// force.
    Exact(f32),
    /// Minimum probe. The child answers with its smallest acceptable extent.
    Zero,
    /// Maximum probe. The child answers with its largest useful extent — a
    /// full unwrapped text run, a scroll container's content size.
    Unbounded,
    /// Ideal probe. The child answers at its natural content extent.
    Unspecified,
}

impl Proposal {
    /// The offered extent when this is [`Proposal::Exact`].
    #[must_use]
    pub fn exact(self) -> Option<f32> {
        match self {
            Self::Exact(v) => Some(v),
            _ => None,
        }
    }

    /// The extent a leaf should treat as available.
    ///
    /// `Zero` is zero, `Exact` is its value, and both open-ended probes are
    /// `None` — a leaf must answer them from its content, never from a
    /// stand-in number that would silently become a layout constant.
    #[must_use]
    pub fn available(self) -> Option<f32> {
        match self {
            Self::Exact(v) => Some(v.max(0.0)),
            Self::Zero => Some(0.0),
            Self::Unbounded | Self::Unspecified => None,
        }
    }

    /// Whether this probe is open-ended.
    #[must_use]
    pub fn is_open(self) -> bool {
        matches!(self, Self::Unbounded | Self::Unspecified)
    }

    /// This proposal reduced by `amount` when it carries a value; open probes
    /// pass through unchanged. Used to reserve spacing before distribution.
    #[must_use]
    pub fn shrink(self, amount: f32) -> Self {
        match self {
            Self::Exact(v) => Self::Exact((v - amount).max(0.0)),
            other => other,
        }
    }
}

impl Eq for Proposal {}

impl Hash for Proposal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Exact(v) => {
                0u8.hash(state);
                // Bit pattern, not value: two `Exact` proposals that differ in
                // the last bit are two cache entries, never one.
                v.to_bits().hash(state);
            }
            Self::Zero => 1u8.hash(state),
            Self::Unbounded => 2u8.hash(state),
            Self::Unspecified => 3u8.hash(state),
        }
    }
}

/// A per-axis size offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SizeProposal {
    /// Horizontal offer.
    pub horizontal: Proposal,
    /// Vertical offer.
    pub vertical: Proposal,
}

impl SizeProposal {
    /// The same proposal on both axes.
    #[must_use]
    pub fn both(p: Proposal) -> Self {
        Self {
            horizontal: p,
            vertical: p,
        }
    }

    /// The minimum probe on both axes.
    #[must_use]
    pub fn zero() -> Self {
        Self::both(Proposal::Zero)
    }

    /// The maximum probe on both axes.
    #[must_use]
    pub fn unbounded() -> Self {
        Self::both(Proposal::Unbounded)
    }

    /// The ideal probe on both axes.
    #[must_use]
    pub fn unspecified() -> Self {
        Self::both(Proposal::Unspecified)
    }

    /// An exact offer on both axes.
    #[must_use]
    pub fn exact(size: Size) -> Self {
        Self {
            horizontal: Proposal::Exact(size.w),
            vertical: Proposal::Exact(size.h),
        }
    }

    /// The offer on `axis`.
    #[must_use]
    pub fn axis(self, axis: Axis) -> Proposal {
        match axis {
            Axis::Horizontal => self.horizontal,
            Axis::Vertical => self.vertical,
        }
    }

    /// This offer with `axis` replaced.
    #[must_use]
    pub fn with_axis(mut self, axis: Axis, p: Proposal) -> Self {
        match axis {
            Axis::Horizontal => self.horizontal = p,
            Axis::Vertical => self.vertical = p,
        }
        self
    }
}

/// The cache key: a node, an offer, and everything a response depends on.
///
/// Theme and scale are the whole invalidation story left in the key itself.
/// Content is not: a per-node global revision here is exactly the design
/// this key used to carry and no longer does
/// (`.agents/notes/implemented/architecture/2026-08-22-petra-incremental-frames.md`,
/// "A per-node revision counters instead of a change set" in Alternatives
/// considered) — content invalidation is [`ChangeSet`], applied once per
/// frame by [`MeasureCache::apply`] rather than compared per lookup. Adding a
/// dependency to a measurement without adding it here (or to `apply`) is how
/// a layout engine starts answering last frame's question, so the key is
/// deliberately explicit rather than a hash of "some context".
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MeasureKey {
    /// Canonical id of the node being measured.
    pub node: String,
    /// The offer.
    pub proposal: SizeProposal,
    /// Revision of the theme snapshot in force.
    pub theme_rev: u64,
    /// Display scale.
    pub scale: Scale,
}

/// What changed behind the view tree since the last frame.
///
/// The host is the only thing that knows what moved, and it hands this back
/// once per frame (`gorgon-petra-egui`'s `App::take_changes`, for the
/// shipping host). The engine, not the host, turns the answer into cache
/// invalidation ([`MeasureCache::apply`]): a host that reported the changed
/// leaf but not its ancestors would otherwise get a stale frame, because
/// `measure` hits at an ancestor and returns before the walk ever reaches the
/// leaf. Doing the ancestor walk here means no host can forget it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeSet {
    /// Everything, or the host does not know which nodes moved. The honest
    /// conservative answer: reproduces today's cost exactly.
    All,
    /// These node ids and nothing else. Ids are canonical key paths
    /// ([`KeyPath::id`]).
    Nodes(BTreeSet<String>),
    /// Nothing moved.
    None,
}

impl ChangeSet {
    /// Whatever moved, moved in the tree: nothing outside it (rows aside,
    /// which the engine checks itself) answers differently.
    ///
    /// For a host that grafts each new view onto the last one
    /// (`gorgon-petra-egui`'s `Host` does), this is all an application that
    /// rebuilds its tree from state has to say: the graft names what moved.
    /// It is not [`ChangeSet::None`], which also tells such a host it may skip
    /// asking for a view at all. To a caller of
    /// [`crate::frame::petrify_with_memo`] that diffs nothing, it reads as
    /// "no node declared", which is what it is.
    #[must_use]
    pub fn in_tree() -> Self {
        Self::Nodes(BTreeSet::new())
    }

    /// A change set naming exactly one node.
    #[must_use]
    pub fn node(id: impl Into<String>) -> Self {
        let mut set = BTreeSet::new();
        set.insert(id.into());
        Self::Nodes(set)
    }
}

/// Memoized responses for one layout pass family.
///
/// Bounded, because the key names a node and a node id is unbounded: a
/// 100 000-row virtualized collection would otherwise leave one entry per row
/// ever scrolled past, which is the growth SC-008 forbids. Eviction is
/// least-recently-used ([`LruCache`]), which keeps both the multi-probe
/// entries of the frame in flight and the previous frame's window. A host
/// that calls [`MeasureCache::begin_pass`] per frame also never loses a
/// measurement the current or the previous frame took, however dense the tree.
#[derive(Debug)]
pub struct MeasureCache {
    /// Each response with the epoch it was stored at.
    entries: LruCache<MeasureKey, (Size, u64)>,
    /// Every node that has an entry held: how many, and the epoch the node
    /// was last invalidated at. An entry stored at or before its node's mark
    /// is stale and answers as a miss.
    ///
    /// Invalidation marks rather than scans: finding a node's entries by scan
    /// is O(entries) per call, and an incremental host invalidates on most
    /// frames (each row a scroll brings into view names its nodes). A mark
    /// is kept only for a node with an entry held, and leaves with the node's
    /// last entry, so the marks are bounded by the entries and never need a
    /// sweep: an id with nothing cached (most of what a scroll names) is
    /// skipped outright. Sized like the entries' index
    /// ([`crate::cache::reserve_index`]), so it never rehashes in a frame.
    nodes: CacheMap<String, NodeEntries>,
    /// How many of `nodes` carry a mark. While none does, a lookup skips the
    /// node table.
    marked: usize,
    /// The theme revision and scale [`MeasureCache::retain_theme_and_scale`]
    /// last kept, and how many entries held now were stored under another
    /// pair. While that count is zero and the pair is unchanged, the call
    /// has nothing to drop and does not walk the entries.
    current: Option<(u64, Scale)>,
    foreign: usize,
    /// Advances on every store and every invalidation.
    epoch: u64,
    hits: u64,
    misses: u64,
}

/// A node's share of [`MeasureCache`]: its entries held, and the epoch it was
/// last invalidated at (0 for never).
#[derive(Clone, Copy, Debug)]
struct NodeEntries {
    held: usize,
    invalidated: u64,
}

impl Default for MeasureCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MeasureCache {
    /// Entries a cache from [`MeasureCache::new`] holds.
    ///
    /// One frame's working set is what has to fit: every (node, proposal) pair
    /// the pass probes. For a *virtualized* tree that is the visible window
    /// plus overscan times the probes each parent makes — tens to low
    /// hundreds. For a *dense* one it is a multiple of the node count, and
    /// spec 003's acceptance application is dense: the fiber inspector puts an
    /// identity bar, a fiber list, an effects tree and several tables on screen
    /// at once, none of them virtualized.
    ///
    /// This default was first set at 8192 from the virtualized reading alone,
    /// and `benches/negotiate.rs` measured what that costs on the other shape:
    /// a 4225-node screen needs about 25 000 entries, so it thrashed at 45 825
    /// evictions and ran 73% slower (16.16 ms against 9.35 ms) while still
    /// reporting a healthy hit rate on the scroll workload the bound was tuned
    /// for. The measured ratio is close to six entries per node, so this value
    /// covers a screen of roughly eleven thousand nodes.
    ///
    /// A full cache is on the order of thirty megabytes: the entries and
    /// their keys, and three tables sized for the bound when the first entry
    /// lands (the index and the node table at twice the bound, so neither
    /// ever rehashes inside a frame; [`crate::cache`] has why). That is a
    /// bound on *entries*, not on bytes — nothing here instruments the
    /// allocator, and a host that cares reads [`MeasureCache::evictions`],
    /// which is non-zero exactly when the bound is costing it work. A host
    /// with a denser tree still raises it with [`MeasureCache::with_capacity`].
    ///
    /// `tests/measure_cache_capacity.rs` fails if a dense screen stops fitting.
    pub const DEFAULT_CAPACITY: usize = 65_536;

    /// An empty cache bounded to [`MeasureCache::DEFAULT_CAPACITY`] entries.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// An empty cache bounded to `capacity` entries.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: LruCache::with_capacity(capacity),
            nodes: CacheMap::default(),
            marked: 0,
            current: None,
            foreign: 0,
            epoch: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// The entry bound in force.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Change the entry bound, evicting immediately if it shrank.
    pub fn set_capacity(&mut self, capacity: usize) {
        let Self {
            entries,
            nodes,
            marked,
            current,
            foreign,
            ..
        } = self;
        entries.set_capacity_reporting(capacity, |key, _| {
            release(nodes, marked, current, foreign, key);
        });
    }

    /// Mark the start of a frame. Measurements taken this frame or the last
    /// are never evicted, so a tree whose working set is over the bound still
    /// hits on every repeat instead of thrashing ([`LruCache::begin_pass`]).
    pub fn begin_pass(&mut self) {
        let Self {
            entries,
            nodes,
            marked,
            current,
            foreign,
            ..
        } = self;
        entries.begin_pass_reporting(|key, _| release(nodes, marked, current, foreign, key));
    }

    /// Entries dropped to stay inside the bound since construction. A number
    /// that climbs every frame means the bound is below this tree's working
    /// set and the cache is thrashing.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.entries.evictions()
    }

    /// Look up a response without computing one, counting the hit or miss.
    ///
    /// Split from [`MeasureCache::insert`] rather than folded into a
    /// `get_or_insert_with`: the compute step re-enters layout with the same
    /// context that owns this cache, and a closure would hold the borrow.
    pub fn peek(&mut self, key: &MeasureKey) -> Option<Size> {
        let mark = if self.marked == 0 {
            0
        } else {
            self.nodes.get(&key.node).map_or(0, |node| node.invalidated)
        };
        match self
            .entries
            .get(key)
            .copied()
            .filter(|(_, stored)| *stored > mark)
            .map(|(size, _)| size)
        {
            Some(hit) => {
                self.hits += 1;
                Some(hit)
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    /// Store a response. Every stored value passes the sanity clamp, because
    /// a cached size is a digest input.
    pub fn insert(&mut self, key: MeasureKey, value: Size) -> Size {
        let value = value.sane();
        self.epoch += 1;
        let capacity = self.entries.capacity();
        let Self {
            entries,
            nodes,
            marked,
            current,
            foreign,
            epoch,
            ..
        } = self;
        let stored = entries.insert_reporting(key, (value, *epoch), |key, _| {
            release(nodes, marked, current, foreign, key);
        });
        if let Some(key) = stored {
            if *current != Some((key.theme_rev, key.scale)) {
                *foreign += 1;
            }
            match nodes.get_mut(&key.node) {
                Some(node) => node.held += 1,
                None => {
                    reserve_index(nodes, capacity);
                    nodes.insert(
                        key.node.clone(),
                        NodeEntries {
                            held: 1,
                            invalidated: 0,
                        },
                    );
                }
            }
        }
        value
    }

    /// Look up a response, or compute and store it.
    pub fn get_or_insert_with<F: FnOnce() -> Size>(&mut self, key: MeasureKey, compute: F) -> Size {
        if let Some(hit) = self.peek(&key) {
            return hit;
        }
        self.insert(key, compute())
    }

    /// Drop every entry whose theme or scale is stale.
    ///
    /// Content changes go through [`MeasureCache::apply`]; theme and scale
    /// are global inputs to every measurement, so a change to either
    /// invalidates wholesale rather than through a [`ChangeSet`].
    ///
    /// A host calls this every frame with the pair in force, so the walk only
    /// runs when it can drop something: the pair changed, or an entry was
    /// stored under another pair since the last call.
    pub fn retain_theme_and_scale(&mut self, theme_rev: u64, scale: Scale) {
        let pair = Some((theme_rev, scale));
        if self.current == pair && self.foreign == 0 {
            return;
        }
        self.current = pair;
        let Self {
            entries,
            nodes,
            marked,
            ..
        } = self;
        entries.retain(|key, _| {
            let keep = key.theme_rev == theme_rev && key.scale == scale;
            if !keep {
                forget(nodes, marked, &key.node);
            }
            keep
        });
        self.foreign = 0;
    }

    /// Drop every entry for one node. Does not touch its ancestors or its
    /// descendants — [`MeasureCache::apply`] is what a host wants for a
    /// content change; this is the primitive it is built from.
    pub fn invalidate_node(&mut self, node: &str) {
        self.invalidate_nodes(&std::iter::once(node.to_owned()).collect());
    }

    /// Drop everything.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.nodes.clear();
        self.marked = 0;
        self.foreign = 0;
    }

    /// Apply a change set: invalidate exactly what it names, and nothing more
    /// or less.
    ///
    /// - [`ChangeSet::All`] clears every entry.
    /// - [`ChangeSet::None`] touches nothing.
    /// - [`ChangeSet::Nodes`] invalidates each named id **and every ancestor
    ///   of it** ([`KeyPath::ancestor_ids`]), so a container above the change
    ///   cannot answer `measure` from a cached size that predates it. A
    ///   malformed id — empty, or missing the leading `/` a canonical id
    ///   always has — has no ancestors and is invalidated on its own; it does
    ///   not panic.
    pub fn apply(&mut self, changes: &ChangeSet) {
        match changes {
            ChangeSet::All => self.clear(),
            ChangeSet::None => {}
            ChangeSet::Nodes(ids) => {
                // One pass over the entries against the whole set, not one
                // `retain` per id: a frame that names thirty nodes fifteen
                // deep would otherwise scan every entry four hundred times.
                let mut doomed: HashSet<String> = HashSet::new();
                for id in ids {
                    if doomed.insert(id.clone()) {
                        doomed.extend(KeyPath::ancestor_ids(id));
                    }
                }
                self.invalidate_nodes(&doomed);
            }
        }
    }

    /// Drop every entry for every node in `ids`. Exactly the named nodes: no
    /// ancestor walk, which is what a caller wants when it has already
    /// decided how far up a change reaches (a collection row, whose ancestors
    /// measure from the extents ledger rather than from the row).
    ///
    /// O(`ids`): each id with an entry held is marked, not searched for, and
    /// an id with nothing held costs one lookup and leaves nothing behind.
    pub fn invalidate_nodes(&mut self, ids: &HashSet<String>) {
        if ids.is_empty() {
            return;
        }
        self.epoch += 1;
        for id in ids {
            if let Some(node) = self.nodes.get_mut(id) {
                if node.invalidated == 0 {
                    self.marked += 1;
                }
                node.invalidated = self.epoch;
            }
        }
    }

    /// Entries that can still answer: held, and stored after their node was
    /// last invalidated. A stale entry still takes a slot until the LRU drops
    /// it or a store under the same key replaces it, so this walks the
    /// entries; it is for tests and diagnostics, not for a frame.
    #[must_use]
    pub fn len(&self) -> usize {
        if self.marked == 0 {
            return self.entries.len();
        }
        self.entries
            .iter()
            .filter(|(key, (_, stored))| {
                *stored > self.nodes.get(&key.node).map_or(0, |node| node.invalidated)
            })
            .count()
    }

    /// Whether the cache holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Hit and miss counts since construction. The bench and the multi-probe
    /// claim in FR-035 are measured from these.
    #[must_use]
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

/// An entry of `key` left the cache: count it off its node, and forget the
/// node, mark and all, with its last entry. A mark only ever guarded entries
/// stored before it, and none of those is held any more.
fn release(
    nodes: &mut CacheMap<String, NodeEntries>,
    marked: &mut usize,
    current: &Option<(u64, Scale)>,
    foreign: &mut usize,
    key: &MeasureKey,
) {
    if *current != Some((key.theme_rev, key.scale)) {
        *foreign = foreign.saturating_sub(1);
    }
    forget(nodes, marked, &key.node);
}

/// One entry of `node` left the cache.
fn forget(nodes: &mut CacheMap<String, NodeEntries>, marked: &mut usize, node: &str) {
    let entries = nodes.get_mut(node);
    debug_assert!(
        entries.is_some(),
        "an entry left for {node}, which held none"
    );
    let Some(entries) = entries else {
        return;
    };
    entries.held -= 1;
    if entries.held == 0 {
        if entries.invalidated != 0 {
            *marked -= 1;
        }
        nodes.remove(node);
    }
}

#[cfg(test)]
mod tests {
    use super::{ChangeSet, MeasureCache, MeasureKey, Proposal, SizeProposal};
    use crate::geom::{Axis, Scale, Size};
    use std::collections::BTreeSet;

    fn key(node: &str, proposal: SizeProposal) -> MeasureKey {
        MeasureKey {
            node: node.into(),
            proposal,
            theme_rev: 1,
            scale: Scale::ONE,
        }
    }

    #[test]
    fn open_probes_have_no_available_extent() {
        assert_eq!(Proposal::Exact(10.0).available(), Some(10.0));
        assert_eq!(Proposal::Zero.available(), Some(0.0));
        assert_eq!(Proposal::Unbounded.available(), None);
        assert_eq!(Proposal::Unspecified.available(), None);
    }

    #[test]
    fn shrink_only_touches_exact_offers() {
        assert_eq!(Proposal::Exact(10.0).shrink(4.0), Proposal::Exact(6.0));
        assert_eq!(Proposal::Exact(2.0).shrink(4.0), Proposal::Exact(0.0));
        assert_eq!(Proposal::Unbounded.shrink(4.0), Proposal::Unbounded);
    }

    #[test]
    fn axis_accessors_agree() {
        let p = SizeProposal::zero().with_axis(Axis::Vertical, Proposal::Unbounded);
        assert_eq!(p.axis(Axis::Horizontal), Proposal::Zero);
        assert_eq!(p.axis(Axis::Vertical), Proposal::Unbounded);
    }

    #[test]
    fn the_cache_answers_a_repeat_probe_without_recomputing() {
        let mut cache = MeasureCache::new();
        let mut calls = 0;
        let k = key("/a", SizeProposal::zero());
        for _ in 0..3 {
            cache.get_or_insert_with(k.clone(), || {
                calls += 1;
                Size::new(4.0, 2.0)
            });
        }
        assert_eq!(calls, 1);
        assert_eq!(cache.stats(), (2, 1));
    }

    /// Two probes of the same node are two entries. The multi-probe pattern
    /// depends on this: a cache keyed only by node would answer `Unbounded`
    /// with the `Zero` response.
    #[test]
    fn distinct_proposals_are_distinct_entries() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/a", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.get_or_insert_with(key("/a", SizeProposal::unbounded()), || Size::new(9.0, 1.0));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.stats(), (0, 2));
    }

    #[test]
    fn a_theme_change_drops_the_cache() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/a", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.retain_theme_and_scale(2, Scale::ONE);
        assert!(cache.is_empty());
    }

    #[test]
    fn a_scale_change_drops_the_cache() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/a", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.retain_theme_and_scale(1, Scale::new(1.25).unwrap());
        assert!(cache.is_empty());
    }

    #[test]
    fn per_node_invalidation_leaves_siblings_alone() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/a", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.get_or_insert_with(key("/b", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.invalidate_node("/a");
        assert_eq!(cache.len(), 1);
    }

    /// An entry stored again after its node was invalidated answers: the
    /// mark only reaches what was stored before it.
    #[test]
    fn a_node_measured_again_after_invalidation_hits() {
        let mut cache = MeasureCache::new();
        let probe = key("/a", SizeProposal::zero());
        cache.insert(probe.clone(), Size::new(1.0, 1.0));
        cache.invalidate_node("/a");
        assert_eq!(cache.peek(&probe), None, "the stale entry answered");
        cache.insert(probe.clone(), Size::new(2.0, 2.0));
        assert_eq!(cache.peek(&probe), Some(Size::new(2.0, 2.0)));
    }

    /// The node table agrees with the entries: one row per node with an entry
    /// held, its count exact, the mark tally exact, and the foreign tally
    /// exact. An eviction that skipped the table would leave a node counted
    /// that holds nothing, and its mark would outlive every entry it guarded.
    fn check_nodes(cache: &MeasureCache) {
        let mut held: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        let mut foreign = 0;
        for (key, _) in cache.entries.iter() {
            *held.entry(key.node.as_str()).or_default() += 1;
            if cache.current != Some((key.theme_rev, key.scale)) {
                foreign += 1;
            }
        }
        assert_eq!(
            held.len(),
            cache.nodes.len(),
            "a node row without entries, or the reverse"
        );
        for (node, count) in &held {
            assert_eq!(cache.nodes[*node].held, *count, "{node}: held count is off");
        }
        let marked = cache.nodes.values().filter(|n| n.invalidated != 0).count();
        assert_eq!(cache.marked, marked, "the mark tally is off");
        assert_eq!(cache.foreign, foreign, "the foreign tally is off");
    }

    /// A long scroll invalidates far more distinct ids than the cache holds.
    /// A mark lives only as long as an entry of its node, so the marks stay
    /// inside the entries, and the live entry keeps answering.
    #[test]
    fn invalidation_marks_stay_inside_the_entries() {
        let mut cache = MeasureCache::with_capacity(8);
        let live = key("/live", SizeProposal::zero());
        cache.insert(live.clone(), Size::new(1.0, 1.0));
        for n in 0..100 {
            let id = format!("/row-{n}");
            cache.insert(key(&id, SizeProposal::zero()), Size::new(1.0, 1.0));
            cache.invalidate_node(&id);
            // Touched every round, so the LRU keeps it newest.
            assert_eq!(cache.peek(&live), Some(Size::new(1.0, 1.0)));
            assert!(
                cache.nodes.len() <= cache.capacity(),
                "{} node rows over an {}-entry bound",
                cache.nodes.len(),
                cache.capacity()
            );
            check_nodes(&cache);
        }
    }

    /// Most of what a scroll names has never been measured: the rows it
    /// brings in. Invalidating those costs a lookup and records nothing.
    #[test]
    fn an_id_with_nothing_cached_leaves_no_mark() {
        let mut cache = MeasureCache::with_capacity(8);
        let held = key("/held", SizeProposal::zero());
        cache.insert(held.clone(), Size::new(1.0, 1.0));
        for n in 0..10_000 {
            cache.invalidate_node(&format!("/row-{n}"));
        }
        assert_eq!(cache.nodes.len(), 1);
        assert_eq!(cache.marked, 0);
        assert_eq!(cache.peek(&held), Some(Size::new(1.0, 1.0)));
        check_nodes(&cache);
    }

    /// Filling the cache and churning through it, the way a long scroll does,
    /// never reallocates the node table either (`crate::cache`'s module
    /// documentation has the measurement).
    #[test]
    fn filling_and_churning_never_reallocates_the_node_table() {
        const BOUND: usize = 1_024;
        let mut cache = MeasureCache::with_capacity(BOUND);
        cache.retain_theme_and_scale(1, Scale::ONE);
        cache.insert(key("/row-0", SizeProposal::zero()), Size::new(1.0, 1.0));
        let table = cache.nodes.capacity();
        assert!(
            table >= 2 * BOUND,
            "the node table holds twice the bound, got {table}"
        );
        for pass in 0..32 {
            cache.begin_pass();
            for n in 0..BOUND / 4 {
                let id = format!("/row-{}", pass * BOUND + n + 1);
                cache.insert(key(&id, SizeProposal::zero()), Size::new(1.0, 1.0));
                cache.insert(key(&id, SizeProposal::unbounded()), Size::new(1.0, 1.0));
                cache.invalidate_node(&format!("/row-{}", pass * BOUND + n));
            }
            let room = cache.nodes.capacity();
            assert!(room <= table, "pass {pass}: the node table grew to {room}");
            assert!(
                room > cache.nodes.len(),
                "pass {pass}: the node table will rehash"
            );
        }
        check_nodes(&cache);
    }

    /// The per-frame theme check skips the walk only when nothing could be
    /// dropped. An entry stored under another pair since the last call still
    /// goes.
    #[test]
    fn a_stray_entry_under_another_theme_is_still_dropped() {
        let mut cache = MeasureCache::new();
        cache.insert(key("/a", SizeProposal::zero()), Size::new(1.0, 1.0));
        cache.retain_theme_and_scale(1, Scale::ONE);
        assert_eq!(cache.len(), 1);
        let mut stray = key("/b", SizeProposal::zero());
        stray.theme_rev = 2;
        cache.insert(stray.clone(), Size::new(1.0, 1.0));
        check_nodes(&cache);
        cache.retain_theme_and_scale(1, Scale::ONE);
        assert_eq!(cache.peek(&stray), None);
        assert_eq!(cache.len(), 1);
        check_nodes(&cache);
    }

    #[test]
    fn apply_all_clears_everything() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/a", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.get_or_insert_with(key("/b", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.apply(&ChangeSet::All);
        assert!(cache.is_empty());
    }

    #[test]
    fn apply_none_touches_nothing() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/a", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.apply(&ChangeSet::None);
        assert_eq!(cache.len(), 1);
    }

    /// The whole point: naming only the leaf still invalidates its ancestors,
    /// because the crate walks them — a host cannot forget the loop the bench
    /// (`Policy::LeafOnly`) shows is required.
    #[test]
    fn apply_nodes_invalidates_the_named_id_and_every_ancestor() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/root/panel-0/row-0", SizeProposal::zero()), || {
            Size::new(1.0, 1.0)
        });
        cache.get_or_insert_with(key("/root/panel-0", SizeProposal::zero()), || {
            Size::new(2.0, 2.0)
        });
        cache.get_or_insert_with(key("/root", SizeProposal::zero()), || Size::new(3.0, 3.0));
        cache.get_or_insert_with(key("/root/panel-1", SizeProposal::zero()), || {
            Size::new(4.0, 4.0)
        });

        cache.apply(&ChangeSet::node("/root/panel-0/row-0"));

        assert_eq!(cache.len(), 1, "only the untouched sibling survives");
        let (_, misses) = cache.stats();
        assert_eq!(misses, 4, "the pre-invalidation probes were all misses");
        // The sibling panel is still cached: re-probing it must not miss.
        let hits_before = cache.stats().0;
        cache.get_or_insert_with(key("/root/panel-1", SizeProposal::zero()), || {
            panic!("panel-1 should still be cached")
        });
        assert_eq!(cache.stats().0, hits_before + 1);
    }

    /// A malformed id — no leading `/`, or empty — is invalidated on its own
    /// (it has no derivable ancestors) and must not panic.
    #[test]
    fn apply_nodes_handles_a_malformed_id_without_panicking() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/a", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.apply(&ChangeSet::Nodes(BTreeSet::from([
            String::new(),
            "no-leading-slash".to_owned(),
        ])));
        // Neither malformed id names "/a", so it survives untouched.
        assert_eq!(cache.len(), 1);
    }

    /// A top-level id (one segment) has no ancestors, so naming it
    /// invalidates only itself.
    #[test]
    fn apply_nodes_on_the_top_level_id_invalidates_only_itself() {
        let mut cache = MeasureCache::new();
        cache.get_or_insert_with(key("/root", SizeProposal::zero()), || Size::new(1.0, 1.0));
        cache.get_or_insert_with(key("/root/child", SizeProposal::zero()), || {
            Size::new(2.0, 2.0)
        });
        cache.apply(&ChangeSet::node("/root"));
        assert_eq!(cache.len(), 1);
        let hits_before = cache.stats().0;
        cache.get_or_insert_with(key("/root/child", SizeProposal::zero()), || {
            panic!("the child was not named and has no ancestor relationship to /root")
        });
        assert_eq!(cache.stats().0, hits_before + 1);
    }

    /// A cached response is a digest input, so it passes through the same
    /// sanity clamp every measurement does.
    #[test]
    fn cached_responses_are_sane() {
        let mut cache = MeasureCache::new();
        let got = cache.get_or_insert_with(key("/a", SizeProposal::zero()), || {
            Size::new(f32::NAN, -1.0)
        });
        assert_eq!(got, Size::ZERO);
    }
}
