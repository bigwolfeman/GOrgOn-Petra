//! The eframe host: one pass per frame, and no frames at idle.
//!
//! The loop is deliberately small, because everything interesting happens in
//! `gorgon-petra`: translate this pass's platform events, route them through
//! the real input path, ask the application for a view tree, petrify it, paint
//! the result, and schedule the next frame only if something is still moving.
//!
//! egui's `Ui` is never constructed. The only egui surfaces used are a raw
//! layer painter, the font stack behind [`crate::text::GalleyShaper`], and the
//! event stream.
//!
//! # Where keyboard reachability lives (FR-025)
//!
//! [`Host`] owns the one [`FocusTree`] a running window has. Each pass
//! reconciles it with the frame just placed
//! ([`FocusTree::update`] — the vanished-focus rule) and writes the result to
//! `LayoutState::focused`, which is what [`gorgon_petra::input::route`] aims
//! every keyboard event with — and what
//! [`gorgon_petra::frame::PlacementSemantics::focused`] is projected from.
//! [`crate::focus_caret::FocusCaret`] interpolates the underline between
//! those targets; Tab and Shift+Tab are consumed here for traversal rather
//! than delivered to the application; everything else is routed. That is the
//! whole of the wiring, and it is deliberately in the host: `gorgon-petra`
//! decides *what* focus does, this crate decides *when*.
//!
//! # Where the pointer lives (`contracts/interaction-state.md` §7)
//!
//! [`Host`] owns the one [`PointerState`] a running window has, beside the
//! focus tree and for the same reason: two facts — where the pointer is
//! between events, and which node grabbed it — cannot be recovered from a
//! frame, and `gorgon-petra` deliberately owns nothing that outlives one.
//! Each pass reconciles it with the frame just placed
//! ([`PointerState::reconcile`] — the vanished-capture rule and the hover
//! re-derivation) and writes the result to `LayoutState::hovered`,
//! `::pressed` and `::capture`, which is what
//! [`gorgon_petra::layout::semantics_of`] projects the five interaction-state
//! flags from.
//!
//! `hover` is **derived here and nowhere else**: an application that
//! re-derived it from a `Route::Pointer` would be running a second hit test
//! by a second owner, and the two drift the first time a surface overlaps
//! something (FR-009, R-A §7(a)).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use egui::{Context, Id, LayerId, Order};
use gorgon_petra::anim::{FrameDecision, TransitionRegistry, wants_frame};
use gorgon_petra::component::registry;
use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{
    FrameCounter, PetrifiedFrame, Placement, TransitionActivity, Viewport, petrify,
};
use gorgon_petra::geom::{Axis, Point, Rect, Scale, Size};
use gorgon_petra::input::{
    InputEvent, KeyCode, PointerButton, PointerRouting, PointerState, Route, RouteOutcome,
    TextSelection,
};
use gorgon_petra::layout::overlay_surface::{focus_taking_surfaces, surface_scopes};
use gorgon_petra::layout::{
    AnchorRects, ChangeSet, LayoutCtx, LayoutState, MeasureCache, RowSource, ScrollStack,
    TextRequest,
};
use gorgon_petra::semantic::{
    ContributionId, ContributionLedger, ContributionStatus, PushOutcome, contribution_key,
};
use gorgon_petra::token::value::CoverageValue;
use gorgon_petra::token::{
    DesignToken, Presenter, StatusToken, Theme, ThemeSnapshot, TokenKind, TokenName, TokenValue,
    Vocabulary, standard_vocabulary,
};
use gorgon_petra::tree::{
    InputPolicy, InsetRefs, Interaction, NodeKind, Props, Registry, TextWrap, ValidatedTree,
    ViewNode, validate,
};

use crate::focus_caret::FocusCaret;
use crate::image::ImageSources;
use crate::input::EventTranslator;
use crate::paint::{
    CaretOverlay, CaretPicture, CustomPainters, PaintReport, caret_bands, caret_clip_limit,
    focused_caret_target, paint_caret_overlay, paint_frame_with_caret,
};
use crate::schedule::FrameMotion;
use crate::text::{FontFaces, GalleyShaper, Typography};

/// The egui layer every Petra frame paints into.
///
/// One layer, not one per Petra z-band: Petra already put the placements in
/// paint order, and egui adds shapes to a layer in the order they arrive. Two
/// orderings would be one too many.
#[must_use]
pub fn petra_layer() -> LayerId {
    LayerId::new(Order::Background, Id::new("gorgon-petra"))
}

/// What a Petra application supplies to the host.
///
/// Deliberately three methods. State lives in the application (D-075), the
/// view tree is plain data, and the host neither owns nor inspects the state
/// — it only needs to know what changed, which is what `take_changes` is
/// for: [`Host::pass`] hands the answer straight to
/// [`gorgon_petra::layout::MeasureCache::apply`], which invalidates exactly
/// what the [`ChangeSet`] names, and every ancestor of it — an application
/// that returns [`ChangeSet::All`] because it has not been taught to track
/// ids is exactly as correct as, and exactly as slow as, invalidating
/// wholesale every frame. The method *takes* rather than reads: a change set
/// is drained by the frame that consumes it, so an application that reports
/// the same change twice pays twice, not that it goes unnoticed.
pub trait App: RowSource {
    /// The host's clock for the pass about to build a view, in seconds.
    ///
    /// The same `egui::InputState::time` the transition scheduler advances
    /// on, handed to the application before [`App::view`] on every pass
    /// that builds one. An application whose picture is a function of time
    /// — a spinner, a live plot — reads its phase from here and rebuilds
    /// the canvas that draws it, which is `contracts/draw-list.md` §8's rule
    /// for such a canvas: resubmit a new list each frame and declare
    /// `ambient`. No other clock reaches an application: an `Instant` read
    /// inside `view` would make two hosts disagree about when now is, and a
    /// driver could not step it.
    ///
    /// The default ignores the clock, for every application that draws
    /// nothing time-dependent.
    fn tick(&mut self, now: f64) {
        let _ = now;
    }
    /// The view tree for this frame.
    fn view(&mut self) -> ViewNode;
    /// Handle one routed input event.
    ///
    /// `frame` is the frame `route` was computed against — the placements
    /// the hit test walked, with their rects. A route names a node; a
    /// gesture needs geometry: a `PointerMoved` delivered to a slider's
    /// handle under capture is a window position, and the value it names is
    /// that position along the *rail's* rect, which the route does not
    /// carry and the handle's own rect cannot give. Every drag an
    /// application will ever turn into a value goes through this argument
    /// (`gorgon_petra::component::slider_value_at` is the first). It is the
    /// whole frame rather than the routed node's rect because the rect an
    /// application needs is rarely the one that was hit.
    ///
    /// `None` exactly once: on the first pass, before any frame has been
    /// placed, when `route` is `Unrouted("no frame has been placed yet")`.
    /// Those events are still delivered rather than dropped — a silent drop
    /// is how a "the click did nothing" bug hides — and there is nothing
    /// to hand them with, so the type says so. A `Pointer` or `Keyboard`
    /// route always comes with `Some`.
    ///
    /// Read-only. The frame is the host's; an application that wants a
    /// different picture answers differently from [`App::view`].
    fn handle(&mut self, event: &InputEvent, route: &Route, frame: Option<&PetrifiedFrame>);
    /// What changed behind [`App::view`] since the last call, draining it in
    /// the same motion. Must name (directly or via [`ChangeSet::All`]) every
    /// node whose measured content this frame would answer differently for.
    fn take_changes(&mut self) -> ChangeSet;
    /// Close every surface named in `ids`, each an
    /// [`InputPolicy::DismissOutside`] surface a press landed outside of.
    ///
    /// The engine is retained and the application owns the view tree, so the
    /// host cannot close anything itself — it can only report the request and
    /// let the next [`App::view`] answer differently. Called before
    /// [`App::handle`] sees the press that caused it, which is the order a
    /// user perceives: the menu shuts, then the click lands on whatever was
    /// behind it.
    ///
    /// The default is to ignore dismissals, for the applications that declare
    /// no `DismissOutside` surface and would otherwise be made to write an
    /// empty method.
    fn dismissed(&mut self, ids: &[String]) {
        let _ = ids;
    }
    /// Keyboard focus moved to `focused`, or to nothing when `None`.
    ///
    /// Focus is the host's (`contracts/interaction-state.md` §1): the focus
    /// tree owns it, the painter draws its ring from the published
    /// projection, and an application never asks for it. What an
    /// application does own is the tree, and some of the tree is a fact
    /// about focus — a Carbon tooltip is revealed on focus as well as on
    /// hover, and the bubble is a node only the application can mount. No
    /// routed event carries "focus arrived": Tab never reaches
    /// [`App::handle`], and a click that seats focus is routed as a click.
    /// So the host reports the move here, from the one place it publishes
    /// it, and the application answers on its next [`App::view`].
    ///
    /// Called once per change, never per pass, and only after the move is
    /// already the host's own state: reading focus back through the
    /// projection during this call returns `focused`.
    ///
    /// The default ignores it, for the applications whose trees do not
    /// depend on where focus is — which is most of them.
    fn focus_changed(&mut self, focused: Option<&str>) {
        let _ = focused;
    }

    /// A theme the application wants published, taken and cleared.
    ///
    /// Asked once at the top of every [`Host::pass`], before anything reads
    /// the registry, so the tree this pass builds is validated and resolved
    /// against the theme the application just asked for rather than the one
    /// before it.
    ///
    /// The host publishes rather than the application because the
    /// [`Presenter`] lives here: an application that held its own handle to
    /// it could publish mid-frame, between the validation and the resolve,
    /// which is the one window `rebind_theme_if_stale` exists to close.
    ///
    /// The default asks for nothing, which is every application whose theme
    /// is chosen outside it.
    fn theme_request(&mut self) -> Option<Theme> {
        None
    }

    /// Text the application wants put on the system clipboard, taken and
    /// cleared.
    ///
    /// Asked once per pass, right after the input for that pass has been
    /// delivered, so a press handled this pass copies on this pass.
    ///
    /// The host asks because the clipboard is the *window's*, not the
    /// application's: `egui::Context::copy_text` is the only path to it and
    /// an application that held a `Context` would be holding the host's own
    /// handle to the backend. A Copy button with nowhere to put its string is
    /// how catalog row 6 shipped a control that looked like it worked.
    ///
    /// The default copies nothing.
    fn clipboard_request(&mut self) -> Option<String> {
        None
    }

    /// A host-clock time this application wants another pass at, in the
    /// seconds [`App::tick`] is handed.
    ///
    /// Asked once per pass, right after the clock is delivered. `None` means
    /// "ask me nothing", which is every application that draws nothing on a
    /// deadline.
    ///
    /// # Why this exists, and why it is not `ambient`
    ///
    /// `contracts/animation.md` §"Frame scheduling" lists four reasons a
    /// frame is scheduled: input, a change set, a viewport change, and
    /// motion. A picture that is a function of the clock but is **not**
    /// moving is none of the four, and it had no way to ask. Catalog row 6's
    /// copy feedback is the first: it says `"Copied!"` for two seconds and
    /// then stops, and nothing about it is a transition — Carbon's is a DOM
    /// node with a `setTimeout` on it, not an animation
    /// (`@carbon/react/lib/components/Copy/Copy.js:37`).
    ///
    /// Without this the message appears on the pass that handled the press
    /// and stays up until the operator's *next* input, whatever it is and
    /// whenever it comes, because an idle Petra window paints nothing (SC-002).
    /// [`crate::tree::ViewNode::with_ambient`] would keep the frames coming,
    /// but `ambient` means *deliberately endless* and is excluded from
    /// settle: a driver waiting for this window to settle would be told it
    /// already had while a two-second message was still counting down.
    ///
    /// A deadline is neither, so it is its own answer: one frame, at a stated
    /// time, and then the window goes back to sleep. SC-002 is intact because
    /// the pass that wakes finds the deadline passed and asks for nothing
    /// more.
    ///
    /// A time already gone asks for the next frame the pump can give. A time
    /// that is not finite is ignored rather than passed to
    /// `std::time::Duration::from_secs_f64`, which would panic.
    ///
    /// **One honest limit.** A pass that reuses the last frame to fly the
    /// focus caret returns before the clock is delivered, so it does not ask
    /// this either. That pass only happens while the caret is moving, which
    /// is already requesting frames of its own, and the first pass after it
    /// re-arms the deadline.
    fn wake_at(&mut self) -> Option<f64> {
        None
    }

    /// Files the window is handing the application, by path.
    ///
    /// One door, two sources. A file dropped on the window arrives here, and
    /// so does a file the operator picked from the system dialog
    /// ([`App::file_request`]) — the application cannot tell them apart, and
    /// should not need to: both mean "the operator chose this file".
    ///
    /// **Paths, never bytes and never a handle.** `egui` delivers a drop as
    /// an `Arc<dyn DroppedFile>` that can read its own contents; letting that
    /// cross would put host I/O inside an application and make
    /// `gorgon-petra`'s tree depend on a file system. A path is a name, and
    /// naming is all this seam is for. An application that wants the bytes
    /// opens the path itself, where its own error handling lives.
    ///
    /// Read in [`Host::pass`] rather than in the `eframe` layer, for
    /// [`App::clipboard_request`]'s reason turned around: the `eframe` hook
    /// only runs under a real window, and the testkit driver steps
    /// `Host::pass` directly. A drop read anywhere else could never be tested
    /// headlessly, and an untested seam is one that works until the day it is
    /// needed.
    fn files_dropped(&mut self, paths: &[std::path::PathBuf]) {
        let _ = paths;
    }

    /// A system file dialog the application wants opened, taken and cleared.
    ///
    /// Asked once per pass right after [`App::clipboard_request`], and for
    /// the same reason: the press that produced the request has already been
    /// delivered this pass, so a click on the drop zone opens the dialog on
    /// the pass that handled the click.
    ///
    /// The host answers through [`App::files_dropped`], not through a return
    /// value, because a dialog is not a function call — the operator may
    /// browse for a minute, or cancel. `None` is the answer for "no dialog",
    /// and cancelling is the answer that never arrives.
    fn file_request(&mut self) -> Option<FilePick> {
        None
    }
}

/// What an application wants a system file dialog to ask for.
///
/// Deliberately small. What Carbon's `FileUploader` exposes that a dialog can
/// honour is here (`accept`, `multiple`) and nothing that it cannot: there is
/// no "start in this directory", because the XDG portal does not promise to
/// honour one and a field a host may silently ignore is a field that lies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilePick {
    /// Whether the operator may choose more than one file.
    pub multiple: bool,
    /// Extensions to offer, without the dot — `["ndjson", "json"]`. Empty
    /// offers every file. This is Carbon's `accept` prop, which is a hint to
    /// the dialog and never a guarantee: a portal may hand back a path
    /// outside the list, so an application still has to check what it got.
    pub extensions: Vec<String>,
}

/// The paths egui took from a drop this pass, in the order the window gave
/// them.
///
/// `RawInput::dropped_files` is a list of `Arc<dyn DroppedFile>`, and a
/// handle can read its own bytes. Only the path crosses — see
/// [`App::files_dropped`] — the handle itself, and the bytes it could read,
/// stay on this side of it.
fn dropped_paths(ctx: &Context) -> Vec<std::path::PathBuf> {
    ctx.input(|input| {
        input
            .raw
            .dropped_files
            .iter()
            .map(|file| file.path().to_path_buf())
            .collect()
    })
}

/// Point egui's glyph rasteriser at the coverage curve its own documentation
/// names for `mode`.
///
/// # Why this is a correctness fix and not a tuning knob
///
/// epaint rasterises every glyph once, white, into one atlas, and the shader
/// multiplies that coverage by the run's colour in gamma space
/// (`egui.wgsl`: `in.color * tex_gamma`). One atlas cannot be right for both
/// dark-on-light and light-on-dark, so epaint bends the coverage on the way
/// in and says so itself (`epaint/src/image.rs`): *"This whole thing is less
/// than rigorous. It would be better to either render all text colors into
/// the font atlas … or do the color compensation in the shader."*
///
/// The bend is [`FontColorTransferFunction`], and it used to have a
/// hardcoded answer per `ThemeMode` — `Off` for light, `TwoCoverageMinusCoverageSq`
/// for dark — because those are epaint's own documented per-mode defaults.
/// That shipped in `86de1a6` and it was **harmful**: it fixed light mode's
/// curve at the identity (`n = 1`), which is the *most* variance-prone
/// setting on the per-character weight variance defect this text pipeline
/// port exists to close (see the port README), not the least. Both curve
/// *and* paint-repeat count are now a function of one token,
/// `text.coverage-curve` ([`CoverageValue`]), read off the theme — not of
/// `ThemeMode` — so light and dark ship identically unless a theme
/// deliberately diverges them.
///
/// # Why this still needs writing at all
///
/// epaint rasterises every glyph once, white, into one atlas, and the shader
/// multiplies that coverage by the run's colour in gamma space
/// (`egui.wgsl`: `in.color * tex_gamma`). One atlas cannot be right for both
/// dark-on-light and light-on-dark, so epaint bends the coverage on the way
/// in and says so itself (`epaint/src/image.rs`): *"This whole thing is less
/// than rigorous. It would be better to either render all text colors into
/// the font atlas … or do the color compensation in the shader."*
///
/// # The atlas curve and the paint count multiply
///
/// `2c - c² ≡ 1 - (1-c)²`, so `TwoCoverageMinusCoverageSq` *is* two-pass
/// compositing, and painting a galley `k` times under egui-wgpu's
/// premultiplied source-over composites to `1 - (1-a)^k`
/// (`crate::paint`'s galley loop). `effective_n = n_atlas · repeats`, which
/// is exactly what [`coverage_plan`] computes and what [`crate::paint`]
/// spends. This function only writes the atlas half of that pair — the
/// repeat half is read straight off the same token, independently, by
/// [`crate::paint::TokenSource::coverage`], so the two halves can never
/// read two different published theme revisions.
///
/// # Why both egui themes are written
///
/// egui keeps a `Style` per *its own* theme and `Options::style()` picks by
/// that, which this crate never sets. Writing only the active one would
/// leave a host that later calls `Context::set_theme` reading a stale curve.
/// Writing the one field into both means Petra's published theme decides the
/// curve whatever egui thinks its own theme is, and takes nothing else from
/// an application that wants to own `Visuals` — which is the narrowest
/// version of this that works.
///
/// No atlas clear is needed: `Fonts::begin_pass` compares the incoming
/// `TextOptions` against the ones the atlas was built with and recreates it
/// when they differ, so writing the field is the whole of the change.
fn bind_glyph_coverage(ctx: &Context, theme: &Theme) {
    let plan = coverage_plan(coverage_value(theme));
    for egui_theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(egui_theme, |style| {
            style.visuals.text_options.color_transfer_function = plan.curve;
            style.visuals.text_options.subpixel_binning = !plan.snap;
        });
    }
}

/// The token name every coverage-curve lookup reads, spelled once so
/// [`bind_glyph_coverage`] and [`crate::paint::TokenSource::coverage`]
/// cannot drift onto two different strings.
pub(crate) const COVERAGE_TOKEN: &str = "text.coverage-curve";

/// The pair a theme predating [`COVERAGE_TOKEN`] falls back to: `passes:
/// 2.0` is exactly what both shipped themes' curves computed to before this
/// token existed (dark ran `TwoCoverageMinusCoverageSq` at one paint, light
/// ran `Off` at one paint under the old mode-keyed match this superseded —
/// `n = 2` and `n = 1` respectively — and `2.0` is the higher, honestly
/// neutral one of the two, not a third number nobody measured), so an
/// application that hands this host an older vocabulary gets the more
/// variance-resistant of the two priors rather than either theme's old
/// number by accident of which mode it happened to be in. `snap: false`
/// matches every theme this workspace has ever shipped.
pub const FALLBACK_COVERAGE: CoverageValue = CoverageValue {
    passes: 2.0,
    snap: false,
};

/// `theme`'s `text.coverage-curve`, or [`FALLBACK_COVERAGE`] when the theme's
/// vocabulary predates the token (an application built its own theme against
/// an older [`gorgon_petra::token::standard_vocabulary`] snapshot) or the
/// name resolves to some other token kind (a theme bug `Theme::build` would
/// have refused had this been declared at the wrong kind — defensive here
/// regardless, because this function must never panic on a hand-built
/// `Theme`).
pub fn coverage_value(theme: &Theme) -> CoverageValue {
    let name = TokenName::new(COVERAGE_TOKEN)
        .expect("\"text.coverage-curve\" is a well-formed token name");
    match theme.value(&name) {
        Some(TokenValue::Coverage(value)) => *value,
        _ => FALLBACK_COVERAGE,
    }
}

/// What one [`CoverageValue`] resolves to on the paint path: the atlas curve
/// [`bind_glyph_coverage`] writes onto the `Context`, and the paint-repeat
/// count and fractional-pass alpha [`crate::paint`] reads to repeat the
/// galley emission.
///
/// Public because it is the *only* authority on that mapping, and an
/// inspection tool that re-derived it would be a second authority able to
/// disagree — silently, and in exactly the artifact a human is looking at to
/// judge the real thing. `tests/text_size_probe.rs` is the caller this was
/// opened for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoveragePlan {
    /// The curve the glyph atlas rasterises through.
    pub curve: egui::epaint::FontColorTransferFunction,
    /// How many whole times to paint the galley.
    pub repeats: u8,
    /// `0.0` for no further pass; otherwise the fraction of the text
    /// colour's alpha to paint one further time, ai-macs' fractional-pass
    /// model ported verbatim (SPEC.md §1.4, §2.3).
    pub fraction: f32,
    /// Whether a glyph's horizontal origin snaps to a whole device pixel.
    /// Carried straight through from [`CoverageValue::snap`] — see that
    /// field's doc comment for the (inverted, `bool`-vs-`bool`, no
    /// compile-time guard) mapping onto
    /// [`egui::TextOptions::subpixel_binning`].
    pub snap: bool,
}

/// `passes → (atlas curve, paint count, fractional-pass alpha)`, the
/// contract table `SPEC.md` §6.3/§6.4 pins:
///
/// | `passes` | atlas curve | paints | effective n |
/// |---|---|---|---|
/// | 1.0 | `Off` | 1 | 1 |
/// | 2.0 | `TwoCoverageMinusCoverageSq` | 1 | 2 |
/// | 3.0 | `Off` | 3 | 3 |
/// | 4.0 | `TwoCoverageMinusCoverageSq` | 2 | 4 |
///
/// # The trap
///
/// `2c - c² ≡ 1 - (1-c)²`: `TwoCoverageMinusCoverageSq` *is* two-pass
/// compositing, so the atlas curve and the paint count MULTIPLY —
/// `effective_n = n_atlas · repeats`. `passes = 2.0` deliberately spends the
/// atlas curve rather than two paints (identical output, one fewer draw);
/// `passes = 4.0` spends it twice. Every other value — every odd integer,
/// and every fractional value, including one whose floor is even — goes
/// through `Off` instead, so the fractional pass below always composites
/// against raw coverage and stays exactly ai-macs' own formula rather than
/// needing a second, curve-scaled fractional term. Painting `TwoCov` three
/// times would be `n = 6`, not `n = 3`; no row of this table, and no value
/// this function returns for any `passes` in the legal `[1.0, 4.0]` range,
/// pairs `TwoCov` with three paints (`two_cov_at_three_paints_would_be_n_six_and_no_row_produces_it`).
pub fn coverage_plan(value: CoverageValue) -> CoveragePlan {
    use egui::epaint::FontColorTransferFunction;

    /// Float wobble tolerance for recognising the two exact anchor points a
    /// theme value most likely lands on. Well under any daylight between two
    /// legal `passes` values a theme would ever assign on purpose.
    const EPS: f32 = 1e-4;

    let (curve, repeats, fraction) = if (value.passes - 2.0).abs() < EPS {
        (
            FontColorTransferFunction::TwoCoverageMinusCoverageSq,
            1,
            0.0,
        )
    } else if (value.passes - 4.0).abs() < EPS {
        (
            FontColorTransferFunction::TwoCoverageMinusCoverageSq,
            2,
            0.0,
        )
    } else {
        let whole = value.passes.floor().max(1.0);
        let frac = value.passes - whole;
        (
            FontColorTransferFunction::Off,
            whole as u8,
            if frac > 0.001 { frac } else { 0.0 },
        )
    };
    CoveragePlan {
        curve,
        repeats,
        fraction,
        snap: value.snap,
    }
}

/// One surface a plugin contributed, as the shell holds it.
///
/// The shell-side peer of `gorgond`'s `Contribution`, and deliberately not
/// the same type. `petra-egui` depends on nothing under `gorgon/`: linking
/// the daemon into the renderer to reach one struct would put a ctl client,
/// a kernel and a Lua host inside the process that draws. So the daemon's
/// fiber id, row path and trace sequence stop at the process boundary, and
/// what crosses is the three fields a picture needs. Spec 004's shell binary
/// depends on both crates and is where the two are joined.
///
/// [`Contribution::tree`] is an **owned snapshot**, which is the whole of
/// `contracts/surface-contribution.md` §8: the frame path holds a value, so
/// there is nothing for it to call and no timeout to tune. A fiber that has
/// stopped pushing leaves the last snapshot on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Contribution {
    /// The daemon's contribution id.
    ///
    /// Becomes the key `ui:<id>` the subtree is mounted under, which is what
    /// makes every node beneath it attributable to its owning fiber through
    /// the path alone (`contracts/surface-contribution.md` §9). No owner
    /// field on any node, and no join.
    pub id: ContributionId,
    /// The publisher's revision of *this contribution*, which must not go
    /// backwards for one id.
    ///
    /// [`ContributionLedger::accept`] drops a push whose revision is not
    /// newer than the one on screen, so an out-of-order arrival cannot
    /// replace a newer picture with an older one. A publisher that reused or
    /// decremented a revision would instead have its new tree silently
    /// ignored — so the obligation is stated here rather than assumed.
    pub revision: u64,
    /// The key of the node in the application's own tree this mounts under.
    ///
    /// A mount slot, not a paint slot ([`gorgon_petra::token::SlotSchema`]):
    /// this names a place in the tree, that names a place on a rect.
    pub slot: String,
    /// The plugin's tree as the daemon accepted it, component references and
    /// all. The shell expands it; see [`Host::set_contributions`].
    pub tree: ViewNode,
}

/// What the shell did with one [`Contribution`] on the last frame it built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MountOutcome {
    /// Expanded, accepted by stage-2 `validate`, and spliced into its slot.
    Mounted,
    /// Expansion or stage-2 acceptance refused it, and the attributed error
    /// card is on screen in its place, in its own slot. The string is what
    /// the card says.
    Refused(String),
    /// No node in the application's tree carries that key, so there is
    /// nowhere to mount. The card goes to the root instead when the root can
    /// hold children; when it cannot, this report is the only record.
    NoSuchSlot,
    /// More than one node carries that key, so "the" slot does not exist.
    /// Mounting under the first one found would put a plugin's surface
    /// somewhere neither the plugin nor the shell author chose, so the shell
    /// refuses instead of guessing. Carries how many were found.
    AmbiguousSlot(usize),
}

/// One line of [`Host::mounts`]: what happened to one contribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountReport {
    /// The contribution's id.
    pub id: ContributionId,
    /// The slot it asked for.
    pub slot: String,
    /// What the shell did with it.
    pub outcome: MountOutcome,
}

/// A contribution after expansion and stage-2 acceptance, ready to splice.
///
/// Prepared once per change to the contribution set or to the registry, not
/// once per frame: expanding 161 registry rows and walking the result through
/// `validate` at 60 Hz would be paid on every frame for a set that changes
/// when a plugin loads. See [`Host::prepare_contributions`].
struct Prepared {
    id: ContributionId,
    slot: String,
    /// What is spliced: the expanded, accepted subtree re-keyed to `ui:<id>`,
    /// or the error card that stands in its place.
    node: ViewNode,
    /// `Some` exactly when `node` is the error card, carrying its message.
    refused: Option<String>,
}

/// Drives one Petra application inside an `eframe` window.
pub struct Host<A: App> {
    app: A,
    /// The one egui context this host is bound to for its life.
    ///
    /// The shaper already holds a clone and has since it was built, so this
    /// records a dependency that existed rather than adding one. It is here
    /// because [`Host::bind_theme`] has to reach egui — the glyph atlas's
    /// coverage curve is a function of the theme's `text.coverage-curve`
    /// token (see that method) — and it is reached from call sites that
    /// have no `&Context` to pass.
    ctx: Context,
    shaper: GalleyShaper,
    translator: EventTranslator,
    cache: MeasureCache,
    counter: FrameCounter,
    state: LayoutState,
    presenter: Presenter,
    registry: Registry,
    /// Token names this application declares beyond the ones its theme
    /// defines, kept apart from `registry` because the registry's vocabulary
    /// is re-derived from the theme on every publication and would otherwise
    /// take them with it.
    extra_vocabulary: Vocabulary,
    /// The theme revision `registry`'s vocabulary and `shaper`'s typography
    /// were derived from. `Presenter::publish` takes `&self` and can be
    /// called from anywhere, so nothing tells the host a theme changed; this
    /// is what lets [`Host::pass`] notice.
    bound_revision: u64,
    /// Which egui font family draws each weight class, spent every time the
    /// typography map is rebuilt from a theme.
    faces: FontFaces,
    /// Host-supplied painters for registered `custom` kinds, and decoded
    /// image sources. Both start empty, which is the pre-FR-059 behaviour
    /// exactly: an unregistered name lands in `PaintReport::undrawn` rather
    /// than being drawn or being silently skipped.
    painters: CustomPainters,
    images: ImageSources,
    /// Every declared transition, the trajectories in flight, and the
    /// repaint decision. Held across passes because a trajectory that did not
    /// survive the frame that started it would restart every frame and never
    /// move.
    motion: FrameMotion,
    last_frame: Option<PetrifiedFrame>,
    last_scopes: BTreeMap<String, InputPolicy>,
    last_report: Option<PaintReport>,
    /// What the last pass's motion decided, including how many undeclared
    /// repaints the ambient ledger recorded on it (FR-062). Kept for the same
    /// reason `last_report` is: the pass computes an integrity fact that
    /// nothing on the frame path can act on, and dropping it would leave the
    /// shipped host deaf to the one failure Petra cannot prevent — a hosted
    /// painter that drives repaints without declaring `ambient`.
    last_motion: Option<FrameDecision>,
    focus: FocusTree,
    /// A `Host::step_focus` traversal target that was not yet in
    /// [`FocusTree::order`] when Tab moved to it, so the move was deferred:
    /// [`Host::apply_scroll`]'s sibling wrote a `scroll_offsets` entry meant
    /// to bring it on screen this pass, and [`Host::pass`] tries the actual
    /// [`FocusTree::focus`] call again once the new frame's `order` exists —
    /// the same "seat it after petrify" shape [`Host::enter_open_modal`]
    /// already uses for a modal taking focus, and for the same reason:
    /// nothing before petrify can promise the target will really be visible
    /// (`focus/mod.rs`'s `reachable` doc).
    pending_focus: Option<String>,
    /// A system file dialog this host opened and has not yet heard back
    /// from.
    ///
    /// The dialog runs on its own thread and answers down a channel, so a
    /// pass never blocks on it. `None` means no dialog is open — and the
    /// receiver is dropped the moment it answers or its thread dies, so a
    /// cancelled dialog (rfd returns `None`, the thread exits) frees itself
    /// with no timeout and no bookkeeping.
    ///
    /// One at a time: while this is `Some`, a further [`App::file_request`]
    /// is refused rather than queued. Two portal dialogs over one window is
    /// a picture with no right answer, and an application that asks twice has
    /// a bug this makes visible instead of hiding.
    #[cfg(not(target_arch = "wasm32"))]
    picking: Option<std::sync::mpsc::Receiver<Vec<std::path::PathBuf>>>,
    /// Every focus-claiming overlay (`Props::takes_focus`) the previous pass
    /// placed, and the node focus was on at the moment each of them opened.
    ///
    /// The map is both halves of the rule at once. A key that is new this
    /// frame is an overlay that just opened, so focus moves into it; a key
    /// that is gone is one that just closed, so focus goes back to the id
    /// stored beside it. `None` beside a key means focus was nowhere when it
    /// opened, and there is nothing to hand back to.
    ///
    /// Diffing frames is what makes the move happen once. `enter_open_modal`
    /// gets that for free — "focus is already inside some blocking scope" is
    /// an idempotent test — but a menu declares no scope at all, so an
    /// unguarded rule would drag focus back to the first item after every
    /// Tab for as long as the menu stayed open.
    focus_taking: BTreeMap<String, Option<String>>,
    /// Where the pointer is, what it is over, and what it has captured. The
    /// pointer's peer of `focus`; see this module's doc.
    pointer: PointerState,
    /// Text this host itself wants on the clipboard, taken and cleared by
    /// the pass that runs after the input.
    ///
    /// Beside [`App::clipboard_request`] rather than through it: that channel
    /// is the *application* asking, and this is the host answering a chord
    /// over state the application does not own. See [`App::copy_selection`].
    pending_copy: Option<String>,
    /// Whether the primary button is down on a run a selection anchored in.
    ///
    /// The half of a selection gesture no capture can hold: a paragraph
    /// declares no [`Interaction::Drag`], so it is never granted one. See
    /// [`App::apply_text_selection`].
    selection_drag: bool,
    /// The underline that interpolates between focus targets. Host-owned so
    /// it never enters petrify or the digest; see `focus_caret`.
    caret: FocusCaret,
    /// Tessellated picture of the last full paint, without the caret. A hop
    /// replays these meshes instead of walking every placement; eframe
    /// tessellates `Shape::Mesh` by pointer, not by rebuilding glyph verts.
    /// Headless tests and wasm stay on this path.
    scene: Vec<(egui::Rect, Arc<egui::Mesh>)>,
    /// Native wgpu state for the scene texture. Bound from `eframe::Frame`
    /// on the window path; `None` in lib tests.
    #[cfg(not(target_arch = "wasm32"))]
    gpu: Option<eframe::egui_wgpu::RenderState>,
    /// Offscreen gallery texture. `None` when there is no GPU or the last
    /// bake failed. A hop blits this and draws only the caret.
    #[cfg(not(target_arch = "wasm32"))]
    scene_tex: Option<crate::scene_cache::SceneCache>,
    /// How many times this host called [`paint_frame_with_caret`]. A hop
    /// that reuses a cached scene must not bump this.
    full_paints: u32,
    /// Passes in the hop that is still flying, including the Tab that started it.
    hop_passes: u32,
    /// Passes in the hop that just landed. Zero before the first hop.
    last_hop_passes: u32,
    /// This hop presents the scene texture, not CPU meshes. Set on the Tab
    /// that starts the hop and held until settle so a mesh→blit switch
    /// cannot pop mid-flight.
    hop_blit: bool,
    /// Every contributed surface this shell is currently showing, in the
    /// order the publisher gave them.
    ///
    /// The order is the publisher's and is never re-sorted here: ordering
    /// within a `list` slot is `TraceSeq` of the `contribute` call
    /// (`contracts/surface-contribution.md` §6), and `TraceSeq` is a fact the
    /// daemon owns. A shell that sorted by id would be inventing a second,
    /// disagreeing order out of a number that only looks monotonic.
    contributions: Vec<Contribution>,
    /// `contributions` after expansion and stage-2 acceptance. Rebuilt when
    /// `prepared_stale`, never per frame.
    prepared: Vec<Prepared>,
    /// Whether `prepared` was derived from the current contribution set *and*
    /// the current registry. Set by [`Host::set_contributions`] and by every
    /// path that can change the registry a contribution is accepted against.
    prepared_stale: bool,
    /// What the last built frame did with each contribution.
    mounts: Vec<MountReport>,
    /// One retained snapshot per live contribution, and whether each is
    /// behind (`contracts/surface-contribution.md` §8).
    ///
    /// **Owned here, and therefore never shared.** `ContributionLedger` has
    /// a `&mut` API and no interior lock, so a shell that took pushes on a
    /// wire thread and rendered on another would have to wrap it. This host
    /// answers that question by ownership instead: the ledger is reached only
    /// through `&mut Host`, the render thread owns the `Host`, and a
    /// transport hands pushes over by calling a method on it. No lock, and
    /// no second owner to disagree with.
    ledger: ContributionLedger,
}

impl<A: App> Host<A> {
    /// A host over `app`, using `ctx` for fonts and `presenter` for the theme.
    ///
    /// # Fonts
    ///
    /// This installs the embedded design-system faces on `ctx`
    /// ([`crate::fonts::install_design_system`]) and binds the weight axis to
    /// them. It is not optional and it is not conditional: the shipped face
    /// is what every extent in this engine is measured against, and a host
    /// that drew in whatever face happened to be lying around would make
    /// SC-004's cross-target digest equality an accident of two machines
    /// having the same fonts installed.
    ///
    /// It costs no I/O — the faces are `include_bytes!` — which is the whole
    /// reason it can live here. The *script* fallbacks
    /// ([`crate::fonts::install_desktop_fallbacks`]) do read files and stay
    /// opt-in for exactly that reason.
    ///
    /// egui applies new definitions at the start of the next pass, so the
    /// first `Host::pass` after construction is the first frame drawn in the
    /// shipped face. An application that wants its own stack calls
    /// `Context::set_fonts` *after* this and takes responsibility for the
    /// consequence.
    pub fn new(ctx: &Context, app: A, presenter: Presenter) -> Self {
        // The registry that accepts a tree and the theme that resolves it
        // must agree about what token names exist (contract C6): a `Host`
        // has a theme from the moment it is built (`presenter`), so its
        // registry starts with that theme's own vocabulary rather than an
        // empty one an application would otherwise have no way to populate
        // for the names this crate's own shipped views need
        // (`refusal_view`'s `surface.base`/`status.down` among them). An
        // application declares anything its own trees need beyond the
        // presenter's theme through `Host::declare_token`, which keeps the
        // declaration where a later theme publication cannot take it away.
        // The shaper's typography map is the other half of the theme a frame
        // reads, so it is bound from the same theme here, and re-bound from
        // the same place.
        crate::fonts::install_design_system(ctx);
        let snapshot = presenter.current();
        // Before the first pass rasterises anything: the curve below is an
        // input to the atlas, and egui rebuilds the atlas when it changes.
        // Setting it here means the first frame is drawn through the right
        // one rather than through one frame of the wrong one.
        bind_glyph_coverage(ctx, snapshot.theme());
        // Not `FontFaces::default()`: that points all three weight classes at
        // one family because egui's default stack has one proportional face.
        // The embedded stack has three, so `Bold` finally paints bold.
        let faces = crate::fonts::design_system_faces();
        let extra_vocabulary = Vocabulary::new();
        let mut registry =
            Registry::with_vocabulary(composed_vocabulary(snapshot.theme(), &extra_vocabulary));
        let definitions = gorgon_petra::anim::shipped_registry();
        definitions.declare_into(&mut registry);
        let mut shaper = GalleyShaper::new(ctx.clone());
        *shaper.typography_mut() = Typography::from_theme(snapshot.theme(), &faces);
        let bound_revision = snapshot.revision();
        drop(snapshot);
        Self {
            app,
            ctx: ctx.clone(),
            shaper,
            translator: EventTranslator::new(),
            cache: MeasureCache::new(),
            counter: FrameCounter::new(),
            state: LayoutState::default(),
            presenter,
            registry,
            extra_vocabulary,
            bound_revision,
            faces,
            painters: CustomPainters::new(),
            images: ImageSources::new(),
            // The component library names `toggle-knob`. Installing the
            // shipped registry here is what makes a tree of library
            // constructors validate without every application repeating the
            // slide. `set_transitions` merges on top and does not drop it.
            motion: FrameMotion::new(definitions),
            last_frame: None,
            last_scopes: BTreeMap::new(),
            last_report: None,
            last_motion: None,
            focus: FocusTree::default(),
            pending_focus: None,
            #[cfg(not(target_arch = "wasm32"))]
            picking: None,
            focus_taking: BTreeMap::new(),
            pointer: PointerState::new(),
            pending_copy: None,
            selection_drag: false,
            caret: FocusCaret::new(),
            scene: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            gpu: None,
            #[cfg(not(target_arch = "wasm32"))]
            scene_tex: None,
            full_paints: 0,
            hop_passes: 0,
            last_hop_passes: 0,
            hop_blit: false,
            contributions: Vec::new(),
            prepared: Vec::new(),
            // `true` rather than `false` on an empty set, so the one place
            // that clears it is the one place that fills `prepared`. A
            // `false` here would be a second claim about the same fact.
            prepared_stale: true,
            mounts: Vec::new(),
            ledger: ContributionLedger::new(),
        }
    }

    /// Bound the two caches this host keeps: measured sizes and shaped
    /// galleys, in entries.
    ///
    /// Both default to a bound well above one frame's working set
    /// ([`MeasureCache::DEFAULT_CAPACITY`], [`GalleyShaper::DEFAULT_CAPACITY`]);
    /// a host with a denser tree than that raises them here. Shrinking evicts
    /// at once.
    pub fn set_cache_capacities(&mut self, measured_sizes: usize, galleys: usize) {
        self.cache.set_capacity(measured_sizes);
        self.shaper.set_capacity(galleys);
    }

    /// The registry tree acceptance validates against — register custom kinds
    /// and transition names here before the first frame.
    ///
    /// **Not the place to declare token names.** The vocabulary inside this
    /// registry is a projection of the published theme and is overwritten
    /// whole every time a new revision is published, so a name declared
    /// through `registry_mut().vocabulary_mut()` survives only until the
    /// next theme swap. Use [`Host::declare_token`] and
    /// [`Host::declare_status`], which are kept and re-applied on top of
    /// every theme.
    pub fn registry_mut(&mut self) -> &mut Registry {
        // Conservative on purpose: this hands out `&mut Registry`, so a
        // caller may register a custom kind or a transition that decides
        // whether a contribution is acceptable, and nothing here can tell
        // whether one did. Re-preparing costs one expand and one walk per
        // contribution, and only on a call a host makes at startup.
        self.prepared_stale = true;
        &mut self.registry
    }

    /// Declare a token name this application's own trees use beyond the ones
    /// its theme defines, and re-derive the registry so it takes effect now.
    ///
    /// A name declared here outlives every theme publication. A name
    /// declared straight into `registry_mut().vocabulary_mut()` does not —
    /// see [`Host::registry_mut`].
    pub fn declare_token(&mut self, token: DesignToken) -> &mut Self {
        self.extra_vocabulary.declare(token);
        self.rebind_current_theme();
        self
    }

    /// Declare a status beyond the theme's own, with the same lifetime rule
    /// as [`Host::declare_token`].
    pub fn declare_status(&mut self, status: StatusToken) -> &mut Self {
        self.extra_vocabulary.declare_status(status);
        self.rebind_current_theme();
        self
    }

    /// Point each [`gorgon_petra::token::TypographyWeight`] class at the egui
    /// font family that draws it, and re-shape everything through the new
    /// mapping.
    ///
    /// The default maps all three classes to `Proportional`, because egui's
    /// built-in fonts install one proportional face — under it a `Bold`
    /// token paints at regular weight. A host that installs a bold face
    /// through `egui::Context::set_fonts` names it here.
    pub fn set_font_faces(&mut self, faces: FontFaces) {
        self.faces = faces;
        self.rebind_current_theme();
    }

    /// Install the host machine's script fallback faces on top of the
    /// embedded ones, and re-shape everything through the new stack.
    ///
    /// The design-system faces are already in force —
    /// [`Host::new`] installs them, they are embedded, and they cost no I/O.
    /// This is the other half: CJK, Arabic, Devanagari and Hebrew, read from
    /// the host's filesystem, which is why it is a call a desktop product
    /// makes rather than something that happens by itself. It cannot work on
    /// wasm, and it is roughly 32 MiB of Noto that this crate deliberately
    /// does not embed — see `fonts`'s module documentation for both.
    ///
    /// Call it once, at startup, and **read the report**.
    /// [`crate::fonts::FontStackReport::is_complete`] answers whether the
    /// machine had every required face, and
    /// [`crate::fonts::FontStackReport::summary`] names the ones it did not
    /// and every path that was tried. A product that drops it has chosen to
    /// find out about a missing face by seeing boxes on screen.
    ///
    /// Two chores a caller must not forget are done here rather than left to
    /// be rediscovered: every cached galley was shaped against the old stack
    /// and is dropped, and the caller is spared having to know that egui
    /// applies new definitions at the start of the *next* pass — which it
    /// still does, so the first frame after this call is the first one drawn
    /// with the new faces.
    pub fn install_script_fallbacks(&mut self, ctx: &Context) -> crate::fonts::FontStackReport {
        let report = crate::fonts::install_desktop_fallbacks(ctx);
        self.shaper.clear();
        self.cache.clear();
        report
    }

    /// The shaper, for its cache statistics and its bound typography map.
    #[must_use]
    pub fn shaper(&self) -> &GalleyShaper {
        &self.shaper
    }

    /// The presenter this host reads its theme from — the way an application
    /// publishes a new one.
    ///
    /// A shared reference is enough because `Presenter::publish` takes
    /// `&self`; the host is not told, and does not need to be. The next
    /// [`Host::pass`] sees a revision it has not bound and re-derives
    /// everything that depends on the theme before a tree is accepted
    /// against it.
    #[must_use]
    pub fn presenter(&self) -> &Presenter {
        &self.presenter
    }

    /// Re-derive everything that is a function of the theme, from whatever
    /// the presenter currently holds.
    fn rebind_current_theme(&mut self) {
        let snapshot = self.presenter.current();
        self.bound_revision = snapshot.revision();
        self.bind_theme(snapshot.theme());
    }

    /// Re-derive the registry's vocabulary and the shaper's typography when
    /// the published revision has moved since they were last derived.
    ///
    /// Contract C6 makes validation theme-independent, and *because* of that
    /// resolution is total: `tree::props::resolve_spacing` panics on a name
    /// the snapshot does not define, deliberately, so that no container
    /// carries a fallback branch. That totality holds only while the
    /// registry a tree is accepted against and the theme the frame resolves
    /// against name the same tokens. Deriving the registry once, in
    /// [`Host::new`], held it only until the first publication: a theme with
    /// a different vocabulary left the registry accepting trees that named
    /// tokens the new theme does not define, and the panic — the assertion
    /// holding C6 — fired mid-frame.
    ///
    /// `Presenter::publish` takes `&self` and can be called from any thread
    /// at any time, so nothing can tell the host it happened. Comparing
    /// revisions at the top of the one pass that reads the snapshot anyway
    /// is the check that needs no cooperation from the publisher, and the
    /// revision is the right key: it moves on *every* publication, even one
    /// that republishes an identical theme.
    fn rebind_theme_if_stale(&mut self, snapshot: &ThemeSnapshot) {
        if snapshot.revision() == self.bound_revision {
            return;
        }
        self.bound_revision = snapshot.revision();
        self.bind_theme(snapshot.theme());
    }

    fn bind_theme(&mut self, theme: &Theme) {
        *self.registry.vocabulary_mut() = composed_vocabulary(theme, &self.extra_vocabulary);
        // Every contribution was accepted against the vocabulary this line
        // just replaced. A tree that named a token the old theme defined and
        // this one does not is no longer acceptable, and one refused for a
        // name the new theme *does* define deserves its second chance. The
        // one funnel every registry vocabulary change goes through, so this
        // covers a theme publication, `declare_token` and `declare_status`
        // alike.
        self.prepared_stale = true;
        *self.shaper.typography_mut() = Typography::from_theme(theme, &self.faces);
        bind_glyph_coverage(&self.ctx, theme);
        // Every cached galley was shaped through the old map. The key
        // carries size, family and line height, so a stale entry could not
        // be *served* — but it can no longer be asked for either, and would
        // sit in the bound taking a slot from a live run.
        self.shaper.clear();
    }

    /// The painter registry for `custom` kinds — register a painter before the
    /// first pass, the same way `registry_mut` registers the kind itself.
    ///
    /// Registering the *kind* makes a tree acceptable; registering the
    /// *painter* makes it visible. They are deliberately separate: a kind with
    /// no painter is a reported gap (`PaintReport::undrawn`), not a refusal,
    /// because a host that measures a region it cannot yet draw is a real and
    /// honest state (FR-059).
    pub fn painters_mut(&mut self) -> &mut CustomPainters {
        &mut self.painters
    }

    /// The image source registry.
    pub fn images_mut(&mut self) -> &mut ImageSources {
        &mut self.images
    }

    /// The application.
    pub fn app(&self) -> &A {
        &self.app
    }

    /// The application, mutably.
    pub fn app_mut(&mut self) -> &mut A {
        &mut self.app
    }

    /// The layout state this host negotiates against: focus and scroll offsets.
    ///
    /// Public because scrolling is driven from outside the engine — by the
    /// scroll gesture handler and by the driver's synthetic input — and
    /// neither lives here. `focused` is **not** a field to write through this
    /// handle: [`Host::pass`] overwrites it from [`Host::focus`] every frame,
    /// so a value poked in here survives only until the next pass. Move focus
    /// with [`Host::focus_mut`] instead, which goes through the focus tree and
    /// therefore respects a modal's scope.
    pub fn state_mut(&mut self) -> &mut LayoutState {
        &mut self.state
    }

    /// The pointer snapshot this host derives hover, press and capture from.
    ///
    /// Read-only on purpose, and there is no `pointer_mut`. Hover is the
    /// engine's to derive (FR-009) and a capture is the engine's to grant
    /// ([`PointerState::route`] hit-tests for `Interaction::Drag` before it
    /// hands one out); an application that could write either would be able
    /// to light a control the router will not click.
    pub fn pointer(&self) -> &PointerState {
        &self.pointer
    }

    /// The focus tree this host traverses: order, current focus, active scope.
    pub fn focus(&self) -> &FocusTree {
        &self.focus
    }

    /// The focus tree, mutably — the way a driver or an application moves
    /// focus programmatically ([`FocusTree::focus`] refuses a move that would
    /// leave an open modal). The move reaches `LayoutState::focused` at the
    /// start of the next [`Host::pass`].
    pub fn focus_mut(&mut self) -> &mut FocusTree {
        &mut self.focus
    }

    /// The layout state this host negotiates against.
    pub fn state(&self) -> &LayoutState {
        &self.state
    }

    /// The most recent petrified frame, or `None` before the first pass.
    pub fn frame(&self) -> Option<&PetrifiedFrame> {
        self.last_frame.as_ref()
    }

    /// What the most recent paint pass did.
    pub fn report(&self) -> Option<&PaintReport> {
        self.last_report.as_ref()
    }

    /// What the most recent pass's motion decided, or `None` before the first
    /// pass.
    ///
    /// [`FrameDecision::undeclared`] is the live FR-062 signal. Non-zero means
    /// a hosted surface is driving repaints without declaring `ambient`, so
    /// idle is unreachable and SC-002 is already broken — Petra refuses to
    /// count such a request as motion, but it cannot stop egui from repainting
    /// on someone else's behalf, so reporting is the only remaining defence.
    ///
    /// **It is an edge, not a level.** The count is how many *new*
    /// `(surface, source)` pairs the ledger had never seen before, so one
    /// painter asking on six hundred consecutive frames reads `1` on the first
    /// and `0` on the rest. Polling this and seeing zero therefore does not
    /// mean the run is clean. The level is
    /// [`Host::motion`]`().scheduler().ledger().is_clean()`, which is O(1) and
    /// safe to read every frame; the named, human-readable form — every
    /// offending surface, the `file:line` that asked, the counts and the
    /// window — is `.idle_audit(window)`, asked once at the end of the span
    /// being judged.
    ///
    /// All three are exercised against this accessor in
    /// `tests/idle_audit.rs`.
    #[must_use]
    pub fn decision(&self) -> Option<FrameDecision> {
        self.last_motion
    }

    /// Bind the window's wgpu state so a hop can blit a scene texture.
    ///
    /// Headless tests never call this. wasm is a no-op: the CPU mesh replay
    /// stays the picture.
    pub fn pass_in_window(&mut self, ctx: &Context, frame: &eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.gpu = frame.wgpu_render_state().cloned();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = frame;
        }
        self.pass(ctx);
    }

    /// Measure and place `tree` against `viewport`, under `theme`.
    ///
    /// Split out of [`Self::pass`] so its stationary-focus reconciliation
    /// can ask for a second petrify against the same `tree` — cheap,
    /// `ValidatedTree` is `Copy` — once it has corrected `self.state`
    /// (a scroll offset, or which node is focused) rather than paint a
    /// frame that still names an off-screen node as focused.
    fn petrify_frame(
        &mut self,
        tree: ValidatedTree<'_>,
        viewport: Viewport,
        theme: &ThemeSnapshot,
    ) -> PetrifiedFrame {
        let mut ctx_layout = LayoutCtx {
            content: &mut self.shaper,
            rows: &mut self.app,
            cache: &mut self.cache,
            state: &self.state,
            theme,
            theme_rev: viewport.theme_rev,
            scale: viewport.scale,
            scroll: ScrollStack::new(),
            reuse: None,
            anchors: AnchorRects::new(),
        };
        petrify(
            self.counter.take(),
            tree,
            &mut ctx_layout,
            viewport,
            // The real count is not known until the transitions have been
            // advanced, and they are advanced against the placements this
            // call produces. So the frame is petrified settled and
            // `FrameMotion::advance` writes the measured activity onto it
            // below, along with the placements that are actually on
            // screen and the digest of them.
            TransitionActivity::default(),
        )
    }

    /// Run one whole pass: input, view, petrify, paint, schedule.
    ///
    /// A caret already in flight, with no new input and no change to the
    /// viewport, skips view and petrify and paints the last picture with a
    /// new bar. Rebuilding the tree to move a 3 px underline is what made a
    /// 160 ms hop show three frames on a full page.
    ///
    /// Public and independent of `eframe` so a test or the driver can step the
    /// host without a window. Interactive windows call [`Self::pass_in_window`]
    /// so the GPU cache can attach.
    pub fn pass(&mut self, ctx: &Context) {
        // Before the snapshot is read, not after: a theme asked for on the
        // last pass has to be in force for this one, or the frame the
        // operator sees is one behind his own click.
        if let Some(theme) = self.app.theme_request() {
            self.presenter.publish(theme);
        }
        let snapshot = self.presenter.current();
        // Before anything reads the registry: a theme published since the
        // last pass may name a different set of tokens, and a tree accepted
        // against the old set would panic when this one resolved it.
        self.rebind_theme_if_stale(&snapshot);
        let scale = Scale::new(ctx.pixels_per_point()).unwrap_or(Scale::ONE);
        let screen = ctx.content_rect();
        let viewport = Viewport {
            size: Size::new(screen.width(), screen.height()),
            scale,
            theme_rev: snapshot.revision(),
            theme_mode: snapshot.mode(),
        };

        let had_input = self.deliver_input(ctx);
        // After the input, not before: a press handled by this pass has to be
        // able to copy on this pass, or the operator's second press is what
        // copies what his first one selected.
        // Taken before the branch, never inside it: a request left sitting
        // here would be answered by whatever the *next* pass copied.
        let mine = self.pending_copy.take();
        // The application's request wins. It is the more specific answer, and
        // the two are different events anyway — the copy button is a press
        // and the chord is a keystroke.
        if let Some(text) = self.app.clipboard_request().or(mine) {
            ctx.copy_text(text);
        }
        // Same pass, same reason. A drop is input the window took before this
        // pass began, so the application sees it before it builds the view it
        // will be judged on.
        let dropped = dropped_paths(ctx);
        if !dropped.is_empty() {
            self.app.files_dropped(&dropped);
        }
        self.serve_file_dialog(ctx);
        // Input may have moved focus or the pointer; the negotiation below
        // reads `LayoutState`, so publish both before the frame is measured
        // rather than after it is painted. A move published here needs no
        // repaint request: this pass's own frame is placed from it, ring,
        // hover and all.
        let _ = self.publish_focus();
        let _ = self.publish_pointer();

        if self.can_reuse_frame(&viewport, had_input) {
            self.paint_caret_on_last_frame(ctx, snapshot.as_ref());
            return;
        }

        // Theme and scale are global inputs to every measurement, so a change
        // to either invalidates wholesale; content changes are the
        // application's change set, applied here so no host wiring can skip
        // the ancestor walk it requires.
        // The clock goes to the application before its change set is read,
        // so a view that depends on the clock can name what the clock moved.
        let now = ctx.input(|input| input.time);
        self.app.tick(now);
        // Immediately after the clock and not later: the application has just
        // been told what time it is, so this is the first moment it can name
        // a deadline measured against it.
        if let Some(at) = self.app.wake_at()
            && at.is_finite()
        {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((at - now).max(0.0)));
        }
        self.cache
            .retain_theme_and_scale(viewport.theme_rev, viewport.scale);
        self.cache.apply(&self.app.take_changes());

        // The application's tree, then every contributed surface spliced into
        // it. In this order and not the other: a contribution mounts *into* a
        // node the application declares, so there is nothing to mount into
        // until the application has answered. Each contribution was expanded
        // and accepted when it was set; this splices the results.
        let authored = self.app.view();
        let app_tree = self.mount_contributions(authored);
        // `refusal_view` only exists to be assigned into on the error arm
        // below; the `let` with no initializer is what lets the borrow
        // minted there outlive the `match` (a binding declared outside the
        // arm that assigns it). Both arms mint through the same `validate`
        // call that decided Ok/Err — the happy path pays for exactly one
        // walk of the tree, not two.
        let refusal_tree;
        let tree = match validate(&app_tree, &self.registry) {
            Ok(validated) => validated,
            // A refused tree is a bug in the application, and the operator
            // has to be able to see which node. Painting the violations is
            // louder than a log line and does not take the window down.
            // `refusal_view` is authored by this crate, not the application,
            // so it is expected to validate; a violation in the view built to
            // report violations would be this crate's own bug, and the
            // `expect` says so by name rather than laying out a tree nothing
            // ever accepted.
            //
            // Validated against the shipped vocabulary, not `self.registry`:
            // `refusal_view` references this crate's own shipped names
            // (`surface.base`, `status.down`), and an application's registry
            // is under no obligation to have declared them — its vocabulary
            // is its own design system, which may not ship either name. The
            // fallback view this crate paints when *that* vocabulary refuses
            // a tree must not itself depend on it.
            Err(errors) => {
                refusal_tree = refusal_view(&errors.to_string());
                validate(
                    &refusal_tree,
                    &Registry::with_vocabulary(standard_vocabulary()),
                )
                .expect("gorgon-petra-egui's own refusal_view must validate")
            }
        };

        let scopes = surface_scopes(&tree);
        let taking = focus_taking_surfaces(&tree);
        // Read before any reconciliation below can move it: this is where
        // focus stood while the *previous* frame's overlays were still
        // mounted, which is the only moment that can answer "was focus
        // inside the overlay that just closed" —
        // `reseat_focus_taking_surfaces` needs that answer and
        // `FocusTree::update`'s vanished-focus rule has already destroyed it
        // by the time that call runs.
        let focus_before = self.focus.current().map(str::to_owned);
        let mut frame = self.petrify_frame(tree, viewport, snapshot.as_ref());

        // `state.focused` was set from the *last* pass's reconciliation,
        // against the *last* pass's geometry. Nothing about this pass's own
        // layout was known then, so a stationary focus (no Tab, no pointer
        // move — `pending_focus` is the in-flight case those two already
        // handle, and a frame mid-transition to a *new* target already
        // tolerates one stale-but-visible frame, see `publish_focus`'s
        // caller below) can still come out the other side of *this*
        // petrify sitting somewhere its own clip no longer reaches: a
        // fiber list gaining a row while `ticker` stayed selected, a panel
        // that used to fit its window and no longer does. Painting that is
        // exactly the one thing `blind_focus` exists to catch, so it has
        // to be corrected before paint, not after.
        //
        // Try the same fix `step_focus` already applies for Tab first —
        // scroll the focused node's own `Scroll` ancestor back into view,
        // keeping the operator's selection — and only fall back to
        // `FocusTree::update`'s vanished-focus rule (moving focus to
        // whatever is reachable instead) when there is no ancestor to
        // scroll, or the scroll did not reach (`write_scroll_offset`'s
        // clamp is one frame stale by construction, so it can fall short).
        // `ValidatedTree` is `Copy`, so a second `petrify` against the same
        // tree is the cost of re-checking, and it only runs on the rare
        // frame this reconciliation finds something to fix.
        if self.pending_focus.is_none()
            && frame
                .placements
                .iter()
                .any(|p| p.semantics.focused && !p.is_visible())
        {
            if let Some(id) = self.state.focused.clone()
                && self.scroll_into_view(&frame, &id)
            {
                frame = self.petrify_frame(tree, viewport, snapshot.as_ref());
            }
            if frame
                .placements
                .iter()
                .any(|p| p.semantics.focused && !p.is_visible())
            {
                self.focus = self.focus.update(&frame.placements, &scopes);
                if self.publish_focus() {
                    frame = self.petrify_frame(tree, viewport, snapshot.as_ref());
                }
            }
        }

        // Layout negotiated the targets; this interpolates the result and
        // rewrites the frame into what is actually painted, activity, digest
        // and all (`gorgon_petra::anim::engine`). A frame with nothing moving
        // comes back untouched, so an idle window's frame is byte-identical
        // to what `petrify` produced.
        let decision = self.motion.advance(ctx, &mut frame, &tree);

        // Tick against the frame about to be painted, not the post-petrify
        // reconciliation: Tab is published before petrify, so this pass
        // already names the new node and the bar can leave on the same frame.
        let now = ctx.input(|input| input.time);
        self.tick_caret(&frame, now);
        let report = self.paint_and_bake_scene(ctx, &frame, snapshot.as_ref());
        // Every field `PaintReport::is_complete` reads, including the two
        // this message used to omit. A failure that printed "0 silent" and
        // counts that summed correctly could not explain itself: the reader
        // was left to rediscover that `blind_focus` and `missing_assets`
        // are also part of the predicate.
        debug_assert!(
            report.is_complete(),
            "a paint pass must account for every placement and leave none \
             silent: {} drawn + {} clipped + {} empty + {} silent of {} \
             placement(s), desynced={}, blind_focus={}, missing_assets={}",
            report.drawn,
            report.skipped_clipped,
            report.empty,
            report.silent,
            report.placements,
            report.desynced,
            report.blind_focus,
            report.missing_assets
        );

        // The frame is placed, so this is the reconciliation point for
        // everything the block above does not already cover: `update`
        // carries the focused node forward, or applies the vanished-focus
        // rule when it is gone. Doing it here rather than at the top of the
        // next pass is what keeps an event arriving between two frames
        // aimed at a node that still exists.
        self.focus = self.focus.update(&frame.placements, &scopes);
        // A `step_focus` that deferred its move (the target was not visible
        // yet, only scrolled toward) tries again here, now that `order` is
        // built from the frame the scroll actually reached. A target the
        // scroll fell short of (see `write_scroll_offset`'s clamp doc) is
        // left to whatever `update` above already chose, rather than erred
        // on — the same "best effort, never broken" choice
        // `seat_pointer_focus` makes for a press that names no focusable
        // ancestor.
        if let Some(target) = self.pending_focus.take() {
            let _ = self.focus.focus(&target);
        }
        self.enter_open_modal(&frame, &scopes);
        self.reseat_focus_taking_surfaces(&frame, &taking, focus_before.as_deref());
        // The pointer's half of the same reconciliation, and it needs the new
        // placements for the same reason focus does: a captured node that is
        // gone has no successor, and hover is a fact about the picture on
        // screen rather than about the last event — a node that moved out
        // from under a stationary pointer stops being hovered here, without
        // waiting for a move that may never come.
        if let Some(ended) = self.pointer.reconcile(&frame, &scopes) {
            self.app
                .handle(&ended.event(), &ended.route(), Some(&frame));
        }
        let moved_focus = self.publish_focus();
        // `|`, not `||`: both have to run. Publishing is what writes the
        // projection, so short-circuiting the second would leave `hovered`
        // stale on every frame that also moved focus.
        if moved_focus | self.publish_pointer() {
            // The frame just painted was placed from the *previous* focus and
            // the *previous* pointer snapshot, so its ring is on the wrong
            // node, or a hovered control is lit that is no longer under the
            // pointer. Every move that reaches here happens after petrify by
            // necessity — the vanished-focus rule, a modal taking focus, and
            // hover re-derivation all need the new placements to decide — so
            // the only honest fix is to ask for one more frame. Without this
            // the indicator waits for the next unrelated event, which on an
            // idle window is forever.
            //
            // **Exactly one** extra frame, which is what keeps SC-002 intact
            // (`tests/idle_audit.rs`): the next pass reconciles against a
            // frame placed from the snapshot it is reconciling, both
            // `publish` calls report no move, and nothing more is requested.
            ctx.request_repaint();
        }

        // The screen reader hears the frame that was just painted, not the
        // tree the application described (FR-027): one projection, from the
        // placements, so a divergence between what an assistive technology
        // announces and what a driver asserts is impossible by construction
        // rather than by discipline.
        //
        // After the focus reconciliation above on purpose. `frame` was placed
        // from the *previous* focus, so its `focused` flags describe the ring
        // that is on screen — which is the thing a reader should announce. The
        // `publish_focus` branch above already asks for one more frame when
        // that ring moved, and this publishes again on that frame.
        //
        // `publish` self-registers its plugin, and the plugin is what makes
        // this survive: `egui::Context::end_pass` writes its own AccessKit
        // update — one `Role::Window` root, since Petra builds no `Ui` — and
        // only `Plugin::output_hook` runs late enough to replace it. Assigning
        // the field from here would be silently undone the moment a real
        // screen reader attached and eframe turned egui's generation on.
        if let Some(tree) = gorgon_petra::semantic::project(&frame) {
            crate::accesskit::publish(ctx, &tree);
        }

        self.schedule(ctx, &frame);
        if self.caret.is_moving() {
            ctx.request_repaint();
        }
        self.last_frame = Some(frame);
        // Input for the *next* pass is routed against this frame, so the
        // policies it was placed with have to survive with it. Recomputing
        // `surface_scopes` at input time would read the tree the application
        // has since changed, and hit-test the frame on screen against scopes
        // that no longer describe it.
        self.last_scopes = scopes;
        self.last_report = Some(report);
        // The pass already paid for this and nothing on the frame path can
        // act on it, so the only question left is whether it survives the
        // pass. It must: a repaint Petra did not make is the one way SC-002
        // breaks that Petra cannot prevent, and an operator who cannot read
        // the count has no way to find out. See `Host::decision`.
        self.last_motion = Some(decision);
    }

    /// Whether this pass can paint the last petrified picture with a new
    /// caret instead of asking the application for a tree.
    ///
    /// Input, a viewport change, and a running transition each need a new
    /// petrify. A caret in flight does not: the bar is not a placement.
    fn can_reuse_frame(&self, viewport: &Viewport, had_input: bool) -> bool {
        if had_input || !self.caret.is_moving() {
            return false;
        }
        // A contribution set that changed since the last frame has to reach
        // the screen on this pass. Without this a surface mounted while the
        // focus caret happened to be mid-hop would wait out the hop before
        // appearing, because this path paints the last picture and never asks
        // for a tree.
        if self.prepared_stale {
            return false;
        }
        let Some(frame) = self.last_frame.as_ref() else {
            return false;
        };
        frame.viewport == *viewport && !wants_frame(frame.transitions)
    }

    /// Tick the caret against the last petrified frame and paint that
    /// picture. The application is not asked for a view; the hop is paint
    /// only.
    fn paint_caret_on_last_frame(&mut self, ctx: &Context, colors: &dyn crate::paint::TokenSource) {
        let frame = self
            .last_frame
            .take()
            .expect("can_reuse_frame required a petrified frame");
        let now = ctx.input(|input| input.time);
        self.tick_caret(&frame, now);
        let report = if self.hop_blit {
            self.paint_scene_texture(ctx, &frame, colors)
        } else if self.scene.is_empty() {
            self.paint_and_bake_scene(ctx, &frame, colors)
        } else {
            self.paint_cached_scene(ctx, &frame, colors)
        };
        debug_assert!(
            report.is_complete(),
            "a reused paint pass must still account for every placement: \
             {} drawn + {} clipped + {} empty + {} silent of {} \
             placement(s), desynced={}",
            report.drawn,
            report.skipped_clipped,
            report.empty,
            report.silent,
            report.placements,
            report.desynced
        );
        self.note_hop_pass();
        if self.caret.is_moving() {
            ctx.request_repaint();
        } else {
            self.scene.clear();
            self.hop_blit = false;
        }
        self.last_frame = Some(frame);
        self.last_report = Some(report);
    }

    fn caret_overlay(&self, frame: &PetrifiedFrame) -> Option<CaretOverlay> {
        let scale = frame.viewport.scale;
        let page = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(frame.viewport.size.w, frame.viewport.size.h),
        );
        let clip = caret_clip_limit(self.caret.clip(), self.caret.is_moving(), page, scale);
        let bands = self.caret.bands()?;
        let picture = if self.caret.is_moving() {
            CaretPicture::Flying(bands)
        } else {
            CaretPicture::Settled {
                mark: self.caret.mark(),
                figure: self.caret.figure(),
                radius: self.caret.radius().map(str::to_owned),
            }
        };
        Some(CaretOverlay { picture, clip })
    }

    fn tick_caret(&mut self, frame: &PetrifiedFrame, now: f64) {
        let scale = frame.viewport.scale;
        match focused_caret_target(frame) {
            Some(target) => {
                self.caret.tick(
                    Some(crate::focus_caret::CaretDest {
                        id: target.id,
                        mark: target.mark,
                        bands: caret_bands(target.mark, target.figure, scale),
                        figure: target.figure,
                        radius: target.radius,
                        clip: target.clip,
                    }),
                    now,
                );
            }
            None => self.caret.tick(None, now),
        }
    }

    /// Paint the petrified picture, tessellate it once, and keep the meshes
    /// so a hop can replay them. When a wgpu state is bound, also render
    /// those primitives into an offscreen texture. The caret is drawn after
    /// the bake so it is not frozen into the cache.
    fn paint_and_bake_scene(
        &mut self,
        ctx: &Context,
        frame: &PetrifiedFrame,
        colors: &dyn crate::paint::TokenSource,
    ) -> PaintReport {
        let overlay = self.caret_overlay(frame);
        let page = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(frame.viewport.size.w, frame.viewport.size.h),
        );
        let mut report = paint_frame_with_caret(
            &ctx.layer_painter(petra_layer()),
            frame,
            &mut self.shaper,
            colors,
            &self.painters,
            &mut self.images,
            // An empty overlay: the frame painter draws no settled ring,
            // because the caret is drawn below, after the bake.
            Some(CaretOverlay {
                picture: CaretPicture::Hidden,
                clip: page,
            }),
        );
        self.full_paints = self.full_paints.saturating_add(1);
        if self.caret.is_moving() {
            let primitives = self.tessellate_layer(ctx);
            self.store_cpu_meshes(&primitives);
            let ready = self.capture_scene_texture(ctx, &primitives, colors);
            self.hop_blit = ready;
            if ready {
                // Bake waited; the front holds this pass's picture. Blit it
                // now so the hop never switches from meshes to a texture.
                self.replace_layer_with_blit(ctx, page);
            } else {
                self.replace_layer_with_meshes(ctx);
            }
        } else {
            self.scene.clear();
            self.hop_blit = false;
        }
        if let Some(overlay) = overlay
            && paint_caret_overlay(
                &ctx.layer_painter(petra_layer()),
                &overlay,
                colors,
                frame.viewport.scale,
                &mut report,
            )
        {
            report.focus_rings += 1;
        }
        self.note_hop_pass();
        report
    }

    fn note_hop_pass(&mut self) {
        if self.caret.is_moving() {
            self.hop_passes = self.hop_passes.saturating_add(1);
        } else if self.hop_passes > 0 {
            self.last_hop_passes = self.hop_passes;
            self.hop_passes = 0;
        }
    }

    fn paint_cached_scene(
        &mut self,
        ctx: &Context,
        frame: &PetrifiedFrame,
        colors: &dyn crate::paint::TokenSource,
    ) -> PaintReport {
        let painter = ctx.layer_painter(petra_layer());
        for (clip, mesh) in &self.scene {
            painter
                .with_clip_rect(*clip)
                .add(egui::Shape::mesh(Arc::clone(mesh)));
        }
        self.finish_cached_paint(&painter, frame, colors)
    }

    fn paint_scene_texture(
        &mut self,
        ctx: &Context,
        frame: &PetrifiedFrame,
        colors: &dyn crate::paint::TokenSource,
    ) -> PaintReport {
        let painter = ctx.layer_painter(petra_layer());
        let page = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(frame.viewport.size.w, frame.viewport.size.h),
        );
        if let Some(shape) = self.scene_blit_shape(page) {
            painter.add(shape);
        }
        self.finish_cached_paint(&painter, frame, colors)
    }

    fn finish_cached_paint(
        &self,
        painter: &egui::Painter,
        frame: &PetrifiedFrame,
        colors: &dyn crate::paint::TokenSource,
    ) -> PaintReport {
        let mut report = self.last_report.clone().unwrap_or_else(|| PaintReport {
            placements: frame.placements.len(),
            drawn: frame.placements.len(),
            desynced: frame.placements.len() != frame.content.len(),
            ..PaintReport::default()
        });
        report.focus_rings = 0;
        report.blind_focus = 0;
        if let Some(overlay) = self.caret_overlay(frame)
            && paint_caret_overlay(painter, &overlay, colors, frame.viewport.scale, &mut report)
        {
            report.focus_rings += 1;
        }
        report
    }

    fn tessellate_layer(&self, ctx: &Context) -> Vec<egui::epaint::ClippedPrimitive> {
        let layer = petra_layer();
        let ppp = ctx.pixels_per_point();
        let shapes: Vec<egui::epaint::ClippedShape> = ctx.graphics(|g| {
            g.get(layer)
                .map(|list| list.all_entries().cloned().collect())
                .unwrap_or_default()
        });
        ctx.tessellate(shapes, ppp)
    }

    fn store_cpu_meshes(&mut self, primitives: &[egui::epaint::ClippedPrimitive]) {
        self.scene.clear();
        self.scene.reserve(primitives.len());
        for primitive in primitives {
            if let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                self.scene
                    .push((primitive.clip_rect, Arc::new(mesh.clone())));
            }
        }
    }

    fn replace_layer_with_meshes(&self, ctx: &Context) {
        let layer = petra_layer();
        ctx.graphics_mut(|g| {
            let list = g.entry(layer);
            let n = list.all_entries().len();
            for i in 0..n {
                list.reset_shape(egui::layers::ShapeIdx(i));
            }
            for (clip, mesh) in &self.scene {
                list.add(*clip, egui::Shape::mesh(Arc::clone(mesh)));
            }
        });
    }

    fn replace_layer_with_blit(&self, ctx: &Context, page: egui::Rect) {
        let Some(shape) = self.scene_blit_shape(page) else {
            return;
        };
        let layer = petra_layer();
        ctx.graphics_mut(|g| {
            let list = g.entry(layer);
            let n = list.all_entries().len();
            for i in 0..n {
                list.reset_shape(egui::layers::ShapeIdx(i));
            }
            list.add(page, shape);
        });
    }

    fn capture_scene_texture(
        &mut self,
        ctx: &Context,
        primitives: &[egui::epaint::ClippedPrimitive],
        colors: &dyn crate::paint::TokenSource,
    ) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(gpu) = self.gpu.clone() else {
                return false;
            };
            let size = physical_window_px(ctx);
            let clear = colors.color("surface.base").unwrap_or(egui::Color32::BLACK);
            crate::scene_cache::SceneCache::capture(
                &mut self.scene_tex,
                &gpu,
                ctx,
                primitives,
                size,
                ctx.pixels_per_point(),
                clear,
            )
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (ctx, primitives, colors);
            false
        }
    }

    fn scene_texture_has_front(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.scene_tex
                .as_ref()
                .is_some_and(|cache| cache.has_front())
        }
        #[cfg(target_arch = "wasm32")]
        false
    }

    fn scene_blit_shape(&self, page: egui::Rect) -> Option<egui::Shape> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.scene_tex
                .as_ref()
                .filter(|cache| cache.has_front())
                .map(|cache| cache.blit_shape(page))
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = page;
            None
        }
    }

    /// Seat focus inside the frontmost blocking surface when it is outside
    /// every one of them.
    ///
    /// [`FocusTree`] traps traversal inside a modal's scope, but only once
    /// focus is *in* it: `next`/`previous` cycle within the current scope, so
    /// a modal that opens while focus sits on the page behind it would never
    /// be reached by Tab at all. Deciding that a modal takes focus when it
    /// opens is a host policy, which is why it lives here and not in the focus
    /// module.
    ///
    /// Two deliberate no-ops: focus already inside *some* blocking scope is
    /// left alone (a second modal opening over an open one does not steal it —
    /// stacked modals need a rule this spec does not have yet), and a modal
    /// with nothing focusable in it does not empty the focus tree.
    fn enter_open_modal(&mut self, frame: &PetrifiedFrame, scopes: &BTreeMap<String, InputPolicy>) {
        if self.focus.active_scope().is_some() {
            return;
        }
        let Some(modal) = frame
            .paint_order()
            .into_iter()
            .rev()
            .find(|p| scopes.get(&p.id) == Some(&InputPolicy::Block))
            .map(|p| p.id.clone())
        else {
            return;
        };
        let Some(target) = self
            .focus
            .order()
            .iter()
            .find(|id| self.focus.scope_of(id) == Some(modal.as_str()))
            .cloned()
        else {
            return;
        };
        if let Err(err) = self.focus.focus(&target) {
            debug_assert!(
                false,
                "a node this frame's own focus order lists was refused focus: {err}"
            );
        }
    }

    /// Move focus into a focus-claiming overlay that just opened, and back
    /// out of one that just closed.
    ///
    /// [`gorgon_petra::tree::Props::takes_focus`] is the declaration; this is
    /// the timing, which is the host's half of the split this module's doc
    /// draws ("`gorgon-petra` decides *what* focus does, this crate decides
    /// *when*"). Carbon's `Menu` is the component that asks for it:
    /// `Menu.js`'s `handleOpen` stores `document.activeElement` and focuses
    /// the list, an effect seats item 0, and `handleClose` calls
    /// `returnFocus`.
    ///
    /// `focus_before` is where focus stood before this pass's reconciliation
    /// touched it — [`Host::pass`] reads it before petrify. It is what the
    /// return half tests: focus goes back to the opener **only when it was
    /// still inside the overlay that closed**. Without that test a press on
    /// some other control would open with that control focused (the press
    /// seats focus before petrify) and then be yanked back to a trigger the
    /// operator had already left.
    ///
    /// Three deliberate no-ops. An overlay with nothing focusable inside is
    /// still recorded as open, so it does not try again every frame, and it
    /// hands nothing back when it goes. A refused
    /// [`FocusTree::focus`] — the opener has since been disabled or
    /// unmounted, or a blocking scope has opened over it — leaves focus
    /// wherever the vanished-focus rule already put it, which is the same
    /// "best effort, never broken" choice [`Host::seat_pointer_focus`] makes.
    /// And an overlay that is placed but clipped out of sight is not open for
    /// this purpose: [`Placement::is_visible`] is the same test
    /// [`FocusTree`] uses to decide what is reachable at all.
    fn reseat_focus_taking_surfaces(
        &mut self,
        frame: &PetrifiedFrame,
        taking: &BTreeSet<String>,
        focus_before: Option<&str>,
    ) {
        let present: BTreeSet<&str> = frame
            .placements
            .iter()
            .filter(|p| p.is_visible() && taking.contains(&p.id))
            .map(|p| p.id.as_str())
            .collect();
        let closed: Vec<(String, Option<String>)> = self
            .focus_taking
            .iter()
            .filter(|(id, _)| !present.contains(id.as_str()))
            .map(|(id, opener)| (id.clone(), opener.clone()))
            .collect();
        for (id, opener) in closed {
            self.focus_taking.remove(&id);
            let was_inside = focus_before.is_some_and(|f| f.starts_with(&format!("{id}/")));
            if was_inside && let Some(target) = opener {
                let _ = self.focus.focus(&target);
            }
        }
        let opened: Vec<String> = present
            .iter()
            .filter(|id| !self.focus_taking.contains_key(**id))
            .map(|id| (*id).to_owned())
            .collect();
        for id in opened {
            let opener = self.focus.current().map(str::to_owned);
            let inside = format!("{id}/");
            if let Some(first) = self
                .focus
                .order()
                .iter()
                .find(|node| node.starts_with(&inside))
                .cloned()
            {
                let _ = self.focus.focus(&first);
            }
            self.focus_taking.insert(id, opener);
        }
    }

    /// Copy the focus tree's current node into the state the engine reads and
    /// the router aims with, and report whether that moved it. The one
    /// direction the copy runs is deliberate: [`FocusTree`] is the owner,
    /// `LayoutState::focused` is the projection.
    ///
    /// The answer matters because the projection is also what the focus ring
    /// is painted from: a move published *after* this pass petrified is a move
    /// this pass's picture does not show.
    fn publish_focus(&mut self) -> bool {
        let current = self.focus.current().map(str::to_owned);
        if self.state.focused == current {
            return false;
        }
        self.state.focused = current;
        // After the projection is written, so an application that reads
        // focus back during the call sees the node it is being told about.
        self.app.focus_changed(self.state.focused.as_deref());
        true
    }

    /// Copy the pointer snapshot into the state the engine reads, and report
    /// whether that moved anything. [`Host::publish_focus`]'s peer, with the
    /// same one-way rule: [`PointerState`] is the owner and `LayoutState` is
    /// the projection.
    ///
    /// All three members are published together and the answer is one
    /// `bool`, because they move together: a press sets `pressed` and
    /// `capture` in the same event, and asking for a repaint once per member
    /// would ask three times for one picture.
    /// Open a dialog the application asked for, and deliver one that has
    /// answered.
    ///
    /// Both halves in one place because they are one story told over several
    /// passes: a press asks, a thread browses, and some later pass hands the
    /// paths to [`App::files_dropped`] — the same door a drop comes through,
    /// so an application cannot tell a picked file from a dropped one.
    ///
    /// **The dialog runs on its own thread.** `rfd`'s blocking API on the
    /// frame thread would stop the window for as long as the operator
    /// browses, which on a portal dialog is unbounded. A `std` thread and an
    /// `mpsc` channel rather than an async runtime: this host has no
    /// executor, and pulling one in to await a single dialog would be the
    /// larger change.
    ///
    /// The repaint request on delivery is not optional. Nothing else wakes
    /// the window when a thread finishes minutes after the last input, so
    /// without it the picked files sit in the channel until the operator
    /// happens to move the mouse.
    #[cfg(not(target_arch = "wasm32"))]
    fn serve_file_dialog(&mut self, ctx: &Context) {
        if let Some(rx) = self.picking.as_ref() {
            match rx.try_recv() {
                Ok(paths) => {
                    self.picking = None;
                    if !paths.is_empty() {
                        self.app.files_dropped(&paths);
                        ctx.request_repaint();
                    }
                }
                // The thread ended without sending: the operator cancelled,
                // or the portal failed. Either way there is nothing to
                // deliver and the slot is free again.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.picking = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let Some(pick) = self.app.file_request() else {
            return;
        };
        if self.picking.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("petra-file-dialog".to_owned())
            .spawn(move || {
                let mut dialog = rfd::FileDialog::new();
                if !pick.extensions.is_empty() {
                    let names: Vec<&str> = pick.extensions.iter().map(String::as_str).collect();
                    dialog = dialog.add_filter("files", &names);
                }
                let picked = if pick.multiple {
                    dialog.pick_files().unwrap_or_default()
                } else {
                    dialog.pick_file().into_iter().collect()
                };
                // Send before the repaint: the pass this wakes has to find
                // the paths already in the channel, or it wakes for nothing
                // and the next delivery waits for the pass after that.
                let _ = tx.send(picked);
                ctx.request_repaint();
            });
        // A host that cannot spawn a thread is a host under a resource limit,
        // and the honest answer is that no dialog opened. Dropping `rx` on
        // that path leaves `picking` at `None`, so the next request tries
        // again rather than being wedged behind a dialog that never was.
        if spawned.is_ok() {
            self.picking = Some(rx);
        }
    }

    /// Seat an already-answered dialog: the picking thread has sent its
    /// paths and ended, which is the state the operator leaves behind by
    /// choosing files.
    ///
    /// Test-only, and it exists because the thread that normally fills this
    /// slot opens a window on the operator's desktop. No automated check in
    /// this workspace may do that, so the half that *can* be driven — the
    /// drain in [`Host::serve_file_dialog`], which is what actually reaches
    /// [`App::files_dropped`] — is driven from here instead. The spawn
    /// itself stays unverified, and is documented as such.
    #[cfg(all(test, not(target_arch = "wasm32")))]
    fn seat_picked_dialog(&mut self, paths: Vec<std::path::PathBuf>) {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(paths).expect("the receiver is alive on this line");
        self.picking = Some(rx);
    }

    /// wasm has no `rfd` and no threads to run one on. A page asking for a
    /// dialog gets none, and says so by nothing arriving at
    /// [`App::files_dropped`] — the same answer a cancelled dialog gives, so
    /// an application needs no second code path for the browser.
    #[cfg(target_arch = "wasm32")]
    fn serve_file_dialog(&mut self, _ctx: &Context) {
        let _ = self.app.file_request();
    }

    fn publish_pointer(&mut self) -> bool {
        let hovered = self.pointer.hovered().map(str::to_owned);
        let pressed = self.pointer.pressed().map(str::to_owned);
        let capture = self.pointer.capture().cloned();
        if self.state.hovered == hovered
            && self.state.pressed == pressed
            && self.state.capture == capture
        {
            return false;
        }
        self.state.hovered = hovered;
        self.state.pressed = pressed;
        self.state.capture = capture;
        true
    }

    /// Keyboard focus follows a primary press onto a focusable node so the
    /// flying caret retargets. Walks up the id path because hit-test may
    /// name a child (the checkbox box) of the interactive row.
    fn seat_pointer_focus(&mut self, id: &str) {
        let mut cur = id;
        loop {
            if self.focus.focus(cur).is_ok() {
                let _ = self.publish_focus();
                return;
            }
            match cur.rsplit_once('/') {
                Some((parent, _)) if !parent.is_empty() => cur = parent,
                _ => return,
            }
        }
    }

    /// Whether `event` changes the tree the *application* would build, as
    /// opposed to a projection this host applies to the frame it already
    /// holds.
    ///
    /// This is [`Host::deliver_input`]'s return value, which is
    /// [`Host::can_reuse_frame`]'s `had_input`. A pass whose every event
    /// answers `false` here, with a caret in flight, repaints the **last**
    /// frame and never asks the application for a tree — so an event that
    /// answers `false` while the application acted on it is an event whose
    /// consequence is not painted.
    fn picture_must_rebuild(&self, event: &InputEvent) -> bool {
        match event {
            // A move with **nothing captured** is a projection: hover is
            // re-derived from the frame this host already has, and a
            // compositor that repeats the pointer position every vsync must
            // not rebuild the gallery under a caret hop.
            //
            // A move **under capture** is not that, and calling it one cost
            // the operator row 30. The engine's rule is that a press on an
            // `Interaction::Drag` node grants the capture and every positional
            // event until the gesture ends routes to the holder
            // unconditionally — because the holder is running a gesture, and a
            // gesture is the application's own state moving. A slider press
            // also seats focus on the handle, which starts the 160 ms caret
            // hop; with this arm answering `false`, every move for the rest of
            // that hop repainted the stale picture while `Slider::volume` went
            // on changing underneath it. It is a loop, not a one-off: the
            // frame that finally rebuilds moves the handle, the caret chases
            // the handle it just moved, and the chase freezes the next nine
            // frames. Measured headlessly at four units of travel per 60 Hz
            // frame, the painted handle updated once every nine frames and
            // trailed the pointer by up to 56 units — the operator's "laggy
            // compared to the cursor". See
            // `shots::tests::the_slider_handle_is_painted_under_the_pointer_on_every_frame_of_a_drag`.
            InputEvent::PointerMoved { .. } => self.pointer.capture().is_some(),
            // Tab's release would otherwise petrify mid-flight.
            InputEvent::Key { pressed: false, .. } => false,
            _ => true,
        }
    }

    fn deliver_input(&mut self, ctx: &Context) -> bool {
        let events = ctx.input(|i| i.events.clone());
        let translated = self.translator.translate_all(&events);
        if translated.is_empty() {
            return false;
        }
        let mut invalidate = false;
        // Routing needs a frame to hit-test against. The first pass has none,
        // so its events are dropped rather than delivered to nothing — and the
        // application is told, through an `Unrouted` route and a `None`
        // frame, instead of the event just vanishing.
        for event in &translated {
            if self.traverse(event) {
                invalidate = true;
                continue;
            }
            // Everything goes through `PointerState`, including the events
            // that are not pointer events at all: a window blur and an
            // Escape both end a gesture in flight
            // (`contracts/interaction-state.md` §7), so a router that only
            // saw the pointer would leave a drag running with nothing left to
            // finish it.
            //
            // This is also where lane A's pointer-exit seam closes.
            // `InputEvent::PointerLeft` carries no position, so it is aimed
            // by memory rather than by hit test:
            // `gorgon_petra::input::route` has no memory and must pass
            // `None` to `route_pointer_exit`, which reports the exit dropped.
            // `PointerState::route` hands it the real hovered id instead, and
            // `pointer_exit_lands_on_the_hovered_node` below asserts the two
            // agree.
            let routing = match &self.last_frame {
                Some(frame) => self.pointer.route(
                    frame,
                    self.state.focused.as_deref(),
                    &self.last_scopes,
                    event,
                ),
                None => PointerRouting {
                    outcome: RouteOutcome {
                        route: Route::Unrouted {
                            reason: "no frame has been placed yet".into(),
                        },
                        dismiss: Vec::new(),
                    },
                    ended: None,
                },
            };
            if !routing.outcome.dismiss.is_empty() {
                self.app.dismissed(&routing.outcome.dismiss);
            }
            if let (
                InputEvent::PointerPressed {
                    button: PointerButton::Primary,
                    ..
                },
                Route::Pointer { node },
            ) = (event, &routing.outcome.route)
            {
                self.seat_pointer_focus(node);
            }
            // A wheel routes like any other pointer event
            // (`InputEvent::Scroll` is positional, `required_interaction`
            // maps it to `Interaction::Scroll`) — but unlike a click or a
            // hover, moving it is this crate's own job, not the
            // application's: nothing in `gorgon-petra` ever turns a routed
            // `Scroll` into a `scroll_offsets` write (see this file's
            // `apply_scroll` doc). Before the application hears it, so a
            // component that reads its own scroll position through
            // `LayoutState` on the same event is reading the number this
            // pass just wrote, not last pass's.
            if let (InputEvent::Scroll { delta, .. }, Route::Pointer { node }) =
                (event, &routing.outcome.route)
            {
                self.apply_scroll(node, *delta);
            }
            // And a press or a captured move over a selectable run is a text
            // selection, moved here for the same reason and at the same
            // point in the pass: nothing in `gorgon-petra` can turn a window
            // position into a byte offset, because that needs shaped glyphs
            // and shaping is this crate's side of the U-09 boundary. Before
            // the application hears the event, so a copy control reading the
            // selection off this frame reads what this press just wrote.
            self.apply_text_selection(event);
            // Re-borrowed: `seat_pointer_focus` and `apply_scroll` above took
            // `&mut self`. Nothing between the routing and here places a
            // frame, so this is the frame the routing was computed against,
            // or `None` on the first pass, matching the `Unrouted` route.
            let frame = self.last_frame.as_ref();
            self.app.handle(event, &routing.outcome.route, frame);
            // The cause, then the consequence: a component hears the release
            // (or the blur, or the Escape) and then hears what it did to the
            // gesture, so it never has to infer the second from the first.
            if let Some(ended) = routing.ended {
                self.app.handle(&ended.event(), &ended.route(), frame);
            }
            // The copy chord over a selection. Here and not in the
            // application, for the reason `copy_selection` gives; an
            // application that answers the chord itself still wins, because
            // `App::clipboard_request` is drained first. After the
            // application's own turn, and after `frame`'s borrow ends.
            self.copy_selection(event);
            if self.picture_must_rebuild(event) {
                invalidate = true;
            }
        }
        invalidate
    }

    /// Move focus if `event` is a traversal keystroke, and report whether it
    /// was consumed.
    ///
    /// **Tab and Shift+Tab always traverse**, whatever the focused node
    /// declares, and never reach [`App::handle`]. FR-025 makes every
    /// interactive node reachable by keyboard; if a focused node could swallow
    /// Tab, reachability would depend on every application getting that right,
    /// and the authoring vocabulary has no "this node wants a literal Tab"
    /// opt-in to make swallowing safe. A node that needs one needs that flag
    /// first — see this crate's README.
    ///
    /// **Home and End traverse only when the focused node cannot use them**,
    /// i.e. when it does not declare [`Interaction::Key`]. Unlike Tab they
    /// have an older meaning inside a node (start and end of a line), and a
    /// text field must keep it. Nothing in `specs/003-petra-layout-engine`
    /// binds these two keys; this is the host's choice, recorded here.
    ///
    /// Arrow keys are *not* traversal: they route to the focused node like any
    /// other key. Directional movement inside a collection needs a rule the
    /// spec does not define yet and a focus tree that knows about geometry.
    fn traverse(&mut self, event: &InputEvent) -> bool {
        let InputEvent::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            return false;
        };
        if modifiers.ctrl || modifiers.alt || modifiers.meta {
            return false;
        }
        match key {
            KeyCode::Tab if modifiers.shift => self.step_focus(-1),
            KeyCode::Tab => self.step_focus(1),
            KeyCode::Home | KeyCode::End if modifiers.shift => return false,
            KeyCode::Home if !self.focus_accepts(Interaction::Key) => self.focus.first(),
            KeyCode::End if !self.focus_accepts(Interaction::Key) => self.focus.last(),
            _ => return false,
        }
        // Later events in this same batch must aim at the node focus just
        // moved to, not at the one it left. This one runs before petrify, so
        // the frame about to be placed already shows it.
        let _ = self.publish_focus();
        true
    }

    /// Move focus by `dir` (`1` for Tab, `-1` for Shift+Tab), scrolling the
    /// nearest enclosing `Scroll` into view first when the target
    /// [`FocusTree::order`] does not have yet.
    ///
    /// [`FocusTree::next`]/[`FocusTree::previous`] alone cannot reach an
    /// off-screen node: `order` only ever holds what the last frame actually
    /// placed on screen. [`FocusTree::reachable`] answers the wider
    /// question — what is the very next focusable node in document order,
    /// on screen or not — and when that answer is already in `order`, this
    /// is exactly `next`/`previous` (same candidates, same wrap, cheaper).
    /// When it is not, [`Self::scroll_into_view`] writes a `scroll_offsets`
    /// entry meant to bring it on screen next pass, and the actual
    /// [`FocusTree::focus`] call is deferred to [`Self::pending_focus`],
    /// which [`Self::pass`] retries once that frame exists.
    ///
    /// A no-op before the first frame, or with nothing focusable at all —
    /// [`FocusTree::reachable`] answers `None` in both.
    fn step_focus(&mut self, dir: isize) {
        let Some(target) = self.last_frame.as_ref().and_then(|frame| {
            self.focus
                .reachable(&frame.placements, &self.last_scopes, dir)
        }) else {
            return;
        };
        if self.focus.order().contains(&target) {
            // Already on screen: the ordinary move is enough, and it is
            // exactly what `reachable` just computed.
            let _ = self.focus.focus(&target);
            return;
        }
        // `scroll_into_view` takes the frame by reference rather than
        // reading `self.last_frame` itself, so `Self::pass`'s own
        // stationary-focus reconciliation can call it against a frame it
        // just petrified, before that frame is `self.last_frame` at all.
        // `take`/put-back is what lets this call it against `self` too,
        // without holding an immutable borrow of the field across the
        // `&mut self` call.
        if let Some(frame) = self.last_frame.take() {
            let _ = self.scroll_into_view(&frame, &target);
            self.last_frame = Some(frame);
        }
        self.pending_focus = Some(target);
    }

    /// The `Placement` nearest `id`, walking upward, that is itself a
    /// `Scroll` container — including `id`'s own placement, if it happens
    /// to be one. `None` when `id` names no placement in `frame`, or no
    /// `Scroll` encloses it.
    ///
    /// Walks `Placement::parent` the way `gorgon_petra::focus`'s
    /// `blocking_scopes` walks it for a blocking surface's scope: `Placement`
    /// is public with no sealed constructor, so a malformed `parent` chain
    /// (past the slice's end, or cyclic) is refused rather than trusted —
    /// bounded by the placement count, the same defence.
    fn scroll_ancestor<'a>(frame: &'a PetrifiedFrame, id: &str) -> Option<&'a Placement> {
        let start = frame.placements.iter().position(|p| p.id == id)?;
        let mut cursor = Some(start);
        let mut steps = 0usize;
        while let Some(i) = cursor {
            let placement = frame.placements.get(i)?;
            if placement.kind == NodeKind::Scroll {
                return Some(placement);
            }
            cursor = placement.parent;
            steps += 1;
            if steps > frame.placements.len() {
                return None;
            }
        }
        None
    }

    /// `scroll`'s own placed child, i.e. what its `scroll_offsets` entry
    /// actually moves. A `Scroll` node has at most one child by construction
    /// (`gorgon_petra::layout::scroll::place` only ever looks at
    /// `node.children.first()`), so the first placement whose `parent` is
    /// `scroll`'s own index is the whole answer.
    fn scroll_content<'a>(frame: &'a PetrifiedFrame, scroll: &str) -> Option<&'a Placement> {
        let idx = frame.placements.iter().position(|p| p.id == scroll)?;
        frame.placements.iter().find(|p| p.parent == Some(idx))
    }

    /// The axis `scroll`'s content actually overflows on, inferred from
    /// geometry rather than looked up: [`PetrifiedFrame`] carries placements,
    /// not the `ViewNode` tree that named the axis, and `Placement` records
    /// no `ScrollProps`.
    ///
    /// `gorgon_petra::layout::scroll::place`'s own doc is the invariant this
    /// leans on: a `Scroll`'s child is placed at its full, unclipped content
    /// size, and clipped only by the paint clip narrowed to the viewport —
    /// so the axis whose content extent exceeds its clip is the scrolling
    /// axis. Content that overflows on neither (nothing to scroll) defaults
    /// to `Vertical`, which is inert either way: every caller here clamps
    /// against that axis's own overflow, and an axis with none clamps to
    /// `0.0`.
    fn scroll_axis(frame: &PetrifiedFrame, scroll: &str) -> Axis {
        let Some(content) = Self::scroll_content(frame, scroll) else {
            return Axis::Vertical;
        };
        let overflow_h = content.rect.h - content.clip.h;
        let overflow_w = content.rect.w - content.clip.w;
        if overflow_h >= overflow_w {
            Axis::Vertical
        } else {
            Axis::Horizontal
        }
    }

    /// How far `scroll`'s content can move along `axis` past its own
    /// viewport, from the last placed frame — `0.0` when `scroll` names no
    /// placement, has no content, or does not overflow that axis.
    fn scroll_overflow(frame: &PetrifiedFrame, scroll: &str, axis: Axis) -> f32 {
        let Some(content) = Self::scroll_content(frame, scroll) else {
            return 0.0;
        };
        (content.rect.size().along(axis) - content.clip.size().along(axis)).max(0.0)
    }

    /// A rect's origin along `axis`: `x` for horizontal, `y` for vertical.
    /// `gorgon-petra`'s own `layout::scroll::origin_along`, restated here —
    /// that one is private to its module and this crate has no `ViewNode`
    /// tree to ask `scroll::place` to do the walk for it.
    fn leading(rect: Rect, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => rect.x,
            Axis::Vertical => rect.y,
        }
    }

    /// Write `desired` into `scroll`'s `scroll_offsets` entry, clamped to
    /// `[0, max_offset]`, and report whether the stored value changed.
    ///
    /// `max_offset` is a parameter rather than read from `self.last_frame`
    /// here: `Self::scroll_into_view`'s stationary-focus caller
    /// (`Host::pass`) computes it against a frame it just petrified, not
    /// yet `self.last_frame` at that point in the pass, and this must
    /// clamp against the frame its own caller actually measured rather
    /// than silently read a stale or absent one. Both callers that still
    /// mean "the frame on screen" (`apply_scroll`, and
    /// `scroll_into_view`'s `step_focus` caller) pass
    /// `Self::scroll_overflow(self.last_frame.., ..)` themselves, so this
    /// changes what is computed by nobody — only where.
    ///
    /// One frame stale by construction there, same as before: input is
    /// delivered ahead of this pass's own `petrify` (`Host::pass`'s
    /// ordering), so the content extent it clamps against is whatever the
    /// previous frame measured, not the one about to be placed.
    /// `gorgon_petra::layout::scroll::place` re-clamps against its own
    /// frame's real extent on every placement regardless — so a view that
    /// shrank between the write and the next placement is corrected there,
    /// one frame later, never left stuck past its own end.
    fn write_scroll_offset(&mut self, scroll: &str, desired: f32, max_offset: f32) -> bool {
        let next = desired.clamp(0.0, max_offset);
        let current = self
            .state
            .scroll_offsets
            .get(scroll)
            .copied()
            .unwrap_or(0.0);
        if next == current {
            return false;
        }
        self.state.scroll_offsets.insert(scroll.to_owned(), next);
        true
    }

    /// Turn a routed wheel event into a `scroll_offsets` write.
    ///
    /// `node` is the id [`gorgon_petra::input::hit_test`] routed the event
    /// to — a placement declaring `Interaction::Scroll`, which every node
    /// this crate's applications build is itself a `Scroll` container (see
    /// `catalog.rs`'s `index_pane`). A `node` that names no `Scroll`
    /// placement in the last frame is a no-op: nothing declares that
    /// interaction, so nothing routed here should exist, and this refuses
    /// to invent a scroll entry under an arbitrary id.
    ///
    /// This is the missing half of the wiring `gorgon-petra-egui/src/input.rs`
    /// and `gorgon-petra`'s own `input.rs` already build:
    /// `EventTranslator::translate` turns egui's wheel into
    /// `InputEvent::Scroll`, `route`/`hit_test` aim it at a `Scroll` node —
    /// and until this method, nothing consumed it. `grep scroll_offsets`
    /// across the crate used to turn up only test harnesses writing it
    /// directly (`testing.rs`, and `catalog.rs`'s own
    /// `writing_the_index_pane_scroll_offset_reveals_row_forty_two`).
    /// Move the text selection from a routed pointer event.
    ///
    /// The whole of the selection gesture, and it is deliberately small:
    /// a press anchors, a captured move extends, and a press anywhere else
    /// clears. A release does nothing, because a selection outlives the drag
    /// that made it — that is the point of it.
    ///
    /// # Why this is the host's and not the application's
    ///
    /// Every other interaction the application could express for itself. This
    /// one it could not: the map from a window position to a byte offset is
    /// per-glyph advances and line breaks, which live only in the shaped run,
    /// and `contracts/view-tree.md`'s U-09 boundary keeps shaping on this side
    /// of the wall. An application asked to do it would need a font.
    ///
    /// So the selection is host-derived interaction state, published through
    /// [`LayoutState`] exactly as `hovered`, `pressed` and the scroll offsets
    /// are, and the application learns what was selected by reading the range
    /// off the frame it is handed
    /// ([`gorgon_petra::frame::PaintContent::selection`]). One owner, one
    /// number, and no way for the highlight and the clipboard to disagree.
    ///
    /// # What makes a run selectable
    ///
    /// Nothing the component says. [`gorgon_petra::input::hit_text`] derives
    /// it from the frame: running text is selectable, and text inside a
    /// control belongs to the control. That function carries the rule and the
    /// reasoning; this one carries the gesture.
    ///
    /// It used to be two declarations, both the component's — a
    /// [`Interaction::Drag`] and a [`crate::paint::SELECTION_SLOT`] binding — and exactly
    /// one component in the library ever made them. The operator, looking at
    /// the catalog: *"a lot of these text elements are not highlightable,
    /// like the lists. A lot are like this."* A rule every author has to
    /// remember is a rule that is remembered once.
    ///
    /// # Why the drag is tracked here and not read off the capture
    ///
    /// A capture is granted to a node that declares [`Interaction::Drag`],
    /// and a paragraph declares nothing. So the extend step cannot ask
    /// [`gorgon_petra::input::PointerState`] whether this gesture is still
    /// running; it has to remember. `selection_drag` is that memory, set when
    /// a press anchors and cleared by the release, the blur, and the pointer
    /// leaving — the same three ends a capture has. It gives the same
    /// guarantee the capture test gave, in the words of the gesture it is
    /// actually about: a bare hover across a paragraph has no button down,
    /// so it does not rewrite a selection made in it.
    fn apply_text_selection(&mut self, event: &InputEvent) {
        match event {
            InputEvent::PointerPressed {
                pos,
                button: PointerButton::Primary,
                ..
            } => {
                // A press on anything that is not a selectable run drops the
                // selection, which is what every text surface does and what
                // stops a stale highlight outliving the block it was made in.
                let node = self
                    .last_frame
                    .as_ref()
                    .and_then(|frame| gorgon_petra::input::hit_text(frame, *pos, &self.last_scopes))
                    .map(|run| run.id.clone());
                let theme = self.presenter.current();
                self.state.text_selection = node.and_then(|node| {
                    Self::text_offset(
                        &mut self.shaper,
                        self.last_frame.as_ref(),
                        &node,
                        *pos,
                        theme.as_ref(),
                    )
                    .map(|at| TextSelection::new(node, at))
                });
                self.selection_drag = self.state.text_selection.is_some();
            }
            InputEvent::PointerReleased {
                button: PointerButton::Primary,
                ..
            }
            | InputEvent::WindowBlurred
            | InputEvent::PointerLeft => self.selection_drag = false,
            InputEvent::PointerMoved { pos } => {
                if !self.selection_drag {
                    return;
                }
                // `reach_text` and not the press's `hit_text`: the focus end
                // follows the pointer into whatever run it has got to, which
                // is how a drag leaves the run it started in. The anchor is
                // untouched, so a drag that turns around and goes back up the
                // page shrinks rather than re-anchoring.
                let Some(node) = self
                    .last_frame
                    .as_ref()
                    .and_then(|frame| {
                        gorgon_petra::input::reach_text(frame, *pos, &self.last_scopes)
                    })
                    .map(|run| run.id.clone())
                else {
                    return;
                };
                let theme = self.presenter.current();
                if let Some(at) = Self::text_offset(
                    &mut self.shaper,
                    self.last_frame.as_ref(),
                    &node,
                    *pos,
                    theme.as_ref(),
                ) && let Some(selection) = self.state.text_selection.as_mut()
                {
                    selection.extend_to(node, at);
                }
            }
            _ => {}
        }
    }

    /// The byte offset in `node`'s painted string under the window position
    /// `pos`, or `None` when `node` paints no text this frame.
    ///
    /// Which runs are selectable is [`gorgon_petra::input::hit_text`]'s
    /// answer and not this function's. This one only measures, and it
    /// measures whatever `Text` placement it is handed.
    ///
    /// An associated function taking the two fields it needs rather than a
    /// method: it reads `last_frame` and writes through `shaper`, and the
    /// caller is holding a third field at the same time.
    ///
    /// The request is rebuilt exactly as [`crate::paint`] builds it, inset and
    /// all, so the galley this asks comes back from the shaper's cache as the
    /// same `Arc` the painter drew. Measuring against a differently-shaped
    /// galley would put the offset a word away from the pointer.
    ///
    /// `Input` used to be refused here on exactly that ground — the painter
    /// insets a field's galley by `spacing-04`, and an offset measured at the
    /// full width would be twelve units out. The operator, on the form page:
    /// *"text is not highlightable in form, which is a text entry box"*. The
    /// inset was a reason to *apply* the inset, not a reason to refuse the
    /// kind, and it is applied below from the same token the painter reads.
    fn text_offset(
        shaper: &mut GalleyShaper,
        frame: Option<&PetrifiedFrame>,
        node: &str,
        pos: Point,
        colors: &dyn crate::paint::TokenSource,
    ) -> Option<usize> {
        let frame = frame?;
        let index = frame.placements.iter().position(|p| p.id == node)?;
        let placement = &frame.placements[index];
        if !matches!(placement.kind, NodeKind::Text | NodeKind::Input) {
            return None;
        }
        let content = frame.content.get(index)?;
        let text = content.text.as_ref()?;
        // From the placement's own kind, never from a constant. Handing this
        // a field's inset for every run put twelve units under every press in
        // the library and shrank each selection to about a third of the
        // letters it crossed; two capture tests caught it by looking at the
        // pixels, which is what they are for.
        let inset_x = crate::paint::text_inset_x(placement.kind, colors);
        let request = TextRequest {
            text: &text.text,
            style: text.style.as_deref(),
            wrap: text.wrap,
            max_lines: text.max_lines,
            available_width: Some((placement.rect.w - 2.0 * inset_x).max(0.0)),
        };
        let galley = shaper.galley(&request);
        // The painter centres a field's galley in the chrome; a run's galley
        // starts at the top of its rect. Undoing both shifts is what makes
        // the offset name the letter the pointer is actually over.
        let inset_y = if placement.kind == NodeKind::Input {
            ((placement.rect.h - galley.rect.height()) * 0.5).max(0.0)
        } else {
            0.0
        };
        Some(crate::text::byte_offset_at(
            &galley,
            egui::vec2(
                pos.x - placement.rect.x - inset_x,
                pos.y - placement.rect.y - inset_y,
            ),
        ))
    }

    /// Put the selected bytes on the clipboard when the operator asks for
    /// them, and report whether there were any.
    ///
    /// # Why the host and not the application
    ///
    /// The same argument [`App::apply_text_selection`] rests on, one step
    /// further along. A selection is host-derived state: no application makes
    /// one, none is told when one changes, and none can turn a window
    /// position into a byte offset. An application asked to answer the chord
    /// would be answering a question about a number it does not own.
    ///
    /// It was the application's until 2026-09-06, on the catalog's row 6,
    /// which is exactly one page of forty-two. The operator had just been
    /// given a highlight on every run in the library
    /// (`gorgon_petra::input::hit_text`), and a highlight nobody can copy is
    /// decoration.
    ///
    /// **The bytes come out of the frame.** `PaintContent` carries the string
    /// that was painted and the range the engine wrote onto it, so what is
    /// taken here is by construction what is under the highlight — across
    /// every run the selection crosses, in the order they are painted.
    /// [`gorgon_petra::frame::PetrifiedFrame::selected_text`] is the whole of
    /// it, and there is no second copy of either to drift.
    ///
    /// # Why one event and not a chord
    ///
    /// This matched `Ctrl`-or-`Cmd` **+ C** as a keystroke until 2026-09-07,
    /// and on a real window it never fired once. `egui-winit` recognises the
    /// copy command in its own key handler and pushes `egui::Event::Copy`
    /// *instead of* the keystroke, so the keystroke this waited for is one no
    /// platform sends. Five capture tests asserted the clipboard and all five
    /// passed, because the driver's injector was synthesizing the same
    /// keystroke by hand — a closed loop between two pieces of this crate with
    /// the window's own behaviour outside it. The operator found it: *"I can't
    /// copy and paste from lists, I suspect I cant control c in more places
    /// too."*
    ///
    /// Both halves moved. [`gorgon_petra::input::InputEvent::Copy`] carries
    /// the intent, and `crate::inject::push_key` now emits `Event::Copy` for
    /// the copy chord exactly as `egui-winit` does, so the driver and the
    /// window produce the same event.
    ///
    /// # Why it answers the bare keystroke as well
    ///
    /// Because the seam that broke cannot be tested from this workspace, and
    /// a feature must not rest on one reading of an untestable seam.
    ///
    /// `winit::event::KeyEvent` carries a `pub(crate) platform_specific`
    /// field, so no crate but `winit` can construct one. There is therefore no
    /// way, short of a physical keyboard on a mapped window, to prove that a
    /// Ctrl+C press becomes `egui::Event::Copy` — only to read `egui-winit`'s
    /// `is_copy_command` and believe it. Not reading it is what caused this
    /// defect, and the cost was a feature green in every test and dead in the
    /// operator's hands.
    ///
    /// So both shapes are answered. This is not a fallback hiding a broken
    /// primary path: they are two real events that both mean *copy this*, and
    /// answering one of them would bet the feature on an assumption nothing
    /// here can check. `Event::Copy` is what a winit window and a browser both
    /// send today; the chord is what any input layer that does not pre-digest
    /// it would send.
    ///
    /// The hole this could re-open — a regressed injector staying green
    /// through the keystroke — is closed somewhere else, by
    /// `inject::tests::a_copy_chord_injects_the_platforms_copy_event_and_never_a_keystroke`,
    /// which asserts the raw egui events and cannot be satisfied by a
    /// keystroke.
    fn copy_selection(&mut self, event: &InputEvent) -> bool {
        if !asks_for_a_copy(event) {
            return false;
        }
        let Some(selected) = self
            .last_frame
            .as_ref()
            .and_then(gorgon_petra::frame::PetrifiedFrame::selected_text)
        else {
            return false;
        };
        self.pending_copy = Some(selected);
        true
    }

    fn apply_scroll(&mut self, node: &str, delta: Size) -> bool {
        let Some(frame) = self.last_frame.as_ref() else {
            return false;
        };
        let Some(placement) = frame.placement(node) else {
            return false;
        };
        if placement.kind != NodeKind::Scroll {
            return false;
        }
        let axis = Self::scroll_axis(frame, node);
        let max_offset = Self::scroll_overflow(frame, node, axis);
        let current = self.state.scroll_offsets.get(node).copied().unwrap_or(0.0);
        // `egui::Event::MouseWheel`'s own doc: "a positive Y-value indicates
        // the content is being moved down" — the opposite sense from this
        // offset, which *grows* to reveal content further along the axis
        // (`layout::scroll::place`: `main_origin = origin_along(content,
        // axis) - offset`, so a larger offset moves the content the other
        // way).
        let desired = current - delta.along(axis);
        self.write_scroll_offset(node, desired, max_offset)
    }

    /// Scroll `target`'s nearest enclosing `Scroll` just far enough that
    /// `target`'s own rect is inside its clip, along whichever axis that
    /// `Scroll` actually scrolls. Reports whether it wrote a new offset.
    ///
    /// Takes `frame` explicitly rather than reading `self.last_frame`: this
    /// has two callers now. `step_focus` still means "the frame on screen"
    /// (Tab is delivered before this pass's own petrify, so `last_frame` is
    /// exactly that), and its target's rect and clip do not overlap at all
    /// along the scrolling axis on entry — `step_focus` only calls this for
    /// an id absent from `FocusTree::order`, and `Placement::is_visible` —
    /// `rect.overlaps(clip)` — is exactly what `order` is filtered by — so
    /// which side it fell off on is unambiguous there. `Host::pass`'s
    /// stationary-focus reconciliation calls it against a frame it just
    /// petrified, for a node that *was* on screen a moment ago and may now
    /// only partly overlap its clip; the "already within range" arm below
    /// is for that caller, not a case that "should not happen".
    fn scroll_into_view(&mut self, frame: &PetrifiedFrame, target: &str) -> bool {
        let Some(placement) = frame.placement(target) else {
            return false;
        };
        let (rect, clip) = (placement.rect, placement.clip);
        let Some(scroll) = Self::scroll_ancestor(frame, target).map(|p| p.id.clone()) else {
            return false;
        };
        let axis = Self::scroll_axis(frame, &scroll);
        let target_lead = Self::leading(rect, axis);
        let target_trail = target_lead + rect.size().along(axis);
        let view_lead = Self::leading(clip, axis);
        let view_trail = view_lead + clip.size().along(axis);
        let current = self
            .state
            .scroll_offsets
            .get(&scroll)
            .copied()
            .unwrap_or(0.0);
        let desired = if target_trail <= view_lead {
            // Entirely above/left of the viewport: bring its leading edge
            // to the viewport's leading edge, the least scroll that shows
            // all of it.
            current + (target_lead - view_lead)
        } else if target_lead >= view_trail {
            // Entirely below/right: align the trailing edges instead.
            current + (target_trail - view_trail)
        } else {
            // Already within range on this axis: nothing to scroll for.
            // `Host::pass`'s caller falls back to moving focus instead when
            // this happens — a partial overlap along the *other* axis, or a
            // clip that collapsed to zero height, is not something a scroll
            // offset can fix.
            return false;
        };
        let max_offset = Self::scroll_overflow(frame, &scroll, axis);
        self.write_scroll_offset(&scroll, desired, max_offset)
    }

    /// Whether the focused node in the last placed frame declares
    /// `interaction`. `false` when nothing is focused, when the focused node
    /// is not in that frame, or before the first frame exists.
    fn focus_accepts(&self, interaction: Interaction) -> bool {
        let Some(frame) = &self.last_frame else {
            return false;
        };
        let Some(id) = self.focus.current() else {
            return false;
        };
        frame
            .placement(id)
            .is_some_and(|p| !p.semantics.disabled && p.semantics.actions.contains(&interaction))
    }

    /// Ask for another frame only when something is still moving.
    ///
    /// This is the whole of the zero-idle contract on this side
    /// (`contracts/animation.md` §"Frame scheduling"): egui already repaints on
    /// input, so the host's only job is to *not* ask when nothing is running.
    /// The decision itself lives in [`crate::schedule::request_if_moving`],
    /// which reads `gorgon_petra::anim::wants_frame` — one rule, one place, so
    /// the host and the engine cannot drift apart on what "moving" means.
    fn schedule(&self, ctx: &Context, frame: &PetrifiedFrame) {
        crate::schedule::request_if_moving(ctx, frame);
    }

    /// Declare the transition definitions this application's trees name.
    ///
    /// Registers them with the engine **and** declares their names into the
    /// registry tree acceptance runs against, in one motion
    /// ([`gorgon_petra::anim::TransitionRegistry::declare_into`]): a name the
    /// engine can resolve is a name a tree may use, and one it cannot is a
    /// tree-acceptance error, which is what
    /// `gorgon_petra::tree::TransitionRef`'s own doc promises.
    ///
    /// Merges `definitions` onto the shipped registry and resets every
    /// trajectory in flight. An application name wins when it collides with
    /// a shipped one; a name the application does not mention stays shipped
    /// so a `toggle` still slides after a host registers the cat walk.
    pub fn set_transitions(&mut self, definitions: TransitionRegistry) -> &mut Self {
        let mut merged = gorgon_petra::anim::shipped_registry();
        for name in definitions.names() {
            if let Some(def) = definitions.get(name) {
                merged.register(name.to_owned(), def.clone());
            }
        }
        merged.declare_into(&mut self.registry);
        self.motion = FrameMotion::new(merged);
        self
    }

    /// Turn reduced motion on or off.
    ///
    /// The host is where this belongs: it is a platform accessibility
    /// preference, not a design-system fact and not part of the theme. Putting
    /// it on `Theme` or `ThemeSnapshot` would give it a revision, and a theme
    /// revision is both a measure-cache key and a frame-digest input — so
    /// toggling it would change the digest of every frame in the application,
    /// including frames with no motion in them, which is the opposite of
    /// FR-031's "identical end states". The argument in full, with the two
    /// rejected alternatives, is in
    /// `.agents/notes/implemented/architecture/2026-08-23-petra-motion.md`.
    ///
    /// Turning it on completes every running movement transition instantly at
    /// its target.
    pub fn set_reduced_motion(&mut self, on: bool) -> &mut Self {
        self.motion.set_reduced_motion(on);
        self.caret.set_reduced_motion(on);
        self
    }

    /// The motion state: frame counts and the zero-idle audit.
    #[must_use]
    pub fn motion(&self) -> &FrameMotion {
        &self.motion
    }

    /// The flying underline. Tests read this; painting reads [`FocusCaret::rect`].
    #[must_use]
    pub fn caret(&self) -> &FocusCaret {
        &self.caret
    }

    /// How many tessellated meshes the last full paint kept. Tests assert a
    /// hop has a scene to replay; zero means the bake never ran.
    #[must_use]
    pub fn scene_mesh_count(&self) -> usize {
        self.scene.len()
    }

    /// Whether a GPU scene texture is bound and ready to blit. Headless lib
    /// tests stay `false` so they do not need a GPU.
    #[must_use]
    pub fn scene_texture_bound(&self) -> bool {
        self.scene_texture_has_front()
    }

    /// How many times this host has called `paint_frame_with_caret`. A hop
    /// that reuses a cached scene must not increment this.
    #[must_use]
    pub fn full_paint_count(&self) -> u32 {
        self.full_paints
    }

    /// Passes in the hop still in flight, or 0 when idle.
    #[must_use]
    pub fn hop_passes(&self) -> u32 {
        self.hop_passes
    }

    /// Passes the last completed hop took. 0 before any hop has landed.
    #[must_use]
    pub fn last_hop_passes(&self) -> u32 {
        self.last_hop_passes
    }

    /// Replace the set of contributed surfaces this shell shows.
    ///
    /// The shell end of `contracts/surface-contribution.md`'s mount path.
    /// Every frame from here on splices each contribution into the node its
    /// `slot` names, under the key `ui:<id>`.
    ///
    /// # What this is not
    ///
    /// It is not a transport. `petra-egui` depends on nothing under
    /// `gorgon/` and must not: a ctl client inside the renderer would link
    /// the daemon, its kernel and its Lua host into the process that draws.
    /// Whoever holds the ctl connection calls this — spec 004's shell binary,
    /// which may depend on both crates. The argument is an owned snapshot,
    /// so nothing on the frame path can call back into a fiber
    /// (`contracts/surface-contribution.md` §8): there is no timeout to tune
    /// because there is no call.
    ///
    /// # Order
    ///
    /// Contributions are spliced into a slot in the order given here and are
    /// never re-sorted. §6 makes that order `TraceSeq` of the `contribute`
    /// call, which only the daemon knows.
    ///
    /// # Cost
    ///
    /// Expansion and stage-2 acceptance run **here**, once, not per frame.
    /// A frame pays for a walk of the application's tree and a shallow clone
    /// of the path down to each mount point; every other `Arc` in the tree is
    /// handed on untouched, so the incremental-frame proof (`Arc::ptr_eq`)
    /// still holds for the subtrees this did not enter.
    ///
    /// # Size
    ///
    /// Expansion is depth-capped ([`registry::MAX_DEPTH`]) and cannot loop —
    /// a constructor in `gorgon_petra::component` has no way to name another
    /// component — so an expanded tree is bounded by the size of what was
    /// handed in. Bounding *that* is the caller's job: on the shipped path
    /// the daemon's 16 MiB ctl frame cap is what does it.
    pub fn set_contributions(&mut self, contributions: Vec<Contribution>) -> &mut Self {
        // The frame this set first reaches. `peek`, not the last frame's
        // seq: a snapshot accepted now is on screen at the next petrify, so
        // its published age starts at zero there rather than at one.
        let seq = self.counter.peek();
        let mut kept: Vec<Contribution> = Vec::with_capacity(contributions.len());
        for incoming in contributions {
            match self.ledger.accept(incoming.id, incoming.revision, seq) {
                PushOutcome::Advanced => kept.push(incoming),
                // Not newer than what is already on screen, so taking it
                // would replace a newer picture with an older one. The held
                // snapshot stays, and the arrival is dropped rather than
                // silently promoted.
                PushOutcome::Superseded => {
                    if let Some(held) = self.contributions.iter().find(|c| c.id == incoming.id) {
                        kept.push(held.clone());
                    }
                }
            }
        }
        // Everything the publisher no longer lists is retracted, which is the
        // ordinary unload path (§7) rather than a bespoke `ui` teardown.
        for gone in &self.contributions {
            if !kept.iter().any(|c| c.id == gone.id) {
                self.ledger.retract(gone.id);
            }
        }
        self.contributions = kept;
        self.prepared_stale = true;
        // Measured sizes are keyed by canonical node id, and a contribution
        // republished under the same `ui:<id>` reuses every id beneath it
        // with different content. Without this the new text would be laid out
        // to the old text's extent.
        self.cache.apply(&ChangeSet::All);
        // Nothing else will ask for a frame. A window sitting idle is the
        // normal state of a shell (SC-002), and a plugin that mounted a
        // surface into one would otherwise wait for an unrelated event.
        self.ctx.request_repaint();
        self
    }

    /// The contributions this shell was last given, in publication order.
    #[must_use]
    pub fn contributions(&self) -> &[Contribution] {
        &self.contributions
    }

    /// What the last built frame did with each contribution.
    ///
    /// Read it. A [`MountOutcome::NoSuchSlot`] against a root that cannot
    /// hold children is the one outcome with nothing on screen to show for
    /// it, and this is where it is recorded.
    ///
    /// Empty until the first frame that builds a tree; a pass that reuses
    /// the last frame does not rebuild this.
    #[must_use]
    pub fn mounts(&self) -> &[MountReport] {
        &self.mounts
    }

    /// Record that this contribution's fiber owes an answer the shell has not
    /// had, which is what makes the snapshot on screen stale (§8).
    ///
    /// Called by whoever sent the fiber something — a routed press on a
    /// contributed control, say — never by the frame path, which sends
    /// nothing. Returns whether the ledger holds that contribution at all.
    ///
    /// Staleness is *behind*, not *old*: a surface nobody has asked anything
    /// of is never stale however long it has been on screen, and there is no
    /// clock here to make it so.
    pub fn contribution_behind(&mut self, id: ContributionId) -> bool {
        let seq = self.counter.peek();
        let marked = self.ledger.mark_behind(id, seq);
        if marked {
            // The stale flag rides `ContributionStatus::mount`, so the frame
            // that carries it has to be rebuilt.
            self.prepared_stale = true;
            self.ctx.request_repaint();
        }
        marked
    }

    /// Every live contribution's retained-snapshot status as of the frame
    /// last built: its revision, the frame it was accepted at, its age in
    /// frames, and whether it is behind.
    ///
    /// The "age published beside it" §8 asks for.
    #[must_use]
    pub fn contribution_statuses(&self) -> Vec<ContributionStatus> {
        self.ledger.statuses(self.counter.peek())
    }

    /// Expand every contribution and put it through stage-2 acceptance.
    ///
    /// **Expand, then validate.** The two are not interchangeable:
    /// `validate` refuses [`NodeKind::Component`] by name and layout's own
    /// arms `unreachable!` on it, so a reference that reached either would be
    /// a refusal or a panic rather than a picture. Expansion is what turns
    /// the name a plugin wrote into the subtree the library ships.
    ///
    /// Acceptance is [`validate`] against `self.registry` — the same
    /// function and the same live registry an application's own tree goes
    /// through in [`Host::pass`], which is what
    /// `contracts/surface-contribution.md` §5's "no weaker, plugin-specific
    /// path" asks for. A plugin naming a `custom` kind this host registered
    /// is accepted for exactly the reason the application would be; one
    /// naming a token this theme does not define is refused for exactly the
    /// reason the application would be.
    fn prepare_contributions(&mut self) {
        let mut prepared = Vec::with_capacity(self.contributions.len());
        for contribution in &self.contributions {
            let outcome = match registry::expand(&contribution.tree) {
                Err(err) => Err(format!("{}: {}", err.component, err.reason)),
                Ok(expanded) => match validate(&expanded, &self.registry) {
                    Ok(_) => Ok(expanded),
                    Err(errors) => Err(errors.to_string()),
                },
            };
            prepared.push(match outcome {
                Ok(node) => Prepared {
                    id: contribution.id,
                    slot: contribution.slot.clone(),
                    node,
                    refused: None,
                },
                Err(reason) => Prepared {
                    node: contribution_card(
                        contribution.id,
                        &contribution.slot,
                        &reason,
                        self.registry.vocabulary(),
                    ),
                    id: contribution.id,
                    slot: contribution.slot.clone(),
                    refused: Some(reason),
                },
            });
        }
        self.prepared = prepared;
        self.prepared_stale = false;
    }

    /// `node` keyed `ui:<id>` and flagged with this contribution's staleness,
    /// through the one call that does both.
    ///
    /// **After expansion, never before.** Expanding a component-rooted tree
    /// returns what the constructor built, under the *constructor's* key, so
    /// a tree keyed first would lose the key it was mounted under and two
    /// contributions in one list slot could collide as
    /// `Violation::DuplicateSiblingKey` — which refuses the whole tree, so
    /// one plugin could refuse another's surface.
    /// `gorgon_petra::semantic`'s
    /// `a_contribution_is_mounted_after_its_components_are_expanded` pins it.
    fn keyed(&self, id: ContributionId, node: ViewNode) -> ViewNode {
        match self.ledger.status(id, self.counter.peek()) {
            Some(status) => status.mount(node),
            // Unreachable by construction: `self.contributions` only holds
            // ids the ledger accepted, and only ids it no longer holds are
            // retracted. Keyed anyway rather than left bare, because an
            // unkeyed contribution is a sibling-key collision waiting to
            // refuse somebody else's tree.
            None => {
                debug_assert!(false, "no ledger entry for a live contribution {id}");
                let mut node = node;
                node.key = contribution_key(id);
                node
            }
        }
    }

    /// Splice every prepared contribution into `root` and report what
    /// happened to each.
    ///
    /// Two walks, and they are the two halves of one question. The first
    /// counts how many nodes carry each wanted key and records the child
    /// index path to each, because a key is identity *within a parent* and
    /// says nothing about the tree: two nodes may carry `shell.status-bar`,
    /// and mounting under whichever the walk met first would put a plugin's
    /// surface somewhere nobody chose. The second follows the recorded paths
    /// and touches nothing else.
    fn mount_contributions(&mut self, mut root: ViewNode) -> ViewNode {
        if self.prepared_stale {
            self.prepare_contributions();
        }
        if self.prepared.is_empty() {
            self.mounts.clear();
            return root;
        }

        let wanted: BTreeSet<&str> = self.prepared.iter().map(|p| p.slot.as_str()).collect();
        let mut found: BTreeMap<&str, Vec<Vec<usize>>> = BTreeMap::new();
        find_slots(&root, &wanted, &mut Vec::new(), &mut found);

        // Collected rather than spliced as they are found: `root` is
        // borrowed by the path walk above, and a card built inside the loop
        // would need `self.registry` while `self.prepared` is borrowed.
        let mut mounts = Vec::with_capacity(self.prepared.len());
        let mut placed: Vec<(Vec<usize>, ViewNode)> = Vec::new();
        let mut orphans: Vec<ViewNode> = Vec::new();
        for prepared in &self.prepared {
            let paths = found
                .get(prepared.slot.as_str())
                .map_or(&[][..], Vec::as_slice);
            let outcome = match paths {
                [path] => {
                    placed.push((path.clone(), self.keyed(prepared.id, prepared.node.clone())));
                    prepared
                        .refused
                        .clone()
                        .map_or(MountOutcome::Mounted, MountOutcome::Refused)
                }
                [] => {
                    orphans.push(self.keyed(
                        prepared.id,
                        contribution_card(
                            prepared.id,
                            &prepared.slot,
                            &format!(
                                "no node in this shell's tree carries the key `{}`",
                                prepared.slot
                            ),
                            self.registry.vocabulary(),
                        ),
                    ));
                    MountOutcome::NoSuchSlot
                }
                many => {
                    orphans.push(self.keyed(
                        prepared.id,
                        contribution_card(
                            prepared.id,
                            &prepared.slot,
                            &format!(
                                "{} nodes carry the key `{}`, so which one is \"the\" slot has \
                                 no answer; the shell refuses rather than guessing",
                                many.len(),
                                prepared.slot
                            ),
                            self.registry.vocabulary(),
                        ),
                    ));
                    MountOutcome::AmbiguousSlot(many.len())
                }
            };
            mounts.push(MountReport {
                id: prepared.id,
                slot: prepared.slot.clone(),
                outcome,
            });
        }
        self.mounts = mounts;

        for (path, node) in placed {
            attach(node_at_path(&mut root, &path), node);
        }
        // A card with nowhere to go still has to be visible, so it goes to
        // the root. A root that cannot hold children leaves `Host::mounts`
        // as the only record, which is why that accessor exists and why its
        // doc says to read it.
        if !orphans.is_empty() && root.kind.is_container() {
            for card in orphans {
                attach(&mut root, card);
            }
        }
        root
    }
}

impl<A: App> eframe::App for Host<A> {
    /// eframe hands us a `Ui`; we take its context and never touch it again.
    ///
    /// Painting has to happen here rather than in `App::logic`, which eframe
    /// documents as "you may NOT show any ui or do any painting" — so this is
    /// the one place a frame can be drawn. Receiving a `Ui` is not the same as
    /// laying out with one: `ui.ctx()` is the whole of what this uses, and the
    /// `Ui`'s cursor, spacing, and layout stack stay untouched.
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.pass_in_window(ui.ctx(), frame);
    }
}

/// Physical pixel size of the Petra page, matching the viewport this host
/// already measures in points. Zero when the window has no area.
#[cfg(not(target_arch = "wasm32"))]
fn physical_window_px(ctx: &Context) -> [u32; 2] {
    let ppp = ctx.pixels_per_point();
    let rect = ctx.content_rect();
    [
        (rect.width() * ppp).round() as u32,
        (rect.height() * ppp).round() as u32,
    ]
}

/// The view a refused tree is replaced with, so the violations are on screen.
/// Whether `event` is the operator asking for the selection to be copied.
///
/// Two shapes, both real. See [`App::copy_selection`], which carries the
/// argument for why this is not one shape with a fallback.
#[must_use]
fn asks_for_a_copy(event: &InputEvent) -> bool {
    match event {
        InputEvent::Copy => true,
        InputEvent::Key {
            key: KeyCode::Char('c'),
            pressed: true,
            modifiers,
            ..
        } => modifiers.ctrl || modifiers.meta,
        _ => false,
    }
}

/// Record the child-index path of every node whose key is in `wanted`.
///
/// Read-only, so no `Arc` in the tree is cloned by looking. The paths are
/// what let the splice reach a mount point while touching only its ancestors.
fn find_slots<'a>(
    node: &ViewNode,
    wanted: &BTreeSet<&'a str>,
    path: &mut Vec<usize>,
    found: &mut BTreeMap<&'a str, Vec<Vec<usize>>>,
) {
    if let Some(slot) = wanted.get(node.key.as_str()) {
        found.entry(slot).or_default().push(path.clone());
    }
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        find_slots(child, wanted, path, found);
        path.pop();
    }
}

/// The node `path` names, cloning only the nodes on the way to it.
///
/// `Arc::make_mut` clones a node when it is shared and hands back the
/// existing one when it is not, and either way its children stay the same
/// allocations. So the ancestors of a mount point are rebuilt — they changed
/// — and every subtree hanging off them is passed on by pointer, which is
/// what keeps `Arc::ptr_eq` a usable proof of "unchanged" for the rest of the
/// frame.
///
/// # Panics
/// Never on a path from [`find_slots`] against the same tree: the indices
/// came from that tree's own `children` vectors and nothing has resized them.
fn node_at_path<'a>(root: &'a mut ViewNode, path: &[usize]) -> &'a mut ViewNode {
    let mut node = root;
    for &index in path {
        node = Arc::make_mut(&mut node.children[index]);
    }
    node
}

/// Add `child` to `parent`, replacing a child that already carries its key.
///
/// Replacing rather than appending is what makes the splice unable to refuse
/// the whole frame: two children with one key is `Violation::DuplicateSiblingKey`,
/// which refuses the tree it is in — so a shell squatting on `ui:7` inside a
/// mount slot would blank the window instead of losing an argument with the
/// mount path. Contribution ids are unique, so no two contributions can reach
/// this with the same key.
fn attach(parent: &mut ViewNode, child: ViewNode) {
    if let Some(taken) = parent.children.iter_mut().find(|old| old.key == child.key) {
        *taken = Arc::new(child);
    } else {
        parent.children.push(Arc::new(child));
    }
}

/// The card the shell paints in place of a contribution it could not mount.
///
/// Keyed `ui:<id>` like the subtree it stands in for, so a refused
/// contribution is attributable by exactly the path a mounted one is
/// (`contracts/surface-contribution.md` §9) and `who-owns ui:7` still
/// answers.
///
/// # Why it checks the vocabulary before binding a token
///
/// This card is spliced into the application's tree and is then accepted
/// against the application's registry along with it. `refusal_view` can bind
/// `surface.base` and `status.down` unconditionally because it *replaces* the
/// tree and is checked against the shipped vocabulary; this one cannot, and a
/// card that refused acceptance would turn one plugin's bad tree into a
/// blank window — the precise failure it exists to prevent. So it binds each
/// token only where the live theme defines it at the right kind, and is a
/// legible unstyled card under a theme that defines neither.
fn contribution_card(
    id: ContributionId,
    slot: &str,
    reason: &str,
    vocabulary: &Vocabulary,
) -> ViewNode {
    let mut props = Props::default();
    for (paint_slot, token) in [
        // A card, so it sits on the shell's ground rather than repainting it.
        ("background", "surface.raised"),
        // Carbon's inline notification carries its severity on the leading
        // edge, and this is the same shape for the same reason: the ground
        // stays legible under any theme and the colour still says "error".
        ("border-left", "support-error"),
        ("foreground", "text.primary"),
    ] {
        if let Some(name) = declared(vocabulary, token, TokenKind::Color) {
            props.tokens.insert(paint_slot.into(), name);
        }
    }
    // Text flush against the edge of its own card reads as a rendering bug
    // rather than as a message. Both of these are token references and are
    // dropped when the live theme does not define the name, exactly as the
    // colours above are.
    props.padding = declared(vocabulary, "spacing-05", TokenKind::Spacing).map(InsetRefs::all);
    props.spacing = declared(vocabulary, "spacing-03", TokenKind::Spacing);
    // Keyed with the contribution's own key here too, even though
    // `Host::keyed` sets it again: a card that left `Host::keyed`'s path
    // would still be a well-formed, attributable node rather than one keyed
    // whatever this function felt like.
    let mut card = ViewNode::new(NodeKind::Stack, contribution_key(id))
        .with_props(props)
        .child(text_line(
            "headline",
            &format!("{id} was refused; it asked for the slot `{slot}`"),
            declared(vocabulary, "typography.heading-sm", TokenKind::Typography),
        ))
        .child(text_line(
            "reason",
            reason,
            declared(vocabulary, "typography.body", TokenKind::Typography),
        ));
    // The card is not interactive, so a label is not required — it is here
    // because a screen reader that reached the two runs separately would
    // announce a reason with nothing to attach it to.
    card.semantics.label = Some(format!("refused contribution {id}"));
    card
}

/// `token` as a [`TokenName`], but only when the live theme defines it at
/// `kind`.
///
/// The gate on every token [`contribution_card`] binds. A card that named a
/// token the application's registry has never heard of would be refused by
/// the acceptance the whole application tree goes through, which would turn
/// one plugin's bad tree into a blank window — the precise failure the card
/// exists to prevent.
fn declared(vocabulary: &Vocabulary, token: &str, kind: TokenKind) -> Option<TokenName> {
    let name = TokenName::new(token).expect("a shipped token name is well-formed");
    (vocabulary.kind_of(&name) == Some(kind)).then_some(name)
}

/// One run of text inside [`contribution_card`].
fn text_line(key: &str, text: &str, style: Option<TokenName>) -> ViewNode {
    ViewNode::new(NodeKind::Text, key).with_props(Props {
        text: Some(text.to_owned()),
        style,
        wrap: Some(TextWrap::Wrap),
        ..Props::default()
    })
}

fn refusal_view(message: &str) -> ViewNode {
    let mut tokens = Props::default();
    tokens
        .tokens
        .insert("background".into(), TokenName::new("surface.base").unwrap());
    tokens
        .tokens
        .insert("foreground".into(), TokenName::new("status.down").unwrap());
    ViewNode::new(NodeKind::Stack, "petra-tree-refused")
        .with_props(tokens)
        .child(
            ViewNode::new(NodeKind::Text, "violations").with_props(Props {
                text: Some(message.to_owned()),
                ..Props::default()
            }),
        )
}

/// The vocabulary a registry is given: every name `theme` defines, plus the
/// application's own declarations on top.
///
/// Built fresh from the theme rather than merged into the previous one, so a
/// name the *old* theme defined and the new one does not leaves the
/// vocabulary with it. Merging would keep accepting trees that name it, and
/// the frame that resolved one would panic — which is the whole failure this
/// composition exists to prevent.
fn composed_vocabulary(theme: &Theme, extra: &Vocabulary) -> Vocabulary {
    let mut vocabulary = Vocabulary::from_theme(theme);
    for name in extra.names() {
        if let Some(kind) = extra.kind_of(name) {
            vocabulary.declare(DesignToken::new(name.clone(), kind));
        }
    }
    for status in extra.statuses() {
        vocabulary.declare_status(status.clone());
    }
    vocabulary
}

/// A presenter over the shipped dark theme, for callers that have no theme of
/// their own yet.
#[must_use]
pub fn default_presenter() -> Presenter {
    Presenter::new(gorgon_petra::token::dark())
}

#[cfg(test)]
mod tests {
    use super::{
        App, ChangeSet, Host, coverage_plan, default_presenter, petra_layer, refusal_view,
    };
    use egui::{Context, Event, Key, Modifiers, RawInput};
    use gorgon_petra::frame::PetrifiedFrame;
    use gorgon_petra::geom::{Point, Rect};
    use gorgon_petra::input::{InputEvent, Route, route_pointer_exit};
    use gorgon_petra::layout::RowSource;
    use gorgon_petra::token::TokenName;
    use gorgon_petra::tree::{
        Anchor, ClampRule, InputPolicy, Interaction, Layer, NodeKind, Props, Role, ViewNode,
    };
    use std::ops::Range;

    #[derive(Default)]
    struct Demo {
        seen: Vec<(String, String)>,
        dismissed: Vec<String>,
        bad_tree: bool,
        /// Bind the application's own spacing token on the root, the way an
        /// application with a design system of its own does.
        gutter: bool,
        modal: bool,
        menu: bool,
        /// Whether the fixture's menu declares `Props::takes_focus`, the way
        /// `gorgon_petra::component::menu` does.
        menu_takes_focus: bool,
        hide_run: bool,
        /// How many times [`App::view`] ran. A caret in flight must not
        /// increment this on every vsync.
        view_calls: usize,
        /// Text this app wants copied on its next pass.
        copy: Option<String>,
        /// A deadline this app names, relative to the clock it was ticked
        /// with: `Some(2.0)` means "wake me two seconds from now".
        wake_in: Option<f64>,
        /// The clock the last pass handed this app.
        now: f64,
        /// Paths this app has been handed, dropped or picked, in order.
        dropped: Vec<std::path::PathBuf>,
    }

    /// A focusable, clickable, hoverable text button — a `Role::Button` node
    /// declaring exactly what a real one does: `Click`, `Focus` and `Hover`,
    /// never `Key`. `Hover` matters as much as the other two: a node that
    /// does not declare it is not a hit-test candidate for hover, so the
    /// engine never lights it.
    fn button(key: &str, label: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key)
            .with_props(Props {
                text: Some(label.to_owned()),
                ..Props::default()
            })
            .interactive(
                Role::Button,
                label.to_owned(),
                &[Interaction::Focus, Interaction::Click, Interaction::Hover],
            )
    }

    impl RowSource for Demo {
        fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<std::sync::Arc<ViewNode>> {
            Vec::new()
        }
    }

    impl App for Demo {
        fn tick(&mut self, now: f64) {
            self.now = now;
        }

        fn wake_at(&mut self) -> Option<f64> {
            self.wake_in.map(|delay| self.now + delay)
        }

        fn clipboard_request(&mut self) -> Option<String> {
            self.copy.take()
        }

        fn files_dropped(&mut self, paths: &[std::path::PathBuf]) {
            self.dropped.extend_from_slice(paths);
        }

        fn view(&mut self) -> ViewNode {
            self.view_calls += 1;
            if self.bad_tree {
                // Two children with one key: a tree-acceptance violation.
                return ViewNode::new(NodeKind::Stack, "root")
                    .child(ViewNode::new(NodeKind::Text, "dup"))
                    .child(ViewNode::new(NodeKind::Text, "dup"));
            }
            let mut panel = Props::default();
            panel
                .tokens
                .insert("background".into(), TokenName::new("surface.base").unwrap());
            if self.gutter {
                panel.spacing = Some(TokenName::new("spacing.app-gutter").unwrap());
            }
            let mut root = ViewNode::new(NodeKind::Stack, "root")
                .with_props(panel)
                .child(ViewNode::new(NodeKind::Text, "title").with_props(Props {
                    text: Some("Fibers".into()),
                    ..Props::default()
                }))
                .child(
                    ViewNode::new(NodeKind::Input, "filter")
                        .with_props(Props {
                            placeholder: Some("Filter".into()),
                            ..Props::default()
                        })
                        .interactive(
                            Role::TextInput,
                            "Filter fibers",
                            &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
                        ),
                )
                .child(if self.hide_run {
                    ViewNode::new(NodeKind::Spacer, "gap")
                } else {
                    button("run", "Run")
                })
                .child(if self.modal {
                    ViewNode::new(NodeKind::Surface, "modal")
                        .with_props(Props {
                            layer: Some(Layer::Modal),
                            anchor: Some(Anchor::Viewport),
                            clamp: Some(ClampRule::Shrink),
                            input_policy: Some(InputPolicy::Block),
                            ..Props::default()
                        })
                        .child(button("yes", "Yes"))
                        .child(button("no", "No"))
                } else {
                    ViewNode::new(NodeKind::Spacer, "no-modal")
                });
            // Appended only when open, unlike `modal`'s placeholder spacer:
            // the menu is the last child, so its absence shifts nothing, and
            // every test written before it must keep seeing the same tree.
            if self.menu {
                root = root.child(
                    ViewNode::new(NodeKind::Surface, "menu")
                        .with_props(Props {
                            layer: Some(Layer::Popup),
                            anchor: Some(Anchor::Viewport),
                            clamp: Some(ClampRule::Shrink),
                            input_policy: Some(InputPolicy::DismissOutside),
                            takes_focus: self.menu_takes_focus.then_some(true),
                            ..Props::default()
                        })
                        .child(button("open", "Open")),
                );
            }
            root
        }

        fn handle(&mut self, event: &InputEvent, route: &Route, _frame: Option<&PetrifiedFrame>) {
            let kind = match event {
                InputEvent::Key { key, .. } => format!("key:{key:?}"),
                InputEvent::Text(_) => "text".to_owned(),
                _ => "other".to_owned(),
            };
            let where_ = match route {
                Route::Pointer { node } | Route::Keyboard { node } | Route::Raw { node } => {
                    node.clone()
                }
                // This fixture drives `deliver_input`, which routes through
                // `PointerState::route` → `route_with_surfaces`, never
                // `route_with_reserved` — so `Reserved` cannot arrive here.
                // Recorded rather than silently folded into `Unrouted` so a
                // future caller that does start producing it is forced to
                // decide what this fixture should say.
                Route::Reserved { chord } => format!("reserved: {chord}"),
                Route::Unrouted { reason } => format!("unrouted: {reason}"),
            };
            self.seen.push((kind, where_));
        }

        fn dismissed(&mut self, ids: &[String]) {
            self.dismissed.extend(ids.iter().cloned());
        }

        fn take_changes(&mut self) -> ChangeSet {
            // The fixture does not track which nodes moved; `All` is the
            // honest conservative answer and is what every test below relies
            // on when it mutates a field and expects the next pass to see it.
            ChangeSet::All
        }
    }

    /// One primary press at `pos`, the way egui reports a click.
    fn press_at(pos: egui::Pos2) -> RawInput {
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::default(),
        });
        input
    }

    /// The placed rectangle of the one placement whose canonical id ends with
    /// `suffix`, and a point outside it.
    ///
    /// Derived from the frame rather than written as a constant, so the test
    /// keeps meaning "outside" when the layout moves the surface.
    fn placed_and_outside(host: &Host<Demo>, suffix: &str) -> (Rect, egui::Pos2) {
        let rect = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("no placement id ends with {suffix}"))
            .rect;
        let outside = egui::Pos2::new(rect.x + rect.w + 20.0, rect.y + rect.h + 20.0);
        assert!(
            !rect.contains(Point::new(outside.x, outside.y)),
            "the fixture's 'outside' point is inside {rect:?}"
        );
        (rect, outside)
    }

    /// `InputPolicy::Block` reaching the host: a press outside an open modal
    /// never gets to the page behind it.
    ///
    /// A file dropped on the window reaches the application on the same pass.
    ///
    /// `Host::pass` reads `RawInput::dropped_files` itself rather than
    /// letting the `eframe` layer do it, and this is why: raw input is
    /// drivable with no window, so the seam has a test. The `eframe` hook is
    /// not.
    #[test]
    fn a_dropped_file_reaches_the_application_on_the_pass_that_took_it() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert!(host.app().dropped.is_empty(), "nothing was dropped yet");

        let input = RawInput {
            dropped_files: vec![
                std::sync::Arc::new(Dropped("/tmp/first.ndjson".into())),
                std::sync::Arc::new(Dropped("/tmp/second.yaml".into())),
            ],
            ..RawInput::default()
        };
        step(&ctx, &mut host, input);

        assert_eq!(
            host.app().dropped,
            vec![
                std::path::PathBuf::from("/tmp/first.ndjson"),
                std::path::PathBuf::from("/tmp/second.yaml"),
            ],
            "the window's drop did not reach the application, or lost its order"
        );
    }

    /// Files chosen in a dialog arrive by the same door a drop arrives by,
    /// and the dialog slot frees for the next request.
    ///
    /// What is driven here is the drain, which is the half that reaches the
    /// application. The spawn above it opens a real portal window and no
    /// check in this workspace may do that, so it stays unverified — see
    /// [`Host::seat_picked_dialog`].
    #[test]
    fn a_finished_dialog_delivers_its_files_and_frees_the_slot() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());

        host.seat_picked_dialog(vec![std::path::PathBuf::from("/tmp/picked.ndjson")]);
        step(&ctx, &mut host, RawInput::default());

        assert_eq!(
            host.app().dropped,
            vec![std::path::PathBuf::from("/tmp/picked.ndjson")],
            "the dialog's answer never reached the application"
        );
        assert!(
            host.picking.is_none(),
            "the answered dialog still holds the slot, so no second dialog \
             could ever open"
        );
    }

    /// A path a dropped file carries, with no bytes behind it.
    ///
    /// `egui`'s trait wants a reader too; nothing on this seam reads one,
    /// because Petra hands the application paths and lets it open what it
    /// likes. Saying so out loud beats returning an empty buffer that a
    /// caller would read as an empty file.
    #[derive(Debug)]
    struct Dropped(std::path::PathBuf);

    impl egui::DroppedFile for Dropped {
        fn path(&self) -> &std::path::Path {
            &self.0
        }

        fn bytes(&self) -> Result<Vec<u8>, String> {
            Err("a dropped path carries no bytes across this seam".to_owned())
        }
    }

    /// The z-order walk alone would have delivered this press to whatever it
    /// hit, because a modal is not obliged to cover the screen. Swallowing it
    /// is the whole difference between a modal and a floating panel.
    #[test]
    fn a_press_outside_an_open_modal_is_swallowed() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        host.app_mut().modal = true;
        step(&ctx, &mut host, RawInput::default());
        let (_, outside) = placed_and_outside(&host, "/modal");

        host.app_mut().seen.clear();
        step(&ctx, &mut host, press_at(outside));

        assert_eq!(
            host.app().seen,
            vec![(
                "other".to_owned(),
                "unrouted: outside every currently-open Block surface's bounds".to_owned()
            )],
            "the press must reach the application as an explicit drop, not vanish"
        );
        assert!(
            host.app().dismissed.is_empty(),
            "a Block surface asks for no dismissal"
        );
    }

    /// `InputPolicy::DismissOutside` reaching the host: the application is
    /// told which surface to close, and the press still routes.
    #[test]
    fn a_press_outside_a_dismiss_surface_is_reported_and_still_routes() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        host.app_mut().menu = true;
        step(&ctx, &mut host, RawInput::default());
        let (_, outside) = placed_and_outside(&host, "/menu");

        host.app_mut().seen.clear();
        step(&ctx, &mut host, press_at(outside));

        let dismissed = &host.app().dismissed;
        assert_eq!(dismissed.len(), 1, "{dismissed:?}");
        assert!(dismissed[0].ends_with("/menu"), "{dismissed:?}");
        assert!(
            !host
                .app()
                .seen
                .iter()
                .any(|(_, w)| w == "unrouted: outside every currently-open Block surface's bounds"),
            "DismissOutside places no swallow boundary: {:?}",
            host.app().seen
        );
    }

    /// `Props::takes_focus` reaching the host: focus moves onto the first
    /// focusable node inside the overlay when it appears, and back to
    /// whatever held focus at that moment when it goes away.
    ///
    /// Carbon's `Menu` is what asks for this
    /// (`@carbon/react/lib/components/Menu/Menu.js`: `handleOpen` stores
    /// `document.activeElement`, an effect seats item 0, `handleClose` calls
    /// `returnFocus`), and `gorgon_petra::component::menu` is the one
    /// constructor that declares it. Driven here on the fixture rather than
    /// on the gallery so the rule is pinned where it lives.
    ///
    /// Note the extra pass after each toggle: the seat happens after the
    /// frame is petrified, the same "seat it after petrify" shape
    /// `enter_open_modal` has, so the ring lands one frame later.
    #[test]
    fn a_focus_taking_overlay_seats_focus_inside_itself_and_hands_it_back() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        host.app_mut().menu_takes_focus = true;
        step(&ctx, &mut host, RawInput::default());
        host.focus_mut()
            .focus("/root/run")
            .expect("run is focusable");
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(host.focus().current(), Some("/root/run"));

        host.app_mut().menu = true;
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.focus().current(),
            Some("/root/menu/open"),
            "the overlay opened and focus stayed outside it"
        );

        host.app_mut().menu = false;
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.focus().current(),
            Some("/root/run"),
            "the overlay closed and focus did not go back to its opener"
        );
    }

    /// The same overlay, without the declaration: focus is left where it is.
    ///
    /// This is what keeps the rule off Carbon's Dropdown, Combo box and Date
    /// picker, which all open an anchored list and all keep focus on the
    /// control that opened it. A host rule keyed on `Role::Overlay` or on
    /// `InputPolicy::DismissOutside` would move focus in every one of them,
    /// and this fixture's menu is exactly that shape.
    #[test]
    fn an_overlay_that_does_not_declare_takes_focus_leaves_focus_alone() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        host.focus_mut()
            .focus("/root/run")
            .expect("run is focusable");
        step(&ctx, &mut host, RawInput::default());

        host.app_mut().menu = true;
        step(&ctx, &mut host, RawInput::default());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.focus().current(),
            Some("/root/run"),
            "an overlay that never claimed focus took it anyway"
        );
    }

    /// The seat happens **once**, on the frame the overlay appears.
    ///
    /// `enter_open_modal` gets idempotence free — "focus is already inside
    /// some blocking scope" is a test it can repeat every frame — but a menu
    /// declares no scope, so the rule is a frame-to-frame diff instead. If
    /// that diff were dropped, Tab out of an open menu would be undone by the
    /// very next pass and the menu would be a focus trap nothing could leave.
    #[test]
    fn a_focus_taking_overlay_does_not_drag_focus_back_every_frame() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        host.app_mut().menu_takes_focus = true;
        host.app_mut().menu = true;
        step(&ctx, &mut host, RawInput::default());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(host.focus().current(), Some("/root/menu/open"));

        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        let after_tab = host.focus().current().map(str::to_owned);
        assert_ne!(
            after_tab.as_deref(),
            Some("/root/menu/open"),
            "Tab did not leave the open overlay at all"
        );
        step(&ctx, &mut host, RawInput::default());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.focus().current(),
            after_tab.as_deref(),
            "an idle pass dragged focus back into the overlay"
        );
    }

    /// Focus goes back to the opener only when it was still **inside** the
    /// overlay that closed.
    ///
    /// A press on some other control closes the menu and seats focus on that
    /// control in the same pass (`seat_pointer_focus` runs before petrify).
    /// Handing focus back to the trigger there would yank it off the thing
    /// the operator just pressed.
    #[test]
    fn closing_a_focus_taking_overlay_leaves_focus_where_a_press_put_it() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        host.app_mut().menu_takes_focus = true;
        host.app_mut().menu = true;
        step(&ctx, &mut host, RawInput::default());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(host.focus().current(), Some("/root/menu/open"));

        let filter = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id == "/root/filter")
            .expect("the fixture's text field is placed")
            .rect;
        host.app_mut().menu = false;
        step(
            &ctx,
            &mut host,
            press_at(egui::Pos2::new(
                filter.x + filter.w / 2.0,
                filter.y + filter.h / 2.0,
            )),
        );
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.focus().current(),
            Some("/root/filter"),
            "the close handed focus back over the press that caused it"
        );
    }

    /// The other half of the contract, so the test above cannot pass by
    /// reporting every press: inside the surface, nothing is dismissed.
    #[test]
    fn a_press_inside_a_dismiss_surface_dismisses_nothing() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        host.app_mut().menu = true;
        step(&ctx, &mut host, RawInput::default());
        let (rect, _) = placed_and_outside(&host, "/menu");
        let inside = egui::Pos2::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);

        step(&ctx, &mut host, press_at(inside));

        assert!(
            host.app().dismissed.is_empty(),
            "{:?}",
            host.app().dismissed
        );
    }

    /// The glyph atlas's coverage curve is a projection of the published
    /// theme's `text.coverage-curve` token — **not** of `ThemeMode`, and it
    /// has to move when the token does, whether or not the mode moves with
    /// it.
    ///
    /// Two defects this guards, one after the other:
    ///
    /// * Nothing in this workspace used to write `Visuals::text_options` at
    ///   all, so the atlas was always built from `Visuals::default()` —
    ///   `Visuals::dark()` — including under `light()`.
    /// * The fix that followed (`86de1a6`) wrote the curve keyed on
    ///   `ThemeMode` instead — `Off` for light, `TwoCoverageMinusCoverageSq`
    ///   for dark — which was itself harmful: it pinned light mode at
    ///   `n = 1`, the identity, the setting *most* exposed to the
    ///   per-character weight variance this text pipeline port exists to
    ///   close, not the least. Both shipped themes now assign
    ///   `text.coverage-curve` the identical `passes: 3.0, snap: false`
    ///   (`ef31ea8`), so a curve that still moved with the mode alone would
    ///   be the same hardcode with an extra layer of indirection.
    ///
    /// Both of egui's own per-theme styles are read, not just the active one,
    /// because [`bind_glyph_coverage`] writes both on purpose: an application
    /// that calls `Context::set_theme` must not thereby get a stale curve.
    #[test]
    fn the_glyph_coverage_curve_follows_the_theme_mode() {
        use egui::epaint::FontColorTransferFunction;
        use gorgon_petra::token::value::CoverageValue;
        use gorgon_petra::token::{Presenter, Theme, TokenValue, dark, light, standard_vocabulary};

        fn bound(ctx: &Context) -> [FontColorTransferFunction; 2] {
            [egui::Theme::Dark, egui::Theme::Light].map(|theme| {
                ctx.style_of(theme)
                    .visuals
                    .text_options
                    .color_transfer_function
            })
        }

        /// `base` with its `text.coverage-curve` overridden — the only way
        /// this test can observe the binder reading the *token* rather than
        /// `ThemeMode`, now that both shipped themes assign the token the
        /// same value.
        fn with_coverage(base: Theme, passes: f32, snap: bool) -> Theme {
            let mode = base.mode();
            let mut values = base.values().clone();
            values.insert(
                TokenName::new("text.coverage-curve")
                    .expect("\"text.coverage-curve\" is a well-formed token name"),
                TokenValue::Coverage(CoverageValue { passes, snap }),
            );
            Theme::build(mode, &standard_vocabulary(), values).expect(
                "overriding an already-declared token's value at its declared \
                 kind keeps the theme complete",
            )
        }

        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), Presenter::new(light()));
        // light's own text.coverage-curve is passes: 2.0 -- the one value
        // the atlas curve spends instead of a second paint. Not
        // TwoCoverageMinusCoverageSq "because it is light mode": see below.
        assert_eq!(
            bound(&ctx),
            [FontColorTransferFunction::TwoCoverageMinusCoverageSq; 2],
            "light's shipped text.coverage-curve (passes: 2.0) must bind, in \
             both of egui's styles, before the first pass rather than after it"
        );

        host.presenter().publish(dark());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            bound(&ctx),
            [FontColorTransferFunction::TwoCoverageMinusCoverageSq; 2],
            "dark ships the identical text.coverage-curve as light, so the \
             curve must stay identical across the swap — this is the proof \
             the binder reads the token and not ThemeMode: a mode-keyed \
             binding (the superseded `86de1a6` hardcode) would have flipped \
             light alone to Off"
        );

        // Now move the token itself, on the theme this host is already
        // publishing dark() as: the curve must follow the *value*, not stay
        // pinned at whatever a shipped theme happens to assign today.
        host.presenter().publish(with_coverage(dark(), 3.0, false));
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            bound(&ctx),
            [FontColorTransferFunction::Off; 2],
            "publishing a theme whose text.coverage-curve moved to \
             passes: 3.0 must move the curve with it — an odd count is three \
             paints through the identity curve; a stale curve is the defect \
             both predecessor tests exist for"
        );

        host.presenter().publish(light());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            bound(&ctx),
            [FontColorTransferFunction::TwoCoverageMinusCoverageSq; 2],
            "and back to light's own passes: 2.0, so this asserts a binding \
             rather than a one-way latch"
        );
    }

    /// `coverage_plan` at every row the contract table names (`SPEC.md`
    /// §6.3): `bind_glyph_coverage` and `crate::paint`'s repeat loop derive
    /// both halves from this one function, so a wrong row here is a wrong
    /// picture on screen from both directions at once.
    #[test]
    fn the_coverage_curve_mapping_matches_every_contract_row() {
        use egui::epaint::FontColorTransferFunction;
        use gorgon_petra::token::value::CoverageValue;

        let cases = [
            (1.0_f32, FontColorTransferFunction::Off, 1_u8),
            (
                2.0,
                FontColorTransferFunction::TwoCoverageMinusCoverageSq,
                1,
            ),
            (3.0, FontColorTransferFunction::Off, 3),
            (
                4.0,
                FontColorTransferFunction::TwoCoverageMinusCoverageSq,
                2,
            ),
        ];
        for (passes, curve, repeats) in cases {
            let plan = coverage_plan(CoverageValue {
                passes,
                snap: false,
            });
            assert_eq!(plan.curve, curve, "passes={passes}: {plan:?}");
            assert_eq!(plan.repeats, repeats, "passes={passes}: {plan:?}");
            assert_eq!(plan.fraction, 0.0, "passes={passes}: {plan:?}");
        }
    }

    /// The trap `SPEC.md` §6.4 names by name: `2c - c² ≡ 1 - (1-c)²`, so
    /// `TwoCoverageMinusCoverageSq` *is* two-pass compositing, and the atlas
    /// curve and the paint count MULTIPLY (`effective_n = n_atlas · repeats`).
    /// `TwoCov` at three paints would be `n = 6`; no legal `passes` in
    /// `[1.0, 4.0]` maps to it, and this pins that for every value the
    /// mapping actually has to answer for, not only the four table rows.
    #[test]
    fn two_cov_at_three_paints_would_be_n_six_and_no_row_produces_it() {
        use egui::epaint::FontColorTransferFunction;
        use gorgon_petra::token::value::CoverageValue;

        for tenths in 10..=40 {
            let passes = tenths as f32 / 10.0;
            let plan = coverage_plan(CoverageValue {
                passes,
                snap: false,
            });
            assert!(
                !(plan.curve == FontColorTransferFunction::TwoCoverageMinusCoverageSq
                    && plan.repeats == 3),
                "passes={passes}: TwoCov at 3 paints is n=6, not a legal row: {plan:?}"
            );
        }

        // The specific pair the gate names: passes=4.0 must be TwoCov at
        // *two* paints (n=4), never TwoCov at three (n=6, wrong) and never
        // Off at four (n=4, correct but one paint more than it needs).
        let four = coverage_plan(CoverageValue {
            passes: 4.0,
            snap: false,
        });
        assert_eq!(
            four.curve,
            FontColorTransferFunction::TwoCoverageMinusCoverageSq,
            "{four:?}"
        );
        assert_eq!(four.repeats, 2, "{four:?}");
    }

    /// `CoverageValue::snap` is deliberately inverted at this binder:
    /// `snap: true` means `subpixel_binning: false`. Both sides are `bool`,
    /// so the compiler cannot catch a polarity flip here — only a test that
    /// asserts the actual direction can.
    #[test]
    fn snap_reaches_subpixel_binning_inverted() {
        use gorgon_petra::token::value::CoverageValue;
        use gorgon_petra::token::{Presenter, Theme, TokenValue, dark, light, standard_vocabulary};

        fn bound_binning(ctx: &Context) -> [bool; 2] {
            [egui::Theme::Dark, egui::Theme::Light]
                .map(|theme| ctx.style_of(theme).visuals.text_options.subpixel_binning)
        }

        fn with_snap(base: Theme, snap: bool) -> Theme {
            let mode = base.mode();
            let mut values = base.values().clone();
            values.insert(
                TokenName::new("text.coverage-curve")
                    .expect("\"text.coverage-curve\" is a well-formed token name"),
                TokenValue::Coverage(CoverageValue { passes: 3.0, snap }),
            );
            Theme::build(mode, &standard_vocabulary(), values)
                .expect("overriding snap alone keeps the theme complete")
        }

        let ctx = headless();
        let mut host = Host::new(
            &ctx,
            Demo::default(),
            Presenter::new(with_snap(light(), false)),
        );
        assert_eq!(
            bound_binning(&ctx),
            [true; 2],
            "snap: false must leave subpixel_binning ON (keep sub-pixel \
             precision) -- epaint's own default"
        );

        host.presenter().publish(with_snap(dark(), true));
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            bound_binning(&ctx),
            [false; 2],
            "snap: true must turn subpixel_binning OFF -- the inversion \
             CoverageValue::snap documents. A binder that wired this straight \
             through (snap -> subpixel_binning with no inversion) would leave \
             this true, and the compiler would not have caught it: both sides \
             are bool"
        );
    }

    /// A theme swap must leave the registry and the theme naming the same
    /// tokens, and must carry the type ramp with it.
    ///
    /// The registry's vocabulary and the shaper's typography map are both
    /// projections of the published theme, and both used to be taken once,
    /// in `Host::new`. `Presenter::publish` takes `&self` and tells nobody,
    /// so after a swap the registry went on accepting a tree that named a
    /// token the new theme does not define — and `tree::props::resolve_*`,
    /// which is *total* by contract C6 and therefore panics rather than
    /// carrying a fallback branch, brought the window down mid-frame.
    ///
    /// Both halves are checked through one swap: the application's own
    /// `spacing.app-gutter` leaves the registry with the theme that defined
    /// it, and `typography.heading` takes the new theme's size.
    #[test]
    fn a_theme_swap_rebinds_the_registry_and_the_type_ramp() {
        use gorgon_petra::token::{
            DesignToken, Presenter, Theme, ThemeMode, TokenKind, TokenValue, TypographyFamily,
            TypographyValue, TypographyWeight, dark, light, standard_vocabulary,
        };

        let gutter = TokenName::new("spacing.app-gutter").unwrap();
        let heading = TokenName::new("typography.heading").unwrap();

        // The application's design system: the shipped ramp plus one name of
        // its own. A complete, legal theme.
        let mut vocabulary = standard_vocabulary();
        vocabulary.declare(DesignToken::new(gutter.clone(), TokenKind::Spacing));
        let mut values = light().values().clone();
        values.insert(gutter.clone(), TokenValue::Spacing(20.0));
        let app_theme =
            Theme::build(ThemeMode::Light, &vocabulary, values).expect("a complete app theme");

        // What the operator switches to: the shipped vocabulary, which does
        // not declare the gutter, and a heading a third larger.
        let mut values = dark().values().clone();
        values.insert(
            heading.clone(),
            // The two trailing fields arrived with the Carbon type ramp on
            // 2026-08-25 and are at their defaults here: this fixture is about
            // the *size* a theme swap installs, and tracking and face class
            // would be two more variables in a test that is not measuring
            // either.
            TokenValue::Typography(TypographyValue {
                size: 33.0,
                line_height: 44.0,
                weight: TypographyWeight::Bold,
                letter_spacing: 0.0,
                family: TypographyFamily::Sans,
            }),
        );
        let other_theme = Theme::build(ThemeMode::Dark, &standard_vocabulary(), values)
            .expect("a complete shipped theme");

        let ctx = headless();
        let demo = Demo {
            gutter: true,
            ..Demo::default()
        };
        let mut host = Host::new(&ctx, demo, Presenter::new(app_theme));

        step(&ctx, &mut host, RawInput::default());
        assert!(
            host.registry_mut().vocabulary().contains(&gutter),
            "the host must seed its registry from the theme it was built with"
        );
        assert!(
            host.frame()
                .expect("a frame")
                .placements
                .iter()
                .any(|p| p.id.ends_with("/root")),
            "the application's own tree must be accepted before the swap"
        );
        assert!(host.report().expect("a report").is_complete());
        assert_eq!(
            host.shaper()
                .typography()
                .style(heading.as_str())
                .expect("the app theme's ramp is bound")
                .font
                .size,
            20.0
        );

        host.presenter().publish(other_theme);

        // The pass that must not panic.
        step(&ctx, &mut host, RawInput::default());

        assert!(
            !host.registry_mut().vocabulary().contains(&gutter),
            "a name the new theme does not define must leave the registry \
             with the old theme; keeping it means a tree naming it is still \
             accepted and the frame that resolves it panics"
        );
        let ids: Vec<&str> = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert!(
            ids.iter().any(|id| id.contains("petra-tree-refused")),
            "the tree must be refused against the new vocabulary and the \
             violation painted, not resolved against a stale one: {ids:?}"
        );
        assert!(
            host.report().expect("a report").is_complete(),
            "{:?}",
            host.report()
        );
        assert_eq!(
            host.shaper()
                .typography()
                .style(heading.as_str())
                .expect("the new theme's ramp is bound")
                .font
                .size,
            33.0,
            "the shaper's typography is the same projection of the same \
             theme as the registry's vocabulary, and follows the same swap"
        );
    }

    /// A name declared through [`Host::declare_token`] survives a theme
    /// swap; the registry's own vocabulary is rebuilt from the theme, so a
    /// name declared straight into it does not.
    #[test]
    fn an_application_declaration_survives_a_theme_swap() {
        use gorgon_petra::token::{DesignToken, Presenter, TokenKind, dark, light};

        let ours = TokenName::new("spacing.app-gutter").unwrap();
        let theirs = TokenName::new("spacing.written-into-the-registry").unwrap();

        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), Presenter::new(light()));
        host.declare_token(DesignToken::new(ours.clone(), TokenKind::Spacing));
        host.registry_mut()
            .vocabulary_mut()
            .declare(DesignToken::new(theirs.clone(), TokenKind::Spacing));
        assert!(host.registry_mut().vocabulary().contains(&ours));
        assert!(host.registry_mut().vocabulary().contains(&theirs));

        host.presenter().publish(dark());
        step(&ctx, &mut host, RawInput::default());

        assert!(
            host.registry_mut().vocabulary().contains(&ours),
            "declare_token is the declaration the host keeps and re-applies"
        );
        assert!(
            !host.registry_mut().vocabulary().contains(&theirs),
            "and registry_mut().vocabulary_mut() is documented as the one \
             that does not survive, so the doc must stay true"
        );
    }

    fn headless() -> Context {
        let ctx = Context::default();
        ctx.run_ui(RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        ctx
    }

    /// One frame, the way `eframe` drives it: the host's pass runs *inside* an
    /// egui pass, because that is the only place a layer painter's shapes are
    /// collected and a repaint request is observed.
    fn step(ctx: &Context, host: &mut Host<Demo>, input: RawInput) -> std::time::Duration {
        let out = ctx.run_ui(input, |_| host.pass(ctx));
        let delay = out
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(std::time::Duration::MAX);
        out.drop_without_applying_deltas();
        delay
    }

    /// An application deadline reaches the window as a timed repaint.
    ///
    /// A picture that is a function of the clock but is **not moving** had no
    /// way to ask for a frame. Catalog row 6's copy feedback is the first:
    /// it says `"Copied!"` for two seconds and then stops, and Petra paints
    /// nothing at idle (SC-002), so without this the word would sit on the
    /// screen until the operator's next input — which on a window nobody is
    /// touching is forever.
    ///
    /// Read where the request actually leaves, `ViewportOutput::repaint_delay`.
    /// An assertion that `App::wake_at` was *called* would pass with the host
    /// wiring missing, which is the state every seam in this file shipped in
    /// at least once.
    #[test]
    fn an_application_deadline_reaches_the_window_as_a_timed_repaint() {
        use std::time::Duration;
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        // The first pass seats focus and asks for one more to paint the ring
        // it just moved, so idle is measured after that has settled.
        let mut idle = Duration::ZERO;
        for _ in 0..4 {
            idle = step(&ctx, &mut host, RawInput::default());
        }
        assert!(
            idle > Duration::from_secs(60),
            "a window with nothing to do asked for a frame in {idle:?}"
        );

        host.app_mut().wake_in = Some(2.0);
        let armed = step(&ctx, &mut host, RawInput::default());
        assert!(
            armed > Duration::from_secs_f64(1.9) && armed <= Duration::from_secs_f64(2.0),
            "a two-second deadline armed a repaint in {armed:?}"
        );

        // Not a time at all: ignored, not passed on. `from_secs_f64` panics
        // on a NaN and a panic in the frame pump takes the window down.
        host.app_mut().wake_in = Some(f64::NAN);
        assert!(
            step(&ctx, &mut host, RawInput::default()) > Duration::from_secs(60),
            "a NaN deadline must be dropped, not armed"
        );

        // An application that stops asking lets the window sleep again, which
        // is the half of SC-002 this seam could have broken.
        host.app_mut().wake_in = None;
        let back = step(&ctx, &mut host, RawInput::default());
        assert!(
            back > Duration::from_secs(60),
            "the window never went back to idle: {back:?}"
        );

        // Already gone: the next frame the pump can give, rather than a panic
        // inside `Duration::from_secs_f64` on a negative. Last, because an
        // immediate request is the one egui carries into the following pass.
        host.app_mut().wake_in = Some(-5.0);
        assert_eq!(
            step(&ctx, &mut host, RawInput::default()),
            Duration::ZERO,
            "an overdue deadline asks for the next frame"
        );
    }

    #[test]
    fn one_pass_produces_a_frame_that_paints_completely() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());

        let frame = host.frame().expect("a frame");
        assert_eq!(frame.seq, 1);
        assert_eq!(frame.placements.len(), 5);
        let report = host.report().expect("a report");
        assert!(report.is_complete(), "{report:?}");
        assert_eq!(
            report.texts, 3,
            "the title, the field's placeholder, and the button"
        );
        assert!(report.unresolved_tokens.is_empty(), "{report:?}");
    }

    /// The host is what reaches the clipboard, because the application has
    /// no `Context`.
    ///
    /// Catalog row 6's Copy button dropped every press and looked exactly
    /// like a working one. `egui::Context::copy_text` was always there; the
    /// missing part was a channel to it. This asserts the whole channel, not
    /// the half the application owns: the string has to come out the far side
    /// as an `OutputCommand::CopyText`.
    #[test]
    fn a_string_the_application_asks_to_copy_reaches_egui_as_a_copy_command() {
        let ctx = Context::default();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        host.app_mut().copy = Some("pcargo test -p gorgon-petra --lib".to_owned());

        let out = ctx.run_ui(RawInput::default(), |_| host.pass(&ctx));
        let copied: Vec<&String> = out
            .platform_output
            .commands
            .iter()
            .filter_map(|cmd| match cmd {
                egui::OutputCommand::CopyText(text) => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(
            copied.as_slice(),
            [&"pcargo test -p gorgon-petra --lib".to_owned()],
            "the host must hand the application's string to egui, {:?}",
            out.platform_output.commands
        );
        out.drop_without_applying_deltas();

        // Taken, not read: a second pass with nothing newly asked for must
        // not copy the same string again.
        let again = ctx.run_ui(RawInput::default(), |_| host.pass(&ctx));
        assert!(
            !again
                .platform_output
                .commands
                .iter()
                .any(|cmd| matches!(cmd, egui::OutputCommand::CopyText(_))),
            "a pass that asked for nothing must copy nothing"
        );
        again.drop_without_applying_deltas();
    }

    #[test]
    fn frame_sequence_numbers_advance_and_never_repeat() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        let seqs: Vec<u64> = (0..3)
            .map(|_| {
                step(&ctx, &mut host, RawInput::default());
                host.frame().unwrap().seq
            })
            .collect();
        assert_eq!(seqs, [1, 2, 3]);
    }

    /// Step until the host stops asking for another frame, and answer how
    /// many passes that took. Panics rather than looping forever, because an
    /// unbounded stream of repaints is exactly what the zero-idle contract
    /// forbids and what this helper exists to catch.
    fn settle(ctx: &Context, host: &mut Host<Demo>, bound: usize) -> usize {
        for pass in 1..=bound {
            if !step(ctx, host, RawInput::default()).is_zero() {
                return pass;
            }
        }
        panic!("the host asked for {bound} frames in a row without settling");
    }

    /// The zero-idle contract on this side: with nothing moving, the host stops
    /// asking for frames.
    ///
    /// It takes three passes rather than one, and both extra passes are
    /// bounded and explained: the first pass seats focus *after* petrify (the
    /// focus tree needs that frame's placements), so it asks for the frame
    /// that will actually show the ring, and egui reports one further pass at
    /// zero delay after an immediate request before it settles. What the
    /// contract forbids is a host that never stops, which is what `settle`'s
    /// bound catches.
    #[test]
    fn an_idle_pass_requests_no_repaint() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        let passes = settle(&ctx, &mut host, 4);
        assert!(passes <= 3, "the host took {passes} passes to go idle");
        let delay = step(&ctx, &mut host, RawInput::default());
        assert!(
            !delay.is_zero(),
            "an idle host asked egui to come back immediately (delay {delay:?})"
        );
        assert!(host.frame().unwrap().transitions.is_settled());
    }

    /// Events on the very first pass have no frame to hit-test against. They
    /// must be reported as unrouted, not silently dropped.
    #[test]
    fn the_first_passs_events_are_reported_unrouted_not_swallowed() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        let mut input = RawInput::default();
        input.events.push(Event::Text("a".into()));
        step(&ctx, &mut host, input);
        let seen = &host.app().seen;
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(seen[0].0, "text");
        assert!(seen[0].1.starts_with("unrouted:"), "{seen:?}");
    }

    /// One key press, as the platform reports it.
    fn key_press(key: Key, modifiers: Modifiers) -> RawInput {
        let mut input = RawInput::default();
        input.events.push(Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });
        input
    }

    /// Once a frame exists, a keystroke routes to the focused node through the
    /// real router — the same path a driver's synthetic key will take.
    #[test]
    fn a_keystroke_routes_to_the_focused_node() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(host.state().focused.as_deref(), Some("/root/filter"));

        step(&ctx, &mut host, key_press(Key::A, Modifiers::NONE));

        let seen = &host.app().seen;
        assert!(
            seen.iter()
                .any(|(k, w)| k.starts_with("key:") && w == "/root/filter"),
            "{seen:?}"
        );
    }

    /// FR-025, the half that decides whether a window is operable at all:
    /// pressing Tab moves focus, and Shift+Tab moves it back. This is the test
    /// that fails when the host holds no focus tree — `state().focused` then
    /// stays `None` for every frame and every key press lands nowhere.
    #[test]
    fn tab_walks_focus_forward_and_shift_tab_walks_it_back() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/filter"),
            "the first focusable in visual order is seated by the first frame"
        );

        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/run"),
            "Tab moves to the next focusable in visual order"
        );

        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::SHIFT));
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/filter"),
            "Shift+Tab moves back"
        );
    }

    /// FR-025's other half. Tab *reaches* the button; Enter has to *work* it.
    /// A button declares `Click` and `Focus` and never `Key`, so this is the
    /// case that used to route nowhere.
    #[test]
    fn enter_works_the_focused_button_the_way_a_click_would() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert_eq!(host.state().focused.as_deref(), Some("/root/run"));

        step(&ctx, &mut host, key_press(Key::Enter, Modifiers::NONE));
        let seen = &host.app().seen;
        assert!(
            seen.iter()
                .any(|(k, w)| k == "key:Enter" && w == "/root/run"),
            "Enter did not land on the focused button: {seen:?}"
        );
    }

    /// Home traverses only when the focused node has no use for it. A text
    /// field declares `Key`, so it keeps its own Home; a button does not, so
    /// Home jumps to the first focusable.
    #[test]
    fn home_traverses_only_when_the_focused_node_cannot_use_it() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(host.state().focused.as_deref(), Some("/root/filter"));

        step(&ctx, &mut host, key_press(Key::Home, Modifiers::NONE));
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/filter"),
            "Home was stolen from a node that declares Key"
        );
        assert!(
            host.app()
                .seen
                .iter()
                .any(|(k, w)| k == "key:Home" && w == "/root/filter"),
            "{:?}",
            host.app().seen
        );

        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        step(&ctx, &mut host, key_press(Key::Home, Modifiers::NONE));
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/filter"),
            "Home on a button must move focus to the first focusable"
        );
        assert!(
            !host
                .app()
                .seen
                .iter()
                .any(|(k, w)| k == "key:Home" && w == "/root/run"),
            "a consumed traversal key was also delivered: {:?}",
            host.app().seen
        );
    }

    /// The vanished-focus rule, through the host rather than in isolation:
    /// the application stops emitting the focused node, and focus lands on a
    /// node that is really in the new frame.
    #[test]
    fn focus_survives_the_focused_node_leaving_the_tree() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert_eq!(host.state().focused.as_deref(), Some("/root/run"));
        let _ = passes_to_settle(&ctx, &mut host);

        host.app_mut().hide_run = true;
        step(&ctx, &mut host, RawInput::default());

        let focused = host.state().focused.clone().expect("focus went nowhere");
        assert_eq!(focused, "/root/filter");
        assert!(
            host.frame().unwrap().placement(&focused).is_some(),
            "focus points at a node that is not in the frame"
        );
    }

    /// A blocking surface takes focus when it opens, keeps Tab inside itself
    /// while it is open, and gives focus back to the page when it closes.
    /// This is the whole overlay-scope path: `surface_scopes` of the same tree
    /// that produced the placements, folded into the focus tree.
    #[test]
    fn a_blocking_surface_takes_focus_traps_tab_and_gives_it_back() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(host.state().focused.as_deref(), Some("/root/filter"));

        host.app_mut().modal = true;
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/modal/yes"),
            "an open modal left focus on the page behind it"
        );
        assert_eq!(host.focus().active_scope(), Some("/root/modal"));

        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert_eq!(host.state().focused.as_deref(), Some("/root/modal/no"));
        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/modal/yes"),
            "Tab escaped the modal instead of wrapping inside it"
        );
        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::SHIFT));
        assert_eq!(host.state().focused.as_deref(), Some("/root/modal/no"));
        let _ = passes_to_settle(&ctx, &mut host);

        host.app_mut().modal = false;
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/run"),
            "focus did not come back out of a closed modal"
        );
        assert_eq!(host.focus().active_scope(), None);
    }

    /// A screen reader must hear the frame the host just painted, through the
    /// host's own pass — not through a projection some other caller remembers
    /// to run.
    ///
    /// This is the assertion the crate doc's claim rests on, and it is written
    /// against the configuration where the wiring can fail: `enable_accesskit`
    /// on, so `egui::Context::end_pass` writes its own one-node
    /// `Role::Window` tree. Asserting `is_some()` would pass on that tree.
    /// The assertion is therefore on a Petra node, by id and by label.
    #[test]
    fn a_host_pass_publishes_the_painted_frame_to_accesskit() {
        let ctx = headless();
        ctx.enable_accesskit();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());

        let mut out = ctx.run_ui(RawInput::default(), |_| host.pass(&ctx));
        // `epaint` asserts on drop that a texture delta was handled; this test
        // reads `platform_output` only, so the delta is cleared rather than
        // silently dropped.
        out.textures_delta.clear();

        let update = out
            .platform_output
            .accesskit_update
            .expect("accesskit is enabled, so end_pass always writes something");
        let wanted = egui::Id::new("/root/run").accesskit_id();
        let (_, node) = update
            .nodes
            .iter()
            .find(|(id, _)| *id == wanted)
            .unwrap_or_else(|| {
                panic!(
                    "the host published no node for /root/run — {} node(s) on \
                     the output, which is egui's own tree and not Petra's",
                    update.nodes.len()
                )
            });
        assert_eq!(node.label(), Some("Run"));
    }

    /// The frame the host paints must show where focus is. The ring is drawn
    /// from `PlacementSemantics::focused`, which is projected at petrify from
    /// `LayoutState::focused` — so this is the end-to-end check that the host's
    /// focus tree, the engine's projection, and the painter's ring are one
    /// chain and not three unconnected pieces.
    #[test]
    fn the_frame_the_host_paints_shows_where_focus_is() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        // The first pass seats focus after petrify, so its own frame is
        // ringless; the repaint it asks for is what produces the one below.
        settle(&ctx, &mut host, 4);

        assert_eq!(host.state().focused.as_deref(), Some("/root/filter"));
        let frame = host.frame().expect("a frame");
        let ringed: Vec<&str> = frame
            .placements
            .iter()
            .filter(|p| p.semantics.focused)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(ringed, ["/root/filter"], "the frame marks no focused node");
        assert_eq!(
            host.report().expect("a report").focus_rings,
            1,
            "the focused node reached the painter without a ring"
        );

        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert_eq!(host.state().focused.as_deref(), Some("/root/run"));
        let moved: Vec<&str> = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .filter(|p| p.semantics.focused)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(
            moved,
            ["/root/run"],
            "Tab moved focus but the ring stayed behind — a traversal move is \
             published before petrify precisely so this pass shows it"
        );
    }

    /// The underline interpolates, then the host goes idle. A caret that kept
    /// requesting frames after landing would break SC-002.
    #[test]
    fn tab_does_not_leave_the_caret_running() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        settle(&ctx, &mut host, 4);
        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert_eq!(host.state().focused.as_deref(), Some("/root/run"));
        let n = passes_to_settle(&ctx, &mut host);
        assert!(n <= 32, "the caret was still flying after {n} quiet passes");
        assert!(!host.caret().is_moving());
        let dest = settled_caret_bar(&host);
        let landed = host.caret().rect().expect("visible once settled");
        assert!(
            (landed.min.x - dest.min.x).abs() < 0.5
                && (landed.min.y - dest.min.y).abs() < 0.5
                && (landed.width() - dest.width()).abs() < 0.5,
            "settled caret {landed:?} must match the focused bar {dest:?}"
        );
    }

    /// Reduced motion is a snap, not a short flight.
    #[test]
    fn reduced_motion_snaps_the_caret_on_tab() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        settle(&ctx, &mut host, 4);
        host.set_reduced_motion(true);
        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert_eq!(host.state().focused.as_deref(), Some("/root/run"));
        assert!(!host.caret().is_moving());
        let dest = settled_caret_bar(&host);
        assert_eq!(host.caret().rect(), Some(dest));
    }

    /// A 160 ms hop on a full page showed three frames because every vsync
    /// rebuilt the tree. Flight paints the last petrified picture.
    #[test]
    fn a_flying_caret_does_not_rebuild_the_tree() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        settle(&ctx, &mut host, 4);
        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert!(host.caret().is_moving(), "Tab must start a hop");
        assert!(
            host.scene_mesh_count() > 0,
            "a hop must tessellate the picture once so later vsyncs replay meshes"
        );
        let after_tab = host.app().view_calls;
        let n = passes_to_settle(&ctx, &mut host);
        assert!(
            n >= 5,
            "the hop must last several quiet passes so reuse is observable, got {n}"
        );
        let rebuilt = host.app().view_calls - after_tab;
        assert!(
            rebuilt <= 1,
            "flight frames must reuse the last picture; view() ran {rebuilt} extra times over {n} passes"
        );
        assert!(!host.caret().is_moving());
        assert!(
            host.last_hop_passes() >= 5,
            "a hop must present several times, not three; last hop was {} passes",
            host.last_hop_passes()
        );
    }

    /// Reuse must not walk placements once a scene exists. Headless lib tests
    /// have no GPU texture; the mesh cache is the scene.
    #[test]
    fn a_hop_does_not_repaint_placements_once_the_scene_is_cached() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        settle(&ctx, &mut host, 4);
        step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
        assert!(host.caret().is_moving(), "Tab must start a hop");
        assert!(
            host.scene_mesh_count() > 0,
            "a hop must tessellate the picture once so later vsyncs replay meshes"
        );
        assert!(
            !host.scene_texture_bound(),
            "headless lib tests must not require a GPU texture"
        );
        let paints = host.full_paint_count();
        let mut n = 0;
        while host.caret().is_moving() {
            n += 1;
            assert!(n <= 32, "the caret was still flying after {n} quiet passes");
            step(&ctx, &mut host, RawInput::default());
            assert_eq!(
                host.full_paint_count(),
                paints,
                "reuse must not call paint_frame_with_caret while the scene is cached (hop pass {n})"
            );
        }
        assert!(
            n >= 5,
            "the hop must last several quiet passes so reuse is observable, got {n}"
        );
    }

    /// Focus that moves *after* the frame is placed — the vanished-focus rule
    /// and a modal taking focus both do — leaves that frame's ring on the
    /// wrong node. The host must ask for one more frame, or on an idle window
    /// the indicator never appears at all.
    #[test]
    fn focus_moving_after_petrify_asks_for_another_frame() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());

        // Pass 1: nothing was focused when the frame was placed, and `update`
        // seats focus on the first focusable afterwards.
        //
        // Deliberately *not* asserted here: that this pass asked for a
        // repaint. egui's own first pass requests one for its own reasons
        // (font atlas, initial sizing), so the assertion would pass with this
        // host's request deleted — a test that cannot fail is worse than no
        // test. The modal below is the steady-state case, and it does fail.
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(host.state().focused.as_deref(), Some("/root/filter"));
        assert!(
            host.frame()
                .unwrap()
                .placements
                .iter()
                .all(|p| !p.semantics.focused),
            "this frame was placed before focus existed"
        );

        // The frame that request produced shows the ring, and then the host
        // goes idle again rather than repainting forever.
        settle(&ctx, &mut host, 4);
        assert_eq!(
            host.report().unwrap().focus_rings,
            1,
            "the repaint arrived and still nothing is ringed"
        );

        // A modal opening takes focus after petrify too.
        host.app_mut().modal = true;
        let opened = step(&ctx, &mut host, RawInput::default());
        assert_eq!(host.state().focused.as_deref(), Some("/root/modal/yes"));
        assert!(
            opened.is_zero(),
            "a modal took focus and the ring was left on the page behind it \
             until the next unrelated event (delay {opened:?})"
        );
        assert!(
            host.frame()
                .unwrap()
                .placements
                .iter()
                .all(|p| p.semantics.focused == (p.id == "/root/filter")),
            "this frame still rings the field behind the modal"
        );
        settle(&ctx, &mut host, 24);
        assert_eq!(
            host.report().unwrap().focus_rings,
            1,
            "the modal's focused button is unringed"
        );
        let ringed: Vec<&str> = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .filter(|p| p.semantics.focused)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(ringed, ["/root/modal/yes"]);
    }

    /// A refused tree is shown, not swallowed and not fatal.
    #[test]
    fn a_refused_tree_is_painted_rather_than_hidden() {
        let ctx = headless();
        let demo = Demo {
            bad_tree: true,
            ..Demo::default()
        };
        let mut host = Host::new(&ctx, demo, default_presenter());
        step(&ctx, &mut host, RawInput::default());
        let frame = host.frame().unwrap();
        let text = frame
            .content
            .iter()
            .filter_map(|c| c.text.as_ref())
            .map(|t| t.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("/root/dup"), "{text}");
        assert!(text.contains("unique"), "{text}");
        assert!(host.report().unwrap().is_complete());
    }

    #[test]
    fn the_refusal_view_is_a_well_formed_tree() {
        let view = refusal_view("boom");
        // The same registry `pass` validates it against (above): the
        // shipped vocabulary, not an empty one — `refusal_view` names this
        // crate's own shipped tokens, never an application's.
        assert!(
            gorgon_petra::tree::validate(
                &view,
                &gorgon_petra::tree::Registry::with_vocabulary(
                    gorgon_petra::token::standard_vocabulary()
                )
            )
            .is_ok()
        );
        assert_eq!(petra_layer(), petra_layer());
    }

    // -----------------------------------------------------------------------
    // Pointer state (`contracts/interaction-state.md` §7, §8, §9)
    // -----------------------------------------------------------------------

    /// One pointer move to `pos`, the way egui reports it.
    fn move_to(pos: egui::Pos2) -> RawInput {
        let mut input = RawInput::default();
        input.events.push(Event::PointerMoved(pos));
        input
    }

    /// The centre of the placement whose id is exactly `id`.
    fn centre_of(host: &Host<Demo>, id: &str) -> egui::Pos2 {
        let rect = host
            .frame()
            .expect("a frame")
            .placement(id)
            .unwrap_or_else(|| panic!("no placement `{id}`"))
            .rect;
        egui::Pos2::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
    }

    fn hovered_ids(host: &Host<Demo>) -> Vec<&str> {
        host.frame()
            .expect("a frame")
            .placements
            .iter()
            .filter(|p| p.semantics.hovered)
            .map(|p| p.id.as_str())
            .collect()
    }

    /// Hover reaches the picture through the shipped host, and exactly one
    /// node carries it.
    ///
    /// The pointer never touches `PointerState` directly here: this is an
    /// `egui::Event::PointerMoved` going in the same door a mouse uses, and
    /// the assertion is on the *placement flag* the painter reads, at the far
    /// end of translate → route → publish → petrify.
    #[test]
    fn a_pointer_move_lights_exactly_the_node_under_it() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert!(hovered_ids(&host).is_empty(), "nothing is hovered at rest");

        let run = centre_of(&host, "/root/run");
        step(&ctx, &mut host, move_to(run));
        assert_eq!(host.pointer().hovered(), Some("/root/run"));
        assert_eq!(hovered_ids(&host), vec!["/root/run"]);

        // Off every interactive node: hover clears rather than sticking to the
        // last thing it touched.
        let (_, outside) = placed_and_outside(&host, "/root/run");
        step(&ctx, &mut host, move_to(outside));
        assert_eq!(host.pointer().hovered(), None);
        assert!(hovered_ids(&host).is_empty());
    }

    /// Pointer-exit lands on the node the pointer was last over.
    ///
    /// This is the seam `gorgon_petra::input::route` cannot close and this
    /// host does (`contracts/interaction-state.md` §8): the router has no
    /// memory between frames and must hand `route_pointer_exit` a `None`,
    /// which reports the exit dropped. The call below is that same function
    /// with the host's real hovered id — the thing lane A left a `None` in
    /// place of — and the assertion is that the two answers differ.
    #[test]
    fn pointer_exit_lands_on_the_hovered_node() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        let run = centre_of(&host, "/root/run");
        step(&ctx, &mut host, move_to(run));
        // Focus is deliberately somewhere else, which is what the pre-fix
        // behaviour would have delivered the exit to.
        host.focus_mut().focus("/root/filter").expect("focusable");
        assert_eq!(host.pointer().hovered(), Some("/root/run"));

        let frame = host.frame().expect("a frame");
        assert_eq!(
            route_pointer_exit(frame, host.pointer().hovered()),
            Route::Pointer {
                node: "/root/run".to_owned()
            },
            "the host's hovered id is what aims a pointer-exit"
        );
        assert!(
            matches!(route_pointer_exit(frame, None), Route::Unrouted { .. }),
            "and a caller with no memory of it can only report the drop"
        );

        // Through the real door: the exit is delivered, and hover clears.
        let mut input = RawInput::default();
        input.events.push(Event::PointerGone);
        step(&ctx, &mut host, input);
        assert_eq!(host.pointer().hovered(), None);
        assert!(
            host.app()
                .seen
                .iter()
                .any(|(_, where_)| where_ == "/root/run"),
            "the application never heard the exit: {:?}",
            host.app().seen
        );
    }

    /// A hover change costs the frame it arrives on and settles immediately
    /// after it.
    ///
    /// SC-002 is a claim about an *idle* window, and hover is the state most
    /// able to break it: it changes on every mouse twitch, and a host that
    /// asked for a follow-up frame each time would leave a window with a
    /// pointer resting in it repainting forever.
    ///
    /// **egui repaints on input by itself**, so the pass carrying the move
    /// reports a zero delay whatever Petra does — asserting on that pass
    /// would be asserting about egui. The pass *after* it is the one Petra
    /// decides, and it settles: the snapshot is published before petrify, so
    /// the frame the move produced already showed the hover; the
    /// post-petrify reconciliation re-derived the same answer against the
    /// frame it had just placed, `publish_pointer` reported no move, and
    /// nothing was scheduled.
    ///
    /// The baseline in the middle is what stops this from passing for the
    /// wrong reason: a move that changes no hover settles the same way, so
    /// the assertion is about the hover rather than about the window being
    /// quiet in general.
    #[test]
    fn a_hover_change_settles_on_the_next_frame() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        let run = centre_of(&host, "/root/run");
        let (_, outside) = placed_and_outside(&host, "/root/run");

        // Baseline: a move that lights nothing. egui itself keeps asking for
        // a frame or two after any input, so this is what "settled" costs
        // when Petra contributes nothing at all.
        step(&ctx, &mut host, move_to(outside));
        assert!(hovered_ids(&host).is_empty());
        let quiet = passes_to_settle(&ctx, &mut host);

        step(&ctx, &mut host, move_to(run));
        assert_eq!(hovered_ids(&host), vec!["/root/run"]);
        let lit = passes_to_settle(&ctx, &mut host);
        assert_eq!(hovered_ids(&host), vec!["/root/run"], "and it stays lit");
        assert_eq!(
            lit, quiet,
            "a hover change cost {lit} passes to settle where a move that lit \
             nothing cost {quiet}; an idle window with a pointer resting in it \
             would never be byte-identical twice"
        );
        assert_eq!(
            step(&ctx, &mut host, RawInput::default()),
            std::time::Duration::MAX,
            "and it is still settled a frame later"
        );
    }

    /// Pass with no input until the window asks for nothing, and answer how
    /// many passes that took. Panics rather than spinning: a window that
    /// never settles is the failure, not a reason to hang.
    fn passes_to_settle(ctx: &Context, host: &mut Host<Demo>) -> usize {
        for count in 1..=32 {
            if step(ctx, host, RawInput::default()) == std::time::Duration::MAX {
                return count;
            }
        }
        panic!("the window never settled over 32 quiet passes");
    }

    /// The first band a settled caret paints, for the tests that track one
    /// rectangle through a hop.
    fn settled_caret_bar(host: &Host<Demo>) -> egui::Rect {
        let frame = host.frame().expect("a frame");
        let target = crate::paint::focused_caret_target(frame).expect("something focused");
        crate::paint::caret_bands(target.mark, target.figure, frame.viewport.scale)
            .into_iter()
            .find(|band| band.width() > 0.0 && band.height() > 0.0)
            .expect("a figure paints at least one band")
    }

    /// A press with a button held is a press. It does **not** grab a
    /// control that never asked to be dragged, and it **does** make that
    /// control look pressed.
    ///
    /// The Demo page declares no `Interaction::Drag` anywhere, so this is
    /// the fall-through half of the grab rule through the shipped host:
    /// pressing still routes as a click and no capture is opened.
    ///
    /// # The half of this that changed, and why
    ///
    /// Until round 4 this test also asserted `pressed() == None` and
    /// `!semantics.active` everywhere, because `PointerState::pressed` was
    /// derived from the capture alone. That made `active` permanently false
    /// for every button, checkbox and menu item in `crate::component` — none
    /// of which declare `Drag` — so every `background@active` binding in the
    /// library was a token nothing could read, and a press animation had no
    /// state to key off.
    ///
    /// Pressed is not a routing fact. `input.rs`'s `down` bit answers it
    /// from the pointer instead, and the capture claim above is untouched:
    /// nothing is captured, nothing routes anywhere new. The two are
    /// asserted together here on purpose, because collapsing them again is
    /// the mistake this test now exists to catch in both directions.
    #[test]
    fn a_press_on_a_click_only_control_presses_it_without_grabbing_it() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        let run = centre_of(&host, "/root/run");

        step(&ctx, &mut host, press_at(run));
        assert!(
            host.pointer().capture().is_none(),
            "nothing declared `Drag`, so no gesture may be opened"
        );
        assert_eq!(
            host.pointer().pressed(),
            Some("/root/run"),
            "the control under a held primary button is pressed, capture or no"
        );
        let frame = host.frame().expect("a frame");
        assert!(
            frame.placements.iter().all(|p| !p.semantics.captured),
            "nothing declared `Drag`, so nothing is captured"
        );
        let active: Vec<&str> = frame
            .placements
            .iter()
            .filter(|p| p.semantics.active)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(
            active,
            vec!["/root/run"],
            "exactly the node under the pointer is active"
        );
        assert_eq!(host.pointer().hovered(), Some("/root/run"));
    }

    /// A primary press on a focusable seats keyboard focus so the flying
    /// caret retargets. Clicking is not a second, silent focus world.
    #[test]
    fn a_press_on_a_focusable_moves_the_caret() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        settle(&ctx, &mut host, 4);
        assert_eq!(host.state().focused.as_deref(), Some("/root/filter"));
        let run = centre_of(&host, "/root/run");
        step(&ctx, &mut host, press_at(run));
        assert_eq!(host.state().focused.as_deref(), Some("/root/run"));
        assert_eq!(host.caret().id(), Some("/root/run"));
    }

    /// Both shapes of "copy this", and nothing else.
    ///
    /// The wiring below this predicate is one body, exercised end to end by
    /// the gallery's six clipboard captures through `InputEvent::Copy`. What
    /// only this test covers is the keystroke arm, which exists because the
    /// `winit` to `egui` seam cannot be constructed in a test at all:
    /// `winit::event::KeyEvent` carries a `pub(crate)` field, so no crate but
    /// `winit` can build one.
    #[test]
    fn a_copy_is_the_platforms_copy_event_or_the_chord_and_nothing_else() {
        use gorgon_petra::input::{InputEvent, KeyCode, Modifiers};
        let chord = |modifiers, pressed| InputEvent::Key {
            key: KeyCode::Char('c'),
            pressed,
            repeat: false,
            modifiers,
        };
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let meta = Modifiers {
            meta: true,
            ..Modifiers::NONE
        };

        assert!(super::asks_for_a_copy(&InputEvent::Copy));
        assert!(super::asks_for_a_copy(&chord(ctrl, true)));
        assert!(super::asks_for_a_copy(&chord(meta, true)), "Cmd+C on macOS");

        assert!(
            !super::asks_for_a_copy(&chord(Modifiers::NONE, true)),
            "a bare c is a letter"
        );
        assert!(
            !super::asks_for_a_copy(&chord(Modifiers::shift(), true)),
            "Shift+C is a capital letter"
        );
        assert!(
            !super::asks_for_a_copy(&chord(ctrl, false)),
            "the release does not copy a second time"
        );
        assert!(
            !super::asks_for_a_copy(&InputEvent::Key {
                key: KeyCode::Char('x'),
                pressed: true,
                repeat: false,
                modifiers: ctrl,
            }),
            "cut is not copy, and Petra has nothing that cuts"
        );
    }
}
