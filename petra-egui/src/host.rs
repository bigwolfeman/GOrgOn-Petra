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
//! [`gorgon_petra::frame::PlacementSemantics::focused`], and therefore the
//! focus ring [`crate::paint`] draws, is projected from. Tab and Shift+Tab are consumed here for
//! traversal rather than delivered to the application; everything else is
//! routed. That is the whole of the wiring, and it is deliberately in the
//! host: `gorgon-petra` decides *what* focus does, this crate decides *when*.

use std::collections::BTreeMap;

use egui::{Context, Id, LayerId, Order};
use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{FrameCounter, PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Scale, Size};
use gorgon_petra::input::{InputEvent, KeyCode, Route, route};
use gorgon_petra::layout::overlay_surface::surface_scopes;
use gorgon_petra::layout::{
    ChangeSet, LayoutCtx, LayoutState, MeasureCache, RowSource, ScrollStack,
};
use gorgon_petra::token::Presenter;
use gorgon_petra::tree::{InputPolicy, Interaction, NodeKind, Props, Registry, ViewNode, validate};

use crate::input::EventTranslator;
use crate::paint::{PaintReport, paint_frame};
use crate::text::GalleyShaper;

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
    fn handle(&mut self, event: &InputEvent, route: &Route);
    /// What changed behind [`App::view`] since the last call, draining it in
    /// the same motion. Must name (directly or via [`ChangeSet::All`]) every
    /// node whose measured content this frame would answer differently for.
    fn take_changes(&mut self) -> ChangeSet;
}

/// Drives one Petra application inside an `eframe` window.
pub struct Host<A: App> {
    app: A,
    shaper: GalleyShaper,
    translator: EventTranslator,
    cache: MeasureCache,
    counter: FrameCounter,
    state: LayoutState,
    presenter: Presenter,
    registry: Registry,
    last_frame: Option<PetrifiedFrame>,
    last_report: Option<PaintReport>,
    focus: FocusTree,
}

impl<A: App> Host<A> {
    /// A host over `app`, using `ctx` for fonts and `presenter` for the theme.
    pub fn new(ctx: &Context, app: A, presenter: Presenter) -> Self {
        Self {
            app,
            shaper: GalleyShaper::new(ctx.clone()),
            translator: EventTranslator::new(),
            cache: MeasureCache::new(),
            counter: FrameCounter::new(),
            state: LayoutState::default(),
            presenter,
            registry: Registry::new(),
            last_frame: None,
            last_report: None,
            focus: FocusTree::default(),
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
    pub fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
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

    /// Run one whole pass: input, view, petrify, paint, schedule.
    ///
    /// Public and independent of `eframe` so a test or the driver can step the
    /// host without a window.
    pub fn pass(&mut self, ctx: &Context) {
        let snapshot = self.presenter.current();
        let scale = Scale::new(ctx.pixels_per_point()).unwrap_or(Scale::ONE);
        let screen = ctx.content_rect();
        let viewport = Viewport {
            size: Size::new(screen.width(), screen.height()),
            scale,
            theme_rev: snapshot.revision(),
            theme_mode: snapshot.mode(),
        };

        self.deliver_input(ctx);
        // Input may have moved focus; the negotiation below reads
        // `LayoutState`, so publish it before the frame is measured rather
        // than after it is painted. A move published here needs no repaint
        // request: this pass's own frame is placed from it, ring and all.
        let _ = self.publish_focus();

        // Theme and scale are global inputs to every measurement, so a change
        // to either invalidates wholesale; content changes are the
        // application's change set, applied here so no host wiring can skip
        // the ancestor walk it requires.
        self.cache
            .retain_theme_and_scale(viewport.theme_rev, viewport.scale);
        self.cache.apply(&self.app.take_changes());

        let tree = self.app.view();
        let tree = match validate(&tree, &self.registry) {
            Ok(()) => tree,
            // A refused tree is a bug in the application, and the operator has
            // to be able to see which node. Painting the violations is louder
            // than a log line and does not take the window down.
            Err(errors) => refusal_view(&errors.to_string()),
        };

        let frame = {
            let mut ctx_layout = LayoutCtx {
                content: &mut self.shaper,
                rows: &mut self.app,
                cache: &mut self.cache,
                state: &self.state,
                theme_rev: viewport.theme_rev,
                scale: viewport.scale,
                scroll: ScrollStack::new(),
            };
            petrify(
                self.counter.take(),
                &tree,
                &mut ctx_layout,
                viewport,
                // Transitions land in US4. Until then every frame is settled,
                // which is the honest report: nothing is moving.
                TransitionActivity::default(),
            )
        };

        let report = paint_frame(
            &ctx.layer_painter(petra_layer()),
            &frame,
            &mut self.shaper,
            snapshot.as_ref(),
        );
        debug_assert!(
            report.is_complete(),
            "a paint pass must account for every placement and leave none \
             silent: {} drawn + {} clipped + {} empty + {} silent of {} \
             placement(s), desynced={}",
            report.drawn,
            report.skipped_clipped,
            report.empty,
            report.silent,
            report.placements,
            report.desynced
        );

        // The frame is placed, so this is the first moment the new placements
        // exist to reconcile focus against: `update` carries the focused node
        // forward, or applies the vanished-focus rule when it is gone. Doing
        // it here rather than at the top of the next pass is what keeps an
        // event arriving between two frames aimed at a node that still exists.
        let scopes = surface_scopes(&tree);
        self.focus = self.focus.update(&frame.placements, &scopes);
        self.enter_open_modal(&frame, &scopes);
        if self.publish_focus() {
            // The frame just painted was placed from the *previous* focus, so
            // its ring is on the wrong node or on none at all. Both moves that
            // reach here happen after petrify by necessity — the vanished-focus
            // rule and a modal taking focus both need the new placements to
            // decide — so the only honest fix is to ask for one more frame.
            // Without this the indicator waits for the next unrelated event,
            // which on an idle window is forever.
            ctx.request_repaint();
        }

        self.schedule(ctx, &frame);
        self.last_frame = Some(frame);
        self.last_report = Some(report);
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

    fn deliver_input(&mut self, ctx: &Context) {
        let events = ctx.input(|i| i.events.clone());
        let translated = self.translator.translate_all(&events);
        if translated.is_empty() {
            return;
        }
        // Routing needs a frame to hit-test against. The first pass has none,
        // so its events are dropped rather than delivered to nothing — and the
        // application is told, through an `Unrouted` route, instead of the
        // event just vanishing.
        for event in &translated {
            if self.traverse(event) {
                continue;
            }
            let route = match &self.last_frame {
                Some(frame) => route(frame, self.state.focused.as_deref(), event),
                None => Route::Unrouted {
                    reason: "no frame has been placed yet",
                },
            };
            self.app.handle(event, &route);
        }
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
            KeyCode::Tab if modifiers.shift => self.focus.previous(),
            KeyCode::Tab => self.focus.next(),
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
    fn schedule(&self, ctx: &Context, frame: &PetrifiedFrame) {
        if frame.transitions.running > 0 || frame.transitions.ambient > 0 {
            ctx.request_repaint();
        }
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.pass(ui.ctx());
    }
}

/// The view a refused tree is replaced with, so the violations are on screen.
fn refusal_view(message: &str) -> ViewNode {
    let mut tokens = Props::default();
    tokens
        .tokens
        .insert("background".into(), "surface.base".into());
    tokens
        .tokens
        .insert("foreground".into(), "status.down".into());
    ViewNode::new(NodeKind::Stack, "petra-tree-refused")
        .with_props(tokens)
        .child(
            ViewNode::new(NodeKind::Text, "violations").with_props(Props {
                text: Some(message.to_owned()),
                ..Props::default()
            }),
        )
}

/// A presenter over the shipped dark theme, for callers that have no theme of
/// their own yet.
#[must_use]
pub fn default_presenter() -> Presenter {
    Presenter::new(gorgon_petra::token::dark())
}

#[cfg(test)]
mod tests {
    use super::{App, ChangeSet, Host, default_presenter, petra_layer, refusal_view};
    use egui::{Context, Event, Key, Modifiers, RawInput};
    use gorgon_petra::input::{InputEvent, Route};
    use gorgon_petra::layout::RowSource;
    use gorgon_petra::tree::{
        Anchor, ClampRule, InputPolicy, Interaction, Layer, NodeKind, Props, Role, ViewNode,
    };
    use std::ops::Range;

    #[derive(Default)]
    struct Demo {
        seen: Vec<(String, String)>,
        bad_tree: bool,
        modal: bool,
        hide_run: bool,
    }

    /// A focusable, clickable text button — a `Role::Button` node declaring
    /// exactly what a real one does: `Click` and `Focus`, never `Key`.
    fn button(key: &str, label: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key)
            .with_props(Props {
                text: Some(label.to_owned()),
                ..Props::default()
            })
            .interactive(
                Role::Button,
                label.to_owned(),
                &[Interaction::Focus, Interaction::Click],
            )
    }

    impl RowSource for Demo {
        fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<ViewNode> {
            Vec::new()
        }
    }

    impl App for Demo {
        fn view(&mut self) -> ViewNode {
            if self.bad_tree {
                // Two children with one key: a tree-acceptance violation.
                return ViewNode::new(NodeKind::Stack, "root")
                    .child(ViewNode::new(NodeKind::Text, "dup"))
                    .child(ViewNode::new(NodeKind::Text, "dup"));
            }
            let mut panel = Props::default();
            panel
                .tokens
                .insert("background".into(), "surface.base".into());
            ViewNode::new(NodeKind::Stack, "root")
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
                })
        }

        fn handle(&mut self, event: &InputEvent, route: &Route) {
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

        fn take_changes(&mut self) -> ChangeSet {
            // The fixture does not track which nodes moved; `All` is the
            // honest conservative answer and is what every test below relies
            // on when it mutates a field and expects the next pass to see it.
            ChangeSet::All
        }
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

        host.app_mut().modal = false;
        step(&ctx, &mut host, RawInput::default());
        assert_eq!(
            host.state().focused.as_deref(),
            Some("/root/run"),
            "focus did not come back out of a closed modal"
        );
        assert_eq!(host.focus().active_scope(), None);
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
        settle(&ctx, &mut host, 4);
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
        assert!(gorgon_petra::tree::validate(&view, &gorgon_petra::tree::Registry::new()).is_ok());
        assert_eq!(petra_layer(), petra_layer());
    }
}
