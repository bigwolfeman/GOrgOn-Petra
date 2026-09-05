//! Inventory row 20, Modal.

use gorgon_petra::component::{button, modal, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, path_has, sp};

const TRIGGER: &str = "open-modal";
const DIALOG: &str = "md";
const CLOSE: &str = "close";
const CANCEL: &str = "cancel";
const PRIMARY: &str = "primary";

/// Live state of the Modal page: whether the dialog is open.
///
/// It held none, and the dialog was mounted unconditionally. A modal takes
/// focus and blocks every press behind it, so a page that is *only* a
/// permanently open modal whose own three controls do nothing is a page
/// where no press anywhere does anything. The operator's words were "pops
/// up an interface that I cant interact with", and that is the whole of it.
#[derive(Default)]
pub struct Modal {
    open: bool,
}

impl Page for Modal {
    fn row(&self) -> &'static str {
        "Modal"
    }

    fn body(&self) -> ViewNode {
        let mut children = vec![button(TRIGGER, "Rebuild fiber")];
        if self.open {
            children.push(modal(
                DIALOG,
                "Confirm rebuild",
                "This unloads the fiber.",
                "Rebuild",
            ));
        }
        section(
            "dialog",
            "Modal",
            vec![body(
                "md-body",
                sp("spacing.md"),
                vec![column("md-col", sp("spacing.md"), children)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // Every one of the dialog's three controls closes it. Carbon's
        // primary action closes on success as much as Cancel does; a
        // confirmation the operator cannot leave is the defect being fixed,
        // so there is no branch here that keeps it open.
        if [CLOSE, CANCEL, PRIMARY]
            .iter()
            .any(|key| path_has(node, key))
        {
            self.open = false;
            true
        } else if path_has(node, TRIGGER) {
            self.open = true;
            true
        } else {
            false
        }
    }
}
