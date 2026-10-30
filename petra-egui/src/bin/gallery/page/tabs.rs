//! Inventory row 32, Tabs.

use gorgon_petra::component::{
    IconMark, contained_tab, contained_tab_bar, contained_tab_with_secondary, section, tab,
    tab_bar, tab_with_icon, vertical_tab, vertical_tab_bar,
};
use gorgon_petra::geom::Align;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{InsetRefs, NodeKind, Props, TrackSize, ViewNode};

use super::Page;
use super::common::{body, column, path_has, sp, tok, wrapped};

const TAB_LINE_0: &str = "tab-line-0";
const TAB_LINE_1: &str = "tab-line-1";
const TAB_CONT_0: &str = "tab-cont-0";
const TAB_CONT_1: &str = "tab-cont-1";
const TAB_VERT: [(&str, &str, &str); 2] = [
    ("tab-vert-0", "North", "North panel"),
    ("tab-vert-1", "South", "South panel"),
];

/// Live state of the Tabs page: the selected tab of each strip.
#[derive(Default)]
pub struct Tabs {
    line_tab: u8,
    contained_tab: u8,
    vertical_tab: usize,
}

/// The panel a vertical strip selects between: Carbon's `TabPanel`, the
/// `$layer` fill the strip sits on, padded, showing the selected tab's
/// content. The line and contained strips show none, matching the Carbon
/// reference shot for this row, which panels only the vertical strip.
fn tab_panel(content: &str) -> ViewNode {
    let mut panel = column("panel", None, vec![wrapped("panel-text", content)]);
    panel.props.padding = Some(InsetRefs::symmetric(tok("spacing.md"), tok("spacing.md")));
    panel
        .props
        .tokens
        .insert("background".into(), tok("surface.raised"));
    panel
}

impl Page for Tabs {
    fn row(&self) -> &'static str {
        "Tabs"
    }

    /// Carbon's tabs skeleton (`_tabs.scss`): each nav link is a 10rem slot
    /// holding a `--skeleton` span, 14 high × 100 % of the slot (160 wide).
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        use super::common::{row, sp};
        Some(row(
            "skeleton-blocks",
            sp("spacing-05"),
            vec![
                skeleton(
                    "skeleton-tab-0",
                    SkeletonAnatomy::Caption,
                    SizeStep::Md,
                    160.0,
                    now,
                ),
                skeleton(
                    "skeleton-tab-1",
                    SkeletonAnatomy::Caption,
                    SizeStep::Md,
                    160.0,
                    now,
                ),
                skeleton(
                    "skeleton-tab-2",
                    SkeletonAnatomy::Caption,
                    SizeStep::Md,
                    160.0,
                    now,
                ),
            ],
        ))
    }

    /// The documented disabled state (spec 008 T025, register row 478): a
    /// tab label reads in `text.muted` and the selected tab's indicator bar
    /// fades to `icon-disabled` (slice-e: border-bottom `$border-disabled`),
    /// so the cell leads with a selected tab under `disabled()`.
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![tab_bar(
                "disabled-tabs",
                vec![
                    tab("disabled-a", "Fibers", true),
                    tab("disabled-b", "Graph", false),
                ],
            )],
        ))]
    }

    /// Carbon's tab-item ladder (register rows #471–473): md 40 / lg 48 /
    /// xl 64 — the three built steps of the contained strip, the one
    /// variant whose Sizes entry names the `lg`–`xl` range (slice-e;
    /// line tabs are documented at 40 alone and the vertical strip pins
    /// 64 in its own right, both on the body above). Each labelled with
    /// its step and the item height it lands on (FR-025).
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{contained_tab, contained_tab_lg, contained_tab_xl, text};

        use super::common::{column, row, sp};
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                row(
                    "size-md",
                    sp("spacing-03"),
                    vec![
                        text("size-md-label", "md · 40"),
                        contained_tab("size-ct-md", "Tab", true),
                    ],
                ),
                row(
                    "size-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-lg-label", "lg · 48"),
                        contained_tab_lg("size-ct-lg", "Tab", true),
                    ],
                ),
                row(
                    "size-xl",
                    sp("spacing-03"),
                    vec![
                        text("size-xl-label", "xl · 64"),
                        contained_tab_xl("size-ct-xl", "Tab", true),
                    ],
                ),
            ],
        )]
    }

    fn body(&self) -> ViewNode {
        let vertical = vertical_tab_bar(
            "vert-strip",
            TAB_VERT
                .iter()
                .enumerate()
                .map(|(i, (key, label, _))| vertical_tab(*key, *label, i == self.vertical_tab))
                .collect(),
        );
        // Strip beside panel: the strip takes its own width, the panel the
        // rest, and `Stretch` makes the panel as tall as the strip.
        let vertical_with_panel = ViewNode::new(NodeKind::Grid, "vert")
            .with_props(Props {
                columns: vec![TrackSize::FitContent, TrackSize::Weight { weight: 1.0 }],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![vertical, tab_panel(TAB_VERT[self.vertical_tab].2)]);
        section(
            "strips",
            "Line, contained, vertical",
            vec![body(
                "tabs",
                sp("spacing.md"),
                vec![
                    tab_bar(
                        "line-strip",
                        vec![
                            tab(TAB_LINE_0, "Fibers", self.line_tab == 0),
                            tab(TAB_LINE_1, "Trace", self.line_tab == 1),
                        ],
                    ),
                    contained_tab_bar(
                        "cont-strip",
                        vec![
                            contained_tab(TAB_CONT_0, "One", self.contained_tab == 0),
                            contained_tab(TAB_CONT_1, "Two", self.contained_tab == 1),
                        ],
                    ),
                    vertical_with_panel,
                    // Register row #466, Carbon's `--icon` modifier: a
                    // glyph trails the label with Carbon's own 8px
                    // (`spacing-03`) between them, in the label's ink.
                    tab_bar(
                        "icon-strip",
                        vec![
                            tab_with_icon("tab-icon-0", "Fibers", true, IconMark::Checkmark),
                            tab_with_icon("tab-icon-1", "Trace", false, IconMark::ChevronDown),
                        ],
                    ),
                    // Register row #467, Carbon's `--secondary-label`:
                    // a second 12px line under the Contained label.
                    contained_tab_bar(
                        "secondary-strip",
                        vec![
                            contained_tab_with_secondary(
                                "tab-sec-0",
                                "Importer",
                                "In progress",
                                true,
                            ),
                            contained_tab_with_secondary("tab-sec-1", "Exporter", "Queued", false),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, TAB_LINE_0) {
            self.line_tab = 0;
        } else if path_has(node, TAB_LINE_1) {
            self.line_tab = 1;
        } else if path_has(node, TAB_CONT_0) {
            self.contained_tab = 0;
        } else if path_has(node, TAB_CONT_1) {
            self.contained_tab = 1;
        } else if let Some(hit) = TAB_VERT.iter().position(|(key, _, _)| path_has(node, key)) {
            self.vertical_tab = hit;
        } else {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use egui::{Pos2, RawInput};
    use gorgon_petra::component::{tab, tab_bar};
    use gorgon_petra::input::{InputEvent, Route};
    use gorgon_petra::layout::{ChangeSet, RowSource};
    use gorgon_petra::tree::ViewNode;
    use gorgon_petra_egui::host::{App, Host, default_presenter};
    use std::ops::Range;
    use std::sync::Arc;

    /// A strip too narrow for its tabs clips, and never clips a label.
    ///
    /// This is the falsification for `component::tabs::scrollable_row`. The
    /// defect it guards shipped for months and read as a normal tab bar: a
    /// tab's `TrackSize::FitContent` column was a preference the strip
    /// could squeeze, so at 900x700 the inspector's own tab labels were
    /// silently cut with no ellipsis and no sign anything was missing.
    ///
    /// The assertion is on `paint.overflowed`, the same flag
    /// `gorgon-inspector`'s `layout_overlap` census reads: a text run that
    /// drew more than its own rect is lost text. Revert `scrollable_row`
    /// to a plain `Stack` and this fails naming the label.
    ///
    /// `PETRA_SHOT_DIR` also writes the picture, because the frame record
    /// cannot show that a clipped strip still looks like a tab bar.
    #[test]
    fn a_strip_too_narrow_for_its_tabs_clips_without_clipping_a_label() {
        struct Probe;
        impl RowSource for Probe {
            fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
                Vec::new()
            }
        }
        impl App for Probe {
            fn view(&mut self) -> ViewNode {
                tab_bar(
                    "strip",
                    vec![
                        tab("t1", "Approvals pending review", false),
                        tab("t2", "Recent leaks and findings", true),
                        tab("t3", "History", false),
                        tab("t4", "Logs", false),
                    ],
                )
            }
            fn handle(
                &mut self,
                _event: &InputEvent,
                _route: &Route,
                _frame: Option<&gorgon_petra::frame::PetrifiedFrame>,
            ) {
            }
            fn take_changes(&mut self) -> ChangeSet {
                ChangeSet::All
            }
        }

        let dir = std::env::var_os("PETRA_SHOT_DIR").map(std::path::PathBuf::from);
        if let Some(dir) = &dir {
            std::fs::create_dir_all(dir).expect("shot dir");
        }
        let narrow = egui::vec2(340.0, 120.0);
        let input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, narrow)),
            ..RawInput::default()
        };

        let ctx = egui::Context::default();
        ctx.run_ui(input.clone(), |_| {})
            .drop_without_applying_deltas();
        let mut host = Host::new(&ctx, Probe, default_presenter());
        ctx.run_ui(input.clone(), |_| host.pass(&ctx))
            .drop_without_applying_deltas();
        host.set_reduced_motion(true);
        let output = ctx.run_ui(input, |_| host.pass(&ctx));

        let mut shooter = gorgon_petra_testkit::snapshot::Snapshotter::new();
        let shot = shooter
            .capture(&ctx, &output, host.frame().expect("a frame"), None)
            .expect("capture refused");
        output.drop_without_applying_deltas();

        if let Some(dir) = &dir {
            std::fs::write(dir.join("narrow-tab-strip.png"), &shot.png).expect("write shot");
        }

        let frame = host.frame().expect("a frame");
        let clipped: Vec<&str> = frame
            .placements
            .iter()
            .filter(|p| p.paint.overflowed)
            .map(|p| p.id.as_str())
            .collect();
        assert!(
            clipped.is_empty(),
            "a strip narrower than its tabs must clip the STRIP, never a \
             label: these drew more than their own rect and lost text:\n{}",
            clipped.join("\n")
        );
    }
}
