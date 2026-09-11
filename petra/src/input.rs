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
use crate::keymap::chord::Chord;
use crate::keymap::reserved::ReservedChords;
use crate::tree::{InputPolicy, Interaction, NodeKind};

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

/// One end of a selection: a byte offset into one text node's painted string.
///
/// Byte offsets, not char offsets, because [`str`] slices on bytes; the char
/// offsets a shaper deals in are converted at the shaper.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextPoint {
    /// Canonical id of the text node this end sits in.
    pub node: String,
    /// Byte offset into that node's own painted string.
    pub offset: usize,
}

impl TextPoint {
    /// A point at `offset` in `node`.
    #[must_use]
    pub fn new(node: impl Into<String>, offset: usize) -> Self {
        Self {
            node: node.into(),
            offset,
        }
    }
}

/// A stretch of the frame's running text the operator has selected, from one
/// point in one node to another point in another.
///
/// # Why this is interaction state and not a tree field
///
/// A selection is made with the pointer, over glyphs the engine cannot see:
/// the mapping from a window position to a byte offset needs shaped text, and
/// `contracts/view-tree.md`'s U-09 boundary puts shaping in the host. So the
/// **host** derives this, exactly as it derives [`Capture`],
/// [`crate::layout::LayoutState::hovered`] and the focused id, and hands it
/// to the engine through [`crate::layout::LayoutState`]
/// (`contracts/interaction-state.md` §1: interaction state has one owner and
/// it is not the application).
///
/// It reaches the picture at paint time and never at measure time
/// ([`crate::frame::PaintContent::selection`]). A container that got taller
/// because three words were selected would re-lay-out the page under the
/// pointer that is selecting them, which is the same rule `hovered` is under
/// and for the same reason.
///
/// # Why two nodes and not one
///
/// It held one node and a byte pair until 2026-09-06. The operator, dragging
/// down a nested list in the catalog:
///
/// > *"when I try to select text on the list I can only select 1 element. A
/// > lot are like this. [...] I cant high light across elements at all"*
///
/// One node is the wrong shape for the thing being modelled. What the
/// operator sees is a *document* — a page of running text that happens to be
/// assembled out of forty separate runs — and a selection in a document runs
/// from a point to a point, crossing whatever lies between. Modelling it as
/// one node made the run boundary, which is an implementation detail of how
/// the page was built, into a wall the operator could see.
///
/// The order "between" is measured in is placement order, which is the
/// pre-order walk of the tree and therefore reading order
/// ([`crate::layout::selection::resolve`] is where the span becomes ranges).
///
/// # Anchor and focus, not start and end
///
/// `anchor` is where the button went down and `focus` is where the pointer is
/// now, so the focus may sit *before* the anchor in the document for a drag
/// that went up the page. Keeping the direction is what lets a drag reverse
/// through its own start without the selection collapsing and re-growing the
/// other way, and it is the pair every text editor keeps.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextSelection {
    /// Where the gesture started.
    pub anchor: TextPoint,
    /// Where the gesture is now.
    pub focus: TextPoint,
}

impl TextSelection {
    /// A fresh, empty selection anchored at `at` in `node`.
    #[must_use]
    pub fn new(node: impl Into<String>, at: usize) -> Self {
        let point = TextPoint::new(node, at);
        Self {
            anchor: point.clone(),
            focus: point,
        }
    }

    /// Move the focus end to `offset` in `node`, keeping the anchor.
    pub fn extend_to(&mut self, node: impl Into<String>, offset: usize) {
        self.focus = TextPoint::new(node, offset);
    }

    /// Whether this selection covers no bytes — a press with no drag.
    ///
    /// An empty selection is kept rather than dropped: it is the anchor a
    /// drag that has not moved yet will grow from, and dropping it would
    /// make the first pointer move of every gesture start from nowhere.
    ///
    /// Only exact when the two ends name the same node. Two ends in two
    /// nodes with nothing selectable between them also cover no bytes, and
    /// that answer belongs to [`crate::layout::selection::resolve`], which is
    /// the only code that knows what the frame actually placed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }
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
    /// The operator asked for the selection to go on the clipboard.
    ///
    /// # Why this is an event and not a chord
    ///
    /// Because that is what the platform sends. `egui-winit` recognises the
    /// copy command in its own key handler and pushes `egui::Event::Copy`,
    /// **returning without emitting the keystroke at all** — so a host
    /// watching for "the C key with control held" watches for something no
    /// keyboard on any platform produces. That is not a hypothetical: this
    /// crate's copy chord was written as a key match, every test that drove
    /// it synthesized the keystroke directly, and the operator found it dead
    /// on the real window: *"I can't copy and paste from lists, I suspect I
    /// cant control c in more places too."*
    ///
    /// Reconstructing the chord instead would also be a lie in two of the
    /// three ways it can arrive. The command is Cmd+C on macOS and Ctrl+C
    /// elsewhere; there is a dedicated `Copy` key on some keyboards that
    /// carries no modifier at all; and an Edit ▸ Copy menu item is a copy
    /// with no keystroke behind it. One intent, several devices, so the
    /// vocabulary names the intent. Text is in this file for the mirror
    /// image of the same reason ([`InputEvent::Text`]).
    ///
    /// It is aimed by neither position nor focus. The selection this copies
    /// is host-derived state that spans whatever runs it spans
    /// ([`TextSelection`]), so there is no one node to route it to.
    Copy,
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
    /// Delivered verbatim, ahead of any binding table, because focus sits
    /// inside a node that declared a raw claim (spec 010 FR-019, FR-020).
    ///
    /// **Only [`route_with_reserved`] can produce this variant.** The plain
    /// [`route`] never does — see its doc comment for why that split is the
    /// whole of FR-021's guarantee, not an implementation detail of it.
    Raw {
        /// Canonical id of the node that claimed raw delivery.
        node: String,
    },
    /// The event's chord is in the reserved set and the shell handles it,
    /// no exceptions — not even a raw claim (spec 010 FR-021). Checked
    /// before everything else in [`route_with_reserved`], including before
    /// the raw-claim check, so a raw claim can never see it and can never
    /// trap the operator behind it.
    ///
    /// Like [`Route::Raw`], only [`route_with_reserved`] can produce this.
    Reserved {
        /// The chord that matched the reserved set.
        chord: Chord,
    },
    /// Nothing accepted it: no node under the pointer accepts this event kind,
    /// or nothing has focus. The event is dropped, and the caller can say so —
    /// a silent drop is how a "the click did nothing" bug hides.
    Unrouted {
        /// Why, in one phrase, for the driver's error text. Most reasons are
        /// fixed strings (`Cow::Borrowed`); a reason naming a specific node —
        /// [`route_keyboard_from`]'s Block-boundary stop (spec 010 T011) is
        /// the one case today — builds one at the call site
        /// (`Cow::Owned`), because FR-024 requires naming *which* node
        /// swallowed the event, not just that one did.
        reason: std::borrow::Cow<'static, str>,
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
///
/// A press or release also splits on which button produced it (spec 009
/// T014): [`PointerButton::Secondary`] asks for
/// [`Interaction::SecondaryClick`] and everything else asks for
/// [`Interaction::Click`]. A node that declares only one of the two is
/// reachable by only that button's press — a plain button (`Click` alone)
/// does not become a right-click target, and a context menu's trigger
/// (`SecondaryClick` alone, or both) does not start absorbing left clicks it
/// never asked for.
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
        InputEvent::PointerPressed { button, .. } | InputEvent::PointerReleased { button, .. } => {
            Some(match button {
                PointerButton::Secondary => Interaction::SecondaryClick,
                PointerButton::Primary | PointerButton::Middle => Interaction::Click,
            })
        }
        InputEvent::Scroll { .. } => Some(Interaction::Scroll),
        InputEvent::Key { .. } => Some(Interaction::Key),
        InputEvent::Text(_) => Some(Interaction::TextEdit),
        // A gesture end is the last event of a drag and reports on one; the
        // node it names declared `Drag` to have been given the capture, so
        // that is the interaction it is heard under.
        InputEvent::GestureEnded { .. } => Some(Interaction::Drag),
        // Neither is aimed at a node, so neither asks a node to declare
        // anything. A copy reads a selection that may span twenty runs and
        // belongs to none of them.
        InputEvent::WindowFocused | InputEvent::WindowBlurred | InputEvent::Copy => None,
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
    route_above(frame, focused, event, None, None)
}

/// [`route`], but able to return [`Route::Raw`] and [`Route::Reserved`] —
/// the **only** function that can (spec 010 FR-005). `route` itself never
/// does; see its lack of a `reserved` parameter as the whole of the
/// guarantee, not an oversight.
///
/// # Why a second function and not a parameter on `route`
///
/// Spec 010 FR-021 requires an empty reserved set to be unrepresentable, and
/// [`ReservedChords::new`] already refuses to build one. That guarantee is
/// worth nothing if a caller can still reach verbatim delivery without a
/// reserved set at all — which is exactly what an optional
/// `reserved: Option<&ReservedChords>` parameter on `route` would allow:
/// pass `None`, get `Raw` anyway. Two functions instead make "raw pass-through
/// is unreachable unless the caller has supplied the escape hatch" a fact a
/// caller reads off which function they called, not a rule they have to
/// remember to uphold. **Do not collapse `route` and `route_with_reserved`
/// into one signature.** A later reader who "simplifies" them away deletes
/// this guarantee: it becomes reachable by passing `None`, precisely what the
/// split exists to prevent.
///
/// # Order
///
/// Spec 010's interpretation order, §"The interpretation order":
///
/// 1. **The event is a key press whose chord is in `reserved`** →
///    [`Route::Reserved`]. Checked first, ahead of the raw-claim check,
///    so the reserved chord always escapes a raw claim (FR-021) — nothing
///    below this can shadow it. This is the whole of US-6: a raw claim
///    takes every key except this one, on purpose.
/// 2. **Focus is inside a node holding a raw claim** — the focused node
///    itself, or any ancestor, walked the same way
///    [`route_keyboard_from`] climbs for an accepting ancestor — and the
///    event is keystroke-shaped (`Key` or `Text`; a raw claim is a
///    declaration about *keystrokes*, and positional events are routed by
///    hit test regardless of focus, per FR-006) → [`Route::Raw`], verbatim,
///    no binding table consulted (FR-020).
/// 3. **Otherwise**, delegate to the same routing [`route`] uses.
///
/// # FR-005
///
/// This is the only place in the crate that decides an event is raw. There
/// is deliberately no standalone "is a raw claim focused" query anywhere —
/// two places to ask is one place to forget, and a later helper answering
/// that question on its own would be exactly the kind of second place FR-005
/// forbids.
#[must_use]
pub fn route_with_reserved(
    frame: &PetrifiedFrame,
    focused: Option<&str>,
    event: &InputEvent,
    reserved: &ReservedChords,
) -> Route {
    reserved_or_raw(frame, focused, event, reserved)
        .unwrap_or_else(|| route_above(frame, focused, event, None, None))
}

/// The one place that decides an event is reserved or raw (FR-005).
///
/// Both [`route_with_reserved`] and [`route_with_surfaces`] ask here rather
/// than each carrying its own copy of the rule. `None` means neither applies
/// and the caller's ordinary routing runs.
///
/// The order is not negotiable: reserved is settled BEFORE raw, and before
/// the frame is touched at all. A raw claim that could swallow the reserved
/// chord is the trap the reserved set exists to prevent.
fn reserved_or_raw(
    frame: &PetrifiedFrame,
    focused: Option<&str>,
    event: &InputEvent,
    reserved: &ReservedChords,
) -> Option<Route> {
    if let InputEvent::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    {
        let chord = Chord {
            key: *key,
            modifiers: *modifiers,
        };
        if reserved.contains(chord) {
            return Some(Route::Reserved { chord });
        }
    }
    if matches!(event, InputEvent::Key { .. } | InputEvent::Text(_))
        && let Some(id) = focused
        && let Some(start) = frame.placements.iter().position(|p| p.id == id)
        && let Some(claimant) = ancestors_from(frame, start).find(|p| p.semantics.raw_claim)
    {
        // The claiming node, not necessarily `id` itself: focus may sit on a
        // descendant of the node that actually declared the raw claim (the
        // "OR any ancestor" half of the rule), and verbatim delivery goes to
        // the claimant — the entity a raw claim exists to feed — not to
        // whichever inner node the engine happens to consider focused.
        return Some(Route::Raw {
            node: claimant.id.clone(),
        });
    }
    None
}

/// [`route`], with the positional branch hit-testing above `floor`
/// ([`hit_test_above`]) and the keyboard branch's ancestor walk stopping at
/// a `Block` surface named in `surfaces` (spec 010 T011). [`route_with_surfaces`]
/// is the caller that has both to pass; [`route`] passes neither.
fn route_above(
    frame: &PetrifiedFrame,
    focused: Option<&str>,
    event: &InputEvent,
    floor: Option<&str>,
    surfaces: Option<&BTreeMap<String, InputPolicy>>,
) -> Route {
    if matches!(event, InputEvent::PointerLeft) {
        return route_pointer_exit(frame, None);
    }
    if let InputEvent::GestureEnded { node, .. } = event {
        return Route::Pointer { node: node.clone() };
    }
    let Some(interaction) = required_interaction(event) else {
        return Route::Unrouted {
            reason: "window-level event, delivered to no node".into(),
        };
    };
    if let Some(pos) = event.pointer_pos() {
        return match hit_test_above(frame, pos, interaction, floor) {
            Some(hit) => Route::Pointer {
                node: hit.id.clone(),
            },
            None => Route::Unrouted {
                reason: "no node under the pointer accepts this event".into(),
            },
        };
    }
    let Some(id) = focused else {
        return Route::Unrouted {
            reason: "nothing has focus".into(),
        };
    };
    let Some(start) = frame.placements.iter().position(|p| p.id == id) else {
        return Route::Unrouted {
            reason: "the focused node is not in this frame".into(),
        };
    };
    route_keyboard_from(frame, start, interaction, event, surfaces)
}

/// Walk from `start` up [`Placement::parent`] and deliver to the first node
/// that accepts, the way [`hit_test_above`] naturally does for the pointer by
/// scanning paint order in reverse — a modal's focused text field does not
/// declare [`Interaction::Key`], so without this walk Escape can never reach
/// the modal that contains it.
///
/// A disabled node is skipped, not treated as a stop: the walk continues past
/// it to whatever is above. The [`activates`] fallback from
/// [`Interaction::Click`] applies at every level climbed, not only at
/// `start`, so Enter or Space still works an ancestor that only declares
/// `Click`.
///
/// # The `Block` ceiling (spec 010 T011)
///
/// A modal is supposed to make the screen behind it inert
/// (`route_with_surfaces`'s doc comment), and that has to hold for the
/// keyboard just as much as for the pointer: before this, a keystroke that
/// nothing *inside* an open modal accepted climbed straight past it to
/// whatever the modal happened to be nested under, because this walk did not
/// know surfaces existed. `surfaces` closes that: if `target` is itself a
/// currently-open `Block` surface and it does not accept, the walk stops
/// there — the surface is the last thing asked, exactly the way
/// [`block_floor`] makes it the last thing a pointer hit test reaches — and
/// reports which surface stopped it, rather than either silently climbing
/// past it (the bug) or silently saying nothing (FR-024). `DismissOutside`
/// and `Passthrough` are not a ceiling here for the same reason they are not
/// a floor in [`block_floor`]: [`blocks_input`] is the one predicate both
/// walks ask.
///
/// `surfaces` is `None` for [`route`] and [`route_with_reserved`], neither of
/// which has a surface map to consult; the walk then behaves exactly as it
/// did before this section existed.
///
/// The walk itself, and its bound, live in [`ancestors_from`].
fn route_keyboard_from(
    frame: &PetrifiedFrame,
    start: usize,
    interaction: Interaction,
    event: &InputEvent,
    surfaces: Option<&BTreeMap<String, InputPolicy>>,
) -> Route {
    for target in ancestors_from(frame, start) {
        if !target.semantics.disabled {
            let accepted = target.semantics.actions.contains(&interaction)
                || (activates(event) && target.semantics.actions.contains(&Interaction::Click));
            if accepted {
                return Route::Keyboard {
                    node: target.id.clone(),
                };
            }
        }
        if surfaces.is_some_and(|surfaces| {
            surfaces
                .get(&target.id)
                .is_some_and(|policy| blocks_input(*policy))
        }) {
            return Route::Unrouted {
                reason: format!(
                    "the keyboard walk stopped at Block surface {:?}: nothing from the \
                     focused node up to it accepts this event",
                    target.id
                )
                .into(),
            };
        }
    }
    Route::Unrouted {
        reason: "no node from the focused one up to the root accepts this event".into(),
    }
}

/// Walk from placement index `start` up [`Placement::parent`], yielding each
/// placement climbed. The one ancestor walk in this file: both
/// [`route_keyboard_from`] (looking for the first accepting ancestor) and
/// [`route_with_reserved`] (looking for a raw claim anywhere on the path)
/// climb through here rather than each keeping its own cursor loop.
///
/// Bounded to `placements.len() + 1` steps, the same bound
/// [`route_keyboard_from`] carried before this walk was extracted:
/// `crate::semantic`'s `a_frame_with_a_forward_parent_link_is_reported_not_panicked`
/// already treats a forward or cyclic parent link as a real input shape a
/// malformed frame can carry, and [`crate::focus::mod`]'s `descends_from`
/// bounds its own walk the same way for the same reason — a frame this small
/// cannot have a chain longer than its own placement count without repeating
/// an index.
fn ancestors_from(frame: &PetrifiedFrame, start: usize) -> impl Iterator<Item = &Placement> {
    let placements = &frame.placements;
    let mut cursor = Some(start);
    let mut budget = placements.len() + 1;
    std::iter::from_fn(move || {
        if budget == 0 {
            return None;
        }
        budget -= 1;
        let index = cursor?;
        let target = placements.get(index)?;
        cursor = target.parent;
        Some(target)
    })
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
            reason: "the pointer left the window and no hovered node is tracked".into(),
        };
    };
    let Some(target) = frame.placement(id) else {
        return Route::Unrouted {
            reason: "the hovered node is not in this frame".into(),
        };
    };
    if target.semantics.disabled || !target.semantics.actions.contains(&Interaction::Hover) {
        return Route::Unrouted {
            reason: "the hovered node does not accept pointer-exit".into(),
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
    /// The chords that escape a raw claim, if this state was built with one.
    ///
    /// [`PointerState::route`] is the shell's one input entry — it handles
    /// keyboard events as well as pointer ones — so this is the seam
    /// [`route_with_surfaces`]'s own `reserved` parameter has to reach
    /// through for a real running shell to ever see [`Route::Raw`] or
    /// [`Route::Reserved`]. `None` (the default, and what
    /// [`PointerState::new`] carries) reproduces exactly the behaviour
    /// before this field existed: every call site below passes it straight
    /// through as `reserved_or_raw`'s `None` case, so `Raw` and `Reserved`
    /// stay as unreachable as [`route`] itself makes them (see
    /// [`route_with_reserved`]'s doc comment on why that is a guarantee and
    /// not an accident). [`PointerState::with_reserved`] is the only way to
    /// supply one.
    reserved: Option<ReservedChords>,
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
    /// Nothing under the pointer, nothing captured, and no reserved set —
    /// [`Route::Raw`] and [`Route::Reserved`] stay unreachable through
    /// [`PointerState::route`] until [`PointerState::with_reserved`] builds
    /// one instead.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Nothing under the pointer, nothing captured, and `reserved` as the
    /// set of chords that escape a raw claim.
    ///
    /// This is the one way to make [`PointerState::route`] able to return
    /// [`Route::Raw`] or [`Route::Reserved`] at all (`crate::keymap` mirrors
    /// this same rule at the two-function split between [`route`] and
    /// [`route_with_reserved`]): a `PointerState` built through
    /// [`PointerState::new`] instead can never produce either, no matter
    /// what the frame declares.
    #[must_use]
    pub fn with_reserved(reserved: ReservedChords) -> Self {
        Self {
            reserved: Some(reserved),
            ..Self::default()
        }
    }

    /// The reserved chord set this state was given, if any.
    ///
    /// `None` is the meaningful answer rather than an absence: it says this
    /// state cannot produce [`Route::Raw`] or [`Route::Reserved`] at all. A
    /// host that means to support a terminal and finds `None` here has found
    /// its bug.
    #[must_use]
    pub fn reserved(&self) -> Option<&ReservedChords> {
        self.reserved.as_ref()
    }

    /// Replace the reserved chord set on a state that already exists.
    ///
    /// The peer of [`PointerState::with_reserved`] for the case a host meets
    /// in practice: the state is built once at startup and the operator's
    /// configuration arrives later. It cannot widen the guarantee by
    /// accident, because supplying a set is exactly what the guarantee asks
    /// for; and it cannot narrow it to nothing, because [`ReservedChords`]
    /// refuses to be empty at construction.
    pub fn set_reserved(&mut self, reserved: ReservedChords) -> &mut Self {
        self.reserved = Some(reserved);
        self
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
                    outcome: route_with_surfaces(
                        frame,
                        focused,
                        event,
                        surfaces,
                        self.reserved.as_ref(),
                    ),
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
                            reason: "escape cancelled the gesture holding the pointer".into(),
                        },
                        dismiss: Vec::new(),
                    },
                    ended,
                }
            }
            _ => match event.pointer_pos() {
                Some(pos) => self.route_positional(frame, focused, surfaces, event, pos),
                None => PointerRouting {
                    outcome: route_with_surfaces(
                        frame,
                        focused,
                        event,
                        surfaces,
                        self.reserved.as_ref(),
                    ),
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
        let mut outcome =
            route_with_surfaces(frame, focused, event, surfaces, self.reserved.as_ref());
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
    reserved: Option<&ReservedChords>,
) -> RouteOutcome {
    let dismiss = dismiss_requests(frame, event, surfaces);
    // The same escape-hatch rule [`route`] and [`route_with_reserved`] obey:
    // `Raw` and `Reserved` are reachable only from a caller that supplied the
    // set of chords that escape them. `None` here is exactly this function's
    // previous behaviour, and it is what every pointer-only caller passes.
    //
    // A `Block` surface does NOT gate this. Reserved is settled before the
    // frame is touched, so a modal cannot swallow the chord that exists to
    // escape one, and a raw claim the operator has focused is theirs to type
    // into whether or not a modal is open above something else.
    if let Some(reserved) = reserved
        && let Some(route) = reserved_or_raw(frame, focused, event, reserved)
    {
        return RouteOutcome { route, dismiss };
    }
    if let Some(pos) = event.pointer_pos()
        && outside_an_open_modal(frame, pos, surfaces)
    {
        return RouteOutcome {
            route: Route::Unrouted {
                reason: "outside every currently-open Block surface's bounds".into(),
            },
            dismiss,
        };
    }
    let floor = event
        .pointer_pos()
        .and_then(|pos| block_floor(frame, pos, surfaces));
    RouteOutcome {
        route: route_above(
            frame,
            focused,
            event,
            floor.map(|p| p.id.as_str()),
            Some(surfaces),
        ),
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
            .is_some_and(|policy| blocks_input(*policy))
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

/// The text run a press at `pos` may start a selection in, or `None` when
/// that press belongs to something else.
///
/// # Why this exists beside [`hit_test`] rather than inside it
///
/// Every other pointer gesture is aimed by a *declaration*: a node says
/// [`Interaction::Click`] and the hit test finds it. A selection cannot be
/// declared that way without saying it forty-two times and then saying it
/// again for every component added after. The operator, on the catalog:
///
/// > *"a lot of these text elements are not highlightable, like the lists.
/// > A lot are like this."*
///
/// So **every run is selectable and a control opts out**, which is the
/// operator's own ruling on the polarity — *"it should be opt out not opt
/// in"* — and which fails towards the mistake that gets reported rather than
/// the one that stays silent. [`crate::tree::Semantics::owns_its_text`]
/// carries the opt-out and says who sets it.
///
/// Walking topmost-first, the first placement whose rect and clip contain
/// `pos` and which is one of these three ends the search:
///
/// 1. **A run no control above it has claimed** is the target. "Above it in
///    the tree", not above it in paint order, and the run itself counts: a
///    code well's run declares [`Interaction::Drag`] to win its own capture
///    and is the most selectable thing in the library, while an inline code
///    chip's run is inside a button that copies on a press. A run is any
///    placement [`paints_text`] accepts, which is both kinds that carry a
///    string of their own.
///
/// 2. **A run a control has claimed** ends the search with `None`. A button's
///    label paints *above* its button, so reverse paint order meets the label
///    first. It ends the search rather than skipping, because the press is
///    the control's and the paragraph the button is drawn over is not a
///    second answer to it.
///
/// 3. **Anything else that claims a press** — [`Interaction::Click`] or
///    [`Interaction::Drag`], the exact pair [`PointerState::route`]
///    hit-tests on a press — ends the search with `None`. This is the clause
///    that stops a press on a control's *padding* from reaching the page
///    behind it. It is about the pointer, not about the text: a data table
///    row claims the press and still lets a selection anchor in its cells,
///    because clause 1 answers before this one ever runs.
///
/// Disabled is deliberately **not** consulted, and this is the one place in
/// this module where it is not. A disabled control still owns its own rect as
/// far as the reader is concerned; dragging a highlight across a greyed-out
/// button's label is not a thing any surface does.
///
/// `surfaces` is the map [`route_with_surfaces`] takes, and is used for the
/// same two things: a position outside an open [`InputPolicy::Block`]
/// surface selects nothing, and nothing behind such a surface is reachable.
/// A dialog's body text is selectable; the page text behind the dialog is
/// not.
///
/// # A press that lands on no text at all
///
/// The gap between two paragraphs, the padding inside a card, the empty
/// right-hand half of a list row. This used to answer `None` there, and the
/// press cleared the selection; the operator, on the catalog:
///
/// > *"I have to start my drag basically on top of the text"*
///
/// So a press that reaches no run **snaps to the nearest one inside the
/// deepest container it did land in**, which is what a browser does with a
/// click in a page margin. The scope matters: a press in a card's padding
/// finds that card's text and not the page's, and the walk goes outward one
/// container at a time only while the inner one holds no run at all.
///
/// The reason this was refused before — "the nearest-run rule needs a
/// distance metric over laid-out glyphs, which is on the host's side of the
/// U-09 boundary" — was half right and the wrong half was load-bearing.
/// Choosing the nearest *run* is a distance to a rect, and this engine has
/// every rect. Only the byte offset inside the chosen run needs shaped text,
/// and that stays the host's ([`crate::layout::LayoutState`] carries the
/// answer back). The boundary is where it was; the work simply divides.
///
/// Nearest is measured lexicographically, vertical distance first: the line
/// the pointer is on wins outright, and only then does the horizontal
/// distance choose between the runs on it. Plain Euclidean distance would
/// hand a press at the far right of a short row to the *next* row down,
/// twenty units away, over the row it was actually on.
#[must_use]
pub fn hit_text<'a>(
    frame: &'a PetrifiedFrame,
    pos: Point,
    surfaces: &BTreeMap<String, InputPolicy>,
) -> Option<&'a Placement> {
    if outside_an_open_modal(frame, pos, surfaces) {
        return None;
    }
    let floor = block_floor(frame, pos, surfaces).map(|p| p.id.as_str());
    // The same floor as an index, because a snap has to obey it too: a press
    // in a dialog's empty half must not reach the page's prose underneath.
    let bound = floor.and_then(|id| frame.placements.iter().position(|p| p.id == id));
    let mut order: Vec<usize> = (0..frame.placements.len()).collect();
    // The same order `PetrifiedFrame::paint_order` produces — a stable sort
    // by z — kept as indices because clause 2 walks `parent`, which is an
    // index into `placements` and not into the sorted view.
    order.sort_by_key(|&i| frame.placements[i].z);
    // The innermost container the press did land in, remembered in case no
    // run is hit and the search has to snap.
    let mut scope = None;
    for &index in order.iter().rev() {
        let placement = &frame.placements[index];
        if placement.rect.contains(pos) && placement.clip.contains(pos) {
            // Text first, and the two answers it can give are both final. A
            // run outside every control is the target; a run inside one is
            // the control's, and so is the press — walking on from it would
            // find whatever the control is drawn over.
            if paints_text(frame, index) {
                return (!claimed_by_a_control(frame, index)).then_some(placement);
            }
            if claims_a_press(placement) {
                return None;
            }
            if scope.is_none() {
                scope = Some(index);
            }
        }
        // A `break` and not a `return`: nothing *behind* the floor is
        // reachable, which is what this stops, but the containers already
        // found in front of it are still where a snap may look.
        if floor == Some(placement.id.as_str()) {
            break;
        }
    }
    scope.and_then(|scope| nearest_outward(frame, pos, scope, bound))
}

/// Where a selection drag has got to: the nearest run to `pos` that a
/// control has not claimed.
///
/// The companion to [`hit_text`], and deliberately a *different* question.
/// A press asks "what did the operator mean by pressing here", which a
/// control can answer instead of the text behind it. A move asks only "where
/// has the gesture reached", and that question was settled the moment the
/// button went down — the control under the pointer now is not being pressed
/// and has no claim on the answer.
///
/// So this ignores [`claims_a_press`] entirely and ignores container scope
/// entirely. Dragging down a page and over a button extends the selection
/// through the runs on either side of it, exactly as a browser does with the
/// `user-select: none` a form control carries; the button's own label is
/// skipped because it is claimed, not because the button is under the
/// pointer.
///
/// The modal rules still hold. A drag inside a dialog cannot reach the page
/// behind it, and a position outside every open [`InputPolicy::Block`]
/// surface reaches nothing.
#[must_use]
pub fn reach_text<'a>(
    frame: &'a PetrifiedFrame,
    pos: Point,
    surfaces: &BTreeMap<String, InputPolicy>,
) -> Option<&'a Placement> {
    if outside_an_open_modal(frame, pos, surfaces) {
        return None;
    }
    // Inside an open modal the whole search is confined to it, which is the
    // positional half of what `Block` means. Outside one, the root of the
    // frame is the scope and that is every placement.
    let scope = block_floor(frame, pos, surfaces)
        .and_then(|floor| frame.placements.iter().position(|p| p.id == floor.id));
    match scope {
        Some(scope) => nearest_in_subtree(frame, pos, scope, None),
        None => nearest_among_where(frame, pos, 0..frame.placements.len(), |_| true),
    }
}

/// The nearest selectable run inside `scope`, else inside `scope`'s parent,
/// and so on outward. Stops at the first container that holds one.
///
/// Outward and not straight to the root, because scope is the point: a press
/// in a card's padding means that card. Walking out only when the inner
/// container is empty of text keeps that meaning while still answering for a
/// press in a bare `Spacer`, which holds nothing and never will.
fn nearest_outward(
    frame: &PetrifiedFrame,
    pos: Point,
    scope: usize,
    bound: Option<usize>,
) -> Option<&Placement> {
    let mut cursor = Some(scope);
    while let Some(at) = cursor {
        if let Some(found) = nearest_in_subtree(frame, pos, at, bound) {
            return Some(found);
        }
        // The walk outward stops at the floor. Past it is the page behind a
        // dialog, which is exactly what a `Block` surface exists to make
        // unreachable.
        if Some(at) == bound {
            break;
        }
        cursor = frame.placements.get(at).and_then(|p| p.parent);
    }
    None
}

/// The nearest selectable run in `root`'s subtree, `root` itself included.
fn nearest_in_subtree(
    frame: &PetrifiedFrame,
    pos: Point,
    root: usize,
    bound: Option<usize>,
) -> Option<&Placement> {
    // Placements are pre-order, so a subtree is a contiguous run starting at
    // its own index — nothing before `root` can be inside it.
    nearest_among_where(frame, pos, root..frame.placements.len(), |index| {
        descends_from(frame, index, root)
            && bound.is_none_or(|bound| descends_from(frame, index, bound))
    })
}

/// The nearest selectable run among `range` that also passes `extra`.
///
/// "Nearest" is `(vertical gap, horizontal gap)` compared in that order, so
/// the line the pointer is on beats every other line however far along it the
/// run sits. A run the pointer is inside scores `(0.0, 0.0)` and wins, which
/// is how an exact hit falls out of the same comparison rather than needing a
/// case of its own.
fn nearest_among_where(
    frame: &PetrifiedFrame,
    pos: Point,
    range: std::ops::Range<usize>,
    extra: impl Fn(usize) -> bool,
) -> Option<&Placement> {
    range
        .filter(|&index| paints_text(frame, index) && !claimed_by_a_control(frame, index))
        .filter(|&index| extra(index))
        .map(|index| (gap_to(&frame.placements[index], pos), index))
        .min_by(|(a, _), (b, _)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)))
        .map(|(_, index)| &frame.placements[index])
}

/// The `(vertical, horizontal)` gap between `pos` and a placement's rect and
/// clip. Zero on an axis the position already falls inside.
///
/// The clip counts as well as the rect: a run scrolled out of its pane is
/// still placed, and snapping a press to a run nobody can see would highlight
/// nothing.
fn gap_to(placement: &Placement, pos: Point) -> (f32, f32) {
    let rect = placement.rect;
    let clip = placement.clip;
    let axis = |lo: f32, hi: f32, clip_lo: f32, clip_hi: f32, at: f32| {
        let lo = lo.max(clip_lo);
        let hi = hi.min(clip_hi);
        if hi < lo {
            // Fully clipped away on this axis: unreachable, not merely far.
            return f32::INFINITY;
        }
        (lo - at).max(at - hi).max(0.0)
    };
    (
        axis(rect.y, rect.y + rect.h, clip.y, clip.y + clip.h, pos.y),
        axis(rect.x, rect.x + rect.w, clip.x, clip.x + clip.w, pos.x),
    )
}

/// Whether `index` is `root` or lies inside it.
fn descends_from(frame: &PetrifiedFrame, index: usize, root: usize) -> bool {
    let mut cursor = Some(index);
    while let Some(at) = cursor {
        if at == root {
            return true;
        }
        cursor = frame.placements.get(at).and_then(|p| p.parent);
    }
    false
}

/// Whether the placement at `index` paints a non-empty string of its own.
///
/// Two kinds carry running text. [`NodeKind::Text`] is the obvious one;
/// [`NodeKind::Input`] paints its own value rather than hanging a child run
/// under it, so a field whose text could not be selected was the operator's
/// *"text is not highlightable in form, which is a text entry box"*. The
/// painter draws both through one path (`crate::paint`), so a selection on
/// either lands the same way.
///
/// The emptiness test is what keeps a zero-width run — a blank label, a
/// placeholder that resolved to nothing — from winning a snap it would then
/// highlight nothing in.
pub(crate) fn paints_text(frame: &PetrifiedFrame, index: usize) -> bool {
    paints_text_in(&frame.placements, &frame.content, index)
}

/// [`paints_text`] over a placement list and payload list that are not a
/// frame yet. See [`claimed_by_a_control_in`] for why the pair exists.
pub(crate) fn paints_text_in(
    placements: &[Placement],
    content: &[crate::frame::PaintContent],
    index: usize,
) -> bool {
    let Some(placement) = placements.get(index) else {
        return false;
    };
    if !matches!(placement.kind, NodeKind::Text | NodeKind::Input) {
        return false;
    }
    content
        .get(index)
        .and_then(|content| content.text.as_ref())
        .is_some_and(|text| !text.text.is_empty())
}

/// Whether a press over this placement is this placement's.
///
/// The pair [`PointerState::route`] hit-tests for on a press, in the order it
/// does: `Drag` first so a gesture can start, then `Click`. Written here as
/// one predicate so [`hit_text`] cannot drift from the routing it defers to.
fn claims_a_press(placement: &Placement) -> bool {
    let actions = &placement.semantics.actions;
    actions.contains(&Interaction::Drag) || actions.contains(&Interaction::Click)
}

/// Whether `index`, or anything it is inside, declared
/// [`crate::tree::Semantics::owns_its_text`].
///
/// The walk starts at the node itself and not at its parent, so a leaf that
/// is its own control — an inline code chip's run inside the chip that copies
/// it — answers for itself. Ancestors carry it because a control says it
/// once, on the node that declares the interaction, and means it for its
/// label, its glyph and its state text.
fn claimed_by_a_control(frame: &PetrifiedFrame, index: usize) -> bool {
    claimed_by_a_control_in(&frame.placements, index)
}

/// [`claimed_by_a_control`] over a placement list that is not a frame yet.
///
/// [`crate::layout::selection`] runs before the frame is sealed and holds the
/// list directly. One body rather than two, so the rule the press obeys and
/// the rule the highlight obeys cannot answer differently.
pub(crate) fn claimed_by_a_control_in(placements: &[Placement], index: usize) -> bool {
    let mut cursor = Some(index);
    while let Some(at) = cursor {
        let Some(placement) = placements.get(at) else {
            return false;
        };
        if placement.semantics.owns_its_text {
            return true;
        }
        cursor = placement.parent;
    }
    false
}

/// Whether `pos` falls outside the bounds of any surface this frame placed
/// whose policy [`blocks_input`]. With more than one open (nested
/// modals), `pos` must be inside every one of them, or the position counts
/// as outside — a click cannot reach the screen behind either.
fn outside_an_open_modal(
    frame: &PetrifiedFrame,
    pos: Point,
    surfaces: &BTreeMap<String, InputPolicy>,
) -> bool {
    surfaces
        .iter()
        .filter(|(_, policy)| blocks_input(**policy))
        .filter_map(|(id, _)| frame.placement(id))
        .any(|surface| !surface.rect.contains(pos))
}

/// Whether a surface declaring `policy` makes its bounds a swallow boundary
/// for input — the pointer floor [`block_floor`] computes, and the keyboard
/// ceiling [`route_keyboard_from`]'s walk stops at (spec 010 T011). One
/// predicate for both, so the two walks cannot disagree about what `Block`
/// means.
///
/// Written as an exhaustive match rather than `policy == InputPolicy::Block`
/// so that `Passthrough` and `DismissOutside` are each a named decision —
/// "does not block" — instead of falling out of what `Block` is not; a
/// fourth policy landing later would fail to compile here until someone
/// decided which side of this line it falls on, rather than silently
/// inheriting `false`.
fn blocks_input(policy: InputPolicy) -> bool {
    match policy {
        InputPolicy::Block => true,
        InputPolicy::Passthrough | InputPolicy::DismissOutside => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Chord, InputEvent, KeyCode, Modifiers, PointerButton, PointerState, ReservedChords, Route,
        activates, hit_test, hit_text, reach_text, required_interaction, route, route_pointer_exit,
        route_with_reserved, route_with_surfaces,
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
        // Every run carries a string, because in a real frame one does: the
        // placement pass attaches a `TextPaint` to every `Text` and `Input`
        // node it places, and `hit_text` refuses a run that paints nothing so
        // an empty one cannot win a snap. A fixture of empty payloads would
        // make every selection test pass for the wrong reason.
        let content: Vec<crate::frame::PaintContent> = placements
            .iter()
            .map(|placement| {
                if matches!(placement.kind, NodeKind::Text | NodeKind::Input) {
                    crate::frame::PaintContent {
                        text: Some(crate::frame::placement::TextPaint {
                            text: placement.id.clone(),
                            style: None,
                            wrap: crate::tree::TextWrap::Clip,
                            max_lines: None,
                            runs: Vec::new(),
                        }),
                        ..crate::frame::PaintContent::default()
                    }
                } else {
                    crate::frame::PaintContent::default()
                }
            })
            .collect();
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

    /// The shape every `hit_text` fixture is built from: a real parent chain,
    /// because clause 2 of the rule walks one. `node` alone cannot express
    /// this — every placement it makes is a root.
    fn kid(
        id: &str,
        kind: NodeKind,
        rect: Rect,
        z: i32,
        actions: &[Interaction],
        parent: Option<usize>,
    ) -> Placement {
        Placement {
            kind,
            parent,
            ..node(id, rect, z, actions)
        }
    }

    /// [`kid`] that has declared `Semantics::owns_its_text` — a button, a
    /// tab, a menu item. The opt-out is the *only* thing that makes a run
    /// unselectable now, so a fixture that wants a control to own its label
    /// has to say so exactly as a component does.
    fn control(
        id: &str,
        rect: Rect,
        z: i32,
        actions: &[Interaction],
        parent: Option<usize>,
    ) -> Placement {
        let mut p = kid(id, NodeKind::Stack, rect, z, actions, parent);
        p.semantics.owns_its_text = true;
        p
    }

    /// The whole point of the pass: text nobody declared anything about.
    #[test]
    fn a_run_no_control_covers_is_the_selection_target() {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            kid(
                "/page/item",
                NodeKind::Text,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[],
                Some(0),
            ),
        ]);
        let hit = hit_text(&f, Point::new(20.0, 15.0), &BTreeMap::new()).unwrap();
        assert_eq!(hit.id, "/page/item");
    }

    /// Clause 2, and the reason the rule cannot be "every `Text` placement".
    /// A button's label paints above its button, so reverse paint order meets
    /// the label first; without the ancestor walk every label in the library
    /// would be draggable and the press would never reach its own control.
    #[test]
    fn a_controls_own_label_is_not_selectable() {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            control(
                "/page/btn",
                Rect::new(10.0, 10.0, 100.0, 40.0),
                0,
                &[Interaction::Click],
                Some(0),
            ),
            kid(
                "/page/btn/label",
                NodeKind::Text,
                Rect::new(20.0, 20.0, 60.0, 20.0),
                0,
                &[],
                Some(1),
            ),
        ]);
        assert!(hit_text(&f, Point::new(30.0, 25.0), &BTreeMap::new()).is_none());
    }

    /// Clause 1. The label is skipped and the walk must then *stop*, not
    /// carry on to whatever the button is drawn over — a press on a control
    /// is the control's, and selecting the paragraph behind it would be the
    /// same defect from the other side.
    #[test]
    fn a_press_on_a_control_does_not_select_the_page_behind_it() {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            kid(
                "/page/prose",
                NodeKind::Text,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                Some(0),
            ),
            control(
                "/page/btn",
                Rect::new(10.0, 10.0, 100.0, 40.0),
                5,
                &[Interaction::Click],
                Some(0),
            ),
            kid(
                "/page/btn/label",
                NodeKind::Text,
                Rect::new(20.0, 20.0, 60.0, 20.0),
                5,
                &[],
                Some(2),
            ),
        ]);
        assert!(hit_text(&f, Point::new(30.0, 25.0), &BTreeMap::new()).is_none());
        // …and the same prose one pixel outside the button still selects.
        let hit = hit_text(&f, Point::new(150.0, 25.0), &BTreeMap::new()).unwrap();
        assert_eq!(hit.id, "/page/prose");
    }

    /// A code well's run declares `Drag` to win its own capture. The ancestor
    /// walk starts at the *parent* for exactly this case: the run is the most
    /// selectable thing in the library and it is also the one run that claims
    /// its own press.
    #[test]
    fn a_run_that_drags_itself_is_still_its_own_selection_target() {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            kid(
                "/page/well",
                NodeKind::Surface,
                Rect::new(0.0, 0.0, 200.0, 60.0),
                0,
                &[],
                Some(0),
            ),
            kid(
                "/page/well/code",
                NodeKind::Text,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[Interaction::Drag],
                Some(1),
            ),
        ]);
        let hit = hit_text(&f, Point::new(20.0, 15.0), &BTreeMap::new()).unwrap();
        assert_eq!(hit.id, "/page/well/code");
    }

    /// A scroll pane is an ancestor of nearly every run in the catalog and it
    /// declares an interaction. If the ancestor walk asked "is any ancestor
    /// interactive" rather than "does any ancestor claim a *press*", the
    /// feature would be dead on arrival on every scrolling page.
    #[test]
    fn a_scrolling_pane_over_a_run_does_not_make_it_the_panes_text() {
        let f = frame(vec![
            kid(
                "/pane",
                NodeKind::Scroll,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Scroll],
                None,
            ),
            kid(
                "/pane/item",
                NodeKind::Text,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[],
                Some(0),
            ),
        ]);
        let hit = hit_text(&f, Point::new(20.0, 15.0), &BTreeMap::new()).unwrap();
        assert_eq!(hit.id, "/pane/item");
    }

    /// The floor, the same one `route_with_surfaces` applies: a dialog's body
    /// text is selectable, and the page text the dialog covers is not.
    #[test]
    fn text_behind_an_open_modal_is_out_of_reach() {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            kid(
                "/page/prose",
                NodeKind::Text,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                Some(0),
            ),
            kid(
                "/dialog",
                NodeKind::Surface,
                Rect::new(40.0, 40.0, 120.0, 120.0),
                10,
                &[],
                None,
            ),
            kid(
                "/dialog/body",
                NodeKind::Text,
                Rect::new(50.0, 50.0, 100.0, 20.0),
                10,
                &[],
                Some(2),
            ),
        ]);
        let surfaces = BTreeMap::from([("/dialog".to_owned(), InputPolicy::Block)]);
        let hit = hit_text(&f, Point::new(60.0, 55.0), &surfaces).unwrap();
        assert_eq!(hit.id, "/dialog/body");
        // Inside the dialog's rect but not on its text. The snap answers, and
        // the whole question is *what* it snaps to: the dialog's own body,
        // never the prose underneath, which is behind the floor. Before the
        // snap existed this asserted `is_none()`, which tested the same rule
        // through a `None` that has since stopped being the right answer.
        let below = hit_text(&f, Point::new(60.0, 120.0), &surfaces).expect("snapped");
        assert_eq!(below.id, "/dialog/body");
        // And outside the dialog entirely, which is the swallow rule.
        assert!(hit_text(&f, Point::new(10.0, 10.0), &surfaces).is_none());
    }

    /// The row case, which is the whole reason the opt-out is a declaration
    /// and not the press claim it used to be.
    ///
    /// A data table row claims its own press — that is how a row gets
    /// selected — and its cells still carry text a person wants to copy. The
    /// derived rule this replaced refused both together, so a table was as
    /// unselectable as a button. The operator: *"it should be opt out not opt
    /// in"*.
    #[test]
    fn a_row_that_claims_its_press_still_lends_out_its_cells() {
        let f = frame(vec![
            kid(
                "/table",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            // Declares `Click` and does *not* declare `owns_its_text`.
            kid(
                "/table/row",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 40.0),
                0,
                &[Interaction::Click],
                Some(0),
            ),
            kid(
                "/table/row/cell",
                NodeKind::Text,
                Rect::new(10.0, 10.0, 80.0, 20.0),
                0,
                &[],
                Some(1),
            ),
        ]);
        let hit = hit_text(&f, Point::new(20.0, 15.0), &BTreeMap::new()).unwrap();
        assert_eq!(hit.id, "/table/row/cell");
        // …and the row's own padding is still the row's: a press there is a
        // row selection and not a selection that starts nowhere.
        assert!(hit_text(&f, Point::new(150.0, 20.0), &BTreeMap::new()).is_none());
    }

    /// Clause 1 does not consult `disabled`, and this is the case that says
    /// why: a greyed-out button still owns its own rect, and dragging a
    /// highlight across its label is not something any surface does.
    #[test]
    fn a_disabled_control_still_owns_its_own_rect() {
        let mut disabled = control(
            "/page/btn",
            Rect::new(10.0, 10.0, 100.0, 40.0),
            0,
            &[Interaction::Click],
            Some(0),
        );
        disabled.semantics.disabled = true;
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            disabled,
            kid(
                "/page/btn/label",
                NodeKind::Text,
                Rect::new(20.0, 20.0, 60.0, 20.0),
                0,
                &[],
                Some(1),
            ),
        ]);
        assert!(hit_text(&f, Point::new(30.0, 25.0), &BTreeMap::new()).is_none());
    }

    /// The gap between two paragraphs. This used to answer `None`, and the
    /// operator's report was *"I have to start my drag basically on top of
    /// the text"*, so it snaps to the nearest run in the container the press
    /// did land in.
    #[test]
    fn a_press_beside_the_words_snaps_to_them() {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            kid(
                "/page/top",
                NodeKind::Text,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[],
                Some(0),
            ),
            kid(
                "/page/bottom",
                NodeKind::Text,
                Rect::new(10.0, 100.0, 100.0, 20.0),
                0,
                &[],
                Some(0),
            ),
        ]);
        // Below the first run and well above the second.
        let hit = hit_text(&f, Point::new(20.0, 45.0), &BTreeMap::new()).expect("snapped");
        assert_eq!(hit.id, "/page/top", "35 units up beats 55 units down");
        // Far to the right of the first run, and nearer the second by
        // straight-line distance. The line the pointer is on still wins,
        // which is the whole reason the comparison is lexicographic.
        let hit = hit_text(&f, Point::new(190.0, 20.0), &BTreeMap::new()).expect("snapped");
        assert_eq!(hit.id, "/page/top");
    }

    /// The move's question is not the press's. A drag that has reached a
    /// button extends through it: the runs on either side are selected and
    /// the button's own label is not, which is what a browser's
    /// `user-select: none` does. The press would have answered `None` at the
    /// same position, and correctly — a press *there* is the button's.
    #[test]
    fn a_drag_reaches_past_a_control_the_press_would_have_stopped_at() {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            kid(
                "/page/before",
                NodeKind::Text,
                Rect::new(0.0, 0.0, 200.0, 20.0),
                0,
                &[],
                Some(0),
            ),
            control(
                "/page/btn",
                Rect::new(0.0, 40.0, 200.0, 20.0),
                0,
                &[Interaction::Click],
                Some(0),
            ),
            kid(
                "/page/btn/label",
                NodeKind::Text,
                Rect::new(10.0, 44.0, 60.0, 12.0),
                0,
                &[],
                Some(2),
            ),
            kid(
                "/page/after",
                NodeKind::Text,
                Rect::new(0.0, 80.0, 200.0, 20.0),
                0,
                &[],
                Some(0),
            ),
        ]);
        let on_the_label = Point::new(30.0, 50.0);
        assert!(
            hit_text(&f, on_the_label, &BTreeMap::new()).is_none(),
            "a press on a button's label is the button's"
        );
        let reached = reach_text(&f, on_the_label, &BTreeMap::new()).expect("the drag reaches on");
        assert!(
            reached.id == "/page/before" || reached.id == "/page/after",
            "the drag landed on {}, which is the label it must skip",
            reached.id
        );
        // And a drag that is genuinely over a run takes that run.
        assert_eq!(
            reach_text(&f, Point::new(30.0, 85.0), &BTreeMap::new()).map(|p| p.id.as_str()),
            Some("/page/after")
        );
    }

    /// A snap that has nowhere to land is still no selection: a frame whose
    /// only run is inside a control has nothing to offer a press in the
    /// margin.
    #[test]
    fn a_press_beside_a_page_with_no_lent_text_finds_nothing() {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            control(
                "/page/btn",
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[],
                Some(0),
            ),
            kid(
                "/page/btn/label",
                NodeKind::Text,
                Rect::new(14.0, 14.0, 92.0, 12.0),
                0,
                &[],
                Some(1),
            ),
        ]);
        // The label is the only run on the page, and it is the button's.
        assert!(hit_text(&f, Point::new(20.0, 80.0), &BTreeMap::new()).is_none());
        // And the same press on a page whose one run is lent finds it.
        let lent = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            kid(
                "/page/prose",
                NodeKind::Text,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[],
                Some(0),
            ),
        ]);
        assert_eq!(
            hit_text(&lent, Point::new(20.0, 80.0), &BTreeMap::new()).map(|p| p.id.as_str()),
            Some("/page/prose"),
            "the negative above must be about the claim, not about the geometry"
        );
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
        let reasons: Vec<String> = [
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
            Route::Unrouted { reason } => reason.clone().into_owned(),
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

    /// SC-001: a modal contains a focused text field that declares only
    /// `TextEdit`, and Escape must still close the modal. Without the
    /// ancestor walk the field alone is asked, it does not declare `Key`, and
    /// Escape is silently dropped.
    #[test]
    fn escape_reaches_an_ancestor_from_a_focused_field() {
        let f = frame(vec![
            kid(
                "/modal",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Key],
                None,
            ),
            kid(
                "/modal/field",
                NodeKind::Input,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[Interaction::TextEdit],
                Some(0),
            ),
        ]);
        let escape = InputEvent::Key {
            key: KeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            route(&f, Some("/modal/field"), &escape),
            Route::Keyboard {
                node: "/modal".into()
            }
        );
    }

    /// [`Route::Keyboard`] names the node that accepted the event, which the
    /// caller then delivers to — not the node that merely held focus.
    #[test]
    fn keyboard_route_names_the_accepting_node_not_the_focused_one() {
        let f = frame(vec![
            kid(
                "/panel",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Key],
                None,
            ),
            kid(
                "/panel/field",
                NodeKind::Input,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[Interaction::TextEdit],
                Some(0),
            ),
        ]);
        let key = InputEvent::Key {
            key: KeyCode::Char('q'),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let got = route(&f, Some("/panel/field"), &key);
        assert_eq!(
            got,
            Route::Keyboard {
                node: "/panel".into()
            }
        );
        assert_ne!(
            got,
            Route::Keyboard {
                node: "/panel/field".into()
            }
        );
    }

    /// A disabled ancestor is skipped, not treated as a stop: the walk
    /// continues past it to whatever accepts above.
    #[test]
    fn a_disabled_ancestor_is_skipped_and_the_walk_continues() {
        let mut disabled_mid = kid(
            "/dialog/mid",
            NodeKind::Stack,
            Rect::new(0.0, 0.0, 150.0, 150.0),
            0,
            &[Interaction::Key],
            Some(0),
        );
        disabled_mid.semantics.disabled = true;
        let f = frame(vec![
            kid(
                "/dialog",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Key],
                None,
            ),
            disabled_mid,
            kid(
                "/dialog/mid/field",
                NodeKind::Input,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[Interaction::TextEdit],
                Some(1),
            ),
        ]);
        let key = InputEvent::Key {
            key: KeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            route(&f, Some("/dialog/mid/field"), &key),
            Route::Keyboard {
                node: "/dialog".into()
            }
        );
    }

    /// [`activates`]'s Enter/Space fallback to `Click` applies at every level
    /// climbed, not only at the focused node itself.
    #[test]
    fn activation_fallback_applies_to_ancestors_too() {
        let f = frame(vec![
            kid(
                "/row",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Click],
                None,
            ),
            kid(
                "/row/label",
                NodeKind::Text,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[Interaction::Focus],
                Some(0),
            ),
        ]);
        let enter = InputEvent::Key {
            key: KeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            route(&f, Some("/row/label"), &enter),
            Route::Keyboard {
                node: "/row".into()
            }
        );
    }

    /// The three `Unrouted` causes on the keyboard path each carry their own
    /// sentence: no focus, focus naming a node not in the frame, and a focus
    /// chain that reaches the root without anything accepting.
    #[test]
    fn unrouted_reasons_distinguish_no_focus_from_no_accepting_ancestor() {
        let f = frame(vec![
            kid(
                "/dialog",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                None,
            ),
            kid(
                "/dialog/field",
                NodeKind::Input,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[Interaction::TextEdit],
                Some(0),
            ),
        ]);
        let key = InputEvent::Key {
            key: KeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let no_focus = match route(&f, None, &key) {
            Route::Unrouted { reason } => reason,
            other => panic!("expected Unrouted, got {other:?}"),
        };
        let not_in_frame = match route(&f, Some("/gone"), &key) {
            Route::Unrouted { reason } => reason,
            other => panic!("expected Unrouted, got {other:?}"),
        };
        let no_accepting_ancestor = match route(&f, Some("/dialog/field"), &key) {
            Route::Unrouted { reason } => reason,
            other => panic!("expected Unrouted, got {other:?}"),
        };
        assert_ne!(no_focus, not_in_frame);
        assert_ne!(no_focus, no_accepting_ancestor);
        assert_ne!(not_in_frame, no_accepting_ancestor);

        // Positive control. Three distinct strings stay distinct even if the
        // ancestor walk is deleted, so the asserts above cannot fail for the
        // reason this test is named after. This half can: give the dialog the
        // interaction its child lacks and the same focused field must now
        // route UP to it. Remove the walk and this line fails.
        let accepting = frame(vec![
            kid(
                "/dialog",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Key],
                None,
            ),
            kid(
                "/dialog/field",
                NodeKind::Input,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[Interaction::TextEdit],
                Some(0),
            ),
        ]);
        assert_eq!(
            route(&accepting, Some("/dialog/field"), &key),
            Route::Keyboard {
                node: "/dialog".to_owned()
            },
            "the reason strings are only meaningful if the walk that produces \
             the third one actually climbs"
        );
    }

    /// A cyclic parent chain must terminate the walk rather than spin
    /// forever. `crate::semantic`'s
    /// `a_frame_with_a_forward_parent_link_is_reported_not_panicked` already
    /// treats this shape as a real malformed-frame input; the keyboard walk
    /// gets the same guarantee.
    #[test]
    fn a_cyclic_parent_chain_terminates_instead_of_hanging() {
        // Built with valid parents first, then broken by direct mutation —
        // `digest::digest` refuses a placement list that names a parent out
        // of pre-order on construction, exactly as
        // `a_frame_with_a_forward_parent_link_is_reported_not_panicked` in
        // `crate::semantic` builds its own malformed fixture.
        let mut f = frame(vec![
            kid(
                "/a",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 100.0, 100.0),
                0,
                &[],
                None,
            ),
            kid(
                "/b",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 100.0, 100.0),
                0,
                &[],
                None,
            ),
        ]);
        f.placements[0].parent = Some(1);
        f.placements[1].parent = Some(0);
        let key = InputEvent::Key {
            key: KeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            route(&f, Some("/a"), &key),
            Route::Unrouted {
                reason: "no node from the focused one up to the root accepts this event".into(),
            }
        );
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

    /// Spec 009 T014: a press or release asks for [`Interaction::Click`] or
    /// [`Interaction::SecondaryClick`] by which button produced it, not by
    /// event kind alone — the split `each_event_kind_asks_for_the_interaction_it_needs`
    /// does not exercise because every event it builds is a bare press with
    /// no button to vary.
    #[test]
    fn a_press_or_release_asks_for_click_or_secondary_click_by_button() {
        let secondary_press = InputEvent::PointerPressed {
            pos: Point::ZERO,
            button: PointerButton::Secondary,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            required_interaction(&secondary_press),
            Some(Interaction::SecondaryClick)
        );
        let secondary_release = InputEvent::PointerReleased {
            pos: Point::ZERO,
            button: PointerButton::Secondary,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            required_interaction(&secondary_release),
            Some(Interaction::SecondaryClick)
        );
        assert_eq!(
            required_interaction(&press(Point::ZERO)),
            Some(Interaction::Click),
            "a primary press must keep asking for Click, not the new variant"
        );
        let middle_press = InputEvent::PointerPressed {
            pos: Point::ZERO,
            button: PointerButton::Middle,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            required_interaction(&middle_press),
            Some(Interaction::Click),
            "the middle button is not secondary; it keeps the old, \
             button-blind answer"
        );
    }

    /// The reachability half of the same split: a node that declares only
    /// [`Interaction::Click`] must stay unreachable by a secondary press,
    /// and a node that declares only [`Interaction::SecondaryClick`] must
    /// stay unreachable by a primary one. Neither variant silently widens
    /// what the other accepts.
    #[test]
    fn a_node_declaring_only_one_of_click_or_secondary_click_is_unreachable_by_the_other_button() {
        // Both rects sit inside `frame`'s fixed 200x200 clip and viewport
        // (see this module's `node`/`frame` test helpers) — a rect placed
        // past x=200 would be clipped out and fail every hit test for a
        // reason unrelated to this test's actual claim.
        let rect = Rect::new(0.0, 0.0, 80.0, 80.0);
        let f = frame(vec![
            node("/click-only", rect, 0, &[Interaction::Click]),
            node(
                "/secondary-only",
                Rect::new(100.0, 0.0, 80.0, 80.0),
                0,
                &[Interaction::SecondaryClick],
            ),
        ]);
        let secondary_press = InputEvent::PointerPressed {
            pos: Point::new(40.0, 40.0),
            button: PointerButton::Secondary,
            modifiers: Modifiers::NONE,
        };
        assert!(
            matches!(route(&f, None, &secondary_press), Route::Unrouted { .. }),
            "a plain Click node must not start accepting a secondary press"
        );
        let primary_press_on_secondary_only = InputEvent::PointerPressed {
            pos: Point::new(140.0, 40.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        assert!(
            matches!(
                route(&f, None, &primary_press_on_secondary_only),
                Route::Unrouted { .. }
            ),
            "a SecondaryClick-only node must not start accepting a primary click"
        );
        // Each node is still reachable by the button it actually declared.
        let secondary_on_its_own_node = InputEvent::PointerPressed {
            pos: Point::new(140.0, 40.0),
            button: PointerButton::Secondary,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            route(&f, None, &secondary_on_its_own_node),
            Route::Pointer {
                node: "/secondary-only".into()
            }
        );
        let primary_on_its_own_node = InputEvent::PointerPressed {
            pos: Point::new(50.0, 50.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            route(&f, None, &primary_on_its_own_node),
            Route::Pointer {
                node: "/click-only".into()
            }
        );
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
        let outcome =
            route_with_surfaces(&f, None, &press(Point::new(10.0, 10.0)), &surfaces, None);
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
        let outcome =
            route_with_surfaces(&f, None, &press(Point::new(60.0, 60.0)), &surfaces, None);
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
            None,
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
        let on_surface = route_with_surfaces(
            &absorbing,
            None,
            &press(Point::new(60.0, 60.0)),
            &surfaces,
            None,
        );
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
        let outcome =
            route_with_surfaces(&f, None, &press(Point::new(10.0, 10.0)), &surfaces, None);
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
        let outcome =
            route_with_surfaces(&f, None, &press(Point::new(10.0, 10.0)), &surfaces, None);
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
        let outcome =
            route_with_surfaces(&f, None, &press(Point::new(10.0, 10.0)), &surfaces, None);
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
        let outcome =
            route_with_surfaces(&f, None, &press(Point::new(60.0, 60.0)), &surfaces, None);
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
        let outcome = route_with_surfaces(&f, None, &moved, &surfaces, None);
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
        let between =
            route_with_surfaces(&f, None, &press(Point::new(30.0, 30.0)), &surfaces, None);
        assert!(matches!(between.route, Route::Unrouted { .. }));

        // Inside both, on the inner dialog's own control: reaches it.
        let inner_ok =
            route_with_surfaces(&f, None, &press(Point::new(65.0, 65.0)), &surfaces, None);
        assert_eq!(
            inner_ok.route,
            Route::Pointer {
                node: "/inner/ok".into()
            }
        );

        // Inside both, on nothing the inner dialog accepts: the outer
        // dialog's full-size control is right there behind it and is not
        // reached, because the inner surface is the floor.
        let inside = route_with_surfaces(&f, None, &press(Point::new(90.0, 90.0)), &surfaces, None);
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

    /// A raw-claiming placement, focused directly, declaring `Key` so it is
    /// still a legal target for ordinary keyboard routing.
    fn raw_claim_node(id: &str, rect: Rect, parent: Option<usize>) -> Placement {
        let mut p = kid(id, NodeKind::Custom, rect, 0, &[Interaction::Key], parent);
        p.semantics.raw_claim = true;
        p
    }

    fn key(k: KeyCode, modifiers: Modifiers) -> InputEvent {
        InputEvent::Key {
            key: k,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// FR-021's structural guarantee, half one: three-argument `route` never
    /// returns `Raw`, even for a node that has declared a raw claim and is
    /// directly focused. Verbatim delivery is unreachable through this
    /// function, full stop — the missing `reserved` parameter is the whole
    /// of the proof, and this test is what falsifies that if it is ever
    /// broken.
    #[test]
    fn plain_route_never_returns_raw_even_for_a_raw_claim() {
        let f = frame(vec![raw_claim_node(
            "/term",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
        )]);
        let got = route(&f, Some("/term"), &key(KeyCode::Char('q'), Modifiers::NONE));
        assert_eq!(
            got,
            Route::Keyboard {
                node: "/term".into()
            },
            "a raw claim must not change what plain `route` returns: {got:?}"
        );
        assert_ne!(
            got,
            Route::Raw {
                node: "/term".into()
            }
        );
    }

    /// `route_with_reserved` returns `Raw` when focus sits on a *descendant*
    /// of the node that declared the claim — the "OR any ancestor" half of
    /// the rule, walked the same way `route_keyboard_from` climbs for an
    /// accepting ancestor. The node named is the claimant itself, not the
    /// focused descendant: verbatim delivery goes to the entity the claim
    /// exists to feed.
    #[test]
    fn focus_inside_a_raw_claim_routes_raw() {
        let term = raw_claim_node("/term", Rect::new(0.0, 0.0, 200.0, 200.0), None);
        let cursor = kid(
            "/term/cursor",
            NodeKind::Text,
            Rect::new(10.0, 10.0, 10.0, 10.0),
            0,
            &[],
            Some(0),
        );
        let f = frame(vec![term, cursor]);
        let reserved = ReservedChords::default();
        let got = route_with_reserved(
            &f,
            Some("/term/cursor"),
            &key(KeyCode::Char('j'), Modifiers::NONE),
            &reserved,
        );
        assert_eq!(
            got,
            Route::Raw {
                node: "/term".into()
            },
            "focus inside the claiming node's subtree must route raw, \
             naming the claimant: {got:?}"
        );
    }

    /// The direct case: focus sits on the claiming node itself, exactly
    /// US-5's shape ("A terminal node holds a raw claim and has focus.").
    #[test]
    fn focus_directly_on_a_raw_claim_routes_raw() {
        let f = frame(vec![raw_claim_node(
            "/term",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
        )]);
        let reserved = ReservedChords::default();
        let got = route_with_reserved(
            &f,
            Some("/term"),
            &key(KeyCode::Char('j'), Modifiers::NONE),
            &reserved,
        );
        assert_eq!(
            got,
            Route::Raw {
                node: "/term".into()
            }
        );
    }

    /// FR-021, US-6: the reserved chord always escapes a raw claim, checked
    /// before the raw-claim test itself — the shell handles it, no
    /// exceptions, and this is the test that proves a terminal cannot trap
    /// the operator behind its own claim.
    #[test]
    fn the_reserved_chord_escapes_a_raw_claim() {
        let f = frame(vec![raw_claim_node(
            "/term",
            Rect::new(0.0, 0.0, 200.0, 200.0),
            None,
        )]);
        let reserved = ReservedChords::default();
        let got = route_with_reserved(
            &f,
            Some("/term"),
            &key(KeyCode::Escape, Modifiers::shift()),
            &reserved,
        );
        assert_eq!(
            got,
            Route::Reserved {
                chord: Chord::reserved_escape()
            },
            "the reserved chord must escape a raw claim even when the \
             claiming node is exactly what is focused: {got:?}"
        );
    }

    /// SC-004, both halves in one test: a raw-claiming node receives a
    /// chord the shell has bound (the binding table is never consulted,
    /// FR-020), and the reserved chord still returns focus even though it
    /// is the exact same node holding the exact same claim. Either half
    /// alone would be a half-truth: a router that always escapes on any
    /// chord is not "raw", and one that never escapes traps the operator.
    #[test]
    fn a_raw_claim_takes_a_bound_chord_but_not_the_reserved_one() {
        let f = frame(vec![raw_claim_node(
            "/term",
            Rect::new(0.0, 0.0, 200.0, 200.0),
            None,
        )]);
        let reserved = ReservedChords::default();

        // Half 1: ctrl-s is exactly the chord US-5 names as shell-bound —
        // it still reaches the raw claim verbatim.
        let ctrl_s = key(
            KeyCode::Char('s'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        let bound = route_with_reserved(&f, Some("/term"), &ctrl_s, &reserved);
        assert_eq!(
            bound,
            Route::Raw {
                node: "/term".into()
            },
            "a bound chord must still reach the raw claim: {bound:?}"
        );

        // Half 2: shift-esc, the reserved chord, still escapes.
        let shift_esc = key(KeyCode::Escape, Modifiers::shift());
        let escaped = route_with_reserved(&f, Some("/term"), &shift_esc, &reserved);
        assert_eq!(
            escaped,
            Route::Reserved {
                chord: Chord::reserved_escape()
            },
            "the reserved chord must still escape the same claim: {escaped:?}"
        );
    }

    /// The reserved chord is checked before anything else, including before
    /// there being any raw claim in the frame at all — the check does not
    /// depend on focus being inside a claim, or on focus existing at all.
    #[test]
    fn the_reserved_chord_is_reserved_with_no_raw_claim_in_play() {
        let f = frame(vec![node(
            "/btn",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[Interaction::Click],
        )]);
        let reserved = ReservedChords::default();
        let got = route_with_reserved(
            &f,
            None,
            &key(KeyCode::Escape, Modifiers::shift()),
            &reserved,
        );
        assert_eq!(
            got,
            Route::Reserved {
                chord: Chord::reserved_escape()
            }
        );
    }

    /// A raw claim is a declaration about keystrokes (FR-019's own wording:
    /// "takes keystrokes verbatim"), not about every event kind. A pointer
    /// press elsewhere in the frame must still hit-test normally rather than
    /// being redirected to whatever node happens to hold focus and a raw
    /// claim — otherwise a raw claim on an unrelated, unfocused-by-pointer
    /// node would silently break ordinary clicking anywhere else on screen
    /// (spec 010 FR-006's principle: raw pass-through must not change
    /// pointer routing).
    #[test]
    fn a_raw_claim_does_not_capture_pointer_events() {
        let term = raw_claim_node("/term", Rect::new(0.0, 0.0, 50.0, 50.0), None);
        let btn = kid(
            "/btn",
            NodeKind::Stack,
            Rect::new(100.0, 100.0, 50.0, 50.0),
            0,
            &[Interaction::Click],
            None,
        );
        let f = frame(vec![term, btn]);
        let reserved = ReservedChords::default();
        let press = InputEvent::PointerPressed {
            pos: Point::new(120.0, 120.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        let got = route_with_reserved(&f, Some("/term"), &press, &reserved);
        assert_eq!(
            got,
            Route::Pointer {
                node: "/btn".into()
            },
            "a raw claim on the focused node must not steal a pointer \
             press aimed elsewhere: {got:?}"
        );
    }

    /// With no raw claim anywhere on the focus path and no reserved chord
    /// match, `route_with_reserved` must behave exactly like `route` —
    /// the escape hatch changes nothing when nobody has asked for it.
    #[test]
    fn route_with_reserved_matches_plain_route_when_nothing_claims_raw() {
        let f = frame(vec![node(
            "/field",
            Rect::new(0.0, 0.0, 100.0, 20.0),
            0,
            &[Interaction::Key],
        )]);
        let reserved = ReservedChords::default();
        let plain = route(
            &f,
            Some("/field"),
            &key(KeyCode::Char('x'), Modifiers::NONE),
        );
        let with_reserved = route_with_reserved(
            &f,
            Some("/field"),
            &key(KeyCode::Char('x'), Modifiers::NONE),
            &reserved,
        );
        assert_eq!(plain, with_reserved);
    }

    /// A `/page` → `/modal` → `/modal/field` chain, with `/modal` open as a
    /// surface under `policy`. `/page` declares `Key` and `/modal` does not,
    /// so without a ceiling an Escape from the field would climb straight
    /// past the modal to the page — exactly the bug T011 exists to close.
    fn page_modal_field(policy: InputPolicy) -> (PetrifiedFrame, BTreeMap<String, InputPolicy>) {
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Key],
                None,
            ),
            kid(
                "/modal",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                Some(0),
            ),
            kid(
                "/modal/field",
                NodeKind::Input,
                Rect::new(10.0, 10.0, 100.0, 20.0),
                0,
                &[Interaction::TextEdit],
                Some(1),
            ),
        ]);
        let surfaces = BTreeMap::from([("/modal".to_owned(), policy)]);
        (f, surfaces)
    }

    /// Spec 010 T011, half one: an intervening `Block` surface stops the
    /// keyboard walk at its own boundary, and the `Unrouted` reason names
    /// it (FR-024) — the walk does not climb past `/modal` to `/page`, even
    /// though `/page` would have accepted the Escape.
    #[test]
    fn an_intervening_block_surface_stops_the_keyboard_walk_and_names_itself() {
        let (f, surfaces) = page_modal_field(InputPolicy::Block);
        let escape = InputEvent::Key {
            key: KeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let got = route_with_surfaces(&f, Some("/modal/field"), &escape, &surfaces, None).route;
        let Route::Unrouted { reason } = &got else {
            panic!("expected Unrouted, the Block surface must stop the walk: {got:?}");
        };
        assert!(
            reason.contains("/modal"),
            "the reason must name the blocking surface, not just say \
             'blocked': {reason:?}"
        );
        assert_ne!(
            got,
            Route::Keyboard {
                node: "/page".into()
            },
            "the walk must not climb past the Block surface to a shared \
             ancestor above it"
        );
    }

    /// Spec 010 T011, half two: a rule that blocks everything passes half
    /// one alone. `DismissOutside` must NOT stop the walk — the same
    /// distinction [`block_floor`] already draws for the pointer.
    #[test]
    fn a_dismiss_outside_surface_does_not_stop_the_keyboard_walk() {
        let (f, surfaces) = page_modal_field(InputPolicy::DismissOutside);
        let escape = InputEvent::Key {
            key: KeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let got = route_with_surfaces(&f, Some("/modal/field"), &escape, &surfaces, None).route;
        assert_eq!(
            got,
            Route::Keyboard {
                node: "/page".into()
            },
            "a DismissOutside surface must not act as a ceiling: {got:?}"
        );
    }

    /// `Passthrough` is the third policy and must behave like
    /// `DismissOutside` here: neither is a ceiling, only `Block` is.
    #[test]
    fn a_passthrough_surface_does_not_stop_the_keyboard_walk() {
        let (f, surfaces) = page_modal_field(InputPolicy::Passthrough);
        let escape = InputEvent::Key {
            key: KeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let got = route_with_surfaces(&f, Some("/modal/field"), &escape, &surfaces, None).route;
        assert_eq!(
            got,
            Route::Keyboard {
                node: "/page".into()
            }
        );
    }

    /// T011's interaction with FR-021: prove the fixture actually would
    /// swallow the chord (so the second half of this test proves something),
    /// then prove the reserved chord escapes it anyway. `route_with_reserved`
    /// checks the reserved set before it ever asks about surfaces or raw
    /// claims, so a modal that could swallow the reserved chord would be the
    /// same trap FR-021 exists to prevent, wearing a different hat — this is
    /// the assertion that closes that gap.
    #[test]
    fn the_reserved_chord_escapes_a_block_surface_that_would_otherwise_swallow_it() {
        let (f, surfaces) = page_modal_field(InputPolicy::Block);
        let shift_esc = key(KeyCode::Escape, Modifiers::shift());

        let blocked =
            route_with_surfaces(&f, Some("/modal/field"), &shift_esc, &surfaces, None).route;
        assert!(
            matches!(&blocked, Route::Unrouted { reason } if reason.contains("/modal")),
            "the fixture must actually demonstrate a swallow, or this test \
             proves nothing: {blocked:?}"
        );

        let reserved = ReservedChords::default();
        let escaped = route_with_reserved(&f, Some("/modal/field"), &shift_esc, &reserved);
        assert_eq!(
            escaped,
            Route::Reserved {
                chord: Chord::reserved_escape()
            },
            "the reserved chord must escape even a Block surface that would \
             otherwise swallow the same keystroke: {escaped:?}"
        );

        // And it composes in the ONE function the shell actually calls.
        // Before the escape hatch was threaded here, a caller had to choose
        // between Block-surface awareness and reserved/raw semantics, and
        // whichever it picked it silently lost the other.
        let composed = route_with_surfaces(
            &f,
            Some("/modal/field"),
            &shift_esc,
            &surfaces,
            Some(&reserved),
        )
        .route;
        assert_eq!(
            composed,
            Route::Reserved {
                chord: Chord::reserved_escape()
            },
            "route_with_surfaces given the escape hatch must reach the same \
             answer as route_with_reserved: {composed:?}"
        );
    }

    #[test]
    fn a_raw_claim_is_reachable_through_the_surface_aware_path() {
        // The other half of the composition. A terminal is a raw claim, and
        // the shell routes through `route_with_surfaces`; if raw were
        // unreachable there, no keystroke would ever arrive at a terminal.
        let mut term = raw_claim_node("/modal/term", Rect::new(10.0, 10.0, 100.0, 20.0), Some(1));
        term.semantics.actions = vec![Interaction::Key];
        let f = frame(vec![
            kid(
                "/page",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[Interaction::Key],
                None,
            ),
            kid(
                "/modal",
                NodeKind::Stack,
                Rect::new(0.0, 0.0, 200.0, 200.0),
                0,
                &[],
                Some(0),
            ),
            term,
        ]);
        let surfaces = BTreeMap::from([("/modal".to_owned(), InputPolicy::Block)]);
        let reserved = ReservedChords::default();
        let ctrl_s = key(
            KeyCode::Char('s'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );

        let without = route_with_surfaces(&f, Some("/modal/term"), &ctrl_s, &surfaces, None).route;
        assert!(
            !matches!(without, Route::Raw { .. }),
            "no escape hatch supplied, so Raw must stay unreachable: {without:?}"
        );

        let with =
            route_with_surfaces(&f, Some("/modal/term"), &ctrl_s, &surfaces, Some(&reserved)).route;
        assert!(
            matches!(&with, Route::Raw { node } if node == "/modal/term"),
            "the surface-aware path must deliver verbatim once the hatch is \
             supplied: {with:?}"
        );

        // And the reserved chord still wins over that raw claim, in the same
        // call, with a Block surface in play. All three concerns at once.
        let escape = key(KeyCode::Escape, Modifiers::shift());
        let out =
            route_with_surfaces(&f, Some("/modal/term"), &escape, &surfaces, Some(&reserved)).route;
        assert_eq!(
            out,
            Route::Reserved {
                chord: Chord::reserved_escape()
            },
            "reserved must outrank a raw claim inside a Block surface: {out:?}"
        );
    }

    /// The one wiring line: `PointerState::route` is the shell's actual
    /// input entry (not `route_with_reserved` directly), so this is the
    /// proof `Route::Raw` and `Route::Reserved` are reachable *through it*
    /// once it is built with a reserved set — not just through the free
    /// functions a test can call on its own.
    #[test]
    fn pointer_state_delivers_reserved_and_raw() {
        let f = frame(vec![raw_claim_node(
            "/term",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
        )]);
        let surfaces = BTreeMap::new();
        let mut pointer = PointerState::with_reserved(ReservedChords::default());

        let ctrl_s = key(
            KeyCode::Char('s'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        let routing = pointer.route(&f, Some("/term"), &surfaces, &ctrl_s);
        assert_eq!(
            routing.outcome.route,
            Route::Raw {
                node: "/term".into()
            },
            "a PointerState built with a reserved set must deliver a raw \
             claim verbatim through its one real input entry: {:?}",
            routing.outcome.route
        );

        let shift_esc = key(KeyCode::Escape, Modifiers::shift());
        let routing = pointer.route(&f, Some("/term"), &surfaces, &shift_esc);
        assert_eq!(
            routing.outcome.route,
            Route::Reserved {
                chord: Chord::reserved_escape()
            },
            "and the reserved chord must still escape the same claim through \
             the same entry: {:?}",
            routing.outcome.route
        );
    }

    /// The other direction of the same guarantee `route` vs. `route_with_reserved`
    /// already proves for the free functions: a `PointerState` built with
    /// [`PointerState::new`] carries no reserved set, so the wiring this
    /// leaf adds must not make `Raw` reachable through it by accident. The
    /// same node, the same focus, the same keystroke as the test above —
    /// the only thing that changed is which constructor built the state.
    #[test]
    fn pointer_state_without_a_reserved_set_never_produces_raw() {
        let f = frame(vec![raw_claim_node(
            "/term",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
        )]);
        let surfaces = BTreeMap::new();
        let mut pointer = PointerState::new();

        let ctrl_s = key(
            KeyCode::Char('s'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        let routing = pointer.route(&f, Some("/term"), &surfaces, &ctrl_s);
        assert_ne!(
            routing.outcome.route,
            Route::Raw {
                node: "/term".into()
            },
            "PointerState::new carries no reserved set, so Raw must stay \
             unreachable through it: {:?}",
            routing.outcome.route
        );
        assert_eq!(
            routing.outcome.route,
            Route::Keyboard {
                node: "/term".into()
            },
            "with no escape hatch supplied, the same node routes through \
             ordinary keyboard delivery instead: {:?}",
            routing.outcome.route
        );
    }

    /// Spec 009 T014, design question 2: a secondary press sets neither
    /// pressed state nor a capture — "neither" is the answer
    /// [`PointerState::route`]'s own "Only the primary: a right-click does
    /// not press a control" comment already commits to. This pins it
    /// against the new variant rather than trusting the comment alone to
    /// stay true once a node can declare `SecondaryClick`.
    #[test]
    fn a_secondary_press_lights_no_pressed_state() {
        // `Hover` is declared too: `pressed_on` is only ever set from
        // `hovered` (`PointerState::route_positional`'s
        // `self.pressed_on = self.hovered.clone()`), and `hovered` answers
        // `None` for a node that never opted into `Hover` in the first
        // place — the same rule `button::chrome`'s own doc names. Omitting
        // it here would make the primary-press half of this test fail for
        // a reason that has nothing to do with what it is pinning down.
        let f = frame(vec![node(
            "/btn",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            0,
            &[
                Interaction::Click,
                Interaction::SecondaryClick,
                Interaction::Hover,
            ],
        )]);
        let surfaces = BTreeMap::new();
        let mut pointer = PointerState::new();
        let secondary_press = InputEvent::PointerPressed {
            pos: Point::new(50.0, 50.0),
            button: PointerButton::Secondary,
            modifiers: Modifiers::NONE,
        };
        let routing = pointer.route(&f, None, &surfaces, &secondary_press);
        assert_eq!(
            routing.outcome.route,
            Route::Pointer {
                node: "/btn".into()
            },
            "the press must still route: this is about pressed state, not \
             delivery"
        );
        assert_eq!(
            pointer.pressed(),
            None,
            "a secondary press must not light pressed state"
        );

        let primary_press = InputEvent::PointerPressed {
            pos: Point::new(50.0, 50.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        pointer.route(&f, None, &surfaces, &primary_press);
        assert_eq!(
            pointer.pressed(),
            Some("/btn"),
            "a primary press on the same node must still light pressed state"
        );
    }
}
