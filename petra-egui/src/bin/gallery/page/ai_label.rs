//! Inventory row 2, AI label.

use gorgon_petra::component::{
    ai_label, ai_label_2xs, ai_label_inline, ai_label_inline_lg, ai_label_inline_sm, ai_label_lg,
    ai_label_mini, ai_label_revert, ai_label_sm, ai_label_xl, ai_label_xs, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, row, sp, wrapped};

/// The one trigger on this page that opens: the others are the size ramp
/// and are there to be looked at, not pressed.
const LIVE: &str = "ai-live";

/// The explainability popover's key inside [`LIVE`].
const PANEL: &str = "panel";

/// What the open panel says.
///
/// One short line, and that is a constraint rather than a style choice.
/// `layout::overlay_surface::natural_size` measures a surface's children at
/// `SizeProposal::unspecified()` — one unbounded line — and the surface's
/// own 368-unit `max` (`popover.rs`'s `MAX_INLINE`) is applied to the
/// placement afterwards, so a body longer than one line is **cut**, not
/// wrapped. Photographed 2026-09-05: a two-sentence body lost its second
/// half mid-word. The rest of what a reader needs is in the page's own
/// prose, which does wrap. Written up in `ROUND3-DEFECTS.md`.
const EXPLANATION: &str = "Model answer, from 4,318 resolved tickets.";

/// Live state of the AI label page: whether the explainability popover is
/// open.
///
/// The row was a unit struct returning `false` from `handle`, showing one
/// closed square and one more square with the word "Undo" wedged inside it,
/// and nothing else. The operator's report was *"this is still a wtf I dont
/// understand this and it looks bad what ever it is"*, which is the right
/// reading of a page that shows a trigger and never what it triggers.
#[derive(Default)]
pub struct AiLabel {
    open: bool,
}

impl Page for AiLabel {
    fn row(&self) -> &'static str {
        "AI label"
    }

    fn body(&self) -> ViewNode {
        section(
            "ai",
            "AI label",
            vec![body(
                "ai-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "note",
                        "An AI label marks a value a model produced rather than \
                         a person. It is also a button: press it and it opens an \
                         explainability popover that says where the value came \
                         from. Press the mark below, and press it again to \
                         close it.",
                    ),
                    row(
                        "live-row",
                        sp("spacing.md"),
                        vec![ai_label(LIVE, "Confidence score", self.open, EXPLANATION)],
                    ),
                    wrapped(
                        "flat-note",
                        "Carbon tints the open panel with a gradient aura. \
                         Petra's draw list has no gradient and no blur \
                         primitive, on purpose (draw/mod.rs), so the panel here \
                         is a flat surface with the same shape.",
                    ),
                    wrapped(
                        "sizes-note",
                        "The default variant ships seven sizes: 16, 20, 24, 32, \
                         40, 48 and 64.",
                    ),
                    row(
                        "sizes",
                        sp("spacing.md"),
                        vec![
                            ai_label_mini("ai-mini", "Confidence, mini", false, EXPLANATION),
                            ai_label_2xs("ai-2xs", "Confidence, 2xs", false, EXPLANATION),
                            ai_label_xs("ai-xs", "Confidence, xs", false, EXPLANATION),
                            ai_label_sm("ai-sm", "Confidence, sm", false, EXPLANATION),
                            ai_label("ai-md", "Confidence, md", false, EXPLANATION),
                            ai_label_lg("ai-lg", "Confidence, lg", false, EXPLANATION),
                            ai_label_xl("ai-xl", "Confidence, xl", false, EXPLANATION),
                        ],
                    ),
                    wrapped(
                        "inline-note",
                        "The inline variant sits in running text. It has a \
                         leading bullet instead of a border, and its own three \
                         sizes: 16, 18 and 22.",
                    ),
                    row(
                        "inline",
                        sp("spacing.md"),
                        vec![
                            ai_label_inline_sm("ai-inline-sm", "Ask AI, sm", false, EXPLANATION),
                            ai_label_inline("ai-inline-md", "Ask AI, md", false, EXPLANATION),
                            ai_label_inline_lg("ai-inline-lg", "Ask AI, lg", false, EXPLANATION),
                        ],
                    ),
                    wrapped(
                        "revert-note",
                        "Revert is the one state unique to this component: the \
                         whole trigger is swapped for a control that puts the \
                         value back. Carbon draws a bare undo arrow; Petra has \
                         no Undo mark and does not ship icon-only controls, so \
                         it is the word.",
                    ),
                    row(
                        "revert-row",
                        sp("spacing.md"),
                        vec![ai_label_revert("ai-revert", "Revert to the AI suggestion")],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, LIVE) {
            self.open = !self.open;
            return true;
        }
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, PANEL) {
            self.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AiLabel, LIVE};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    fn press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: gorgon_petra::geom::Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    fn panel_mounted(page: &AiLabel) -> bool {
        let tree = page.body();
        let live = find(&tree, LIVE).expect("the live AI label is on the page");
        find(live, "panel").is_some()
    }

    /// A press on the trigger mounts the explainability popover, and a
    /// second press takes it away.
    ///
    /// The panel is a node, so its presence in the tree is the whole claim:
    /// the old page could not build one at all.
    #[test]
    fn a_press_on_the_trigger_opens_the_explainability_popover() {
        let mut page = AiLabel::default();
        assert!(!panel_mounted(&page));
        assert!(page.handle(&press(), "/page/ai/ai-body/live-row/ai-live/trigger"));
        assert!(panel_mounted(&page), "the press did not open the panel");
        assert!(page.handle(&press(), "/page/ai/ai-body/live-row/ai-live/trigger"));
        assert!(!panel_mounted(&page), "the press did not shut the panel");
    }

    /// A press outside the open panel shuts it, the way every other
    /// `DismissOutside` surface in this catalog does.
    #[test]
    fn a_press_outside_the_panel_shuts_it() {
        let mut page = AiLabel::default();
        assert!(page.handle(&press(), "/page/ai/ai-body/live-row/ai-live/trigger"));
        assert!(panel_mounted(&page));
        page.dismissed(&["/page/ai/ai-body/live-row/ai-live/panel".to_owned()]);
        assert!(!panel_mounted(&page));
    }

    /// The size ramp is on the page, and pressing one of its triggers does
    /// not open the live one.
    ///
    /// The old page hid the inline variant behind a comment saying
    /// `validate` refused it. That has not been true since
    /// `every_constructor_produces_a_tree_validate_accepts` started
    /// validating all three inline sizes.
    #[test]
    fn every_size_of_both_variants_is_on_the_page() {
        let mut page = AiLabel::default();
        let tree = page.body();
        for key in [
            "ai-mini",
            "ai-2xs",
            "ai-xs",
            "ai-sm",
            "ai-md",
            "ai-lg",
            "ai-xl",
            "ai-inline-sm",
            "ai-inline-md",
            "ai-inline-lg",
            "ai-revert",
        ] {
            assert!(find(&tree, key).is_some(), "{key} is not on the page");
        }
        assert!(!page.handle(&press(), "/page/ai/ai-body/sizes/ai-xl/trigger"));
        assert!(!panel_mounted(&page));
    }
}
