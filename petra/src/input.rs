//! Input events and routing.
//!
//! One vocabulary, one router. A synthetic action from the driver
//! (`contracts/driver-protocol.md`: "every `act` synthesizes the same low-level
//! input events a physical device produces") becomes exactly these events and
//! goes through exactly this router — there is no second path, which is what
//! makes a driver-run journey evidence about the product rather than about the
//! driver.

use std::collections::BTreeMap;

use crate::frame::{PetrifiedFrame, Placement};
use crate::geom::{Point, Size};
use crate::tree::{InputPolicy, Interaction};

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

/// One pointer capture in force: which node grabbed the pointer, with which
/// button, and where the gesture has been.
///
/// `origin` is where the button went down and `last` is where the pointer was
/// when this capture last saw an event. Both are kept because a drag is
/// defined by the distance between them: a component that needs "how far have
/// I been dragged" must not have to remember the press itself, and one that
/// needs "where is the pointer now" must not have to re-derive it from a
/// stream it may have missed events from.
#[derive(Clone, Debug, PartialEq)]
pub struct Capture {
    /// Canonical id of the node holding the capture.
    pub node: String,
    /// The button whose press opened the gesture. Only this button's release
    /// closes it.
    pub button: PointerButton,
    /// Where the press landed, in logical units.
    pub origin: Point,
    /// Where the pointer was at this capture's most recent event.
    pub last: Point,
}

/// Why a gesture ended without completing.
///
/// Four reasons rather than one bit, because a component undoes different
/// amounts of work for each: a blur may come back, an Escape is the author
/// saying "put it back", and a vanished node has nothing to put back at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CancelReason {
    /// The window lost keyboard focus while the gesture was in flight.
    Blurred,
    /// The pointer left: either out of the window
    /// ([`InputEvent::PointerLeft`]), or off the captured node's own rect at
    /// the moment the button came up.
    ///
    /// The second case is what stops a press-then-drag-off-then-release from
    /// activating a button (FR-016). `contracts/interaction-state.md` §7
    /// writes the release rule as "`PointerReleased`→`Completed`" and leaves
    /// this reason's trigger unassigned; a release outside the rect is the
    /// one release that did *not* complete the gesture, so it is the reason's
    /// second trigger rather than a fifth reason.
    Left,
    /// Escape was pressed while the gesture was in flight.
    Escape,
    /// The captured node is not in the newly placed frame. Unlike focus,
    /// capture has no successor rule: there is nothing to hand a half-finished
    /// gesture to.
    Vanished,
}

/// How a gesture ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GestureOutcome {
    /// The button came up inside the captured node. This is the only outcome
    /// a component may treat as activation.
    Completed,
    /// The gesture was cut short. See [`CancelReason`].
    Cancelled(CancelReason),
}

impl GestureOutcome {
    /// Whether the gesture finished the way its component intended.
    #[must_use]
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }
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
    /// A pointer gesture that held capture is over, and this is how it ended
    /// (`contracts/interaction-state.md` §7 "Ending").
    ///
    /// Synthesized by [`PointerState`], never produced by a device. It is an
    /// event rather than a return value because the node that held the
    /// capture has to *hear* about the end through the same `handle` path it
    /// heard the press through: a drag that is cancelled by a window blur
    /// gets no release, so a component listening only for
    /// [`InputEvent::PointerReleased`] would stay stuck mid-drag forever.
    ///
    /// It carries its own target, so it is aimed by neither position nor
    /// focus — the node named here is the one that held capture, whether or
    /// not it is still under the pointer, and whether or not it still exists
    /// ([`CancelReason::Vanished`]).
    GestureEnded {
        /// Canonical id of the node that held the capture.
        node: String,
        /// Whether the gesture finished or was cut short, and why.
        outcome: GestureOutcome,
    },
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
    /// Delivered to the node under the pointer, or — for a pointer-exit,
    /// which carries no position — to the node it was last over.
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

/// The interaction a node must declare to receive an event, with no pointer
/// capture in force.
///
/// [`required_interaction_during`] is the whole rule; this is the common call
/// with `capture` set to `None`.
#[must_use]
pub fn required_interaction(event: &InputEvent) -> Option<Interaction> {
    required_interaction_during(event, None)
}

/// The interaction a node must declare to receive `event`, given the pointer
/// capture in force.
///
/// A pointer stream means two different things depending on whether a button
/// is down inside a node that asked for it. With nothing captured, a move is
/// a hover and a release is half of a click, which is what
/// [`required_interaction`] answers. With a capture in force the same three
/// positional events are the *body of a drag*
/// (`contracts/interaction-state.md` §7), and the interaction the holder had
/// to declare to be receiving them at all is [`Interaction::Drag`] — it is
/// what [`PointerState::route`] hit-tested for before it granted the capture.
///
/// This is a statement about the *declaration*, not about delivery: during a
/// capture the event routes to the holder unconditionally, so nothing
/// re-checks this. It is the answer a driver, a component author, or a test
/// gets when it asks "what is this event, right now" — and answering "hover"
/// for the middle of a drag would be a lie about which contract the component
/// is operating under.
#[must_use]
pub fn required_interaction_during(
    event: &InputEvent,
    capture: Option<&Capture>,
) -> Option<Interaction> {
    if capture.is_some() && event.is_positional() {
        return Some(Interaction::Drag);
    }
    match event {
        InputEvent::PointerMoved { .. } | InputEvent::PointerLeft => Some(Interaction::Hover),
        InputEvent::PointerPressed { .. } | InputEvent::PointerReleased { .. } => {
            Some(Interaction::Click)
        }
        InputEvent::Scroll { .. } => Some(Interaction::Scroll),
        InputEvent::Key { .. } => Some(Interaction::Key),
        InputEvent::Text(_) => Some(Interaction::TextEdit),
        // A gesture end is the last event of a drag and reports on one; the
        // node it names declared `Drag` to have been given the capture, so
        // that is the interaction it is heard under.
        InputEvent::GestureEnded { .. } => Some(Interaction::Drag),
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
///
/// This is [`hit_test_above`] with no floor: every placement in the frame
/// is a candidate. A host routing through [`route_with_surfaces`] or
/// [`PointerState::route`] gets the floor an open `Block` surface imposes.
#[must_use]
pub fn hit_test(
    frame: &PetrifiedFrame,
    pos: Point,
    interaction: Interaction,
) -> Option<&Placement> {
    hit_test_above(frame, pos, interaction, None)
}

/// [`hit_test`], stopping at `floor`: the placement with that id is the last
/// candidate considered, and nothing behind it in paint order is reachable.
///
/// The floor is what makes an [`InputPolicy::Block`] surface *opaque* over
/// its own rect, the other half of "a modal makes the screen behind it
/// inert" ([`route_with_surfaces`] swallows the outside; this closes the
/// inside). A click on a dialog's body text, which accepts nothing, must not
/// land on the page button the dialog happens to be covering. The floor
/// placement itself is still a candidate — a scrim that declares `Click` to
/// absorb it is heard — so the rule is "nothing *behind* the surface",
/// never "nothing at or above its z".
///
/// A `floor` naming no placement in `frame` is the same as no floor.
#[must_use]
pub fn hit_test_above<'a>(
    frame: &'a PetrifiedFrame,
    pos: Point,
    interaction: Interaction,
    floor: Option<&str>,
) -> Option<&'a Placement> {
    for p in frame.paint_order().into_iter().rev() {
        if p.rect.contains(pos)
            && p.clip.contains(pos)
            && p.semantics.actions.contains(&interaction)
            && !p.semantics.disabled
        {
            return Some(p);
        }
        if floor == Some(p.id.as_str()) {
            return None;
        }
    }
    None
}

/// Route one event against a frame and the current focus.
///
/// Two events are handled before anything else and never reach the focus
/// branch below, because both are aimed by something other than position or
/// focus.
///
/// [`InputEvent::PointerLeft`] is a pointer event carrying no position, so
/// `event.pointer_pos()` is `None` for it and it would otherwise fall
/// straight through to the focused node — telling whatever holds focus that
/// the pointer left it, when the pointer may never have been over it.
/// [`route_pointer_exit`] is where pointer-exit does go; `route` has no hover
/// state of its own to hand it, so it passes `None` and the exit is reported
/// dropped rather than misdelivered. **[`PointerState::route`] is the caller
/// that has one** (`contracts/interaction-state.md` §8), and a host routes
/// through it rather than through here.
///
/// [`InputEvent::GestureEnded`] carries its own target: the node that held
/// the capture, which may be neither under the pointer nor focused, and under
/// [`CancelReason::Vanished`] is not in this frame at all. So it is delivered
/// by name, with none of the acceptance checks the other branches apply — a
/// gesture end is a report about a contract the node already entered, not an
/// offer of a new one it may decline.
#[must_use]
pub fn route(frame: &PetrifiedFrame, focused: Option<&str>, event: &InputEvent) -> Route {
    route_above(frame, focused, event, None)
}

/// [`route`], with the positional branch hit-testing above `floor`
/// ([`hit_test_above`]). [`route_with_surfaces`] is the caller that has a
/// floor to pass; [`route`] passes none.
fn route_above(
    frame: &PetrifiedFrame,
    focused: Option<&str>,
    event: &InputEvent,
    floor: Option<&str>,
) -> Route {
    if matches!(event, InputEvent::PointerLeft) {
        return route_pointer_exit(frame, None);
    }
    if let InputEvent::GestureEnded { node, .. } = event {
        return Route::Pointer { node: node.clone() };
    }
    let Some(interaction) = required_interaction(event) else {
        return Route::Unrouted {
            reason: "window-level event, delivered to no node",
        };
    };
    if let Some(pos) = event.pointer_pos() {
        return match hit_test_above(frame, pos, interaction, floor) {
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

/// Route a pointer-exit ([`InputEvent::PointerLeft`]) to the node the pointer
/// was last over.
///
/// Pointer-exit is the one pointer event with no position, so no hit test can
/// aim it. The node that needs to hear it is the one that was under the
/// pointer when it left — a hovered button has to stop looking hovered — and
/// nothing in this crate remembers which node that was between frames. The
/// host's `PointerState` (`contracts/interaction-state.md` §7, owned beside
/// [`crate::focus::FocusTree`]) is what will, and `hovered` is where it plugs
/// in: a caller that tracks the hovered id passes it and the exit lands
/// there; a caller that does not passes `None` and the exit is reported
/// dropped. [`route`] is the second kind today, and that is the whole of the
/// remaining gap — the routing rule itself is here and tested.
///
/// A node hears pointer-exit on the same terms it hears any other hover
/// event: it must be in this frame, declare [`Interaction::Hover`], and not
/// be disabled.
#[must_use]
pub fn route_pointer_exit(frame: &PetrifiedFrame, hovered: Option<&str>) -> Route {
    let Some(id) = hovered else {
        return Route::Unrouted {
            reason: "the pointer left the window and no hovered node is tracked",
        };
    };
    let Some(target) = frame.placement(id) else {
        return Route::Unrouted {
            reason: "the hovered node is not in this frame",
        };
    };
    if target.semantics.disabled || !target.semantics.actions.contains(&Interaction::Hover) {
        return Route::Unrouted {
            reason: "the hovered node does not accept pointer-exit",
        };
    }
    Route::Pointer {
        node: id.to_owned(),
    }
}

/// The pointer's position, what it is over, and what it has captured.
///
/// One per window, owned by the host beside its
/// [`FocusTree`](crate::focus::FocusTree) — the same ownership shape and for
/// the same reason (`contracts/interaction-state.md` §7). Two facts here
/// cannot be recovered from a frame: where the pointer was between events,
/// and which node grabbed it. Both have to be remembered by something that
/// outlives a frame, and `gorgon-petra` deliberately owns no such thing, so
/// the host does.
///
/// # Hover is derived, never declared
///
/// `hovered` is written only by [`hit_test`] against `Interaction::Hover` —
/// the same hit test that routes a click (FR-009), so the lit control and the
/// clicked control cannot be two different nodes. It is at most one node by
/// construction: `hit_test` returns the topmost accepting placement or
/// nothing.
///
/// An application must not derive its own hover from a `Route::Pointer`: that
/// is a second hit test by a second owner, and the two drift the first time a
/// surface overlaps something.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointerState {
    pos: Option<Point>,
    hovered: Option<String>,
    capture: Option<Capture>,
    /// Whether the node this state calls pressed is pressed: the pointer is
    /// inside that node's own rect. Kept as a bit rather than recomputed on
    /// demand because the answer is a fact about a frame, and the caller that
    /// needs it — a host publishing `LayoutState` before the next
    /// negotiation — is between frames and holds none.
    pressed: bool,
    /// The node an **uncaptured** primary press went down on, and `None`
    /// whenever the primary button is up, a gesture owns the pointer, or the
    /// press landed on nothing.
    ///
    /// # Why this exists at all
    ///
    /// Capture is granted only to a node declaring [`Interaction::Drag`]
    /// (see [`PointerState::route_positional`]), which is right: a gesture
    /// that has to keep receiving moves after leaving its own rect is a
    /// drag. A button declares `Click`, never `Drag`, so before this field
    /// existed a button could not be pressed at all — [`PointerState::pressed`]
    /// was derived from the capture alone, so
    /// [`crate::frame::PlacementSemantics::active`] was permanently false for
    /// every button, checkbox and menu item in the library, and every
    /// `background@active` binding in `crate::component` was a token nothing
    /// could ever read.
    ///
    /// Pressed is not a routing fact. It is a fact about the pointer, so it
    /// is answered from the pointer and routing is left exactly as it was:
    /// no `Click` node gains the capture, no event changes where it lands.
    ///
    /// # Why the *node* and not a bare "is the button down" bit
    ///
    /// A press belongs to the node it started on. A bare bit made a held
    /// pointer light up whatever it slid over — press `Cancel`, drag across
    /// `Delete`, and `Delete` looked pressed under a finger that never went
    /// down on it. Remembering the node makes the uncaptured press behave
    /// the way the captured one already does: it sticks to its own control,
    /// and it stops looking pressed when the pointer leaves that control's
    /// rect without ever moving to a neighbour.
    pressed_on: Option<String>,
}

/// What one event did to a [`PointerState`], beyond where it routed.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerRouting {
    /// Where the event landed, or why it did not, plus any dismissals.
    pub outcome: RouteOutcome,
    /// The gesture this event ended, if it ended one. A host delivers it as
    /// [`InputEvent::GestureEnded`] immediately after the event that caused
    /// it, so the holder hears the cause and then the consequence.
    pub ended: Option<GestureEnd>,
}

/// One ended gesture: which node held the capture, and how it finished.
#[derive(Clone, Debug, PartialEq)]
pub struct GestureEnd {
    /// Canonical id of the node that held the capture.
    pub node: String,
    /// How it finished.
    pub outcome: GestureOutcome,
}

impl GestureEnd {
    /// The event a host delivers to report this end.
    #[must_use]
    pub fn event(&self) -> InputEvent {
        InputEvent::GestureEnded {
            node: self.node.clone(),
            outcome: self.outcome,
        }
    }

    /// Where that event is delivered: to the node that held the capture, by
    /// name rather than by hit test — it may no longer be under the pointer,
    /// and under [`CancelReason::Vanished`] it may no longer exist.
    #[must_use]
    pub fn route(&self) -> Route {
        Route::Pointer {
            node: self.node.clone(),
        }
    }
}

impl PointerState {
    /// Nothing under the pointer, nothing captured.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the pointer is, or `None` before the first positional event and
    /// after it leaves the window.
    #[must_use]
    pub fn pos(&self) -> Option<Point> {
        self.pos
    }

    /// The one node the pointer is over, if any.
    #[must_use]
    pub fn hovered(&self) -> Option<&str> {
        self.hovered.as_deref()
    }

    /// The capture in force, if any.
    #[must_use]
    pub fn capture(&self) -> Option<&Capture> {
        self.capture.as_ref()
    }

    /// The node that is *pressed*: the primary button is down and the
    /// pointer was inside that node's rect as of the most recent frame or
    /// event.
    ///
    /// Which node, in the two cases:
    ///
    /// * **A gesture is in flight.** The capture's holder, because every
    ///   move routes there and nothing else may light up underneath it.
    /// * **Nothing is captured.** The node the press went down on, because a
    ///   button declares [`Interaction::Click`] and never
    ///   [`Interaction::Drag`] and so is never granted the capture — see
    ///   [`PointerState::pressed_on`] for the whole of that argument.
    ///
    /// Distinct from [`PointerState::capture`] on purpose
    /// (`contracts/interaction-state.md` §1): a button pressed and then
    /// dragged off its own rect keeps the capture — every move still routes
    /// to it — and stops looking pressed. One bit standing for both would
    /// either leave the button lit with the pointer elsewhere, or drop the
    /// gesture at the rect's edge.
    #[must_use]
    pub fn pressed(&self) -> Option<&str> {
        if !self.pressed {
            return None;
        }
        match &self.capture {
            Some(capture) => Some(capture.node.as_str()),
            None => self.pressed_on.as_deref(),
        }
    }

    /// Route `event` against `frame`, updating hover and capture.
    ///
    /// The whole of `contracts/interaction-state.md` §7 and §8, in the order
    /// the contract puts them:
    ///
    /// 1. **The capture short-circuit runs first**, ahead of `hit_test` *and*
    ///    ahead of the modal swallow test, so a drag begun inside a modal
    ///    keeps receiving moves outside the modal's rect.
    /// 2. **Pointer-exit is aimed by memory**, through
    ///    [`route_pointer_exit`] with this state's real `hovered` id, which
    ///    is the seam [`route`] alone has to pass `None` into.
    /// 3. **A press hit-tests for [`Interaction::Drag`] before
    ///    [`Interaction::Click`]**; a hit grants the capture and routes the
    ///    press there, a miss falls through to the ordinary click path
    ///    unchanged.
    ///
    /// `surfaces` is the same map [`route_with_surfaces`] takes.
    pub fn route(
        &mut self,
        frame: &PetrifiedFrame,
        focused: Option<&str>,
        surfaces: &BTreeMap<String, InputPolicy>,
        event: &InputEvent,
    ) -> PointerRouting {
        match event {
            InputEvent::PointerLeft => self.pointer_left(frame),
            InputEvent::WindowBlurred => {
                // A window that lost keyboard focus is not going to see the
                // button come up, so the gesture has to end here or it never
                // ends at all. The same argument covers the uncaptured
                // press: a node left looking held while the window is in the
                // background is a control the operator cannot let go of.
                self.pressed_on = None;
                self.pressed = false;
                let ended = self.end_capture(CancelReason::Blurred);
                PointerRouting {
                    outcome: route_with_surfaces(frame, focused, event, surfaces),
                    ended,
                }
            }
            InputEvent::Key {
                key: KeyCode::Escape,
                pressed: true,
                ..
            } if self.capture.is_some() => {
                // Escape is the author saying "put it back", and it is
                // swallowed rather than also delivered: a component that got
                // both would undo the gesture and then run whatever its own
                // Escape handler does, which on a dialog is closing the
                // dialog the drag was inside.
                let ended = self.end_capture(CancelReason::Escape);
                PointerRouting {
                    outcome: RouteOutcome {
                        route: Route::Unrouted {
                            reason: "escape cancelled the gesture holding the pointer",
                        },
                        dismiss: Vec::new(),
                    },
                    ended,
                }
            }
            _ => match event.pointer_pos() {
                Some(pos) => self.route_positional(frame, focused, surfaces, event, pos),
                None => PointerRouting {
                    outcome: route_with_surfaces(frame, focused, event, surfaces),
                    ended: None,
                },
            },
        }
    }

    /// Reconcile against a newly placed frame, the way
    /// [`FocusTree::update`](crate::focus::FocusTree::update) does.
    ///
    /// Two jobs, both of which need placements that did not exist when the
    /// last event was routed:
    ///
    /// * **The vanished rule.** A captured node absent from the new frame
    ///   ends its gesture as [`CancelReason::Vanished`]. Unlike focus there
    ///   is no successor: a half-finished gesture has nothing to be handed
    ///   to.
    /// * **Hover follows the picture, not the pointer.** Hover is a fact
    ///   about the frame on screen, so a node that moved out from under a
    ///   stationary pointer — or became disabled, or stopped declaring
    ///   `Hover` — stops being hovered here, without waiting for a move that
    ///   may never come.
    ///
    /// Returns the gesture this frame ended, if any.
    pub fn reconcile(
        &mut self,
        frame: &PetrifiedFrame,
        surfaces: &BTreeMap<String, InputPolicy>,
    ) -> Option<GestureEnd> {
        let ended = match &self.capture {
            Some(capture) if frame.placement(&capture.node).is_none() => {
                self.end_capture(CancelReason::Vanished)
            }
            _ => None,
        };
        self.hovered = self.derive_hover(frame, surfaces);
        self.refresh_pressed(frame);
        ended
    }

    /// Route [`InputEvent::PointerLeft`] (`contracts/interaction-state.md`
    /// §8) and forget where the pointer was.
    ///
    /// Pointer-exit is the one pointer event with no position, so no hit test
    /// can aim it; it goes to the node the pointer was last over, which is
    /// exactly what this struct remembers. A capture in flight ends as
    /// [`CancelReason::Left`] — the pointer is outside the window and the
    /// button will come up somewhere this host will never hear about.
    fn pointer_left(&mut self, frame: &PetrifiedFrame) -> PointerRouting {
        let ended = self.end_capture(CancelReason::Left);
        let route = route_pointer_exit(frame, self.hovered.as_deref());
        self.hovered = None;
        self.pos = None;
        self.pressed = false;
        // The pointer is outside the window and the release will happen
        // somewhere this host never hears about, so the button is not held
        // as far as anything here can know.
        self.pressed_on = None;
        PointerRouting {
            outcome: RouteOutcome {
                route,
                dismiss: Vec::new(),
            },
            ended,
        }
    }

    /// The positional path: move, press, release, scroll.
    fn route_positional(
        &mut self,
        frame: &PetrifiedFrame,
        focused: Option<&str>,
        surfaces: &BTreeMap<String, InputPolicy>,
        event: &InputEvent,
        pos: Point,
    ) -> PointerRouting {
        self.pos = Some(pos);
        if let Some(capture) = &mut self.capture {
            capture.last = pos;
            let node = capture.node.clone();
            // Hover keeps deriving normally underneath the gesture
            // (`contracts/interaction-state.md` §1 defines hover as the hit
            // test's answer and carves out no exception for capture), which
            // is what keeps `hovered` and `captured` two separate readings of
            // one pointer rather than one reading with a special case in it.
            self.hovered = self.derive_hover(frame, surfaces);
            let ended = self.end_of_gesture(frame, event, pos);
            self.refresh_pressed(frame);
            return PointerRouting {
                outcome: RouteOutcome {
                    // Unconditional, ahead of both `hit_test` and the modal
                    // swallow test: the holder of the capture receives this
                    // wherever the pointer is.
                    route: Route::Pointer { node },
                    // A gesture owns the pointer, so a press inside it is not
                    // a click outside a popover — it is part of the drag, and
                    // dismissing on it would close the surface the drag is
                    // happening in.
                    dismiss: Vec::new(),
                },
                ended,
            };
        }

        let swallowed = outside_an_open_modal(frame, pos, surfaces);
        self.hovered = self.derive_hover(frame, surfaces);
        let mut outcome = route_with_surfaces(frame, focused, event, surfaces);
        if let InputEvent::PointerPressed { button, .. } = event
            && !swallowed
            && let Some(hit) = hit_test(frame, pos, Interaction::Drag)
        {
            let node = hit.id.clone();
            self.capture = Some(Capture {
                node: node.clone(),
                button: *button,
                origin: pos,
                last: pos,
            });
            outcome.route = Route::Pointer { node };
        }
        // Which node the primary button went down on, recorded *after* the
        // grant above so a press that opened a gesture records nothing —
        // the capture owns that press, and `pressed()` reads the holder.
        // Only the primary: a right-click does not press a control.
        match event {
            InputEvent::PointerPressed {
                button: PointerButton::Primary,
                ..
            } if self.capture.is_none() => self.pressed_on = self.hovered.clone(),
            InputEvent::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => self.pressed_on = None,
            _ => {}
        }
        // Unconditional: with nothing captured this clears the bit, which is
        // the right answer and one branch fewer than asking first.
        self.refresh_pressed(frame);
        PointerRouting {
            outcome,
            ended: None,
        }
    }

    /// Recompute whether the capture in force is pressed, against `frame`.
    ///
    /// Both halves of the answer move independently: the pointer moves, and
    /// so does the rect underneath it, so this is re-asked after every
    /// positional event *and* after every newly placed frame.
    fn refresh_pressed(&mut self, frame: &PetrifiedFrame) {
        let inside = |id: &str, pos: Point| {
            frame
                .placement(id)
                .is_some_and(|p| p.rect.contains(pos) && p.clip.contains(pos))
        };
        self.pressed = match (&self.capture, self.pos) {
            (Some(capture), Some(pos)) => inside(&capture.node, pos),
            // Nothing captured: the node the press went down on is pressed
            // for as long as the pointer is still on it. Both halves are
            // needed. `hovered` is re-derived immediately before every call
            // to this function, so comparing against it is what stops a
            // press sliding onto a neighbour and lighting that up instead;
            // the rect check is what makes a node that moved out from under
            // a stationary held pointer stop looking pressed.
            (None, Some(pos)) => self
                .pressed_on
                .as_deref()
                .is_some_and(|id| self.hovered.as_deref() == Some(id) && inside(id, pos)),
            _ => false,
        };
    }

    /// Whether `event` closes the capture in force, and how.
    ///
    /// Only the button that opened the gesture closes it: a second button
    /// going down or coming up mid-drag routes to the holder as an ordinary
    /// press or release and starts no second gesture (FR-015 — `capture` is
    /// one `Option`, so there is nowhere for a second one to go).
    ///
    /// A release **inside** the captured node completes; a release
    /// **outside** it cancels as [`CancelReason::Left`]. That is the whole of
    /// "a release outside a pressed button does not activate it": the holder
    /// still hears the release, and it hears immediately afterwards that the
    /// gesture did not complete, which is the only signal a component may
    /// treat as activation.
    fn end_of_gesture(
        &mut self,
        frame: &PetrifiedFrame,
        event: &InputEvent,
        pos: Point,
    ) -> Option<GestureEnd> {
        let capture = self.capture.as_ref()?;
        let InputEvent::PointerReleased { button, .. } = event else {
            return None;
        };
        if *button != capture.button {
            return None;
        }
        let inside = frame
            .placement(&capture.node)
            .is_some_and(|p| p.rect.contains(pos) && p.clip.contains(pos));
        let outcome = if inside {
            GestureOutcome::Completed
        } else {
            GestureOutcome::Cancelled(CancelReason::Left)
        };
        self.pressed = false;
        let node = self.capture.take()?.node;
        Some(GestureEnd { node, outcome })
    }

    /// Drop the capture in force and report its end, or `None` when nothing
    /// was captured.
    fn end_capture(&mut self, reason: CancelReason) -> Option<GestureEnd> {
        self.pressed = false;
        self.capture.take().map(|capture| GestureEnd {
            node: capture.node,
            outcome: GestureOutcome::Cancelled(reason),
        })
    }

    /// The one node under the pointer that accepts hover, or `None`.
    ///
    /// A position behind an open `Block` surface is nowhere: a modal makes
    /// the screen behind it inert, and a control that lit up under a pointer
    /// that cannot click it would be advertising an interaction the router
    /// refuses.
    fn derive_hover(
        &self,
        frame: &PetrifiedFrame,
        surfaces: &BTreeMap<String, InputPolicy>,
    ) -> Option<String> {
        let pos = self.pos?;
        if outside_an_open_modal(frame, pos, surfaces) {
            return None;
        }
        let floor = block_floor(frame, pos, surfaces);
        hit_test_above(frame, pos, Interaction::Hover, floor.map(|p| p.id.as_str()))
            .map(|hit| hit.id.clone())
    }
}

/// [`route`]'s outcome, plus every dismissal a `DismissOutside` surface
/// requests.
///
/// The engine is retained and the host owns the view tree
/// (`overlay_surface.rs`'s module doc), so "dismiss" cannot mean the engine
/// deleting a node — it can only mean *reporting* the request, the same way
/// [`Route::Unrouted`] reports a drop instead of silently discarding the
/// event. A host decides what closing the surface means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteOutcome {
    /// Where the event landed, or why it did not.
    pub route: Route,
    /// Canonical ids of every `DismissOutside` surface a `PointerPressed` in
    /// this event landed outside of. Always empty for every other event
    /// kind — only a press dismisses, matching [`activates`]'s convention
    /// that the press, not the release, is where an action fires.
    pub dismiss: Vec<String>,
}

/// [`route`], but aware of every `surface` node's declared [`InputPolicy`]
/// (`surfaces`: the same map [`crate::layout::overlay_surface::surface_scopes`]
/// builds, keyed by canonical placement id).
///
/// [`route`] alone has no notion of a surface's input policy at all — every
/// placement is reached purely by z-order and its own declared interactions,
/// which is exactly right for [`InputPolicy::Passthrough`] (nothing changes:
/// a surface with no accepting content already lets input fall through to
/// what is underneath, the same as any other non-accepting placement) but
/// wrong for [`InputPolicy::Block`]: a modal is supposed to make the screen
/// behind it inert, and nothing in [`hit_test`]'s z-order walk stops a click
/// outside the modal from reaching a placement further back. This function
/// adds exactly that: a positional event outside every currently-placed
/// `Block` surface's bounds is swallowed here before [`route`] ever runs.
///
/// Inside a `Block` surface's bounds the surface is *opaque*: the positional
/// hit test runs down to that surface and no further ([`hit_test_above`]),
/// so a press on dialog content that accepts nothing lands nowhere rather
/// than on the page control the dialog is covering. Together the two rules
/// are the whole of "the screen behind a modal is inert": outside the
/// surface the event is swallowed here, inside it the frame ends at the
/// surface. A scrim that covers the window is therefore a `Block` surface
/// like any other — nothing is ever outside it, and nothing behind it is
/// ever reached.
///
/// [`InputPolicy::DismissOutside`] behaves like `Passthrough` for routing —
/// it places no swallow boundary and no floor — and additionally contributes
/// to `RouteOutcome::dismiss`.
#[must_use]
pub fn route_with_surfaces(
    frame: &PetrifiedFrame,
    focused: Option<&str>,
    event: &InputEvent,
    surfaces: &BTreeMap<String, InputPolicy>,
) -> RouteOutcome {
    let dismiss = dismiss_requests(frame, event, surfaces);
    if let Some(pos) = event.pointer_pos()
        && outside_an_open_modal(frame, pos, surfaces)
    {
        return RouteOutcome {
            route: Route::Unrouted {
                reason: "outside every currently-open Block surface's bounds",
            },
            dismiss,
        };
    }
    let floor = event
        .pointer_pos()
        .and_then(|pos| block_floor(frame, pos, surfaces));
    RouteOutcome {
        route: route_above(frame, focused, event, floor.map(|p| p.id.as_str())),
        dismiss,
    }
}

/// The topmost `Block` surface this frame placed whose rect contains `pos`:
/// the floor below which [`hit_test_above`] does not look. `None` when no
/// open `Block` surface contains the position — every placement is then a
/// candidate, exactly as [`route`] alone treats them.
///
/// "Topmost" is paint order, the same order the hit test walks, so with two
/// nested modals the inner one is the floor and the outer one's content is
/// as unreachable as the page.
fn block_floor<'a>(
    frame: &'a PetrifiedFrame,
    pos: Point,
    surfaces: &BTreeMap<String, InputPolicy>,
) -> Option<&'a Placement> {
    frame.paint_order().into_iter().rev().find(|p| {
        surfaces
            .get(&p.id)
            .is_some_and(|policy| blocks_positional_input(*policy))
            && p.rect.contains(pos)
    })
}

/// Every `DismissOutside` surface a `PointerPressed` in `event` lands outside
/// the placed bounds of. Empty for any other event kind, and empty for a
/// surface this frame did not place (nothing to be outside of).
fn dismiss_requests(
    frame: &PetrifiedFrame,
    event: &InputEvent,
    surfaces: &BTreeMap<String, InputPolicy>,
) -> Vec<String> {
    let InputEvent::PointerPressed { pos, .. } = event else {
        return Vec::new();
    };
    surfaces
        .iter()
        .filter(|(_, policy)| **policy == InputPolicy::DismissOutside)
        .filter_map(|(id, _)| frame.placement(id).map(|p| (id, p)))
        .filter(|(_, placement)| !placement.rect.contains(*pos))
        .map(|(id, _)| id.clone())
        .collect()
}

/// Whether `pos` falls outside the bounds of any surface this frame placed
/// whose policy [`blocks_positional_input`]. With more than one open (nested
/// modals), `pos` must be inside every one of them, or the position counts
/// as outside — a click cannot reach the screen behind either.
fn outside_an_open_modal(
    frame: &PetrifiedFrame,
    pos: Point,
    surfaces: &BTreeMap<String, InputPolicy>,
) -> bool {
    surfaces
        .iter()
        .filter(|(_, policy)| blocks_positional_input(**policy))
        .filter_map(|(id, _)| frame.placement(id))
        .any(|surface| !surface.rect.contains(pos))
}

/// Whether a surface declaring `policy` makes its bounds a swallow boundary
/// for positional input.
///
/// Written as an exhaustive match rather than `policy == InputPolicy::Block`
/// so that `Passthrough` and `DismissOutside` are each a named decision —
/// "does not block" — instead of falling out of what `Block` is not; a
/// fourth policy landing later would fail to compile here until someone
/// decided which side of this line it falls on, rather than silently
/// inheriting `false`.
fn blocks_positional_input(policy: InputPolicy) -> bool {
    match policy {
        InputPolicy::Block => true,
        InputPolicy::Passthrough | InputPolicy::DismissOutside => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InputEvent, KeyCode, Modifiers, PointerButton, Route, activates, hit_test,
        required_interaction, route, route_pointer_exit, route_with_surfaces,
    };
    use crate::frame::{
        FrameDigest, PaintState, PetrifiedFrame, Placement, PlacementSemantics, TransitionActivity,
        Viewport, digest,
    };
    use crate::geom::{Point, Rect, Size};
    use crate::token::ThemeMode;
    use crate::tree::{InputPolicy, Interaction, NodeKind};
    use std::collections::BTreeMap;

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

    // -- route_with_surfaces: the three input policies. --------------------

    /// A frame with one plain clickable backdrop under one surface: the
    /// surface itself carries no accepting interaction (the common shape —
    /// its content, not the surface node, accepts clicks), so every case
    /// below turns on whether `surfaces`' declared policy stops a click
    /// outside the surface from reaching the backdrop underneath.
    fn modal_scenario() -> PetrifiedFrame {
        frame(vec![
            node(
                "/backdrop",
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Click],
            ),
            node("/surface", Rect::new(50.0, 50.0, 50.0, 50.0), 5, &[]),
        ])
    }

    fn press(pos: Point) -> InputEvent {
        InputEvent::PointerPressed {
            pos,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    fn policy_map(policy: InputPolicy) -> BTreeMap<String, InputPolicy> {
        let mut m = BTreeMap::new();
        m.insert("/surface".to_owned(), policy);
        m
    }

    /// `Block` swallows a click landing outside the surface's own bounds:
    /// without the swallow check, this click would fall through to
    /// `/backdrop` exactly as [`route`] alone lets it. Deleting the swallow
    /// check in `route_with_surfaces` turns this assertion false.
    #[test]
    fn block_swallows_a_click_outside_its_own_bounds() {
        let f = modal_scenario();
        let surfaces = policy_map(InputPolicy::Block);
        let outcome = route_with_surfaces(&f, None, &press(Point::new(10.0, 10.0)), &surfaces);
        assert!(
            matches!(outcome.route, Route::Unrouted { .. }),
            "a click outside the modal must not reach /backdrop, got {:?}",
            outcome.route
        );
        assert!(outcome.dismiss.is_empty(), "Block never dismisses");
    }

    /// `Block` is opaque inside its own bounds: a click the surface's
    /// content does not take stops at the surface and never reaches
    /// `/backdrop` behind it. Until 2026-09-04 this test asserted the
    /// opposite — that such a click "routes normally" through to the
    /// backdrop — on the argument that anything else would be a crude
    /// z-order swallow. It was not crude, it was the missing half of the
    /// policy: a dialog whose body text let clicks through to the page
    /// button under it was never inert, and a scrim covering the window
    /// (nothing is outside it) would have blocked nothing at all.
    #[test]
    fn block_is_opaque_to_a_click_inside_its_bounds_that_its_content_does_not_take() {
        let f = modal_scenario();
        let surfaces = policy_map(InputPolicy::Block);
        let outcome = route_with_surfaces(&f, None, &press(Point::new(60.0, 60.0)), &surfaces);
        assert!(
            matches!(outcome.route, Route::Unrouted { .. }),
            "a click inside the modal on nothing that accepts it must not reach \
             /backdrop behind the modal, got {:?}",
            outcome.route
        );
        assert!(outcome.dismiss.is_empty(), "Block never dismisses");
    }

    /// The floor is "nothing *behind* the surface", not "nothing at its z":
    /// the surface's own content, above it in paint order, is reached, and
    /// so is the surface itself when it declares an interaction (a scrim
    /// absorbing clicks).
    #[test]
    fn block_routes_a_click_inside_its_bounds_to_its_own_content_or_itself() {
        let surfaces = policy_map(InputPolicy::Block);
        let with_content = frame(vec![
            node(
                "/backdrop",
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Click],
            ),
            node("/surface", Rect::new(50.0, 50.0, 50.0, 50.0), 5, &[]),
            node(
                "/surface/ok",
                Rect::new(60.0, 60.0, 20.0, 20.0),
                5,
                &[Interaction::Click],
            ),
        ]);
        let on_content = route_with_surfaces(
            &with_content,
            None,
            &press(Point::new(65.0, 65.0)),
            &surfaces,
        );
        assert_eq!(
            on_content.route,
            Route::Pointer {
                node: "/surface/ok".into()
            }
        );

        let absorbing = frame(vec![
            node(
                "/backdrop",
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Click],
            ),
            node(
                "/surface",
                Rect::new(50.0, 50.0, 50.0, 50.0),
                5,
                &[Interaction::Click],
            ),
        ]);
        let on_surface =
            route_with_surfaces(&absorbing, None, &press(Point::new(60.0, 60.0)), &surfaces);
        assert_eq!(
            on_surface.route,
            Route::Pointer {
                node: "/surface".into()
            }
        );
    }

    /// A `Block` surface covering the whole window: nothing is outside it,
    /// so the swallow rule never fires, and the floor is what keeps the
    /// backdrop unreachable. This is the scrim shape `component::modal`
    /// builds.
    #[test]
    fn a_window_covering_block_surface_still_makes_the_backdrop_unreachable() {
        let f = frame(vec![
            node(
                "/backdrop",
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Click],
            ),
            node("/scrim", Rect::new(0.0, 0.0, 200.0, 200.0), 5, &[]),
        ]);
        let mut surfaces = BTreeMap::new();
        surfaces.insert("/scrim".to_owned(), InputPolicy::Block);
        let outcome = route_with_surfaces(&f, None, &press(Point::new(10.0, 10.0)), &surfaces);
        assert!(
            matches!(outcome.route, Route::Unrouted { .. }),
            "got {:?}",
            outcome.route
        );
        assert!(
            hit_test(&f, Point::new(10.0, 10.0), Interaction::Click).is_some(),
            "and it is the floor doing it: the plain hit test still finds /backdrop"
        );
    }

    /// `Passthrough` places no swallow boundary at all: the same
    /// outside-the-surface click that `Block` swallows reaches `/backdrop`
    /// here. This is the behavioural inverse the finding named, proven by
    /// the fact that this test and `block_swallows_a_click_outside_its_own_bounds`
    /// disagree on the identical click.
    #[test]
    fn passthrough_lets_a_click_outside_its_bounds_reach_whats_underneath() {
        let f = modal_scenario();
        let surfaces = policy_map(InputPolicy::Passthrough);
        let outcome = route_with_surfaces(&f, None, &press(Point::new(10.0, 10.0)), &surfaces);
        assert_eq!(
            outcome.route,
            Route::Pointer {
                node: "/backdrop".into()
            }
        );
        assert!(outcome.dismiss.is_empty(), "Passthrough never dismisses");
    }

    /// `DismissOutside` routes exactly like `Passthrough` (no swallow
    /// boundary) but additionally names the surface in `dismiss` when the
    /// press lands outside its bounds.
    #[test]
    fn dismiss_outside_reports_a_press_outside_its_bounds_and_still_routes_through() {
        let f = modal_scenario();
        let surfaces = policy_map(InputPolicy::DismissOutside);
        let outcome = route_with_surfaces(&f, None, &press(Point::new(10.0, 10.0)), &surfaces);
        assert_eq!(
            outcome.route,
            Route::Pointer {
                node: "/backdrop".into()
            }
        );
        assert_eq!(outcome.dismiss, vec!["/surface".to_owned()]);
    }

    /// A press inside a `DismissOutside` surface's own bounds must not
    /// report a dismissal — only an outside press does.
    #[test]
    fn dismiss_outside_does_not_fire_for_a_press_inside_its_bounds() {
        let f = modal_scenario();
        let surfaces = policy_map(InputPolicy::DismissOutside);
        let outcome = route_with_surfaces(&f, None, &press(Point::new(60.0, 60.0)), &surfaces);
        assert!(outcome.dismiss.is_empty());
    }

    /// Only a press dismisses: a pointer move outside a `DismissOutside`
    /// surface must not report one, matching `activates`'s press-only
    /// convention.
    #[test]
    fn dismiss_outside_does_not_fire_for_a_non_press_event() {
        let f = modal_scenario();
        let surfaces = policy_map(InputPolicy::DismissOutside);
        let moved = InputEvent::PointerMoved {
            pos: Point::new(10.0, 10.0),
        };
        let outcome = route_with_surfaces(&f, None, &moved, &surfaces);
        assert!(outcome.dismiss.is_empty());
    }

    /// Two nested `Block` surfaces: a click must land inside *both* to
    /// reach anything, since neither modal's screen may be reached through
    /// the other — and inside both, the inner one is the floor, so only the
    /// inner dialog's own content is reachable, never the outer dialog's
    /// and never the page.
    #[test]
    fn nested_block_surfaces_require_being_inside_both() {
        let f = frame(vec![
            node(
                "/backdrop",
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Click],
            ),
            node("/outer", Rect::new(20.0, 20.0, 150.0, 150.0), 5, &[]),
            node(
                "/outer/ok",
                Rect::new(20.0, 20.0, 150.0, 150.0),
                5,
                &[Interaction::Click],
            ),
            node("/inner", Rect::new(60.0, 60.0, 40.0, 40.0), 10, &[]),
            node(
                "/inner/ok",
                Rect::new(60.0, 60.0, 10.0, 10.0),
                10,
                &[Interaction::Click],
            ),
        ]);
        let mut surfaces = BTreeMap::new();
        surfaces.insert("/outer".to_owned(), InputPolicy::Block);
        surfaces.insert("/inner".to_owned(), InputPolicy::Block);

        // Inside /outer but outside /inner: still blocked.
        let between = route_with_surfaces(&f, None, &press(Point::new(30.0, 30.0)), &surfaces);
        assert!(matches!(between.route, Route::Unrouted { .. }));

        // Inside both, on the inner dialog's own control: reaches it.
        let inner_ok = route_with_surfaces(&f, None, &press(Point::new(65.0, 65.0)), &surfaces);
        assert_eq!(
            inner_ok.route,
            Route::Pointer {
                node: "/inner/ok".into()
            }
        );

        // Inside both, on nothing the inner dialog accepts: the outer
        // dialog's full-size control is right there behind it and is not
        // reached, because the inner surface is the floor.
        let inside = route_with_surfaces(&f, None, &press(Point::new(90.0, 90.0)), &surfaces);
        assert!(
            matches!(inside.route, Route::Unrouted { .. }),
            "got {:?}",
            inside.route
        );
        assert_ne!(
            inside.route,
            Route::Pointer {
                node: "/outer/ok".into()
            }
        );
    }

    // -- Pointer-exit is aimed by hover, never by focus. --------------------

    /// The regression the [`InputEvent::PointerLeft`] special case exists for.
    ///
    /// Pointer-exit carries no position, so before the special case it fell
    /// past [`route`]'s positional branch into the focus branch and told the
    /// focused node the pointer had left it — a node the pointer may never
    /// have been over at all.
    #[test]
    fn pointer_left_is_never_delivered_to_the_focused_node() {
        let f = frame(vec![node(
            "/field",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[Interaction::Hover, Interaction::Key],
        )]);
        let outcome = route(&f, Some("/field"), &InputEvent::PointerLeft);
        assert!(
            !matches!(outcome, Route::Keyboard { .. }),
            "pointer-exit reached the focus branch: {outcome:?}"
        );
        assert!(
            matches!(outcome, Route::Unrouted { .. }),
            "`route` tracks no hovered node, so it has nowhere to aim a \
             pointer-exit and must say the event was dropped: {outcome:?}"
        );
    }

    /// The other half of the same rule: the node the pointer was actually
    /// over does hear the exit, once a caller can name it.
    #[test]
    fn pointer_left_lands_on_the_hovered_node_when_the_caller_tracks_one() {
        let f = frame(vec![
            node(
                "/btn",
                Rect::new(0.0, 0.0, 100.0, 100.0),
                0,
                &[Interaction::Hover, Interaction::Click],
            ),
            node(
                "/field",
                Rect::new(0.0, 150.0, 100.0, 20.0),
                0,
                &[Interaction::Key],
            ),
        ]);
        assert_eq!(
            route_pointer_exit(&f, Some("/btn")),
            Route::Pointer {
                node: "/btn".into()
            }
        );
    }

    /// A hovered node that this frame no longer places cannot be told
    /// anything, and the drop is reported rather than swallowed.
    #[test]
    fn pointer_left_is_dropped_when_the_hovered_node_left_the_frame() {
        let f = frame(vec![node(
            "/btn",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[Interaction::Hover],
        )]);
        assert!(matches!(
            route_pointer_exit(&f, Some("/gone")),
            Route::Unrouted { .. }
        ));
    }

    /// Pointer-exit is a hover event: a node that never declared
    /// [`Interaction::Hover`] has no hover state to be told about.
    #[test]
    fn pointer_left_is_dropped_when_the_hovered_node_does_not_accept_hover() {
        let f = frame(vec![node(
            "/btn",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[Interaction::Click],
        )]);
        assert!(matches!(
            route_pointer_exit(&f, Some("/btn")),
            Route::Unrouted { .. }
        ));
    }
}
