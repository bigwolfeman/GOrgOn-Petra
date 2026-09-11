//! Catalog row 47, Input group.

use gorgon_petra::component::{
    button, field, input_group, input_group_with_addon, labeled, section, valued,
};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const HOST: &str = "ig-host";
const HOST_PAIR: &str = "ig-host-pair";
const PORT: &str = "ig-port";

/// Live state of the Input group page: two wells in a spaced row.
pub struct InputGroup {
    host: String,
    port: String,
}

impl Default for InputGroup {
    fn default() -> Self {
        Self {
            host: "kernel.local".to_owned(),
            port: "9p".to_owned(),
        }
    }
}

impl Page for InputGroup {
    fn row(&self) -> &'static str {
        "Input group"
    }

    fn body(&self) -> ViewNode {
        section(
            "group",
            "Spaced addons",
            vec![filled_body(
                "ig-body",
                sp("spacing.md"),
                vec![
                    labeled(
                        "host-item",
                        "Host",
                        input_group_with_addon(
                            "ig-addon",
                            button("ig-prefix", "9p://"),
                            valued(field(HOST, "Host"), self.host.clone()),
                        ),
                    ),
                    labeled(
                        "pair-item",
                        "Host and port",
                        input_group(
                            "ig-pair",
                            vec![
                                valued(field(HOST_PAIR, "Host"), self.host.clone()),
                                valued(field(PORT, "Port"), self.port.clone()),
                                button("ig-go", "Connect"),
                            ],
                        ),
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        let target = if path_has(node, HOST) || path_has(node, HOST_PAIR) {
            Some(&mut self.host)
        } else if path_has(node, PORT) {
            Some(&mut self.port)
        } else {
            None
        };
        let Some(value) = target else {
            return false;
        };
        match event {
            InputEvent::Text(typed) => {
                value.push_str(typed);
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } => {
                value.pop();
                true
            }
            _ => false,
        }
    }
}
