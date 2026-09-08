//! [`Chord`]: one [`KeyCode`] plus the [`Modifiers`] in force, with a
//! canonical text form.
//!
//! The text form is what an operator's configuration file and a plugin's
//! Lua both write (spec 010 FR-001), so [`fmt::Display`] and
//! [`std::str::FromStr`] are the two directions of one contract: rendering a
//! parsed chord must reproduce the exact bytes a well-formed input carried,
//! and a malformed string is refused with a reason rather than silently
//! defaulted.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::input::{KeyCode, Modifiers};

/// One key press together with the modifiers held at the moment it fired.
///
/// `Chord { key: KeyCode::Char('k'), modifiers: ctrl }` is the physical
/// event "control and k, together". A [`crate::keymap::binding::Binding`]'s
/// trigger is a non-empty sequence of these (spec 010 FR-002), so a chord is
/// the unit a trigger is built from, not the trigger itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    /// The key that was pressed.
    pub key: KeyCode,
    /// The modifiers held at the same time.
    pub modifiers: Modifiers,
}

impl Chord {
    /// A chord with no modifiers held.
    #[must_use]
    pub fn bare(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    /// The reserved chord an operator uses to leave a raw claim
    /// (spec 010 FR-021a). `shift-esc`: shift is the only modifier a person
    /// holds without meaning to, so the reserved chord pairs it with a key
    /// nobody presses by accident.
    #[must_use]
    pub fn reserved_escape() -> Self {
        Self {
            key: KeyCode::Escape,
            modifiers: Modifiers::shift(),
        }
    }
}

/// Why a chord's text form failed to parse.
///
/// Carries the offending text and a human-readable reason, so a refusal
/// names what was wrong rather than leaving the caller to diff strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordParseError {
    /// The text that failed to parse.
    pub input: String,
    /// Why it was refused.
    pub reason: String,
}

impl fmt::Display for ChordParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "malformed chord {:?}: {}", self.input, self.reason)
    }
}

impl std::error::Error for ChordParseError {}

impl ChordParseError {
    fn new(input: &str, reason: impl Into<String>) -> Self {
        Self {
            input: input.to_owned(),
            reason: reason.into(),
        }
    }
}

/// The canonical modifier prefixes, in the fixed order [`fmt::Display`]
/// renders them: control, then shift, then alt, then meta. A config file's
/// bytes must be stable, so this order is not "however the caller built the
/// struct" — it is fixed once, here, and nowhere else.
type ModifierPrefix = (&'static str, fn(Modifiers) -> bool);

const MODIFIER_PREFIXES: [ModifierPrefix; 4] = [
    ("ctrl-", |m| m.ctrl),
    ("shift-", |m| m.shift),
    ("alt-", |m| m.alt),
    ("meta-", |m| m.meta),
];

/// The canonical text for a named (non-character, non-function) key.
///
/// `Escape` renders as `esc`, the short form spec 010 FR-021a's reserved
/// chord `shift-esc` is written with; every other named key renders as its
/// own Rust identifier, matching the `alt-Left` example in the spec's
/// Definitions section.
fn named_key_text(key: KeyCode) -> Option<&'static str> {
    Some(match key {
        KeyCode::Tab => "Tab",
        KeyCode::Enter => "Enter",
        KeyCode::Escape => "esc",
        KeyCode::Space => "Space",
        KeyCode::Backspace => "Backspace",
        KeyCode::Delete => "Delete",
        KeyCode::Up => "Up",
        KeyCode::Down => "Down",
        KeyCode::Left => "Left",
        KeyCode::Right => "Right",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PageUp",
        KeyCode::PageDown => "PageDown",
        KeyCode::Function(_) | KeyCode::Char(_) => return None,
    })
}

/// The key text a candidate string names, matched against the canonical
/// vocabulary — never a fallback guess.
fn parse_key_text(text: &str) -> Result<KeyCode, String> {
    match text {
        "Tab" => return Ok(KeyCode::Tab),
        "Enter" => return Ok(KeyCode::Enter),
        "esc" => return Ok(KeyCode::Escape),
        "Space" => return Ok(KeyCode::Space),
        "Backspace" => return Ok(KeyCode::Backspace),
        "Delete" => return Ok(KeyCode::Delete),
        "Up" => return Ok(KeyCode::Up),
        "Down" => return Ok(KeyCode::Down),
        "Left" => return Ok(KeyCode::Left),
        "Right" => return Ok(KeyCode::Right),
        "Home" => return Ok(KeyCode::Home),
        "End" => return Ok(KeyCode::End),
        "PageUp" => return Ok(KeyCode::PageUp),
        "PageDown" => return Ok(KeyCode::PageDown),
        _ => {}
    }
    if let Some(digits) = text.strip_prefix('f')
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
    {
        let n: u32 = digits
            .parse()
            .map_err(|_| format!("{digits:?} is not a valid function key number"))?;
        return match u8::try_from(n) {
            Ok(n) if (1..=24).contains(&n) => Ok(KeyCode::Function(n)),
            _ => Err(format!("function key {n} is out of range 1-24")),
        };
    }
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => {
            if c.is_ascii_uppercase() {
                Err(format!(
                    "{c:?} must be written lower-case; use shift-{} for an uppercase letter",
                    c.to_ascii_lowercase()
                ))
            } else {
                Ok(KeyCode::Char(c))
            }
        }
        _ => Err(format!("{text:?} does not name a key")),
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (prefix, held) in MODIFIER_PREFIXES {
            if held(self.modifiers) {
                f.write_str(prefix)?;
            }
        }
        match self.key {
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Function(n) => write!(f, "f{n}"),
            named => f.write_str(named_key_text(named).expect("every named key has text")),
        }
    }
}

impl FromStr for Chord {
    type Err = ChordParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ChordParseError::new(s, "empty chord text"));
        }
        let mut rest = s;
        let mut modifiers = Modifiers::NONE;
        loop {
            let mut matched = false;
            for (prefix, held) in MODIFIER_PREFIXES {
                if let Some(after) = rest.strip_prefix(prefix) {
                    if after.is_empty() {
                        return Err(ChordParseError::new(
                            s,
                            format!("{prefix:?} has no key after it"),
                        ));
                    }
                    if held(modifiers) {
                        return Err(ChordParseError::new(
                            s,
                            format!("modifier {prefix:?} repeated"),
                        ));
                    }
                    match prefix {
                        "ctrl-" => modifiers.ctrl = true,
                        "shift-" => modifiers.shift = true,
                        "alt-" => modifiers.alt = true,
                        "meta-" => modifiers.meta = true,
                        _ => unreachable!("MODIFIER_PREFIXES is exhaustive"),
                    }
                    rest = after;
                    matched = true;
                    break;
                }
            }
            if !matched {
                break;
            }
        }
        let key = parse_key_text(rest).map_err(|reason| ChordParseError::new(s, reason))?;
        Ok(Chord { key, modifiers })
    }
}

impl Serialize for Chord {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let text = String::deserialize(de)?;
        Self::from_str(&text).map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl() -> Modifiers {
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        }
    }

    fn alt() -> Modifiers {
        Modifiers {
            alt: true,
            ..Modifiers::NONE
        }
    }

    #[test]
    fn ctrl_k_round_trips() {
        let chord = Chord {
            key: KeyCode::Char('k'),
            modifiers: ctrl(),
        };
        assert_eq!(chord.to_string(), "ctrl-k");
        assert_eq!("ctrl-k".parse::<Chord>().unwrap(), chord);
    }

    #[test]
    fn ctrl_shift_p_round_trips() {
        let chord = Chord {
            key: KeyCode::Char('p'),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        };
        assert_eq!(chord.to_string(), "ctrl-shift-p");
        assert_eq!("ctrl-shift-p".parse::<Chord>().unwrap(), chord);
    }

    #[test]
    fn alt_left_round_trips() {
        let chord = Chord {
            key: KeyCode::Left,
            modifiers: alt(),
        };
        assert_eq!(chord.to_string(), "alt-Left");
        assert_eq!("alt-Left".parse::<Chord>().unwrap(), chord);
    }

    #[test]
    fn f5_round_trips() {
        let chord = Chord::bare(KeyCode::Function(5));
        assert_eq!(chord.to_string(), "f5");
        assert_eq!("f5".parse::<Chord>().unwrap(), chord);
    }

    #[test]
    fn shift_esc_round_trips() {
        let chord = Chord::reserved_escape();
        assert_eq!(chord.to_string(), "shift-esc");
        assert_eq!("shift-esc".parse::<Chord>().unwrap(), chord);
        assert_eq!(
            chord,
            Chord {
                key: KeyCode::Escape,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::NONE
                },
            }
        );
    }

    #[test]
    fn modifier_order_is_canonical_regardless_of_construction() {
        // Display always emits ctrl, shift, alt, meta — the struct fields
        // carry no order of their own, so this is the only place an order
        // could leak in and it must not.
        let chord = Chord {
            key: KeyCode::Char('x'),
            modifiers: Modifiers {
                meta: true,
                alt: true,
                shift: true,
                ctrl: true,
            },
        };
        assert_eq!(chord.to_string(), "ctrl-shift-alt-meta-x");
    }

    #[test]
    fn a_malformed_chord_is_refused_with_a_reason() {
        let err = "".parse::<Chord>().unwrap_err();
        assert!(err.reason.contains("empty"));

        let err = "ctrl-".parse::<Chord>().unwrap_err();
        assert!(err.reason.contains("no key"));

        let err = "ctrl-ctrl-k".parse::<Chord>().unwrap_err();
        assert!(err.reason.contains("repeated"));

        let err = "f99".parse::<Chord>().unwrap_err();
        assert!(err.reason.contains("out of range"));

        let err = "banana".parse::<Chord>().unwrap_err();
        assert!(err.reason.contains("does not name a key"));

        let err = "ctrl-K".parse::<Chord>().unwrap_err();
        assert!(err.reason.contains("lower-case"));
    }

    #[test]
    fn escape_alone_parses_as_esc_not_escape() {
        assert_eq!(
            "esc".parse::<Chord>().unwrap(),
            Chord::bare(KeyCode::Escape)
        );
        assert!("Escape".parse::<Chord>().is_err());
    }

    #[test]
    fn a_literal_hyphen_key_round_trips() {
        // Char('-') is a real key on real keyboards; the parser must not
        // confuse the modifier separator with the key text.
        let chord = Chord {
            key: KeyCode::Char('-'),
            modifiers: ctrl(),
        };
        assert_eq!(chord.to_string(), "ctrl--");
        assert_eq!("ctrl--".parse::<Chord>().unwrap(), chord);
    }

    #[test]
    fn json_round_trip_uses_the_text_form() {
        let chord = Chord::reserved_escape();
        let json = serde_json::to_string(&chord).unwrap();
        assert_eq!(json, "\"shift-esc\"");
        let back: Chord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, chord);
    }

    #[test]
    fn json_deserialize_refuses_malformed_text() {
        let err = serde_json::from_str::<Chord>("\"banana\"").unwrap_err();
        assert!(err.to_string().contains("does not name a key"));
    }

    use proptest::prop_assert_eq;

    proptest::proptest! {
        #[test]
        fn display_from_str_round_trips_for_any_char_chord(
            c in proptest::char::range('a', 'z'),
            ctrl in proptest::bool::ANY,
            shift in proptest::bool::ANY,
            alt in proptest::bool::ANY,
            meta in proptest::bool::ANY,
        ) {
            let chord = Chord {
                key: KeyCode::Char(c),
                modifiers: Modifiers { shift, ctrl, alt, meta },
            };
            let text = chord.to_string();
            let parsed: Chord = text.parse().expect("canonical text must parse");
            prop_assert_eq!(parsed, chord);
            prop_assert_eq!(parsed.to_string(), text);
        }

        #[test]
        fn display_from_str_round_trips_for_any_function_key(
            n in 1u8..=24u8,
            ctrl in proptest::bool::ANY,
            shift in proptest::bool::ANY,
            alt in proptest::bool::ANY,
            meta in proptest::bool::ANY,
        ) {
            let chord = Chord {
                key: KeyCode::Function(n),
                modifiers: Modifiers { shift, ctrl, alt, meta },
            };
            let text = chord.to_string();
            let parsed: Chord = text.parse().expect("canonical text must parse");
            prop_assert_eq!(parsed, chord);
            prop_assert_eq!(parsed.to_string(), text);
        }
    }
}
