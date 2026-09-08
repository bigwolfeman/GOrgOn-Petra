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
        for ch in self.0.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '/' => out.push_str("\\/"),
                other => out.push(other),
            }
        }
        out
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyPath(Vec<Key>);

impl KeyPath {
    /// The empty path (the root's parent).
    #[must_use]
    pub fn root() -> Self {
        Self(Vec::new())
    }

    /// This path with `key` appended.
    #[must_use]
    pub fn child(&self, key: &Key) -> Self {
        let mut next = self.0.clone();
        next.push(key.clone());
        Self(next)
    }

    /// Append `key` in place. Pair with [`KeyPath::pop`] when walking a tree,
    /// which is why the walk does not allocate a path per node.
    pub fn push(&mut self, key: Key) {
        self.0.push(key);
    }

    /// Remove the last segment.
    pub fn pop(&mut self) -> Option<Key> {
        self.0.pop()
    }

    /// The segments, root first.
    #[must_use]
    pub fn segments(&self) -> &[Key] {
        &self.0
    }

    /// Number of segments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether this is the root path.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The canonical id string: escaped segments joined by `/`.
    ///
    /// The root path prints as `/`, so an id is never the empty string.
    #[must_use]
    pub fn id(&self) -> String {
        Self::join(self.0.iter())
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
        let parent = self.0.len().saturating_sub(1);
        Self::join(self.0[..parent].iter().chain(std::iter::once(key)))
    }

    /// `/`-joined escaped `segments`; `/` alone for none.
    fn join<'a>(segments: impl Iterator<Item = &'a Key>) -> String {
        let mut out = String::new();
        for key in segments {
            out.push('/');
            out.push_str(&key.escaped());
        }
        if out.is_empty() {
            out.push('/');
        }
        out
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
        if !id.starts_with('/') {
            return Vec::new();
        }
        let bytes = id.as_bytes();
        let mut boundaries = Vec::new();
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
        boundaries
            .into_iter()
            .rev()
            .map(|b| id[..b].to_owned())
            .collect()
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyPath};

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
