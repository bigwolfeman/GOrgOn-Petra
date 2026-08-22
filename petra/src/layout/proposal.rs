//! The proposal/response vocabulary and the measurement cache.
//!
//! Four proposal values, per axis (`contracts/view-tree.md`). A parent may
//! probe a child with several of them before it commits one; the cache is what
//! makes that affordable, which is why it is part of the vocabulary module
//! rather than an optimisation bolted on later.

use std::hash::{Hash, Hasher};

use crate::cache::LruCache;
use crate::geom::{Axis, Scale, Size};

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
/// The four revisions are the whole invalidation story. Adding a dependency to
/// a measurement without adding it here is how a layout engine starts
/// answering last frame's question, so the key is deliberately explicit rather
/// than a hash of "some context".
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MeasureKey {
    /// Canonical id of the node being measured.
    pub node: String,
    /// The offer.
    pub proposal: SizeProposal,
    /// Revision of the content this node reads.
    pub content_rev: u64,
    /// Revision of the theme snapshot in force.
    pub theme_rev: u64,
    /// Display scale.
    pub scale: Scale,
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
    /// the pass probes, which for a virtualized tree is the visible window
    /// plus overscan times the probes each parent makes — tens to low
    /// hundreds, not thousands. The default is far above that so the bound
    /// never costs a hit in an ordinary frame, and small enough that a full
    /// cache is on the order of a megabyte. A host with a denser tree raises
    /// it with [`MeasureCache::with_capacity`]; a host that wants to know
    /// whether it needs to reads [`MeasureCache::evictions`].
    pub const DEFAULT_CAPACITY: usize = 8192;

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
    /// Content revisions are per node and handled by the key; theme and scale
    /// are global, so a change there invalidates wholesale.
    pub fn retain_theme_and_scale(&mut self, theme_rev: u64, scale: Scale) {
        self.entries
            .retain(|key, _| key.theme_rev == theme_rev && key.scale == scale);
    }

    /// Drop every entry for one node.
    pub fn invalidate_node(&mut self, node: &str) {
        self.entries.retain(|key, _| key.node != node);
    }

    /// Drop everything.
    pub fn clear(&mut self) {
        self.entries.clear();
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
    use super::{MeasureCache, MeasureKey, Proposal, SizeProposal};
    use crate::geom::{Axis, Scale, Size};

    fn key(node: &str, proposal: SizeProposal) -> MeasureKey {
        MeasureKey {
            node: node.into(),
            proposal,
            content_rev: 1,
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
