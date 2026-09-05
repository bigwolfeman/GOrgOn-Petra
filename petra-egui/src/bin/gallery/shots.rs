//! Drive a catalog page headlessly and photograph what comes out.
//!
//! # Why this exists
//!
//! The operator walked all 42 pages of the running catalog on 2026-09-04 and
//! found 39 wrong, under a fully green suite. Two things went wrong before
//! that, and this module answers both.
//!
//! The first is that a frame record is not a picture. A rect touching another
//! rect with no gap is a legal frame record, and so is a label sitting on top
//! of its neighbour. `catalog.rs`'s
//! `every_built_page_rasterizes_to_more_than_one_colour` fixed that for the
//! resting state by rasterizing through a real `wgpu` device.
//!
//! The second is that a resting state is not the component. Nine of the
//! operator's rows read "does not work at all" — a dropdown that never opens,
//! a tooltip that never appears, a field that never takes a keystroke. A
//! photograph of the closed trigger cannot tell those apart from a working
//! one, so the audit that only photographed resting states was blind to the
//! largest single class of defect on the list.
//!
//! [`gorgon_petra_egui::inject`] already carries the whole action vocabulary —
//! click, drag, hover, focus, text-edit, scroll, key — and routes it through
//! the same translator a physical device goes through. This module is that
//! driver bolted to the camera, so proving a dropdown opens costs three lines
//! instead of forty:
//!
//! ```ignore
//! let mut cam = Camera::on("Dropdown");
//! cam.click("dropdown-trigger");
//! cam.shoot("11-dropdown-open");
//! ```
//!
//! Then **look at the PNG**. The capture is the point; an assertion that the
//! page still rasterizes is not a substitute for reading it.
//!
//! # No window, ever
//!
//! [`gorgon_petra_testkit::snapshot::Snapshotter`] rasterizes through a
//! headless `wgpu` device. Nothing here opens a window, reads `DISPLAY`, or
//! touches the operator's compositor — which matters, because he is working
//! on that desktop while these tests run.

#![cfg(test)]
// Each fix wave uses the two or three helpers its own pages need. A helper no
// wave has reached yet reads as dead here, which is the same arrangement, and
// the same reason, as `cat.rs`.
#![allow(dead_code)]

use egui::{Context, Pos2, RawInput};
use gorgon_petra::frame::{CaretPaint, PaintContent, PetrifiedFrame};
use gorgon_petra::geom::{Point, Rect, Size};
use gorgon_petra::input::{KeyCode, Modifiers};
use gorgon_petra_egui::host::{Host, default_presenter};
use gorgon_petra_egui::inject::{Action, Target, inject_action};
use gorgon_petra_testkit::snapshot::Snapshotter;

use crate::catalog::{Catalog, WINDOW};

/// Device pixels per logical point for a captured page.
///
/// **Two reasons this is 2 and not 1.**
///
/// The first is that half-pixel defects only exist at 1x, and they are exactly
/// the ones a placement rect cannot show. The radio's inner dot centred on a
/// 4.5 inset, which is a whole 9 device pixels at 2x and a snapped-off-centre
/// mark at 1x. Capturing at both is how a fix is told apart from a rounding
/// accident.
///
/// The second is comparison. The IBM Carbon reference app under
/// `ignored/carbon-ref/` captures at device pixel ratio 2. A Petra shot and a
/// Carbon shot of the same component have to be the same size, or every
/// side-by-side needs a resample first and a resample is where a one-pixel
/// difference goes to die.
const CAPTURE_SCALE: f32 = 2.0;

/// A catalog page, a driver for it, and a camera pointed at it.
pub struct Camera {
    ctx: Context,
    host: Host<Catalog>,
    shooter: Snapshotter,
    /// The page's row name, so a failure says which page it was on.
    page: String,
    /// Everything the app has asked egui to put on the clipboard since this
    /// camera opened. A driven `Copy` control is otherwise invisible: the
    /// string leaves through `PlatformOutput`, not through the frame.
    clipboard: Vec<String>,
}

impl Camera {
    /// Open the page whose inventory row is named `component`, and settle it.
    ///
    /// Two passes: the first registers the font atlas, the second is the one a
    /// caller drives and photographs. Reduced motion is set so a shot taken
    /// straight after a click shows the settled result rather than something
    /// a third of the way through a transition.
    ///
    /// # Panics
    /// If no inventory row is named `component`. The message lists the rows,
    /// because the usual cause is a name that differs from Carbon's by a
    /// capital letter.
    pub fn on(component: &str) -> Self {
        Self::at_scale(component, CAPTURE_SCALE)
    }

    /// [`Camera::on`], at a chosen device pixel ratio.
    ///
    /// Use it to photograph the same page at 1 and at 2 when a defect is
    /// suspected to be a rounding artifact rather than a layout one. A mark
    /// that is centred at 2x and off centre at 1x is being snapped, not
    /// mis-placed, and the two pictures say so where one cannot.
    ///
    /// # Panics
    /// If no inventory row is named `component`.
    pub fn at_scale(component: &str, scale: f32) -> Self {
        let app = Catalog::on_page(component);
        let ctx = headless();
        ctx.set_pixels_per_point(scale);
        let mut host = Host::new(&ctx, app, default_presenter());
        ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx))
            .drop_without_applying_deltas();
        crate::catalog::seat_index_focus(&mut host);
        host.set_reduced_motion(true);
        let mut cam = Self {
            ctx,
            host,
            shooter: Snapshotter::new(),
            page: component.to_owned(),
            clipboard: Vec::new(),
        };
        cam.settle();
        cam
    }

    /// Run one pass with no input, so the frame matches the current state.
    fn settle(&mut self) {
        let ctx = self.ctx.clone();
        ctx.run_ui(sized(RawInput::default()), |_| self.host.pass(&ctx))
            .drop_without_applying_deltas();
    }

    /// Resolve a placement id by its tail.
    ///
    /// Callers name the node key they wrote in the component — `"dd-trigger"` —
    /// not the full `/root/body/.../dd-trigger` path, so a later reshuffle of
    /// the page's nesting does not silently turn a driving step into a no-op.
    ///
    /// # Panics
    /// If nothing matches, or if more than one placement does. Both messages
    /// list candidates: an ambiguous tail wants a longer one, and a missing
    /// tail is usually a key that never made it into the tree at all, which is
    /// itself the defect worth seeing.
    pub fn id(&self, tail: &str) -> String {
        let frame = self.frame();
        let hits: Vec<&str> = frame
            .placements
            .iter()
            .map(|p| p.id.as_str())
            .filter(|id| id.ends_with(tail) || id.ends_with(&format!("{tail}/")))
            .collect();
        match hits.len() {
            1 => hits[0].to_owned(),
            0 => panic!(
                "{}: no placement ends with {tail:?}. Placed ids:\n  {}",
                self.page,
                frame
                    .placements
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>()
                    .join("\n  ")
            ),
            _ => panic!(
                "{}: {tail:?} is ambiguous, {} placements end with it:\n  {}",
                self.page,
                hits.len(),
                hits.join("\n  ")
            ),
        }
    }

    /// Whether any placement's id ends with `tail`. For asserting a thing
    /// appeared, or did not.
    pub fn has(&self, tail: &str) -> bool {
        self.frame()
            .placements
            .iter()
            .any(|p| p.id.ends_with(tail) || p.id.ends_with(&format!("{tail}/")))
    }

    /// Every placed id, for working out what a page actually built.
    pub fn ids(&self) -> Vec<String> {
        self.frame()
            .placements
            .iter()
            .map(|p| p.id.clone())
            .collect()
    }

    /// Inject one action and run the pass that delivers it, so the next
    /// lookup resolves against the frame the action produced.
    fn act(&mut self, target: Target, action: &Action) -> &mut Self {
        let mut raw = RawInput::default();
        inject_action(&mut self.host, &target, action, &mut raw).unwrap_or_else(|err| {
            panic!("{}: {action:?} refused: {err:?}", self.page);
        });
        let ctx = self.ctx.clone();
        let out = ctx.run_ui(sized(raw), |_| self.host.pass(&ctx));
        self.clipboard
            .extend(out.platform_output.commands.iter().filter_map(|cmd| {
                match cmd {
                    egui::OutputCommand::CopyText(text) => Some(text.clone()),
                    _ => None,
                }
            }));
        out.drop_without_applying_deltas();
        self
    }

    /// Everything the page has put on the clipboard since this camera opened.
    #[must_use]
    pub fn clipboard(&self) -> &[String] {
        &self.clipboard
    }

    /// A primary-button press and release on the node whose id ends `tail`.
    pub fn click(&mut self, tail: &str) -> &mut Self {
        let id = self.id(tail);
        self.act(
            Target::NodeId(id),
            &Action::Click {
                modifiers: Modifiers::default(),
            },
        )
    }

    /// Move the pointer onto the node whose id ends `tail`, pressing nothing.
    /// This is what a `slot@hover` binding and a hover-revealed tooltip need.
    pub fn hover(&mut self, tail: &str) -> &mut Self {
        let id = self.id(tail);
        self.act(Target::NodeId(id), &Action::Hover)
    }

    /// Press the primary button on the node whose id ends `tail`, move the
    /// pointer to `to` (through a midpoint, as `inject::push_drag` does), and
    /// release there — the press-move-release a physical drag produces,
    /// routed through the host's pointer capture exactly as a mouse would be.
    ///
    /// `to` is a window position in logical units. A caller working out
    /// where along a rail to let go reads the rail's rect from
    /// [`Camera::frame`] first.
    pub fn drag(&mut self, tail: &str, to: Point) -> &mut Self {
        let id = self.id(tail);
        self.act(
            Target::NodeId(id),
            &Action::Drag {
                to,
                modifiers: Modifiers::default(),
            },
        )
    }

    /// The placed rect of the node whose id ends `tail`, in logical units.
    ///
    /// # Panics
    /// As [`Camera::id`] does, on a missing or ambiguous tail.
    pub fn rect(&self, tail: &str) -> Rect {
        let id = self.id(tail);
        self.frame()
            .placement(&id)
            .unwrap_or_else(|| panic!("{}: {id} resolved but is not placed", self.page))
            .rect
    }

    /// What the node whose id ends `tail` paints: its token bindings, its
    /// engine caret, its text run. The peer of [`Camera::rect`] for the
    /// questions geometry cannot answer — which fill a bubble binds, and
    /// whether the engine drew a beak back at the anchor.
    ///
    /// # Panics
    /// As [`Camera::id`] does, on a missing or ambiguous tail.
    pub fn paint(&self, tail: &str) -> &PaintContent {
        let id = self.id(tail);
        let frame = self.frame();
        let index = frame
            .placements
            .iter()
            .position(|p| p.id == id)
            .unwrap_or_else(|| panic!("{}: {id} resolved but is not placed", self.page));
        &frame.content[index]
    }

    /// The caret the engine draws for the anchored surface whose id ends
    /// `tail`: `None` for a surface declared `Tip::Flush`, or for one that
    /// is not anchored at all.
    pub fn caret(&self, tail: &str) -> Option<CaretPaint> {
        self.paint(tail).caret
    }

    /// The token the node whose id ends `tail` binds to `slot`, verbatim
    /// from its props — `"background"`, `"background@hover"` — or `None`
    /// when it binds nothing there.
    pub fn token(&self, tail: &str, slot: &str) -> Option<String> {
        self.paint(tail).tokens.get(slot).cloned()
    }

    /// Move the pointer to a raw position — for leaving a node, where there is
    /// no node to name.
    pub fn hover_at(&mut self, x: f32, y: f32) -> &mut Self {
        self.act(Target::Pos(Point::new(x, y)), &Action::Hover)
    }

    /// A primary-button press and release at a raw position — for pressing
    /// *outside* something, which is what dismisses a `DismissOutside`
    /// surface and has no node to name.
    pub fn click_at(&mut self, x: f32, y: f32) -> &mut Self {
        self.act(
            Target::Pos(Point::new(x, y)),
            &Action::Click {
                modifiers: Modifiers::default(),
            },
        )
    }

    /// The open page's body as its module builds it now. For reading back a
    /// fact the frame does not carry (a leaf's text); see
    /// `Catalog::open_page_body`.
    pub fn tree(&self) -> gorgon_petra::tree::ViewNode {
        self.host.app().open_page_body()
    }

    /// Move keyboard focus to the node whose id ends `tail`.
    pub fn focus(&mut self, tail: &str) -> &mut Self {
        let id = self.id(tail);
        self.act(Target::NodeId(id), &Action::Focus)
    }

    /// Focus the node whose id ends `tail`, then commit `text` to it — the
    /// two-step a driver must do because text routes by focus, exactly as a
    /// physical keyboard's does.
    pub fn type_into(&mut self, tail: &str, text: &str) -> &mut Self {
        self.focus(tail);
        let id = self.id(tail);
        self.act(
            Target::NodeId(id),
            &Action::TextEdit {
                text: text.to_owned(),
            },
        )
    }

    /// Commit `text` to whatever holds focus, moving focus nowhere.
    ///
    /// [`Camera::type_into`] focuses first, which is convenient and is also
    /// how a whole round of text-field tests passed against a field no hand
    /// could reach. Use this one after a [`Camera::click`] to prove the path
    /// an operator actually walks.
    ///
    /// # Panics
    /// If the frame holds no placements, which cannot happen after
    /// [`Camera::on`].
    pub fn type_here(&mut self, text: &str) -> &mut Self {
        let id = self
            .frame()
            .placements
            .first()
            .unwrap_or_else(|| panic!("{}: the frame holds no placements", self.page))
            .id
            .clone();
        self.act(
            Target::NodeId(id),
            &Action::TextEdit {
                text: text.to_owned(),
            },
        )
    }

    /// A full keystroke to whatever holds focus.
    ///
    /// Aimed at the focused placement, or at the first one when nothing has
    /// focus. `inject_action` routes a key by focus exactly as a physical
    /// keyboard does and checks the target only for staleness, so the target
    /// is a liveness token rather than an address. It used to be the literal
    /// `"/root"`, which is not this catalog's root — its tree is rooted at
    /// `/page` — so every call panicked before delivering anything.
    ///
    /// # Panics
    /// If the frame holds no placements at all, which cannot happen after
    /// [`Camera::on`].
    pub fn key(&mut self, key: KeyCode) -> &mut Self {
        let frame = self.frame();
        let id = frame
            .placements
            .iter()
            .find(|p| p.semantics.focused)
            .or_else(|| frame.placements.first())
            .unwrap_or_else(|| panic!("{}: the frame holds no placements", self.page))
            .id
            .clone();
        self.act(
            Target::NodeId(id),
            &Action::Key {
                key,
                modifiers: Modifiers::default(),
            },
        )
    }

    /// A wheel delta on the node whose id ends `tail`. Negative `dy` scrolls
    /// the content up, matching `egui::Event::MouseWheel`.
    pub fn scroll(&mut self, tail: &str, dy: f32) -> &mut Self {
        let id = self.id(tail);
        self.act(
            Target::NodeId(id),
            &Action::Scroll {
                delta: Size::new(0.0, dy),
            },
        )
    }

    /// Which placement the pointer is over, per the host's own derivation.
    pub fn hovered(&self) -> Option<String> {
        self.host.pointer().hovered().map(str::to_owned)
    }

    /// Which node holds keyboard focus, per the host's own focus tree.
    ///
    /// The peer of [`Camera::hovered`]. A test that wants to know whether a
    /// *click* seated focus cannot ask [`Camera::focus`], which moves focus
    /// itself; it has to read the tree back.
    pub fn focused(&self) -> Option<String> {
        self.host.focus().current().map(str::to_owned)
    }

    /// Which placement the frame paints the focus ring on, per its own
    /// `semantics.focused` flag; `None` when no placed node is focused.
    ///
    /// Stricter than [`Camera::focused`], which reads the focus tree's id
    /// and keeps answering it after the node it names has left the frame.
    /// A test that wants to know whether focus *survived* a rebuild — the
    /// trigger of a list box that just opened — has to ask the frame, or
    /// it passes on a stale id while the ring has gone.
    pub fn ring(&self) -> Option<String> {
        self.frame()
            .placements
            .iter()
            .find(|p| p.semantics.focused)
            .map(|p| p.id.clone())
    }

    /// The frame as it now stands.
    ///
    /// # Panics
    /// If no pass has produced one, which cannot happen after [`Camera::on`].
    pub fn frame(&self) -> &PetrifiedFrame {
        self.host
            .frame()
            .unwrap_or_else(|| panic!("{}: no frame", self.page))
    }

    /// Photograph the current frame. Returns the PNG bytes, and writes them to
    /// `$PETRA_SHOT_DIR/<name>.png` when that variable is set.
    ///
    /// The name should carry the inventory row number so a directory of shots
    /// sorts into inventory order: `"11-dropdown-open"`.
    ///
    /// # Panics
    /// If the capture is refused, or if the result is not a decodable PNG.
    /// Both are defects in the snapshotter rather than in the page.
    pub fn shoot(&mut self, name: &str) -> Vec<u8> {
        let ctx = self.ctx.clone();
        let output = ctx.run_ui(sized(RawInput::default()), |_| self.host.pass(&ctx));
        let frame = self
            .host
            .frame()
            .unwrap_or_else(|| panic!("{}: the shooting pass produced no frame", self.page));
        let shot = self
            .shooter
            .capture(&ctx, &output, frame, None)
            .unwrap_or_else(|err| panic!("{}: capture refused: {err:?}", self.page));
        output.drop_without_applying_deltas();
        if let Some(dir) = std::env::var_os("PETRA_SHOT_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).expect("shot dir");
            std::fs::write(dir.join(format!("{name}.png")), &shot.png).expect("write shot");
        }
        shot.png
    }

    /// Mean luminance of the current frame, 0.0 (black) to 1.0 (white).
    ///
    /// The coarsest possible reading of a picture, and the right one for the
    /// question "is this page dark or light": a theme swap moves every pixel
    /// on the page, and no token-level assertion proves the swap reached the
    /// rasterizer.
    ///
    /// # Panics
    /// If the shot is not a decodable PNG.
    pub fn mean_luma(&mut self) -> f32 {
        let png = self.shoot("_scratch");
        let image = image::load_from_memory(&png)
            .unwrap_or_else(|err| panic!("{}: shot is not a PNG: {err}", self.page))
            .to_rgba8();
        let mut total = 0.0_f64;
        for px in image.pixels() {
            let [r, g, b, _] = px.0;
            total += 0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b);
        }
        let count = image.pixels().len() as f64;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a mean of u8 luminances divided by 255 is inside f32"
        )]
        {
            (total / count / 255.0) as f32
        }
    }

    /// How many distinct RGBA values the current frame rasterizes to, capped
    /// at `cap` so a full-page scan stops early.
    ///
    /// One is a blank page. It is the weakest useful assertion about a
    /// picture and it is not a substitute for reading the picture.
    ///
    /// # Panics
    /// If the shot is not a decodable PNG.
    pub fn colours(&mut self, cap: usize) -> usize {
        let png = self.shoot("_scratch");
        let image = image::load_from_memory(&png)
            .unwrap_or_else(|err| panic!("{}: shot is not a PNG: {err}", self.page))
            .to_rgba8();
        let mut seen: std::collections::HashSet<[u8; 4]> = std::collections::HashSet::new();
        for px in image.pixels() {
            seen.insert(px.0);
            if seen.len() >= cap {
                break;
            }
        }
        seen.len()
    }
}

fn sized(mut input: RawInput) -> RawInput {
    input.screen_rect = Some(egui::Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(WINDOW[0], WINDOW[1]),
    ));
    input
}

fn headless() -> Context {
    let ctx = Context::default();
    ctx.run_ui(sized(RawInput::default()), |_| {})
        .drop_without_applying_deltas();
    ctx
}

#[cfg(test)]
mod tests {
    use super::{CAPTURE_SCALE, Camera};
    use crate::catalog::WINDOW;
    use gorgon_petra::geom::{Point, Rect};
    use gorgon_petra::input::KeyCode;
    use gorgon_petra::token::FocusRing;
    use gorgon_petra::tree::Edge;

    // ===== TRIAGE (temporary, 2026-09-04) — delete before merge =====
    //
    // Sweeps every page and prints where the focus bar landed. Asserts
    // nothing: the printout is the evidence. Its sibling, the "drive every
    // dead page" instrument, is gone: every row it drove now has its own
    // asserting test below (Wave 7), and its node tails named trees those
    // pages no longer build.

    /// Is the stuck focus bar a device-pixel-ratio bug?
    ///
    /// At 2x the bar tracks the selected row for pages 1-15 and sticks under
    /// row 1 from page 16 on. If the boundary moves to 31 at 1x, the lookup is
    /// dividing by pixels-per-point once too often. If it stays at 16, it is
    /// not a scale bug.
    #[test]
    fn triage_focus_bar_boundary_by_scale() {
        for scale in [1.0_f32, 2.0] {
            let mut stuck = Vec::new();
            for row in 1..=42u32 {
                let name = crate::inventory::ROWS[(row - 1) as usize].component;
                let mut cam = Camera::at_scale(name, scale);
                let png = cam.shoot("_triage-scale");
                let img = image::load_from_memory(&png).expect("png").to_rgba8();
                let (w, h) = img.dimensions();
                let rail = (240.0 * scale) as u32;
                let mut ys = Vec::new();
                for y in 0..h {
                    for x in 0..rail.min(w) {
                        let p = img.get_pixel(x, y).0;
                        if p[2] > 200 && p[0] < 120 && p[1] < 180 {
                            ys.push(y);
                            break;
                        }
                    }
                }
                let bar_row = ys.first().map(|y| (*y as f32 / (30.0 * scale)) as u32 + 1);
                if bar_row != Some(row) {
                    stuck.push((row, bar_row));
                }
            }
            println!(
                "TRIAGE scale {scale}: {} rows where the bar is not on the open page",
                stuck.len()
            );
            for (row, at) in &stuck {
                println!("TRIAGE   page {row} -> bar on row {at:?}");
            }
        }
    }

    /// The camera's click reaches application state.
    ///
    /// This is the falsification for the whole module. Every later wave proves
    /// "the dropdown opens" by driving it through [`Camera::click`] and reading
    /// the picture, and all of that is worthless if the click is inert. The
    /// Toggle page is the one page the operator accepted, so a failure here is
    /// the driver and never the component.
    ///
    /// Asserted on the rasterized picture rather than on a state field: a
    /// bool flipping in `Catalog` proves the handler ran, not that anything
    /// reached the screen, and reaching the screen is the only claim this
    /// module exists to support.
    #[test]
    fn a_camera_click_changes_what_the_page_rasterizes_to() {
        let mut cam = Camera::on("Toggle");
        let before = cam.shoot("_smoke-toggle-before");
        cam.click("toggle-default-off");
        let after = cam.shoot("_smoke-toggle-after");
        assert_ne!(
            before, after,
            "clicking a toggle through the camera changed no pixel, so the \
             driver is inert and every picture taken after a driving step \
             proves nothing"
        );
    }

    /// A drag reaches application state and moves what the page draws, in
    /// the direction the pointer went.
    ///
    /// Row 30. The slider's handle declared `Interaction::Drag` and the host
    /// granted the capture, and nothing ever moved, because `App::handle`
    /// had no geometry to turn a pointer position into a value. This drives
    /// the whole path — `Action::Drag` through the translator, the capture,
    /// `App::handle` with the frame, `slider_value_at`, a rebuilt tree — and
    /// reads the result two ways: the fill's placed width grew, and the
    /// picture changed. Both, because a placement can move while the
    /// picture does not (the paint pass ignoring it) and a picture can
    /// change for reasons that are not the fill (hover lighting the
    /// handle).
    ///
    /// Asserted on direction, not just difference: a drag to the right that
    /// shrank the fill would also "change the picture".
    #[test]
    fn dragging_the_slider_handle_moves_the_fill_the_way_the_pointer_went() {
        let mut cam = Camera::on("Slider");
        let before = cam.shoot("30-slider-before-drag");
        let fill_before = cam.rect("/rail/fill");
        let rail = cam.rect("/row/rail");
        let handle = cam.rect("/rail/handle");
        assert!(
            (fill_before.w / rail.w - 0.4).abs() < 0.05,
            "the page opens at 40%, fill {} of rail {}",
            fill_before.w,
            rail.w
        );
        let to = Point::new(rail.x + rail.w * 0.8, handle.y + handle.h / 2.0);
        cam.drag("/rail/handle", to);
        let after = cam.shoot("30-slider-after-drag");
        let fill_after = cam.rect("/rail/fill");
        assert!(
            fill_after.w > fill_before.w,
            "the pointer went right, so the fill must be wider: {} -> {}",
            fill_before.w,
            fill_after.w
        );
        assert!(
            (fill_after.w / rail.w - 0.8).abs() < 0.05,
            "released at 80% of the rail, the fill is about 80%: {} of {}",
            fill_after.w,
            rail.w
        );
        let handle_after = cam.rect("/rail/handle");
        assert!(
            (handle_after.x + handle_after.w / 2.0 - to.x).abs() < 1.5,
            "the handle's centre is under where the pointer let go: {} vs {}",
            handle_after.x + handle_after.w / 2.0,
            to.x
        );
        assert_ne!(
            before, after,
            "the fill's placement moved and not one pixel changed: the drag \
             never reached the picture"
        );
    }

    /// The modal covers the window, not the card that declares it.
    ///
    /// Row 20. `modal()` is a viewport-anchored `Layer::Modal` surface, and
    /// the catalog mounts it inside a page card, three containers deep. It
    /// used to land inside that card, a dialog the size of a paragraph. Now
    /// its scrim is the window and its dialog is centred in the window,
    /// over the index pane and the page chrome alike — and a press on the
    /// dimmed index pane reaches nothing.
    #[test]
    fn the_modal_covers_the_window_not_the_card() {
        let mut cam = Camera::on("Modal");
        // The dialog opens from a trigger now, as Carbon's does; the page no
        // longer mounts it at rest.
        cam.click("open-modal");
        cam.shoot("20-modal");
        let window = Rect::new(0.0, 0.0, WINDOW[0], WINDOW[1]);
        assert_eq!(cam.rect("md-col/md"), window, "the scrim is the window");
        let dialog = cam.rect("/seat/dialog");
        assert!(
            (dialog.x - WINDOW[0] * 0.2).abs() < 0.5 && (dialog.w - WINDOW[0] * 0.6).abs() < 0.5,
            "the dialog is the middle 60% of the window, got {dialog:?}"
        );
        assert!(
            (dialog.y + dialog.h / 2.0 - WINDOW[1] / 2.0).abs() < 1.0,
            "and vertically centred in it, got {dialog:?}"
        );
        let page_before = cam.host.app().current().row.number;
        cam.click("/idx-1");
        assert_eq!(
            cam.host.app().current().row.number,
            page_before,
            "a click on the dimmed index pane must not turn the page"
        );
    }

    /// Hover reaches a component and changes what it draws.
    ///
    /// `Interaction::Hover` and a `slot@hover` token binding both exist, but
    /// the operator's list has rows hanging on whether hover actually lights
    /// one up — the Tooltip row says outright that it "should be an on hover
    /// event". Reading the code cannot settle it. Driving it can.
    ///
    /// The **ghost** button, not the primary one. `button::chrome` gives
    /// `Variant::Primary` `hover: None` on purpose: its two state tones step
    /// off the layer ramp, and binding them would turn the page's one accent
    /// control grey under the pointer. So a primary button is the one button
    /// on the page that correctly does not move, and hovering it proves
    /// nothing either way. Ghost binds `LAYER_HOVER`, so it must move.
    #[test]
    fn hovering_a_ghost_button_changes_what_the_page_rasterizes_to() {
        let mut cam = Camera::on("Button");
        let resting = cam.shoot("_smoke-ghost-resting");
        cam.hover("btn-ghost");
        assert_eq!(
            cam.hovered().as_deref(),
            Some("/page/shell/main-scroll/main/variants/buttons/kinds/btn-ghost"),
            "the host did not even register the pointer over the ghost button, \
             so this test cannot say anything about painting"
        );
        let hovered = cam.shoot("_smoke-ghost-hovered");
        assert_ne!(
            resting, hovered,
            "the host reports the ghost button hovered and it binds \
             `background@hover`, yet not one pixel changed: the hover state \
             never reaches the paint pass"
        );
    }

    // ===== Wave 7: catalog state and routing =====
    //
    // Every row below reached the operator as "does not work". A resting
    // photograph cannot tell a working control from a dead one, so each
    // test drives the control and photographs the result, and asserts on
    // the frame that came out (ids, rects) and on the page's own tree where
    // the frame does not carry the fact (a leaf's text). The picture is
    // written to `PETRA_SHOT_DIR` and is the evidence; the assertions are
    // what fails when the arm stops firing.

    /// The text a leaf keyed `key` paints, from the page's own tree.
    fn leaf_text(cam: &Camera, key: &str) -> String {
        crate::page::common::find(&cam.tree(), key)
            .unwrap_or_else(|| panic!("no node keyed {key:?} in the page tree"))
            .props
            .text
            .clone()
            .unwrap_or_else(|| panic!("{key:?} paints no text"))
    }

    /// A press on empty ground at the window's lower-right corner: inside
    /// no surface, on no control. What dismisses a `DismissOutside` surface.
    fn press_empty_ground(cam: &mut Camera) {
        cam.click_at(WINDOW[0] - 8.0, WINDOW[1] - 8.0);
    }

    /// Drive an anchored surface open and check it landed *over the page*:
    /// placed, hanging under its trigger, and as tall as its own content
    /// asks (at least `min_h`), not boxed into the column that declares it.
    /// Returns the surface's rect for a row's own extra checks.
    ///
    /// `trigger` and `surface` are placement-id tails; `shot` names the
    /// open picture, and `<shot>-closed` the resting one.
    fn opens_over_the_page(
        cam: &mut Camera,
        trigger: &str,
        surface: &str,
        min_h: f32,
        shot: &str,
    ) -> Rect {
        let closed = cam.shoot(&format!("{shot}-closed"));
        assert!(
            !cam.has(surface),
            "{surface} is placed before anything was pressed: the page \
             mounts its open form at rest"
        );
        let trigger_rect = cam.rect(trigger);
        cam.click(trigger);
        let open = cam.shoot(shot);
        assert!(
            cam.has(surface),
            "pressing {trigger} placed no {surface}: the handler arm did \
             not fire. Placed:\n  {}",
            cam.ids().join("\n  ")
        );
        let rect = cam.rect(surface);
        assert!(
            rect.w > 0.0 && rect.h >= min_h,
            "{surface} is placed but boxed: {rect:?}, wanted at least {min_h} tall"
        );
        assert!(
            rect.y >= trigger_rect.y + trigger_rect.h - 1.0,
            "{surface} at {rect:?} is not under its trigger at {trigger_rect:?}"
        );
        assert!(
            rect.y + rect.h <= WINDOW[1] + 0.5,
            "{surface} at {rect:?} runs off the window"
        );
        assert_ne!(
            closed, open,
            "{surface} was placed and not one pixel changed: it never \
             reached the picture"
        );
        rect
    }

    /// Row 1. The second section's header was built with a literal
    /// `false` and no handler arm.
    #[test]
    fn the_second_accordion_section_expands_when_its_header_is_pressed() {
        let mut cam = Camera::on("Accordion");
        let closed = cam.shoot("01-accordion-rest");
        assert!(cam.has("acc-1/header"));
        assert!(
            !cam.has("acc-1/body"),
            "the second section starts collapsed"
        );
        cam.click("acc-1/header");
        let open = cam.shoot("01-accordion-second-open");
        assert!(cam.has("acc-1/body"), "the second section did not expand");
        let body = cam.rect("acc-1/body");
        assert!(body.h > 0.0, "the body is placed with no height: {body:?}");
        assert_ne!(closed, open);
        cam.click("acc-1/header");
        assert!(!cam.has("acc-1/body"), "a second press collapses it again");
    }

    /// Row 5. "mixed does not work": the row had no state behind it. Three
    /// presses walk mixed, checked, unchecked, mixed — each a different
    /// mark inside the box, which the frame carries as a different child.
    #[test]
    fn the_mixed_checkbox_cycles_through_its_three_states_when_pressed() {
        let mut cam = Camera::on("Checkbox");
        let mixed = cam.shoot("05-checkbox-mixed");
        assert!(cam.has("check-mixed/box/dash"), "starts mixed");
        cam.click("check-mixed");
        let checked = cam.shoot("05-checkbox-mixed-then-checked");
        assert!(cam.has("check-mixed/box/tick"), "mixed then checked");
        assert!(!cam.has("check-mixed/box/dash"));
        assert_ne!(mixed, checked);
        cam.click("check-mixed");
        cam.shoot("05-checkbox-mixed-then-unchecked");
        assert!(!cam.has("check-mixed/box/tick"), "checked then unchecked");
        assert!(!cam.has("check-mixed/box/dash"));
        cam.click("check-mixed");
        assert!(cam.has("check-mixed/box/dash"), "and back to mixed");
    }

    /// Row 6. The page never called `code_snippet_multi`; now the
    /// multi-line well is on the page at its Carbon minimum height.
    #[test]
    fn the_multi_line_snippet_is_a_tall_well_on_the_page() {
        let mut cam = Camera::on("Code snippet");
        cam.shoot("06-code-snippet");
        let well = cam.rect("code/snip-multi");
        assert!(
            well.h >= 288.0,
            "the multi-line well is {}px tall, under Carbon's 288 minimum",
            well.h
        );
        assert!(
            cam.has("snip-multi/copy-row/copy"),
            "the well carries its Copy button, in the trailing-aligned row \
             that pins it to the right the way Carbon does"
        );
    }

    /// Row 7. The rows are `list_row`s with a selection behind them, not
    /// en-dashed `list_item`s.
    #[test]
    fn a_contained_list_row_selects_when_pressed() {
        let mut cam = Camera::on("Contained list");
        let resting = cam.shoot("07-contained-list");
        assert!(
            !cam.has("cl-0/marker"),
            "a contained-list row carries no list marker"
        );
        let unselected = crate::page::common::find(&cam.tree(), "cl-0")
            .unwrap()
            .semantics
            .selected;
        assert!(!unselected);
        cam.click("cl-0");
        let selected_shot = cam.shoot("07-contained-list-trace-selected");
        let selected = crate::page::common::find(&cam.tree(), "cl-0")
            .unwrap()
            .semantics
            .selected;
        assert!(selected, "pressing the row did not select it");
        assert_ne!(resting, selected_shot);
    }

    /// Whether the node keyed `key` in the open page's tree is selected.
    fn selected(cam: &Camera, key: &str) -> bool {
        crate::page::common::find(&cam.tree(), key)
            .unwrap_or_else(|| panic!("no node keyed {key:?} in the page tree"))
            .semantics
            .selected
    }

    /// Row 9, round 2 ("not in carbon style"). Carbon's data table is a
    /// selection column of checkboxes, an accent header in compact heading
    /// type, and one rule under each row (`09-data-table.png`). Driven the
    /// way a hand drives it: a press on a row selects it and its box takes
    /// the tick; a press on the header's select-all selects every row, and
    /// a second press clears them. Read back from the tree and from the
    /// pixels, because a `selected` flag that flips while the picture does
    /// not is the round-1 failure this module exists to catch.
    #[test]
    fn a_data_table_row_selects_when_pressed_and_select_all_selects_every_row() {
        let mut cam = Camera::on("Data table");
        let resting = cam.shoot("09-data-table");
        for key in ["dt-0", "dt-1", "dt-2", "dt-3"] {
            assert!(
                cam.has(&format!("{key}/select/box")),
                "{key} has no checkbox in its selection column"
            );
            assert!(
                !cam.has(&format!("{key}/select/box/tick")) || selected(&cam, key),
                "{key}: a tick on an unselected row"
            );
        }
        assert!(
            cam.has("header/select/select-all"),
            "no select-all in the header"
        );
        assert!(!selected(&cam, "dt-0"));
        assert!(
            selected(&cam, "dt-1"),
            "the page opens with its second row selected"
        );

        cam.click("dt-0/c0");
        let one_more = cam.shoot("09-data-table-row-one-selected");
        assert!(selected(&cam, "dt-0"), "pressing the row did not select it");
        assert!(
            cam.has("dt-0/select/box/tick"),
            "the selected row's checkbox did not take the tick"
        );
        assert_ne!(resting, one_more, "the selection never reached the picture");

        cam.click("select-all");
        let every = cam.shoot("09-data-table-all-selected");
        for key in ["dt-0", "dt-1", "dt-2", "dt-3"] {
            assert!(selected(&cam, key), "select-all left {key} unselected");
            assert!(
                cam.has(&format!("{key}/select/box/tick")),
                "{key} has no tick"
            );
        }
        assert!(
            selected(&cam, "select-all"),
            "with every row selected the header box is checked"
        );
        assert_ne!(one_more, every);

        cam.click("select-all");
        let none = cam.shoot("09-data-table-none-selected");
        for key in ["dt-0", "dt-1", "dt-2", "dt-3"] {
            assert!(
                !selected(&cam, key),
                "a second select-all press left {key} selected"
            );
            assert!(
                !cam.has(&format!("{key}/select/box/tick")),
                "{key} kept its tick"
            );
        }
        assert_ne!(every, none);
    }

    /// Row 31, round 2. A structured list's selectable rows are
    /// single-select: a press on the first row moves the selection and its
    /// check mark off the second row and onto the first, and the picture
    /// changes with it.
    #[test]
    fn a_structured_list_row_takes_the_selection_when_pressed() {
        let mut cam = Camera::on("Structured list");
        let resting = cam.shoot("31-structured-list");
        assert!(!selected(&cam, "sl-0"));
        assert!(
            selected(&cam, "sl-1"),
            "the page opens with its second row selected"
        );
        // Every row reserves the mark's footprint (a same-size spacer keyed
        // `mark`), so "has a mark" is "the node keyed `mark` is a glyph".
        let mark_is_glyph = |cam: &Camera, row: &str| {
            crate::page::common::find(&cam.tree(), row)
                .and_then(|r| crate::page::common::find(r, "mark"))
                .is_some_and(|m| m.kind == gorgon_petra::tree::NodeKind::Canvas)
        };
        assert!(
            mark_is_glyph(&cam, "sl-1") && !mark_is_glyph(&cam, "sl-0"),
            "the mark is on the selected row alone"
        );
        cam.click("sl-0/c0/lead/c0");
        let moved = cam.shoot("31-structured-list-first-selected");
        assert!(
            selected(&cam, "sl-0"),
            "pressing the first row did not select it"
        );
        assert!(
            !selected(&cam, "sl-1"),
            "single-select: the second row must let go"
        );
        assert!(
            mark_is_glyph(&cam, "sl-0"),
            "the mark did not move onto the first row"
        );
        assert!(
            !mark_is_glyph(&cam, "sl-1"),
            "the mark stayed on the second row"
        );
        assert_ne!(resting, moved, "the selection never reached the picture");
    }

    /// Row 23, round 2 ("I dont think is ibm style"). Carbon's bar is two
    /// pickers with chevrons and two caret icon buttons
    /// (`23-pagination.png`). Driven: Next is a caret button and a press on
    /// it moves the page and the range; the page-size picker opens a list
    /// under itself and a press on `20` reflows the whole bar to 3 pages;
    /// the page picker opens too and a press on a page number jumps to it.
    #[test]
    fn the_pagination_bar_pages_forward_and_its_pickers_pick() {
        let mut cam = Camera::on("Pagination");
        let first = cam.shoot("23-pagination");
        assert!(
            cam.has("pager/bar/items-per-page"),
            "no items-per-page group"
        );
        assert!(cam.has("range/range-text"), "no range text");
        assert!(
            cam.has("previous/caret") && cam.has("next/caret"),
            "the nav buttons are carets"
        );
        assert!(
            cam.has("page-size-picker/chevron"),
            "the page-size picker has no chevron"
        );
        assert_eq!(leaf_text(&cam, "page-size"), "10");
        assert_eq!(leaf_text(&cam, "range-text"), "1\u{2013}10 of 50 items");
        assert_eq!(leaf_text(&cam, "page"), "1");
        assert!(!cam.has("menu"), "no picker is open at rest");

        cam.click("controls/next");
        let second = cam.shoot("23-pagination-page-two");
        assert_eq!(leaf_text(&cam, "page"), "2");
        assert_eq!(leaf_text(&cam, "range-text"), "11\u{2013}20 of 50 items");
        assert_ne!(first, second);

        cam.click("page-size-picker");
        let sizes_open = cam.shoot("23-pagination-sizes-open");
        assert!(
            cam.has("page-size-cell/menu"),
            "the page-size picker did not open its list"
        );
        let picker = cam.rect("page-size-picker");
        let menu = cam.rect("page-size-cell/menu");
        assert!(
            menu.y >= picker.y + picker.h - 1.0 && (menu.x - picker.x).abs() < 1.0,
            "the list at {menu:?} does not hang under the picker at {picker:?}"
        );
        assert_ne!(second, sizes_open);

        cam.click("size-20");
        let twenty = cam.shoot("23-pagination-twenty-per-page");
        assert!(!cam.has("menu"), "choosing a size did not close the list");
        assert_eq!(leaf_text(&cam, "page-size"), "20");
        assert_eq!(leaf_text(&cam, "page-count"), "of 3 pages");
        assert_eq!(leaf_text(&cam, "range-text"), "21\u{2013}40 of 50 items");
        assert_ne!(sizes_open, twenty);

        cam.click("page-picker");
        assert!(
            cam.has("page-cell/menu"),
            "the page picker did not open its list"
        );
        cam.click("page-3");
        let third = cam.shoot("23-pagination-page-three");
        assert!(!cam.has("menu"));
        assert_eq!(leaf_text(&cam, "page"), "3");
        assert_eq!(leaf_text(&cam, "range-text"), "41\u{2013}50 of 50 items");
        assert_ne!(twenty, third);
    }

    /// Row 32. The vertical strip was hardcoded and had no panel, so a
    /// press changed nothing. Now the panel beside it reads the selected
    /// tab's content.
    #[test]
    fn the_vertical_tabs_switch_the_panel_beside_them() {
        let mut cam = Camera::on("Tabs");
        let north = cam.shoot("32-tabs-north");
        assert_eq!(leaf_text(&cam, "panel-text"), "North panel");
        let strip = cam.rect("vert/vert-strip");
        let panel = cam.rect("vert/panel");
        assert!(
            panel.x >= strip.x + strip.w - 0.5,
            "the panel sits beside the strip: strip {strip:?}, panel {panel:?}"
        );
        assert!(
            (panel.h - strip.h).abs() < 0.5,
            "the panel is as tall as the strip: strip {strip:?}, panel {panel:?}"
        );
        cam.click("tab-vert-1");
        let south = cam.shoot("32-tabs-south");
        assert_eq!(leaf_text(&cam, "panel-text"), "South panel");
        assert_ne!(north, south);
    }

    /// Row 10. The calendar opens under the field over the page, a day
    /// becomes the value and closes it, and a press on empty ground closes
    /// it too.
    #[test]
    fn the_date_picker_opens_its_calendar_over_the_page() {
        let mut cam = Camera::on("Date picker");
        let calendar = opens_over_the_page(
            &mut cam,
            "dp/when",
            "when/calendar",
            336.0,
            "10-date-picker-open",
        );
        assert!(
            calendar.w >= 288.0,
            "the calendar is Carbon's 288 wide, got {calendar:?}"
        );
        cam.click("days/day-12");
        cam.shoot("10-date-picker-picked");
        assert!(!cam.has("when/calendar"), "picking a day did not close it");
        assert_eq!(leaf_text(&cam, "value"), "2026-08-12");
        cam.click("dp/when");
        assert!(cam.has("when/calendar"));
        press_empty_ground(&mut cam);
        assert!(
            !cam.has("when/calendar"),
            "a press outside did not dismiss it"
        );
    }

    /// Row 11. The list opens under the field over the page, an option
    /// becomes the value, and a press on the open field closes it — the
    /// press that both dismisses and toggles, in that order's inverse.
    #[test]
    fn the_dropdown_opens_its_list_over_the_page_and_an_option_selects() {
        let mut cam = Camera::on("Dropdown");
        let menu =
            opens_over_the_page(&mut cam, "dd-body/dd", "dd/menu", 100.0, "11-dropdown-open");
        let field = cam.rect("dd/field");
        assert!(
            menu.w >= field.w * 0.5,
            "the list is a real list, not a sliver: {menu:?} under {field:?}"
        );
        cam.click("opt-light");
        cam.shoot("11-dropdown-light");
        assert!(
            !cam.has("dd/menu"),
            "choosing an option did not close the list"
        );
        assert_eq!(leaf_text(&cam, "value"), "Light");
        cam.click("dd-body/dd");
        assert!(cam.has("dd/menu"));
        cam.click("dd/field");
        assert!(
            !cam.has("dd/menu"),
            "a press on the open field must close the list, not dismiss it \
             and toggle it straight back open"
        );
    }

    /// Row 18. A menu with something to open it, floating over the page.
    ///
    /// A Carbon menu is a list box (slice-b §18): a `$layer` panel flush
    /// under its trigger with no caret, at least 160 wide, whose items are
    /// full-width 40-tall rows with the label 16 in. The wave-7 menu was a
    /// `popover_with` — a beaked, content-hugging card — so every item
    /// hugged its own text and a caret pointed at the trigger.
    #[test]
    fn the_menu_opens_over_the_page_and_an_item_closes_it() {
        let mut cam = Camera::on("Menu");
        let menu = opens_over_the_page(
            &mut cam,
            "mn-pair/trigger",
            "mn-pair/menu",
            80.0,
            "18-menu-open",
        );
        assert!(
            menu.w >= 160.0,
            "Carbon's menu is at least 160 wide, got {menu:?}"
        );
        assert!(cam.has("menu/content/mn-0") && cam.has("menu/content/mn-1"));
        assert!(
            cam.caret("mn-pair/menu").is_none(),
            "a Carbon menu is a flush list box: the engine must draw no beak \
             back at the trigger"
        );
        let trigger = cam.rect("mn-pair/trigger");
        assert!(
            (menu.y - (trigger.y + trigger.h)).abs() < 0.5,
            "the list box sits flush under its trigger, no air gap: {menu:?} \
             under {trigger:?}"
        );
        assert!(
            (menu.x - trigger.x).abs() < 0.5,
            "the list box's leading edge is the trigger's: {menu:?} under {trigger:?}"
        );
        let item = cam.rect("mn-0");
        assert!(
            (item.w - menu.w).abs() < 0.5 && (item.h - 40.0).abs() < 0.5,
            "an item spans the list box at Carbon's 40: {item:?} in {menu:?}"
        );
        let label = cam.rect("mn-0/label");
        assert!(
            (label.x - item.x - 16.0).abs() < 0.5,
            "the item label sits 16 in from the row's edge: {label:?} in {item:?}"
        );
        cam.click("mn-1");
        cam.shoot("18-menu-after-delete");
        assert!(
            !cam.has("mn-pair/menu"),
            "choosing an item did not close the menu"
        );
    }

    /// Row 19. The trigger opens its menu over the page.
    ///
    /// A Carbon menu button is a primary button at least 160 wide with its
    /// chevron on the trailing edge, and its menu is a list box flush under
    /// it with no caret, leading edges aligned (slice-b §19). Opening the
    /// menu leaves keyboard focus on the trigger — the recorded defect was
    /// focus falling off it, because the open form was a different node.
    #[test]
    fn the_menu_button_opens_its_menu_over_the_page() {
        let mut cam = Camera::on("Menu buttons");
        let trigger = cam.rect("mb/trigger");
        assert!(
            trigger.w >= 160.0 && (trigger.h - 40.0).abs() < 0.5,
            "the trigger is a primary button at Carbon's 160 by 40 minimum: {trigger:?}"
        );
        let chevron = cam.rect("mb/trigger/caret");
        assert!(
            (trigger.x + trigger.w - (chevron.x + chevron.w) - 16.0).abs() < 0.5,
            "the chevron ends 16 in from the trigger's trailing edge: {chevron:?} in {trigger:?}"
        );
        let menu = opens_over_the_page(
            &mut cam,
            "mb/trigger",
            "mb/menu",
            40.0,
            "19-menu-buttons-open",
        );
        assert!(cam.has("menu/content/mb-0"));
        assert!(
            cam.caret("mb/menu").is_none(),
            "a menu button's menu is a flush list box: no beak"
        );
        assert!(
            (menu.x - trigger.x).abs() < 0.5 && (menu.y - (trigger.y + trigger.h)).abs() < 0.5,
            "the menu hangs flush under the trigger, leading edges aligned: \
             {menu:?} under {trigger:?}"
        );
        assert!(
            menu.w >= trigger.w - 0.5,
            "the menu is at least as wide as its trigger: {menu:?} under {trigger:?}"
        );
        assert!(
            cam.ring().is_some_and(|id| id.ends_with("mb/trigger")),
            "opening the menu moved keyboard focus off the trigger: the ring \
             is on {:?}",
            cam.ring()
        );
        cam.click("mb-0");
        assert!(
            !cam.has("mb/menu"),
            "choosing the item did not close the menu"
        );
        cam.click("mb/trigger");
        assert!(cam.has("mb/menu"));
        press_empty_ground(&mut cam);
        cam.shoot("19-menu-buttons-dismissed");
        assert!(!cam.has("mb/menu"), "a press outside did not dismiss it");
    }

    /// Row 24. The note is a floating surface anchored to the button, not a
    /// paragraph printed under it.
    ///
    /// Carbon's popover is a `$layer` panel with a drawn 12-wide caret on
    /// the edge that faces the anchor, its body left-aligned (slice-b §24).
    /// The wave-7 popover spelled its caret as the word `^` in a text node
    /// and centred everything, so the picture carried two beaks — the
    /// engine's and the word — and a centred paragraph.
    #[test]
    fn the_popover_opens_over_the_page_and_dismisses_outside() {
        let mut cam = Camera::on("Popover");
        let note = opens_over_the_page(
            &mut cam,
            "po-pair/pop-anchor",
            "po-pair/pop-note",
            30.0,
            "24-popover-open",
        );
        assert!(
            !cam.has("pop-note/content/caret"),
            "the caret is drawn by the engine, not spelled as a text node"
        );
        let caret = cam
            .caret("po-pair/pop-note")
            .expect("the engine draws the popover's caret back at its anchor");
        assert_eq!(
            caret.side,
            Edge::Bottom,
            "the note hangs under the anchor, so the caret is on its top edge"
        );
        assert!(
            (caret.w - 12.0).abs() < 0.5,
            "Carbon's caret is 12 wide at its base: {caret:?}"
        );
        let anchor = cam.rect("po-pair/pop-anchor");
        assert!(
            (note.y - (anchor.y + anchor.h) - 8.0).abs() < 0.5,
            "the note stands off its anchor by the caret's depth, 8: \
             {note:?} under {anchor:?}"
        );
        let body = cam.rect("pop-note/content/body");
        let content = cam.rect("pop-note/content");
        assert!(
            (body.x - content.x).abs() < 0.5,
            "the body is left-aligned in the note, not centred: {body:?} in {content:?}"
        );
        press_empty_ground(&mut cam);
        cam.shoot("24-popover-dismissed");
        assert!(
            !cam.has("po-pair/pop-note"),
            "a press outside did not dismiss it"
        );
    }

    /// Row 29. The field opens a list over the page and an option becomes
    /// the value.
    ///
    /// A Carbon select is a label over a `$field` box the full width of its
    /// container, 40 tall, closed by a 1-unit `$border-strong` rule along
    /// its bottom edge only, with the chevron 16 in from the trailing edge
    /// (slice-b §29). Its list is a list box: flush under the field, the
    /// field's width, no caret, the current option marked with a check
    /// glyph. Pressing the field keeps keyboard focus on it — the recorded
    /// defect was focus falling off, because the open form replaced the
    /// button with a column and the focused id vanished.
    #[test]
    fn the_select_opens_its_list_over_the_page_and_an_option_selects() {
        let mut cam = Camera::on("Select");
        let column = cam.rect("select/sel");
        let label = cam.rect("theme/label");
        let field = cam.rect("theme/field");
        assert!(
            label.y + label.h <= field.y + 0.5,
            "the label sits above the field: {label:?} over {field:?}"
        );
        assert!(
            (field.w - column.w).abs() < 0.5 && (field.h - 40.0).abs() < 0.5,
            "the field fills its column at Carbon's 40: {field:?} in {column:?}"
        );
        let rule = cam.rect("field/rule");
        assert!(
            (rule.h - 1.0).abs() < 0.5
                && (rule.w - field.w).abs() < 0.5
                && (rule.y + rule.h - (field.y + field.h)).abs() < 0.5,
            "the field's only border is a 1-unit rule along its bottom edge: \
             {rule:?} in {field:?}"
        );
        let chevron = cam.rect("field/row/chevron");
        assert!(
            (field.x + field.w - (chevron.x + chevron.w) - 16.0).abs() < 0.5,
            "the chevron ends 16 in from the field's trailing edge: {chevron:?} in {field:?}"
        );
        let menu = opens_over_the_page(
            &mut cam,
            "theme/field",
            "theme/menu",
            100.0,
            "29-select-open",
        );
        assert!(
            cam.ring().is_some_and(|id| id.ends_with("theme/field")),
            "opening the list moved keyboard focus off the field: the ring is \
             on {:?}",
            cam.ring()
        );
        assert!(
            cam.caret("theme/menu").is_none(),
            "a select's list is a flush list box: no beak"
        );
        assert!(
            (menu.x - field.x).abs() < 0.5
                && (menu.w - field.w).abs() < 0.5
                && (menu.y - (field.y + field.h)).abs() < 0.5,
            "the list box is the field's width, flush under it: {menu:?} under {field:?}"
        );
        assert!(
            cam.has("opt-dark/mark") && !cam.has("opt-light/mark"),
            "the current option, and only it, carries the check glyph"
        );
        cam.click("opt-system");
        cam.shoot("29-select-system");
        assert!(
            !cam.has("theme/menu"),
            "choosing an option did not close the list"
        );
        assert_eq!(leaf_text(&cam, "value"), "System");
        cam.click("theme/field");
        assert!(
            cam.has("opt-system/mark") && !cam.has("opt-dark/mark"),
            "the check glyph follows the chosen option"
        );
    }

    /// Row 37. The toggletip opens on press and a second press on its own
    /// trigger closes it, which is the case the dismissal order exists for.
    #[test]
    fn the_toggletip_opens_over_the_page_and_its_trigger_closes_it() {
        let mut cam = Camera::on("Toggletip");
        opens_over_the_page(&mut cam, "tt/trigger", "tt/tip", 30.0, "37-toggletip-open");
        cam.click("tt/trigger");
        cam.shoot("37-toggletip-closed-again");
        assert!(
            !cam.has("tt/tip"),
            "a second press on the trigger must close the tip: the dismissal \
             and the toggle cancelled each other"
        );
    }

    /// Row 38. There was no trigger on the page at all. Now hovering the
    /// button reveals the bubble over the page, and leaving it for empty
    /// ground hides it.
    #[test]
    fn the_tooltip_appears_on_hover_and_leaves_with_the_pointer() {
        let mut cam = Camera::on("Tooltip");
        let resting = cam.shoot("38-tooltip-rest");
        assert!(
            cam.has("tip-pair/trigger"),
            "the page has a trigger to hover"
        );
        assert!(!cam.has("bubble"), "no bubble at rest");
        let trigger = cam.rect("tip-pair/trigger");
        cam.hover("tip-pair/trigger");
        let hovered = cam.shoot("38-tooltip-hovered");
        assert!(cam.has("bubble"), "hovering the trigger placed no bubble");
        let bubble = cam.rect("bubble");
        assert!(
            bubble.y >= trigger.y + trigger.h - 1.0 && bubble.h > 20.0,
            "the bubble hangs under the trigger over the page: {bubble:?} under {trigger:?}"
        );
        assert_ne!(resting, hovered);
        assert_eq!(
            cam.token("bubble", "background").as_deref(),
            Some("surface.layer-three"),
            "the bubble takes the ramp's last rung: a step away from both the \
             page and a card, because it can be dragged over either. Carbon's \
             own answer is the inverse polarity, which the operator asked us \
             to drop on 2026-09-05"
        );
        assert_eq!(
            cam.caret("bubble").map(|c| c.side),
            Some(Edge::Bottom),
            "the bubble points back up at its trigger"
        );
        cam.hover_at(WINDOW[0] - 8.0, WINDOW[1] - 8.0);
        cam.shoot("38-tooltip-left");
        assert!(
            !cam.has("bubble"),
            "the pointer left the trigger for empty ground and the bubble stayed"
        );
    }

    /// Row 38. Carbon reveals a tooltip on keyboard focus as well as on
    /// hover (slice-c §38), and hides it when focus moves on. Focus is
    /// driven from the keyboard: Tab, from wherever the chrome seats it,
    /// until the trigger has it; never [`Camera::focus`], which would prove
    /// only that a driver can seat focus, not that a keyboard can.
    #[test]
    fn the_tooltip_appears_on_keyboard_focus_and_leaves_with_it() {
        let mut cam = Camera::on("Tooltip");
        assert!(!cam.has("bubble"), "no bubble at rest");
        // Tab visits every focusable node once before it wraps, and the
        // chrome seats it in the index, forty-two rows ahead of the page,
        // so the bound is the placement count, not a guess.
        let bound = cam.ids().len();
        let mut tabs = 0;
        while !cam
            .focused()
            .is_some_and(|id| id.ends_with("tip-pair/trigger"))
        {
            assert!(
                tabs < bound,
                "Tab never reached the trigger; focus rests on {:?}",
                cam.focused()
            );
            cam.key(KeyCode::Tab);
            tabs += 1;
        }
        cam.shoot("38-tooltip-focused");
        assert!(
            cam.has("bubble"),
            "focus landed on the trigger and no bubble was placed"
        );
        let trigger = cam.rect("tip-pair/trigger");
        let bubble = cam.rect("bubble");
        assert!(
            bubble.y >= trigger.y + trigger.h - 1.0,
            "the bubble hangs under the trigger: {bubble:?} under {trigger:?}"
        );
        cam.key(KeyCode::Tab);
        cam.shoot("38-tooltip-focus-left");
        assert!(
            !cam.has("bubble"),
            "focus left the trigger and the bubble stayed"
        );
    }

    /// Row 42, replacing `the_right_panel_opens_under_the_header_over_the_page`.
    ///
    /// That test asserted the panel hung *over* the page under its trigger
    /// and was as tall as its own content — which is a popover, and the one
    /// shape `_header-panel.scss` is not. Every assertion in it passed on
    /// the anatomy the operator called conceptually wrong. What Carbon
    /// states is a region docked to the trailing edge, running the header's
    /// bottom rule to the bottom of the viewport, 256 wide.
    #[test]
    fn the_right_panel_docks_from_the_header_to_the_foot_of_the_frame() {
        let cam = Camera::on("UI shell right panel");
        let frame = cam.rect("shell-frame");
        let header = cam.rect("shell-frame/shell-header");
        let panel = cam.rect("shell-content/shell-switcher");
        assert!(
            (panel.w - 256.0).abs() < 0.5,
            "the panel is Carbon's mini-units(32) = 256 wide, got {panel:?}"
        );
        assert!(
            (panel.y - (header.y + header.h)).abs() < 1.0,
            "`inset-block-start: mini-units(6)`: the panel starts at the \
             header's bottom edge. panel {panel:?}, header {header:?}"
        );
        assert!(
            (panel.y + panel.h - (frame.y + frame.h)).abs() < 1.0,
            "`inset-block-end: 0`: and runs to the foot of the viewport. \
             panel {panel:?}, frame {frame:?}"
        );
        assert!(
            (panel.x + panel.w - (frame.x + frame.w)).abs() < 1.0,
            "`inset-inline-end: 0`: on the trailing edge. panel {panel:?}, \
             frame {frame:?}"
        );
        assert!(
            panel.h > header.h * 2.0,
            "a panel as tall as its own two items is the popover this row \
             used to be: {panel:?}"
        );
    }

    /// Typing into a text field puts the characters on screen.
    ///
    /// Row 34, and the reason the operator called four separate pages broken.
    /// Every field constructor writes only `props.placeholder`; nothing ever
    /// wrote `props.text`, so no field in this library could contain anything.
    /// `layout::paint_content_of` had always been ready to paint a value.
    ///
    /// Driven, not inspected. A state field holding "hello" proves the
    /// handler ran; it does not prove a single pixel changed, and the whole
    /// reason this catalog reached the operator broken under a green suite is
    /// that nobody was looking at pixels.
    #[test]
    fn typing_into_a_text_field_changes_what_the_page_rasterizes() {
        let mut cam = Camera::on("Text input");
        let before = cam.shoot("34-text-input-empty");
        cam.type_into("field-sm", "gorgon");
        let after = cam.shoot("34-text-input-typed");
        assert_ne!(
            before, after,
            "six characters were typed into a focused field and not one pixel \
             changed: either the text never reached the page or the value \
             never reached the paint pass"
        );
    }

    /// Backspace takes a character back off.
    ///
    /// The other half of the round trip. A field that only grows is not an
    /// editable field, and a page that appends without ever removing would
    /// pass the test above forever.
    #[test]
    fn backspace_shortens_a_field_and_the_page_shows_it() {
        let mut cam = Camera::on("Search");
        cam.type_into("query/input", "fiber");
        let full = cam.shoot("28-search-typed");
        cam.key(gorgon_petra::input::KeyCode::Backspace);
        let shorter = cam.shoot("28-search-backspaced");
        assert_ne!(
            full, shorter,
            "backspace on a focused field changed nothing on screen"
        );
    }

    /// A bracket typed into a field stays in the field.
    ///
    /// The catalog pages on `[` and `]`, so before this a keystroke meant for
    /// a text field navigated the whole application away from the page the
    /// operator was typing on. A shortcut that eats what is being typed is a
    /// worse defect than no shortcut, so the field wins whenever the route
    /// names a `Role::TextInput`.
    #[test]
    fn a_bracket_typed_into_a_field_does_not_page_the_catalog() {
        let mut cam = Camera::on("Search");
        cam.type_into("query/input", "[");
        assert!(
            cam.has("/query"),
            "the catalog navigated away from the Search page while a bracket \
             was being typed into its field"
        );
    }

    // ===== Wave R6: loading, progress and notification =====

    /// The pixels inside `rect` (logical units) of a captured page, at the
    /// camera's device scale, so two frames can be compared on one control
    /// and not on the whole window.
    fn pixels_in(png: &[u8], rect: Rect) -> Vec<u8> {
        let image = image::load_from_memory(png)
            .expect("a shot is a PNG")
            .to_rgba8();
        let scale = super::CAPTURE_SCALE;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x, y, w, h) = (
            (rect.x * scale).floor() as u32,
            (rect.y * scale).floor() as u32,
            (rect.w * scale).ceil() as u32,
            (rect.h * scale).ceil() as u32,
        );
        image::imageops::crop_imm(&image, x, y, w, h)
            .to_image()
            .into_raw()
    }

    /// Rows 17 and 14. A spinner is motion, and a resting photograph of one
    /// proves nothing — the round-1 page was two still rings under a green
    /// suite. So this photographs two frames of each spinner and asserts
    /// the spinner's own pixels changed between them, and photographs two
    /// frames of a still control and asserts its pixels did not. Every
    /// camera pass advances egui's clock by one sixtieth of a second, so
    /// the two frames are a known fraction of the 690 ms turn apart.
    ///
    /// The frame's own motion count is asserted too: the host keeps asking
    /// for frames only while `transitions.ambient` is non-zero, so a
    /// spinner that turned under the camera but declared nothing would
    /// freeze on an idle window.
    #[test]
    fn the_loading_spinners_turn_between_frames_and_a_still_control_does_not() {
        let mut cam = Camera::on("Loading");
        let large = cam.rect("sizes/load-lg");
        let small = cam.rect("sizes/load-sm");
        let first = cam.shoot("17-loading");
        for _ in 0..4 {
            cam.shoot("_scratch");
        }
        let later = cam.shoot("17-loading-later");
        assert_ne!(
            pixels_in(&first, large),
            pixels_in(&later, large),
            "five sixtieths of a second later the large spinner has not moved a pixel"
        );
        assert_ne!(
            pixels_in(&first, small),
            pixels_in(&later, small),
            "five sixtieths of a second later the small spinner has not moved a pixel"
        );
        assert_eq!(
            cam.frame().transitions.ambient,
            2,
            "both spinners must declare ambient, or the host stops scheduling frames"
        );

        let mut cam = Camera::on("Inline loading");
        let mark = cam.rect("il-on/mark");
        let first = cam.shoot("14-inline-loading");
        for _ in 0..4 {
            cam.shoot("_scratch");
        }
        let later = cam.shoot("14-inline-loading-later");
        assert_ne!(
            pixels_in(&first, mark),
            pixels_in(&later, mark),
            "the inline spinner has not moved a pixel"
        );
        assert_eq!(cam.frame().transitions.ambient, 1);

        // The control: a still page's control is byte-identical across the
        // same interval, so the difference above is the spinner and not
        // the camera.
        let mut cam = Camera::on("Progress indicator");
        let steps = cam.rect("pi/pi");
        let first = cam.shoot("26-progress-indicator");
        for _ in 0..4 {
            cam.shoot("_scratch");
        }
        let later = cam.shoot("_scratch");
        assert_eq!(
            pixels_in(&first, steps),
            pixels_in(&later, steps),
            "a still progress indicator changed between two frames"
        );
        assert_eq!(cam.frame().transitions.ambient, 0);
    }

    /// Row 21, the operator's decision made twice: no rail, text centred.
    /// Asserted on the placed frame — the card's leftmost child is the
    /// title's column and not a 3-unit bar, and the title's centre is the
    /// card's centre — and photographed.
    #[test]
    fn the_notification_card_has_no_rail_and_its_text_is_centred() {
        let mut cam = Camera::on("Notification");
        cam.shoot("21-notification");
        assert!(!cam.has("nt/panel/rail"), "the accent rail is still placed");
        let panel = cam.rect("nt/panel");
        let title = cam.rect("panel/title");
        let body = cam.rect("panel/body");
        for (name, line) in [("title", title), ("body", body)] {
            let off = (line.x + line.w / 2.0) - (panel.x + panel.w / 2.0);
            assert!(
                off.abs() <= 0.5,
                "the {name} is {off} off the card's centre: {line:?} in {panel:?}"
            );
        }
        assert!(
            (panel.w - 288.0).abs() < 0.5,
            "Carbon's toast is 288 wide, got {panel:?}"
        );
    }

    /// Clicking a text field puts the caret in it.
    ///
    /// The live path, which `typing_into_a_text_field_changes_what_the_page_
    /// rasterizes` does not exercise: `Camera::type_into` moves focus itself
    /// with `Action::Focus`, so it proves the value plumbing and says nothing
    /// about how a person reaches the field. An operator clicks.
    #[test]
    fn clicking_a_text_field_seats_the_caret_in_it() {
        let mut cam = Camera::on("Text input");
        cam.click("field-sm");
        let focused = cam.focused();
        assert!(
            focused.as_deref().is_some_and(|id| id.contains("field-sm")),
            "a click on a text field left focus at {focused:?}: nothing a \
             person types can reach the field"
        );
        let before = cam.shoot("34-text-input-clicked");
        cam.type_here("gorgon");
        let after = cam.shoot("34-text-input-clicked-typed");
        assert_ne!(
            before, after,
            "the field took focus from the click and then six typed \
             characters changed nothing on screen"
        );
    }

    /// Every icon button in the shell header centres its glyph.
    ///
    /// The operator's words were "these buttons on the shell (notably the
    /// notification one) are not probably centered in their button". They
    /// were not: `header_action` set `align`, which is the *cross* axis, and
    /// left `justify` unset, so each 20px glyph sat flush against the
    /// inline-start edge of its own 48px button — 14px left of centre,
    /// measured. The trigger was a copy of the same code and had the same
    /// defect.
    #[test]
    fn every_header_icon_button_centres_its_glyph() {
        let cam = Camera::on("UI shell header");
        for button in ["shell-menu", "shell-action-notify", "shell-action-switcher"] {
            let outer = cam.rect(button);
            let glyph = cam.rect(&format!("{button}/glyph"));
            let dx = (glyph.x + glyph.w / 2.0) - (outer.x + outer.w / 2.0);
            let dy = (glyph.y + glyph.h / 2.0) - (outer.y + outer.h / 2.0);
            assert!(
                dx.abs() < 0.51 && dy.abs() < 0.51,
                "{button}: the glyph sits {dx:+.1},{dy:+.1} off the centre of \
                 its own button ({outer:?} vs {glyph:?})"
            );
            assert!(
                (outer.w - 48.0).abs() < 0.01,
                "{button}: a header icon button is 48 wide (slice-f.md:150), \
                 this one is {:.1} — a stretched button moves the centred \
                 glyph away from where the picture says to press",
                outer.w
            );
        }
    }

    /// Clicking a header nav item makes it the current page.
    ///
    /// Row 40 held no state at all: every control was built from a literal,
    /// so the page drew one fixed picture and swallowed every press. That is
    /// what "none of the elements actually do anything" meant.
    #[test]
    fn clicking_a_header_nav_item_moves_the_current_page_indicator() {
        let mut cam = Camera::on("UI shell header");
        let before = cam.shoot("40-ui-shell-header");
        cam.click("shell-nav-fibers");
        let after = cam.shoot("40-ui-shell-header-fibers");
        assert_ne!(
            before, after,
            "the second nav item was pressed and the header did not change: \
             the current-page indicator never moved"
        );
    }

    /// Pressing a header utility opens it, and pressing it again closes it.
    ///
    /// Every shot is taken after a press, so the focus ring is in all three
    /// and the comparison is about the toggle alone. Comparing against the
    /// untouched page would fail on the ring, which is a real difference and
    /// not the one under test.
    #[test]
    fn a_header_utility_toggles_on_its_own_press() {
        let mut cam = Camera::on("UI shell header");
        cam.click("shell-action-notify");
        let open = cam.shoot("40-ui-shell-header-notify-active");
        cam.click("shell-action-notify");
        let shut = cam.shoot("40-ui-shell-header-notify-closed");
        cam.click("shell-action-notify");
        let open_again = cam.shoot("40-ui-shell-header-notify-reopened");
        assert!(
            open != shut,
            "a second press on an open utility must close it"
        );
        assert!(
            open == open_again,
            "a third press must reopen it to exactly the state the first \
             press produced"
        );
    }

    /// Collapsing the left panel's sub-menu takes its nested row off screen.
    ///
    /// A stronger assertion than "the pixels changed": the child mounts only
    /// while its parent is expanded (FR-026), so if the press was heard the
    /// row is not in the frame at all.
    #[test]
    fn collapsing_the_left_panel_sub_menu_unmounts_its_nested_row() {
        let mut cam = Camera::on("UI shell left panel");
        assert!(
            cam.has("shell-left-fibers"),
            "the sub-menu starts open, so its nested row starts placed"
        );
        // `shell-left-kernel/row`, not `shell-left-kernel`. The parent item's
        // placement is 64 tall — its own 32px row plus the 32px block its
        // children occupy — so its centre is the first pixel row of "Fibers",
        // and a click aimed at the parent's centre correctly hits the child.
        // A person presses the title row.
        cam.click("shell-left-kernel/row");
        assert!(
            !cam.has("shell-left-fibers"),
            "the sub-menu was pressed and its nested row is still placed: the \
             caret says collapsed and the panel says otherwise"
        );
        cam.click("shell-left-kernel/row");
        assert!(cam.has("shell-left-fibers"), "and it opens again");
    }

    /// The modal opens from a trigger and every one of its controls closes it.
    ///
    /// The page mounted the dialog unconditionally and wired nothing, so a
    /// modal that blocks every press behind it and answers none of its own
    /// made the whole page inert.
    #[test]
    fn the_modal_opens_from_its_trigger_and_each_control_closes_it() {
        for control in ["close", "cancel", "primary"] {
            let mut cam = Camera::on("Modal");
            assert!(
                !cam.has("md/frame/seat/dialog"),
                "the page starts with the dialog shut"
            );
            cam.click("open-modal");
            assert!(
                cam.has("md/frame/seat/dialog"),
                "pressing the trigger must open the dialog"
            );
            cam.click(control);
            assert!(
                !cam.has("md/frame/seat/dialog"),
                "{control} left the dialog open: an operator who opens this \
                 has no way back out of it"
            );
        }
    }

    /// The dialog and its Cancel button are not the same grey.
    ///
    /// They were, byte for byte. `modal.rs` seats Cancel one step above the
    /// dialog with `on_layer`, and the gallery then re-seats the whole page
    /// at depth 1 because every component sits on a card; `on_layer` moved
    /// the dialog and left the already-ordinal Cancel alone, so the two met
    /// on `surface.layer-two`. A footer button with no edge, no fill of its
    /// own and no separation from the dialog is not a button.
    #[test]
    fn the_modal_footer_button_is_a_different_tone_from_the_dialog() {
        let mut cam = Camera::on("Modal");
        cam.click("open-modal");
        let png = cam.shoot("20-modal-open");
        let body = cam.rect("dialog/content");
        let cancel = cam.rect("footer/cancel");
        let image = image_at(&png);
        let a = sample(&image, body.x + body.w / 2.0, body.y + body.h / 2.0);
        let b = sample(&image, cancel.x + 8.0, cancel.y + cancel.h - 8.0);
        let step = i32::from(a).abs_diff(i32::from(b));
        assert!(
            step >= 8,
            "the dialog reads {a} and its Cancel button reads {b}: a step of \
             {step} of 255. Carbon's own pair is 38 and 111"
        );
    }

    /// Read `png` back as an image, for the two tests that measure a tone.
    fn image_at(png: &[u8]) -> image::RgbaImage {
        image::load_from_memory(png)
            .expect("the snapshotter writes valid PNG")
            .to_rgba8()
    }

    /// The grey level at a logical point, at the capture's own scale.
    fn sample(image: &image::RgbaImage, x: f32, y: f32) -> u8 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a logical coordinate inside a placed rect, scaled to the \
                      capture, is a small positive number"
        )]
        let (px, py) = (
            (x * super::CAPTURE_SCALE) as u32,
            (y * super::CAPTURE_SCALE) as u32,
        );
        image.get_pixel(px, py)[0]
    }

    /// The accordion's chevron sits at the trailing edge of a full-width row.
    ///
    /// It followed the title by one 8px gap, in a row that hugged its own
    /// text rather than filling the container. Carbon's
    /// `.cds--accordion__heading` is full width with `justify-content:
    /// space-between`, and `Align` has no such variant, so the free space is
    /// a `Spacer` child.
    #[test]
    fn the_accordion_chevron_sits_at_the_trailing_edge_of_a_full_width_row() {
        let cam = Camera::on("Accordion");
        let list = cam.rect("accordion/acc");
        let header = cam.rect("acc-0/header");
        let chevron = cam.rect("acc-0/header/chevron");
        assert!(
            (header.w - list.w).abs() < 0.5,
            "the item's header is {:.1} wide inside a {:.1} list: an \
             accordion row fills its container",
            header.w,
            list.w
        );
        let gap = (header.x + header.w) - (chevron.x + chevron.w);
        assert!(
            (gap - 16.0).abs() < 1.0,
            "the chevron's trailing edge is {gap:.1} from the row's, and \
             Carbon's inline pad is 16"
        );
    }

    /// A contained list's header and its rows start on the same inset, with
    /// a rule between each pair.
    ///
    /// Row 7 was "this is just markdown, wtf" twice. Round 1 put each entry
    /// on its own line; it still had no rules, the list hugged its own text
    /// instead of filling the card, and the rows sat 8px inside the header's
    /// 16px so nothing lined up with anything.
    #[test]
    fn a_contained_list_lines_its_header_up_with_its_rows_and_rules_between() {
        let cam = Camera::on("Contained list");
        let card = cam.rect("contained");
        let list = cam.rect("contained/cl");
        assert!(
            (list.w - card.w).abs() < 0.5,
            "the list is {:.1} wide in a {:.1} card",
            list.w,
            card.w
        );
        let header = cam.rect("cl/header/title");
        for row in ["cl-0", "cl-1"] {
            let label = cam.rect(&format!("{row}/label"));
            assert!(
                (label.x - header.x).abs() < 0.5,
                "{row}'s label starts at {:.1} and the header's at {:.1}",
                label.x,
                header.x
            );
        }
        // One rule per row: between the header and the first, and between
        // the two rows. None after the last, which would border nothing.
        for i in 0..2 {
            let rule = cam.rect(&format!("cl/rule-{i}"));
            assert!(
                (rule.h - 1.0).abs() < 0.01,
                "a Carbon boundary is 1px, this one is {:.2}",
                rule.h
            );
            assert!((rule.w - list.w).abs() < 0.5, "and spans the list");
        }
        assert!(!cam.has("cl/rule-2"), "no rule after the last row");
    }

    // ===== Wave R4: form controls against Carbon's field anatomy =====
    //
    // Carbon's field is a fill with a single rule along its bottom edge
    // (`_text-input.scss`: `border-block-end: 1px solid $border-strong`,
    // `border-radius: 0`). The rows below read that off the raster, not off
    // the token bindings: a `border-bottom` slot the painter ignored would
    // pass every tree-level test and still draw a box.

    /// The raster of the current frame, for reading pixels back.
    fn raster(cam: &mut Camera, name: &str) -> image::RgbaImage {
        let png = cam.shoot(name);
        image::load_from_memory(&png)
            .unwrap_or_else(|err| panic!("shot {name:?} is not a PNG: {err}"))
            .to_rgba8()
    }

    /// One pixel of `img` at a logical position, at the capture scale.
    fn px(img: &image::RgbaImage, x: f32, y: f32) -> [u8; 4] {
        let (dx, dy) = ((x * CAPTURE_SCALE) as u32, (y * CAPTURE_SCALE) as u32);
        img.get_pixel(dx.min(img.width() - 1), dy.min(img.height() - 1))
            .0
    }

    /// The pixels along one device row across a logical x span.
    fn device_row(img: &image::RgbaImage, x0: f32, x1: f32, dy: u32) -> Vec<[u8; 4]> {
        let (dx0, dx1) = ((x0 * CAPTURE_SCALE) as u32, (x1 * CAPTURE_SCALE) as u32);
        (dx0..dx1.min(img.width()))
            .map(|dx| img.get_pixel(dx, dy.min(img.height() - 1)).0)
            .collect()
    }

    /// The last device row inside a logical rect.
    fn bottom_device_row(rect: Rect) -> u32 {
        ((rect.y + rect.h) * CAPTURE_SCALE).round() as u32 - 1
    }

    /// Assert a field keyed `tail` is a Carbon well: a fill, one rule along
    /// its bottom edge, and nothing drawn on its other three edges.
    ///
    /// Sampled at each edge's midpoint, so a rule and a box are told apart
    /// by the top edge and the two sides reading as fill. `filled` says
    /// whether the fill differs from the ground above the field (a read-only
    /// field is transparent).
    fn assert_carbon_well(cam: &mut Camera, tail: &str, shot: &str, filled: bool) {
        let rect = cam.rect(tail);
        let img = raster(cam, shot);
        let (mid_x, mid_y) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        let inside = px(&img, mid_x, mid_y);
        let top = px(&img, mid_x, rect.y);
        let left = px(&img, rect.x, mid_y);
        let right = px(&img, rect.x + rect.w - 0.5, mid_y);
        let above = px(&img, mid_x, rect.y - 2.0);
        assert_eq!(
            top, inside,
            "{tail}: the top edge is drawn; a Carbon field has no box"
        );
        assert_eq!(
            left, inside,
            "{tail}: the left edge is drawn; a Carbon field has no box"
        );
        assert_eq!(
            right, inside,
            "{tail}: the right edge is drawn; a Carbon field has no box"
        );
        let rule_row = bottom_device_row(rect);
        let rule = device_row(&img, rect.x, rect.x + rect.w, rule_row);
        assert!(
            rule.iter().all(|p| *p == rule[0]),
            "{tail}: the bottom rule is not one colour end to end"
        );
        assert_ne!(
            rule[0], inside,
            "{tail}: there is no rule along the bottom edge"
        );
        let fill_row = device_row(&img, rect.x, rect.x + rect.w, rule_row - 3);
        assert!(
            fill_row.iter().filter(|p| **p == inside).count() * 10 > fill_row.len() * 9,
            "{tail}: the rule is thicker than one snapped pixel"
        );
        if filled {
            assert_ne!(above, inside, "{tail}: the field has no fill of its own");
        } else {
            assert_eq!(above, inside, "{tail}: a read-only field is not filled");
        }
    }

    /// Row 34. Every text input on the page is a Carbon well.
    ///
    /// Before this the fields carried a `border` slot and a `radius`, so the
    /// painter stroked a rounded box around each one: the operator's
    /// "frames are not IBM carbon style". `34-text-input.png` in the
    /// reference set shows a flat fill, square, with one rule under it.
    #[test]
    fn a_text_field_is_a_fill_with_one_rule_under_it_and_no_box() {
        let mut cam = Camera::on("Text input");
        for tail in ["field-md", "field-sm", "field-lg"] {
            assert_carbon_well(&mut cam, tail, "34-text-input-wells", true);
        }
        assert_carbon_well(&mut cam, "field-ro", "34-text-input-wells", false);
    }

    /// Row 28. The search well is the same anatomy, with the glass inset.
    #[test]
    fn the_search_field_is_a_carbon_well_with_the_glass_inside_it() {
        let mut cam = Camera::on("Search");
        assert_carbon_well(&mut cam, "/query", "28-search-well", true);
        let well = cam.rect("/query");
        let glass = cam.rect("query/magnifier");
        assert!(
            glass.x > well.x && glass.x + glass.w < well.x + well.w,
            "the glass sits inside the well: {glass:?} in {well:?}"
        );
        let inset = glass.x - well.x;
        assert!(
            ((glass.y - well.y) - (well.h - glass.h) / 2.0).abs() < 0.5,
            "the glass is centred on the well's height: {glass:?} in {well:?}"
        );
        assert!(
            (inset - (well.h - glass.h) / 2.0).abs() < 0.5,
            "the glass is inset from the left by the same amount it is inset \
             from the top ({inset}), Carbon's square icon cell"
        );
    }

    /// Row 22. The number input is one well: the value cell takes the row,
    /// the two steppers are unfilled squares on its right with a rule
    /// divider between them, and the bottom rule runs unbroken under all of
    /// it. A press on a stepper moves the number the page shows.
    ///
    /// The operator's "visually broken": the value cell hugged its digits,
    /// the steppers sat in the middle of the well, and a stepper's own fill
    /// painted over the rule under it.
    #[test]
    fn the_number_input_is_one_well_and_its_steppers_step() {
        let mut cam = Camera::on("Number input");
        assert_carbon_well(&mut cam, "/n-md", "22-number-input-well", true);
        let well = cam.rect("/n-md");
        let value = cam.rect("n-md/value");
        let dec = cam.rect("n-md/decrement");
        let inc = cam.rect("n-md/increment");
        let divider = cam.rect("n-md/divider");
        assert!(
            value.w > well.w / 2.0,
            "the value cell takes the remainder of the well: {value:?} in {well:?}"
        );
        assert_eq!(
            inc.x + inc.w,
            well.x + well.w,
            "the increment is flush right"
        );
        assert_eq!(
            dec.x + dec.w,
            divider.x,
            "the decrement is flush against the divider"
        );
        assert_eq!(
            divider.x + divider.w,
            inc.x,
            "the divider is flush against the increment"
        );
        assert_eq!(
            (dec.w, dec.h, inc.w, inc.h),
            (well.h, well.h, well.h, well.h)
        );
        let img = raster(&mut cam, "22-number-input-well");
        let fill = px(&img, value.x + value.w / 2.0, value.y + value.h / 2.0);
        assert_eq!(
            px(&img, dec.x + 2.0, dec.y + 2.0),
            fill,
            "a resting stepper has the well's own fill, not a fill of its own"
        );
        assert_ne!(
            px(&img, divider.x, divider.y + divider.h / 2.0),
            fill,
            "the divider is drawn"
        );
        assert_eq!(leaf_text(&cam, "value"), "12");
        cam.click("n-md/increment");
        assert_eq!(leaf_text(&cam, "value"), "13", "a press on + adds one");
        cam.click("n-md/decrement");
        cam.click("n-md/decrement");
        assert_eq!(leaf_text(&cam, "value"), "11", "two presses on - take two");
        cam.shoot("22-number-input-stepped");
    }

    /// Row 30. Moving the pointer across the rail with no button down moves
    /// nothing; a press on the track jumps the value there; and a drag that
    /// leaves the rail keeps dragging until the button comes up.
    ///
    /// The operator's report: "wants to drag when my mouse is close to it,
    /// whether I click it or not. But if I move too fast it stops dragging."
    /// The page was applying every routed pointer move, and hover routes a
    /// move to the node under the pointer; so a hover dragged and a fast
    /// move that left the handle stopped. The page now mirrors the engine's
    /// capture: a press opens the drag and only `GestureEnded` closes it.
    #[test]
    fn the_slider_moves_on_press_and_drag_but_never_on_hover() {
        let mut cam = Camera::on("Slider");
        let rail = cam.rect("/row/rail");
        let mid_y = rail.y + rail.h / 2.0;
        let at_rest = cam.rect("/rail/fill");
        let handle = cam.rect("/rail/handle");
        // Across the track, then across the handle from its left edge to its
        // right. The handle is the one node here that declares `Hover`, and
        // a page that applied hover-routed moves would read the pointer's x
        // off the handle's edge and nudge the value by half a handle: the
        // exact "wants to drag when my mouse is close to it". Its centre
        // would not do, since the pointer there names the value already set.
        for frac in [0.1, 0.3, 0.45, 0.7, 0.95] {
            cam.hover_at(rail.x + rail.w * frac, mid_y);
        }
        for x in [handle.x + 1.0, handle.x + handle.w - 1.0, handle.x + 1.0] {
            cam.hover_at(x, mid_y);
            assert_eq!(
                cam.rect("/rail/fill"),
                at_rest,
                "the pointer crossed the rail and the handle with no button \
                 down and the fill moved: the page is dragging on hover"
            );
            assert!(
                cam.hovered()
                    .as_deref()
                    .is_some_and(|id| id.ends_with("/handle")),
                "the pointer at {x} is over the handle {handle:?} and the host \
                 did not register it"
            );
        }
        cam.shoot("30-slider-hovered");
        cam.click_at(rail.x + rail.w * 0.8, mid_y);
        let jumped = cam.rect("/rail/fill");
        assert!(
            (jumped.w / rail.w - 0.8).abs() < 0.05,
            "a press on the track jumps the value there: fill {} of {}",
            jumped.w,
            rail.w
        );
        cam.shoot("30-slider-jumped");
        let far_below = Point::new(rail.x + rail.w * 0.2, mid_y + 200.0);
        cam.drag("/rail/handle", far_below);
        let dragged = cam.rect("/rail/fill");
        assert!(
            (dragged.w / rail.w - 0.2).abs() < 0.05,
            "the pointer left the rail 200 units below it and let go at 20%; \
             capture holds the drag, so the fill follows: {} of {}",
            dragged.w,
            rail.w
        );
        cam.shoot("30-slider-dragged-off-rail");
    }

    /// The most device pixels that are not `ground` on any one of the
    /// `rows` device rows up from the bottom of `rect`.
    fn widest_mark_under(img: &image::RgbaImage, rect: Rect, ground: [u8; 4], rows: u32) -> usize {
        let bottom = bottom_device_row(rect);
        (0..rows)
            .map(|up| {
                device_row(img, rect.x, rect.x + rect.w, bottom - up)
                    .iter()
                    .filter(|p| **p != ground)
                    .count()
            })
            .max()
            .unwrap_or(0)
    }

    /// Row 15. A standalone link underlines under the pointer; an inline
    /// link is underlined at rest.
    ///
    /// The operator's "does nothing or has any indication what it is, just
    /// looks like a text label". The ink is now `link-primary` and the
    /// underline is the second channel, the one that survives red-green
    /// colour blindness; Carbon underlines a standalone link on hover and an
    /// inline one always (`_link.scss`).
    #[test]
    fn a_link_underlines_on_hover_and_an_inline_link_is_underlined_at_rest() {
        let mut cam = Camera::on("Link");
        let link = cam.rect("/docs");
        let inline = cam.rect("/docs-inline");
        let resting = raster(&mut cam, "15-link-resting");
        let ground = px(&resting, link.x + link.w / 2.0, link.y + link.h + 2.0);
        let width = (link.w * CAPTURE_SCALE) as usize;
        let descenders = widest_mark_under(&resting, link, ground, 3);
        assert!(
            descenders * 2 < width,
            "at rest the standalone link's bottom rows hold only glyph \
             descenders, got {descenders} of {width} marked"
        );
        let inline_width = (inline.w * CAPTURE_SCALE) as usize;
        let inline_rule = widest_mark_under(&resting, inline, ground, 3);
        assert!(
            inline_rule * 10 >= inline_width * 9,
            "an inline link is underlined at rest: {inline_rule} of {inline_width} marked"
        );
        cam.hover("/docs");
        assert!(
            cam.hovered()
                .as_deref()
                .is_some_and(|id| id.ends_with("/docs")),
            "the host did not register the pointer over the link"
        );
        let hovered = raster(&mut cam, "15-link-hovered");
        let underline = widest_mark_under(&hovered, link, ground, 3);
        assert!(
            underline * 10 >= width * 9,
            "under the pointer the link is underlined end to end: {underline} \
             of {width} marked"
        );
    }

    /// Row 34's Port field validated nothing.
    ///
    /// It was a `field_invalid` built with a constant message, so it said
    /// "must be a number" while the operator typed numbers into it. His
    /// words: *"port doesnt work, i put in numbers and it complains theyre
    /// not numbers."*
    ///
    /// Driven the way a hand drives it: click into the well, backspace the
    /// four wrong characters away, type digits, and read the frame back. The
    /// error edge is `support.error`; a legal value binds no `border` at all.
    #[test]
    fn typing_digits_into_the_port_field_clears_its_complaint() {
        let mut cam = Camera::on("Text input");
        let edge = cam.token("field-port/input", "border");
        assert_eq!(
            edge.as_deref(),
            Some("support-error"),
            "the page opens on the invalid state, so the row shows one"
        );

        cam.click("field-port/input");
        for _ in 0.."http".len() {
            cam.key(KeyCode::Backspace);
        }
        cam.type_here("8080");

        assert_eq!(
            cam.token("field-port/input", "border"),
            None,
            "a port of 8080 is a number, so the field must stop saying it is not"
        );
        assert!(
            !cam.has("field-port/helper"),
            "and the helper line must go with it"
        );
        cam.shoot("34-text-input-port-fixed");

        cam.type_here("x");
        assert_eq!(
            cam.token("field-port/input", "border").as_deref(),
            Some("support-error"),
            "and it must come back the moment the value stops being a number"
        );
    }

    /// Row 27's radio group is labelled Theme and offers Dark and Light.
    ///
    /// Until 2026-09-05 choosing Light moved a dot and nothing else. The
    /// operator: *"in here you have dark and light as options, actually
    /// implement that so I can see the light theme version."*
    ///
    /// Driven the way a hand drives it, and read as pixels: a theme swap that
    /// only reached the token map would pass every binding assertion and
    /// still leave a dark window on his screen.
    #[test]
    fn choosing_light_on_the_radio_row_turns_the_whole_catalog_light() {
        let mut cam = Camera::on("Radio button");
        let dark = cam.mean_luma();
        assert!(
            dark < 0.25,
            "the catalog opens on the dark theme, mean luma {dark}"
        );

        cam.click("radio-b");
        let light = cam.mean_luma();
        cam.shoot("27-radio-button-light");
        assert!(
            light > dark + 0.3,
            "choosing Light must light the page up: {dark} -> {light}"
        );

        cam.click("radio-a");
        let back = cam.mean_luma();
        assert!(
            back < dark + 0.05,
            "and choosing Dark must put it back: {light} -> {back}"
        );
    }

    /// Row 6's Copy button dropped every press and looked exactly like a
    /// working one. The operator: *"copy button does not work"*.
    ///
    /// Driven from a click, and read where the string actually leaves — the
    /// `PlatformOutput` command, not the frame. A page-level assertion that
    /// `clipboard_request` returns something would pass with the host wiring
    /// missing, which is the state this row shipped in.
    #[test]
    fn pressing_copy_puts_that_snippet_on_the_clipboard() {
        let mut cam = Camera::on("Code snippet");
        assert!(
            cam.clipboard().is_empty(),
            "nothing is copied before anything is pressed"
        );

        cam.click("snip/copy");
        assert_eq!(
            cam.clipboard(),
            ["pcargo test -p gorgon-petra --lib".to_owned()],
            "the single-line well copies its own one line"
        );

        cam.click("snip-multi/copy-row/copy");
        let last = cam.clipboard().last().expect("a second copy");
        assert!(
            last.lines().count() > 5 && last.contains("xtask verify-notes"),
            "the multi-line well copies its own many lines, got {last:?}"
        );
    }
    // ---- The focus caret on field-shaped controls (round 3, wave F1) ----
    //
    // Every test below starts from a `Camera::click`, because no person has
    // an `Action::Focus`, and reads the raster back: the caret is host-owned
    // geometry that no frame-level assertion can see.

    /// The two hug bars the settled indicator draws around the well keyed
    /// `well_tail`, read back from the raster, or a panic naming which part
    /// of the figure is wrong.
    ///
    /// Asserts, from `FocusRing::STANDARD`'s own geometry: a bar of one
    /// accent colour `hug_gap` outside each side; each bar exactly the
    /// well's height, top and bottom; the gap between bar and well left as
    /// ground; and the ground directly under each bar's foot the same as the
    /// ground beside it, which is what a shadow smudge breaks. Returns the
    /// bar colour so a caller can look for it elsewhere.
    fn assert_hugs_well(cam: &mut Camera, well_tail: &str, shot: &str) -> [u8; 4] {
        let ring = FocusRing::STANDARD;
        let well = cam.rect(well_tail);
        let img = raster(cam, shot);
        let [left, right] = ring.hugs(well);
        let mid_y = well.y + well.h / 2.0;
        let (lx, rx) = (left.x + left.w / 2.0, right.x + right.w / 2.0);
        let bar = px(&img, lx, mid_y);
        assert_eq!(
            px(&img, rx, mid_y),
            bar,
            "{well_tail}: the right hug is not the same colour as the left"
        );
        let inside = px(&img, well.x + 3.0, well.y + 3.0);
        assert_ne!(
            bar, inside,
            "{well_tail}: no bar {ring:?} outside the left edge"
        );
        assert!(
            bar[2] > bar[0] && bar[2] > bar[1],
            "{well_tail}: the bar is not the accent: {bar:?}"
        );
        for (side, x, outward) in [("left", lx, -1.0), ("right", rx, 1.0)] {
            let beside = px(&img, x + outward * 6.0, mid_y);
            assert_ne!(bar, beside, "{well_tail}: the {side} bar bleeds outward");
            let gap = px(
                &img,
                x - outward * (ring.thickness / 2.0 + ring.hug_gap / 2.0),
                mid_y,
            );
            assert_ne!(
                bar, gap,
                "{well_tail}: the {side} bar touches the well; the gap is gone"
            );
            assert_eq!(
                px(&img, x, well.y + 0.5),
                bar,
                "{well_tail}: the {side} bar does not reach the well's top"
            );
            assert_eq!(
                px(&img, x, well.y + well.h - 0.5),
                bar,
                "{well_tail}: the {side} bar stops short of the well's bottom rule"
            );
            assert_ne!(
                px(&img, x, well.y - 1.5),
                bar,
                "{well_tail}: the {side} bar overhangs the well's top"
            );
            let under = px(&img, x, well.y + well.h + 1.5);
            assert_ne!(
                under, bar,
                "{well_tail}: the {side} bar overhangs the well's bottom rule"
            );
            assert_eq!(
                under,
                px(&img, x + outward * 6.0, well.y + well.h + 1.5),
                "{well_tail}: the ground under the {side} bar's foot is darker \
                 than the ground beside it: a shadow smudge past the rule"
            );
        }
        bar
    }

    /// The device row where an underline under `rect` would sit.
    fn underline_row(rect: Rect) -> u32 {
        let ring = FocusRing::STANDARD;
        ((rect.y + rect.h + ring.gap + ring.thickness / 2.0) * CAPTURE_SCALE) as u32
    }

    /// Row 28. A click in the search field brackets the **well**, magnifier
    /// included, not the input leaf beside the glyph.
    ///
    /// The operator's "the cursor going to the sides doesn't respect the
    /// whole box and needs a bit of padding; has clipping". The left bar
    /// stood between the magnifier and the text, and the overlay shadow
    /// put a smudge under its foot past the rule.
    #[test]
    fn clicking_the_search_field_brackets_the_whole_well() {
        let mut cam = Camera::on("Search");
        cam.click("query/input");
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("query/input")),
            "the click did not seat focus in the field: {:?}",
            cam.focused()
        );
        let well = cam.rect("/query");
        let leaf = cam.rect("query/input");
        assert!(
            leaf.x > well.x + 8.0,
            "the leaf sits past the glyph: {leaf:?} in {well:?}"
        );
        assert_hugs_well(&mut cam, "/query", "28-search-focused");
        // Where a hug of the leaf would stand: inside the well, beside the
        // glyph. It has to be the well's own fill.
        let img = raster(&mut cam, "28-search-focused");
        let inside = px(&img, well.x + 3.0, well.y + 3.0);
        assert_eq!(
            px(&img, leaf.x - 5.5, well.y + 3.0),
            inside,
            "a bar stands inside the well between the magnifier and the text"
        );
    }

    /// Row 22. A click in the number field brackets the **well**, steppers
    /// included, not the value cell beside the Subtract glyph.
    #[test]
    fn clicking_the_number_value_brackets_the_whole_well() {
        let mut cam = Camera::on("Number input");
        cam.click("n-md/value");
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("n-md/value")),
            "the click did not seat focus in the value cell: {:?}",
            cam.focused()
        );
        let well = cam.rect("/n-md");
        let value = cam.rect("n-md/value");
        assert!(
            value.x + value.w < well.x + well.w - 40.0,
            "the steppers sit past the value"
        );
        assert_hugs_well(&mut cam, "/n-md", "22-number-input-focused");
        let img = raster(&mut cam, "22-number-input-focused");
        let inside = px(&img, well.x + 3.0, well.y + 3.0);
        assert_eq!(
            px(&img, value.x + value.w + 4.5, well.y + 3.0),
            inside,
            "a bar stands in the middle of the well, beside the Subtract stepper"
        );
    }

    /// Row 29. A click on the select field opens the list and brackets the
    /// field; nothing is drawn across the list's first row, where a
    /// button's underline used to land.
    #[test]
    fn clicking_the_select_field_brackets_it_and_nothing_crosses_the_open_list() {
        let mut cam = Camera::on("Select");
        cam.click("theme/field");
        assert!(cam.has("theme/menu"), "the click did not open the list");
        assert!(
            cam.ring()
                .as_deref()
                .is_some_and(|id| id.ends_with("theme/field")),
            "focus left the field when the list opened: {:?}",
            cam.ring()
        );
        let field = cam.rect("theme/field");
        let bar = assert_hugs_well(&mut cam, "theme/field", "29-select-open-focused");
        let img = raster(&mut cam, "29-select-open-focused");
        let row = device_row(&img, field.x, field.x + field.w, underline_row(field));
        assert!(
            !row.contains(&bar),
            "an underline crosses the open list's first row"
        );
    }

    /// Row 11. The dropdown field: the same anatomy as row 29.
    #[test]
    fn clicking_the_dropdown_field_brackets_it_and_nothing_crosses_the_open_list() {
        let mut cam = Camera::on("Dropdown");
        cam.click("dd/field");
        assert!(cam.has("dd/menu"), "the click did not open the list");
        assert!(
            cam.ring()
                .as_deref()
                .is_some_and(|id| id.ends_with("dd/field")),
            "focus left the field when the list opened: {:?}",
            cam.ring()
        );
        let field = cam.rect("dd/field");
        let bar = assert_hugs_well(&mut cam, "dd/field", "11-dropdown-open-focused");
        let img = raster(&mut cam, "11-dropdown-open-focused");
        let row = device_row(&img, field.x, field.x + field.w, underline_row(field));
        assert!(
            !row.contains(&bar),
            "an underline crosses the open list's first row"
        );
    }

    /// Row 10. A click on the date field opens the calendar, **keeps focus
    /// on the field**, and brackets it.
    ///
    /// The operator's "when I click it the cursor flies away". The closed
    /// form was the field and the open form wrapped it, so the id focus sat
    /// on stopped existing as a focusable the moment the calendar opened,
    /// and the vanished-focus rule sent focus to the chrome's Next button.
    #[test]
    fn clicking_the_date_field_keeps_focus_on_it_and_brackets_it() {
        let mut cam = Camera::on("Date picker");
        cam.click("when/field");
        assert!(
            cam.has("when/calendar"),
            "the click did not open the calendar"
        );
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("when/field")),
            "focus flew away from the field when the calendar opened: {:?}",
            cam.focused()
        );
        assert!(
            cam.ring()
                .as_deref()
                .is_some_and(|id| id.ends_with("when/field")),
            "the frame rings something other than the field: {:?}",
            cam.ring()
        );
        let field = cam.rect("when/field");
        let bar = assert_hugs_well(&mut cam, "when/field", "10-date-picker-open-focused");
        let img = raster(&mut cam, "10-date-picker-open-focused");
        let row = device_row(&img, field.x, field.x + field.w, underline_row(field));
        assert!(
            !row.contains(&bar),
            "an underline crosses the calendar's top"
        );
    }

    /// Row 37. A click on the toggletip trigger opens the tip and brackets
    /// the trigger, the way a text input is bracketed; nothing is drawn on
    /// the tip's beak, where the underline used to land.
    #[test]
    fn clicking_the_toggletip_trigger_brackets_it_like_a_text_input() {
        let mut cam = Camera::on("Toggletip");
        cam.click("tt/trigger");
        assert!(cam.has("tt/tip"), "the click did not open the tip");
        let trigger = cam.rect("tt/trigger");
        let bar = assert_hugs_well(&mut cam, "tt/trigger", "37-toggletip-open-focused");
        let img = raster(&mut cam, "37-toggletip-open-focused");
        let row = device_row(
            &img,
            trigger.x,
            trigger.x + trigger.w,
            underline_row(trigger),
        );
        assert!(
            !row.contains(&bar),
            "an underline sits under the trigger, on the tip's beak"
        );
    }

    /// Row 18. A menu's trigger is a button and underlines; while its menu
    /// is open flush beneath it, the underline would cross the menu's first
    /// row, so it is withheld — and comes back the moment the menu shuts.
    #[test]
    fn a_menu_trigger_underline_is_withheld_while_the_menu_covers_it() {
        let mut cam = Camera::on("Menu");
        cam.click("mn-pair/trigger");
        assert!(
            cam.has("mn-pair/menu"),
            "the first click did not open the menu"
        );
        cam.click("mn-pair/trigger");
        assert!(
            !cam.has("mn-pair/menu"),
            "the second click did not shut the menu"
        );
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("mn-pair/trigger")),
            "focus is not on the trigger after two clicks: {:?}",
            cam.focused()
        );
        let trigger = cam.rect("mn-pair/trigger");
        let shut = raster(&mut cam, "18-menu-shut-focused");
        let row = underline_row(trigger);
        let bar = px(
            &shut,
            trigger.x + trigger.w / 2.0,
            row as f32 / CAPTURE_SCALE,
        );
        assert!(
            bar[2] > bar[0] && bar[2] > bar[1],
            "with the menu shut the trigger's underline is not there: {bar:?}"
        );
        cam.click("mn-pair/trigger");
        assert!(
            cam.has("mn-pair/menu"),
            "the third click did not reopen the menu"
        );
        let open = raster(&mut cam, "18-menu-open-focused");
        let across = device_row(&open, trigger.x, trigger.x + trigger.w, row);
        assert!(
            !across.contains(&bar),
            "the trigger's underline is painted across the open menu's first row"
        );
    }

    /// Row 34. A bare text field's hug stands `hug_gap` off the well with no
    /// smudge under its foot: the same figure the seven rows above get,
    /// checked on the control that always had it.
    #[test]
    fn clicking_a_text_field_brackets_it_off_the_well_with_a_clean_foot() {
        let mut cam = Camera::on("Text input");
        cam.click("field-md");
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("field-md")),
            "the click did not seat focus in the field: {:?}",
            cam.focused()
        );
        assert_hugs_well(&mut cam, "field-md", "34-text-input-md-focused");
    }

    // ------------------------------------------------------------------
    // Wave C, round 3: rows 41 and 42. One contiguous block, appended.
    // ------------------------------------------------------------------

    /// Row 41. Picking a width mode changes the panel's width, and the
    /// picture with it.
    ///
    /// The page shipped one mode, titled "Fixed panel", and Carbon's left
    /// panel *is* its width modes — every modifier in `_side-nav.scss:65-117`
    /// sets `inline-size` and nothing else. A page showing one of four
    /// shows none of the component, which is what "you kinda fucked this up
    /// on a conceptual level" meant.
    ///
    /// Driven from a click on the mode selector, and the widths are read
    /// out of placed geometry, not out of the mode enum.
    #[test]
    fn picking_a_left_panel_width_mode_resizes_the_panel_and_repaints_it() {
        let mut cam = Camera::on("UI shell left panel");
        let mut pictures = Vec::new();
        for (control, want, shot) in [
            ("shell-left-rail", 48.0, "41-ui-shell-left-panel-rail"),
            ("shell-left-fixed", 256.0, "41-ui-shell-left-panel"),
            (
                "shell-left-expandable",
                256.0,
                "41-ui-shell-left-panel-expandable",
            ),
            ("shell-left-hidden", 0.0, "41-ui-shell-left-panel-hidden"),
        ] {
            cam.click(control);
            let rect = cam.rect("shell-left-content/shell-left");
            assert!(
                (rect.w - want).abs() < 0.5,
                "{control}: Carbon's own inline-size for this mode is {want}, \
                 the panel placed {rect:?}"
            );
            pictures.push((control, cam.shoot(shot)));
        }
        for (i, (a, pa)) in pictures.iter().enumerate() {
            for (b, pb) in &pictures[i + 1..] {
                assert_ne!(
                    pa, pb,
                    "{a} and {b} rasterize identically: the mode selector \
                     moved a dot and nothing else"
                );
            }
        }
    }

    /// Row 41. The rail keeps the icon and loses the label, on the same
    /// rows the other three modes draw in full.
    ///
    /// This is the mode `ui_shell_left_panel_rail` has never been able to
    /// show. No item constructor had an icon slot, so 48px of squeezed
    /// label was the whole picture, and no catalog page mounted it. The
    /// glyph is the only channel a 48px column has.
    #[test]
    fn the_rail_keeps_its_icons_and_loses_its_labels() {
        let mut cam = Camera::on("UI shell left panel");

        cam.click("shell-left-fixed");
        let wide = cam.rect("shell-left-petra/row/body/lead/label");
        assert!(
            wide.w > 8.0,
            "at 256 the label is drawn: {wide:?} — otherwise the rail proves \
             nothing"
        );

        cam.click("shell-left-rail");
        let panel = cam.rect("shell-left-content/shell-left");
        let icon = cam.rect("shell-left-petra/row/body/lead/icon");
        let label = cam.rect("shell-left-petra/row/body/lead/label");
        assert!(
            (panel.w - 48.0).abs() < 0.5,
            "the rail is Carbon's mini-units(6): {panel:?}"
        );
        assert!(
            (icon.x - panel.x - 16.0).abs() < 0.5 && (icon.w - 16.0).abs() < 0.5,
            "the 16px glyph keeps its full size at the row's own 16 inset, \
             which centres it in a 48 rail: panel {panel:?}, icon {icon:?}"
        );
        assert!(
            icon.x + icon.w <= panel.x + panel.w + 0.5,
            "and it fits inside the rail: icon {icon:?}, panel {panel:?}"
        );
        assert!(
            label.w < 0.5,
            "the label has to go: 16 + 16 + 16 leaves it nothing, and the \
             panel clips whatever is left. Got {label:?} in {panel:?}"
        );
        cam.shoot("41-ui-shell-left-panel-rail-icons");
    }

    /// Row 41. The hamburger is the Expandable panel's one control, and
    /// pressing it opens and shuts the panel.
    ///
    /// Carbon keeps one boolean and hands it to `HeaderMenuButton` as
    /// `isActive` and to `SideNav` as `expanded` (`HeaderContainer.js:24-35`).
    /// Row 40's complaint — "the 4 parallel bars on the left side has no
    /// children to display" — is that same button with the second consumer
    /// missing. Here it has one.
    #[test]
    fn the_hamburger_opens_and_shuts_the_expandable_left_panel() {
        let mut cam = Camera::on("UI shell left panel");
        cam.click("shell-left-expandable");
        let open = cam.shoot("41-ui-shell-left-panel-expandable-open");
        assert!(
            (cam.rect("shell-left-content/shell-left").w - 256.0).abs() < 0.5,
            "Expandable starts open"
        );
        cam.click("shell-left-menu");
        let shut = cam.shoot("41-ui-shell-left-panel-expandable-shut");
        assert!(
            cam.rect("shell-left-content/shell-left").w < 0.5,
            "pressing the hamburger must take the panel to width 0, got {:?}",
            cam.rect("shell-left-content/shell-left")
        );
        assert_ne!(
            open, shut,
            "the panel closed in the tree and not one pixel moved"
        );
        cam.click("shell-left-menu");
        assert!(
            (cam.rect("shell-left-content/shell-left").w - 256.0).abs() < 0.5,
            "and a second press must reopen it"
        );
    }

    /// Row 41. Collapsing a branch over the current page keeps the mark on
    /// the branch.
    ///
    /// `_side-nav.scss:283-289`: `--item--active __submenu[aria-expanded='false']`
    /// takes `$background-selected` and its own 3px `::before`. The nested
    /// row unmounts when the branch shuts (FR-026), so without this the
    /// operator collapses "Kernel" over a current "Fibers" and nothing on
    /// screen says which page is open.
    ///
    /// The comparison is between two *collapsed* Kernels — one over a
    /// current child, one not — so the only difference in the picture is
    /// the mark itself, not the sub-menu opening and closing.
    #[test]
    fn a_collapsed_left_panel_branch_wears_its_current_childs_mark() {
        let mut cam = Camera::on("UI shell left panel");
        cam.click("shell-left-fibers");
        cam.click("shell-left-kernel/row");
        assert!(
            !cam.has("shell-left-fibers"),
            "the branch is collapsed, so the current row is off screen"
        );
        let over_current = cam.shoot("41-ui-shell-left-panel-collapsed-current");
        assert!(
            selected_ids(&cam)
                .iter()
                .any(|id| id.ends_with("shell-left-kernel")),
            "the collapsed branch does not carry the flag the `layer-selected` \
             fill and the accent bar are painted from. Selected: {:?}",
            selected_ids(&cam)
        );

        cam.click("shell-left-kernel/row");
        cam.click("shell-left-petra");
        cam.click("shell-left-kernel/row");
        assert!(!cam.has("shell-left-fibers"), "collapsed again");
        let over_nothing = cam.shoot("41-ui-shell-left-panel-collapsed-plain");
        assert!(
            !selected_ids(&cam)
                .iter()
                .any(|id| id.ends_with("shell-left-kernel")),
            "and a collapsed branch over no current page must not wear the \
             mark, or the mark says nothing"
        );

        assert_ne!(
            over_current, over_nothing,
            "a collapsed branch holding the current page looks the same as \
             one that is not: the current page has vanished from the picture"
        );
    }

    /// Row 42. Pressing the header action shuts the panel and pressing it
    /// again reopens it, and shut is width 0 rather than unmounted.
    ///
    /// `_header-panel.scss` transitions `width` alone and never unmounts;
    /// this page used to mount the panel conditionally, so its resting
    /// photograph was a header and an empty card.
    #[test]
    fn the_right_panels_trigger_shuts_it_to_width_zero_and_reopens_it() {
        let mut cam = Camera::on("UI shell right panel");
        let open = cam.shoot("42-ui-shell-right-panel-open");
        assert!(
            (cam.rect("shell-content/shell-switcher").w - 256.0).abs() < 0.5,
            "the row rests open, so the row's own subject is in its own picture"
        );
        cam.click("shell-switcher-trigger");
        let shut = cam.shoot("42-ui-shell-right-panel-shut");
        let rect = cam.rect("shell-content/shell-switcher");
        assert!(
            rect.w < 0.5,
            "shut is `inline-size: 0`, still placed, got {rect:?}"
        );
        assert_ne!(open, shut, "the trigger fired and nothing moved");
        cam.click("shell-switcher-trigger");
        assert!(
            (cam.rect("shell-content/shell-switcher").w - 256.0).abs() < 0.5,
            "a second press must reopen it"
        );
    }

    /// Row 42. Choosing an app marks it and shuts the panel.
    ///
    /// The selected state is the one this module argued in writing that
    /// Carbon does not have. It does: `_switcher.scss` carries
    /// `--switcher__item-link--selected` and `SwitcherItem.js:29` carries
    /// `isSelected`, against one sentence of usage-page prose, and T070
    /// ranks the SCSS first.
    #[test]
    fn choosing_an_app_in_the_switcher_marks_it_and_shuts_the_panel() {
        let mut cam = Camera::on("UI shell right panel");
        let first = cam.shoot("42-ui-shell-right-panel-petra-current");
        assert!(
            selected_ids(&cam).iter().any(|id| id.ends_with("petra")),
            "the first app rests current. Selected: {:?}",
            selected_ids(&cam)
        );
        cam.click("shell-switcher-inspector");
        assert!(
            cam.rect("shell-content/shell-switcher").w < 0.5,
            "picking a destination shuts the panel"
        );
        cam.click("shell-switcher-trigger");
        let second = cam.shoot("42-ui-shell-right-panel-inspector-current");
        let marked = selected_ids(&cam);
        assert!(
            marked.iter().any(|id| id.ends_with("inspector"))
                && !marked.iter().any(|id| id.ends_with("petra")),
            "the mark did not move to the app that was pressed. Selected: {marked:?}"
        );
        assert_ne!(
            first, second,
            "the mark moved in the tree and not one pixel changed"
        );
    }

    /// Every placement the frame reports as selected. For asserting a mark
    /// moved, where the mark is a `semantics.selected` flag the engine
    /// paints rather than a node of its own.
    fn selected_ids(cam: &Camera) -> Vec<String> {
        cam.frame()
            .placements
            .iter()
            .filter(|p| p.semantics.selected)
            .map(|p| p.id.clone())
            .collect()
    }
}
