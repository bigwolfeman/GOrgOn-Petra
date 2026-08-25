//! The display-free gate the coverage-curve port rests on (C1 of
//! `ignored/builds/2026-08-24-text-pipeline-port/gates/leaf-C-measure.md`),
//! plus an opt-in relative cost measurement (C4).
//!
//! # What C1 asserts, and why it is enough
//!
//! `ignored/builds/.../port-spec.md` §7.3 tier 1: "the coverage curve is a
//! pure function of coverage... a test that renders one text run at
//! `passes = 1, 2, 3` into [an] offscreen texture and asserts the composited
//! alpha follows `1-(1-a)^n` catches every regression in the *mechanism*, on
//! any machine, with no window server of any kind." This file is that test.
//!
//! It does not exercise `host::bind_glyph_coverage` or `paint.rs`'s repeat
//! loop (Leaf B's glue, covered by that leaf's own
//! `the_glyph_coverage_curve_follows_the_theme_mode`) — it drives
//! `egui::Painter::galley` directly, `paints` times, the same primitive
//! operation `paint.rs:770` performs. That is the actual claim the whole
//! port rests on: that repeated `Painter::galley` calls composite under
//! egui-wgpu's premultiplied source-over blend to `1-(1-a)^paints`, and that
//! `FontColorTransferFunction::TwoCoverageMinusCoverageSq` applied once to the
//! font atlas produces the same bytes as `Off` applied twice through that
//! blend. A future refactor that breaks either — changes the blend mode,
//! short-circuits the repeat loop, or changes what the atlas curve computes —
//! moves the pixel this test samples, and this test has no display, no
//! window, and no dependency on any other leaf's files to catch it.
//!
//! # Why this file builds its own headless `wgpu` pipeline
//!
//! Same wall Leaf A hit, recorded in `text_contact_sheet.rs`'s module doc:
//! `gorgon-petra-testkit::snapshot::gpu::Gpu` is `pub(super)`, unreachable
//! from a `petra-egui` test without a `Cargo.toml` dev-dependency edit this
//! leaf does not own, and its reachable surface (`Snapshotter::capture`)
//! demands a `PetrifiedFrame` this file has no reason to construct. The
//! `Gpu` type below is a from-scratch equivalent built only from
//! `eframe::wgpu` / `eframe::egui_wgpu`, both already reachable through this
//! crate's existing `eframe` dependency (`Cargo.toml`'s `wgpu` feature) — no
//! manifest edit needed. It is a near-verbatim copy of
//! `text_contact_sheet.rs`'s own `Gpu`, duplicated rather than shared because
//! this crate's `tests/` files do not share a common module today
//! (`idle_audit.rs`, `text_scripts.rs`, `text_contact_sheet.rs` each build
//! their own harness) and `text_contact_sheet.rs` is owned by Leaf A.

use eframe::{egui_wgpu, wgpu};
use egui::epaint::FontColorTransferFunction;
use egui::{Color32, Context, FontId, Pos2, RawInput, Rect, vec2};
use gorgon_petra_egui::fonts;

/// A minimal `pollster`-equivalent — see `text_contact_sheet.rs`'s
/// identical `block_on` for the full argument (`pollster` is a private
/// dependency of `eframe`, unreachable from here).
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
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
/// rather than hanging the gate lane. Matches `text_contact_sheet.rs` /
/// `snapshot::gpu`'s own budget.
const WAIT: std::time::Duration = std::time::Duration::from_secs(10);

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// One wgpu device, requested once and reused across every render in this
/// file — bring-up (adapter/device request) is the expensive, one-off part.
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: wgpu::AdapterInfo,
}

impl Gpu {
    fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY | wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap_or_else(|err| {
                panic!("no wgpu adapter for a headless coverage-curve render (PRIMARY|GL backends): {err}")
            });
        let adapter_info = adapter.get_info();
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("gorgon-petra-egui text coverage curve gate"),
            required_limits: adapter.limits(),
            ..wgpu::DeviceDescriptor::default()
        }))
        .unwrap_or_else(|err| panic!("no wgpu device from the chosen adapter: {err}"));
        Self {
            device,
            queue,
            adapter_info,
        }
    }

    /// Render one pass's shapes into a fresh `width x height` texture, using
    /// the given renderer (already carrying the atlas this `output` needs),
    /// and read the RGBA8 bytes back.
    fn capture(
        &self,
        renderer: &mut egui_wgpu::Renderer,
        ctx: &Context,
        output: &egui::FullOutput,
        width: u32,
        height: u32,
    ) -> Vec<u8> {
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
                label: Some("text-coverage-curve encoder"),
            });
        let user_buffers =
            renderer.update_buffers(&self.device, &self.queue, &mut encoder, &primitives, &screen);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text-coverage-curve target"),
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
                    label: Some("text-coverage-curve pass"),
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
            label: Some("text-coverage-curve readback"),
            size: (padded * height as usize) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("text-coverage-curve readback encoder"),
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

/// Build a context with the design-system fonts installed and the given
/// atlas curve bound on both style slots — the same defensive symmetry
/// `text_contact_sheet.rs::render_arm` uses, since `host::bind_glyph_coverage`
/// writes both slots and a curve read from the wrong one would be a silent
/// miss, not a loud one.
fn context_with_curve(curve: FontColorTransferFunction) -> Context {
    let ctx = Context::default();
    fonts::install_design_system(&ctx);
    for egui_theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(egui_theme, |style| {
            style.visuals.text_options.color_transfer_function = curve;
        });
    }
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx
}

const PROBE_W: u32 = 64;
const PROBE_H: u32 = 64;
const PROBE_TEXT: &str = "M";
const PROBE_SIZE: f32 = 40.0;
const PROBE_POS: Pos2 = Pos2::new(4.0, 4.0);

/// Render `PROBE_TEXT` painted `paints` times under `curve`, alone on a
/// transparent canvas (no background fill), and return the RGBA8 bytes.
/// Transparent-on-clear is deliberate: it makes the alpha CHANNEL itself the
/// composited-coverage value with no inversion against a measured
/// background needed, unlike `lab/greycol.py`'s captures (which composite
/// text over an opaque surface fill and must recover coverage from RGB).
fn render_probe(gpu: &Gpu, curve: FontColorTransferFunction, paints: u8) -> Vec<u8> {
    let ctx = context_with_curve(curve);
    let mut renderer =
        egui_wgpu::Renderer::new(&gpu.device, TARGET_FORMAT, egui_wgpu::RendererOptions::PREDICTABLE);

    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(PROBE_W as f32, PROBE_H as f32)));

    // Galleys are shaped inside the pass: `ctx.fonts(...)` (which
    // `Painter::layout_no_wrap` calls internally) panics on a context that
    // has never run one — `egui::Context::run()`'s own panic message says
    // so. One pass is all this probe gets, so shaping happens inside it,
    // the same discipline `text_contact_sheet.rs::render_arm` follows.
    let output = ctx.run_ui(input, |ui| {
        let galley = ui.painter().layout_no_wrap(
            PROBE_TEXT.to_owned(),
            FontId::proportional(PROBE_SIZE),
            Color32::WHITE,
        );
        // The primitive this test exists to pin: `paint.rs:770`'s repeat
        // loop, reproduced directly rather than reached through `host`/
        // `paint`'s public entry points, which is the display-free
        // boundary this gate holds (see the module doc).
        for _ in 0..paints {
            ui.painter().galley(PROBE_POS, galley.clone(), Color32::WHITE);
        }
    });

    let pixels = gpu.capture(&mut renderer, &ctx, &output, PROBE_W, PROBE_H);
    // `TexturesDelta` panics in its own destructor when dropped unapplied
    // (`epaint-0.36.1/src/textures.rs:337`); `Gpu::capture` already applied
    // every entry above.
    let mut output = output;
    output.textures_delta.clear();
    pixels
}

fn alpha_at(pixels: &[u8], width: u32, x: u32, y: u32) -> u8 {
    let i = ((y * width + x) * 4 + 3) as usize;
    pixels[i]
}

/// The first pixel whose alpha sits comfortably inside `(20, 235)` — an
/// antialiased edge pixel, neither saturated ink nor untouched background.
/// Scanned rather than hardcoded because the exact coordinate depends on
/// font metrics this test has no reason to pin.
fn find_partial_coverage_pixel(pixels: &[u8], width: u32, height: u32) -> (u32, u32) {
    for y in 0..height {
        for x in 0..width {
            let a = alpha_at(pixels, width, x, y);
            if a > 20 && a < 235 {
                return (x, y);
            }
        }
    }
    panic!(
        "no partial-coverage pixel found in a {width}x{height} render of \"{PROBE_TEXT}\" at \
         {PROBE_SIZE}px; every ink pixel is fully saturated or fully background, so this probe \
         cannot see an antialiased edge — try a different sample glyph or size"
    );
}

/// C1: composited alpha follows `1-(1-a)^n` at `n = 1, 2, 3`, and
/// `TwoCoverageMinusCoverageSq` at one paint equals `Off` at two paints to
/// within the u8 quantum.
#[test]
fn composited_alpha_follows_the_repeat_paint_identity() {
    let gpu = Gpu::new();
    eprintln!(
        "text_coverage_curve: wgpu adapter {} ({:?}, {:?})",
        gpu.adapter_info.name, gpu.adapter_info.backend, gpu.adapter_info.device_type
    );

    let off_1 = render_probe(&gpu, FontColorTransferFunction::Off, 1);
    let (probe_x, probe_y) = find_partial_coverage_pixel(&off_1, PROBE_W, PROBE_H);
    let a1 = alpha_at(&off_1, PROBE_W, probe_x, probe_y);
    assert!(
        a1 > 0 && a1 < 255,
        "probe pixel ({probe_x},{probe_y}) has alpha {a1}, not a partial-coverage pixel"
    );
    let a1f = f64::from(a1) / 255.0;

    // n = 1 is trivial (a1 vs itself) but asserted anyway: the gate names
    // n = 1, 2, 3 explicitly, and a probe that cannot even reproduce its own
    // baseline is not a probe worth trusting for n = 2, 3.
    for n in [1u32, 2, 3] {
        let pixels = if n == 1 {
            off_1.clone()
        } else {
            render_probe(&gpu, FontColorTransferFunction::Off, n as u8)
        };
        let observed = i32::from(alpha_at(&pixels, PROBE_W, probe_x, probe_y));
        let expected = ((1.0 - (1.0 - a1f).powi(n as i32)) * 255.0).round() as i32;
        let diff = (observed - expected).abs();
        assert!(
            diff <= 1,
            "n={n}: composited alpha {observed} does not follow 1-(1-a)^n (expected {expected} \
             from a={a1} at ({probe_x},{probe_y})); diff={diff} exceeds the u8 quantum"
        );
    }

    let twocov_1 = render_probe(&gpu, FontColorTransferFunction::TwoCoverageMinusCoverageSq, 1);
    let off_2 = render_probe(&gpu, FontColorTransferFunction::Off, 2);
    let a_twocov1 = i32::from(alpha_at(&twocov_1, PROBE_W, probe_x, probe_y));
    let a_off2 = i32::from(alpha_at(&off_2, PROBE_W, probe_x, probe_y));
    let diff = (a_twocov1 - a_off2).abs();
    assert!(
        diff <= 1,
        "TwoCoverageMinusCoverageSq at 1 paint ({a_twocov1}) does not equal Off at 2 paints \
         ({a_off2}) within the u8 quantum at ({probe_x},{probe_y}); diff={diff}"
    );
}

// ---------------------------------------------------------------------------
// C4: relative cost of paint repeats — CPU and GPU per frame, as a ratio.
//
// Model: ai-macs' `TestMeasureCoveragePassCost`
// (`ai-macs/pkg/ui/gio/text_cost_uidebug_test.go:169-174`, branch
// `glyph-pixel-snap`). Same two buckets, same reasoning for keeping them
// separate ("CPU is what the UI goroutine spends... GPU is what the
// compositor spends and decides whether frames still fit in a refresh
// interval"), same reason the assertion is a RATIO and not an absolute
// bound: this machine builds under `pcargo` at `CARGO_BUILD_JOBS=2`,
// `nice -n 19`, serialized behind a flock shared with three other worktrees
// (`/home/wolfe/.claude/jobs/5626d2f5/tmp/pcargo`) — an absolute frame time
// here measures the load on that machine at that moment, not the software.
// ai-macs' own words for the same reasoning: "this headless context falls
// back to software EGL, so the numbers are a slow lower bound and would only
// mislead if pinned."
//
// Bucket split (this renderer's shape differs from Gio's, so the split is
// re-derived rather than copied): CPU = `ctx.run_ui(...)`, which shapes and
// lays out the galley and issues shapes — no tessellation, no GPU work.
// GPU = tessellation + texture upload + encode + submit + poll + blocking
// readback, i.e. everything `Gpu::capture` above does — mirroring
// `window.Frame()` + `window.Screenshot()` being timed together in ai-macs'
// `GPU` bucket for the same reason: `Frame()` alone is asynchronous there
// too, and only a blocking readback forces the work to actually finish
// before the clock stops.

const COST_LINES: usize = 45;
const COST_WIDTH: f32 = 1200.0;
const COST_LINE_H: f32 = 19.0;
const COST_REPS: usize = 20;
const COST_PAINT_ARMS: [u8; 4] = [1, 2, 3, 4];

/// One line of the text-heavy sample: same shape ai-macs' `textFrameOps`
/// uses (a source line with a running index, long enough to hold many
/// glyphs per row), not the same string, since this port's font/kerning
/// differ from Gio's and the actual characters do not matter to the
/// measurement.
fn cost_line(i: usize) -> String {
    format!("{i:3}  fn paint_label(ui: &mut Ui, bounds: Rect, text: &str) -> Option<Response> {{}}")
}

/// Build and paint one frame of `lines` rows, each painted `paints` times —
/// the text-heavy view C4 asks for, not the single glyph C1 probes. Returns
/// the `FullOutput` and the CPU time spent producing it (`run_ui` only).
fn cost_frame(ctx: &Context, lines: usize, paints: u8) -> (egui::FullOutput, std::time::Duration) {
    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(
        Pos2::ZERO,
        vec2(COST_WIDTH, (lines.max(1) as f32) * COST_LINE_H),
    ));
    let start = std::time::Instant::now();
    let output = ctx.run_ui(input, |ui| {
        let painter = ui.painter();
        for i in 0..lines {
            let galley =
                painter.layout_no_wrap(cost_line(i), FontId::monospace(12.0), Color32::WHITE);
            let pos = Pos2::new(0.0, i as f32 * COST_LINE_H);
            for _ in 0..paints {
                painter.galley(pos, galley.clone(), Color32::WHITE);
            }
        }
    });
    let cpu = start.elapsed();
    (output, cpu)
}

/// One (build, capture) cycle, timed the way ai-macs times `window.Frame()` +
/// `window.Screenshot()`: from just before tessellation to just after the
/// blocking readback returns, so the GPU is provably finished with the work
/// before the clock stops (submission alone is asynchronous).
fn cost_gpu_frame(
    gpu: &Gpu,
    renderer: &mut egui_wgpu::Renderer,
    ctx: &Context,
    mut output: egui::FullOutput,
    height: u32,
) -> std::time::Duration {
    let start = std::time::Instant::now();
    let pixels = gpu.capture(renderer, ctx, &output, COST_WIDTH as u32, height);
    std::hint::black_box(&pixels);
    let elapsed = start.elapsed();
    // `TexturesDelta` panics in its own destructor when dropped unapplied
    // (`epaint-0.36.1/src/textures.rs:337`); `Gpu::capture` already applied
    // every entry above. Cleared after the clock stops so it costs nothing
    // toward the measured GPU bucket.
    output.textures_delta.clear();
    elapsed
}

/// C4: CPU and GPU cost per frame at 1, 2, 3, 4 paints on a text-heavy view,
/// reported as ratios against the 1-paint arm. Opt-in (`PETRA_TEXT_COST=1`)
/// because it is a wall-clock measurement, not a pure function — it should
/// never gate the normal suite on a shared, loaded build machine (this
/// file's own module doc explains why), only be run deliberately when the
/// question "is n paints affordable" needs an answer.
///
/// Structurally an assertion, not just a print, for the same reason
/// ai-macs' `TestMeasureCoveragePassCost` is: printing a number nobody reads
/// is not a gate. The bound (3 paints costs at most 2x 1 paint's GPU time)
/// is ai-macs' own bound (`text_cost_uidebug_test.go:169-174`), reused
/// because it is the bound this repo's SPEC cites as the thing deciding
/// whether n=3 is affordable — not because ai-macs' number for repeating a
/// *path fill* is assumed to transfer to egui's *textured quads* (SPEC's own
/// "What I did NOT verify" #3 names this gap explicitly; this test is what
/// closes it, with this repo's own measurement).
#[test]
fn coverage_pass_relative_cost() {
    if std::env::var_os("PETRA_TEXT_COST").is_none() {
        println!(
            "coverage_pass_relative_cost: skipping (set PETRA_TEXT_COST=1 to run). This is a \
             wall-clock measurement, not a pure function, and must not destabilise the default \
             suite on a shared build machine."
        );
        return;
    }

    let gpu = Gpu::new();
    eprintln!(
        "coverage_pass_relative_cost: wgpu adapter {} ({:?}, {:?})",
        gpu.adapter_info.name, gpu.adapter_info.backend, gpu.adapter_info.device_type
    );

    let ctx = context_with_curve(FontColorTransferFunction::Off);
    let mut renderer =
        egui_wgpu::Renderer::new(&gpu.device, TARGET_FORMAT, egui_wgpu::RendererOptions::PREDICTABLE);

    // Fixed cost of an empty frame (0 lines), the same control ai-macs uses:
    // "submitting and reading back a frame costs the same whether or not
    // there is text in it, and on this headless context that fixed cost
    // turned out to dominate completely" — subtracted from the GPU bucket
    // only, matching ai-macs' own `gpu/reps - baseline`.
    let (warm_output, _) = cost_frame(&ctx, 0, 1);
    let _ = cost_gpu_frame(&gpu, &mut renderer, &ctx, warm_output, 1);
    let mut baseline = std::time::Duration::ZERO;
    for _ in 0..COST_REPS {
        let (output, _) = cost_frame(&ctx, 0, 1);
        baseline += cost_gpu_frame(&gpu, &mut renderer, &ctx, output, 1);
    }
    baseline /= COST_REPS as u32;
    println!("coverage_pass_relative_cost: empty frame (fixed submit+readback cost): {baseline:?}");

    let height = (COST_LINES as f32 * COST_LINE_H) as u32;
    let mut results: std::collections::BTreeMap<u8, (std::time::Duration, std::time::Duration)> =
        std::collections::BTreeMap::new();

    for paints in COST_PAINT_ARMS {
        // Warm the glyph cache and let the GPU settle before timing, the
        // same discipline ai-macs' `warm` frame follows, so cold-cache /
        // pipeline-creation one-off costs are not attributed to `paints`.
        let (warm_output, _) = cost_frame(&ctx, COST_LINES, paints);
        let _ = cost_gpu_frame(&gpu, &mut renderer, &ctx, warm_output, height);

        let mut cpu_total = std::time::Duration::ZERO;
        let mut gpu_total = std::time::Duration::ZERO;
        for _ in 0..COST_REPS {
            let (output, cpu) = cost_frame(&ctx, COST_LINES, paints);
            cpu_total += cpu;
            gpu_total += cost_gpu_frame(&gpu, &mut renderer, &ctx, output, height);
        }
        let cpu_per = cpu_total / COST_REPS as u32;
        let gpu_per = gpu_total / COST_REPS as u32;
        let gpu_text_only = gpu_per.saturating_sub(baseline);
        results.insert(paints, (cpu_per, gpu_text_only));
        println!(
            "coverage_pass_relative_cost: {paints} paint(s): CPU {cpu_per:?}/frame   GPU \
             {gpu_text_only:?}/frame text-only ({gpu_per:?} raw - {baseline:?} fixed)"
        );
    }

    let one = results[&1];
    for &n in &[2u8, 3, 4] {
        let arm = results[&n];
        let cpu_ratio = arm.0.as_secs_f64() / one.0.as_secs_f64();
        let gpu_ratio = arm.1.as_secs_f64() / one.1.as_secs_f64();
        println!(
            "coverage_pass_relative_cost: {n} paints vs 1: CPU {:+.1}%  GPU {:+.1}%",
            100.0 * (cpu_ratio - 1.0),
            100.0 * (gpu_ratio - 1.0)
        );
    }

    // The gate is relative, deliberately — see the module doc. Bound copied
    // from ai-macs' own (`text_cost_uidebug_test.go:169-174`).
    let three = results[&3];
    let gpu_ratio_3 = three.1.as_secs_f64() / one.1.as_secs_f64();
    assert!(
        gpu_ratio_3 <= 2.0,
        "3 paints costs {gpu_ratio_3:.2}x the GPU text time of 1 ({:?} vs {:?}); that is close \
         to re-rasterising the glyph per paint, not re-compositing an already-rasterised one",
        three.1,
        one.1
    );
}
