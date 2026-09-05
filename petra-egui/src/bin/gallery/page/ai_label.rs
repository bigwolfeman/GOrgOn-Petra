//! Inventory row 2, AI label.

use gorgon_petra::component::{ai_label, ai_label_revert, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The AI label page. It holds no live state.
pub struct AiLabel;

impl Page for AiLabel {
    fn row(&self) -> &'static str {
        "AI label"
    }

    fn body(&self) -> ViewNode {
        section(
            "ai",
            "Triggers (closed)",
            vec![body(
                "ai-body",
                sp("spacing.md"),
                vec![
                    // The closed trigger. The open form's explainability
                    // popover names the trigger by sibling key and mounts
                    // at this depth (`ai_label_open_validates_when_mounted_at_catalog_depth`);
                    // this page does not yet hold the state to open it.
                    //
                    // `ai_label_inline` is NOT shown here: its trigger
                    // (`ai_label.rs::inline_trigger`) sets `props.padding`
                    // on the "AI" caption, which is a `text` leaf —
                    // `validate` refuses a leaf declaring padding it has
                    // no children to apply. This is a pre-existing defect
                    // in `ai_label.rs`, which this integration task does
                    // not own or edit; reported separately.
                    ai_label(
                        "ai-default",
                        "Confidence score",
                        false,
                        "Trained on ticket history.",
                    ),
                    ai_label_revert("ai-revert", "Revert to AI suggestion"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
