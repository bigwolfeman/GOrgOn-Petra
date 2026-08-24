//! Journey helpers with per-step evidence capture (FR-044, "Capture duty").
//!
//! A journey is an ordered list of steps. Each step, once run, is recorded
//! with real evidence of what happened — the frame seq/digest fetched from
//! the server immediately before and immediately after the step (and a
//! screenshot too, when the step asked for one) — never a formatted
//! restatement of the request that was sent. A failing step's record names
//! which step it was, what it was trying to do, and exactly what the driver
//! said, because that is the whole point of a journey: `screenshot` +
//! `tree` are the capture mechanism for UI PR review and agent
//! self-verification (`contracts/driver-protocol.md`, "Capture duty"), and a
//! journey is that mechanism strung across more than one step.

use serde_json::Value;

use gorgon_petra::tree::Interaction;

use super::client::Client;
use super::error::DriverError;
use super::node::TreeAnswer;
use super::node::TreeQuery;
use super::verbs::{ActResult, ActTarget, ScreenshotRegion, ScreenshotResult, WaitSettleResult};

/// What one journey step does.
#[derive(Debug, Clone)]
pub enum StepAction {
    /// Synthesize `kind` at `target` and wait for it to apply and settle.
    Act {
        /// The interaction kind.
        kind: Interaction,
        /// The target.
        target: ActTarget,
        /// Optional payload (text to type, key code, scroll delta, ...).
        payload: Option<Value>,
    },
    /// Block until the UI settles or `timeout_ms` elapses.
    WaitSettle {
        /// The deadline.
        timeout_ms: u64,
    },
    /// A read-only assertion step: query the tree and record what came
    /// back, without touching the UI.
    Query(TreeQuery),
}

/// One step of a journey, as authored.
#[derive(Debug, Clone)]
pub struct JourneyStep {
    /// What this step is trying to do, in words — carried into both the
    /// step's own record and any failure report.
    pub description: String,
    /// What to do.
    pub action: StepAction,
    /// Whether to capture a screenshot as part of this step's before/after
    /// evidence, on top of the frame seq/digest every step always captures.
    pub capture_screenshot: bool,
}

impl JourneyStep {
    /// An `act` step.
    #[must_use]
    pub fn act(
        description: impl Into<String>,
        kind: Interaction,
        target: ActTarget,
        payload: Option<Value>,
    ) -> Self {
        Self {
            description: description.into(),
            action: StepAction::Act {
                kind,
                target,
                payload,
            },
            capture_screenshot: false,
        }
    }

    /// A `wait_settle` step.
    #[must_use]
    pub fn wait_settle(description: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            description: description.into(),
            action: StepAction::WaitSettle { timeout_ms },
            capture_screenshot: false,
        }
    }

    /// A read-only `tree` query step.
    #[must_use]
    pub fn query(description: impl Into<String>, query: TreeQuery) -> Self {
        Self {
            description: description.into(),
            action: StepAction::Query(query),
            capture_screenshot: false,
        }
    }

    /// Ask this step to also capture a screenshot as before/after evidence.
    #[must_use]
    pub fn with_screenshot(mut self) -> Self {
        self.capture_screenshot = true;
        self
    }
}

/// Real evidence captured at one moment: a fresh `frame` call's seq/digest,
/// and a fresh `screenshot` call's result when the step asked for one.
/// Never derived from the request that was sent — both come from a real
/// round trip to the server at the moment the evidence was taken.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    /// The frame sequence at the moment this evidence was captured.
    pub frame_seq: u64,
    /// That frame's digest.
    pub digest: String,
    /// A screenshot, if this step asked for one.
    pub screenshot: Option<ScreenshotResult>,
}

/// What a step's action produced, on success.
#[derive(Debug, Clone)]
pub enum StepOutcome {
    /// An `act` step's result.
    Acted(ActResult),
    /// A `wait_settle` step's result.
    Settled(WaitSettleResult),
    /// A `Query` step's result.
    Queried(TreeAnswer),
}

/// One step's full record: what it was trying to do, the evidence taken
/// before and after, and what actually happened.
///
/// Not `Clone`: [`DriverError`] carries a `std::io::Error`, which is not
/// `Clone`, so neither is this — a record is read, not duplicated.
#[derive(Debug)]
pub struct StepRecord {
    /// This step's position in the journey (0-based).
    pub index: usize,
    /// What this step was trying to do.
    pub description: String,
    /// Evidence captured immediately before the step ran.
    pub before: Evidence,
    /// Evidence captured immediately after the step ran — captured even on
    /// failure, so a failed `act` still shows what the UI looked like
    /// afterward.
    pub after: Evidence,
    /// The step's own outcome.
    pub outcome: Result<StepOutcome, DriverError>,
}

impl StepRecord {
    /// A human-readable failure line: which step, what it expected, what the
    /// driver said. `None` if this step succeeded.
    #[must_use]
    pub fn failure_report(&self) -> Option<String> {
        let err = self.outcome.as_ref().err()?;
        Some(format!(
            "step {} ({:?}) failed: {err}",
            self.index, self.description
        ))
    }
}

/// The full record of a run journey. Not `Clone`, for the same reason
/// [`StepRecord`] is not.
#[derive(Debug, Default)]
pub struct JourneyReport {
    /// Every step's record, in order. Shorter than the authored step list
    /// when a step failed and the journey stopped there.
    pub steps: Vec<StepRecord>,
}

impl JourneyReport {
    /// Whether every recorded step succeeded.
    #[must_use]
    pub fn all_ok(&self) -> bool {
        self.steps.iter().all(|step| step.outcome.is_ok())
    }

    /// The first failed step's record, if any.
    #[must_use]
    pub fn first_failure(&self) -> Option<&StepRecord> {
        self.steps.iter().find(|step| step.outcome.is_err())
    }

    /// The first failure's report line, if any.
    #[must_use]
    pub fn failure_report(&self) -> Option<String> {
        self.first_failure().and_then(StepRecord::failure_report)
    }
}

impl Client {
    /// Run a journey: execute each step in order, capturing real
    /// before/after evidence around it, stopping at (and reporting) the
    /// first failure.
    ///
    /// A step fails either because its own action returned a
    /// [`DriverError`], or because capturing evidence around it did (the
    /// connection dropped mid-journey, say) — either way the returned
    /// report's last record names exactly which step and what the driver
    /// said, never a silently truncated journey.
    pub async fn run_journey(&self, steps: Vec<JourneyStep>) -> JourneyReport {
        let mut records = Vec::with_capacity(steps.len());
        for (index, step) in steps.into_iter().enumerate() {
            let before = match self.capture_evidence(step.capture_screenshot).await {
                Ok(evidence) => evidence,
                Err(err) => {
                    records.push(StepRecord {
                        index,
                        description: step.description,
                        before: Evidence::default(),
                        after: Evidence::default(),
                        outcome: Err(err),
                    });
                    break;
                }
            };

            let outcome = self.run_step_action(&step.action).await;
            let failed_before_after = outcome.is_err();

            let after = match self.capture_evidence(step.capture_screenshot).await {
                Ok(evidence) => evidence,
                Err(err) => {
                    // The step's own outcome (even if `Ok`) still matters to
                    // the caller, so it is kept; the after-evidence capture
                    // failure is reported as this record's outcome only when
                    // the step itself had succeeded — a step that already
                    // failed keeps its own, more specific, error.
                    let final_outcome = if failed_before_after {
                        outcome
                    } else {
                        Err(err)
                    };
                    let stop = final_outcome.is_err();
                    records.push(StepRecord {
                        index,
                        description: step.description,
                        before,
                        after: Evidence::default(),
                        outcome: final_outcome,
                    });
                    if stop {
                        break;
                    }
                    continue;
                }
            };

            let stop = outcome.is_err();
            records.push(StepRecord {
                index,
                description: step.description,
                before,
                after,
                outcome,
            });
            if stop {
                break;
            }
        }
        JourneyReport { steps: records }
    }

    async fn capture_evidence(&self, want_screenshot: bool) -> Result<Evidence, DriverError> {
        let frame = self.frame().await?;
        let screenshot = if want_screenshot {
            Some(self.screenshot(ScreenshotRegion::default()).await?)
        } else {
            None
        };
        Ok(Evidence {
            frame_seq: frame.seq,
            digest: frame.digest,
            screenshot,
        })
    }

    async fn run_step_action(&self, action: &StepAction) -> Result<StepOutcome, DriverError> {
        match action {
            StepAction::Act {
                kind,
                target,
                payload,
            } => self
                .act(*kind, target.clone(), payload.clone())
                .await
                .map(StepOutcome::Acted),
            StepAction::WaitSettle { timeout_ms } => self
                .wait_settle(*timeout_ms)
                .await
                .map(StepOutcome::Settled),
            StepAction::Query(query) => self.tree(query).await.map(StepOutcome::Queried),
        }
    }
}
