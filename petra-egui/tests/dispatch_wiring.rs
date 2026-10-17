//! The shell dispatcher wired into the shipped `Host` (spec 010 T046,
//! T050, T051; FR-024's audit record).
//!
//! `src/dispatch.rs`'s own tests pin the interpretation order as a leaf.
//! These pin the wiring: that `Host::deliver_input` actually runs it, that a
//! matched binding consumes the event on its way to `App::handle`, that a
//! raw claim receives its chord verbatim and the trace says `raw`, and that
//! the reserved chord escapes the raw claim and returns focus — SC-004's
//! "both halves in one test, because either alone is a half-truth".

use std::ops::Range;
use std::sync::Arc;

use egui::{Event, Key, Modifiers, RawInput};
use gorgon_petra::input::{InputEvent, KeyCode, Route};
use gorgon_petra::keymap::{Binding, Chord, CommandName, Owner, Scope};
use gorgon_petra::layout::ChangeSet;
use gorgon_petra::tree::{Interaction, NodeKind, Role, ViewNode};
use gorgon_petra_egui::dispatch::InputDecision;
use gorgon_petra_egui::host::App;
use gorgon_petra_egui::testing::{boot, step};

/// The shell fixture: one ordinary focusable control and one raw claimant,
/// plus whatever binding set the test arms on the root.
#[derive(Default)]
struct ShellApp {
    /// Everything [`App::handle`] received, in order. A route name and the
    /// event's key, which is enough for every assertion below.
    handled: Vec<String>,
    /// [`App::decided`]'s audit lines, in order (FR-024).
    decided: Vec<String>,
    /// [`App::keymap_event`] reports, in order (FR-010, FR-011).
    keymap: Vec<String>,
    /// Bindings placed on the root, gathered into the table on the first
    /// frame like any node's.
    bindings: Vec<Binding>,
}

impl ShellApp {
    fn with_bindings(bindings: Vec<Binding>) -> Self {
        Self {
            bindings,
            ..Self::default()
        }
    }
}

impl gorgon_petra::layout::RowSource for ShellApp {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for ShellApp {
    fn view(&mut self) -> ViewNode {
        let mut root = ViewNode::new(NodeKind::Stack, "root");
        root.bindings = self.bindings.clone();
        root.child(ViewNode::new(NodeKind::Input, "shell").interactive(
            Role::TextInput,
            "Shell",
            &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
        ))
        .child(
            // An `Input`, not a `Custom`: the fixture's point is the raw
            // claim, and a `Custom` would need a registered `custom_kind`
            // measure or the whole tree is refused — which paints the
            // refusal view instead of seating focus.
            ViewNode::new(NodeKind::Input, "term")
                .with_raw_claim()
                .interactive(
                    Role::TextInput,
                    "Terminal",
                    &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
                ),
        )
    }

    fn handle(
        &mut self,
        event: &InputEvent,
        route: &Route,
        _frame: Option<&gorgon_petra::frame::PetrifiedFrame>,
    ) {
        let what = match event {
            InputEvent::Key { key, .. } => format!("key:{key:?}@{route:?}"),
            other => format!("{other:?}@{route:?}"),
        };
        self.handled.push(what);
    }

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::None
    }

    fn decided(&mut self, decision: &InputDecision) {
        self.decided.push(decision.to_string());
    }

    fn keymap_event(&mut self, event: &gorgon_petra_egui::dispatch::KeymapEvent) {
        self.keymap.push(event.to_string());
    }
}

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

fn egui_ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    }
}

/// The chord vocabulary's modifier set (`gorgon_petra`'s, not egui's — the
/// two are distinct types with one name).
fn petra_ctrl() -> gorgon_petra::Modifiers {
    gorgon_petra::Modifiers {
        ctrl: true,
        ..gorgon_petra::Modifiers::NONE
    }
}

fn binding(trigger: Vec<Chord>, name: &str, owner: Owner) -> Binding {
    Binding::new(trigger, CommandName::builtin(name), Scope::Global, owner)
        .expect("the fixture's bindings are legal")
}

/// The shell's save chord: `ctrl-s`, legal globally because it is not a bare
/// character chord (FR-013a).
fn save_trigger() -> Vec<Chord> {
    vec![Chord {
        key: KeyCode::Char('s'),
        modifiers: petra_ctrl(),
    }]
}

/// SC-004 in one test: a raw claim receives a chord the shell has bound
/// (T050) **and** the reserved chord never reaches the raw receiver and
/// returns focus (T051). Either half alone is a half-truth.
#[test]
fn a_raw_claim_gets_the_bound_chord_and_the_reserved_chord_escapes_it() {
    let app = ShellApp::with_bindings(vec![binding(
        save_trigger(),
        "save",
        Owner::Plugin("shell".to_owned()),
    )]);
    let (ctx, mut host) = boot(app);

    // Focus the raw claimant the way the operator would: Tab from the
    // shell control the first frame seated.
    step(&ctx, &mut host, key_press(Key::Tab, Modifiers::NONE));
    assert_eq!(host.state().focused.as_deref(), Some("/root/term"));

    // First half: the shell's own binding, pressed while a raw claim holds
    // focus. The claimant receives it verbatim — and the audit record says
    // `raw`, not `bound` (FR-005, FR-020).
    step(&ctx, &mut host, key_press(Key::S, egui_ctrl()));
    assert!(
        host.app()
            .handled
            .iter()
            .any(|h| h.contains("Char('s')") && h.contains("Raw")),
        "the raw claimant must receive the chord verbatim: {:?}",
        host.app().handled
    );
    let raw_line = host
        .app()
        .decided
        .iter()
        .find(|l| l.starts_with("raw:"))
        .cloned()
        .expect("the verbatim delivery must be traceable as `raw` (FR-024)");
    assert!(raw_line.contains("/root/term"), "{raw_line}");
    assert!(
        !host.app().decided.iter().any(|l| l.starts_with("bound:")),
        "the binding table must not be consulted inside a raw claim (FR-020): {:?}",
        host.app().decided
    );

    // Second half: the reserved chord. It reaches no receiver at all —
    // neither `App::handle` nor the raw claimant — and focus returns to the
    // shell (US-6).
    let handled_before = host.app().handled.len();
    step(&ctx, &mut host, key_press(Key::Escape, Modifiers::SHIFT));
    assert_eq!(
        host.app().handled.len(),
        handled_before,
        "the reserved chord must reach nobody: {:?}",
        host.app().handled
    );
    assert!(
        host.app()
            .decided
            .iter()
            .any(|l| l.starts_with("bound:") && l.contains("reserved")),
        "the reserved chord's own dispatch record must name it: {:?}",
        host.app().decided
    );
    assert_eq!(
        host.state().focused.as_deref(),
        Some("/root/shell"),
        "the reserved chord returns focus to the shell (US-6)"
    );
}

/// Step 3 wired: the binding table consumes the chord and the command is
/// dispatched through the audit record — `App::handle` never sees it
/// (FR-007).
#[test]
fn a_bound_chord_is_consumed_and_dispatched_not_delivered() {
    let app = ShellApp::with_bindings(vec![binding(
        save_trigger(),
        "save",
        Owner::Plugin("shell".to_owned()),
    )]);
    let (ctx, mut host) = boot(app);
    assert_eq!(host.state().focused.as_deref(), Some("/root/shell"));

    step(&ctx, &mut host, key_press(Key::S, egui_ctrl()));

    assert!(
        !host.app().handled.iter().any(|h| h.contains("Char('s')")),
        "a consumed chord must not also reach the focused node: {:?}",
        host.app().handled
    );
    let line = host
        .app()
        .decided
        .iter()
        .find(|l| l.starts_with("bound:"))
        .cloned()
        .expect("a matched binding must leave a dispatch record (FR-024)");
    assert!(line.contains("save"), "{line}");
    assert!(
        line.contains("global"),
        "the record must name the scope it matched in: {line}"
    );
}

/// T032 at the shell: the pending sequence is observable while it waits, so
/// the indicator is not guesswork (US-8).
#[test]
fn a_pending_sequence_is_observable_at_the_shell() {
    let app = ShellApp::with_bindings(vec![binding(
        vec![
            Chord {
                key: KeyCode::Char('k'),
                modifiers: petra_ctrl(),
            },
            Chord {
                key: KeyCode::Char('x'),
                modifiers: petra_ctrl(),
            },
        ],
        "chord-x",
        Owner::Plugin("shell".to_owned()),
    )]);
    let (ctx, mut host) = boot(app);

    step(&ctx, &mut host, key_press(Key::K, egui_ctrl()));

    assert_eq!(
        host.pending_chords().len(),
        1,
        "the first chord of a two-chord trigger must be observable in flight"
    );
    assert!(
        !host.app().decided.iter().any(|l| l.starts_with("bound:")),
        "a sequence that is still open dispatches nothing: {:?}",
        host.app().decided
    );
}

/// FR-010's refusal reaches the application rather than being swallowed: two
/// plugins, one trigger, one scope.
#[test]
fn a_duplicate_plugin_binding_is_refused_and_reported() {
    let app = ShellApp::with_bindings(vec![
        binding(save_trigger(), "save", Owner::Plugin("alpha".to_owned())),
        binding(save_trigger(), "save", Owner::Plugin("beta".to_owned())),
    ]);
    let (ctx, host) = boot(app);
    let _ = &ctx;

    assert!(
        host.app()
            .keymap
            .iter()
            .any(|e| e.contains("beta") && e.contains("alpha")),
        "the refusal must name the incoming binding and the sitting owner (FR-010): {:?}",
        host.app().keymap
    );
}
