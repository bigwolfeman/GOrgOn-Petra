//! A capacity-bounded LRU map, shared by every memo the engine and its hosts
//! keep.
//!
//! Petra has two caches with the same shape of problem: the measurement cache
//! ([`crate::layout::MeasureCache`]) and the host's shaped-galley cache. Both
//! are keyed per node or per string, so both grow with *everything the user
//! ever scrolled past* rather than with what is on screen — which is the one
//! thing SC-008 forbids ("memory that does not grow with total row count").
//! Both therefore need the same eviction, and one implementation of it is the
//! only way the two do not drift into two different definitions of "bounded".
//!
//! The policy is least-recently-used with a hard entry cap, not a per-frame
//! sweep. Two access patterns have to survive it:
//!
//! * *Within* a frame a parent probes one child with several proposals before
//!   it commits one, which is the reason the measurement cache exists at all.
//!   LRU keeps every one of those entries: they are the most recently used
//!   things in the map.
//! * *Across* frames a scroll shifts the visible window by a row or two, so
//!   the overwhelming majority of the next frame's keys are the previous
//!   frame's keys. LRU keeps those too, and drops the rows that left the
//!   window — exactly the entries that will not be asked for again.
//!
//! A per-frame generation sweep bounds memory just as well but throws away the
//! cross-frame reuse: every frame would start cold, and a scroll would
//! re-measure and re-shape the whole window each time. That is why the cap is
//! on entries, not on age.
//!
//! # The live set is never evicted
//!
//! A plain entry cap has a cliff. When one frame asks for more distinct keys
//! than the cap, LRU evicts each key shortly before the same frame (or the
//! next one) asks for it again, so nearly every lookup misses, frame after
//! frame. Measured on the GOrgOn shell (2026-11-22): one frame asked the
//! galley cache for 2 072 distinct keys against a cap of 2 048, and one frame
//! re-shaped 1 578 galleys and took 3.5 times as long as the frame before it.
//! Thirty more keys turned a full hit rate into a full miss rate.
//!
//! So an owner that knows where its frames start calls
//! [`LruCache::begin_pass`] once per frame, and from then on an entry touched
//! in the current pass or the pass before it is *live* and is never evicted.
//! The cap then bounds only the entries kept for cross-frame reuse beyond the
//! live set: the cache holds at most `max(capacity, live set)` entries, and
//! the live set is what two consecutive frames asked for, which grows with
//! what is on screen and not with what was ever scrolled past. An owner that
//! never calls `begin_pass` keeps the plain hard cap.
//!
//! Keys are held once, behind an [`Arc`], because the eviction list and the
//! lookup index both need to name an entry and a galley key owns a whole text
//! run. Cloning the key into both would double the largest allocation in the
//! map.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

/// One live entry, plus its neighbours in the recency list.
#[derive(Debug)]
struct Entry<K, V> {
    key: Arc<K>,
    value: V,
    /// The next entry towards the least-recently-used end.
    older: Option<usize>,
    /// The next entry towards the most-recently-used end.
    newer: Option<usize>,
    /// The pass ([`LruCache::begin_pass`]) that last touched this entry.
    used: u64,
}

/// A map that holds at most `capacity` entries, dropping the least recently
/// used one to make room, except entries still live in the current or the
/// previous pass once the owner marks passes ([`LruCache::begin_pass`]).
///
/// Lookup, insertion, and eviction are all O(1): the recency order is an
/// intrusive doubly linked list over a slab, so touching an entry moves two
/// indices instead of shifting a queue.
#[derive(Debug)]
pub struct LruCache<K: Eq + Hash, V> {
    index: HashMap<Arc<K>, usize>,
    slots: Vec<Option<Entry<K, V>>>,
    free: Vec<usize>,
    newest: Option<usize>,
    oldest: Option<usize>,
    capacity: usize,
    evictions: u64,
    /// The current pass, or 0 for an owner that never marked one (plain hard
    /// cap). Recency order and `Entry::used` agree: walking from the oldest
    /// entry towards the newest, `used` never decreases, because a touch moves
    /// an entry to the newest end and stamps it with the current pass.
    pass: u64,
}

impl<K: Eq + Hash, V> LruCache<K, V> {
    /// A cache bounded to `capacity` entries.
    ///
    /// A capacity of zero is raised to one: a cache that can hold nothing
    /// would answer every probe with a miss while still looking like a cache,
    /// and the caller that asked for it would never find out.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            index: HashMap::new(),
            slots: Vec::new(),
            free: Vec::new(),
            newest: None,
            oldest: None,
            capacity: capacity.max(1),
            evictions: 0,
            pass: 0,
        }
    }

    /// The entry bound in force.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Change the bound, evicting immediately if the new one is smaller.
    /// Live entries ([`LruCache::begin_pass`]) stay until they stop being
    /// live.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.max(1);
        self.trim();
    }

    /// Mark the start of a new pass (one frame of the owner's work).
    ///
    /// Entries touched in this pass or the one before it are live and are
    /// never evicted; the cap applies to the rest. Entries that stopped being
    /// live with this call are trimmed now, oldest first, down to the cap.
    /// See the module documentation for the cliff this removes.
    pub fn begin_pass(&mut self) {
        self.pass += 1;
        self.trim();
    }

    /// Whether `slot` is live: touched in the current or the previous pass of
    /// an owner that marks passes.
    fn live(&self, slot: usize) -> bool {
        self.pass != 0
            && self.slots[slot]
                .as_ref()
                .expect("probed slot is live")
                .used
                .saturating_add(1)
                >= self.pass
    }

    /// Evict the oldest entries until the cache is inside the cap or the
    /// oldest entry is live. Recency order means every entry newer than a live
    /// one is live too, so stopping at the first is exact.
    fn trim(&mut self) {
        while self.len() > self.capacity
            && self.oldest.is_some_and(|slot| !self.live(slot))
            && self.evict_oldest()
        {}
    }

    /// Entries currently held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Whether the cache holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Entries dropped to stay inside the bound since construction.
    ///
    /// Reported rather than kept private: a host whose per-frame working set
    /// is larger than the bound is thrashing, and this is the number that says
    /// so. A cache that hid it would look identical to one that fits.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    /// The value for `key`, marked as most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let slot = *self.index.get(key)?;
        self.touch(slot);
        Some(
            &self.slots[slot]
                .as_ref()
                .expect("indexed slot is live")
                .value,
        )
    }

    /// Store `value` under `key`, evicting the least recently used entry if
    /// the cache is full. An existing key keeps its slot and is refreshed.
    pub fn insert(&mut self, key: K, value: V) {
        if let Some(&slot) = self.index.get(&key) {
            self.slots[slot]
                .as_mut()
                .expect("indexed slot is live")
                .value = value;
            self.touch(slot);
            return;
        }
        while self.len() >= self.capacity
            && self.oldest.is_some_and(|slot| !self.live(slot))
            && self.evict_oldest()
        {}
        let key = Arc::new(key);
        let entry = Entry {
            key: Arc::clone(&key),
            value,
            older: None,
            newer: None,
            used: self.pass,
        };
        let slot = match self.free.pop() {
            Some(slot) => {
                self.slots[slot] = Some(entry);
                slot
            }
            None => {
                self.slots.push(Some(entry));
                self.slots.len() - 1
            }
        };
        self.index.insert(key, slot);
        self.link_newest(slot);
    }

    /// Keep only the entries `predicate` accepts. Recency order is preserved
    /// for the survivors.
    pub fn retain<F: FnMut(&K, &V) -> bool>(&mut self, mut predicate: F) {
        let doomed: Vec<usize> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, held)| {
                let entry = held.as_ref()?;
                (!predicate(&entry.key, &entry.value)).then_some(slot)
            })
            .collect();
        for slot in doomed {
            self.remove_slot(slot);
        }
    }

    /// Drop every entry. The capacity and the eviction count survive.
    pub fn clear(&mut self) {
        self.index.clear();
        self.slots.clear();
        self.free.clear();
        self.newest = None;
        self.oldest = None;
    }

    /// Move `slot` to the most-recently-used end and stamp it with the
    /// current pass.
    fn touch(&mut self, slot: usize) {
        self.slots[slot]
            .as_mut()
            .expect("touched slot is live")
            .used = self.pass;
        if self.newest == Some(slot) {
            return;
        }
        self.unlink(slot);
        self.link_newest(slot);
    }

    /// Drop the least recently used entry. `false` when there was none.
    fn evict_oldest(&mut self) -> bool {
        let Some(slot) = self.oldest else {
            return false;
        };
        self.remove_slot(slot);
        self.evictions += 1;
        true
    }

    /// Unlink `slot`, forget its key, and return the slot to the free list.
    fn remove_slot(&mut self, slot: usize) {
        self.unlink(slot);
        let entry = self.slots[slot].take().expect("removed slot is live");
        self.index.remove(entry.key.as_ref());
        self.free.push(slot);
    }

    /// Detach `slot` from the recency list, repairing both neighbours.
    fn unlink(&mut self, slot: usize) {
        let (older, newer) = {
            let entry = self.slots[slot].as_mut().expect("unlinked slot is live");
            let pair = (entry.older, entry.newer);
            entry.older = None;
            entry.newer = None;
            pair
        };
        match newer {
            Some(n) => self.slots[n].as_mut().expect("neighbour is live").older = older,
            None => self.newest = older,
        }
        match older {
            Some(o) => self.slots[o].as_mut().expect("neighbour is live").newer = newer,
            None => self.oldest = newer,
        }
    }

    /// Attach `slot` at the most-recently-used end. It must be unlinked.
    fn link_newest(&mut self, slot: usize) {
        let previous = self.newest;
        {
            let entry = self.slots[slot].as_mut().expect("linked slot is live");
            entry.older = previous;
            entry.newer = None;
            entry.used = self.pass;
        }
        if let Some(p) = previous {
            self.slots[p].as_mut().expect("neighbour is live").newer = Some(slot);
        }
        self.newest = Some(slot);
        if self.oldest.is_none() {
            self.oldest = Some(slot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LruCache;
    use std::fmt::Debug;
    use std::hash::Hash;

    /// The recency list must stay a well-formed doubly linked list over
    /// exactly the live slots. If it does not, an eviction walks off a stale
    /// index and the cache loses entries it still counts.
    fn check_invariants<K: Eq + Hash + Debug, V>(cache: &LruCache<K, V>) {
        let live = cache.slots.iter().filter(|s| s.is_some()).count();
        assert_eq!(live, cache.index.len(), "index and slab disagree on size");
        assert!(
            cache.index.len() <= cache.capacity || cache.oldest.is_some_and(|s| cache.live(s)),
            "over capacity with an evictable entry still held"
        );

        let mut walked = 0;
        let mut cursor = cache.newest;
        let mut previous = None;
        let mut newer_used = u64::MAX;
        while let Some(slot) = cursor {
            let entry = cache.slots[slot].as_ref().expect("linked slot is live");
            assert_eq!(entry.newer, previous, "back link is wrong at {slot}");
            assert!(
                entry.used <= newer_used,
                "recency and pass stamps disagree at {slot}"
            );
            newer_used = entry.used;
            previous = Some(slot);
            cursor = entry.older;
            walked += 1;
            assert!(walked <= live, "recency list is cyclic");
        }
        assert_eq!(walked, live, "recency list misses live entries");
        assert_eq!(cache.oldest, previous, "oldest is not the list tail");
    }

    #[test]
    fn a_full_cache_drops_the_least_recently_used_entry() {
        let mut cache = LruCache::with_capacity(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        // Touching "a" makes "b" the eviction candidate.
        assert_eq!(cache.get(&"a"), Some(&1));
        cache.insert("c", 3);
        check_invariants(&cache);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"c"), Some(&3));
        assert_eq!(cache.get(&"b"), None, "the untouched entry is the one lost");
        assert_eq!(cache.evictions(), 1);
    }

    /// The multi-probe pattern the measurement cache exists for: several
    /// probes of one hot key inside a frame while cold keys stream past. Every
    /// hot probe must still hit.
    #[test]
    fn a_hot_key_survives_a_stream_of_cold_ones() {
        let mut cache = LruCache::with_capacity(8);
        for round in 0..100 {
            cache.insert(("hot", 0), round);
            assert_eq!(cache.get(&("hot", 0)), Some(&round));
            for cold in 0..4 {
                cache.insert(("cold", round * 4 + cold), cold);
            }
            assert_eq!(cache.get(&("hot", 0)), Some(&round), "round {round}");
        }
        check_invariants(&cache);
        assert_eq!(cache.len(), 8);
    }

    #[test]
    fn reinserting_a_key_replaces_rather_than_grows() {
        let mut cache = LruCache::with_capacity(4);
        cache.insert("a", 1);
        cache.insert("a", 2);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&"a"), Some(&2));
        assert_eq!(cache.evictions(), 0);
        check_invariants(&cache);
    }

    #[test]
    fn retain_drops_the_rejected_entries_and_keeps_the_list_walkable() {
        let mut cache = LruCache::with_capacity(16);
        for i in 0..10 {
            cache.insert(i, i * 10);
        }
        cache.retain(|k, _| k % 2 == 0);
        check_invariants(&cache);
        assert_eq!(cache.len(), 5);
        assert_eq!(cache.get(&4), Some(&40));
        assert_eq!(cache.get(&5), None);
        // Freed slots are reused rather than leaked.
        for i in 100..105 {
            cache.insert(i, i);
        }
        check_invariants(&cache);
        assert_eq!(cache.len(), 10);
        assert_eq!(cache.slots.len(), 10, "the slab reuses freed slots");
    }

    #[test]
    fn shrinking_the_capacity_evicts_immediately() {
        let mut cache = LruCache::with_capacity(10);
        for i in 0..10 {
            cache.insert(i, i);
        }
        cache.set_capacity(3);
        check_invariants(&cache);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&9), Some(&9), "the newest entries are kept");
        assert_eq!(cache.get(&0), None);
        assert_eq!(cache.evictions(), 7);
    }

    #[test]
    fn a_zero_capacity_still_holds_one_entry_rather_than_pretending() {
        let mut cache = LruCache::with_capacity(0);
        assert_eq!(cache.capacity(), 1);
        cache.insert("a", 1);
        assert_eq!(cache.get(&"a"), Some(&1));
        check_invariants(&cache);
    }

    #[test]
    fn clearing_empties_the_list_and_the_index() {
        let mut cache = LruCache::with_capacity(4);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.clear();
        check_invariants(&cache);
        assert!(cache.is_empty());
        cache.insert("c", 3);
        check_invariants(&cache);
        assert_eq!(cache.get(&"c"), Some(&3));
    }

    /// Look `key` up and insert it on a miss, the way both owners use the
    /// cache. `true` on a miss.
    fn probe(cache: &mut LruCache<u32, u32>, key: u32) -> bool {
        if cache.get(&key).is_some() {
            return false;
        }
        cache.insert(key, key);
        true
    }

    /// The cliff: a frame that asks for a few more keys than the cap misses
    /// on nearly every key, every frame, under a plain cap. With passes
    /// marked, the same frames miss only on the first.
    #[test]
    fn a_working_set_over_the_cap_hits_every_frame_once_passes_are_marked() {
        let frame: Vec<u32> = (0..10).collect();

        let mut plain = LruCache::with_capacity(8);
        let plain_misses: Vec<usize> = (0..4)
            .map(|_| frame.iter().filter(|&&k| probe(&mut plain, k)).count())
            .collect();
        assert_eq!(
            plain_misses,
            [10, 10, 10, 10],
            "a plain cap thrashes on a cyclic working set over the cap"
        );

        let mut marked = LruCache::with_capacity(8);
        let marked_misses: Vec<usize> = (0..4)
            .map(|_| {
                marked.begin_pass();
                let misses = frame.iter().filter(|&&k| probe(&mut marked, k)).count();
                check_invariants(&marked);
                misses
            })
            .collect();
        assert_eq!(
            marked_misses,
            [10, 0, 0, 0],
            "the live set must survive its own frame and the next"
        );
        assert_eq!(marked.len(), 10, "the cache holds the live set, no more");
        assert_eq!(marked.evictions(), 0);
    }

    /// Entries that stop being live go back under the cap, oldest first, at
    /// the next pass boundary.
    #[test]
    fn entries_that_leave_the_live_set_are_trimmed_to_the_cap() {
        let mut cache = LruCache::with_capacity(8);
        cache.begin_pass();
        for k in 0..20 {
            probe(&mut cache, k);
        }
        assert_eq!(cache.len(), 20, "one pass's working set is live");
        cache.begin_pass();
        for k in 100..105 {
            probe(&mut cache, k);
        }
        check_invariants(&cache);
        assert_eq!(cache.len(), 25, "the previous pass is still live");
        cache.begin_pass();
        check_invariants(&cache);
        assert_eq!(cache.len(), 8, "stale entries are trimmed to the cap");
        for k in 100..105 {
            assert!(cache.get(&k).is_some(), "live key {k} was trimmed");
        }
        for k in 17..20 {
            assert!(
                cache.get(&k).is_some(),
                "the newest stale keys fill the cap"
            );
        }
        assert!(cache.get(&16).is_none());
    }

    /// A scroll past many distinct keys with passes marked stays inside the
    /// cap plus two passes' working set. Memory does not grow with the total.
    #[test]
    fn a_long_marked_run_is_bounded_by_the_cap_and_the_live_set() {
        let mut cache = LruCache::with_capacity(32);
        for pass in 0..1_000u32 {
            cache.begin_pass();
            for k in 0..10 {
                probe(&mut cache, pass * 10 + k);
            }
            assert!(cache.len() <= 32, "pass {pass}: {} entries", cache.len());
        }
        check_invariants(&cache);
        assert_eq!(cache.evictions(), 10_000 - 32);
    }

    /// Eviction order follows use, not insertion, over a long run.
    #[test]
    fn a_long_run_keeps_exactly_the_last_capacity_keys() {
        let mut cache = LruCache::with_capacity(32);
        for i in 0..10_000 {
            cache.insert(i, i);
        }
        check_invariants(&cache);
        assert_eq!(cache.len(), 32);
        for i in 9_968..10_000 {
            assert_eq!(cache.get(&i), Some(&i), "key {i} should still be resident");
        }
        assert_eq!(cache.get(&9_967), None);
        assert_eq!(cache.evictions(), 10_000 - 32);
    }
}

// No Kani harness lives here, and the reason is a hard tool limit rather than
// a gap. A capacity-2 `LruCache<u8, u8>` driven through 3 symbolic
// insert/get steps, then checked for the same structural invariants
// `check_invariants` asserts by hand, was written and run: it did not
// terminate inside 600 s. The cost is not the trace length. `LruCache`'s
// index is a `std::HashMap`, and CBMC cannot get through hashbrown's SIMD
// group scan (`RawTableInner::find_insert_slot` reaching
// `simd_bitmask_impl::<i8, 16>`). A minimal reproduction — one
// `HashSet<u8>` insert plus one lookup, nothing else — reached 412 498
// variables and 1 153 452 clauses with no verdict in 300 s, while the same
// shape over a `BTreeSet` verified in 1.19 s. So this is a property of the
// hasher, not of this module, and no amount of shrinking the bound here
// would recover it. See
// `.agents/notes/proposed/testing/2026-08-30-kani-bounded-verification.md`.
