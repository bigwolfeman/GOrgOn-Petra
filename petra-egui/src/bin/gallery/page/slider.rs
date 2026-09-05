//! Inventory row 30, Slider.

use gorgon_petra::component::{
    section, slider, slider_input_text, slider_value_at, slider_value_of_input, valued,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const VOLUME: &str = "vol";
/// The paired number input's key inside `slider`.
const INPUT: &str = "input";

/// Live state of the Slider page: the one value the handle drags, and
/// whether a drag is in flight.
pub struct Slider {
    /// Normalised into `[0, 1]`, the same range `slider` takes.
    volume: f32,
    /// What the paired number input shows. Rewritten from `volume` by a
    /// drag; edited a keystroke at a time by the keyboard, and parsed back
    /// into `volume` when it names a number, so a half-typed field does not
    /// jump the handle to zero.
    draft: String,
    /// Whether a press on the slider opened a gesture that has not ended.
    ///
    /// **This is the whole fix for "it drags when my mouse is close to it,
    /// whether I click it or not."** The handle declares `Hover`, so a
    /// pointer move over it routes to the page as a `PointerMoved` naming
    /// the handle — and that is the same event a drag delivers. The old
    /// `gesture` took a position from every move it was handed and could
    /// not tell a hover from a drag; the operator's pointer passing near the
    /// handle moved the value, and a fast move that left the 14-unit handle
    /// stopped "dragging" because nothing was captured and the hover ended.
    ///
    /// The engine's rule is that a press on a `Drag` node grants the
    /// capture, every positional event until the gesture ends routes to the
    /// holder unconditionally, and the end is always announced
    /// (`GestureEnded`, on release, blur, Escape, pointer-exit or the node
    /// vanishing). This bit mirrors exactly that: set by the press, cleared
    /// by the end, and no move is a drag without it.
    dragging: bool,
}

impl Default for Slider {
    fn default() -> Self {
        Self {
            volume: 0.4,
            draft: slider_input_text(0.4),
            dragging: false,
        }
    }
}

impl Slider {
    fn set_from_pointer(&mut self, value: f32) {
        self.volume = value;
        self.draft = slider_input_text(value);
    }

    fn set_from_draft(&mut self) {
        if let Some(value) = slider_value_of_input(&self.draft) {
            self.volume = value;
        }
    }
}

impl Page for Slider {
    fn row(&self) -> &'static str {
        "Slider"
    }

    fn body(&self) -> ViewNode {
        section(
            "slide",
            "Slider",
            vec![body(
                "slid",
                sp("spacing.md"),
                vec![valued(
                    slider(VOLUME, "Volume", self.volume),
                    self.draft.clone(),
                )],
            )],
        )
    }

    /// The paired number input is a text field: digits append, backspace
    /// takes one back, and the handle follows whenever the text names a
    /// number.
    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if !(path_has(node, VOLUME) && path_has(node, INPUT)) {
            return false;
        }
        match event {
            InputEvent::Text(typed) => {
                let digits: String = typed.chars().filter(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    return false;
                }
                self.draft.push_str(&digits);
                self.set_from_draft();
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } => {
                self.draft.pop();
                self.set_from_draft();
                true
            }
            _ => false,
        }
    }

    /// The press seats the value under the pointer and opens the drag;
    /// every move and the release while it is open follow the pointer; the
    /// gesture's end closes it. A move with no drag open — a hover — is
    /// news the page does not act on, whatever node it names.
    fn gesture(&mut self, event: &InputEvent, node: &str, frame: &PetrifiedFrame) -> bool {
        if !path_has(node, VOLUME) || path_has(node, INPUT) {
            return false;
        }
        match event {
            InputEvent::GestureEnded { .. } => {
                // Carries no position and changes nothing: the last move
                // already put the value where the pointer let go.
                self.dragging = false;
                true
            }
            InputEvent::PointerPressed { pos, .. } => {
                let Some(value) = slider_value_at(frame, node, *pos) else {
                    return false;
                };
                self.dragging = true;
                self.set_from_pointer(value);
                true
            }
            InputEvent::PointerMoved { pos } | InputEvent::PointerReleased { pos, .. } => {
                if !self.dragging {
                    return false;
                }
                match slider_value_at(frame, node, *pos) {
                    Some(value) => {
                        self.set_from_pointer(value);
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Slider, VOLUME};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use gorgon_petra::geom::{Point, Size};
    use gorgon_petra::input::{GestureOutcome, InputEvent, KeyCode, Modifiers, PointerButton};
    use gorgon_petra::testing::{Harness, validated_with};
    use gorgon_petra::token::{ThemeMode, standard_vocabulary};
    use gorgon_petra::tree::Registry;

    fn value_of(page: &Slider) -> String {
        find(&page.body(), VOLUME)
            .unwrap()
            .semantics
            .value
            .clone()
            .unwrap()
    }

    fn input_text(page: &Slider) -> String {
        find(&page.body(), "input")
            .expect("the paired input is in the tree")
            .props
            .text
            .clone()
            .expect("the input shows text")
    }

    /// The page's frame, with the handle's id, its centre, and the rail's
    /// rect.
    fn placed(page: &Slider) -> (PetrifiedFrame, String, Point, gorgon_petra::geom::Rect) {
        let root = page.body();
        let registry = Registry::with_vocabulary(standard_vocabulary());
        let mut harness = Harness::new();
        let viewport = Viewport::new(Size::new(900.0, 700.0), ThemeMode::Dark);
        harness.scale = viewport.scale;
        let frame = petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        );
        let handle = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/rail/handle"))
            .expect("the handle is placed");
        let rail = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/row/rail"))
            .expect("the rail is placed")
            .rect;
        let id = handle.id.clone();
        let centre = Point::new(
            handle.rect.x + handle.rect.w / 2.0,
            handle.rect.y + handle.rect.h / 2.0,
        );
        (frame, id, centre, rail)
    }

    fn pressed(pos: Point) -> InputEvent {
        InputEvent::PointerPressed {
            pos,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    fn released(pos: Point) -> InputEvent {
        InputEvent::PointerReleased {
            pos,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    /// A press on the handle, then a move under capture, then a release:
    /// the value follows the pointer along the rail and the readout the
    /// tree carries changes with it. The paired input follows too.
    #[test]
    fn a_drag_along_the_rail_moves_the_value_the_way_the_pointer_went() {
        let mut page = Slider::default();
        assert_eq!(value_of(&page), "40%");
        assert_eq!(input_text(&page), "40");
        let (frame, handle_id, start, rail) = placed(&page);
        let end = Point::new(rail.x + rail.w * 0.85, start.y);

        assert!(page.gesture(&pressed(start), &handle_id, &frame));
        assert_eq!(value_of(&page), "40%", "a press on the handle holds still");
        assert!(page.gesture(&InputEvent::PointerMoved { pos: end }, &handle_id, &frame));
        let after_move = page.volume;
        assert!(
            after_move > 0.4,
            "the pointer went right, so the value went up: {after_move}"
        );
        assert!(page.gesture(&released(end), &handle_id, &frame));
        assert_eq!(page.volume, after_move, "the release leaves it there");
        assert!(page.gesture(
            &InputEvent::GestureEnded {
                node: handle_id.clone(),
                outcome: GestureOutcome::Completed,
            },
            &handle_id,
            &frame,
        ));
        assert_ne!(value_of(&page), "40%");
        assert_eq!(input_text(&page), format!("{:.0}", after_move * 100.0));

        // A gesture on some other page's control is left alone.
        assert!(!page.gesture(
            &InputEvent::PointerMoved { pos: end },
            "/page/shell/main-scroll/main/nav/next",
            &frame,
        ));
        assert_eq!(page.volume, after_move);
    }

    /// **The operator's bug.** A pointer moving over the handle with no
    /// button down is a hover, and a hover routes to the page as the same
    /// `PointerMoved` a drag does. It must move nothing: no press opened a
    /// gesture. And after a gesture has ended, a move is a hover again.
    #[test]
    fn a_move_with_no_press_is_a_hover_and_moves_nothing() {
        let mut page = Slider::default();
        let (frame, handle_id, start, rail) = placed(&page);
        let far = Point::new(rail.x + rail.w * 0.9, start.y);

        assert!(
            !page.gesture(&InputEvent::PointerMoved { pos: far }, &handle_id, &frame),
            "a move with no press is not the page's to act on"
        );
        assert_eq!(
            page.volume, 0.4,
            "hovering across the track moved the value"
        );

        // A whole gesture, then its end, then a hover: still nothing.
        assert!(page.gesture(&pressed(start), &handle_id, &frame));
        assert!(page.gesture(&released(start), &handle_id, &frame));
        assert!(page.gesture(
            &InputEvent::GestureEnded {
                node: handle_id.clone(),
                outcome: GestureOutcome::Completed,
            },
            &handle_id,
            &frame,
        ));
        let settled = page.volume;
        assert!(!page.gesture(&InputEvent::PointerMoved { pos: far }, &handle_id, &frame));
        assert_eq!(
            page.volume, settled,
            "a hover after the gesture ended moved the value"
        );
    }

    /// A press on the track, away from the handle, jumps the value there
    /// (Carbon: "clicking anywhere on the track jumps the value to that
    /// point") and opens a drag from it.
    #[test]
    fn a_press_on_the_track_jumps_the_value_there() {
        let mut page = Slider::default();
        let (frame, _, start, rail) = placed(&page);
        let rail_id = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/row/rail"))
            .unwrap()
            .id
            .clone();
        let at = Point::new(rail.x + rail.w * 0.8, start.y);
        assert!(page.gesture(&pressed(at), &rail_id, &frame));
        assert!(
            (page.volume - 0.8).abs() < 0.05,
            "the press put the value under the pointer: {}",
            page.volume
        );
        let further = Point::new(rail.x + rail.w * 0.95, start.y);
        assert!(page.gesture(&InputEvent::PointerMoved { pos: further }, &rail_id, &frame));
        assert!(page.volume > 0.9, "and the drag that followed moved it on");
    }

    /// Typing into the paired input moves the handle, and a half-typed
    /// field does not jump it to zero.
    #[test]
    fn typing_a_number_into_the_input_moves_the_handle() {
        let mut page = Slider::default();
        let input = "/page/x/slid/vol/row/input";
        let backspace = InputEvent::Key {
            key: KeyCode::Backspace,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(page.handle(&backspace, input));
        assert_eq!(input_text(&page), "4");
        assert!((page.volume - 0.04).abs() < 1e-6, "\"4\" names 4%");
        assert!(page.handle(&backspace, input));
        assert_eq!(input_text(&page), "");
        assert!(
            (page.volume - 0.04).abs() < 1e-6,
            "an emptied field names nothing and leaves the handle where it was"
        );
        assert!(page.handle(&InputEvent::Text("7".to_owned()), input));
        assert!(page.handle(&InputEvent::Text("5".to_owned()), input));
        assert_eq!(input_text(&page), "75");
        assert!((page.volume - 0.75).abs() < 1e-6);
        assert_eq!(value_of(&page), "75%");
        assert!(
            !page.handle(&InputEvent::Text("x".to_owned()), input),
            "a letter is refused"
        );
        assert!(
            !page.handle(
                &InputEvent::Text("1".to_owned()),
                "/page/x/slid/vol/row/rail"
            ),
            "text aimed at the rail is not the input's"
        );
    }
}
