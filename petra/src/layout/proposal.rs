//! The proposal/response vocabulary and the measurement cache.
//!
//! Four proposal values, per axis (`contracts/view-tree.md`). A parent may
//! probe a child with several of them before it commits one; the cache is what
//! makes that affordable, which is why it is part of the vocabulary module
//! rather than an optimisation bolted on later.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use crate::cache::LruCache;
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
/// (`.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`,
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
/// entries of the frame in flight and the previous frame's window.
#[derive(Debug)]
pub struct MeasureCache {
    entries: LruCache<MeasureKey, Size>,
    hits: u64,
    misses: u64,
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
    /// A full cache is on the order of ten megabytes. That is a bound on
    /// *entries*, not on bytes — nothing here instruments the allocator, and a
    /// host that cares reads [`MeasureCache::evictions`], which is non-zero
    /// exactly when the bound is costing it work. A host with a denser tree
    /// still raises it with [`MeasureCache::with_capacity`].
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
        self.entries.set_capacity(capacity);
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
        match self.entries.get(key).copied() {
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
        self.entries.insert(key, value);
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
    pub fn retain_theme_and_scale(&mut self, theme_rev: u64, scale: Scale) {
        self.entries
            .retain(|key, _| key.theme_rev == theme_rev && key.scale == scale);
    }

    /// Drop every entry for one node. Does not touch its ancestors or its
    /// descendants — [`MeasureCache::apply`] is what a host wants for a
    /// content change; this is the primitive it is built from.
    pub fn invalidate_node(&mut self, node: &str) {
        self.entries.retain(|key, _| key.node != node);
    }

    /// Drop everything.
    pub fn clear(&mut self) {
        self.entries.clear();
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
                for id in ids {
                    self.invalidate_node(id);
                    for ancestor in KeyPath::ancestor_ids(id) {
                        self.invalidate_node(&ancestor);
                    }
                }
            }
        }
    }

    /// Entries currently held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Hit and miss counts since construction. The bench and the multi-probe
    /// claim in FR-035 are measured from these.
    #[must_use]
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
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
