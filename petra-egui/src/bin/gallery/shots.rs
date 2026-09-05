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
use gorgon_petra::frame::PetrifiedFrame;
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
        ctx.run_ui(sized(raw), |_| self.host.pass(&ctx))
            .drop_without_applying_deltas();
        self
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
    use super::Camera;
    use crate::catalog::WINDOW;
    use gorgon_petra::geom::{Point, Rect};

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
            cam.has("snip-multi/copy"),
            "the well carries its Copy button"
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

    /// Row 9. The zebra form: the second body row binds the raised fill.
    #[test]
    fn the_data_table_is_the_zebra_form() {
        let mut cam = Camera::on("Data table");
        cam.shoot("09-data-table");
        let tree = cam.tree();
        let fill = |key: &str| {
            crate::page::common::find(&tree, key)
                .unwrap()
                .props
                .tokens
                .get("background")
                .map(|t| t.as_str().to_owned())
        };
        assert_eq!(fill("dt-0").as_deref(), Some("surface.base"));
        assert_eq!(
            fill("dt-1").as_deref(),
            Some("surface.raised"),
            "the odd row is not striped: the page is not calling `data_table_zebra`"
        );
    }

    /// Row 23. The page called the three-argument `pagination`, so the
    /// bar's whole left half was empty. Now the items group and the range
    /// are on the bar, and Next moves both the page and the range.
    #[test]
    fn the_pagination_bar_has_its_items_group_and_pages_forward() {
        let mut cam = Camera::on("Pagination");
        let first = cam.shoot("23-pagination");
        assert!(
            cam.has("pager/bar/items-per-page"),
            "no items-per-page group"
        );
        assert!(cam.has("range/range-text"), "no range text");
        assert_eq!(leaf_text(&cam, "page-size"), "10");
        assert_eq!(leaf_text(&cam, "range-text"), "1\u{2013}10 of 50 items");
        assert_eq!(leaf_text(&cam, "page"), "1");
        cam.click("controls/next");
        let second = cam.shoot("23-pagination-page-two");
        assert_eq!(leaf_text(&cam, "page"), "2");
        assert_eq!(leaf_text(&cam, "range-text"), "11\u{2013}20 of 50 items");
        assert_ne!(first, second);
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
        cam.click("mn-1");
        cam.shoot("18-menu-after-delete");
        assert!(
            !cam.has("mn-pair/menu"),
            "choosing an item did not close the menu"
        );
    }

    /// Row 19. The trigger opens its menu over the page.
    #[test]
    fn the_menu_button_opens_its_menu_over_the_page() {
        let mut cam = Camera::on("Menu buttons");
        opens_over_the_page(
            &mut cam,
            "mb/trigger",
            "mb/menu",
            40.0,
            "19-menu-buttons-open",
        );
        assert!(cam.has("menu/content/mb-0"));
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
    #[test]
    fn the_popover_opens_over_the_page_and_dismisses_outside() {
        let mut cam = Camera::on("Popover");
        opens_over_the_page(
            &mut cam,
            "po-pair/pop-anchor",
            "po-pair/pop-note",
            30.0,
            "24-popover-open",
        );
        assert!(
            cam.has("pop-note/content/caret"),
            "the popover has its caret"
        );
        press_empty_ground(&mut cam);
        cam.shoot("24-popover-dismissed");
        assert!(
            !cam.has("po-pair/pop-note"),
            "a press outside did not dismiss it"
        );
    }

    /// Row 29. `select_open` is new this wave: the field opens a list over
    /// the page and an option becomes the value.
    #[test]
    fn the_select_opens_its_list_over_the_page_and_an_option_selects() {
        let mut cam = Camera::on("Select");
        opens_over_the_page(&mut cam, "sel/theme", "theme/menu", 100.0, "29-select-open");
        cam.click("opt-system");
        cam.shoot("29-select-system");
        assert!(
            !cam.has("theme/menu"),
            "choosing an option did not close the list"
        );
        assert_eq!(leaf_text(&cam, "value"), "System");
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
        cam.hover_at(WINDOW[0] - 8.0, WINDOW[1] - 8.0);
        cam.shoot("38-tooltip-left");
        assert!(
            !cam.has("bubble"),
            "the pointer left the trigger for empty ground and the bubble stayed"
        );
    }

    /// Row 42. The switcher opens from the header action, docked under the
    /// header's trailing edge at Carbon's 256, over the page.
    #[test]
    fn the_right_panel_opens_under_the_header_over_the_page() {
        let mut cam = Camera::on("UI shell right panel");
        let panel = opens_over_the_page(
            &mut cam,
            "shell-switcher-trigger",
            "shell-right/shell-switcher",
            60.0,
            "42-ui-shell-right-panel-open",
        );
        let header = cam.rect("shell-right/shell-header");
        assert!(
            (panel.w - 256.0).abs() < 0.5,
            "the panel is Carbon's 256 wide, got {panel:?}"
        );
        assert!(
            (panel.x + panel.w - (header.x + header.w)).abs() < 1.0,
            "the panel's trailing edge is the header's: panel {panel:?}, header {header:?}"
        );
        assert!(cam.has("shell-switcher-petra") && cam.has("shell-switcher-inspector"));
        cam.click("shell-switcher-inspector");
        assert!(
            !cam.has("shell-right/shell-switcher"),
            "choosing an app did not close it"
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
}
