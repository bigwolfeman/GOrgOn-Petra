//! Inventory row 27, Radio button.

use gorgon_petra::component::{radio, radio_group, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const RADIO_A: &str = "radio-a";
const RADIO_B: &str = "radio-b";

/// Live state of the Radio button page.
#[derive(Default)]
pub struct RadioButton {
    radio: u8,
}

impl Page for RadioButton {
    fn row(&self) -> &'static str {
        "Radio button"
    }

    fn body(&self) -> ViewNode {
        section(
            "group",
            "Group",
            vec![body(
                "radios",
                sp("spacing.md"),
                vec![radio_group(
                    "radio-group",
                    "Theme",
                    vec![
                        radio(RADIO_A, "Dark", self.radio == 0),
                        radio(RADIO_B, "Light", self.radio == 1),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, RADIO_A) {
            self.radio = 0;
        } else if path_has(node, RADIO_B) {
            self.radio = 1;
        } else {
            return false;
        }
        true
    }
}
