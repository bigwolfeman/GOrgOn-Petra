//! Who authored a node: the key convention that answers it.
//!
//! A surface contribution is a subtree a fiber authored and the shell spliced
//! into a slot of its own tree
//! (`specs/005-petra-carbon-authoring/contracts/surface-contribution.md` §9).
//! Every node in that subtree has to be attributable to the fiber that owns
//! it, and the contract fixes *how*: the shell splices each contribution
//! under a key derived from its contribution id, so the id of every node
//! beneath it reads `/…/ui:7/…`. The kernel's operator-facing name for that
//! contribution is the same string — `ResourceRef::Ui(7)` prints `ui:7` — so
//! `who-owns ui:7` answers for the whole subtree through the query path that
//! is already in place. No new join, and no owner field.
//!
//! # Why not an owner field on every node
//!
//! [`crate::semantic::SemanticNode`] could carry an `owner: Option<...>` that
//! the projection filled in. It would be a second source of a fact the id
//! already carries, free to disagree with it, and it would cost every
//! host-authored node — which is nearly all of them — a field it never uses,
//! on a wire form three consumers read. The key path is not a hint about
//! ownership. It *is* ownership: a node's id is its key path
//! (`crate::tree::KeyPath::id`), so a node under `ui:7` cannot be moved out
//! from under it without becoming a different node.
//!
//! # What this module trusts, and what it refuses to
//!
//! The splice, and only the splice. A `ui:<n>` segment is a plain string in a
//! plain key, and nothing in it is signed, so the question is which of them
//! the *shell* wrote. The shell writes exactly one per contribution: the
//! outermost, at the mount slot. Everything deeper is inside a subtree a
//! plugin authored.
//!
//! So [`owner_of`] takes the **outermost** contribution segment, not the
//! innermost. A plugin that keys one of its own children `ui:9` gets nothing
//! for it: its nodes still read as its own, `ui:9` is not reported as a
//! contribution in the tree ([`SemanticNode::contributions`]), and
//! [`SemanticNode::contribution_root`] will not hand it back as contribution
//! 9's root. Reading the innermost instead would let any plugin hang its
//! nodes on a neighbour's name — an attribution forgery in the exact
//! mechanism §9 mandates.
//!
//! That rule is exact today because **contributions do not nest**: a mount
//! slot is host-declared, so every contribution is spliced into the host's
//! own tree and a chain of two is always a forgery. [`owner_chain`] returns
//! the whole chain for a caller that wants to see one. If a slot ever appears
//! *inside* a contributed tree, this is the paragraph to come back to.
//!
//! Two more things narrow it. [`contribution_key`] is the one sanctioned way
//! to mint the key, so the shell never hand-formats one; and
//! [`crate::semantic::ContributionLedger::owner_of`] answers only for
//! contributions the shell is actually holding, so a key naming a
//! contribution that does not exist attributes to nobody. A *host* tree that
//! keys its own node `ui:<digits>` is still a defect in that tree — the shell
//! authors both sides of that one, and no plugin can cause it.

use serde::Serialize;

use crate::semantic::node::{SemanticNode, SemanticTree};
use crate::tree::Key;

/// The prefix a contribution key carries: `ui:7` is contribution 7.
///
/// The same spelling `gorgon_kernel::ResourceRef::Ui` prints, because it names
/// the same thing. A change here is a change to an operator-facing name.
pub const CONTRIBUTION_KEY_PREFIX: &str = "ui:";

/// A surface contribution's id — the `7` in `ui:7`.
///
/// Minted by the daemon's registry when a fiber's `contribute` call is
/// accepted, and handed back to the fiber as `{ ui = 7 }`. Petra never
/// allocates one; it only reads them back out of key paths.
///
/// Serializes as its operator-facing name (`"ui:7"`), not as a bare integer,
/// so a published status block names the same string a `who-owns` query takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContributionId(u64);

impl ContributionId {
    /// The contribution with this id.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The bare id, for a caller talking to the kernel's `tables.ui`.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The key this contribution is spliced under. See [`contribution_key`].
    #[must_use]
    pub fn key(self) -> Key {
        contribution_key(self)
    }

    /// The contribution named by one key-path segment, if it names one.
    ///
    /// Canonical form only: `ui:7`, never `ui:007`, `ui:+7`, `ui:` or
    /// `ui:7x`. A non-canonical spelling is refused rather than accepted,
    /// because two spellings of one id would be two keys, and two keys are
    /// two nodes — the surface would split in half at a re-splice.
    #[must_use]
    pub fn parse(segment: &str) -> Option<Self> {
        let digits = segment.strip_prefix(CONTRIBUTION_KEY_PREFIX)?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if digits.len() > 1 && digits.starts_with('0') {
            return None;
        }
        digits.parse().ok().map(Self)
    }
}

impl std::fmt::Display for ContributionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{CONTRIBUTION_KEY_PREFIX}{}", self.0)
    }
}

impl Serialize for ContributionId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// The key a contribution is spliced under.
///
/// The one place the splice key is minted. The shell calls this rather than
/// formatting `ui:{id}` itself, so the string the splice writes and the string
/// [`owner_of`] reads cannot drift apart into a surface nobody owns.
///
/// Keying by the contribution id is also what keeps two contributions in one
/// `list` slot from colliding: ids are unique, so their keys are, so
/// `crate::tree::Violation::DuplicateSiblingKey` — which refuses the *whole*
/// tree — cannot fire between two plugins. One misbehaving plugin must not be
/// able to refuse another's surface.
///
/// The key needs no escaping (`ui:7` holds neither `/` nor `\`), so the
/// segment as it appears in an id is this string byte for byte.
#[must_use]
pub fn contribution_key(id: ContributionId) -> Key {
    Key::new(id.to_string())
}

/// The contribution that owns the node with this id, or `None` for a
/// host-authored node.
///
/// The **outermost** contribution segment: the one the shell wrote at the
/// mount slot, which is the only one it wrote. A deeper `ui:<n>` is a key a
/// plugin chose inside its own tree and names nobody — see the module docs.
#[must_use]
pub fn owner_of(id: &str) -> Option<ContributionId> {
    segments(id).find_map(ContributionId::parse)
}

/// Every `ui:<n>` segment in this node's id, outermost first.
///
/// Empty for a host node, and one long for a contributed node. Longer than
/// one only when a plugin keyed one of its own nodes like a contribution,
/// which today is always a forgery: contributions do not nest, so the shell
/// wrote the first entry and the plugin wrote the rest. [`owner_of`] is the
/// answer; this is the evidence.
#[must_use]
pub fn owner_chain(id: &str) -> Vec<ContributionId> {
    segments(id).filter_map(ContributionId::parse).collect()
}

impl SemanticNode {
    /// The root of contribution `id` within this subtree: the node the shell
    /// spliced, whose own key is `ui:<id>`.
    ///
    /// The whole contribution is that node's subtree, so a caller wanting
    /// "every node this fiber authored" walks it — there is no second index to
    /// consult and nothing to join.
    ///
    /// A node a plugin keyed `ui:<id>` inside its own tree is not one: only a
    /// splice point is, and a splice point has no contribution above it.
    #[must_use]
    pub fn contribution_root(&self, id: ContributionId) -> Option<&Self> {
        self.iter()
            .find(|node| spliced_contribution(&node.id) == Some(id))
    }

    /// Every contribution spliced into this subtree, in pre-order, each listed
    /// once.
    ///
    /// Pre-order rather than sorted, so the order is the order the surfaces
    /// appear in — which for a `list` slot is `TraceSeq` of the `contribute`
    /// calls, the order the slot was built in.
    #[must_use]
    pub fn contributions(&self) -> Vec<ContributionId> {
        let mut found = Vec::new();
        for node in self {
            if let Some(id) = spliced_contribution(&node.id)
                && !found.contains(&id)
            {
                found.push(id);
            }
        }
        found
    }
}

impl SemanticTree {
    /// [`SemanticNode::contribution_root`] over the whole tree.
    #[must_use]
    pub fn contribution_root(&self, id: ContributionId) -> Option<&SemanticNode> {
        self.root().contribution_root(id)
    }

    /// [`SemanticNode::contributions`] over the whole tree.
    #[must_use]
    pub fn contributions(&self) -> Vec<ContributionId> {
        self.root().contributions()
    }
}

/// The contribution this node is the splice point *of*: its own key is a
/// contribution key, and no segment above it is one.
///
/// The second half is what makes this a splice point rather than a node a
/// plugin keyed to look like one — a splice point is written by the shell
/// into the host's tree, so it never has a contribution above it.
fn spliced_contribution(id: &str) -> Option<ContributionId> {
    let mut found = None;
    for segment in segments(id) {
        if let Some(id) = ContributionId::parse(segment) {
            if found.is_some() {
                // A contribution inside a contribution: a plugin's own key.
                return None;
            }
            found = Some(id);
        } else if found.is_some() {
            // The contribution key was an ancestor, not this node's own.
            return None;
        }
    }
    found
}

/// The escaped segments of the canonical id `id`, root first.
///
/// Byte-address arithmetic over `crate::tree::KeyPath::id`'s own output, on
/// the same rule `crate::tree::KeyPath::ancestor_ids` scans by: an id is `/`
/// followed by escaped segments, `Key::escaped` doubles every literal `\` and
/// prefixes every literal `/` with `\`, so a `/` byte reached while not
/// consuming the second half of an escape is always a real separator. Escapes
/// are stepped over in pairs, so a key holding its own `/` cannot forge a
/// segment boundary — which is what stops a node keyed `a/ui:7` from reading
/// as contribution 7's. `\` and `/` are ASCII and no UTF-8 continuation byte
/// can equal one, so every boundary found is a `char` boundary.
///
/// Segments come back still escaped. A contribution key never contains an
/// escape, so [`ContributionId::parse`] compares against the escaped text
/// directly; a caller wanting the author's original text would have to
/// unescape, and none here does.
///
/// A malformed id — empty, or missing the leading `/` — has no segments,
/// matching `ancestor_ids`, which returns no ancestors for one. `/` alone is
/// the root path and likewise has none.
/// `segments_agree_with_the_key_path_scanner` pins the two against drift.
fn segments(id: &str) -> Segments<'_> {
    let at = if id == "/" || !id.starts_with('/') {
        // Past the end: the iterator is finished before it starts.
        id.len() + 1
    } else {
        1
    };
    Segments { id, at }
}

/// [`segments`]'s iterator.
///
/// Forward only. An escape is only resolvable left to right — scanning
/// backwards would read the `/` in `a\/b` as a boundary — so there is no
/// honest `next_back`, and callers that want the last segment run the scan to
/// the end with `Iterator::last`.
struct Segments<'a> {
    id: &'a str,
    /// Byte index the next segment starts at; one past the end when done.
    at: usize,
}

impl<'a> Iterator for Segments<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.at > self.id.len() {
            return None;
        }
        let bytes = self.id.as_bytes();
        let start = self.at;
        let mut i = start;
        while i < bytes.len() {
            match bytes[i] {
                // A lone trailing `\` is a malformed escape: one byte, not
                // two, rather than reading past the end.
                b'\\' if i + 1 < bytes.len() => i += 2,
                b'\\' => i += 1,
                b'/' => break,
                _ => i += 1,
            }
        }
        let end = i.min(self.id.len());
        self.at = end + 1;
        Some(&self.id[start..end])
    }
}

#[cfg(test)]
mod tests {
    use super::{ContributionId, contribution_key, owner_chain, owner_of, segments};
    use crate::tree::{Key, KeyPath};

    /// The id of a node under `path` with the given keys appended.
    fn id_of(keys: &[&str]) -> String {
        let mut path = KeyPath::root();
        for key in keys {
            path.push(Key::new(*key));
        }
        path.id()
    }

    #[test]
    fn a_contribution_key_is_the_operator_facing_name() {
        let id = ContributionId::new(7);
        assert_eq!(id.to_string(), "ui:7");
        assert_eq!(contribution_key(id).as_str(), "ui:7");
        assert_eq!(contribution_key(id).escaped(), "ui:7", "needs no escaping");
        assert_eq!(ContributionId::parse("ui:7"), Some(id));
        assert_eq!(id.get(), 7);
    }

    #[test]
    fn a_contribution_id_serializes_as_its_name() {
        let json = serde_json::to_string(&ContributionId::new(7)).expect("an id serializes");
        assert_eq!(json, "\"ui:7\"");
    }

    /// One id, one spelling. Two spellings would be two keys, and two keys
    /// under one slot are two nodes.
    #[test]
    fn only_the_canonical_spelling_names_a_contribution() {
        for spelling in [
            "ui:",
            "ui:007",
            "ui:+7",
            "ui:-7",
            "ui:7x",
            "ui: 7",
            "uid:7",
            "ui",
            "7",
            "",
            "ui:18446744073709551616",
        ] {
            assert_eq!(
                ContributionId::parse(spelling),
                None,
                "{spelling:?} must not name a contribution"
            );
        }
        assert_eq!(ContributionId::parse("ui:0"), Some(ContributionId::new(0)));
        assert_eq!(
            ContributionId::parse("ui:18446744073709551615"),
            Some(ContributionId::new(u64::MAX))
        );
    }

    /// The scanner here and `KeyPath::ancestor_ids` read the same escape
    /// rule. They are two implementations, so a test holds them together:
    /// every ancestor id must be a segment boundary prefix of the id.
    #[test]
    fn segments_agree_with_the_key_path_scanner() {
        for keys in [
            vec!["root"],
            vec!["root", "ui:7"],
            vec!["root", "ui:7", "a", "b"],
            vec!["a/b", "ui:7"],
            vec!["a\\", "ui:7"],
            vec!["a\\b", "c/d", "ui:9"],
            vec![""],
            vec!["root", ""],
        ] {
            let id = id_of(&keys);
            let walked: Vec<&str> = segments(&id).collect();
            let ancestors = KeyPath::ancestor_ids(&id);
            if id == "/" {
                assert!(walked.is_empty(), "the root path names no segment");
                continue;
            }
            assert_eq!(
                walked.len(),
                ancestors.len() + 1,
                "{id:?}: one more segment than ancestors"
            );
            // Each ancestor id is this id cut at a segment boundary, so the
            // ancestor's own last segment must be the segment this scanner
            // yielded at that depth.
            for (depth, ancestor) in ancestors.iter().rev().enumerate() {
                let rebuilt: String = walked[..=depth]
                    .iter()
                    .map(|segment| format!("/{segment}"))
                    .collect();
                assert_eq!(&rebuilt, ancestor, "{id:?} at depth {depth}");
            }
            let unescaped: Vec<String> = keys
                .iter()
                .map(|key| Key::new(*key).escaped())
                .collect::<Vec<_>>();
            assert_eq!(walked, unescaped, "{id:?}: segments are the escaped keys");
        }
    }

    #[test]
    fn a_host_authored_node_has_no_owner() {
        assert_eq!(owner_of(&id_of(&["root", "bar", "clock"])), None);
        assert_eq!(owner_chain(&id_of(&["root", "bar", "clock"])), Vec::new());
        assert_eq!(owner_of(""), None, "a malformed id names no owner");
        assert_eq!(owner_of("/"), None, "the root path names no owner");
        assert_eq!(owner_of("root/ui:7"), None, "an id starts with `/`");
    }

    #[test]
    fn every_node_under_a_contribution_names_it() {
        let id = id_of(&["shell", "status-bar", "ui:7", "row", "label"]);
        assert_eq!(owner_of(&id), Some(ContributionId::new(7)));
        assert_eq!(
            owner_of(&id_of(&["shell", "status-bar", "ui:7"])),
            Some(ContributionId::new(7)),
            "the splice point is owned by the contribution it is"
        );
    }

    /// A plugin keying one of its own nodes `ui:9` must not hand that node,
    /// or anything under it, to contribution 9. The shell wrote the outermost
    /// key and nothing else, so the outermost key is the answer.
    #[test]
    fn a_key_forged_inside_a_contribution_reattributes_nothing() {
        let id = id_of(&["shell", "ui:7", "panel", "ui:9", "row"]);
        assert_eq!(
            owner_of(&id),
            Some(ContributionId::new(7)),
            "still seven's node: seven authored the key that says otherwise"
        );
        assert_eq!(
            owner_chain(&id),
            vec![ContributionId::new(7), ContributionId::new(9)],
            "the chain is the evidence that a forgery was attempted"
        );
    }

    /// A key holding a separator escapes, so it cannot forge a segment. This
    /// is the same guarantee `separators_in_keys_cannot_forge_a_path` makes
    /// for ids, read through attribution: a plugin cannot name a key that
    /// makes its node read as another contribution's.
    #[test]
    fn a_separator_in_a_key_cannot_forge_a_contribution() {
        assert_eq!(owner_of(&id_of(&["shell", "a/ui:7"])), None);
        assert_eq!(owner_of(&id_of(&["shell", "a/ui:7", "row"])), None);
        // A key ending in a backslash is escaped to a pair, so the `/` after
        // it is still a real boundary and the contribution after it is real.
        assert_eq!(
            owner_of(&id_of(&["shell", "a\\", "ui:7"])),
            Some(ContributionId::new(7)),
            "an escaped backslash does not swallow the next separator"
        );
    }
}
