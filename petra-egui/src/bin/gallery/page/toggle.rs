//! Inventory row 36, Toggle.

use gorgon_petra::component::{disabled, section, toggle, toggle_sm};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const TOGGLE_DEFAULT_OFF: &str = "toggle-default-off";
const TOGGLE_DEFAULT_ON: &str = "toggle-default-on";
const TOGGLE_SM_OFF: &str = "toggle-sm-off";
const TOGGLE_SM_ON: &str = "toggle-sm-on";

/// Live state of the Toggle page.
pub struct Toggle {
    default_off: bool,
    default_on: bool,
    small_off: bool,
    small_on: bool,
}

impl Default for Toggle {
    fn default() -> Self {
        Self {
            default_off: false,
            default_on: true,
            small_off: false,
            small_on: true,
        }
    }
}

impl Page for Toggle {
    fn row(&self) -> &'static str {
        "Toggle"
    }

    fn body(&self) -> ViewNode {
        section(
            "states",
            "States",
            vec![body(
                "toggles",
                sp("spacing.md"),
                vec![
                    toggle(TOGGLE_DEFAULT_OFF, "Default off", self.default_off),
                    toggle(TOGGLE_DEFAULT_ON, "Default on", self.default_on),
                    toggle_sm(TOGGLE_SM_OFF, "Small off", self.small_off),
                    toggle_sm(TOGGLE_SM_ON, "Small on", self.small_on),
                    // T0.1: the disabled ink family, off and on, so the
                    // track's grey fill and the faded label both show
                    // against the two live tracks above them.
                    disabled(toggle("toggle-disabled-off", "Disabled off", false)),
                    disabled(toggle("toggle-disabled-on", "Disabled on", true)),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, TOGGLE_DEFAULT_OFF) {
            self.default_off = !self.default_off;
        } else if path_has(node, TOGGLE_DEFAULT_ON) {
            self.default_on = !self.default_on;
        } else if path_has(node, TOGGLE_SM_OFF) {
            self.small_off = !self.small_off;
        } else if path_has(node, TOGGLE_SM_ON) {
            self.small_on = !self.small_on;
        } else {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{TOGGLE_DEFAULT_OFF, TOGGLE_DEFAULT_ON, TOGGLE_SM_OFF, TOGGLE_SM_ON, Toggle};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::geom::Point;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    fn press(page: &mut Toggle, node: &str) -> bool {
        page.handle(
            &InputEvent::PointerPressed {
                pos: Point::ZERO,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            node,
        )
    }

    fn selected(page: &Toggle, key: &str) -> bool {
        find(&page.body(), key).unwrap().semantics.selected
    }

    #[test]
    fn clicking_a_toggle_flips_application_state() {
        let mut page = Toggle::default();
        assert!(!selected(&page, TOGGLE_DEFAULT_OFF));
        assert!(selected(&page, TOGGLE_DEFAULT_ON));
        assert!(!selected(&page, TOGGLE_SM_OFF));
        assert!(selected(&page, TOGGLE_SM_ON));

        for key in [
            TOGGLE_DEFAULT_OFF,
            TOGGLE_DEFAULT_ON,
            TOGGLE_SM_OFF,
            TOGGLE_SM_ON,
        ] {
            assert!(
                press(&mut page, &format!("/page/root/{key}")),
                "{key} is this page's own control, so the press is consumed"
            );
        }

        assert!(selected(&page, TOGGLE_DEFAULT_OFF));
        assert!(!selected(&page, TOGGLE_DEFAULT_ON));
        assert!(selected(&page, TOGGLE_SM_OFF));
        assert!(!selected(&page, TOGGLE_SM_ON));
    }

    /// A press on a child of the toggle (the knob) must still flip it.
    #[test]
    fn a_press_on_the_knob_flips_the_toggle() {
        let mut page = Toggle::default();
        let knob =
            "/page/shell/main-scroll/main/states/toggles/toggle-default-off/appearance/track/knob";
        assert!(!page.default_off);
        assert!(press(&mut page, knob));
        assert!(
            page.default_off,
            "a press on the knob must flip the control"
        );
        assert!(press(&mut page, knob));
        assert!(
            !page.default_off,
            "a second press on the same knob must flip again"
        );
    }

    /// A node that is not one of this page's controls is left for the chrome.
    #[test]
    fn a_press_elsewhere_is_not_consumed() {
        let mut page = Toggle::default();
        assert!(!press(&mut page, "/page/shell/main-scroll/main/nav/next"));
        assert!(!page.default_off);
    }
}
