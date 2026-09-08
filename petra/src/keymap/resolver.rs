//! [`Resolver`]: a trie over chord sequences that resolves a
//! [`Binding`](crate::keymap::binding::Binding) as each chord arrives.
//!
//! Spec 010 US-8 / FR-013: the operator presses `ctrl-k`, the shell shows a
//! pending indicator, they press `ctrl-s` and the command runs; or they wait
//! past a timeout and the pending state clears with nothing dispatched. A
//! chord that begins no trigger at all must resolve to [`Resolution::NoMatch`]
//! on that same key press, not after a timeout — the timeout exists only to
//! abandon a sequence that started matching and then stalled.
//!
//! The resolver does not know about focus or the frame tree; it is handed
//! the set of scopes currently reachable (always including
//! [`crate::keymap::scope::Scope::Global`] when the operator has granted it,
//! plus the [`crate::keymap::scope::Scope::Subtree`] roots on the focus
//! path) on every call, per FR-008a: a subtree binding whose root is not in
//! that set simply does not match, and is not an error.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::keymap::binding::{Binding, Owner};
use crate::keymap::chord::Chord;
use crate::keymap::scope::Scope;

/// How one chord resolved against the binding table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Resolution {
    /// The chord (or chord sequence) completed a binding's trigger in a
    /// scope that is currently reachable.
    Matched(Binding),
    /// The chord extends a valid prefix of a longer trigger. The sequence
    /// is not abandoned until [`Resolver::abandon_if_timed_out`] says so, or
    /// a further chord resolves it.
    Pending,
    /// The chord (given whatever prefix was already pending) begins no
    /// trigger reachable from the current scopes. Reported immediately;
    /// this outcome never waits for the timeout.
    NoMatch,
}

/// A binding conflicting with one already held in the same scope
/// (spec 010 FR-010).
///
/// Names both the incoming binding and the sitting one, each of which
/// already carries its own owner and scope in its [`fmt::Display`], so the
/// refusal text names the incoming binding, the sitting owner, and the
/// scope without duplicating any of it by hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictError {
    /// The binding that was being inserted.
    pub incoming: Binding,
    /// The binding already sitting on the same trigger in the same scope.
    pub sitting: Binding,
}

impl fmt::Display for ConflictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "refused: incoming binding {} conflicts with {} already held by \
             {} in {}",
            self.incoming,
            self.sitting,
            self.sitting.owner,
            self.sitting.scope.describe()
        )
    }
}

impl std::error::Error for ConflictError {}

/// One node of the trigger trie.
///
/// `reachable_scopes` is the set of scopes with at least one binding
/// reachable at or below this node, kept up to date on every insert. It is
/// what lets [`Resolver::resolve`] answer "does any trigger reachable from
/// the *active* scopes begin with this chord?" in one hash lookup per chord,
/// rather than searching every leaf.
#[derive(Default)]
struct TrieNode {
    children: HashMap<Chord, TrieNode>,
    bindings: Vec<Binding>,
    reachable_scopes: HashSet<Scope>,
}

/// The default pending-sequence timeout (spec 010 decision 010-4).
///
/// Long enough that a deliberate two-chord trigger like `ctrl-k ctrl-s`
/// comfortably fits between key presses, short enough that an operator who
/// pressed the first chord by mistake is not left wondering, a second and a
/// half later, whether anything happened. [`Resolver::with_timeout`]
/// overrides it at construction; nothing here reads a settings file.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1500);

/// A trie-backed resolver over chord sequences.
///
/// Holds every inserted [`Binding`], indexed by its trigger, and the state
/// of one in-flight chord sequence. `resolve` is the only way that state
/// advances; there is no second place a caller can ask "are we mid-sequence"
/// and get a different answer.
pub struct Resolver {
    root: TrieNode,
    timeout: Duration,
    pending: Vec<Chord>,
}

impl Resolver {
    /// A resolver with no bindings and [`DEFAULT_TIMEOUT`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// A resolver with no bindings and an explicit pending-sequence timeout.
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            root: TrieNode::default(),
            timeout,
            pending: Vec::new(),
        }
    }

    /// This resolver's pending-sequence timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// The chords typed so far in an in-flight sequence, for a shell to
    /// render a pending indicator from (spec 010 FR-013: "so the indicator
    /// is not guesswork"). Empty when no sequence is in flight.
    #[must_use]
    pub fn pending(&self) -> &[Chord] {
        &self.pending
    }

    /// Whether a chord sequence is currently in flight.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Inserts `binding` and reports what it displaced.
    ///
    /// Two rules meet here. FR-010 refuses a binding that collides with one
    /// already sitting on the same trigger in the same scope. FR-011 makes
    /// operator configuration outrank any plugin binding, so the one
    /// collision that is not a refusal is an operator binding landing on a
    /// plugin's trigger: the plugin's binding is pushed out and **handed
    /// back** as `Ok(Some(displaced))`.
    ///
    /// Displacement is returned rather than logged because this crate has no
    /// logger and should not grow one. FR-011 requires the displacement be
    /// logged rather than silent, and a value the caller must destructure is
    /// the strongest form of "not silent" available here: the daemon writes
    /// the trace entry, and a caller that drops it on the floor has made a
    /// visible choice rather than inherited a default.
    ///
    /// On refusal the table is unchanged and `binding` is handed back inside
    /// the error.
    pub fn insert(&mut self, binding: Binding) -> Result<Option<Binding>, Box<ConflictError>> {
        let scope = binding.scope.clone();
        let mut node = &mut self.root;
        for chord in binding.trigger() {
            node.reachable_scopes.insert(scope.clone());
            node = node.children.entry(*chord).or_default();
        }
        node.reachable_scopes.insert(scope.clone());
        let sitting = node.bindings.iter().position(|b| b.scope == scope);
        if let Some(at) = sitting {
            let outranks =
                binding.owner == Owner::Operator && node.bindings[at].owner != Owner::Operator;
            if !outranks {
                return Err(Box::new(ConflictError {
                    incoming: binding,
                    sitting: node.bindings[at].clone(),
                }));
            }
            let displaced = std::mem::replace(&mut node.bindings[at], binding);
            return Ok(Some(displaced));
        }
        node.bindings.push(binding);
        Ok(None)
    }

    /// Feeds one chord to the resolver and reports how it resolved.
    ///
    /// `active_scopes` is every scope currently reachable — the caller's
    /// answer to "who could this chord be for right now" (FR-008a). A
    /// [`Resolution::NoMatch`] or [`Resolution::Matched`] clears any pending
    /// sequence; [`Resolution::Pending`] extends it.
    pub fn resolve(&mut self, chord: Chord, active_scopes: &[Scope]) -> Resolution {
        let mut candidate = std::mem::take(&mut self.pending);
        candidate.push(chord);

        let mut node = &self.root;
        for step in &candidate {
            match node.children.get(step) {
                Some(child) => node = child,
                None => return Resolution::NoMatch,
            }
        }

        if !node
            .reachable_scopes
            .iter()
            .any(|scope| active_scopes.contains(scope))
        {
            return Resolution::NoMatch;
        }

        if let Some(binding) = node
            .bindings
            .iter()
            .find(|b| active_scopes.contains(&b.scope))
        {
            return Resolution::Matched(binding.clone());
        }

        self.pending = candidate;
        Resolution::Pending
    }

    /// Clears an in-flight sequence if `elapsed_since_last_chord` has
    /// reached this resolver's timeout, and reports whether it did.
    ///
    /// The resolver keeps no clock of its own; a caller wired to real time
    /// (or, in a test, to a fake one) supplies the elapsed duration.
    pub fn abandon_if_timed_out(&mut self, elapsed_since_last_chord: Duration) -> bool {
        if self.is_pending() && elapsed_since_last_chord >= self.timeout {
            self.pending.clear();
            true
        } else {
            false
        }
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, Modifiers};
    use crate::keymap::binding::Owner;
    use crate::keymap::command::CommandName;

    fn chord(key: KeyCode) -> Chord {
        Chord::bare(key)
    }

    fn ctrl(key: KeyCode) -> Chord {
        Chord {
            key,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        }
    }

    fn global_binding(trigger: Vec<Chord>, name: &str) -> Binding {
        Binding::new(
            trigger,
            CommandName::builtin(name),
            Scope::Global,
            Owner::Operator,
        )
        .unwrap()
    }

    fn owned_binding(trigger: Vec<Chord>, name: &str, owner: Owner) -> Binding {
        Binding::new(trigger, CommandName::builtin(name), Scope::Global, owner).unwrap()
    }

    #[test]
    fn an_operator_binding_displaces_a_plugin_one_and_reports_it() {
        let mut resolver = Resolver::new();
        let trigger = vec![ctrl(KeyCode::Char('s'))];
        resolver
            .insert(owned_binding(
                trigger.clone(),
                "plugin-save",
                Owner::Plugin("notes".into()),
            ))
            .expect("plugin binding lands on an empty trigger");

        let displaced = resolver
            .insert(owned_binding(trigger.clone(), "save", Owner::Operator))
            .expect("FR-011: operator configuration outranks a plugin binding");

        // Not silent. The caller is handed the binding it pushed out, so the
        // daemon can trace it and a caller that discards it has chosen to.
        let displaced = displaced.expect("displacement must be reported, not swallowed");
        assert_eq!(displaced.command.name, "plugin-save");
        assert_eq!(displaced.owner, Owner::Plugin("notes".into()));

        // And the operator's binding is the one that actually resolves now.
        match resolver.resolve(ctrl(KeyCode::Char('s')), &[Scope::Global]) {
            Resolution::Matched(b) => {
                assert_eq!(b.command.name, "save");
                assert_eq!(b.owner, Owner::Operator);
            }
            other => panic!("expected the operator binding to match, got {other:?}"),
        }
    }

    #[test]
    fn a_plugin_binding_cannot_displace_an_operator_one() {
        let mut resolver = Resolver::new();
        let trigger = vec![ctrl(KeyCode::Char('s'))];
        resolver
            .insert(owned_binding(trigger.clone(), "save", Owner::Operator))
            .expect("operator binding lands on an empty trigger");

        let refused = resolver.insert(owned_binding(
            trigger.clone(),
            "plugin-save",
            Owner::Plugin("notes".into()),
        ));
        let err = refused.expect_err("a plugin must not outrank the operator");
        assert_eq!(err.incoming.command.name, "plugin-save");
        assert_eq!(err.sitting.owner, Owner::Operator);

        // The table is unchanged: the operator's binding still resolves.
        match resolver.resolve(ctrl(KeyCode::Char('s')), &[Scope::Global]) {
            Resolution::Matched(b) => assert_eq!(b.command.name, "save"),
            other => panic!("expected the operator binding to survive, got {other:?}"),
        }
    }

    #[test]
    fn two_plugins_on_one_trigger_are_still_refused() {
        // FR-011 relaxes exactly one case. Plugin versus plugin stays FR-010.
        let mut resolver = Resolver::new();
        let trigger = vec![ctrl(KeyCode::Char('s'))];
        resolver
            .insert(owned_binding(
                trigger.clone(),
                "a",
                Owner::Plugin("first".into()),
            ))
            .expect("first plugin lands");
        resolver
            .insert(owned_binding(trigger, "b", Owner::Plugin("second".into())))
            .expect_err("a second plugin on one trigger is still a conflict");
    }

    #[test]
    fn a_single_chord_trigger_matches_immediately() {
        let mut resolver = Resolver::new();
        resolver
            .insert(global_binding(vec![ctrl(KeyCode::Char('s'))], "save"))
            .unwrap();

        let resolution = resolver.resolve(ctrl(KeyCode::Char('s')), &[Scope::Global]);
        match resolution {
            Resolution::Matched(binding) => assert_eq!(binding.command.name, "save"),
            other => panic!("expected Matched, got {other:?}"),
        }
        assert!(!resolver.is_pending());
    }

    #[test]
    fn a_prefix_of_a_longer_trigger_is_pending_then_matches() {
        let mut resolver = Resolver::new();
        resolver
            .insert(global_binding(
                vec![ctrl(KeyCode::Char('k')), ctrl(KeyCode::Char('s'))],
                "save-as",
            ))
            .unwrap();

        let resolution = resolver.resolve(ctrl(KeyCode::Char('k')), &[Scope::Global]);
        assert_eq!(resolution, Resolution::Pending);
        assert_eq!(resolver.pending(), &[ctrl(KeyCode::Char('k'))]);

        let resolution = resolver.resolve(ctrl(KeyCode::Char('s')), &[Scope::Global]);
        match resolution {
            Resolution::Matched(binding) => assert_eq!(binding.command.name, "save-as"),
            other => panic!("expected Matched, got {other:?}"),
        }
        assert!(!resolver.is_pending());
    }

    #[test]
    fn a_chord_beginning_no_trigger_is_nomatch_immediately() {
        let mut resolver = Resolver::new();
        resolver
            .insert(global_binding(vec![ctrl(KeyCode::Char('s'))], "save"))
            .unwrap();

        let resolution = resolver.resolve(chord(KeyCode::Function(9)), &[Scope::Global]);
        assert_eq!(resolution, Resolution::NoMatch);
        assert!(
            !resolver.is_pending(),
            "NoMatch must not leave a pending sequence behind"
        );
    }

    #[test]
    fn a_continuation_that_matches_nothing_is_nomatch_and_clears_pending() {
        let mut resolver = Resolver::new();
        resolver
            .insert(global_binding(
                vec![ctrl(KeyCode::Char('k')), ctrl(KeyCode::Char('s'))],
                "save-as",
            ))
            .unwrap();

        assert_eq!(
            resolver.resolve(ctrl(KeyCode::Char('k')), &[Scope::Global]),
            Resolution::Pending
        );
        // ctrl-p never continues any trigger that starts with ctrl-k.
        assert_eq!(
            resolver.resolve(ctrl(KeyCode::Char('p')), &[Scope::Global]),
            Resolution::NoMatch
        );
        assert!(!resolver.is_pending());
    }

    #[test]
    fn a_subtree_binding_does_not_match_when_its_root_is_not_active() {
        let mut resolver = Resolver::new();
        let binding = Binding::new(
            vec![chord(KeyCode::Char('j'))],
            CommandName::builtin("down"),
            Scope::Subtree {
                root: "panel:1".to_owned(),
            },
            Owner::Plugin("vim-motions".to_owned()),
        )
        .unwrap();
        resolver.insert(binding).unwrap();

        // No active scopes include panel:1 — FR-008a: absent, not an error.
        let resolution = resolver.resolve(chord(KeyCode::Char('j')), &[Scope::Global]);
        assert_eq!(resolution, Resolution::NoMatch);

        // With panel:1 active, the same chord matches.
        let resolution = resolver.resolve(
            chord(KeyCode::Char('j')),
            &[Scope::Subtree {
                root: "panel:1".to_owned(),
            }],
        );
        assert!(matches!(resolution, Resolution::Matched(_)));
    }

    #[test]
    fn abandon_if_timed_out_clears_pending_only_past_the_timeout() {
        let mut resolver = Resolver::with_timeout(Duration::from_millis(500));
        resolver
            .insert(global_binding(
                vec![ctrl(KeyCode::Char('k')), ctrl(KeyCode::Char('s'))],
                "save-as",
            ))
            .unwrap();
        resolver.resolve(ctrl(KeyCode::Char('k')), &[Scope::Global]);
        assert!(resolver.is_pending());

        assert!(!resolver.abandon_if_timed_out(Duration::from_millis(200)));
        assert!(resolver.is_pending(), "not yet timed out");

        assert!(resolver.abandon_if_timed_out(Duration::from_millis(500)));
        assert!(
            !resolver.is_pending(),
            "timeout clears with nothing dispatched"
        );
    }

    #[test]
    fn abandon_if_timed_out_is_a_no_op_with_nothing_pending() {
        let mut resolver = Resolver::new();
        assert!(!resolver.abandon_if_timed_out(Duration::from_secs(999)));
    }

    #[test]
    fn timeout_is_configurable_at_construction() {
        let resolver = Resolver::with_timeout(Duration::from_millis(42));
        assert_eq!(resolver.timeout(), Duration::from_millis(42));
        assert_eq!(Resolver::new().timeout(), DEFAULT_TIMEOUT);
    }

    #[test]
    fn a_conflicting_binding_is_refused_naming_both_owners() {
        let mut resolver = Resolver::new();
        let sitting = Binding::new(
            vec![ctrl(KeyCode::Char('g'))],
            CommandName::scoped("git", "status"),
            Scope::Global,
            Owner::Plugin("git".to_owned()),
        )
        .unwrap();
        resolver.insert(sitting).unwrap();

        let incoming = Binding::new(
            vec![ctrl(KeyCode::Char('g'))],
            CommandName::scoped("files", "grep"),
            Scope::Global,
            Owner::Plugin("files".to_owned()),
        )
        .unwrap();
        let err = resolver.insert(incoming).unwrap_err();

        let text = err.to_string();
        assert!(
            text.contains("files"),
            "must name the incoming owner: {text}"
        );
        assert!(text.contains("git"), "must name the sitting owner: {text}");
        assert!(text.contains("global"), "must name the scope: {text}");
        assert_eq!(err.incoming.command.name, "grep");
        assert_eq!(err.sitting.command.name, "status");
    }

    #[test]
    fn a_conflict_in_one_scope_does_not_block_the_same_trigger_in_another() {
        let mut resolver = Resolver::new();
        resolver
            .insert(global_binding(vec![ctrl(KeyCode::Char('g'))], "global-g"))
            .unwrap();

        let subtree = Binding::new(
            vec![ctrl(KeyCode::Char('g'))],
            CommandName::builtin("subtree-g"),
            Scope::Subtree {
                root: "panel:1".to_owned(),
            },
            Owner::Plugin("panel".to_owned()),
        )
        .unwrap();
        resolver
            .insert(subtree)
            .expect("same trigger, different scope, is not a conflict");
    }

    #[test]
    fn a_binding_that_is_a_prefix_of_another_does_not_conflict() {
        let mut resolver = Resolver::new();
        resolver
            .insert(global_binding(vec![ctrl(KeyCode::Char('k'))], "mark"))
            .unwrap();
        resolver
            .insert(global_binding(
                vec![ctrl(KeyCode::Char('k')), ctrl(KeyCode::Char('s'))],
                "save-as",
            ))
            .expect("a longer trigger extending a shorter one is not a conflict");
    }

    #[test]
    fn resolution_serde_round_trips() {
        let binding = global_binding(vec![ctrl(KeyCode::Char('s'))], "save");
        let resolution = Resolution::Matched(binding);
        let json = serde_json::to_string(&resolution).unwrap();
        let back: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(back, resolution);

        let json = serde_json::to_string(&Resolution::Pending).unwrap();
        assert_eq!(json, "\"pending\"");
        let json = serde_json::to_string(&Resolution::NoMatch).unwrap();
        assert_eq!(json, "\"no-match\"");
    }
}
