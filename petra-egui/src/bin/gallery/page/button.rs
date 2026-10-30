//! Inventory row 4, Button.

use gorgon_petra::component::{
    button, button_2xl, button_lg, button_sm, button_xl, button_xs, danger_button,
    danger_ghost_button, danger_tertiary_button, ghost_button, loading_button, primary_button,
    section, tertiary_button,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, row, sp};

/// The Button page. Its only live state is the host's clock, which the
/// loading button's spinner turns on (register row 066).
#[derive(Default)]
pub struct Button {
    now: f64,
}

impl Page for Button {
    fn row(&self) -> &'static str {
        "Button"
    }

    fn tick(&mut self, now: f64) {
        self.now = now;
    }

    /// Carbon's button skeleton (`_button.scss`): a 150-wide block at the
    /// button's own step height (32/40/48 at sm/md/lg).
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        use super::common::{row, sp};
        Some(row(
            "skeleton-blocks",
            sp("spacing-05"),
            vec![
                skeleton(
                    "skeleton-btn-sm",
                    SkeletonAnatomy::Control,
                    SizeStep::Sm,
                    150.0,
                    now,
                ),
                skeleton(
                    "skeleton-btn-md",
                    SkeletonAnatomy::Control,
                    SizeStep::Md,
                    150.0,
                    now,
                ),
                skeleton(
                    "skeleton-btn-lg",
                    SkeletonAnatomy::Control,
                    SizeStep::Lg,
                    150.0,
                    now,
                ),
            ],
        ))
    }

    /// Carbon's button sizing (`_button.scss` `layout.use`, 24/32/40/48/
    /// 64/80): all six built steps side by side, each labeled with its step
    /// and the height it lands on (FR-025).
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{
            button_2xl, button_lg, button_sm, button_xl, button_xs, text,
        };

        use super::common::{column, row, sp};
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                row(
                    "size-xs",
                    sp("spacing-03"),
                    vec![
                        text("size-xs-label", "xs · 24"),
                        button_xs("size-btn-xs", "Button"),
                    ],
                ),
                row(
                    "size-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-sm-label", "sm · 32"),
                        button_sm("size-btn-sm", "Button"),
                    ],
                ),
                row(
                    "size-md",
                    sp("spacing-03"),
                    vec![
                        text("size-md-label", "md · 40"),
                        button("size-btn-md", "Button"),
                    ],
                ),
                row(
                    "size-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-lg-label", "lg · 48"),
                        button_lg("size-btn-lg", "Button"),
                    ],
                ),
                row(
                    "size-xl",
                    sp("spacing-03"),
                    vec![
                        text("size-xl-label", "xl · 64"),
                        button_xl("size-btn-xl", "Button"),
                    ],
                ),
                row(
                    "size-2xl",
                    sp("spacing-03"),
                    vec![
                        text("size-2xl-label", "2xl · 80"),
                        button_2xl("size-btn-2xl", "Button"),
                    ],
                ),
            ],
        )]
    }

    /// The documented disabled state (spec 008 T025, register row 63): the
    /// filled pair swaps its ground to `button-disabled` and the tertiary
    /// keeps only its faded edge — one cell covers both treatments.
    fn disabled(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![
                primary_button("disabled-primary", "Primary"),
                button("disabled-default", "Default"),
                tertiary_button("disabled-tertiary", "Tertiary"),
            ],
        ))]
    }

    fn body(&self) -> ViewNode {
        section(
            "variants",
            "Variants and sizes",
            vec![body(
                "buttons",
                sp("spacing.md"),
                vec![
                    row(
                        "kinds",
                        sp("spacing.md"),
                        vec![
                            primary_button("btn-primary", "Primary"),
                            button("btn-default", "Default"),
                            tertiary_button("btn-tertiary", "Tertiary"),
                            ghost_button("btn-ghost", "Ghost"),
                        ],
                    ),
                    // Carbon's danger triple, directly under the four safe
                    // variants so "danger against default" is one glance.
                    // All three lead with the `status.down` octagon in
                    // `support-error`: the channel that tells danger from
                    // default without spending a colour alone.
                    //
                    // This row used to carry a claim that the three had to
                    // sit apart from the kinds row because
                    // `layout::stack::distribute` clipped a mixed-width
                    // sibling and broke `Delete`'s label across two lines.
                    // Round 3 (wave STACK) put the trio back into the kinds
                    // row and rasterized twice, with and without a
                    // candidate fix: byte-identical both times, one line
                    // both times. The defect did not reproduce, so that
                    // justification was stale (`ROUND3-DEFECTS.md`). The
                    // split here is a grouping choice, not a layout
                    // workaround; not re-verified in this change.
                    row(
                        "danger",
                        sp("spacing.md"),
                        vec![
                            danger_button("btn-danger", "Delete"),
                            danger_tertiary_button("btn-danger-tertiary", "Delete"),
                            danger_ghost_button("btn-danger-ghost", "Delete"),
                        ],
                    ),
                    // Register row 066, the loading state: the button
                    // stands aside for the in-button loader and is
                    // disabled for as long as the action flies.
                    row(
                        "loading",
                        sp("spacing.md"),
                        vec![loading_button("btn-loading", "Saving", self.now)],
                    ),
                    // All six of Carbon's size steps, in order, so the
                    // xs-to-2xl progression is one row to look at rather
                    // than a fact only a unit test can see.
                    row(
                        "sizes",
                        sp("spacing.md"),
                        vec![
                            button_xs("btn-xs", "X-small 24"),
                            button_sm("btn-sm", "Small 32"),
                            button("btn-md", "Medium 40"),
                            button_lg("btn-lg", "Large 48"),
                            button_xl("btn-xl", "X-large 64"),
                            button_2xl("btn-2xl", "2X-large 80"),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
