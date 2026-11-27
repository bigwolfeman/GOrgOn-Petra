//! A Fenwick (binary indexed) tree over chunk sizes.
//!
//! [`super::buffer::TextBuffer`] keeps two of these: one over the byte length
//! of each chunk, one over its newline count. A prefix sum answers "where does
//! chunk `i` start" and a descent answers "which chunk holds byte `b`", both in
//! `O(log chunks)`. A point update after an edit inside one chunk is also
//! `O(log chunks)`; a split or merge of chunks rebuilds in `O(chunks)`.

/// Prefix sums over a sequence of `usize` counts.
#[derive(Clone, Debug, Default)]
pub(crate) struct Fenwick {
    /// 1-based tree; `tree[0]` is unused.
    tree: Vec<usize>,
}

impl Fenwick {
    /// A tree over `counts`, built in `O(n)`.
    pub(crate) fn build(counts: impl ExactSizeIterator<Item = usize>) -> Self {
        let n = counts.len();
        let mut tree = Vec::with_capacity(n + 1);
        tree.push(0);
        tree.extend(counts);
        for i in 1..=n {
            let parent = i + (i & i.wrapping_neg());
            if parent <= n {
                tree[parent] += tree[i];
            }
        }
        Self { tree }
    }

    /// Number of counts.
    pub(crate) fn len(&self) -> usize {
        self.tree.len().saturating_sub(1)
    }

    /// Add `delta` (which may be negative) to count `i`.
    pub(crate) fn add(&mut self, i: usize, delta: isize) {
        let n = self.len();
        let mut at = i + 1;
        while at <= n {
            self.tree[at] = self.tree[at].wrapping_add_signed(delta);
            at += at & at.wrapping_neg();
        }
    }

    /// Sum of counts `0..i`.
    pub(crate) fn prefix(&self, i: usize) -> usize {
        let mut at = i.min(self.len());
        let mut sum = 0;
        while at > 0 {
            sum += self.tree[at];
            at &= at - 1;
        }
        sum
    }

    /// The smallest `i` with `prefix(i + 1) > target`, that is, the index of
    /// the count that holds unit `target` when the counts are laid end to
    /// end. Returns `len()` when `target` is at or past the total.
    pub(crate) fn find(&self, target: usize) -> usize {
        let n = self.len();
        let mut pos = 0;
        let mut rest = target;
        let mut step = if n == 0 { 0 } else { 1 << n.ilog2() };
        while step > 0 {
            let next = pos + step;
            if next <= n && self.tree[next] <= rest {
                pos = next;
                rest -= self.tree[next];
            }
            step >>= 1;
        }
        pos
    }
}

#[cfg(test)]
mod tests {
    use super::Fenwick;

    #[test]
    fn prefix_and_find_agree_with_a_linear_scan() {
        let counts = [3usize, 0, 5, 1, 0, 0, 7, 2];
        let mut f = Fenwick::build(counts.iter().copied());
        let linear = |c: &[usize], i: usize| c[..i].iter().sum::<usize>();
        for i in 0..=counts.len() {
            assert_eq!(f.prefix(i), linear(&counts, i));
        }
        let total: usize = counts.iter().sum();
        for t in 0..total {
            let i = f.find(t);
            assert!(
                linear(&counts, i) <= t && t < linear(&counts, i + 1),
                "t={t} i={i}"
            );
        }
        assert_eq!(f.find(total), counts.len());
        f.add(1, 4);
        f.add(2, -5);
        let changed = [3usize, 4, 0, 1, 0, 0, 7, 2];
        for i in 0..=changed.len() {
            assert_eq!(f.prefix(i), linear(&changed, i));
        }
    }
}
