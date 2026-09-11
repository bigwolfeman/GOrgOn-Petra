//! Catalog row 51, Textarea.

use gorgon_petra::component::{
    labeled, section, textarea, textarea_invalid, textarea_warning, valued,
};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const NOTES: &str = "ta-notes";
const BAD: &str = "ta-bad";
const WARN: &str = "ta-warn";

/// Live state of the Textarea page.
pub struct Textarea {
    notes: String,
}

impl Default for Textarea {
    fn default() -> Self {
        Self {
            notes: "The fiber scheduled this pass.".to_owned(),
        }
    }
}

impl Page for Textarea {
    fn row(&self) -> &'static str {
        "Textarea"
    }

    fn body(&self) -> ViewNode {
        section(
            "areas",
            "Default, invalid, warning",
            vec![filled_body(
                "ta-body",
                sp("spacing.md"),
                vec![
                    labeled(
                        "notes-item",
                        "Notes",
                        valued(textarea(NOTES, "Notes"), self.notes.clone()),
                    ),
                    labeled(
                        "bad-item",
                        "Invalid",
                        textarea_invalid(BAD, "Invalid", "must not be empty"),
                    ),
                    labeled(
                        "warn-item",
                        "Warning",
                        textarea_warning(WARN, "Warning", "looks old"),
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if !path_has(node, NOTES) {
            return false;
        }
        match event {
            InputEvent::Text(typed) => {
                self.notes.push_str(typed);
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } => {
                self.notes.pop();
                true
            }
            _ => false,
        }
    }
}
