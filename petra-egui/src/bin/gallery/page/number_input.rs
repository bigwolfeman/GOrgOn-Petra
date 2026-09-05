//! Inventory row 22, Number input.

use gorgon_petra::component::{labeled, number_input, section};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const COUNT: &str = "n-md";
/// The value cell's key inside `number_input`.
const VALUE: &str = "value";
/// The stepper keys inside `number_input`.
const INCREMENT: &str = "increment";
const DECREMENT: &str = "decrement";

/// The Number input page.
///
/// It holds the value as the text the field shows, so the steppers and the
/// keyboard both edit the same thing. A press on a stepper parses the text,
/// steps it and writes it back; a keystroke appends a digit. Before this
/// the page held nothing and the steppers were two dead squares, which is
/// not what Carbon's `22-number-input.png` shows.
pub struct NumberInput {
    value: String,
}

impl Default for NumberInput {
    fn default() -> Self {
        Self {
            value: "12".to_owned(),
        }
    }
}

impl NumberInput {
    /// The number the field holds, or zero when it holds no number — an
    /// empty field stepped up reads `1`, which is what a person expects.
    fn number(&self) -> i64 {
        self.value.trim().parse().unwrap_or(0)
    }

    fn step(&mut self, by: i64) {
        self.value = (self.number() + by).to_string();
    }
}

impl Page for NumberInput {
    fn row(&self) -> &'static str {
        "Number input"
    }

    fn body(&self) -> ViewNode {
        section(
            "number",
            "Number input",
            vec![filled_body(
                "num",
                sp("spacing.md"),
                vec![labeled(
                    "count",
                    "Count",
                    number_input(COUNT, "Count", self.value.clone()),
                )],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if !path_has(node, COUNT) {
            return false;
        }
        match event {
            InputEvent::Text(typed) if path_has(node, VALUE) => {
                // Digits and a leading sign only: the field is numeric, and
                // a letter typed into it is refused rather than parsed away
                // on the next step.
                let accepted: String = typed
                    .chars()
                    .filter(|c| c.is_ascii_digit() || (*c == '-' && self.value.is_empty()))
                    .collect();
                if accepted.is_empty() {
                    return false;
                }
                self.value.push_str(&accepted);
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } if path_has(node, VALUE) => {
                self.value.pop();
                true
            }
            // Anything else that reaches here is an activation the chrome
            // already accepted: a press, Enter or Space on a stepper.
            _ if path_has(node, INCREMENT) => {
                self.step(1);
                true
            }
            _ if path_has(node, DECREMENT) => {
                self.step(-1);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NumberInput;
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::geom::Point;
    use gorgon_petra::input::{InputEvent, KeyCode, Modifiers, PointerButton};

    fn shown(page: &NumberInput) -> String {
        find(&page.body(), "value")
            .expect("the value cell is in the tree")
            .props
            .text
            .clone()
            .expect("the value cell shows text")
    }

    fn press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    /// A press on either stepper moves the number by one, and the tree
    /// shows the new number.
    #[test]
    fn the_steppers_move_the_value_by_one() {
        let mut page = NumberInput::default();
        assert_eq!(shown(&page), "12");
        assert!(page.handle(&press(), "/page/x/num/count/n-md/increment"));
        assert_eq!(shown(&page), "13");
        assert!(page.handle(&press(), "/page/x/num/count/n-md/decrement"));
        assert!(page.handle(&press(), "/page/x/num/count/n-md/decrement"));
        assert_eq!(shown(&page), "11");
        assert!(
            !page.handle(&press(), "/page/x/other/increment"),
            "another page's stepper is left alone"
        );
    }

    /// Digits typed into the value cell append; letters are refused;
    /// backspace takes one back.
    #[test]
    fn typing_edits_the_value_and_refuses_letters() {
        let mut page = NumberInput::default();
        let cell = "/page/x/num/count/n-md/value";
        assert!(page.handle(&InputEvent::Text("3".to_owned()), cell));
        assert_eq!(shown(&page), "123");
        assert!(!page.handle(&InputEvent::Text("x".to_owned()), cell));
        assert_eq!(shown(&page), "123");
        assert!(page.handle(
            &InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            cell
        ));
        assert_eq!(shown(&page), "12");
    }
}
