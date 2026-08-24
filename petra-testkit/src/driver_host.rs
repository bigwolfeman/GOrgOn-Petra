//! The application-side loop: one place that services driver jobs, runs a
//! pass, and publishes what the driver reads.
//!
//! `contracts/driver-protocol.md` says every action goes through the real
//! path — "synthesizes the same low-level input events a physical device
//! produces and injects them at the platform-input boundary". For Petra that
//! boundary is [`egui::RawInput`]: it is what winit hands egui, and
//! everything downstream of it (egui's own `InputState` construction,
//! [`gorgon_petra_egui::input::EventTranslator`], hit-testing, focus,
//! routing) runs identically whether the events came from a mouse or from
//! [`gorgon_petra_egui::inject::inject_action`]. So [`DriverHost::step`]
//! appends the injected events to the `RawInput` it is about to hand
//! [`egui::Context::run`] — not to an `InputState` mid-pass, which would
//! skip egui's own pointer and modifier bookkeeping and quietly diverge from
//! what a human produces.
//!
//! # The limitation this has, stated rather than hidden
//!
//! `eframe` owns the `RawInput` for a real OS window; egui 0.36 exposes no
//! way to push events into a running native integration (there is no
//! `Context::push_raw_input`). So a driver-controlled application steps
//! itself: it owns an [`egui::Context`], builds its own `RawInput`, and
//! calls [`DriverHost::step`] in a loop. That is the same thing
//! `egui_kittest` does, and it is faithful at the only boundary Petra owns —
//! what it does not exercise is winit, which is not Petra's code. Driving a
//! *windowed* eframe application from the driver is not solved by this wave
//! and no code here pretends otherwise.
//!
//! # Why capture is a trait
//!
//! Two reasons, neither of them scheduling. First, an application that never
//! screenshots should not link a GPU renderer — [`crate::snapshot`] pulls
//! `wgpu` in, and making the loop generic keeps that cost with the
//! applications that ask for it. Second, the two hosting shapes capture
//! differently: an offscreen headless run renders the tessellated primitives
//! itself, while a windowed one would ask the compositor
//! (`egui::ViewportCommand::Screenshot`). One loop, two backends, chosen by
//! the application.

use egui::{Context, FullOutput, RawInput};

use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_egui::inject::{InjectError, inject_action};

use crate::bridge::{Answer, Job, SettleState, Ticket, UiBridge};
use crate::server::FrameHub;
use crate::wire::{ErrorKind, WireError, WireRect};

/// How an application turns a painted pass into PNG bytes.
///
/// Implemented by [`crate::snapshot`] for headless offscreen rendering. The
/// `output` is taken by `&mut` because tessellation consumes the pass's
/// shapes and the font-atlas delta has to be applied before painting; an
/// implementation is free to take them.
pub trait Capture {
    /// Capture the frame this pass painted.
    ///
    /// `frame` is the [`PetrifiedFrame`] the pass petrified — the identity
    /// the answer must carry (FR-040), not whatever happens to be current
    /// when a later request arrives.
    ///
    /// # Errors
    /// Any refusal, in the protocol's own vocabulary, so the socket layer
    /// never has to invent an error kind.
    fn capture(
        &mut self,
        ctx: &Context,
        output: &mut FullOutput,
        frame: &PetrifiedFrame,
        region: Option<WireRect>,
    ) -> Result<Answer, WireError>;

    /// Take this pass's texture changes, whether or not anything is being
    /// captured.
    ///
    /// `egui::Context::load_texture` hands a texture's pixels over exactly
    /// once, in the `FullOutput` of the pass that loaded it, and
    /// `epaint::TextureManager` keeps no copy. So a backend that only ever
    /// looks at the passes it captures will find a hole where a host image
    /// should be. `DriverHost::step` calls this on every pass for that
    /// reason. The default does nothing, which is right for a backend that
    /// holds no texture state of its own.
    fn absorb(&mut self, _delta: &egui::TexturesDelta) {}
}

/// The shipped backend: real pixels, headless, identity-verified.
///
/// The `impl` lives here rather than in [`crate::snapshot`] because the trait
/// is this module's — and because writing it here is what keeps the
/// snapshotter usable on its own, by a test or a tool that wants a capture
/// without standing up a driver at all.
///
/// [`crate::snapshot::CaptureError`] already knows which wire kind it maps to
/// and why (`CaptureError::wire_kind`), so nothing is decided a second time
/// here; this only carries the message across.
impl Capture for crate::snapshot::Snapshotter {
    fn capture(
        &mut self,
        ctx: &Context,
        output: &mut FullOutput,
        frame: &PetrifiedFrame,
        region: Option<WireRect>,
    ) -> Result<Answer, WireError> {
        // `&*output`: the snapshotter borrows the pass and never drains it,
        // because a real window still has to apply the same `textures_delta`
        // to its own renderer afterwards.
        match crate::snapshot::Snapshotter::capture(self, ctx, &*output, frame, region) {
            Ok(shot) => Ok(Answer::Captured {
                seq: shot.seq,
                digest: shot.digest,
                png: shot.png,
                hosted: shot.hosted,
            }),
            Err(err) => Err(WireError::new(err.wire_kind(), err.to_string())),
        }
    }

    fn absorb(&mut self, delta: &egui::TexturesDelta) {
        crate::snapshot::Snapshotter::absorb(self, delta);
    }
}

/// A [`Host`], the driver's job queue, and the frame state the server reads —
/// stepped together so the three cannot disagree about which frame is
/// current.
pub struct DriverHost<A: App, C: Capture> {
    host: Host<A>,
    hub: FrameHub,
    bridge: UiBridge,
    capture: C,
    applied_frame_seq: u64,
}

impl<A: App, C: Capture> DriverHost<A, C> {
    /// Bind a host, a hub and a bridge into one loop.
    ///
    /// Attaches `ctx` to the bridge, so a job queued while the window is idle
    /// wakes it ([`UiBridge::attach`]); without that an application that
    /// paints no frames at idle — which is every Petra application, by
    /// design — would never notice the job.
    pub fn new(ctx: &Context, host: Host<A>, hub: FrameHub, bridge: UiBridge, capture: C) -> Self {
        bridge.attach(ctx);
        Self {
            host,
            hub,
            bridge,
            capture,
            applied_frame_seq: 0,
        }
    }

    /// The hosted application.
    pub fn app(&self) -> &A {
        self.host.app()
    }

    /// The hosted application, mutably.
    pub fn app_mut(&mut self) -> &mut A {
        self.host.app_mut()
    }

    /// The host, for the configuration `Host` owns (tokens, painters, fonts).
    pub fn host_mut(&mut self) -> &mut Host<A> {
        &mut self.host
    }

    /// The most recent petrified frame, or `None` before the first step.
    pub fn frame(&self) -> Option<&PetrifiedFrame> {
        self.host.frame()
    }

    /// Run one loop iteration: service every queued job, paint, publish.
    ///
    /// `raw` is the input this iteration would have had anyway — whatever the
    /// application's own event source produced. Injected events are appended
    /// to it, so a driver action and a real event arriving in the same
    /// iteration are ordered exactly as they were received rather than one
    /// class jumping the other.
    ///
    /// Returns egui's [`FullOutput`].
    ///
    /// # Panics
    /// Not here — but [`egui::TexturesDelta`] panics when dropped with
    /// unapplied deltas (`egui-0.36.1/src/data/output.rs:72`), so a caller
    /// that does not render the returned output must retire it with
    /// [`FullOutput::drop_without_applying_deltas`]. That is egui's contract,
    /// not this crate's, and it is repeated here because `step` is where a
    /// caller first meets it.
    pub fn step(&mut self, ctx: &Context, mut raw: RawInput) -> FullOutput {
        let mut acting: Vec<Ticket> = Vec::new();
        let mut capturing: Vec<(Ticket, Option<WireRect>)> = Vec::new();

        for ticket in self.bridge.take_pending() {
            match ticket.job().clone() {
                Job::Act { target, action } => {
                    // `inject_action` leaves `raw` untouched on error, so a
                    // refused action contributes no half-written gesture to
                    // the events the accepted ones are about to run with.
                    match inject_action(&mut self.host, &target, &action, &mut raw) {
                        Ok(()) => acting.push(ticket),
                        Err(err) => ticket.answer(Answer::Refused(inject_refusal(&err))),
                    }
                }
                Job::Capture { region } => capturing.push((ticket, region)),
            }
        }

        // `run_ui` is egui 0.36's only public entry point, and it hands the
        // closure a `Ui` — which `Host::pass` never uses, taking only
        // `ui.ctx()`. `host` is borrowed out of `self` first because the
        // closure is `FnMut` and would otherwise hold all of `self`.
        let host = &mut self.host;
        let mut output = ctx.run_ui(raw, |ui| {
            let inner = ui.ctx().clone();
            host.pass(&inner);
        });

        // `Host::pass` always petrifies, so a frame exists from the first
        // step onward. The `else` arm is not a fallback: it answers every
        // waiter honestly and returns, rather than unwrapping into a panic
        // that would take the UI thread down and hang every socket client.
        // Before anything else this pass: the delta is only offered once,
        // and a pass nobody captures still carries textures a later capture
        // needs.
        self.capture.absorb(&output.textures_delta);

        let Some(frame) = self.host.frame() else {
            let why = WireError::new(
                ErrorKind::Timeout,
                "the pass produced no frame; the host has not petrified anything yet",
            );
            for ticket in acting {
                ticket.answer(Answer::Refused(why.clone()));
            }
            for (ticket, _) in capturing {
                ticket.answer(Answer::Refused(why.clone()));
            }
            return output;
        };

        let seq = frame.seq;
        if !acting.is_empty() {
            self.applied_frame_seq = seq;
        }
        for ticket in acting {
            ticket.answer(Answer::Acted {
                applied_frame_seq: seq,
            });
        }
        for (ticket, region) in capturing {
            let answer = self
                .capture
                .capture(ctx, &mut output, frame, region)
                .unwrap_or_else(Answer::Refused);
            ticket.answer(answer);
        }

        let settle = SettleState {
            frame_seq: seq,
            applied_frame_seq: self.applied_frame_seq,
            running_transitions: frame.transitions.running,
            ambient_transitions: frame.transitions.ambient,
            // Read after the tickets above were answered and dropped, so a
            // job that arrived *during* this pass is counted and this frame
            // is honestly reported unsettled.
            queued_jobs: self.bridge.queued(),
            repaint_pending: repaint_pending(&output),
        };
        self.hub.publish_with(frame, settle);
        output
    }
}

/// Whether egui asked for another frame.
///
/// `ViewportOutput::repaint_delay` is `Duration::MAX` when nothing scheduled
/// one (`egui-0.36.1/src/viewport.rs:1278`); any finite delay — zero from
/// `request_repaint`, or a real interval from an animation — means a frame is
/// coming, which is exactly what "not settled yet" means.
fn repaint_pending(output: &FullOutput) -> bool {
    output
        .viewport_output
        .values()
        .any(|v| v.repaint_delay < std::time::Duration::MAX)
}

/// Map a refusal from the injector onto the protocol's closed error kinds.
///
/// Only `StaleNode` has a kind of its own in the contract. The other four are
/// all "you asked for something this frame cannot do" — a node that exists
/// but is not focusable, a focus move out of a modal trap, a point with
/// nothing focusable under it, a key with no synthesis — which is
/// `invalid-params` against the frame the request named. The `Display` text
/// is carried through verbatim so the message still names the node.
fn inject_refusal(err: &InjectError) -> WireError {
    let kind = match err {
        InjectError::StaleNode(_) => ErrorKind::StaleNode,
        InjectError::NotFocusable(_)
        | InjectError::OutsideActiveScope { .. }
        | InjectError::NoFocusableAt(_)
        | InjectError::UnsupportedKey(_) => ErrorKind::InvalidParams,
    };
    WireError::new(kind, err.to_string())
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::sync::Arc;

    use egui::{Pos2, RawInput};

    use gorgon_petra::component::button;
    use gorgon_petra::input::{InputEvent, Route};
    use gorgon_petra::layout::{ChangeSet, RowSource};
    use gorgon_petra::token::{Presenter, dark};
    use gorgon_petra::tree::{NodeKind, ViewNode};
    use gorgon_petra_egui::host::Host;
    use gorgon_petra_egui::inject::{Action, Target};

    use super::{Capture, Context, DriverHost, FullOutput, PetrifiedFrame};
    use crate::bridge::{Answer, Job, UiBridge};
    use crate::server::FrameHub;
    use crate::wire::{ErrorKind, WireError, WireRect};

    /// One real interactive node, and a record of every route the router
    /// actually delivered — the whole point of the "real path only" rule is
    /// that this record is written by the router, not by the injector.
    #[derive(Default)]
    struct DemoApp {
        routed: Vec<(String, String)>,
    }

    impl RowSource for DemoApp {
        fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
            Vec::new()
        }
    }

    impl gorgon_petra_egui::host::App for DemoApp {
        fn view(&mut self) -> ViewNode {
            ViewNode::new(NodeKind::Stack, "root").child(button("go", "Go"))
        }

        fn handle(&mut self, event: &InputEvent, route: &Route) {
            let where_to = match route {
                Route::Pointer { node } | Route::Keyboard { node } => node.clone(),
                Route::Unrouted { reason } => format!("unrouted: {reason}"),
            };
            self.routed.push((format!("{event:?}"), where_to));
        }

        fn take_changes(&mut self) -> ChangeSet {
            ChangeSet::All
        }
    }

    /// Records what it was asked for and answers with fixed bytes.
    ///
    /// A test double, and only ever reachable from `#[cfg(test)]`: the real
    /// backend is `crate::snapshot`, and the end-to-end proof that pixels
    /// come out of it lives with that module and with the socket test. What
    /// this double proves is the thing `DriverHost` alone is responsible
    /// for — that a `Capture` job reaches the backend with *this pass's*
    /// frame identity, not a later one.
    #[derive(Default)]
    struct RecordingCapture {
        seen: Vec<(u64, Option<WireRect>)>,
        refuse: bool,
    }

    impl Capture for RecordingCapture {
        fn capture(
            &mut self,
            _ctx: &Context,
            _output: &mut FullOutput,
            frame: &PetrifiedFrame,
            region: Option<WireRect>,
        ) -> Result<Answer, WireError> {
            self.seen.push((frame.seq, region));
            if self.refuse {
                return Err(WireError::new(
                    ErrorKind::InvalidParams,
                    "refused on purpose",
                ));
            }
            Ok(Answer::Captured {
                seq: frame.seq,
                digest: frame.digest.hex(),
                png: vec![0x89, b'P', b'N', b'G'],
                hosted: frame.hosted(),
            })
        }
    }

    const WINDOW: [f32; 2] = [400.0, 300.0];

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

    fn driver_host() -> (
        Context,
        FrameHub,
        UiBridge,
        DriverHost<DemoApp, RecordingCapture>,
    ) {
        let ctx = headless();
        let host = Host::new(&ctx, DemoApp::default(), Presenter::new(dark()));
        let hub = FrameHub::new();
        let bridge = UiBridge::new();
        let driver = DriverHost::new(
            &ctx,
            host,
            hub.clone(),
            bridge.clone(),
            RecordingCapture::default(),
        );
        (ctx, hub, bridge, driver)
    }

    fn step(ctx: &Context, driver: &mut DriverHost<DemoApp, RecordingCapture>) {
        driver
            .step(ctx, sized(RawInput::default()))
            .drop_without_applying_deltas();
    }

    /// The load-bearing one: an injected click is delivered by the *router*,
    /// to the node the driver named. Nothing in `DriverHost` writes this
    /// record — `Host::pass` does, from `route_with_surfaces`.
    #[test]
    fn an_injected_click_reaches_the_app_through_the_real_router() {
        let (ctx, _hub, bridge, mut driver) = driver_host();
        step(&ctx, &mut driver); // first frame: the button now has a placement

        let answer = submit_on_this_thread(
            &bridge,
            &ctx,
            &mut driver,
            Job::Act {
                target: Target::NodeId("/root/go".into()),
                action: Action::Click {
                    modifiers: gorgon_petra::Modifiers::default(),
                },
            },
        );
        match answer {
            Answer::Acted { applied_frame_seq } => assert!(applied_frame_seq >= 2),
            other => panic!("expected Acted, got {other:?}"),
        }

        let routed = &driver.app().routed;
        assert!(
            routed
                .iter()
                .any(|(event, node)| event.starts_with("PointerPressed") && node == "/root/go"),
            "no press was routed to /root/go; the router saw {routed:?}"
        );
        assert!(
            routed
                .iter()
                .any(|(event, node)| event.starts_with("PointerReleased") && node == "/root/go"),
            "no release was routed to /root/go; the router saw {routed:?}"
        );
    }

    #[test]
    fn a_stale_node_is_refused_as_stale_node_and_injects_nothing() {
        let (ctx, _hub, bridge, mut driver) = driver_host();
        step(&ctx, &mut driver);
        let before = driver.app().routed.len();

        let answer = submit_on_this_thread(
            &bridge,
            &ctx,
            &mut driver,
            Job::Act {
                target: Target::NodeId("/root/does-not-exist".into()),
                action: Action::Click {
                    modifiers: gorgon_petra::Modifiers::default(),
                },
            },
        );
        match answer {
            Answer::Refused(err) => {
                assert_eq!(err.kind, ErrorKind::StaleNode);
                assert!(err.message.contains("does-not-exist"), "{}", err.message);
            }
            other => panic!("expected Refused, got {other:?}"),
        }
        assert_eq!(
            driver.app().routed.len(),
            before,
            "a refused action still delivered events"
        );
    }

    #[test]
    fn a_capture_job_is_answered_with_this_passs_frame_identity() {
        let (ctx, hub, bridge, mut driver) = driver_host();
        step(&ctx, &mut driver);
        step(&ctx, &mut driver);
        let expected_seq = driver.frame().expect("a frame").seq + 1;

        let answer =
            submit_on_this_thread(&bridge, &ctx, &mut driver, Job::Capture { region: None });
        match answer {
            Answer::Captured {
                seq, digest, png, ..
            } => {
                assert_eq!(seq, expected_seq, "captured a frame other than this pass's");
                assert_eq!(digest, driver.frame().expect("a frame").digest.hex());
                assert_eq!(png, vec![0x89, b'P', b'N', b'G']);
            }
            other => panic!("expected Captured, got {other:?}"),
        }
        let published = hub.current().expect("published");
        assert_eq!(published.settle.frame_seq, expected_seq);
    }

    #[test]
    fn the_published_settle_state_reports_this_pass_not_a_guess() {
        let (ctx, hub, _bridge, mut driver) = driver_host();
        step(&ctx, &mut driver);
        let published = hub.current().expect("published");
        let frame = driver.frame().expect("a frame");
        assert_eq!(published.settle.frame_seq, frame.seq);
        assert_eq!(
            published.settle.running_transitions,
            frame.transitions.running
        );
        assert_eq!(
            published.settle.ambient_transitions,
            frame.transitions.ambient
        );
        assert_eq!(
            published.settle.queued_jobs, 0,
            "nothing was queued, so nothing may be reported queued"
        );
        assert_eq!(
            published.settle.applied_frame_seq, 0,
            "no action has been applied, and 0 is the contract's `never` value"
        );
    }

    #[test]
    fn applied_frame_seq_names_the_frame_that_consumed_the_action() {
        let (ctx, hub, bridge, mut driver) = driver_host();
        step(&ctx, &mut driver);
        submit_on_this_thread(
            &bridge,
            &ctx,
            &mut driver,
            Job::Act {
                target: Target::NodeId("/root/go".into()),
                action: Action::Hover,
            },
        );
        let published = hub.current().expect("published");
        assert_eq!(
            published.settle.applied_frame_seq, published.settle.frame_seq,
            "the frame that ran the action is the frame that must be reported"
        );
        // And it stays reported on later, action-free passes: a client that
        // asked "which frame applied my action" must still get an answer
        // after the UI has moved on.
        let applied = published.settle.applied_frame_seq;
        step(&ctx, &mut driver);
        let later = hub.current().expect("published");
        assert_eq!(later.settle.applied_frame_seq, applied);
        assert!(later.settle.frame_seq > applied);
    }

    #[test]
    fn a_backend_refusal_is_passed_through_in_the_protocols_vocabulary() {
        let (ctx, _hub, bridge, mut driver) = driver_host();
        step(&ctx, &mut driver);
        driver.capture.refuse = true;
        let answer = submit_on_this_thread(
            &bridge,
            &ctx,
            &mut driver,
            Job::Capture {
                region: Some(WireRect {
                    x: 0,
                    y: 0,
                    w: 10,
                    h: 10,
                }),
            },
        );
        match answer {
            Answer::Refused(err) => assert_eq!(err.kind, ErrorKind::InvalidParams),
            other => panic!("expected Refused, got {other:?}"),
        }
        assert_eq!(
            driver.capture.seen,
            vec![(
                2,
                Some(WireRect {
                    x: 0,
                    y: 0,
                    w: 10,
                    h: 10
                })
            )]
        );
    }

    /// Submit a job and run the one step that services it, on this thread.
    ///
    /// `UiBridge::submit` is async and the UI side is sync, so a normal
    /// application has them on different threads. A test does not need a
    /// second thread to prove the loop: it needs the submit to have happened
    /// *before* the step. This pushes the job with a runtime that makes no
    /// progress past the first await, runs the step, then drains the answer.
    fn submit_on_this_thread(
        bridge: &UiBridge,
        ctx: &Context,
        driver: &mut DriverHost<DemoApp, RecordingCapture>,
        job: Job,
    ) -> Answer {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a current-thread runtime");
        let pending = bridge.clone();
        let submit = runtime.spawn(async move { pending.submit(job).await });
        // One poll: enough to enqueue the job and park on the oneshot.
        runtime.block_on(async { tokio::task::yield_now().await });
        assert_eq!(bridge.queued(), 1, "the job did not reach the queue");
        step(ctx, driver);
        runtime.block_on(submit).expect("the submit task")
    }
}
