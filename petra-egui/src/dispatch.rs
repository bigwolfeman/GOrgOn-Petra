//! The shell dispatcher: spec 010's *interpretation order*, wired
//! (`specs/010-input-routing/spec.md` §"The interpretation order", tasks
//! T046, T032, T054; D-105 named this leaf as the one gap between a
//! published binding and a dispatched command).
//!
//! The order is the load-bearing decision and this module is where it runs:
//!
//! 1. **The chord is in the reserved set** → [`Route::Reserved`], the shell
//!    handles it (FR-021). Settled inside
//!    [`gorgon_petra::input::route_with_surfaces`], before the frame is
//!    touched, so nothing here can shadow it.
//! 2. **`route` answered [`Route::Raw`]** → verbatim delivery to the raw
//!    claimant (FR-005, FR-020). The binding table is not consulted.
//! 3. **The chord's trigger matches a binding in a reachable scope** →
//!    [`InputDecision::Bound`], the command dispatched, the event consumed.
//! 4. **Otherwise the [`Route`] from that same `route` call** →
//!    [`InputDecision::Intent`] when a node on its path declared one,
//!    [`InputDecision::Unrouted`] with a stated reason otherwise.
//!
//! One `route` call, inspected twice (steps 2 and 4): there is deliberately
//! no "is a raw claim focused" query here — [`gorgon_petra::input`] owns
//! that decision (FR-005), and a second place to ask is one place to
//! forget.
//!
//! # FR-024: every decision is traceable
//!
//! [`InputDecision`] is the audit record: every interpreted event is one of
//! exactly `raw`, `bound`, `intent` or `unrouted` ([`InputDecision::class`]),
//! each with its stated reason ([`InputDecision::reason`]). `Route::Unrouted`
//! already carried its reason; this record extends the same principle to the
//! other three (T054). A reserved chord is classified `bound`: it is the
//! shell's own binding — the one chord no raw claim can receive — and it has
//! no [`Binding`] value because [`gorgon_petra::keymap::ReservedChords`] is
//! not a binding table (see [`InputDecision::Bound`]).
//!
//! An event that reached a node's own handling without a binding and without
//! a declared behaviour is `unrouted` with a reason that says so — the
//! spec's step 4 ("INTENT, if a node on its path declares one. Otherwise
//! Unrouted, with a stated reason") is literal, and the reason text keeps
//! the delivery visible so "the key did nothing" bugs cannot hide behind the
//! class word alone.
//!
//! # What this module does not do
//!
//! It does not route (that is [`gorgon_petra::input`]), does not own focus
//! (the host does, `contracts/interaction-state.md` §1), and does not know
//! where a dispatched command goes — [`App::decided`](crate::host::App::decided)
//! carries the decision to the application, which owns its own effects
//! (D-075). It holds no clock: [`Dispatcher::tick`] and [`Dispatcher::claim`]
//! take the host's `now` (the same `egui::InputState::time` `App::tick`
//! sees), so a driver can step time and two hosts cannot disagree about
//! when the pending-sequence timeout expired.

use std::fmt;
use std::time::Duration;

use gorgon_petra::frame::Placement;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::keymap::{
    Binding, Chord, ConflictError, DEFAULT_TIMEOUT, Resolution, Resolver, Scope,
};

use crate::host::FiredIntent;

/// FR-024's audit record: where one interpreted event went, and why.
///
/// Exactly four classes ([`InputDecision::class`] returns the word the
/// trace records): `raw`, `bound`, `intent`, `unrouted`. Every arm carries
/// its reason because a silently dropped keystroke is how "the key did
/// nothing" bugs hide.
#[derive(Clone, Debug, PartialEq)]
pub enum InputDecision {
    /// `raw`: verbatim delivery to the node whose raw claim caught the
    /// keystroke. The binding table was not consulted (FR-020).
    Raw {
        /// Canonical id of the raw claimant — what `route` named.
        node: String,
        /// Why this class.
        reason: String,
    },
    /// `bound`: the chord matched a binding, or the reserved set.
    ///
    /// `binding: None` is the reserved chord (FR-021): the shell's own
    /// handling, checked before everything else. It has no [`Binding`]
    /// because [`gorgon_petra::keymap::ReservedChords`] is not a binding
    /// table and inventing a synthetic binding for it would be mock data in
    /// a production type.
    Bound {
        /// The binding whose trigger matched, or `None` for the reserved set.
        binding: Option<Binding>,
        /// Why this class.
        reason: String,
    },
    /// `intent`: the routed event satisfied the [`gorgon_petra::tree::Behaviour`]
    /// a node declared (FR-014…FR-016).
    Intent {
        /// What fired.
        fired: FiredIntent,
        /// Why this class.
        reason: String,
    },
    /// `unrouted`: nothing claimed it — or a chord sequence is still in
    /// flight and this chord deliberately dispatched nothing yet (FR-013).
    Unrouted {
        /// Why, in one phrase. Most are the router's own
        /// `Route::Unrouted { reason }`, kept verbatim.
        reason: String,
        /// The chords typed so far of an in-flight sequence, empty when
        /// none is. This is FR-013's "observable by the shell, so the
        /// indicator is not guesswork" riding the record rather than a
        /// second query.
        pending: Vec<Chord>,
    },
}

impl InputDecision {
    /// The FR-024 class word the trace records: `raw`, `bound`, `intent`
    /// or `unrouted`.
    #[must_use]
    pub fn class(&self) -> &'static str {
        match self {
            Self::Raw { .. } => "raw",
            Self::Bound { .. } => "bound",
            Self::Intent { .. } => "intent",
            Self::Unrouted { .. } => "unrouted",
        }
    }

    /// Why this decision, in one phrase.
    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            Self::Raw { reason, .. }
            | Self::Bound { reason, .. }
            | Self::Intent { reason, .. }
            | Self::Unrouted { reason, .. } => reason,
        }
    }

    /// The in-flight chord sequence, if any (FR-013's indicator data).
    #[must_use]
    pub fn pending(&self) -> &[Chord] {
        match self {
            Self::Unrouted { pending, .. } => pending,
            _ => &[],
        }
    }
}

impl fmt::Display for InputDecision {
    /// One audit line: `"<class>: <reason>"`. SC-004's "the trace records
    /// 'raw' as the reason" is this line starting with `raw:`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class(), self.reason())
    }
}

/// One binding-table event the caller must not swallow (FR-010, FR-011).
///
/// [`Resolver::insert`] hands these back as values rather than logging them
/// — `gorgon-petra` has no logger and should not grow one — so the table
/// rebuild reports them here and the application writes its trace. A caller
/// that drops them has made a visible choice rather than inherited a
/// default.
#[derive(Clone, Debug, PartialEq)]
pub enum KeymapEvent {
    /// A binding conflicted with one already sitting on the same trigger in
    /// the same scope and was refused (FR-010). The error's text names the
    /// incoming binding, the sitting owner and the scope.
    Refused(Box<ConflictError>),
    /// Operator configuration displaced a plugin binding (FR-011). The
    /// displacement is reported rather than silent.
    Displaced(Binding),
}

impl fmt::Display for KeymapEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(err) => write!(f, "{err}"),
            Self::Displaced(binding) => {
                write!(f, "displaced: operator configuration took {binding}")
            }
        }
    }
}

/// What the first three steps of the interpretation order did with one
/// event — the fourth step's answer comes from
/// [`Dispatcher::classify_rest`] instead.
#[derive(Clone, Debug, PartialEq)]
pub enum Claim {
    /// Steps 1–2: the event is reserved or raw. The application still
    /// receives it through [`App::handle`](crate::host::App::handle) — the
    /// shell handles a reserved chord, a raw claimant gets its keystrokes
    /// verbatim — and the decision is recorded alongside.
    Verbatim(InputDecision),
    /// Step 3: the chord matched a binding, or is extending one. Consumed:
    /// nothing else sees it. A matched binding dispatches its command; a
    /// pending chord dispatches nothing yet.
    Consumed(InputDecision),
}

impl Claim {
    /// The decision inside this claim.
    #[must_use]
    pub fn decision(&self) -> &InputDecision {
        match self {
            Self::Verbatim(decision) | Self::Consumed(decision) => decision,
        }
    }
}

/// Every scope reachable from `focused` (FR-008a): [`Scope::Global`], plus
/// one [`Scope::Subtree`] per node on the focus path — the focused node and
/// each of its ancestors, walked through [`Placement::parent`].
///
/// The host computes this because the host owns focus; this function is here
/// because the walk is pure and the answer is this module's vocabulary. A
/// subtree binding whose root is not on the path simply finds no active
/// scope at [`Dispatcher::claim`] time and does not match — the mounted-but-
/// hidden case (FR-008a), not a fault.
///
/// A malformed parent chain (an index past the end, or a cycle — shapes
/// `gorgon_petra::frame`'s digest checks already refuse) stops the walk
/// rather than spinning or panicking.
#[must_use]
pub fn active_scopes(placements: &[Placement], focused: Option<&str>) -> Vec<Scope> {
    let mut scopes = vec![Scope::Global];
    let Some(focused) = focused else {
        return scopes;
    };
    let Some(mut index) = placements.iter().position(|p| p.id == focused) else {
        return scopes;
    };
    for _ in 0..placements.len() {
        let placement = &placements[index];
        scopes.push(Scope::Subtree {
            root: placement.id.clone(),
        });
        match placement.parent {
            Some(parent) if parent < placements.len() => index = parent,
            _ => return scopes,
        }
    }
    scopes
}

/// The binding table and the one in-flight chord sequence (spec 010
/// US-2, US-8, FR-007…FR-013a).
///
/// Holds [`gorgon_petra::keymap::Resolver`] — the trie — and keeps it in
/// step with the frame the host routes against. [`Dispatcher::sync`] is the
/// only way bindings enter, and it is idempotent: it rebuilds when and only
/// when the frame's bindings or the operator's changed, so a sequence in
/// flight survives an unrelated frame (a rebuild would clear the resolver's
/// pending state, which is exactly the bug this rule prevents).
pub struct Dispatcher {
    resolver: Resolver,
    /// Operator configuration (FR-011), inserted after the frame's bindings
    /// so an [`gorgon_petra::keymap::Owner::Operator`] binding displaces a
    /// plugin's rather than being refused by it.
    operator: Vec<Binding>,
    /// What the resolver currently holds, in insertion order, for the
    /// change check in [`Dispatcher::sync`].
    snapshot: Vec<Binding>,
    /// `now` of the last chord that reached the table, for the pending
    /// timeout (FR-013). The host's clock units (seconds).
    last_chord: Option<f64>,
}

impl Dispatcher {
    /// An empty table and [`DEFAULT_TIMEOUT`] (spec 010 decision 010-4:
    /// 1500 ms — long enough for a deliberate two-chord trigger, short
    /// enough that a mistaken first chord is not left hanging).
    #[must_use]
    pub fn new() -> Self {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// An empty table and an explicit pending-sequence timeout.
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            resolver: Resolver::with_timeout(timeout),
            operator: Vec::new(),
            snapshot: Vec::new(),
            last_chord: None,
        }
    }

    /// This dispatcher's pending-sequence timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.resolver.timeout()
    }

    /// The chords typed so far of an in-flight sequence (FR-013), empty
    /// when none is.
    #[must_use]
    pub fn pending(&self) -> &[Chord] {
        self.resolver.pending()
    }

    /// Install operator configuration (FR-011: it outranks any plugin
    /// binding, and the displacement it causes is reported by the next
    /// [`Dispatcher::sync`], never silent).
    ///
    /// [`gorgon_petra::keymap::Owner`] is read off each binding, so a
    /// mislabelled operator entry loses to a plugin's exactly like a
    /// plugin's would — the label is the authority, not this method.
    pub fn set_operator_bindings(&mut self, bindings: Vec<Binding>) {
        self.operator = bindings;
    }

    /// Bring the table in step with the latest frame's placements, and
    /// report every conflict refusal (FR-010) and displacement (FR-011) the
    /// rebuild caused. The host passes `&frame.placements`; this module
    /// reads nothing else off the frame.
    ///
    /// Bindings ride on placements
    /// ([`gorgon_petra::frame::PlacementSemantics::bindings`]); a binding
    /// declared on several nodes of one tree — a compound that publishes its
    /// chords on both its open and its closed root — is one table entry, so
    /// identical bindings are deduplicated before insertion and only
    /// genuinely different owners collide.
    ///
    /// Insertion order is frame order, then operator configuration. FR-008a
    /// needs no check here: a subtree binding whose root is absent simply
    /// finds no active scope at [`Dispatcher::claim`] time and does not
    /// match — the normal case for a mounted-but-hidden contribution, not a
    /// fault.
    pub fn sync(&mut self, placements: &[Placement]) -> Vec<KeymapEvent> {
        let mut table: Vec<Binding> = Vec::new();
        for placement in placements {
            for binding in &placement.semantics.bindings {
                if !table.contains(binding) {
                    table.push(binding.clone());
                }
            }
        }
        for binding in &self.operator {
            if !table.contains(binding) {
                table.push(binding.clone());
            }
        }
        if table == self.snapshot {
            // Unchanged: the resolver keeps its pending sequence. This is
            // the whole reason sync is change-checked rather than eager.
            return Vec::new();
        }
        let mut events = Vec::new();
        self.resolver = Resolver::with_timeout(self.resolver.timeout());
        self.snapshot = table.clone();
        for binding in table {
            match self.resolver.insert(binding) {
                Ok(Some(displaced)) => events.push(KeymapEvent::Displaced(displaced)),
                Ok(None) => {}
                Err(conflict) => events.push(KeymapEvent::Refused(conflict)),
            }
        }
        events
    }

    /// Abandon an in-flight sequence whose timeout has passed (FR-013:
    /// "the timeout clears it with nothing dispatched"), and report whether
    /// it did.
    ///
    /// `now` is the host's clock in seconds. A caller that paints a pending
    /// indicator asks for a repaint when this returns `true`.
    pub fn tick(&mut self, now: f64) -> bool {
        let Some(last) = self.last_chord else {
            return false;
        };
        // `max(0.0)` because `now` is the host's clock and a driver may
        // step it backwards; `Duration::from_secs_f64` panics on a negative.
        let elapsed = Duration::from_secs_f64((now - last).max(0.0));
        self.resolver.abandon_if_timed_out(elapsed)
    }

    /// Steps 1–3 of the interpretation order for one event.
    ///
    /// `gesture_ended` says the router ended a gesture on this event — the
    /// Escape that cancels a capture (`contracts/interaction-state.md` §7)
    /// is swallowed by the pointer state and must not *also* resolve as a
    /// chord, or one keystroke would undo the gesture and run a command.
    ///
    /// `active` is every scope currently reachable (FR-008a): always
    /// [`Scope::Global`] for a table that only holds admitted bindings —
    /// FR-012 is enforced where bindings *enter*, at contribution
    /// acceptance — plus one [`Scope::Subtree`] per placement on the focus
    /// path. The caller computes it; this module does not know the frame.
    ///
    /// `None` means step 4 decides: pass the route and any fired behaviour
    /// to [`Dispatcher::classify_rest`].
    pub fn claim(
        &mut self,
        event: &InputEvent,
        route: &Route,
        gesture_ended: bool,
        active: &[Scope],
        now: f64,
    ) -> Option<Claim> {
        self.tick(now);
        match route {
            // Step 1. The shell handles it — including the focus return
            // (US-6), which is the host's to perform, not this module's.
            Route::Reserved { chord } => {
                return Some(Claim::Verbatim(InputDecision::Bound {
                    binding: None,
                    reason: format!(
                        "reserved chord {chord} matched the reserved set (FR-021); the shell \
                         handles it and no raw claim or binding table sees it"
                    ),
                }));
            }
            // Step 2. `route` is the only place that decides this
            // (FR-005); the binding table is not consulted (FR-020).
            Route::Raw { node } => {
                return Some(Claim::Verbatim(InputDecision::Raw {
                    node: node.clone(),
                    reason: format!(
                        "raw: focus sits inside the raw claimant {node} (FR-019); the event is \
                         delivered verbatim and the binding table is not consulted (FR-020)"
                    ),
                }));
            }
            _ => {}
        }
        let InputEvent::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            return None;
        };
        if gesture_ended {
            // The pointer state swallowed this keystroke as a gesture
            // cancel; step 4 will record it as unrouted with that reason.
            return None;
        }
        // Step 3. A release is not a chord; only a press resolves
        // (`reserved_or_raw` draws the same line).
        let chord = Chord {
            key: *key,
            modifiers: *modifiers,
        };
        self.last_chord = Some(now);
        match self.resolver.resolve(chord, active) {
            Resolution::Matched(binding) => Some(Claim::Consumed(InputDecision::Bound {
                reason: format!(
                    "trigger {chord} matched {} in {} (owner {})",
                    binding.command,
                    binding.scope.describe(),
                    binding.owner
                ),
                binding: Some(binding),
            })),
            // A chord that extends a trigger dispatches nothing yet — but
            // it is consumed all the same: it must not fall through to the
            // node's own handling while the sequence is still open.
            Resolution::Pending => Some(Claim::Consumed(InputDecision::Unrouted {
                reason: format!(
                    "chord {chord} extends a pending sequence; nothing dispatched, and the \
                     sequence clears after {:?} with nothing dispatched (FR-013)",
                    self.resolver.timeout()
                ),
                pending: self.resolver.pending().to_vec(),
            })),
            // Begins no trigger: report immediately (T033), and step 4
            // decides what the route does with it.
            Resolution::NoMatch => None,
        }
    }

    /// Step 4: classify what the route from the *same* `route` call did,
    /// given whether a declared behaviour fired on the way past.
    ///
    /// `fired` is `Some` exactly when [`App::intent`](crate::host::App::intent)
    /// was called for this event; the decision is the record of that same
    /// fact (FR-024), not a second interpretation.
    pub fn classify_rest(&mut self, route: &Route, fired: Option<FiredIntent>) -> InputDecision {
        let pending = self.resolver.pending().to_vec();
        if let Some(fired) = fired {
            return InputDecision::Intent {
                reason: format!(
                    "node {} declared {:?}/{:?} and this event satisfied it (FR-014)",
                    fired.node, fired.intent, fired.phase
                ),
                fired,
            };
        }
        match route {
            Route::Unrouted { reason } => InputDecision::Unrouted {
                reason: reason.to_string(),
                pending,
            },
            Route::Pointer { node } | Route::Keyboard { node } => InputDecision::Unrouted {
                reason: format!(
                    "delivered to {node} for its own handling; no binding matched and the node \
                     declares no intent for this event"
                ),
                pending,
            },
            // Reached only if a caller runs classify_rest on an event
            // `claim` already answered. Answered here rather than with a
            // panic: an audit record that lies is worse than one that is
            // redundant.
            Route::Raw { node } => InputDecision::Raw {
                node: node.clone(),
                reason: format!(
                    "raw: focus sits inside the raw claimant {node} (FR-019); the event is \
                     delivered verbatim and the binding table is not consulted (FR-020)"
                ),
            },
            Route::Reserved { chord } => InputDecision::Bound {
                binding: None,
                reason: format!(
                    "reserved chord {chord} matched the reserved set (FR-021); the shell \
                     handles it and no raw claim or binding table sees it"
                ),
            },
        }
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gorgon_petra::frame::PlacementSemantics;
    use gorgon_petra::geom::Rect;
    use gorgon_petra::input::{KeyCode, Modifiers};
    use gorgon_petra::keymap::{CommandName, Owner};
    use gorgon_petra::tree::{Interaction, NodeKind};

    fn ctrl(_key: KeyCode) -> Modifiers {
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        }
    }

    fn chord(key: KeyCode, modifiers: Modifiers) -> Chord {
        Chord { key, modifiers }
    }

    fn key_press(key: KeyCode, modifiers: Modifiers) -> InputEvent {
        InputEvent::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn binding(trigger: Vec<Chord>, name: &str, scope: Scope, owner: Owner) -> Binding {
        Binding::new(trigger, CommandName::builtin(name), scope, owner)
            .expect("a well-formed test binding")
    }

    /// One placement carrying `id`, `parent`, and the bindings a node
    /// declares — the direct-construction pattern `frame/placement.rs`'s own
    /// tests use; running layout to build a one-node frame would test
    /// layout.
    fn placement(
        id: &str,
        parent: Option<usize>,
        bindings: Vec<Binding>,
    ) -> gorgon_petra::frame::Placement {
        gorgon_petra::frame::Placement {
            id: id.to_owned(),
            kind: NodeKind::Stack,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            z: 0,
            clip: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            opacity: 1.0,
            paint: gorgon_petra::frame::PaintState::default(),
            semantics: PlacementSemantics {
                actions: [Interaction::Key, Interaction::Focus].into_iter().collect(),
                bindings,
                ..PlacementSemantics::default()
            },
            parent,
        }
    }

    fn frame(placements: Vec<Placement>) -> Vec<Placement> {
        placements
    }

    fn global(name: &str, trigger: Vec<Chord>) -> Binding {
        binding(
            trigger,
            name,
            Scope::Global,
            Owner::Plugin("demo".to_owned()),
        )
    }

    #[test]
    fn a_matched_chord_is_consumed_and_names_the_command() {
        let mut dispatcher = Dispatcher::new();
        let frame = frame(vec![placement(
            "root",
            None,
            vec![global(
                "save",
                vec![chord(KeyCode::Char('s'), ctrl(KeyCode::Char('s')))],
            )],
        )]);
        assert!(
            dispatcher.sync(&frame).is_empty(),
            "a clean table reports nothing"
        );

        let event = key_press(KeyCode::Char('s'), ctrl(KeyCode::Char('s')));
        let claim = dispatcher
            .claim(
                &event,
                &Route::Keyboard {
                    node: "root".to_owned(),
                },
                false,
                &[Scope::Global],
                0.0,
            )
            .expect("a chord on a sitting trigger resolves");

        let decision = claim.decision();
        assert_eq!(decision.class(), "bound");
        assert!(
            matches!(&claim, Claim::Consumed(_)),
            "bound consumes the event"
        );
        assert!(
            decision.reason().contains("save"),
            "the reason names the command: {}",
            decision.reason()
        );
    }

    #[test]
    fn a_chord_dispatches_with_focus_inside_the_scope_and_not_outside() {
        // US-2 / T039: reachable scopes decide, per FR-008a.
        let mut dispatcher = Dispatcher::new();
        let binding = binding(
            vec![chord(KeyCode::Char('j'), Modifiers::NONE)],
            "down",
            Scope::Subtree {
                root: "panel".to_owned(),
            },
            Owner::Plugin("vim-motions".to_owned()),
        );
        let frame = frame(vec![placement("panel", None, vec![binding])]);
        dispatcher.sync(&frame);

        let event = key_press(KeyCode::Char('j'), Modifiers::NONE);
        let outside = Scope::Global;
        assert!(
            dispatcher
                .claim(
                    &event,
                    &Route::Keyboard {
                        node: "root".into()
                    },
                    false,
                    &[outside],
                    0.0
                )
                .is_none(),
            "focus outside the subtree: no match, step 4 decides"
        );

        let inside = Scope::Subtree {
            root: "panel".to_owned(),
        };
        let claim = dispatcher
            .claim(
                &event,
                &Route::Keyboard {
                    node: "panel".into(),
                },
                false,
                &[inside],
                0.0,
            )
            .expect("focus inside the subtree: the chord dispatches");
        assert_eq!(claim.decision().class(), "bound");
    }

    #[test]
    fn a_raw_route_outranks_a_sitting_binding() {
        // US-5 / T050: the binding table is not consulted on the raw path.
        let mut dispatcher = Dispatcher::new();
        let frame = frame(vec![placement(
            "root",
            None,
            vec![global(
                "save",
                vec![chord(KeyCode::Char('s'), ctrl(KeyCode::Char('s')))],
            )],
        )]);
        dispatcher.sync(&frame);

        let event = key_press(KeyCode::Char('s'), ctrl(KeyCode::Char('s')));
        let claim = dispatcher
            .claim(
                &event,
                &Route::Raw {
                    node: "terminal".to_owned(),
                },
                false,
                &[Scope::Global],
                0.0,
            )
            .expect("the raw claim answers before the table");

        assert_eq!(claim.decision().class(), "raw");
        assert!(
            matches!(&claim, Claim::Verbatim(_)),
            "raw is delivered verbatim"
        );
        let InputDecision::Raw { node, reason } = claim.decision() else {
            panic!("class said raw: {:?}", claim.decision());
        };
        assert_eq!(node, "terminal");
        assert!(reason.contains("raw"), "SC-004: the reason records 'raw'");
    }

    #[test]
    fn the_reserved_chord_outranks_a_raw_claim() {
        // US-6 / T051's routing half: the reserved chord is settled before
        // the raw check, so a raw claim can never receive it.
        let mut dispatcher = Dispatcher::new();
        let event = key_press(
            KeyCode::Escape,
            Modifiers {
                shift: true,
                ..Modifiers::NONE
            },
        );
        let claim = dispatcher
            .claim(
                &event,
                &Route::Reserved {
                    chord: Chord::reserved_escape(),
                },
                false,
                &[Scope::Global],
                0.0,
            )
            .expect("the reserved chord answers before everything");

        assert_eq!(claim.decision().class(), "bound");
        let InputDecision::Bound { binding, .. } = claim.decision() else {
            panic!("class said bound: {:?}", claim.decision());
        };
        assert!(binding.is_none(), "the reserved chord has no Binding value");
    }

    #[test]
    fn a_pending_sequence_is_observable_and_times_out_with_nothing_dispatched() {
        // US-8 / T032 / T042: pending state readable, timeout stated,
        // cleared with nothing dispatched.
        let mut dispatcher = Dispatcher::with_timeout(Duration::from_millis(500));
        let frame = frame(vec![placement(
            "root",
            None,
            vec![global(
                "save-as",
                vec![
                    chord(KeyCode::Char('k'), ctrl(KeyCode::Char('k'))),
                    chord(KeyCode::Char('s'), ctrl(KeyCode::Char('s'))),
                ],
            )],
        )]);
        dispatcher.sync(&frame);

        let first = key_press(KeyCode::Char('k'), ctrl(KeyCode::Char('k')));
        let claim = dispatcher
            .claim(
                &first,
                &Route::Keyboard {
                    node: "root".into(),
                },
                false,
                &[Scope::Global],
                10.0,
            )
            .expect("a prefix chord resolves as pending");

        assert_eq!(claim.decision().class(), "unrouted");
        assert!(
            !claim.decision().pending().is_empty(),
            "the record carries the sequence, so the indicator is not guesswork"
        );
        assert_eq!(dispatcher.pending().len(), 1, "and the table says the same");

        assert!(
            !dispatcher.tick(10.2),
            "before the timeout the sequence stands"
        );
        assert!(dispatcher.tick(10.6), "past the timeout it clears");
        assert!(
            dispatcher.pending().is_empty(),
            "cleared with nothing dispatched"
        );
    }

    #[test]
    fn a_chord_beginning_no_trigger_reaches_step_4_immediately() {
        // T033: no waiting out the timeout.
        let mut dispatcher = Dispatcher::new();
        let frame = frame(vec![placement(
            "root",
            None,
            vec![global(
                "save",
                vec![chord(KeyCode::Char('s'), ctrl(KeyCode::Char('s')))],
            )],
        )]);
        dispatcher.sync(&frame);

        let event = key_press(KeyCode::Function(5), Modifiers::NONE);
        assert!(
            dispatcher
                .claim(
                    &event,
                    &Route::Keyboard {
                        node: "root".into()
                    },
                    false,
                    &[Scope::Global],
                    0.0
                )
                .is_none(),
            "NoMatch returns None at once — step 4 decides"
        );
        assert!(
            dispatcher.pending().is_empty(),
            "and leaves no pending state"
        );
    }

    #[test]
    fn a_release_never_resolves_as_a_chord() {
        let mut dispatcher = Dispatcher::new();
        let frame = frame(vec![placement(
            "root",
            None,
            vec![global(
                "save",
                vec![chord(KeyCode::Char('s'), ctrl(KeyCode::Char('s')))],
            )],
        )]);
        dispatcher.sync(&frame);

        let event = InputEvent::Key {
            key: KeyCode::Char('s'),
            pressed: false,
            repeat: false,
            modifiers: ctrl(KeyCode::Char('s')),
        };
        assert!(
            dispatcher
                .claim(
                    &event,
                    &Route::Keyboard {
                        node: "root".into()
                    },
                    false,
                    &[Scope::Global],
                    0.0
                )
                .is_none(),
            "a release is not a chord"
        );
    }

    #[test]
    fn a_key_that_ended_a_gesture_never_also_resolves_as_a_chord() {
        // One keystroke must not both cancel a capture (the router's
        // swallow) and dispatch a command.
        let mut dispatcher = Dispatcher::new();
        let frame = frame(vec![placement(
            "root",
            None,
            vec![global(
                "escape",
                vec![chord(KeyCode::Escape, Modifiers::NONE)],
            )],
        )]);
        dispatcher.sync(&frame);

        let event = key_press(KeyCode::Escape, Modifiers::NONE);
        assert!(
            dispatcher
                .claim(
                    &event,
                    &Route::Unrouted {
                        reason: "escape cancelled the gesture holding the pointer".into()
                    },
                    true,
                    &[Scope::Global],
                    0.0
                )
                .is_none(),
            "a swallowed keystroke reaches no binding table"
        );
    }

    #[test]
    fn sync_is_idempotent_and_keeps_a_pending_sequence_alive() {
        let mut dispatcher = Dispatcher::new();
        let frame = frame(vec![placement(
            "root",
            None,
            vec![global(
                "save-as",
                vec![
                    chord(KeyCode::Char('k'), ctrl(KeyCode::Char('k'))),
                    chord(KeyCode::Char('s'), ctrl(KeyCode::Char('s'))),
                ],
            )],
        )]);
        dispatcher.sync(&frame);
        let first = key_press(KeyCode::Char('k'), ctrl(KeyCode::Char('k')));
        dispatcher.claim(
            &first,
            &Route::Keyboard {
                node: "root".into(),
            },
            false,
            &[Scope::Global],
            0.0,
        );

        // The same frame again: no rebuild, so the sequence is still open.
        assert!(dispatcher.sync(&frame).is_empty());
        assert_eq!(dispatcher.pending().len(), 1);

        // The second chord resolves it — which only works if the trie and
        // the pending prefix both survived the frame.
        let second = key_press(KeyCode::Char('s'), ctrl(KeyCode::Char('s')));
        let claim = dispatcher
            .claim(
                &second,
                &Route::Keyboard {
                    node: "root".into(),
                },
                false,
                &[Scope::Global],
                0.1,
            )
            .expect("the sequence completes");
        assert_eq!(claim.decision().class(), "bound");
    }

    #[test]
    fn identical_bindings_on_two_nodes_are_one_table_entry() {
        // A compound publishes its chords on both its open and its closed
        // root (petra-compound's Command palette): one binding, one entry.
        let mut dispatcher = Dispatcher::new();
        let twice = global(
            "toggle",
            vec![chord(KeyCode::Char('k'), ctrl(KeyCode::Char('k')))],
        );
        let frame = frame(vec![
            placement("open", None, vec![twice.clone()]),
            placement("closed", Some(0), vec![twice]),
        ]);
        assert!(
            dispatcher.sync(&frame).is_empty(),
            "no conflict: the same binding twice is one entry"
        );
    }

    #[test]
    fn a_conflicting_binding_is_refused_and_the_refusal_names_both_owners() {
        // FR-010 / SC-002's table half: refused, named, reported.
        let mut dispatcher = Dispatcher::new();
        let first = binding(
            vec![chord(KeyCode::Char('g'), ctrl(KeyCode::Char('g')))],
            "status",
            Scope::Global,
            Owner::Plugin("git".to_owned()),
        );
        let second = binding(
            vec![chord(KeyCode::Char('g'), ctrl(KeyCode::Char('g')))],
            "grep",
            Scope::Global,
            Owner::Plugin("files".to_owned()),
        );
        let frame = frame(vec![
            placement("a", None, vec![first]),
            placement("b", Some(0), vec![second]),
        ]);

        let events = dispatcher.sync(&frame);
        assert_eq!(events.len(), 1, "one refusal, reported");
        let KeymapEvent::Refused(err) = &events[0] else {
            panic!("expected a refusal, got {events:?}");
        };
        let text = err.to_string();
        assert!(text.contains("git"), "names the sitting owner: {text}");
        assert!(text.contains("files"), "names the incoming owner: {text}");
    }

    #[test]
    fn operator_configuration_displaces_a_plugin_binding_and_says_so() {
        // FR-011: outranks, and the displacement is reported.
        let mut dispatcher = Dispatcher::new();
        dispatcher.set_operator_bindings(vec![binding(
            vec![chord(KeyCode::Char('k'), ctrl(KeyCode::Char('k')))],
            "palette",
            Scope::Global,
            Owner::Operator,
        )]);
        let frame = frame(vec![placement(
            "panel",
            None,
            vec![binding(
                vec![chord(KeyCode::Char('k'), ctrl(KeyCode::Char('k')))],
                "plugin-palette",
                Scope::Global,
                Owner::Plugin("demo".to_owned()),
            )],
        )]);

        let events = dispatcher.sync(&frame);
        assert_eq!(events.len(), 1, "one displacement, reported");
        assert!(matches!(&events[0], KeymapEvent::Displaced(_)));
        assert!(events[0].to_string().contains("displaced"));
    }

    #[test]
    fn classify_rest_says_intent_when_a_behaviour_fired() {
        let mut dispatcher = Dispatcher::new();
        let fired = FiredIntent {
            node: "btn".to_owned(),
            intent: gorgon_petra::tree::Intent::Activate,
            phase: gorgon_petra::tree::Phase::OnRelease,
            value: None,
            weights: None,
        };
        let decision = dispatcher.classify_rest(
            &Route::Pointer {
                node: "btn".to_owned(),
            },
            Some(fired),
        );
        assert_eq!(decision.class(), "intent");
        assert!(decision.reason().contains("btn"));
    }

    #[test]
    fn classify_rest_records_the_routers_own_reason_for_an_unrouted_event() {
        let mut dispatcher = Dispatcher::new();
        let decision = dispatcher.classify_rest(
            &Route::Unrouted {
                reason: "no frame has been placed yet".into(),
            },
            None,
        );
        assert_eq!(decision.class(), "unrouted");
        assert_eq!(decision.reason(), "no frame has been placed yet");
        assert_eq!(
            decision.to_string(),
            "unrouted: no frame has been placed yet",
            "SC-004's audit line shape"
        );
    }

    #[test]
    fn a_delivered_event_without_a_behaviour_is_unrouted_with_that_reason() {
        // Spec 010's step 4 is literal: no declared intent, and the reason
        // must keep the delivery visible.
        let mut dispatcher = Dispatcher::new();
        let decision = dispatcher.classify_rest(
            &Route::Keyboard {
                node: "field".to_owned(),
            },
            None,
        );
        assert_eq!(decision.class(), "unrouted");
        assert!(decision.reason().contains("field"), "{}", decision.reason());
        assert!(
            decision.reason().contains("delivered"),
            "{}",
            decision.reason()
        );
    }

    #[test]
    fn active_scopes_are_global_plus_the_whole_focus_path() {
        let placements = vec![
            placement("root", None, vec![]),
            placement("panel", Some(0), vec![]),
            placement("field", Some(1), vec![]),
        ];
        assert_eq!(
            active_scopes(&placements, Some("field")),
            vec![
                Scope::Global,
                Scope::Subtree {
                    root: "field".to_owned()
                },
                Scope::Subtree {
                    root: "panel".to_owned()
                },
                Scope::Subtree {
                    root: "root".to_owned()
                },
            ],
            "the focused node and every ancestor are reachable; nothing else"
        );
        assert_eq!(active_scopes(&placements, None), vec![Scope::Global]);
        assert_eq!(
            active_scopes(&placements, Some("absent")),
            vec![Scope::Global],
            "a focused id the frame does not hold leaves only global"
        );
    }

    #[test]
    fn active_scopes_stops_on_a_malformed_parent_chain_without_hanging() {
        // A cycle and an out-of-range parent, the two shapes
        // `gorgon_petra::frame`'s digest checks already refuse: the walk
        // stops instead of spinning, and stops short instead of panicking.
        let cyclic = vec![
            placement("root", Some(0), vec![]),
            placement("field", Some(0), vec![]),
        ];
        assert_eq!(
            active_scopes(&cyclic, Some("field")).len(),
            3,
            "field, root, then the cycle stops the walk"
        );
        let dangling = vec![placement("root", Some(9), vec![])];
        assert_eq!(
            active_scopes(&dangling, Some("root")),
            vec![
                Scope::Global,
                Scope::Subtree {
                    root: "root".to_owned()
                },
            ],
            "a parent index past the end stops the walk"
        );
    }
}
