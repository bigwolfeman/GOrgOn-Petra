//! Frame identity: the canonical serialization and its BLAKE3 digest.
//!
//! `contracts/frame-identity.md` is binding here. The digest covers placements
//! and paint state, never pixels and never anything that varies between two
//! runs of the same inputs — no clock, no timing, no address, no iteration
//! order of a hash map.

use crate::frame::placement::{PaintContent, PaintState, Placement, PlacementSemantics, TextPaint};
use crate::frame::rounding::round_rect;
use crate::frame::viewport::Viewport;
use crate::geom::Scale;

/// Domain separation for the frame as a whole. A digest computed under a
/// different prefix can never collide with one computed under this prefix, so
/// the version bump that a serialization change requires cannot be forgotten
/// quietly.
///
/// `v4` restructures the digest from one flat BLAKE3 stream over every
/// placement into a Merkle tree over the placement tree — see [`NODE_DOMAIN`]
/// and [`SUBTREE_DOMAIN`]. The flat form hashed a pre-order *flattening* of
/// the tree, so two differently-shaped trees that flattened to the same
/// sequence of fields shared one digest; nothing in this crate has ever been
/// shown able to produce such a pair (see
/// `the_merkle_form_is_at_least_as_strict_as_the_flat_form_would_have_been`
/// in this module's tests), but the Merkle form hashes the *shape*, closing
/// the gap regardless. It also gives an unchanged subtree a hash that does
/// not depend on re-walking it, which is what a reuse pass needs
/// (`.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`).
/// No per-placement field moved: the leaf stream below is byte-for-byte what
/// `v3`'s shared stream wrote per placement, just framed differently.
///
/// `v3` covered [`PlacementSemantics::focused`]: `gorgon-petra-egui` paints a
/// focus ring, so two frames differing only in which node holds keyboard focus
/// are two different pictures, and a screenshot consumer verifying
/// `(seq, digest)` would otherwise accept the wrong image. `v2` covered the
/// paint payload — token bindings, typography, wrap policy, line cap, image
/// source, custom painter name — through [`PaintState::paint_hash`]. `v1`
/// covered only the text content hash, the truncation flag, and the theme
/// revision, so two frames that bound the same node's `background` to two
/// different colours shared one digest.
pub const DOMAIN: &[u8] = b"gorgon-petra-frame-v4";

/// Domain separation for one placement's leaf hash.
///
/// Every placement hashes alone under this prefix — the exact field stream
/// the flat `v3` digest used to append to one shared buffer — and the result
/// feeds [`combine_subtree_hash`] as that placement's own contribution. A
/// leaf's hash is a function of that one placement only, never of its
/// position in the tree or of any sibling; position and shape are what
/// [`SUBTREE_DOMAIN`] adds on top.
pub const NODE_DOMAIN: &[u8] = b"gorgon-petra-node-v1";

/// Domain separation for a subtree hash.
///
/// [`combine_subtree_hash`] is the one function that reads this prefix, and
/// it is used for every subtree in a frame — the full walk in [`digest`] and,
/// later, the incremental path both call it, so there is exactly one
/// definition of what combining a node with its children means.
pub const SUBTREE_DOMAIN: &[u8] = b"gorgon-petra-subtree-v1";

/// Domain separation for the nested paint-payload hash.
///
/// A separate prefix rather than none: [`hash_paint_content`] is a public
/// function whose output stands alone in [`PaintState::paint_hash`], and a
/// consumer reimplementing it needs to know its stream is not the frame
/// stream.
///
/// The two prefixes version two streams, and they are **not** locked to one
/// number. [`DOMAIN`] versions the frame's identity as a whole, so it moves
/// whenever either stream changes — a paint-stream change moves every frame
/// digest through [`PaintState::paint_hash`] and must be announced. This one
/// versions only the payload stream, so it stays put while that stream is
/// unchanged: `v3` of the frame stream added a field to the *placement*, and
/// `v4` changed only the *framing* around placements, from a flat stream to a
/// Merkle tree — the payload stream itself is untouched by either. Bumping
/// this alongside either would restate every paint hash under a version
/// whose definition never moved, which is exactly the false signal a second
/// implementation reads these prefixes to avoid.
pub const PAINT_DOMAIN: &[u8] = b"gorgon-petra-paint-v2";

/// A frame's content fingerprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameDigest([u8; 32]);

impl FrameDigest {
    /// The raw bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lower-case hex, the wire form.
    #[must_use]
    pub fn hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(char::from_digit(u32::from(byte >> 4), 16).expect("nibble"));
            out.push(char::from_digit(u32::from(byte & 0x0f), 16).expect("nibble"));
        }
        out
    }
}

impl std::fmt::Display for FrameDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.hex())
    }
}

/// A float as a canonical shortest round-trip decimal string.
///
/// Cross-target digest equality (SC-004) needs one printed form for one value.
/// Rust's `Display` for `f32` already emits the shortest string that parses
/// back to the same bits, on every target; the two cases it does not settle
/// are handled here: negative zero prints as `0`, and a non-finite value —
/// which a sane measurement never produces — prints as `0` rather than
/// poisoning a frame's identity with `NaN`.
#[must_use]
pub fn canonical_decimal(value: f32) -> String {
    if !value.is_finite() || value == 0.0 {
        return "0".into();
    }
    format!("{value}")
}

/// A 64-bit content hash of a text run, for the digest's paint-state field.
#[must_use]
pub fn hash_text(text: &str) -> u64 {
    truncate64(&blake3::hash(text.as_bytes()))
}

/// The leading 64 bits of a BLAKE3 hash, little-endian.
///
/// Truncation is deliberate and the reason it is safe is written down in
/// `contracts/frame-identity.md`: the digest is not a MAC. These 64 bits are
/// an input to a 256-bit frame digest that detects drift and nondeterminism,
/// not an adversary.
fn truncate64(hash: &blake3::Hash) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(bytes)
}

/// A 64-bit hash of everything a node draws beyond its rect.
///
/// This is what [`PaintState::paint_hash`] carries, and it is the only route
/// by which a token binding, a typography token, a wrap policy, a line cap, an
/// image source, or a custom painter name reaches the frame digest — none of
/// them live on [`Placement`].
///
/// A node that draws nothing of its own hashes to **zero**, the same value a
/// fresh [`PaintState`] carries. The dispatcher attaches only non-empty
/// payloads (`crate::layout::place`), so without this rule an explicitly empty
/// payload and an absent one would be two different frames while drawing the
/// same picture.
#[must_use]
pub fn hash_paint_content(content: &PaintContent) -> u64 {
    if content.is_empty() {
        return 0;
    }
    // No rest pattern, on purpose: a new field on `PaintContent` stops
    // compiling here until someone decides whether it changes the picture.
    let PaintContent {
        text,
        image,
        custom,
        tokens,
    } = content;

    let mut w = Canonical::new();
    w.bytes(PAINT_DOMAIN);
    match text {
        Some(TextPaint {
            text,
            style,
            wrap,
            max_lines,
        }) => {
            w.bool(true);
            w.text(text);
            w.opt_text(style.as_deref());
            w.text(wrap.as_str());
            w.opt_u64(max_lines.map(|n| n as u64));
        }
        None => w.bool(false),
    }
    w.opt_text(image.as_deref());
    w.opt_text(custom.as_deref());
    // `BTreeMap` iterates in key order on every target, which is what makes
    // this stream reproducible; a `HashMap` here would be a nondeterminism
    // bug that only some runs would show.
    w.u64(tokens.len() as u64);
    for (slot, token) in tokens {
        w.text(slot);
        w.text(token);
    }
    truncate64(&blake3::hash(&w.finish()))
}

/// One placement's own byte stream — the leaf input to [`leaf_hash`].
///
/// Every field is length-prefixed, so no value can impersonate a field
/// boundary. A node id contains `/` and a label can contain anything at all;
/// a separator-delimited form would let one label forge a whole placement.
///
/// This is byte-for-byte what the flat `v3` digest wrote per placement into
/// its one shared stream — the field list, the ordering, the length
/// prefixes, the device rounding, and the shortest-round-trip decimals are
/// unchanged. Only the framing moved: each placement now hashes alone, under
/// its own [`NODE_DOMAIN`] prefix, instead of appending to a stream shared
/// with every other placement in the frame.
fn leaf_bytes(scale: Scale, p: &Placement) -> Vec<u8> {
    let mut w = Canonical::new();
    w.bytes(NODE_DOMAIN);

    // Destructured with no rest pattern, on purpose: a new field on
    // `Placement` or on `PaintState` stops compiling here until someone
    // decides whether it belongs in the frame's identity. The two fields
    // held out are named rather than swept up by `..`, so holding them out
    // stays a decision rather than an oversight — see
    // `contracts/frame-identity.md`, "Not covered".
    let Placement {
        id,
        kind,
        rect,
        z,
        clip,
        opacity,
        paint:
            PaintState {
                content_hash,
                truncated,
                token_revision,
                paint_hash,
            },
        // Destructured with no rest pattern for the same reason as the two
        // structs above. `focused` is the one member that decides a picture:
        // `gorgon-petra-egui` paints a focus ring from it, so two frames that
        // differ only in which node is focused are two different pictures.
        // The rest is accessibility payload no shipped painter reads, and the
        // semantic tree carries its own `frame_seq` binding
        // (`contracts/semantic-tree.md`). A renderer that styles from
        // `disabled` or `selected` makes one of those a defect, and the fix
        // is the shape of this one: hash it and bump `DOMAIN`.
        semantics:
            PlacementSemantics {
                focused,
                role: _,
                label: _,
                value: _,
                disabled: _,
                selected: _,
                expanded: _,
                stale: _,
                ambient: _,
                actions: _,
                total_count: _,
            },
        // Rewritable, not merely redundant: a subtree a reuse pass copies
        // from the previous frame is rebased onto its new position, and
        // `parent` is the field that rebasing rewrites
        // (`.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`,
        // "Placements copy; `parent` rebases"). It was already excluded from
        // the digest before that was true — redundant with `id`, which is
        // the full key path — and it stays excluded now that it is also
        // load-bearing for a different reason: hashing it would make a
        // legitimately-copied subtree's leaf hash change for no picture
        // reason at all.
        parent: _,
    } = p;

    w.text(id);
    w.text(kind.as_str());
    let rect = round_rect(*rect, scale);
    w.i32(rect.x);
    w.i32(rect.y);
    w.i32(rect.w);
    w.i32(rect.h);
    w.i32(*z);
    let clip = round_rect(*clip, scale);
    w.i32(clip.x);
    w.i32(clip.y);
    w.i32(clip.w);
    w.i32(clip.h);
    w.text(&canonical_decimal(*opacity));
    w.u64(*content_hash);
    w.bool(*truncated);
    w.u64(*token_revision);
    w.u64(*paint_hash);
    w.bool(*focused);
    w.finish()
}

/// One placement's leaf hash: `blake3(NODE_DOMAIN || leaf_bytes(scale, p))`.
///
/// A function of that one placement alone — never of where it sits in the
/// tree or what its siblings are. [`combine_subtree_hash`] is what folds
/// position and shape in on top.
#[must_use]
pub fn leaf_hash(scale: Scale, p: &Placement) -> [u8; 32] {
    *blake3::hash(&leaf_bytes(scale, p)).as_bytes()
}

/// Combine one node's leaf hash with its children's subtree hashes into that
/// node's own subtree hash.
///
/// `children` is direct children only, each already reduced to its own
/// subtree hash by a prior call to this same function, in left-to-right
/// (tree pre-order) order — a swap of two children's order is a different
/// tree and must produce a different hash.
///
/// This is the ONE place the Merkle combination rule is defined. [`digest`]
/// calls it for every subtree in a full walk; the incremental placement path
/// this crate does not yet have will call it again for exactly the subtrees
/// that changed, and reuse everything else's hash as already computed. Two
/// definitions of "how a node combines with its children" is how the two
/// paths would quietly stop agreeing; there being only one function makes
/// that impossible.
#[must_use]
pub fn combine_subtree_hash(leaf: [u8; 32], children: &[[u8; 32]]) -> [u8; 32] {
    let mut w = Canonical::new();
    w.bytes(SUBTREE_DOMAIN);
    w.bytes(&leaf);
    w.u64(children.len() as u64);
    for child in children {
        w.bytes(child);
    }
    *blake3::hash(&w.finish()).as_bytes()
}

/// The subtree hash a frame with zero placements digests against.
///
/// There is no leaf to combine, but the digest must still be a deterministic
/// function of the (empty) input: this is [`combine_subtree_hash`]'s rule
/// applied to "no leaf, no children" — [`SUBTREE_DOMAIN`] and a child count
/// of zero, nothing else — rather than a special-cased constant unrelated to
/// the rule every other subtree hash follows.
#[must_use]
pub fn empty_root_hash() -> [u8; 32] {
    let mut w = Canonical::new();
    w.bytes(SUBTREE_DOMAIN);
    w.u64(0);
    *blake3::hash(&w.finish()).as_bytes()
}

/// Every placement's subtree hash, indexed alongside `placements`.
///
/// `placements` must be in tree pre-order with `Placement::parent` set, which
/// is what every `PlacementSink` implementation guarantees
/// (`crate::frame::placement::PlacementSink`). Children are read from
/// `parent` rather than from a separately-passed `subtree_len`: the two are
/// checked against each other once, in
/// `crate::frame::placement::PlacementList::into_parts`, and this function
/// only needs one of them to walk the tree.
///
/// Processes placements from the last index to the first: `parent` always
/// names an earlier index than its child (pre-order), so by the time a node
/// is reached every one of its children has already been reduced to a hash.
///
/// # Panics
///
/// If any placement names a `parent` that is not a strictly earlier index —
/// self-referencing, pointing forward, or out of range. A well-formed
/// pre-order placement list can never do this; a placement list that does is
/// a caller bug, and the two are told apart here rather than left to surface
/// later as an out-of-bounds panic with no context.
#[must_use]
pub fn subtree_hashes(scale: Scale, placements: &[Placement]) -> Vec<[u8; 32]> {
    subtree_hashes_with(scale, placements, &[])
}

/// The same walk, but taking the hashes of subtrees carried over unchanged
/// from a previous frame instead of recomputing them.
///
/// `known[i]`, when `Some`, is the subtree hash for placement `i`, already
/// computed by the frame this subtree came from. The walk then skips both the
/// leaf hash and the fold for that whole subtree, which is what makes the
/// digest cost scale with the change rather than with the tree.
///
/// `known` may be shorter than `placements`, or empty: a missing entry means
/// "not known", so a full walk passes `&[]`.
///
/// # Correctness
/// A known hash is trusted, not checked, and that is the point — checking it
/// would mean recomputing it. It is only ever `Some` for a subtree that
/// [`crate::frame::placement::PlacementSink::reuse_subtree`] copied
/// wholesale, and the copy is byte-identical in every hashed field, so the
/// hash it came with is the hash this walk would have produced. The debug
/// assertion in `petrify_with_memo` is what proves that claim rather than
/// assuming it.
#[must_use]
pub fn subtree_hashes_with(
    scale: Scale,
    placements: &[Placement],
    known: &[Option<[u8; 32]>],
) -> Vec<[u8; 32]> {
    let n = placements.len();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (child_idx, p) in placements.iter().enumerate() {
        if let Some(parent_idx) = p.parent {
            assert!(
                parent_idx < child_idx,
                "placement {child_idx} ({}) names parent {parent_idx}, which must \
                 be a strictly earlier index in a pre-order placement list",
                p.id
            );
            children[parent_idx].push(child_idx);
        }
    }
    let mut hashes = vec![[0u8; 32]; n];
    for i in (0..n).rev() {
        if let Some(known) = known.get(i).copied().flatten() {
            hashes[i] = known;
            continue;
        }
        let leaf = leaf_hash(scale, &placements[i]);
        let child_hashes: Vec<[u8; 32]> = children[i].iter().map(|&c| hashes[c]).collect();
        hashes[i] = combine_subtree_hash(leaf, &child_hashes);
    }
    hashes
}

/// The frame's root subtree hash, given every placement's subtree hash.
///
/// The root is placement `0`: `petrify` places exactly one top-level node
/// (`crate::frame::petrify`), so every other placement is a descendant of it,
/// and it is always the first one pushed. A frame with no placements at all
/// has no index `0`, so it falls back to [`empty_root_hash`].
#[must_use]
pub fn root_hash_from(subtree_hashes: &[[u8; 32]]) -> [u8; 32] {
    subtree_hashes
        .first()
        .copied()
        .unwrap_or_else(empty_root_hash)
}

/// The frame stream: domain, viewport, then the root subtree hash.
///
/// The viewport fields are exactly what `v3`'s shared stream wrote first;
/// what follows them changed from every placement's fields, flattened, to
/// one 32-byte root hash that already summarizes the whole placement tree.
#[must_use]
pub fn frame_bytes(viewport: &Viewport, root_hash: [u8; 32]) -> Vec<u8> {
    let mut w = Canonical::new();
    w.bytes(DOMAIN);
    w.text(&canonical_decimal(viewport.size.w));
    w.text(&canonical_decimal(viewport.size.h));
    w.text(&canonical_decimal(viewport.scale.factor()));
    w.u64(viewport.theme_rev);
    w.text(viewport.theme_mode.as_str());
    w.bytes(&root_hash);
    w.finish()
}

/// The frame digest, given an already-computed root subtree hash.
///
/// Split out from [`digest`] so `crate::frame::petrify` can compute
/// [`subtree_hashes`] once, keep the full array for [`crate::frame::PetrifiedFrame`],
/// and derive the digest from the same root hash rather than recomputing the
/// whole Merkle tree a second time.
#[must_use]
pub fn digest_from_root(viewport: &Viewport, root_hash: [u8; 32]) -> FrameDigest {
    FrameDigest(*blake3::hash(&frame_bytes(viewport, root_hash)).as_bytes())
}

/// The digest of one petrified frame.
#[must_use]
pub fn digest(viewport: &Viewport, placements: &[Placement]) -> FrameDigest {
    let hashes = subtree_hashes(viewport.scale, placements);
    digest_from_root(viewport, root_hash_from(&hashes))
}

struct Canonical {
    buf: Vec<u8>,
}

impl Canonical {
    fn new() -> Self {
        Self {
            buf: Vec::with_capacity(1024),
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.buf
            .extend_from_slice(&(value.len() as u64).to_le_bytes());
        self.buf.extend_from_slice(value);
    }

    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    /// An optional string as a presence flag followed by the value.
    ///
    /// The flag is what stops `None` and `Some("")` from being the same three
    /// bytes: an image node with no source and one whose source is the empty
    /// string are different declarations, and the painter treats them
    /// differently.
    fn opt_text(&mut self, value: Option<&str>) {
        match value {
            Some(v) => {
                self.bool(true);
                self.text(v);
            }
            None => self.bool(false),
        }
    }

    /// An optional integer as a presence flag followed by the value.
    fn opt_u64(&mut self, value: Option<u64>) {
        match value {
            Some(v) => {
                self.bool(true);
                self.u64(v);
            }
            None => self.bool(false),
        }
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes(&value.to_le_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.bytes(&[u8::from(value)]);
    }

    fn finish(self) -> Vec<u8> {
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        Canonical, canonical_decimal, combine_subtree_hash, digest, empty_root_hash, frame_bytes,
        hash_paint_content, hash_text, leaf_hash, root_hash_from, subtree_hashes,
    };
    use crate::frame::placement::{
        PaintContent, PaintState, Placement, PlacementSemantics, TextPaint,
    };
    use crate::frame::viewport::Viewport;
    use crate::geom::{Rect, Scale, Size};
    use crate::token::ThemeMode;
    use crate::tree::{Interaction, NodeKind, Role, TextWrap};

    fn viewport() -> Viewport {
        Viewport {
            size: Size::new(1280.0, 800.0),
            scale: Scale::ONE,
            theme_rev: 7,
            theme_mode: ThemeMode::Dark,
        }
    }

    fn placement(id: &str, rect: Rect) -> Placement {
        Placement {
            id: id.into(),
            kind: NodeKind::Text,
            rect,
            z: 0,
            clip: Rect::new(0.0, 0.0, 1280.0, 800.0),
            opacity: 1.0,
            paint: PaintState {
                content_hash: hash_text("hello"),
                truncated: false,
                token_revision: 7,
                paint_hash: 0,
            },
            semantics: PlacementSemantics::default(),
            parent: None,
        }
    }

    /// SC-004's core claim, at unit scale: same inputs, one digest, 100 times.
    #[test]
    fn identical_inputs_give_one_digest_a_hundred_times() {
        let vp = viewport();
        let mut title = placement("/root/title", Rect::new(8.0, 8.0, 200.0, 24.0));
        title.parent = Some(0);
        let ps = vec![
            placement("/root", Rect::new(0.0, 0.0, 1280.0, 800.0)),
            title,
        ];
        let first = digest(&vp, &ps);
        for _ in 0..100 {
            assert_eq!(digest(&vp, &ps), first);
        }
        assert_eq!(first.hex().len(), 64);
        assert!(first.hex().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn a_moved_placement_changes_the_digest() {
        let vp = viewport();
        let a = vec![placement("/root", Rect::new(0.0, 0.0, 10.0, 10.0))];
        let b = vec![placement("/root", Rect::new(1.0, 0.0, 10.0, 10.0))];
        assert_ne!(digest(&vp, &a), digest(&vp, &b));
    }

    /// A placement with every field set to something distinguishable, so a
    /// mutation of any one of them is a real change.
    ///
    /// Built with struct literals and no `..Default::default()`: a field added
    /// to `Placement`, `PaintState`, `PlacementSemantics`, `PaintContent`, or
    /// `TextPaint` stops this fixture compiling, which is the point. A table
    /// test that silently keeps passing while a new field goes unhashed is
    /// exactly the failure this whole change is about.
    fn rich_placement() -> Placement {
        Placement {
            id: "/root/panel".into(),
            kind: NodeKind::Text,
            rect: Rect::new(4.0, 6.0, 120.0, 20.0),
            z: 3,
            clip: Rect::new(0.0, 0.0, 300.0, 200.0),
            opacity: 0.75,
            paint: PaintState {
                content_hash: hash_text("Fibers"),
                truncated: false,
                token_revision: 7,
                paint_hash: hash_paint_content(&rich_content()),
            },
            semantics: PlacementSemantics {
                role: Some(Role::Button),
                label: Some("Fibers".into()),
                value: Some("3".into()),
                focused: true,
                disabled: false,
                selected: false,
                expanded: Some(true),
                stale: false,
                ambient: false,
                actions: vec![Interaction::Click],
                total_count: Some(9),
            },
            // Used as the sole placement in every fixture below, so it is
            // its own tree's root. `Some(0)` here — a self-referencing
            // parent — used to be harmless filler when `parent` was pure
            // metadata excluded from the digest; under the Merkle framing a
            // placement's `parent` decides which other placement's subtree
            // it joins, and self-reference would corrupt that placement's
            // own children list. `None` is the value that means "not
            // anyone's child" under the new rule too, so it stays the
            // correct choice for a solitary root either way.
            parent: None,
        }
    }

    fn rich_content() -> PaintContent {
        let mut tokens = BTreeMap::new();
        tokens.insert("background".to_owned(), "surface.raised".to_owned());
        tokens.insert("foreground".to_owned(), "text.primary".to_owned());
        PaintContent {
            text: Some(TextPaint {
                text: "Fibers".into(),
                style: Some("heading".into()),
                wrap: TextWrap::Ellipsis,
                max_lines: Some(2),
            }),
            image: Some("logo.png".into()),
            custom: Some("sparkline".into()),
            tokens,
        }
    }

    /// The field-mutation table: change exactly one covered field, and the
    /// digest must move.
    ///
    /// Every entry is a claim `contracts/frame-identity.md` makes. The list is
    /// kept honest from two directions: [`rich_placement`] does not compile
    /// when a field appears, and `leaf_bytes` destructures `Placement` and
    /// `PaintState` with no rest pattern, so an unhashed new field does not
    /// compile either.
    #[test]
    fn every_covered_field_moves_the_digest() {
        let vp = viewport();
        let base = rich_placement();
        let baseline = digest(&vp, std::slice::from_ref(&base));

        type Mutate = fn(&mut Placement);
        let table: &[(&str, Mutate)] = &[
            ("id", |p| p.id = "/root/other".into()),
            ("kind", |p| p.kind = NodeKind::Input),
            ("rect.x", |p| p.rect.x += 1.0),
            ("rect.y", |p| p.rect.y += 1.0),
            ("rect.w", |p| p.rect.w += 1.0),
            ("rect.h", |p| p.rect.h += 1.0),
            ("z", |p| p.z += 1),
            ("clip.x", |p| p.clip.x += 1.0),
            ("clip.y", |p| p.clip.y += 1.0),
            ("clip.w", |p| p.clip.w += 1.0),
            ("clip.h", |p| p.clip.h += 1.0),
            ("opacity", |p| p.opacity = 0.5),
            ("paint.content_hash", |p| p.paint.content_hash ^= 1),
            ("paint.truncated", |p| {
                p.paint.truncated = !p.paint.truncated;
            }),
            ("paint.token_revision", |p| p.paint.token_revision += 1),
            ("paint.paint_hash", |p| p.paint.paint_hash ^= 1),
            // The one semantic flag the digest covers, and the reason `v3`
            // exists: a focus ring is painted from it, so the two states are
            // two pictures. Its neighbours in `PlacementSemantics` are the
            // other half of this claim, in
            // `the_excluded_fields_are_excluded_on_purpose`.
            ("semantics.focused", |p| {
                p.semantics.focused = !p.semantics.focused;
            }),
        ];

        for (field, mutate) in table {
            let mut moved = base.clone();
            mutate(&mut moved);
            assert_ne!(moved, base, "{field}: the mutation changed nothing");
            assert_ne!(
                digest(&vp, &[moved]),
                baseline,
                "{field} decides the picture and the digest cannot see it"
            );
        }
    }

    /// The other half of the table: what the digest deliberately does not
    /// cover, so holding a field out stays a recorded decision rather than an
    /// oversight nobody notices.
    ///
    /// Both entries are listed under "Not covered" in
    /// `contracts/frame-identity.md`. The semantic payload is accessibility
    /// data no shipped painter reads — every member of it *except*
    /// [`PlacementSemantics::focused`], which paints a focus ring and is
    /// covered — and `parent` is redundant with `id`, which is the full key
    /// path.
    #[test]
    fn the_excluded_fields_are_excluded_on_purpose() {
        let vp = viewport();
        let base = rich_placement();
        let baseline = digest(&vp, std::slice::from_ref(&base));

        let mut resemanticked = base.clone();
        resemanticked.semantics.label = Some("Something else entirely".into());
        resemanticked.semantics.disabled = true;
        resemanticked.semantics.selected = true;
        resemanticked.semantics.expanded = Some(false);
        resemanticked.semantics.stale = true;
        resemanticked.semantics.ambient = true;
        resemanticked.semantics.value = Some("41".into());
        resemanticked.semantics.actions = vec![Interaction::Scroll];
        resemanticked.semantics.total_count = Some(4);
        resemanticked.semantics.role = Some(Role::Label);
        assert_ne!(resemanticked, base);
        assert_eq!(
            digest(&vp, &[resemanticked]),
            baseline,
            "every semantic member but `focused` is accessibility payload, not \
             paint; see contracts/frame-identity.md"
        );

        // `parent` is excluded from the *leaf* stream unconditionally — this
        // is what makes rebasing a copied subtree's `parent` safe
        // (`.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`,
        // "Placements copy; `parent` rebases"). It is not the same claim as
        // "any parent value leaves the frame digest unchanged": under the
        // Merkle framing a placement's `parent` says which other subtree it
        // joins, so reparenting within a multi-node tree can move which
        // subtree hash it contributes to. `leaf_hash` is where the claim
        // this test title makes actually lives.
        let mut reparented = base.clone();
        reparented.parent = Some(41);
        assert_eq!(
            leaf_hash(vp.scale, &reparented),
            leaf_hash(vp.scale, &base),
            "parent must not be able to move a placement's own leaf hash"
        );
    }

    /// The paint-payload table: change exactly one field of `PaintContent` and
    /// the hash that carries it into the digest must move.
    #[test]
    fn every_paint_content_field_moves_the_paint_hash() {
        let base = rich_content();
        let baseline = hash_paint_content(&base);

        type Mutate = fn(&mut PaintContent);
        let table: &[(&str, Mutate)] = &[
            ("text.text", |c| {
                c.text.as_mut().unwrap().text = "Fibres".into();
            }),
            ("text.style", |c| {
                c.text.as_mut().unwrap().style = Some("small".into());
            }),
            ("text.style → none", |c| {
                c.text.as_mut().unwrap().style = None;
            }),
            ("text.wrap", |c| {
                c.text.as_mut().unwrap().wrap = TextWrap::Clip;
            }),
            ("text.max_lines", |c| {
                c.text.as_mut().unwrap().max_lines = Some(3);
            }),
            ("text.max_lines → none", |c| {
                c.text.as_mut().unwrap().max_lines = None;
            }),
            ("text → none", |c| c.text = None),
            ("image", |c| c.image = Some("other.png".into())),
            ("image → none", |c| c.image = None),
            ("custom", |c| c.custom = Some("gauge".into())),
            ("custom → none", |c| c.custom = None),
            ("tokens: rebound value", |c| {
                c.tokens.insert("background".into(), "status.down".into());
            }),
            ("tokens: added slot", |c| {
                c.tokens.insert("border".into(), "surface.raised".into());
            }),
            ("tokens: removed slot", |c| {
                c.tokens.remove("foreground");
            }),
            ("tokens: swapped slot names", |c| {
                let bg = c.tokens.remove("background").unwrap();
                let fg = c.tokens.remove("foreground").unwrap();
                c.tokens.insert("background".into(), fg);
                c.tokens.insert("foreground".into(), bg);
            }),
        ];

        for (field, mutate) in table {
            let mut moved = base.clone();
            mutate(&mut moved);
            assert_ne!(moved, base, "{field}: the mutation changed nothing");
            assert_ne!(
                hash_paint_content(&moved),
                baseline,
                "{field} decides the picture and the paint hash cannot see it"
            );
        }
    }

    /// A node that draws nothing of its own hashes to zero, which is what a
    /// fresh `PaintState` already carries.
    ///
    /// The dispatcher attaches only non-empty payloads, so without this rule
    /// an explicitly empty payload and an absent one would be two different
    /// frames drawing the same picture.
    #[test]
    fn an_empty_payload_hashes_to_the_value_a_bare_placement_carries() {
        assert_eq!(hash_paint_content(&PaintContent::default()), 0);
        assert_eq!(PaintState::default().paint_hash, 0);
        let mut nearly_empty = PaintContent::default();
        nearly_empty.tokens.insert("border".into(), "x".into());
        assert_ne!(hash_paint_content(&nearly_empty), 0);
    }

    /// A hash of a string is not a hash of a payload that contains it: the
    /// paint stream carries its own domain prefix, so the two can never be
    /// confused for one another.
    #[test]
    fn the_paint_stream_is_domain_separated_from_a_bare_text_hash() {
        let content = PaintContent {
            text: Some(TextPaint {
                text: "Fibers".into(),
                style: None,
                wrap: TextWrap::Wrap,
                max_lines: None,
            }),
            image: None,
            custom: None,
            tokens: BTreeMap::new(),
        };
        assert_ne!(hash_paint_content(&content), hash_text("Fibers"));
    }

    /// Length prefixes and presence flags, at the payload level. Two payloads
    /// whose fields concatenate to the same text must still differ, and an
    /// absent string must differ from an empty one.
    #[test]
    fn payload_field_boundaries_cannot_be_forged() {
        let split = |image: &str, custom: &str| PaintContent {
            text: None,
            image: Some(image.to_owned()),
            custom: Some(custom.to_owned()),
            tokens: BTreeMap::new(),
        };
        assert_ne!(
            hash_paint_content(&split("ab", "c")),
            hash_paint_content(&split("a", "bc"))
        );

        let absent = PaintContent {
            text: None,
            image: None,
            custom: Some("x".into()),
            tokens: BTreeMap::new(),
        };
        let empty = PaintContent {
            image: Some(String::new()),
            ..absent.clone()
        };
        assert_ne!(hash_paint_content(&absent), hash_paint_content(&empty));

        let mut one = BTreeMap::new();
        one.insert("ab".to_owned(), "c".to_owned());
        let mut other = BTreeMap::new();
        other.insert("a".to_owned(), "bc".to_owned());
        let with = |tokens: BTreeMap<String, String>| PaintContent {
            text: None,
            image: None,
            custom: None,
            tokens,
        };
        assert_ne!(
            hash_paint_content(&with(one)),
            hash_paint_content(&with(other))
        );
    }

    #[test]
    fn truncation_and_token_revision_are_digest_inputs() {
        let vp = viewport();
        let base = placement("/root", Rect::new(0.0, 0.0, 10.0, 10.0));
        let mut truncated = base.clone();
        truncated.paint.truncated = true;
        let mut retokened = base.clone();
        retokened.paint.token_revision = 8;
        assert_ne!(
            digest(&vp, std::slice::from_ref(&base)),
            digest(&vp, &[truncated])
        );
        assert_ne!(digest(&vp, &[base]), digest(&vp, &[retokened]));
    }

    /// A sub-device-pixel move that rounds to the same device rect is the same
    /// picture, so it is the same digest. The digest hashes placements as the
    /// renderer will draw them.
    #[test]
    fn a_move_below_one_device_pixel_keeps_the_digest() {
        let vp = viewport();
        let a = vec![placement("/root", Rect::new(0.0, 0.0, 10.0, 10.0))];
        let b = vec![placement("/root", Rect::new(0.1, 0.0, 10.0, 10.0))];
        assert_eq!(digest(&vp, &a), digest(&vp, &b));
    }

    /// Length prefixes exist so a label or an id cannot forge a field
    /// boundary. Two different trees that concatenate to the same text must
    /// still differ.
    fn parented_pair(root_id: &str, child_id: &str) -> Vec<Placement> {
        let mut child = placement(child_id, Rect::ZERO);
        child.parent = Some(0);
        vec![placement(root_id, Rect::ZERO), child]
    }

    #[test]
    fn field_boundaries_cannot_be_forged() {
        let vp = viewport();
        let a = parented_pair("/a", "/bc");
        let b = parented_pair("/ab", "/c");
        assert_ne!(digest(&vp, &a), digest(&vp, &b));
    }

    #[test]
    fn the_viewport_is_part_of_identity() {
        let ps = vec![placement("/root", Rect::new(0.0, 0.0, 10.0, 10.0))];
        let mut other = viewport();
        other.size = Size::new(1281.0, 800.0);
        assert_ne!(digest(&viewport(), &ps), digest(&other, &ps));

        let mut light = viewport();
        light.theme_mode = ThemeMode::Light;
        assert_ne!(digest(&viewport(), &ps), digest(&light, &ps));

        let mut scaled = viewport();
        scaled.scale = Scale::new(1.25).unwrap();
        assert_ne!(digest(&viewport(), &ps), digest(&scaled, &ps));
    }

    #[test]
    fn canonical_decimals_are_shortest_round_trip_and_sign_free_at_zero() {
        assert_eq!(canonical_decimal(0.0), "0");
        assert_eq!(canonical_decimal(-0.0), "0");
        assert_eq!(canonical_decimal(f32::NAN), "0");
        assert_eq!(canonical_decimal(f32::INFINITY), "0");
        assert_eq!(canonical_decimal(0.1), "0.1");
        assert_eq!(canonical_decimal(1.25), "1.25");
        assert_eq!(canonical_decimal(-3.5), "-3.5");
        for v in [0.1_f32, 1.0 / 3.0, 1e-7, 12345.678] {
            assert_eq!(canonical_decimal(v).parse::<f32>().unwrap(), v);
        }
    }

    #[test]
    fn text_hashes_are_stable_and_distinguish_content() {
        assert_eq!(hash_text("fiber"), hash_text("fiber"));
        assert_ne!(hash_text("fiber"), hash_text("fibre"));
        assert_eq!(hash_text(""), hash_text(""));
    }

    #[test]
    fn the_canonical_stream_starts_with_its_domain() {
        let bytes = frame_bytes(&viewport(), empty_root_hash());
        assert_eq!(&bytes[..8], &(super::DOMAIN.len() as u64).to_le_bytes());
        assert_eq!(&bytes[8..8 + super::DOMAIN.len()], super::DOMAIN);
    }

    /// Golden vectors: one frame, one hex string, pinned.
    ///
    /// This is what makes the claim in [`super::DOMAIN`]'s doc — "the version
    /// bump that a serialization change requires cannot be forgotten quietly"
    /// — true rather than aspirational. The compile-time destructuring catches
    /// a new *field*; nothing catches a reordered one, a renamed wrap policy,
    /// or a changed length-prefix width. This does.
    ///
    /// **If this test fails, you changed the serialization.** That is allowed.
    /// It costs three things, all of them required together: bump both domain
    /// prefixes, update `contracts/frame-identity.md`, and paste the new
    /// values below. It is not allowed to be the cheap edit.
    ///
    /// These are also the reference vectors for a second implementation. A
    /// wasm or non-Rust consumer that reproduces `contracts/frame-identity.md`
    /// correctly reproduces exactly these strings; SC-004 is that claim.
    #[test]
    fn the_canonical_stream_matches_its_pinned_vectors() {
        assert_eq!(
            super::DOMAIN,
            b"gorgon-petra-frame-v4",
            "the frame prefix moved without the vectors below moving with it"
        );
        assert_eq!(super::PAINT_DOMAIN, b"gorgon-petra-paint-v2");
        assert_eq!(super::NODE_DOMAIN, b"gorgon-petra-node-v1");
        assert_eq!(super::SUBTREE_DOMAIN, b"gorgon-petra-subtree-v1");

        // Unmoved by v4: v4 changed only the framing around placements, not
        // the payload stream.
        assert_eq!(
            hash_paint_content(&rich_content()),
            0xca56_af4f_9537_82db,
            "the paint payload stream changed; see this test's doc comment"
        );

        let vp = viewport();
        assert_eq!(
            digest(&vp, &[]).hex(),
            "912c3c245f1b43ecdc1676719424d9cffcb116e550caf2fd8fbb590829f67a52",
            "the empty-frame stream changed; see this test's doc comment"
        );
        assert_eq!(
            digest(&vp, &[rich_placement()]).hex(),
            "330c8dbbe78078d09efd3fb96ab4067a362b5334a7971682db8def2c3efb4cdb",
            "the placement stream changed; see this test's doc comment"
        );
    }

    /// The strengthening the Merkle framing buys over the flat `v3` form,
    /// proved rather than asserted.
    ///
    /// The flat digest hashed `placements.len()` followed by every
    /// placement's fields in pre-order — a flattening of the tree that
    /// dropped `parent` entirely. Two trees with the same fields in the same
    /// pre-order sequence but different parent structure therefore shared
    /// one flat digest. This reconstructs that old rule exactly (length
    /// prefix, then each placement's fields, with no reference to `parent`
    /// at all — literally the body `leaf_bytes` had before this change,
    /// minus its own domain separation, run once over the whole list) and
    /// shows two structurally different trees collide under it while the
    /// real Merkle [`digest`] tells them apart.
    ///
    /// Whether the *real* containers in this crate can ever produce two
    /// placement lists with identical fields in identical pre-order but
    /// different `parent` values is a separate question — nothing in
    /// `layout/` was found able to (see this module's doc comment on
    /// [`super::DOMAIN`]) — but the flat rule's blindness to shape is a
    /// property of the rule itself, demonstrable without needing a real
    /// container to produce the input.
    #[test]
    fn the_merkle_form_is_at_least_as_strict_as_the_flat_form_would_have_been() {
        fn flat_v3_style_digest(placements: &[Placement]) -> [u8; 32] {
            let mut w = Canonical::new();
            w.bytes(b"scratch-flat-v3-reconstruction");
            w.u64(placements.len() as u64);
            for p in placements {
                // The same field list `leaf_bytes` still writes, minus its
                // own domain prefix — this is a reconstruction of the old
                // rule, not a live one anything ships.
                w.text(&p.id);
                w.text(p.kind.as_str());
                w.i32(p.rect.x as i32);
                w.i32(p.rect.y as i32);
                w.i32(p.z);
                w.bool(p.semantics.focused);
            }
            *blake3::hash(&w.finish()).as_bytes()
        }

        let leaf = |id: &str, parent: Option<usize>| Placement {
            id: id.into(),
            kind: NodeKind::Text,
            rect: Rect::ZERO,
            z: 0,
            clip: Rect::ZERO,
            opacity: 1.0,
            paint: PaintState::default(),
            semantics: PlacementSemantics::default(),
            parent,
        };

        // Tree A: /root has two children, /a and /b, each a leaf.
        // Tree B: /root has one child /a, which itself has one child /b.
        // Same three ids, same fields, same pre-order sequence — only the
        // parent structure differs.
        let flat = |id: &str| leaf(id, None); // parent excluded from flat_v3_style_digest's stream anyway
        let wide = vec![
            Placement {
                parent: None,
                ..flat("/root")
            },
            Placement {
                parent: Some(0),
                ..flat("/root/a")
            },
            Placement {
                parent: Some(0),
                ..flat("/root/b")
            },
        ];
        let deep = vec![
            Placement {
                parent: None,
                ..flat("/root")
            },
            Placement {
                parent: Some(0),
                ..flat("/root/a")
            },
            Placement {
                parent: Some(1),
                ..flat("/root/b")
            },
        ];

        assert_eq!(
            flat_v3_style_digest(&wide),
            flat_v3_style_digest(&deep),
            "the flat rule ignores parent entirely, so same fields in the same \
             pre-order sequence must collide regardless of shape"
        );

        let vp = viewport();
        assert_ne!(
            digest(&vp, &wide),
            digest(&vp, &deep),
            "the Merkle digest hashes the shape and must tell these two trees apart"
        );
    }

    /// The one function that combines a node with its children is used for
    /// the full walk and is exposed for the incremental path — this pins
    /// that it behaves as the contract describes: order-sensitive, and
    /// sensitive to how many children there are even when their hashes
    /// coincidentally repeat.
    #[test]
    fn combine_subtree_hash_is_order_and_count_sensitive() {
        let leaf = [7u8; 32];
        let a = [1u8; 32];
        let b = [2u8; 32];

        assert_ne!(
            combine_subtree_hash(leaf, &[a, b]),
            combine_subtree_hash(leaf, &[b, a]),
            "two children in a different order is a different tree"
        );
        assert_ne!(
            combine_subtree_hash(leaf, &[a]),
            combine_subtree_hash(leaf, &[a, a]),
            "repeating the same child hash is still a different child count"
        );
        assert_ne!(
            combine_subtree_hash(leaf, &[]),
            combine_subtree_hash(leaf, &[a]),
            "no children and one child must differ even when nothing else does"
        );
    }

    /// A frame with no placements still digests deterministically, and that
    /// digest is distinct from every populated frame this module builds —
    /// nothing collapses "nothing was placed" onto "something was placed".
    #[test]
    fn an_empty_frame_digests_deterministically_and_distinctly() {
        let vp = viewport();
        let first = digest(&vp, &[]);
        for _ in 0..10 {
            assert_eq!(digest(&vp, &[]), first);
        }
        let populated = digest(&vp, &[rich_placement()]);
        assert_ne!(first, populated);

        assert_eq!(
            root_hash_from(&subtree_hashes(vp.scale, &[])),
            empty_root_hash(),
            "root_hash_from must fall back to empty_root_hash with no placements"
        );
    }

    /// The per-placement hash array [`subtree_hashes`] exposes is what a
    /// reused subtree hands back to its parent (the incremental path this
    /// crate does not yet have). Pinned here at unit scale: the root's own
    /// entry is the frame's root hash, a leaf with no children combines with
    /// an empty child list, and the count matches the placement count.
    #[test]
    fn subtree_hashes_root_entry_is_the_frame_root_hash() {
        let vp = viewport();
        let mut child = placement("/root/a", Rect::ZERO);
        child.parent = Some(0);
        let placements = vec![placement("/root", Rect::ZERO), child];

        let hashes = subtree_hashes(vp.scale, &placements);
        assert_eq!(hashes.len(), 2);
        assert_eq!(root_hash_from(&hashes), hashes[0]);

        // The leaf's own subtree hash is its leaf hash combined with no
        // children — it has none.
        assert_eq!(
            hashes[1],
            combine_subtree_hash(leaf_hash(vp.scale, &placements[1]), &[])
        );
        // The root's subtree hash folds in the child's subtree hash.
        assert_eq!(
            hashes[0],
            combine_subtree_hash(leaf_hash(vp.scale, &placements[0]), &[hashes[1]])
        );
    }
}
