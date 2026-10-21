//! Image sources for `PaintContent.image` (FR-059 / T081).
//!
//! `PaintContent` carries an image *source name*, not pixels — Petra's
//! engine has no opinion about where a picture comes from (D-069), the same
//! reason [`crate::paint`]'s custom-kind dispatch takes a name rather than a
//! drawing. This module is the one place in the crate that turns such a name
//! into something [`egui::Painter::image`] can draw, and it does that behind
//! a cache keyed by the source name so a picture painted every frame is
//! uploaded to the GPU exactly once.
//!
//! # Why this registry takes pixels, not files
//!
//! A registered source is **already-decoded** RGBA8 pixel data
//! ([`ImagePixels`]), not PNG/JPEG bytes. Format decoding is a dependency
//! this crate does not carry — `gorgon-petra-egui`'s own manifest lists only
//! `egui`/`eframe` and `gorgon-petra` — and `egui::ColorImage`'s own
//! constructor for raw pixels
//! (`ColorImage::from_rgba_unmultiplied`) is already the whole of the
//! "decode" step flat pixels need. A host that has an encoded file decodes
//! it with whatever crate it already depends on (the driver, an asset
//! pipeline, `eframe`'s own icon loading) and registers the result once;
//! this module owns the upload and the cache, not the codec.

use std::collections::BTreeMap;

use egui::{ColorImage, Context, Rect, TextureHandle, TextureOptions, Vec2};

/// Raw RGBA8 pixels a host registers under a source name.
///
/// Straight (unassociated) alpha, row-major, top-to-bottom — the exact
/// layout `ColorImage::from_rgba_unmultiplied` expects, so registering a
/// source is handing over bytes a decoder already produced, unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImagePixels {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl ImagePixels {
    /// `rgba` must be exactly `width * height * 4` bytes. Anything else is
    /// refused here with [`PixelsError`] rather than accepted and silently
    /// never resolving — FR-036's loud failure at registration time, per
    /// `contracts/draw-list.md` §9: the infallible constructor is gone, not
    /// kept beside this one.
    pub fn try_new(width: usize, height: usize, rgba: Vec<u8>) -> Result<Self, PixelsError> {
        check_len(width, height, rgba.len())?;
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    /// The byte-count invariant, re-checked at registration. `try_new` is
    /// the only way a caller can build these pixels, so this passes for
    /// anything constructed in public; [`ImageSources::register`] still
    /// checks, because the registry is the loud gate and must refuse a
    /// mismatch even if one reaches it past the constructor.
    fn validate(&self) -> Result<(), PixelsError> {
        check_len(self.width, self.height, self.rgba.len())
    }

    /// Turn these pixels into the type `egui::Context::load_texture` wants,
    /// or `None` when the byte count does not match the declared
    /// dimensions. `try_new` and [`ImageSources::register`] both refuse
    /// those bytes, so this guard only fires for pixels that reached the
    /// registry past both — and when it does, the failure stays the same
    /// shape it has always been: no texture, so [`crate::paint`] names the
    /// source in `undrawn`.
    fn decode(&self) -> Option<ColorImage> {
        let expected = self.width.checked_mul(self.height)?.checked_mul(4)?;
        if self.rgba.len() != expected {
            return None;
        }
        Some(ColorImage::from_rgba_unmultiplied(
            [self.width, self.height],
            &self.rgba,
        ))
    }
}

/// `width * height * 4`, or the [`PixelsError`] naming what disagrees.
fn check_len(width: usize, height: usize, len: usize) -> Result<(), PixelsError> {
    let expected = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4));
    if expected == Some(len) {
        Ok(())
    } else {
        Err(PixelsError { width, height, len })
    }
}

/// A registration refused: the declared dimensions and the supplied byte
/// length disagree, so the bytes cannot be RGBA8 pixels of the size
/// claimed. Carries both numbers so the message names the mismatch
/// wherever the failure surfaces — constructor, registration, or a log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelsError {
    /// The declared width in pixels.
    pub width: usize,
    /// The declared height in pixels.
    pub height: usize,
    /// How many bytes were supplied.
    pub len: usize,
}

impl std::fmt::Display for PixelsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self
            .width
            .checked_mul(self.height)
            .and_then(|pixels| pixels.checked_mul(4))
        {
            Some(expected) => write!(
                f,
                "image source {}x{} RGBA8 is {} bytes, got {}",
                self.width, self.height, expected, self.len
            ),
            None => write!(
                f,
                "image source {}x{} RGBA8 overflows usize, got {} bytes",
                self.width, self.height, self.len
            ),
        }
    }
}

impl std::error::Error for PixelsError {}

/// Host-registered image sources, decoded and uploaded to the GPU behind a
/// source-keyed cache.
///
/// Empty by default: a frame with no registrations resolves nothing, and
/// every image source lands in [`crate::paint::PaintReport::undrawn`] named
/// by itself — the same "declared, not registered, so it is named" shape
/// [`crate::paint::CustomPainters`] uses for `PaintContent.custom`.
#[derive(Default)]
pub struct ImageSources {
    pixels: BTreeMap<String, ImagePixels>,
    uploaded: BTreeMap<String, TextureHandle>,
}

impl ImageSources {
    /// An empty registry: nothing registered, nothing cached.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `source`'s pixel data, refusing a byte-count mismatch with
    /// [`PixelsError`] (FR-036) and leaving the registry untouched: a
    /// refused registration keeps whatever is already registered under the
    /// name — and if nothing is, the name resolves to nothing and
    /// [`crate::paint`] names it in `undrawn`.
    ///
    /// Replacing an existing registration evicts its cached upload: the
    /// next [`ImageSources::resolve`] re-decodes and re-uploads rather than
    /// drawing the picture registered under the old name. Without the
    /// eviction a host that hot-swaps a source (a live chart, a changed
    /// icon) would draw stale pixels forever, since the cache has no other
    /// way to learn the registration changed.
    pub fn register(
        &mut self,
        source: impl Into<String>,
        pixels: ImagePixels,
    ) -> Result<(), PixelsError> {
        pixels.validate()?;
        let source = source.into();
        self.uploaded.remove(&source);
        self.pixels.insert(source, pixels);
        Ok(())
    }

    /// The texture for `source`, uploading it to `ctx` on first use and
    /// returning the cached handle after.
    ///
    /// `None` when nothing is registered under this name, or when the
    /// registered pixels fail to decode — the caller
    /// ([`crate::paint::paint_one`]) is what turns that into an `undrawn`
    /// entry naming the source; this method only ever answers "is there a
    /// texture," never why there is not one.
    pub fn resolve(&mut self, ctx: &Context, source: &str) -> Option<&TextureHandle> {
        if !self.uploaded.contains_key(source) {
            let image = self.pixels.get(source)?.decode()?;
            let handle = ctx.load_texture(source, image, TextureOptions::LINEAR);
            self.uploaded.insert(source.to_owned(), handle);
        }
        self.uploaded.get(source)
    }
}

/// Fit `natural`'s aspect ratio inside `offer`, centred, without stretching
/// — "aspect handling per the offer" (T081): Petra has already decided how
/// much space this node gets ([`gorgon_petra::frame::Placement::rect`]) and
/// this crate does not renegotiate layout from the painter, so a wide
/// picture offered a square rect gets letterboxed rather than squashed.
///
/// Falls back to `offer` unchanged — not a panic, not a zero rect — when
/// either rectangle is degenerate: an offer with no area has nothing to
/// centre inside, and a natural size of zero (an image whose texture
/// reports no pixels) has no ratio to preserve.
#[must_use]
pub fn contain(offer: Rect, natural: Vec2) -> Rect {
    if !offer.is_positive() || natural.x <= 0.0 || natural.y <= 0.0 {
        return offer;
    }
    let offer_ratio = offer.width() / offer.height();
    let natural_ratio = natural.x / natural.y;
    let size = if natural_ratio > offer_ratio {
        // Wider (relative to its own height) than the offer: the width
        // fills the offer and the height is derived from it, leaving
        // letterboxing bands above and below.
        Vec2::new(offer.width(), offer.width() / natural_ratio)
    } else {
        // Taller than the offer, or exactly matching: the height fills the
        // offer and pillarboxing bands land left and right (or neither
        // band exists, when the ratios are equal).
        Vec2::new(offer.height() * natural_ratio, offer.height())
    };
    Rect::from_center_size(offer.center(), size)
}

#[cfg(test)]
mod tests {
    use super::{ImagePixels, ImageSources, PixelsError, contain};
    use egui::{Context, Rect, Vec2, pos2, vec2};

    fn headless() -> Context {
        let ctx = Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        ctx
    }

    /// A tiny 2x1 opaque-red image: the smallest fixture with a
    /// non-square aspect ratio, so tests that care about aspect are not
    /// vacuously true on a 1x1 pixel.
    fn red_pixels(width: usize, height: usize) -> ImagePixels {
        let mut rgba = Vec::with_capacity(width * height * 4);
        for _ in 0..(width * height) {
            rgba.extend_from_slice(&[255, 0, 0, 255]);
        }
        ImagePixels::try_new(width, height, rgba)
            .expect("fixture pixels must match their declared dimensions")
    }

    #[test]
    fn a_registered_source_resolves_to_a_texture_of_its_own_size() {
        let ctx = headless();
        let mut sources = ImageSources::new();
        sources
            .register("logo", red_pixels(2, 1))
            .expect("valid fixture pixels must register");

        let handle = sources
            .resolve(&ctx, "logo")
            .expect("a registered source with valid pixels must resolve");
        assert_eq!(handle.size(), [2, 1]);
    }

    #[test]
    fn an_unregistered_source_does_not_resolve() {
        let ctx = headless();
        let mut sources = ImageSources::new();
        assert!(sources.resolve(&ctx, "nowhere").is_none());
    }

    /// FR-036's refusal at the constructor: pixels whose byte count
    /// disagrees with their declared dimensions are refused by name, not
    /// accepted and silently never resolved.
    #[test]
    fn try_new_rejects_a_wrong_byte_count_with_the_named_error() {
        let err = ImagePixels::try_new(4, 4, vec![0, 0, 0, 0])
            .expect_err("4x4 RGBA8 needs 64 bytes, not 4");
        assert_eq!(
            err,
            PixelsError {
                width: 4,
                height: 4,
                len: 4
            }
        );
        assert_eq!(err.to_string(), "image source 4x4 RGBA8 is 64 bytes, got 4");
        assert!(
            ImagePixels::try_new(2, 1, vec![0; 8]).is_ok(),
            "a matching byte count must construct"
        );
    }

    /// Registration is the loud gate (FR-036): `register` re-checks the
    /// byte count and refuses a mismatch without touching the registry —
    /// an entry already registered under the name keeps drawing.
    #[test]
    fn register_returns_err_for_a_wrong_byte_count() {
        let ctx = headless();
        let mut sources = ImageSources::new();
        sources
            .register("logo", red_pixels(2, 2))
            .expect("valid fixture pixels must register");

        // Built past `try_new` on purpose: field surgery is what makes
        // `register`'s own check reachable, and what proves it is armed.
        let bad = ImagePixels {
            width: 4,
            height: 4,
            rgba: vec![1, 2, 3],
        };
        let err = sources
            .register("logo", bad)
            .expect_err("4x4 RGBA8 needs 64 bytes, not 3");
        assert_eq!(
            err,
            PixelsError {
                width: 4,
                height: 4,
                len: 3
            }
        );
        assert!(
            sources.resolve(&ctx, "logo").is_some(),
            "a refused registration must leave the previous entry drawing"
        );
    }

    /// The one way "decode" can fail: pixel bytes that do not match the
    /// declared dimensions. `try_new` and `register` both refuse those
    /// bytes now; this is the last net for bytes that reach the registry
    /// past both (field surgery, or a future in-crate constructor) — it
    /// must not panic and must not resolve, so `crate::paint` names the
    /// source in `undrawn` exactly as it names an unregistered one.
    #[test]
    fn a_source_with_the_wrong_byte_count_fails_to_resolve() {
        let ctx = headless();
        let mut sources = ImageSources::new();
        // Past `try_new` and `register`, on purpose — see the doc above.
        sources.pixels.insert(
            "broken".into(),
            ImagePixels {
                width: 4,
                height: 4,
                rgba: vec![0, 0, 0, 0],
            },
        );
        assert!(
            sources.resolve(&ctx, "broken").is_none(),
            "4x4 RGBA8 needs 64 bytes, not 4"
        );
    }

    /// The cache, not just the decode: resolving twice must not re-decode.
    /// Proven by corrupting the *registration* after the first resolve and
    /// upload — a resolve that re-checked the pixels every call would start
    /// failing here; the cached handle must not.
    #[test]
    fn a_resolved_source_is_cached_rather_than_re_decoded_every_call() {
        let ctx = headless();
        let mut sources = ImageSources::new();
        sources
            .register("logo", red_pixels(2, 2))
            .expect("valid fixture pixels must register");
        let first_id = sources.resolve(&ctx, "logo").expect("first resolve").id();

        // Reach past `register` (which would evict the cache on purpose)
        // straight into the private field, simulating a resolve call that
        // bypassed decoding because the answer was already cached.
        sources.pixels.insert(
            "logo".into(),
            ImagePixels {
                width: 4,
                height: 4,
                rgba: vec![9; 3],
            },
        );

        let second = sources
            .resolve(&ctx, "logo")
            .expect("the cached upload must still answer, not the corrupted registration");
        assert_eq!(
            second.id(),
            first_id,
            "a cache hit must return the same texture"
        );
    }

    /// Re-registering the same name evicts the old upload: the next
    /// resolve must reflect the new pixels, not the stale texture.
    #[test]
    fn re_registering_a_source_replaces_its_cached_texture() {
        let ctx = headless();
        let mut sources = ImageSources::new();
        sources
            .register("logo", red_pixels(2, 1))
            .expect("valid fixture pixels must register");
        let first = sources.resolve(&ctx, "logo").expect("first").id();

        sources
            .register("logo", red_pixels(4, 4))
            .expect("valid fixture pixels must register");
        let second = sources
            .resolve(&ctx, "logo")
            .expect("re-registered source must still resolve");
        assert_eq!(
            second.size(),
            [4, 4],
            "the new pixels must be what is drawn"
        );
        assert_ne!(
            second.id(),
            first,
            "a fresh upload must not be mistaken for the evicted one"
        );
    }

    // --- contain() ----------------------------------------------------

    #[test]
    fn a_matching_aspect_fills_the_offer_exactly() {
        let offer = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        let fit = contain(offer, vec2(200.0, 100.0));
        assert_eq!(fit, offer);
    }

    #[test]
    fn a_wider_image_is_letterboxed_top_and_bottom() {
        let offer = Rect::from_min_size(pos2(10.0, 10.0), vec2(100.0, 100.0));
        // 2:1, offered a square: width fills, height shrinks to 50.
        let fit = contain(offer, vec2(200.0, 100.0));
        assert!((fit.width() - 100.0).abs() < 1e-4, "{fit:?}");
        assert!((fit.height() - 50.0).abs() < 1e-4, "{fit:?}");
        assert!((fit.center() - offer.center()).length() < 1e-4, "{fit:?}");
        assert!(
            offer.contains_rect(fit),
            "a contained fit must never spill past the offer: {fit:?} vs {offer:?}"
        );
    }

    #[test]
    fn a_taller_image_is_pillarboxed_left_and_right() {
        let offer = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0));
        // 1:2, offered a square: height fills, width shrinks to 50.
        let fit = contain(offer, vec2(100.0, 200.0));
        assert!((fit.width() - 50.0).abs() < 1e-4, "{fit:?}");
        assert!((fit.height() - 100.0).abs() < 1e-4, "{fit:?}");
        assert!(offer.contains_rect(fit), "{fit:?} vs {offer:?}");
    }

    #[test]
    fn a_degenerate_offer_or_natural_size_falls_back_to_the_offer_without_panicking() {
        let offer = Rect::from_min_size(pos2(0.0, 0.0), vec2(80.0, 40.0));
        let zero_offer = Rect::from_min_size(pos2(0.0, 0.0), Vec2::ZERO);
        assert_eq!(contain(zero_offer, vec2(10.0, 10.0)), zero_offer);
        assert_eq!(contain(offer, Vec2::ZERO), offer);
        assert_eq!(contain(offer, vec2(-5.0, 10.0)), offer);
    }
}
