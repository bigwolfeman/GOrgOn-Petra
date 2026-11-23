//! Node keys and key paths — the identity spine of the engine.
//!
//! A [`Key`] is a node's identity within its parent. The chain of keys from
//! the root is a [`KeyPath`], and the canonical string form of a key path is
//! the semantic-tree `id` (`contracts/semantic-tree.md`). Ids therefore stay
//! stable across frames, scrolls, and re-parents that preserve the path, and
//! are never recycled, because they are derived rather than allocated.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A node's identity within its parent.
///
/// Keys are author-supplied strings. `/` and `\` are escaped in the canonical
/// path form so no two distinct paths can ever print the same id.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Key(String);

impl Key {
    /// A key from anything string-like.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The raw, unescaped key text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The key text with path separators escaped.
    #[must_use]
    pub fn escaped(&self) -> String {
        let mut out = String::with_capacity(self.0.len());
        self.escape_into(&mut out);
        out
    }

    /// [`Key::escaped`] appended to `out`. A key with nothing to escape is
    /// one copy.
    fn escape_into(&self, out: &mut String) {
        if !self.0.contains(['\\', '/']) {
            out.push_str(&self.0);
            return;
        }
        for ch in self.0.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '/' => out.push_str("\\/"),
                other => out.push(other),
            }
        }
    }
}

impl From<&str> for Key {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Key {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The chain of keys from the root to a node.
///
/// The canonical id is kept alongside the keys and grown and cut with them,
/// so [`KeyPath::id`] is one copy rather than a join, and [`KeyPath::as_id`]
/// is none: a layout walk asks for the id of nearly every node it visits.
/// See `.agents/notes/implemented/bug-fix/2026-11-23-scroll-frames-cost-what-they-change.md`.
#[derive(Clone, Debug, Default)]
pub struct KeyPath {
    keys: Vec<Key>,
    /// `/`-joined escaped keys: [`KeyPath::id`] without the root's lone `/`.
    id: String,
    /// `id`'s length before each key was pushed, one per key.
    ends: Vec<usize>,
}

impl PartialEq for KeyPath {
    fn eq(&self, other: &Self) -> bool {
        self.keys == other.keys
    }
}

impl Eq for KeyPath {}

impl std::hash::Hash for KeyPath {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.keys.hash(state);
    }
}

impl PartialOrd for KeyPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KeyPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.keys.cmp(&other.keys)
    }
}

impl KeyPath {
    /// The empty path (the root's parent).
    #[must_use]
    pub fn root() -> Self {
        Self::default()
    }

    /// This path with `key` appended.
    #[must_use]
    pub fn child(&self, key: &Key) -> Self {
        let mut next = self.clone();
        next.push(key.clone());
        next
    }

    /// Append `key` in place. Pair with [`KeyPath::pop`] when walking a tree,
    /// which is why the walk does not allocate a path per node.
    pub fn push(&mut self, key: Key) {
        self.ends.push(self.id.len());
        self.id.push('/');
        key.escape_into(&mut self.id);
        self.keys.push(key);
    }

    /// Remove the last segment.
    pub fn pop(&mut self) -> Option<Key> {
        let key = self.keys.pop()?;
        if let Some(end) = self.ends.pop() {
            self.id.truncate(end);
        }
        Some(key)
    }

    /// The segments, root first.
    #[must_use]
    pub fn segments(&self) -> &[Key] {
        &self.keys
    }

    /// Number of segments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether this is the root path.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The canonical id string: escaped segments joined by `/`.
    ///
    /// The root path prints as `/`, so an id is never the empty string.
    #[must_use]
    pub fn id(&self) -> String {
        self.as_id().to_owned()
    }

    /// [`KeyPath::id`], borrowed.
    #[must_use]
    pub fn as_id(&self) -> &str {
        if self.id.is_empty() { "/" } else { &self.id }
    }

    /// The canonical id of the node keyed `key` in the same child list as
    /// this path's last segment: this path with its last segment replaced by
    /// `key`.
    ///
    /// This is how [`crate::tree::Anchor::Sibling`] resolves — an anchored
    /// surface names a sibling by bare key, and the surface's own path is
    /// the only context needed to turn that key into a canonical id. No
    /// search, no ambiguity: the answer is a function of this path and
    /// `key` alone, so two surfaces at different depths that both name
    /// `"trigger"` can never resolve to each other's trigger.
    ///
    /// The root path has no parent, so its "sibling" is the top-level id
    /// `/key`. That names the tree's own top node only when `key` is its
    /// key — and a surface anchored to itself is a cycle `validate`
    /// refuses — so a `Sibling` anchor on a root surface never resolves to
    /// a usable node, which is the right answer: a root has no siblings.
    #[must_use]
    pub fn sibling_id(&self, key: &Key) -> String {
        let parent = self.keys.len().saturating_sub(1);
        Self::join(self.keys[..parent].iter().chain(std::iter::once(key)))
    }

    /// `/`-joined escaped `segments`; `/` alone for none.
    fn join<'a>(segments: impl Iterator<Item = &'a Key>) -> String {
        let mut out = String::new();
        for key in segments {
            out.push('/');
            key.escape_into(&mut out);
        }
        if out.is_empty() {
            out.push('/');
        }
        out
    }

    /// Whether the canonical id `id` names `ancestor` itself or a node
    /// anywhere under it.
    ///
    /// A prefix test with a boundary check: `/ab` is not under `/a`, so the
    /// byte after the prefix must be the separator. Sound for the same reason
    /// [`KeyPath::ancestor_ids`] is: an escaped key never puts a bare `/`
    /// right after an ancestor's last byte.
    #[must_use]
    pub fn is_at_or_under(id: &str, ancestor: &str) -> bool {
        id.strip_prefix(ancestor)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    }

    /// The ids of every ancestor of the canonical id `id`, nearest parent
    /// first and the tree's top-level node last, not including `id` itself.
    ///
    /// This is byte-address arithmetic over [`KeyPath::id`]'s own output, not
    /// a general parser: an id is `/` followed by escaped segments
    /// (`Key::escaped` doubles every literal `\` and prefixes every literal
    /// `/` with `\`), so a `/` byte reached while not consuming the second
    /// half of a `\`-escape is always a real separator. Escaped bytes are
    /// skipped in pairs, so a key containing its own `/` or `\` cannot be
    /// mistaken for a path boundary — the tree the id was built from
    /// (`ViewNode`) is not needed to tell the two apart. `\`, `/`, and every
    /// separator byte this scans for are ASCII, and no UTF-8 continuation or
    /// lead byte can equal one, so slicing at a found boundary is always a
    /// valid `char` boundary regardless of what the keys otherwise contain.
    ///
    /// A malformed id — empty, or missing the leading `/` every id
    /// [`KeyPath::id`] produces carries — has no ancestors: there is no
    /// ancestry to recover from a string that was never one of `id`'s
    /// outputs, so this returns an empty list rather than guessing or
    /// panicking. The tree's own top-level node (an id with one segment, e.g.
    /// `/root`) likewise has no ancestors above it — `KeyPath::root()`, the
    /// empty path, names no real node and is never returned here.
    #[must_use]
    pub fn ancestor_ids(id: &str) -> Vec<String> {
        Self::ancestor_ends(id)
            .map(|end| id[..end].to_owned())
            .collect()
    }

    /// Where each ancestor id of `id` ends, nearest parent first:
    /// `id[..end]` is that ancestor's id. The same answer as
    /// [`KeyPath::ancestor_ids`] without copying a string, for a caller that
    /// keeps only some of them.
    pub fn ancestor_ends(id: &str) -> impl Iterator<Item = usize> {
        let mut boundaries = Vec::new();
        if id.starts_with('/') {
            let bytes = id.as_bytes();
            let mut i = 1; // the leading '/' is not itself a boundary
            while i < bytes.len() {
                match bytes[i] {
                    // A lone trailing backslash is a malformed escape; treat it
                    // as one byte rather than reading past the end.
                    b'\\' if i + 1 < bytes.len() => i += 2,
                    b'\\' => i += 1,
                    b'/' => {
                        boundaries.push(i);
                        i += 1;
                    }
                    _ => i += 1,
                }
            }
        }
        boundaries.into_iter().rev()
    }
}

impl KeyPath {
    /// The path whose [`KeyPath::id`] is `id`: the inverse of `id`, escapes
    /// undone. `None` for a string `id` never produces (empty, no leading
    /// `/`, an empty segment, or a lone trailing `\`).
    ///
    /// For a reader of a routed id that needs a segment's key back, such as
    /// the row a focused tree item stands for. A segment's key may contain
    /// `/` or `\`, which is why a plain `split('/')` is wrong.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        let rest = id.strip_prefix('/')?;
        if rest.is_empty() {
            return Some(Self::root());
        }
        let mut keys = Vec::new();
        let mut key = String::new();
        let mut chars = rest.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '\\' => key.push(chars.next()?),
                '/' => {
                    if key.is_empty() {
                        return None;
                    }
                    keys.push(Key::new(std::mem::take(&mut key)));
                }
                other => key.push(other),
            }
        }
        if key.is_empty() {
            return None;
        }
        keys.push(Key::new(key));
        let mut path = Self::root();
        for key in keys {
            path.push(key);
        }
        Some(path)
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_id())
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyPath};

    /// `parse` undoes `id` for keys holding `/` and `\`, and refuses what
    /// `id` never writes.
    #[test]
    fn parse_is_the_inverse_of_id() {
        let path = KeyPath::root()
            .child(&Key::new("files-tree"))
            .child(&Key::new("rows"))
            .child(&Key::new("/home/w/a\\b.rs"));
        assert_eq!(KeyPath::parse(&path.id()), Some(path.clone()));
        assert_eq!(KeyPath::parse("/"), Some(KeyPath::root()));
        for bad in ["", "root", "/a//b", "/a/", "/a\\"] {
            assert_eq!(KeyPath::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn root_id_is_a_single_slash() {
        assert_eq!(KeyPath::root().id(), "/");
    }

    #[test]
    fn ids_nest() {
        let path = KeyPath::root()
            .child(&Key::new("app"))
            .child(&Key::new("list"));
        assert_eq!(path.id(), "/app/list");
        assert_eq!(path.len(), 2);
    }

    /// Two different paths must never print the same id, or a driver `act`
    /// could reach a node the caller did not name.
    #[test]
    fn separators_in_keys_cannot_forge_a_path() {
        let forged = KeyPath::root().child(&Key::new("a/b"));
        let real = KeyPath::root().child(&Key::new("a")).child(&Key::new("b"));
        assert_ne!(forged.id(), real.id());
        assert_eq!(forged.id(), "/a\\/b");
    }

    #[test]
    fn backslashes_escape_too() {
        assert_eq!(KeyPath::root().child(&Key::new("a\\b")).id(), "/a\\\\b");
    }

    /// A sibling id is this path with its last segment swapped: the same
    /// parent, a different key. Depth does not enter into it.
    #[test]
    fn sibling_id_swaps_the_last_segment_only() {
        let surface = KeyPath::root()
            .child(&Key::new("root"))
            .child(&Key::new("page"))
            .child(&Key::new("menu"));
        assert_eq!(
            surface.sibling_id(&Key::new("trigger")),
            "/root/page/trigger"
        );
        assert_eq!(
            surface.sibling_id(&Key::new("trigger")),
            KeyPath::root()
                .child(&Key::new("root"))
                .child(&Key::new("page"))
                .child(&Key::new("trigger"))
                .id(),
            "a sibling id is exactly the id the sibling's own path prints"
        );
    }

    /// The sibling's key is escaped the way every segment is, so a key
    /// holding a separator cannot forge a deeper path.
    #[test]
    fn sibling_id_escapes_the_key() {
        let surface = KeyPath::root().child(&Key::new("a")).child(&Key::new("s"));
        assert_eq!(surface.sibling_id(&Key::new("b/c")), "/a/b\\/c");
    }

    /// A root has no parent and so no siblings: the id it produces is a
    /// top-level one, which names the tree's own top node only when the key
    /// is that node's — a self-anchor, which acceptance refuses as a cycle.
    #[test]
    fn sibling_id_of_the_root_path_is_top_level() {
        assert_eq!(KeyPath::root().sibling_id(&Key::new("x")), "/x");
        assert_eq!(
            KeyPath::root()
                .child(&Key::new("root"))
                .sibling_id(&Key::new("x")),
            "/x"
        );
    }

    #[test]
    fn ancestor_ids_are_the_successive_prefixes_root_last() {
        assert_eq!(
            KeyPath::ancestor_ids("/root/panel-0/row-0"),
            vec!["/root/panel-0".to_owned(), "/root".to_owned()]
        );
    }

    /// The tree's top-level node has no ancestor above it, and neither does
    /// the empty root path — `KeyPath::root()` names no real node.
    #[test]
    fn a_top_level_id_and_the_root_path_have_no_ancestors() {
        assert!(KeyPath::ancestor_ids("/root").is_empty());
        assert!(KeyPath::ancestor_ids(&KeyPath::root().id()).is_empty());
    }

    /// Neither an empty string nor an id missing its leading `/` is a
    /// canonical id; both are handled without panicking and both come back
    /// with no ancestors, because there is no ancestry to recover from text
    /// that was never one of `KeyPath::id`'s outputs.
    #[test]
    fn a_malformed_id_has_no_ancestors_and_does_not_panic() {
        assert!(KeyPath::ancestor_ids("").is_empty());
        assert!(KeyPath::ancestor_ids("root/panel-0").is_empty());
    }

    /// An escaped `/` inside a key must not be read as a path boundary: the
    /// only real boundary here is the one before `c`.
    #[test]
    fn an_escaped_separator_inside_a_key_is_not_a_boundary() {
        let path = KeyPath::root()
            .child(&Key::new("a/b"))
            .child(&Key::new("c"));
        assert_eq!(path.id(), "/a\\/b/c");
        assert_eq!(KeyPath::ancestor_ids(&path.id()), vec!["/a\\/b".to_owned()]);
    }

    #[test]
    fn push_and_pop_walk_without_cloning() {
        let mut path = KeyPath::root();
        path.push(Key::new("a"));
        path.push(Key::new("b"));
        assert_eq!(path.id(), "/a/b");
        assert_eq!(path.pop(), Some(Key::new("b")));
        assert_eq!(path.id(), "/a");
    }

    /// The id from scratch: escaped keys joined by `/`, the root a lone `/`.
    fn joined(keys: &[Key]) -> String {
        if keys.is_empty() {
            return "/".to_owned();
        }
        keys.iter().map(|k| format!("/{}", k.escaped())).collect()
    }

    proptest::proptest! {
        /// The id a path keeps beside its keys is the id its keys spell,
        /// after any walk of pushes and pops, separators in keys included.
        #[test]
        fn the_kept_id_is_the_joined_id_after_any_walk(
            steps in proptest::collection::vec(
                proptest::option::of("[a/\\\\b]{0,3}"),
                0..40,
            )
        ) {
            let mut path = KeyPath::root();
            let mut keys: Vec<Key> = Vec::new();
            for step in steps {
                match step {
                    Some(key) => {
                        path.push(Key::new(key.clone()));
                        keys.push(Key::new(key));
                    }
                    None => {
                        proptest::prop_assert_eq!(path.pop(), keys.pop());
                    }
                }
                let want = joined(&keys);
                proptest::prop_assert_eq!(path.id(), want.clone());
                proptest::prop_assert_eq!(path.as_id(), want.as_str());
                proptest::prop_assert_eq!(path.segments(), keys.as_slice());
                // `parse` refuses an empty segment, so only a path with none
                // round-trips.
                if keys.iter().all(|k| !k.as_str().is_empty()) {
                    proptest::prop_assert_eq!(KeyPath::parse(path.as_id()), Some(path.clone()));
                }
            }
        }

        /// `ancestor_ends` names the same ancestors `ancestor_ids` copies.
        #[test]
        fn ancestor_ends_are_where_ancestor_ids_end(
            keys in proptest::collection::vec("[a/\\\\b]{1,3}", 0..8)
        ) {
            let mut path = KeyPath::root();
            for key in keys {
                path.push(Key::new(key));
            }
            let id = path.id();
            let ends: Vec<String> = KeyPath::ancestor_ends(&id)
                .map(|end| id[..end].to_owned())
                .collect();
            proptest::prop_assert_eq!(ends, KeyPath::ancestor_ids(&id));
        }
    }

    /// Equality, order and hashing read the keys, never the kept id, so two
    /// paths built differently are one path.
    #[test]
    fn two_walks_to_one_path_are_equal() {
        use std::hash::{BuildHasher, RandomState};
        let mut walked = KeyPath::root();
        walked.push(Key::new("x"));
        walked.pop();
        walked.push(Key::new("a"));
        walked.push(Key::new("b"));
        let built = KeyPath::root().child(&Key::new("a")).child(&Key::new("b"));
        assert_eq!(walked, built);
        assert_eq!(walked.cmp(&built), std::cmp::Ordering::Equal);
        let state = RandomState::new();
        assert_eq!(state.hash_one(&walked), state.hash_one(&built));
    }
}

// No Kani harness lives here, and the reason is a hard tool limit rather than
// a gap. `escaped` is injective — the module doc's "no two distinct paths can
// ever print the same id" — was written against a 3-letter alphabet (`a`, and
// both separators `escaped` treats specially) over every 2-character key, the
// smallest bound that still exercises both escape branches. It did not
// terminate inside 600 s. The wall here is `String` rather than hashing:
// CBMC's trace ends in `alloc::raw_vec::RawVecInner`, `capacity_overflow`,
// and `handle_alloc_error`, so a growable allocation costs more than the
// 81-key input space it was carrying. A proof of this property needs an
// `escaped` that writes into a fixed buffer, which is a design change and not
// a survey's business. See
// `.agents/notes/proposed/testing/2026-08-30-kani-bounded-verification.md`.
