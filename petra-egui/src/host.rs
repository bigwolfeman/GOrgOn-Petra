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

use egui::{Context, Id, LayerId, Order};
use gorgon_petra::frame::{FrameCounter, PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Scale, Size};
use gorgon_petra::input::{InputEvent, Route, route};
use gorgon_petra::layout::{LayoutCtx, LayoutState, MeasureCache, RowSource};
use gorgon_petra::token::Presenter;
use gorgon_petra::tree::{NodeKind, Props, Registry, ViewNode, validate};

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
/// Deliberately three methods. State lives in the application (D-075), the view
/// tree is plain data, and the host neither owns nor inspects the state — it
/// only needs to know when the state changed, which is what `content_rev` is
/// for: it is part of the measurement cache key, so a stale revision shows up
/// as a stale layout rather than as a silently wrong one.
pub trait App: RowSource {
    /// The view tree for this frame.
    fn view(&mut self) -> ViewNode;
    /// Handle one routed input event.
    fn handle(&mut self, event: &InputEvent, route: &Route);
    /// Revision of the content behind [`App::view`]. Must change whenever a
    /// mutation would change the tree.
    fn content_rev(&self) -> u64;
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
        }
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
    /// Public because focus and scrolling are driven from outside the engine —
    /// by the focus tree, by the scroll gesture handler, and by the driver's
    /// synthetic input — and none of those live here.
    pub fn state_mut(&mut self) -> &mut LayoutState {
        &mut self.state
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

        // Theme and scale are global inputs to every measurement, so a change
        // to either invalidates wholesale; per-node content revisions are
        // handled by the cache key itself.
        self.cache
            .retain_theme_and_scale(viewport.theme_rev, viewport.scale);
        self.state.content_rev = self.app.content_rev();

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
            "gorgon-petra-egui: {} of {} placements reached a painter",
            report.visited,
            report.placements
        );

        self.schedule(ctx, &frame);
        self.last_frame = Some(frame);
        self.last_report = Some(report);
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
            let route = match &self.last_frame {
                Some(frame) => route(frame, self.state.focused.as_deref(), event),
                None => Route::Unrouted {
                    reason: "no frame has been placed yet",
                },
            };
            self.app.handle(event, &route);
        }
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
    use super::{App, Host, default_presenter, petra_layer, refusal_view};
    use egui::{Context, Event, Key, Modifiers, RawInput};
    use gorgon_petra::input::{InputEvent, Route};
    use gorgon_petra::layout::RowSource;
    use gorgon_petra::tree::{Interaction, NodeKind, Props, Role, ViewNode};
    use std::ops::Range;

    #[derive(Default)]
    struct Demo {
        rev: u64,
        seen: Vec<(String, String)>,
        bad_tree: bool,
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
                        .interactive(Role::TextInput, "Filter fibers", &[Interaction::Key]),
                )
        }

        fn handle(&mut self, event: &InputEvent, route: &Route) {
            let kind = match event {
                InputEvent::Key { .. } => "key",
                InputEvent::Text(_) => "text",
                _ => "other",
            };
            let where_ = match route {
                Route::Pointer { node } | Route::Keyboard { node } => node.clone(),
                Route::Unrouted { reason } => format!("unrouted: {reason}"),
            };
            self.seen.push((kind.into(), where_));
        }

        fn content_rev(&self) -> u64 {
            self.rev
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
        assert_eq!(frame.placements.len(), 3);
        let report = host.report().expect("a report");
        assert!(report.is_complete(), "{report:?}");
        assert_eq!(report.texts, 2, "the title and the field's placeholder");
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

    /// The zero-idle contract on this side: with nothing moving, the host asks
    /// for no repaint at all.
    #[test]
    fn an_idle_pass_requests_no_repaint() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
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

    /// Once a frame exists, a keystroke routes to the focused node through the
    /// real router — the same path a driver's synthetic key will take.
    #[test]
    fn a_keystroke_routes_to_the_focused_node() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Demo::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        host.state_mut().focused = Some("/root/filter".into());

        let mut input = RawInput::default();
        input.events.push(Event::Key {
            key: Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        step(&ctx, &mut host, input);

        let seen = &host.app().seen;
        assert!(
            seen.iter().any(|(k, w)| k == "key" && w == "/root/filter"),
            "{seen:?}"
        );
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
