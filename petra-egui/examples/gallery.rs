//! A live window over every Petra component that has a painter today.
//!
//! This is the first thing in the repository that opens a window. Everything
//! else drives `Host` headless, which proves the layout and the digest but
//! never proves that a font loads, that wgpu is reachable, or that a click
//! from a real compositor lands where the hit test says it does.
//!
//! It is a **harness, not a demo**. The band under the masthead reports the
//! previous frame's own honesty counters — placements, silent nodes,
//! unresolved tokens, focused-but-unringed nodes — so a component that
//! "looks fine" while the engine is unhappy about it is visible rather than
//! flattering. A demo hides those. This one is built around them, and the
//! sparkline beside them is the same numbers over time, drawn by a
//! host-registered painter through the FR-059 escape hatch.
//!
//! Run it: `cargo run -p gorgon-petra-egui --example gallery`
//! Light theme: `PETRA_GALLERY_THEME=light cargo run … --example gallery`
//! Capture one frame and exit: `PETRA_GALLERY_SHOT=/tmp/gallery.ppm cargo run …`
//! See the whole page, not the top of it: `PETRA_GALLERY_SIZE=1200x1900 cargo run …`
//!   — see [`viewport_from_env`]. The default pin is what makes two captures
//!   comparable, so anything taken with this set is a picture, not a
//!   measurement, and the run says so on stderr.
//! Retone any colour token: `PETRA_GALLERY_COLOR=text.muted=#c6c6c6 cargo run …`
//!   — see [`colors_from_env`]. Comma-separate to move several at once.
//! Sweep the glyph-sharpness dial: `PETRA_GALLERY_COVERAGE=3,snap cargo run …`
//!   — see [`coverage_from_env`]. Overrides `text.coverage-curve` for this one
//!   window only, so the two dials can be judged by eye at real size without
//!   editing and rebuilding `gorgon-petra` once per value.
//!
//! # What this file is not allowed to do (T078, SC-011)
//!
//! It contains **no literal style value**. Every gap is a step of the shipped
//! spacing ramp, every corner a step of the shipped shape ramp, every colour
//! and type step a name from [`gorgon_petra::token::standard_vocabulary`].
//! `cargo xtask verify-literal-style` scans this file — it is most of that
//! lane's whole corpus — and the scan is text-based, so **every token name
//! below is spelled as a literal at its own call site on purpose**. Hiding
//! the names behind `const`s the way `petra/src/component/tokens.rs` does
//! would move them out of the lane's reach and turn a real gate into a
//! rubber stamp. The repetition here is the gate's fixture.
//!
//! # What it composes from
//!
//! Everything the component library covers comes from the component library
//! ([`gorgon_petra::component`], thirteen names). The primitives are reached
//! for only where C13 deliberately stops: `Grid` track sizing, `Scroll`,
//! `Collection`, `Overlay`, `Surface`, `Separator`, `Spacer`, and the two
//! hosted kinds. Three gaps in the library turned up while writing this page
//! and are worked around here rather than papered over — see [`muted`] and
//! [`caption`], each of which names the one it is standing in for, and
//! neither of which reaches past a component's own public surface into its
//! children. Two more gaps were real too and are now closed in the library
//! itself rather than worked around here: a control had no way to know which
//! surface it was being re-seated onto
//! ([`gorgon_petra::component::on_layer`]), and there was no disabled variant
//! ([`gorgon_petra::component::disabled`]).
//!
//! # What this page assumes about its window
//!
//! At least about 1100 logical units of width. Everything on it is weighted
//! or fit-to-content except one track: the text card's measure is a fixed
//! 360-unit column, because a demonstration of wrapping has to be measured
//! against a width the window cannot widen out from under it. Below roughly
//! 1100 the two page columns fall under 360 and that card starts truncating
//! — honestly, and visibly, which is the point, but it is a limit rather
//! than a surprise. There is no breakpoint machinery in Petra to turn the
//! two columns into one, and inventing one here would be a layout feature
//! wearing a gallery's clothes.
//!
//! # One thing on this page is still wrong, and is not this file's to fix
//!
//! * **A `progress` bar draws no fill.** `petra/src/component/progress.rs`
//!   builds both of its cells with `swatch(_, 0.0, 10.0, …)`, and `swatch`
//!   clamps width to exactly its first argument — so the fill and the track
//!   are both zero units wide whatever the grid's column weights say, and
//!   the bar is an empty outline. The percentage beside it is text this
//!   file adds, which is why the value is still legible.
//!
//! The type ramp used to be the other entry here: the shaper's style map was
//! keyed on `body`/`heading`/`small`/`mono` while the vocabulary declared
//! `typography.body`/`typography.heading`/…, so every run on this page
//! painted at 14 units. `Typography::from_theme` now reads the map off the
//! theme and `Host` binds it, so the sizes in the tree are the sizes on
//! screen. Weight is still flat: egui's default fonts install one
//! proportional face, so a `Bold` token paints at regular weight until a host
//! installs a bold face and names it through `Host::set_font_faces`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gorgon_petra::component::{
    button, checkbox, field, heading, list_row, on_layer, primary_button, progress, radio, section,
    tab, tab_bar, text, toggle,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::value::{ColorValue, CoverageValue, TokenValue};
use gorgon_petra::token::{Presenter, Theme, TokenName, dark, light, standard_vocabulary};
use gorgon_petra::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, InputPolicy, Layer, NodeKind, Props, Role,
    Semantics, TrackSize, ViewNode,
};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_egui::paint::{CustomPaintCtx, CustomPainters};

/// The shared authoring helpers of this page and `parity.rs`'s — the token
/// references, the text roles the component library does not carry, and the
/// primitive shapes C13 leaves to the primitives. Included per-target via
/// `#[path]`, so this example stays one self-contained build target and the
/// wasm canary cannot break on this page's edits.
#[path = "support/mod.rs"]
mod support;

use support::*;

/// The one custom kind this gallery declares. Registered on the host's
/// [`gorgon_petra::tree::Registry`] before the first pass (an unregistered
/// custom kind is a tree-acceptance violation) *and* on the host's
/// [`CustomPainters`] (an unregistered painter is a reported gap in
/// `PaintReport::undrawn`). Two registrations, two different failures,
/// deliberately separate: see [`register_sparkline`].
const CUSTOM_KIND: &str = "sparkline";

/// The image source this page declares and **does not** register a loader
/// for, on purpose. It is the control for the sparkline: the two hosted
/// kinds are described in the same card, one drawn through a registered
/// painter and one left to land in `PaintReport::undrawn` under its own
/// name — so the counter that reports the gap is proved to still work at the
/// same moment the escape hatch is proved to work.
const IMAGE_SOURCE: &str = "gallery/logo";

/// Rows the virtualized `Collection` section pulls from.
const ROW_SOURCE: &str = "gallery/rows";
/// How many rows that collection claims to have. Far more than fit, which is
/// the point: `Collection` asks for the visible window only.
const TOTAL_ROWS: usize = 100_000;

/// How many passes the sparkline remembers.
const HISTORY: usize = 64;

/// The three fibers the status card reports on: a key, the status token that
/// names their state, and the detail line under the name.
///
/// The token names are looked up in [`standard_vocabulary`] at build time
/// rather than reassembled here, because [`status`] takes a whole
/// [`StatusToken`] — colour, shape and text together, FR-015 — and there is
/// deliberately no way to hand it a colour on its own.
const FIBERS: [(&str, &str, &str); 3] = [
    (
        "supervisor/root",
        "status.ok",
        "12 children · 0 restarts · uptime 4h11m",
    ),
    (
        "worker/indexer",
        "status.degraded",
        "retry 3 of 5 · last error 40s ago",
    ),
    ("worker/shaper", "status.down", "disposer deadline exceeded"),
];

/// The restart policy the radio group in the Controls card chooses between.
const POLICIES: [(&str, &str); 3] = [
    ("policy-never", "Never"),
    ("policy-failure", "On failure"),
    ("policy-always", "Always"),
];

// ---------------------------------------------------------------------------
// The design rules this page follows
// ---------------------------------------------------------------------------
//
// Stated once, here, and then obeyed at every call site below. A gallery that
// picks each gap and each corner by eye is a pile of controls; the rules are
// what make it a page.
//
// **Corner scale — radius grows with elevation.**
//   * `shape.corner-full` for anything whose meaning is "round": a status
//     dot, a radio, a toggle's track and knob, a progress bar's ends. The
//     component library already binds these; nothing here overrides them.
//   * `shape.corner-sm` (4) for things flush in a dense list: inputs, tabs,
//     list rows, and the two hosted wells. Library-bound for the first three.
//   * `shape.corner-md` (8) for things that sit *on* the page: buttons and
//     cards. Library-bound for `button` and `section`; this file binds it on
//     the one card it assembles itself (the telemetry band) so that card and
//     a `section` are the same object.
//   * `shape.corner-lg` (12) for the three surfaces that float *above* the
//     page: the modal, the popup and the toast. This is the only step this
//     file chooses, and it is the only one the library does not already fix.
//
// **Spacing rhythm — the gap grows with the depth of the seam it crosses.**
//   All eight steps are used, and each one has exactly one job:
//   `spacing.2xs` (2) inside one label pair: a caption over the thing it
//   names, and the rows of the popup.
//   `spacing.xs` (4) between the rows of one dense grid, where the rule is
//   already doing the separating.
//   `spacing.sm` (8) between the parts of one group: a tab strip and its
//   rule, a fiber's name and its detail, the runs inside the text measure.
//   `spacing.md` (12) the page's working gap — between the blocks inside a
//   card, and between the controls of one row. It is the most common gap on
//   the page by a wide margin, which is what a working gap should be.
//   `spacing.lg` (16) a card's own vertical padding, and the gap between
//   the stat tiles, which is the one place md was too tight to read as five
//   columns.
//   `spacing.xl` (24) between cards, between the two page columns, and a
//   surface's own horizontal padding.
//   `spacing.2xl` (32) between the page's three regions, and its top and
//   bottom margin.
//   `spacing.3xl` (48) the page's left and right margin — the widest gap on
//   the page belongs at its edge.
//   A gap that wants to be something else is a gap nobody chose.
//
// **Hierarchy through type and space.** Four type steps ship: `body` (14),
// `heading-sm` (16), `heading` (20), `heading-lg` (28). The masthead binds
// `heading-lg`, every card title and every stat number binds `heading`, and
// everything else binds `body` — muted and set in capitals when it labels
// something else, plain and primary when it is the thing itself. That is
// five distinguishable roles out of two sizes and two tones.
//
// All five reach the screen. This paragraph used to say the opposite -- that
// the renderer's typography map was keyed on pre-vocabulary names, so every
// run drew at 14 units and the size half of the hierarchy was invisible --
// and told the reader to distrust any capture of this page on that basis.
// Measured on a real 1200x900 capture (2026-08-25), the masthead's ink spans
// 28 rows, a card title 15, a stat number 14 and a caption 12: `heading-lg`,
// `heading` and `body` are all distinct on screen. Whether the claim was
// always wrong or was fixed and the comment left behind is not recoverable
// from the file; either way a comment telling a reader to disbelieve a
// correct capture is worse than no comment, because it retires a real
// observation before it can be made.
//
// What *is* still true is narrower and belongs to the component library, not
// the renderer: `typography.heading-sm` (14, `Medium` since the Carbon ramp
// landed on 2026-08-25 — it was 16 `Bold`) is reachable from no component
// at all, and `heading-lg` only by building a raw `Props` by hand, which the
// masthead below does. Ten declared steps, two with a public function.
//
// **Colour is never the only channel (FR-015).** The three fiber states go
// through `status`, which carries the shape and the words beside the hue. The
// one library control distinguished only by fill — a selected tab — also
// declares `Semantics.selected`. The disabled button is un-filled *and*
// strips its interactions *and* says so in a caption. The popup's dangerous
// item says so in words instead of being painted red. The sparkline is drawn
// in ink, not in a status hue: nothing about it means anything by colour.

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
    /// The ten tiles the stat grid draws, in reading order: a caps label and
    /// the value under it.
    ///
    /// Ten short strings rather than one long sentence, because the sentence
    /// this replaced ran to about 150 characters and at `typography.body`
    /// that is wider than the page. It truncated — and Petra truncates
    /// honestly, so the harness's own readout was the first thing on the
    /// page to lose its tail. A stat grid wraps by construction.
    fn tiles(&self) -> [(&'static str, String); 10] {
        [
            ("placements", self.placements.to_string()),
            ("drawn", self.drawn.to_string()),
            ("empty", self.empty.to_string()),
            ("clipped", self.clipped.to_string()),
            ("silent", self.silent.to_string()),
            ("rings", self.focus_rings.to_string()),
            ("blind focus", self.blind_focus.to_string()),
            ("unresolved", self.unresolved.to_string()),
            ("undrawn", self.undrawn.to_string()),
            (
                "digest",
                if self.desynced { "DESYNCED" } else { "in sync" }.to_owned(),
            ),
        ]
    }
}

/// The last [`HISTORY`] placement counts, shared between the application that
/// records them and the painter that draws them.
///
/// `Rc<RefCell<_>>` because a registered painter is a `'static` closure that
/// cannot borrow the application, and egui runs this whole loop on one
/// thread. The series is real: [`App::view`] pushes the number the
/// PLACEMENTS tile prints, once per pass, so the sparkline and the tile are
/// the same measurement at two time scales rather than two numbers that
/// could disagree.
#[derive(Clone, Default)]
struct History(Rc<RefCell<VecDeque<f32>>>);

impl History {
    fn push(&self, value: f32) {
        let mut samples = self.0.borrow_mut();
        if samples.len() == HISTORY {
            samples.pop_front();
        }
        samples.push_back(value);
    }

    fn samples(&self) -> Vec<f32> {
        self.0.borrow().iter().copied().collect()
    }
}

/// How long a live toast stays up without hover.
const TOAST_HOLD: Duration = Duration::from_secs(4);
/// Extra time after the pointer last touched the toast.
const TOAST_LINGER: Duration = Duration::from_millis(2000);
const TOAST_SLIDE: Duration = Duration::from_millis(180);
const TOAST_FADE: Duration = Duration::from_millis(400);

/// One desktop-style notice. Click may be a no-op (`action` is `None`).
struct Notice {
    id: u64,
    title: String,
    body: String,
    /// `None` means the click only dismisses.
    action: Option<&'static str>,
    shown: Instant,
    deadline: Instant,
    hovered: bool,
}

/// A dismissed notice, kept for audit (A-07).
#[allow(dead_code)] // read by operators and a later inspector, not by this harness
struct NoticeRecord {
    id: u64,
    title: String,
    dismissed: &'static str,
}

enum ToastMode {
    Off,
    /// Times out. The Toast button on the page.
    Live(Notice),
    /// Capture / `PETRA_GALLERY_OPEN=toast`. No ambient, no clock.
    Pinned,
}

struct Gallery {
    counters: Counters,
    /// What the last routed event did, echoed on screen so the input path is
    /// observable without a debugger.
    last_event: String,
    modal: bool,
    menu: bool,
    toast: ToastMode,
    next_notice: u64,
    notice_log: Vec<NoticeRecord>,
    last_viewport: (f32, f32),
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
    /// The two independent checkboxes.
    stopped: bool,
    verbose: bool,
    /// Which restart policy the radio group has chosen: an index into
    /// [`POLICIES`].
    policy: usize,
    /// The toggle.
    tracing: bool,
    /// How full the progress bar is, 0..=1.
    progress: f32,
    /// Which row of the virtualized list is selected.
    selected_row: Option<usize>,
    /// The series the registered sparkline painter draws.
    history: History,
    /// Passes the last flying-caret hop took, copied off the host after
    /// each pass so the telemetry band can show it.
    last_hop: u32,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            counters: Counters::default(),
            last_event: String::new(),
            modal: false,
            menu: false,
            toast: ToastMode::Off,
            next_notice: 1,
            notice_log: Vec::new(),
            last_viewport: (1200.0, 1800.0),
            dismissals: 0,
            probe: None,
            tab: 0,
            stopped: true,
            verbose: false,
            policy: 1,
            tracing: true,
            // Not zero and not one: a bar pinned to either end demonstrates
            // nothing about how the two weighted tracks split.
            progress: 0.62,
            selected_row: Some(3),
            history: History::default(),
            last_hop: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

impl Gallery {
    /// Title, and one line saying what this window is for.
    fn masthead() -> ViewNode {
        let mut title = Props {
            text: Some("Petra component gallery".to_owned()),
            style: Some(tok("typography.heading-lg")),
            ..Props::default()
        };
        title
            .tokens
            .insert("foreground".into(), tok("text.primary"));
        column(
            "masthead",
            sp("spacing.2xs"),
            vec![
                ViewNode::new(NodeKind::Text, "title").with_props(title),
                note(
                    "tagline",
                    "Every component the library ships, over a live host, beside the \
                     counters that say whether the engine agrees it drew them.",
                ),
            ],
        )
    }

    /// The honesty band: ten counters as stat tiles, the last routed event,
    /// and the sparkline of the placement count over recent passes.
    fn telemetry(&self) -> ViewNode {
        let tiles = self.counters.tiles();
        let mut stats = ViewNode::new(NodeKind::Grid, "stats").with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }; 5],
            column_spacing: sp("spacing.lg"),
            row_spacing: sp("spacing.md"),
            ..Props::default()
        });
        for (label, value) in &tiles {
            let key = label.replace(' ', "-");
            stats = stats.child(column(
                &key,
                sp("spacing.2xs"),
                vec![
                    caption("label", label),
                    // `heading` rather than a bespoke Text: a stat's number
                    // is the one place on this page where a 20-unit run is
                    // not a title, and reusing the component keeps it the
                    // same type step as one.
                    heading("value", value.as_str()),
                ],
            ));
        }

        let left = column(
            "readout",
            sp("spacing.md"),
            vec![
                caption(
                    "heading",
                    &format!("frame {} · previous pass", self.counters.seq),
                ),
                stats,
                note(
                    "last",
                    &format!(
                        "last event: {}   ·   last hop: {} frames   ·   dismissals reported: {}",
                        if self.last_event.is_empty() {
                            "none yet — click a control or press Tab"
                        } else {
                            &self.last_event
                        },
                        self.last_hop,
                        self.dismissals
                    ),
                ),
            ],
        );

        let right = column(
            "trend",
            sp("spacing.2xs"),
            vec![
                caption("label", "placements per pass"),
                Self::sparkline(),
                muted("scale", &format!("last {HISTORY} passes")),
            ],
        );

        card(
            ViewNode::new(NodeKind::Grid, "telemetry")
                .with_props(Props {
                    columns: vec![TrackSize::Weight { weight: 1.0 }, TrackSize::FitContent],
                    column_spacing: sp("spacing.xl"),
                    ..Props::default()
                })
                .child(left)
                .child(right),
        )
    }

    /// The custom node the registered painter draws into.
    ///
    /// A well on `surface.base` inside a `surface.raised` card, so the plot
    /// area is a hole in the card rather than a box on top of it. The fill
    /// and the corner are this file's; everything inside the rect is the
    /// painter's, and Petra never sees it — which is what makes the frame
    /// *hosted* (FR-060).
    ///
    /// Deliberately **not** run through [`on_layer`]: that function only
    /// knows how to raise a node onto its ground, so it maps `surface.base`
    /// to the *ground's* tone (here, `LAYER_TOKENS[1]` — the card's own
    /// fill, which would erase the hole). A sunken well wants the opposite —
    /// one step *back*, toward the page — and `LAYER_TOKENS[0]` is exactly
    /// what `surface.base` already names, so the direct binding is correct
    /// as written and no border is needed: the fill against the card's fill
    /// is the edge.
    fn sparkline() -> ViewNode {
        let mut props = Props {
            custom_kind: Some(CUSTOM_KIND.to_owned()),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), tok("surface.base"));
        props.tokens.insert("radius".into(), tok("shape.corner-sm"));
        ViewNode::new(NodeKind::Custom, "sparkline")
            .with_props(props)
            .with_constraints(Constraints {
                horizontal: AxisConstraint {
                    min: Some(240.0),
                    max: Some(240.0),
                    priority: 0,
                },
                vertical: AxisConstraint {
                    min: Some(56.0),
                    max: Some(56.0),
                    priority: 0,
                },
            })
    }

    /// Buttons, the three binary controls, the tab strip and a progress bar:
    /// nine of the thirteen component names, in one card.
    fn controls_card(&self) -> ViewNode {
        let buttons = row(
            "buttons",
            sp("spacing.md"),
            Align::Center,
            vec![
                // R5: the one accent on this page. Save is the fibers form's
                // single loudest action — the control a reader should find
                // first — filled with `accent.primary` instead of a grey
                // step. See `primary_button`'s rustdoc for why an outline
                // was the wrong way to say that.
                primary_button("save", "Save"),
                // Measured on the first capture of this rewrite, before the
                // library had an answer for it: a raised control on a
                // raised card has no edge at all. Save and Cancel drew
                // nothing but their focus ring, every Store row read as
                // selected, and the selected tab read as the unselected
                // one. `on_layer` (`gorgon_petra::component`) is the
                // library's fix now — depth 1, because a control on this
                // card sits on `LAYER_TOKENS[1]`.
                on_layer(button("cancel", "Cancel"), 1),
                disabled_button("retire", "Retire", 1),
            ],
        );

        let checks = row(
            "checks",
            sp("spacing.md"),
            Align::Center,
            vec![
                checkbox("stopped", "Include stopped", self.stopped),
                checkbox("verbose", "Verbose trace", self.verbose),
            ],
        );

        let policies = row(
            "policies",
            sp("spacing.md"),
            Align::Center,
            POLICIES
                .iter()
                .enumerate()
                .map(|(i, (key, label))| radio(*key, *label, i == self.policy))
                .collect(),
        );

        let tabs = column(
            "tabs",
            sp("spacing.sm"),
            vec![
                tab_bar(
                    "tablist",
                    vec![
                        on_layer(tab("tab-fibers", "Fibers", self.tab == 0), 1),
                        on_layer(tab("tab-trace", "Trace", self.tab == 1), 1),
                        on_layer(tab("tab-caps", "Capabilities", self.tab == 2), 1),
                    ],
                ),
                rule("tab-rule"),
            ],
        );

        let done = self.progress.clamp(0.0, 1.0);
        let bar = row(
            "rebuild",
            sp("spacing.md"),
            Align::Center,
            vec![
                caption("label", "rebuild"),
                progress("bar", "Rebuild", done)
                    .with_constraints(at_least(Axis::Horizontal, 140.0)),
                text("value", format!("{:.0}%", done * 100.0)),
            ],
        );

        section(
            "controls",
            "Controls",
            vec![body(
                "body",
                sp("spacing.md"),
                vec![
                    buttons,
                    caption(
                        "buttons-note",
                        "the third is disabled: no interactions, no fill",
                    ),
                    rule("rule-a"),
                    checks,
                    row(
                        "policy-row",
                        sp("spacing.md"),
                        Align::Center,
                        vec![caption("label", "restart"), policies],
                    ),
                    toggle("tracing", "Trace this tree", self.tracing),
                    rule("rule-b"),
                    tabs,
                    bar,
                ],
            )],
        )
    }

    /// A two-column form: a caption beside a field.
    fn form_card() -> ViewNode {
        let grid = ViewNode::new(NodeKind::Grid, "form")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 120.0 },
                    TrackSize::Weight { weight: 1.0 },
                ],
                column_spacing: sp("spacing.md"),
                row_spacing: sp("spacing.sm"),
                // A field's natural width is its placeholder's width, so
                // without this the two boxes are as wide as the words in
                // them and the form has a ragged right edge. `Stretch` is
                // the declaration that says "fill the track", and it is a
                // layout answer rather than a width guessed per field.
                align: Some(Align::Stretch),
                ..Props::default()
            })
            // Stretch fills the row to `size-md`. A bare caption paints at
            // `rect.min`, so the words sat on the top of the 40-unit cell
            // while the field text sat on the midline. A one-child Center
            // row keeps Stretch (the field still fills the track) and
            // seats the caption on that same midline.
            .child(row(
                "l-name",
                None,
                Align::Center,
                vec![caption("l-name-text", "fiber name")],
            ))
            .child(field("f-name", "supervisor/root"))
            .child(row(
                "l-cap",
                None,
                Align::Center,
                vec![caption("l-cap-text", "capability")],
            ))
            .child(field("f-cap", "fs.read"));

        section(
            "form",
            "Form",
            vec![body("body", sp("spacing.md"), vec![grid])],
        )
    }

    /// Wrapping, elision, and mixed scripts.
    /// The three surface input policies, each openable and each behaving
    /// differently on a click outside it.
    fn surface_card() -> ViewNode {
        section(
            "overlays",
            "Overlays",
            vec![body(
                "body",
                sp("spacing.md"),
                vec![
                    note(
                        "note",
                        "Block swallows a click outside it · DismissOutside lets \
                         it through and asks this application to close · Toast \
                         is a desktop notice: top-right, times out, hover holds.",
                    ),
                    row(
                        "open",
                        sp("spacing.md"),
                        Align::Center,
                        vec![
                            on_layer(button("open-modal", "Block modal"), 1),
                            on_layer(button("open-menu", "Dismiss popup"), 1),
                            on_layer(button("open-pass", "Toast"), 1),
                        ],
                    ),
                ],
            )],
        )
    }

    /// The two hosted kinds described side by side: one drawn through a
    /// registered painter, one deliberately left undrawn.
    ///
    /// This is the honest half of FR-059 and FR-060 in one card. Both nodes
    /// declare content Petra never sees the pixels of; the sparkline has a
    /// painter and is counted in `PaintReport::customs`, the image has no
    /// source and lands in `PaintReport::undrawn` under its own name, so an
    /// operator learns *which* picture is missing rather than that "an image"
    /// is.
    fn hosted_card() -> ViewNode {
        let mut image = Props {
            image: Some(IMAGE_SOURCE.to_owned()),
            ..Props::default()
        };
        // Same sunken-well tone as `sparkline`, not run through `on_layer`
        // for the same reason — see that function's rustdoc. The fill
        // against the card's fill is the edge, so (R2, R3) the
        // `text.muted` outline this used to carry is gone: it named a text
        // tone as a border and did work the fill was already doing.
        image
            .tokens
            .insert("background".into(), tok("surface.base"));
        image.tokens.insert("radius".into(), tok("shape.corner-sm"));

        // A `FitContent` well beside a `Weight` caption, rather than two
        // equal columns: the well is 96 units wide and a half-width column
        // would strand it against a gap the size of the caption beside it.
        let pair = ViewNode::new(NodeKind::Grid, "pair")
            .with_props(Props {
                columns: vec![TrackSize::FitContent, TrackSize::Weight { weight: 1.0 }],
                column_spacing: sp("spacing.md"),
                row_spacing: sp("spacing.sm"),
                ..Props::default()
            })
            .child(
                ViewNode::new(NodeKind::Image, "logo")
                    .with_props(image)
                    .with_constraints(Constraints {
                        horizontal: AxisConstraint {
                            min: Some(96.0),
                            max: Some(96.0),
                            priority: 0,
                        },
                        vertical: AxisConstraint {
                            min: Some(48.0),
                            max: Some(48.0),
                            priority: 0,
                        },
                    }),
            )
            .child(column(
                "logo-caption",
                sp("spacing.2xs"),
                vec![
                    caption("label", "image · no source"),
                    note(
                        "detail",
                        "no loader is registered for it, so the paint pass names it in \
                         the undrawn set and the UNDRAWN tile above reads 1",
                    ),
                ],
            ))
            .child(Self::sparkline_swatch())
            .child(column(
                "spark-caption",
                sp("spacing.2xs"),
                vec![
                    caption("label", "custom · sparkline"),
                    note(
                        "detail",
                        "a registered painter draws it, in Petra's own paint order, at \
                         the top of this page",
                    ),
                ],
            ));

        section(
            "hosted",
            "Hosted content",
            vec![body(
                "body",
                sp("spacing.md"),
                vec![
                    note(
                        "note",
                        "Petra places and clips both of these; their pixels come from \
                         the host and the frame digest cannot see either, which is what \
                         makes this frame hosted.",
                    ),
                    pair,
                ],
            )],
        )
    }

    /// A second, smaller sparkline, so the card that explains the escape
    /// hatch shows the thing it is explaining.
    ///
    /// The same registered painter draws it — one name, one painter, two
    /// placements — which is the cheapest available proof that the dispatch
    /// is keyed on the custom name rather than on the node.
    ///
    /// Same sunken-well tone, and the same reason it is not run through
    /// `on_layer`, as [`Self::sparkline`].
    fn sparkline_swatch() -> ViewNode {
        let mut props = Props {
            custom_kind: Some(CUSTOM_KIND.to_owned()),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), tok("surface.base"));
        props.tokens.insert("radius".into(), tok("shape.corner-sm"));
        ViewNode::new(NodeKind::Custom, "spark")
            .with_props(props)
            .with_constraints(Constraints {
                horizontal: AxisConstraint {
                    min: Some(96.0),
                    max: Some(96.0),
                    priority: 0,
                },
                vertical: AxisConstraint {
                    min: Some(48.0),
                    max: Some(48.0),
                    priority: 0,
                },
            })
    }

    /// Every open surface, not one of them. Overlay children share the
    /// viewport; z-order is the `Layer` on each surface. Returning a single
    /// child made the toast vanish whenever a modal or menu opened.
    fn overlay_surfaces(&mut self) -> Vec<ViewNode> {
        let mut surfaces = Vec::new();
        if let Some(toast) = self.toast_overlay() {
            surfaces.push(toast);
        }
        if self.menu {
            surfaces.push(self.menu_surface());
        }
        if self.modal {
            surfaces.push(modal_surface(
                "Retire fiber",
                "Retire worker/indexer?",
                "Its three children are retired with it. A click \
                 outside this dialog is swallowed — that is what Block \
                 means.",
                vec![
                    on_layer(button("modal-close", "Cancel"), 1),
                    on_layer(button("modal-confirm", "Retire"), 1),
                ],
            ));
        }
        surfaces
    }

    fn menu_surface(&self) -> ViewNode {
        surface("menu", Layer::Popup, InputPolicy::DismissOutside)
            .with_semantics(Semantics {
                role: Some(Role::List),
                label: Some("Fiber actions".to_owned()),
                ..Semantics::default()
            })
            .child(column(
                "body",
                sp("spacing.2xs"),
                vec![
                    caption("title", "fiber actions"),
                    on_layer(list_row("menu-inspect", "Inspect", false), 1),
                    on_layer(list_row("menu-trace", "Follow trace", false), 1),
                    rule("rule"),
                    on_layer(list_row("menu-kill", "Kill — cannot be undone", false), 1),
                ],
            ))
    }

    fn spawn_toast(&mut self, title: &str, body: &str, action: Option<&'static str>) {
        let now = Instant::now();
        let id = self.next_notice;
        self.next_notice += 1;
        self.toast = ToastMode::Live(Notice {
            id,
            title: title.to_owned(),
            body: body.to_owned(),
            action,
            shown: now,
            deadline: now + TOAST_HOLD,
            hovered: false,
        });
    }

    fn archive_toast(&mut self, reason: &'static str) {
        if let ToastMode::Live(notice) = &self.toast {
            self.notice_log.push(NoticeRecord {
                id: notice.id,
                title: notice.title.clone(),
                dismissed: reason,
            });
        }
        self.toast = ToastMode::Off;
    }

    fn toast_overlay(&mut self) -> Option<ViewNode> {
        if let ToastMode::Live(notice) = &self.toast {
            let now = Instant::now();
            if !notice.hovered && now >= notice.deadline + TOAST_FADE {
                self.archive_toast("timeout");
            }
        }
        match &self.toast {
            ToastMode::Off => None,
            ToastMode::Pinned => {
                Some(self.toast_surface(1.0, self.last_viewport.0 - 16.0, 16.0, false))
            }
            ToastMode::Live(notice) => {
                let now = Instant::now();
                let age = now.saturating_duration_since(notice.shown);
                let slide_t = (age.as_secs_f32() / TOAST_SLIDE.as_secs_f32()).clamp(0.0, 1.0);
                let y = 16.0 - 24.0 * (1.0 - slide_t);
                let fading = !notice.hovered && now >= notice.deadline;
                let fade_t = if fading {
                    let into = now.saturating_duration_since(notice.deadline);
                    (into.as_secs_f32() / TOAST_FADE.as_secs_f32()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let opacity = slide_t * (1.0 - fade_t);
                let x = self.last_viewport.0 - 16.0;
                Some(self.toast_surface(opacity, x, y, true))
            }
        }
    }

    fn toast_surface(&self, opacity: f32, x: f32, y: f32, ambient: bool) -> ViewNode {
        let (title, body) = match &self.toast {
            ToastMode::Live(n) => (n.title.as_str(), n.body.as_str()),
            ToastMode::Pinned => (
                "Supervisor restarted",
                "Passthrough: clicks reach what is underneath.",
            ),
            ToastMode::Off => ("", ""),
        };
        let mut surface = surface("toast", Layer::Toast, InputPolicy::Passthrough);
        surface.props.anchor = Some(Anchor::Point { x, y });
        surface.props.clamp = Some(ClampRule::Flip);
        surface.props.opacity = Some(opacity.clamp(0.0, 1.0));
        if ambient {
            surface = surface.with_ambient(true);
        }
        let log_n = self.notice_log.len();
        surface.child(column(
            "body",
            sp("spacing.md"),
            vec![
                heading("title", title),
                note("note", body),
                muted("log", &format!("audit queue: {log_n}")),
                on_layer(button("toast-body", "Open"), 1),
            ],
        ))
    }
}

impl RowSource for Gallery {
    fn rows(&mut self, source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        window_rows(source, ROW_SOURCE, range, self.selected_row)
    }
}

impl App for Gallery {
    fn view(&mut self) -> ViewNode {
        // Recorded before the tree is built, so the series the sparkline
        // draws is exactly the series the PLACEMENTS tile prints — one sample
        // per pass, never two numbers that could disagree. It also means the
        // painter is never handed an empty series at paint time.
        #[allow(clippy::cast_precision_loss)]
        self.history.push(self.counters.placements as f32);

        let columns = two_column_grid(
            vec![
                self.controls_card(),
                Self::form_card(),
                text_card(vec![
                    text("scripts", "mixed scripts: Ünïcödé · 日本語 · العربية"),
                    caption(
                        "scripts-note",
                        "the shipped font set has no CJK or Arabic · the boxes are \
                         the fallback glyph, not a layout fault",
                    ),
                ]),
                layout_card(),
            ],
            vec![
                // First, and not by accident: this card holds the one
                // node on the page with no painter, and the UNDRAWN
                // tile in the band directly above it is the counter
                // that reports it. A card that explains a counter
                // belongs next to the counter — and a hosted node
                // scrolled below the fold is clipped away before the
                // paint pass can report it at all, which would make
                // that tile read 0 for a reason that has nothing to do
                // with painters.
                Self::hosted_card(),
                status_card(
                    &FIBERS,
                    "Colour is never the only channel: every state carries its own \
                     word and its own silhouette — OK is a disc, Degraded a \
                     triangle, Down an octagon — because `status` takes a whole \
                     StatusToken and there is no way to hand it a hue on its own.",
                ),
                collection_card(
                    ROW_SOURCE,
                    TOTAL_ROWS,
                    28.0,
                    224.0,
                    "100 000 rows declared · only the visible window is placed",
                ),
                Self::surface_card(),
            ],
        );

        let mut root = page_root(Self::masthead(), self.telemetry(), columns);
        if let Some(probe) = self.probe.clone() {
            root = root.child(probe);
        }
        overlay_shell(scrolled_page(root), self.overlay_surfaces())
    }

    fn handle(&mut self, event: &InputEvent, route: &Route, _frame: Option<&PetrifiedFrame>) {
        let node = match route {
            Route::Pointer { node } | Route::Keyboard { node } | Route::Raw { node } => {
                node.clone()
            }
            // This example's `Host` routes through `route_with_surfaces`,
            // which never produces `Reserved` — the shell alone decides that
            // (`route_with_reserved`, spec 010's interpretation order, step
            // 1) and would resolve it before this example ever sees the
            // event. Named explicitly rather than folded into `Unrouted` so
            // a future wiring change cannot start matching this arm
            // silently.
            Route::Reserved { chord } => {
                self.last_event = format!("reserved ({chord})");
                return;
            }
            Route::Unrouted { reason } => {
                self.last_event = format!("unrouted ({reason})");
                return;
            }
        };
        if let ToastMode::Live(notice) = &mut self.toast {
            let on_toast = node.contains("/toast");
            notice.hovered = on_toast;
            if on_toast {
                notice.deadline = Instant::now() + TOAST_LINGER;
            }
        }
        if !activated(event) {
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
            "open-pass" => self.spawn_toast(
                "Supervisor restarted",
                "worker-00003 came back. Click Open or wait.",
                Some("opened the fiber log"),
            ),
            "modal-close" | "modal-confirm" => self.modal = false,
            "toast-body" => {
                if let ToastMode::Live(notice) = &self.toast
                    && let Some(action) = notice.action
                {
                    self.last_event = action.to_owned();
                }
                self.archive_toast("click");
            }
            "toast-close" => self.archive_toast("close"),
            "menu-inspect" | "menu-trace" | "menu-kill" => self.menu = false,
            "tab-fibers" => self.tab = 0,
            "tab-trace" => self.tab = 1,
            "tab-caps" => self.tab = 2,
            "stopped" => self.stopped = !self.stopped,
            "verbose" => self.verbose = !self.verbose,
            "tracing" => self.tracing = !self.tracing,
            _ => {
                if let Some(i) = POLICIES.iter().position(|(key, _)| *key == tail) {
                    self.policy = i;
                } else if let Some(i) = tail.strip_prefix("row-").and_then(|n| n.parse().ok()) {
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
        // This gallery rebuilds its whole tree every pass; see
        // [`support::whole_tree_changes`] for why `All` is the only honest
        // answer.
        whole_tree_changes()
    }
}

// ---------------------------------------------------------------------------
// The registered sparkline painter (T087, FR-059)
// ---------------------------------------------------------------------------

/// Draw `samples` as a sparkline inside `ctx.rect`.
///
/// Returns whether it drew anything, which is the whole contract: a
/// registered painter that draws nothing must return `false` so its name
/// still lands in `PaintReport::undrawn`, rather than the frame reading
/// complete over a placement nobody painted. Three ways that happens here —
/// an empty series, a theme with no ink colour, and a rect too small to plot
/// in — and all three are real rather than defensive: [`App::view`] pushes a
/// sample every pass, so an empty series means the painter ran without the
/// application behind it.
///
/// Every colour *and every gap* comes from `ctx.tokens`, the same
/// [`gorgon_petra_egui::paint::TokenSource`] the rest of the frame painted
/// with, so the sparkline follows the theme instead of carrying a palette
/// and a spacing ramp of its own. Nothing here means anything by hue: the
/// line is ink, and the number it plots is printed beside it in the
/// PLACEMENTS tile.
fn paint_sparkline(painter: &egui::Painter, ctx: &CustomPaintCtx<'_>, samples: &[f32]) -> bool {
    if samples.is_empty() {
        return false;
    }
    // The plotted line is ink; the baseline under it is a boundary, so the
    // two take different tones. The baseline read `text.muted` until
    // 2026-08-25, which drew the well's axis at the same weight as the
    // series plotted above it.
    let (Some(ink), Some(rule)) = (
        ctx.tokens.color("text.primary"),
        ctx.tokens.color("border.subtle"),
    ) else {
        return false;
    };
    // The well's inset and the head marker sit on the theme's spacing ramp,
    // not on numbers this function picked. A theme that does not carry the
    // ramp cannot be plotted on it, and saying so — `false`, so the name
    // lands in `undrawn` — is the honest answer, the same one an absent ink
    // colour gets above.
    let (Some(inset), Some(marker)) = (
        ctx.tokens.spacing("spacing.xs"),
        ctx.tokens.spacing("spacing.2xs"),
    ) else {
        return false;
    };

    // One device pixel, whatever the window's scale is. `ctx.scale` exists
    // for exactly this: a hairline declared in logical units is two pixels on
    // a 2x display, which reads as a border rather than as a baseline.
    let hairline = (1.0 / ctx.scale.factor()).max(0.5);
    let plot = ctx.rect.shrink(inset);
    if plot.width() <= 0.0 || plot.height() <= 0.0 {
        return false;
    }

    painter.line_segment(
        [plot.left_bottom(), plot.right_bottom()],
        egui::Stroke::new(hairline, rule),
    );

    let lo = samples.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = samples.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    // A flat series still has to plot somewhere, and dividing by its own zero
    // range would put it at NaN. One is the smallest span that keeps a
    // constant line on the baseline instead of off the top of the well.
    let span = (hi - lo).max(1.0);
    #[allow(clippy::cast_precision_loss)]
    let last = samples.len().saturating_sub(1) as f32;
    let at = |i: usize, v: f32| {
        #[allow(clippy::cast_precision_loss)]
        let x = if last > 0.0 {
            plot.left() + plot.width() * (i as f32 / last)
        } else {
            plot.right()
        };
        egui::pos2(x, plot.bottom() - plot.height() * ((v - lo) / span))
    };

    let points: Vec<egui::Pos2> = samples.iter().enumerate().map(|(i, v)| at(i, *v)).collect();
    // Read before the vector is moved into the shape below.
    let head = points.last().copied();
    if points.len() > 1 {
        painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, ink)));
    }
    // The head of the series, marked, so a one-sample history is still a
    // drawn sparkline rather than an empty well that claims to have drawn.
    if let Some(head) = head {
        painter.circle_filled(head, marker, ink);
    }
    true
}

/// Register the sparkline painter over `history`.
///
/// A function rather than an inline closure at both call sites, because the
/// window and the test suite must register the *same* painter over the *same*
/// series — a test that registered a simpler one would prove that some
/// painter draws, not that this page's does.
fn register_sparkline(painters: &mut CustomPainters, history: History) {
    painters.register(CUSTOM_KIND, move |painter, ctx| {
        paint_sparkline(painter, ctx, &history.samples())
    });
}

/// A host over a fresh gallery, with the custom kind registered on the tree
/// registry and the painter registered on the paint dispatch.
///
/// One function, used by `main` and by every test below, so the window and
/// the suite cannot drift into registering different things.
fn build_host(ctx: &egui::Context, presenter: Presenter) -> Host<Gallery> {
    let app = Gallery::default();
    let history = app.history.clone();
    let mut host = Host::new(ctx, app, presenter);
    host.registry_mut().register_custom_kind(CUSTOM_KIND);
    register_sparkline(host.painters_mut(), history);
    host
}

/// Open one overlay at startup, from `PETRA_GALLERY_OPEN`.
///
/// The three surfaces are the only part of this page a capture cannot
/// otherwise show: they are opened by a click, and a run that screenshots
/// itself never clicks anything. Anything unrecognised opens nothing, which
/// is the ordinary state.
fn open_from_env(app: &mut Gallery) {
    match std::env::var("PETRA_GALLERY_OPEN").as_deref() {
        Ok("modal") => app.modal = true,
        Ok("menu") => app.menu = true,
        Ok("toast") => app.toast = ToastMode::Pinned,
        _ => {}
    }
}

/// Which shipped theme to present, from `PETRA_GALLERY_THEME`.
///
/// Both themes are shipped and both must render this page, so the window can
/// show either. Anything but `light` is dark, which is the default the rest
/// of the crate uses (`host::default_presenter`).
fn presenter_from_env() -> Presenter {
    let theme = match std::env::var("PETRA_GALLERY_THEME").as_deref() {
        Ok("light") => light(),
        _ => dark(),
    };
    Presenter::new(colors_from_env(coverage_from_env(theme)))
}

/// Override any colour token from `PETRA_GALLERY_COLOR`, so a tone can be
/// judged in place instead of guessed at.
///
/// Syntax is a comma-separated list of `token=#rrggbb` or `token=#rrggbbaa`:
///
/// ```text
/// PETRA_GALLERY_COLOR="text.muted=#c6c6c6,surface.layer-one=#1e1e1e"
/// ```
///
/// **Deliberately general, rather than one knob per token.** `text.muted`
/// is the tone being tuned today; a `PETRA_GALLERY_MUTED` would have been
/// shorter to write and would have been followed by `PETRA_GALLERY_PRIMARY`
/// the next time, and by a drawer of dead one-offs the time after. One
/// parser that takes any declared colour name costs the same and answers
/// every future version of this question.
///
/// Nothing in `src/` reads this. It bends only the copy this one window
/// presents, and a malformed value panics rather than quietly showing the
/// shipped colour under a label claiming otherwise — a capture that lies
/// about its own settings is worse than no capture.
fn colors_from_env(theme: Theme) -> Theme {
    let Ok(spec) = std::env::var("PETRA_GALLERY_COLOR") else {
        return theme;
    };
    let mode = theme.mode();
    let mut values = theme.values().clone();
    for pair in spec.split(',').filter(|p| !p.trim().is_empty()) {
        let (token, hex) = pair
            .split_once('=')
            .unwrap_or_else(|| panic!("PETRA_GALLERY_COLOR={spec}: {pair:?} is not token=#rrggbb"));
        let name = TokenName::new(token.trim())
            .unwrap_or_else(|err| panic!("PETRA_GALLERY_COLOR={spec}: {token:?}: {err}"));
        assert!(
            values.contains_key(&name),
            "PETRA_GALLERY_COLOR={spec}: {token:?} is not a token this theme assigns"
        );
        values.insert(name, TokenValue::Color(parse_hex(hex.trim(), &spec)));
    }
    Theme::build(mode, &standard_vocabulary(), values)
        .expect("only declared colour tokens changed, each to a usable colour")
}

/// `#rrggbb` or `#rrggbbaa` to a [`ColorValue`]. Panics with the whole
/// offending spec, because a half-parsed colour is how a tuning session ends
/// up arguing about a capture nobody can reproduce.
fn parse_hex(hex: &str, spec: &str) -> ColorValue {
    let digits = hex.strip_prefix('#').unwrap_or_else(|| {
        panic!("PETRA_GALLERY_COLOR={spec}: {hex:?} must start with '#'");
    });
    assert!(
        digits.len() == 6 || digits.len() == 8,
        "PETRA_GALLERY_COLOR={spec}: {hex:?} must be #rrggbb or #rrggbbaa"
    );
    let byte = |i: usize| {
        u8::from_str_radix(&digits[i..i + 2], 16)
            .unwrap_or_else(|err| panic!("PETRA_GALLERY_COLOR={spec}: {hex:?}: {err}"))
    };
    ColorValue::from_srgb8(
        byte(0),
        byte(2),
        byte(4),
        if digits.len() == 8 { byte(6) } else { 0xff },
    )
}

/// Override `text.coverage-curve` from `PETRA_GALLERY_COVERAGE`, so the two
/// glyph-sharpness dials can be swept without editing and rebuilding
/// `gorgon-petra`.
///
/// This is the slider. ai-macs tuned its own defaults by dragging one
/// (`text_tuning.go`), and the value that ships is a taste call made by
/// looking at real glyphs at real size, not one derived from a metric — so
/// the gallery, which is the page those glyphs are looked at on, is where the
/// dial belongs. Nothing in `src/` reads this variable: the shipped themes
/// carry the shipped value, and this only bends the copy this one window
/// presents.
///
/// Syntax is `passes[,snap]`: `3`, `3.5`, `4,snap`. `passes` is clamped to
/// the `[1.0, 4.0]` range `Theme::build` accepts. `snap` sets
/// [`gorgon_petra::token::value::CoverageValue::snap`], which the host
/// inverts into epaint's `subpixel_binning` — with it on, every glyph
/// rasterises once at an integer x instead of at one of four sub-pixel
/// phases.
///
/// An unset variable returns `theme` untouched. A malformed one panics rather
/// than silently presenting the shipped value under a label claiming
/// otherwise, which would make every capture taken with it a lie.
fn coverage_from_env(theme: Theme) -> Theme {
    let Ok(spec) = std::env::var("PETRA_GALLERY_COVERAGE") else {
        return theme;
    };
    let mut parts = spec.split(',');
    let passes: f32 = parts
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("PETRA_GALLERY_COVERAGE={spec}: passes is not a number"));
    let mut snap = false;
    for flag in parts {
        match flag.trim() {
            "snap" => snap = true,
            "nosnap" | "" => {}
            other => panic!("PETRA_GALLERY_COVERAGE={spec}: unknown flag {other:?}"),
        }
    }

    let mode = theme.mode();
    let mut values = theme.values().clone();
    values.insert(
        TokenName::new("text.coverage-curve").expect("a literal, well-formed token name"),
        TokenValue::Coverage(CoverageValue {
            passes: passes.clamp(1.0, 4.0),
            snap,
        }),
    );
    Theme::build(mode, &standard_vocabulary(), values)
        .expect("only text.coverage-curve changed, and it stayed in range")
}

// ---------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------

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

/// Write the pinned page, not the granted window.
///
/// A screenshot is of the framebuffer, so it is whatever size the window
/// manager handed out — [`GALLERY_VIEWPORT`] pins where the *page* is laid
/// out, and the margin around it is not part of the page. Cropping here is
/// what makes two captures taken on two runs hold the same pixels; the
/// `petra-parity` lane does exactly this and for exactly this reason.
///
/// A window smaller than the pin is refused rather than padded or clamped: a
/// short capture would silently drop the bottom of the page, and a
/// measurement taken from it would be a measurement of a different page that
/// looked like a valid one.
fn write_ppm(
    path: &std::path::Path,
    image: &egui::ColorImage,
    viewport: (f32, f32),
) -> std::io::Result<()> {
    use std::io::Write as _;
    let [src_w, src_h] = image.size;
    // `viewport`, not [`GALLERY_VIEWPORT`]. Reading the constant here while
    // `PETRA_GALLERY_SIZE` moved the pin produced precisely the failure the
    // doc above describes: a run pinned to 1200x1750 wrote 1200x900 and
    // *announced 1750*, because the guard below compared the granted window
    // against the wrong number and the crop then took the wrong rows.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (w, h) = (viewport.0 as usize, viewport.1 as usize);
    if src_w < w || src_h < h {
        return Err(std::io::Error::other(format!(
            "the window manager granted {src_w}x{src_h}, which is smaller than the \
             pinned {w}x{h} page; the capture would be missing part of the page"
        )));
    }
    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(out, "P6\n{w} {h}\n255\n")?;
    for row in 0..h {
        for col in 0..w {
            let [r, g, b, _a] = image.pixels[row * src_w + col].to_srgba_unmultiplied();
            out.write_all(&[r, g, b])?;
        }
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
    /// The pinned surface for this run: [`GALLERY_VIEWPORT`], or whatever
    /// [`viewport_from_env`] resolved. Held on the window rather than read
    /// per frame, so one run cannot change size underneath itself.
    viewport: (f32, f32),
    /// Capture runs pin `screen_rect` so two PPMs are the same page.
    /// Interactive runs do not: pinning 1200×900 inside a taller window
    /// truncated the page and left a black band, which is what the operator
    /// could not scroll past.
    pin: bool,
    /// Interactive runs ask the compositor for keyboard focus once, so Tab
    /// is not swallowed by the terminal that launched the window.
    grabbed_focus: bool,
}

/// The layout surface every capture of this page is taken at.
///
/// A window manager is free to ignore `with_inner_size`, and a tiling one
/// always does: the same `PETRA_GALLERY_SHOT` invocation produced 2009x1392
/// on one run and 1401x1392 on the next. Petra's layout is viewport-relative,
/// so those two captures hold *different pages* — different wraps, different
/// track widths — and a measurement taken from one cannot be compared with a
/// measurement taken from the other. It broke a real run: the ink pixel count
/// in a fixed band nearly doubled between two arms of an experiment that
/// changed one rasteriser flag, because the band was sampling different text.
///
/// So the surface is pinned here rather than requested, which is the same fix
/// and for the same reason as `examples/parity.rs`'s own
/// `PARITY_VIEWPORT` — see that constant for the fuller argument, and for why
/// `safe_area_insets` has to be zeroed alongside it.
const GALLERY_VIEWPORT: (f32, f32) = (1200.0, 900.0);

/// Inner size an interactive window *asks* for. Not a layout pin: the host
/// lays out at whatever the compositor grants. 1800 is tall enough that the
/// page of cards is on screen without relying on wheel-scroll to find a field.
const GALLERY_INTERACTIVE: (f32, f32) = (1200.0, 1800.0);

/// The pinned surface for this run: [`GALLERY_VIEWPORT`] unless
/// `PETRA_GALLERY_SIZE=WIDTHxHEIGHT` says otherwise.
///
/// **The default is the point and the override is the exception.** The pin
/// exists so two captures are comparable, and a knob that changes it can
/// destroy exactly the property the constant was added for — a measurement
/// taken at one size cannot be compared with one taken at another, because
/// the text rewraps and the grid tracks move.
///
/// It is here because 1200x900 shows the top of a longer page, and "what
/// does the whole thing look like" is a real question the file could not
/// answer. Every comparison capture still comes from the default; anything
/// taken with this set is a *picture*, not a measurement, and a run that
/// uses it says so on stderr rather than quietly producing a raster that
/// looks like all the others.
///
/// # Panics
/// On a value that is not `WIDTHxHEIGHT` in positive, finite logical units.
/// A silently-ignored size knob would be worse than no knob: the operator
/// would read the resulting capture as the size they asked for.
fn viewport_from_env() -> (f32, f32) {
    let Ok(spec) = std::env::var("PETRA_GALLERY_SIZE") else {
        return GALLERY_VIEWPORT;
    };
    let bad = || -> ! {
        panic!("PETRA_GALLERY_SIZE={spec:?} must be WIDTHxHEIGHT in logical units, e.g. 1200x1600")
    };
    let (w, h) = spec.split_once(['x', 'X']).unwrap_or_else(|| bad());
    let parse = |s: &str| -> f32 {
        let v: f32 = s.trim().parse().unwrap_or_else(|_| bad());
        if !v.is_finite() || v <= 0.0 {
            bad();
        }
        v
    };
    let size = (parse(w), parse(h));
    eprintln!(
        "gallery: PETRA_GALLERY_SIZE pins the page to {}x{} instead of the usual {}x{}. \
         This capture is a picture, not a measurement -- it cannot be compared with one \
         taken at the default size, because the text rewraps and the grid tracks move.",
        size.0, size.1, GALLERY_VIEWPORT.0, GALLERY_VIEWPORT.1
    );
    size
}

impl eframe::App for GalleryWindow {
    /// Capture runs pin the layout surface so two PPMs are comparable.
    /// Interactive runs take the window the compositor actually granted.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.safe_area_insets = Some(egui::SafeAreaInsets::default());
        if self.pin {
            raw_input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.viewport.0, self.viewport.1),
            ));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.grabbed_focus && self.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.grabbed_focus = true;
        }
        self.host.pass_in_window(&ctx, frame);

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
        // to show this pass's own numbers this pass; the label says "previous
        // pass" rather than pretending otherwise.
        self.host.app_mut().counters = counters;
        self.host.app_mut().last_hop = if self.host.caret().is_moving() {
            self.host.hop_passes()
        } else {
            self.host.last_hop_passes()
        };
        if let Some(frame) = self.host.frame() {
            self.host.app_mut().last_viewport = (frame.viewport.size.w, frame.viewport.size.h);
        }

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
            match write_ppm(&plan.path, &image, self.viewport) {
                Ok(()) => println!(
                    "gallery: wrote the pinned {}x{} page, cropped from a {}x{} window, to {}",
                    self.viewport.0 as usize,
                    self.viewport.1 as usize,
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
    let shot = std::env::var_os("PETRA_GALLERY_SHOT").map(|path| ShotPlan {
        path: std::path::PathBuf::from(path),
        passes: 0,
        requested: false,
    });
    let size_pin = std::env::var("PETRA_GALLERY_SIZE").is_ok();
    let pin = shot.is_some() || size_pin;
    let viewport = if pin {
        viewport_from_env()
    } else {
        GALLERY_INTERACTIVE
    };
    // Interactive runs ask the compositor to activate the window so Tab
    // reaches Petra instead of staying in the terminal that launched us.
    let mut builder = egui::ViewportBuilder::default().with_inner_size([viewport.0, viewport.1]);
    if shot.is_none() {
        builder = builder.with_active(true);
    }
    let mut options = eframe::NativeOptions {
        // Asked for, not relied on: for a capture, `raw_input_hook` is what
        // actually decides the layout surface. Interactive runs let the
        // compositor's window be the surface.
        viewport: builder,
        ..eframe::NativeOptions::default()
    };
    // Interactive hops need the swapchain to queue a frame, not stall on
    // latency 1 (eframe's LOW_LATENCY default). Captures keep the default
    // so a screenshot is not a different present mode than CI.
    if shot.is_none() {
        options.wgpu_options = eframe::WgpuConfiguration::default()
            .with_surface_config(eframe::SurfaceConfig::HIGH_THROUGHPUT);
    }
    eframe::run_native(
        "Petra component gallery",
        options,
        Box::new(move |cc| {
            let mut host = build_host(&cc.egui_ctx, presenter_from_env());
            open_from_env(host.app_mut());
            Ok(Box::new(GalleryWindow {
                host,
                shot,
                viewport,
                pin,
                grabbed_focus: false,
            }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{Gallery, TOTAL_ROWS, build_host, exact, row, sp};
    use egui::{Context, Event, Modifiers, Pos2, RawInput};
    use gorgon_petra::geom::{Align, Point};
    use gorgon_petra::token::{Presenter, dark, light};
    use gorgon_petra::tree::{NodeKind, Role, ViewNode};
    use gorgon_petra_egui::host::{App, Host};

    /// The window size `main` asks for.
    ///
    /// The tests run at it deliberately. An `egui::Context` with a default
    /// `RawInput` reports a screen about 10 000pt tall, and at that size
    /// nothing in this gallery ever clips, scrolls, or runs out of room — so
    /// a headless suite that takes the default is not exercising the layout a
    /// person actually sees.
    const WINDOW: [f32; 2] = [1200.0, 900.0];

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

    /// A host presenting `presenter`, with the custom kind and the sparkline
    /// painter registered exactly as `main` registers them.
    fn host_with(presenter: Presenter) -> (Context, Host<Gallery>) {
        let ctx = headless();
        let host = build_host(&ctx, presenter);
        (ctx, host)
    }

    fn host() -> (Context, Host<Gallery>) {
        host_with(Presenter::new(dark()))
    }

    /// One settled pass over a fresh gallery.
    fn settled(presenter: Presenter) -> (Context, Host<Gallery>) {
        let (ctx, mut host) = host_with(presenter);
        step(&ctx, &mut host, RawInput::default());
        (ctx, host)
    }

    fn key_press(key: egui::Key, modifiers: Modifiers) -> RawInput {
        let mut input = RawInput::default();
        input.events.push(Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });
        input
    }

    /// Tab and Shift+Tab walk the gallery's own focus tree, not egui's.
    #[test]
    fn tab_and_shift_tab_walk_the_gallery() {
        let (ctx, mut host) = settled(Presenter::new(light()));
        let first = host
            .state()
            .focused
            .clone()
            .expect("the first pass seats a focusable");
        step(&ctx, &mut host, key_press(egui::Key::Tab, Modifiers::NONE));
        let second = host
            .state()
            .focused
            .clone()
            .expect("Tab must land on a focusable");
        assert_ne!(first, second, "Tab must leave {first} for the next control");
        step(&ctx, &mut host, key_press(egui::Key::Tab, Modifiers::SHIFT));
        assert_eq!(
            host.state().focused.as_deref(),
            Some(first.as_str()),
            "Shift+Tab must walk back to {first}"
        );
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
        let mut registry = gorgon_petra::tree::Registry::with_vocabulary(
            gorgon_petra::token::standard_vocabulary(),
        );
        registry.register_custom_kind(super::CUSTOM_KIND);
        gorgon_petra::anim::shipped_registry().declare_into(&mut registry);
        if let Err(errors) = gorgon_petra::tree::validate(&tree, &registry) {
            panic!("the gallery's own tree is not acceptable: {errors}");
        }
    }

    /// T078's structural half: every role only a component sets is on the
    /// page.
    ///
    /// Asserted through the placed semantic roles rather than by grepping
    /// this file for function names, because a call that never reaches the
    /// tree proves nothing. `Role::Button` covers `button`, `checkbox`,
    /// `radio` and `toggle`; `TabList`, `Tab`, `TextInput`, `Progress`,
    /// `Status` and `ListItem` are each set by exactly one component and by
    /// nothing else in this file.
    #[test]
    fn every_component_role_reaches_the_frame() {
        let (_ctx, host) = settled(Presenter::new(dark()));
        let frame = host.frame().expect("a frame");
        for role in [
            Role::Button,
            Role::TabList,
            Role::Tab,
            Role::TextInput,
            Role::Progress,
            Role::Status,
            Role::ListItem,
            Role::Table,
        ] {
            assert!(
                frame
                    .placements
                    .iter()
                    .any(|p| p.semantics.role.as_ref() == Some(&role)),
                "no placement carries {role:?}; the page is missing the component that \
                 sets it"
            );
        }
    }

    /// SC-011: the page renders under both shipped themes with nothing
    /// unresolved and no slot the painter does not know.
    ///
    /// Two themes, one loop, because "it works in dark" is half a claim: the
    /// light theme assigns different colours to the same names, and a name
    /// only one theme declares would resolve in one and land in
    /// `unresolved_tokens` in the other.
    #[test]
    fn the_page_renders_under_both_shipped_themes() {
        for (what, presenter) in [
            ("dark", Presenter::new(dark())),
            ("light", Presenter::new(light())),
        ] {
            let (_ctx, host) = settled(presenter);
            let report = host.report().expect("a paint report");
            assert!(report.is_complete(), "{what}: {report:?}");
            assert!(!report.desynced, "{what}: {report:?}");
            assert!(
                report.unresolved_tokens.is_empty(),
                "{what}: every token this gallery binds must exist in the shipped theme: {:?}",
                report.unresolved_tokens
            );
            assert!(
                report.unknown_slots.is_empty(),
                "{what}: the gallery must not bind a slot the painter does not know: {:?}",
                report.unknown_slots
            );
            assert_eq!(
                report.blind_focus, 0,
                "{what}: a focused node with no ring is a node a keyboard user cannot \
                 find: {report:?}"
            );
            assert!(
                report.texts > 40,
                "{what}: the gallery declares far more than forty text runs; {} drawn \
                 means whole cards silently vanished",
                report.texts
            );
        }
    }

    /// T087: the sparkline is drawn by the registered painter, and is not in
    /// the undrawn set.
    #[test]
    fn the_sparkline_paints_through_the_registered_painter() {
        let (_ctx, host) = settled(Presenter::new(dark()));
        let report = host.report().expect("a paint report");
        assert_eq!(
            report.customs, 2,
            "both custom placements — the band's sparkline and the swatch in the \
             Hosted content card — must have been drawn by the registered painter: \
             {report:?}"
        );
        assert!(
            !report
                .undrawn
                .contains(&format!("custom:{}", super::CUSTOM_KIND)),
            "the sparkline has a painter and must not be reported undrawn: {report:?}"
        );
    }

    /// The inverse, so the test above cannot pass by the painter being
    /// irrelevant: with no painter registered, the same tree reports the same
    /// kind undrawn.
    ///
    /// This is the sabotage the gate asks for, kept as a test rather than as
    /// a one-off edit — it re-runs on every `cargo test` instead of being
    /// something somebody once did.
    #[test]
    fn without_the_registered_painter_the_sparkline_is_undrawn() {
        let ctx = headless();
        // Deliberately *not* `build_host`: the kind is registered on the tree
        // registry, so the tree is still accepted, and the painter is not
        // registered at all.
        let mut host = Host::new(&ctx, Gallery::default(), Presenter::new(dark()));
        host.registry_mut().register_custom_kind(super::CUSTOM_KIND);
        step(&ctx, &mut host, RawInput::default());

        let report = host.report().expect("a paint report");
        assert_eq!(report.customs, 0, "{report:?}");
        assert!(
            report
                .undrawn
                .contains(&format!("custom:{}", super::CUSTOM_KIND)),
            "an unregistered custom kind must be named in the undrawn set: {report:?}"
        );
    }

    /// FR-060's honest half: a frame carrying content Petra never sees the
    /// pixels of says so, and names which placements those are.
    #[test]
    fn the_frame_reports_itself_hosted() {
        let (_ctx, host) = settled(Presenter::new(dark()));
        let frame = host.frame().expect("a frame");
        assert!(
            frame.hosted(),
            "this page declares a custom node and an image; its digest cannot see \
             either, and the frame must say so"
        );
        let hosted: Vec<&str> = frame
            .hosted_placements()
            .map(|(p, _)| p.id.as_str())
            .collect();
        assert!(
            hosted.iter().any(|id| id.ends_with("/sparkline")),
            "the sparkline must be one of the hosted placements: {hosted:?}"
        );
        assert!(
            hosted.iter().any(|id| id.ends_with("/logo")),
            "the image must be one of the hosted placements: {hosted:?}"
        );
    }

    /// The image is the only content on this page without a painter.
    ///
    /// It is deliberately left unregistered: it is the control that proves
    /// `undrawn` still reports, at the same moment the sparkline proves the
    /// escape hatch works. T081 makes an unresolvable image name *itself*, so
    /// an operator learns which picture is missing.
    #[test]
    fn only_the_unregistered_image_lacks_a_painter() {
        let (_ctx, host) = settled(Presenter::new(dark()));
        let undrawn = &host.report().expect("a report").undrawn;
        assert_eq!(
            undrawn.len(),
            1,
            "expected exactly the unregistered image: {undrawn:?}"
        );
        assert!(
            undrawn.contains(&format!("image:{}", super::IMAGE_SOURCE)),
            "{undrawn:?}"
        );
    }

    /// Nothing on this page truncates except the one node that asks to.
    ///
    /// This is the assertion the rewrite is measured by, and it is the one no
    /// other test here makes. Petra truncates *honestly*: a row budget too
    /// tight for a padded control, a card too narrow for its own note, and a
    /// stack squeezed below its children's natural heights all surface as
    /// `PaintState::truncated` rather than as overflow. Before this rewrite
    /// the same assertion found **seventeen** truncated placements at
    /// 1100x900 — every button, every control label, every tab, and the whole
    /// text section — while all ten of the tests that shipped beside them
    /// passed.
    #[test]
    fn nothing_truncates_except_the_node_that_declares_it() {
        let (_ctx, host) = settled(Presenter::new(dark()));
        let frame = host.frame().expect("a frame");
        let truncated: Vec<&str> = frame
            .placements
            .iter()
            .filter(|p| p.paint.truncated)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(
            truncated,
            vec!["/shell/page/root/columns/left/type/body/measure/elided"],
            "exactly one node on this page declares `TextWrap::Ellipsis` with \
             `max_lines: 1`; anything else in this list is a budget too tight for what \
             it holds"
        );
    }

    /// The wrapped and the elided string are the same text and behave
    /// differently.
    #[test]
    fn the_elided_text_truncates_and_the_wrapped_one_does_not() {
        let (_ctx, host) = settled(Presenter::new(dark()));
        let frame = host.frame().expect("a frame");
        let get = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"))
        };
        let wrapped = get("/type/body/measure/wrapped");
        let elided = get("/type/body/measure/elided");

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

    /// A `Spacer` with no constraints takes the cross axis as well as the
    /// main one.
    ///
    /// Pinned rather than fixed. `layout/leaf.rs` answers a spacer's measure
    /// with the offered extent on *both* axes, so a spacer in a horizontal
    /// row is as tall as the row can be — which is not what a reader coming
    /// from SwiftUI expects. Changing it is a semantics decision in shipped,
    /// tested engine code and is not this gallery's to make; recording it is.
    #[test]
    fn an_unconstrained_spacer_claims_the_cross_axis_too() {
        // Clamped tall on purpose. The question this test asks is what an
        // unconstrained spacer does with the *cross* axis it is offered, so
        // the row has to be offered a cross extent worth claiming — the page
        // itself never offers one, because every vertical run on it is
        // `FitContent` and a spacer's natural extent under an open probe is
        // zero.
        let row = row(
            "probe",
            sp("spacing.sm"),
            Align::Start,
            vec![
                gorgon_petra::component::text("a", "left"),
                ViewNode::new(NodeKind::Spacer, "gap"),
                gorgon_petra::component::text("b", "right"),
            ],
        )
        .with_constraints(exact(gorgon_petra::geom::Axis::Vertical, 200.0));
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
            "this test exists because the spacer is much taller than its text siblings; \
             if it is not, the engine's spacer semantics changed and the layout card's \
             `gap` clamp and its comment should go: gap {gap_h}, text {text_h}"
        );
    }

    /// Nothing claims an absurd extent.
    ///
    /// Written after the first capture showed a four-hundred-unit hole in
    /// the middle of the page. A hole is invisible to every other assertion
    /// here — placements are placed, paint is complete, tokens resolve — so
    /// without this the harness would have called that frame clean.
    ///
    /// The bound is the *page's* own height, not the viewport's. The page
    /// scrolls, so a column taller than the window is the normal state of a
    /// gallery with eight cards in it and asserting against the viewport
    /// only says "this window is small". Asserting against the page says the
    /// thing that is actually a fault: nothing inside the page is bigger
    /// than the page. The collection's own box is the one exemption, and it
    /// is a real one — its extent is the whole store's, which is what the
    /// scroll above it exists to move through.
    #[test]
    fn no_placement_claims_an_absurd_extent() {
        let (_ctx, host) = settled(Presenter::new(dark()));
        let frame = host.frame().expect("a frame");
        let page_h = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/page/root"))
            .expect("the page content was placed")
            .rect
            .h;
        let mut tall: Vec<(String, f32)> = frame
            .placements
            .iter()
            .filter(|p| !p.id.ends_with("/page/root") && !p.id.ends_with("/rows"))
            .map(|p| (p.id.clone(), p.rect.h))
            .filter(|(_, h)| *h > page_h)
            .collect();
        tall.sort_by(|a, b| b.1.total_cmp(&a.1));
        assert!(
            tall.is_empty(),
            "these placements are taller than the whole {page_h}pt page: {tall:#?}"
        );
    }

    /// The collection claims a hundred thousand rows and places a window.
    #[test]
    fn the_collection_places_a_window_not_the_whole_store() {
        let (_ctx, host) = settled(Presenter::new(dark()));
        let placed = host.frame().expect("a frame").placements.len();
        assert!(
            placed < 900,
            "the whole gallery placed {placed} nodes against a store of {TOTAL_ROWS} \
             rows; virtualization is not doing its job"
        );
    }

    /// A click on a checkbox flips it, which is the difference between a
    /// control and a picture of one.
    #[test]
    fn clicking_a_checkbox_flips_its_declared_state() {
        let (ctx, mut host) = host();
        step(&ctx, &mut host, RawInput::default());
        let before = host.app().stopped;

        let rect = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id.ends_with("/checks/stopped"))
            .expect("the checkbox was placed")
            .rect;
        step(
            &ctx,
            &mut host,
            press_at(Pos2::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)),
        );

        assert_ne!(
            host.app().stopped,
            before,
            "the press must have reached the checkbox: {}",
            host.app().last_event
        );
    }

    /// A click on the toggle slides the knob; it does not teleport.
    #[test]
    fn toggling_tracing_slides_the_knob() {
        let (ctx, mut host) = host();
        step(&ctx, &mut host, RawInput::default());
        assert!(host.app().tracing, "the gallery ships with tracing on");

        let knob = |host: &Host<Gallery>| {
            host.frame()
                .expect("a frame")
                .placements
                .iter()
                .find(|p| p.id.ends_with("/tracing/appearance/track/knob"))
                .expect("the toggle knob was placed")
                .rect
        };
        let start_x = knob(&host).x;
        let row = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id.ends_with("/tracing"))
            .expect("the toggle row was placed")
            .rect;
        step(
            &ctx,
            &mut host,
            press_at(Pos2::new(row.x + row.w / 2.0, row.y + row.h / 2.0)),
        );
        assert!(
            !host.app().tracing,
            "the press must have flipped tracing: {}",
            host.app().last_event
        );

        let mut seen_mid = false;
        let mut n = 0;
        loop {
            n += 1;
            assert!(n <= 32, "the knob was still sliding after {n} quiet passes");
            let x = knob(&host).x;
            if x < start_x - 0.5
                && gorgon_petra::anim::wants_frame(host.frame().unwrap().transitions)
            {
                seen_mid = true;
            }
            if !gorgon_petra::anim::wants_frame(host.frame().unwrap().transitions) {
                assert!(
                    seen_mid,
                    "the knob jumped from {start_x} to {x} with no in-between frame"
                );
                assert!(
                    x < start_x - 1.0,
                    "off sits the knob left of on: start {start_x}, settled {x}"
                );
                break;
            }
            step(&ctx, &mut host, RawInput::default());
        }
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

    /// Overlay children share the viewport. A toast that vanished because
    /// a modal opened was one `Option` fighting for the only child slot.
    #[test]
    fn a_toast_and_a_modal_are_both_placed() {
        let (ctx, mut host) = host();
        host.app_mut().modal = true;
        host.app_mut().toast = super::ToastMode::Pinned;
        step(&ctx, &mut host, RawInput::default());
        let ids: Vec<&str> = host
            .frame()
            .expect("a frame")
            .placements
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert!(
            ids.iter().any(|id| id.ends_with("/modal")),
            "modal missing from {ids:?}"
        );
        assert!(
            ids.iter().any(|id| id.ends_with("/toast")),
            "toast missing from {ids:?}"
        );
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
    /// every press: a press *inside* the popup is delivered to the popup and
    /// reports no dismissal at all.
    ///
    /// The popup may well close afterwards — the press lands on one of its
    /// items and this application closes the menu when an item is chosen,
    /// which is what a menu is for. What must not happen is a *dismissal*:
    /// `App::dismissed` is the engine telling the application that a press
    /// landed outside a `DismissOutside` surface, and this press did not.
    #[test]
    fn a_press_inside_the_dismiss_popup_reports_no_dismissal() {
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
        assert_eq!(
            host.app().dismissals,
            0,
            "a press inside the surface is not a dismissal: {}",
            host.app().last_event
        );
        assert!(
            host.app().last_event.contains("/menu/"),
            "the press must have been routed into the popup, not dropped: {}",
            host.app().last_event
        );
    }
}
