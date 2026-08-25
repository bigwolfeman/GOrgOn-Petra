//! A headless `wgpu` pipeline that renders one `egui` pass to CPU memory,
//! shared by every text-inspection test in this crate.
//!
//! # Why this exists rather than living in one test file
//!
//! It was written inside `text_contact_sheet.rs`, which needed it first.
//! `text_size_probe.rs` needs exactly the same three things — bring a device
//! up, render a pass, write a PPM — and copying them would have started two
//! copies of a `wgpu` bring-up drifting apart, which is the one kind of
//! duplication a device-and-queue lifecycle punishes hardest: the copies
//! fail differently on the same driver and neither file's author sees the
//! other's failure.
//!
//! # Why a private pipeline at all, rather than `petra-testkit`
//!
//! `gorgon_petra_testkit::snapshot`'s `Gpu` is `pub(super)`, so it cannot be
//! reached from an integration test in this crate. `eframe` re-exports both
//! `egui_wgpu` and `wgpu` (`pub use {egui_wgpu, egui_wgpu::wgpu}`), and this
//! crate already depends on `eframe` with its `wgpu` feature, so building one
//! here needed no `Cargo.toml` change anywhere.
//!
//! # Output format: PPM, not PNG
//!
//! A raw `P6` PPM needs no image crate, and `lab/greycol.py` (via
//! ImageMagick) reads it interchangeably with PNG.

// Each test binary that declares `mod support` compiles this file into
// itself and uses only the part it needs, so items the *other* binary uses
// read as dead here.
#![allow(dead_code)]

use eframe::{egui_wgpu, wgpu};
use egui::Context;
use std::io::Write as _;

/// A minimal `pollster`-equivalent: `pollster` itself is a private
/// dependency of `eframe` and not reachable from here (see the module doc's
/// "why this file builds its own headless wgpu pipeline"). wgpu's native
/// adapter/device futures resolve after being polled a handful of times, so
/// a busy-poll with a short sleep between attempts is a faithful stand-in,
/// not a shortcut: the awaited work is the same either way, only who drives
/// the executor differs.
pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context as TaskContext, Poll, RawWaker, RawWakerVTable, Waker};

    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);

    let raw = RawWaker::new(std::ptr::null(), &VTABLE);
    // SAFETY: every function in `VTABLE` is a no-op over a null data
    // pointer; nothing ever dereferences it.
    let waker = unsafe { Waker::from_raw(raw) };
    let mut cx = TaskContext::from_waker(&waker);
    let mut fut = Box::pin(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::sleep(std::time::Duration::from_millis(1)),
        }
    }
}

/// How long any single wgpu wait is given before this test fails loudly
/// rather than hanging the gate lane. Matches `snapshot::gpu`'s own budget.
const WAIT: std::time::Duration = std::time::Duration::from_secs(10);

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// One wgpu device, kept across every arm: bring-up is the expensive part,
/// not the per-arm renderer this struct rebuilds nowhere (a fresh
/// `egui::Context` per arm means a fresh font atlas per arm, so a fresh
/// `egui_wgpu::Renderer` goes with it — seeded from the same device).
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY | wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap_or_else(|err| {
                panic!("no wgpu adapter for a headless text render (PRIMARY|GL backends): {err}")
            });
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("gorgon-petra-egui headless text render"),
            required_limits: adapter.limits(),
            ..wgpu::DeviceDescriptor::default()
        }))
        .unwrap_or_else(|err| panic!("no wgpu device from the chosen adapter: {err}"));
        Self { device, queue }
    }

    /// Render one pass's shapes into a fresh `width x height` texture and
    /// read the RGBA bytes back. One renderer is built per call because one
    /// renderer is built per arm's font atlas — see the struct doc.
    pub fn capture(
        &self,
        ctx: &Context,
        output: &egui::FullOutput,
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        let mut renderer = egui_wgpu::Renderer::new(
            &self.device,
            TARGET_FORMAT,
            egui_wgpu::RendererOptions::PREDICTABLE,
        );
        for (id, images) in &output.textures_delta.set {
            for image in images {
                renderer.update_texture(&self.device, &self.queue, *id, image);
            }
        }
        for id in &output.textures_delta.free {
            renderer.free_texture(id);
        }

        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: output.pixels_per_point,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("headless-text encoder"),
            });
        let user_buffers = renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &primitives,
            &screen,
        );
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless-text target"),
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
                    label: Some("headless-text pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            renderer.render(&mut pass, &primitives, &screen);
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
            .unwrap_or_else(|err| panic!("poll after render: {err}"));

        self.read_back(&texture, width, height)
    }

    /// Copy a rendered texture into CPU memory as tightly packed RGBA8,
    /// unpadding rows the same way `copy_texture_to_buffer` pads them.
    fn read_back(&self, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
        let unpadded = width as usize * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded = unpadded + (align - unpadded % align) % align;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless-text readback"),
            size: (padded * height as usize) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("headless-text readback encoder"),
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
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(WAIT),
            })
            .unwrap_or_else(|err| panic!("poll after readback copy: {err}"));
        rx.recv_timeout(WAIT)
            .unwrap_or_else(|err| panic!("readback buffer mapping never resolved: {err}"))
            .unwrap_or_else(|err| panic!("map readback buffer: {err}"));

        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .unwrap_or_else(|err| panic!("read mapped range: {err}"));
        let pixels: Vec<u8> = mapped
            .chunks_exact(padded)
            .flat_map(|row| row[..unpadded].iter().copied())
            .collect();
        drop(mapped);
        buffer.unmap();
        pixels
    }
}

/// Write tightly packed RGB bytes out as a raw `P6` PPM.
///
/// `pub` so both text-inspection binaries can call it; see this module's
/// "Output format" note for why PPM rather than PNG.
pub fn write_ppm(path: &std::path::Path, width: u32, height: u32, rgb: &[u8]) {
    let mut file = std::fs::File::create(path)
        .unwrap_or_else(|err| panic!("create {}: {err}", path.display()));
    write!(file, "P6\n{width} {height}\n255\n")
        .unwrap_or_else(|err| panic!("write PPM header: {err}"));
    file.write_all(rgb)
        .unwrap_or_else(|err| panic!("write PPM body: {err}"));
}
