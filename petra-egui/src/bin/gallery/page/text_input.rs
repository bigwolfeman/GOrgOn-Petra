//! Inventory row 34, Text input.

use gorgon_petra::component::{
    field, field_lg, field_readonly, field_sm, field_validated, labeled, section, valued,
};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const MD: &str = "field-md";
const SM: &str = "field-sm";
const LG: &str = "field-lg";
/// The validating field. One key for both the valid and the invalid build,
/// so flipping between them does not drop focus mid-edit.
const PORT: &str = "field-port";
/// The read-only field.
const RO: &str = "field-ro";

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
    /// The validating field's text. Whether it is invalid is *derived* from
    /// this string by [`TextInput::port_is_valid`], never stored, so the two
    /// cannot disagree.
    port: String,
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
            // Starts wrong on purpose, so the page shows the invalid state
            // at rest the way Carbon's own docs page does. Backspace it to
            // digits and the error goes; type a letter and it comes back.
            port: "http".to_owned(),
        }
    }
}

impl TextInput {
    /// The field the route names, if it names one of the four editable ones.
    fn target(&mut self, node: &str) -> Option<&mut String> {
        if path_has(node, MD) {
            Some(&mut self.md)
        } else if path_has(node, SM) {
            Some(&mut self.sm)
        } else if path_has(node, LG) {
            Some(&mut self.lg)
        } else if path_has(node, PORT) {
            Some(&mut self.port)
        } else {
            None
        }
    }

    /// A port is a number, and nothing else. An empty field is not a number.
    fn port_is_valid(&self) -> bool {
        !self.port.is_empty() && self.port.chars().all(|c| c.is_ascii_digit())
    }

    /// The validating field, built from the text it currently holds.
    ///
    /// Until 2026-09-05 this row built `field_invalid(.., "must be a
    /// number")` unconditionally, with no state behind it. It read like a
    /// validating field, it accepted no keystroke, and it said "must be a
    /// number" whatever you typed. The operator typed numbers into it and it
    /// went on complaining, which is exactly what a hardcoded error string
    /// does. The error is derived from the value now, so the message can
    /// only ever be true.
    fn port_field(&self) -> ViewNode {
        let message = (!self.port_is_valid()).then(|| "must be a number".to_owned());
        valued(field_validated(PORT, "Port", message), self.port.clone())
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
                    labeled("port", "Port", self.port_field()),
                    labeled(
                        "read-only",
                        "Read only",
                        // A read-only well with nothing in it shows its
                        // placeholder, which is indistinguishable from an
                        // empty editable one. The point of the row is that
                        // the value is there and cannot be changed.
                        valued(field_readonly(RO, "Read-only value"), "9p://kernel/0"),
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
