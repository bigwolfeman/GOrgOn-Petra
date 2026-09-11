//! Catalog row 47, Input group.

use gorgon_petra::component::{
    button, field, input_group, input_group_seamless, input_group_with_addon,
    input_group_with_addon_seamless, labeled, section, valued,
};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{column, filled_body, path_has, sp};

const HOST: &str = "ig-host";
const HOST_PAIR: &str = "ig-host-pair";
const PORT: &str = "ig-port";
const HOST_SEAMLESS: &str = "ig-host-seamless";
const HOST_PAIR_SEAMLESS: &str = "ig-host-pair-seamless";
const PORT_SEAMLESS: &str = "ig-port-seamless";

/// Live state of the Input group page: spaced and seamless rows sharing the
/// same host/port text, so the two anatomies can be compared side by side.
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
        column(
            "group",
            sp("spacing.lg"),
            vec![
                section(
                    "spaced",
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
                ),
                // Spec 009 T040: the same two rows, zero gap and squared
                // inner corners at the seam. Same values, so the only
                // visible difference between this section and the one
                // above is the anatomy the seam draws.
                section(
                    "seamless",
                    "Seamless addons",
                    vec![filled_body(
                        "ig-body-seamless",
                        sp("spacing.md"),
                        vec![
                            labeled(
                                "host-item-seamless",
                                "Host",
                                input_group_with_addon_seamless(
                                    "ig-addon-seamless",
                                    button("ig-prefix-seamless", "9p://"),
                                    valued(field(HOST_SEAMLESS, "Host"), self.host.clone()),
                                ),
                            ),
                            labeled(
                                "pair-item-seamless",
                                "Host and port",
                                input_group_seamless(
                                    "ig-pair-seamless",
                                    vec![
                                        valued(
                                            field(HOST_PAIR_SEAMLESS, "Host"),
                                            self.host.clone(),
                                        ),
                                        valued(field(PORT_SEAMLESS, "Port"), self.port.clone()),
                                        button("ig-go-seamless", "Connect"),
                                    ],
                                ),
                            ),
                        ],
                    )],
                ),
            ],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        let target = if path_has(node, HOST) || path_has(node, HOST_PAIR)
            || path_has(node, HOST_SEAMLESS)
            || path_has(node, HOST_PAIR_SEAMLESS)
        {
            Some(&mut self.host)
        } else if path_has(node, PORT) || path_has(node, PORT_SEAMLESS) {
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
