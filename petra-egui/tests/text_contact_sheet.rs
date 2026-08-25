//! A LOOKING tool for the coverage-curve / paint-repeat port (§5.2 of
//! `ignored/builds/2026-08-24-text-pipeline-port/SPEC.md`), not an
//! assertion. It renders the same two text samples under every arm of
//! `{FontColorTransferFunction::Off, TwoCoverageMinusCoverageSq} x
//! {1,2,3,4 galley paints} x {subpixel_binning off, on}`, in both shipped
//! themes, magnified 5x nearest-neighbour, and writes the result to the path
//! named by `PETRA_TEXT_SHEET`. Skips (prints and returns, asserting
//! nothing) unless that variable is set, the same discipline ai-macs'
//! `TestRenderTextSamplesToPNG` and this crate's own
//! `examples/gallery.rs`'s `PETRA_GALLERY_SHOT` use, so the normal suite
//! never pays for a GPU device it does not need.
//!
//! # Two samples, not one
//!
//! A prose line alone cannot show the defect this port exists to fix.
//! `.agents`/vlt root-cause note (`ai-macs/Ai-notes/08-15-2026/TextRendering/
//! 01-glyph-pixel-snap.md`) traces "text changes colour part way through a
//! span" to `Glyph.Y` being whole-pixel while `Glyph.X` is sub-pixel: a stem
//! landing on a pixel boundary is one dark column, the same stem half a
//! pixel over spreads across two columns at half coverage each and reads
//! grey. That is invisible in a prose sample, where every glyph is a
//! different shape at a different fractional offset and "this letter is a
//! bit lighter" reads as typography, not placement. The fix is
//! `subpixel_binning = false` (`epaint-0.36.1/src/text/font.rs:622-628`),
//! structurally the same operation as ai-macs' `snapFixed`
//! (`text_snap.go:55-57`) — it rounds the glyph's horizontal origin to a
//! whole device pixel. Coverage-pass compositing (the axis this port's other
//! leaves build) treats a *symptom* of the same defect: it saturates partial
//! coverage faster, which narrows the visible gap between a landed stem and
//! a split one without moving either. Both axes are on this sheet because a
//! reader needs to see the cause fixed and the symptom masked side by side,
//! not either alone mistaken for the whole story.
//!
//! `SAMPLE_REPEAT` is the probe that can actually show it: the same
//! character repeated, so identical shape at identical size means any
//! difference in peak ink between instances is placement, not typography —
//! ai-macs' own `glyphInkSpread` / `TestRepeatedGlyphsRenderWithUniformInk`
//! argument, ported below as [`glyph_group_peaks`] and
//! [`spread_and_stddev`]. Its numbers are printed per arm rather than baked
//! into the image, because they are a diagnostic over the row's pixels, not
//! a second thing to look at.
//!
//! **The two diagnostics disagree in sign, and that is not a bug in
//! either.** ai-macs' own measured case (`01-glyph-pixel-snap.md`, "Option 2
//! phase 1"): grid fitting took crisp columns from 26.9% to 58.6% (better on
//! `lab/greycol.py`'s column metric) while peak-ink spread rose 20 → 31
//! (worse on this file's metric), because fitting rounds stem widths per
//! glyph and different letters then round differently. Grey-column fraction
//! is a readability number; peak-ink spread is a placement-variance number.
//! Report both, never infer one from the other.
//!
//! # Effective `n`, not just the two settings
//!
//! Painting a galley k times under egui-wgpu's premultiplied source-over
//! composites to exactly `1-(1-a)^k` (SPEC §2.3), and `TwoCoverageMinus
//! CoverageSq` is `1-(1-c)^2` by construction, so the atlas curve and the
//! paint count multiply: `TwoCov` at 2 paints is `n=4`, not `n=2`. Every
//! row's label states the effective `n = curve_n * paints`, not just the two
//! settings that produced it, for exactly that reason.
//!
//! # Why this file builds its own headless `wgpu` pipeline
//!
//! The obvious path — `gorgon-petra-testkit`'s `Snapshotter` /
//! `snapshot::gpu::Gpu` — is not reachable from here without editing
//! `petra-egui/Cargo.toml` to add a dev-dependency on
//! `gorgon-petra-testkit`, which is outside this leaf's file ownership (only
//! this file and `lab/greycol.py`) and which `petra-egui`'s `tests/` files
//! do not do today (`idle_audit.rs`, `text_scripts.rs` both build their own
//! headless `egui::Context` and drive `gorgon_petra_egui::host::Host`
//! directly). `snapshot::gpu::Gpu` itself is `pub(super)` — private outside
//! `gorgon-petra-testkit::snapshot` even if the dependency existed — so the
//! reachable surface is `Snapshotter::capture`, which additionally demands a
//! `PetrifiedFrame` for its identity check (FR-040), tying it to a full
//! Petra tree pass rather than the raw, per-arm `TextOptions` control this
//! sheet needs. This is the finding the brief asked to route around rather
//! than restructure for: the render pipeline below (`Gpu::new`,
//! `Gpu::capture`, `read_back`) is a from-scratch equivalent of
//! `snapshot::gpu::Gpu`'s bring-up/render/read-back, built only from
//! `eframe::wgpu` and `eframe::egui_wgpu` — both already reachable from this
//! crate's *existing* `[dependencies]` entry on `eframe` with its `wgpu`
//! feature (`eframe::lib.rs`: `pub use {egui_wgpu, egui_wgpu::wgpu}` behind
//! `wgpu_no_default_features`, which `wgpu` implies) — so no `Cargo.toml`
//! anywhere needed to change. `wgpu-30.0.0` is the only version in the
//! workspace lockfile, the same one `gorgon-petra-testkit` depends on, so
//! this is not a second copy of a dependency, only a second, smaller caller
//! of the same crate's API.
//!
//! The one thing this pipeline does not need that `snapshot::gpu::Gpu` does:
//! multi-pass atlas reuse. Every arm here is one fresh `egui::Context`
//! rendered exactly once, so the pass's own `textures_delta.set` already
//! carries every glyph texture the pass drew, with nothing left over from
//! (and nothing to leave for) another pass.
//!
//! # Output format: PPM, not PNG
//!
//! `image`'s PNG encoder is not reachable for the same reason
//! `gorgon-petra-testkit` is not: it is a workspace dependency alias but not
//! a `[dependencies]` entry of this crate, and adding one is a `Cargo.toml`
//! edit this leaf does not own. `PETRA_TEXT_SHEET`'s value is used verbatim
//! as the output path regardless of its extension, and the bytes written are
//! a raw `P6` PPM — the same format the existing captures in
//! `ignored/experiment-artifacts/2026-08-24-glyph-rasteriser-settings/`
//! already use, and `lab/greycol.py` (via ImageMagick) reads either format
//! interchangeably.

use eframe::{egui_wgpu, wgpu};
use egui::epaint::FontColorTransferFunction;
use egui::{Context, Pos2, RawInput, Rect, vec2};
use gorgon_petra::layout::TextRequest;
use gorgon_petra::token::{Presenter, Theme, dark, light};
use gorgon_petra::tree::TextWrap;
use gorgon_petra_egui::fonts;
use gorgon_petra_egui::paint::TokenSource;
use gorgon_petra_egui::text::{GalleyShaper, Typography};
use std::io::Write as _;

/// ai-macs' own sample, kept verbatim (`text_render_uidebug_test.go:40`) so
/// this sheet's numbers stay comparable to that project's, per SPEC §5.2.
const SAMPLE_PROSE: &str = "Select a file from the Files rail.";

/// A repeated glyph with separating spaces, so contiguous inked columns
/// group into distinct glyphs rather than merging into one span — the same
/// shape ai-macs' `TestDilationReducesPerGlyphInkVariation` uses
/// (`text_snap_uidebug_test.go:128`), at a count that comfortably clears its
/// own "at least 8 groups" floor while fitting `REPEAT_W`.
const SAMPLE_REPEAT: &str = "l l l l l l l l l l l l";

/// ai-macs' cell width for the prose sample (`text_render_uidebug_test.go:41-43`).
const CELL_W: f32 = 260.0;
/// Width of the repeated-glyph cell.
const REPEAT_W: f32 = 260.0;
/// Width of the left label gutter.
const GUTTER_W: f32 = 260.0;
/// Gap between the prose cell and the repeated-glyph cell.
const GAP: f32 = 12.0;
/// ai-macs' cell height (`text_render_uidebug_test.go:41-43`).
const ROW_H: f32 = 22.0;
/// Nearest-neighbour magnification: a stem two device pixels wide must be
/// ten pixels wide in the sheet, not blurred.
const ZOOM: u32 = 5;
/// Margin above the row's own background peak before a column counts as
/// ink, for grouping the repeated-glyph diagnostic. ai-macs' `inkFloor`
/// (`text_snap_uidebug_test.go:98`) and `greyFraction`'s floor
/// (`text_diag_uidebug_test.go:76`) are a literal `8`, which is this same
/// margin over an implicit background of `0` — ai-macs' diagnostic canvas is
/// never painted with a real background fill, so its background pixels
/// genuinely are `0`. This sheet's rows carry a real `surface.base` fill
/// (`grey7`/`(18,18,18)` on the shipped dark theme, not `0`), so the margin
/// is added to the row's *measured* background peak rather than to a
/// hardcoded zero — using the literal ai-macs constant here would count
/// every background column as ink on both shipped themes and collapse every
/// row to one glyph "group" spanning the whole cell, which is not a
/// diagnostic, it is silence. See `lab/greycol.py`'s module doc for the
/// same distinction on its own, deliberately unadapted, column metric.
const INK_MARGIN: u8 = 8;

const ROW_W: f32 = GUTTER_W + CELL_W + GAP + REPEAT_W;

/// One coverage-curve arm and the atlas exponent it contributes to the
/// effective `n` a row's label states.
#[derive(Clone, Copy)]
struct Curve {
    value: FontColorTransferFunction,
    name: &'static str,
    atlas_n: u32,
}

const CURVES: [Curve; 2] = [
    Curve {
        value: FontColorTransferFunction::Off,
        name: "Off",
        atlas_n: 1,
    },
    Curve {
        value: FontColorTransferFunction::TwoCoverageMinusCoverageSq,
        name: "TwoCov",
        atlas_n: 2,
    },
];

const PAINTS: [u8; 4] = [1, 2, 3, 4];
const BINNINGS: [bool; 2] = [false, true];

/// One shipped Petra theme, named for the row label.
#[derive(Clone, Copy)]
struct ThemeArm {
    name: &'static str,
    theme: fn() -> Theme,
}

const THEMES: [ThemeArm; 2] = [
    ThemeArm {
        name: "dark",
        theme: dark,
    },
    ThemeArm {
        name: "light",
        theme: light,
    },
];

/// A minimal `pollster`-equivalent: `pollster` itself is a private
/// dependency of `eframe` and not reachable from here (see the module doc's
/// "why this file builds its own headless wgpu pipeline"). wgpu's native
/// adapter/device futures resolve after being polled a handful of times, so
/// a busy-poll with a short sleep between attempts is a faithful stand-in,
/// not a shortcut: the awaited work is the same either way, only who drives
/// the executor differs.
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
/// rather than hanging the gate lane. Matches `snapshot::gpu`'s own budget.
const WAIT: std::time::Duration = std::time::Duration::from_secs(10);

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// One wgpu device, kept across every arm: bring-up is the expensive part,
/// not the per-arm renderer this struct rebuilds nowhere (a fresh
/// `egui::Context` per arm means a fresh font atlas per arm, so a fresh
/// `egui_wgpu::Renderer` goes with it — seeded from the same device).
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY | wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap_or_else(|err| {
                panic!(
                    "no wgpu adapter for a headless text-sheet render (PRIMARY|GL backends): {err}"
                )
            });
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("gorgon-petra-egui text contact sheet"),
            required_limits: adapter.limits(),
            ..wgpu::DeviceDescriptor::default()
        }))
        .unwrap_or_else(|err| panic!("no wgpu device from the chosen adapter: {err}"));
        Self { device, queue }
    }

    /// Render one pass's shapes into a fresh `width x height` texture and
    /// read the RGBA bytes back. One renderer is built per call because one
    /// renderer is built per arm's font atlas — see the struct doc.
    fn capture(&self, ctx: &Context, output: &egui::FullOutput, width: u32, height: u32) -> Vec<u8> {
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
                label: Some("text-contact-sheet encoder"),
            });
        let user_buffers =
            renderer.update_buffers(&self.device, &self.queue, &mut encoder, &primitives, &screen);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text-contact-sheet target"),
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
                    label: Some("text-contact-sheet pass"),
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
            label: Some("text-contact-sheet readback"),
            size: (padded * height as usize) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("text-contact-sheet readback encoder"),
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

/// Per-column peak ink STRENGTH over one rectangular region of an RGBA8
/// buffer: for every pixel, how far `max(R,G,B)` departs from
/// `background_peak` in EITHER direction, then the column's strongest
/// departure over its rows.
///
/// ai-macs' own `columnPeak` (`text_snap_uidebug_test.go:87-97`) is a plain
/// `max(R,G,B)`, with no background subtraction, and that is correct *only*
/// because ai-macs' canvas is always bright ink on a near-black background —
/// a column's brightest pixel is its ink there, full stop. This sheet also
/// renders the shipped LIGHT theme, dark-on-near-white, where a plain
/// column max is nearly always just the background's own white: unless a
/// column is ink from top to bottom, some row in it is bare background, and
/// background (255ish) outshines dark ink every time. Checked against a real
/// light-theme row: a plain `max(R,G,B)` column peak found **zero** inked
/// columns across twelve rendered "l"s that are plainly visible in the same
/// pixels — the direction of "ink" flipped and the metric never noticed.
/// Measuring departure from the row's own background, in whichever
/// direction that departure runs, is the fix, and it is a superset of
/// ai-macs' formula: on a background of `0`, `|peak - 0| = peak`, the exact
/// original expression.
fn column_peaks(
    pixels: &[u8],
    full_width: u32,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    background_peak: u8,
) -> Vec<u8> {
    let bg = i32::from(background_peak);
    (0..w)
        .map(|dx| {
            (0..h)
                .map(|dy| {
                    let x = x0 + dx;
                    let y = y0 + dy;
                    let i = ((y * full_width + x) * 4) as usize;
                    let raw = pixels[i].max(pixels[i + 1]).max(pixels[i + 2]);
                    (i32::from(raw) - bg).unsigned_abs().min(255) as u8
                })
                .max()
                .unwrap_or(0)
        })
        .collect()
}

/// Group contiguous inked columns into glyphs and take each group's peak ink
/// strength — ai-macs' `glyphInkSpread` grouping loop
/// (`text_snap_uidebug_test.go:99-116`), operating on the ink-strength
/// values [`column_peaks`] already normalised against the row's background
/// (rather than on raw brightness), and floored at [`INK_MARGIN`] rather
/// than the literal `8` ai-macs uses against its always-near-zero
/// background — see both constants' docs for why.
fn glyph_group_peaks(peaks: &[u8]) -> Vec<u8> {
    let mut groups = Vec::new();
    let mut current = 0u8;
    let mut in_glyph = false;
    for &p in peaks {
        if p > INK_MARGIN {
            current = current.max(p);
            in_glyph = true;
        } else if in_glyph {
            groups.push(current);
            current = 0;
            in_glyph = false;
        }
    }
    if in_glyph {
        groups.push(current);
    }
    groups
}

/// Max-min spread (ai-macs' own statistic,
/// `TestDilationReducesPerGlyphInkVariation`) and population standard
/// deviation (this port's addition — spread alone is a two-point summary
/// of a distribution that can hold a dozen groups) over one row's
/// repeated-glyph peaks.
fn spread_and_stddev(peaks: &[u8]) -> (u8, f64) {
    let lo = *peaks.iter().min().expect("at least one glyph group");
    let hi = *peaks.iter().max().expect("at least one glyph group");
    let mean = peaks.iter().map(|&p| f64::from(p)).sum::<f64>() / peaks.len() as f64;
    let variance = peaks
        .iter()
        .map(|&p| (f64::from(p) - mean).powi(2))
        .sum::<f64>()
        / peaks.len() as f64;
    (hi - lo, variance.sqrt())
}

/// Render one arm: one fresh context, one fresh atlas, one pass. Returns the
/// row's label and its `ROW_W x ROW_H` RGBA8 pixels.
fn render_arm(
    gpu: &Gpu,
    theme_arm: ThemeArm,
    curve: Curve,
    binning: bool,
    paints: u8,
) -> (String, Vec<u8>, u8) {
    let ctx = Context::default();
    fonts::install_design_system(&ctx);

    // Both style slots, the same way `host::bind_glyph_coverage` does
    // (`petra-egui/src/host.rs:140-152`): a curve read from the wrong slot
    // because `ctx.theme()` disagreed with the slot written would be a
    // silent miss, not a loud one.
    for egui_theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(egui_theme, |style| {
            style.visuals.text_options.color_transfer_function = curve.value;
            style.visuals.text_options.subpixel_binning = binning;
        });
    }
    ctx.set_theme(egui::ThemePreference::Dark);

    let petra_theme = (theme_arm.theme)();
    let faces = fonts::design_system_faces();
    let typography = Typography::from_theme(&petra_theme, &faces);
    let mut shaper = GalleyShaper::with_typography(ctx.clone(), typography);

    let presenter = Presenter::new(petra_theme);
    let snapshot = presenter.current();
    let surface_base = snapshot
        .color("surface.base")
        .unwrap_or_else(|| panic!("{}: the shipped theme has no surface.base", theme_arm.name));
    let text_primary = snapshot
        .color("text.primary")
        .unwrap_or_else(|| panic!("{}: the shipped theme has no text.primary", theme_arm.name));

    let n = curve.atlas_n * u32::from(paints);
    let label = format!(
        "{:<5} {:<6} p{} bin={:<3} n={}",
        theme_arm.name,
        curve.name,
        paints,
        if binning { "on" } else { "off" },
        n
    );

    let mut input = RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(ROW_W, ROW_H)));

    // Galleys are shaped inside the pass, not before it: `GalleyShaper`
    // reads `ctx.fonts(...)`, and no `Fonts` exists on a context that has
    // never run a pass (`egui::Context::run()`'s own panic message says so
    // outright). One pass is all this arm gets, so there is nowhere else to
    // put the shaping.
    let output = ctx.run_ui(input, |ui| {
        let label_galley = shaper.galley(&TextRequest {
            text: &label,
            style: None,
            wrap: TextWrap::Clip,
            max_lines: Some(1),
            available_width: Some(GUTTER_W - 4.0),
        });
        let prose_galley = shaper.galley(&TextRequest {
            text: SAMPLE_PROSE,
            style: None,
            wrap: TextWrap::Clip,
            max_lines: Some(1),
            available_width: Some(CELL_W),
        });
        let repeat_galley = shaper.galley(&TextRequest {
            text: SAMPLE_REPEAT,
            style: None,
            wrap: TextWrap::Clip,
            max_lines: Some(1),
            available_width: Some(REPEAT_W),
        });

        let painter = ui.painter();
        // Slightly larger than the canvas so no rounding at the edge of the
        // background layer's clip rect leaves a sliver unpainted — the
        // capture below only ever reads the `ROW_W x ROW_H` region.
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(-4.0, -4.0), vec2(ROW_W + 8.0, ROW_H + 8.0)),
            0.0,
            surface_base,
        );
        painter.galley(Pos2::new(2.0, 2.0), label_galley.clone(), text_primary);
        // The repeat mechanic Leaf B is wiring into `paint.rs:770`: the same
        // galley, painted `paints` times at the same position, so
        // egui-wgpu's premultiplied source-over blend composites it to
        // `1-(1-a)^paints` — SPEC §2.3.
        for _ in 0..paints {
            painter.galley(Pos2::new(GUTTER_W, 0.0), prose_galley.clone(), text_primary);
        }
        for _ in 0..paints {
            painter.galley(
                Pos2::new(GUTTER_W + CELL_W + GAP, 0.0),
                repeat_galley.clone(),
                text_primary,
            );
        }
    });

    let pixels = gpu.capture(&ctx, &output, ROW_W as u32, ROW_H as u32);
    // `TexturesDelta` panics in its own destructor when dropped unapplied
    // (`epaint-0.36.1/src/textures.rs:337`) — a safeguard against silently
    // losing an atlas update, not something this test wants to trip.
    // `Gpu::capture` already applied every entry to its renderer above.
    let mut output = output;
    output.textures_delta.clear();
    let background_peak = surface_base.r().max(surface_base.g()).max(surface_base.b());
    (label, pixels, background_peak)
}

/// Nearest-neighbour magnify `src` (`src_w x src_h` RGBA8, un-zoomed) by
/// `ZOOM` into `dest` (RGB8, already `src_w*ZOOM` wide) at row `dest_y0`
/// (in destination pixels). Alpha is dropped, not composited: every arm's
/// pass paints an opaque background rect first (see `render_arm`), so every
/// texel this reads is already fully opaque.
fn blit_magnified(dest: &mut [u8], dest_w: u32, dest_y0: u32, src: &[u8], src_w: u32, src_h: u32) {
    for sy in 0..src_h {
        for sx in 0..src_w {
            let i = ((sy * src_w + sx) * 4) as usize;
            let rgb = [src[i], src[i + 1], src[i + 2]];
            for zy in 0..ZOOM {
                let dy = dest_y0 + sy * ZOOM + zy;
                for zx in 0..ZOOM {
                    let dx = sx * ZOOM + zx;
                    let di = ((dy * dest_w + dx) * 3) as usize;
                    dest[di..di + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
}

fn write_ppm(path: &std::path::Path, width: u32, height: u32, rgb: &[u8]) {
    let mut file = std::fs::File::create(path)
        .unwrap_or_else(|err| panic!("create {}: {err}", path.display()));
    write!(file, "P6\n{width} {height}\n255\n").unwrap_or_else(|err| panic!("write PPM header: {err}"));
    file.write_all(rgb)
        .unwrap_or_else(|err| panic!("write PPM body: {err}"));
}

#[test]
fn text_contact_sheet() {
    let Some(path) = std::env::var_os("PETRA_TEXT_SHEET") else {
        println!(
            "text_contact_sheet: skipping (set PETRA_TEXT_SHEET=<path> to render). Written as a \
             raw P6 PPM regardless of the path's extension -- see this file's module doc, \
             \"Output format: PPM, not PNG\", for why."
        );
        return;
    };
    let path = std::path::PathBuf::from(path);

    let gpu = Gpu::new();

    let mut rows: Vec<(String, Vec<u8>)> = Vec::new();
    let mut diagnostics: Vec<String> = Vec::new();

    for theme_arm in THEMES {
        for curve in CURVES {
            for binning in BINNINGS {
                for paints in PAINTS {
                    let (label, pixels, background_peak) =
                        render_arm(&gpu, theme_arm, curve, binning, paints);

                    let repeat_x0 = (GUTTER_W + CELL_W + GAP) as u32;
                    let peaks = column_peaks(
                        &pixels,
                        ROW_W as u32,
                        repeat_x0,
                        0,
                        REPEAT_W as u32,
                        ROW_H as u32,
                        background_peak,
                    );
                    let groups = glyph_group_peaks(&peaks);
                    if groups.len() < 4 {
                        diagnostics.push(format!(
                            "{label}: only {} glyph group(s) found in the repeated-glyph cell; \
                             spread/stddev below are unreliable at this count",
                            groups.len()
                        ));
                    } else {
                        let (spread, stddev) = spread_and_stddev(&groups);
                        diagnostics.push(format!(
                            "{label}: repeated-glyph peak-ink groups={groups:?} spread={spread} \
                             stddev={stddev:.2}"
                        ));
                    }

                    rows.push((label, pixels));
                }
            }
        }
    }

    println!(
        "text_contact_sheet: peak-ink spread (max-min) and standard deviation over the \
         repeated-glyph row's glyph groups, per arm. This tracks per-CHARACTER placement \
         variance (the glyph-pixel-snap defect: Glyph.Y whole-pixel, Glyph.X sub-pixel, so an \
         identically-shaped stem reads a different weight depending on where it lands). It moves \
         INDEPENDENTLY of, and can disagree in sign with, lab/greycol.py's grey-column fraction: \
         ai-macs' own measured case took crisp columns 26.9% -> 58.6% (better) while peak-ink \
         spread rose 20 -> 31 (worse), because grid-fitting rounds stem widths per glyph and \
         different letters round differently. Neither number predicts the other; both are \
         reported because the claim they check is not the same claim."
    );
    for line in &diagnostics {
        println!("{line}");
    }

    let row_w_px = ROW_W as u32 * ZOOM;
    let row_h_px = ROW_H as u32 * ZOOM;
    let divider_px = ZOOM;
    let rows_per_theme = CURVES.len() * BINNINGS.len() * PAINTS.len();
    let total_h_px = rows.len() as u32 * row_h_px + divider_px;

    // Mid-grey canvas so the divider between the dark-theme block and the
    // light-theme block reads as a seam, not a stray light or dark row.
    let mut sheet = vec![96u8; (row_w_px as usize) * (total_h_px as usize) * 3];
    for (i, (_, pixels)) in rows.iter().enumerate() {
        let mut y0 = i as u32 * row_h_px;
        if i >= rows_per_theme {
            y0 += divider_px;
        }
        blit_magnified(&mut sheet, row_w_px, y0, pixels, ROW_W as u32, ROW_H as u32);
    }

    write_ppm(&path, row_w_px, total_h_px, &sheet);
    println!("text_contact_sheet: wrote {}", path.display());
    println!(
        "text_contact_sheet: {} rows total, {} arms (curve x binning x paints) per theme, {} \
         themes; each row's label is baked into the image as a left gutter, format \
         \"<theme> <curve> p<paints> bin=<on|off> n=<effective n>\"",
        rows.len(),
        rows_per_theme,
        THEMES.len()
    );
}
