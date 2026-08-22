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
        if self.0.is_empty() {
            return "/".into();
        }
        let mut out = String::new();
        for key in &self.0 {
            out.push('/');
            out.push_str(&key.escaped());
        }
        out
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
