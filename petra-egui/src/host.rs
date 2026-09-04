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

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Context, Id, LayerId, Order};
use gorgon_petra::anim::{FrameDecision, TransitionRegistry, wants_frame};
use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{
    FrameCounter, PetrifiedFrame, Placement, TransitionActivity, Viewport, petrify,
};
use gorgon_petra::geom::{Axis, Rect, Scale, Size};
use gorgon_petra::input::{
    InputEvent, KeyCode, PointerButton, PointerRouting, PointerState, Route, RouteOutcome,
};
use gorgon_petra::layout::overlay_surface::surface_scopes;
use gorgon_petra::layout::{
    AnchorRects, ChangeSet, LayoutCtx, LayoutState, MeasureCache, RowSource, ScrollStack,
};
use gorgon_petra::token::value::CoverageValue;
use gorgon_petra::token::{
    DesignToken, Presenter, StatusToken, Theme, ThemeSnapshot, TokenName, TokenValue, Vocabulary,
    standard_vocabulary,
};
use gorgon_petra::tree::{
    InputPolicy, Interaction, NodeKind, Props, Registry, ValidatedTree, ViewNode, validate,
};

use crate::focus_caret::FocusCaret;
use crate::image::ImageSources;
use crate::input::EventTranslator;
use crate::paint::{
    CustomPainters, PaintReport, caret_clip_limit, caret_dest_pair, focused_caret_target,
    paint_caret_overlay, paint_frame_with_caret,
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
    /// Where the pointer is, what it is over, and what it has captured. The
    /// pointer's peer of `focus`; see this module's doc.
    pointer: PointerState,
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
            pointer: PointerState::new(),
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
        self.cache
            .retain_theme_and_scale(viewport.theme_rev, viewport.scale);
        self.cache.apply(&self.app.take_changes());

        let app_tree = self.app.view();
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

    fn caret_overlay(&self, frame: &PetrifiedFrame) -> Option<(Vec<egui::Rect>, egui::Rect)> {
        let scale = frame.viewport.scale;
        let page = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(frame.viewport.size.w, frame.viewport.size.h),
        );
        self.caret.bars().map(|bars| {
            (
                bars,
                caret_clip_limit(self.caret.clip(), self.caret.is_moving(), page, scale),
            )
        })
    }

    fn tick_caret(&mut self, frame: &PetrifiedFrame, now: f64) {
        let scale = frame.viewport.scale;
        match focused_caret_target(frame) {
            Some((id, node, figure, clip)) => {
                self.caret.tick(
                    Some((id, caret_dest_pair(node, figure, scale), figure, clip)),
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
            Some((Vec::new(), page)),
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
        if let Some((bars, clip)) = overlay
            && paint_caret_overlay(
                &ctx.layer_painter(petra_layer()),
                &bars,
                clip,
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
        if let Some((bars, clip)) = self.caret_overlay(frame)
            && paint_caret_overlay(
                painter,
                &bars,
                clip,
                colors,
                frame.viewport.scale,
                &mut report,
            )
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

    fn picture_must_rebuild(event: &InputEvent) -> bool {
        match event {
            // Hover is a projection. A compositor that repeats pointer
            // position every vsync must not rebuild the gallery under a hop.
            InputEvent::PointerMoved { .. } => false,
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
                            reason: "no frame has been placed yet",
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
            if Self::picture_must_rebuild(event) {
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
        hide_run: bool,
        /// How many times [`App::view`] ran. A caret in flight must not
        /// increment this on every vsync.
        view_calls: usize,
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
                Route::Pointer { node } | Route::Keyboard { node } => node.clone(),
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

    fn settled_caret_bar(host: &Host<Demo>) -> egui::Rect {
        let frame = host.frame().expect("a frame");
        let (node, figure) = crate::paint::focused_caret_target(frame)
            .map(|(_, node, figure, _)| (node, figure))
            .expect("something focused");
        crate::paint::caret_dest_pair(node, figure, frame.viewport.scale)[0]
    }

    /// A press with a button held is a press, and it does not grab a control
    /// that never asked to be dragged.
    ///
    /// The Demo page declares no `Interaction::Drag` anywhere, so this is the
    /// fall-through half of the grab rule through the shipped host: pressing
    /// still routes as a click and no capture is opened.
    #[test]
    fn a_press_on_a_click_only_control_opens_no_gesture() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        let run = centre_of(&host, "/root/run");

        step(&ctx, &mut host, press_at(run));
        assert!(host.pointer().capture().is_none());
        assert_eq!(host.pointer().pressed(), None);
        assert!(
            host.frame()
                .expect("a frame")
                .placements
                .iter()
                .all(|p| !p.semantics.captured && !p.semantics.active),
            "nothing declared `Drag`, so nothing is captured"
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
}
