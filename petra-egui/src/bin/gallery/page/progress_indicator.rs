//! Inventory row 26, Progress indicator.

use gorgon_petra::component::{progress_indicator, progress_step, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Progress indicator page. It holds no live state.
pub struct ProgressIndicator;

impl Page for ProgressIndicator {
    fn row(&self) -> &'static str {
        "Progress indicator"
    }

    /// Carbon's progress-indicator skeleton (`_progress.scss`): the label
    /// bar only, 14×40 — an intentional partial-skeleton: the step icon and
    /// connector line stay real even while the label shimmers.
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        Some(skeleton(
            "skeleton-label",
            SkeletonAnatomy::Caption,
            SizeStep::Md,
            40.0,
            now,
        ))
    }

    /// The documented disabled state (spec 008 T025, register row 384): a
    /// disabled step's label in `text.muted` and its status glyph in the
    /// `$icon-disabled` fade (slice-d).
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![progress_indicator(
                "disabled-steps",
                vec![
                    progress_step("disabled-done", "Create", true, false),
                    progress_step("disabled-step", "Review", false, false),
                ],
            )],
        ))]
    }

    fn body(&self) -> ViewNode {
        section(
            "steps",
            "Steps",
            vec![body(
                "pi",
                sp("spacing.md"),
                vec![progress_indicator(
                    "pi",
                    vec![
                        progress_step("st-0", "Clone", true, false),
                        progress_step("st-1", "Build", false, true),
                        progress_step("st-2", "Run", false, false),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
