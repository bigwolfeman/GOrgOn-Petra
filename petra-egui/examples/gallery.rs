//! A live window over every Petra component that has a painter today.
//!
//! This is the first thing in the repository that opens a window. Everything
//! else drives `Host` headless, which proves the layout and the digest but
//! never proves that a font loads, that wgpu is reachable, or that a click
//! from a real compositor lands where the hit test says it does.
//!
//! It is a **harness, not a demo**. The strip along the top reports the
//! previous frame's own honesty counters — placements, silent nodes,
//! unresolved tokens, focused-but-unringed nodes — so a component that
//! "looks fine" while the engine is unhappy about it is visible rather than
//! flattering. A demo hides those. This one is built around them.
//!
//! Run it: `cargo run -p gorgon-petra-egui --example gallery`
//!
//! What it cannot show: `NodeKind::Image` and `NodeKind::Custom` have no
//! painter in this crate yet, and the section at the bottom declares one of
//! each on purpose so they turn up in the `undrawn` counter instead of being
//! quietly left out. Animation is not here at all — `petra/src/anim/` is a
//! module header.

use std::ops::Range;
use std::sync::Arc;

use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::input::{InputEvent, Route, activates};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, InputPolicy, Interaction, Layer, NodeKind,
    Props, Role, Semantics, TextWrap, TrackSize, ViewNode,
};

/// A spacing token reference, for the styling props that take one (FR-053).
///
/// Every gap on this page is a step of the shipped ramp
/// (`gorgon_petra::token::standard_vocabulary`), because the page's job is to
/// show what the design system can say. A gap that used to be a float and did
/// not land on a step moved to the nearest step, ties upward — six of them
/// did, by one or two logical units each.
fn sp(name: &str) -> Option<TokenName> {
    Some(TokenName::new(name).expect("gallery spacing tokens are well-formed"))
}
use gorgon_petra_egui::host::{App, Host, default_presenter};

/// The one custom kind this gallery declares. Registered on the host before
/// the first pass: an unregistered custom kind is a tree-acceptance
/// violation, so this is not decoration.
const CUSTOM_KIND: &str = "sparkline";

/// Rows the virtualized `Collection` section pulls from.
const ROW_SOURCE: &str = "gallery/rows";
/// How many rows that collection claims to have. Far more than fit, which is
/// the point: `Collection` asks for the visible window only.
const TOTAL_ROWS: usize = 100_000;

/// The three shipped status tokens, each with the shape and the words that
/// carry the same meaning the colour does.
///
/// Colour is never the only channel here, and that is a property of
/// `StatusToken` rather than a courtesy of this file: it carries `shape()`
/// and `text()` beside its colour token precisely so a reader who cannot
/// separate the hues still gets the state. Showing all three channels at
/// once is how you check that claim is still true.
const STATUSES: [(&str, &str, &str, &str); 3] = [
    (
        "supervisor/root",
        "status.ok",
        "Ok",
        "12 children, 0 restarts",
    ),
    (
        "worker/indexer",
        "status.degraded",
        "Degraded",
        "retry 3 of 5, last error 40s ago",
    ),
    (
        "worker/shaper",
        "status.down",
        "Down",
        "disposer deadline exceeded",
    ),
];

/// The previous frame's counters, read back off the host after each pass.
///
/// One frame stale by construction: the application builds the tree before
/// the frame that measures it exists. The label on screen says so rather
/// than pretending the number is current.
#[derive(Clone, Default)]
struct Counters {
    seq: u64,
    placements: usize,
    drawn: usize,
    silent: usize,
    empty: usize,
    clipped: usize,
    focus_rings: usize,
    blind_focus: usize,
    unresolved: usize,
    undrawn: usize,
    desynced: bool,
}

impl Counters {
    fn line(&self) -> String {
        format!(
            "frame {} (previous pass) · {} placements · {} drawn · {} empty · {} clipped · \
             {} silent · rings {} · blind focus {} · unresolved tokens {} · undrawn {} · {}",
            self.seq,
            self.placements,
            self.drawn,
            self.empty,
            self.clipped,
            self.silent,
            self.focus_rings,
            self.blind_focus,
            self.unresolved,
            self.undrawn,
            if self.desynced { "DESYNCED" } else { "in sync" },
        )
    }
}

struct Gallery {
    counters: Counters,
    /// What the last routed event did, echoed on screen so the input path is
    /// observable without a debugger.
    last_event: String,
    modal: bool,
    menu: bool,
    passthrough: bool,
    /// Ids the host reported as dismissed, which is the only way a
    /// `DismissOutside` surface can close: the engine is retained and this
    /// application owns the tree.
    dismissals: usize,
    /// A subtree a test can graft on, so a layout question can be asked
    /// through the real host rather than through a second fixture that
    /// drifts from this one.
    probe: Option<ViewNode>,
    /// Which tab the tab strip has selected.
    tab: usize,
    /// How full the progress bar is, 0..=1.
    progress: f32,
    /// Which row of the virtualized list is selected.
    selected_row: Option<usize>,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            counters: Counters::default(),
            last_event: String::new(),
            modal: false,
            menu: false,
            passthrough: false,
            dismissals: 0,
            probe: None,
            tab: 0,
            // Not zero and not one: a bar pinned to either end demonstrates
            // nothing about how the two weighted tracks split.
            progress: 0.62,
            selected_row: Some(3),
        }
    }
}

impl Gallery {
    fn heading(key: &str, text: &str) -> ViewNode {
        let mut props = Props {
            text: Some(text.to_owned()),
            style: Some("typography.heading".into()),
            ..Props::default()
        };
        props
            .tokens
            .insert("foreground".into(), "text.primary".into());
        ViewNode::new(NodeKind::Text, key).with_props(props)
    }

    fn body(key: &str, text: &str, token: &str) -> ViewNode {
        let mut props = Props {
            text: Some(text.to_owned()),
            style: Some("typography.body".into()),
            ..Props::default()
        };
        props.tokens.insert("foreground".into(), token.into());
        ViewNode::new(NodeKind::Text, key).with_props(props)
    }

    /// A focusable, clickable label. `Role::Button` with `Focus` and `Click`
    /// and deliberately no `Key`: a button is not a text field, and claiming
    /// `Key` would make Tab traversal have to guess.
    fn button(key: &str, label: &str) -> ViewNode {
        let mut props = Props {
            text: Some(label.to_owned()),
            style: Some("typography.body".into()),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), "surface.raised".into());
        props
            .tokens
            .insert("foreground".into(), "text.primary".into());
        ViewNode::new(NodeKind::Text, key)
            .with_props(props)
            .interactive(
                Role::Button,
                label.to_owned(),
                &[Interaction::Focus, Interaction::Click],
            )
    }

    fn row(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                spacing,
                align: Some(Align::Start),
                ..Props::default()
            })
            .with_children(children)
    }

    fn column(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
            .with_props(Props {
                axis: Some(Axis::Vertical),
                spacing,
                ..Props::default()
            })
            .with_children(children)
    }

    /// A titled block, so every section on the page has the same shape.
    fn section(key: &str, title: &str, children: Vec<ViewNode>) -> ViewNode {
        let mut rows = vec![Self::heading(&format!("{key}-h"), title)];
        rows.extend(children);
        Self::column(key, sp("spacing.sm"), rows)
    }

    /// Buttons in the three states a real one has.
    fn buttons_row(&self) -> ViewNode {
        Self::row(
            "buttons",
            sp("spacing.md"),
            vec![
                // Primary is the inverted pair rather than a coloured accent:
                // the palette has no accent that is not a *status*, and
                // spending `status.ok` on "this button matters" would make
                // green mean two different things.
                Self::control(
                    "primary",
                    "Save",
                    Some("text.primary"),
                    "surface.base",
                    None,
                ),
                Self::control(
                    "secondary",
                    "Cancel",
                    Some("surface.raised"),
                    "text.primary",
                    Some("text.muted"),
                ),
                // No `Click`, and `disabled` in its semantics: the tree says
                // it is unavailable rather than the colour implying it.
                Self::chip(
                    "disabled",
                    "Disabled",
                    Some("surface.base"),
                    "text.muted",
                    Some("text.muted"),
                )
                .with_semantics(Semantics {
                    role: Some(Role::Button),
                    label: Some("Disabled".to_owned()),
                    disabled: true,
                    ..Semantics::default()
                }),
            ],
        )
    }

    /// A checkbox, a radio and a toggle — each a box inside a box.
    fn controls_row(&self) -> ViewNode {
        let check = |key: &str, label: &str, on: bool| {
            Self::row(
                key,
                sp("spacing.sm"),
                vec![
                    Self::swatch(
                        "box",
                        12.0,
                        12.0,
                        on.then_some("text.primary"),
                        Some("text.muted"),
                    ),
                    Self::body("label", label, "text.primary"),
                ],
            )
            .interactive(
                Role::Button,
                label.to_owned(),
                &[Interaction::Focus, Interaction::Click],
            )
        };
        let toggle = |key: &str, label: &str, on: bool| {
            let mut track = vec![];
            if on {
                track.push(Self::swatch("pad", 14.0, 12.0, None, None));
            }
            track.push(Self::swatch("knob", 12.0, 12.0, Some("text.primary"), None));
            if !on {
                track.push(Self::swatch("pad", 14.0, 12.0, None, None));
            }
            Self::row(
                key,
                sp("spacing.sm"),
                vec![
                    Self::row("track", None, track).with_props({
                        let mut p = Props {
                            axis: Some(Axis::Horizontal),
                            ..Props::default()
                        };
                        p.tokens
                            .insert("background".into(), "surface.raised".into());
                        p.tokens.insert("border".into(), "text.muted".into());
                        p
                    }),
                    Self::body("label", label, "text.primary"),
                ],
            )
            .interactive(
                Role::Button,
                label.to_owned(),
                &[Interaction::Focus, Interaction::Click],
            )
        };
        Self::row(
            "controls",
            sp("spacing.xl"),
            vec![
                check("check-on", "Checked", true),
                check("check-off", "Unchecked", false),
                toggle("toggle-on", "Toggle on", true),
                toggle("toggle-off", "Toggle off", false),
            ],
        )
    }

    /// A tab strip: the selected tab is a different fill *and* carries
    /// `selected` in its semantics, so the state is not only a colour.
    fn tabs_row(&self) -> ViewNode {
        let names = ["Fibers", "Trace", "Capabilities"];
        let tabs = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let on = i == self.tab;
                Self::chip(
                    name,
                    name,
                    Some(if on { "surface.raised" } else { "surface.base" }),
                    if on { "text.primary" } else { "text.muted" },
                    on.then_some("text.primary"),
                )
                .with_semantics(Semantics {
                    role: Some(Role::Tab),
                    label: Some((*name).to_owned()),
                    selected: on,
                    ..Semantics::default()
                })
                .interactive(
                    Role::Tab,
                    (*name).to_owned(),
                    &[Interaction::Focus, Interaction::Click],
                )
            })
            .collect();
        Self::column(
            "tabs",
            None,
            vec![
                Self::row("tablist", sp("spacing.xs"), tabs).with_semantics(Semantics {
                    role: Some(Role::TabList),
                    ..Semantics::default()
                }),
                ViewNode::new(NodeKind::Separator, "tab-rule"),
            ],
        )
    }

    /// A progress bar: a filled box inside a wider one, sized by weight.
    fn progress_row(&self) -> ViewNode {
        let done = self.progress.clamp(0.0, 1.0);
        let rest = (1.0 - done).max(0.001);
        let bar = ViewNode::new(NodeKind::Grid, "bar")
            .with_props({
                let mut p = Props {
                    columns: vec![
                        TrackSize::Weight {
                            weight: done.max(0.001),
                        },
                        TrackSize::Weight { weight: rest },
                    ],
                    ..Props::default()
                };
                p.tokens.insert("border".into(), "text.muted".into());
                p
            })
            .child(Self::swatch("done", 0.0, 10.0, Some("status.ok"), None))
            .child(Self::swatch("todo", 0.0, 10.0, None, None))
            .with_constraints(Self::width(260.0))
            .with_semantics(Semantics {
                role: Some(Role::Progress),
                label: Some("Rebuild".to_owned()),
                value: Some(format!("{:.0}%", done * 100.0)),
                ..Semantics::default()
            });
        Self::row(
            "progress",
            sp("spacing.md"),
            vec![
                Self::body("progress-l", "Rebuild", "text.muted"),
                bar,
                Self::body(
                    "progress-v",
                    &format!("{:.0}%", done * 100.0),
                    "text.primary",
                ),
            ],
        )
    }

    /// A two-column form: label beside field.
    fn form_grid() -> ViewNode {
        let field = |key: &str, placeholder: &str| {
            let mut props = Props {
                placeholder: Some(placeholder.to_owned()),
                style: Some("typography.body".into()),
                ..Props::default()
            };
            props
                .tokens
                .insert("background".into(), "surface.raised".into());
            props
                .tokens
                .insert("foreground".into(), "text.muted".into());
            props.tokens.insert("border".into(), "text.muted".into());
            ViewNode::new(NodeKind::Input, key)
                .with_props(props)
                .with_constraints(Self::width(220.0))
                .interactive(
                    Role::TextInput,
                    placeholder.to_owned(),
                    &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
                )
        };
        ViewNode::new(NodeKind::Grid, "form")
            .with_props(Props {
                columns: vec![TrackSize::Fixed { value: 110.0 }, TrackSize::FitContent],
                column_spacing: sp("spacing.md"),
                row_spacing: sp("spacing.sm"),
                ..Props::default()
            })
            .child(Self::body("f-l1", "Fiber name", "text.muted"))
            .child(field("f-name", "supervisor/root"))
            .child(Self::body("f-l2", "Capability", "text.muted"))
            .child(field("f-cap", "fs.read"))
    }

    /// A table whose status column is a drawn swatch beside the word.
    ///
    /// The swatch is a real rectangle rather than a glyph on purpose: two of
    /// the three shapes `StatusToken` declares render as tofu in the shipped
    /// font, and nothing in the painter consumes [`StatusShape`] at all, so a
    /// drawn box is the only shape channel that actually reaches the screen
    /// today.
    fn status_table() -> ViewNode {
        let mut grid = ViewNode::new(NodeKind::Grid, "table")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 150.0 },
                    TrackSize::Fixed { value: 110.0 },
                    TrackSize::FitContent,
                ],
                column_spacing: sp("spacing.lg"),
                row_spacing: sp("spacing.xs"),
                ..Props::default()
            })
            .with_semantics(Semantics {
                role: Some(Role::Table),
                ..Semantics::default()
            })
            .child(Self::body("h1", "FIBER", "text.muted"))
            .child(Self::body("h2", "STATE", "text.muted"))
            .child(Self::body("h3", "DETAIL", "text.muted"));
        for (key, token, word, detail) in STATUSES {
            grid = grid
                .child(Self::body(key, key, "text.primary"))
                .child(Self::row(
                    &format!("{key}-state"),
                    sp("spacing.sm"),
                    vec![
                        Self::swatch("dot", 10.0, 10.0, Some(token), None),
                        Self::body("word", word, "text.primary"),
                    ],
                ))
                .child(Self::body(&format!("{key}-detail"), detail, "text.muted"));
        }
        grid
    }

    /// Every shipped status, through colour, a drawn swatch and the word.
    fn status_section() -> ViewNode {
        Self::column(
            "status",
            sp("spacing.sm"),
            vec![
                Self::heading("status-h", "Status"),
                Self::body(
                    "status-note",
                    "colour is never the only channel: each state carries a word, and \
                     the swatch is drawn rather than typed because two of the three \
                     shapes StatusToken declares are tofu in the shipped font",
                    "text.muted",
                ),
                Self::status_table(),
            ],
        )
    }

    /// A `Grid` with three tracks of different sizing rules beside a nested
    /// vertical `Stack`, a `Spacer` and a `Separator`.
    fn layout_section() -> ViewNode {
        let grid = ViewNode::new(NodeKind::Grid, "grid")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 90.0 },
                    TrackSize::FitContent,
                    TrackSize::Weight { weight: 1.0 },
                ],
                column_spacing: sp("spacing.sm"),
                row_spacing: sp("spacing.xs"),
                ..Props::default()
            })
            .child(Self::body("g-a", "Fixed 90", "text.primary"))
            .child(Self::body("g-b", "FitContent", "text.primary"))
            .child(Self::body(
                "g-c",
                "Weight(1) takes the rest",
                "text.primary",
            ))
            .child(Self::body("g-d", "row two", "text.muted"))
            .child(Self::body("g-e", "wider cell here", "text.muted"))
            .child(Self::body("g-f", "and the remainder", "text.muted"));

        Self::column(
            "layout",
            sp("spacing.sm"),
            vec![
                Self::heading("layout-h", "Grid, Stack, Spacer, Separator"),
                grid,
                Self::row(
                    "layout-row",
                    sp("spacing.sm"),
                    vec![
                        Self::body("l-left", "left", "text.primary"),
                        // Height-clamped on purpose. An unconstrained
                        // `Spacer` takes everything offered on *both* axes
                        // (`layout/leaf.rs`), so one in a horizontal row
                        // claims the row's whole height and opens a hole the
                        // size of the window. SwiftUI's `Spacer` expands only
                        // along its stack's axis; Petra's does not, and
                        // `an_unconstrained_spacer_claims_the_cross_axis_too`
                        // pins that so the difference is a decision rather
                        // than a surprise.
                        ViewNode::new(NodeKind::Spacer, "l-gap").with_constraints(Constraints {
                            vertical: AxisConstraint {
                                min: Some(0.0),
                                max: Some(0.0),
                                priority: 0,
                            },
                            ..Constraints::default()
                        }),
                        ViewNode::new(NodeKind::Separator, "l-sep"),
                        Self::body("l-right", "right of a separator", "text.primary"),
                    ],
                ),
            ],
        )
    }

    /// Wrapping, elision and a placeholder-only `Input`.
    fn text_section() -> ViewNode {
        let long = "A long line that has to wrap, because the whole point of a text \
                    node is that the engine measures it against the width it is offered \
                    rather than trusting whoever wrote the string.";
        let mut wrapped = Props {
            text: Some(long.to_owned()),
            wrap: Some(TextWrap::Wrap),
            style: Some("typography.body".into()),
            ..Props::default()
        };
        // Both text nodes are width-clamped below. Without it the window is
        // wide enough that the "wrapped" string fits on one line and the
        // "elided" one never truncates — a section claiming to demonstrate
        // two behaviours while demonstrating neither.
        wrapped
            .tokens
            .insert("foreground".into(), "text.primary".into());

        let mut clipped = Props {
            text: Some(long.to_owned()),
            wrap: Some(TextWrap::Ellipsis),
            max_lines: Some(1),
            style: Some("typography.body".into()),
            ..Props::default()
        };
        clipped
            .tokens
            .insert("foreground".into(), "text.muted".into());

        let mut field = Props {
            placeholder: Some("An Input with a placeholder and no value".into()),
            style: Some("typography.body".into()),
            ..Props::default()
        };
        field
            .tokens
            .insert("background".into(), "surface.raised".into());
        field
            .tokens
            .insert("foreground".into(), "text.muted".into());

        Self::column(
            "text",
            sp("spacing.sm"),
            vec![
                Self::heading("text-h", "Text and Input"),
                ViewNode::new(NodeKind::Text, "wrapped")
                    .with_props(wrapped)
                    .with_constraints(Constraints {
                        horizontal: AxisConstraint {
                            min: Some(420.0),
                            max: Some(420.0),
                            priority: 0,
                        },
                        // A floor, not a cap. A vertical stack offers each
                        // child an exact height, and `layout/text.rs` treats
                        // an exact vertical offer as a ceiling it will never
                        // grow past — so without room reserved here the
                        // wrapping node is capped to a single clipped line
                        // and the section demonstrates nothing.
                        vertical: AxisConstraint {
                            min: Some(80.0),
                            max: None,
                            priority: 0,
                        },
                    }),
                Self::body(
                    "elide-note",
                    "the same string capped at one line:",
                    "text.muted",
                ),
                ViewNode::new(NodeKind::Text, "elided")
                    .with_props(clipped)
                    .with_constraints(Self::width(420.0)),
                Self::body(
                    "scripts",
                    "mixed scripts: Ünïcödé · 日本語 · العربية",
                    "text.primary",
                ),
                ViewNode::new(NodeKind::Input, "field")
                    .with_props(field)
                    .interactive(
                        Role::TextInput,
                        "Filter".to_owned(),
                        &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
                    ),
            ],
        )
    }

    /// A `Scroll` over a `Collection` that claims a hundred thousand rows.
    ///
    /// The collection asks [`RowSource`] only for the window it can see, so
    /// the row count below is a claim about the store and not about work
    /// this frame did. Watch the placement counter in the strip: it does not
    /// grow with `TOTAL_ROWS`.
    fn collection_section() -> ViewNode {
        let list = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            source: Some(ROW_SOURCE.into()),
            total_count: Some(TOTAL_ROWS),
            estimated_extent: Some(22.0),
            axis: Some(Axis::Vertical),
            ..Props::default()
        });
        Self::column(
            "collection",
            sp("spacing.sm"),
            vec![
                Self::heading("coll-h", "Scroll over a virtualized Collection"),
                Self::body(
                    "coll-note",
                    "100 000 rows declared; only the visible window is ever placed",
                    "text.muted",
                ),
                ViewNode::new(NodeKind::Scroll, "scroll")
                    .with_props(Props {
                        axis: Some(Axis::Vertical),
                        // On the scroll, not on the collection inside it. Tree
                        // acceptance refuses `overscan` on a nested collection
                        // rather than silently ignoring it, because the scroll
                        // is what actually resolves the value.
                        overscan: Some(64.0),
                        ..Props::default()
                    })
                    .with_constraints(Constraints {
                        vertical: AxisConstraint {
                            min: Some(220.0),
                            max: Some(220.0),
                            priority: 0,
                        },
                        ..Constraints::default()
                    })
                    .child(list),
            ],
        )
    }

    /// The three surface input policies, each openable and each behaving
    /// differently on a click outside it.
    fn surface_section(&self) -> ViewNode {
        Self::column(
            "surfaces",
            sp("spacing.sm"),
            vec![
                Self::heading("surf-h", "Surface input policies"),
                Self::body(
                    "surf-note",
                    "Block swallows a click outside it · Passthrough lets it through · \
                     DismissOutside lets it through and asks to close",
                    "text.muted",
                ),
                Self::row(
                    "surf-row",
                    sp("spacing.md"),
                    vec![
                        Self::control(
                            "open-modal",
                            "Open Block modal",
                            Some("surface.raised"),
                            "text.primary",
                            Some("text.muted"),
                        ),
                        Self::control(
                            "open-menu",
                            "Open DismissOutside popup",
                            Some("surface.raised"),
                            "text.primary",
                            Some("text.muted"),
                        ),
                        Self::control(
                            "open-pass",
                            "Open Passthrough panel",
                            Some("surface.raised"),
                            "text.primary",
                            Some("text.muted"),
                        ),
                    ],
                ),
            ],
        )
    }

    /// One `Image` and one `Custom`, declared so their absence is counted.
    fn undrawn_section() -> ViewNode {
        Self::column(
            "undrawn",
            sp("spacing.sm"),
            vec![
                Self::heading("undrawn-h", "Declared with no painter"),
                Self::body(
                    "undrawn-note",
                    "laid out and measured, but their content has no painter; \
                     they appear in the undrawn counter above",
                    "text.muted",
                ),
                Self::row(
                    "undrawn-row",
                    sp("spacing.md"),
                    vec![
                        Self::placeholder_box(
                            NodeKind::Image,
                            "logo",
                            Props {
                                image: Some("gallery/logo".into()),
                                ..Props::default()
                            },
                        ),
                        Self::placeholder_box(
                            NodeKind::Custom,
                            "sparkline",
                            Props {
                                custom_kind: Some(CUSTOM_KIND.into()),
                                ..Props::default()
                            },
                        ),
                    ],
                ),
            ],
        )
    }

    /// A filled rectangle of an exact size.
    ///
    /// The painter knows exactly three token slots — `background`, `border`
    /// and `foreground` — so every piece of widget chrome in this file is
    /// this function: a coloured box, sometimes with an edge. There is no
    /// corner radius, no shadow, no icon set. A checkbox is a box inside a
    /// box; a toggle is a box that moves; a progress bar is a box inside a
    /// wider box. Saying so plainly is more useful than making it look like
    /// there is more vocabulary than there is.
    fn swatch(key: &str, w: f32, h: f32, fill: Option<&str>, edge: Option<&str>) -> ViewNode {
        let mut props = Props::default();
        if let Some(fill) = fill {
            props.tokens.insert("background".into(), fill.into());
        }
        if let Some(edge) = edge {
            props.tokens.insert("border".into(), edge.into());
        }
        ViewNode::new(NodeKind::Spacer, key)
            .with_props(props)
            .with_constraints(Constraints {
                horizontal: AxisConstraint {
                    min: Some(w),
                    max: Some(w),
                    priority: 0,
                },
                vertical: AxisConstraint {
                    min: Some(h),
                    max: Some(h),
                    priority: 0,
                },
            })
    }

    /// Text on a filled, optionally bordered box — the shape every control
    /// here has.
    fn chip(key: &str, label: &str, fill: Option<&str>, fg: &str, edge: Option<&str>) -> ViewNode {
        let mut props = Props {
            text: Some(label.to_owned()),
            style: Some("typography.body".into()),
            ..Props::default()
        };
        props.tokens.insert("foreground".into(), fg.into());
        if let Some(fill) = fill {
            props.tokens.insert("background".into(), fill.into());
        }
        if let Some(edge) = edge {
            props.tokens.insert("border".into(), edge.into());
        }
        ViewNode::new(NodeKind::Text, key).with_props(props)
    }

    /// A focusable, clickable control.
    ///
    /// Flush against its own edge, because padding cannot be composed here.
    /// `Props` has no padding field, and the obvious workaround — spacers
    /// above and below the label inside the fill — does not survive layout: a
    /// vertical `Stack` divides the height it is offered equally among its
    /// children instead of sizing to content, so a three-child column offered
    /// 15.1pt hands the label 5.0pt for a run that needs 14 and the glyphs
    /// spill out of the box. Measured, both with inflexible pads and with
    /// pads declared `min: 0, max: pad`; the split was 5/5/5 either way.
    ///
    /// So: a `padding` prop is not a nicety here, it is the difference
    /// between being able to draw a button and not.
    fn control(
        key: &str,
        label: &str,
        fill: Option<&str>,
        fg: &str,
        edge: Option<&str>,
    ) -> ViewNode {
        Self::chip(key, label, fill, fg, edge).interactive(
            Role::Button,
            label.to_owned(),
            &[Interaction::Focus, Interaction::Click],
        )
    }

    /// A hard width clamp, so a text node is measured against a width the
    /// window cannot widen out from under it.
    fn width(value: f32) -> Constraints {
        Constraints {
            horizontal: AxisConstraint {
                min: Some(value),
                max: Some(value),
                priority: 0,
            },
            ..Constraints::default()
        }
    }

    /// A node whose *content* has no painter, given a background so it still
    /// emits a shape.
    ///
    /// Not cosmetic. `PaintReport::is_complete` treats a placement that
    /// emitted nothing as `silent`, and `Host::pass` debug-asserts on it — so
    /// a bare `Image` or `Custom` node **panics a debug-build host**, even
    /// though the missing painter is already recorded in `report.undrawn`.
    /// The known gap trips the alarm meant for unknown ones. Giving these a
    /// background is the workaround; the inconsistency between `undrawn` and
    /// `is_complete` is the thing to fix.
    fn placeholder_box(kind: NodeKind, key: &str, props: Props) -> ViewNode {
        let mut props = props;
        props
            .tokens
            .insert("background".into(), "surface.raised".into());
        props.tokens.insert("border".into(), "text.muted".into());
        ViewNode::new(kind, key).with_props(props)
    }

    fn overlay(&self) -> Option<ViewNode> {
        if self.modal {
            let mut props = Props {
                layer: Some(Layer::Modal),
                anchor: Some(Anchor::Viewport),
                clamp: Some(ClampRule::Shrink),
                input_policy: Some(InputPolicy::Block),
                axis: Some(Axis::Vertical),
                spacing: sp("spacing.sm"),
                ..Props::default()
            };
            props
                .tokens
                .insert("background".into(), "surface.raised".into());
            return Some(
                ViewNode::new(NodeKind::Surface, "modal")
                    .with_props(props)
                    .with_semantics(Semantics {
                        role: Some(Role::Dialog),
                        label: Some("Retire fiber".to_owned()),
                        ..Semantics::default()
                    })
                    .child(Self::heading("modal-t", "Retire worker/indexer?"))
                    .child(ViewNode::new(NodeKind::Separator, "modal-rule"))
                    .child(Self::body(
                        "modal-b",
                        "Its three children are retired with it. A click outside \
                         this dialog is swallowed — that is what Block means.",
                        "text.muted",
                    ))
                    .child(Self::row(
                        "modal-actions",
                        sp("spacing.md"),
                        vec![
                            ViewNode::new(NodeKind::Spacer, "modal-push").with_constraints(
                                Constraints {
                                    vertical: AxisConstraint {
                                        min: Some(0.0),
                                        max: Some(0.0),
                                        priority: 0,
                                    },
                                    ..Constraints::default()
                                },
                            ),
                            Self::control(
                                "modal-close",
                                "Cancel",
                                Some("surface.base"),
                                "text.primary",
                                Some("text.muted"),
                            ),
                            Self::control(
                                "modal-confirm",
                                "Retire",
                                Some("text.primary"),
                                "surface.base",
                                None,
                            ),
                        ],
                    )),
            );
        }
        if self.menu {
            let mut props = Props {
                layer: Some(Layer::Popup),
                anchor: Some(Anchor::Viewport),
                clamp: Some(ClampRule::Shrink),
                input_policy: Some(InputPolicy::DismissOutside),
                axis: Some(Axis::Vertical),
                spacing: sp("spacing.sm"),
                ..Props::default()
            };
            props
                .tokens
                .insert("background".into(), "surface.raised".into());
            let item = |key: &str, label: &str, fg: &str| {
                Self::control(key, label, Some("surface.raised"), fg, None)
            };
            return Some(
                ViewNode::new(NodeKind::Surface, "menu")
                    .with_props(props)
                    .with_semantics(Semantics {
                        role: Some(Role::List),
                        label: Some("Fiber actions".to_owned()),
                        ..Semantics::default()
                    })
                    .child(Self::body("menu-t", "Fiber actions", "text.muted"))
                    .child(item("menu-item", "Inspect", "text.primary"))
                    .child(item("menu-trace", "Follow trace", "text.primary"))
                    .child(ViewNode::new(NodeKind::Separator, "menu-rule"))
                    .child(item("menu-kill", "Kill", "status.down")),
            );
        }
        if self.passthrough {
            let mut props = Props {
                layer: Some(Layer::Toast),
                anchor: Some(Anchor::Viewport),
                clamp: Some(ClampRule::Shrink),
                input_policy: Some(InputPolicy::Passthrough),
                axis: Some(Axis::Vertical),
                spacing: sp("spacing.sm"),
                ..Props::default()
            };
            props
                .tokens
                .insert("background".into(), "surface.raised".into());
            return Some(
                ViewNode::new(NodeKind::Surface, "toast")
                    .with_props(props)
                    .child(Self::body(
                        "toast-t",
                        "Passthrough: clicks reach what is underneath. \
                         Press its button below to close.",
                        "text.primary",
                    ))
                    .child(Self::button("toast-close", "Close")),
            );
        }
        None
    }
}

impl RowSource for Gallery {
    fn rows(&mut self, source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        if source != ROW_SOURCE {
            return Vec::new();
        }
        range
            .map(|i| {
                // Keyed by the row's own index, not by its position in this
                // window, so scrolling does not renumber what is on screen.
                // Uniform, deliberately. An earlier version alternated
                // `text.primary`/`text.muted` per row, which read as a
                // rendering fault rather than as zebra striping — the engine
                // does nothing of the kind on its own.
                let selected = self.selected_row == Some(i);
                Arc::new(
                    Gallery::chip(
                        &format!("row-{i}"),
                        &format!("row {i:>6}  ·  a virtualized entry"),
                        selected.then_some("surface.raised"),
                        "text.primary",
                        None,
                    )
                    .with_semantics(Semantics {
                        role: Some(Role::ListItem),
                        label: Some(format!("row {i}")),
                        selected,
                        ..Semantics::default()
                    }),
                )
            })
            .collect()
    }
}

impl App for Gallery {
    fn view(&mut self) -> ViewNode {
        let mut page = Props {
            axis: Some(Axis::Vertical),
            spacing: sp("spacing.lg"),
            ..Props::default()
        };
        page.tokens
            .insert("background".into(), "surface.base".into());

        let mut root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(page)
            .child(Gallery::heading("title", "Petra component gallery"))
            .child(Gallery::body(
                "counters",
                &self.counters.line(),
                "text.muted",
            ))
            .child(Gallery::body(
                "last",
                &format!(
                    "last event: {}   ·   dismissals reported: {}",
                    if self.last_event.is_empty() {
                        "none yet — click a button or press Tab"
                    } else {
                        &self.last_event
                    },
                    self.dismissals
                ),
                "text.muted",
            ))
            .child(Gallery::section(
                "widgets",
                "Controls",
                vec![
                    self.buttons_row(),
                    self.controls_row(),
                    self.tabs_row(),
                    self.progress_row(),
                ],
            ))
            .child(Gallery::section(
                "forms",
                "Form",
                vec![Gallery::form_grid()],
            ))
            .child(Gallery::layout_section())
            .child(Gallery::text_section())
            .child(Gallery::status_section())
            .child(self.surface_section())
            .child(Gallery::collection_section())
            .child(Gallery::undrawn_section());

        if let Some(probe) = self.probe.clone() {
            root = root.child(probe);
        }
        // The page scrolls. Without this the root stack is offered exactly the
        // window's height and divides it among its sections, so adding a
        // section does not make the page longer — it makes every existing
        // section shorter, down to text squeezed to a few points tall. A
        // vertical stack distributes the height it is given; something has to
        // give it an unbounded one, and a scroll is that something.
        let page = ViewNode::new(NodeKind::Scroll, "page")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                overscan: Some(64.0),
                ..Props::default()
            })
            .child(root);

        // Surfaces sit beside the page, not inside it. They are anchored to
        // the viewport, and a viewport-anchored thing inside scrolling content
        // is placed in the wrong coordinate space — a popup that moves when
        // the page moves, and hit-testing that disagrees with what is on
        // screen. `Overlay` is the container for this: every child gets the
        // container's own proposal, z-order by child order.
        let mut shell = ViewNode::new(NodeKind::Overlay, "shell").child(page);
        if let Some(overlay) = self.overlay() {
            shell = shell.child(overlay);
        }
        shell
    }

    fn handle(&mut self, event: &InputEvent, route: &Route) {
        let node = match route {
            Route::Pointer { node } | Route::Keyboard { node } => node.clone(),
            Route::Unrouted { reason } => {
                self.last_event = format!("unrouted ({reason})");
                return;
            }
        };
        if !activates(event) {
            self.last_event = format!("{node} (not an activation)");
            return;
        }
        self.last_event = format!("activated {node}");
        // Ids are canonical key paths, so match on the tail rather than
        // rebuilding the whole path here.
        let tail = node.rsplit('/').next().unwrap_or_default().to_owned();
        match tail.as_str() {
            "open-modal" => self.modal = true,
            "open-menu" => self.menu = true,
            "open-pass" => self.passthrough = true,
            "modal-close" | "modal-confirm" => self.modal = false,
            "toast-close" => self.passthrough = false,
            "menu-item" | "menu-trace" | "menu-kill" => self.menu = false,
            "Fibers" => self.tab = 0,
            "Trace" => self.tab = 1,
            "Capabilities" => self.tab = 2,
            _ => {
                if let Some(i) = tail.strip_prefix("row-").and_then(|n| n.parse().ok()) {
                    self.selected_row = Some(i);
                }
            }
        }
    }

    fn dismissed(&mut self, ids: &[String]) {
        // The engine cannot close anything: it reports the request and this
        // application decides. Closing the menu here is what makes
        // `InputPolicy::DismissOutside` mean anything on screen.
        self.dismissals += ids.len();
        if ids.iter().any(|id| id.ends_with("/menu")) {
            self.menu = false;
        }
    }

    fn take_changes(&mut self) -> ChangeSet {
        // This gallery rebuilds its whole tree every pass, so `All` is the
        // only honest answer. Naming individual nodes while handing back
        // fresh `Arc`s is exactly the under-declaration
        // `ReuseState::verify_declaration` panics on in a debug build.
        ChangeSet::All
    }
}

/// An opt-in capture, driven by `PETRA_GALLERY_SHOT=<path>`.
///
/// Exists because a window nobody can see is weak evidence. The compositor on
/// this machine gives no reliable way to grab one window by name, so the
/// window grabs itself: it settles, asks egui for its own framebuffer, writes
/// it out, and closes.
///
/// The format is binary PPM, chosen so this needs no image crate — a header
/// and RGB triples. Convert with any of the usual tools if you want a PNG.
struct ShotPlan {
    path: std::path::PathBuf,
    passes: u32,
    requested: bool,
}

/// Passes to let the host settle before asking for the framebuffer.
///
/// Three, not one: the first pass seats focus *after* petrify and asks for
/// another frame, so a capture on pass one photographs a frame the host has
/// already said is not final.
const SHOT_AFTER_PASSES: u32 = 3;

fn write_ppm(path: &std::path::Path, image: &egui::ColorImage) -> std::io::Result<()> {
    use std::io::Write as _;
    let [w, h] = image.size;
    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(out, "P6\n{w} {h}\n255\n")?;
    for px in &image.pixels {
        let [r, g, b, _a] = px.to_srgba_unmultiplied();
        out.write_all(&[r, g, b])?;
    }
    out.flush()
}

/// Wraps the host so each pass can read the counters back off it.
///
/// `Host` already implements `eframe::App`, and using it directly would be
/// one line — but then nothing could see `report()` afterwards, and the
/// counters are the reason this window exists.
struct GalleryWindow {
    host: Host<Gallery>,
    shot: Option<ShotPlan>,
}

impl eframe::App for GalleryWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.host.pass(&ctx);

        let seq = self.host.frame().map_or(0, |f| f.seq);
        let placements = self.host.frame().map_or(0, |f| f.placements.len());
        let counters = self
            .host
            .report()
            .map_or_else(Counters::default, |r| Counters {
                seq,
                placements,
                drawn: r.drawn,
                silent: r.silent,
                empty: r.empty,
                clipped: r.skipped_clipped,
                focus_rings: r.focus_rings,
                blind_focus: r.blind_focus,
                unresolved: r.unresolved_tokens.len(),
                undrawn: r.undrawn.len(),
                desynced: r.desynced,
            });
        // Written for the *next* pass to render. The application builds its
        // tree before the frame that measures it exists, so there is no way
        // to show this pass's own numbers this pass; the label says
        // "previous pass" rather than pretending otherwise.
        self.host.app_mut().counters = counters;

        let Some(plan) = &mut self.shot else { return };
        plan.passes += 1;
        // The host stops asking for frames once nothing is moving, which is
        // the zero-idle contract working — so a pending capture has to keep
        // asking for itself.
        ctx.request_repaint();
        if plan.passes >= SHOT_AFTER_PASSES && !plan.requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            plan.requested = true;
            return;
        }
        if !plan.requested {
            return;
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            match write_ppm(&plan.path, &image) {
                Ok(()) => println!(
                    "gallery: wrote {}x{} to {}",
                    image.size[0],
                    image.size[1],
                    plan.path.display()
                ),
                // Loud, and still closes: a capture run that silently leaves
                // no file and exits 0 is the theatre this whole harness is
                // built to avoid.
                Err(err) => eprintln!("gallery: could not write {}: {err}", plan.path.display()),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 900.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Petra component gallery",
        options,
        Box::new(|cc| {
            let mut host = Host::new(&cc.egui_ctx, Gallery::default(), default_presenter());
            host.registry_mut().register_custom_kind(CUSTOM_KIND);
            let shot = std::env::var_os("PETRA_GALLERY_SHOT").map(|path| ShotPlan {
                path: std::path::PathBuf::from(path),
                passes: 0,
                requested: false,
            });
            Ok(Box::new(GalleryWindow { host, shot }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{Gallery, TOTAL_ROWS, sp};
    use egui::{Context, Event, Modifiers, Pos2, RawInput};
    use gorgon_petra::geom::Point;
    use gorgon_petra::tree::{NodeKind, ViewNode};
    use gorgon_petra_egui::host::{App, Host, default_presenter};

    /// The window size `main` asks for.
    ///
    /// The tests run at it deliberately. An `egui::Context` with a default
    /// `RawInput` reports a screen about 10 000pt tall, and at that size
    /// nothing in this gallery ever clips, scrolls, or runs out of room — so
    /// a headless suite that takes the default is not exercising the layout
    /// a person actually sees.
    const WINDOW: [f32; 2] = [1100.0, 900.0];

    fn sized(mut input: RawInput) -> RawInput {
        input.screen_rect = Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(WINDOW[0], WINDOW[1]),
        ));
        input
    }

    fn headless() -> Context {
        let ctx = Context::default();
        ctx.run_ui(sized(RawInput::default()), |_| {})
            .drop_without_applying_deltas();
        ctx
    }

    fn step(ctx: &Context, host: &mut Host<Gallery>, input: RawInput) {
        ctx.run_ui(sized(input), |_| host.pass(ctx))
            .drop_without_applying_deltas();
    }

    fn press_at(pos: Pos2) -> RawInput {
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::default(),
        });
        input
    }

    fn host() -> (Context, Host<Gallery>) {
        let ctx = headless();
        let mut host = Host::new(&ctx, Gallery::default(), default_presenter());
        host.registry_mut().register_custom_kind(super::CUSTOM_KIND);
        (ctx, host)
    }

    /// The gallery's own tree is acceptable.
    ///
    /// First, because a refused tree is replaced wholesale by `refusal_view`
    /// and every other test here would then be asserting against an error
    /// message instead of the gallery — a failure mode that reads as "five
    /// unrelated things broke" rather than "one node is wrong".
    #[test]
    fn the_gallery_tree_is_accepted() {
        let tree = Gallery::default().view();
        let mut registry = gorgon_petra::tree::Registry::new();
        registry.register_custom_kind(super::CUSTOM_KIND);
        if let Err(errors) = gorgon_petra::tree::validate(&tree, &registry) {
            panic!("the gallery's own tree is not acceptable: {errors}");
        }
    }

    /// A `Spacer` with no constraints takes the cross axis as well as the
    /// main one.
    ///
    /// Pinned rather than fixed. `layout/leaf.rs` answers a spacer's measure
    /// with the offered extent on *both* axes, so a spacer in a horizontal
    /// row is as tall as the row can be — which is not what a reader coming
    /// from SwiftUI expects, where `Spacer()` in an `HStack` contributes no
    /// height. Changing it is a semantics decision in shipped, tested engine
    /// code and is not this gallery's to make; recording it is.
    #[test]
    fn an_unconstrained_spacer_claims_the_cross_axis_too() {
        let row = Gallery::row(
            "probe",
            sp("spacing.sm"),
            vec![
                Gallery::body("a", "left", "text.primary"),
                ViewNode::new(NodeKind::Spacer, "gap"),
                Gallery::body("b", "right", "text.primary"),
            ],
        );
        let (ctx, mut host) = host();
        host.app_mut().probe = Some(row);
        step(&ctx, &mut host, RawInput::default());

        let frame = host.frame().expect("a frame");
        let find = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"))
                .rect
                .h
        };
        let text_h = find("/probe/a");
        let gap_h = find("/probe/gap");
        assert!(
            gap_h > text_h * 2.0,
            "this test exists because the spacer is much taller than its text \
             siblings; if it is not, the engine's spacer semantics changed and \
             the gallery's `l-gap` clamp and its comment should go: \
             gap {gap_h}, text {text_h}"
        );
    }

    /// The wrapped and the elided string are the same text and behave
    /// differently.
    ///
    /// Asserted through `PaintState::truncated` and the placed height rather
    /// than by eye: the section is only worth having if the two nodes
    /// actually diverge, and before the width clamp went in they did not.
    #[test]
    fn the_elided_text_truncates_and_the_wrapped_one_does_not() {
        let (ctx, mut host) = host();
        step(&ctx, &mut host, RawInput::default());

        let frame = host.frame().expect("a frame");
        let get = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"))
        };
        let wrapped = get("/text/wrapped");
        let elided = get("/text/elided");

        assert!(
            !wrapped.paint.truncated,
            "the wrapping node must show the whole string: {wrapped:?}"
        );
        assert!(
            elided.paint.truncated,
            "the one-line node must report that it cut the string: {elided:?}"
        );
        assert!(
            wrapped.rect.h > elided.rect.h,
            "wrapping must cost more height than eliding: wrapped {}, elided {}",
            wrapped.rect.h,
            elided.rect.h
        );
    }

    /// Nothing claims an absurd extent.
    ///
    /// Written after the first capture showed a four-hundred-pixel hole in
    /// the middle of the page. A hole is invisible to every other assertion
    /// here — placements are placed, paint is complete, tokens resolve — so
    /// without this the harness would have called that frame clean.
    #[test]
    fn no_placement_claims_an_absurd_extent() {
        let (ctx, mut host) = host();
        step(&ctx, &mut host, RawInput::default());

        let frame = host.frame().expect("a frame");
        let viewport_h = frame.viewport.size.h;
        let mut tall: Vec<(String, f32)> = frame
            .placements
            .iter()
            // Scroll *content* is allowed to dwarf the viewport — that is
            // what the scroll above it exists to move through. Two things
            // qualify: the collection's own box, which is the whole store's
            // extent, and the page itself, which is as long as its sections
            // need. Everything else is a layout fault.
            .filter(|p| !p.id.ends_with("/page/root") && !p.id.ends_with("/rows"))
            .map(|p| (p.id.clone(), p.rect.h))
            .filter(|(_, h)| *h > viewport_h)
            .collect();
        tall.sort_by(|a, b| b.1.total_cmp(&a.1));
        assert!(
            tall.is_empty(),
            "these placements are taller than the whole {viewport_h}pt viewport: {tall:#?}"
        );
    }

    /// The whole gallery lays out and paints with nothing left silent.
    ///
    /// This is the assertion the window cannot make for itself: a component
    /// can look plausible on screen while the paint pass quietly failed to
    /// account for it.
    #[test]
    fn every_component_places_and_paints_completely() {
        let (ctx, mut host) = host();
        step(&ctx, &mut host, RawInput::default());

        let report = host.report().expect("a paint report");
        assert!(report.is_complete(), "{report:?}");
        assert!(!report.desynced, "{report:?}");
        assert!(
            report.unresolved_tokens.is_empty(),
            "every token this gallery binds must exist in the shipped theme: {:?}",
            report.unresolved_tokens
        );
        assert!(
            report.unknown_slots.is_empty(),
            "the gallery must not bind a slot the painter does not know: {:?}",
            report.unknown_slots
        );
        assert_eq!(
            report.blind_focus, 0,
            "a focused node with no ring is a node a keyboard user cannot find: {report:?}"
        );
        assert!(
            report.texts > 20,
            "the gallery declares far more than twenty text runs; {} drawn means whole \
             sections silently vanished",
            report.texts
        );
    }

    /// `Image` and `Custom` are the only two kinds without a painter, and the
    /// gallery declares exactly one of each so the gap stays counted.
    ///
    /// If someone adds a painter for either, this test fails and tells them
    /// to update the gallery's own claim about itself — which is the point.
    #[test]
    fn only_the_declared_undrawn_section_lacks_a_painter() {
        let (ctx, mut host) = host();
        step(&ctx, &mut host, RawInput::default());

        let undrawn = &host.report().expect("a report").undrawn;
        // Keyed by the content that had no painter, not by node id.
        assert_eq!(
            undrawn.len(),
            2,
            "expected exactly the image and the custom kind: {undrawn:?}"
        );
        // T081 makes an unresolvable image source name *itself* rather than
        // reporting a bare kind, so an operator reading `undrawn` learns which
        // source failed. The gallery declares one image, `gallery/logo`, and no
        // `ImageSources` registry is wired here, so that is the name it lands as.
        assert!(undrawn.contains("image:gallery/logo"), "{undrawn:?}");
        assert!(
            undrawn.contains(&format!("custom:{}", super::CUSTOM_KIND)),
            "{undrawn:?}"
        );
    }

    /// The collection claims a hundred thousand rows and places a window.
    ///
    /// Asserted as a bound on total placements rather than on the row count,
    /// because the failure this guards against — virtualization silently
    /// falling back to placing everything — shows up as a frame with a
    /// hundred thousand placements in it.
    #[test]
    fn the_collection_places_a_window_not_the_whole_store() {
        let (ctx, mut host) = host();
        step(&ctx, &mut host, RawInput::default());

        let placed = host.frame().expect("a frame").placements.len();
        assert!(
            placed < 500,
            "the whole gallery placed {placed} nodes against a store of {TOTAL_ROWS} rows; \
             virtualization is not doing its job"
        );
    }

    /// Opening the `Block` modal makes the page behind it inert.
    #[test]
    fn the_block_modal_swallows_a_click_outside_itself() {
        let (ctx, mut host) = host();
        host.app_mut().modal = true;
        step(&ctx, &mut host, RawInput::default());

        let modal = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id.ends_with("/modal"))
            .expect("the modal was placed")
            .rect;
        let outside = Pos2::new(modal.x + modal.w + 20.0, modal.y + modal.h + 20.0);
        assert!(!modal.contains(Point::new(outside.x, outside.y)));

        step(&ctx, &mut host, press_at(outside));
        assert!(
            host.app().last_event.starts_with("unrouted"),
            "a press outside a Block surface must be reported as a drop, not delivered: {}",
            host.app().last_event
        );
        assert!(host.app().modal, "nothing asked the modal to close");
    }

    /// A press outside the `DismissOutside` popup closes it, through the
    /// application, because the engine cannot close anything itself.
    #[test]
    fn a_press_outside_the_dismiss_popup_closes_it() {
        let (ctx, mut host) = host();
        host.app_mut().menu = true;
        step(&ctx, &mut host, RawInput::default());

        let menu = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id.ends_with("/menu"))
            .expect("the menu was placed")
            .rect;
        let outside = Pos2::new(menu.x + menu.w + 20.0, menu.y + menu.h + 20.0);

        step(&ctx, &mut host, press_at(outside));
        assert_eq!(
            host.app().dismissals,
            1,
            "the host must report exactly one dismissal"
        );
        assert!(
            !host.app().menu,
            "the application must have closed the menu"
        );
    }

    /// The inverse of the test above, so it cannot pass by dismissing on
    /// every press: inside the popup, nothing closes.
    #[test]
    fn a_press_inside_the_dismiss_popup_closes_nothing() {
        let (ctx, mut host) = host();
        host.app_mut().menu = true;
        step(&ctx, &mut host, RawInput::default());

        let menu = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id.ends_with("/menu"))
            .expect("the menu was placed")
            .rect;
        let inside = Pos2::new(menu.x + menu.w / 2.0, menu.y + menu.h / 2.0);

        step(&ctx, &mut host, press_at(inside));
        assert_eq!(host.app().dismissals, 0);
        assert!(host.app().menu, "a press inside must not close it");
    }
}
