//! Frame identity: the canonical serialization and its BLAKE3 digest.
//!
//! `contracts/frame-identity.md` is binding here. The digest covers placements
//! and paint state, never pixels and never anything that varies between two
//! runs of the same inputs — no clock, no timing, no address, no iteration
//! order of a hash map.

use crate::frame::placement::{PaintContent, PaintState, Placement, PlacementSemantics, TextPaint};
use crate::frame::rounding::round_rect;
use crate::frame::viewport::Viewport;

/// Domain separation. A digest computed under a different prefix can never
/// collide with one computed under this prefix, so the version bump that a
/// serialization change requires cannot be forgotten quietly.
///
/// `v3` covers [`PlacementSemantics::focused`]: `gorgon-petra-egui` paints a
/// focus ring, so two frames differing only in which node holds keyboard focus
/// are two different pictures, and a screenshot consumer verifying
/// `(seq, digest)` would otherwise accept the wrong image. `v2` covered the
/// paint payload — token bindings, typography, wrap policy, line cap, image
/// source, custom painter name — through [`PaintState::paint_hash`]. `v1`
/// covered only the text content hash, the truncation flag, and the theme
/// revision, so two frames that bound the same node's `background` to two
/// different colours shared one digest.
pub const DOMAIN: &[u8] = b"gorgon-petra-frame-v3";

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
/// bumping this alongside it would restate every paint hash under a version
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

/// The byte stream the digest hashes.
///
/// Every field is length-prefixed, so no value can impersonate a field
/// boundary. A node id contains `/` and a label can contain anything at all;
/// a separator-delimited form would let one label forge a whole placement.
#[must_use]
pub fn canonical_bytes(viewport: &Viewport, placements: &[Placement]) -> Vec<u8> {
    let mut w = Canonical::new();
    w.bytes(DOMAIN);

    // 1. Viewport.
    w.text(&canonical_decimal(viewport.size.w));
    w.text(&canonical_decimal(viewport.size.h));
    w.text(&canonical_decimal(viewport.scale.factor()));
    w.u64(viewport.theme_rev);
    w.text(viewport.theme_mode.as_str());

    // 2 and 3. Placements in tree pre-order, geometry then paint state.
    w.u64(placements.len() as u64);
    for p in placements {
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
            // Destructured with no rest pattern for the same reason as the
            // two structs above. `focused` is the one member that decides a
            // picture: `gorgon-petra-egui` paints a focus ring from it, so
            // two frames that differ only in which node is focused are two
            // different pictures. The rest is accessibility payload no
            // shipped painter reads, and the semantic tree carries its own
            // `frame_seq` binding (`contracts/semantic-tree.md`). A renderer
            // that styles from `disabled` or `selected` makes one of those a
            // defect, and the fix is the shape of this one: hash it and bump
            // `DOMAIN`.
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
            // Redundant with `id`, which is the full key path.
            parent: _,
        } = p;

        w.text(id);
        w.text(kind.as_str());
        let rect = round_rect(*rect, viewport.scale);
        w.i32(rect.x);
        w.i32(rect.y);
        w.i32(rect.w);
        w.i32(rect.h);
        w.i32(*z);
        let clip = round_rect(*clip, viewport.scale);
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
    }
    w.finish()
}

/// The digest of one petrified frame.
#[must_use]
pub fn digest(viewport: &Viewport, placements: &[Placement]) -> FrameDigest {
    FrameDigest(*blake3::hash(&canonical_bytes(viewport, placements)).as_bytes())
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

    use super::{canonical_bytes, canonical_decimal, digest, hash_paint_content, hash_text};
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
        let ps = vec![
            placement("/root", Rect::new(0.0, 0.0, 1280.0, 800.0)),
            placement("/root/title", Rect::new(8.0, 8.0, 200.0, 24.0)),
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
            parent: Some(0),
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
    /// when a field appears, and `canonical_bytes` destructures `Placement`
    /// and `PaintState` with no rest pattern, so an unhashed new field does
    /// not compile either.
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

        let mut reparented = base.clone();
        reparented.parent = Some(41);
        assert_eq!(digest(&vp, &[reparented]), baseline);
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
    #[test]
    fn field_boundaries_cannot_be_forged() {
        let vp = viewport();
        let a = vec![placement("/a", Rect::ZERO), placement("/bc", Rect::ZERO)];
        let b = vec![placement("/ab", Rect::ZERO), placement("/c", Rect::ZERO)];
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
        let bytes = canonical_bytes(&viewport(), &[]);
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
            b"gorgon-petra-frame-v3",
            "the frame prefix moved without the vectors below moving with it"
        );
        assert_eq!(super::PAINT_DOMAIN, b"gorgon-petra-paint-v2");

        assert_eq!(
            hash_paint_content(&rich_content()),
            0xca56_af4f_9537_82db,
            "the paint payload stream changed; see this test's doc comment"
        );

        let vp = viewport();
        assert_eq!(
            digest(&vp, &[]).hex(),
            "bc57d6803ce15c12fd2fe790b3e96151cbe9120524d3c07ea3e35355fb9a1261",
            "the empty-frame stream changed; see this test's doc comment"
        );
        assert_eq!(
            digest(&vp, &[rich_placement()]).hex(),
            "23a2ce997c5b62c503e28b42b027dd9feb2571ad28ada482e12e0aee67f51da0",
            "the placement stream changed; see this test's doc comment"
        );
    }
}
