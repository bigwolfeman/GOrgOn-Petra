//! Catalog row 49, OTP.

use gorgon_petra::component::{otp, section};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const GROUP: &str = "otp";
const LENGTH: u32 = 6;

/// Live state of the OTP page: the digits the wells show.
pub struct Otp {
    value: String,
}

impl Default for Otp {
    fn default() -> Self {
        Self {
            value: "204".to_owned(),
        }
    }
}

impl Page for Otp {
    fn row(&self) -> &'static str {
        "OTP"
    }

    fn body(&self) -> ViewNode {
        section(
            "code",
            "One-time code",
            vec![filled_body(
                "otp-body",
                sp("spacing.md"),
                vec![otp(GROUP, LENGTH, &self.value)],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if !path_has(node, GROUP) {
            return false;
        }
        match event {
            InputEvent::Text(typed) => {
                for c in typed.chars() {
                    if self.value.len() >= LENGTH as usize {
                        break;
                    }
                    if c.is_ascii_digit() {
                        self.value.push(c);
                    }
                }
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } => {
                self.value.pop();
                true
            }
            _ => false,
        }
    }
}
