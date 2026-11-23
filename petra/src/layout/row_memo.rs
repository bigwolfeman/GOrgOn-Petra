//! The rows each collection materialized last, so the next fetch of the same
//! row can be told apart from a changed one.
//!
//! A [`crate::layout::RowSource`] answers with fresh nodes every frame, and
//! the measure cache is keyed by id, not content. A row whose key stayed put
//! while its text changed would answer `measure` from the size its old text
//! took. Before incremental frames that could not happen in practice, because
//! a host that rebuilt its rows also answered `ChangeSet::All` and cleared
//! the cache every frame. With the cache kept across frames, the engine has
//! to notice for itself: each fetched row is grafted onto the row with the
//! same key from the last fetch ([`crate::tree::graft_shared`]), and the
//! nodes that moved are named for invalidation. A host's row source keeps
//! the plain contract it always had; it never has to report a row change.
//!
//! # Bounds
//!
//! Each list keeps the rows of its last fetch only, so one list holds at most
//! one window. Lists are kept least-recently-fetched first up to
//! [`LIST_CAP`], so a session that opens many lists (one per file viewed, say)
//! does not hold every window it ever showed.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::tree::{KeyPath, ViewNode, graft_shared};

/// The most lists whose last rows are remembered.
pub const LIST_CAP: usize = 256;

/// One list's rows from its last fetch, by row key.
#[derive(Clone, Debug, Default)]
struct ListRows {
    /// When this list was last fetched, on [`RowMemo`]'s clock.
    touched: u64,
    /// The fetch in progress: rows grafted since [`RowMemo::begin`].
    current: HashMap<String, Arc<ViewNode>>,
    /// The fetch before it, which `current` is grafted onto.
    previous: HashMap<String, Arc<ViewNode>>,
}

/// The rows every collection last fetched. Owned by
/// [`crate::layout::RowExtents`], beside the extents it keeps per list.
#[derive(Clone, Debug, Default)]
pub struct RowMemo {
    lists: BTreeMap<String, ListRows>,
    clock: u64,
}

impl RowMemo {
    /// Start a fetch of `list`: what it fetched last becomes what this fetch
    /// is compared with.
    pub(crate) fn begin(&mut self, list: &str) {
        self.clock += 1;
        let clock = self.clock;
        let rows = self.lists.entry(list.to_owned()).or_default();
        rows.touched = clock;
        rows.previous = std::mem::take(&mut rows.current);
        if self.lists.len() > LIST_CAP
            && let Some(oldest) = self
                .lists
                .iter()
                .min_by_key(|(_, rows)| rows.touched)
                .map(|(id, _)| id.clone())
        {
            self.lists.remove(&oldest);
        }
    }

    /// Graft `row`, fetched for `list`, onto its counterpart from the last
    /// fetch, and push every id under it that moved to `moved`.
    ///
    /// `path` is the collection's own path; the row's key is pushed and
    /// popped here. A row the last fetch did not have names every node
    /// under it, so a stale measurement left at that path by an older row
    /// cannot answer for it.
    pub(crate) fn graft(
        &mut self,
        list: &str,
        row: Arc<ViewNode>,
        path: &mut KeyPath,
        moved: &mut Vec<String>,
    ) -> Arc<ViewNode> {
        let rows = self.lists.entry(list.to_owned()).or_default();
        let key = row.key.as_str().to_owned();
        let old = rows.current.get(&key).or_else(|| rows.previous.get(&key));
        let kept = graft_shared(old, row, path, moved);
        rows.current.insert(key, Arc::clone(&kept));
        kept
    }

    /// Forget every list.
    pub fn clear(&mut self) {
        self.lists.clear();
    }

    /// How many lists are remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lists.len()
    }

    /// Whether no list is remembered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{LIST_CAP, RowMemo};
    use crate::tree::{KeyPath, NodeKind, ViewNode};

    fn row(key: &str, body: &str) -> Arc<ViewNode> {
        let mut text = ViewNode::new(NodeKind::Text, "label");
        text.props.text = Some(body.to_owned());
        Arc::new(ViewNode::new(NodeKind::Stack, key).child(text))
    }

    fn fetch(memo: &mut RowMemo, rows: &[Arc<ViewNode>]) -> (Vec<Arc<ViewNode>>, Vec<String>) {
        let mut path = KeyPath::root();
        path.push(crate::tree::Key::new("list"));
        memo.begin("/list");
        let mut moved = Vec::new();
        let kept = rows
            .iter()
            .map(|r| memo.graft("/list", Arc::clone(r), &mut path, &mut moved))
            .collect();
        (kept, moved)
    }

    #[test]
    fn a_first_fetch_names_every_node_it_brings() {
        let mut memo = RowMemo::default();
        let (_, moved) = fetch(&mut memo, &[row("a", "one")]);
        assert_eq!(moved, ["/list/a", "/list/a/label"]);
    }

    #[test]
    fn an_equal_refetch_names_nothing_and_keeps_the_old_allocation() {
        let mut memo = RowMemo::default();
        let (first, _) = fetch(&mut memo, &[row("a", "one")]);
        let (second, moved) = fetch(&mut memo, &[row("a", "one")]);
        assert!(moved.is_empty(), "{moved:?}");
        assert!(Arc::ptr_eq(&first[0], &second[0]));
    }

    /// The case this module exists for: same key, new text.
    #[test]
    fn a_row_whose_text_changed_under_the_same_key_names_the_text() {
        let mut memo = RowMemo::default();
        fetch(&mut memo, &[row("a", "one")]);
        let (_, moved) = fetch(&mut memo, &[row("a", "a much longer label")]);
        assert_eq!(moved, ["/list/a/label"]);
    }

    /// A row scrolled out of one fetch and back into a later one is new
    /// again: only the last fetch is remembered.
    #[test]
    fn only_the_last_fetch_is_remembered() {
        let mut memo = RowMemo::default();
        fetch(&mut memo, &[row("a", "one")]);
        fetch(&mut memo, &[row("b", "two")]);
        let (_, moved) = fetch(&mut memo, &[row("a", "one")]);
        assert_eq!(moved, ["/list/a", "/list/a/label"]);
    }

    #[test]
    fn the_least_recently_fetched_list_goes_first_past_the_cap() {
        let mut memo = RowMemo::default();
        for n in 0..=LIST_CAP {
            memo.begin(&format!("/list-{n}"));
        }
        assert_eq!(memo.len(), LIST_CAP);
        assert!(!memo.lists.contains_key("/list-0"));
        assert!(memo.lists.contains_key(&format!("/list-{LIST_CAP}")));
    }
}
