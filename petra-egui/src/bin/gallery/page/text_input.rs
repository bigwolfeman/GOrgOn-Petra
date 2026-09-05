//! Inventory row 34, Text input.

use gorgon_petra::component::{
    field, field_invalid, field_lg, field_readonly, field_sm, labeled, section, valued,
};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const MD: &str = "field-md";
const SM: &str = "field-sm";
const LG: &str = "field-lg";

/// The Text input page.
///
/// It holds a value per editable field, which is the whole of what row 34
/// was missing. Every field constructor takes `(key, label)` and writes only
/// `props.placeholder`, so before this the page could not have contained
/// anything: the operator saw five empty wells and called them broken, which
/// they were.
///
/// Every field sits under Carbon's label (`labeled`), the way
/// `34-text-input.png` shows all five: label above, then the well. A bare
/// well with its name inside it as a placeholder is a search box, not a
/// text input.
pub struct TextInput {
    md: String,
    sm: String,
    lg: String,
}

impl Default for TextInput {
    fn default() -> Self {
        // The medium field starts with content and the other two start
        // empty, so one page shows both states side by side. A catalog page
        // where every field is empty cannot show that a value renders at all,
        // and a page where every field is full cannot show the placeholder.
        Self {
            md: "kernel-boot".to_owned(),
            sm: String::new(),
            lg: String::new(),
        }
    }
}

impl TextInput {
    /// The field the route names, if it names one of the three editable ones.
    fn target(&mut self, node: &str) -> Option<&mut String> {
        if path_has(node, MD) {
            Some(&mut self.md)
        } else if path_has(node, SM) {
            Some(&mut self.sm)
        } else if path_has(node, LG) {
            Some(&mut self.lg)
        } else {
            None
        }
    }
}

impl Page for TextInput {
    fn row(&self) -> &'static str {
        "Text input"
    }

    fn body(&self) -> ViewNode {
        section(
            "fields",
            "Default sizes and states",
            vec![filled_body(
                "inputs",
                sp("spacing.md"),
                vec![
                    labeled(
                        "fiber-name",
                        "Fiber name",
                        valued(field(MD, "Fiber name"), self.md.clone()),
                    ),
                    labeled(
                        "small",
                        "Small",
                        valued(field_sm(SM, "Small input"), self.sm.clone()),
                    ),
                    labeled(
                        "large",
                        "Large",
                        valued(field_lg(LG, "Large input"), self.lg.clone()),
                    ),
                    labeled(
                        "port",
                        "Port",
                        field_invalid("field-bad", "Port", "must be a number"),
                    ),
                    labeled(
                        "read-only",
                        "Read only",
                        field_readonly("field-ro", "Read-only value"),
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        match event {
            InputEvent::Text(typed) => {
                let Some(value) = self.target(node) else {
                    return false;
                };
                value.push_str(typed);
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } => {
                let Some(value) = self.target(node) else {
                    return false;
                };
                value.pop();
                true
            }
            _ => false,
        }
    }
}
