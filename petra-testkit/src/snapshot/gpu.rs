//! The rasterizer: one wgpu device, one [`egui_wgpu::Renderer`], one readback.
//!
//! This is a deliberate mirror of `egui_kittest`'s wgpu harness (R3:
//! "mirror, don't build on"), read at
//! `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui_kittest-0.36.1/src/wgpu.rs:161`
//! (`TestRenderer::render`) and `.../src/texture_to_image.rs:10`
//! (`texture_to_image`). Three things differ, each on purpose:
//!
//! * **`Rgba8Unorm`, always.** kittest renders into
//!   `RenderState::target_format`, which is whatever the surface negotiated,
//!   and then reads the bytes back as if they were RGBA. On a `Bgra8`
//!   surface that swaps two channels in the saved image. This module names
//!   the format itself, so the readback's channel order is a fact, not a
//!   negotiation.
//! * **No `egui_wgpu::RenderState` and no `WgpuSetup`.** Those exist to share
//!   a device with a window's surface. There is no window here (FR-043's
//!   "no display" case is the *normal* case for the driver), so the device is
//!   requested straight from `wgpu` with no surface and no display handle —
//!   `wgpu::InstanceDescriptor::new_without_display_handle`
//!   (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/wgpu-types-30.0.0/src/instance.rs:67`).
//! * **The font atlas is uploaded whole, every capture.** See
//!   [`Gpu::upload_font_atlas`].
//!
//! # Why the caller has to hand over `FullOutput`
//!
//! `egui::Context` does not retain a finished pass's shapes.
//! `GraphicLayers::drain` (`egui-0.36.1/src/layers.rs:213`) `append`s every
//! paint list into the returned vector, which leaves each list empty, and
//! `ContextImpl::end_pass` (`egui-0.36.1/src/context.rs:2661`) calls it. The
//! same `end_pass` also calls `tex_manager.take_delta()`
//! (`context.rs:2676`), and `TextureManager` keeps only metadata —
//! `TextureMeta` is name, size, bytes-per-pixel, retain count, options
//! (`epaint-0.36.1/src/textures.rs:126`) — so the pixels of an uploaded
//! texture exist exactly once, in the delta. Both leave the `Context` with
//! the pass's `FullOutput`. That is why [`super::Snapshotter::capture`] takes
//! it, rather than trying to recover either from a bare `&Context`.

use std::sync::OnceLock;
use std::sync::mpsc::channel;
use std::time::Duration;

use egui::epaint::{ClippedPrimitive, ImageDelta, Primitive, TextureId};
use egui::{Context, TextureOptions, TexturesDelta};
use image::RgbaImage;

use super::CaptureError;

/// The colour format every capture renders into and reads back.
///
/// Gamma-space and RGBA-ordered: `egui_wgpu::Renderer` picks its fragment
/// entry point from `output_color_format.is_srgb()`
/// (`egui-wgpu-0.36.1/src/renderer.rs:267` onward), so a non-sRGB target gets
/// the gamma-writing shader and the bytes that land in the texture are the
/// same gamma-space bytes an `egui::Color32` carries.
const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How long a capture waits on the GPU before giving up.
///
/// kittest's own reasoning applies unchanged (`egui_kittest-0.36.1/src/wgpu.rs:16`):
/// a driver reset lands well inside this, and a wait that never returns is
/// worse than a named failure.
const WAIT: Duration = Duration::from_secs(10);

/// A device, a queue, and the egui renderer that owns the uploaded textures.
///
/// Built once and kept: `request_device` plus pipeline creation is tens of
/// milliseconds, and — more importantly — [`egui_wgpu::Renderer`] is where
/// uploaded textures live, so a renderer rebuilt per capture would start
/// every shot with no font atlas and no images.
pub(super) struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: egui_wgpu::Renderer,
    /// What adapter answered, for the record a report can quote.
    adapter: String,
}

/// How long anybody waits for the GPU to come up at all.
///
/// Separate constant from [`WAIT`] only so the two can be reasoned about
/// separately; they are the same number today because they answer the same
/// question — "is the driver coming back?"
const BRINGUP_WAIT: Duration = WAIT;

/// Enumerate an adapter and open a device. Runs on its own thread; see
/// [`Gpu::new`] for why.
fn bring_up() -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), CaptureError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY | wgpu::Backends::GL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|err| CaptureError::NoAdapter(err.to_string()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("gorgon-petra-testkit snapshot"),
        required_limits: adapter.limits(),
        ..wgpu::DeviceDescriptor::default()
    }))
    .map_err(|err| CaptureError::NoDevice(err.to_string()))?;
    Ok((adapter, device, queue))
}

impl Gpu {
    /// Bring up a headless device once, or say why not.
    ///
    /// Called only through [`Self::shared_device`], which caches whichever
    /// answer this gives for the life of the process.
    ///
    /// `Backends::PRIMARY | Backends::GL` and no surface: nothing here needs
    /// a window, an X display, or a Wayland socket. `BROWSER_WEBGPU` and
    /// `NOOP` are both left out — the first cannot exist in a native driver
    /// process, and the second would answer every capture with a blank image
    /// while reporting success, which is the exact failure this whole task
    /// exists to not commit.
    fn open_device() -> Result<(String, wgpu::Device, wgpu::Queue), CaptureError> {
        // Bring-up runs on its own thread with a bounded wait, for the same
        // reason `WAIT` bounds the polls below and for one more: adapter
        // enumeration is the one step that talks to a driver this process has
        // never touched, and a driver that does not answer would otherwise
        // park the UI thread — and, through it, every driver client and the
        // whole gate lane — with no error and no deadline. `pollster` cannot
        // cancel a future, so the thread is left to finish on its own if it
        // ever does; what is bounded is how long anybody waits for it.
        //
        // The 10s budget is `WAIT`. On this machine a cold bring-up measures
        // in tens of milliseconds; 10 s is "the driver is not coming back",
        // not "the driver is slow".
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("petra-snapshot-gpu-bringup".to_owned())
            .spawn(move || {
                let _ = tx.send(bring_up());
            })
            .map_err(|err| CaptureError::NoDevice(format!("could not start bring-up: {err}")))?;
        let (adapter, device, queue) = match rx.recv_timeout(BRINGUP_WAIT) {
            Ok(result) => result?,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                return Err(CaptureError::NoAdapter(format!(
                    "no wgpu adapter answered within {}s; the graphics driver did not respond \
                     to adapter enumeration or device creation",
                    BRINGUP_WAIT.as_secs()
                )));
            }
            // The bring-up thread ended without sending. It cannot return
            // early and it cannot panic silently, so this is a panic in
            // `bring_up` — report it as a device failure rather than
            // unwrapping into a second panic on this thread.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(CaptureError::NoDevice(
                    "the wgpu bring-up thread ended without answering".to_owned(),
                ));
            }
        };
        let info = adapter.get_info();
        Ok((
            format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type),
            device,
            queue,
        ))
    }

    /// The one device this **process** opens, and the adapter line naming it.
    ///
    /// A [`Gpu`] used to call [`Self::open_device`] itself, so a process got
    /// one `wgpu` device per [`super::Snapshotter`]. That is fine for the
    /// driver, which builds one, and it is not fine for a test binary that
    /// builds one per test: the gallery holds about sixty capture tests, and
    /// at cargo's default thread count `request_device` began refusing with
    /// `NoDevice("Not enough memory left.")` and
    /// `NoDevice("Parent device is lost")` — a different test each run, on an
    /// otherwise idle card with 29 of 32 GB free. `cargo xtask gates` failed
    /// at random, which is worse than failing, because a gate nobody can
    /// trust gets read as noise. This crate's own tests hit the same wall
    /// earlier and from the other side, hanging rather than refusing, and
    /// worked around it with a private mutex that was never carried across.
    ///
    /// **The device is the scarce thing, and it is the only thing shared
    /// here.** `wgpu::Device` and `wgpu::Queue` are `Arc`-backed handles that
    /// clone cheaply and take work from several threads at once, so captures
    /// still run in parallel. What stays per-[`Gpu`] is the
    /// [`egui_wgpu::Renderer`], which owns uploaded textures and mutates its
    /// own buffers as it draws, and so is not something two threads may share.
    /// Building one costs pipeline creation rather than a device; the price
    /// is a font atlas per snapshotter, which is far below a device apiece.
    ///
    /// A failed bring-up is cached along with a successful one. That is
    /// deliberate: every failure this can return means the driver is not
    /// answering, [`BRINGUP_WAIT`] has already given it ten seconds, and
    /// retrying it once per test would turn one named failure into sixty.
    fn shared_device() -> Result<(String, wgpu::Device, wgpu::Queue), CaptureError> {
        static SHARED: OnceLock<Result<(String, wgpu::Device, wgpu::Queue), CaptureError>> =
            OnceLock::new();
        SHARED.get_or_init(Self::open_device).clone()
    }

    /// A snapshotter's GPU: this process's device, and a renderer of its own.
    pub(super) fn new() -> Result<Self, CaptureError> {
        let (adapter, device, queue) = Self::shared_device()?;
        let renderer = egui_wgpu::Renderer::new(
            &device,
            TARGET_FORMAT,
            // MSAA off, dithering off, predictable texture filtering on. A
            // screenshot a gate compares is worth more when the same frame
            // rasterizes the same way twice.
            egui_wgpu::RendererOptions::PREDICTABLE,
        );
        Ok(Self {
            device,
            queue,
            renderer,
            adapter,
        })
    }

    /// The adapter this device came from, for a report or an error message.
    pub(super) fn adapter(&self) -> &str {
        &self.adapter
    }

    /// Apply one pass's texture changes.
    ///
    /// Read-only in the `egui` sense: the delta is *applied*, not consumed,
    /// so the caller's own renderer — a real `eframe` window, when the
    /// testkit is compiled into one — still gets to apply the same delta.
    /// That is why this takes `&TexturesDelta` and never `take_delta`.
    pub(super) fn absorb(&mut self, delta: &TexturesDelta) {
        for (id, images) in &delta.set {
            for image in images {
                // A sub-region update needs the base allocation to exist:
                // `Renderer::update_texture` panics outright otherwise
                // (`egui-wgpu-0.36.1/src/renderer.rs:669`). That happens for
                // real — the font atlas grows by partial deltas, and a
                // renderer built after the first pass never saw the
                // allocation the first delta made. Skipping is safe rather
                // than lossy: the atlas is re-uploaded whole below, and any
                // other texture in that state is caught by
                // `check_drawable`, which refuses the capture by name.
                if !image.is_whole() && self.renderer.texture(id).is_none() {
                    continue;
                }
                self.renderer
                    .update_texture(&self.device, &self.queue, *id, image);
            }
        }
        for id in &delta.free {
            self.renderer.free_texture(id);
        }
    }

    /// Upload the whole font atlas as `TextureId::default()`.
    ///
    /// Deltas are incremental, and a capture only sees the deltas of the
    /// passes it was called on: a glyph that entered the atlas during a pass
    /// nobody captured is in no delta this module will ever be handed. The
    /// atlas image itself is not incremental — `FontsView::image` returns the
    /// full current image (`epaint-0.36.1/src/text/fonts.rs:774`) — so
    /// re-uploading it whole makes text correct for every capture regardless
    /// of which passes were captured. `TextureId::default()` is the font
    /// texture by construction: `WrappedTextureManager::default` allocates it
    /// first and asserts the id (`egui-0.36.1/src/context.rs:78`).
    /// `TextureOptions::LINEAR` is what the atlas itself asks for
    /// (`epaint-0.36.1/src/texture_atlas.rs:191`).
    pub(super) fn upload_font_atlas(&mut self, ctx: &Context) {
        let atlas = ctx.fonts(|fonts| fonts.image());
        let delta = ImageDelta::full(atlas, TextureOptions::LINEAR);
        self.renderer
            .update_texture(&self.device, &self.queue, TextureId::default(), &delta);
    }

    /// Refuse a draw list this renderer cannot honestly rasterize.
    ///
    /// Two ways that happens, and neither may become a quietly wrong
    /// picture:
    ///
    /// * a mesh naming a texture that was never uploaded here — the pixels
    ///   for it are in a delta this module never saw (see
    ///   [`Gpu::upload_font_atlas`] for why that is possible at all);
    /// * an `epaint::Primitive::Callback`, which `egui_wgpu` renders only by
    ///   downcasting to its own `CallbackTrait` and otherwise logs and skips.
    ///   A skipped callback is a hole in the image with a success code
    ///   attached.
    pub(super) fn check_drawable(
        &self,
        primitives: &[ClippedPrimitive],
    ) -> Result<(), CaptureError> {
        for primitive in primitives {
            match &primitive.primitive {
                Primitive::Mesh(mesh) => {
                    if self.renderer.texture(&mesh.texture_id).is_none() {
                        return Err(CaptureError::MissingTexture {
                            id: format!("{:?}", mesh.texture_id),
                        });
                    }
                }
                Primitive::Callback(_) => return Err(CaptureError::PaintCallback),
            }
        }
        Ok(())
    }

    /// Rasterize `primitives` into a `width x height` device-pixel image.
    pub(super) fn render(
        &mut self,
        primitives: &[ClippedPrimitive],
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) -> Result<RgbaImage, CaptureError> {
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gorgon-petra-testkit snapshot encoder"),
            });
        let user_buffers = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            primitives,
            &screen,
        );
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gorgon-petra-testkit snapshot target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gorgon-petra-testkit snapshot pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // Transparent, not opaque: a Petra frame paints
                            // its own background token, and clearing to a
                            // colour of this module's choosing would put a
                            // pixel in the shot that no placement drew.
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, primitives, &screen);
        }
        self.queue.submit(
            user_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(WAIT),
            })
            .map_err(|err| CaptureError::Render(format!("poll after render: {err}")))?;
        self.read_back(&texture)
    }

    /// Copy a rendered texture into CPU memory, unpadding the rows.
    ///
    /// `copy_texture_to_buffer` requires each row to start on a
    /// `COPY_BYTES_PER_ROW_ALIGNMENT` boundary, so the buffer is wider than
    /// the image and the padding is dropped on the way out — the same shape
    /// as `egui_kittest`'s `BufferDimensions`
    /// (`egui_kittest-0.36.1/src/texture_to_image.rs:74`).
    fn read_back(&self, texture: &wgpu::Texture) -> Result<RgbaImage, CaptureError> {
        let width = texture.width();
        let height = texture.height();
        let unpadded = width as usize * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded = unpadded + (align - unpadded % align) % align;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gorgon-petra-testkit snapshot readback"),
            size: (padded * height as usize) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gorgon-petra-testkit snapshot readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let submission = self.queue.submit(std::iter::once(encoder.finish()));

        let slice = buffer.slice(..);
        let (sender, receiver) = channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            // The receiver is only dropped after this closure has run or the
            // poll below failed; a send into a dropped channel is the second
            // case and carries no information the poll error does not.
            drop(sender.send(result));
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(WAIT),
            })
            .map_err(|err| CaptureError::Render(format!("poll after readback copy: {err}")))?;
        // Bounded, not `recv()`. The poll above is what drives the map
        // callback, so by the time it returns the result is normally already
        // in the channel; an unbounded wait here would turn any way that
        // stops being true into a gate that hangs forever instead of a gate
        // that fails, and a hung gate takes the whole lane with it.
        receiver
            .recv_timeout(WAIT)
            .map_err(|err| {
                CaptureError::Render(format!(
                    "waited {WAIT:?} for the readback buffer mapping and it never resolved: {err}"
                ))
            })?
            .map_err(|err| CaptureError::Render(format!("map readback buffer: {err}")))?;

        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|err| CaptureError::Render(format!("read mapped range: {err}")))?;
        let pixels: Vec<u8> = mapped
            .chunks_exact(padded)
            .flat_map(|row| row.iter().take(unpadded))
            .copied()
            .collect();
        drop(mapped);
        buffer.unmap();

        RgbaImage::from_raw(width, height, pixels).ok_or_else(|| {
            CaptureError::Render(format!(
                "readback produced the wrong byte count for {width}x{height}"
            ))
        })
    }
}
