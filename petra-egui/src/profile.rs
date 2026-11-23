//! Where one [`crate::host::Host::pass`] spent its time.
//!
//! Off by default and free when off: no clock is read until a host installs
//! one with [`crate::host::Host::set_profile_clock`]. This crate owns no
//! clock for the same reason `gorgon-petra` owns none (wasm has no `std`
//! clock, and a driver steps its own), so the caller hands in a plain
//! function that answers seconds since any fixed origin.
//!
//! The split follows the pass in order. Every field is seconds, and the
//! named phases add up to [`PassProfile::total`] less whatever ran between
//! them unnamed (a few comparisons, never a walk).

use gorgon_petra::layout::PhaseProfile;

/// The wall-clock split of the last pass that built a frame.
///
/// A pass that painted the last picture with a new caret (a focus flight)
/// builds no frame and leaves the previous profile in place.
#[derive(Clone, Copy, Debug)]
pub struct PassProfile {
    /// From the top of the pass to the change set: input delivery, focus and
    /// pointer publication, the clock, the chord timeout.
    pub input: f64,
    /// [`crate::host::App::view`].
    pub view: f64,
    /// Contributions spliced into the application's tree.
    pub mount: f64,
    /// The new tree grafted onto the last one: what moved, and the shared
    /// allocations for what did not.
    pub diff: f64,
    /// Stage-2 acceptance of the mounted tree.
    pub validate: f64,
    /// The engine's own split, summed over every petrify this pass ran.
    pub petrify: PhaseProfile,
    /// How many times the pass petrified (a focus correction places the same
    /// tree again).
    pub petrifies: u32,
    /// Around the petrify calls: the surface-scope walks, the focus
    /// correction, and the memo kept for the next pass.
    pub around: f64,
    /// Transitions advanced against the placed frame.
    pub motion: f64,
    /// The frame painted into egui shapes.
    pub paint: f64,
    /// Everything after paint: focus and pointer reconciliation, the
    /// semantic projection, scheduling.
    pub after: f64,
    /// The whole pass.
    pub total: f64,
    /// Measure-cache hits this pass.
    pub measure_hits: u64,
    /// Measure-cache misses this pass: the measurements actually computed.
    pub measure_misses: u64,
    /// Placements carried over from the previous frame, in the last petrify.
    pub reused_nodes: usize,
    /// Placements the last petrify built itself.
    pub replaced_nodes: usize,
}

impl PassProfile {
    /// An empty profile reading `clock`.
    #[must_use]
    pub fn new(clock: fn() -> f64) -> Self {
        Self {
            input: 0.0,
            view: 0.0,
            mount: 0.0,
            diff: 0.0,
            validate: 0.0,
            petrify: PhaseProfile::new(clock),
            petrifies: 0,
            around: 0.0,
            motion: 0.0,
            paint: 0.0,
            after: 0.0,
            total: 0.0,
            measure_hits: 0,
            measure_misses: 0,
            reused_nodes: 0,
            replaced_nodes: 0,
        }
    }

    /// One line, milliseconds, for a log or a test's output.
    #[must_use]
    pub fn line(&self) -> String {
        let ms = |s: f64| s * 1000.0;
        format!(
            "total {:.2} ms | input {:.2} view {:.2} mount {:.2} diff {:.2} validate {:.2} \
             | petrify x{} {:.2} (docks {:.2} measure {:.2} place {:.2} resolve {:.2} digest {:.2}) \
             around {:.2} | motion {:.2} paint {:.2} after {:.2} | measured {} hit {} \
             | reused {} placed {}",
            ms(self.total),
            ms(self.input),
            ms(self.view),
            ms(self.mount),
            ms(self.diff),
            ms(self.validate),
            self.petrifies,
            ms(self.petrify.total()),
            ms(self.petrify.docks),
            ms(self.petrify.measure),
            ms(self.petrify.place),
            ms(self.petrify.resolve),
            ms(self.petrify.digest),
            ms(self.around),
            ms(self.motion),
            ms(self.paint),
            ms(self.after),
            self.measure_misses,
            self.measure_hits,
            self.reused_nodes,
            self.replaced_nodes,
        )
    }
}

/// A stopwatch over an optional clock: each [`Stopwatch::lap`] answers the
/// seconds since the last one, or `0.0` with no clock and no read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stopwatch {
    clock: Option<fn() -> f64>,
    start: f64,
    last: f64,
}

impl Stopwatch {
    pub(crate) fn start(clock: Option<fn() -> f64>) -> Self {
        let now = clock.map_or(0.0, |c| c());
        Self {
            clock,
            start: now,
            last: now,
        }
    }

    pub(crate) fn lap(&mut self) -> f64 {
        let Some(clock) = self.clock else {
            return 0.0;
        };
        let now = clock();
        let lap = now - self.last;
        self.last = now;
        lap
    }

    pub(crate) fn since_start(&self) -> f64 {
        self.clock.map_or(0.0, |c| c() - self.start)
    }
}
