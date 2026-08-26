//! The draw-list interpreter: `gorgon_petra::draw` turned into `egui` shapes.
//!
//! `specs/005-petra-carbon-authoring/contracts/draw-list.md` is binding here.
//! The vocabulary lives in `gorgon-petra`, which knows no toolkit; this module
//! is the only place that knows a [`DrawList`] command becomes an
//! [`egui::Shape`], the same division of labour [`crate::image`] draws for
//! images and [`crate::triangle`] for the caret.
//!
//! # The one rounding rule (§3)
//!
//! A canvas gets **one** device rounding from the engine: its placement rect,
//! through [`round_rect`] — the same function the digest and every other
//! placement use. Inside the canvas nothing is device-rounded. Coordinates go
//! to the tessellator at sub-pixel precision, unmodified, and the only
//! exception is [`Command::Rect`] with `snap: true`, which may round its four
//! edges through [`round_coord`] and no other command may.
//!
//! **A control point has no correct rounding rule.** Snapping a bézier handle
//! or an ellipse centre deforms the curve; snapping only a shape's bounding
//! box moves the geometry relative to its own stroke. `round_rect` is defined
//! for axis-aligned rect edges and extending it to a point would invent an
//! unstated second rule — one the digest does not apply, so the picture and
//! its digest would stop describing the same thing.
//!
//! # What it reports
//!
//! [`CanvasReport`] counts what was drawn and, separately, what was
//! *declared and not drawn*: a `Sprite` whose asset does not resolve, and a
//! token that does not. Both are the canvas's version of the failure
//! [`crate::paint::PaintReport`] exists to catch — a placement that declared
//! content and emitted nothing — and neither is allowed to read as a
//! successful paint.

use std::collections::BTreeSet;

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke as EguiStroke, StrokeKind, Vec2};
use gorgon_petra::draw::{
    Affine, AssetRef, ColorRef, Command, Corners, DrawList, Fit, PathVerb, Stroke, Width,
};
use gorgon_petra::frame::{round_coord, round_rect};
use gorgon_petra::geom::{Point, Rect as PetraRect, Scale};

use crate::image::contain;
use crate::paint::TokenSource;

/// What one canvas produced.
///
/// Two halves that must not be added together: `shapes` is work that reached
/// the screen, and the other three are declarations that did not. A canvas
/// that drew ten rects and dropped a sprite is not "ten out of eleven fine",
/// it is a picture with a hole in it, and [`CanvasReport::is_complete`] says
/// so.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CanvasReport {
    /// Shapes handed to `egui`.
    pub shapes: usize,
    /// `Sprite` commands whose asset did not resolve.
    ///
    /// A term of its own rather than a line in [`CanvasReport::undrawn`],
    /// because `contracts/draw-list.md` §10 requires the count: one `Rect`
    /// plus one `Sprite` on a missing asset must not read as a complete
    /// canvas, and a set would say "an asset was missing" without saying how
    /// much of the picture is gone.
    pub missing_assets: usize,
    /// Commands that resolved everything and still emitted no shape: a
    /// degenerate ellipse, a path of one point, a `Paint` with neither a fill
    /// nor a stroke.
    pub silent: usize,
    /// Token names this canvas asked for and could not resolve, sorted.
    pub unresolved_tokens: BTreeSet<String>,
    /// Asset references this canvas asked for and could not resolve, sorted
    /// by `owner/name`.
    pub undrawn: BTreeSet<String>,
}

impl CanvasReport {
    /// Whether every command this canvas carried reached the screen.
    ///
    /// Deliberately **not** `shapes > 0`: a canvas that drew one shape and
    /// silently dropped four is the failure this whole report exists to name.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.missing_assets == 0 && self.silent == 0 && self.unresolved_tokens.is_empty()
    }
}

/// Where a `Sprite`'s pixels come from.
///
/// A trait rather than a concrete registry for the same reason
/// [`TokenSource`] is one: the interpreter has no opinion about who owns an
/// asset, and a test can answer with four pixels and no GPU. The engine side
/// of this is `contracts/draw-list.md` §9, and the decoding it requires
/// happens **above** this crate — only RGBA8 crosses the boundary.
pub trait CanvasAssets {
    /// The texture for `asset` and its size in texels, or `None` when nothing
    /// is registered under that `owner/name`.
    fn texture(&mut self, asset: &AssetRef) -> Option<(egui::TextureId, Vec2)>;
}

/// A registry that resolves nothing.
///
/// The honest default for a host that has registered no assets: every
/// `Sprite` lands in [`CanvasReport::missing_assets`] named by itself, rather
/// than a placeholder being drawn in its place.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoAssets;

impl CanvasAssets for NoAssets {
    fn texture(&mut self, _asset: &AssetRef) -> Option<(egui::TextureId, Vec2)> {
        None
    }
}

/// The existing host image registry, exposed to a canvas under the reserved
/// owner [`AssetRef::HOST_OWNER`].
///
/// `contracts/draw-list.md` §9's compatibility clause, made real rather than
/// promised: a bare image string a host already registered through
/// [`crate::image::ImageSources`] is reachable from a draw list as
/// `host/<name>`, with no second registry and no re-upload. An asset under
/// any other owner does not resolve here — that is the plugin-owned half of
/// §9 and it belongs to the asset registry work, not to this adapter.
pub struct HostImages<'a> {
    sources: &'a mut crate::image::ImageSources,
    ctx: &'a egui::Context,
}

impl<'a> HostImages<'a> {
    /// Bridge `sources` into the canvas asset vocabulary.
    pub fn new(sources: &'a mut crate::image::ImageSources, ctx: &'a egui::Context) -> Self {
        Self { sources, ctx }
    }
}

impl CanvasAssets for HostImages<'_> {
    fn texture(&mut self, asset: &AssetRef) -> Option<(egui::TextureId, Vec2)> {
        if asset.owner != AssetRef::HOST_OWNER {
            return None;
        }
        let handle = self.sources.resolve(self.ctx, &asset.name)?;
        let size = handle.size_vec2();
        Some((handle.id(), size))
    }
}

/// One frame of the interpreter's state stack.
#[derive(Clone, Copy, Debug)]
struct State {
    /// Canvas-local logical units to frame logical units.
    transform: Affine,
    /// In force, frame logical units.
    clip: Rect,
    /// Cumulative, `[0, 1]`.
    opacity: f32,
}

/// Execute `list` inside the placement rect `rect`.
///
/// `rect` is the canvas placement's rect in logical units, exactly as the
/// engine placed it. It is device-rounded **here, once**, through
/// [`round_rect`], and the rounded origin is the canvas-local origin every
/// command is drawn relative to (§3.1). Nothing else in this function rounds
/// anything, except a [`Command::Rect`] that asked to (§3.3).
///
/// The returned [`CanvasReport`] is what a caller folds into
/// [`crate::paint::PaintReport`]: a canvas with a missing asset or an
/// unresolved token must not read as a drawn placement.
pub fn paint_canvas(
    painter: &Painter,
    list: &DrawList,
    rect: PetraRect,
    scale: Scale,
    colors: &dyn TokenSource,
    assets: &mut dyn CanvasAssets,
) -> CanvasReport {
    let bounds = snapped(rect, scale);
    let mut report = CanvasReport::default();
    let base = State {
        // Translate only. The canvas's own rect is already in frame logical
        // units and its interior is authored in logical units too, so there is
        // no scale to fold in here — `Width::Device` is the one place a
        // canvas ever sees the device scale, and it sees it at the stroke.
        transform: Affine::translate(bounds.min.x, bounds.min.y),
        clip: bounds.intersect(painter.clip_rect()),
        opacity: 1.0,
    };
    let mut stack: Vec<State> = Vec::new();
    let mut state = base;

    for command in list.commands() {
        match command {
            Command::Push {
                transform,
                clip,
                opacity,
            } => {
                stack.push(state);
                if let Some(t) = transform {
                    state.transform = t.then(state.transform);
                }
                if let Some(c) = clip {
                    state.clip = state
                        .clip
                        .intersect(to_egui(state.transform.apply_rect(*c)));
                }
                if let Some(o) = opacity {
                    state.opacity *= o.clamp(0.0, 1.0);
                }
            }
            Command::Pop => {
                // `DrawList::new` refuses an unbalanced list, so the stack
                // cannot be empty here. `unwrap_or(base)` rather than a panic
                // anyway: the alternative to a defined answer is a renderer
                // that aborts a frame, and the construction bound is the place
                // that failure is supposed to be reported from.
                state = stack.pop().unwrap_or(base);
            }
            other => {
                let mut local = painter.with_clip_rect(state.clip);
                local.multiply_opacity(state.opacity);
                draw_one(
                    &local,
                    other,
                    state.transform,
                    scale,
                    colors,
                    assets,
                    &mut report,
                );
            }
        }
    }
    report
}

/// One shape command.
#[allow(clippy::too_many_arguments)]
fn draw_one(
    painter: &Painter,
    command: &Command,
    transform: Affine,
    scale: Scale,
    colors: &dyn TokenSource,
    assets: &mut dyn CanvasAssets,
    report: &mut CanvasReport,
) {
    match command {
        Command::Rect {
            rect,
            radius,
            snap,
            paint,
        } => {
            let placed = transform.apply_rect(*rect);
            // The one command allowed to snap, and the only place inside a
            // canvas that `round_coord` is called (§3.3).
            let placed = if *snap {
                snap_edges(placed, scale)
            } else {
                placed
            };
            let target = to_egui(placed);
            if !target.is_positive() {
                report.silent += 1;
                return;
            }
            let corners = to_corner_radius(*radius);
            let mut drew = false;
            if let Some(fill) = resolve(colors, paint.fill.as_ref(), report) {
                painter.add(Shape::rect_filled(target, corners, fill));
                report.shapes += 1;
                drew = true;
            }
            if let Some(stroke) = resolve_stroke(colors, paint.stroke.as_ref(), scale, report) {
                painter.add(Shape::rect_stroke(
                    target,
                    corners,
                    stroke,
                    StrokeKind::Inside,
                ));
                report.shapes += 1;
                drew = true;
            }
            if !drew {
                report.silent += 1;
            }
        }
        Command::Ellipse {
            center,
            radii,
            paint,
        } => {
            // Never rounded: an ellipse centre moved to the device grid moves
            // the whole figure relative to its own stroke.
            let placed = transform.apply(*center);
            let radius = Vec2::new(radii.w * transform.sx, radii.h * transform.sy);
            if radius.x <= 0.0 || radius.y <= 0.0 {
                report.silent += 1;
                return;
            }
            let at = Pos2::new(placed.x, placed.y);
            let mut drew = false;
            if let Some(fill) = resolve(colors, paint.fill.as_ref(), report) {
                painter.add(Shape::ellipse_filled(at, radius, fill));
                report.shapes += 1;
                drew = true;
            }
            if let Some(stroke) = resolve_stroke(colors, paint.stroke.as_ref(), scale, report) {
                painter.add(Shape::ellipse_stroke(at, radius, stroke));
                report.shapes += 1;
                drew = true;
            }
            if !drew {
                report.silent += 1;
            }
        }
        Command::Path {
            verbs,
            closed,
            paint,
        } => {
            let points = flatten(verbs, transform);
            if points.len() < 2 {
                report.silent += 1;
                return;
            }
            let mut drew = false;
            // Fill first, so a stroke of the same path sits on top of it —
            // the order every other shape in this crate draws in.
            if *closed && let Some(fill) = resolve(colors, paint.fill.as_ref(), report) {
                // `DrawList::new` already refused a non-convex filled path, so
                // this is a convex polygon by construction rather than by
                // hope. That refusal is the reason this line is allowed to
                // exist at all: `epaint` states fill is supported for convex
                // polygons only.
                painter.add(Shape::convex_polygon(
                    points.clone(),
                    fill,
                    EguiStroke::NONE,
                ));
                report.shapes += 1;
                drew = true;
            }
            if let Some(stroke) = resolve_stroke(colors, paint.stroke.as_ref(), scale, report) {
                painter.add(if *closed {
                    Shape::closed_line(points, stroke)
                } else {
                    Shape::line(points, stroke)
                });
                report.shapes += 1;
                drew = true;
            }
            if !drew {
                report.silent += 1;
            }
        }
        Command::Sprite {
            asset,
            dst,
            src,
            fit,
            tint,
        } => {
            let Some((texture, natural)) = assets.texture(asset) else {
                // Counted, named, and nothing drawn. Drawing a placeholder
                // here would put a picture on the screen the digest never saw.
                report.missing_assets += 1;
                report.undrawn.insert(asset.wire());
                return;
            };
            let target = to_egui(transform.apply_rect(*dst));
            if !target.is_positive() || natural.x <= 0.0 || natural.y <= 0.0 {
                report.silent += 1;
                return;
            }
            let uv = match src {
                // Source rects are in source pixels; `egui` wants normalized
                // texture coordinates, so the division is by the texture's own
                // size and not by anything on screen.
                Some(sub) => Rect::from_min_size(
                    Pos2::new(sub.x / natural.x, sub.y / natural.y),
                    Vec2::new(sub.w / natural.x, sub.h / natural.y),
                ),
                None => Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            };
            // `contain` is `crate::image`'s existing letterbox rule, shared
            // rather than rewritten: a canvas sprite and an `image` node fit
            // their offers by one function or they drift apart.
            let target = match fit {
                Fit::Fill => target,
                Fit::Contain => contain(target, Vec2::new(uv.width(), uv.height()) * natural),
            };
            let colour = match tint {
                Some(reference) => match resolve(colors, Some(reference), report) {
                    Some(colour) => colour,
                    // The tint did not resolve. Drawing the asset untinted
                    // would be a different picture from the one declared, so
                    // the sprite is not drawn and the miss is already recorded
                    // in `unresolved_tokens`.
                    None => {
                        report.silent += 1;
                        return;
                    }
                },
                None => Color32::WHITE,
            };
            painter.add(Shape::image(texture, target, uv, colour));
            report.shapes += 1;
        }
        // Handled by the caller, which owns the state stack.
        Command::Push { .. } | Command::Pop => unreachable!("the stack is the caller's"),
    }
}

/// The canvas's placement rect on the device grid, back in logical units.
///
/// The **only** device rounding a canvas gets from the engine (§3.1), and it
/// is [`round_rect`] — the same function `crate::frame::digest` hashes rects
/// through, which is what makes the digest describe the box that was actually
/// painted.
///
/// Byte-for-byte what `paint.rs`'s private `to_egui_snapped` does. The two
/// should be one function once T135 wires a canvas into the paint pass; until
/// then this file cannot reach that one, and a canvas that rounded its box a
/// second, different way would be worse than the duplication.
#[must_use]
pub fn snapped(rect: PetraRect, scale: Scale) -> Rect {
    let d = round_rect(rect, scale);
    let f = scale.factor();
    #[allow(clippy::cast_precision_loss)]
    Rect::from_min_size(
        Pos2::new(d.x as f32 / f, d.y as f32 / f),
        Vec2::new(d.w as f32 / f, d.h as f32 / f),
    )
}

/// A rect whose four edges are on the device grid, in logical units.
///
/// Reached only by [`Command::Rect`] with `snap: true`. Edges, not
/// origin-plus-size, for the reason `frame::rounding` gives: two rects sharing
/// an edge in logical units must share it in device pixels, or a canvas grows
/// a seam at fractional scale.
fn snap_edges(rect: PetraRect, scale: Scale) -> PetraRect {
    let f = scale.factor();
    #[allow(clippy::cast_precision_loss)]
    let edge = |v: f32| round_coord(v, scale) as f32 / f;
    let x0 = edge(rect.x);
    let y0 = edge(rect.y);
    let x1 = edge(rect.right());
    let y1 = edge(rect.bottom());
    PetraRect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

/// Every point of `verbs` under `transform`, béziers flattened by `epaint`'s
/// own subdivision.
///
/// `epaint`'s flattener rather than one written here: the tessellator is going
/// to subdivide these curves anyway, and two subdivision rules in one pipeline
/// is how a stroke and the fill under it end up with different silhouettes.
/// Control points are transformed and never rounded.
fn flatten(verbs: &[PathVerb], transform: Affine) -> Vec<Pos2> {
    let at = |p: Point| {
        let moved = transform.apply(p);
        Pos2::new(moved.x, moved.y)
    };
    let mut out: Vec<Pos2> = Vec::with_capacity(verbs.len());
    for verb in verbs {
        match verb {
            PathVerb::MoveTo(p) | PathVerb::LineTo(p) => out.push(at(*p)),
            PathVerb::QuadTo { ctrl, to } => {
                let from = out.last().copied().unwrap_or_else(|| at(*ctrl));
                let curve = egui::epaint::QuadraticBezierShape::from_points_stroke(
                    [from, at(*ctrl), at(*to)],
                    false,
                    Color32::TRANSPARENT,
                    EguiStroke::NONE,
                );
                // `flatten` repeats the start point, which is already in
                // `out`.
                out.extend(curve.flatten(None).into_iter().skip(1));
            }
            PathVerb::CubicTo { c1, c2, to } => {
                let from = out.last().copied().unwrap_or_else(|| at(*c1));
                let curve = egui::epaint::CubicBezierShape::from_points_stroke(
                    [from, at(*c1), at(*c2), at(*to)],
                    false,
                    Color32::TRANSPARENT,
                    EguiStroke::NONE,
                );
                out.extend(curve.flatten(None).into_iter().skip(1));
            }
            // The `closed` flag on the command is what closes the shape;
            // repeating the first point here would put a zero-length segment
            // in every closed path.
            PathVerb::Close => {}
        }
    }
    out
}

/// A canvas-local rect in frame logical units, as an `egui` rect.
fn to_egui(rect: PetraRect) -> Rect {
    Rect::from_min_size(
        Pos2::new(rect.x, rect.y),
        Vec2::new(rect.w.max(0.0), rect.h.max(0.0)),
    )
}

/// Four logical radii as `epaint`'s four-corner form.
fn to_corner_radius(corners: Corners) -> egui::CornerRadius {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = |v: f32| v.clamp(0.0, 255.0).round() as u8;
    egui::CornerRadius {
        nw: byte(corners.top_left),
        ne: byte(corners.top_right),
        se: byte(corners.bottom_right),
        sw: byte(corners.bottom_left),
    }
}

/// Resolve a colour reference, recording an unresolved token by name.
///
/// A miss draws nothing rather than guessing, the same rule
/// `paint.rs`'s `resolve_or_record` follows for a fill: a shape in a colour
/// nobody chose is worse than a shape that is not there, and the report says
/// which token was missing either way.
fn resolve(
    colors: &dyn TokenSource,
    reference: Option<&ColorRef>,
    report: &mut CanvasReport,
) -> Option<Color32> {
    match reference? {
        ColorRef::Token(name) => {
            let colour = colors.color(name);
            if colour.is_none() {
                report.unresolved_tokens.insert(name.clone());
            }
            colour
        }
        ColorRef::Rgba([r, g, b, a]) => Some(Color32::from_rgba_unmultiplied(*r, *g, *b, *a)),
    }
}

/// Resolve a stroke, turning `Width::Device` into logical units at `scale`.
///
/// The resolution happens here, at execution, rather than at authoring: that
/// is what lets one list draw a one-device-pixel hairline at every scale
/// without the list's own bytes — and so its digest — moving when the scale
/// does.
fn resolve_stroke(
    colors: &dyn TokenSource,
    stroke: Option<&Stroke>,
    scale: Scale,
    report: &mut CanvasReport,
) -> Option<EguiStroke> {
    let Stroke { width, color } = stroke?;
    let colour = resolve(colors, Some(color), report)?;
    let logical = match width {
        Width::Logical(units) => *units,
        Width::Device(pixels) => pixels / scale.factor(),
    };
    if logical <= 0.0 {
        return None;
    }
    Some(EguiStroke::new(logical, colour))
}

#[cfg(test)]
mod tests {
    use super::{CanvasAssets, CanvasReport, HostImages, NoAssets, paint_canvas, snapped};
    use egui::{Color32, Shape, Vec2};
    use gorgon_petra::draw::{
        Affine, AssetRef, ColorRef, Command, Corners, DrawList, Fit, Paint, PathVerb, Stroke, Width,
    };
    use gorgon_petra::geom::{Point, Rect as PetraRect, Scale, Size};
    use std::collections::BTreeMap;

    /// A `TokenSource` that knows two colours and nothing else, so an
    /// unresolved token is a real miss rather than a theme gap.
    struct TwoColours(BTreeMap<String, Color32>);

    impl TwoColours {
        fn new() -> Self {
            let mut map = BTreeMap::new();
            map.insert("surface.raised".to_owned(), Color32::from_rgb(20, 20, 24));
            map.insert("text.primary".to_owned(), Color32::from_rgb(230, 230, 235));
            Self(map)
        }
    }

    impl crate::paint::TokenSource for TwoColours {
        fn color(&self, token: &str) -> Option<Color32> {
            self.0.get(token).copied()
        }
    }

    /// A headless context that hands back its texture deltas on drop, the
    /// same harness shape `triangle.rs` and `paint.rs` use.
    struct Headless(egui::Context);

    impl Headless {
        fn new() -> Self {
            let ctx = egui::Context::default();
            Self::pass(&ctx);
            Self(ctx)
        }

        fn painter(&self) -> egui::Painter {
            self.0.layer_painter(egui::LayerId::new(
                egui::Order::Background,
                egui::Id::new("petra-canvas-test"),
            ))
        }

        fn pass(ctx: &egui::Context) {
            ctx.run_ui(egui::RawInput::default(), |_| {})
                .drop_without_applying_deltas();
        }
    }

    impl Drop for Headless {
        fn drop(&mut self) {
            Self::pass(&self.0);
        }
    }

    /// Run `list` and hand back both the report and the shapes `egui`
    /// actually received, so an assertion can be about geometry rather than
    /// about a counter this module incremented itself.
    fn run(list: &DrawList, rect: PetraRect, scale: Scale) -> (CanvasReport, Vec<Shape>) {
        run_with(list, rect, scale, &mut NoAssets)
    }

    fn run_with(
        list: &DrawList,
        rect: PetraRect,
        scale: Scale,
        assets: &mut dyn CanvasAssets,
    ) -> (CanvasReport, Vec<Shape>) {
        let headless = Headless::new();
        let painter = headless.painter();
        let colours = TwoColours::new();
        let report = paint_canvas(&painter, list, rect, scale, &colours, assets);
        let shapes = headless
            .0
            .graphics(|g| {
                g.get(painter.layer_id()).map(|list| {
                    list.all_entries()
                        .map(|c| c.shape.clone())
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        (report, shapes)
    }

    fn rect_at(x: f32, y: f32, w: f32, h: f32, snap: bool) -> Command {
        Command::Rect {
            rect: PetraRect::new(x, y, w, h),
            radius: Corners::SQUARE,
            snap,
            paint: Paint::filled(ColorRef::Token("surface.raised".to_owned())),
        }
    }

    fn first_rect(shapes: &[Shape]) -> egui::epaint::RectShape {
        shapes
            .iter()
            .find_map(|s| match s {
                Shape::Rect(r) => Some(r.clone()),
                _ => None,
            })
            .expect("the fixture draws a rect")
    }

    /// T129: the canvas rect rounds exactly once, through `round_rect`, and
    /// the rounded origin is where canvas-local `(0, 0)` lands.
    ///
    /// Hand-computed at 1.25 scale: `10.3 * 1.25 = 12.875` rounds to `13`
    /// device pixels, which is `10.4` logical units. A shape at canvas-local
    /// `(0, 0)` therefore lands at `10.4`, not at `10.3`.
    #[test]
    fn the_canvas_rect_rounds_once_and_the_interior_follows_it() {
        let scale = Scale::new(1.25).unwrap();
        let bounds = snapped(PetraRect::new(10.3, 20.7, 100.0, 50.0), scale);
        assert!((bounds.min.x - 10.4).abs() < 1e-4, "{bounds:?}");
        assert!((bounds.min.y - 20.8).abs() < 1e-4, "{bounds:?}");

        let list = DrawList::new(vec![rect_at(0.0, 0.0, 8.0, 8.0, false)]).unwrap();
        let (report, shapes) = run(&list, PetraRect::new(10.3, 20.7, 100.0, 50.0), scale);
        assert_eq!(report.shapes, 1);
        assert!(report.is_complete(), "{report:?}");
        let drawn = first_rect(&shapes);
        assert!((drawn.rect.min.x - 10.4).abs() < 1e-4, "{drawn:?}");
        assert!((drawn.rect.min.y - 20.8).abs() < 1e-4, "{drawn:?}");
    }

    /// A control point is never rounded, at any scale. This is the half §3
    /// spends its whole argument on: an interior coordinate that landed on
    /// the device grid would be a picture the digest — which rounds nothing
    /// inside a canvas — does not describe.
    #[test]
    fn no_interior_coordinate_is_ever_rounded() {
        let scale = Scale::new(1.25).unwrap();
        // Canvas placed on the grid, so the origin contributes nothing and
        // any movement seen below is the interior's own.
        let at = PetraRect::new(0.0, 0.0, 200.0, 200.0);
        let list = DrawList::new(vec![
            Command::Ellipse {
                center: Point::new(13.37, 21.73),
                radii: Size::new(4.19, 2.71),
                paint: Paint::filled(ColorRef::Token("text.primary".to_owned())),
            },
            Command::Path {
                verbs: vec![
                    PathVerb::MoveTo(Point::new(1.11, 2.22)),
                    PathVerb::LineTo(Point::new(31.37, 44.79)),
                ],
                closed: false,
                paint: Paint::stroked(Stroke {
                    width: Width::Logical(1.0),
                    color: ColorRef::Token("text.primary".to_owned()),
                }),
            },
            // Unsnapped, so its edges keep their fractions too.
            rect_at(5.55, 6.66, 7.77, 8.88, false),
        ])
        .unwrap();
        let (report, shapes) = run(&list, at, scale);
        assert_eq!(report.shapes, 3, "{report:?}");

        let ellipse = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Ellipse(e) => Some(*e),
                _ => None,
            })
            .expect("the fixture draws an ellipse");
        assert_eq!(ellipse.center, egui::pos2(13.37, 21.73));
        assert_eq!(ellipse.radius, Vec2::new(4.19, 2.71));

        let path = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Path(p) => Some(p.clone()),
                _ => None,
            })
            .expect("the fixture draws a path");
        assert_eq!(path.points[0], egui::pos2(1.11, 2.22));
        assert_eq!(path.points[1], egui::pos2(31.37, 44.79));

        let drawn = first_rect(&shapes);
        assert_eq!(drawn.rect.min, egui::pos2(5.55, 6.66));
    }

    /// `snap: true` is the one exception, and it applies to the rect's four
    /// edges only. `5.55 * 1.25 = 6.9375` rounds to `7` device pixels, which
    /// is `5.6` logical units.
    #[test]
    fn only_a_snapping_rect_reaches_the_device_grid() {
        let scale = Scale::new(1.25).unwrap();
        let at = PetraRect::new(0.0, 0.0, 200.0, 200.0);
        let list = DrawList::new(vec![rect_at(5.55, 6.66, 7.77, 8.88, true)]).unwrap();
        let (_, shapes) = run(&list, at, scale);
        let drawn = first_rect(&shapes);
        assert!((drawn.rect.min.x - 5.6).abs() < 1e-4, "{:?}", drawn.rect);
        // 6.66 * 1.25 = 8.325 -> 8 device -> 6.4 logical.
        assert!((drawn.rect.min.y - 6.4).abs() < 1e-4, "{:?}", drawn.rect);

        // The same rect unsnapped keeps its fractions, so the assertion above
        // is about `snap` and not about the scale.
        let plain = DrawList::new(vec![rect_at(5.55, 6.66, 7.77, 8.88, false)]).unwrap();
        let (_, plain_shapes) = run(&plain, at, scale);
        assert_eq!(first_rect(&plain_shapes).rect.min, egui::pos2(5.55, 6.66));
    }

    /// `Width::Device` resolves against the frame's scale at execution, so
    /// one list is one device pixel wide at every scale.
    #[test]
    fn a_device_width_is_one_device_pixel_at_every_scale() {
        for factor in [1.0_f32, 1.25, 1.5, 2.0] {
            let scale = Scale::new(factor).unwrap();
            let list = DrawList::new(vec![Command::Path {
                verbs: vec![
                    PathVerb::MoveTo(Point::new(0.0, 0.0)),
                    PathVerb::LineTo(Point::new(20.0, 0.0)),
                ],
                closed: false,
                paint: Paint::stroked(Stroke {
                    width: Width::Device(1.0),
                    color: ColorRef::Token("text.primary".to_owned()),
                }),
            }])
            .unwrap();
            let (_, shapes) = run(&list, PetraRect::new(0.0, 0.0, 50.0, 50.0), scale);
            let path = shapes
                .iter()
                .find_map(|s| match s {
                    Shape::Path(p) => Some(p.clone()),
                    _ => None,
                })
                .expect("the fixture strokes a path");
            let width = path.stroke.width;
            assert!(
                (width * factor - 1.0).abs() < 1e-4,
                "at scale {factor} the stroke is {width} logical units, which is \
                 {} device pixels",
                width * factor
            );
        }
    }

    /// Push composes and Pop restores: a rect drawn inside a translate lands
    /// offset, and the same rect after the Pop does not.
    #[test]
    fn the_state_stack_composes_and_restores() {
        let list = DrawList::new(vec![
            Command::Push {
                transform: Some(Affine::translate(10.0, 20.0)),
                clip: None,
                opacity: None,
            },
            rect_at(1.0, 2.0, 4.0, 4.0, false),
            Command::Pop,
            rect_at(1.0, 2.0, 4.0, 4.0, false),
        ])
        .unwrap();
        let (report, shapes) = run(&list, PetraRect::new(0.0, 0.0, 100.0, 100.0), Scale::ONE);
        assert_eq!(report.shapes, 2);
        let rects: Vec<egui::Pos2> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Rect(r) => Some(r.rect.min),
                _ => None,
            })
            .collect();
        assert_eq!(rects, [egui::pos2(11.0, 22.0), egui::pos2(1.0, 2.0)]);
    }

    /// A nested Push multiplies opacity rather than replacing it.
    #[test]
    fn nested_pushes_multiply_opacity() {
        let list = DrawList::new(vec![
            Command::Push {
                transform: None,
                clip: None,
                opacity: Some(0.5),
            },
            Command::Push {
                transform: None,
                clip: None,
                opacity: Some(0.5),
            },
            rect_at(0.0, 0.0, 4.0, 4.0, false),
            Command::Pop,
            Command::Pop,
        ])
        .unwrap();
        let (_, shapes) = run(&list, PetraRect::new(0.0, 0.0, 100.0, 100.0), Scale::ONE);
        let drawn = first_rect(&shapes);
        let solid = TwoColours::new().0["surface.raised"];
        assert!(
            drawn.fill.a() < solid.a() / 2,
            "two 50% pushes must compose to 25%, got alpha {} against {}",
            drawn.fill.a(),
            solid.a()
        );
    }

    /// A token that does not resolve draws nothing and is named. Guessing a
    /// colour would put a shape on screen in a colour nobody chose.
    #[test]
    fn an_unresolved_token_draws_nothing_and_is_named() {
        let list = DrawList::new(vec![Command::Rect {
            rect: PetraRect::new(0.0, 0.0, 4.0, 4.0),
            radius: Corners::SQUARE,
            snap: false,
            paint: Paint::filled(ColorRef::Token("surface.nonexistent".to_owned())),
        }])
        .unwrap();
        let (report, shapes) = run(&list, PetraRect::new(0.0, 0.0, 100.0, 100.0), Scale::ONE);
        assert_eq!(report.shapes, 0);
        assert_eq!(report.silent, 1);
        assert!(report.unresolved_tokens.contains("surface.nonexistent"));
        assert!(!report.is_complete());
        assert!(!shapes.iter().any(|s| matches!(s, Shape::Rect(_))));
    }

    /// A literal colour needs no theme, which is the whole reason
    /// `ColorRef::Rgba` exists.
    #[test]
    fn a_literal_colour_resolves_without_a_theme() {
        let list = DrawList::new(vec![Command::Rect {
            rect: PetraRect::new(0.0, 0.0, 4.0, 4.0),
            radius: Corners::all(3.0),
            snap: false,
            paint: Paint::filled(ColorRef::Rgba([10, 20, 30, 255])),
        }])
        .unwrap();
        let (report, shapes) = run(&list, PetraRect::new(0.0, 0.0, 100.0, 100.0), Scale::ONE);
        assert!(report.is_complete(), "{report:?}");
        let drawn = first_rect(&shapes);
        assert_eq!(drawn.fill, Color32::from_rgba_unmultiplied(10, 20, 30, 255));
        assert_eq!(drawn.corner_radius.nw, 3);
    }

    /// A `Sprite` whose asset does not resolve draws nothing, is counted in
    /// its own term, and fails the report (`contracts/draw-list.md` §10).
    ///
    /// The `Rect` beside it is the point: one drawn shape and one missing
    /// asset must not read as a complete canvas.
    #[test]
    fn a_missing_asset_is_its_own_term_and_fails_the_report() {
        let list = DrawList::new(vec![
            rect_at(0.0, 0.0, 4.0, 4.0, false),
            Command::Sprite {
                asset: AssetRef::host("cat.png"),
                dst: PetraRect::new(0.0, 0.0, 16.0, 16.0),
                src: None,
                fit: Fit::Contain,
                tint: None,
            },
        ])
        .unwrap();
        let (report, _) = run(&list, PetraRect::new(0.0, 0.0, 100.0, 100.0), Scale::ONE);
        assert_eq!(report.shapes, 1, "the rect drew");
        assert_eq!(report.missing_assets, 1);
        assert!(report.undrawn.contains("host/cat.png"));
        assert!(
            !report.is_complete(),
            "one Rect plus one Sprite on a missing asset is not a complete canvas"
        );
    }

    /// The other half: a registered asset draws, through the *existing* host
    /// image registry rather than a second one.
    #[test]
    fn a_registered_host_asset_draws_through_the_existing_registry() {
        let headless = Headless::new();
        let mut sources = crate::image::ImageSources::new();
        sources.register(
            "cat.png",
            crate::image::ImagePixels::new(2, 2, vec![255; 2 * 2 * 4]),
        );
        let list = DrawList::new(vec![Command::Sprite {
            asset: AssetRef::host("cat.png"),
            dst: PetraRect::new(0.0, 0.0, 16.0, 16.0),
            src: None,
            fit: Fit::Fill,
            tint: None,
        }])
        .unwrap();
        let painter = headless.painter();
        let colours = TwoColours::new();
        let mut assets = HostImages::new(&mut sources, &headless.0);
        let report = paint_canvas(
            &painter,
            &list,
            PetraRect::new(0.0, 0.0, 100.0, 100.0),
            Scale::ONE,
            &colours,
            &mut assets,
        );
        assert_eq!(report.missing_assets, 0, "{report:?}");
        assert_eq!(report.shapes, 1);
        assert!(report.is_complete(), "{report:?}");

        // An asset under any other owner is not the host's, and does not
        // resolve here.
        let foreign = DrawList::new(vec![Command::Sprite {
            asset: AssetRef::new("plugin.weather", "cat.png"),
            dst: PetraRect::new(0.0, 0.0, 16.0, 16.0),
            src: None,
            fit: Fit::Fill,
            tint: None,
        }])
        .unwrap();
        let mut assets = HostImages::new(&mut sources, &headless.0);
        let report = paint_canvas(
            &painter,
            &foreign,
            PetraRect::new(0.0, 0.0, 100.0, 100.0),
            Scale::ONE,
            &colours,
            &mut assets,
        );
        assert_eq!(report.missing_assets, 1);
        assert!(report.undrawn.contains("plugin.weather/cat.png"));
    }

    /// A filled closed path reaches `epaint` as a convex polygon, which is
    /// only sound because `DrawList::new` refused a non-convex one.
    #[test]
    fn a_filled_closed_path_draws_as_a_convex_polygon() {
        let list = DrawList::new(vec![Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(0.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 10.0)),
            ],
            closed: true,
            paint: Paint::filled(ColorRef::Token("text.primary".to_owned())),
        }])
        .unwrap();
        let (report, shapes) = run(&list, PetraRect::new(0.0, 0.0, 100.0, 100.0), Scale::ONE);
        assert_eq!(report.shapes, 1);
        let path = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Path(p) => Some(p.clone()),
                _ => None,
            })
            .expect("the fixture fills a path");
        assert!(path.fill != Color32::TRANSPARENT);
        assert_eq!(path.points.len(), 3);
    }

    /// A bézier reaches the tessellator as more points than it was written
    /// with — it was flattened — and both endpoints survive exactly.
    #[test]
    fn a_bezier_is_flattened_without_moving_its_endpoints() {
        let list = DrawList::new(vec![Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(0.0, 0.0)),
                PathVerb::CubicTo {
                    c1: Point::new(10.0, 30.0),
                    c2: Point::new(30.0, 30.0),
                    to: Point::new(40.0, 0.0),
                },
            ],
            closed: false,
            paint: Paint::stroked(Stroke {
                width: Width::Logical(1.0),
                color: ColorRef::Token("text.primary".to_owned()),
            }),
        }])
        .unwrap();
        let (_, shapes) = run(&list, PetraRect::new(0.0, 0.0, 100.0, 100.0), Scale::ONE);
        let path = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Path(p) => Some(p.clone()),
                _ => None,
            })
            .expect("the fixture strokes a path");
        assert!(
            path.points.len() > 2,
            "a cubic must be subdivided, got {} point(s)",
            path.points.len()
        );
        assert_eq!(path.points[0], egui::pos2(0.0, 0.0));
        assert_eq!(*path.points.last().unwrap(), egui::pos2(40.0, 0.0));
    }

    /// An empty list draws nothing and reports a complete pass: there was
    /// nothing to fail at.
    #[test]
    fn an_empty_list_is_complete_and_silent() {
        let list = DrawList::new(Vec::new()).unwrap();
        let (report, shapes) = run(&list, PetraRect::new(0.0, 0.0, 10.0, 10.0), Scale::ONE);
        assert_eq!(report, CanvasReport::default());
        assert!(report.is_complete());
        assert!(shapes.is_empty());
    }

    /// A shape with neither a fill nor a stroke is silent, not drawn. A
    /// canvas that counted it would report a picture nothing painted.
    #[test]
    fn a_shape_with_no_paint_is_silent() {
        let list = DrawList::new(vec![Command::Rect {
            rect: PetraRect::new(0.0, 0.0, 4.0, 4.0),
            radius: Corners::SQUARE,
            snap: false,
            paint: Paint::default(),
        }])
        .unwrap();
        let (report, _) = run(&list, PetraRect::new(0.0, 0.0, 100.0, 100.0), Scale::ONE);
        assert_eq!(report.shapes, 0);
        assert_eq!(report.silent, 1);
        assert!(!report.is_complete());
    }
}
