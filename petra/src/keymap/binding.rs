//! [`Binding`]: a trigger, a command, a scope, and who set it.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::input::KeyCode;
use crate::keymap::chord::Chord;
use crate::keymap::command::CommandName;
use crate::keymap::scope::Scope;

/// Who a binding belongs to.
///
/// Distinct from [`CommandName`]'s plugin qualifier: `owner` is who
/// contributed the *binding* (and therefore whose lifecycle retracts it,
/// FR-009, and who outranks whom, FR-011), while `CommandName::plugin` is
/// who defines the *command* the binding dispatches. A plugin may bind a
/// built-in command, and the operator may bind a plugin's command; the two
/// fields vary independently.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Owner {
    /// The operator's own configuration.
    Operator,
    /// A plugin, named by its id.
    Plugin(String),
}

impl fmt::Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Operator => f.write_str("operator"),
            Self::Plugin(id) => write!(f, "plugin {id:?}"),
        }
    }
}

/// Why a [`Binding`] could not be constructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingError {
    /// A trigger must be a non-empty sequence of chords (spec 010 FR-002).
    EmptyTrigger,
    /// FR-013a: a global-scope binding used a bare `KeyCode::Char` chord —
    /// one with no ctrl, alt or meta held. Letters and digits are text, so
    /// such a binding would fire while the operator types into any ordinary
    /// field. `ctrl-k` is fine globally; bare `k` is not, and neither is
    /// `shift-k`, since shift alone still produces text (an upper-case
    /// letter), not a command chord.
    GlobalBareChar {
        /// The offending chord within the trigger.
        chord: Chord,
    },
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTrigger => f.write_str("a binding's trigger must have at least one chord"),
            Self::GlobalBareChar { chord } => write!(
                f,
                "global-scope binding cannot use bare character chord {chord} \
                 (letters and digits are text; hold ctrl, alt or meta to make \
                 it a command chord)"
            ),
        }
    }
}

impl std::error::Error for BindingError {}

/// Whether `chord` is a "bare" character chord: a printable character with
/// none of ctrl, alt or meta held. Shift alone does not exempt it, because
/// shift on a character key still produces text (spec 010 FR-013a).
fn is_bare_char(chord: &Chord) -> bool {
    matches!(chord.key, KeyCode::Char(_))
        && !chord.modifiers.ctrl
        && !chord.modifiers.alt
        && !chord.modifiers.meta
}

/// A trigger, a command, a scope, and who set it.
///
/// Constructed only through [`Binding::new`], which enforces FR-002 (a
/// non-empty trigger) and FR-013a (no bare character chord in a global-scope
/// trigger) — a `Binding` that exists is one of these two never had to be
/// checked again downstream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// The chord sequence that fires this binding. Non-empty (FR-002).
    trigger: Vec<Chord>,
    /// The command this binding dispatches.
    pub command: CommandName,
    /// Where this binding is reachable from.
    pub scope: Scope,
    /// Who contributed this binding.
    pub owner: Owner,
}

impl Binding {
    /// Builds a binding, refusing an empty trigger (FR-002) or a
    /// global-scope binding on a bare character chord (FR-013a).
    pub fn new(
        trigger: Vec<Chord>,
        command: CommandName,
        scope: Scope,
        owner: Owner,
    ) -> Result<Self, BindingError> {
        if trigger.is_empty() {
            return Err(BindingError::EmptyTrigger);
        }
        if scope == Scope::Global
            && let Some(chord) = trigger.iter().find(|c| is_bare_char(c))
        {
            return Err(BindingError::GlobalBareChar { chord: *chord });
        }
        Ok(Self {
            trigger,
            command,
            scope,
            owner,
        })
    }

    /// The chord sequence that fires this binding.
    #[must_use]
    pub fn trigger(&self) -> &[Chord] {
        &self.trigger
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, chord) in self.trigger.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{chord}")?;
        }
        write!(
            f,
            "] -> {} ({}, owned by {})",
            self.command,
            self.scope.describe(),
            self.owner
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;

    fn ctrl_k() -> Chord {
        Chord {
            key: KeyCode::Char('k'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        }
    }

    #[test]
    fn empty_trigger_is_refused() {
        let err = Binding::new(
            vec![],
            CommandName::builtin("save"),
            Scope::Global,
            Owner::Operator,
        )
        .unwrap_err();
        assert_eq!(err, BindingError::EmptyTrigger);
    }

    #[test]
    fn a_global_binding_refuses_bare_char_and_accepts_f5() {
        let bare = Chord::bare(KeyCode::Char('k'));
        let err = Binding::new(
            vec![bare],
            CommandName::builtin("save"),
            Scope::Global,
            Owner::Operator,
        )
        .unwrap_err();
        assert_eq!(err, BindingError::GlobalBareChar { chord: bare });

        let f5 = Chord::bare(KeyCode::Function(5));
        let binding = Binding::new(
            vec![f5],
            CommandName::builtin("save"),
            Scope::Global,
            Owner::Operator,
        )
        .expect("f5 is not text and stays eligible globally");
        assert_eq!(binding.trigger(), &[f5]);
    }

    #[test]
    fn ctrl_k_is_fine_globally_even_though_it_is_a_char_chord() {
        Binding::new(
            vec![ctrl_k()],
            CommandName::builtin("save"),
            Scope::Global,
            Owner::Operator,
        )
        .expect("ctrl-k is a command chord, not text");
    }

    #[test]
    fn shift_alone_does_not_exempt_a_character_chord() {
        let shift_k = Chord {
            key: KeyCode::Char('k'),
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::NONE
            },
        };
        let err = Binding::new(
            vec![shift_k],
            CommandName::builtin("save"),
            Scope::Global,
            Owner::Operator,
        )
        .unwrap_err();
        assert_eq!(err, BindingError::GlobalBareChar { chord: shift_k });
    }

    #[test]
    fn a_bare_char_is_fine_in_subtree_scope() {
        let bare = Chord::bare(KeyCode::Char('j'));
        Binding::new(
            vec![bare],
            CommandName::builtin("down"),
            Scope::Subtree {
                root: "panel:1".to_owned(),
            },
            Owner::Plugin("vim-motions".to_owned()),
        )
        .expect("FR-013a only restricts global scope");
    }

    #[test]
    fn display_names_trigger_command_scope_and_owner() {
        let binding = Binding::new(
            vec![ctrl_k(), Chord::bare(KeyCode::Char('s'))],
            CommandName::scoped("editor", "save"),
            Scope::Subtree {
                root: "editor:1".to_owned(),
            },
            Owner::Plugin("editor".to_owned()),
        )
        .unwrap();
        let text = binding.to_string();
        assert!(text.contains("ctrl-k"));
        assert!(text.contains('s'));
        assert!(text.contains("editor.save"));
        assert!(text.contains("editor:1"));
        assert!(text.contains("editor"));
    }

    #[test]
    fn serde_round_trips() {
        let binding = Binding::new(
            vec![ctrl_k()],
            CommandName::builtin("save"),
            Scope::Global,
            Owner::Operator,
        )
        .unwrap();
        let json = serde_json::to_string(&binding).unwrap();
        let back: Binding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, binding);
    }
}
