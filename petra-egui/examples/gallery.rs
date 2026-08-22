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
use gorgon_petra::token::StatusShape;
use gorgon_petra::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, InputPolicy, Interaction, Layer, NodeKind,
    Props, Role, TextWrap, TrackSize, ViewNode,
};
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
const STATUSES: [(&str, &str); 3] = [
    ("status.ok", "Ok"),
    ("status.degraded", "Degraded"),
    ("status.down", "Down"),
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

#[derive(Default)]
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

    fn row(key: &str, spacing: f32, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                spacing: Some(spacing),
                align: Some(Align::Start),
                ..Props::default()
            })
            .with_children(children)
    }

    fn column(key: &str, spacing: f32, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
            .with_props(Props {
                axis: Some(Axis::Vertical),
                spacing: Some(spacing),
                ..Props::default()
            })
            .with_children(children)
    }

    /// Every shipped status, shown through all three of its channels at once.
    fn status_section() -> ViewNode {
        let cells = STATUSES
            .iter()
            .map(|(token, words)| {
                let shape = match *token {
                    "status.ok" => StatusShape::Circle,
                    "status.degraded" => StatusShape::Triangle,
                    _ => StatusShape::Square,
                };
                let glyph = match shape {
                    StatusShape::Circle => "●",
                    StatusShape::Triangle => "▲",
                    StatusShape::Square => "■",
                    StatusShape::Diamond => "◆",
                };
                let mut props = Props {
                    text: Some(format!("{glyph}  {words}")),
                    style: Some("typography.body".into()),
                    ..Props::default()
                };
                props.tokens.insert("foreground".into(), (*token).into());
                ViewNode::new(NodeKind::Text, *token).with_props(props)
            })
            .collect();
        Self::column(
            "status",
            6.0,
            vec![
                Self::heading("status-h", "Status tokens"),
                Self::body(
                    "status-note",
                    "the design gives each status a shape and a word so hue is never \
                     the only channel — but two of the three shapes are tofu in the \
                     shipped font, so today only the word survives",
                    "text.muted",
                ),
                Self::row("status-row", 24.0, cells),
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
                column_spacing: Some(8.0),
                row_spacing: Some(4.0),
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
            6.0,
            vec![
                Self::heading("layout-h", "Grid, Stack, Spacer, Separator"),
                grid,
                Self::row(
                    "layout-row",
                    8.0,
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
            6.0,
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
            6.0,
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
                    .child(list),
            ],
        )
    }

    /// The three surface input policies, each openable and each behaving
    /// differently on a click outside it.
    fn surface_section(&self) -> ViewNode {
        Self::column(
            "surfaces",
            6.0,
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
                    12.0,
                    vec![
                        Self::button("open-modal", "Open Block modal"),
                        Self::button("open-menu", "Open DismissOutside popup"),
                        Self::button("open-pass", "Open Passthrough panel"),
                    ],
                ),
            ],
        )
    }

    /// One `Image` and one `Custom`, declared so their absence is counted.
    fn undrawn_section() -> ViewNode {
        Self::column(
            "undrawn",
            6.0,
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
                    12.0,
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
                spacing: Some(8.0),
                ..Props::default()
            };
            props
                .tokens
                .insert("background".into(), "surface.raised".into());
            return Some(
                ViewNode::new(NodeKind::Surface, "modal")
                    .with_props(props)
                    .child(Self::body(
                        "modal-t",
                        "Block: a click outside this panel is swallowed",
                        "text.primary",
                    ))
                    .child(Self::button("modal-close", "Close")),
            );
        }
        if self.menu {
            let mut props = Props {
                layer: Some(Layer::Popup),
                anchor: Some(Anchor::Viewport),
                clamp: Some(ClampRule::Shrink),
                input_policy: Some(InputPolicy::DismissOutside),
                axis: Some(Axis::Vertical),
                spacing: Some(8.0),
                ..Props::default()
            };
            props
                .tokens
                .insert("background".into(), "surface.raised".into());
            return Some(
                ViewNode::new(NodeKind::Surface, "menu")
                    .with_props(props)
                    .child(Self::body(
                        "menu-t",
                        "DismissOutside: click anywhere outside to close",
                        "text.primary",
                    ))
                    .child(Self::button("menu-item", "An item")),
            );
        }
        if self.passthrough {
            let mut props = Props {
                layer: Some(Layer::Toast),
                anchor: Some(Anchor::Viewport),
                clamp: Some(ClampRule::Shrink),
                input_policy: Some(InputPolicy::Passthrough),
                axis: Some(Axis::Vertical),
                spacing: Some(8.0),
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
                Arc::new(Gallery::body(
                    &format!("row-{i}"),
                    &format!("row {i:>6}  ·  a virtualized entry"),
                    if i % 2 == 0 {
                        "text.primary"
                    } else {
                        "text.muted"
                    },
                ))
            })
            .collect()
    }
}

impl App for Gallery {
    fn view(&mut self) -> ViewNode {
        let mut page = Props {
            axis: Some(Axis::Vertical),
            spacing: Some(18.0),
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
            .child(Gallery::layout_section())
            .child(Gallery::text_section())
            .child(Gallery::status_section())
            .child(self.surface_section())
            .child(Gallery::collection_section())
            .child(Gallery::undrawn_section());

        if let Some(probe) = self.probe.clone() {
            root = root.child(probe);
        }
        if let Some(overlay) = self.overlay() {
            root = root.child(overlay);
        }
        root
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
        match node.rsplit('/').next().unwrap_or_default() {
            "open-modal" => self.modal = true,
            "open-menu" => self.menu = true,
            "open-pass" => self.passthrough = true,
            "modal-close" => self.modal = false,
            "toast-close" => self.passthrough = false,
            _ => {}
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
    use super::{Gallery, TOTAL_ROWS};
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
            8.0,
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
            // A collection's own box is the whole store's extent by design —
            // that is what the scroll above it scrolls through — so it is the
            // one thing here allowed to dwarf the viewport.
            .filter(|p| !p.id.ends_with("/rows"))
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
        assert!(undrawn.contains("image"), "{undrawn:?}");
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
