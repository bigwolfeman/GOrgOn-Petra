//! Measured row extents for virtualized collections, remembered across
//! frames.
//!
//! A `collection` materializes only its window, so the extent of a row it has
//! never placed is the declared `estimated_extent`. Before this ledger the
//! estimate was also the answer for rows it *had* placed: the content extent
//! was `total_count × estimate`, and the window's first row sat on that
//! uniform grid. Rows taller than the estimate made the end of a list
//! unreachable, and a wheel step that moved the window's first row jumped
//! every visible row by that row's measure less the estimate.
//!
//! [`RowExtents`] keeps what each placed row measured, keyed by the
//! collection and the row's index, and checked against the row's own key so a
//! measurement is only reused for the row it was taken from. A row's leading
//! edge is then the sum of the extents before it: measured where known,
//! estimated elsewhere. Content extent, the window's first row and a reveal's
//! target span all read that one sum.
//!
//! # Anchoring
//!
//! A new measurement above the first visible row moves every row below it.
//! [`crate::layout::scroll::place_collection`] keeps the visible rows still:
//! it places the first visible row where the ledger said it was at the start
//! of the pass, lays the rows above it upward from there, and records the
//! distance the ledger moved that row as a pending shift for the enclosing
//! scroll ([`RowExtents::take_shifts`]). The host adds the shift to the
//! scroll's offset before the next frame, and the row stays where the eye
//! was. Every ledger change takes this one path: a first measurement, a
//! re-measurement, an eviction, and a reset after the cross extent or the
//! theme changed.
//!
//! # Cost
//!
//! A row that measures exactly its estimate stores nothing. A row that does
//! not costs one map entry, and a list keeps at most [`ROW_CAP`] of them,
//! evicting the one farthest from the window, so a long scroll does not
//! accumulate memory (SC-008). Prefix sums run on a Fenwick tree over chunks
//! of [`CHUNK`] rows: `total_count / CHUNK` numbers, so a 100 000-row list
//! holds 98 of them, and a sum costs `log(chunks)` plus a walk over the
//! measured rows of one chunk.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::geom::Scale;

/// Rows per prefix-sum chunk.
pub const CHUNK: usize = 1024;

/// The most measured rows one list remembers. Past it the measurement
/// farthest from the window is dropped, and the row goes back to its
/// estimate.
pub const ROW_CAP: usize = 4096;

/// What a list's measurements are only valid under: rows measured at another
/// cross extent, theme or scale would measure differently now.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Basis {
    estimate: f32,
    cross: f32,
    theme_rev: u64,
    scale: f32,
}

#[derive(Clone, Copy, Debug)]
struct Seen {
    identity: u64,
    extent: f32,
}

/// One collection's measured rows.
#[derive(Clone, Debug)]
pub struct ListExtents {
    basis: Basis,
    count: usize,
    rows: BTreeMap<usize, Seen>,
    /// Fenwick tree over each chunk's sum of `extent - estimate`, 1-indexed.
    /// Empty until a row measures off its estimate.
    chunks: Vec<f64>,
}

impl ListExtents {
    fn new(basis: Basis, count: usize) -> Self {
        Self {
            basis,
            count,
            rows: BTreeMap::new(),
            chunks: Vec::new(),
        }
    }

    /// Rows in the list.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// The extent a row is assumed to have until it is measured.
    #[must_use]
    pub fn estimate(&self) -> f32 {
        self.basis.estimate
    }

    /// Rows remembered with a measurement other than the estimate.
    #[must_use]
    pub fn measured(&self) -> usize {
        self.rows.len()
    }

    /// Row `index`'s extent: measured if remembered, else the estimate.
    #[must_use]
    pub fn extent(&self, index: usize) -> f32 {
        self.rows
            .get(&index)
            .map_or(self.basis.estimate, |seen| seen.extent)
    }

    /// Distance from the list's leading edge to row `index`'s.
    #[must_use]
    pub fn prefix(&self, index: usize) -> f32 {
        let index = index.min(self.count);
        let chunk = index / CHUNK;
        let est = f64::from(self.basis.estimate);
        let within: f64 = self
            .rows
            .range(chunk * CHUNK..index)
            .map(|(_, seen)| f64::from(seen.extent) - est)
            .sum();
        (index as f64 * est + self.chunk_prefix(chunk) + within) as f32
    }

    /// The whole list's extent.
    #[must_use]
    pub fn total(&self) -> f32 {
        self.prefix(self.count)
    }

    /// The row whose span holds `along`, clamped into the list: row 0 for
    /// anything at or before the leading edge, the last row for anything at
    /// or past the end. 0 for an empty list.
    #[must_use]
    pub fn index_at(&self, along: f32) -> usize {
        if self.count == 0 || along <= 0.0 {
            return 0;
        }
        let along = f64::from(along);
        let est = f64::from(self.basis.estimate);
        let chunk = self.chunk_at(along);
        let mut index = chunk * CHUNK;
        let end = ((chunk + 1) * CHUNK).min(self.count);
        let mut pos = index as f64 * est + self.chunk_prefix(chunk);
        for (&k, seen) in self.rows.range(index..end) {
            // The estimated rows before `k`.
            let gap = (k - index) as f64 * est;
            if pos + gap > along {
                return index + ((along - pos) / est).floor() as usize;
            }
            pos += gap;
            let extent = f64::from(seen.extent);
            if pos + extent > along {
                return k;
            }
            pos += extent;
            index = k + 1;
        }
        let rest = ((along - pos) / est).floor().max(0.0) as usize;
        (index + rest).min(self.count - 1)
    }

    /// Remember that row `index`, keyed `key`, measured `extent`. Rows past
    /// [`ROW_CAP`] are evicted farthest from `near` first. Answers whether
    /// any extent changed.
    fn record(&mut self, index: usize, key: &str, extent: f32, near: usize) -> bool {
        if index >= self.count || !extent.is_finite() {
            return false;
        }
        let was = self.extent(index);
        let identity = identity(key);
        if was == extent
            && self
                .rows
                .get(&index)
                .is_none_or(|seen| seen.identity == identity)
        {
            return false;
        }
        if extent == self.basis.estimate {
            self.rows.remove(&index);
        } else {
            self.rows.insert(index, Seen { identity, extent });
        }
        self.add(index, f64::from(extent) - f64::from(was));
        while self.rows.len() > ROW_CAP {
            let (&first, _) = self.rows.first_key_value().expect("over the cap");
            let (&last, _) = self.rows.last_key_value().expect("over the cap");
            let far = if near.abs_diff(first) >= near.abs_diff(last) {
                first
            } else {
                last
            };
            self.forget(far);
        }
        true
    }

    /// Drop a remembered row whose key is no longer the one at its index.
    /// Answers whether one was dropped.
    fn check(&mut self, index: usize, key: &str) -> bool {
        let stale = self
            .rows
            .get(&index)
            .is_some_and(|seen| seen.identity != identity(key));
        if stale {
            self.forget(index);
        }
        stale
    }

    fn forget(&mut self, index: usize) {
        if let Some(seen) = self.rows.remove(&index) {
            self.add(
                index,
                f64::from(self.basis.estimate) - f64::from(seen.extent),
            );
        }
    }

    /// Change the row count, dropping measurements past the new end.
    /// Answers whether the count changed.
    fn resize(&mut self, count: usize) -> bool {
        if count == self.count {
            return false;
        }
        self.count = count;
        self.rows.split_off(&count);
        self.rebuild();
        true
    }

    fn chunk_count(&self) -> usize {
        self.count.div_ceil(CHUNK).max(1)
    }

    fn rebuild(&mut self) {
        self.chunks.clear();
        if self.rows.is_empty() {
            return;
        }
        self.chunks = vec![0.0; self.chunk_count() + 1];
        let est = f64::from(self.basis.estimate);
        let entries: Vec<(usize, f64)> = self
            .rows
            .iter()
            .map(|(&k, seen)| (k, f64::from(seen.extent) - est))
            .collect();
        for (k, delta) in entries {
            self.fenwick_add(k / CHUNK, delta);
        }
    }

    fn add(&mut self, index: usize, delta: f64) {
        if delta == 0.0 {
            return;
        }
        if self.chunks.is_empty() {
            self.chunks = vec![0.0; self.chunk_count() + 1];
        }
        self.fenwick_add(index / CHUNK, delta);
    }

    fn fenwick_add(&mut self, chunk: usize, delta: f64) {
        let mut i = chunk + 1;
        while i < self.chunks.len() {
            self.chunks[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Sum of `extent - estimate` over every chunk before `chunk`.
    fn chunk_prefix(&self, chunk: usize) -> f64 {
        if self.chunks.is_empty() {
            return 0.0;
        }
        let mut i = chunk.min(self.chunks.len() - 1);
        let mut sum = 0.0;
        while i > 0 {
            sum += self.chunks[i];
            i -= i & i.wrapping_neg();
        }
        sum
    }

    /// The last chunk whose leading edge is at or before `along`.
    ///
    /// A Fenwick descent over `c · CHUNK · estimate + chunk_prefix(c)`, which
    /// rises with `c` because every row's extent is at least zero.
    fn chunk_at(&self, along: f64) -> usize {
        let last = self.chunk_count() - 1;
        let span = CHUNK as f64 * f64::from(self.basis.estimate);
        if self.chunks.is_empty() {
            return ((along / span).floor() as usize).min(last);
        }
        let n = self.chunks.len() - 1;
        let mut pos = 0;
        let mut acc = 0.0;
        let mut step = n.next_power_of_two();
        while step > 0 {
            let next = pos + step;
            if next <= n && next as f64 * span + acc + self.chunks[next] <= along {
                pos = next;
                acc += self.chunks[next];
            }
            step >>= 1;
        }
        pos.min(last)
    }
}

/// Every collection's [`ListExtents`], and the scroll shifts that keep their
/// visible rows still. Owned by the host beside its measure cache.
#[derive(Clone, Debug, Default)]
pub struct RowExtents {
    lists: BTreeMap<String, ListExtents>,
    shifts: BTreeMap<String, f32>,
    revision: u64,
}

impl RowExtents {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The collection `id`'s rows, if it has been placed.
    #[must_use]
    pub fn list(&self, id: &str) -> Option<&ListExtents> {
        self.lists.get(id)
    }

    /// The content extent of collection `id`: what it measured, if it has
    /// been placed under this estimate, theme and scale, else
    /// `count × estimate`.
    #[must_use]
    pub fn content_extent(
        &self,
        id: &str,
        count: usize,
        estimate: f32,
        theme_rev: u64,
        scale: Scale,
    ) -> f32 {
        match self.lists.get(id) {
            Some(list)
                if list.basis.estimate == estimate
                    && list.basis.theme_rev == theme_rev
                    && list.basis.scale == scale.factor()
                    && list.count == count =>
            {
                list.total()
            }
            _ => count as f32 * estimate,
        }
    }

    /// Scroll offsets to move, by id, since the last call: the distance
    /// ledger changes moved each list's first visible row. A host adds each
    /// to its stored offset before the next frame.
    pub fn take_shifts(&mut self) -> BTreeMap<String, f32> {
        std::mem::take(&mut self.shifts)
    }

    /// Forget every list.
    pub fn clear(&mut self) {
        self.lists.clear();
        self.shifts.clear();
        self.revision += 1;
    }

    /// Moves whenever any list's extents change. A frame placed against a
    /// ledger whose revision then moved showed the extents from before its
    /// own measurements, so a host asks for one more frame; a frame that
    /// measured nothing new leaves it alone, so an idle window stays idle.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Open `id` for this pass's measurements: reset under a new basis,
    /// resized to `count`.
    pub(crate) fn open(
        &mut self,
        id: &str,
        estimate: f32,
        cross: f32,
        theme_rev: u64,
        scale: Scale,
        count: usize,
    ) -> ListWriter<'_> {
        let basis = Basis {
            estimate,
            cross,
            theme_rev,
            scale: scale.factor(),
        };
        let list = self
            .lists
            .entry(id.to_owned())
            .or_insert_with(|| ListExtents::new(basis, count));
        let mut changed = false;
        if list.basis != basis {
            changed = !list.rows.is_empty() || list.basis.estimate != estimate;
            *list = ListExtents::new(basis, count);
        }
        changed |= list.resize(count);
        if changed {
            self.revision += 1;
        }
        ListWriter {
            list,
            revision: &mut self.revision,
        }
    }

    /// Queue `delta` for the scroll `id`.
    pub(crate) fn shift(&mut self, id: &str, delta: f32) {
        if delta != 0.0 && delta.is_finite() {
            *self.shifts.entry(id.to_owned()).or_insert(0.0) += delta;
        }
    }
}

/// A list opened for one pass's measurements.
pub(crate) struct ListWriter<'a> {
    list: &'a mut ListExtents,
    revision: &'a mut u64,
}

impl ListWriter<'_> {
    /// Drop a remembered row whose key changed; see [`ListExtents::check`].
    pub(crate) fn check(&mut self, index: usize, key: &str) {
        if self.list.check(index, key) {
            *self.revision += 1;
        }
    }

    /// See [`ListExtents::record`].
    pub(crate) fn record(&mut self, index: usize, key: &str, extent: f32, near: usize) {
        if self.list.record(index, key, extent, near) {
            *self.revision += 1;
        }
    }

    /// The list as it stands now.
    pub(crate) fn list(&self) -> &ListExtents {
        self.list
    }
}

/// A row key's identity, stable within a process.
fn identity(key: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{CHUNK, ROW_CAP, RowExtents};
    use crate::geom::Scale;

    fn open(ledger: &mut RowExtents, count: usize) -> super::ListWriter<'_> {
        ledger.open("/list", 20.0, 100.0, 1, Scale::ONE, count)
    }

    /// Prefix sums agree with a plain running sum, across chunk boundaries
    /// and around measured rows on both sides of them.
    #[test]
    fn prefix_and_index_agree_with_a_running_sum() {
        let mut ledger = RowExtents::new();
        let count = 3 * CHUNK + 17;
        let mut w = open(&mut ledger, count);
        let tall = [
            0,
            1,
            5,
            CHUNK - 1,
            CHUNK,
            CHUNK + 3,
            2 * CHUNK + 9,
            count - 1,
        ];
        for (n, &k) in tall.iter().enumerate() {
            w.record(k, &format!("r{k}"), 8.0 + 13.0 * n as f32, 0);
        }
        let list = w.list();
        let mut sum = 0.0_f64;
        for k in 0..count {
            assert!(
                (f64::from(list.prefix(k)) - sum).abs() < 1e-3,
                "prefix({k}) = {} against a running sum of {sum}",
                list.prefix(k)
            );
            let extent = f64::from(list.extent(k));
            // The middle of the row's span belongs to the row.
            assert_eq!(list.index_at((sum + extent / 2.0) as f32), k);
            sum += extent;
        }
        assert!((f64::from(list.total()) - sum).abs() < 1e-3);
        assert_eq!(list.index_at(-5.0), 0);
        assert_eq!(list.index_at(sum as f32 + 100.0), count - 1);
    }

    /// A row that measures its estimate costs nothing, and a row whose key
    /// changed loses the measurement taken from the row before it.
    #[test]
    fn only_off_estimate_rows_are_kept_and_only_for_their_own_key() {
        let mut ledger = RowExtents::new();
        let mut w = open(&mut ledger, 100);
        w.record(3, "a", 20.0, 0);
        assert_eq!(w.list().measured(), 0);
        w.record(3, "a", 50.0, 0);
        assert_eq!(w.list().extent(3), 50.0);
        w.check(3, "a");
        assert_eq!(w.list().extent(3), 50.0);
        w.check(3, "b");
        assert_eq!(w.list().extent(3), 20.0);
        assert_eq!(w.list().total(), 2000.0);
    }

    /// The ledger is bounded: past [`ROW_CAP`] the measurement farthest from
    /// the window goes, and the prefix sums still agree with what is kept.
    #[test]
    fn a_long_scroll_keeps_at_most_the_cap() {
        let mut ledger = RowExtents::new();
        let count = 3 * ROW_CAP;
        let mut w = open(&mut ledger, count);
        for k in 0..count {
            w.record(k, &format!("r{k}"), 30.0, k);
        }
        let list = w.list();
        assert_eq!(list.measured(), ROW_CAP);
        assert_eq!(list.extent(0), 20.0, "the far end was evicted");
        assert_eq!(list.extent(count - 1), 30.0, "the near end was kept");
        let expected = (count - ROW_CAP) as f32 * 20.0 + ROW_CAP as f32 * 30.0;
        assert!((list.total() - expected).abs() < 1e-2);
    }

    /// A new cross extent is a new basis: what was measured at the old width
    /// is gone.
    #[test]
    fn a_new_basis_resets_the_list() {
        let mut ledger = RowExtents::new();
        open(&mut ledger, 10).record(2, "r2", 44.0, 0);
        assert_eq!(ledger.list("/list").unwrap().extent(2), 44.0);
        let w = ledger.open("/list", 20.0, 140.0, 1, Scale::ONE, 10);
        assert_eq!(w.list().extent(2), 20.0);
    }
}
