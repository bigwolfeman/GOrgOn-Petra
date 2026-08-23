//! Synthetic input at the platform boundary (T030, FR-038).
//!
//! `contracts/driver-protocol.md`'s load-bearing rule: *"every `act`
//! synthesizes the same low-level input events a physical device produces
//! and injects them at the platform-input boundary — hit-testing, focus
//! rules, and event ordering run exactly as for a human. No direct widget
//! invocation exists in the protocol."* This module is the one place that
//! rule is kept: [`inject_action`] turns a driver [`Action`] into
//! [`egui::Event`] values and appends them to the [`egui::RawInput`] the next
//! [`crate::host::Host::pass`] consumes. From there the events go through
//! [`crate::input::EventTranslator`] — the *same* function that translates a
//! real device's events — and then through [`gorgon_petra::input::route`],
//! exactly as for a human. There is no second, shorter path: this module
//! never calls [`crate::host::App::handle`] and never constructs a
//! [`gorgon_petra::input::Route`] itself.
//!
//! # Two kinds of target, and why one of them cannot become the other
//!
//! A driver action aims at a [`Target`]: a stable node id, or a raw
//! device-pixel-independent position (`contracts/driver-protocol.md`'s
//! `target: {node_id | pos}`). A `NodeId` target is resolved against
//! [`crate::host::Host::frame`]'s placements before anything else happens,
//! and that resolution is the *only* place a coordinate for it is ever
//! produced. [`resolve_point`] and [`resolve_focus_id`] return
//! `Result<_, InjectError>`, and every caller in this module propagates the
//! error with `?` rather than substituting a default — there is no code path
//! in which an id that failed to resolve is also holding a point, because
//! the two are mutually exclusive arms of one `Result`. A `node_id` that no
//! longer resolves fails as [`InjectError::StaleNode`], naming the id, and
//! the events for that action are never constructed — nothing is delivered
//! to whatever now occupies the id's last-known coordinates
//! (`contracts/driver-protocol.md`, "stale targets fail loudly").
//!
//! # `focus` is the one action that is not a device event
//!
//! No physical device sends "focus node X"; a human reaches a node by
//! clicking it or by tabbing to it. [`crate::host::Host::focus_mut`] is
//! documented as "the way a driver ... moves focus programmatically", and it
//! goes through [`gorgon_petra::focus::FocusTree::focus`] — the same
//! function [`crate::host::Host`]'s own Tab-traversal calls — which refuses
//! a move that would leave an open modal's scope
//! ([`gorgon_petra::focus::FocusError::OutsideActiveScope`]). That is "the
//! focus rules" this module's tests hold `Action::Focus` to: the action
//! reaches [`gorgon_petra::focus::FocusTree::focus`], never a raw write to
//! [`gorgon_petra::layout::LayoutState::focused`] that would bypass it.
//! `text-edit` uses the same call for the same reason — a field cannot
//! receive typed text it is not focused to receive.

use egui::{Event, MouseWheelUnit, PointerButton as EguiButton, Pos2, RawInput, TouchPhase, Vec2};
use gorgon_petra::focus::FocusError;
use gorgon_petra::input::hit_test;
use gorgon_petra::tree::Interaction;
use gorgon_petra::{KeyCode, Modifiers, PetrifiedFrame, Point, Rect, Size};

use crate::host::{App, Host};

/// Where a driver action aims (`contracts/driver-protocol.md`'s
/// `target: {node_id | pos}`).
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A stable node id, resolved against the current frame's placements.
    /// Never resolved by falling through to a coordinate: see the module
    /// doc's "two kinds of target" section.
    NodeId(String),
    /// A raw position in logical units — the same units
    /// [`egui::Event::PointerMoved`] carries. Bypasses node resolution
    /// entirely; there is nothing to go stale.
    Pos(Point),
}

/// The closed action vocabulary (`contracts/semantic-tree.md`): `click`,
/// `drag`, `hover`, `focus`, `text-edit`, `scroll`, `key`. One variant per
/// wire spelling, by construction — this enum and
/// [`gorgon_petra::tree::Interaction`] name the same seven things because
/// they are the same seven things.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A primary-button press and release at the target, with the pointer
    /// moved there first — the sequence a physical click produces.
    Click {
        /// Modifiers held for both the press and the release.
        modifiers: Modifiers,
    },
    /// A primary-button press at the target, one or more moves, and a
    /// release at `to` — press-move-release, per
    /// [`gorgon_petra::tree::Interaction::Drag`].
    Drag {
        /// Where the drag ends, in logical units.
        to: Point,
        /// Modifiers held for the whole gesture.
        modifiers: Modifiers,
    },
    /// The pointer moves to the target and no button changes state.
    Hover,
    /// Focus moves to the target through
    /// [`gorgon_petra::focus::FocusTree::focus`] — see the module doc.
    Focus,
    /// Committed text delivered to whichever node focus is currently on.
    /// The target is checked for staleness but is not what routes the
    /// text: [`gorgon_petra::input::route`] routes text by focus, the same
    /// as a physical keyboard's text always is. A driver that wants the
    /// text to land on a specific node sends `focus` first.
    TextEdit {
        /// The text, as a physical keyboard's composed keystrokes, a
        /// paste, or an IME commit would deliver it
        /// (`crate::input::EventTranslator` unifies all three).
        text: String,
    },
    /// The pointer moves to the target and a wheel delta is delivered
    /// there. Petra's [`gorgon_petra::input::InputEvent::Scroll`] carries no
    /// modifiers, so none are threaded through here — a field this crate's
    /// own translator always discards is a field with no honest use.
    Scroll {
        /// The wheel delta, in the units [`egui::MouseWheelUnit::Point`]
        /// carries: one logical unit in, one logical unit of scroll out,
        /// with no platform-dependent line/page rescaling in between.
        delta: Size,
    },
    /// A full keystroke — press then release — delivered to whichever node
    /// is focused, the same as `text-edit`.
    Key {
        /// Which key.
        key: KeyCode,
        /// Modifiers held for both the press and the release.
        modifiers: Modifiers,
    },
}

/// Why an action could not be turned into events, or why a programmatic
/// focus move was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum InjectError {
    /// `node_id` does not resolve against the current frame's placements:
    /// renamed, removed, or asked before any frame has been placed. No
    /// events were synthesized. Maps to `contracts/driver-protocol.md`'s
    /// `stale-node` error kind.
    StaleNode(String),
    /// The id resolved to a real placement this frame, but it is not
    /// currently a focusable one — disabled, or declaring no
    /// [`gorgon_petra::tree::Interaction::Focus`]. Distinct from
    /// `StaleNode`: the node is real, the request is simply not valid
    /// against it right now.
    NotFocusable(String),
    /// `id` is focusable, but a blocking (modal) scope is active and `id`
    /// is outside it — the same refusal Tab/Shift+Tab traversal is subject
    /// to. A driver gets no side door around a focus trap.
    OutsideActiveScope {
        /// The id that was requested.
        id: String,
        /// The blocking surface's id whose scope it would have left.
        scope: String,
    },
    /// A `Pos` target for `focus` hit nothing that accepts
    /// [`gorgon_petra::tree::Interaction::Focus`] this frame — either no
    /// frame has been placed yet, or nothing focusable sits under that
    /// point.
    NoFocusableAt(Point),
    /// `key` has no `egui::Key` this crate can produce: a `Function` index
    /// outside 1..=35, or a `Char` outside the set
    /// [`crate::input::key_code`] can ever translate back from. The closed
    /// [`KeyCode`] vocabulary this crate's own translator emits always maps;
    /// this is reachable only from a `KeyCode` a caller built by hand.
    UnsupportedKey(KeyCode),
}

impl std::fmt::Display for InjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleNode(id) => write!(
                f,
                "stale-node: {id:?} does not resolve in the current frame"
            ),
            Self::NotFocusable(id) => write!(f, "{id:?} exists this frame but is not focusable"),
            Self::OutsideActiveScope { id, scope } => write!(
                f,
                "{id:?} is outside the blocking scope {scope:?} that currently traps focus"
            ),
            Self::NoFocusableAt(pos) => write!(f, "nothing focusable is under {pos:?}"),
            Self::UnsupportedKey(key) => write!(f, "{key:?} has no egui key to synthesize"),
        }
    }
}

impl std::error::Error for InjectError {}

impl From<FocusError> for InjectError {
    fn from(err: FocusError) -> Self {
        match err {
            FocusError::UnknownId(id) => Self::NotFocusable(id),
            FocusError::OutsideActiveScope { id, scope } => Self::OutsideActiveScope { id, scope },
        }
    }
}

/// Turn one driver action into the platform events a physical device would
/// have produced, and append them to `raw` — the [`egui::RawInput`] the next
/// [`crate::host::Host::pass`] will read from
/// [`egui::Context::input`]. `host` is read to resolve a [`Target::NodeId`]
/// against the current frame, and mutated only for [`Action::Focus`], via
/// [`crate::host::Host::focus_mut`] (see the module doc).
///
/// Nothing here calls [`crate::host::App::handle`] or constructs a
/// [`gorgon_petra::input::Route`]. Whatever is pushed onto `raw` is routed by
/// the same [`crate::input::EventTranslator`] and
/// [`gorgon_petra::input::route_with_surfaces`] a human's input goes through,
/// the next time `raw` is handed to `egui::Context::run_ui` around
/// [`crate::host::Host::pass`].
///
/// # Errors
/// [`InjectError::StaleNode`] when a [`Target::NodeId`] does not resolve;
/// [`InjectError::NotFocusable`] / [`InjectError::OutsideActiveScope`] /
/// [`InjectError::NoFocusableAt`] when [`Action::Focus`] or
/// [`Action::TextEdit`]'s implicit focus step is refused;
/// [`InjectError::UnsupportedKey`] when [`Action::Key`] names a `KeyCode`
/// this crate cannot turn into an `egui::Key`. On every error, `raw` is left
/// exactly as it was passed in.
pub fn inject_action<A: App>(
    host: &mut Host<A>,
    target: &Target,
    action: &Action,
    raw: &mut RawInput,
) -> Result<(), InjectError> {
    match action {
        Action::Click { modifiers } => {
            let pos = resolve_point(host.frame(), target)?;
            push_click(raw, pos, *modifiers);
        }
        Action::Drag { to, modifiers } => {
            let from = resolve_point(host.frame(), target)?;
            push_drag(raw, from, *to, *modifiers);
        }
        Action::Hover => {
            let pos = resolve_point(host.frame(), target)?;
            raw.events.push(Event::PointerMoved(to_pos2(pos)));
        }
        Action::Scroll { delta } => {
            let pos = resolve_point(host.frame(), target)?;
            push_scroll(raw, pos, *delta);
        }
        Action::Key { key, modifiers } => {
            require_present(host.frame(), target)?;
            push_key(raw, *key, *modifiers)?;
        }
        Action::TextEdit { text } => {
            require_present(host.frame(), target)?;
            raw.events.push(Event::Text(text.clone()));
        }
        Action::Focus => {
            let id = resolve_focus_id(host.frame(), target, Interaction::Focus)?;
            host.focus_mut().focus(&id)?;
        }
    }
    Ok(())
}

/// `target` resolved to a point in logical units — the units every
/// pointer-carrying [`egui::Event`] uses. [`Target::Pos`] is returned as-is;
/// [`Target::NodeId`] is looked up in `frame`'s placements (which are
/// already in logical units — device-pixel rounding happens once, at
/// petrify, downstream of this) and fails as [`InjectError::StaleNode`] with
/// no coordinate produced on the error path.
fn resolve_point(frame: Option<&PetrifiedFrame>, target: &Target) -> Result<Point, InjectError> {
    match target {
        Target::Pos(p) => Ok(*p),
        Target::NodeId(id) => frame
            .and_then(|f| f.placement(id))
            .map(|p| center(p.rect))
            .ok_or_else(|| InjectError::StaleNode(id.clone())),
    }
}

/// The staleness half of target resolution, for an action that routes by
/// focus rather than by position ([`Action::Key`], [`Action::TextEdit`]): a
/// [`Target::NodeId`] must still resolve, but no point is produced from it.
/// A [`Target::Pos`] is not a node id and cannot be stale, so it always
/// passes.
fn require_present(frame: Option<&PetrifiedFrame>, target: &Target) -> Result<(), InjectError> {
    match target {
        Target::Pos(_) => Ok(()),
        Target::NodeId(id) => {
            if frame.and_then(|f| f.placement(id)).is_some() {
                Ok(())
            } else {
                Err(InjectError::StaleNode(id.clone()))
            }
        }
    }
}

/// The id [`Action::Focus`] should move focus to. A [`Target::NodeId`] is
/// checked for existence in `frame` (staleness only — whether it is
/// *currently focusable* is [`gorgon_petra::focus::FocusTree::focus`]'s own
/// check, made from the real focus tree, not duplicated here). A
/// [`Target::Pos`] is hit-tested for `interaction`, the same
/// [`gorgon_petra::input::hit_test`] a real click's routing uses.
fn resolve_focus_id(
    frame: Option<&PetrifiedFrame>,
    target: &Target,
    interaction: Interaction,
) -> Result<String, InjectError> {
    match target {
        Target::NodeId(id) => {
            frame
                .and_then(|f| f.placement(id))
                .ok_or_else(|| InjectError::StaleNode(id.clone()))?;
            Ok(id.clone())
        }
        Target::Pos(pos) => frame
            .and_then(|f| hit_test(f, *pos, interaction))
            .map(|p| p.id.clone())
            .ok_or(InjectError::NoFocusableAt(*pos)),
    }
}

/// The center of `rect`, in the same logical units the rect is already in.
fn center(rect: Rect) -> Point {
    Point::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
}

fn to_pos2(p: Point) -> Pos2 {
    Pos2::new(p.x, p.y)
}

/// Petra's modifiers as egui's, round-tripping exactly through
/// [`crate::input::translate_modifiers`]: `command` is set whenever `meta`
/// or `ctrl` is, so the forward translation's
/// `meta = mac_cmd || (command && !ctrl)` reduces to `meta = m.meta` for any
/// modifiers produced here, whatever `ctrl` is. Proven by
/// `modifiers_round_trip_through_translate_modifiers` below.
fn to_egui_modifiers(m: Modifiers) -> egui::Modifiers {
    egui::Modifiers {
        alt: m.alt,
        ctrl: m.ctrl,
        shift: m.shift,
        mac_cmd: m.meta,
        command: m.meta || m.ctrl,
    }
}

const LETTER_KEYS: [egui::Key; 26] = [
    egui::Key::A,
    egui::Key::B,
    egui::Key::C,
    egui::Key::D,
    egui::Key::E,
    egui::Key::F,
    egui::Key::G,
    egui::Key::H,
    egui::Key::I,
    egui::Key::J,
    egui::Key::K,
    egui::Key::L,
    egui::Key::M,
    egui::Key::N,
    egui::Key::O,
    egui::Key::P,
    egui::Key::Q,
    egui::Key::R,
    egui::Key::S,
    egui::Key::T,
    egui::Key::U,
    egui::Key::V,
    egui::Key::W,
    egui::Key::X,
    egui::Key::Y,
    egui::Key::Z,
];

const DIGIT_KEYS: [egui::Key; 10] = [
    egui::Key::Num0,
    egui::Key::Num1,
    egui::Key::Num2,
    egui::Key::Num3,
    egui::Key::Num4,
    egui::Key::Num5,
    egui::Key::Num6,
    egui::Key::Num7,
    egui::Key::Num8,
    egui::Key::Num9,
];

const FUNCTION_KEYS: [egui::Key; 35] = [
    egui::Key::F1,
    egui::Key::F2,
    egui::Key::F3,
    egui::Key::F4,
    egui::Key::F5,
    egui::Key::F6,
    egui::Key::F7,
    egui::Key::F8,
    egui::Key::F9,
    egui::Key::F10,
    egui::Key::F11,
    egui::Key::F12,
    egui::Key::F13,
    egui::Key::F14,
    egui::Key::F15,
    egui::Key::F16,
    egui::Key::F17,
    egui::Key::F18,
    egui::Key::F19,
    egui::Key::F20,
    egui::Key::F21,
    egui::Key::F22,
    egui::Key::F23,
    egui::Key::F24,
    egui::Key::F25,
    egui::Key::F26,
    egui::Key::F27,
    egui::Key::F28,
    egui::Key::F29,
    egui::Key::F30,
    egui::Key::F31,
    egui::Key::F32,
    egui::Key::F33,
    egui::Key::F34,
    egui::Key::F35,
];

/// Petra's [`KeyCode`] as egui's `Key`, the reverse of
/// [`crate::input::key_code`]. Total over every value that forward function
/// can ever produce; `None` only for a `Function`/`Char` outside the ranges
/// it ever emits.
fn to_egui_key(key: KeyCode) -> Option<egui::Key> {
    Some(match key {
        KeyCode::Tab => egui::Key::Tab,
        KeyCode::Enter => egui::Key::Enter,
        KeyCode::Escape => egui::Key::Escape,
        KeyCode::Space => egui::Key::Space,
        KeyCode::Backspace => egui::Key::Backspace,
        KeyCode::Delete => egui::Key::Delete,
        KeyCode::Up => egui::Key::ArrowUp,
        KeyCode::Down => egui::Key::ArrowDown,
        KeyCode::Left => egui::Key::ArrowLeft,
        KeyCode::Right => egui::Key::ArrowRight,
        KeyCode::Home => egui::Key::Home,
        KeyCode::End => egui::Key::End,
        KeyCode::PageUp => egui::Key::PageUp,
        KeyCode::PageDown => egui::Key::PageDown,
        KeyCode::Function(n) => {
            let idx = usize::from(n.wrapping_sub(1));
            *FUNCTION_KEYS.get(idx)?
        }
        KeyCode::Char(c) => {
            let lower = c.to_ascii_lowercase();
            match lower {
                'a'..='z' => LETTER_KEYS[(lower as u8 - b'a') as usize],
                '0'..='9' => DIGIT_KEYS[(lower as u8 - b'0') as usize],
                ':' => egui::Key::Colon,
                ',' => egui::Key::Comma,
                '\\' => egui::Key::Backslash,
                '/' => egui::Key::Slash,
                '|' => egui::Key::Pipe,
                '?' => egui::Key::Questionmark,
                '!' => egui::Key::Exclamationmark,
                '[' => egui::Key::OpenBracket,
                ']' => egui::Key::CloseBracket,
                '{' => egui::Key::OpenCurlyBracket,
                '}' => egui::Key::CloseCurlyBracket,
                '`' => egui::Key::Backtick,
                '-' => egui::Key::Minus,
                '.' => egui::Key::Period,
                '+' => egui::Key::Plus,
                '=' => egui::Key::Equals,
                ';' => egui::Key::Semicolon,
                '\'' => egui::Key::Quote,
                _ => return None,
            }
        }
    })
}

/// The move-press-release sequence a physical primary click produces.
fn push_click(raw: &mut RawInput, pos: Point, modifiers: Modifiers) {
    let at = to_pos2(pos);
    let mods = to_egui_modifiers(modifiers);
    raw.events.push(Event::PointerMoved(at));
    raw.events.push(Event::PointerButton {
        pos: at,
        button: EguiButton::Primary,
        pressed: true,
        modifiers: mods,
    });
    raw.events.push(Event::PointerButton {
        pos: at,
        button: EguiButton::Primary,
        pressed: false,
        modifiers: mods,
    });
}

/// The press-move-move-release sequence a physical primary-button drag
/// produces, with one interpolated waypoint so the gesture crosses whatever
/// is between `from` and `to` rather than teleporting.
fn push_drag(raw: &mut RawInput, from: Point, to: Point, modifiers: Modifiers) {
    let start = to_pos2(from);
    let end = to_pos2(to);
    let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    let mods = to_egui_modifiers(modifiers);
    raw.events.push(Event::PointerMoved(start));
    raw.events.push(Event::PointerButton {
        pos: start,
        button: EguiButton::Primary,
        pressed: true,
        modifiers: mods,
    });
    raw.events.push(Event::PointerMoved(mid));
    raw.events.push(Event::PointerMoved(end));
    raw.events.push(Event::PointerButton {
        pos: end,
        button: EguiButton::Primary,
        pressed: false,
        modifiers: mods,
    });
}

/// A pointer move to `pos` followed by a wheel delta there, in
/// [`egui::MouseWheelUnit::Point`] — the one unit
/// [`crate::input::EventTranslator`] passes through unscaled. The move is
/// not decorative: `EventTranslator` routes a wheel event to its *last*
/// pointer position and drops one with none, exactly as
/// `a_wheel_event_borrows_the_last_pointer_position` in `crate::input`
/// pins down, so a wheel event with no preceding move is a wheel event that
/// silently never arrives.
fn push_scroll(raw: &mut RawInput, pos: Point, delta: Size) {
    let at = to_pos2(pos);
    raw.events.push(Event::PointerMoved(at));
    raw.events.push(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: Vec2::new(delta.w, delta.h),
        phase: TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
}

/// A full keystroke: press, then release, matching a physical key tap.
fn push_key(raw: &mut RawInput, key: KeyCode, modifiers: Modifiers) -> Result<(), InjectError> {
    let egui_key = to_egui_key(key).ok_or(InjectError::UnsupportedKey(key))?;
    let mods = to_egui_modifiers(modifiers);
    for pressed in [true, false] {
        raw.events.push(Event::Key {
            key: egui_key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: mods,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Action, InjectError, Target, inject_action, to_egui_modifiers};
    use crate::host::{App, Host, default_presenter};
    use egui::{Context, RawInput};
    use gorgon_petra::input::{InputEvent, Route};
    use gorgon_petra::layout::{ChangeSet, RowSource};
    use gorgon_petra::tree::{
        Anchor, ClampRule, InputPolicy, Interaction, Layer, NodeKind, Props, Role, ViewNode,
    };
    use gorgon_petra::{KeyCode, Modifiers, Point, Size};
    use std::ops::Range;

    /// One node accepting every positional/focus interaction, one text
    /// field, and an optional `Block` modal over a focusable node of its
    /// own — enough surface to exercise all seven action kinds and the
    /// two routing guarantees (modal swallow, focus-scope refusal).
    #[derive(Default)]
    struct Demo {
        seen: Vec<(String, Route)>,
        modal: bool,
    }

    impl RowSource for Demo {
        fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<std::sync::Arc<ViewNode>> {
            Vec::new()
        }
    }

    fn describe(event: &InputEvent) -> String {
        match event {
            InputEvent::PointerMoved { .. } => "moved".into(),
            InputEvent::PointerPressed { .. } => "pressed".into(),
            InputEvent::PointerReleased { .. } => "released".into(),
            InputEvent::Scroll { .. } => "scroll".into(),
            InputEvent::Key { key, pressed, .. } => format!("key:{key:?}:{pressed}"),
            InputEvent::Text(text) => format!("text:{text}"),
            other => format!("{other:?}"),
        }
    }

    impl App for Demo {
        fn view(&mut self) -> ViewNode {
            let mut root = ViewNode::new(NodeKind::Stack, "root")
                .child(
                    ViewNode::new(NodeKind::Text, "run")
                        .with_props(Props {
                            text: Some("Run".into()),
                            ..Props::default()
                        })
                        .interactive(
                            Role::Button,
                            "Run",
                            &[
                                Interaction::Focus,
                                Interaction::Click,
                                Interaction::Drag,
                                Interaction::Hover,
                                Interaction::Scroll,
                            ],
                        ),
                )
                .child(
                    ViewNode::new(NodeKind::Input, "filter")
                        .with_props(Props {
                            placeholder: Some("Filter".into()),
                            ..Props::default()
                        })
                        .interactive(
                            Role::TextInput,
                            "Filter",
                            &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
                        ),
                );
            if self.modal {
                root = root.child(
                    ViewNode::new(NodeKind::Surface, "modal")
                        .with_props(Props {
                            layer: Some(Layer::Modal),
                            anchor: Some(Anchor::Viewport),
                            clamp: Some(ClampRule::Shrink),
                            input_policy: Some(InputPolicy::Block),
                            ..Props::default()
                        })
                        .child(
                            ViewNode::new(NodeKind::Text, "yes")
                                .with_props(Props {
                                    text: Some("Yes".into()),
                                    ..Props::default()
                                })
                                .interactive(
                                    Role::Button,
                                    "Yes",
                                    &[Interaction::Focus, Interaction::Click],
                                ),
                        ),
                );
            }
            root
        }

        fn handle(&mut self, event: &InputEvent, route: &Route) {
            self.seen.push((describe(event), route.clone()));
        }

        fn take_changes(&mut self) -> ChangeSet {
            ChangeSet::All
        }
    }

    fn headless() -> Context {
        let ctx = Context::default();
        ctx.run_ui(RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        ctx
    }

    fn step(ctx: &Context, host: &mut Host<Demo>, input: RawInput) {
        let out = ctx.run_ui(input, |_| host.pass(ctx));
        out.drop_without_applying_deltas();
    }

    /// The placement id ending with `suffix` in the current frame.
    fn id_ending(host: &Host<Demo>, suffix: &str) -> String {
        host.frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("no placement id ends with {suffix}"))
            .id
            .clone()
    }

    fn new_demo(modal: bool) -> (Context, Host<Demo>) {
        let ctx = headless();
        let mut host = Host::new(
            &ctx,
            Demo {
                modal,
                ..Demo::default()
            },
            default_presenter(),
        );
        step(&ctx, &mut host, RawInput::default());
        (ctx, host)
    }

    // -- click: real move+press+release, hit-tested and routed for real. --

    #[test]
    fn a_click_reaches_the_target_through_real_routing() {
        let (ctx, mut host) = new_demo(false);
        let run = id_ending(&host, "run");
        let mut raw = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(run.clone()),
            &Action::Click {
                modifiers: Modifiers::NONE,
            },
            &mut raw,
        )
        .unwrap();
        assert_eq!(raw.events.len(), 3, "{:?}", raw.events);

        host.app_mut().seen.clear();
        step(&ctx, &mut host, raw);

        let clicked: Vec<_> = host
            .app()
            .seen
            .iter()
            .filter(|(kind, route)| {
                (kind == "pressed" || kind == "released")
                    && matches!(route, Route::Pointer { node } if *node == run)
            })
            .collect();
        assert_eq!(
            clicked.len(),
            2,
            "a press and a release should both land on the button: {:?}",
            host.app().seen
        );
    }

    /// The sharp routing test: a click's coordinates sit on "run", which is
    /// outside the open modal's bounds. The real path swallows it — the
    /// same rule `host::tests::a_press_outside_an_open_modal_is_swallowed`
    /// pins for a physical press. A direct-invocation implementation that
    /// looked up "run" and called it straight through would show the click
    /// landing anyway; see this file's sabotage-and-restore log in the
    /// agent note for the red run this test produces under that fault.
    #[test]
    fn a_click_behind_an_open_modal_is_swallowed_by_the_real_router() {
        let (ctx, mut host) = new_demo(true);
        let run = id_ending(&host, "run");
        let mut raw = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(run.clone()),
            &Action::Click {
                modifiers: Modifiers::NONE,
            },
            &mut raw,
        )
        .unwrap();

        host.app_mut().seen.clear();
        step(&ctx, &mut host, raw);

        assert!(
            host.app()
                .seen
                .iter()
                .all(|(_, route)| !matches!(route, Route::Pointer { node } if *node == run)),
            "the modal must swallow this press, not deliver it to run: {:?}",
            host.app().seen
        );
        assert!(
            host.app()
                .seen
                .iter()
                .any(|(_, route)| matches!(route, Route::Unrouted { reason }
                    if *reason == "outside every currently-open Block surface's bounds")),
            "{:?}",
            host.app().seen
        );
    }

    // -- drag: press-move-move-release, both ends real. --

    #[test]
    fn a_drag_presses_at_the_target_and_releases_at_to() {
        let (ctx, mut host) = new_demo(false);
        let run = id_ending(&host, "run");
        let (run_rect, outside) = {
            let frame = host.frame().unwrap();
            let rect = frame.placement(&run).unwrap().rect;
            (
                rect,
                Point::new(rect.x + rect.w + 40.0, rect.y + rect.h + 40.0),
            )
        };
        let mut raw = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(run.clone()),
            &Action::Drag {
                to: outside,
                modifiers: Modifiers::NONE,
            },
            &mut raw,
        )
        .unwrap();
        assert_eq!(raw.events.len(), 5, "{:?}", raw.events);

        host.app_mut().seen.clear();
        step(&ctx, &mut host, raw);

        let (_, route) = host
            .app()
            .seen
            .iter()
            .find(|(kind, _)| kind == "pressed")
            .expect("a press was delivered");
        assert!(matches!(route, Route::Pointer { node } if *node == run));
        assert!(
            run_rect.contains(Point::new(run_rect.x + 1.0, run_rect.y + 1.0)),
            "sanity: run_rect is non-empty"
        );
    }

    // -- hover: a bare move, nothing else. --

    #[test]
    fn hover_is_a_bare_pointer_move() {
        let (ctx, mut host) = new_demo(false);
        let run = id_ending(&host, "run");
        let mut raw = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(run.clone()),
            &Action::Hover,
            &mut raw,
        )
        .unwrap();
        assert_eq!(raw.events.len(), 1);
        assert!(matches!(raw.events[0], egui::Event::PointerMoved(_)));

        host.app_mut().seen.clear();
        step(&ctx, &mut host, raw);
        assert_eq!(
            host.app().seen,
            vec![("moved".to_owned(), Route::Pointer { node: run })]
        );
    }

    // -- scroll: a move (so the translator has a position) then a wheel. --

    #[test]
    fn scroll_moves_the_pointer_before_wheeling_so_the_delta_is_not_dropped() {
        let (ctx, mut host) = new_demo(false);
        let run = id_ending(&host, "run");
        let mut raw = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(run.clone()),
            &Action::Scroll {
                delta: Size::new(0.0, -12.0),
            },
            &mut raw,
        )
        .unwrap();

        host.app_mut().seen.clear();
        step(&ctx, &mut host, raw);
        assert_eq!(
            host.app().seen,
            vec![
                ("moved".to_owned(), Route::Pointer { node: run.clone() }),
                ("scroll".to_owned(), Route::Pointer { node: run }),
            ]
        );
    }

    // -- key: press then release, routed by focus like a real keyboard. --

    #[test]
    fn key_presses_and_releases_reach_the_focused_node() {
        let (ctx, mut host) = new_demo(false);
        let filter = id_ending(&host, "filter");
        host.focus_mut().focus(&filter).unwrap();
        step(&ctx, &mut host, RawInput::default());

        let mut raw = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(filter.clone()),
            &Action::Key {
                key: KeyCode::Char('a'),
                modifiers: Modifiers::NONE,
            },
            &mut raw,
        )
        .unwrap();
        assert_eq!(raw.events.len(), 2);

        host.app_mut().seen.clear();
        step(&ctx, &mut host, raw);
        assert_eq!(
            host.app().seen,
            vec![
                (
                    "key:Char('a'):true".to_owned(),
                    Route::Keyboard {
                        node: filter.clone()
                    }
                ),
                (
                    "key:Char('a'):false".to_owned(),
                    Route::Keyboard { node: filter }
                ),
            ]
        );
    }

    // -- text-edit: committed text, routed by focus, not by the named target. --

    #[test]
    fn text_edit_lands_on_whichever_node_is_actually_focused() {
        let (ctx, mut host) = new_demo(false);
        let filter = id_ending(&host, "filter");
        host.focus_mut().focus(&filter).unwrap();
        step(&ctx, &mut host, RawInput::default());

        let mut raw = RawInput::default();
        // The target names "run" — a real node, just not the focused one.
        // A direct-invocation implementation could force text onto "run"
        // regardless; the real path cannot, because text always routes by
        // focus.
        let run = id_ending(&host, "run");
        inject_action(
            &mut host,
            &Target::NodeId(run),
            &Action::TextEdit { text: "hi".into() },
            &mut raw,
        )
        .unwrap();

        host.app_mut().seen.clear();
        step(&ctx, &mut host, raw);
        assert_eq!(
            host.app().seen,
            vec![("text:hi".to_owned(), Route::Keyboard { node: filter })]
        );
    }

    // -- focus: through the real focus tree, honoring the modal trap. --

    #[test]
    fn focus_moves_through_the_real_focus_tree() {
        let (ctx, mut host) = new_demo(false);
        let filter = id_ending(&host, "filter");
        let mut raw = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(filter.clone()),
            &Action::Focus,
            &mut raw,
        )
        .unwrap();
        assert!(
            raw.events.is_empty(),
            "focus is programmatic, not a device event"
        );
        assert_eq!(host.focus().current(), Some(filter.as_str()));
        let _ = ctx;
    }

    /// The claim this task exists to make checkable: `Action::Focus` reaches
    /// [`gorgon_petra::focus::FocusTree::focus`] and is therefore subject to
    /// its scope trap, rather than writing focus behind it. A raw write to
    /// `LayoutState::focused` would not refuse this and would leave focus on
    /// "run" — this test would then fail to observe the refusal and current()
    /// would show "run" instead of staying on "yes".
    #[test]
    fn focus_reaches_the_focus_rules_and_is_refused_outside_an_open_modal() {
        let (_ctx, mut host) = new_demo(true);
        let yes = id_ending(&host, "yes");
        let run = id_ending(&host, "run");
        host.focus_mut().focus(&yes).unwrap();
        assert_eq!(host.focus().current(), Some(yes.as_str()));

        let mut raw = RawInput::default();
        let err =
            inject_action(&mut host, &Target::NodeId(run), &Action::Focus, &mut raw).unwrap_err();
        assert!(
            matches!(&err, InjectError::OutsideActiveScope { scope, .. } if scope.ends_with("modal")),
            "{err:?}"
        );
        assert!(raw.events.is_empty());
        assert_eq!(
            host.focus().current(),
            Some(yes.as_str()),
            "a refused move must not have moved focus"
        );
    }

    // -- stale-node: the id is named, nothing is delivered, no fallthrough. --

    #[test]
    fn a_stale_node_id_fails_named_and_injects_nothing() {
        let (_ctx, mut host) = new_demo(false);
        let mut raw = RawInput::default();
        let err = inject_action(
            &mut host,
            &Target::NodeId("root/does-not-exist".into()),
            &Action::Click {
                modifiers: Modifiers::NONE,
            },
            &mut raw,
        )
        .unwrap_err();
        assert_eq!(err, InjectError::StaleNode("root/does-not-exist".into()));
        assert!(
            raw.events.is_empty(),
            "a stale target must inject nothing, not fall through to a coordinate: {:?}",
            raw.events
        );
    }

    #[test]
    fn a_stale_node_id_for_focus_fails_named_and_does_not_move_focus() {
        let (_ctx, mut host) = new_demo(false);
        let before = host.focus().current().map(str::to_owned);
        let mut raw = RawInput::default();
        let err = inject_action(
            &mut host,
            &Target::NodeId("nope".into()),
            &Action::Focus,
            &mut raw,
        )
        .unwrap_err();
        assert_eq!(err, InjectError::StaleNode("nope".into()));
        assert_eq!(host.focus().current().map(str::to_owned), before);
    }

    // -- modifiers round-trip: proves the doc comment's algebra. --

    #[test]
    fn modifiers_round_trip_through_translate_modifiers() {
        for m in [
            Modifiers::NONE,
            Modifiers::shift(),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
            Modifiers {
                meta: true,
                ..Modifiers::NONE
            },
            Modifiers {
                shift: true,
                ctrl: true,
                alt: true,
                meta: true,
            },
        ] {
            let back = crate::input::translate_modifiers(to_egui_modifiers(m));
            assert_eq!(back, m, "{m:?} did not round-trip");
        }
    }
}
