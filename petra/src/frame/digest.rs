//! Frame identity: the canonical serialization and its BLAKE3 digest.
//!
//! `contracts/frame-identity.md` is binding here. The digest covers placements
//! and paint state, never pixels and never anything that varies between two
//! runs of the same inputs — no clock, no timing, no address, no iteration
//! order of a hash map.

use crate::frame::placement::Placement;
use crate::frame::rounding::round_rect;
use crate::frame::viewport::Viewport;

/// Domain separation. A digest computed under a different prefix can never
/// collide with one computed under this prefix, so the version bump that a
/// serialization change requires cannot be forgotten quietly.
pub const DOMAIN: &[u8] = b"gorgon-petra-frame-v1";

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
    let hash = blake3::hash(text.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(bytes)
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
        w.text(&p.id);
        w.text(p.kind.as_str());
        let rect = round_rect(p.rect, viewport.scale);
        w.i32(rect.x);
        w.i32(rect.y);
        w.i32(rect.w);
        w.i32(rect.h);
        w.i32(p.z);
        let clip = round_rect(p.clip, viewport.scale);
        w.i32(clip.x);
        w.i32(clip.y);
        w.i32(clip.w);
        w.i32(clip.h);
        w.text(&canonical_decimal(p.opacity));
        w.u64(p.paint.content_hash);
        w.bool(p.paint.truncated);
        w.u64(p.paint.token_revision);
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
    use super::{canonical_bytes, canonical_decimal, digest, hash_text};
    use crate::frame::placement::{PaintState, Placement, PlacementSemantics};
    use crate::frame::viewport::Viewport;
    use crate::geom::{Rect, Scale, Size};
    use crate::token::ThemeMode;
    use crate::tree::NodeKind;

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

    #[test]
    fn truncation_and_token_revision_are_digest_inputs() {
        let vp = viewport();
        let base = placement("/root", Rect::new(0.0, 0.0, 10.0, 10.0));
        let mut truncated = base.clone();
        truncated.paint.truncated = true;
        let mut retokened = base.clone();
        retokened.paint.token_revision = 8;
        assert_ne!(digest(&vp, std::slice::from_ref(&base)), digest(&vp, &[truncated]));
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
}
