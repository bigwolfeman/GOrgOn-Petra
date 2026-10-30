//! Inventory row 20, Modal.

use gorgon_petra::component::{button, modal, modal_lg, modal_sm, modal_xs, section, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, path_has, row, sp};

const TRIGGER: &str = "open-modal";
const DIALOG: &str = "md";
const CLOSE: &str = "close";
const CANCEL: &str = "cancel";
const PRIMARY: &str = "primary";

/// The width ladder's step (register rows #299–302): Carbon's four modal
/// sizes as [`ModalSize`]'s width shares. Each step gets a trigger and a
/// dialog of its own in the Sizes row; only one dialog is ever mounted,
/// because an open modal blocks the page behind it.
#[derive(Clone, Copy, Default, PartialEq)]
enum Step {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
}

impl Step {
    const ALL: [Step; 4] = [Step::Xs, Step::Sm, Step::Md, Step::Lg];

    /// The Sizes row's trigger key for this step.
    fn trigger(self) -> &'static str {
        match self {
            Step::Xs => "size-open-xs",
            Step::Sm => "size-open-sm",
            Step::Md => "size-open-md",
            Step::Lg => "size-open-lg",
        }
    }

    /// The Sizes row's dialog key for this step.
    fn dialog_key(self) -> &'static str {
        match self {
            Step::Xs => "size-dlg-xs",
            Step::Sm => "size-dlg-sm",
            Step::Md => "size-dlg-md",
            Step::Lg => "size-dlg-lg",
        }
    }

    /// The Sizes row's own key for this step.
    fn row_key(self) -> &'static str {
        match self {
            Step::Xs => "size-xs",
            Step::Sm => "size-sm",
            Step::Md => "size-md",
            Step::Lg => "size-lg",
        }
    }

    /// The label on this step's cell: the step and the width share it
    /// lands on (FR-025).
    fn label(self) -> &'static str {
        match self {
            Step::Xs => "xs · 32%",
            Step::Sm => "sm · 42%",
            Step::Md => "md · 60%",
            Step::Lg => "lg · 84%",
        }
    }

    /// This step's dialog.
    fn dialog(self, key: &'static str) -> ViewNode {
        let copy = "This unloads the fiber.";
        match self {
            Step::Xs => modal_xs(key, "Confirm rebuild", copy, "Rebuild"),
            Step::Sm => modal_sm(key, "Confirm rebuild", copy, "Rebuild"),
            Step::Md => modal(key, "Confirm rebuild", copy, "Rebuild"),
            Step::Lg => modal_lg(key, "Confirm rebuild", copy, "Rebuild"),
        }
    }
}

/// Live state of the Modal page: whether the body's dialog is open, and
/// which size step (if any) the Sizes row is showing.
///
/// It held none, and the dialog was mounted unconditionally. A modal takes
/// focus and blocks every press behind it, so a page that is *only* a
/// permanently open modal whose own three controls do nothing is a page
/// where no press anywhere does anything. The operator's words were "pops
/// up an interface that I cant interact with", and that is the whole of it.
/// The size steps ride the same rule: their dialogs mount on their
/// trigger's press and never rest on the page.
#[derive(Default)]
pub struct Modal {
    open: bool,
    size_open: Option<Step>,
}

impl Page for Modal {
    fn row(&self) -> &'static str {
        "Modal"
    }

    fn body(&self) -> ViewNode {
        let mut children = vec![button(TRIGGER, "Rebuild fiber")];
        if self.open {
            children.push(Step::Md.dialog(DIALOG));
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

    /// The width ladder (register rows #299–302): one labelled cell per
    /// built step, each a trigger that mounts that step's dialog (FR-025).
    /// The dialog is not resting in the cell — see [`Modal`]'s own doc for
    /// why a permanently mounted modal would own the whole page.
    fn sizes(&self) -> Vec<ViewNode> {
        vec![column(
            "size-steps",
            sp("spacing-05"),
            Step::ALL
                .into_iter()
                .map(|step| {
                    let mut cell = vec![
                        text(format!("{}-label", step.dialog_key()), step.label()),
                        button(step.trigger(), "Open"),
                    ];
                    if self.size_open == Some(step) {
                        cell.push(step.dialog(step.dialog_key()));
                    }
                    row(step.row_key(), sp("spacing-03"), cell)
                })
                .collect(),
        )]
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
            self.size_open = None;
            true
        } else if path_has(node, TRIGGER) {
            self.open = true;
            true
        } else if let Some(step) = Step::ALL
            .into_iter()
            .find(|step| path_has(node, step.trigger()))
        {
            self.size_open = Some(step);
            true
        } else {
            false
        }
    }
}
