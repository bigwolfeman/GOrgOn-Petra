//! Input translation: egui events in, Petra events out.
//!
//! This is the boundary the driver's synthetic input also crosses
//! (`contracts/driver-protocol.md`: hit-testing, focus rules, and event
//! ordering run exactly as for a human). Keeping the translation in one place
//! is what makes that true — an action injected as an `egui::Event` becomes the
//! same [`InputEvent`] a physical device produces, because it goes through this
//! function and there is no other one.

use std::collections::BTreeMap;

use egui::{Event, Key, MouseWheelUnit, PointerButton as EguiButton};
use gorgon_petra::geom::{Point, Size};
use gorgon_petra::input::{InputEvent, KeyCode, Modifiers, PointerButton};

/// Logical units one wheel "line" scrolls, on desktop.
///
/// egui uses the same two numbers for the same reason: the platforms disagree
/// about what a line is, and 40 is the value that ended up matching every other
/// desktop application (egui#461). Mirroring it here keeps a Petra window
/// scrolling at the same speed as the rest of the desktop.
pub const LINE_EXTENT_DESKTOP: f32 = 40.0;

/// Logical units one wheel "line" scrolls, on the web.
pub const LINE_EXTENT_WEB: f32 = 8.0;

/// Logical units one wheel "page" scrolls, as a multiple of a line.
pub const LINES_PER_PAGE: f32 = 20.0;

/// Turns egui's event stream into Petra's.
///
/// Stateful for one reason: egui reports a wheel event with a delta and no
/// position, so the translator remembers where the pointer last was. A
/// stateless function would have to invent a position, and a scroll delivered
/// to the wrong container is the kind of bug that only shows up under a
/// pointer that has not moved recently.
#[derive(Clone, Debug)]
pub struct EventTranslator {
    last_pointer: Option<Point>,
    line_extent: f32,
    untranslated: BTreeMap<&'static str, u64>,
}

impl Default for EventTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl EventTranslator {
    /// A translator with this target's line-scroll extent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_pointer: None,
            line_extent: if cfg!(target_arch = "wasm32") {
                LINE_EXTENT_WEB
            } else {
                LINE_EXTENT_DESKTOP
            },
            untranslated: BTreeMap::new(),
        }
    }

    /// Override the line-scroll extent.
    #[must_use]
    pub fn with_line_extent(mut self, extent: f32) -> Self {
        self.line_extent = extent;
        self
    }

    /// The last position the pointer was reported at.
    #[must_use]
    pub fn pointer(&self) -> Option<Point> {
        self.last_pointer
    }

    /// egui events this translator dropped, by variant name and count.
    ///
    /// Not every egui event has a Petra meaning yet. Counting them by name is
    /// the difference between a known gap and a mystery: a panel that ignores
    /// touch says so here rather than just not responding.
    #[must_use]
    pub fn untranslated(&self) -> Vec<(&'static str, u64)> {
        self.untranslated.iter().map(|(k, v)| (*k, *v)).collect()
    }

    /// Translate one egui event, or record it as untranslated.
    pub fn translate(&mut self, event: &Event) -> Option<InputEvent> {
        match event {
            Event::PointerMoved(pos) => {
                let p = Point::new(pos.x, pos.y);
                self.last_pointer = Some(p);
                Some(InputEvent::PointerMoved { pos: p })
            }
            Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } => {
                let p = Point::new(pos.x, pos.y);
                self.last_pointer = Some(p);
                let button = pointer_button(*button)?;
                let modifiers = translate_modifiers(*modifiers);
                Some(if *pressed {
                    InputEvent::PointerPressed {
                        pos: p,
                        button,
                        modifiers,
                    }
                } else {
                    InputEvent::PointerReleased {
                        pos: p,
                        button,
                        modifiers,
                    }
                })
            }
            Event::PointerGone => {
                self.last_pointer = None;
                Some(InputEvent::PointerLeft)
            }
            Event::MouseWheel { unit, delta, .. } => {
                // A wheel event with no pointer position has nowhere to go.
                // Dropping it is right: routing it to the origin would scroll
                // whatever happens to be in the top-left corner.
                let pos = self.last_pointer?;
                let scale = match unit {
                    MouseWheelUnit::Point => 1.0,
                    MouseWheelUnit::Line => self.line_extent,
                    MouseWheelUnit::Page => self.line_extent * LINES_PER_PAGE,
                };
                Some(InputEvent::Scroll {
                    pos,
                    delta: Size::new(delta.x * scale, delta.y * scale),
                })
            }
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                ..
            } => Some(InputEvent::Key {
                key: key_code(*key)?,
                pressed: *pressed,
                repeat: *repeat,
                modifiers: translate_modifiers(*modifiers),
            }),
            // Typed text, pasted text, and a committed composition are the same
            // thing to a field: text that arrived. Reconstructing it from key
            // events instead is how a toolkit gets every non-Latin script wrong.
            Event::Text(text) | Event::Paste(text) => Some(InputEvent::Text(text.clone())),
            Event::Ime(ime) => match ime {
                egui::ImeEvent::Commit(text) => Some(InputEvent::Text(text.clone())),
                _ => {
                    self.drop_event("Ime(preedit)");
                    None
                }
            },
            Event::WindowFocused(true) => Some(InputEvent::WindowFocused),
            Event::WindowFocused(false) => Some(InputEvent::WindowBlurred),
            other => {
                self.drop_event(variant_name(other));
                None
            }
        }
    }

    /// Translate a whole pass's events, in order.
    pub fn translate_all(&mut self, events: &[Event]) -> Vec<InputEvent> {
        events.iter().filter_map(|e| self.translate(e)).collect()
    }

    fn drop_event(&mut self, name: &'static str) {
        *self.untranslated.entry(name).or_insert(0) += 1;
    }
}

/// Petra's modifier set from egui's.
#[must_use]
pub fn translate_modifiers(m: egui::Modifiers) -> Modifiers {
    Modifiers {
        shift: m.shift,
        ctrl: m.ctrl,
        alt: m.alt,
        meta: m.mac_cmd || m.command && !m.ctrl,
    }
}

/// Petra's pointer button from egui's, or `None` for a button Petra has no
/// meaning for.
#[must_use]
pub fn pointer_button(button: EguiButton) -> Option<PointerButton> {
    match button {
        EguiButton::Primary => Some(PointerButton::Primary),
        EguiButton::Secondary => Some(PointerButton::Secondary),
        EguiButton::Middle => Some(PointerButton::Middle),
        EguiButton::Extra1 | EguiButton::Extra2 => None,
    }
}

/// Petra's key from egui's, or `None` for a key Petra has no name for.
///
/// The unmapped set is deliberate rather than incomplete: bare modifier
/// presses, browser navigation, and egui's synthetic `Copy`/`Cut`/`Paste` keys
/// are not keystrokes a view reacts to. Modifiers arrive on the events they
/// modify, and clipboard actions arrive as [`Event::Text`].
#[must_use]
pub fn key_code(key: Key) -> Option<KeyCode> {
    use Key as K;
    Some(match key {
        K::ArrowUp => KeyCode::Up,
        K::ArrowDown => KeyCode::Down,
        K::ArrowLeft => KeyCode::Left,
        K::ArrowRight => KeyCode::Right,
        K::Escape => KeyCode::Escape,
        K::Tab => KeyCode::Tab,
        K::Backspace => KeyCode::Backspace,
        K::Enter => KeyCode::Enter,
        K::Space => KeyCode::Space,
        K::Delete => KeyCode::Delete,
        K::Home => KeyCode::Home,
        K::End => KeyCode::End,
        K::PageUp => KeyCode::PageUp,
        K::PageDown => KeyCode::PageDown,
        K::Colon => KeyCode::Char(':'),
        K::Comma => KeyCode::Char(','),
        K::Backslash | K::IntlBackslash => KeyCode::Char('\\'),
        K::Slash => KeyCode::Char('/'),
        K::Pipe => KeyCode::Char('|'),
        K::Questionmark => KeyCode::Char('?'),
        K::Exclamationmark => KeyCode::Char('!'),
        K::OpenBracket => KeyCode::Char('['),
        K::CloseBracket => KeyCode::Char(']'),
        K::OpenCurlyBracket => KeyCode::Char('{'),
        K::CloseCurlyBracket => KeyCode::Char('}'),
        K::Backtick => KeyCode::Char('`'),
        K::Minus => KeyCode::Char('-'),
        K::Period => KeyCode::Char('.'),
        K::Plus => KeyCode::Char('+'),
        K::Equals => KeyCode::Char('='),
        K::Semicolon => KeyCode::Char(';'),
        K::Quote => KeyCode::Char('\''),
        K::Insert | K::Copy | K::Cut | K::Paste | K::BrowserBack => return None,
        K::ShiftLeft
        | K::ShiftRight
        | K::ControlLeft
        | K::ControlRight
        | K::AltLeft
        | K::AltRight
        | K::SuperLeft
        | K::SuperRight => return None,
        other => {
            // Letters, digits, and function keys carry their name, so one
            // lookup covers all seventy of them and cannot drift out of order.
            // egui names a letter key "A", a digit key "7", and a function
            // key "F7" — so the function-key test has to come first, and
            // `Key::F` (the letter) survives it because "" does not parse.
            let name = other.name();
            if let Some(digits) = name.strip_prefix('F')
                && let Ok(n) = digits.parse::<u8>()
                && (1..=35).contains(&n)
            {
                return Some(KeyCode::Function(n));
            }
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => {
                    KeyCode::Char(c.to_ascii_lowercase())
                }
                _ => return None,
            }
        }
    })
}

/// The egui variant name of an event, for the untranslated tally.
///
/// Exhaustive on purpose, with no catch-all: a new `egui::Event` in a future
/// release becomes a compile error here rather than a silent "unknown" that
/// nobody notices until an input kind stops working.
fn variant_name(event: &Event) -> &'static str {
    match event {
        Event::Copy => "Copy",
        Event::Cut => "Cut",
        Event::Paste(_) => "Paste",
        Event::Text(_) => "Text",
        Event::Key { .. } => "Key",
        Event::ModifiersChanged(_) => "ModifiersChanged",
        Event::PointerMoved(_) => "PointerMoved",
        Event::MouseMoved(_) => "MouseMoved",
        Event::PointerButton { .. } => "PointerButton",
        Event::PointerGone => "PointerGone",
        Event::Zoom(_) => "Zoom",
        Event::Rotate(_) => "Rotate",
        Event::Ime(_) => "Ime",
        Event::Touch { .. } => "Touch",
        Event::MouseWheel { .. } => "MouseWheel",
        Event::WindowFocused(_) => "WindowFocused",
        Event::AccessKitActionRequest(_) => "AccessKitActionRequest",
        Event::Screenshot { .. } => "Screenshot",
    }
}

#[cfg(test)]
mod tests {
    use super::{EventTranslator, LINE_EXTENT_DESKTOP, key_code, translate_modifiers};
    use egui::{Event, Key, MouseWheelUnit, Pos2, TouchPhase, Vec2};
    use gorgon_petra::geom::{Point, Size};
    use gorgon_petra::input::{InputEvent, KeyCode, PointerButton};

    fn translator() -> EventTranslator {
        EventTranslator::new().with_line_extent(LINE_EXTENT_DESKTOP)
    }

    #[test]
    fn pointer_events_carry_their_position() {
        let mut t = translator();
        assert_eq!(
            t.translate(&Event::PointerMoved(Pos2::new(3.0, 4.0))),
            Some(InputEvent::PointerMoved {
                pos: Point::new(3.0, 4.0)
            })
        );
        assert_eq!(t.pointer(), Some(Point::new(3.0, 4.0)));
        assert_eq!(
            t.translate(&Event::PointerGone),
            Some(InputEvent::PointerLeft)
        );
        assert_eq!(t.pointer(), None);
    }

    #[test]
    fn press_and_release_are_distinct_events() {
        let mut t = translator();
        for pressed in [true, false] {
            let out = t
                .translate(&Event::PointerButton {
                    pos: Pos2::new(1.0, 2.0),
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::SHIFT,
                })
                .unwrap();
            match (pressed, out) {
                (
                    true,
                    InputEvent::PointerPressed {
                        button, modifiers, ..
                    },
                )
                | (
                    false,
                    InputEvent::PointerReleased {
                        button, modifiers, ..
                    },
                ) => {
                    assert_eq!(button, PointerButton::Secondary);
                    assert!(modifiers.shift);
                }
                (_, other) => panic!("wrong event: {other:?}"),
            }
        }
    }

    /// A wheel event has a delta and no position. It must be routed to where
    /// the pointer actually is, not to the origin.
    #[test]
    fn a_wheel_event_borrows_the_last_pointer_position() {
        let mut t = translator();
        let wheel = Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: Vec2::new(0.0, -2.0),
            phase: TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(t.translate(&wheel), None, "no pointer, nowhere to send it");
        assert_eq!(
            t.untranslated(),
            Vec::new(),
            "a dropped wheel is not an unknown event"
        );

        t.translate(&Event::PointerMoved(Pos2::new(10.0, 20.0)));
        assert_eq!(
            t.translate(&wheel),
            Some(InputEvent::Scroll {
                pos: Point::new(10.0, 20.0),
                delta: Size::new(0.0, -2.0 * LINE_EXTENT_DESKTOP)
            })
        );
    }

    #[test]
    fn wheel_units_scale_differently() {
        let mut t = translator();
        t.translate(&Event::PointerMoved(Pos2::ZERO));
        let delta = |unit| {
            let mut t = t.clone();
            match t.translate(&Event::MouseWheel {
                unit,
                delta: Vec2::new(0.0, 1.0),
                phase: TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }) {
                Some(InputEvent::Scroll { delta, .. }) => delta.h,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(delta(MouseWheelUnit::Point), 1.0);
        assert_eq!(delta(MouseWheelUnit::Line), LINE_EXTENT_DESKTOP);
        assert!(delta(MouseWheelUnit::Page) > delta(MouseWheelUnit::Line));
    }

    /// Typed, pasted, and IME-committed text all reach a field the same way.
    #[test]
    fn every_source_of_text_becomes_text() {
        let mut t = translator();
        for event in [
            Event::Text("あ".into()),
            Event::Paste("あ".into()),
            Event::Ime(egui::ImeEvent::Commit("あ".into())),
        ] {
            assert_eq!(t.translate(&event), Some(InputEvent::Text("あ".into())));
        }
    }

    /// A composition in progress is not committed text. Delivering it would
    /// make a field show every intermediate keystroke of a CJK input.
    #[test]
    fn a_preedit_is_recorded_not_delivered() {
        let mut t = translator();
        let out = t.translate(&Event::Ime(egui::ImeEvent::Preedit {
            text: "か".into(),
            active_range_chars: None,
        }));
        assert_eq!(out, None);
        assert_eq!(t.untranslated(), [("Ime(preedit)", 1)]);
    }

    #[test]
    fn keys_map_by_role_and_by_name() {
        assert_eq!(key_code(Key::Tab), Some(KeyCode::Tab));
        assert_eq!(key_code(Key::ArrowUp), Some(KeyCode::Up));
        assert_eq!(key_code(Key::A), Some(KeyCode::Char('a')));
        assert_eq!(key_code(Key::Z), Some(KeyCode::Char('z')));
        assert_eq!(key_code(Key::Num7), Some(KeyCode::Char('7')));
        assert_eq!(key_code(Key::F1), Some(KeyCode::Function(1)));
        assert_eq!(key_code(Key::F12), Some(KeyCode::Function(12)));
        assert_eq!(key_code(Key::F35), Some(KeyCode::Function(35)));
        assert_eq!(key_code(Key::Slash), Some(KeyCode::Char('/')));
    }

    /// Bare modifier presses and egui's synthetic clipboard keys are not
    /// keystrokes a view reacts to, and must not arrive as if they were.
    #[test]
    fn keys_with_no_petra_meaning_are_refused() {
        for key in [
            Key::ShiftLeft,
            Key::ControlRight,
            Key::SuperLeft,
            Key::Copy,
            Key::Cut,
            Key::Paste,
            Key::Insert,
            Key::BrowserBack,
        ] {
            assert_eq!(key_code(key), None, "{key:?} should not translate");
        }
    }

    /// Every key egui can report either translates or is deliberately refused.
    /// This is what stops a new egui key silently mapping to the wrong char.
    #[test]
    fn every_egui_key_is_accounted_for() {
        let mut mapped = 0;
        let mut refused = 0;
        for key in Key::ALL {
            match key_code(*key) {
                Some(_) => mapped += 1,
                None => refused += 1,
            }
        }
        assert_eq!(mapped + refused, Key::ALL.len());
        assert!(mapped > 60, "only {mapped} of {} keys map", Key::ALL.len());
        assert!(
            refused >= 8,
            "the refused set should hold the modifiers and clipboard keys"
        );
    }

    #[test]
    fn an_event_petra_has_no_meaning_for_is_counted_by_name() {
        let mut t = translator();
        assert_eq!(t.translate(&Event::Zoom(1.1)), None);
        assert_eq!(t.translate(&Event::Zoom(1.2)), None);
        assert_eq!(t.translate(&Event::Copy), None);
        assert_eq!(t.untranslated(), [("Copy", 1), ("Zoom", 2)]);
    }

    #[test]
    fn command_on_mac_and_control_elsewhere_both_read_as_meta_or_ctrl() {
        let m = translate_modifiers(egui::Modifiers {
            alt: false,
            ctrl: true,
            shift: false,
            mac_cmd: false,
            command: true,
        });
        assert!(m.ctrl && !m.meta, "{m:?}");
        let mac = translate_modifiers(egui::Modifiers {
            alt: false,
            ctrl: false,
            shift: false,
            mac_cmd: true,
            command: true,
        });
        assert!(mac.meta && !mac.ctrl, "{mac:?}");
    }

    #[test]
    fn a_whole_pass_translates_in_order() {
        let mut t = translator();
        let out = t.translate_all(&[
            Event::PointerMoved(Pos2::new(1.0, 1.0)),
            Event::Zoom(1.0),
            Event::Text("a".into()),
            Event::WindowFocused(false),
        ]);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[0], InputEvent::PointerMoved { .. }));
        assert_eq!(out[1], InputEvent::Text("a".into()));
        assert_eq!(out[2], InputEvent::WindowBlurred);
    }
}
