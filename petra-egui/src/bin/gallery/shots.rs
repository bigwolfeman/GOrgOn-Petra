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
use gorgon_petra::tree::FocusFigure;
use gorgon_petra_egui::host::{App, Host, default_presenter};
use gorgon_petra_egui::inject::{
    Action, Target, inject_action, push_pointer_down, push_pointer_move, push_pointer_up,
};
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
pub struct Camera<A: App = Catalog> {
    ctx: Context,
    host: Host<A>,
    shooter: Snapshotter,
    /// The page's row name, so a failure says which page it was on.
    page: String,
    /// Everything the app has asked egui to put on the clipboard since this
    /// camera opened. A driven `Copy` control is otherwise invisible: the
    /// string leaves through `PlatformOutput`, not through the frame.
    clipboard: Vec<String>,
    /// Logical seconds handed to egui, advanced by one 60 Hz frame per pass.
    ///
    /// Without this every pass reports whatever wall-clock time it ran at, and
    /// a driven step runs microseconds after the one before it — so a 160 ms
    /// caret hop or a 140 ms toggle slide is, to the driver, permanently at
    /// its first instant. Anything whose defect only appears while a clock is
    /// running is invisible to a driver with no clock, which is how row 30's
    /// nine-frame freeze survived a green suite.
    clock: f64,
    /// Where the pointer was left, so [`Camera::release`] can let go there
    /// rather than inventing a move a device never sent.
    pointer: Option<Point>,
}

/// One 60 Hz frame, the step [`Camera`] advances its clock by per pass.
const FRAME: f64 = 1.0 / 60.0;

impl Camera<Catalog> {
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
        // egui has already run a pass on its own wall clock, and
        // `emath::History` panics if time ever steps backwards. So the driver
        // clock starts from where egui left off rather than from zero.
        let clock = ctx.input(|input| input.time);
        let mut cam = Self {
            ctx,
            host,
            shooter: Snapshotter::new(),
            page: component.to_owned(),
            clipboard: Vec::new(),
            clock,
            pointer: None,
        };
        cam.settle();
        cam
    }
}

impl<A: App> Camera<A> {
    /// Run one pass with no input, so the frame matches the current state.
    fn settle(&mut self) {
        self.step(RawInput::default());
    }

    /// Run exactly one [`Host::pass`] over `raw`, one 60 Hz frame after the
    /// last one, and keep whatever the app put on the clipboard.
    ///
    /// The single place a pass is run, so no driving step can accidentally
    /// skip the clock.
    fn step(&mut self, raw: RawInput) {
        let raw = self.timed(raw);
        let ctx = self.ctx.clone();
        let out = ctx.run_ui(raw, |_| self.host.pass(&ctx));
        self.clipboard.extend(
            out.platform_output
                .commands
                .iter()
                .filter_map(|cmd| match cmd {
                    egui::OutputCommand::CopyText(text) => Some(text.clone()),
                    _ => None,
                }),
        );
        out.drop_without_applying_deltas();
    }

    /// Whether the focus caret is mid-hop.
    ///
    /// The one piece of host state a latency test has to read: `Host::pass`'s
    /// frame-reuse path is reachable only while this is true, so a test that
    /// means to exercise it has to prove it got there.
    pub fn host_caret_is_moving(&self) -> bool {
        self.host.caret().is_moving()
    }

    /// Advance the driver clock by one 60 Hz frame and stamp `raw` with it,
    /// window size and all.
    ///
    /// **Every** pass this camera runs goes through here, [`Camera::shoot`]'s
    /// included. A pass that stamped no time would leave egui to fall back on
    /// its own wall clock, and `emath::History` panics the moment time steps
    /// backwards from that into a driver frame.
    fn timed(&mut self, mut raw: RawInput) -> RawInput {
        self.clock += FRAME;
        raw.time = Some(self.clock);
        sized(raw)
    }

    /// Stop reducing motion, so transitions and the focus caret run at their
    /// real durations.
    ///
    /// [`Camera::on`] reduces motion so a shot taken straight after a click
    /// shows the settled result. A test **about** latency must not: the whole
    /// of row 30's defect lived in `Host::can_reuse_frame`, which only ever
    /// fires while the caret is in flight, and reduced motion means it never
    /// is.
    pub fn live_motion(&mut self) -> &mut Self {
        self.host.set_reduced_motion(false);
        self
    }

    /// Advance one 60 Hz frame with no input, the way a window pumps while a
    /// transition runs.
    ///
    /// [`Camera::live_motion`] plus this is how a *mid-flight* frame is
    /// reached: `focus` starts the hop, and each `tick` is one more frame of
    /// it. Without this the only way to step the clock was to send input,
    /// which changes hover and press state and so changes the picture for a
    /// second reason.
    pub fn tick(&mut self) -> &mut Self {
        self.settle();
        self
    }

    /// Press the primary button on the node whose id ends `tail`, in a pass of
    /// its own, and hold it.
    ///
    /// The opening of a frame-by-frame gesture. Pair with [`Camera::move_to`]
    /// and [`Camera::release`]; [`Camera::drag`] is the one-pass form.
    ///
    /// [`Camera::click`] cannot photograph a held state at all: its press and
    /// its release land in one pass, so every picture taken after it is of a
    /// control that has already come back up. A press animation is invisible
    /// to a driver that only knows how to click, which is how one could ship
    /// and no test see it. Row 04's crunch was exactly that.
    ///
    /// The camera is left with a button down, which is a real state a real
    /// pointer can be in; the next driving step sees it, as it would on a desk.
    ///
    /// # Panics
    /// As [`Camera::id`] does, on a missing or ambiguous tail.
    pub fn press(&mut self, tail: &str) -> &mut Self {
        let rect = self.rect(tail);
        let at = Point::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        let mut raw = RawInput::default();
        push_pointer_down(&mut raw, at, Modifiers::default());
        self.pointer = Some(at);
        self.step(raw);
        self
    }

    /// Move the pointer to `to` in a pass of its own, changing no button.
    ///
    /// Under a held press this is one frame of a drag, and the frame the
    /// window would have painted for it is the frame this camera can then
    /// read. With nothing held it is one frame of a hover.
    pub fn move_to(&mut self, to: Point) -> &mut Self {
        let mut raw = RawInput::default();
        push_pointer_move(&mut raw, to);
        self.pointer = Some(to);
        self.step(raw);
        self
    }

    /// Let the primary button up where the pointer is, in a pass of its own.
    ///
    /// # Panics
    /// If nothing has moved or pressed the pointer yet — there is no position
    /// to release at, and inventing one would be a move the device never sent.
    pub fn release(&mut self) -> &mut Self {
        let at = self
            .pointer
            .unwrap_or_else(|| panic!("{}: release with the pointer nowhere", self.page));
        let mut raw = RawInput::default();
        push_pointer_up(&mut raw, at, Modifiers::default());
        self.step(raw);
        self
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
        self.step(raw);
        self
    }

    /// Everything the page has put on the clipboard since this camera opened.
    #[must_use]
    pub fn clipboard(&self) -> &[String] {
        &self.clipboard
    }

    /// Drop files on the window, the way a file manager does.
    ///
    /// The one thing that makes `App::files_dropped` testable. `Host::pass`
    /// reads `RawInput::dropped_files`, so a drop is expressible as raw
    /// input and needs no window, no file manager and no real file — which
    /// is exactly why the seam was put in `Host::pass` and not in the
    /// `eframe` layer, where nothing headless could ever reach it.
    ///
    /// The paths need not exist. Only the name crosses the seam.
    pub fn drop_files(&mut self, paths: &[&str]) -> &mut Self {
        let raw = RawInput {
            dropped_files: paths
                .iter()
                .map(|path| -> egui::DroppedFileHandle {
                    std::sync::Arc::new(TestDrop(std::path::PathBuf::from(path)))
                })
                .collect(),
            ..RawInput::default()
        };
        self.step(raw);
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

    /// A press at `from`, a move through the midpoint, and a release at `to`
    /// — [`Camera::drag`] aimed by raw position at both ends.
    ///
    /// A drag that starts at a node's centre is the right driver for a slider
    /// handle and the wrong one for a text selection, where *where in the
    /// run* the press landed is the whole of what is being tested.
    pub fn drag_at(&mut self, from: Point, to: Point) -> &mut Self {
        self.act(
            Target::Pos(from),
            &Action::Drag {
                to,
                modifiers: Modifiers::default(),
            },
        )
    }

    /// [`Camera::key`] with modifiers held — the copy chord, and anything
    /// else that is a chord rather than a key.
    ///
    /// # Panics
    /// If the frame holds no placements.
    pub fn chord(&mut self, key: KeyCode, modifiers: Modifiers) -> &mut Self {
        let frame = self.frame();
        let id = frame
            .placements
            .iter()
            .find(|p| p.semantics.focused)
            .or_else(|| frame.placements.first())
            .unwrap_or_else(|| panic!("{}: the frame holds no placements", self.page))
            .id
            .clone();
        self.act(Target::NodeId(id), &Action::Key { key, modifiers })
    }

    /// The byte range of its own painted string that the node whose id ends
    /// `tail` reports selected, or `None`.
    ///
    /// Read off the frame, which is the same place the painter reads it and
    /// the same place the application reads it — so a test asserting on this
    /// is asserting on the one number all three share.
    ///
    /// # Panics
    /// As [`Camera::id`] does, on a missing or ambiguous tail.
    pub fn selection(&self, tail: &str) -> Option<std::ops::Range<usize>> {
        self.paint(tail).selection.clone()
    }

    /// The text the node whose id ends `tail` paints, verbatim.
    ///
    /// # Panics
    /// As [`Camera::id`] does, and if the node paints no text at all.
    pub fn painted_text(&self, tail: &str) -> String {
        self.paint(tail)
            .text
            .as_ref()
            .unwrap_or_else(|| panic!("{}: {tail} paints no text", self.page))
            .text
            .clone()
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

    /// The accessible name the frame carries for the node whose id ends
    /// `tail`, or `None` when it names nothing.
    ///
    /// The channel a screen reader gets, and the one a copy control's
    /// feedback has to move as well as the picture: Carbon swaps
    /// `CopyButton`'s own `aria-label` to the feedback string while the
    /// bubble is up (`@carbon/react/lib/components/Copy/Copy.js:61`), so a
    /// test that only read pixels would pass a control that told a reader
    /// nothing had happened.
    ///
    /// # Panics
    /// As [`Camera::id`] does, on a missing or ambiguous tail.
    pub fn label(&self, tail: &str) -> Option<String> {
        let id = self.id(tail);
        self.frame()
            .placement(&id)
            .unwrap_or_else(|| panic!("{}: {id} resolved but is not placed", self.page))
            .semantics
            .label
            .clone()
    }

    /// Whether the frame carries the node whose id ends `tail` as selected.
    ///
    /// The declared fact, not the fill. Two controls that write one piece of
    /// state — the chrome's theme switcher and row 27's radio group — are
    /// how a control comes to show the wrong answer, and the fill is the
    /// weaker channel for an operator who is red-green colour blind. This
    /// reads the channel that is supposed to be authoritative.
    ///
    /// # Panics
    /// As [`Camera::id`] does, on a missing or ambiguous tail.
    pub fn selected(&self, tail: &str) -> bool {
        let id = self.id(tail);
        self.frame()
            .placement(&id)
            .unwrap_or_else(|| panic!("{}: {id} resolved but is not placed", self.page))
            .semantics
            .selected
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
        let raw = self.timed(RawInput::default());
        let ctx = self.ctx.clone();
        let output = ctx.run_ui(raw, |_| self.host.pass(&ctx));
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

/// A page of focusable nodes whose focus figures the test picks.
///
/// The catalog is Carbon's 42 components and every one of them declares the
/// figure Carbon gives it. After the tab citation was corrected on
/// 2026-09-05 — `_tabs.scss:596` is `// Item Selected`, not a focus rule,
/// and `.cds--tabs__nav-link:focus` is `focus-outline('outline')` — no
/// shipped component declares [`FocusFigure::BarUnder`]. So no catalog row
/// can be photographed wearing one, and no catalog page can put a
/// `BarUnder` and another figure side by side for a morph.
///
/// Building nodes that declare the figures is the honest way to photograph
/// them. Putting a wrong figure on a real control so that a picture exists
/// is what produced the defect this fixture replaces. Anything measured
/// here is a property of a figure; nothing here is a claim about a
/// component.
struct Fixture {
    subjects: Vec<(&'static str, FocusFigure)>,
    /// Taken by the first [`App::theme_request`] and never asked for again.
    pending: Option<gorgon_petra::token::Theme>,
}

impl gorgon_petra::layout::RowSource for Fixture {
    fn rows(
        &mut self,
        _source: &str,
        _range: std::ops::Range<usize>,
    ) -> Vec<std::sync::Arc<gorgon_petra::tree::ViewNode>> {
        Vec::new()
    }
}

impl App for Fixture {
    fn view(&mut self) -> gorgon_petra::tree::ViewNode {
        use crate::page::common::{body, column, sp};

        // Plain buttons rather than components that carry marks of their
        // own, so nothing in the picture is inherited from a control. The
        // figure is set here and never inferred from the role, which is the
        // rule `paint.rs`'s `the_figure_is_the_declaration_not_the_role`
        // holds for the shipped path too.
        let subjects: Vec<gorgon_petra::tree::ViewNode> = self
            .subjects
            .iter()
            .map(|(key, figure)| {
                let mut node = gorgon_petra::component::button(*key, "Subject");
                node.semantics.focus_figure = *figure;
                node
            })
            .collect();
        // A wide gap between them and around them: `BarUnder`'s bar hangs
        // below its node and `Sides`' bars stand outside it, so a tight
        // column would put one figure's indicator on another's node.
        let inner = column("inner", sp("spacing.3xl"), subjects);
        let mut page = body("body", sp("spacing.3xl"), vec![inner]);
        // The catalog paints the card its pages sit on. A fixture has no
        // shell, so it binds its own ground — without it every pixel of the
        // capture is the clear colour and a light-theme reading is a
        // reading of nothing.
        page.props.tokens.insert(
            "background".into(),
            crate::page::common::tok("surface.base"),
        );
        page.props.padding = Some(gorgon_petra::tree::InsetRefs::all(
            crate::page::common::tok("spacing.3xl"),
        ));
        page
    }

    fn handle(
        &mut self,
        _event: &gorgon_petra::input::InputEvent,
        _route: &gorgon_petra::input::Route,
        _frame: Option<&gorgon_petra::frame::PetrifiedFrame>,
    ) {
    }

    /// The fixture rebuilds its whole tree every pass, so `All` is the only
    /// honest answer — the same reason `Catalog::take_changes` gives.
    fn take_changes(&mut self) -> gorgon_petra::layout::ChangeSet {
        gorgon_petra::layout::ChangeSet::All
    }

    /// Published once. `Host::pass` reads this at the top of a pass, so the
    /// theme is in force from the second pass on and every photograph this
    /// fixture takes is settled in it.
    fn theme_request(&mut self) -> Option<gorgon_petra::token::Theme> {
        self.pending.take()
    }
}

impl Camera<Fixture> {
    /// Host a [`Fixture`] the same way [`Camera::at_scale`] hosts the
    /// catalog, so every driving primitive on `Camera` works on it.
    ///
    /// `theme` is `"dark"` or `"light"`; anything else panics, because a
    /// typo would otherwise silently photograph the wrong one.
    fn fixture(subjects: &[(&'static str, FocusFigure)], theme: &str) -> Self {
        let picked = match theme {
            "dark" => gorgon_petra::token::dark(),
            "light" => gorgon_petra::token::light(),
            other => panic!("no shipped theme is named {other:?}"),
        };
        let app = Fixture {
            subjects: subjects.to_vec(),
            pending: Some(picked),
        };
        let ctx = headless();
        ctx.set_pixels_per_point(CAPTURE_SCALE);
        let mut host = Host::new(&ctx, app, default_presenter());
        ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx))
            .drop_without_applying_deltas();
        host.set_reduced_motion(true);
        let clock = ctx.input(|input| input.time);
        let mut cam = Self {
            ctx,
            host,
            shooter: Snapshotter::new(),
            page: format!("fixture({theme})"),
            clipboard: Vec::new(),
            clock,
            pointer: None,
        };
        // Two passes: the first delivers the theme request, the second lays
        // out and paints under it.
        cam.settle();
        cam.settle();
        let luma = cam.mean_luma();
        match theme {
            "light" => assert!(luma > 0.5, "the fixture is not light: luma {luma}"),
            _ => assert!(luma < 0.5, "the fixture is not dark: luma {luma}"),
        }
        cam
    }
}

/// A dropped file that is only a path.
///
/// `egui::DroppedFile` can also read its own bytes; nothing on Petra's side
/// of the seam may, so this refuses. A test that started depending on the
/// bytes would be testing something the contract does not offer.
#[derive(Debug)]
struct TestDrop(std::path::PathBuf);

impl egui::DroppedFile for TestDrop {
    fn path(&self) -> &std::path::Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        Err("a dropped path carries no bytes across this seam".to_owned())
    }
}

/// The driver moves the catalog itself: these two reach into
/// [`Catalog`]'s own pages and mean nothing for any other application.
impl Camera<Catalog> {
    /// Put the whole catalog on the light theme, then come back to this page.
    ///
    /// The theme belongs to row 27's radio group and the host holds whatever
    /// that page last published, so switching it means visiting row 27,
    /// choosing Light and returning. That is exactly what a hand does, and
    /// the return trip is `Catalog::open` rather than an index-row click so
    /// it reaches a row below the pane's fold.
    ///
    /// Light is where a shadow or an off-by-a-few-units mark shows: R6 was
    /// invisible dark-on-dark and obvious on the light card.
    pub fn light(&mut self) -> &mut Self {
        let here = self.page.clone();
        self.host.app_mut().open("Radio button");
        self.settle();
        self.click("radio-b");
        // One more pass on row 27 before leaving it. `Host::pass` reads
        // `theme_request` at the *top* of a pass, before it delivers that
        // pass's input, so the choice the click just made is still pending
        // when the click's own pass ends. Walking away here would leave it
        // pending on a page nobody asks again, and the catalog would stay
        // dark under a `light()` that reported nothing wrong.
        self.settle();
        self.host.app_mut().open(&here);
        self.settle();
        assert!(
            self.mean_luma() > 0.5,
            "{}: the light theme did not take; mean luma is still dark",
            self.page
        );
        self
    }

    /// The open page's body as its module builds it now. For reading back a
    /// fact the frame does not carry (a leaf's text); see
    /// `Catalog::open_page_body`.
    pub fn tree(&self) -> gorgon_petra::tree::ViewNode {
        self.host.app().open_page_body()
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
    use super::{CAPTURE_SCALE, Camera, FocusFigure};
    use crate::catalog::WINDOW;
    use gorgon_petra::geom::{Point, Rect};
    use gorgon_petra::input::{KeyCode, Modifiers};
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

    /// Row 30. **The handle is under the pointer on every frame of a drag,
    /// not on every ninth one.**
    ///
    /// The operator, twice: *"slider still renders poorly. It is laggy
    /// compared to the cursor. It needs to update like the cursor does."*
    /// `dragging_the_slider_handle_moves_the_fill_the_way_the_pointer_went`
    /// above was green through both reports, because it delivers the press,
    /// both moves and the release in **one** `RawInput` and photographs where
    /// the gesture finished. A defect that takes nine passes to show cannot
    /// appear in a one-pass gesture.
    ///
    /// So this drives the drag the way the window pumps it — one pass per
    /// event, on a real 60 Hz clock, with motion **not** reduced — and checks
    /// the painted handle after every single one.
    ///
    /// What it caught, measured on 2026-09-05 at the parent of this commit:
    /// the press seats focus on the handle, which starts the focus caret's
    /// 160 ms hop; `Host::deliver_input` answered "this event does not change
    /// the picture" for every move under that hop, so `Host::can_reuse_frame`
    /// repainted the stale frame and never asked the page for a tree.
    /// `Slider::volume` moved the whole time and none of it was painted. It is
    /// a loop, not a single stall: the frame that finally rebuilds moves the
    /// handle, the caret chases the handle it just moved, and the chase
    /// freezes the next nine frames. At four units of travel per frame the
    /// painted handle updated once every nine frames and fell up to 56 units
    /// behind the pointer. `Host::picture_must_rebuild` now answers for a move
    /// under a pointer capture the way it answers for a press.
    ///
    /// Half a unit of tolerance, not a proportion of the rail: the claim is
    /// that the handle's centre is *under the pointer*, and the arithmetic in
    /// `slider_value_at` is exact, so anything looser would pass with a frame
    /// of lag in it.
    #[test]
    fn the_slider_handle_is_painted_under_the_pointer_on_every_frame_of_a_drag() {
        let mut cam = Camera::at_scale("Slider", 1.0);
        cam.live_motion();
        let rail = cam.rect("/row/rail");
        let handle = cam.rect("/rail/handle");
        let y = handle.y + handle.h / 2.0;
        let start = handle.x + handle.w / 2.0;
        // 40 frames x 4 units of travel has to finish short of the rail's end,
        // or `normalise`'s clamp would hold the handle still and agree with a
        // pointer that had stopped too — the one way "0 behind" could be true
        // and mean nothing.
        assert!(
            start + 160.0 < rail.x + rail.w - handle.w,
            "the drag must stay off the clamp: it ends at {} and the rail's \
             last handle centre is {}",
            start + 160.0,
            rail.x + rail.w - handle.w / 2.0
        );
        cam.press("/rail/handle");
        assert!(
            cam.host_caret_is_moving(),
            "the press must seat focus on the handle and start the caret hop, \
             or this test is not exercising the frame-reuse path at all"
        );

        // Four units per frame for forty frames: a slow, ordinary drag, and
        // long enough to cover several 160 ms hops.
        let mut worst = 0.0_f32;
        for step in 1..=40i16 {
            let x = start + 4.0 * f32::from(step);
            cam.move_to(Point::new(x, y));
            let painted = cam.rect("/rail/handle");
            let centre = painted.x + painted.w / 2.0;
            worst = worst.max((centre - x).abs());
            assert!(
                (centre - x).abs() < 0.5,
                "frame {step} of the drag: the pointer is at {x:.2} and the \
                 painted handle centre is at {centre:.2}, {:.2} behind. The \
                 handle has to be under the pointer on the frame that carried \
                 the move, not on some later one.",
                x - centre
            );
        }
        assert!(
            worst < 0.5,
            "worst tracking error over the drag was {worst:.3} units"
        );
        cam.release();
        let released = cam.rect("/rail/handle");
        assert!(
            (released.x + released.w / 2.0 - (start + 160.0)).abs() < 0.5,
            "the release leaves the handle where the last move put it"
        );
    }

    /// The same drag, photographed six frames in and twenty-four frames in,
    /// so the fix is **looked at** rather than only asserted.
    ///
    /// The capture is the whole point here. Its `assert_ne!` is deliberately
    /// weak and is not the gate — 18 frames apart the handle moved even with
    /// the defect present, so this test was green before the fix and is green
    /// after it. The gate is
    /// [`the_slider_handle_is_painted_under_the_pointer_on_every_frame_of_a_drag`]
    /// above. Read the PNGs: the handle sits at 50% with the input reading
    /// `50` in the first, at 78% reading `78` in the second, the filled track
    /// ends at the handle in both, and the focus caret is under the handle
    /// rather than behind it.
    #[test]
    fn the_slider_handle_mid_drag_is_photographed_at_two_positions() {
        let mut cam = Camera::on("Slider");
        cam.live_motion();
        let rail = cam.rect("/row/rail");
        let handle = cam.rect("/rail/handle");
        let y = handle.y + handle.h / 2.0;
        let start = handle.x + handle.w / 2.0;
        assert!(
            start + 240.0 < rail.x + rail.w - handle.w,
            "the drag must stay off `normalise`'s clamp"
        );
        cam.press("/rail/handle");
        for step in 1..=6i16 {
            cam.move_to(Point::new(start + 10.0 * f32::from(step), y));
        }
        let first = cam.shoot("30-slider-mid-drag-a");
        for step in 7..=24i16 {
            cam.move_to(Point::new(start + 10.0 * f32::from(step), y));
        }
        let second = cam.shoot("30-slider-mid-drag-b");
        cam.release();
        assert_ne!(
            first, second,
            "the handle moved 180 units between the two shots and not one \
             pixel changed: the drag never reached the picture"
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

    /// Photograph every page the T0 batch changed, in both themes.
    ///
    /// The batch added a disabled treatment to checkbox, radio and toggle, a
    /// four-step size scale to dropdown, six button sizes, and all twelve
    /// popover placements. Each of those is a claim about **what a person
    /// sees**, and this repository has learned twice that a frame-record
    /// test stays green over a visibly broken layout. So the pictures are
    /// the evidence and this test is what writes them.
    ///
    /// It is a real gate, not only a shooter: a page that rasterizes to one
    /// flat colour has failed to draw, and a light shot that is not lighter
    /// than its dark twin means the theme never reached the frame — the
    /// exact defect [`the_nav_row_theme_switcher_lights_the_catalog_from_any_page`]
    /// was written for, checked here on the pages the batch touched rather
    /// than on row 04 alone.
    #[test]
    fn every_page_the_size_and_state_batch_touched_is_photographed_in_both_themes() {
        for (page, name) in [
            ("Checkbox", "05-checkbox-disabled"),
            ("Radio button", "27-radio-disabled"),
            ("Toggle", "36-toggle-disabled"),
            ("Dropdown", "11-dropdown-sizes"),
            ("Button", "04-button-sizes"),
            // `24-popover-resting`, not `24-popover-placements`. This camera
            // never presses anything, so it photographs row 24 at rest — the
            // `po-pair` panel open and no placement grid. The grid's own
            // picture is written by
            // [`the_twelve_placement_bubbles_neither_overlap_nor_leave_the_page`],
            // which clicks the trigger first. Both names were
            // `24-popover-placements` for one run, and whichever test
            // finished last owned the file: a picture called "placements"
            // that could hold no placements at all, depending on thread
            // scheduling.
            ("Popover", "24-popover-resting"),
        ] {
            let mut dark = Camera::on(page);
            let dark_luma = dark.mean_luma();
            dark.shoot(&format!("{name}-dark"));

            let mut light = Camera::on(page);
            light.light();
            let light_luma = light.mean_luma();
            light.shoot(&format!("{name}-light"));

            assert!(
                light_luma > dark_luma,
                "{page}: the light shot ({light_luma:.4}) is not lighter than the dark one \
                 ({dark_luma:.4}), so the theme never reached the frame"
            );
        }
    }

    /// No two of the Popover page's twelve placement bubbles overlap, and
    /// none of them leaves the page's own content column.
    ///
    /// A popover bubble is an **overlay**: it paints outside its parent's
    /// box and the layout pass reserves nothing for it. So a grid of twelve
    /// of them is the one page in the catalog where correct components and
    /// a correct engine still photograph as a fault, and three separate
    /// attempts at this grid did exactly that — triggers sitting on their
    /// neighbours' bubbles, and then, once the grid ran past the fold,
    /// `ClampRule::Flip` correctly turning the `bottom-*` row into a second
    /// `top-*` row. Neither showed up in any assertion. Only the picture
    /// did, which is what this test is here to stop.
    #[test]
    fn the_twelve_placement_bubbles_neither_overlap_nor_leave_the_page() {
        let mut cam = Camera::on("Popover");
        // The grid shows when the `po-pair` panel is shut: see that page's
        // own note on why one overlay demo at a time is the only honest
        // arrangement on a single viewport.
        cam.click("pop-anchor");
        let tags = [
            "top-start",
            "top",
            "top-end",
            "right-start",
            "right",
            "right-end",
            "bottom-start",
            "bottom",
            "bottom-end",
            "left-start",
            "left",
            "left-end",
        ];
        let bubbles: Vec<_> = tags
            .iter()
            .map(|tag| (*tag, cam.rect(&format!("pop-grid-{tag}-bubble"))))
            .collect();

        for (i, (a_tag, a)) in bubbles.iter().enumerate() {
            for (b_tag, b) in &bubbles[i + 1..] {
                let apart =
                    a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(
                    apart,
                    "{a_tag} and {b_tag} overlap: {a_tag} at {a:?}, {b_tag} at {b:?}"
                );
            }
        }

        // Both axes. The vertical is the one that bit: four 148-unit rows
        // ran to y 1024 against a 900-unit viewport, the `left-*` row sat
        // wholly below the fold, and all three of its bubbles clamped to
        // the viewport's bottom edge — 82 units off their own triggers and
        // sitting on the `bottom-*` row's. A horizontal-only check saw
        // none of it.
        let page = cam.rect("pop-grid");
        for (tag, r) in &bubbles {
            assert!(
                r.x >= page.x && r.x + r.w <= page.x + page.w,
                "{tag} leaves the grid horizontally: bubble at {r:?}, grid {page:?}"
            );
            assert!(
                r.y >= page.y && r.y + r.h <= page.y + page.h,
                "{tag} leaves the grid vertically: bubble at {r:?}, grid {page:?}"
            );
        }

        // Every bubble sits on the side it is named for.
        //
        // Without this the page can photograph as a lie and still pass:
        // `ClampRule::Flip` measures a bubble against the **viewport**, and
        // when the grid ran past the fold it correctly flipped the
        // `bottom-*` row above its triggers, where it read as a second
        // `top-*` row. Overlap and containment were both still clean. Only
        // the side is the claim the page is actually making.
        for (tag, r) in &bubbles {
            let anchor = cam.rect(&format!("pop-grid-{tag}"));
            let (edge, _) = tag.split_once('-').unwrap_or((tag, ""));
            let ok = match edge {
                "top" => r.y + r.h <= anchor.y,
                "bottom" => r.y >= anchor.y + anchor.h,
                "left" => r.x + r.w <= anchor.x,
                "right" => r.x >= anchor.x + anchor.w,
                other => panic!("unknown edge in {tag:?}: {other:?}"),
            };
            assert!(
                ok,
                "{tag} is not on its own side of its trigger: bubble {r:?}, trigger {anchor:?}"
            );
        }

        // The picture, from the same state the assertions just measured.
        cam.shoot("24-popover-placements-dark");
        let mut light = Camera::on("Popover");
        light.light();
        light.click("pop-anchor");
        light.shoot("24-popover-placements-light");
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

    /// The text of the leaf keyed `key` inside the subtree keyed `owner`.
    ///
    /// Row 10 shows two date pickers whose insides carry identical keys, so
    /// a bare `find` for `"value"` answers whichever picker the page
    /// happened to build first.
    fn leaf_text_in(cam: &Camera, owner: &str, key: &str) -> String {
        let tree = cam.tree();
        let pane = crate::page::common::find(&tree, owner)
            .unwrap_or_else(|| panic!("no subtree keyed {owner:?} in the page tree"));
        crate::page::common::find(pane, key)
            .unwrap_or_else(|| panic!("no node keyed {key:?} under {owner:?}"))
            .props
            .text
            .clone()
            .unwrap_or_else(|| panic!("{owner:?}/{key:?} paints no text"))
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

    /// Row 6, the second half of *"it needs to support colorization for
    /// later LSP integration"*: the colour actually reaches the raster.
    ///
    /// Read off the pixels, because every layer below this one can be green
    /// on a snippet that draws in a single ink. `Props::runs` accepts, the
    /// digest moves, the shaper builds sections — and if the painter passed
    /// the plain galley, or the theme resolved every class to the same
    /// value, the picture would still be one colour and nothing in the frame
    /// record would say so.
    ///
    /// Three claims, all inside the multi-line well's own rect:
    ///
    /// 1. There are at least three distinct inks in it. One-ink code is one
    ///    ink plus the fill plus antialiasing, so a comfortable floor is
    ///    "three tones that are each far from the fill and far from each
    ///    other".
    /// 2. The comment ink is dimmer than the command ink. That is the one
    ///    ordering every syntax theme agrees on and it is the operator's
    ///    channel — he is red-green colourblind, so the classes have to
    ///    separate by lightness and not only by hue.
    /// 3. The blue command ink is genuinely blue: blue minus red is wide.
    ///    Without it, three greys would pass claim 1.
    #[test]
    fn the_snippet_draws_its_syntax_classes_in_three_separable_inks() {
        let mut cam = Camera::on("Code snippet");
        let well = cam.rect("code/snip-multi");
        let shot = raster(&mut cam, "06-code-snippet-coloured");

        // Tones far enough from the well's fill to be ink rather than
        // antialiasing, counted by how many pixels wear each one, so a
        // single stray blended pixel cannot pass for a class.
        let fill = px(&shot, well.x + well.w - 4.0, well.y + well.h - 4.0);
        let luma = |p: [u8; 4]| {
            (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000
        };
        let mut counts: std::collections::HashMap<[u8; 4], usize> =
            std::collections::HashMap::new();
        for p in inset_pixels(&shot, well, 2) {
            if luma(p).abs_diff(luma(fill)) > 40 {
                *counts.entry(p).or_default() += 1;
            }
        }
        let mut inks: Vec<[u8; 4]> = counts
            .into_iter()
            .filter(|(_, n)| *n > 200)
            .map(|(p, _)| p)
            .collect();
        inks.sort_by_key(|p| luma(*p));
        assert!(
            inks.len() >= 3,
            "the well draws {} solid ink tones; a snippet with no colour \
             runs draws one, so the runs are not reaching the picture. Got \
             {inks:?} against a fill of {fill:?}",
            inks.len()
        );

        let (dimmest, brightest) = (inks[0], inks[inks.len() - 1]);
        assert!(
            luma(brightest) - luma(dimmest) > 40,
            "the classes are {} luma apart, which is not a channel a \
             colourblind reader can use: {dimmest:?} vs {brightest:?}",
            luma(brightest) - luma(dimmest)
        );
        assert!(
            inks.iter().any(|p| u32::from(p[2]) > u32::from(p[0]) + 60),
            "no blue ink in the well, so the three tones are three greys \
             and the keyword class is not resolving: {inks:?}"
        );
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
        cam.click("sl-0/c0/name");
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
            "compact/field",
            "compact/calendar",
            336.0,
            "10-date-picker-open",
        );
        assert!(
            calendar.w >= 288.0,
            "the calendar is Carbon's 288 wide, got {calendar:?}"
        );
        cam.click("compact/calendar/content/days/day-12");
        cam.shoot("10-date-picker-picked");
        assert!(
            !cam.has("compact/calendar"),
            "picking a day did not close it"
        );
        assert_eq!(leaf_text_in(&cam, "compact", "value"), "2026-08-12");
        cam.click("compact/field");
        assert!(cam.has("compact/calendar"));
        press_empty_ground(&mut cam);
        assert!(
            !cam.has("compact/calendar"),
            "a press outside did not dismiss it"
        );
    }

    /// Row 10, round 4. The operator: *"the date picker doesnt let me go
    /// to other months, I click an arrow and it goes away"*.
    ///
    /// Driven, because a photograph of the open calendar cannot tell a
    /// working arrow from one that shuts the panel. Two assertions, and the
    /// first one alone is the operator's whole complaint: the calendar is
    /// still placed after the press. The second says the press did the job
    /// it was drawn for rather than merely surviving.
    #[test]
    fn stepping_the_month_arrow_keeps_the_calendar_open_and_turns_the_month() {
        let mut cam = Camera::on("Date picker");
        cam.click("compact/field");
        assert!(cam.has("compact/calendar"), "the field did not open it");
        let august = leaf_text_in(&cam, "compact", "month");
        cam.click("compact/calendar/content/month-header/next-month");
        assert!(
            cam.has("compact/calendar"),
            "the next-month arrow dismissed the calendar"
        );
        assert_eq!(
            leaf_text_in(&cam, "compact", "month"),
            "September 2026",
            "the arrow did not turn the month on from {august:?}"
        );
        assert_eq!(
            leaf_text_in(&cam, "compact", "value"),
            "2026-08-30",
            "browsing dragged the selected date with it"
        );
        cam.click("compact/calendar/content/month-header/prev-month");
        cam.click("compact/calendar/content/month-header/prev-month");
        assert!(
            cam.has("compact/calendar"),
            "the previous-month arrow dismissed the calendar"
        );
        assert_eq!(leaf_text_in(&cam, "compact", "month"), "July 2026");
        cam.shoot("10-date-picker-compact-open");
    }

    /// Row 10, round 4, the full form. The operator: *"we need 2 date
    /// pickers 'compact' and 'full' ... and in the full view the month is a
    /// button, not just a label."*
    ///
    /// Driven end to end, because the chooser is a surface **inside** a
    /// surface and everything that could go wrong there is invisible in a
    /// tree: whether the router reaches a control the day grid is under,
    /// whether `DismissOutside` on the inner panel shuts the outer one with
    /// it, and whether a press on the month button toggles or fights its own
    /// dismissal.
    #[test]
    fn the_full_forms_month_button_raises_a_chooser_that_moves_the_grid() {
        let mut cam = Camera::on("Date picker");
        cam.click("full/field");
        assert!(cam.has("full/calendar"), "the full field did not open");
        assert!(
            !cam.has("compact/month-button"),
            "the compact form grew a month button"
        );
        let closed = cam.shoot("10-date-picker-full-open");

        cam.click("full/calendar/content/month-header/month-button");
        assert!(
            cam.has("full/calendar/content/chooser"),
            "the month button raised nothing. Placed:\n  {}",
            cam.ids().join("\n  ")
        );
        assert!(
            cam.has("full/calendar"),
            "raising the chooser shut the calendar under it"
        );
        let chooser = cam.rect("full/calendar/content/chooser");
        let calendar = cam.rect("full/calendar");
        assert!(
            chooser.x >= calendar.x - 0.5
                && chooser.y >= calendar.y - 0.5
                && chooser.x + chooser.w <= calendar.x + calendar.w + 0.5
                && chooser.y + chooser.h <= calendar.y + calendar.h + 0.5,
            "the chooser at {chooser:?} escapes the calendar at {calendar:?}, \
             so every press inside it is a press outside the calendar"
        );
        let open = cam.shoot("10-date-picker-full-choosing");
        assert_ne!(
            closed, open,
            "the chooser was placed and not one pixel changed"
        );

        cam.click("chooser/chooser-content/year-header/next-year");
        assert_eq!(leaf_text_in(&cam, "full", "year"), "2027");
        assert!(
            cam.has("full/calendar/content/chooser"),
            "the year arrow shut it"
        );

        cam.click("chooser/chooser-content/months/mon-3");
        assert!(
            !cam.has("full/calendar/content/chooser"),
            "picking a month left the chooser up"
        );
        assert!(
            cam.has("full/calendar"),
            "picking a month shut the calendar it was picked in"
        );
        assert_eq!(leaf_text_in(&cam, "full", "month"), "March 2027");
        assert_eq!(
            leaf_text_in(&cam, "full", "value"),
            "2026-08-30",
            "choosing a month moved the selected date"
        );
        cam.shoot("10-date-picker-full-march");

        // And the compact picker beside it never opened.
        assert!(
            !cam.has("compact/calendar"),
            "driving the full picker opened the compact one"
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

    /// Row 18. Carbon's `Menu` seats keyboard focus on its first item when
    /// it opens, and hands focus back to whatever had it when it closes.
    ///
    /// `@carbon/react/lib/components/Menu/Menu.js`: `handleOpen` (66-85)
    /// saves `document.activeElement` in `focusReturn` and focuses the menu
    /// itself; the `useEffect` at 177-186 then calls `focusItem()` with no
    /// event, which lands on index 0 (97-111); `handleClose` (86-89) calls
    /// `returnFocus()`, putting focus back on the trigger.
    ///
    /// The recorded defect (round-3 catalog row 18): Petra left focus on the
    /// trigger, and the cover rule then withheld the trigger's underline
    /// because the open menu sat on top of where it would land — so the
    /// operator saw no focus indicator anywhere until the menu shut.
    #[test]
    fn opening_the_menu_seats_focus_on_its_first_item_and_closing_returns_it() {
        let mut cam = Camera::on("Menu");
        cam.click("mn-pair/trigger");
        assert!(cam.has("mn-pair/menu"), "the click did not open the menu");
        // The seat happens after this pass petrified (`Host`'s "seat it
        // after petrify" shape, the same one `enter_open_modal` has), so the
        // ring lands one frame later. One empty pass is that frame.
        cam.settle();
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("menu/content/mn-0")),
            "focus is on {:?}, not the menu's first item",
            cam.focused()
        );
        assert!(
            cam.ring()
                .as_deref()
                .is_some_and(|id| id.ends_with("menu/content/mn-0")),
            "the frame rings {:?}, so the operator sees no indicator inside \
             the open menu",
            cam.ring()
        );
        cam.shoot("18-menu-open-focus-inside");
        press_empty_ground(&mut cam);
        assert!(
            !cam.has("mn-pair/menu"),
            "a press outside did not dismiss the menu"
        );
        cam.settle();
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("mn-pair/trigger")),
            "closing the menu did not hand focus back to the trigger: {:?}",
            cam.focused()
        );
    }

    /// Row 19. The trigger opens its menu over the page.
    ///
    /// A Carbon menu button is a primary button at least 160 wide with its
    /// chevron on the trailing edge, and its menu is a list box flush under
    /// it with no caret, leading edges aligned (slice-b §19).
    ///
    /// Opening the menu moves keyboard focus onto its first item: a menu
    /// button renders the same `Menu` a bare menu does
    /// (`@carbon/react/lib/components/MenuButton/index.js:112`), so it gets
    /// the same `handleOpen` that seats item 0. Until 2026-09-05 this test
    /// asserted the opposite — focus stays on the trigger — which was the
    /// round-3 catalog's row-18 defect written down as a check. The defect it
    /// was really guarding is still guarded: focus must land *somewhere*
    /// real when the open form is a different node, and the assertion below
    /// names where.
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
            cam.ring()
                .is_some_and(|id| id.ends_with("menu/content/mb-0")),
            "opening the menu did not seat the ring on its first item: the \
             ring is on {:?}",
            cam.ring()
        );
        cam.click("mb-0");
        cam.settle();
        assert!(
            cam.focused().is_some_and(|id| id.ends_with("mb/trigger")),
            "choosing an item did not hand focus back to the trigger: {:?}",
            cam.focused()
        );
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
        // Since 2026-09-05 the page mounts its open form at rest — a closed
        // disclosure photographs as one button and no disclosure, which is
        // what the operator walked past on rows 24 and 37. Shut it with a
        // press first, so the shared helper still drives it open from a
        // resting closed state.
        cam.click("po-pair/pop-anchor");
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
        // The page's popover holds caller children now, so its first run is
        // keyed by the page rather than by `popover`'s own one-run form.
        let body = cam.rect("pop-note/content/pop-body");
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
        // Open at rest since 2026-09-05; see the popover test above.
        cam.click("tt/trigger");
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

    /// Row 21: no rail, and the toast is Carbon's 288.
    ///
    /// The operator refused the accent rail twice
    /// (`.agents/carbon-waves/ROUND2-DEFECTS.md` row 21) and it stays
    /// refused. The same sentence asked for the text to be centred, and this
    /// test asserted that until 2026-09-05; centring is what made the status
    /// glyph's inset a function of the title's length, which is the
    /// operator's round-4 line.
    /// `every_notification_kind_hangs_its_glyph_on_the_card_leading_edge`
    /// owns the replacement claim, measured against Carbon's own
    /// stylesheet, and takes the photograph.
    ///
    /// The width stays here because it is the toast's own number:
    /// `inline-size: convert.to-rem(288px)`, `_toast-notification.scss:34`.
    #[test]
    fn the_notification_toast_has_no_rail_and_holds_carbons_288() {
        let cam = Camera::on("Notification");
        assert!(!cam.has("nt/panel/rail"), "the accent rail is still placed");
        let panel = cam.rect("nt/panel");
        assert!(
            (panel.w - 288.0).abs() < 0.5,
            "Carbon's toast is 288 wide, got {panel:?}"
        );
        let glyph = cam.rect("panel/glyph");
        let title = cam.rect("panel/details/title");
        assert!(
            glyph.x + glyph.w <= title.x,
            "the status glyph must lead the title, got {glyph:?} then {title:?}"
        );
        let centre_off = ((title.x + title.w / 2.0) - (panel.x + panel.w / 2.0)).abs();
        assert!(
            centre_off > 0.5,
            "the title is centred in the card again: {title:?} in {panel:?}"
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
        //
        // The rule is not Carbon's `1px`: the separator carries the rule
        // material, so the shipped painter grooves it — two device pixels
        // of shadow above two of highlight — and the node reserves exactly
        // those four.
        //
        // Asserted in **device** pixels, which is the unit the material is
        // specified in. `Camera::rect` answers in logical units and this
        // page captures at `CAPTURE_SCALE`, so the two differ by that
        // factor. They did not use to: the rule pinned four *logical* units
        // from a constant, which reserved eight device pixels here for a
        // four-pixel groove, and the assertion below passed on it. Reading
        // the material through `token::rule::thickness` is what removed
        // both the constant and the mismatch.
        let expected = gorgon_petra::token::rule::total_units();
        for i in 0..2 {
            let rule = cam.rect(&format!("cl/rule-{i}"));
            let device = rule.h * CAPTURE_SCALE;
            assert!(
                (device - expected).abs() < 0.01,
                "the rule material's groove is {expected} device pixels, \
                 this one reserves {device:.2} ({:.2} logical at scale \
                 {CAPTURE_SCALE})",
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
    ///
    /// Generic over the hosted application, so a figure photographed on a
    /// `Fixture` is read back the same way a component on the catalog is.
    fn raster<A: super::App>(cam: &mut Camera<A>, name: &str) -> image::RgbaImage {
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
    /// field is transparent). `grooved` says which material the bottom rule
    /// is drawn in: `false` is `border-strong`, the control-boundary tone
    /// every *interactive* well keeps flat and one device pixel deep; `true`
    /// is `border.subtle`, the decorative tone a **read-only** field binds
    /// (`field::bind_field_chrome`) and which the shipped painter now
    /// grooves — see `crate::token::rule` and this contract's "Scope:
    /// horizontal rules only", which does not carve out an exception for a
    /// field, only for a *control's* boundary.
    fn assert_carbon_well(cam: &mut Camera, tail: &str, shot: &str, filled: bool, grooved: bool) {
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
        if grooved {
            // Four device rows, shadow above highlight: `rule_row` and
            // `rule_row - 1` are the highlight (the physically lower half,
            // sampled here from the bottom up), `rule_row - 2` and
            // `rule_row - 3` are the shadow, and the row above that must be
            // back to the field's own fill — the groove is exactly four
            // device pixels deep, not a thicker flat band.
            let highlight_1 = device_row(&img, rect.x, rect.x + rect.w, rule_row - 1);
            let shadow_0 = device_row(&img, rect.x, rect.x + rect.w, rule_row - 2);
            let shadow_1 = device_row(&img, rect.x, rect.x + rect.w, rule_row - 3);
            let past_the_groove = device_row(&img, rect.x, rect.x + rect.w, rule_row - 4);
            assert!(
                highlight_1.iter().all(|p| *p == rule[0]),
                "{tail}: the highlight band is not two uniform rows"
            );
            assert!(
                shadow_0.iter().all(|p| *p == shadow_0[0]),
                "{tail}: the shadow band is not uniform"
            );
            assert_eq!(
                shadow_1, shadow_0,
                "{tail}: the shadow band is not two uniform rows"
            );
            assert_ne!(
                shadow_0[0], rule[0],
                "{tail}: shadow and highlight must be two distinguishable \
                 strokes, not one flat band twice as deep"
            );
            assert_eq!(
                past_the_groove[0], inside,
                "{tail}: the groove is not exactly four device pixels deep"
            );
        } else {
            let fill_row = device_row(&img, rect.x, rect.x + rect.w, rule_row - 3);
            assert!(
                fill_row.iter().filter(|p| **p == inside).count() * 10 > fill_row.len() * 9,
                "{tail}: the rule is thicker than one snapped pixel"
            );
        }
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
            assert_carbon_well(&mut cam, tail, "34-text-input-wells", true, false);
        }
        assert_carbon_well(&mut cam, "field-ro", "34-text-input-wells", false, true);
    }

    /// Row 28. The search well is the same anatomy, with the glass inset.
    #[test]
    fn the_search_field_is_a_carbon_well_with_the_glass_inside_it() {
        let mut cam = Camera::on("Search");
        assert_carbon_well(&mut cam, "/query", "28-search-well", true, false);
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
        assert_carbon_well(&mut cam, "/n-md", "22-number-input-well", true, false);
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

    /// The theme switcher in the nav row lights the catalog from **any**
    /// page, not just row 27.
    ///
    /// The operator, 2026-09-05: *"Light mode no longer loads from any place
    /// I select it."* Two places were broken and neither was the mechanism.
    /// `catalog::run` built its presenter from `default_presenter()`, which
    /// is dark and reads no environment, so `PETRA_GALLERY_THEME=light` in
    /// front of `--bin gallery` did nothing at all — and the only control
    /// that could move the theme was a radio group on one page out of
    /// forty-two, which the operator had to page to before he could use it.
    /// `choosing_light_on_the_radio_row_turns_the_whole_catalog_light` was
    /// green through both, because it starts on row 27.
    ///
    /// So this one starts somewhere else on purpose.
    #[test]
    fn the_nav_row_theme_switcher_lights_the_catalog_from_any_page() {
        let mut cam = Camera::on("Button");
        let dark = cam.mean_luma();
        assert!(
            dark < 0.25,
            "the catalog opens dark with no environment set, mean luma {dark}"
        );

        cam.click(crate::catalog::THEME_LIGHT);
        let light = cam.mean_luma();
        cam.shoot("00-chrome-theme-light");
        assert!(
            light > dark + 0.3,
            "the chrome switcher must light the page from row 04: {dark} -> {light}"
        );

        cam.click(crate::catalog::THEME_DARK);
        let back = cam.mean_luma();
        assert!(back < dark + 0.05, "and put it back: {light} -> {back}");
    }

    /// The switcher's two segments report the theme that is actually on,
    /// including after row 27's radio group moved it.
    ///
    /// Two controls writing one piece of state is how a control comes to
    /// lie about it. `Catalog::theme_request` writes `self.theme` from
    /// whichever fired, which is what keeps them agreeing.
    #[test]
    fn the_switcher_segments_follow_the_radio_row_that_also_moves_the_theme() {
        let mut cam = Camera::on("Radio button");
        assert!(
            cam.selected(crate::catalog::THEME_DARK) && !cam.selected(crate::catalog::THEME_LIGHT),
            "the catalog opens dark and the switcher says so"
        );

        cam.click("radio-b");
        assert!(cam.mean_luma() > 0.5, "the radio group lit the catalog");
        assert!(
            cam.selected(crate::catalog::THEME_LIGHT) && !cam.selected(crate::catalog::THEME_DARK),
            "and the chrome switcher moved with it, rather than still \
             claiming Dark on a light window"
        );
    }

    /// A control that shows its focus on another node and that node agree
    /// about the figure.
    ///
    /// `FocusFigure` and `FocusShownOn` are orthogonal by design, and the
    /// host resolves them in that order: `focused_caret_target` finds the
    /// rect from the *holder's* `focus_shown_on`, then reads the figure off
    /// the node it landed on — `paint.rs`, `let figure =
    /// shown_on.semantics.focus_figure`. So declaring a figure on the holder
    /// and not on the target is a silent no-op, and the holder's
    /// declaration is the one a reader believes.
    ///
    /// It has bitten twice in two days. `code_snippet` gave its code run
    /// `OnWell` while the well kept the default, and `tree_view` gave the
    /// item `Border` while its head row — the node every tree figure is
    /// actually drawn on — kept whatever the default happened to be, which
    /// changed under it when the default moved on 2026-09-05.
    ///
    /// `Well` is an ancestor and `Head` a descendant, so the two links are
    /// walked in opposite directions.
    ///
    /// # How this goes red
    ///
    /// Drop `row.semantics.focus_figure` from `component::tree_view` and
    /// every tree page names the row.
    #[test]
    fn a_control_and_the_node_it_shows_focus_on_agree_about_the_figure() {
        use gorgon_petra::tree::FocusShownOn;

        let mut wrong: Vec<String> = Vec::new();
        for cell in crate::cell::Cell::roster() {
            if !cell.is_built() {
                continue;
            }
            let cam = Camera::on(cell.row.component);
            let places = &cam.frame().placements;
            for (i, node) in places.iter().enumerate() {
                let target = match node.semantics.focus_shown_on {
                    // Up the parent chain to the nearest `Well`.
                    FocusShownOn::OnWell => {
                        let mut up = places[i].parent;
                        let mut found = None;
                        while let Some(k) = up {
                            if places[k].semantics.focus_shown_on == FocusShownOn::Well {
                                found = Some(k);
                                break;
                            }
                            up = places[k].parent;
                        }
                        found
                    }
                    // Down into the subtree to the nearest `Head`.
                    FocusShownOn::OnHead => places.iter().enumerate().find_map(|(k, p)| {
                        if p.semantics.focus_shown_on != FocusShownOn::Head {
                            return None;
                        }
                        let mut up = p.parent;
                        while let Some(a) = up {
                            if a == i {
                                return Some(k);
                            }
                            up = places[a].parent;
                        }
                        None
                    }),
                    _ => None,
                };
                let Some(k) = target else {
                    continue;
                };
                if node.semantics.focus_figure != places[k].semantics.focus_figure {
                    wrong.push(format!(
                        "{}: {} declares {:?} but its figure is drawn on {}, \
                         which declares {:?}",
                        cell.row.component,
                        node.id,
                        node.semantics.focus_figure,
                        places[k].id,
                        places[k].semantics.focus_figure,
                    ));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "the figure is read off the node focus is shown on, so these \
             holders declare a shape nothing draws:\n{}",
            wrong.join("\n")
        );
    }

    /// Every control that wears `FocusFigure::BarUnder` has somewhere to
    /// put the bar.
    ///
    /// The operator's rule, 2026-09-05: *"underlines are preferred to boxes,
    /// boxes are just for when underlines stick too far off and look bad."*
    /// The second half is a measurement, not a taste: a bar sits
    /// `FocusRing::gap` below the node's bottom edge and is
    /// `FocusRing::thickness` tall, so it needs **five** units of clear run
    /// under the control. Less than that and it paints on whatever is next,
    /// which is how the checkbox group read before this test existed —
    /// `check-a`'s bottom at 308.0, `check-b`'s top at 312.0, and a bar
    /// occupying 310.0 to 313.0, so its last unit was inside the next row's
    /// box and no one could tell which of the two rows it marked.
    ///
    /// Walks all forty-two pages, because the run below a control is a fact
    /// about the *page* that laid it out, not about the component: `toggle`
    /// and `checkbox` are built by neighbouring functions in one file and
    /// their pages leave 12 units and 4.
    ///
    /// Ancestors and descendants are exempt. A control's own label is inside
    /// it and its column contains it; neither is something the bar could be
    /// mistaken for.
    ///
    /// # How this goes red
    ///
    /// Take `FocusFigure::Border` off `component::controls`' `labelled_box`
    /// and the Checkbox and Radio button pages both name a collision.
    #[test]
    fn every_bar_under_has_five_units_of_clear_run_below_it() {
        let ring = FocusRing::STANDARD;
        let mut collisions: Vec<String> = Vec::new();
        for cell in crate::cell::Cell::roster() {
            if !cell.is_built() {
                continue;
            }
            let mut cam = Camera::on(cell.row.component);
            // Open what the page hides behind a trigger. A menu's items, a
            // dropdown's options and a calendar's day cells are the most
            // tightly packed rows in the library and none of them exist in a
            // shut frame, so a gate that only ever looked at the opening
            // picture would hold none of them.
            for trigger in OPENERS {
                if cam.has(trigger) {
                    cam.click(trigger);
                }
            }
            let frame = cam.frame();
            let places = &frame.placements;
            // Ancestry by index, walked through `Placement::parent`.
            let is_kin = |a: usize, b: usize| {
                let mut up = Some(a);
                while let Some(i) = up {
                    if i == b {
                        return true;
                    }
                    up = places[i].parent;
                }
                let mut up = Some(b);
                while let Some(i) = up {
                    if i == a {
                        return true;
                    }
                    up = places[i].parent;
                }
                false
            };
            for (i, node) in places.iter().enumerate() {
                if node.semantics.focus_figure != FocusFigure::BarUnder
                    || !node
                        .semantics
                        .actions
                        .contains(&gorgon_petra::tree::Interaction::Focus)
                    || !node.is_visible()
                {
                    continue;
                }
                let bar = ring.bar(node.rect);
                if caret_is_covered(places, i, &[bar]) {
                    continue;
                }
                for (k, other) in places.iter().enumerate() {
                    // Two kinds of collision count. Another **control**, so
                    // the operator cannot tell which of the two the bar
                    // marks. And any other node carrying its own **fill**,
                    // because a bar laid over a neighbouring card reads as
                    // belonging to that card — the file uploader's drop zone
                    // did exactly that, and a focusable-only rule missed it,
                    // because the file rows under the zone are not themselves
                    // controls; only their Remove buttons are.
                    let is_control = other
                        .semantics
                        .actions
                        .contains(&gorgon_petra::tree::Interaction::Focus);
                    let has_fill = frame.content[k].tokens.contains_key("background");
                    if k == i || (!is_control && !has_fill) || !other.is_visible() || is_kin(i, k) {
                        continue;
                    }
                    if overlaps(bar, other.rect) {
                        collisions.push(format!(
                            "{}: {}'s bar {bar:?} lands on {}",
                            cell.row.component, node.id, other.id
                        ));
                    }
                }
            }
        }
        assert!(
            collisions.is_empty(),
            "a bar under needs {} units of clear run and these do not have \
             it, so they want FocusFigure::BarInside — the same stripe on \
             the node's own bottom edge, which needs no run at all. Not \
             FocusFigure::Border: this message asked for a box four times \
             and the operator asked for the boxes back out again each \
             time.\n{}",
            ring.gap + ring.thickness,
            collisions.join("\n")
        );
    }

    /// Every `Sides` figure in the catalog stands inside the thing that
    /// contains it, and every `BarInside` stripe stays inside its own node.
    ///
    /// The companion to
    /// [`every_bar_under_has_five_units_of_clear_run_below_it`], and it did
    /// not exist until 2026-09-06. That asymmetry is exactly why the modal
    /// shipped with its Close button's right-hand bar painted on the page
    /// behind the dialog: `BarUnder` had a gate and `Sides` had none, so a
    /// figure that reaches seven units outside the rect was never checked
    /// against anything.
    ///
    /// A `Sides` bar is measured against its **nearest filled ancestor** —
    /// the card, dialog or panel it visually sits on. That is the boundary a
    /// person reads as "the box", and leaving it is what reads as a spill.
    /// A `BarInside` stripe is measured against its own node, where it is
    /// contained by construction, so this half of the test is a guard on the
    /// geometry rather than on any component's judgement.
    #[test]
    fn no_focus_figure_paints_outside_the_box_it_belongs_to() {
        let ring = FocusRing::STANDARD;
        let mut escapes: Vec<String> = Vec::new();
        for cell in crate::cell::Cell::roster() {
            if !cell.is_built() {
                continue;
            }
            let mut cam = Camera::on(cell.row.component);
            for trigger in OPENERS {
                if cam.has(trigger) {
                    cam.click(trigger);
                }
            }
            let frame = cam.frame();
            let places = &frame.placements;
            for (i, node) in places.iter().enumerate() {
                if !node
                    .semantics
                    .actions
                    .contains(&gorgon_petra::tree::Interaction::Focus)
                    || !node.is_visible()
                {
                    continue;
                }
                // Resolve `FocusShownOn` the way `paint.rs`'s
                // `focused_caret_target` does, and read the figure off the
                // node the figure is *shown on*. Measuring the focused
                // node's own rect instead reports a search field's input
                // leaf spilling when what actually paints is its well —
                // which is what the first run of this gate did.
                let i = shown_on(places, i);
                let node = &places[i];
                match node.semantics.focus_figure {
                    FocusFigure::BarInside => {
                        let bar = ring.bar_inside(gorgon_petra::focus::marked_rect(
                            places,
                            i,
                            FocusFigure::BarInside,
                        ));
                        if caret_is_covered(places, i, &[bar]) {
                            continue;
                        }
                        if !contains(node.rect, bar) {
                            escapes.push(format!(
                                "{}: {}'s inside stripe {bar:?} leaves its own \
                                 node {:?}",
                                cell.row.component, node.id, node.rect
                            ));
                        }
                    }
                    FocusFigure::Sides => {
                        // The nearest ancestor that paints a fill: the box a
                        // person sees this control sitting on.
                        let mut up = places[i].parent;
                        let mut box_of = None;
                        while let Some(k) = up {
                            if frame.content[k].tokens.contains_key("background") {
                                box_of = Some(places[k].rect);
                                break;
                            }
                            up = places[k].parent;
                        }
                        let Some(container) = box_of else {
                            continue;
                        };
                        if caret_is_covered(places, i, &ring.sides(node.rect)) {
                            continue;
                        }
                        for bar in ring.sides(node.rect) {
                            if !contains(container, bar) {
                                escapes.push(format!(
                                    "{}: {}'s side bar {bar:?} leaves the box \
                                     it sits on {container:?}",
                                    cell.row.component, node.id
                                ));
                            }
                        }
                    }
                    FocusFigure::Border | FocusFigure::BarUnder => {}
                }
            }
        }
        assert!(
            escapes.is_empty(),
            "a focus figure painted outside the box it belongs to. A `Sides` \
             bar reaches {} units past the rect; where that leaves the card, \
             the control wants FocusFigure::BarInside instead:\n{}",
            ring.hug_gap + ring.thickness,
            escapes.join("\n")
        );
    }

    /// The placement a focused `places[i]` shows its figure on, per
    /// [`gorgon_petra::tree::FocusShownOn`].
    ///
    /// `OnWell` points up the ancestor chain at the nearest `Well`; `OnHead`
    /// points down into the subtree at the nearest `Head`. Both fall back to
    /// the node itself rather than going blind, exactly as the painter does.
    fn shown_on(places: &[gorgon_petra::frame::Placement], i: usize) -> usize {
        use gorgon_petra::tree::FocusShownOn;
        match places[i].semantics.focus_shown_on {
            FocusShownOn::Own | FocusShownOn::Well | FocusShownOn::Head => i,
            FocusShownOn::OnWell => {
                let mut up = places[i].parent;
                while let Some(k) = up {
                    if places[k].semantics.focus_shown_on == FocusShownOn::Well {
                        return k;
                    }
                    up = places[k].parent;
                }
                i
            }
            FocusShownOn::OnHead => {
                let descends = |mut k: usize| {
                    while let Some(p) = places[k].parent {
                        if p == i {
                            return true;
                        }
                        k = p;
                    }
                    false
                };
                places
                    .iter()
                    .enumerate()
                    .find(|(k, p)| p.semantics.focus_shown_on == FocusShownOn::Head && descends(*k))
                    .map_or(i, |(k, _)| k)
            }
        }
    }

    /// The rect the figure on `tail` is measured against, resolved exactly as
    /// the painter resolves it: `FocusShownOn` to pick the placement, then
    /// [`gorgon_petra::focus::marked_rect`] to pick the run on it.
    ///
    /// A test that probes a bar has to ask for this and not for
    /// [`Camera::rect`]. Since 2026-09-06 a bar spans the control's label
    /// rather than the control, so on a row that is wide and a label that is
    /// not, the control's own mid-point is off the end of the stripe and a
    /// probe there reads the row's fill.
    fn marked_run(frame: &gorgon_petra::frame::PetrifiedFrame, tail: &str) -> Rect {
        let places = &frame.placements;
        let i = places
            .iter()
            .position(|p| p.id.ends_with(tail))
            .unwrap_or_else(|| panic!("no placement ends with {tail:?}"));
        let i = shown_on(places, i);
        gorgon_petra::focus::marked_rect(places, i, places[i].semantics.focus_figure)
    }

    /// Does `outer` wholly contain `inner`?
    fn contains(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x + inner.w <= outer.x + outer.w
            && inner.y + inner.h <= outer.y + outer.h
    }

    /// Every control wearing a bar has content for the bar to span.
    ///
    /// Since 2026-09-06 a bar's width is the control's content run
    /// ([`gorgon_petra::focus::marked_rect`]), and a control with no content
    /// at all falls back to its own rect. That fallback is right for a snug
    /// control and wrong for anything wide: a full-width stripe on a row is
    /// exactly the picture the rule replaced.
    ///
    /// So the fallback is allowed to happen, and is not allowed to happen
    /// *quietly on something wide*. This names any control where a bar is
    /// spanning the whole of a node wider than a comfortable label, which is
    /// the only shape the fallback can go wrong on.
    ///
    /// Falsify by putting `BarUnder` on a wide iconless container.
    #[test]
    fn every_bar_figure_finds_its_content() {
        // 240 units: the widest a control can be and still read as hugging
        // its own text. The gallery's own left rail is 240 and holds a
        // number and a word; nothing wider than this is a label.
        const SNUG: f32 = 240.0;

        let mut blind = Vec::new();
        for cell in crate::inventory::ROWS {
            let cam = Camera::on(cell.component);
            let frame = cam.frame();
            let places = &frame.placements;
            for (i, node) in places.iter().enumerate() {
                // The variants by name, not `FocusFigure::marks_the_label`.
                // A gate that filters on the predicate it is checking skips
                // its whole population the moment that predicate is wrong,
                // and reports green — which is what the first draft of this
                // one did when the rule was removed to falsify it.
                if !matches!(
                    node.semantics.focus_figure,
                    gorgon_petra::tree::FocusFigure::BarUnder
                        | gorgon_petra::tree::FocusFigure::BarInside
                ) || !node.is_visible()
                {
                    continue;
                }
                if !node
                    .semantics
                    .actions
                    .contains(&gorgon_petra::tree::Interaction::Focus)
                {
                    continue;
                }
                let i = shown_on(places, i);
                let seat = &places[i];
                let run = gorgon_petra::focus::marked_rect(places, i, seat.semantics.focus_figure);
                if run.w >= seat.rect.w && seat.rect.w > SNUG {
                    blind.push(format!(
                        "{}: {} is {:.0} wide, its {:?} found no content, so the bar spans it all",
                        cell.component, seat.id, seat.rect.w, seat.semantics.focus_figure
                    ));
                }
            }
        }
        assert!(
            blind.is_empty(),
            "a bar figure fell back to its control's own rect on a control too              wide for that to read as an underline. Give the control a text              child, or move it to FocusFigure::Sides:\n{}",
            blind.join("\n")
        );
    }

    /// The 2026-09-06 cursor pass, photographed: one focused control on
    /// every row whose focus figure changed, in both themes.
    ///
    /// The operator's rule for that pass is that a control wears an
    /// underline or a pair of brackets and never a box, and the library had
    /// no *contained* underline until then — which is why eleven components
    /// carried a box and the same one-line comment saying rows stack flush.
    /// `FocusFigure::BarInside` is that figure and this is its picture.
    ///
    /// A resting photograph cannot show a focus figure at all, so every one
    /// of these is driven: the camera seats focus on a named control and
    /// then shoots. If a key here stops existing the test panics naming
    /// every placed id, which is the fastest way to find where it went.
    #[test]
    fn every_row_the_cursor_pass_changed_is_photographed_focused() {
        for (page, key, name) in [
            ("Tree view", "tv-gorgon", "39-tree-view-cursor"),
            ("Checkbox", "check-a", "05-checkbox-cursor"),
            ("Radio button", "radio-a", "27-radio-cursor"),
            ("Structured list", "sl-0", "31-structured-list-cursor"),
            ("Tile", "tile-click", "35-tile-cursor"),
            ("Breadcrumb", "bc-0", "03-breadcrumb-cursor"),
            // Row 7, not row 16: `component::list_row` is the interactive
            // row this pass retargeted, and row 16's list items are inert
            // text by design (`component::list`'s own module doc, FR-058).
            ("Contained list", "cl-0", "07-contained-list-cursor"),
            ("Accordion", "acc-0/header", "01-accordion-cursor"),
            ("Data table", "dt-0", "09-data-table-cursor"),
            // Rows 33 and 36 joined the list on 2026-09-06, when the
            // operator caught the bar sitting off-centre on both: a tag's
            // dismiss cross and a toggle's knob are not text, so a run
            // measured from text alone drifted away from them.
            ("Tag", "tag-x", "33-tag-cursor"),
            ("Toggle", "toggle-default-off", "36-toggle-cursor"),
            // Rows 40-42 are what the operator's first sentence named:
            // "on every UI shell element it should be the horizontal cursor
            // or the 2 vertical bars, not the boxed cursor". Five of the
            // seventeen boxes lived in `ui_shell.rs`.
            (
                "UI shell header",
                "shell-nav-overview",
                "40-ui-shell-header-cursor",
            ),
            (
                "UI shell left panel",
                "shell-left-kernel",
                "41-ui-shell-left-cursor",
            ),
            (
                "UI shell right panel",
                "shell-switcher-petra",
                "42-ui-shell-right-cursor",
            ),
        ] {
            for theme in ["dark", "light"] {
                let mut cam = Camera::on(page);
                if theme == "light" {
                    cam.light();
                }
                let before = cam.shoot(&format!("{name}-{theme}-rest"));
                cam.focus(key);
                assert!(
                    cam.ring().is_some_and(|id| id.ends_with(key)),
                    "{page}/{theme}: the driver did not seat focus on {key:?}, \
                     it sits on {:?}",
                    cam.ring()
                );
                let after = cam.shoot(&format!("{name}-{theme}"));
                assert_ne!(
                    before, after,
                    "{page}/{theme}: seating focus on {key:?} changed not one \
                     pixel, so whatever figure it declares never reached the \
                     frame"
                );
            }
        }
    }

    /// What each focusable control is actually made of: its own rect, the
    /// run `marked_rect` hands its figure, and every leaf inside it.
    ///
    /// The instrument behind every width decision in
    /// `gorgon_petra::focus::marked_rect`. Both wrong answers that shipped on
    /// 2026-09-06 came from reasoning about control anatomy without looking
    /// at it: a toggle turned out to carry its caption *inside* the
    /// focusable node, and a checkbox turned out to draw its box as a bare
    /// spacer when empty. Neither is visible from the component source.
    ///
    /// Prints, asserts nothing.
    #[test]
    fn triage_what_each_control_is_made_of() {
        for page in [
            "Checkbox",
            "Menu",
            "Accordion",
            "Contained list",
            "UI shell left panel",
            "Modal",
        ] {
            let mut cam = Camera::on(page);
            for opener in ["open-modal", "mn-pair/trigger"] {
                if cam.has(opener) {
                    cam.click(opener);
                }
            }
            let frame = cam.frame();
            let places = &frame.placements;
            println!("TRIAGE ===== {page}");
            for (i, node) in places.iter().enumerate() {
                let figure = node.semantics.focus_figure;
                if !node
                    .semantics
                    .actions
                    .contains(&gorgon_petra::tree::Interaction::Focus)
                    || !node.is_visible()
                    || node.id.contains("/nav/")
                    || node.id.contains("/rows/")
                    || node.id.contains("/idx-")
                {
                    continue;
                }
                let seat = shown_on(places, i);
                let run = gorgon_petra::focus::marked_rect(
                    places,
                    seat,
                    places[seat].semantics.focus_figure,
                );
                let short = node.id.rsplit('/').take(3).collect::<Vec<_>>().join("\\");
                println!(
                    "TRIAGE {short}  {:?}  node x={:.0} w={:.0} y={:.0} h={:.0}  run x={:.0} w={:.0}",
                    figure, node.rect.x, node.rect.w, node.rect.y, node.rect.h, run.x, run.w
                );
                for (k, leaf) in places.iter().enumerate() {
                    let mut up = leaf.parent;
                    let mut inside = false;
                    while let Some(a) = up {
                        if a == seat {
                            inside = true;
                            break;
                        }
                        up = places[a].parent;
                    }
                    if !inside || k == seat {
                        continue;
                    }
                    let leafy = !places.iter().any(|c| c.parent == Some(k));
                    if !leafy {
                        continue;
                    }
                    println!(
                        "TRIAGE       leaf {:?} x={:.0} w={:.0} y={:.0} h={:.0} vis={}  {}",
                        leaf.kind,
                        leaf.rect.x,
                        leaf.rect.w,
                        leaf.rect.y,
                        leaf.rect.h,
                        leaf.is_visible(),
                        leaf.id.rsplit('/').next().unwrap_or("")
                    );
                }
            }
        }
    }

    /// Is the figure `places[i]` would draw hidden under a surface painted
    /// over it?
    ///
    /// The same rule `paint.rs`'s `focused_caret_target` applies before it
    /// paints anything: a visible `Surface` with a higher `z`, outside this
    /// node's own lineage, overlapping where the figure would go, means the
    /// figure never reaches the screen. A gate that skipped this rule
    /// reports collisions the operator can never see — the catalog's own
    /// theme switcher, sitting under an open modal's scrim, against the
    /// modal covering it.
    fn caret_is_covered(
        places: &[gorgon_petra::frame::Placement],
        i: usize,
        marks: &[Rect],
    ) -> bool {
        let mut lineage = vec![i];
        let mut up = places[i].parent;
        while let Some(k) = up {
            lineage.push(k);
            up = places[k].parent;
        }
        places.iter().enumerate().any(|(k, s)| {
            !lineage.contains(&k)
                && s.kind == gorgon_petra::tree::NodeKind::Surface
                && s.z > places[i].z
                && s.is_visible()
                && marks.iter().any(|m| overlaps(*m, s.rect))
        })
    }

    /// The triggers a focus-figure gate presses before it measures.
    ///
    /// Shared by both gates so they cannot drift apart, and because what a
    /// gate never opens it never checks. The most tightly packed rows in the
    /// library — a menu's items, a dropdown's options, a calendar's day
    /// cells — do not exist in a shut frame at all.
    ///
    /// `"open-modal"` is here because it was missing, and the miss was not
    /// theoretical: the operator photographed the modal's Close button with
    /// its right-hand focus bar painted on the page behind the dialog, and
    /// the containment gate written to catch exactly that ran green,
    /// because row 20's dialog is shut at rest and the button it complained
    /// about was not in the frame being measured.
    const OPENERS: [&str; 5] = [
        "mn-pair/trigger",
        "dd/field",
        "sel/field",
        "full/field",
        "open-modal",
    ];

    /// Do two rects share any area?
    fn overlaps(a: Rect, b: Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
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

    /// Row 6, round 4. The operator: *"when clicking the copy text button it
    /// should give me some feed back that it copied"*.
    ///
    /// Carbon's answer is a tooltip reading `"Copied!"` that clears itself
    /// after `feedbackTimeout` milliseconds, 2000 by default
    /// (`@carbon/react/lib/components/Copy/Copy.js:30`), built from the
    /// tooltip caret and content mixins
    /// (`@carbon/styles/scss/components/copy-button/_copy-button.scss:44,50`).
    /// Carbon also swaps the button's own `aria-label` to that string while
    /// the bubble is up (`Copy.js:56,61`).
    ///
    /// **The feedback is a word, never a tint**, which is the operator's
    /// channel: he is red-green colourblind, so a copy control that answered
    /// with a fill step would answer him with nothing. Both channels are
    /// asserted here — the painted string and the accessible name — and so
    /// is the clearing, because feedback that never goes away stops being
    /// feedback the second time it is pressed.
    #[test]
    fn a_copy_says_copied_and_the_word_clears_on_carbons_two_second_timer() {
        let mut cam = Camera::on("Code snippet");
        cam.shoot("06-code-snippet-before-copy");
        assert!(
            !cam.has("copied"),
            "the page says Copied before anything was pressed"
        );
        assert_eq!(
            cam.label("snip/copy").as_deref(),
            Some("Copy"),
            "the resting control is named for what it does"
        );

        cam.click("snip/copy");
        // Photographed before anything is resolved, so the red run of this
        // test leaves the picture of a press that did nothing.
        let shot = raster(&mut cam, "06-code-snippet-copied");
        let bubble = cam.rect("snip/copy/copied");
        assert_eq!(
            cam.paint("snip/copy/copied/content/body")
                .text
                .as_ref()
                .map(|run| run.text.as_str()),
            Some("Copied!"),
            "the feedback bubble carries Carbon's own feedback string"
        );
        assert_eq!(
            cam.label("snip/copy").as_deref(),
            Some("Copied!"),
            "and so does the button's accessible name, as Carbon's does"
        );
        // The word is on the screen, not only in the frame record: a bubble
        // painted in one flat colour is a bubble with no text in it.
        let inks: std::collections::HashSet<[u8; 4]> =
            inset_pixels(&shot, bubble, 2).into_iter().collect();
        assert!(
            inks.len() > 2,
            "the feedback bubble rasterizes to {} colours, so nothing is \
             written in it",
            inks.len()
        );

        // Carbon's two seconds, counted in passes, plus a frame of slack.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let two_seconds = (2.0 / FRAME_SECONDS).round() as usize + 1;
        advance(&mut cam, two_seconds);
        cam.shoot("06-code-snippet-copy-cleared");
        assert!(
            !cam.has("copied"),
            "the feedback is still up two seconds later; Carbon clears it at \
             feedbackTimeout and a message that never leaves cannot report \
             the next press"
        );
        assert_eq!(
            cam.label("snip/copy").as_deref(),
            Some("Copy"),
            "and the accessible name goes back with it"
        );
    }

    /// The operator, round five: *"the code snippet, the one with the
    /// smallest box, should auto copy and paste when clicked and do a little
    /// reaction to show it happened like the rest."*
    ///
    /// The inline chip was Carbon's `hideCopyButton` branch written down as
    /// if it were the default. `CodeSnippet.js` renders `type="inline"` as a
    /// `Copy` — a real button — and this drives the whole of that: the press,
    /// the string leaving through `PlatformOutput` (not the frame, which is
    /// where a page-level assertion would pass with the host wiring missing),
    /// the same `Copied!` bubble the two wells raise, and the same two-second
    /// clear. *"Like the rest"* is the load-bearing half of the ask.
    #[test]
    fn pressing_the_inline_chip_copies_it_and_says_so_like_the_wells_do() {
        let mut cam = Camera::on("Code snippet");
        assert!(
            cam.clipboard().is_empty(),
            "nothing is copied before anything is pressed"
        );
        assert_eq!(
            cam.label("snip-in").as_deref(),
            Some("Copy to clipboard"),
            "the resting chip is named for what it does, which is the only \
             channel that says a chip with no glyph is pressable"
        );
        // Carbon's chip is `display: inline` — the width of the words in it.
        // It filled the whole card until `layout::grid` was taught to honour
        // `align_self`, and a card-wide copy control is a press that lands
        // three inches from the thing it names.
        let (chip, well) = (cam.rect("snip-in"), cam.rect("snip-multi"));
        assert!(
            chip.w < well.w / 2.0,
            "the chip is {} wide against a {}-wide well, so it is a block \
             and not a chip",
            chip.w,
            well.w
        );

        cam.click("snip-in");
        let shot = raster(&mut cam, "06-code-snippet-inline-copied");
        assert_eq!(
            cam.clipboard(),
            ["cargo xtask gates".to_owned()],
            "the chip copies its own line"
        );
        assert_eq!(
            cam.paint("snip-in/copied/content/body")
                .text
                .as_ref()
                .map(|run| run.text.as_str()),
            Some("Copied!"),
            "the chip answers with the same word the wells answer with"
        );
        assert_eq!(
            cam.label("snip-in").as_deref(),
            Some("Copied!"),
            "and its accessible name moves with the word, as Carbon's does"
        );
        let bubble = cam.rect("snip-in/copied");
        let inks: std::collections::HashSet<[u8; 4]> =
            inset_pixels(&shot, bubble, 2).into_iter().collect();
        assert!(
            inks.len() > 2,
            "the chip's bubble rasterizes to {} colours, so nothing is \
             written in it",
            inks.len()
        );

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let two_seconds = (2.0 / FRAME_SECONDS).round() as usize + 1;
        advance(&mut cam, two_seconds);
        assert!(
            !cam.has("copied"),
            "the chip's feedback is still up two seconds later"
        );
        assert_eq!(cam.label("snip-in").as_deref(), Some("Copy to clipboard"));
    }

    /// The operator, round five: *"a lot of these text elements are not
    /// highlightable, like the lists. A lot are like this."*
    ///
    /// A list item declares nothing — no role, no interaction, no selection
    /// ground. That is the whole point of this test: what makes it selectable
    /// is `gorgon_petra::input::hit_text`'s derived rule, and there is no line
    /// in `component/list.rs` that could be deleted to break it. Before this
    /// rule exactly one component in the library — the code snippet — had ever
    /// declared itself selectable.
    #[test]
    fn dragging_across_a_list_item_highlights_the_words_it_crossed() {
        let mut cam = Camera::on("List");
        assert_eq!(
            cam.selection("fix-0/label"),
            None,
            "nothing is selected before anything is dragged"
        );
        let resting = raster(&mut cam, "16-list-unselected");

        let run = cam.rect("fix-0/label");
        let mid = run.y + run.h / 2.0;
        cam.drag_at(
            Point::new(run.x + run.w * 0.25, mid),
            Point::new(run.x + run.w * 0.75, mid),
        );
        let shot = raster(&mut cam, "16-list-selected");

        let range = cam
            .selection("fix-0/label")
            .expect("a drag across a list item selected nothing at all");
        let painted = cam.painted_text("fix-0/label");
        let selected = painted[range.clone()].to_owned();
        assert!(
            !selected.is_empty() && selected.len() < painted.len(),
            "the drag selected {selected:?} of {painted:?}, which is not a \
             fragment"
        );

        // The band is on the screen, counted as a tone the run's rect did not
        // carry before — the same measurement the code well's selection is
        // held to, and for the same reason: a point sample lands on a glyph
        // as easily as on the ground.
        let (band, count) = new_tone(&resting, &shot, run);
        assert!(
            count > 100,
            "the drag added no new tone to the item's rect, so nothing was \
             painted behind the selected glyphs"
        );
        // `accent.primary` is `#0f62fe`: blue dominant, and far enough off
        // the page's greys that the operator reads it as a step even with no
        // red-green channel. Asserted as "blue leads the other two by a
        // margin no grey can fake" rather than as an exact triple, because
        // the band is composited over whatever the row's own ground is.
        assert!(
            u32::from(band[2]) > u32::from(band[0]) + 60
                && u32::from(band[2]) > u32::from(band[1]) + 40,
            "the selection band is {band:?}, which is not the accent fill: a \
             highlight told apart from the page by luminance alone is one \
             this operator cannot rely on"
        );
        // And it is a fragment, not the whole line.
        let (left, right) = tone_span(&shot, run, band);
        assert!(
            left > run.x + 2.0 && right < run.x + run.w - 2.0,
            "the highlight runs {left} to {right} across a run at {} to {}, \
             so it is the whole label rather than the stretch dragged over",
            run.x,
            run.x + run.w
        );

        // And the chord takes it. This is the half that makes the highlight
        // more than decoration, and until 2026-09-06 it existed on exactly
        // one page of forty-two: the chord lived in row 6's own
        // `Page::gesture`, so `Ctrl+C` over a selected list item copied
        // nothing. It is `Host::copy_selection`'s now, over state no
        // application owns.
        let before = cam.clipboard().len();
        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        );
        let copied = cam.clipboard();
        assert_eq!(
            copied.len(),
            before + 1,
            "the copy chord put nothing on the clipboard"
        );
        assert_eq!(
            copied.last().map(String::as_str),
            Some(selected.as_str()),
            "the chord copied something other than what is highlighted"
        );

        // The same band in light, measured against a light resting shot and
        // not against the dark one — the ground under it goes from near-black
        // to white, so a cross-theme difference would be mostly the theme.
        // The pair is legible by construction, `text.on-accent` being defined
        // as the ink for this fill, but a highlight that reads as a step in
        // one theme and a smudge in the other is the defect this catalog
        // keeps finding, so it is looked at rather than argued.
        cam.light();
        // A press on nothing drops the selection, which is the rule
        // `hit_text` states and the way to get a clean resting shot.
        press_empty_ground(&mut cam);
        assert_eq!(
            cam.selection("fix-0/label"),
            None,
            "a press on empty ground left the highlight up"
        );
        let resting = raster(&mut cam, "16-list-unselected-light");
        cam.drag_at(
            Point::new(run.x + run.w * 0.25, mid),
            Point::new(run.x + run.w * 0.75, mid),
        );
        let lit = raster(&mut cam, "16-list-selected-light");
        let (band, count) = new_tone(&resting, &lit, run);
        assert!(
            count > 100,
            "the light theme painted no band behind the selected glyphs"
        );
        assert!(
            u32::from(band[2]) > u32::from(band[0]) + 60
                && u32::from(band[2]) > u32::from(band[1]) + 40,
            "the light band is {band:?} rather than the accent fill"
        );
    }

    /// A table cell lends out its text and its row still takes the press.
    ///
    /// The operator, after the first cut of the selection rule refused
    /// anything a control covered: *"it should be opt out not opt in"*. A
    /// data table row claims its press — that is how a row gets selected —
    /// and its cells carry the content a person actually wants to copy, so
    /// the row does not declare `owning_its_text` and a button does.
    ///
    /// **Both halves are asserted here**, because the doubt a reader has is
    /// whether one gesture can do two things. It can: routing is untouched,
    /// so the row hears the press exactly as before, and the selection
    /// anchors alongside it.
    #[test]
    fn dragging_across_a_table_cell_highlights_it_and_the_row_still_takes_the_press() {
        let mut cam = Camera::on("Data table");
        let selected = |cam: &Camera| {
            cam.frame()
                .placements
                .iter()
                .filter(|p| p.id.ends_with("/dt-0"))
                .map(|p| p.semantics.selected)
                .next()
                .expect("the first data table row")
        };
        // The page opens with its *second* row selected, so the first is the
        // one whose flag a press has somewhere to move.
        assert!(!selected(&cam), "the row starts unselected");
        let resting = raster(&mut cam, "09-data-table-unselected");

        let run = cam.rect("dt-0/c0/name");
        let mid = run.y + run.h / 2.0;
        cam.drag_at(
            Point::new(run.x + run.w * 0.25, mid),
            Point::new(run.x + run.w * 0.75, mid),
        );
        let shot = raster(&mut cam, "09-data-table-cell-selected");

        let range = cam
            .selection("dt-0/c0/name")
            .expect("a drag across a table cell selected nothing at all");
        let painted = cam.painted_text("dt-0/c0/name");
        assert!(
            !painted[range.clone()].is_empty(),
            "the drag lit an empty range of {painted:?}"
        );
        let (band, count) = new_tone(&resting, &shot, run);
        assert!(count > 40, "no band was painted behind the selected glyphs");
        assert!(
            u32::from(band[2]) > u32::from(band[0]) + 60,
            "the band is {band:?} rather than the accent fill"
        );
        assert!(
            selected(&cam),
            "the row did not hear the press that started the selection, so \
             lending out its text cost it its own gesture"
        );
    }

    /// A tile lends out its prose, which is the one `Role::Button` in the
    /// library that does.
    ///
    /// Operator decision, 2026-09-06, taken after the opt-out landed on all
    /// 45 press-acting controls at once. A tile is a content surface that
    /// happens to be pressable — Carbon's clickable tile is an `<a>` — and
    /// `component/tile.rs`'s own doc had already named the thing that decides
    /// it: *"`body` is the visible text ... whose content is not always its
    /// name."*
    ///
    /// **The price is asserted here rather than left in a comment.** A press
    /// anywhere on an interactive tile still fires it, so the drag that lit
    /// this highlight also toggled the tile beside it. That is what he chose;
    /// this test is where a reader finds out.
    #[test]
    fn a_tile_lends_out_its_prose_and_the_press_still_fires() {
        let mut cam = Camera::on("Tile");
        let ticked = |cam: &Camera| {
            cam.frame()
                .placements
                .iter()
                .find(|p| p.id.ends_with("/tile-sel"))
                .map(|p| p.semantics.selected)
                .expect("the selectable tile")
        };
        assert!(!ticked(&cam), "the selectable tile starts unselected");
        let resting = raster(&mut cam, "35-tile-unselected");

        // The clickable tile's body: prose that is not the control's name,
        // which is the whole of the argument for lending it out.
        //
        // Absolute offsets and not a fraction of the rect. A tile's body runs
        // the full width of the tile — 812 here — and its sentence ends
        // around 170, so a 20%-to-60% drag starts on the last full stop and
        // ends in empty space. The first cut of this test did exactly that,
        // lit one character, and passed: the picture is what caught it.
        let run = cam.rect("tile-click/body");
        let mid = run.y + run.h / 2.0;
        cam.drag_at(
            Point::new(run.x + 10.0, mid),
            Point::new(run.x + 120.0, mid),
        );
        let shot = raster(&mut cam, "35-tile-prose-selected");
        let range = cam
            .selection("tile-click/body")
            .expect("a drag across a tile's body selected nothing at all");
        let painted = cam.painted_text("tile-click/body");
        let selected = painted[range].to_owned();
        assert!(
            selected.len() > 4 && selected.len() < painted.len(),
            "the drag lit {selected:?} of {painted:?}, which is not a \
             fragment a reader would call a selection"
        );
        // Counted as accent-coloured pixels rather than as `new_tone`'s
        // busiest new tone: the drag leaves the pointer inside the tile, so
        // the tile's own `layer-hover` fill is new across the whole rect and
        // outvotes the band by an order of magnitude. Asking the narrower
        // question — how many pixels here are the *accent* — is the honest
        // measurement when a second fill changed at the same time.
        let accent = |img: &image::RgbaImage| {
            inset_pixels(img, run, 0)
                .into_iter()
                .filter(|p| {
                    u32::from(p[2]) > u32::from(p[0]) + 60 && u32::from(p[2]) > u32::from(p[1]) + 40
                })
                .count()
        };
        let (before, after) = (accent(&resting), accent(&shot));
        assert!(
            before == 0 && after > 40,
            "the run carried {before} accent pixels before the drag and \
             {after} after, which is not a band appearing behind the \
             selected glyphs"
        );

        // And the cost, on the tile that can show it: the same gesture the
        // selectable tile hears is still a press.
        // Absolute offsets, not a fraction of the rect: this label is
        // stretched to the tile's full 812 and its four words end around 120,
        // so a drag at 20%-60% of the rect lands entirely past the last glyph
        // and collapses on the final byte.
        let sel = cam.rect("tile-sel/row/label");
        cam.drag_at(
            Point::new(sel.x + 8.0, sel.y + sel.h / 2.0),
            Point::new(sel.x + 70.0, sel.y + sel.h / 2.0),
        );
        assert!(
            cam.selection("tile-sel/row/label").is_some(),
            "the selectable tile's own label refused a selection"
        );
        assert!(
            ticked(&cam),
            "the tile did not hear the press that started the selection"
        );
    }

    /// The other half of the rule, and the one that would have broken the
    /// library if it were missing: a button's label is a `Text` placement
    /// that paints *above* its own button, so a naive "every run is
    /// selectable" would have handed every press in the catalog to a label
    /// and left every button inert.
    #[test]
    fn dragging_across_a_buttons_label_selects_nothing_and_the_button_still_hears_it() {
        let mut cam = Camera::on("Button");
        let run = cam.rect("btn-primary-label");
        let mid = run.y + run.h / 2.0;
        cam.drag_at(
            Point::new(run.x + run.w * 0.25, mid),
            Point::new(run.x + run.w * 0.75, mid),
        );
        assert_eq!(
            cam.selection("btn-primary-label"),
            None,
            "a drag across a button's label selected it, so the label has \
             taken a press its own button needed"
        );
    }

    /// Row 6, round 4. The operator: *"code snippet: I cant highlight text
    /// inside the code snippet blocks"*.
    ///
    /// Carbon's snippet container is a read-only `textbox` the browser lets
    /// you drag across (`CodeSnippet.js:120-124`). Petra has no browser, so
    /// the gesture is built: the press anchors a byte offset, the drag moves
    /// the other end, the frame carries the range, the painter fills behind
    /// those glyphs, and the copy chord takes exactly those bytes.
    ///
    /// **All four are asserted here, and the last one is the point.** A
    /// highlight that does not copy is decoration, and a copy that takes the
    /// whole block while a fragment is lit is a lie. The copied string is
    /// compared against the substring the frame says is selected, sliced out
    /// of the string the frame says is painted — the same two numbers the
    /// painter used — so the picture and the clipboard cannot disagree
    /// without this going red.
    #[test]
    fn dragging_across_the_code_selects_those_bytes_and_the_copy_chord_takes_them() {
        let mut cam = Camera::on("Code snippet");
        assert_eq!(
            cam.selection("snip/code"),
            None,
            "nothing is selected before anything is dragged"
        );
        let resting = raster(&mut cam, "06-code-snippet-unselected");

        // Across the middle of the single-line run, from a quarter of the way
        // along to three quarters, so the selection is a genuine fragment
        // with unselected code on both sides of it.
        let run = cam.rect("snip/code");
        let mid = run.y + run.h / 2.0;
        cam.drag_at(
            Point::new(run.x + run.w * 0.25, mid),
            Point::new(run.x + run.w * 0.75, mid),
        );
        let shot = raster(&mut cam, "06-code-snippet-selected");

        let range = cam
            .selection("snip/code")
            .expect("a drag across the code selected nothing at all");
        let painted = cam.painted_text("snip/code");
        let selected = painted[range.clone()].to_owned();
        assert!(
            !selected.is_empty() && selected.len() < painted.len(),
            "the drag selected {selected:?} of {painted:?}, which is not a \
             fragment: a quarter-to-three-quarters drag must take some of \
             the line and leave some of it"
        );

        // The highlight is on the screen, measured as a tone the run's own
        // rect did not carry before the drag. Counting tones rather than
        // sampling a point: a point sample lands on a glyph as easily as on
        // the ground, and "this pixel changed" is as true of a moved caret
        // as of a painted band.
        let (band, count) = new_tone(&resting, &shot, run);
        assert!(
            count > 200,
            "the drag added no new tone to the run's rect at all, so nothing \
             was painted behind the selected glyphs"
        );
        // Lighter than the fill it sits on, which is the operator's channel:
        // he is red-green colourblind, so a highlight separated by hue alone
        // is one he does not receive. `layer-selected` is the design system's
        // own selected step, seven L* off this well's ground.
        //
        // Sampled inside the **well** and past the end of the code, which is
        // the only place on this row that is certainly the well's own fill.
        // Above the run is the card the well sits on, a different grey, and
        // reading it here is how the first cut of this assertion compared the
        // highlight against the wrong ground and called a real band too dim.
        let well = cam.rect("code/snip");
        let fill = px(&shot, well.x + well.w - 8.0, mid);
        assert!(
            pixel_luma(band) > pixel_luma(fill) + 8,
            "the highlight is {} against a {} ground, which is not a \
             luminance step a reader can use: {band:?} on {fill:?}",
            pixel_luma(band),
            pixel_luma(fill)
        );
        // And it is a band, not the whole run: unselected code is left on
        // both sides of it.
        let (left, right) = tone_span(&shot, run, band);
        assert!(
            left > run.x + 2.0 && right < run.x + run.w - 2.0,
            "the highlight runs {left} to {right} across a run at {} to {}, \
             so it is the whole line rather than the fragment that was \
             dragged over",
            run.x,
            run.x + run.w
        );

        // And the chord copies exactly those bytes, not the whole line.
        let before = cam.clipboard().len();
        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        );
        let copied = cam.clipboard();
        assert_eq!(
            copied.len(),
            before + 1,
            "the copy chord put nothing on the clipboard"
        );
        assert_eq!(
            copied.last().map(String::as_str),
            Some(selected.as_str()),
            "the chord copied something other than what is highlighted"
        );
        assert_ne!(
            copied.last().map(String::as_str),
            Some(painted.as_str()),
            "the chord took the whole well while a fragment was selected, \
             which is the copy that pretends to be a selection"
        );
    }

    /// A drag down the multi-line well selects across the line break, lights
    /// both rows, and repaints the selected comment in the selection's ink.
    ///
    /// Three claims the single-line case cannot make.
    ///
    /// **One rectangle per row.** A selection that crossed a newline and
    /// painted one box from the first character to the last would cover the
    /// whole right-hand margin of the first line and the whole left margin of
    /// the second. The bands here are measured on two separate rows.
    ///
    /// **The line break is in the copied bytes.** A selection whose range
    /// stopped at the end of a row would copy one line and look like two.
    ///
    /// **The selected comment changes ink.** The comment class is
    /// `text.muted` and the selection's ink is `text.primary`, so the same
    /// words are brighter inside the highlight than outside it. That is the
    /// half of `::selection` that keeps a coloured run legible on a ground it
    /// was never measured against — the keyword class clears AA on this well
    /// by 0.13 in the light theme, so a highlight that left the ink alone
    /// would sink it.
    #[test]
    fn a_drag_down_the_multi_line_well_lights_both_rows_and_relights_the_ink() {
        let mut cam = Camera::on("Code snippet");
        let run = cam.rect("snip-multi/code");
        let lines = cam.painted_text("snip-multi/code").lines().count();
        assert!(lines > 10, "the multi-line sample is {lines} lines");
        #[allow(clippy::cast_precision_loss)]
        let line_h = run.h / lines as f32;
        let row_mid = |n: usize| {
            #[allow(clippy::cast_precision_loss)]
            let n = n as f32;
            run.y + line_h * (n + 0.5)
        };

        let resting = raster(&mut cam, "06-code-snippet-multi-unselected");
        // From a third of the way along the comment on row 0, down to two
        // thirds along the command on row 1.
        let press_x = run.x + run.w * 0.20;
        cam.drag_at(
            Point::new(press_x, row_mid(0)),
            Point::new(run.x + run.w * 0.45, row_mid(1)),
        );
        let shot = raster(&mut cam, "06-code-snippet-multi-selected");

        let range = cam
            .selection("snip-multi/code")
            .expect("the drag down the well selected nothing");
        let painted = cam.painted_text("snip-multi/code");
        let selected = &painted[range];
        assert!(
            selected.contains('\n'),
            "a drag from one row to the next selected {selected:?}, which \
             holds no line break, so the range stopped at the end of a row"
        );

        // A band on each row, each ending short of the run's right edge —
        // one box spanning both rows would run the full width of the first.
        let (band, _) = new_tone(&resting, &shot, run);
        for row in [0usize, 1] {
            let strip = Rect::new(run.x, row_mid(row) - 1.0, run.w, 2.0);
            let (left, right) = tone_span(&shot, strip, band);
            assert!(
                left.is_finite() && right < run.x + run.w - 2.0,
                "row {row} carries no highlight band inside the run, or it \
                 runs the whole width: {left} to {right} of {} to {}",
                run.x,
                run.x + run.w
            );
        }

        // The comment on row 0 is brighter inside the selection than outside
        // it, which is the ink swap and nothing else: same words, same size,
        // same face.
        let strip =
            |x0: f32, x1: f32| Rect::new(x0, row_mid(0) - line_h * 0.4, x1 - x0, line_h * 0.8);
        let outside = brightest(&shot, strip(run.x + 1.0, press_x - 1.0));
        let inside = brightest(&shot, strip(press_x + 2.0, run.x + run.w * 0.6));
        // Twenty, because the two inks are thirty apart and not a hundred:
        // dark `text.muted` is `#d4d4d4` and `text.primary` is `#f2f2f2`, so
        // the swap this proves is a real but narrow one. Without it both
        // readings are the same number and the margin is zero.
        assert!(
            inside > outside + 20,
            "the selected comment peaks at {inside} and the unselected part \
             of the same line at {outside}; the selection is not repainting \
             the run in its own ink"
        );
    }

    /// A press somewhere else drops the selection, and a bare hover across a
    /// block does not make one.
    ///
    /// Both are what every text surface does, and both are the ways a
    /// selection built on a raw pointer stream goes wrong: a highlight that
    /// outlives the block it was made in, and one that follows the mouse
    /// around with no button held.
    #[test]
    fn a_selection_needs_a_held_button_and_does_not_outlive_a_press_elsewhere() {
        let mut cam = Camera::on("Code snippet");
        let run = cam.rect("snip/code");
        let mid = run.y + run.h / 2.0;
        cam.drag_at(
            Point::new(run.x + run.w * 0.2, mid),
            Point::new(run.x + run.w * 0.8, mid),
        );
        let held = cam.selection("snip/code").expect("the drag selected");

        cam.hover_at(run.x + run.w * 0.4, mid);
        assert_eq!(
            cam.selection("snip/code"),
            Some(held),
            "a hover with no button held rewrote the selection"
        );

        cam.click("snip/copy");
        assert_eq!(
            cam.selection("snip/code"),
            None,
            "a press on another control left the old highlight lit"
        );
    }

    /// The multi-line well answers a press the same way, from its own button.
    ///
    /// Both wells name their control `copy`, and the first cut of this
    /// feedback hung the bubble off the snippet rather than off the button
    /// that was pressed — which reads correct on a page with one well and
    /// wrong on this one.
    #[test]
    fn the_multi_line_well_gets_its_own_copied_bubble_and_not_the_other_ones() {
        let mut cam = Camera::on("Code snippet");
        cam.click("snip-multi/copy-row/copy");
        cam.shoot("06-code-snippet-multi-copied");
        assert!(
            cam.has("snip-multi/copy-row/copy/copied"),
            "the well that was pressed says Copied"
        );
        assert!(
            !cam.has("snip/copy/copied"),
            "and the well that was not pressed says nothing"
        );
        assert_eq!(cam.label("snip/copy").as_deref(), Some("Copy"));
    }
    // ---- The focus caret on field-shaped controls (round 3, wave F1) ----
    //
    // Every test below starts from a `Camera::click`, because no person has
    // an `Action::Focus`, and reads the raster back: the caret is host-owned
    // geometry that no frame-level assertion can see.

    /// What lies under a bracketed well, which decides whether the ground
    /// below its feet can be measured at all.
    ///
    /// A `Sides` bar stands beside the well and casts down onto whatever is
    /// there. On most pages that is open card, and the two feet's shadows
    /// mirror each other exactly. On a page whose control opens a list or a
    /// bubble *under* the field, the card is covered by that surface's own
    /// `shadow.overlay`, which follows the well's footprint and not the
    /// bar's: the inward side of each foot sits on it and the outward side
    /// does not. Measured on `dd/field`, both feet read 31, 31, 32, 33 out
    /// from the bar and then part, the inward side holding 32 while the
    /// outward rises to the page's 34.
    ///
    /// That is the neighbour, not the bar, so [`Foot::UnderSurface`] skips
    /// the symmetry probe rather than weakening it for every page.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Foot {
        /// Open card below the well. The shadow's symmetry is measurable.
        OnCard,
        /// Another surface hangs under the well and darkens the ground the
        /// probe would read.
        UnderSurface,
    }

    /// The two side bars the settled indicator draws around the well keyed
    /// `well_tail`, read back from the raster, or a panic naming which part
    /// of the figure is wrong.
    ///
    /// Asserts, from `FocusRing::STANDARD`'s own geometry: a bar of one
    /// accent colour `hug_gap` outside each side; each bar exactly the
    /// well's height, top and bottom; the gap between bar and well left as
    /// ground; and, under each foot, a shadow rather than a bleed. Neutral,
    /// darker than the card 11 units further down, and nowhere near the
    /// accent. Returns the bar colour so a caller can look for it elsewhere.
    fn assert_hugs_well(cam: &mut Camera, well_tail: &str, shot: &str, foot: Foot) -> [u8; 4] {
        let ring = FocusRing::STANDARD;
        let well = cam.rect(well_tail);
        let img = raster(cam, shot);
        let [left, right] = ring.sides(well);
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
        // Device columns from logical units. `px` truncates, but a band's
        // edges must round: three logical units is six device columns, and a
        // truncated edge reads one column short on one side and not on the
        // other.
        let edge = |v: f32| (v * CAPTURE_SCALE).round() as i64;
        // One mirrored pair of device columns, read on every row from the
        // first below the accent to the sixth, so none of the four rows the
        // seam occupied is stepped over.
        let mirrored_below_the_foot = |l: i64, r: i64, what: &str| {
            for step in 1..=6 {
                let dy = ((well.y + well.h) * CAPTURE_SCALE) as u32 + step;
                assert_eq!(
                    img.get_pixel(l as u32, dy).0,
                    img.get_pixel(r as u32, dy).0,
                    "{well_tail}: {what}, at device columns {l} and {r}, \
                     {step} row(s) below the foot: a tessellation seam"
                );
            }
        };
        for (side, x, outward, band) in [("left", lx, -1.0, left), ("right", rx, 1.0, right)] {
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
            // What lies under the foot is the bar's **shadow**, and this
            // pair of samples is what tells a shadow from a bleed.
            //
            // Until 2026-09-06 the line here read `assert_eq!(under,
            // beside)`, no darkening at all past the rule, and it enforced a
            // policy the operator has since reversed: *"not all the cursors
            // have the same shadow. The underbar (original one) has it
            // right."* Every bar casts one now, so the honest property is
            // not "no shadow" but "a shadow and not a bar": a neutral
            // darkening that fades out with distance, never the accent.
            //
            // `far` is sampled past the shadow's reach, offset 2 plus blur 6
            // is 8, and inside `indicator_outset`'s 13, so it is ground the
            // caret was already allowed to paint over and did not.
            let far = px(&img, x, well.y + well.h + 11.0);
            assert!(
                under[2] <= bar[2] / 2,
                "{well_tail}: under the {side} foot is the accent bleeding \
                 down, not a shadow: {under:?} against a bar of {bar:?}"
            );
            assert!(
                under.iter().zip(far).all(|(u, f)| *u <= f) && under[..3] != far[..3],
                "{well_tail}: the {side} bar's foot casts no shadow on the \
                 card: {under:?} at 1.5 below the rule against {far:?} at 11"
            );
            // And that shadow is symmetric about the bar it belongs to.
            //
            // The operator's report of 2026-09-06: *"the vertical shadow has
            // a visual bug."* `epaint::Shadow` clamps a blur to the caster's
            // short side and then adds half of it back as a corner radius,
            // so on a 3-unit bar the two corner arcs nearly meet and their
            // feather skirts overlap. The shadow blends twice along one
            // corner's 45-degree bisector and leaves a four-pixel diagonal
            // darker than the rest of the shadow. As long as a `Sides` bar
            // is broad, which is why it read as a hook hanging off the foot.
            // `crate::shadow::box_shadow` draws the mesh instead and is
            // symmetric by construction; this states that fix where the
            // operator met it, in pixels on seven pages, rather than only in
            // the mesh's own unit test.
            //
            // **Mirrored in device columns, not logical units.** `px`
            // truncates a logical coordinate into a device one, and the bar's
            // centre falls on a device-pixel *boundary*. Probing `x - d`
            // against `x + d` therefore compares columns one apart and fails
            // by a level or two on a gradient, which is what the first
            // version of this check did on all seven pages. The true mirror
            // of the column `k` left of the bar is the column `k` right of
            // it.
            //
            // **This pass runs inside the bar's own columns and no wider.**
            // Most of the seam lived *within* the bar's six device columns,
            // where the accent hides it until the fill ends and the shadow
            // is left exposed; a check that only mirrored the ground beside
            // the bar saw three of the four bad rows as clean. The ground
            // beside the bar is checked as well, but not against this bar:
            // the left-against-right pass after this loop does that, and the
            // comment there says why it cannot be done here.
            if foot == Foot::OnCard {
                let (bx0, bx1) = (edge(band.x), edge(band.x + band.w));
                for i in 0..(bx1 - bx0) / 2 {
                    mirrored_below_the_foot(
                        bx0 + i,
                        bx1 - 1 - i,
                        &format!("the {side} bar's own shadow is lopsided"),
                    );
                }
            }
        }
        // The ground beside the bars, left figure against right figure.
        //
        // A bar is not mirrored by its own surroundings and never was. Card
        // lies outside it; `hug_gap` of gap and then the well lie inside it.
        // A pass that mirrors one bar against itself past its own columns is
        // therefore comparing card against gap, and it passed until
        // 2026-09-09 only because a well carrying a `corner-md` radius held
        // its own shadow back out of that gap. The corner-radius flag day
        // squared the menu trigger, the well's shadow moved one device
        // column further in, and the fourth column out parted by a single
        // level: 32 on the card against 31 in the gap, with the bar itself
        // blameless.
        //
        // What *is* mirrored is the whole figure. `sides()` places two bars
        // about a well, and the well's own shadow is symmetric about the
        // well's own centre, so the honest partner of the k-th column
        // outward of the left bar is the k-th column outward of the right
        // bar, and likewise inward. Card then meets card and gap meets gap,
        // the neighbour contributes the same darkening to both, and a seam
        // on one corner of one bar still parts its pair.
        //
        // `hug_gap` device columns each way, because that is how much gap
        // the figure itself leaves before an inward probe would land on the
        // well. No distance is calibrated here.
        //
        // Falsified before it was kept. Expanding only the first bar's
        // shadow caster by half a unit, symmetric about that bar's own
        // centre, leaves the self-mirror above green and lands here:
        //
        // ```text
        // mn-pair/trigger: the card outside the two bars is not the same
        // ground, at device columns 569 and 758, 1 row(s) below the foot: a
        // tessellation seam
        //   left: [29, 29, 29, 255]
        //  right: [30, 30, 30, 255]
        // ```
        if foot == Foot::OnCard {
            let (lx0, lx1) = (edge(left.x), edge(left.x + left.w));
            let (rx0, rx1) = (edge(right.x), edge(right.x + right.w));
            for k in 0..edge(ring.hug_gap) {
                mirrored_below_the_foot(
                    lx0 - 1 - k,
                    rx1 + k,
                    "the card outside the two bars is not the same ground",
                );
                mirrored_below_the_foot(
                    lx1 + k,
                    rx0 - 1 - k,
                    "the gap inside the two bars is not the same ground",
                );
            }
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
        assert_hugs_well(&mut cam, "/query", "28-search-focused", Foot::OnCard);
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
        assert_hugs_well(&mut cam, "/n-md", "22-number-input-focused", Foot::OnCard);
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
        let bar = assert_hugs_well(
            &mut cam,
            "theme/field",
            "29-select-open-focused",
            Foot::UnderSurface,
        );
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
        let bar = assert_hugs_well(
            &mut cam,
            "dd/field",
            "11-dropdown-open-focused",
            Foot::UnderSurface,
        );
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
        cam.click("compact/field");
        assert!(
            cam.has("compact/calendar"),
            "the click did not open the calendar"
        );
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("compact/field")),
            "focus flew away from the field when the calendar opened: {:?}",
            cam.focused()
        );
        assert!(
            cam.ring()
                .as_deref()
                .is_some_and(|id| id.ends_with("compact/field")),
            "the frame rings something other than the field: {:?}",
            cam.ring()
        );
        let field = cam.rect("compact/field");
        let bar = assert_hugs_well(
            &mut cam,
            "compact/field",
            "10-date-picker-open-focused",
            Foot::UnderSurface,
        );
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
        // Open at rest since 2026-09-05: shut it, then open it, so the
        // bracket under test is the one a press puts there.
        cam.click("tt/trigger");
        cam.click("tt/trigger");
        assert!(cam.has("tt/tip"), "the click did not open the tip");
        let trigger = cam.rect("tt/trigger");
        let bar = assert_hugs_well(
            &mut cam,
            "tt/trigger",
            "37-toggletip-open-focused",
            Foot::UnderSurface,
        );
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

    /// Row 18. Two figures on one gesture: the trigger is a button, so it is
    /// bracketed (`FocusFigure::Sides`), and the menu items it opens are
    /// packed rows, so they are ringed (`FocusFigure::Border`). Opening the
    /// menu hands keyboard focus to the first item
    /// (`../../.agents/notes/implemented/bug-fix/2026-09-05-an-open-menu-takes-keyboard-focus.md`),
    /// so the indicator both **moves** and **changes shape**.
    ///
    /// A menu trigger is the sharpest case for the operator's rule that a
    /// bar under is the default and a box is for where a bar would not fit.
    /// This test twice asserted something else, and both times it was right
    /// at the time. First: the trigger underlined, the bar hung two units
    /// below it, an open menu sits on exactly those two units, and the
    /// indicator was **withheld** — no focus indicator anywhere on the page.
    /// Then: a ring on the trigger, which fixed that by containment.
    /// `Sides` fixes it the same way and keeps the trigger reading as a
    /// control you press rather than a row in a list — the bars stand left
    /// and right, and the menu opens downward past them.
    /// `paint.rs`'s `a_caret_crossing_a_surface_above_its_node_is_withheld`
    /// still holds the withholding rule for `BarUnder`.
    ///
    /// # How this goes red
    ///
    /// Give `button` `FocusFigure::Border` and the trigger grows an accent
    /// band on its own top edge, which the second assertion refuses; give it
    /// `BarUnder` and `assert_hugs_well` finds no bar beside it at all.
    #[test]
    fn a_menu_trigger_is_bracketed_and_the_mark_moves_into_the_menu() {
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
        let ring = FocusRing::STANDARD;
        // The whole `Sides` figure, checked band by band: a bar of one
        // accent `hug_gap` outside each edge, exactly the trigger's height,
        // with clean ground in the gap and under each foot.
        let accent = assert_hugs_well(
            &mut cam,
            "mn-pair/trigger",
            "18-menu-shut-focused",
            Foot::OnCard,
        );
        let trigger = cam.rect("mn-pair/trigger");
        let mid_x = trigger.x + trigger.w / 2.0;
        // The middle of where a ring's accent band would be, on the
        // trigger's own top edge. A bracketed control must leave it alone.
        let band_y = trigger.y + ring.stroke / 2.0;
        let shut = raster(&mut cam, "18-menu-shut-focused");
        assert_ne!(
            px(&shut, mid_x, band_y),
            accent,
            "a button is bracketed, not ringed: nothing may paint the accent \
             on the trigger's own top edge"
        );

        cam.click("mn-pair/trigger");
        assert!(
            cam.has("mn-pair/menu"),
            "the third click did not reopen the menu"
        );
        // Photograph first, then read focus back. The surface's `takes_focus`
        // is seated by the host on the pass *after* the menu appears, so a
        // `ring()` read taken straight off the click still names the trigger;
        // the shooting pass is the one that moves it.
        let open = raster(&mut cam, "18-menu-open-focused");
        assert!(
            cam.ring().is_some_and(|id| id.ends_with("mn-0")),
            "an open menu takes keyboard focus onto its first item; focus is \
             {:?}",
            cam.ring()
        );
        // A menu item wears `BarInside` since 2026-09-06 — items stack
        // flush, so its stripe sits on its own bottom edge rather than five
        // units below it, on the next item. Read the middle of that stripe.
        // Probing the item's *top* edge, where a ring's band used to be,
        // now reads the item's own fill and says nothing.
        let stripe = ring.bar_inside(marked_run(cam.frame(), "mn-0"));
        let on_item = px(&open, stripe.x + stripe.w / 2.0, stripe.y + stripe.h / 2.0);
        assert_eq!(
            on_item, accent,
            "the menu's first item did not take its focus stripe in the same \
             accent the trigger's brackets were drawn in"
        );
        let [left, _] = ring.sides(trigger);
        assert_ne!(
            px(&open, left.x + left.w / 2.0, trigger.y + trigger.h / 2.0),
            accent,
            "the trigger still wears its brackets while the menu holds focus"
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
        assert_hugs_well(
            &mut cam,
            "field-md",
            "34-text-input-md-focused",
            Foot::OnCard,
        );
    }

    // ======================================================================
    // Wave B, round 3 — rows 31 (Structured list), 09 (Data table) and
    // 13 (Form). One contiguous block: three waves edit this file in three
    // worktrees and the merge is done by hand.
    // ======================================================================

    /// The text a node keyed `key` in the open page's tree carries — its
    /// `props.text`, which is where `valued` writes a field's value.
    fn text_of(cam: &Camera, key: &str) -> String {
        crate::page::common::find(&cam.tree(), key)
            .unwrap_or_else(|| panic!("no node keyed {key:?} in the page tree"))
            .props
            .text
            .clone()
            .unwrap_or_default()
    }

    /// Row 31, round 3: *"we should probably let the column spacers always
    /// we draggable."*
    ///
    /// Driven the way a hand drives it — press on the divider, move, let go
    /// — and read off the **placed column rects**, not off the page's own
    /// weights: a weight that changes while the columns do not is exactly
    /// the class of green test this catalog shipped a round of.
    #[test]
    fn dragging_a_structured_list_divider_moves_the_column_boundary() {
        let mut cam = Camera::on("Structured list");
        let before = cam.shoot("31-structured-list-before-drag");
        let first = cam.rect("/sl/header/c0");
        let second = cam.rect("/sl/header/c1");
        let divider = cam.rect("/sl/header/div0");
        assert!(
            (divider.w - 8.0).abs() < 0.01,
            "the divider's drag target is {} wide, not the 8 that makes it \
             a thing a hand can hit",
            divider.w
        );

        // Drag the boundary a quarter of the pair's width to the left.
        let travel = second.right() - first.x - divider.w;
        let target = first.x + travel * 0.25 + divider.w / 2.0;
        cam.drag("/sl/header/div0", Point::new(target, divider.y + 2.0));
        let after = cam.shoot("31-structured-list-column-dragged");

        let moved = cam.rect("/sl/header/c0");
        assert!(
            moved.w < first.w - 10.0,
            "the first column went from {} to {}, so the drag did not reach \
             the columns",
            first.w,
            moved.w
        );
        assert!(
            (moved.w - travel * 0.25).abs() < 2.0,
            "the boundary landed at {} where the pointer asked for {}",
            moved.w,
            travel * 0.25
        );
        // Every row's column 0 follows, not just the header's: the grid
        // tracks are what keep a table's columns one width.
        for row in ["sl-0", "sl-1"] {
            assert!(
                (cam.rect(&format!("/{row}/c0")).w - moved.w).abs() < 0.01,
                "{row}'s first column did not follow the header's"
            );
        }
        assert_ne!(before, after, "the drag never reached the picture");
    }

    /// The round-2 slider bug, asserted away on the new control before it
    /// can happen again: **a pointer that merely passes a divider does not
    /// resize anything.** Only a press opens the gesture.
    ///
    /// Two shipped decisions hold that, and this covers both. The divider
    /// declares no `Interaction::Hover`, so a button-up move never names it
    /// at all; and the page's `gesture` ignores a positional event with no
    /// drag open. The sweep deliberately stops on the divider's **leading
    /// edge** rather than its middle: a hover in the middle would compute
    /// the boundary it is already at, so a test that hovered there would go
    /// green whether the guard was there or not. Falsified 2026-09-05 by
    /// adding `Hover` to the divider and taking the guard out; it moves the
    /// column by half the target's width.
    #[test]
    fn a_pointer_passing_a_structured_list_divider_moves_no_column() {
        let mut cam = Camera::on("Structured list");
        let before = cam.rect("/sl/header/c0");
        let divider = cam.rect("/sl/header/div0");
        // Straight across the divider, left to right, button up.
        cam.hover_at(divider.x - 40.0, divider.y + 2.0);
        cam.hover_at(divider.x + 0.5, divider.y + 2.0);
        cam.hover_at(divider.right() - 0.5, divider.y + 2.0);
        cam.hover_at(divider.x + 40.0, divider.y + 2.0);
        assert_eq!(
            cam.rect("/sl/header/c0").w,
            before.w,
            "the pointer passing the divider resized a column, which is the \
             round-2 slider bug wearing a different hat"
        );
    }

    /// Row 9, round 3: *"neither it or the other table like structure is
    /// editable."*
    ///
    /// Carbon v11 core ships no inline-edit anatomy; what slice-b:69 does
    /// sanction is a form control placed in a cell, so the Kind column is a
    /// `field_sm` well per row. Driven the way a hand drives it: **click,
    /// then type**. `Camera::type_into` would move focus with an action no
    /// person has, which is how a whole round of green tests shipped over a
    /// field nobody could click into.
    #[test]
    fn clicking_a_data_table_cell_and_typing_changes_the_value() {
        let mut cam = Camera::on("Data table");
        let before = cam.shoot("09-data-table-before-typing");
        assert_eq!(text_of(&cam, "dt-kind-0"), "runtime");

        cam.click("dt-kind-0");
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("dt-kind-0")),
            "the click did not seat the caret in the cell: {:?}",
            cam.focused()
        );
        cam.type_here("!");
        let after = cam.shoot("09-data-table-cell-edited");

        assert_eq!(
            text_of(&cam, "dt-kind-0"),
            "runtime!",
            "the keystroke did not reach the cell"
        );
        assert_eq!(
            text_of(&cam, "dt-kind-1"),
            "layout",
            "the keystroke reached a cell it was not aimed at"
        );
        cam.key(KeyCode::Backspace);
        cam.key(KeyCode::Backspace);
        assert_eq!(
            text_of(&cam, "dt-kind-0"),
            "runtim",
            "backspace did not reach the cell"
        );
        assert_ne!(before, after, "the edit never reached the picture");
    }

    /// A press meant to put the caret in a cell must not also select the
    /// row it is in. Every route into a cell names its row too, so this is
    /// a real ordering hazard and not a hypothetical one.
    #[test]
    fn clicking_a_data_table_cell_does_not_select_its_row() {
        let mut cam = Camera::on("Data table");
        assert!(!selected(&cam, "dt-0"));
        cam.click("dt-kind-0");
        assert!(
            !selected(&cam, "dt-0"),
            "clicking into the cell selected the row underneath it"
        );
        // The row itself still selects, so nothing was swallowed wholesale.
        cam.click("dt-0/c0/name");
        assert!(selected(&cam, "dt-0"), "the row stopped being selectable");
    }

    /// Row 13, round 3: *"broken elements."* The page was a unit struct
    /// whose `handle` returned `false`, so the field took no keystroke and
    /// the checkbox never moved. Both are driven here from a click.
    #[test]
    fn the_form_field_takes_a_keystroke_after_a_click() {
        let mut cam = Camera::on("Form");
        let before = cam.shoot("13-form");
        assert_eq!(text_of(&cam, "form-name"), "");

        cam.click("form-name");
        assert!(
            cam.focused()
                .as_deref()
                .is_some_and(|id| id.ends_with("form-name")),
            "the click did not seat the caret in the form field: {:?}",
            cam.focused()
        );
        cam.type_here("fiber-0");
        let after = cam.shoot("13-form-typed");
        assert_eq!(
            text_of(&cam, "form-name"),
            "fiber-0",
            "the form field took no keystroke"
        );
        assert_ne!(before, after, "the typing never reached the picture");
    }

    /// The other half of row 13: the checkbox held no state either.
    #[test]
    fn the_form_checkbox_toggles_when_it_is_pressed() {
        let mut cam = Camera::on("Form");
        assert!(selected(&cam, "form-ok"), "the page opens with it checked");
        let on = cam.shoot("13-form-checked");
        cam.click("form-ok");
        let off = cam.shoot("13-form-unchecked");
        assert!(
            !selected(&cam, "form-ok"),
            "pressing the form checkbox did not unset it"
        );
        assert_ne!(on, off, "the toggle never reached the picture");
        cam.click("form-ok");
        assert!(selected(&cam, "form-ok"), "it does not come back on");
    }

    // ===== Wave A, round 3: rows 39, 02 and 03 =====
    //
    // All three were the same defect wearing three faces: a unit-struct page
    // whose `handle` returned `false`, so the component underneath it was
    // photographed and never driven. Every test below starts from a
    // `Camera::click` and finishes by reading the raster, because a page that
    // holds state is exactly the thing a resting photograph cannot show.

    /// The strongest blue-over-red a pixel inside `rect` has.
    ///
    /// Link ink is `#4589ff` (b - r = 186) and both page ink `#f4f4f4` and
    /// every grey ground are neutral (0), so this one number separates "this
    /// text is a link" from "this text is prose" off the picture rather than
    /// off a token binding. Antialiasing only ever pulls the number down, so
    /// a high reading cannot be an artefact.
    fn blue_lead(img: &image::RgbaImage, rect: Rect) -> i32 {
        let x0 = (rect.x * CAPTURE_SCALE) as u32;
        let x1 = ((rect.x + rect.w) * CAPTURE_SCALE) as u32;
        let y0 = (rect.y * CAPTURE_SCALE) as u32;
        let y1 = ((rect.y + rect.h) * CAPTURE_SCALE) as u32;
        let mut best = 0;
        for y in y0..y1.min(img.height()) {
            for x in x0..x1.min(img.width()) {
                let p = img.get_pixel(x, y).0;
                best = best.max(i32::from(p[2]) - i32::from(p[0]));
            }
        }
        best
    }

    /// Row 39. A press on a shut branch opens it: the hidden rows arrive in
    /// the frame and the picture changes.
    ///
    /// Falsified 2026-09-05 by putting `fn handle(..) -> bool { false }` back
    /// on `page/tree_view.rs`: `tv-gallery` never appears and `Camera::id`
    /// panics listing every placed id, which is the old page exactly.
    #[test]
    fn clicking_a_shut_tree_branch_mounts_the_rows_underneath_it() {
        let mut cam = Camera::on("Tree view");
        assert!(
            !cam.has("tv-gallery"),
            "petra-egui is meant to start shut, so this proves nothing"
        );
        let before = cam.shoot("39-tree-view-before-branch-opens");
        cam.click("tv-petra-egui/row");
        assert!(
            cam.has("tv-gallery"),
            "the press did not open the branch: the page dropped it"
        );
        let after = cam.shoot("39-tree-view-expanded");
        assert_ne!(
            before, after,
            "the branch opened in the tree and changed no pixel"
        );
    }

    /// Row 39. The accent bar moves to the row that was pressed.
    ///
    /// Read at the row's inline-start edge, where Carbon's
    /// `.cds--tree-node--active` 4-unit bar lives, because
    /// `Semantics.selected` is a flag and the operator's complaint was the
    /// picture. Both halves are asserted: the bar arrives on the new row
    /// **and** leaves the old one, because a bar that never moves would pass
    /// the first half on its own.
    ///
    /// The press names the row and not the item: a branch item's rect spans
    /// its children too, so its centre is inside a child and that is where a
    /// hand would land. `.../tv-petra` alone shut nothing for exactly that
    /// reason, which is worth knowing before writing the next tree test.
    #[test]
    fn a_press_moves_the_tree_selection_bar_onto_the_row_it_landed_on() {
        let mut cam = Camera::on("Tree view");
        let was = cam.rect("tv-component/row");
        let shot = raster(&mut cam, "39-tree-view-resting");
        let bar = px(&shot, was.x + 1.0, was.y + was.h / 2.0);
        assert!(
            i32::from(bar[2]) - i32::from(bar[0]) > 100,
            "no accent bar at the selected leaf's inline edge: {bar:?}"
        );

        cam.click("tv-inspector/row");
        let now = cam.rect("tv-inspector/row");
        let was = cam.rect("tv-component/row");
        let shot = raster(&mut cam, "39-tree-view-selection-moved");
        let moved = px(&shot, now.x + 1.0, now.y + now.h / 2.0);
        assert!(
            i32::from(moved[2]) - i32::from(moved[0]) > 100,
            "the press did not move the accent bar onto the row: {moved:?}"
        );
        let left = px(&shot, was.x + 1.0, was.y + was.h / 2.0);
        assert!(
            i32::from(left[2]) - i32::from(left[0]) < 40,
            "the accent bar is still on the row that was selected before: {left:?}"
        );
    }

    /// Row 03. The leading crumbs are painted in link ink and the last one
    /// is not, which is the whole of what "i dont understand this one" was
    /// about: three identical grey words with slashes between them.
    ///
    /// Falsified 2026-09-05 by making `breadcrumb_item` a bare
    /// `text(key, label)` again: `blue_lead` on the leading crumb drops from
    /// 186 to 0 and this fails on the first assertion.
    #[test]
    fn a_breadcrumb_paints_its_links_blue_and_the_page_you_are_on_in_page_ink() {
        let mut cam = Camera::on("Breadcrumb");
        let link = cam.rect("bc-0");
        let current = cam.rect("bc-3");
        let shot = raster(&mut cam, "03-breadcrumb-resting");
        assert!(
            blue_lead(&shot, link) > 40,
            "the leading crumb is not in link ink"
        );
        assert!(
            blue_lead(&shot, current) < 12,
            "the current page is painted like a link"
        );
    }

    /// Row 03. A press on a leading crumb walks the trail up: the levels
    /// below it leave the frame and it becomes the crumb in page ink.
    #[test]
    fn clicking_a_crumb_walks_the_trail_up_to_it() {
        let mut cam = Camera::on("Breadcrumb");
        assert!(cam.has("bc-3"), "the trail starts at its deepest level");
        cam.click("bc-1");
        assert!(!cam.has("bc-2"), "the trail did not shorten");
        assert!(!cam.has("bc-3"));
        let now_current = cam.rect("bc-1");
        let shot = raster(&mut cam, "03-breadcrumb-walked-up");
        assert!(
            blue_lead(&shot, now_current) < 12,
            "the crumb pressed is still painted as a link to somewhere else"
        );
        assert!(
            blue_lead(&shot, cam.rect("bc-0")) > 40,
            "the level above it stopped being a link"
        );
    }

    /// Row 02. The AI mark is a button, and what it opens is the thing the
    /// page exists to show.
    ///
    /// Falsified 2026-09-05 by returning `false` from
    /// `page/ai_label.rs::handle`: `panel` never appears and the two shots
    /// are byte-identical.
    #[test]
    fn clicking_the_ai_mark_opens_the_explainability_popover() {
        let mut cam = Camera::on("AI label");
        assert!(!cam.has("panel"), "the page starts closed");
        let before = cam.shoot("02-ai-label-resting");
        cam.click("ai-live/trigger");
        assert!(
            cam.has("panel"),
            "the press did not open the explainability popover"
        );
        let after = cam.shoot("02-ai-label-open");
        assert_ne!(before, after, "the panel opened and changed no pixel");
        cam.click("ai-live/trigger");
        assert!(!cam.has("panel"), "the second press did not shut it");
    }

    /// Row 02. The revert control holds its word with air on both sides.
    ///
    /// Pinned square at 40 the glyphs sat 3 points off one border and 2.5
    /// off the other. Sampled on the raster at the row through the middle of
    /// the control: the two device columns just inside each border must be
    /// the control's own fill, not part of a letter.
    #[test]
    fn the_revert_control_is_not_crammed_against_its_own_border() {
        let mut cam = Camera::on("AI label");
        let revert = cam.rect("ai-revert");
        let shot = raster(&mut cam, "02-ai-label-revert");
        let mid = revert.y + revert.h / 2.0;
        let ground = px(&shot, revert.x + revert.w / 2.0, revert.y + 3.0);
        for offset in [3.0_f32, 5.0, 7.0] {
            for edge in [revert.x + offset, revert.x + revert.w - offset] {
                assert_eq!(
                    px(&shot, edge, mid),
                    ground,
                    "a glyph reaches to within {offset} points of the revert \
                     control's border, which is the cramming this row was \
                     reported for"
                );
            }
        }
    }

    // ===== Wave D, round 3: loading, inline loading, notification, toggle,
    // accordion. One contiguous block, appended; nothing above this line is
    // touched, because two other waves edit this file in their own trees.

    /// The device-pixel rect of a logical rect, inset on every side.
    ///
    /// The inset is what makes a fill test about the fill: a rounded control
    /// antialiases its own edge into the ground behind it, so an
    /// uninset crop of a knob always carries the track's tone and every
    /// "is this one colour" question answers no.
    fn inset_pixels(img: &image::RgbaImage, rect: Rect, inset: u32) -> Vec<[u8; 4]> {
        let scale = super::CAPTURE_SCALE;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x0, y0, x1, y1) = (
            (rect.x * scale).round() as u32 + inset,
            (rect.y * scale).round() as u32 + inset,
            ((rect.x + rect.w) * scale).round() as u32 - inset,
            ((rect.y + rect.h) * scale).round() as u32 - inset,
        );
        let mut out = Vec::new();
        for y in y0..y1.min(img.height()) {
            for x in x0..x1.min(img.width()) {
                out.push(img.get_pixel(x, y).0);
            }
        }
        assert!(!out.is_empty(), "{rect:?} inset by {inset} has no pixels");
        out
    }

    /// Rec. 601 luminance of one pixel, 0-255.
    fn pixel_luma(p: [u8; 4]) -> u32 {
        (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000
    }

    /// The tone `after` carries inside `rect` that `before` did not, and how
    /// many pixels wear it.
    ///
    /// "A new tone appeared" is the honest reading of "a fill was painted".
    /// A point sample lands on a glyph as often as on the ground, and
    /// counting *changed* pixels cannot tell a painted band from a caret
    /// that moved through the same rect. This is the strongest claim two
    /// rasters of the same rect support without knowing the theme.
    ///
    /// The busiest such tone wins, so antialiasing along a glyph edge — a
    /// handful of pixels each — cannot outvote a band.
    fn new_tone(
        before: &image::RgbaImage,
        after: &image::RgbaImage,
        rect: Rect,
    ) -> ([u8; 4], usize) {
        let count = |img: &image::RgbaImage| {
            let mut seen: std::collections::HashMap<[u8; 4], usize> =
                std::collections::HashMap::new();
            for p in inset_pixels(img, rect, 0) {
                *seen.entry(p).or_default() += 1;
            }
            seen
        };
        let (was, now) = (count(before), count(after));
        now.into_iter()
            .filter(|(tone, _)| was.get(tone).copied().unwrap_or(0) < 20)
            .max_by_key(|(_, n)| *n)
            .unwrap_or(([0, 0, 0, 0], 0))
    }

    /// The highest per-pixel luminance inside `rect`.
    ///
    /// For telling one ink from another where the two are the same glyphs at
    /// the same size: the peak is the glyph's own core, before antialiasing
    /// drags it towards the ground.
    fn brightest(img: &image::RgbaImage, rect: Rect) -> u32 {
        inset_pixels(img, rect, 0)
            .into_iter()
            .map(pixel_luma)
            .max()
            .unwrap_or(0)
    }

    /// The leftmost and rightmost logical x inside `rect` wearing `tone`.
    fn tone_span(img: &image::RgbaImage, rect: Rect, tone: [u8; 4]) -> (f32, f32) {
        let scale = super::CAPTURE_SCALE;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x0, y0, x1, y1) = (
            (rect.x * scale).round() as u32,
            (rect.y * scale).round() as u32,
            ((rect.x + rect.w) * scale).round() as u32,
            ((rect.y + rect.h) * scale).round() as u32,
        );
        let (mut left, mut right) = (f32::MAX, f32::MIN);
        for y in y0..y1.min(img.height()) {
            for x in x0..x1.min(img.width()) {
                if img.get_pixel(x, y).0 == tone {
                    #[allow(clippy::cast_precision_loss)]
                    let at = x as f32 / scale;
                    left = left.min(at);
                    right = right.max(at);
                }
            }
        }
        (left, right)
    }

    /// How many of two equal-length pixel runs differ at all.
    fn differing(a: &[[u8; 4]], b: &[[u8; 4]]) -> usize {
        assert_eq!(a.len(), b.len(), "two crops of the same rect");
        a.iter().zip(b).filter(|(p, q)| p != q).count()
    }

    /// Run `n` host passes without capturing, to advance egui's clock.
    ///
    /// `Camera` exposes no clock. Every pass egui runs with `RawInput::time`
    /// unset advances its own `input.time` by the predicted frame time, one
    /// sixtieth of a second, and `Host::pass` hands that to `App::tick` — so
    /// counting passes *is* the clock, and it is the only clock a driven test
    /// has. Hovering a fixed point in the window's bottom-right corner is the
    /// cheapest pass this driver can produce: it takes no GPU capture, and
    /// every assertion below reads one control's own rect, so a hover
    /// highlight anywhere else cannot reach the measurement.
    fn advance(cam: &mut Camera, n: usize) {
        for _ in 0..n {
            cam.hover_at(WINDOW[0] - 4.0, WINDOW[1] - 4.0);
        }
    }

    /// One sixtieth of a second, the step `advance` spends per pass.
    const FRAME_SECONDS: f64 = 1.0 / 60.0;

    /// Row 17. The turn takes 1.38 s of wall clock, read off the pixels.
    ///
    /// **This is a departure from Carbon and the test says so.**
    /// `_animation.scss` measures 690 ms and this library drew at it; the
    /// operator asked twice for it to slow down and chose that over keeping
    /// Carbon's rate on 2026-09-05. `loading.rs`'s
    /// `one_turn_takes_twice_carbon_s_690ms_of_real_time` pins the constant
    /// against real seconds; this pins that the constant reaches the picture.
    ///
    /// The measurement is a period, not a speed, so it needs no angle
    /// arithmetic: photograph the spinner, advance a whole turn, photograph
    /// again — few pixels differ. Advance a half turn instead and the arc is
    /// on the other side of the ring, so most of them do. At Carbon's 690 ms
    /// those two swap places, which is exactly how this goes red.
    #[test]
    fn the_large_spinner_takes_one_and_a_third_seconds_to_come_back_round() {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let passes = |seconds: f64| (seconds / FRAME_SECONDS).round() as usize;
        let turn = passes(1.38);
        let half = passes(0.69);
        assert_eq!((turn, half), (83, 41));

        let mut cam = Camera::on("Loading");
        let ring = cam.rect("sizes/load-lg");
        let start = inset_pixels(&raster(&mut cam, "17-loading-large-first-frame"), ring, 0);

        advance(&mut cam, half - 1);
        let half_way = inset_pixels(&raster(&mut cam, "17-loading-half-turn"), ring, 0);

        advance(&mut cam, turn - half - 1);
        let full = inset_pixels(&raster(&mut cam, "17-loading-one-turn"), ring, 0);

        let moved = differing(&start, &half_way);
        let returned = differing(&start, &full);
        assert!(
            returned * 4 < moved,
            "a full turn later the spinner differs from where it started in \
             {returned} pixels, but a half turn later in only {moved} — the \
             picture is not coming back round at 1.38 s. At Carbon's 690 ms \
             these two are the wrong way about, which is the whole point of \
             this test."
        );
        // And it is genuinely moving, so "returned" is not "frozen".
        assert!(
            moved > 200,
            "half a turn moved only {moved} pixels; the spinner is not turning"
        );
    }

    /// Row 14. The finished mark's tick is centred in its own disc.
    ///
    /// **Not driven from a click, and it cannot be**: the Inline loading page
    /// is two status readouts with no control on it, and `Role::Status`
    /// declares no interaction, so there is nothing on the page a hand can
    /// press. What is driven is the clock, through `advance`, which is what
    /// separates the still finished mark from the turning one beside it.
    ///
    /// The assertion is on ink, not on rects, because the rects were already
    /// right while the picture was wrong: the badge declared 16x16 and the
    /// tick declared 10x10 the whole time the tick sat 3 units left of
    /// centre. Ink is every pixel inside the disc that is not the disc's own
    /// fill; its bounding box must be centred on the disc.
    #[test]
    fn the_saved_marks_tick_sits_in_the_middle_of_its_disc() {
        let mut cam = Camera::on("Inline loading");
        let disc = cam.rect("il-off/mark");
        let shot = raster(&mut cam, "14-inline-loading-finished");
        let scale = super::CAPTURE_SCALE;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x0, y0, x1, y1) = (
            (disc.x * scale).round() as u32,
            (disc.y * scale).round() as u32,
            ((disc.x + disc.w) * scale).round() as u32,
            ((disc.y + disc.h) * scale).round() as u32,
        );
        // The disc's own fill, read from a point a quarter in from its left
        // edge on its middle row — inside the fill, outside any centred mark.
        let fill = shot.get_pixel(x0 + (x1 - x0) / 6, (y0 + y1) / 2).0;
        let dark = |p: [u8; 4]| {
            // The tick is `text.on-accent` on `accent.primary`: much darker
            // than the fill on every channel.
            u32::from(fill[0]) + u32::from(fill[1]) + u32::from(fill[2])
                > u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]) + 150
        };
        let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (u32::MAX, 0u32, u32::MAX, 0u32);
        let mut ink = 0usize;
        for y in y0..y1 {
            for x in x0..x1 {
                if dark(shot.get_pixel(x, y).0) {
                    ink += 1;
                    lo_x = lo_x.min(x);
                    hi_x = hi_x.max(x);
                    lo_y = lo_y.min(y);
                    hi_y = hi_y.max(y);
                }
            }
        }
        assert!(
            ink > 20,
            "found {ink} ink pixels inside the finished disc: there is no \
             tick on it at all"
        );
        let ink_centre = (
            f32::from(u16::try_from(lo_x + hi_x).unwrap()) / 2.0,
            f32::from(u16::try_from(lo_y + hi_y).unwrap()) / 2.0,
        );
        let disc_centre = (
            f32::from(u16::try_from(x0 + x1).unwrap()) / 2.0,
            f32::from(u16::try_from(y0 + y1).unwrap()) / 2.0,
        );
        let (dx, dy) = (ink_centre.0 - disc_centre.0, ink_centre.1 - disc_centre.1);
        assert!(
            dx.abs() <= 1.0 && dy.abs() <= 1.0,
            "the tick's ink is centred {dx:+.1},{dy:+.1} device pixels off \
             the disc's own centre. Before 2026-09-05 this was -6,0: the \
             10-unit mark packed against the leading edge of the 16 disc \
             because the badge set `align` (the cross axis) and no `justify`."
        );
    }

    /// Row 36. **Departure from Carbon, operator decision 2026-09-05:** the
    /// small on-toggle's knob is bare.
    ///
    /// Carbon puts a 6x5 `$support-success` tick inside that knob —
    /// `Toggle.js` renders it under `isSm && !readOnly`, and
    /// `ignored/carbon-ref/shots/36-toggle.png` shows it. The operator asked
    /// twice for the mark to go and chose deletion over Carbon's placement
    /// when both were put to him. This test is what a later conformance pass
    /// has to argue with before putting it back.
    ///
    /// Driven from a click on the small **off** toggle, so the knob under
    /// test is one this test turned on rather than one the page was born
    /// with, and read off the pixels: the knob's interior is one flat tone.
    #[test]
    fn clicking_the_small_toggle_on_leaves_its_knob_bare() {
        let mut cam = Camera::on("Toggle");
        cam.click("toggle-sm-off");
        let knob = cam.rect("toggle-sm-off/appearance/track/knob");
        let shot = raster(&mut cam, "36-toggle-sm-clicked-on");
        // 4 device pixels in from a 20-device-pixel circle leaves a 12x12
        // square whose half-diagonal is 8.49, inside the 10 radius.
        let interior = inset_pixels(&shot, knob, 4);
        let mut tones: Vec<[u8; 4]> = interior.clone();
        tones.sort_unstable();
        tones.dedup();
        assert_eq!(
            tones.len(),
            1,
            "the knob's interior is {} tones, not one: something is drawn \
             inside it ({:?})",
            tones.len(),
            &tones[..tones.len().min(4)]
        );
        // The rect really landed on the knob and not on the track: the two
        // are different tones, and the track is the accent.
        let knob_tone = tones[0];
        let track = cam.rect("toggle-sm-off/appearance/track");
        let track_tone = shot
            .get_pixel(
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    ((track.x + track.w - 2.0) * super::CAPTURE_SCALE) as u32
                },
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    ((track.y + track.h / 2.0) * super::CAPTURE_SCALE) as u32
                },
            )
            .0;
        assert_ne!(
            knob_tone, track_tone,
            "the rect under test is the track, not the knob"
        );
        assert!(
            u32::from(track_tone[2]) > u32::from(track_tone[0]) + 60,
            "the clicked toggle's track is not the accent: {track_tone:?}"
        );
    }

    /// Row 21. The icon field the operator asked for, driven and read.
    ///
    /// Two claims, both from pixels. The four kinds on the page draw four
    /// different marks, so a reader who cannot use hue can still tell an
    /// error from a success — Carbon separates them by `$support-error` and
    /// `$support-success` as well as by glyph, and this library has only the
    /// glyph. And the toast's own field is live: pressing its action steps
    /// the kind and the mark in the picture changes with it.
    #[test]
    fn the_notification_kinds_draw_four_different_marks_and_the_toast_steps() {
        let mut cam = Camera::on("Notification");
        let cards = ["nt-error", "nt-warning", "nt-info", "nt-success"];
        // The column is one column. Round 3 photographed a staircase: each
        // card had shrink-wrapped its own sentence, so four cards drew four
        // widths. Carbon floors an inline notification at 288 and caps it at
        // the ramp's `md` step, and the page stretches them to that band.
        let widths: Vec<f32> = cards.iter().map(|card| cam.rect(card).w).collect();
        for (card, width) in cards.iter().zip(&widths) {
            assert!(
                (width - widths[0]).abs() <= 0.5,
                "{card} is {width} wide where {} is {}: the cards stair-step",
                cards[0],
                widths[0]
            );
        }
        assert!(
            widths[0] >= 288.0,
            "an inline card is under Carbon's 288 floor: {}",
            widths[0]
        );
        let rects: Vec<Rect> = cards
            .iter()
            .map(|card| cam.rect(&format!("{card}/glyph")))
            .collect();
        let shot = raster(&mut cam, "21-notification");
        let marks: Vec<Vec<[u8; 4]>> = rects
            .iter()
            .map(|rect| inset_pixels(&shot, *rect, 0))
            .collect();
        for (i, a) in marks.iter().enumerate() {
            for (j, b) in marks.iter().enumerate().skip(i + 1) {
                assert!(
                    differing(a, b) > 20,
                    "{} and {} draw the same mark: the kind is not reaching \
                     the picture, and hue is not a channel this operator has",
                    cards[i],
                    cards[j]
                );
            }
        }

        // The toast opens on Info. Press its action and the mark must change.
        let toast_glyph = cam.rect("panel/glyph");
        let before = inset_pixels(&shot, toast_glyph, 0);
        cam.click("actions/action");
        let after_shot = raster(&mut cam, "21-notification-stepped");
        let after = inset_pixels(&after_shot, cam.rect("panel/glyph"), 0);
        assert!(
            differing(&before, &after) > 20,
            "a click on the toast's action left its status mark unchanged"
        );
    }

    /// Row 01. The nested accordion, opened by a click on an inner header.
    ///
    /// The operator asked to see one. **Carbon writes no rule for nesting**
    /// — no permission, no ban, no depth limit, no nested styling — and
    /// steers deep hierarchy at Tree view instead, so this is off the
    /// conformance target rather than against it; `accordion_item_with`'s
    /// doc carries the sources.
    ///
    /// Driven, because a resting photograph of an open panel cannot tell a
    /// working nested accordion from a picture of one, and because the inner
    /// header's routed path names the outer item too — a handler that reads
    /// the outer key first swallows every inner press.
    #[test]
    fn clicking_an_inner_accordion_header_opens_only_that_inner_panel() {
        let mut cam = Camera::on("Accordion");
        assert!(
            cam.has("acc-nest/body/inner/acc-nest-1"),
            "the nested list is not on the page"
        );
        assert!(
            !cam.has("acc-nest-1/body"),
            "the second nested section starts shut"
        );
        let outer = cam.rect("acc/acc-nest");
        let before = raster(&mut cam, "01-accordion");

        cam.click("acc-nest-1/header");
        assert!(
            cam.has("acc-nest-1/body"),
            "the click on the inner header mounted no panel"
        );
        assert!(
            cam.has("acc-nest-0/body"),
            "opening one nested section shut its sibling; Carbon's accordion \
             holds any number open"
        );
        let after = raster(&mut cam, "01-accordion-inner-open");
        assert!(
            cam.rect("acc/acc-nest").h > outer.h,
            "the outer item did not grow to hold the newly opened inner panel"
        );
        assert_ne!(
            before.dimensions(),
            (0, 0),
            "the shot before the click is empty"
        );
        assert_ne!(
            before.into_raw(),
            after.into_raw(),
            "the click changed nothing on screen"
        );

        // The inner list is indented inside the outer item's panel, which is
        // the only geometry this library states for a nested accordion: the
        // panel's own inline padding is the indent.
        let outer_header = cam.rect("acc-nest/header");
        let inner_header = cam.rect("acc-nest-1/header");
        assert!(
            inner_header.x > outer_header.x + 8.0,
            "the nested header sits at x {} against the outer header's {}: \
             nothing marks the level",
            inner_header.x,
            outer_header.x
        );
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
    // ----------------------------------------------------------------
    // Wave E, 2026-09-05. Rows 12 (File uploader), 04 (Button), 24/37
    // (Popover and Toggletip). One contiguous block, appended at the end:
    // another wave edits this file in its own worktree and the merge is by
    // hand.
    // ----------------------------------------------------------------

    /// Mean luma of a crop, Rec. 709, so a claim can be made about what a
    /// reader who cannot use hue sees.
    fn luma(pixels: &[[u8; 4]]) -> f32 {
        let sum: f32 = pixels
            .iter()
            .map(|p| 0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]))
            .sum();
        sum / pixels.len() as f32
    }

    /// How many pixels of a crop are red-dominant by `lead` on both of the
    /// other two channels. A grey, a blue and a white all score zero.
    fn red_lead(pixels: &[[u8; 4]], lead: i32) -> usize {
        pixels
            .iter()
            .filter(|p| {
                let (r, g, b) = (i32::from(p[0]), i32::from(p[1]), i32::from(p[2]));
                r - g > lead && r - b > lead
            })
            .count()
    }

    /// The modal colour of a crop, so a bubble's fill can be named rather
    /// than sampled at one point that might land on a glyph.
    fn modal_colour(pixels: &[[u8; 4]]) -> [u8; 4] {
        let mut counts: std::collections::HashMap<[u8; 4], usize> =
            std::collections::HashMap::new();
        for p in pixels {
            *counts.entry(*p).or_default() += 1;
        }
        counts
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(c, _)| c)
            .expect("a crop has pixels")
    }

    /// Row 12. The drop zone is Carbon's 320 x 96 box with its link in the
    /// top-left corner, and a row's remove control really removes it.
    ///
    /// **What was wrong.** `file_uploader.rs:68` set only
    /// `zone.constraints.vertical.min`, so the box took the width of the
    /// words inside it — 121 points against Carbon's 320, measured off
    /// `12-file-uploader.png` — and `zone.props.align = Center` put the
    /// caption on its midline where Carbon's is `align-items: flex-start`.
    /// The caption was grey body text reading "Drop files here", so nothing
    /// in the picture said a click does anything.
    ///
    /// **How this went red before the fix.** Restore either half and it
    /// fails on its own: dropping the horizontal constraint fails the width
    /// assertion at 121, and putting `Align::Center` back fails the
    /// top-left corner assertion.
    ///
    /// The remove half is driven, because a list that cannot lose a row is
    /// a picture of a list. Carbon's `status="edit"` file row is a `Close`
    /// button and this is that button, pressed.
    #[test]
    fn the_drop_zone_is_a_carbon_box_and_a_file_row_can_be_removed() {
        let mut cam = Camera::on("File uploader");
        let zone = cam.rect("fu/zone");
        assert!(
            (zone.w - 320.0).abs() < 0.5,
            "the drop zone is {} wide; Carbon's `.cds--file-browse-btn` caps \
             at 320 and the reference renders it there",
            zone.w
        );
        assert!(
            (zone.h - 96.0).abs() < 0.5,
            "the drop zone is {} tall; `_file-uploader.scss:425` is 96",
            zone.h
        );
        // **Centred, which is a departure from Carbon and is the operator's
        // instruction.** MEASURED `_file-uploader.scss:415-426`:
        // `.cds--file__drop-container` is `align-items: flex-start;
        // justify-content: space-between`, so Carbon's own prompt sits in
        // the box's top-left corner with 60 points of empty box under it.
        // Round 4, row 12: *"file uploader: the text needs to be centered in
        // the ui"*. Both axes, because one line alone in a 320 x 96 box is
        // off-centre in both and a person asking for "centered" means the
        // middle of the box.
        //
        // Held to a point rather than to an edge, so the claim survives a
        // change in the prompt's own width: the two centres coincide.
        let prompt = cam.rect("fu/zone/prompt");
        let dx = (prompt.x + prompt.w / 2.0) - (zone.x + zone.w / 2.0);
        let dy = (prompt.y + prompt.h / 2.0) - (zone.y + zone.h / 2.0);
        assert!(
            dx.abs() < 1.0 && dy.abs() < 1.0,
            "the prompt's centre is ({dx}, {dy}) from the zone's; the drop \
             zone's one line has to sit in the middle of the box on both \
             axes, and it is off by more than a point"
        );

        // The rows under it line up with the zone: both are 320 in Carbon,
        // and ours were 320 and 288 while the module doc cited `18rem`.
        let item = cam.rect("fu-busy");
        assert!(
            (item.w - zone.w).abs() < 0.5,
            "the file row is {} wide against a {}-wide zone; Carbon's \
             `.cds--file__selected-file` caps at the same 320",
            item.w,
            zone.w
        );

        let before = raster(&mut cam, "12-file-uploader");
        // The prompt is a link: blue ink with a rule under it. Read off the
        // pixels, because `foreground` and `underline` are two bindings and
        // a slot the painter ignored would pass every tree-level check.
        let ink = inset_pixels(&before, prompt, 0);
        let blue = ink
            .iter()
            .filter(|p| i32::from(p[2]) - i32::from(p[0]) > 40)
            .count();
        assert!(
            blue > 40,
            "only {blue} pixels of the prompt are blue-leaning: the link ink \
             is not reaching the picture, and a grey prompt is what the \
             operator read as `not ibm carbon`"
        );

        assert!(
            cam.has("fu-1"),
            "the page starts holding two removable files"
        );
        cam.click("fu-1/remove");
        let after = raster(&mut cam, "12-file-uploader-removed");
        assert!(
            !cam.has("fu-1"),
            "pressing a row's remove control left the row on the page. \
             Placed:\n  {}",
            cam.ids().join("\n  ")
        );
        assert!(cam.has("fu-0"), "removing one row took its sibling with it");
        assert_ne!(
            before.into_raw(),
            after.into_raw(),
            "the removal changed nothing on screen"
        );
    }

    /// Row 04. **The filled danger button carries no mark; the two unfilled
    /// ones lead with Carbon's `WarningFilled` glyph.**
    ///
    /// # What the operator said, and what it split into
    ///
    /// Round 4, row 04: *"we had the danger button before I complained
    /// looked too much like default. So now you built 3 delete buttons.
    /// They do not look that good. I think just the issue is this rounded
    /// square in the center of it looks bad. Probably remove it for the
    /// solid colored button and change the shape on the other 2."*
    ///
    /// He is describing `shape.silhouette-octagon`, and he read it right.
    /// `paint.rs`'s `silhouette_points` chamfers the box by 0.16 of a side,
    /// which is a *chamfered square*, not a regular octagon: at the 16-unit
    /// button mark the four flat sides are 10.9 units and the four
    /// diagonals are 3.6, a 3:1 ratio, so the axis-aligned sides dominate
    /// and the figure reads as a box with the corners knocked off. That
    /// number was tuned for the 10x10 status marker, where the job was to
    /// look unlike a *disc*; at 16 units against a word it looks like a
    /// rounded square. `paint.rs`'s own comment says so — "chamfered box,
    /// not a regular octagon" — and the octagon is not changed here,
    /// because `status.down` is drawn from the same figure and that is not
    /// this row.
    ///
    /// # The two halves this asserts
    ///
    /// **The filled variant has no mark at all.** Its second channel is
    /// its own fill's *luminance*: `button-danger-primary` sits between
    /// the default button's grey and the primary's blue with a wide gap on
    /// each side, so it survives a reader with no red-green channel
    /// without anything drawn on top of it. That is measured below in
    /// greyscale, not asserted. Its third channel is
    /// `Semantics.value = "danger"`, which is exactly Carbon's own
    /// affordance — MEASURED
    /// `@carbon/react/lib/components/Button/ButtonBase.js:49-68`, where
    /// `dangerDescription` is rendered as a `cds--visually-hidden` span
    /// and hung off `aria-describedby`. Carbon draws **no** visible danger
    /// glyph on any button variant; there is none in
    /// `@carbon/styles/scss/components/button/_button.scss` either.
    ///
    /// **The two unfilled variants lead with a glyph.** They have no fill
    /// to carry the kind, so they keep a mark, and the mark is now
    /// `IconMark::WarningFilled` — a real traced Carbon path — instead of
    /// the silhouette. The assertion that tells the two apart is
    /// **coverage**: a solid chamfered box fills about 97% of its own 16x16
    /// box, and a ring with a stem and a dot fills a fraction of it. So the
    /// bound fails in both directions — too high means the blob is back,
    /// too low means the glyph vanished.
    ///
    /// **How this went red before the fix.** With the octagon restored,
    /// the first assertion fails at `btn-danger still draws a mark`, and
    /// with only the filled variant changed the coverage bound fails at
    /// about 0.97 on `btn-danger-tertiary`.
    #[test]
    fn the_filled_danger_button_drops_the_mark_and_the_other_two_carry_a_glyph() {
        let mut cam = Camera::on("Button");
        // Driven, so the claim is about a live page and not a constructor:
        // a pointer on the danger button must not change what it carries.
        cam.hover("btn-danger");
        let shot = raster(&mut cam, "04-button-danger");

        // ---- The filled variant: no mark, and two channels that are not
        // the hue.
        assert!(
            !cam.has("btn-danger/mark"),
            "btn-danger still draws a mark. The red fill is the channel on \
             this variant; a white blob on top of it is what the operator \
             asked to have removed."
        );
        let value = cam
            .frame()
            .placement(&cam.id("btn-danger"))
            .expect("the danger button is placed")
            .semantics
            .value
            .clone();
        assert_eq!(
            value.as_deref(),
            Some("danger"),
            "the filled danger button stopped naming its kind. That string \
             is the only channel left for a reader who is not looking at \
             the picture, and it is Carbon's own -- `dangerDescription` in \
             `ButtonBase.js`."
        );

        /// How far apart two button fills have to sit in greyscale.
        ///
        /// **This is the tightest number in this test and it is set from a
        /// measurement, not from a standard.** Read off the raster on
        /// 2026-09-05: the danger fill is Rec. 709 luma 70.7, the default
        /// button's `surface.raised` `#333333` is 51.0, and the primary's
        /// `accent.primary` `#4589ff` is 118.2. So the narrow side is 19.7
        /// and the wide side is 47.5. The floor sits just under the narrow
        /// one, which is what makes a later re-tone of
        /// `button-danger-primary` toward its grey neighbour fail here
        /// rather than quietly turn the filled variant back into the
        /// default button — the operator's round 3 complaint.
        ///
        /// What this floor is **not** is a claim that 19.7 is comfortable.
        /// `#da1e28` against `#333333` is 2.53:1 as a contrast ratio, and
        /// ΔE\*ab 28.9 protanope / 62.5 deuteranope under the
        /// Viénot-Brettel-Mollon transform `token::shipped`'s own gate
        /// uses — 1.1 short of this library's `MIN_STATUS_SEPARATION` of
        /// 30 on the protanope side. Those three numbers were computed off
        /// the sampled pixels rather than asserted by any gate, because
        /// the simulation matrices live in `shipped.rs`'s private test
        /// module and copying them here is the drift that keeps two
        /// numbers from ever agreeing again. The wave's Agent Note carries
        /// the consequence: the label is the channel that closes that gap,
        /// and it is the same channel Carbon relies on.
        const MIN_FILL_LUMA_GAP: f32 = 15.0;

        // A strip inside each button's trailing edge, clear of the label
        // and of any mark: the fill, and nothing else.
        let fill_of = |cam: &Camera, tail: &str| -> Rect {
            let b = cam.rect(tail);
            Rect {
                x: b.x + b.w - 6.0,
                y: b.y + 4.0,
                w: 4.0,
                h: b.h - 8.0,
            }
        };
        let danger_fill = inset_pixels(&shot, fill_of(&cam, "btn-danger"), 0);
        assert!(
            red_lead(&danger_fill, 60) > danger_fill.len() / 2,
            "btn-danger's fill does not lead red: {} of {} pixels. With the \
             mark gone the fill is the whole picture.",
            red_lead(&danger_fill, 60),
            danger_fill.len()
        );
        // And against the page it sits on, which is the claim SC 1.4.11
        // actually makes: a filled button draws no edge (see `button.rs`'s
        // "a filled button sits *on* the surface"), so the fill is the only
        // thing separating the control from its ground. `#da1e28` on
        // `surface.base` `#121212` is 3.48:1.
        let ground = inset_pixels(
            &shot,
            Rect {
                x: cam.rect("btn-danger").x - 8.0,
                y: cam.rect("btn-danger").y + 4.0,
                w: 4.0,
                h: cam.rect("btn-danger").h - 8.0,
            },
            0,
        );
        assert!(
            (luma(&danger_fill) - luma(&ground)).abs() > 30.0,
            "the danger button is luma {:.1} on a ground of luma {:.1}. \
             It draws no edge, so the fill is the whole boundary.",
            luma(&danger_fill),
            luma(&ground)
        );

        let danger_luma = luma(&danger_fill);
        for other in ["btn-primary", "btn-default", "btn-tertiary"] {
            let gap = (danger_luma - luma(&inset_pixels(&shot, fill_of(&cam, other), 0))).abs();
            assert!(
                gap > MIN_FILL_LUMA_GAP,
                "btn-danger's fill is luma {danger_luma:.1} and {other}'s is \
                 {:.1}, a gap of {gap:.1}. Under {MIN_FILL_LUMA_GAP} the two \
                 buttons are the same button to a reader with no red-green \
                 channel, and the mark this variant gave up was the thing \
                 covering that.",
                luma(&inset_pixels(&shot, fill_of(&cam, other), 0))
            );
        }

        // ---- The two unfilled variants: a glyph, red, and not a blob.

        /// Least of the mark's own box the glyph may ink. Under this the
        /// mark has gone missing or shrunk to a speck.
        const MIN_GLYPH_COVERAGE: f32 = 0.12;
        /// Most of it. A filled `shape.silhouette-octagon` inks about 0.97
        /// of its box -- that is the rounded square the operator asked to
        /// be rid of -- so this bound is what a solid figure fails.
        const MAX_GLYPH_COVERAGE: f32 = 0.70;

        for tail in ["btn-danger-tertiary", "btn-danger-ghost"] {
            assert!(
                cam.has(&format!("{tail}/mark")),
                "{tail} draws no mark. It has no fill to carry the kind, so \
                 danger here is a colour-only kind again."
            );
            let mark = cam.rect(&format!("{tail}/mark"));
            let crop = inset_pixels(&shot, mark, 0);
            let inked = red_lead(&crop, 60);
            #[allow(clippy::cast_precision_loss)]
            let coverage = inked as f32 / crop.len() as f32;
            assert!(
                coverage > MIN_GLYPH_COVERAGE,
                "{tail}'s mark inks {coverage:.2} of its own box ({inked} of \
                 {} pixels): the glyph is missing or has shrunk to a speck",
                crop.len()
            );
            assert!(
                coverage < MAX_GLYPH_COVERAGE,
                "{tail}'s mark inks {coverage:.2} of its own box ({inked} of \
                 {} pixels). A solid silhouette inks about 0.97 -- this is \
                 the rounded square again, not a glyph.",
                crop.len()
            );

            // A marked button must still fit its own label on one line.
            // `layout::stack::distribute` clips the widest child in a row
            // to the mean of the row's naturals, and a 16-unit mark is what
            // makes a danger button the widest; the first arrangement of
            // this page broke `Delete` across two lines inside a 40-unit
            // box. 20 is one line at `typography.body`.
            let caption = cam.rect(&format!("{tail}/{tail}-label"));
            assert!(
                caption.h < 24.0,
                "{tail}'s label is {} tall, so it wrapped inside a 40-unit \
                 button: the row clipped it",
                caption.h
            );

            // The luma step, which is the channel that survives protanopia.
            let fill = inset_pixels(&shot, fill_of(&cam, tail), 0);
            assert!(
                (luma(&crop) - luma(&fill)).abs() > 12.0,
                "{tail}'s mark is luma {:.1} on a fill of luma {:.1}: a \
                 reader with no red-green channel cannot see it",
                luma(&crop),
                luma(&fill)
            );
        }

        for tail in ["btn-primary", "btn-default", "btn-tertiary", "btn-ghost"] {
            assert!(
                !cam.has(&format!("{tail}/mark")),
                "{tail} grew a danger mark; the mark is what tells the two \
                 apart and it has to be absent from the safe four"
            );
        }

        // Carbon's ghost button is written in `$link-primary`
        // (`_button.scss:196`); ours was `text.primary`, the same white as
        // the default button beside it.
        let ghost = cam.rect("btn-ghost/btn-ghost-label");
        let ink = inset_pixels(&shot, ghost, 0);
        let blue = ink
            .iter()
            .filter(|p| i32::from(p[2]) - i32::from(p[0]) > 40)
            .count();
        assert!(
            blue > 20,
            "only {blue} pixels of the ghost label lean blue: Carbon draws \
             it in the link ink and this one is still page ink"
        );
    }

    /// Row 04. **A held button crunches down, and comes back when it is let
    /// go.**
    ///
    /// Round 4, row 04: *"Buttons: we need to give these an on click
    /// animation so it is like it is crunching down on the click"*.
    ///
    /// # This is a departure from Carbon and the departure is the point
    ///
    /// MEASURED `@carbon/styles/scss/components/button/_mixins.scss:72-76`:
    /// Carbon's `.cds--btn` transitions `background`, `box-shadow`,
    /// `border-color` and `outline` at `$duration-fast-01`, and nothing
    /// else. There is no `transform`, no `scale` and no press-down anywhere
    /// in `_button.scss`. What is Carbon's here is the timing — 70 ms on
    /// `motion(entrance, productive)`, the token whose own doc names
    /// buttons — and what is not is the dip.
    ///
    /// # Driven and photographed, not asserted
    ///
    /// [`Camera::click`] presses and releases inside one pass, so every
    /// picture it can take is of a button that has already come back up. A
    /// press animation is invisible to it. [`Camera::press`] holds the
    /// button down, which is what makes the middle of this test possible at
    /// all, and `04-button-pressed.png` is the frame with the finger still
    /// on it.
    ///
    /// The camera runs with reduced motion on, so what is photographed is
    /// the *settled* crunch rather than a sample a third of the way in.
    /// That is deliberate: the frame this checks is the end state, and the
    /// travel between the two is `anim::engine`'s own business and is
    /// tested there.
    ///
    /// **How this went red before the fix.** Every assertion below fails on
    /// a button with no `.with_transition(BUTTON_PRESS)`: the pressed rect
    /// is the resting rect to the last decimal, so the width never moves.
    #[test]
    fn a_pressed_button_crunches_down_and_comes_back() {
        /// `anim::shipped`'s `PRESS_INSET`, in logical units, doubled: the
        /// rect loses one inset off each edge on both axes.
        const CRUNCH: f32 = 4.0;

        let mut cam = Camera::on("Button");
        let resting = cam.rect("btn-primary");
        let resting_shot = raster(&mut cam, "04-button-resting");

        cam.press("btn-primary");
        assert_eq!(
            cam.hovered().as_deref(),
            Some(cam.id("btn-primary").as_str()),
            "the press did not land on the button, so nothing below is \
             about a pressed button"
        );
        assert!(
            cam.frame()
                .placement(&cam.id("btn-primary"))
                .is_some_and(|p| p.semantics.active),
            "the button is not `active` while held. `PointerState::pressed` \
             answered from the capture alone until this wave, and a button \
             declares Click and never Drag, so it could never be captured \
             and never be pressed -- see `input.rs`'s `down`."
        );
        let held = cam.rect("btn-primary");
        let pressed_shot = raster(&mut cam, "04-button-pressed");

        assert!(
            (resting.w - held.w - CRUNCH).abs() < 0.51,
            "the held button is {} wide against a resting {}: the crunch \
             should take {CRUNCH} off the width and it took {}",
            held.w,
            resting.w,
            resting.w - held.w
        );
        assert!(
            (resting.h - held.h - CRUNCH).abs() < 0.51,
            "the held button is {} tall against a resting {}: the crunch \
             should take {CRUNCH} off the height and it took {}",
            held.h,
            resting.h,
            resting.h - held.h
        );
        // Scaled about its own centre, not shrunk from one corner. A
        // corner-anchored dip reads as the button sliding, which is the
        // wrong picture and the easy thing to get wrong.
        assert!(
            (resting.x + resting.w / 2.0 - (held.x + held.w / 2.0)).abs() < 0.26
                && (resting.y + resting.h / 2.0 - (held.y + held.h / 2.0)).abs() < 0.26,
            "the held button's centre moved from ({}, {}) to ({}, {}): a \
             crunch scales about the centre, it does not slide",
            resting.x + resting.w / 2.0,
            resting.y + resting.h / 2.0,
            held.x + held.w / 2.0,
            held.y + held.h / 2.0
        );

        // Let go. The rect comes back **exactly**: a crunch that leaves a
        // fraction of a unit behind on every press walks the button off its
        // own row over an afternoon.
        cam.release();
        let released = cam.rect("btn-primary");
        assert_eq!(
            (released.x, released.y, released.w, released.h),
            (resting.x, resting.y, resting.w, resting.h),
            "the button did not come back to where layout put it: \
             {released:?} against {resting:?}"
        );
        let released_shot = raster(&mut cam, "04-button-released");

        // And the dip reached the picture, which is the half a rect cannot
        // answer. Held against the **released** frame rather than the
        // resting one: a press seats keyboard focus (`Host::seat_pointer_focus`),
        // so the released button legitimately carries a focus ring the
        // untouched one did not, and comparing against the first shot would
        // be measuring the ring.
        let over_button = |img: &image::RgbaImage| inset_pixels(img, resting, 0);
        let moved = differing(&over_button(&pressed_shot), &over_button(&released_shot));
        assert!(
            moved > 200,
            "only {moved} pixels over the button differ between held and \
             released: the rect crunched and the raster did not"
        );
        // Specifically the lid: the row one unit inside the resting top
        // edge is the button's own fill when it is up and the page behind
        // it when it is down.
        let lid = |img: &image::RgbaImage| {
            device_row(
                img,
                resting.x + resting.w / 2.0 - 4.0,
                resting.x + resting.w / 2.0 + 4.0,
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    ((resting.y + 0.5) * CAPTURE_SCALE).round() as u32
                },
            )
        };
        assert_ne!(
            lid(&pressed_shot),
            lid(&released_shot),
            "the top edge of the held button paints what the released one \
             paints, so the dip never reached a pixel"
        );
        // The resting shot is not decoration either: it proves the released
        // button came back to the fill it started with, so "it comes back"
        // is a claim about the picture and not only about the rect. Sampled
        // at the button's own centre-left, inside the fill and clear of
        // both the label and the focus ring under the bottom edge.
        let centre_fill =
            |img: &image::RgbaImage| px(img, resting.x + 6.0, resting.y + resting.h / 2.0);
        assert_eq!(
            centre_fill(&released_shot),
            centre_fill(&resting_shot),
            "the released button's fill is not the fill it had at rest"
        );
        // One unit inside the resting left edge is the button's fill when it
        // is up and the page behind it when it is down, because the dip is
        // two units. Read at mid-height, clear of the rounded corners.
        let edge = |img: &image::RgbaImage| px(img, resting.x + 1.0, resting.y + resting.h / 2.0);
        assert_eq!(
            edge(&resting_shot),
            centre_fill(&resting_shot),
            "the sample point is not inside the resting button at all"
        );
        assert_ne!(
            edge(&pressed_shot),
            edge(&resting_shot),
            "one unit inside the resting left edge still paints the button \
             while it is held, so the leading edge never moved"
        );

        // The four safe variants and the danger triple all carry it: the
        // operator said "buttons", not "the primary button".
        for tail in [
            "btn-default",
            "btn-tertiary",
            "btn-ghost",
            "btn-danger",
            "btn-danger-tertiary",
            "btn-danger-ghost",
            "btn-sm",
            "btn-lg",
        ] {
            let rest = cam.rect(tail);
            cam.press(tail);
            let down = cam.rect(tail);
            assert!(
                (rest.w - down.w - CRUNCH).abs() < 0.51,
                "{tail} took {} off its width on a press, not {CRUNCH}",
                rest.w - down.w
            );
            cam.release();
        }
    }

    /// Rows 24 and 37. The operator: *"popover: this is the same as toggle
    /// tip as far as I can tell just as a button?"* He was right, and the
    /// two open bubbles measured the same `#333333`.
    ///
    /// **What was wrong.** `toggletip.rs:45` called `popover_with` and then
    /// changed exactly one thing, the width cap. Fill, radius, shadow,
    /// caret, padding and layer were literally one code path, and both
    /// catalog pages photographed **closed**, so the picture of each row was
    /// one trigger and no disclosure at all.
    ///
    /// **How this went red before the fix.** Delete the `background` insert
    /// in `toggletip::bubble` and the tone assertion fails with the two
    /// bubbles reported at the same colour, which is the defect stated as a
    /// number.
    ///
    /// Both halves are driven: each page now mounts its open form at rest,
    /// so each is closed with a press and opened again with a second one
    /// before anything is measured.
    #[test]
    fn the_popover_and_the_toggletip_open_onto_two_different_bubbles() {
        let mut pop = Camera::on("Popover");
        pop.click("po-pair/pop-anchor");
        assert!(
            !pop.has("po-pair/pop-note"),
            "a press did not shut the panel"
        );
        pop.click("po-pair/pop-anchor");
        assert!(pop.has("po-pair/pop-note"), "a press did not reopen it");
        let pop_rect = pop.rect("po-pair/pop-note");
        let pop_shot = raster(&mut pop, "24-popover-open");
        let pop_fill = modal_colour(&inset_pixels(&pop_shot, pop_rect, 6));

        // A popover is the container other overlays compose on, so the page
        // has to show something living in it. Two real controls.
        for tail in [
            "pop-note/content/pop-actions/pop-cancel",
            "pop-note/content/pop-actions/pop-apply",
        ] {
            assert!(
                pop.has(tail),
                "the popover holds no interactive child, so the page still \
                 says nothing about what a popover is for. Placed:\n  {}",
                pop.ids().join("\n  ")
            );
        }

        let mut tip = Camera::on("Toggletip");
        tip.click("tt/trigger");
        assert!(!tip.has("tt/tip"), "a press did not shut the tip");
        tip.click("tt/trigger");
        assert!(tip.has("tt/tip"), "a press did not reopen it");
        let tip_rect = tip.rect("tt/tip");
        let tip_shot = raster(&mut tip, "37-toggletip-open");
        let tip_fill = modal_colour(&inset_pixels(&tip_shot, tip_rect, 6));

        let step = i32::from(tip_fill[0]) - i32::from(pop_fill[0]);
        assert!(
            step.abs() >= 8,
            "the two bubbles are {pop_fill:?} and {tip_fill:?}, {} sRGB \
             levels apart. That is the operator's complaint restated: two \
             rows, one picture",
            step.abs()
        );
        assert!(
            tip_rect.w < pop_rect.w - 40.0,
            "the tip is {} wide against the popover's {}; Carbon caps them \
             at 288 and 368",
            tip_rect.w,
            pop_rect.w
        );

        // The trigger the operator saw as the bare word "Why" now leads with
        // an information mark, which is what makes it read as a control on a
        // card its own fill disappears into.
        assert!(
            tip.has("tt/trigger/glyph"),
            "the toggletip trigger draws no mark"
        );
        let glyph = tip.rect("tt/trigger/glyph");
        let crop = inset_pixels(&tip_shot, glyph, 1);
        assert!(
            differing(&crop, &vec![crop[0]; crop.len()]) > crop.len() / 8,
            "the trigger's mark box is one flat colour: the glyph is not \
             reaching the picture"
        );
    }

    // ================================================================
    // The file seam, 2026-09-05. Row 12's other half: "should open file
    // explorer for searching too when clicked". Appended as one block.
    // ================================================================

    /// A file dropped on the window reaches the open page and the picture.
    ///
    /// This is the whole reason `Host::pass` reads `RawInput::dropped_files`
    /// rather than the `eframe` layer doing it: the hook only runs under a
    /// real window, and this test has none. It drives the seam the way the
    /// window drives it and then reads the raster, because a page that
    /// records a file it never draws is the exact defect this round kept
    /// finding.
    #[test]
    fn a_file_dropped_on_the_window_joins_the_uploader_list() {
        let mut cam = Camera::on("File uploader");
        let before = cam.shoot("12-file-uploader-before-drop");
        let rows_before = cam.ids().iter().filter(|id| id.contains("/fu-")).count();

        cam.drop_files(&["/tmp/kernel-boot.ndjson", "/tmp/second.ndjson"]);

        let after = cam.shoot("12-file-uploader-dropped");
        let rows_after = cam.ids().iter().filter(|id| id.contains("/fu-")).count();
        assert!(
            rows_after > rows_before,
            "a drop of two files left the list at {rows_before} rows: the \
             seam does not reach the page"
        );
        assert_ne!(
            before, after,
            "the dropped files reached the page and never reached the picture"
        );
        assert!(
            cam.ids().iter().any(|id| id.ends_with("fu-2")),
            "the first dropped file did not become a row"
        );
    }

    /// Focus on an expanded tree item puts the bar on its own head row.
    ///
    /// The frame-level arithmetic is pinned in `paint.rs`; what this adds is
    /// the picture, because the focus indicator is painted by the host and
    /// never enters the frame record — no assertion on placements can see
    /// it. Row 39's item `tv-gorgon` holds two open levels, so a bar on the
    /// item's own rect lands roughly 200 points down, under `inspector.rs`.
    ///
    /// Read `39-tree-view-focused.png` beside `39-tree-view.png`.
    #[test]
    fn focusing_an_expanded_tree_item_changes_what_the_page_draws() {
        let mut cam = Camera::on("Tree view");
        let before = cam.shoot("39-tree-view");

        cam.focus("tv-gorgon");
        assert_eq!(
            cam.focused().as_deref(),
            Some(cam.id("tv-gorgon").as_str()),
            "the driver did not seat focus on the item"
        );
        let after = cam.shoot("39-tree-view-focused");
        assert_ne!(
            before, after,
            "focus reached the item and no focus indicator was drawn"
        );

        let item = cam.rect("tv-gorgon");
        let head = cam.rect("tv-gorgon/row");
        assert!(
            item.h > head.h * 4.0,
            "the fixture must have two levels open: item {} high, head {} high",
            item.h,
            head.h
        );
    }

    // ===== Wave WRAP, round 4: an anchored surface wraps its body inside
    // its own ceiling. Appended; nothing above this line is touched, because
    // other waves edit this file in their own trees.

    /// Row 02. The explainability panel's body **wraps**, and the box it
    /// wraps inside is the 368 units Carbon declares, not 368 plus the
    /// panel's own padding.
    ///
    /// Two defects met on this row. `component::ai_label` built its body
    /// from a plain `text` run, and a run that does not wrap answers with
    /// one unbroken line whatever it is offered, so a two-sentence
    /// explanation lost its tail mid-word. And
    /// `layout::overlay_surface::natural_size` offered the whole ceiling to
    /// the children and let `place` add the panel's 24-unit `spacing-06`
    /// padding on top, so the box came out 416 wide — the ceiling plus the
    /// padding — and every line wrapped 48 units too late.
    ///
    /// The one-line reference is the trigger's own `"AI"` caption: at
    /// `DEFAULT_LG` it carries no style override, so it is one line of the
    /// same body typography the panel's run is set in. Nothing here is a
    /// tolerance; the run either is taller than one such line or it is not.
    #[test]
    fn the_ai_explainability_panel_wraps_its_body_inside_its_ceiling() {
        let mut cam = Camera::on("AI label");
        cam.click("ai-live/trigger");
        assert!(cam.has("panel"), "the press did not open the panel");

        let panel = cam.rect("panel");
        let run = cam.rect("panel/content/body");
        let one_line = cam.rect("ai-live/trigger/text").h;
        cam.shoot("02-ai-label-panel-wrapped");

        assert!(
            panel.w <= 368.5,
            "the panel is {} wide against the 368 its `max-inline-size` \
             declares: the padding is being added on top of the ceiling \
             instead of taken out of it",
            panel.w
        );
        assert!(
            run.h >= 2.0 * one_line,
            "the body run is {} tall against a single line's {}: it answered \
             with one unwrapped line and its tail was cut",
            run.h,
            one_line
        );
        assert!(
            run.x >= panel.x + 23.5 && run.right() <= panel.right() - 23.5,
            "the run {run:?} does not sit inside the panel's 24-unit padding \
             on both sides of {panel:?}"
        );
    }

    // ===== Wave ANCHOR: a surface docked to a viewport edge =====

    /// Carbon's toast region inset, `$spacing-05` = 16
    /// (`petra/src/token/shipped.rs:994`, `("spacing-05", 16.0)`).
    const TOAST_DOCK_INSET: f32 = 16.0;

    /// Row 21. Carbon puts a toast in a top-trailing region. `Anchor::Viewport`
    /// could only put it in the middle of the page, so it landed on top of the
    /// inline cards the row exists to show.
    ///
    /// Three claims, and the first two are pixels because the frame record
    /// could not make them. A toast overlapping a card is legal at every
    /// level the frame knows about — both are placed, both carry their
    /// semantics, both bind their tokens — and the toast paints the *same*
    /// `$layer` fill the cards do, so it does not even show up as a colour
    /// change across the middle of one. What it does do is put its own edge,
    /// its shadow and its own text through the card under it.
    ///
    /// 1. **The trailing edge of every inline card is the same picture.** The
    ///    four cards are one component at one width on one fill, laid out one
    ///    under the other, so the trailing 40 units of each must rasterize
    ///    identically to the trailing 40 of the first. Anything lying over one
    ///    of them breaks that, whatever colour it is.
    /// 2. **The window's top-trailing corner is not bare page.** The toast is
    ///    somewhere, and this says where.
    /// 3. The geometry, last: 16 down and 16 in, and no caret.
    #[test]
    fn the_toast_docks_top_trailing_and_leaves_the_inline_cards_readable() {
        let cards = ["nt-error", "nt-warning", "nt-info", "nt-success"];
        let mut cam = Camera::on("Notification");
        let shot = raster(&mut cam, "21-notification-docked");

        let strips: Vec<Vec<[u8; 4]>> = cards
            .iter()
            .map(|card| {
                let rect = cam.rect(card);
                inset_pixels(
                    &shot,
                    Rect::new(rect.right() - 40.0, rect.y, 40.0, rect.h),
                    0,
                )
            })
            .collect();
        for (card, pixels) in cards.iter().zip(&strips).skip(1) {
            assert_eq!(
                differing(pixels, &strips[0]),
                0,
                "{card}: {} of {} pixels up its trailing edge differ from \
                 {}'s, which is the same card at the same width on the same \
                 fill — something is drawn over it",
                differing(pixels, &strips[0]),
                pixels.len(),
                cards[0]
            );
        }

        let corner = inset_pixels(&shot, Rect::new(WINDOW[0] - 320.0, 0.0, 320.0, 148.0), 0);
        let bare = vec![corner[0]; corner.len()];
        assert!(
            differing(&corner, &bare) > corner.len() / 8,
            "the window's top-trailing corner is one flat colour: nothing is \
             docked there"
        );

        let toast = cam.rect("nt");
        assert!(
            (toast.y - TOAST_DOCK_INSET).abs() <= 0.5,
            "the toast is {} down from the window's top, not Carbon's 16: {toast:?}",
            toast.y
        );
        assert!(
            (WINDOW[0] - toast.right() - TOAST_DOCK_INSET).abs() <= 0.5,
            "the toast's trailing edge is {} in from the window's, not \
             Carbon's 16: {toast:?}",
            WINDOW[0] - toast.right()
        );
        assert_eq!(
            cam.caret("nt"),
            None,
            "a docked toast points at a window edge, so it draws no beak"
        );
    }

    // ------------------------------------------------------------------
    // Wave TOK, 2026-09-05: the token layer's three owed names.
    //
    // `button-danger-primary` and `text-on-color` are new; `border.subtle`
    // split into a decorative tone and a control-boundary tone. Each of the
    // three claims below is read off a rasterized page, because every one
    // of them was already legal at the frame-record level: a black toggle
    // knob binds a real token, and a divider at 5.80:1 passes every
    // contrast floor in the repo by passing them harder.
    // ------------------------------------------------------------------

    /// WCAG 2.x relative luminance of a captured pixel, alpha ignored.
    ///
    /// The pixels here come off an opaque rasterized page, so there is no
    /// alpha to composite; `gorgon_petra::token::ColorValue` is not reachable
    /// from a `[u8; 4]` without a conversion that would be longer than this.
    fn pixel_luminance(p: [u8; 4]) -> f32 {
        let chan = |v: u8| {
            let c = f32::from(v) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.212_672_9 * chan(p[0]) + 0.715_152_2 * chan(p[1]) + 0.072_175 * chan(p[2])
    }

    /// The WCAG contrast ratio between two captured pixels.
    fn pixel_contrast(a: [u8; 4], b: [u8; 4]) -> f32 {
        let (x, y) = (pixel_luminance(a), pixel_luminance(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Walk `dy` down a one-pixel column and return the pixel that differs
    /// most in luminance from `ground` — the drawn line, whatever row the
    /// painter snapped it to.
    ///
    /// A hairline at device pixel ratio 2 lands on one of two rows and may
    /// be feathered across both, so sampling a single named row is how a
    /// working rule gets reported as missing. This takes the strongest
    /// pixel in a short window instead, which is the line if there is one
    /// and the ground if there is not.
    fn strongest_line_pixel(
        img: &image::RgbaImage,
        x: f32,
        y: f32,
        rows: u32,
        ground: [u8; 4],
    ) -> [u8; 4] {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (dx, dy0) = (
            (x * super::CAPTURE_SCALE) as u32,
            (y * super::CAPTURE_SCALE) as u32,
        );
        (dy0.saturating_sub(rows)..=(dy0 + rows).min(img.height() - 1))
            .map(|dy| img.get_pixel(dx.min(img.width() - 1), dy).0)
            .max_by(|a, b| {
                pixel_contrast(*a, ground)
                    .partial_cmp(&pixel_contrast(*b, ground))
                    .expect("contrast ratios are finite")
            })
            .expect("the window has at least one row")
    }

    /// Row 36. The on-toggle's handle is **white**, and the off-toggle's is
    /// not, in the dark theme the catalog opens in.
    ///
    /// # The defect
    ///
    /// `controls.rs` bound `TEXT_ON_ACCENT` for the on state. That token is
    /// the theme's own `surface.base`, so in dark it resolves to `#121212`
    /// and row 36 drew a **black knob** inside a blue track where Carbon's
    /// is white. Carbon's handle is `background-color: $icon-on-color` on
    /// `.cds--toggle__switch::before` with no `--checked` override (SOURCED
    /// `.agents/research/08-25-2026/Carbon-Component-Inventory/slice-f.md:26`),
    /// so it is white in both states. Wave D recorded it and left it for the
    /// token layer; `text-on-color` is that name.
    ///
    /// # Why the frame record could not see it
    ///
    /// Because the black knob was *correct* at every level above the pixel.
    /// The node existed, it was keyed `knob`, it was pinned to Carbon's 18
    /// units, it bound a declared colour token, and that token was even the
    /// right one for the ink on an accent **fill**. Nothing but the picture
    /// distinguishes "the ink that goes on the accent" from "the mark that
    /// rides the accent", and this reads the picture.
    ///
    /// # How this went red
    ///
    /// Restore `TEXT_ON_COLOR` to `TEXT_ON_ACCENT` in `controls.rs`'s knob
    /// and the first assertion reports the dark knob's channels.
    ///
    /// Driven, not composed: the on state under test is reached by pressing
    /// the off toggle, so this is a live page rather than a constructor.
    #[test]
    fn switching_a_toggle_on_gives_it_carbons_white_handle() {
        /// Every channel a white handle must reach. `#ffffff` rasterizes to
        /// 255s; 240 leaves room for the painter's own rounding without
        /// admitting `text.on-accent`'s `#121212` or `text.primary`'s
        /// `#f2f2f2`-at-a-glance — the latter is 242, which is why the
        /// second assertion below reads the *off* knob rather than trusting
        /// this bound to separate them.
        const WHITE_FLOOR: u8 = 240;

        let mut cam = Camera::on("Toggle");

        // The resting off knob first, so the pair is measured on one page.
        let off_knob = cam.rect("toggle-default-off/appearance/track/knob");
        let resting = raster(&mut cam, "36-toggle-before-press");
        // The handle is a full pill, so a square crop has to fit inside the
        // circle rather than inside the bounding box: 18 logical units is 36
        // device pixels at `CAPTURE_SCALE`, radius 18, and an inset of 8
        // leaves a 20x20 square whose half-diagonal is 14.1. An inset of 4
        // leaves 19.8, and the four corners then sample the accent track --
        // which is how this test first reported a white knob as 24 blue
        // pixels.
        let off = inset_pixels(&resting, off_knob, 8);
        let off_tone = off[0];

        cam.click("toggle-default-off");
        let on_knob = cam.rect("toggle-default-off/appearance/track/knob");
        let shot = raster(&mut cam, "36-toggle-pressed-on");
        let on = inset_pixels(&shot, on_knob, 8);

        let dark = on
            .iter()
            .filter(|p| p[0] < WHITE_FLOOR || p[1] < WHITE_FLOOR || p[2] < WHITE_FLOOR)
            .count();
        assert_eq!(
            dark,
            0,
            "{dark} of {} pixels of the switched-on handle are under \
             {WHITE_FLOOR} on some channel (first: {:?}). Carbon's handle is \
             $icon-on-color, white in both states; a handle painted in \
             `text.on-accent` is `#121212` in this theme and reads as a hole \
             in the track.",
            on.len(),
            on.iter()
                .find(|p| p[0] < WHITE_FLOOR)
                .copied()
                .unwrap_or([0, 0, 0, 0])
        );

        // The track really is the accent underneath it, so the rect under
        // test is the handle and not some white the page already had.
        let track = cam.rect("toggle-default-off/appearance/track");
        let track_tone = px(&shot, track.x + track.w - 2.0, track.y + track.h / 2.0);
        assert!(
            u32::from(track_tone[2]) > u32::from(track_tone[0]) + 60,
            "the switched-on track is not the accent: {track_tone:?}; the \
             handle above was measured against the wrong ground"
        );

        // And the off handle is a different tone: `text.primary` in dark is
        // `#f2f2f2`, which would pass a naive "is it light" check, so the
        // claim is that the two states differ rather than that one is dark.
        assert_ne!(
            off_tone, on[0],
            "the handle is one tone in both states, so the on state is not \
             saying anything the off state does not"
        );
    }

    /// Rows 31 and 34. **A row rule is quiet and a field rule is a
    /// boundary**, measured on the two pages side by side.
    ///
    /// # What this is testing
    ///
    /// The 2026-09-05 border split, at the only level that can see it. One
    /// tone used to do both jobs and it was pinned at WCAG SC 1.4.11's 3:1
    /// because one of the two is a checkbox outline. Wave B measured the
    /// consequence against the Carbon reference at the same dpr: our row
    /// rule was `#9c9c9c` (156) where Carbon's is `#393939` (57), and our
    /// field rule `#b8b8b8` (184) where Carbon's is `#6f6f6f` (111). Every
    /// table, divider, field underline and panel edge in the catalog
    /// inherited it, and it is why the pages read as a wireframe.
    ///
    /// So the assertion is a **band**, and both ends matter:
    ///
    /// - A divider must clear [`MIN_DIVIDER`], because a rule nobody can see
    ///   is a missing rule and the operator is red-green colour blind — two
    ///   greys within about three of 255 are invisible to everybody.
    /// - A divider must stay **under** [`CONTROL_FLOOR`]. This is the half
    ///   that catches the regression: the old tone measures 5.80:1 against
    ///   the card and fails no floor in the repo, it passes every one of
    ///   them by a factor of two, which is exactly why no existing test
    ///   could have caught it.
    /// - A field's bottom rule must clear [`CONTROL_FLOOR`], because that
    ///   rule *is* the field's boundary — Carbon draws no box — and SC
    ///   1.4.11 is about the information that identifies a component.
    ///
    /// # How this went red
    ///
    /// Point `light_border`/`dark_border` in `token::shipped` back at the
    /// control floor (`MIN_CONTROL_BOUNDARY` instead of
    /// `MIN_DIVIDER_CONTRAST`) and the structured list's rule is reported at
    /// 5.80:1, over the ceiling. Point `dark_border_strong` at the divider
    /// floor and the text field's rule is reported under 3:1.
    #[test]
    fn a_row_rule_is_a_quiet_divider_and_a_field_rule_is_a_control_boundary() {
        /// WCAG 2.1 SC 1.4.11 *Non-text Contrast*, Level AA.
        const CONTROL_FLOOR: f32 = 3.0;
        /// The floor a decorative rule is held to, mirroring
        /// `token::shipped`'s `MIN_DIVIDER_CONTRAST`. Carbon's own g100 row
        /// rule (`$border-subtle-01` `#393939` on `$layer-01` `#262626`)
        /// measures 1.31:1; the painter feathers a hairline across two
        /// device rows at dpr 2, so the pixel this reads can land a little
        /// under the token's own value and 1.2 is the fence that allows for
        /// that without allowing a missing rule.
        const MIN_DIVIDER: f32 = 1.2;

        // --- the decorative side: a structured list's row rule -----------
        let mut list = Camera::on("Structured list");
        let row = list.rect("sl-1");
        let shot = raster(&mut list, "31-structured-list-rule");
        // Well inside the row, below any rule on its top edge.
        let ground = px(&shot, row.x + row.w / 2.0, row.y + row.h / 2.0);
        let rule = strongest_line_pixel(&shot, row.x + row.w / 2.0, row.y, 2, ground);
        let ratio = pixel_contrast(rule, ground);
        assert!(
            ratio >= MIN_DIVIDER,
            "the structured list's row rule measures {ratio:.2}:1 against the \
             row it separates ({rule:?} on {ground:?}), under {MIN_DIVIDER}:1. \
             A divider a reader cannot find is a missing divider."
        );
        assert!(
            ratio < CONTROL_FLOOR,
            "the structured list's row rule measures {ratio:.2}:1 against the \
             row it separates ({rule:?} on {ground:?}), at or over the \
             {CONTROL_FLOOR}:1 control-boundary floor. A hairline between two \
             table rows identifies no component, and holding it to that floor \
             is what made every page in this catalog read as a wireframe -- \
             Carbon's own rule is 1.31:1."
        );

        // --- the control side: a text field's bottom rule ----------------
        let mut field = Camera::on("Text input");
        let well = field.rect("field-md");
        let shot = raster(&mut field, "34-text-input-rule");
        let fill = px(&shot, well.x + well.w / 2.0, well.y + well.h / 2.0);
        let under = device_row(
            &shot,
            well.x + 4.0,
            well.x + well.w - 4.0,
            bottom_device_row(well),
        );
        let rule = under
            .iter()
            .copied()
            .max_by(|a, b| {
                pixel_contrast(*a, fill)
                    .partial_cmp(&pixel_contrast(*b, fill))
                    .expect("contrast ratios are finite")
            })
            .expect("the rule row is not empty");
        let ratio = pixel_contrast(rule, fill);
        assert!(
            ratio >= CONTROL_FLOOR,
            "the text field's bottom rule measures {ratio:.2}:1 against the \
             well it closes ({rule:?} on {fill:?}), under the \
             {CONTROL_FLOOR}:1 SC 1.4.11 floor. Carbon draws no box around a \
             field, so this rule is the whole boundary: at this contrast \
             there is no field on the page, only a grey patch."
        );
    }

    /// Every card on row 21, inline and toast alike.
    const NOTE_CARDS: [&str; 5] = [
        "nt-error",
        "nt-warning",
        "nt-info",
        "nt-success",
        "nt/panel",
    ];

    /// Carbon's leading inset to a notification's status glyph:
    /// `padding-inline-start: convert.to-rem(13px)`
    /// (`_toast-notification.scss:35`) plus the `border-inline-start: 3px`
    /// status rail (`_mixins.scss:35`) this library does not draw, which is
    /// 16 — `$spacing-05`, on the ramp.
    const NOTE_LEAD: f32 = 16.0;
    /// Carbon's notification glyph, `size: 20` (`Notification.js:152`).
    const NOTE_GLYPH: f32 = 20.0;
    /// Carbon `__icon { margin-inline-end: $spacing-05 }`
    /// (`_inline-notification.scss:281`).
    const NOTE_GAP: f32 = 16.0;

    /// Row 21. The glyph hangs on the card's leading edge and the text is a
    /// left-aligned column beside it, which is Carbon's whole anatomy for
    /// this component: `display: flex` with the icon first, then
    /// `__text-wrapper` (`_inline-notification.scss:29,253-266`).
    ///
    /// The card centred every line until 2026-09-05, so the glyph's leading
    /// inset was a function of the *title's length* — measured 58.66, 72.98,
    /// 80.62 and 91.80 on the four inline cards of one page, four different
    /// places for the same field — and the message sat on a third edge again.
    /// That is the "icons and the way it is layed out" the operator named.
    #[test]
    fn every_notification_kind_hangs_its_glyph_on_the_card_leading_edge() {
        let mut cam = Camera::on("Notification");
        for card in NOTE_CARDS {
            let panel = cam.rect(card);
            let glyph = cam.rect(&format!("{card}/glyph"));
            let title = cam.rect(&format!("{card}/details/title"));
            let body = cam.rect(&format!("{card}/details/body"));
            assert!(
                (glyph.x - panel.x - NOTE_LEAD).abs() <= 0.5,
                "{card}: the glyph is {} in from the card's edge, not {NOTE_LEAD}: \
                 {glyph:?} in {panel:?}",
                glyph.x - panel.x
            );
            assert!(
                (glyph.y - panel.y - NOTE_LEAD).abs() <= 0.5,
                "{card}: the glyph is {} down from the card's top, not {NOTE_LEAD}",
                glyph.y - panel.y
            );
            assert!(
                (title.x - panel.x - (NOTE_LEAD + NOTE_GLYPH + NOTE_GAP)).abs() <= 0.5,
                "{card}: the title starts {} in, not {}",
                title.x - panel.x,
                NOTE_LEAD + NOTE_GLYPH + NOTE_GAP
            );
            assert!(
                (body.x - title.x).abs() <= 0.5,
                "{card}: the message starts at {} and the title at {}; \
                 Carbon's `__text-wrapper` gives them one leading edge",
                body.x,
                title.x
            );
        }
        assert!(
            !cam.has("nt/panel/rail"),
            "the accent rail is back; the operator refused it twice"
        );
        cam.shoot("21-notification-glyph-edges");
    }

    /// Row 16, the operator's round-4 line: *"List: we need several types of
    /// bullet points like word or emacs org mode has"*
    /// (`.agents/carbon-waves/ROUND4-DEFECTS.md` R7).
    ///
    /// Four claims on the placed frame, and a capture that has to be read:
    ///
    /// 1. Every unordered marker is a **canvas**, not a text leaf. That is
    ///    what makes it a drawn mark rather than a character riding the
    ///    label's font, and it is the thing a rect cannot show.
    /// 2. The four levels of the rotating chain draw **four different**
    ///    pictures. Same reason `the_notification_kinds_draw_four_different_
    ///    marks` reads pixels: this operator cannot use hue, so the shape is
    ///    the whole channel.
    /// 3. Nesting is real to four levels — each level's marker starts
    ///    further in than the one above it.
    /// 4. Every marker sits in its own 16-unit column with its label at the
    ///    column's end, which is the hanging indent `list.rs`'s module doc
    ///    measures off Carbon's own `16-list.png`.
    #[test]
    fn the_list_page_nests_four_drawn_bullets_and_indents_every_level() {
        let mut cam = Camera::on("List");
        let shot = raster(&mut cam, "16-list");

        let levels = ["rot-1", "rot-2", "rot-3", "rot-4"];
        let mut lefts = Vec::new();
        let mut marks = Vec::new();
        for key in levels {
            let row = format!("{key}/row");
            let marker = if cam.has(&row) {
                cam.rect(&format!("{row}/marker"))
            } else {
                cam.rect(&format!("{key}/marker"))
            };
            let label = if cam.has(&row) {
                cam.rect(&format!("{row}/label"))
            } else {
                cam.rect(&format!("{key}/label"))
            };
            assert!(
                (marker.w - 16.0).abs() <= 0.5,
                "{key}: the marker column is {}, not Carbon's 16 hang",
                marker.w
            );
            assert!(
                (label.x - (marker.x + marker.w)).abs() <= 0.5,
                "{key}: the label starts at {} and the column ends at {}",
                label.x,
                marker.x + marker.w
            );
            lefts.push(marker.x);
            marks.push(inset_pixels(&shot, marker, 0));
        }

        for (level, pair) in lefts.windows(2).enumerate() {
            assert!(
                pair[1] > pair[0] + 8.0,
                "level {} starts at {} and level {} at {}: the nesting is \
                 not indented",
                level + 1,
                pair[0],
                level + 2,
                pair[1]
            );
        }
        for (i, a) in marks.iter().enumerate() {
            for (j, b) in marks.iter().enumerate().skip(i + 1) {
                assert!(
                    differing(a, b) > 8,
                    "levels {} and {} draw the same mark, so a rotation of \
                     four is a rotation of one",
                    i + 1,
                    j + 1
                );
            }
        }
    }

    // ===== Wave FIGURE, round 5: three focus figures, the bar's shadow
    // (R6) and the tree row's width (R8). Appended; nothing above this line
    // is touched, because other waves edit this file in their own trees.

    /// R6, measured in the theme it showed up in.
    ///
    /// The operator, 2026-09-05: *"Light mode showed that the shadow on the
    /// cursor is broken. It looks really bad in light mode. Like there are
    /// just 2 lines instead of a shadow."*
    ///
    /// The bar is `FocusRing::thickness` (3) units tall and used to cast
    /// `shadow.overlay`, whose offset is `[0, 4]`. Four units of drop under a
    /// three-unit bar means the shadow's own pixels never touch the bar's:
    /// the card shows through between them and the pair reads as two lines.
    /// It now casts `focus::BAR_SHADOW_TOKEN` (`shadow.raised`, offset
    /// `[0, 2]`), and two units of drop under three units of bar overlap.
    ///
    /// Read as a column of pixels straight down from the bar's centre. A
    /// shadow is darkest where it meets the thing casting it and fades from
    /// there; two lines are darkest somewhere below a gap. So:
    ///
    /// 1. the darkest row under the bar is the first row under the bar, and
    /// 2. darkness never rises again on the way down.
    ///
    /// # How this goes red
    ///
    /// Point `focus::BAR_SHADOW_TOKEN` back at `"shadow.overlay"`. Claim 1
    /// fails: the peak moves several rows down and the rows just under the
    /// bar come back near the card.
    ///
    /// # Why a fixture
    ///
    /// This used to be photographed on a tab. `component::tabs` declared
    /// `BarUnder` on the strength of `_tabs.scss:596`, which is the
    /// selection rule and not the focus rule, so the picture was of a
    /// mis-assigned figure. Tabs now ring, and the bar is photographed on a
    /// node built to wear one.
    #[test]
    fn the_bar_under_casts_a_shadow_and_not_a_second_line_in_light_mode() {
        // A fixture and not a tab: after the tab citation was corrected no
        // shipped component wears `BarUnder`, and mis-assigning one to keep
        // this photograph is exactly the move that produced the defect.
        let mut cam = Camera::fixture(&[("bar", FocusFigure::BarUnder)], "light");
        cam.focus("bar");
        assert!(
            cam.ring().is_some_and(|id| id.ends_with("bar")),
            "the driver did not seat focus on the subject"
        );
        let tab = cam.rect("bar");
        let img = raster(&mut cam, "fixture-bar-under-shadow-light");

        let ring = FocusRing::STANDARD;
        let bar = ring.bar(tab);
        let ground = px(&img, bar.x + bar.w / 2.0, bar.y + bar.h + 14.0);
        assert!(
            pixel_luminance(ground) > 0.5,
            "this must be the light theme; the ground under the bar reads \
             {ground:?}"
        );

        // Every device row from the first one clear of the bar down to the
        // shadow's full reach (offset 2 + blur 6 = 8 logical units).
        let mid_x = bar.x + bar.w / 2.0;
        let first = ((bar.y + bar.h) * CAPTURE_SCALE).ceil() as u32 + 1;
        let last = ((bar.y + bar.h + 8.0) * CAPTURE_SCALE) as u32;
        let column: Vec<(u32, f32)> = (first..last)
            .map(|dy| {
                let p = img.get_pixel((mid_x * CAPTURE_SCALE) as u32, dy).0;
                (dy, pixel_luminance(p))
            })
            .collect();
        assert!(
            column.len() >= 8,
            "the sampling window is too short to say anything: {column:?}"
        );

        let darkest = column
            .iter()
            .copied()
            .min_by(|a, b| a.1.partial_cmp(&b.1).expect("luminances are finite"))
            .expect("the column is not empty");
        assert_eq!(
            darkest.0,
            column[0].0,
            "the shadow's darkest row is {} device rows below the bar, not \
             touching it: the card shows through between the two and they \
             read as two lines. Column (device row, luminance): {column:?}",
            darkest.0 - column[0].0
        );

        let mut prev = column[0].1;
        for &(dy, luma) in &column[1..] {
            assert!(
                luma >= prev - 0.002,
                "the shadow gets darker again at device row {dy} \
                 ({prev:.4} -> {luma:.4}), which is a second line and not a \
                 fade. Column: {column:?}"
            );
            prev = luma;
        }
        assert!(
            column[0].1 < pixel_luminance(ground) - 0.02,
            "there is no shadow at all under the bar: first row \
             {:.4} against ground {:.4}",
            column[0].1,
            pixel_luminance(ground)
        );
    }

    /// R8, measured: the ring on a tree item's head row spans the same band
    /// the selection fill covers.
    ///
    /// The operator, 2026-09-05: *"In tree view the cursor doesn't position
    /// itself well on any of these elements"*, and after the round-4 ring
    /// went in, *"still misaligned"*.
    ///
    /// The item is a `Grid` with a `Weight` column and the fill is bound on
    /// the item, so the highlighted band is full width. The head row inside
    /// that cell settled at its content width until `Align::Stretch` went on
    /// the grid, so every figure drawn on the row — and the item declares
    /// `FocusShownOn::OnHead`, so all of them are — stopped short of it.
    ///
    /// Measured on the frame, not by eye: the placed row and the placed item
    /// must share both edges. Then photographed, because the frame record
    /// cannot show the indicator at all — the host paints it.
    ///
    /// # How this goes red
    ///
    /// Drop `align: Some(Align::Stretch)` from `tree_item_sized`'s grid.
    #[test]
    fn a_tree_rows_indicator_spans_the_band_the_selection_fills() {
        for theme in ["dark", "light"] {
            let mut cam = Camera::on("Tree view");
            // The theme walk goes to row 27 and clicks, which moves focus, so
            // it has to happen before focus is seated rather than between the
            // two photographs.
            if theme == "light" {
                cam.light();
            }
            cam.focus("tv-gorgon");
            assert!(
                cam.ring().is_some_and(|id| id.ends_with("tv-gorgon")),
                "{theme}: the driver did not seat focus on the item"
            );
            let item = cam.rect("tv-gorgon");
            let head = cam.rect("tv-gorgon/row");
            assert_eq!(
                head.x, item.x,
                "{theme}: the head row starts right of the band the fill covers"
            );
            assert_eq!(
                head.x + head.w,
                item.x + item.w,
                "{theme}: the head row stops {} short of the band's right \
                 edge, so the indicator drawn on it does too",
                (item.x + item.w) - (head.x + head.w)
            );
            cam.shoot(&format!("39-tree-view-border-{theme}"));
        }
    }

    /// One photograph of each figure, in each theme. Six pictures, and the
    /// point of the test is that a person opens them.
    ///
    /// The assertion is deliberately weak — focus landed and the picture
    /// moved — because a frame-record assertion cannot see the indicator at
    /// all and a strong one here would only be theatre. `paint.rs` pins the
    /// arithmetic; the two tests above pin the two defects. This exists so
    /// the three figures are on disk side by side.
    #[test]
    fn every_figure_is_photographed_in_both_themes() {
        // Two figures have a shipped component that declares them, and one
        // does not. `Sides` is every field well; `Border` is the default and
        // every contained control. `BarUnder` has no component after the tab
        // citation was corrected — the operator asked for three figures by
        // name and "bar under" is one of them, so it stays in the
        // vocabulary and gets photographed on a node built to wear it,
        // rather than on a control talked into it.
        for (page, tail, name) in [
            ("Search", "query/input", "28-search-sides"),
            ("Button", "btn-primary", "04-button-border"),
        ] {
            for theme in ["dark", "light"] {
                let mut cam = Camera::on(page);
                if theme == "light" {
                    cam.light();
                }
                let before = cam.shoot(&format!("{name}-{theme}-rest"));
                cam.focus(tail);
                assert!(
                    cam.ring().is_some_and(|id| id.ends_with(tail)),
                    "{page}/{theme}: the driver did not seat focus on {tail}"
                );
                let after = cam.shoot(&format!("{name}-{theme}"));
                assert_ne!(
                    before, after,
                    "{page}/{theme}: focus reached {tail} and nothing was drawn"
                );
            }
        }
        for theme in ["dark", "light"] {
            // Two nodes, and the bar is the second. The host spawns focus on
            // the first focusable node of a fresh page, so a one-node
            // fixture is already focused before the driver touches it and a
            // "before" frame would be no such thing.
            let mut cam = Camera::fixture(
                &[
                    ("park", FocusFigure::Border),
                    ("bar", FocusFigure::BarUnder),
                ],
                theme,
            );
            let bar_rect = cam.rect("bar");
            let before = raster(&mut cam, &format!("fixture-bar-under-{theme}-rest"));
            cam.focus("bar");
            assert!(
                cam.ring().is_some_and(|id| id.ends_with("bar")),
                "fixture/{theme}: the driver did not seat focus on the subject"
            );
            let after = raster(&mut cam, &format!("fixture-bar-under-{theme}"));

            // Probe the bar's own pixels rather than compare whole files: a
            // byte difference anywhere in a 2400x1800 capture would pass
            // even if the bar were never drawn.
            let bar = FocusRing::STANDARD.bar(bar_rect);
            let at = (bar.x + bar.w / 2.0, bar.y + bar.h / 2.0);
            let rest = px(&before, at.0, at.1);
            let lit = px(&after, at.0, at.1);
            assert_ne!(
                rest, lit,
                "fixture/{theme}: the middle of the bar's own rect {bar:?} \
                 reads {rest:?} whether or not the subject has focus, so no \
                 bar was drawn"
            );
        }
    }

    /// The worst case for a tab, photographed rather than argued about: the
    /// tab that is **selected** is also the one focused, so its own accent
    /// indicator and the focus ring are the same colour.
    ///
    /// `$focus` and `$border-interactive` are both `#0f62fe` in the white
    /// and g10 themes, so the two marks cannot be told apart by hue. What
    /// tells them apart is **place**: the indicator owns the Line tab's
    /// bottom edge, and the focus figure must therefore appear somewhere
    /// that edge is not. This reads the raster where the figure lives and
    /// requires accent there that the unfocused frame does not have.
    ///
    /// A Line tab takes the bar *below* it (`FocusFigure::BarUnder`) since
    /// 2026-09-06. It rang, then bracketed for a day, and the claim is
    /// unchanged by either move: a ring answered "not the bottom edge" with
    /// its other three, brackets answered it by standing outside the tab,
    /// and the bar answers it by hanging two units clear underneath. What
    /// moves each time is where this test looks, not what it asserts.
    ///
    /// The brackets went because the operator photographed them clipped —
    /// the strip clips its own height, so a bar standing `hug_gap` outside a
    /// tab is cut off top and bottom.
    ///
    /// # How this goes red
    ///
    /// Put `FocusFigure::BarInside` on `tab_variant`'s Line arm and the probe
    /// stops changing, because that stripe sits *on* the bottom edge — the
    /// one edge the indicator already owns, which is the whole defect this
    /// test exists to catch.
    #[test]
    fn a_selected_tab_focused_shows_its_ring_around_its_indicator() {
        for theme in ["dark", "light"] {
            let mut cam = Camera::on("Tabs");
            if theme == "light" {
                cam.light();
            }
            let tab = cam.rect("tab-line-0");
            let before = raster(&mut cam, &format!("32-tabs-selected-{theme}-rest"));
            cam.focus("tab-line-0");
            assert!(
                cam.ring().is_some_and(|id| id.ends_with("tab-line-0")),
                "{theme}: the driver did not seat focus on the selected tab"
            );
            let indicator = cam.rect("tab-line-0/indicator");
            let after = raster(&mut cam, &format!("32-tabs-selected-focused-{theme}"));

            // The indicator is the bottom edge of a Line tab; the bar hangs
            // `gap` below that edge, clear of it. Sample the middle of the
            // bar's own row, which is where the figure is and where the
            // indicator can never be.
            let ring = FocusRing::STANDARD;
            let run = marked_run(cam.frame(), "tab-line-0");
            let y = tab.bottom() + ring.gap + ring.thickness / 2.0;
            let rest = px(&before, run.x + run.w / 2.0, y);
            let lit = px(&after, run.x + run.w / 2.0, y);
            assert_ne!(
                rest, lit,
                "{theme}: the row two units under the tab reads {rest:?} both \
                 before and after focus, so focus has nowhere to show that \
                 the selection indicator does not already own"
            );
            assert!(
                y > indicator.bottom(),
                "{theme}: the bar's row {y} is not clear of the indicator \
                 {indicator:?}, so this probe cannot tell them apart"
            );

            // And the indicator is still its own mark under all that: it is
            // pinned to the bottom edge, which is where a Line tab puts it.
            assert!(
                indicator.bottom() >= tab.bottom() - 0.01,
                "{theme}: the indicator {indicator:?} is not on the tab's \
                 bottom edge {tab:?}, so this test is probing the wrong place"
            );
        }
    }

    /// The morph, photographed: a mid-flight frame of every figure pair,
    /// both ways.
    ///
    /// The caret is four springs, one band per edge, so a change of figure
    /// is a change of where those four sit and never a swap.
    /// `focus_caret.rs`'s `every_pair_of_figures_morphs_band_by_band` holds
    /// that per band and per frame; this is the picture, because a frame
    /// record cannot show the indicator at all.
    ///
    /// Driven with motion live and stepped a frame at a time. `Camera::on`
    /// reduces motion so a shot taken after a click shows the settled
    /// result; a test about a transition must not.
    ///
    /// # Why a fixture and not catalog pages
    ///
    /// Three figures make three unordered pairs, six with direction. No
    /// catalog row carries two different figures — after the tab correction
    /// no row carries `BarUnder` at all — so on the catalog only
    /// `Sides <-> Border` can be photographed, and round 5 shipped with
    /// `BarUnder <-> Sides` unphotographed. A fixture holding one node per
    /// figure photographs all six and owes no component anything.
    ///
    /// # How this goes red
    ///
    /// Make `FocusCaret::retarget` snap instead of spring and
    /// `host_caret_is_moving` is false on the frame after the hop starts.
    #[test]
    fn each_figure_pair_is_photographed_in_flight() {
        let subjects = [
            ("bar", FocusFigure::BarUnder),
            ("sides", FocusFigure::Sides),
            ("border", FocusFigure::Border),
        ];
        for (i, (from, _)) in subjects.iter().enumerate() {
            for (to, _) in subjects.iter().skip(i + 1).map(|s| (s.0, s.1)) {
                for (a, b, way) in [(*from, to, "out"), (to, *from, "back")] {
                    let mut cam = Camera::fixture(&subjects, "dark");
                    cam.focus(a);
                    assert!(
                        cam.ring().is_some_and(|id| id.ends_with(a)),
                        "{a}->{b}: the driver did not seat focus on {a}"
                    );
                    cam.live_motion();
                    cam.focus(b);
                    assert!(
                        cam.host_caret_is_moving(),
                        "{a}->{b}: the hop never started, so there is no \
                         morph to photograph"
                    );
                    // Two frames in: the springs have visibly left `a` and a
                    // 160 ms hop is about ten frames, so they cannot have
                    // arrived.
                    cam.tick();
                    cam.tick();
                    assert!(
                        cam.host_caret_is_moving(),
                        "{a}->{b}: the caret settled before the photograph"
                    );
                    cam.shoot(&format!("fixture-{from}-to-{to}-{way}-mid"));
                }
            }
        }
    }

    /// The operator, dragging down the nested lists on row 16: *"when I try
    /// to select text on the list I can only select 1 element. A lot are like
    /// this."*
    ///
    /// One gesture, seventeen runs, four levels of nesting, two lists. The
    /// clipboard is asserted whole rather than by fragments: the shape of the
    /// answer — one line per line, indented by nesting depth, ordered markers
    /// carried along because they are painted text — is the part that would
    /// rot silently, and a `contains` check would not see it go.
    ///
    /// The unordered markers are **not** in the string, and that is the
    /// honest limit: Carbon's `::before` bullets are drawn shapes here
    /// (`component/list.rs`: *"a typed marker cannot be sized, centred or
    /// snapped"*), so there is no text to copy. The operator asked for
    /// markdown *"if we can"*; this is as far as "can" reaches without
    /// inventing syntax the picture does not contain.
    #[test]
    fn a_drag_down_a_nested_list_takes_every_line_and_its_indent() {
        let mut cam = Camera::on("List");
        let first = cam.rect("kinds/lists/ul/ul-1/row/label");
        let last = cam.rect("kinds/lists/ol/ol-2/label");
        let resting = raster(&mut cam, "16-list-unselected");
        cam.drag_at(
            Point::new(first.x + 1.0, first.y + first.h / 2.0),
            Point::new(last.x + last.w - 1.0, last.y + last.h / 2.0),
        );
        let shot = raster(&mut cam, "16-list-span-selected");

        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(
            cam.clipboard().last().map(String::as_str),
            Some(concat!(
                "- Inbox\n  - Archive\n    - 2026\n      - March\n",
                "- Disc\n  - Ring\n    - Square\n      - Dash\n",
                "- Fixed square\n- at every level\n",
                "1. Clone\n2. Build\n  a. Compile\n  b. Link\n",
                "    i. Static\n    ii. Dynamic\n3. Run"
            )),
            "one line per line, two spaces per level below the first, every \
             drawn bullet as Markdown's `-` whatever mark it drew, and each \
             ordered marker left as the counter the picture shows, joined to \
             its label by the one space the layout put between them"
        );

        // And the picture agrees with the string, on runs from three
        // different depths of the two lists.
        for tail in [
            "kinds/lists/ul/ul-1/row/label",
            "kinds/lists/ul/ul-1/ul-l2/ul-2/ul-l3/ul-3/row/label",
            "kinds/lists/ol/ol-1/ol-l2/ol-1-0/label",
        ] {
            let run = cam.rect(tail);
            let (band, count) = new_tone(&resting, &shot, run);
            assert!(count > 40, "{tail}: no band was painted behind the glyphs");
            assert!(
                u32::from(band[2]) > u32::from(band[0]) + 60,
                "{tail}: the band is {band:?} rather than the accent fill"
            );
        }
    }

    /// The operator, on the same report: *"links stop copying too"*.
    ///
    /// A link is an `<a>`, so its words are content. The span here runs
    /// straight through one — `"Read "`, the link, `" when a link sits in
    /// running text."` are three separate runs on one line — and the three
    /// come back as one sentence with no seam and no double space, because
    /// the runs already carry their own.
    #[test]
    fn a_drag_through_running_text_carries_the_link_with_it() {
        let mut cam = Camera::on("Link");
        let first = cam.rect("links/link-row/prose/before");
        let last = cam.rect("links/link-row/prose/after");
        let resting = raster(&mut cam, "15-link-unselected");
        cam.drag_at(
            Point::new(first.x + 1.0, first.y + first.h / 2.0),
            Point::new(last.x + last.w - 1.0, last.y + last.h / 2.0),
        );
        let shot = raster(&mut cam, "15-link-span-selected");

        let link = cam.rect("links/link-row/prose/docs-inline");
        let range = cam
            .selection("links/link-row/prose/docs-inline")
            .expect("the link in the middle of the span carries no highlight");
        assert_eq!(
            cam.painted_text("links/link-row/prose/docs-inline")[range],
            *"the inline form",
            "a run in the middle of a span is selected whole"
        );
        let (band, count) = new_tone(&resting, &shot, link);
        assert!(count > 40, "the link's own words got no band");
        assert!(
            u32::from(band[2]) > u32::from(band[0]) + 60,
            "the band is {band:?} rather than the accent fill"
        );

        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(
            cam.clipboard().last().map(String::as_str),
            Some("Read the inline form when a link sits in running text."),
            "one line, one space at each seam, and the link's words in it"
        );
    }

    /// The operator, on row 13: *"text is not highlightable in form, which is
    /// a text entry box"* and *"check boxes need their text highlightable"*.
    ///
    /// Both in one gesture, because they are one page. The field is a
    /// `NodeKind::Input`, which paints its own value rather than hanging a
    /// run under it, and the checkbox's caption is a `<label>` beside the
    /// control rather than inside it.
    #[test]
    fn a_drag_down_a_form_takes_the_field_and_the_checkbox_label() {
        let mut cam = Camera::on("Form");
        let first = cam.rect("form/form-body/demo-form/legend");
        let last = cam.rect("form/form-body/demo-form/form-ok/label");
        let field = cam.rect("form/form-body/demo-form/name-item/form-name");
        let resting = raster(&mut cam, "13-form-unselected");
        cam.drag_at(
            Point::new(first.x + 1.0, first.y + first.h / 2.0),
            Point::new(last.x + last.w - 1.0, last.y + last.h / 2.0),
        );
        let shot = raster(&mut cam, "13-form-span-selected");

        assert_eq!(
            cam.selection("form/form-body/demo-form/name-item/form-name")
                .map(
                    |range| cam.painted_text("form/form-body/demo-form/name-item/form-name")[range]
                        .to_owned()
                ),
            Some("fiber-7".to_owned()),
            "the entry box's own value is text like any other"
        );
        // The band sits where the letters are, twelve units in from the
        // chrome — `paint::text_inset_x`. A highlight measured at the full
        // width would sit beside them.
        let inked = Rect::new(field.x + 12.0, field.y, field.w - 24.0, field.h);
        let (band, count) = new_tone(&resting, &shot, inked);
        assert!(count > 40, "the field's value got no band");
        assert!(
            u32::from(band[2]) > u32::from(band[0]) + 60,
            "the band is {band:?} rather than the accent fill"
        );

        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(
            cam.clipboard().last().map(String::as_str),
            Some("Fiber\nName\nfiber-7\nEnabled"),
            "the legend, the field's label, the value inside the box, and \
             the checkbox's caption"
        );
    }

    /// The operator, on the whole of it: *"I have to start my drag basically
    /// on top of the text"*.
    ///
    /// Two presses that land on no letters at all: one on the drawn bullet in
    /// the marker column, one ninety units past the end of the word. Before
    /// the snap both pressed on nothing, cleared the selection, and the drag
    /// that followed did not exist.
    ///
    /// The two answers differ, and the difference is the point. A press to
    /// the *left* of a run anchors at its start and the word is in the span;
    /// a press to the *right* anchors at its end and the word is behind the
    /// caret. That is a browser's rule for a click in a margin, and it falls
    /// out of the offset the shaper gives for a position past the last glyph
    /// rather than being written anywhere.
    #[test]
    fn a_press_beside_the_words_still_anchors_in_them() {
        let last = "kinds/lists/ul/ul-1/ul-l2/ul-2/ul-l3/ul-3/ul-l4/ul-4/label";

        let mut cam = Camera::on("List");
        let first = cam.rect("kinds/lists/ul/ul-1/row/label");
        let bullet = cam.rect("kinds/lists/ul/ul-1/row/marker");
        let end = cam.rect(last);
        let on_the_bullet = Point::new(bullet.x + bullet.w / 2.0, bullet.y + bullet.h / 2.0);
        assert!(
            !first.contains(on_the_bullet),
            "the press has to land off the glyphs or this proves nothing"
        );
        cam.drag_at(
            on_the_bullet,
            Point::new(end.x + end.w - 1.0, end.y + end.h / 2.0),
        );
        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(
            cam.clipboard().last().map(String::as_str),
            Some("- Inbox\n  - Archive\n    - 2026\n      - March"),
            "a press on the bullet snapped rightwards onto the word beside it"
        );

        let mut cam = Camera::on("List");
        let beside = Point::new(first.x + first.w + 90.0, first.y + first.h / 2.0);
        assert!(!first.contains(beside));
        cam.drag_at(beside, Point::new(end.x + end.w - 1.0, end.y + end.h / 2.0));
        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(
            cam.clipboard().last().map(String::as_str),
            Some("  - Archive\n    - 2026\n      - March"),
            "a press past the end of the line anchored there, so the line \
             itself is behind the caret and the three under it are not. \
             `Inbox` keeps no bullet because it keeps no bytes: an empty \
             range is dropped, and the three lines that are taken whole each \
             lead with the mark their item drew"
        );
    }

    /// A span that crosses a control leaves the control's own words behind,
    /// which is what a browser's `user-select: none` does on a form control.
    ///
    /// Driven on a real page rather than on a fixture, because the thing that
    /// could break it is a component's declaration and not the resolver: the
    /// four controls this span crosses are the catalog's own Prev, Next, Dark
    /// and Light, and every one of them still owns its label.
    #[test]
    fn a_span_across_the_page_leaves_the_buttons_labels_alone() {
        let mut cam = Camera::on("List");
        let title = cam.rect("main/title");
        let last = cam.rect("kinds/lists/ul/ul-1/row/label");
        cam.drag_at(
            Point::new(title.x + 1.0, title.y + title.h / 2.0),
            Point::new(last.x + last.w - 1.0, last.y + last.h / 2.0),
        );
        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        let copied = cam.clipboard().last().cloned().unwrap_or_default();
        assert!(
            copied.starts_with("16  List") && copied.ends_with("Inbox"),
            "the span ran from the page title to the first list item, got \
             {copied:?}"
        );
        for label in ["Prev", "Next", "Dark", "Light"] {
            assert!(
                !copied.contains(label),
                "{label:?} is a button's own chrome and must not be in \
                 {copied:?}"
            );
            assert!(
                cam.selection(&format!(
                    "nav/{}",
                    match label {
                        "Prev" => "prev/prev-label",
                        "Next" => "next/next-label",
                        "Dark" => "theme/theme-dark/label",
                        _ => "theme/theme-light/label",
                    }
                ))
                .is_none(),
                "{label:?} is painted with a highlight it is not in the \
                 clipboard for"
            );
        }
    }

    /// The page the operator photographed: *"I should be able to drag any
    /// here and copy from multiple elements. By default obv."*
    ///
    /// Nine runs of four different kinds — a heading, a helper line, five file
    /// names each sitting beside its own remove button, and an error message
    /// under one of them — come out as nine lines in the order they are read.
    ///
    /// The drop zone's prompt is **not** among them, and that is a decision
    /// rather than an oversight. Carbon's drop container is a `<label>`, so
    /// the rule the rest of this change follows would lend its words out. It
    /// keeps the opt-out because the press it acts on opens a **system file
    /// dialog**: every other control this change opened up costs a stray drag
    /// a toggle or a page change, and this one costs an OS window the
    /// application cannot dismiss. One sentence from the operator flips it.
    #[test]
    fn a_drag_down_the_uploader_takes_every_row_but_not_the_drop_zone() {
        let mut cam = Camera::on("File uploader");
        let first = cam.rect("fu/heading");
        let last = cam.rect("fu-body/fu-bad/message");
        cam.drag_at(
            Point::new(first.x + 1.0, first.y + first.h / 2.0),
            Point::new(last.x + last.w - 1.0, last.y + last.h / 2.0),
        );
        cam.chord(
            KeyCode::Char('c'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(
            cam.clipboard().last().map(String::as_str),
            Some(concat!(
                "Upload a trace\n",
                "NDJSON or YAML, up to 5 MB\n",
                "trace.ndjson\nfiber-dump.ndjson\nkernel.yaml\nnotes.txt\n",
                "core.dump\nFile is over 5 MB"
            )),
            "every run the span crosses, in reading order, one per line"
        );
        assert!(
            cam.selection("fu/zone/prompt").is_none(),
            "the drop zone opens a system file dialog on the press, so it \
             keeps its own words"
        );
    }
}
