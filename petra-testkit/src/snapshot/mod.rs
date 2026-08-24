//! `screenshot`: real pixels, headless, bound to one frame's identity.
//!
//! Binding: `contracts/driver-protocol.md` (the `screenshot` verb, "Hosted
//! content", "Capture duty") and `contracts/frame-identity.md` (FR-040 and
//! its two readings of a digest). Task T032.
//!
//! # What "real pixels" means here
//!
//! The bytes in [`Shot::png`] are the output of `egui`'s own tessellator over
//! the shapes Petra's paint path produced in the pass being captured, drawn
//! by `egui_wgpu`'s own render pipeline onto a real `wgpu` device and copied
//! back. Nothing in this module draws a rectangle, re-derives a colour from
//! [`PetrifiedFrame::placements`], or substitutes a placeholder. If the GPU
//! cannot be reached the capture *fails*, naming the adapter error
//! (FR-043: a missing prerequisite is a failure, never a pass).
//!
//! No window, no `DISPLAY`, no `WAYLAND_DISPLAY`: the device is requested
//! with no surface and no display handle. See [`gpu`] for the wgpu detail and
//! for what was read out of `egui_kittest` (R3: "mirror, don't build on") and
//! out of `egui`/`wgpu` themselves, with file and line.
//!
//! # Why this is a struct and not a free function
//!
//! Two pieces of state have to outlive one shot. A `wgpu::Device` costs tens
//! of milliseconds to request, and — the load-bearing half —
//! [`egui_wgpu::Renderer`] is *where uploaded textures live*. A renderer
//! rebuilt per capture would begin every shot with no font atlas, so the
//! second capture would have to re-upload everything the first one did. The
//! caller (`DriverHost`) keeps one [`Snapshotter`] for the process.
//!
//! **One device per process is a requirement, not an optimization.** The
//! first version of this module's tests each built their own
//! [`Snapshotter`], so `cargo test` created five `wgpu` devices at once on a
//! GPU a training run was already holding; all five capture tests parked and
//! none of the three GPU-free ones did. Serialized, the same eight finish in
//! 1.6s. The tests now share one device behind a `Mutex` (see the test
//! module), and every wait this module makes is bounded and names what it
//! was waiting on — a capture that can hang forever is worse than one that
//! fails, because it takes a whole gate lane with it.
//!
//! # What [`Snapshotter::capture`] verifies, and what that is worth (FR-040)
//!
//! Verified, on the server, before any [`Shot`] is returned:
//!
//! 1. **The digest is this frame's own.** It is recomputed with
//!    [`gorgon_petra::frame::digest::digest`] over `(viewport, placements)` and
//!    compared against the recorded [`PetrifiedFrame::digest`]. The pair on
//!    the wire is never simply copied out of the record and trusted.
//! 2. **The digest's paint inputs match what was painted.**
//!    [`PetrifiedFrame::paint_hashes_agree`] checks the per-placement paint
//!    hashes against the frame's content array, so a frame whose payloads
//!    drifted from the hashes that entered the digest is refused rather than
//!    photographed.
//! 3. **The rasterization used this frame's viewport.** `FullOutput`'s
//!    `pixels_per_point` and the `Context`'s content rect are compared
//!    against [`gorgon_petra::frame::Viewport`]'s scale and size. A capture
//!    taken across a resize, or against a frame petrified at another scale,
//!    is refused instead of being returned at the wrong size.
//!
//! **Not** verified, and the honest limits of the claim:
//!
//! * *That these shapes came from petrifying this frame.* `egui::FullOutput`
//!   carries no Petra frame identity — it is a list of shapes and a scale.
//!   The binding is the caller's: `DriverHost::step` hands over the
//!   `FullOutput` and the `PetrifiedFrame` that one and the same `ctx.run`
//!   produced. A caller that paired a stale frame with a fresh output *of
//!   the same size and scale* would get a wrong answer this module cannot
//!   see. Check 3 catches every pairing where the size or scale moved, which
//!   is every resize and every scale change, and nothing else.
//! * *That two shots sharing a digest are the same picture.* They are not,
//!   once hosted content is on screen — `contracts/frame-identity.md`, "Not
//!   covered": a registered painter's *name* is hashed, never its pixels.
//!   That is exactly what [`Shot::hosted`] is for, and it is read from
//!   [`PetrifiedFrame::hosted`], never re-derived here (the same rule
//!   `crate::wire::frame_result` follows).

mod gpu;

use std::fmt;

use egui::{Context, FullOutput};
use gorgon_petra::frame::digest::digest;
use gorgon_petra::frame::{PetrifiedFrame, round_rect};
use gorgon_petra::geom::Rect;

use crate::wire::{ErrorKind, WireRect};

use gpu::Gpu;

/// One identity-verified screenshot.
///
/// The four members the `screenshot` verb's result is built from
/// (`{seq, digest, png_base64, hosted}` — this carries the PNG bytes; base64
/// is the wire's business, not this module's).
#[derive(Clone, PartialEq, Eq)]
pub struct Shot {
    /// The captured frame's sequence, monotone within one application run.
    pub seq: u64,
    /// The captured frame's hex-encoded digest, recomputed and checked
    /// before this value was produced.
    pub digest: String,
    /// PNG bytes of the rasterized frame, cropped to the requested region.
    pub png: Vec<u8>,
    /// Whether any placement in the captured frame was drawn by a
    /// host-registered custom painter (FR-060). Read from
    /// [`PetrifiedFrame::hosted`].
    pub hosted: bool,
}

impl fmt::Debug for Shot {
    /// The PNG is summarized by length: a `Debug` that dumped a megabyte of
    /// compressed pixels into a test failure would bury the three fields
    /// that explain it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shot")
            .field("seq", &self.seq)
            .field("digest", &self.digest)
            .field("png", &format_args!("{} bytes", self.png.len()))
            .field("hosted", &self.hosted)
            .finish()
    }
}

/// Every way a capture can refuse.
///
/// Closed and specific: "capture failed" with a string would make the
/// dispatch mapping below a guess. Every variant either names a missing
/// prerequisite, names a caller mistake, or names an internal disagreement
/// the server found in its own frame record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// No `wgpu` adapter answered. The prerequisite FR-043 says to name.
    NoAdapter(String),
    /// An adapter answered but no device could be created from it.
    NoDevice(String),
    /// The GPU accepted the work and then failed, or the readback did.
    Render(String),
    /// PNG encoding failed.
    Encode(String),
    /// The frame's recorded digest is not the digest of its own placements.
    DigestMismatch {
        /// The frame's sequence.
        seq: u64,
        /// What the frame carries.
        recorded: String,
        /// What its placements actually hash to.
        recomputed: String,
    },
    /// The frame's per-placement paint hashes disagree with its content
    /// array, so the digest describes payloads other than the painted ones.
    PaintHashDrift {
        /// The frame's sequence.
        seq: u64,
    },
    /// The pass being captured did not run at the frame's viewport.
    ViewportMismatch {
        /// The frame's sequence.
        seq: u64,
        /// Logical size and scale the frame was petrified against.
        frame: String,
        /// Logical size and scale the pass actually ran at.
        pass: String,
    },
    /// A requested region has no area.
    EmptyRegion(WireRect),
    /// A requested region is not wholly inside the frame's device rect.
    RegionOutsideViewport {
        /// What was asked for.
        region: WireRect,
        /// What exists, in device pixels.
        viewport: WireRect,
    },
    /// A mesh named a texture this renderer never received the pixels for.
    MissingTexture {
        /// The `epaint::TextureId`, debug-formatted.
        id: String,
    },
    /// The draw list contains an `epaint` paint callback, which this
    /// renderer would silently skip rather than draw.
    PaintCallback,
    /// The frame's viewport rounds to zero device pixels, so there is no
    /// texture to render into.
    EmptyViewport {
        /// The frame's sequence.
        seq: u64,
        /// The frame's device rect, both extents of which cannot be positive.
        viewport: WireRect,
    },
}

impl CaptureError {
    /// The wire error kind this maps to.
    ///
    /// The vocabulary is closed at five (`contracts/driver-protocol.md`) and
    /// deliberately has no internal-error kind, so two of these arms are
    /// judgement calls and are written down as such:
    ///
    /// * **Missing prerequisite → `unsupported`.** "Verb known, feature
    ///   absent — carries why" is exactly a build that cannot reach a GPU,
    ///   or a draw list carrying content this capture path does not render.
    ///   FR-043's rule that a gate names the missing prerequisite is served
    ///   by the message.
    /// * **Caller mistake → `invalid-params`.** A region with no area, or a
    ///   region outside the frame, is a request this frame cannot answer.
    /// * **Server-side disagreement → `timeout`.** A digest that does not
    ///   match its own placements, a paint-hash drift, a viewport that moved
    ///   under the capture, a GPU or encoder failure: the server holds state
    ///   it cannot answer from. `timeout` is the kind
    ///   `crate::server::dispatch` already spends for that case ("the
    ///   published frame has no placements to project a tree from"), and one
    ///   precedent is better than a sixth kind invented here. It is a poor
    ///   fit and it is named as one: closing it is a contract change, not a
    ///   call site's decision.
    #[must_use]
    pub fn wire_kind(&self) -> ErrorKind {
        match self {
            Self::NoAdapter(_)
            | Self::NoDevice(_)
            | Self::MissingTexture { .. }
            | Self::PaintCallback => ErrorKind::Unsupported,
            Self::EmptyRegion(_) | Self::RegionOutsideViewport { .. } => ErrorKind::InvalidParams,
            Self::EmptyViewport { .. } => ErrorKind::Timeout,
            Self::Render(_)
            | Self::Encode(_)
            | Self::DigestMismatch { .. }
            | Self::PaintHashDrift { .. }
            | Self::ViewportMismatch { .. } => ErrorKind::Timeout,
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter(err) => write!(
                f,
                "no wgpu adapter for a headless capture (PRIMARY|GL backends): {err}"
            ),
            Self::NoDevice(err) => write!(f, "no wgpu device from the chosen adapter: {err}"),
            Self::Render(err) => write!(f, "the capture failed on the GPU: {err}"),
            Self::Encode(err) => write!(f, "the capture could not be encoded as PNG: {err}"),
            Self::DigestMismatch {
                seq,
                recorded,
                recomputed,
            } => write!(
                f,
                "frame {seq} carries digest {recorded} but its placements hash to {recomputed}; \
                 refusing to answer a screenshot with an identity that is not the frame's"
            ),
            Self::PaintHashDrift { seq } => write!(
                f,
                "frame {seq}'s placement paint hashes disagree with its paint content, so its \
                 digest describes payloads other than the ones painted"
            ),
            Self::ViewportMismatch { seq, frame, pass } => write!(
                f,
                "frame {seq} was petrified against {frame} but the captured pass ran at {pass}"
            ),
            Self::EmptyRegion(region) => write!(
                f,
                "region {}x{} at ({}, {}) has no area",
                region.w, region.h, region.x, region.y
            ),
            Self::RegionOutsideViewport { region, viewport } => write!(
                f,
                "region {}x{} at ({}, {}) is not inside the frame's {}x{} device rect; \
                 a region outside the viewport is refused, not clamped, because a clamped \
                 crop answers a question the caller did not ask",
                region.w, region.h, region.x, region.y, viewport.w, viewport.h
            ),
            Self::MissingTexture { id } => write!(
                f,
                "the draw list names texture {id}, whose pixels never reached this renderer"
            ),
            Self::PaintCallback => write!(
                f,
                "the draw list contains an epaint paint callback, which this renderer would \
                 skip rather than draw"
            ),
            Self::EmptyViewport { seq, viewport } => write!(
                f,
                "frame {seq} rounds to a {}x{} device rect, which has no pixels to capture",
                viewport.w, viewport.h
            ),
        }
    }
}

impl std::error::Error for CaptureError {}

/// The GPU, brought up at most once, and its outcome remembered.
///
/// Cold until the first capture: a driver server that never takes a
/// screenshot must not pay for a `wgpu` device, and — more to the point — a
/// build on a machine with no GPU must still start, serve `health`, `tree`
/// and `frame`, and fail only the verb that actually needs one.
enum Device {
    Cold,
    Ready(Box<Gpu>),
    Failed(CaptureError),
}

/// Holds the capture device and the textures uploaded to it.
///
/// One per process. See the module docs for why this is not a free function.
pub struct Snapshotter {
    device: Device,
}

impl Default for Snapshotter {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotter {
    /// A snapshotter that has not touched the GPU yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            device: Device::Cold,
        }
    }

    /// The adapter backing this snapshotter, once one has been chosen.
    ///
    /// `None` before the first capture, and after a failed bring-up — in
    /// which case the failure is what the next capture returns, verbatim.
    #[must_use]
    pub fn adapter(&self) -> Option<&str> {
        match &self.device {
            Device::Ready(gpu) => Some(gpu.adapter()),
            Device::Cold | Device::Failed(_) => None,
        }
    }

    /// Apply one pass's texture changes without capturing.
    ///
    /// Optional, and the capture path does not depend on it: text is correct
    /// regardless, because the font atlas is re-uploaded whole on every
    /// capture ([`gpu::Gpu::upload_font_atlas`]). What it buys is *host
    /// images*: `egui::Context::load_texture` hands over a texture's pixels
    /// exactly once, in the `FullOutput` of the pass that loaded it, and
    /// `epaint::TextureManager` keeps no copy
    /// (`epaint-0.36.1/src/textures.rs:126` — `TextureMeta` is metadata
    /// only). A caller that calls this every pass can capture a frame whose
    /// images were registered during a pass nobody captured; a caller that
    /// does not gets [`CaptureError::MissingTexture`] naming the id, rather
    /// than a picture with a hole in it.
    ///
    /// Does nothing before the GPU exists — there is no renderer to hold the
    /// upload, and forcing one into existence here would make an application
    /// that never screenshots pay for a device anyway.
    pub fn absorb(&mut self, delta: &egui::TexturesDelta) {
        if let Device::Ready(gpu) = &mut self.device {
            gpu.absorb(delta);
        }
    }

    /// Capture the pass described by `output`, as the frame `frame`.
    ///
    /// `output` is borrowed, never drained: a real `eframe` window applies
    /// the same `textures_delta` to its own renderer after this returns, and
    /// a capture that stole it would blank the window's fonts. The caller
    /// still owns the delta and still has to apply or discard it.
    ///
    /// `region` is in **device pixels**, the same units and the same
    /// rounding [`crate::wire::WirePlacement::rect`] carries
    /// ([`gorgon_petra::frame::round_rect`]). A region that is not wholly
    /// inside the frame's device rect is [`CaptureError::RegionOutsideViewport`],
    /// not a clamp.
    ///
    /// # Errors
    ///
    /// Every variant of [`CaptureError`]; see its docs and
    /// [`CaptureError::wire_kind`].
    pub fn capture(
        &mut self,
        ctx: &Context,
        output: &FullOutput,
        frame: &PetrifiedFrame,
        region: Option<WireRect>,
    ) -> Result<Shot, CaptureError> {
        // Identity first, and before the GPU is even woken: a frame whose
        // record disagrees with itself must not be rasterized at all, and a
        // caller that asked for an impossible region should learn that
        // without waiting on a device.
        verify_identity(ctx, output, frame)?;
        let full = device_rect(frame);
        if full.w <= 0 || full.h <= 0 {
            return Err(CaptureError::EmptyViewport {
                seq: frame.seq,
                viewport: full,
            });
        }
        let crop = match region {
            Some(region) => Some(check_region(region, full)?),
            None => None,
        };

        let gpu = self.gpu()?;
        gpu.absorb(&output.textures_delta);
        gpu.upload_font_atlas(ctx);

        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        gpu.check_drawable(&primitives)?;
        // Both extents are positive: `round_rect` clamps them to zero at
        // worst, and the zero case was refused above. The casts cannot wrap.
        let raster = gpu.render(
            &primitives,
            output.pixels_per_point,
            full.w as u32,
            full.h as u32,
        )?;
        let raster = match crop {
            Some(rect) => image::imageops::crop_imm(
                &raster,
                rect.x as u32,
                rect.y as u32,
                rect.w as u32,
                rect.h as u32,
            )
            .to_image(),
            None => raster,
        };

        let mut png = Vec::new();
        raster
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|err| CaptureError::Encode(err.to_string()))?;

        Ok(Shot {
            seq: frame.seq,
            digest: frame.digest.hex(),
            png,
            hosted: frame.hosted(),
        })
    }

    /// The device, brought up on first use and remembered either way.
    fn gpu(&mut self) -> Result<&mut Gpu, CaptureError> {
        if matches!(self.device, Device::Cold) {
            self.device = match Gpu::new() {
                Ok(gpu) => Device::Ready(Box::new(gpu)),
                // Remembered, not retried: an adapter that did not answer
                // once will not answer on the next screenshot either, and a
                // driver that re-probed on every request would turn one
                // missing prerequisite into a per-request stall.
                Err(err) => Device::Failed(err),
            };
        }
        match &mut self.device {
            Device::Ready(gpu) => Ok(gpu),
            Device::Failed(err) => Err(err.clone()),
            Device::Cold => unreachable!("the branch above leaves no Cold device"),
        }
    }
}

/// The frame's whole drawable area in device pixels.
///
/// Through [`round_rect`], so the PNG's dimensions are the same device
/// numbers `crate::wire::frame_result` puts on the wire for placements — one
/// rounding rule, spent in one place (`gorgon/petra/src/frame/rounding.rs`).
fn device_rect(frame: &PetrifiedFrame) -> WireRect {
    let size = frame.viewport.size;
    let rect = round_rect(Rect::new(0.0, 0.0, size.w, size.h), frame.viewport.scale);
    WireRect {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    }
}

/// FR-040, server side. See the module docs for what this proves.
fn verify_identity(
    ctx: &Context,
    output: &FullOutput,
    frame: &PetrifiedFrame,
) -> Result<(), CaptureError> {
    let recomputed = digest(&frame.viewport, &frame.placements);
    if recomputed != frame.digest {
        return Err(CaptureError::DigestMismatch {
            seq: frame.seq,
            recorded: frame.digest.hex(),
            recomputed: recomputed.hex(),
        });
    }
    if !frame.paint_hashes_agree() {
        return Err(CaptureError::PaintHashDrift { seq: frame.seq });
    }
    // Exact equality on purpose. Both numbers are produced by the same two
    // expressions `gorgon_petra_egui::host::Host::pass` reads
    // (`Context::pixels_per_point` and `Context::content_rect`), so within
    // one pass they are bit-identical; a tolerance here would only widen the
    // window in which a stale frame passes for a fresh one.
    let screen = ctx.content_rect();
    let scale = frame.viewport.scale.factor();
    let size = frame.viewport.size;
    if output.pixels_per_point != scale || screen.width() != size.w || screen.height() != size.h {
        return Err(CaptureError::ViewportMismatch {
            seq: frame.seq,
            frame: format!("{}x{} at scale {scale}", size.w, size.h),
            pass: format!(
                "{}x{} at scale {}",
                screen.width(),
                screen.height(),
                output.pixels_per_point
            ),
        });
    }
    Ok(())
}

/// Validate a requested crop against the frame's device rect.
fn check_region(region: WireRect, full: WireRect) -> Result<WireRect, CaptureError> {
    if region.w <= 0 || region.h <= 0 {
        return Err(CaptureError::EmptyRegion(region));
    }
    // i64 so a caller-supplied `i32::MAX` width cannot wrap the bound check
    // it is being measured against.
    let right = i64::from(region.x) + i64::from(region.w);
    let bottom = i64::from(region.y) + i64::from(region.h);
    if region.x < 0 || region.y < 0 || right > i64::from(full.w) || bottom > i64::from(full.h) {
        return Err(CaptureError::RegionOutsideViewport {
            region,
            viewport: full,
        });
    }
    Ok(region)
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

    use egui::{Context, FullOutput, Pos2, RawInput};
    use gorgon_petra::frame::PetrifiedFrame;
    use gorgon_petra::frame::digest::digest;
    use gorgon_petra::frame::round_rect;
    use gorgon_petra::geom::Size;
    use gorgon_petra::input::{InputEvent, Route};
    use gorgon_petra::layout::{ChangeSet, RowSource};
    use gorgon_petra::token::{Presenter, Theme, TokenName, dark, light};
    use gorgon_petra::tree::{NodeKind, Props, ViewNode};
    use gorgon_petra_egui::host::{App, Host};
    use gorgon_petra_egui::paint::TokenSource;
    use image::RgbaImage;

    use super::{CaptureError, Shot, Snapshotter};
    use crate::wire::{ErrorKind, WireRect};

    /// A panel that binds a background token and holds one line of text, so
    /// the picture has exactly two ingredients a test can name: a fill in
    /// `surface.base` and glyphs in `text.primary`.
    struct Panel;

    impl RowSource for Panel {
        fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
            Vec::new()
        }
    }

    impl App for Panel {
        fn view(&mut self) -> ViewNode {
            let mut panel = Props::default();
            panel.tokens.insert(
                "background".into(),
                TokenName::new("surface.base").expect("a shipped token name"),
            );
            ViewNode::new(NodeKind::Stack, "root")
                .with_props(panel)
                .child(ViewNode::new(NodeKind::Text, "label").with_props(Props {
                    text: Some("Petra".into()),
                    ..Props::default()
                }))
        }

        fn handle(&mut self, _event: &InputEvent, _route: &Route) {}

        fn take_changes(&mut self) -> ChangeSet {
            ChangeSet::All
        }
    }

    /// Logical window size every test runs at.
    const WINDOW: Size = Size::new(200.0, 120.0);

    fn sized(mut input: RawInput) -> RawInput {
        input.screen_rect = Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(WINDOW.w, WINDOW.h),
        ));
        input
    }

    fn headless() -> Context {
        let ctx = Context::default();
        ctx.run_ui(sized(RawInput::default()), |_| {})
            .drop_without_applying_deltas();
        ctx
    }

    /// One real host pass — input, view, petrify, paint — and what a capture
    /// needs out of it.
    ///
    /// Owns its `FullOutput` and clears the texture delta on drop.
    /// `epaint::TexturesDelta` panics in its own destructor when it is
    /// dropped unapplied (`epaint-0.36.1/src/textures.rs:337`), and a
    /// destructor that panics while a failed assertion is already unwinding
    /// aborts the process — which loses the assertion message that explains
    /// the failure. Every test here would otherwise have to remember to
    /// discard the delta before its first `assert!`.
    struct Pass {
        output: FullOutput,
        frame: PetrifiedFrame,
    }

    impl Drop for Pass {
        fn drop(&mut self) {
            self.output.textures_delta.clear();
        }
    }

    fn one_pass(ctx: &Context, host: &mut Host<Panel>) -> Pass {
        let output = ctx.run_ui(sized(RawInput::default()), |_| host.pass(ctx));
        let frame = host.frame().expect("a pass leaves a frame").clone();
        Pass { output, frame }
    }

    /// A context and a host that have run exactly one pass under `presenter`.
    fn painted(presenter: Presenter) -> (Context, Host<Panel>, Pass) {
        let ctx = headless();
        let mut host = Host::new(&ctx, Panel, presenter);
        let pass = one_pass(&ctx, &mut host);
        (ctx, host, pass)
    }

    /// The one device every test in this module shares.
    ///
    /// Not a convenience. Each test used to build its own [`Snapshotter`],
    /// which meant `cargo test`'s default thread pool asked a busy GPU for
    /// five `wgpu` devices at the same moment; every capture test parked
    /// indefinitely and only the three that never touch the GPU finished.
    /// One device behind a `Mutex` both fixes that and matches how the
    /// driver actually uses this module — one [`Snapshotter`] per process.
    ///
    /// A poisoned lock is taken anyway: the poison means some other test
    /// already failed, and turning that into "poisoned lock" failures in
    /// every remaining test would bury the one message that explains it.
    fn shared() -> MutexGuard<'static, Snapshotter> {
        static SHARED: OnceLock<Mutex<Snapshotter>> = OnceLock::new();
        SHARED
            .get_or_init(|| Mutex::new(Snapshotter::new()))
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Capture, failing the test — never skipping it — when the GPU is not
    /// reachable (FR-043: a missing prerequisite is a failure that names
    /// itself).
    fn shoot(
        snapshotter: &mut Snapshotter,
        ctx: &Context,
        output: &FullOutput,
        frame: &PetrifiedFrame,
        region: Option<WireRect>,
    ) -> Shot {
        snapshotter
            .capture(ctx, output, frame, region)
            .unwrap_or_else(|err| panic!("capture failed: {err}"))
    }

    fn decode(png: &[u8]) -> RgbaImage {
        image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .expect("the capture must be a decodable PNG")
            .to_rgba8()
    }

    /// The colour `token` resolves to under `theme`, as the renderer itself
    /// resolves it — the same `TokenSource` impl the paint path uses, so this
    /// is not a second transfer function written for the test to agree with.
    ///
    /// Takes the theme constructor rather than a `Presenter`, because
    /// `Presenter` is deliberately not `Clone` and a test needs to name the
    /// same theme twice: once to paint with and once to check against.
    fn token_rgb(theme: fn() -> Theme, token: &str) -> [u8; 3] {
        let presenter = Presenter::new(theme());
        let snapshot = presenter.current();
        let color = snapshot
            .color(token)
            .unwrap_or_else(|| panic!("the shipped theme defines {token}"));
        [color.r(), color.g(), color.b()]
    }

    fn near(actual: [u8; 3], expected: [u8; 3], slack: i32) -> bool {
        (0..3)
            .all(|i| i32::from(actual[i]).abs_diff(i32::from(expected[i])) <= slack.unsigned_abs())
    }

    fn pixel(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
        image.get_pixel(x, y).0
    }

    /// The frame's placement of `kind`, in device pixels.
    fn device_rect_of(frame: &PetrifiedFrame, kind: &str) -> WireRect {
        let placement = frame
            .placements
            .iter()
            .find(|p| p.kind.as_str() == kind)
            .unwrap_or_else(|| panic!("the frame has a {kind} placement"));
        let rect = round_rect(placement.rect, frame.viewport.scale);
        WireRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        }
    }

    /// The whole point of T032: the bytes come back from the GPU, they are a
    /// real PNG of the frame's device size, and the colour in them is the
    /// token the tree bound.
    #[test]
    fn a_real_pass_rasterizes_to_a_png_of_the_frames_device_size() {
        let (ctx, _host, pass) = painted(Presenter::new(dark()));
        let mut snapshotter = shared();
        let shot = shoot(&mut snapshotter, &ctx, &pass.output, &pass.frame, None);

        let image = decode(&shot.png);
        assert_eq!(
            image.dimensions(),
            (WINDOW.w as u32, WINDOW.h as u32),
            "at scale 1 the capture is the viewport's device rect"
        );

        let panel = device_rect_of(&pass.frame, "stack");
        assert!(panel.w > 4 && panel.h > 4, "the panel has area: {panel:?}");
        let centre = pixel(
            &image,
            (panel.x + panel.w / 2) as u32,
            (panel.y + panel.h / 2) as u32,
        );
        let background = token_rgb(dark, "surface.base");
        assert_eq!(centre[3], 255, "the panel fill is opaque: {centre:?}");
        assert!(
            near([centre[0], centre[1], centre[2]], background, 2),
            "the panel's pixels are surface.base: got {centre:?}, theme says {background:?}"
        );

        // Not a uniform field: the text node put glyphs on top of the fill.
        let text = device_rect_of(&pass.frame, "text");
        let glyph_pixels = (text.y..text.y + text.h)
            .flat_map(|y| (text.x..text.x + text.w).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = pixel(&image, x as u32, y as u32);
                !near([p[0], p[1], p[2]], background, 8)
            })
            .count();
        assert!(
            glyph_pixels > 0,
            "the text placement {text:?} must contain pixels that are not the background"
        );
    }

    /// The identity on the shot is the captured frame's own, and `hosted`
    /// comes from the frame rather than from anything this module derives.
    #[test]
    fn the_shot_carries_the_captured_frames_identity() {
        let (ctx, _host, pass) = painted(Presenter::new(dark()));
        let mut snapshotter = shared();
        let shot = shoot(&mut snapshotter, &ctx, &pass.output, &pass.frame, None);

        assert_eq!(shot.seq, pass.frame.seq);
        assert_eq!(shot.digest, pass.frame.digest.hex());
        assert_eq!(shot.hosted, pass.frame.hosted());
        assert!(
            !shot.hosted,
            "this tree registers no custom painter, so nothing in it is hosted"
        );
    }

    /// FR-040 with teeth: a frame whose recorded digest is not the digest of
    /// its own placements is refused, not photographed.
    #[test]
    fn a_frame_whose_digest_is_not_its_own_is_refused() {
        let (ctx, _host, pass) = painted(Presenter::new(dark()));
        let mut tampered = pass.frame.clone();
        // Move one node. The placements now hash to something else, while
        // the recorded digest still claims the old picture.
        tampered.placements[0].rect.x += 7.0;

        let mut snapshotter = shared();
        let err = snapshotter
            .capture(&ctx, &pass.output, &tampered, None)
            .expect_err("a self-inconsistent frame must not produce a screenshot");

        match err {
            CaptureError::DigestMismatch {
                seq,
                recorded,
                recomputed,
            } => {
                assert_eq!(seq, pass.frame.seq);
                assert_eq!(recorded, pass.frame.digest.hex());
                assert_ne!(recomputed, recorded);
            }
            other => panic!("expected a digest mismatch, got {other:?}"),
        }
    }

    /// The other half of FR-040's verification: a frame petrified against a
    /// different viewport than the pass ran at is refused, even when its own
    /// digest is internally consistent.
    #[test]
    fn a_frame_from_another_viewport_is_refused() {
        let (ctx, _host, pass) = painted(Presenter::new(dark()));
        let mut elsewhere = pass.frame.clone();
        elsewhere.viewport.size = Size::new(WINDOW.w + 40.0, WINDOW.h);
        // Re-hash, so the digest check passes and the viewport check is the
        // only thing left that can catch this.
        elsewhere.digest = digest(&elsewhere.viewport, &elsewhere.placements);

        let mut snapshotter = shared();
        let err = snapshotter
            .capture(&ctx, &pass.output, &elsewhere, None)
            .expect_err("a frame from another viewport must not be captured");

        assert!(
            matches!(err, CaptureError::ViewportMismatch { .. }),
            "expected a viewport mismatch, got {err:?}"
        );
    }

    /// A region crops in device pixels, and the crop is the same pixels the
    /// full capture has at that offset.
    #[test]
    fn a_region_crops_the_same_pixels_the_full_shot_has() {
        let (ctx, _host, pass) = painted(Presenter::new(dark()));
        let mut snapshotter = shared();
        let region = WireRect {
            x: 12,
            y: 8,
            w: 40,
            h: 25,
        };
        let full = decode(&shoot(&mut snapshotter, &ctx, &pass.output, &pass.frame, None).png);
        let cropped = decode(
            &shoot(
                &mut snapshotter,
                &ctx,
                &pass.output,
                &pass.frame,
                Some(region),
            )
            .png,
        );

        assert_eq!(cropped.dimensions(), (region.w as u32, region.h as u32));
        for y in 0..region.h as u32 {
            for x in 0..region.w as u32 {
                assert_eq!(
                    pixel(&cropped, x, y),
                    pixel(&full, x + region.x as u32, y + region.y as u32),
                    "crop pixel ({x}, {y}) must be the full shot's ({}, {})",
                    x + region.x as u32,
                    y + region.y as u32
                );
            }
        }
    }

    /// A region that leaves the viewport is an error, not a clamp — a
    /// clamped crop answers a different question at the same success code.
    #[test]
    fn a_region_outside_the_viewport_is_refused_rather_than_clamped() {
        let (ctx, _host, pass) = painted(Presenter::new(dark()));
        let mut snapshotter = shared();
        let overhang = WireRect {
            x: (WINDOW.w as i32) - 10,
            y: 0,
            w: 40,
            h: 10,
        };
        let err = snapshotter
            .capture(&ctx, &pass.output, &pass.frame, Some(overhang))
            .expect_err("a region past the right edge must be refused");
        assert!(
            matches!(err, CaptureError::RegionOutsideViewport { .. }),
            "expected an out-of-viewport refusal, got {err:?}"
        );
        assert_eq!(err.wire_kind(), ErrorKind::InvalidParams);

        let empty = WireRect {
            x: 0,
            y: 0,
            w: 0,
            h: 10,
        };
        let err = snapshotter
            .capture(&ctx, &pass.output, &pass.frame, Some(empty))
            .expect_err("a region with no area must be refused");
        assert!(
            matches!(err, CaptureError::EmptyRegion(_)),
            "expected an empty-region refusal, got {err:?}"
        );
        assert_eq!(err.wire_kind(), ErrorKind::InvalidParams);
    }

    /// Two themes over one tree are two pictures. Same geometry, same text,
    /// different tokens — so the pixels can only differ if the capture is
    /// really reading the paint pass rather than the placements.
    #[test]
    fn two_themes_of_one_tree_produce_different_pixels() {
        let (dark_ctx, _dh, dark_pass) = painted(Presenter::new(dark()));
        let (light_ctx, _lh, light_pass) = painted(Presenter::new(light()));

        let mut snapshotter = shared();
        let dark_shot = shoot(
            &mut snapshotter,
            &dark_ctx,
            &dark_pass.output,
            &dark_pass.frame,
            None,
        );
        let light_shot = shoot(
            &mut snapshotter,
            &light_ctx,
            &light_pass.output,
            &light_pass.frame,
            None,
        );

        assert_ne!(
            dark_shot.png, light_shot.png,
            "two themes must not rasterize to identical bytes"
        );
        assert_ne!(dark_shot.digest, light_shot.digest);

        let dark_image = decode(&dark_shot.png);
        let light_image = decode(&light_shot.png);
        let panel = device_rect_of(&dark_pass.frame, "stack");
        let (cx, cy) = (
            (panel.x + panel.w / 2) as u32,
            (panel.y + panel.h / 2) as u32,
        );
        let dark_pixel = pixel(&dark_image, cx, cy);
        let light_pixel = pixel(&light_image, cx, cy);
        assert!(
            near(
                [dark_pixel[0], dark_pixel[1], dark_pixel[2]],
                token_rgb(dark, "surface.base"),
                2
            ),
            "dark capture centre {dark_pixel:?} is the dark theme's surface.base"
        );
        assert!(
            near(
                [light_pixel[0], light_pixel[1], light_pixel[2]],
                token_rgb(light, "surface.base"),
                2
            ),
            "light capture centre {light_pixel:?} is the light theme's surface.base"
        );
    }

    /// The capture is sized in device pixels, so a scale change changes the
    /// image and not just the numbers on the wire.
    #[test]
    fn the_capture_is_sized_in_device_pixels() {
        let ctx = headless();
        ctx.set_pixels_per_point(2.0);
        let mut host = Host::new(&ctx, Panel, Presenter::new(dark()));
        // Two passes: `set_pixels_per_point` takes effect at the start of the
        // next pass, so the first one is still at scale 1.
        drop(one_pass(&ctx, &mut host));
        let pass = one_pass(&ctx, &mut host);

        assert_eq!(
            pass.frame.viewport.scale.factor(),
            2.0,
            "the host petrified at the scale the context reports"
        );
        let mut snapshotter = shared();
        let shot = shoot(&mut snapshotter, &ctx, &pass.output, &pass.frame, None);

        let image = decode(&shot.png);
        assert_eq!(
            image.dimensions(),
            ((WINDOW.w * 2.0) as u32, (WINDOW.h * 2.0) as u32),
            "at scale 2 the capture is twice the logical size in each axis"
        );
    }
}
