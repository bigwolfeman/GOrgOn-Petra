//! Inventory row 27, Radio button.

use gorgon_petra::component::{disabled, radio, radio_group, radio_warning, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const RADIO_A: &str = "radio-a";
const RADIO_B: &str = "radio-b";

/// Live state of the Radio button page.
///
/// The group is labelled Theme and its two choices are Dark and Light, so
/// they switch the theme. A radio group that names a thing and does not do
/// it is the same defect as row 34's Port field: the label promises and the
/// control does not answer.
#[derive(Default)]
pub struct RadioButton {
    radio: u8,
    /// Set by a press, taken by the chrome on the next pass. `None` between
    /// presses, so the theme is published once and not re-published every
    /// frame.
    pending: Option<ThemeMode>,
}

impl Page for RadioButton {
    fn row(&self) -> &'static str {
        "Radio button"
    }

    /// Carbon's radio-button skeleton (`_radio-button.scss`): the label bar,
    /// 18×100 — the appearance slot is hidden (`display: none`), not
    /// skeletonized.
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        Some(skeleton(
            "skeleton-label",
            SkeletonAnatomy::Label,
            SizeStep::Md,
            100.0,
            now,
        ))
    }

    /// The documented disabled state (spec 008 T025, register row 397): the
    /// ring and the selected fill fade to `icon-disabled`, the label to the
    /// label rule's `text.muted` — off and on both shown, because the two
    /// marks bind different slots.
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![
                radio("disabled-off", "Unavailable", false),
                radio("disabled-on", "Unavailable", true),
            ],
        ))]
    }

    fn body(&self) -> ViewNode {
        section(
            "group",
            "Group",
            vec![body(
                "radios",
                sp("spacing.md"),
                vec![
                    radio_group(
                        "radio-group",
                        "Theme",
                        vec![
                            radio(RADIO_A, "Dark", self.radio == 0),
                            radio(RADIO_B, "Light", self.radio == 1),
                            // T0.1: the disabled ink family, on both an
                            // unselected and a selected radio, so the border
                            // fade and the fill swap both show in one shot.
                            disabled(radio("radio-disabled", "Disabled", false)),
                            disabled(radio("radio-disabled-on", "Disabled, on", true)),
                        ],
                    ),
                    radio_warning("radio-warn", "Unusual choice", true, "confirm this"),
                ],
            )],
        )
    }

    fn theme_request(&mut self) -> Option<ThemeMode> {
        self.pending.take()
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, RADIO_A) {
            self.radio = 0;
            self.pending = Some(ThemeMode::Dark);
        } else if path_has(node, RADIO_B) {
            self.radio = 1;
            self.pending = Some(ThemeMode::Light);
        } else {
            return false;
        }
        true
    }
}
