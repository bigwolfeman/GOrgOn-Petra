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
    /// `rgba` must be exactly `width * height * 4` bytes. A source
    /// registered with the wrong length is not rejected here — a
    /// constructor that could fail would force every call site to handle an
    /// error for what is, in practice, an authoring mistake made once — it
    /// simply never resolves; see [`ImageSources::resolve`].
    #[must_use]
    pub fn new(width: usize, height: usize, rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            rgba,
        }
    }

    /// Turn these pixels into the type `egui::Context::load_texture` wants,
    /// or `None` when the byte count does not match the declared
    /// dimensions — the one way this "decode" step can fail, since there is
    /// no format to misparse.
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

    /// Register `source`'s pixel data.
    ///
    /// Replacing an existing registration evicts its cached upload: the
    /// next [`ImageSources::resolve`] re-decodes and re-uploads rather than
    /// drawing the picture registered under the old name. Without the
    /// eviction a host that hot-swaps a source (a live chart, a changed
    /// icon) would draw stale pixels forever, since the cache has no other
    /// way to learn the registration changed.
    pub fn register(&mut self, source: impl Into<String>, pixels: ImagePixels) {
        let source = source.into();
        self.uploaded.remove(&source);
        self.pixels.insert(source, pixels);
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
    use super::{ImagePixels, ImageSources, contain};
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
        ImagePixels::new(width, height, rgba)
    }

    #[test]
    fn a_registered_source_resolves_to_a_texture_of_its_own_size() {
        let ctx = headless();
        let mut sources = ImageSources::new();
        sources.register("logo", red_pixels(2, 1));

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

    /// The one way "decode" can fail: pixel bytes that do not match the
    /// declared dimensions. Registering fewer bytes than `width * height *
    /// 4` demands must not panic and must not resolve.
    #[test]
    fn a_source_with_the_wrong_byte_count_fails_to_resolve() {
        let ctx = headless();
        let mut sources = ImageSources::new();
        sources.register("broken", ImagePixels::new(4, 4, vec![0, 0, 0, 0]));
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
        sources.register("logo", red_pixels(2, 2));
        let first_id = sources.resolve(&ctx, "logo").expect("first resolve").id();

        // Reach past `register` (which would evict the cache on purpose)
        // straight into the private field, simulating a resolve call that
        // bypassed decoding because the answer was already cached.
        sources
            .pixels
            .insert("logo".into(), ImagePixels::new(4, 4, vec![9; 3]));

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
        sources.register("logo", red_pixels(2, 1));
        let first = sources.resolve(&ctx, "logo").expect("first").id();

        sources.register("logo", red_pixels(4, 4));
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
