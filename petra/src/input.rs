//! Input events and routing.
//!
//! One vocabulary, one router. A synthetic action from the driver
//! (`contracts/driver-protocol.md`: "every `act` synthesizes the same low-level
//! input events a physical device produces") becomes exactly these events and
//! goes through exactly this router — there is no second path, which is what
//! makes a driver-run journey evidence about the product rather than about the
//! driver.

use crate::frame::{PetrifiedFrame, Placement};
use crate::geom::{Point, Size};
use crate::tree::Interaction;

/// A pointer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    /// Left button, or the primary tap.
    Primary,
    /// Right button, or a long press.
    Secondary,
    /// Middle button.
    Middle,
}

/// Chord modifiers in force for an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Shift.
    pub shift: bool,
    /// Control.
    pub ctrl: bool,
    /// Alt / Option.
    pub alt: bool,
    /// Command / Super / Windows.
    pub meta: bool,
}

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };

    /// Shift alone.
    #[must_use]
    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Self::NONE
        }
    }

    /// Whether any modifier is held.
    #[must_use]
    pub fn any(self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }
}

/// A physical key, named by its role rather than by a scancode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// Focus traversal.
    Tab,
    /// Activate.
    Enter,
    /// Dismiss.
    Escape,
    /// Space bar.
    Space,
    /// Delete backwards.
    Backspace,
    /// Delete forwards.
    Delete,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Start of line or list.
    Home,
    /// End of line or list.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// A function key, 1-24.
    Function(u8),
    /// A printable character key, lower-cased.
    Char(char),
}

/// One low-level input event.
///
/// Text arrives as [`InputEvent::Text`] rather than being derived from
/// [`InputEvent::Key`]: composition, dead keys, and IME all produce text with
/// no matching key press, and a toolkit that reconstructs text from keys gets
/// every non-Latin script wrong.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// The pointer moved to a position in logical units.
    PointerMoved {
        /// Where it is now.
        pos: Point,
    },
    /// A pointer button went down.
    PointerPressed {
        /// Where.
        pos: Point,
        /// Which button.
        button: PointerButton,
        /// Modifiers held.
        modifiers: Modifiers,
    },
    /// A pointer button came up.
    PointerReleased {
        /// Where.
        pos: Point,
        /// Which button.
        button: PointerButton,
        /// Modifiers held.
        modifiers: Modifiers,
    },
    /// The pointer left the window.
    PointerLeft,
    /// A scroll delta in logical units at a position.
    Scroll {
        /// Where.
        pos: Point,
        /// How far, per axis.
        delta: Size,
    },
    /// A key changed state.
    Key {
        /// Which key.
        key: KeyCode,
        /// Down (`true`) or up (`false`).
        pressed: bool,
        /// Whether this is an auto-repeat.
        repeat: bool,
        /// Modifiers held.
        modifiers: Modifiers,
    },
    /// Committed text, from a keystroke, a paste, or an IME composition.
    Text(String),
    /// The window gained keyboard focus.
    WindowFocused,
    /// The window lost keyboard focus.
    WindowBlurred,
}

impl InputEvent {
    /// The pointer position this event carries, if it carries one.
    #[must_use]
    pub fn pointer_pos(&self) -> Option<Point> {
        match self {
            Self::PointerMoved { pos }
            | Self::PointerPressed { pos, .. }
            | Self::PointerReleased { pos, .. }
            | Self::Scroll { pos, .. } => Some(*pos),
            _ => None,
        }
    }

    /// Whether this event is aimed by position rather than by focus.
    #[must_use]
    pub fn is_positional(&self) -> bool {
        self.pointer_pos().is_some()
    }
}

/// Where an event is delivered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// Delivered to the node under the pointer.
    Pointer {
        /// Canonical id of the node hit.
        node: String,
    },
    /// Delivered to the focused node.
    Keyboard {
        /// Canonical id of the focused node.
        node: String,
    },
    /// Nothing accepted it: no node under the pointer accepts this event kind,
    /// or nothing has focus. The event is dropped, and the caller can say so —
    /// a silent drop is how a "the click did nothing" bug hides.
    Unrouted {
        /// Why, in one phrase, for the driver's error text.
        reason: &'static str,
    },
}

/// The interaction a node must declare to receive an event.
#[must_use]
pub fn required_interaction(event: &InputEvent) -> Option<Interaction> {
    match event {
        InputEvent::PointerMoved { .. } | InputEvent::PointerLeft => Some(Interaction::Hover),
        InputEvent::PointerPressed { .. } | InputEvent::PointerReleased { .. } => {
            Some(Interaction::Click)
        }
        InputEvent::Scroll { .. } => Some(Interaction::Scroll),
        InputEvent::Key { .. } => Some(Interaction::Key),
        InputEvent::Text(_) => Some(Interaction::TextEdit),
        InputEvent::WindowFocused | InputEvent::WindowBlurred => None,
    }
}

/// Whether this event is a keyboard activation — the keystroke that stands in
/// for a primary click on the focused node.
///
/// FR-025 asks for every interactive node to be "reachable **and operable** by
/// keyboard alone". A button declares [`Interaction::Click`] and
/// [`Interaction::Focus`] and has no reason to declare [`Interaction::Key`], so
/// without this rule Tab reaches it and Enter does nothing at all: the node is
/// reachable and inert. Enter and Space are the desktop convention for that
/// stand-in, and only on the press — a release activating a second time would
/// fire every handler twice.
///
/// This widens [`route`] rather than [`required_interaction`], because the
/// interaction a node must declare does not change: an activation key is
/// accepted by a node declaring *either* `Key` (which takes precedence, so a
/// text field still sees its own Enter) *or* `Click`.
#[must_use]
pub fn activates(event: &InputEvent) -> bool {
    matches!(
        event,
        InputEvent::Key {
            key: KeyCode::Enter | KeyCode::Space,
            pressed: true,
            ..
        }
    )
}

/// The topmost placement at `pos` that accepts `interaction`.
///
/// "Topmost" is the frame's own paint order read backwards, so what the user
/// clicks is what the user sees. A placement is only a candidate when `pos` is
/// inside both its rect and the clip in force: a row scrolled out of its
/// viewport is still placed, and must not be clickable.
#[must_use]
pub fn hit_test(
    frame: &PetrifiedFrame,
    pos: Point,
    interaction: Interaction,
) -> Option<&Placement> {
    frame.paint_order().into_iter().rev().find(|p| {
        p.rect.contains(pos)
            && p.clip.contains(pos)
            && p.semantics.actions.contains(&interaction)
            && !p.semantics.disabled
    })
}

/// Route one event against a frame and the current focus.
#[must_use]
pub fn route(frame: &PetrifiedFrame, focused: Option<&str>, event: &InputEvent) -> Route {
    let Some(interaction) = required_interaction(event) else {
        return Route::Unrouted {
            reason: "window-level event, delivered to no node",
        };
    };
    if let Some(pos) = event.pointer_pos() {
        return match hit_test(frame, pos, interaction) {
            Some(hit) => Route::Pointer {
                node: hit.id.clone(),
            },
            None => Route::Unrouted {
                reason: "no node under the pointer accepts this event",
            },
        };
    }
    let Some(id) = focused else {
        return Route::Unrouted {
            reason: "nothing has focus",
        };
    };
    let Some(target) = frame.placement(id) else {
        return Route::Unrouted {
            reason: "the focused node is not in this frame",
        };
    };
    let accepted = target.semantics.actions.contains(&interaction)
        || (activates(event) && target.semantics.actions.contains(&Interaction::Click));
    if target.semantics.disabled || !accepted {
        return Route::Unrouted {
            reason: "the focused node does not accept this event",
        };
    }
    Route::Keyboard {
        node: id.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InputEvent, KeyCode, Modifiers, PointerButton, Route, activates, hit_test,
        required_interaction, route,
    };
    use crate::frame::{
        FrameDigest, PaintState, PetrifiedFrame, Placement, PlacementSemantics, TransitionActivity,
        Viewport, digest,
    };
    use crate::geom::{Point, Rect, Size};
    use crate::token::ThemeMode;
    use crate::tree::{Interaction, NodeKind};

    fn node(id: &str, rect: Rect, z: i32, actions: &[Interaction]) -> Placement {
        Placement {
            id: id.into(),
            kind: NodeKind::Text,
            rect,
            z,
            clip: Rect::new(0.0, 0.0, 200.0, 200.0),
            opacity: 1.0,
            paint: PaintState::default(),
            semantics: PlacementSemantics {
                actions: actions.to_vec(),
                ..PlacementSemantics::default()
            },
            parent: None,
        }
    }

    fn frame(placements: Vec<Placement>) -> PetrifiedFrame {
        let viewport = Viewport::new(Size::new(200.0, 200.0), ThemeMode::Dark);
        let digest: FrameDigest = digest::digest(&viewport, &placements);
        // These fixtures are independent, overlapping hit-test regions, not a
        // real placed tree — every one carries `parent: None`. `subtree_len`
        // of `1` each is the honest reflection of that: none of them has a
        // real child.
        let subtree_hashes = digest::subtree_hashes(viewport.scale, &placements);
        let subtree_len = vec![1; placements.len()];
        // Likewise for the slots: these fixtures were never offered anything
        // by a parent, so each stands in as its own offer. Nothing in
        // hit-testing reads them.
        let slots: Vec<crate::layout::Slot> = placements
            .iter()
            .map(|p| crate::layout::Slot::new(p.rect))
            .collect();
        let content = vec![crate::frame::PaintContent::default(); placements.len()];
        PetrifiedFrame {
            seq: 1,
            digest,
            placements,
            content,
            subtree_hashes,
            subtree_len,
            slots,
            viewport,
            transitions: TransitionActivity::default(),
        }
    }

    #[test]
    fn the_topmost_accepting_node_wins() {
        let f = frame(vec![
            node(
                "/under",
                Rect::new(0.0, 0.0, 100.0, 100.0),
                0,
                &[Interaction::Click],
            ),
            node(
                "/over",
                Rect::new(0.0, 0.0, 100.0, 100.0),
                5,
                &[Interaction::Click],
            ),
        ]);
        let hit = hit_test(&f, Point::new(10.0, 10.0), Interaction::Click).unwrap();
        assert_eq!(hit.id, "/over");
    }

    /// A node that does not declare the interaction is not a hit, so the click
    /// reaches what is underneath rather than being swallowed.
    #[test]
    fn a_node_that_declares_nothing_does_not_swallow_the_click() {
        let f = frame(vec![
            node(
                "/under",
                Rect::new(0.0, 0.0, 100.0, 100.0),
                0,
                &[Interaction::Click],
            ),
            node("/over", Rect::new(0.0, 0.0, 100.0, 100.0), 5, &[]),
        ]);
        let hit = hit_test(&f, Point::new(10.0, 10.0), Interaction::Click).unwrap();
        assert_eq!(hit.id, "/under");
    }

    /// A row scrolled out of its viewport is still placed. It must not be
    /// clickable, or a driver could act on something no human can reach.
    #[test]
    fn a_clipped_away_node_is_not_hittable() {
        let mut clipped = node(
            "/row",
            Rect::new(0.0, 300.0, 100.0, 20.0),
            0,
            &[Interaction::Click],
        );
        clipped.clip = Rect::new(0.0, 0.0, 200.0, 200.0);
        let f = frame(vec![clipped]);
        assert!(hit_test(&f, Point::new(10.0, 310.0), Interaction::Click).is_none());
    }

    #[test]
    fn a_disabled_node_is_not_hittable() {
        let mut disabled = node(
            "/btn",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[Interaction::Click],
        );
        disabled.semantics.disabled = true;
        let f = frame(vec![disabled]);
        assert!(hit_test(&f, Point::new(10.0, 10.0), Interaction::Click).is_none());
    }

    #[test]
    fn keyboard_events_go_to_focus_and_pointer_events_do_not() {
        let f = frame(vec![
            node(
                "/btn",
                Rect::new(0.0, 0.0, 100.0, 100.0),
                0,
                &[Interaction::Click],
            ),
            node(
                "/field",
                Rect::new(0.0, 150.0, 100.0, 20.0),
                0,
                &[Interaction::Key],
            ),
        ]);
        let key = InputEvent::Key {
            key: KeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            route(&f, Some("/field"), &key),
            Route::Keyboard {
                node: "/field".into()
            }
        );
        let click = InputEvent::PointerPressed {
            pos: Point::new(10.0, 10.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            route(&f, Some("/field"), &click),
            Route::Pointer {
                node: "/btn".into()
            }
        );
    }

    /// Every way an event can fail to land says which way it was, so the
    /// driver's error names the cause instead of "nothing happened".
    #[test]
    fn every_unrouted_case_names_itself() {
        let f = frame(vec![node(
            "/btn",
            Rect::new(0.0, 0.0, 10.0, 10.0),
            0,
            &[Interaction::Click],
        )]);
        let key = InputEvent::Key {
            key: KeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        // Not an activation key: `/btn` declares `Click` alone, and only
        // Enter/Space reach a click-only node.
        let typing = InputEvent::Key {
            key: KeyCode::Char('x'),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let reasons: Vec<&str> = [
            route(&f, None, &key),
            route(&f, Some("/gone"), &key),
            route(&f, Some("/btn"), &typing),
            route(
                &f,
                None,
                &InputEvent::PointerPressed {
                    pos: Point::new(150.0, 150.0),
                    button: PointerButton::Primary,
                    modifiers: Modifiers::NONE,
                },
            ),
            route(&f, None, &InputEvent::WindowFocused),
        ]
        .iter()
        .map(|r| match r {
            Route::Unrouted { reason } => *reason,
            other => panic!("expected Unrouted, got {other:?}"),
        })
        .collect();
        assert_eq!(reasons.len(), 5);
        // Five distinct causes, five distinct sentences.
        let mut sorted = reasons.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 5, "{reasons:?}");
    }

    /// FR-025's "operable": Tab reaches a button, and Enter or Space then
    /// works it, even though a button declares `Click` and never `Key`.
    #[test]
    fn enter_and_space_work_a_focused_node_that_only_declares_click() {
        let f = frame(vec![node(
            "/btn",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[Interaction::Click, Interaction::Focus],
        )]);
        for key in [KeyCode::Enter, KeyCode::Space] {
            let press = InputEvent::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            };
            assert!(activates(&press), "{key:?} should activate");
            assert_eq!(
                route(&f, Some("/btn"), &press),
                Route::Keyboard {
                    node: "/btn".into()
                },
                "{key:?} on a focused button must land on it"
            );
        }
    }

    /// The widening is exactly two keys on press. Anything else still needs
    /// the `Key` interaction, so a click-only node does not become a keyboard
    /// sink.
    #[test]
    fn only_the_activation_keys_reach_a_click_only_node() {
        let f = frame(vec![node(
            "/btn",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[Interaction::Click, Interaction::Focus],
        )]);
        let cases = [
            KeyCode::Char('x'),
            KeyCode::Tab,
            KeyCode::Escape,
            KeyCode::Down,
        ];
        for key in cases {
            let press = InputEvent::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            };
            assert!(!activates(&press), "{key:?} must not activate");
            assert!(
                matches!(route(&f, Some("/btn"), &press), Route::Unrouted { .. }),
                "{key:?} reached a node that never declared Key"
            );
        }
        let release = InputEvent::Key {
            key: KeyCode::Enter,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(!activates(&release), "only the press activates");
    }

    /// A disabled node is not operable by keyboard either, activation key or
    /// not.
    #[test]
    fn a_disabled_node_is_not_activated_by_a_key() {
        let mut disabled = node(
            "/btn",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[Interaction::Click, Interaction::Focus],
        );
        disabled.semantics.disabled = true;
        let f = frame(vec![disabled]);
        let press = InputEvent::Key {
            key: KeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(matches!(
            route(&f, Some("/btn"), &press),
            Route::Unrouted { .. }
        ));
    }

    #[test]
    fn each_event_kind_asks_for_the_interaction_it_needs() {
        assert_eq!(
            required_interaction(&InputEvent::Text("x".into())),
            Some(Interaction::TextEdit)
        );
        assert_eq!(
            required_interaction(&InputEvent::Scroll {
                pos: Point::ZERO,
                delta: Size::new(0.0, -10.0)
            }),
            Some(Interaction::Scroll)
        );
        assert_eq!(required_interaction(&InputEvent::WindowFocused), None);
    }
}
