//! Inventory row 30, Slider.

use gorgon_petra::component::{section, slider, slider_value_at};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const VOLUME: &str = "vol";

/// Live state of the Slider page: the one value the handle drags.
pub struct Slider {
    /// Normalised into `[0, 1]`, the same range `slider` takes.
    volume: f32,
}

impl Default for Slider {
    fn default() -> Self {
        Self { volume: 0.4 }
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
                vec![slider(VOLUME, "Volume", self.volume)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }

    /// The press seats the value under the pointer, every move under
    /// capture follows it, and the release leaves it where it is. All three
    /// read the same way: the pointer's position along the rail this frame
    /// placed, which is what `slider_value_at` answers from any node inside
    /// the slider.
    fn gesture(&mut self, event: &InputEvent, node: &str, frame: &PetrifiedFrame) -> bool {
        if !path_has(node, VOLUME) {
            return false;
        }
        let Some(pos) = event.pointer_pos() else {
            // `GestureEnded` carries no position and changes nothing: the
            // last move already put the value where the pointer let go.
            return matches!(event, InputEvent::GestureEnded { .. });
        };
        match slider_value_at(frame, node, pos) {
            Some(value) => {
                self.volume = value;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Slider, VOLUME};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
    use gorgon_petra::geom::{Point, Size};
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};
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

    /// A press on the handle, then a move under capture, then a release:
    /// the value follows the pointer along the rail and the readout the
    /// tree carries changes with it.
    #[test]
    fn a_drag_along_the_rail_moves_the_value_the_way_the_pointer_went() {
        let mut page = Slider::default();
        assert_eq!(value_of(&page), "40%");

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
        let handle_id = handle.id.clone();
        let start = Point::new(
            handle.rect.x + handle.rect.w / 2.0,
            handle.rect.y + handle.rect.h / 2.0,
        );
        let end = Point::new(rail.x + rail.w * 0.85, start.y);

        assert!(page.gesture(
            &InputEvent::PointerPressed {
                pos: start,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            &handle_id,
            &frame,
        ));
        assert_eq!(value_of(&page), "40%", "a press on the handle holds still");
        assert!(page.gesture(&InputEvent::PointerMoved { pos: end }, &handle_id, &frame));
        let after_move = page.volume;
        assert!(
            after_move > 0.4,
            "the pointer went right, so the value went up: {after_move}"
        );
        assert!(page.gesture(
            &InputEvent::PointerReleased {
                pos: end,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            &handle_id,
            &frame,
        ));
        assert_eq!(page.volume, after_move, "the release leaves it there");
        assert_ne!(value_of(&page), "40%");

        // A gesture on some other page's control is left alone.
        assert!(!page.gesture(
            &InputEvent::PointerMoved { pos: end },
            "/page/shell/main-scroll/main/nav/next",
            &frame,
        ));
        assert_eq!(page.volume, after_move);
    }
}
