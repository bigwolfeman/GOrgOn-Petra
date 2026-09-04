//! Offscreen picture of the last full Petra paint, for a flying-caret hop.
//!
//! Two color targets. A bake writes the back buffer; a blit samples the
//! front. After `queue.submit` the baker waits for that submission so the
//! host can blit the new front on the same pass. Sampling a target this
//! pass `LoadOp::Clear`ed is the flicker. Switching from CPU meshes to a
//! blit mid-hop is the remaining pop; the host blits for the whole hop
//! once this wait succeeds, or stays on meshes if it does not.
//!
//! The cache is native-only. Headless tests never construct one; they keep
//! the CPU mesh replay in [`crate::host::Host`]. See
//! `.agents/notes/implemented/architecture/2026-08-30-scene-texture-caret-overlay.md`.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use eframe::{egui_wgpu, wgpu};
use egui::epaint::{ClippedPrimitive, ImageDelta, Primitive, TextureId};
use egui::{Color32, ColorImage, Context, Rect, TextureOptions};

/// Format `register_native_texture` will sample. Must stay `Rgba8Unorm`.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    id: TextureId,
}

/// Ping-pong window-sized color targets plus the baker that renders into them.
pub(crate) struct SceneCache {
    gpu: egui_wgpu::RenderState,
    baker: egui_wgpu::Renderer,
    targets: [Target; 2],
    /// Index sampled by [`Self::blit_shape`].
    display: usize,
    size: [u32; 2],
    /// Front holds a finished picture.
    have_front: bool,
}

impl SceneCache {
    /// Render `primitives` into `slot`, reusing the pair when the size
    /// matches. `clear` is `surface.base` in gamma; a black clear is what
    /// made translucent shadows pop against the page. Returns whether the
    /// new front is ready to blit on this pass.
    pub(crate) fn capture(
        slot: &mut Option<Self>,
        gpu: &egui_wgpu::RenderState,
        ctx: &Context,
        primitives: &[ClippedPrimitive],
        size: [u32; 2],
        pixels_per_point: f32,
        clear: Color32,
    ) -> bool {
        if size[0] == 0 || size[1] == 0 {
            *slot = None;
            return false;
        }
        let reuse = slot.as_ref().is_some_and(|cache| cache.size == size);
        if !reuse {
            *slot = Some(Self::new(gpu, size));
        }
        let Some(cache) = slot.as_mut() else {
            return false;
        };
        cache.paint_into(ctx, primitives, pixels_per_point, clear)
    }

    fn new(gpu: &egui_wgpu::RenderState, size: [u32; 2]) -> Self {
        let baker =
            egui_wgpu::Renderer::new(&gpu.device, FORMAT, egui_wgpu::RendererOptions::default());
        let targets = [make_target(gpu, size), make_target(gpu, size)];
        Self {
            gpu: gpu.clone(),
            baker,
            targets,
            display: 0,
            size,
            have_front: false,
        }
    }

    /// Whether the front buffer holds a finished picture.
    #[must_use]
    pub(crate) fn has_front(&self) -> bool {
        self.have_front
    }

    /// Fullscreen image of the **front** buffer. `rect` is the window in points.
    pub(crate) fn blit_shape(&self, rect: Rect) -> egui::Shape {
        egui::Shape::image(
            self.targets[self.display].id,
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        )
    }

    fn paint_into(
        &mut self,
        ctx: &Context,
        primitives: &[ClippedPrimitive],
        pixels_per_point: f32,
        clear: Color32,
    ) -> bool {
        if !self.seed_textures(ctx, primitives) {
            return false;
        }
        let back = 1 - self.display;
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: self.size,
            pixels_per_point,
        };
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("petra-scene-cache"),
            });
        let user_buffers = self.baker.update_buffers(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            primitives,
            &screen,
        );
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("petra-scene-cache"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.targets[back].view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(gamma_clear(clear)),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.baker.render(&mut pass, primitives, &screen);
        }
        let index = self.gpu.queue.submit(
            user_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        if self
            .gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .is_err()
        {
            return false;
        }
        self.display = back;
        self.have_front = true;
        true
    }

    /// Font atlas from `ctx`; every other mesh texture from the shared
    /// eframe renderer. A callback or a texture eframe has not uploaded yet
    /// fails the bake so the host stays on the CPU mesh replay.
    fn seed_textures(&mut self, ctx: &Context, primitives: &[ClippedPrimitive]) -> bool {
        let mut ids = BTreeSet::new();
        for primitive in primitives {
            match &primitive.primitive {
                Primitive::Mesh(mesh) => {
                    ids.insert(mesh.texture_id);
                }
                Primitive::Callback(_) => return false,
            }
        }

        let atlas = ctx.fonts(|fonts| fonts.image());
        self.baker.update_texture(
            &self.gpu.device,
            &self.gpu.queue,
            TextureId::default(),
            &ImageDelta::full(atlas, TextureOptions::LINEAR),
        );

        let views = {
            let shared = self.gpu.renderer.read();
            let mut views = Vec::new();
            for id in ids {
                if id == TextureId::default() {
                    continue;
                }
                let Some(gpu_tex) = shared.texture(&id).and_then(|t| t.texture.as_ref()) else {
                    return false;
                };
                views.push((
                    id,
                    gpu_tex.create_view(&wgpu::TextureViewDescriptor::default()),
                ));
            }
            views
        };
        for (id, view) in views {
            if self.baker.texture(&id).is_none() {
                let dummy = ImageDelta::full(
                    ColorImage::filled([1, 1], Color32::WHITE),
                    TextureOptions::LINEAR,
                );
                self.baker
                    .update_texture(&self.gpu.device, &self.gpu.queue, id, &dummy);
            }
            self.baker.update_egui_texture_from_wgpu_texture(
                &self.gpu.device,
                &view,
                wgpu::FilterMode::Linear,
                id,
            );
        }
        true
    }
}

impl Drop for SceneCache {
    fn drop(&mut self) {
        let mut shared = self.gpu.renderer.write();
        for target in &self.targets {
            shared.free_texture(&target.id);
        }
        drop(shared);
        for target in &self.targets {
            target.texture.destroy();
        }
    }
}

fn gamma_clear(color: Color32) -> wgpu::Color {
    wgpu::Color {
        r: f64::from(color.r()) / 255.0,
        g: f64::from(color.g()) / 255.0,
        b: f64::from(color.b()) / 255.0,
        a: 1.0,
    }
}

fn make_target(gpu: &egui_wgpu::RenderState, size: [u32; 2]) -> Target {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("petra-scene-cache"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let id =
        gpu.renderer
            .write()
            .register_native_texture(&gpu.device, &view, wgpu::FilterMode::Nearest);
    Target { texture, view, id }
}
