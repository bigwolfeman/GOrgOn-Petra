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
use gorgon_petra::geom::Size;
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

    /// Move the pointer to a raw position — for leaving a node, where there is
    /// no node to name.
    pub fn hover_at(&mut self, x: f32, y: f32) -> &mut Self {
        self.act(
            Target::Pos(gorgon_petra::geom::Point::new(x, y)),
            &Action::Hover,
        )
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

    /// A full keystroke to whatever holds focus.
    pub fn key(&mut self, key: KeyCode) -> &mut Self {
        let id = self.id("/root");
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
}
