//! The web lane's canary: one page that must build for the browser as well
//! as for a window (T060, spec 003 US6 scenario 2).
//!
//! `gallery.rs` next door is the *rendering* harness — it opens a window and
//! reports whether the engine agrees it drew what it declared. This file is
//! the *portability* harness, and the two questions are different. A change
//! that reaches for a unix socket, a filesystem path, `winit`, or an `eframe`
//! feature that only exists off the web breaks nothing a native gate can
//! see. It breaks the browser, months later, when somebody finally tries.
//!
//! So this example exists to fail at **build time**:
//!
//! ```text
//! cargo build -p gorgon-petra-egui --example parity --target wasm32-unknown-unknown
//! ```
//!
//! A red exit there is the whole product. Everything below is in service of
//! making that build touch as much of the stack as one page can.
//!
//! # What it deliberately instantiates
//!
//! Compiling a crate for `wasm32` proves its *own* items are portable, but
//! `Host<A>` is generic, and a generic function is only codegen'd where it is
//! called. A canary that built a tree and stopped would leave most of
//! `petra-egui`'s public surface untouched by the wasm build. So this page:
//!
//! * declares **every** [`NodeKind`] — all twelve, the eleven layout- and
//!   content-bearing ones by name and `Custom` through a registered painter;
//! * registers a real image source ([`gorgon_petra_egui::image::ImagePixels`])
//!   so the texture-upload path is compiled, and a second, unregistered one
//!   so the `undrawn` bookkeeping is compiled beside it;
//! * drives a [`RowSource`] under a `Scroll`, so virtualisation is compiled;
//! * reads back [`Host::frame`], [`Host::report`], [`Host::focus`],
//!   [`Host::state`], [`Host::decision`], [`Host::motion`] and
//!   [`Host::shaper`] every pass into a readout the page prints — the
//!   accessors are instantiated *because* the number is on screen, not by a
//!   `let _ =` nobody would notice going stale;
//! * paints through the crate's own `impl eframe::App for Host<_>` rather
//!   than around it ([`ParityWindow::ui`]).
//!
//! # What it cannot catch, and does not pretend to
//!
//! A build gate proves compilation, not behaviour. `std::time::Instant`,
//! `std::thread`, and blocking I/O all compile for `wasm32-unknown-unknown`
//! and then panic or hang in a browser. Catching *those* needs the bundle to
//! be loaded and stepped, which is the next lane's job, not this file's.
//!
//! # Text
//!
//! The scripts card carries Latin with diacritics, Greek, Cyrillic, Hebrew,
//! Arabic, Devanagari, Thai, CJK, and four kinds of emoji — a plain
//! pictograph, a variation-selector sequence, a zero-width-joiner sequence
//! and a skin-tone modifier. Which of those the shipped font set can actually
//! draw is a question for `petra-egui/src/text.rs` and its own tests;
//! what this file guarantees is that the *shaping* path meets them on both
//! targets. A run that comes out as fallback boxes is a font gap, and the
//! caption under the card says so rather than letting the page read as a
//! layout fault.
//!
//! # No literal style values
//!
//! `cargo xtask literal-style` (a `gorgon-xtask` lane that lives only in the
//! private GOrgOn monorepo, not in this repository) scans
//! `gorgon/petra-egui/examples` there. Every colour, corner, gap and type
//! step below is a name from
//! [`gorgon_petra::token::standard_vocabulary`], spelled out at its own call
//! site so the text-based scan can see it.
//!
//! # Running it
//!
//! Native: `cargo run -p gorgon-petra-egui --example parity`
//! Light theme: `PETRA_PARITY_THEME=light cargo run … --example parity`
//! Web: build for `wasm32-unknown-unknown`, then bundle with `wasm-bindgen
//! --target web`, serve the output directory, and open `index.html` —
//! `examples/parity.html` is that page, and it looks for a canvas with the id
//! [`web::CANVAS_ID`].

use std::ops::Range;
use std::sync::Arc;

use egui::{Context, FontId, RawInput};
use gorgon_petra::component::{
    button, checkbox, field, heading, list_row, on_layer, primary_button, progress, radio, section,
    status, tab, tab_bar, text, toggle,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::input::{InputEvent, PointerButton, Route, activates};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::{Presenter, StatusToken, TokenName, dark, standard_vocabulary};
// The light theme is only ever selected from the environment, and there is no
// environment on the web. Importing it unconditionally would be an unused
// import on the one target this file exists to keep building.
#[cfg(not(target_arch = "wasm32"))]
use gorgon_petra::token::light;
use gorgon_petra::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, InputPolicy, InsetRefs, Layer, NodeKind, Props,
    Role, Semantics, TextWrap, TrackSize, ViewNode,
};
use gorgon_petra_egui::fonts::{self, GlyphProbe};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_egui::image::ImagePixels;
use gorgon_petra_egui::paint::{CustomPaintCtx, CustomPainters};

// ---------------------------------------------------------------------------
// Token helpers — the same three `gallery.rs` uses, and for the same reason:
// every name reaches the scan as a literal at its own call site.
// ---------------------------------------------------------------------------

/// A spacing token reference, for the styling props that take one (FR-053).
fn sp(name: &str) -> Option<TokenName> {
    Some(TokenName::new(name).expect("parity spacing tokens are well-formed"))
}

/// A colour, typography or shape token reference.
fn tok(name: &str) -> TokenName {
    TokenName::new(name).expect("parity style tokens are well-formed")
}

/// Symmetric inset from two spacing steps, horizontal first.
fn inset(horizontal: &str, vertical: &str) -> InsetRefs {
    InsetRefs::symmetric(tok(horizontal), tok(vertical))
}

// ---------------------------------------------------------------------------
// The names this page registers, and the one it deliberately does not
// ---------------------------------------------------------------------------

/// The custom kind this page declares, drawn by [`paint_meter`].
///
/// Registered twice on purpose — once on the tree [`gorgon_petra::tree::Registry`],
/// where an unregistered kind is a tree-acceptance violation, and once on
/// [`CustomPainters`], where an unregistered painter is a reported gap. Two
/// registrations, two different failures.
const CUSTOM_KIND: &str = "parity-meter";

/// The image source this page registers pixels for.
const SWATCH_SOURCE: &str = "parity/swatch";

/// The image source this page names and never registers, so the `undrawn`
/// bookkeeping has something to report. It is the control for the swatch: one
/// picture arrives, one is named as missing, and the counter beside them says
/// which is which.
const MISSING_SOURCE: &str = "parity/missing";

/// The row source the virtualized `Collection` pulls from.
const ROW_SOURCE: &str = "parity/rows";

/// How many rows that collection claims. Far more than fit, which is the
/// point: only the visible window is ever built.
const TOTAL_ROWS: usize = 50_000;

/// The swatch is a square this many pixels on a side.
const SWATCH_EDGE: usize = 48;

/// The scripts card's runs: a label, and a string in that writing system.
///
/// Nine rows rather than one long line, because a script that fails to shape
/// should be identifiable by name instead of by squinting at a sentence.
const SCRIPTS: [(&str, &str); 9] = [
    ("latin", "Ünïcödé — fiber ligature test: fi fl ffi"),
    ("greek", "Ελληνικά · διεργασία"),
    ("cyrillic", "русский · процесс"),
    ("hebrew", "עברית · תהליך"),
    ("arabic", "العربية · عملية"),
    ("devanagari", "हिन्दी · प्रक्रिया"),
    ("thai", "ไทย · กระบวนการ"),
    ("cjk", "日本語 · 進程 · 한국어"),
    ("emoji", "🌍 ⚠️ 👩‍💻 👋🏽 🇯🇵"),
];

/// The three fiber states the status card reports, as a key, the status token
/// that names the state, and the detail line under it.
const FIBERS: [(&str, &str, &str); 3] = [
    ("supervisor/root", "status.ok", "12 children · 0 restarts"),
    ("worker/bundler", "status.degraded", "retry 2 of 5"),
    ("worker/uploader", "status.down", "canvas lost its context"),
];

/// The three restart policies the radio group chooses between.
const POLICIES: [(&str, &str); 3] = [
    ("policy-never", "Never"),
    ("policy-failure", "On failure"),
    ("policy-always", "Always"),
];

// ---------------------------------------------------------------------------
// The application
// ---------------------------------------------------------------------------

/// What the previous pass's host reported about itself.
///
/// One frame stale by construction: the application builds its tree before
/// the frame that measures it exists. The label on screen says so instead of
/// pretending the number is current.
///
/// This exists to be *printed*. Every field is read off a different
/// [`Host`] accessor, so a wasm build that could not instantiate one of them
/// fails to compile — which is the only reason a portability canary carries a
/// telemetry band at all.
#[derive(Clone, Default)]
struct Readout {
    seq: u64,
    placements: usize,
    drawn: usize,
    silent: usize,
    empty: usize,
    customs: usize,
    undrawn: usize,
    unresolved: usize,
    desynced: bool,
    focusables: usize,
    focused: Option<String>,
    page_offset: f32,
    repaint: bool,
    undeclared: usize,
    ambient_clean: bool,
    galley_evictions: u64,
}

impl Readout {
    /// The eight tiles the stat grid draws, in reading order.
    fn tiles(&self) -> [(&'static str, String); 8] {
        [
            ("placements", self.placements.to_string()),
            ("drawn", self.drawn.to_string()),
            ("silent", self.silent.to_string()),
            ("empty", self.empty.to_string()),
            ("customs", self.customs.to_string()),
            ("undrawn", self.undrawn.to_string()),
            ("unresolved", self.unresolved.to_string()),
            (
                "digest",
                if self.desynced { "DESYNCED" } else { "in sync" }.to_owned(),
            ),
        ]
    }

    /// The one-line summary under the tiles: the readings that come from the
    /// focus tree, the layout state and the motion scheduler rather than from
    /// the paint report.
    fn summary(&self) -> String {
        format!(
            "focusables {} · focused {} · page offset {:.0} · repaint {} · undeclared {} \
             · ambient {} · galley evictions {}",
            self.focusables,
            self.focused.as_deref().unwrap_or("none"),
            self.page_offset,
            self.repaint,
            self.undeclared,
            if self.ambient_clean { "clean" } else { "DIRTY" },
            self.galley_evictions,
        )
    }
}

/// The parity page's own state. Everything a control on the page can change.
struct Parity {
    readout: Readout,
    /// What the last routed event did, echoed on screen so the input path is
    /// observable in a browser without a debugger.
    last_event: String,
    modal: bool,
    tab: usize,
    stopped: bool,
    verbose: bool,
    policy: usize,
    tracing: bool,
    progress: f32,
    selected_row: Option<usize>,
    /// True when this page must not move in response to input.
    ///
    /// The desktop half of the `petra-parity` lane is a **real window on the
    /// operator's live X server**, and the web half is a headless canvas that
    /// no pointer can reach. Every field above this one is mutable from
    /// input, and all but `last_event` are drawn outside the lane's declared
    /// non-parity regions — so a stray click while the window is up moves the
    /// desktop raster and nothing moves the browser's. That is not a parity
    /// regression, but the comparator cannot tell the difference, and it is
    /// exactly what happened on 2026-08-24: the bundle bar read 50% in the
    /// window and 62% in the browser, and the lane failed with a worst delta
    /// of 209/255 at the bar's own pixels.
    ///
    /// A pixel-parity fixture has to be a pure function of its build, its
    /// viewport and its theme. When this flag is set the event still routes
    /// and still reaches `handle` — nothing about Petra's input path is
    /// bypassed — and `handle` then changes nothing at all, `last_event`
    /// included. Not even the echo: `last_event` is drawn through `note`,
    /// which wraps, so a longer line could take a second row and paint below
    /// the `readout-summary` rectangle declared to excuse it.
    ///
    /// The freeze is not silent. The sample prints `petra parity: frozen=true`
    /// on stderr before it opens a window, the lane refuses to capture without
    /// that answer, and it echoes the handshake into its own output.
    frozen: bool,
}

impl Default for Parity {
    fn default() -> Self {
        Self {
            readout: Readout::default(),
            last_event: String::new(),
            modal: false,
            tab: 0,
            stopped: true,
            verbose: false,
            policy: 1,
            tracing: true,
            // Neither end: a bar pinned to zero or one demonstrates nothing
            // about how the two weighted tracks split.
            progress: 0.62,
            selected_row: Some(2),
            // Off by default: running the sample by hand is supposed to be
            // interactive, and every test below wants the live input path.
            // Only the capture lane turns it on, through `FROZEN_VAR`.
            frozen: false,
        }
    }
}

/// The variable the capture lane sets to freeze this page.
///
/// Read by [`build_window`] on the native target only. Its spelling is a
/// contract with `gorgon/xtask/src/parity/desktop.rs` (the `gorgon-xtask`
/// capture lane, private-monorepo-only), which sets it, and the
/// two cannot drift silently: `main` prints `petra parity: frozen=<bool>` on
/// stderr and `NativeWindow::open` fails the lane when it asked for a frozen
/// page and did not get that line. The same shape the `WINDOW_TITLE` literal
/// already uses across the same crate boundary.
#[cfg(not(target_arch = "wasm32"))]
const FROZEN_VAR: &str = "PETRA_PARITY_FROZEN";

/// The logical rectangle this page lays itself out in, on **both** targets.
///
/// Petra's layout is viewport-relative: the page grid is one weight-1.0
/// column and the card band under it is two, so every card's width — and,
/// through wrapping, every row's height below it — is a function of how wide
/// the surface is. A window manager that hands the sample a different size on
/// two consecutive runs therefore hands the `petra-parity` lane a different
/// picture, and SC-006's ppm moves with the desktop rather than with the
/// code. Measured on 2026-08-24 at one unchanged commit: 1600x1000 gave 81
/// ppm over 1 368 424 compared pixels with 14.5% of the frame excluded by the
/// declared masks, and 2009x1392 gave 60 ppm over 2 540 412 with 9.2%
/// excluded.
///
/// So the page does not use the surface it is given. [`ParityWindow`] pins
/// `RawInput::screen_rect` to exactly this rectangle at the surface's own
/// origin, on the native target and in the browser alike, and whatever the
/// window system granted beyond it stays cleared. The lane crops its desktop
/// capture to the same rectangle and drives the browser canvas at it, so both
/// captures are the same fixed geometry however large the window was.
///
/// The two sides do not take each other on trust: the sample reports this
/// size as `viewport=` in its stderr handshake and
/// `gorgon/xtask/src/parity/desktop.rs`'s `check_handshake` refuses any value
/// but its own `CAPTURE_SIZE`.
///
/// Sized under [`WINDOW_REQUEST`] with room to spare, because the pin is only
/// useful while the granted surface is at least this large; below it the lane
/// fails naming the size rather than cropping a page that never painted
/// there.
const PARITY_VIEWPORT: (f32, f32) = (1400.0, 900.0);

/// The inner size the native sample asks the window system for.
///
/// **At or above `CAPTURE_ORIGIN + CAPTURE_SIZE` in
/// `gorgon/xtask/src/parity.rs`, which is 1400x900 at (0, 0).** This used to
/// be 1200x900 — narrower than that — so the `petra-parity` lane could only
/// capture the page when a tiling compositor happened to enlarge the window
/// past what the sample asked for, and failed with "the compositor gave the
/// sample a 1200x900 window" whenever the window manager simply honoured the
/// request. Measured on 2026-08-24: three consecutive runs at 2009x1392,
/// 1401x1392 and 1200x900, the last of them a gate failure caused by nothing
/// but the operator's desktop state.
///
/// What the window manager grants above this no longer changes what is
/// measured — the page lays itself out in [`PARITY_VIEWPORT`] and the lane
/// crops to it — but it still has to grant enough for that rectangle to sit
/// inside. The lane does not take this on trust: the sample reports it in the
/// same stderr handshake that carries `frozen=`, and `NativeWindow::confirm`
/// fails by name if it is too small.
#[cfg(not(target_arch = "wasm32"))]
const WINDOW_REQUEST: (f32, f32) = (1600.0, 1000.0);

/// Whether [`FROZEN_VAR`]'s value asks for a frozen page.
///
/// Split from the environment read so it can be tested without mutating a
/// process-wide variable — the same shape `xtask`'s own `require_runtime_dir`
/// uses for the same reason. Set-but-empty and `0` are "no", so a caller that
/// clears the variable by setting it empty gets the interactive page rather
/// than a silently frozen one.
#[cfg(not(target_arch = "wasm32"))]
fn frozen_from_env(value: Option<std::ffi::OsString>) -> bool {
    value.is_some_and(|value| !value.is_empty() && value != "0")
}

/// Whether this build must ignore input that would move the page.
///
/// Always false on the web: that target has no environment to read and no
/// pointer to defend against — its canvas is driven by WebDriver, which
/// clicks nothing.
fn frozen() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        frozen_from_env(std::env::var_os(FROZEN_VAR))
    }
}

// ---------------------------------------------------------------------------
// The three places the component library does not reach
// ---------------------------------------------------------------------------
//
// These three are the same three `gallery.rs` documents at length, applying
// the same rules. They are restated here rather than shared because two
// examples cannot share a private module without inventing a crate to hold
// it, and because this file must stay buildable on its own: a canary that
// stopped compiling when its neighbour was edited would report the wrong
// failure. A fourth place, re-seating a control placed on a card, used to
// live here too as a private `on_card`; it is now
// `gorgon_petra::component::on_layer`, imported rather than restated,
// because the rule it encodes (which tone a control's own `background`
// resolves to at a given nesting depth) belongs to the library's token
// vocabulary, not to this page.

/// Body text in the muted tone. `text` binds `text.primary` and takes no tone
/// argument, so the component is composed and its one slot rebound rather
/// than a `NodeKind::Text` being hand-rolled with a different wrap policy.
fn muted(key: &str, content: &str) -> ViewNode {
    let mut node = text(key, content);
    node.props
        .tokens
        .insert("foreground".into(), tok("text.muted"));
    node
}

/// A muted run that is allowed to wrap. Every explanatory line on this page
/// goes through here: without it, a note wider than its card is truncated —
/// honestly, but truncated.
fn note(key: &str, content: &str) -> ViewNode {
    let mut node = muted(key, content);
    node.props.wrap = Some(TextWrap::Wrap);
    node
}

/// A field label or column header: muted, set in capitals.
///
/// The uppercasing happens here, not at the call sites, so a caller who
/// forgets cannot leave a label in a different typographic role from its
/// neighbours.
fn caption(key: &str, content: &str) -> ViewNode {
    muted(key, &content.to_uppercase())
}

// ---------------------------------------------------------------------------
// Primitive helpers
// ---------------------------------------------------------------------------

impl Parity {
    /// A horizontal stack. `align` is explicit at every call site, because a
    /// control row and a text row want different answers.
    fn row(key: &str, gap: Option<TokenName>, align: Align, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                spacing: gap,
                align: Some(align),
                ..Props::default()
            })
            .with_children(children)
    }

    /// A vertical run of blocks, each as tall as it needs to be.
    ///
    /// A single-column `Grid`, not a `Stack`: a vertical `Stack` placed at an
    /// exact height divides that height among its children by equal share, so
    /// one tall block in a run of short ones is squeezed and its text
    /// truncates. A `Grid`'s implicit rows are fit-to-content, which is what a
    /// vertical run of blocks actually wants.
    fn column(key: &str, gap: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
        Self::tracks(key, vec![TrackSize::Weight { weight: 1.0 }], gap, children)
    }

    /// A vertical run whose one column is a fixed measure rather than the room
    /// available. The width has to be the track's, not a clamp on each child:
    /// a text node measures its wrapped height against the width it is
    /// offered, and narrowing the box afterwards does not re-wrap the run.
    fn measure_column(
        key: &str,
        width: f32,
        gap: Option<TokenName>,
        children: Vec<ViewNode>,
    ) -> ViewNode {
        Self::tracks(key, vec![TrackSize::Fixed { value: width }], gap, children)
    }

    /// The shared shape of [`Self::column`] and [`Self::measure_column`].
    fn tracks(
        key: &str,
        columns: Vec<TrackSize>,
        gap: Option<TokenName>,
        children: Vec<ViewNode>,
    ) -> ViewNode {
        ViewNode::new(NodeKind::Grid, key)
            .with_props(Props {
                columns,
                row_spacing: gap,
                ..Props::default()
            })
            .with_children(children)
    }

    /// The body of a card: a [`Self::column`] that outranks the card's own
    /// title when the card divides its height.
    ///
    /// `section` is a vertical `Stack`, so title and body go through the
    /// equal-share distribution, and with two children of very different
    /// heights the body is the one that loses. Priority is the declared way
    /// out: a body above the title is offered everything the title's floor
    /// does not need.
    fn body(key: &str, gap: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
        Self::column(key, gap, children).with_constraints(Constraints {
            vertical: AxisConstraint {
                min: None,
                max: None,
                priority: 1,
            },
            ..Constraints::default()
        })
    }

    /// A drawn horizontal rule.
    ///
    /// A bare `Separator` paints nothing: the painter knows four token slots,
    /// and a node that binds none of them declares no content. Binding the
    /// background is what makes a rule a rule — and a `Separator` has no
    /// `border` slot to bind, so its `background` *is* the edge, the same
    /// role a card's outline used to play. That makes `text.muted` here the
    /// same conscription BORDERS.md names everywhere else (R2), so it takes
    /// `border.subtle` rather than a text tone.
    ///
    /// It is the **decorative** tone and the sentence that used to be here
    /// said otherwise. "A divider is a component boundary, WCAG 2.1 SC
    /// 1.4.11's 3:1 case" over-reads that clause: SC 1.4.11 covers the
    /// information that identifies a *component*, and a line between two
    /// blocks identifies none. Holding every rule to the control floor is
    /// what made this page read as a wireframe; the floor a rule is held to
    /// now is Carbon's own 1.3:1. `gorgon_petra::token::shipped`'s
    /// `BORDER_TOKEN` carries the table.
    fn rule(key: &str) -> ViewNode {
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), tok("border.subtle"));
        ViewNode::new(NodeKind::Separator, key).with_props(props)
    }

    /// A hard clamp on one axis, so a node is measured against an extent the
    /// window cannot change out from under it.
    ///
    /// Layout declaration, not styling: the literal-style lane never inspects
    /// constraints, and says so, because a track size and a text measure are
    /// numeric by spec.
    fn exact(axis: Axis, value: f32) -> Constraints {
        Self::clamp(axis, Some(value), Some(value))
    }

    /// A floor on one axis with no ceiling: what a control that must be big
    /// enough to look like a control, but may grow, declares.
    fn at_least(axis: Axis, value: f32) -> Constraints {
        Self::clamp(axis, Some(value), None)
    }

    /// The shared shape of [`Self::exact`] and [`Self::at_least`].
    fn clamp(axis: Axis, min: Option<f32>, max: Option<f32>) -> Constraints {
        let clamp = AxisConstraint {
            min,
            max,
            priority: 0,
        };
        match axis {
            Axis::Horizontal => Constraints {
                horizontal: clamp,
                ..Constraints::default()
            },
            Axis::Vertical => Constraints {
                vertical: clamp,
                ..Constraints::default()
            },
        }
    }

    /// A rect of exactly `width` by `height`, for the two hosted wells.
    fn well(width: f32, height: f32) -> Constraints {
        Constraints {
            horizontal: AxisConstraint {
                min: Some(width),
                max: Some(width),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(height),
                max: Some(height),
                priority: 0,
            },
        }
    }

    /// The card treatment: raised fill, an elevation shadow, a medium corner.
    ///
    /// `section` applies exactly this to every titled block (down to the
    /// same `shadow.raised` token, for the same reason: BORDERS.md R2/R3 —
    /// no `border` may bind a text tone, and depth is drawn as elevation, not
    /// as an outline, once there is a fill to spend). The telemetry band is
    /// the one card here that is a `Grid` rather than a titled column, so it
    /// cannot go through `section`; this function is how the two stay one
    /// object.
    fn card(mut node: ViewNode) -> ViewNode {
        node.props.padding = Some(inset("spacing.lg", "spacing.md"));
        node.props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        node.props
            .tokens
            .insert("shadow".into(), tok("shadow.raised"));
        node.props
            .tokens
            .insert("radius".into(), tok("shape.corner-md"));
        node
    }

    /// A floating surface: the modal and the toast, the only two things here
    /// with the largest corner.
    ///
    /// `shadow.overlay`, not `shadow.raised`: `SHADOW_GEOMETRY`'s own doc
    /// draws the line at "has left the page entirely" for the deeper of the
    /// two elevations, and a viewport-anchored dialog or toast is exactly
    /// that — unlike [`Self::card`], which is still part of the page's own
    /// flow.
    fn surface(key: &str, layer: Layer, policy: InputPolicy) -> ViewNode {
        let mut props = Props {
            layer: Some(layer),
            anchor: Some(Anchor::Viewport),
            clamp: Some(ClampRule::Shrink),
            input_policy: Some(policy),
            // One child, always. A surface stacks its children the way a
            // vertical `Stack` does, and stacking blocks of different heights
            // inside one is the equal-share squeeze all over again.
            axis: Some(Axis::Vertical),
            padding: Some(inset("spacing.xl", "spacing.lg")),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        props.tokens.insert("shadow".into(), tok("shadow.overlay"));
        props.tokens.insert("radius".into(), tok("shape.corner-lg"));
        ViewNode::new(NodeKind::Surface, key).with_props(props)
    }
}

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

impl Parity {
    /// Title, target, and one line saying what a red build here means.
    fn masthead() -> ViewNode {
        let mut title = Props {
            text: Some("Petra web parity".to_owned()),
            style: Some(tok("typography.heading-lg")),
            ..Props::default()
        };
        title
            .tokens
            .insert("foreground".into(), tok("text.primary"));
        Self::column(
            "masthead",
            sp("spacing.2xs"),
            vec![
                ViewNode::new(NodeKind::Text, "title").with_props(title),
                note(
                    "tagline",
                    "The same page, built for a window and for a browser. If the \
                     wasm32 build of this example goes red, the web lane is broken \
                     — that is what this file is for.",
                ),
                muted("target", target_line()),
            ],
        )
    }

    /// The telemetry band: eight tiles off the paint report, and one summary
    /// line off the focus tree, the layout state, the motion scheduler and
    /// the galley cache.
    fn telemetry(&self) -> ViewNode {
        let mut stats = ViewNode::new(NodeKind::Grid, "stats").with_props(Props {
            columns: vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Weight { weight: 1.0 },
            ],
            column_spacing: sp("spacing.lg"),
            row_spacing: sp("spacing.md"),
            ..Props::default()
        });
        for (label, value) in &self.readout.tiles() {
            stats = stats.child(Self::column(
                &label.replace(' ', "-"),
                sp("spacing.2xs"),
                vec![caption("label", label), heading("value", value.as_str())],
            ));
        }

        Self::card(Self::column(
            "telemetry",
            sp("spacing.md"),
            vec![
                caption(
                    "heading",
                    &format!("frame {} · previous pass", self.readout.seq),
                ),
                stats,
                note("summary", &self.readout.summary()),
                note(
                    "last",
                    &format!(
                        "last event: {}",
                        if self.last_event.is_empty() {
                            "none yet — click a control or press Tab"
                        } else {
                            &self.last_event
                        }
                    ),
                ),
            ],
        ))
    }

    /// Mixed scripts, one row per writing system, plus the emoji row.
    ///
    /// Two columns: the script's name in capitals, and the run itself. The
    /// name is Latin whatever happens to the run beside it, so a script that
    /// comes out as fallback boxes is still identifiable.
    fn scripts_card() -> ViewNode {
        let mut grid = ViewNode::new(NodeKind::Grid, "scripts").with_props(Props {
            columns: vec![
                TrackSize::Fixed { value: 110.0 },
                TrackSize::Weight { weight: 1.0 },
            ],
            column_spacing: sp("spacing.md"),
            row_spacing: sp("spacing.sm"),
            ..Props::default()
        });
        for (name, run) in SCRIPTS {
            let mut props = Props {
                text: Some(run.to_owned()),
                wrap: Some(TextWrap::Wrap),
                style: Some(tok("typography.body")),
                ..Props::default()
            };
            props
                .tokens
                .insert("foreground".into(), tok("text.primary"));
            grid = grid
                .child(caption(&format!("l-{name}"), name))
                .child(ViewNode::new(NodeKind::Text, name).with_props(props));
        }

        section(
            "scripts",
            "Scripts",
            vec![Self::body(
                "body",
                sp("spacing.md"),
                vec![
                    grid,
                    note(
                        "note",
                        "Whether these shape or fall back to boxes is a property of \
                         the installed font set, not of the layout. The row label \
                         stays Latin so a gap is identifiable by name.",
                    ),
                ],
            )],
        )
    }

    /// Wrapping against a fixed measure, and the same string elided at one
    /// line.
    ///
    /// Both runs share one measure, and the measure is the track's width
    /// rather than a clamp on each node. At the card's full width the wrapped
    /// string fits on one line and the elided one never truncates, and then
    /// the card demonstrates neither.
    fn text_card() -> ViewNode {
        let long = "A long line that has to wrap, because the whole point of a text \
                    node is that the engine measures it against the width it is \
                    offered rather than trusting whoever wrote the string.";

        let mut wrapped = Props {
            text: Some(long.to_owned()),
            wrap: Some(TextWrap::Wrap),
            style: Some(tok("typography.body")),
            ..Props::default()
        };
        wrapped
            .tokens
            .insert("foreground".into(), tok("text.primary"));

        let mut elided = Props {
            text: Some(long.to_owned()),
            wrap: Some(TextWrap::Ellipsis),
            max_lines: Some(1),
            style: Some(tok("typography.body")),
            ..Props::default()
        };
        elided.tokens.insert("foreground".into(), tok("text.muted"));

        section(
            "type",
            "Text",
            vec![Self::body(
                "body",
                sp("spacing.md"),
                vec![Self::measure_column(
                    "measure",
                    360.0,
                    sp("spacing.sm"),
                    vec![
                        ViewNode::new(NodeKind::Text, "wrapped").with_props(wrapped),
                        caption("elide-note", "the same string, capped at one line"),
                        ViewNode::new(NodeKind::Text, "elided").with_props(elided),
                    ],
                )],
            )],
        )
    }

    /// Nine of the component library's names in one card: buttons, the three
    /// binary controls, the tab strip, a progress bar and a field.
    fn controls_card(&self) -> ViewNode {
        let buttons = Self::row(
            "buttons",
            sp("spacing.md"),
            Align::Center,
            vec![
                // R5: one accent per view. This is the page's single loudest
                // action — the entry point into the whole ship-the-bundle
                // flow — so it is the one control that spends
                // `accent.primary` rather than a layer step. `primary_button`
                // is not passed through `on_layer`: it fills with the accent,
                // which `on_layer` never rewrites (see its own doc), so
                // wrapping it here would be a no-op that misstates the call
                // site's intent.
                primary_button("open-modal", "Open modal"),
                on_layer(button("bump", "Bump progress"), 1),
            ],
        );

        let checks = Self::row(
            "checks",
            sp("spacing.md"),
            Align::Center,
            vec![
                checkbox("stopped", "Include stopped", self.stopped),
                checkbox("verbose", "Verbose trace", self.verbose),
            ],
        );

        let policies = Self::row(
            "policies",
            sp("spacing.md"),
            Align::Center,
            POLICIES
                .iter()
                .enumerate()
                .map(|(i, (key, label))| radio(*key, *label, i == self.policy))
                .collect(),
        );

        let tabs = Self::column(
            "tabs",
            sp("spacing.sm"),
            vec![
                tab_bar(
                    "tablist",
                    vec![
                        on_layer(tab("tab-native", "Native", self.tab == 0), 1),
                        on_layer(tab("tab-web", "Web", self.tab == 1), 1),
                        on_layer(tab("tab-both", "Both", self.tab == 2), 1),
                    ],
                ),
                Self::rule("tab-rule"),
            ],
        );

        let done = self.progress.clamp(0.0, 1.0);
        let bar = Self::row(
            "bundle",
            sp("spacing.md"),
            Align::Center,
            vec![
                caption("label", "bundle"),
                progress("bar", "Bundle", done)
                    .with_constraints(Self::at_least(Axis::Horizontal, 140.0)),
                text("value", format!("{:.0}%", done * 100.0)),
            ],
        );

        section(
            "controls",
            "Controls",
            vec![Self::body(
                "body",
                sp("spacing.md"),
                vec![
                    buttons,
                    Self::rule("rule-a"),
                    checks,
                    Self::row(
                        "policy-row",
                        sp("spacing.md"),
                        Align::Center,
                        vec![caption("label", "restart"), policies],
                    ),
                    toggle("tracing", "Trace this tree", self.tracing),
                    Self::rule("rule-b"),
                    tabs,
                    bar,
                ],
            )],
        )
    }

    /// The two shapes of text entry: the `field` component, and a bare
    /// `NodeKind::Input` that takes no input.
    ///
    /// The bare one is not a demonstration of an editable box — it declares no
    /// interactions, so nothing can focus or type into it. It is here because
    /// the component builds its own `Input` internally and this file must
    /// prove that the kind is reachable directly too, on both targets.
    fn form_card() -> ViewNode {
        let mut readonly = Props {
            placeholder: Some("wasm32-unknown-unknown".to_owned()),
            style: Some(tok("typography.body")),
            ..Props::default()
        };
        // `surface.raised`, not `surface.base`, and seated with `on_layer`
        // below at the call site — the same recipe `field`'s own doc spells
        // out ("seats a field one step ahead of whatever it is placed on,
        // so it reads as a well cut into the card"). This node exists to
        // prove `NodeKind::Input` is reachable without the component, so it
        // earns that by matching what the component actually paints, not by
        // improvising a different box.
        readonly
            .tokens
            .insert("background".into(), tok("surface.raised"));
        readonly
            .tokens
            .insert("foreground".into(), tok("text.muted"));
        readonly
            .tokens
            .insert("radius".into(), tok("shape.corner-sm"));

        let grid = ViewNode::new(NodeKind::Grid, "form")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 110.0 },
                    TrackSize::Weight { weight: 1.0 },
                ],
                column_spacing: sp("spacing.md"),
                row_spacing: sp("spacing.sm"),
                // A field's natural width is its placeholder's width, so
                // without this the boxes are as wide as the words in them and
                // the form has a ragged right edge.
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .child(Self::row(
                "l-name",
                None,
                Align::Center,
                vec![caption("l-name-text", "fiber name")],
            ))
            .child(field("f-name", "supervisor/root"))
            .child(Self::row(
                "l-target",
                None,
                Align::Center,
                vec![caption("l-target-text", "build target")],
            ))
            .child(
                on_layer(
                    ViewNode::new(NodeKind::Input, "f-target").with_props(readonly),
                    1,
                )
                // Not `field()`, so it does not carry `size-md` itself.
                .with_constraints(Self::at_least(Axis::Vertical, 40.0)),
            );

        section(
            "form",
            "Form",
            vec![Self::body(
                "body",
                sp("spacing.md"),
                vec![
                    grid,
                    note(
                        "note",
                        "The second box is a bare Input with no interactions: it is \
                         here to prove the kind is reachable without the component, \
                         not to be typed into.",
                    ),
                ],
            )],
        )
    }

    /// The three track-sizing rules, a `Spacer` and a `Separator` — the grid
    /// vocabulary the component library leaves to the primitives.
    fn layout_card() -> ViewNode {
        let grid = ViewNode::new(NodeKind::Grid, "tracks")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 90.0 },
                    TrackSize::FitContent,
                    TrackSize::Weight { weight: 1.0 },
                ],
                column_spacing: sp("spacing.md"),
                row_spacing: sp("spacing.xs"),
                ..Props::default()
            })
            .with_semantics(Semantics {
                role: Some(Role::Table),
                ..Semantics::default()
            })
            .child(caption("h-a", "fixed 90"))
            .child(caption("h-b", "fitcontent"))
            .child(caption("h-c", "weight(1)"))
            .child(text("g-a", "clamped"))
            .child(text("g-b", "wider cell here"))
            .child(text("g-c", "takes the rest"));

        let spacer_row = Self::row(
            "spacer-row",
            sp("spacing.sm"),
            Align::Center,
            vec![
                text("left", "left"),
                // Height-clamped on purpose. An unconstrained `Spacer` takes
                // everything offered on *both* axes, so one in a horizontal
                // row claims the row's whole height and opens a hole the size
                // of the window.
                ViewNode::new(NodeKind::Spacer, "gap")
                    .with_constraints(Self::exact(Axis::Vertical, 0.0)),
                text("right", "pushed right by a Spacer"),
            ],
        );

        section(
            "layout",
            "Layout primitives",
            vec![Self::body(
                "body",
                sp("spacing.md"),
                vec![grid, Self::rule("layout-rule"), spacer_row],
            )],
        )
    }

    /// Every shipped status, through colour, shape and the word.
    fn status_card() -> ViewNode {
        let vocabulary = standard_vocabulary();
        let mut grid = ViewNode::new(NodeKind::Grid, "fibers").with_props(Props {
            columns: vec![TrackSize::FitContent, TrackSize::Weight { weight: 1.0 }],
            column_spacing: sp("spacing.md"),
            row_spacing: sp("spacing.sm"),
            ..Props::default()
        });
        for (fiber, token, detail) in FIBERS {
            let declared: &StatusToken = vocabulary
                .status(&tok(token))
                .unwrap_or_else(|| panic!("{token} must be a declared status"));
            grid = grid
                .child(status(format!("{fiber}-state"), declared))
                .child(Self::column(
                    fiber,
                    sp("spacing.2xs"),
                    vec![text("name", fiber), note("detail", detail)],
                ));
        }

        section(
            "status",
            "Fibers",
            vec![Self::body(
                "body",
                sp("spacing.md"),
                vec![
                    grid,
                    note(
                        "note",
                        "Colour is never the only channel: every state carries its own \
                         word and its own silhouette, because `status` takes a whole \
                         StatusToken and there is no way to hand it a hue on its own.",
                    ),
                ],
            )],
        )
    }

    /// A `Scroll` over a `Collection` that claims fifty thousand rows.
    ///
    /// The collection asks [`RowSource`] only for the window it can see, so the
    /// row count is a claim about the store and not about work this frame did.
    /// Watch the placements tile: it does not grow with [`TOTAL_ROWS`].
    fn collection_card() -> ViewNode {
        let list = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            source: Some(ROW_SOURCE.to_owned()),
            total_count: Some(TOTAL_ROWS),
            // A `list_row` is a padded stack around a 20-unit body line.
            // Declaring 28 rather than the bare line height keeps the rows
            // contiguous instead of leaving a gap under each one.
            estimated_extent: Some(28.0),
            axis: Some(Axis::Vertical),
            ..Props::default()
        });

        section(
            "store",
            "Store",
            vec![Self::body(
                "body",
                sp("spacing.md"),
                vec![
                    caption(
                        "note",
                        "50 000 rows declared · only the visible window is placed",
                    ),
                    ViewNode::new(NodeKind::Scroll, "scroll")
                        .with_props(Props {
                            axis: Some(Axis::Vertical),
                            // On the scroll, not on the collection inside it:
                            // tree acceptance refuses `overscan` on a nested
                            // collection rather than silently ignoring it.
                            overscan: Some(64.0),
                            ..Props::default()
                        })
                        .with_constraints(Self::exact(Axis::Vertical, 200.0))
                        .child(list),
                ],
            )],
        )
    }

    /// The two hosted kinds, one of each outcome: a registered image and a
    /// registered painter beside an image source nobody registered.
    ///
    /// This is the card that makes the texture-upload path part of the wasm
    /// build. An `Image` node with no registered source compiles the same as
    /// one with, so a page that declared only the missing one would prove
    /// nothing about whether a picture can reach the GPU in a browser.
    fn hosted_card() -> ViewNode {
        let mut swatch = Props {
            image: Some(SWATCH_SOURCE.to_owned()),
            ..Props::default()
        };
        swatch
            .tokens
            .insert("radius".into(), tok("shape.corner-sm"));

        let mut missing = Props {
            image: Some(MISSING_SOURCE.to_owned()),
            ..Props::default()
        };
        // `surface.base` stays literal, not `on_layer`-seated: this box
        // sits beside `Self::meter`'s in the same row, and that one's own
        // doc calls the identical pattern out by name — "a well on the page
        // colour inside a raised card, so the plot area is a hole in the
        // card rather than a box on top of it." Re-seating one of the two
        // and not the other would make two visually-matched wells in one
        // row read as different depths. The fill alone (no border) still
        // paints a shape, so the fixture's whole point — an unregistered
        // image source landing in `PaintReport::undrawn` while the node
        // around it is not `Outcome::Silent` — still holds.
        missing
            .tokens
            .insert("background".into(), tok("surface.base"));
        missing
            .tokens
            .insert("radius".into(), tok("shape.corner-sm"));

        let pair = ViewNode::new(NodeKind::Grid, "pair")
            .with_props(Props {
                columns: vec![TrackSize::FitContent, TrackSize::Weight { weight: 1.0 }],
                column_spacing: sp("spacing.md"),
                row_spacing: sp("spacing.sm"),
                ..Props::default()
            })
            .child(
                ViewNode::new(NodeKind::Image, "swatch")
                    .with_props(swatch)
                    .with_constraints(Self::well(96.0, 48.0)),
            )
            .child(Self::column(
                "swatch-caption",
                sp("spacing.2xs"),
                vec![
                    caption("label", "image · registered"),
                    note(
                        "detail",
                        "pixels this file generates, uploaded once and cached by \
                         source name — the path a browser has to reach for a picture \
                         to appear at all",
                    ),
                ],
            ))
            .child(
                ViewNode::new(NodeKind::Image, "absent")
                    .with_props(missing)
                    .with_constraints(Self::well(96.0, 48.0)),
            )
            .child(Self::column(
                "absent-caption",
                sp("spacing.2xs"),
                vec![
                    caption("label", "image · no source"),
                    note(
                        "detail",
                        "named and never registered, so the paint pass reports it by \
                         name and the undrawn tile above reads 1",
                    ),
                ],
            ))
            .child(Self::meter("meter-swatch", 96.0, 48.0))
            .child(Self::column(
                "meter-caption",
                sp("spacing.2xs"),
                vec![
                    caption("label", "custom · meter"),
                    note(
                        "detail",
                        "a registered painter draws it, in Petra's own paint order, \
                         from the same theme snapshot the rest of the frame used",
                    ),
                ],
            ));

        section(
            "hosted",
            "Hosted content",
            vec![Self::body(
                "body",
                sp("spacing.md"),
                vec![
                    note(
                        "note",
                        "Petra places and clips all four of these; the pixels come \
                         from the host and the frame digest cannot see them, which is \
                         what makes this frame hosted.",
                    ),
                    pair,
                ],
            )],
        )
    }

    /// The custom node [`paint_meter`] draws into.
    ///
    /// A well on the page colour inside a raised card, so the plot area is a
    /// hole in the card rather than a box on top of it. Everything inside the
    /// rect is the painter's, and Petra never sees it.
    fn meter(key: &str, width: f32, height: f32) -> ViewNode {
        let mut props = Props {
            custom_kind: Some(CUSTOM_KIND.to_owned()),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), tok("surface.base"));
        props.tokens.insert("radius".into(), tok("shape.corner-sm"));
        ViewNode::new(NodeKind::Custom, key)
            .with_props(props)
            .with_constraints(Self::well(width, height))
    }

    /// The toast that is always on screen, and the modal that is not.
    ///
    /// The toast is deliberately unconditional. A `Surface` that only appears
    /// after a click would leave the kind unplaced in every capture and in
    /// every headless pass, and a canary that has to be clicked before it
    /// covers its own subject is a canary with a hole in it.
    fn toast() -> ViewNode {
        Self::surface("toast", Layer::Toast, InputPolicy::Passthrough).child(Self::column(
            "body",
            sp("spacing.2xs"),
            vec![
                caption("label", "surface · passthrough"),
                muted("detail", "clicks reach the page underneath this strip"),
            ],
        ))
    }

    /// The modal, when it is open: the blocking input policy, in a dialog.
    fn modal(&self) -> Option<ViewNode> {
        if !self.modal {
            return None;
        }
        Some(
            Self::surface("modal", Layer::Modal, InputPolicy::Block)
                .with_semantics(Semantics {
                    role: Some(Role::Dialog),
                    label: Some("Ship the web bundle".to_owned()),
                    ..Semantics::default()
                })
                .child(Self::column(
                    "body",
                    sp("spacing.md"),
                    vec![
                        heading("title", "Ship the web bundle?"),
                        Self::rule("rule"),
                        note(
                            "note",
                            "A click outside this dialog is swallowed — that is what \
                             Block means. Focus cannot leave it either.",
                        ),
                        Self::row(
                            "actions",
                            sp("spacing.md"),
                            Align::Center,
                            vec![
                                ViewNode::new(NodeKind::Spacer, "push")
                                    .with_constraints(Self::exact(Axis::Vertical, 0.0)),
                                on_layer(button("modal-close", "Cancel"), 1),
                                on_layer(button("modal-confirm", "Ship"), 1),
                            ],
                        ),
                    ],
                )),
        )
    }
}

/// The line under the tagline naming the target this binary was built for.
///
/// Two arms of one `cfg`, so the page itself says which build produced it —
/// a screenshot of the web bundle and a screenshot of the window are
/// otherwise indistinguishable, which is exactly the confusion a parity page
/// must not create.
fn target_line() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    {
        "built for wasm32-unknown-unknown · running in a browser canvas"
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "built for the host target · running in a native window"
    }
}

/// Whether `event` should act on the node it routed to.
///
/// [`activates`] answers only for the keyboard stand-in, because that is the
/// part the router has to widen to keep a click-only button operable from the
/// keyboard. A press that reaches [`Route::Pointer`] has already passed the
/// hit test for a click, so the node it names is a node that asked to be
/// clicked.
fn activated(event: &InputEvent) -> bool {
    activates(event)
        || matches!(
            event,
            InputEvent::PointerPressed {
                button: PointerButton::Primary,
                ..
            }
        )
}

impl RowSource for Parity {
    fn rows(&mut self, source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        if source != ROW_SOURCE {
            return Vec::new();
        }
        range
            .map(|i| {
                // Keyed by the row's own index, not by its position in this
                // window, so scrolling does not renumber what is on screen.
                Arc::new(on_layer(
                    list_row(
                        format!("row-{i}"),
                        format!("supervisor/worker-{i:05}"),
                        self.selected_row == Some(i),
                    ),
                    1,
                ))
            })
            .collect()
    }
}

impl App for Parity {
    fn view(&mut self) -> ViewNode {
        let columns = ViewNode::new(NodeKind::Grid, "columns")
            .with_props(Props {
                columns: vec![
                    TrackSize::Weight { weight: 1.0 },
                    TrackSize::Weight { weight: 1.0 },
                ],
                column_spacing: sp("spacing.xl"),
                row_spacing: sp("spacing.xl"),
                ..Props::default()
            })
            .child(Self::column(
                "left",
                sp("spacing.xl"),
                vec![
                    Self::scripts_card(),
                    self.controls_card(),
                    Self::form_card(),
                ],
            ))
            .child(Self::column(
                "right",
                sp("spacing.xl"),
                vec![
                    // First, and not by accident: this card holds the node
                    // with no painter, and the undrawn tile in the band above
                    // it is the counter that reports it. A hosted node
                    // scrolled below the fold is clipped away before the paint
                    // pass can report it at all.
                    Self::hosted_card(),
                    Self::text_card(),
                    Self::status_card(),
                    Self::layout_card(),
                    Self::collection_card(),
                ],
            ));

        let mut page = Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            row_spacing: sp("spacing.2xl"),
            padding: Some(inset("spacing.3xl", "spacing.2xl")),
            ..Props::default()
        };
        page.tokens.insert("background".into(), tok("surface.base"));

        let root = ViewNode::new(NodeKind::Grid, "root")
            .with_props(page)
            .child(Self::masthead())
            .child(self.telemetry())
            .child(Self::rule("masthead-rule"))
            .child(columns);

        // The page scrolls. Without this the root is offered exactly the
        // viewport's height and divides it among its regions, so adding a card
        // does not make the page longer — it makes every existing card
        // shorter. The page fill lives on the scroll, not only on the content
        // inside it: a page shorter than the viewport would otherwise leave
        // the rest of the frame painted by whatever cleared it.
        let mut scroll = Props {
            axis: Some(Axis::Vertical),
            overscan: Some(64.0),
            ..Props::default()
        };
        scroll
            .tokens
            .insert("background".into(), tok("surface.base"));
        let page = ViewNode::new(NodeKind::Scroll, "page")
            .with_props(scroll)
            .child(root);

        // Surfaces sit beside the page, not inside it. They are anchored to
        // the viewport, and a viewport-anchored thing inside scrolling content
        // is placed in the wrong coordinate space. `Overlay` is the container
        // for this: every child gets the container's own proposal, z-order by
        // child order.
        let mut shell = ViewNode::new(NodeKind::Overlay, "shell")
            .child(page)
            .child(Self::toast());
        if let Some(modal) = self.modal() {
            shell = shell.child(modal);
        }
        shell
    }

    fn handle(&mut self, event: &InputEvent, route: &Route, _frame: Option<&PetrifiedFrame>) {
        // Before anything, including `last_event`. See `Parity::frozen`: while
        // this page is being captured it must move by not one pixel, and
        // `last_event` is drawn through `note`, which wraps — a longer echo
        // could take a second line and paint below the declared region that
        // exists to excuse it.
        if self.frozen {
            return;
        }
        let node = match route {
            Route::Pointer { node } | Route::Keyboard { node } | Route::Raw { node } => {
                node.clone()
            }
            // Same reasoning as `gallery.rs`'s `App::handle`: this example's
            // `Host` never produces `Reserved` (it routes through
            // `route_with_surfaces`, not `route_with_reserved`), so this arm
            // does not fire today. Named rather than folded into `Unrouted`
            // so it cannot start silently doing the wrong thing later.
            Route::Reserved { chord } => {
                self.last_event = format!("reserved ({chord})");
                return;
            }
            Route::Unrouted { reason } => {
                self.last_event = format!("unrouted ({reason})");
                return;
            }
        };
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
            "modal-close" | "modal-confirm" => self.modal = false,
            // Wraps rather than saturating: a bar stuck at full is a control
            // that stops proving anything after the fourth click.
            "bump" => {
                self.progress = if self.progress >= 1.0 {
                    0.0
                } else {
                    self.progress + 0.1
                }
            }
            "tab-native" => self.tab = 0,
            "tab-web" => self.tab = 1,
            "tab-both" => self.tab = 2,
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

    fn take_changes(&mut self) -> ChangeSet {
        // This page rebuilds its whole tree every pass, so `All` is the only
        // honest answer. Naming individual nodes while handing back fresh
        // `Arc`s is exactly the under-declaration a debug build panics on.
        ChangeSet::All
    }
}

// ---------------------------------------------------------------------------
// The registered painter
// ---------------------------------------------------------------------------

/// Draw a segmented meter inside `ctx.rect`, filled to `fraction`.
///
/// Returns whether it drew anything, which is the whole contract: a registered
/// painter that draws nothing must return `false` so its name still lands in
/// the undrawn set, rather than the frame reading complete over a placement
/// nobody painted. Three ways that happens here — a theme with no ink, a theme
/// with no spacing ramp, and a rect too small to segment — and none of them is
/// defensive: each is a real theme or layout state this page can be put in.
///
/// Every colour *and every gap* comes from `ctx.tokens`, the same source the
/// rest of the frame painted with, so the meter follows the theme instead of
/// carrying a palette of its own. Nothing here means anything by hue: a filled
/// segment is ink, an empty one is an outline in the border tone, and the
/// count of filled segments is the channel a reader who cannot separate the
/// two still gets.
fn paint_meter(painter: &egui::Painter, ctx: &CustomPaintCtx<'_>, fraction: f32) -> bool {
    // `border.subtle`, not `text.muted`. An empty segment below is drawn with
    // `rect_stroke` -- it is an outline, not a fill, which is what makes it a
    // component boundary and not ink. It bound `text.muted` until 2026-08-25;
    // at 10.73:1 on the card, the *empty* half of this meter was drawn as
    // loudly as the full half, which is the one thing a meter must not do.
    let (Some(ink), Some(rule)) = (
        ctx.tokens.color("text.primary"),
        ctx.tokens.color("border.subtle"),
    ) else {
        return false;
    };
    let (Some(pad), Some(gap)) = (
        ctx.tokens.spacing("spacing.xs"),
        ctx.tokens.spacing("spacing.2xs"),
    ) else {
        return false;
    };

    let plot = ctx.rect.shrink(pad);
    if plot.width() <= 0.0 || plot.height() <= 0.0 {
        return false;
    }

    // Ten segments, so the fraction is legible by counting rather than by
    // judging a length against an edge.
    const SEGMENTS: usize = 10;
    #[allow(clippy::cast_precision_loss)]
    let width = (plot.width() - gap * (SEGMENTS as f32 - 1.0)) / SEGMENTS as f32;
    if width <= 0.0 {
        return false;
    }

    // One device pixel, whatever the scale is. A hairline declared in logical
    // units is two pixels on a 2x display, which reads as a border rather than
    // as an outline.
    let hairline = (1.0 / ctx.scale.factor()).max(0.5);
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    let filled = (fraction.clamp(0.0, 1.0) * SEGMENTS as f32).round() as usize;
    for i in 0..SEGMENTS {
        #[allow(clippy::cast_precision_loss)]
        let left = plot.left() + (width + gap) * i as f32;
        let cell = egui::Rect::from_min_size(
            egui::pos2(left, plot.top()),
            egui::vec2(width, plot.height()),
        );
        if i < filled {
            painter.rect_filled(cell, 0.0, ink);
        } else {
            painter.rect_stroke(
                cell,
                0.0,
                egui::Stroke::new(hairline, rule),
                egui::StrokeKind::Inside,
            );
        }
    }
    true
}

/// Register the meter painter, reading `fraction` from a shared cell.
///
/// A function rather than an inline closure at each call site, because the
/// window, the browser and the test suite must register the *same* painter — a
/// test that registered a simpler one would prove that some painter draws, not
/// that this page's does.
fn register_meter(painters: &mut CustomPainters, fraction: std::rc::Rc<std::cell::Cell<f32>>) {
    painters.register(CUSTOM_KIND, move |painter, ctx| {
        paint_meter(painter, ctx, fraction.get())
    });
}

/// The swatch's pixels: a checkerboard whose two squares differ in lightness,
/// crossed by a left-to-right ramp.
///
/// Generated rather than embedded so this example carries no binary asset and
/// no image decoder — the registry takes already-decoded RGBA8, which is the
/// whole of the "decode" step flat pixels need. Lightness and structure, not
/// hue: the picture stays readable to someone who cannot separate red from
/// green, and it is obvious at a glance whether it uploaded or not.
fn swatch_pixels() -> ImagePixels {
    let mut rgba = Vec::with_capacity(SWATCH_EDGE * SWATCH_EDGE * 4);
    for y in 0..SWATCH_EDGE {
        for x in 0..SWATCH_EDGE {
            let square = ((x / 8) + (y / 8)) % 2 == 0;
            #[allow(clippy::cast_possible_truncation)]
            let ramp = ((x * 120) / SWATCH_EDGE) as u8;
            let level = if square { 40 + ramp } else { 120 + ramp };
            rgba.extend_from_slice(&[level, level, level, 255]);
        }
    }
    ImagePixels::new(SWATCH_EDGE, SWATCH_EDGE, rgba)
}

// ---------------------------------------------------------------------------
// Wiring, shared by every entry point
// ---------------------------------------------------------------------------

/// The window over the host: one pass, then the readout for the next one.
///
/// `Host` already implements `eframe::App`, and this delegates to that impl
/// rather than calling `Host::pass` directly — the point of a parity canary is
/// to compile the path the crate actually ships, not an adjacent one. What it
/// adds is the read-back afterwards, which is the only place the accessors on
/// `Host` are exercised for the wasm target.
struct ParityWindow {
    host: Host<Parity>,
    /// The progress value the registered painter reads, shared with the
    /// painter because a registered painter is a `'static` closure that cannot
    /// borrow the application.
    meter: std::rc::Rc<std::cell::Cell<f32>>,
}

impl eframe::App for ParityWindow {
    /// Pin the layout surface to [`PARITY_VIEWPORT`], whatever the host gave.
    ///
    /// `eframe` calls this on the native and the web target both, after it has
    /// filled `screen_rect` from the window or the canvas and before `egui`
    /// reads it, so this one override is what makes the two hosts lay out the
    /// same page — and makes each host lay out the same page twice running
    /// under a window manager that resizes it.
    ///
    /// `safe_area_insets` is zeroed alongside it because `content_rect`, which
    /// is what `Host::pass` measures against, is `screen_rect` minus those
    /// insets: pinning the rect and leaving an inset behind would pin
    /// everything except the number actually used.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(PARITY_VIEWPORT.0, PARITY_VIEWPORT.1),
        ));
        raw_input.safe_area_insets = Some(egui::SafeAreaInsets::default());
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Before the pass, not after. The painter runs *inside* the pass, so a
        // value written afterwards is one frame behind the number the page
        // prints beside it — two readings of one state that can disagree,
        // which is the failure this whole page is built to make visible.
        self.meter.set(self.host.app().progress);
        eframe::App::ui(&mut self.host, ui, frame);
        let readout = read_back(&self.host);
        self.host.app_mut().readout = readout;
    }
}

/// Everything the page prints about the pass that just ran.
///
/// Deliberately reaches through seven different accessors. Each one is a
/// generic method on `Host<Parity>` that is only codegen'd where it is called,
/// so this function is what makes the wasm build type-check and monomorphise
/// them at all.
fn read_back(host: &Host<Parity>) -> Readout {
    let seq = host.frame().map_or(0, |f| f.seq);
    let placements = host.frame().map_or(0, |f| f.placements.len());
    let decision = host.decision();
    let mut readout = Readout {
        seq,
        placements,
        focusables: host.focus().order().len(),
        focused: host.focus().current().map(str::to_owned),
        page_offset: host.state().scroll_offset("/shell/page"),
        repaint: decision.is_some_and(|d| d.repaint),
        undeclared: decision.map_or(0, |d| d.undeclared),
        ambient_clean: host.motion().scheduler().ledger().is_clean(),
        galley_evictions: host.shaper().evictions(),
        ..Readout::default()
    };
    if let Some(report) = host.report() {
        readout.drawn = report.drawn;
        readout.silent = report.silent;
        readout.empty = report.empty;
        readout.customs = report.customs;
        readout.undrawn = report.undrawn.len();
        readout.unresolved = report.unresolved_tokens.len();
        readout.desynced = report.desynced;
    }
    readout
}

/// A window over a fresh page, with the custom kind on the tree registry, the
/// painter on the paint dispatch, and the swatch on the image registry.
///
/// One function, used by the native entry point, the web entry point and every
/// test below, so no two of them can drift into registering different things.
fn build_window(ctx: &egui::Context, presenter: Presenter) -> ParityWindow {
    let app = Parity {
        frozen: frozen(),
        ..Parity::default()
    };
    let meter = std::rc::Rc::new(std::cell::Cell::new(app.progress));
    let mut host = Host::new(ctx, app, presenter);
    host.registry_mut().register_custom_kind(CUSTOM_KIND);
    register_meter(host.painters_mut(), meter.clone());
    host.images_mut().register(SWATCH_SOURCE, swatch_pixels());
    // Both are defaults elsewhere; setting them here is what compiles them for
    // this target. The capacities are the shipped ones, so the page measures
    // the same as a host that never called this.
    host.set_cache_capacities(1024, 1024);
    host.set_reduced_motion(false);
    ParityWindow { host, meter }
}

/// Which shipped theme to present.
///
/// Both themes ship and both must render this page. On the web there is no
/// environment to read, so the browser gets the dark theme — the same default
/// the rest of the crate uses.
fn presenter() -> Presenter {
    #[cfg(target_arch = "wasm32")]
    {
        Presenter::new(dark())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        match std::env::var("PETRA_PARITY_THEME").as_deref() {
            Ok("light") => Presenter::new(light()),
            _ => Presenter::new(dark()),
        }
    }
}

// ---------------------------------------------------------------------------
// SC-009's other half: what this target's font stack can actually draw
// ---------------------------------------------------------------------------

/// The point size the glyph probe measures at.
///
/// Not a style token and deliberately not one: this is the size the
/// *measurement* runs at, not a size anything on the page is drawn at. The
/// probe compares atlas regions, and epaint allocates one region per (face,
/// glyph, metrics, subpixel bin) — so the measurement has to name a size, and
/// naming the body size the page itself uses is the closest thing to what a
/// reader would see.
const GLYPH_PROBE_SIZE: f32 = 16.0;

/// The first line of a healthy report, so a reader (and the lane) can tell a
/// report from a truncated one.
const GLYPH_HEADER: &str = "PETRA-GLYPH 1";

/// Measure this target's font stack against
/// [`gorgon_petra_egui::fonts::script_samples`].
///
/// # Why this builds its own `Context` instead of using the page's
///
/// The probe lays every sample codepoint out on its own, which pulls glyphs
/// into the font atlas that the page never draws. Doing that on the running
/// application's context would grow *its* atlas, and this example is the one
/// the pixel-parity lane compares desktop against web on. A separate context
/// keeps the measurement from touching the picture it is measured beside.
///
/// # Why the answer differs between the two targets, and why that is honest
///
/// [`gorgon_petra_egui::fonts::install_desktop_fallbacks`] reads font *files*
/// off the host. `wasm32-unknown-unknown` has no filesystem, so the browser
/// gets exactly what `eframe`'s `default_fonts` embeds and nothing else. This
/// function therefore reports what the *current target* can draw rather than
/// asserting a number, and the caller decides what that number has to be. A
/// desktop caller that wants the fuller stack installs the fallbacks first;
/// see `tests/text_scripts.rs`.
///
/// # Errors
/// The probe refused to build, which means the detector could not tell a box
/// from a glyph on this stack — reported rather than silently downgraded to
/// "everything rendered".
fn glyph_report() -> Result<String, String> {
    let ctx = Context::default();
    // egui has no fonts at all until a pass has run.
    ctx.run_ui(RawInput::default(), |_| {})
        .drop_without_applying_deltas();
    let probe = GlyphProbe::new(&ctx, FontId::proportional(GLYPH_PROBE_SIZE))?;

    let mut out = format!("{GLYPH_HEADER}\n");
    let mut detail = String::new();
    for (script, sample) in fonts::script_samples() {
        let coverage = probe.coverage(&ctx, script, sample);
        out.push_str(&coverage.report_line());
        out.push('\n');
        detail.push_str(&coverage.gap_report());
    }
    out.push_str(&detail);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Entry points: one per target, and neither one names the other's API
// ---------------------------------------------------------------------------

/// The native entry point: a window, the same shape `gallery.rs` opens.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    // The same measurement the web build publishes into its page, on the
    // target that has a filesystem to install fallback faces from. Printed,
    // not asserted: this file is a canary, and the desktop numbers are held
    // to a floor by `tests/text_scripts.rs`. Printing it on both targets is
    // what keeps the two answers comparable by a reader — SC-009's two halves
    // differ, and a reader who can only see one of them cannot tell by how
    // much.
    match glyph_report() {
        Ok(report) => eprint!("{report}"),
        Err(err) => eprintln!("petra parity: the glyph probe refused to build: {err}"),
    }
    // The handshake `NativeWindow::open` checks. A capture lane that set
    // `PETRA_PARITY_FROZEN` and got an interactive page anyway would compare
    // a window a pointer can move against a canvas nothing can, which is the
    // failure the flag exists to remove — so the lane refuses rather than
    // trusting that the variable arrived under the name it was sent.
    eprintln!(
        "petra parity: frozen={} window={}x{} viewport={}x{}",
        frozen(),
        WINDOW_REQUEST.0 as u32,
        WINDOW_REQUEST.1 as u32,
        PARITY_VIEWPORT.0 as u32,
        PARITY_VIEWPORT.1 as u32
    );
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_REQUEST.0, WINDOW_REQUEST.1]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Petra web parity",
        options,
        Box::new(|cc| Ok(Box::new(build_window(&cc.egui_ctx, presenter())))),
    )
}

/// The web entry point: a canvas, from `examples/parity.html`.
///
/// `eframe::run_native` does not exist on `wasm32` — there is no event loop to
/// own and no window to make. The browser owns both, so the app is handed a
/// canvas and started asynchronously instead.
#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::Cell;

    use wasm_bindgen::prelude::*;

    /// The canvas element `examples/parity.html` declares, and the only thing
    /// this module looks for in the document.
    pub const CANVAS_ID: &str = "petra-parity-canvas";

    /// Where [`publish_glyph_report`] writes SC-009's web measurement, and
    /// where the `petra-parity` lane reads it from.
    ///
    /// Two elements rather than one, the same protocol
    /// `gorgon/xtask/parity-wasm/digests.html` (the `gorgon-xtask` capture
    /// lane, private-monorepo-only) uses for the digest report: a
    /// reader that polled the body alone could not tell "the module has not
    /// run yet" from "the module ran and measured nothing", and those are
    /// opposite answers. Both are `display: none` in the host page — this
    /// example is the page a pixel-parity comparison is run against, and a
    /// measurement that changed the picture it is measured beside would be
    /// worse than no measurement.
    const GLYPH_OUT_ID: &str = "petra-glyph-out";

    /// The element that says whether [`GLYPH_OUT_ID`] is finished: `pending`
    /// from the host page, then `ok` or `error` from here.
    const GLYPH_STATE_ID: &str = "petra-glyph-state";

    /// Measure this target's font stack and write the answer into the page.
    ///
    /// Runs before the app starts and on its own `egui::Context`, so nothing
    /// it lays out reaches the canvas the lane screenshots. A failure is
    /// published as `error` with the reason rather than left as `pending`:
    /// the difference between "the probe refused to build" and "the module
    /// never loaded" is the difference between a product defect and a broken
    /// harness, and a lane that cannot tell them apart reports the wrong one.
    fn publish_glyph_report() {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            fail("petra parity: no document, so no glyph report was published");
            return;
        };
        let (state, body) = match super::glyph_report() {
            Ok(report) => ("ok", report),
            Err(err) => ("error", err),
        };
        for (id, text) in [(GLYPH_OUT_ID, body.as_str()), (GLYPH_STATE_ID, state)] {
            match document.get_element_by_id(id) {
                Some(element) => element.set_text_content(Some(text)),
                None => fail(&format!(
                    "petra parity: no element with id `{id}`; the host page must declare it \
                     for the SC-009 glyph measurement to be readable"
                )),
            }
        }
    }

    thread_local! {
        /// Whether [`start`] has already run.
        ///
        /// `wasm-bindgen` may call `main` itself when the module initialises,
        /// and `parity.html` also calls [`start_parity`] explicitly so the
        /// page works whether or not it does. Two runners fighting over one
        /// canvas is a real failure — the second `start` tears the first one
        /// down mid-frame — so the second call is a no-op rather than a race.
        static STARTED: Cell<bool> = const { Cell::new(false) };
    }

    /// Report a failure where a browser user will actually see it.
    ///
    /// The console, not a `log` crate: this example carries no logger, and a
    /// web entry point that fails silently is the exact theatre a parity
    /// canary exists to prevent — a blank canvas with a clean build behind it
    /// looks identical to a page that has not finished loading.
    fn fail(message: &str) {
        web_sys::console::error_1(&JsValue::from_str(message));
    }

    /// The canvas this app mounts on, or `None` with the reason already
    /// reported.
    fn canvas() -> Option<web_sys::HtmlCanvasElement> {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            fail("petra parity: no document — is this running outside a browser page?");
            return None;
        };
        let Some(element) = document.get_element_by_id(CANVAS_ID) else {
            fail(&format!(
                "petra parity: no element with id `{CANVAS_ID}`; the host page must \
                 declare <canvas id=\"{CANVAS_ID}\"></canvas>"
            ));
            return None;
        };
        match element.dyn_into::<web_sys::HtmlCanvasElement>() {
            Ok(canvas) => Some(canvas),
            Err(_) => {
                fail(&format!(
                    "petra parity: the element with id `{CANVAS_ID}` is not a <canvas>"
                ));
                None
            }
        }
    }

    /// Mount the page on the canvas and start running it. Safe to call twice;
    /// the second call does nothing.
    pub fn start() {
        if STARTED.with(Cell::get) {
            return;
        }
        // Before the canvas is even looked for: the measurement is about this
        // build's font stack, not about the app, and a page whose canvas is
        // missing should still be able to answer SC-009.
        publish_glyph_report();
        // The flag is claimed only once a canvas is actually in hand. Setting
        // it first would make a missing canvas permanent: `main` runs before
        // the host page's own call, so a page that mounts its canvas late
        // would report the error once and then refuse every retry, with the
        // console line already scrolled away.
        let Some(canvas) = canvas() else { return };
        STARTED.with(|started| started.set(true));

        let runner = eframe::WebRunner::new();
        wasm_bindgen_futures::spawn_local(async move {
            let started = runner
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| {
                        Ok(Box::new(super::build_window(
                            &cc.egui_ctx,
                            super::presenter(),
                        )))
                    }),
                )
                .await;
            if let Err(err) = started {
                fail(&format!("petra parity: eframe failed to start: {err:?}"));
            }
        });
    }

    /// The named export `parity.html` calls after the module loads.
    ///
    /// An explicit export rather than trusting `wasm-bindgen` to call `main`:
    /// whether a bin's `main` becomes the module's start function is a detail
    /// of the bundler, and a canary that only runs under one bundler is not
    /// much of a canary. [`STARTED`] makes both routes safe.
    #[wasm_bindgen]
    pub fn start_parity() {
        start();
    }
}

/// The web entry point. Does the same thing `web::start_parity` does, so the
/// page runs whether the bundler calls `main` or the host page calls the
/// export.
#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(test)]
mod tests {
    use super::{CUSTOM_KIND, GLYPH_HEADER, MISSING_SOURCE, Parity, build_window, glyph_report};
    use egui::{Context, Pos2, RawInput};
    use gorgon_petra::geom::Point;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton, Route};
    use gorgon_petra::token::{Presenter, dark, light};
    use gorgon_petra::tree::{NodeKind, ViewNode};
    use gorgon_petra_egui::host::App;

    /// The viewport size `main` asks for.
    ///
    /// The tests run at it deliberately. An `egui::Context` with a default
    /// `RawInput` reports a screen about 10 000pt tall, and at that size
    /// nothing here ever clips or scrolls — so a headless suite that takes the
    /// default is not exercising the layout anyone actually sees.
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

    /// One settled pass over a fresh page under `presenter`.
    fn settled(presenter: Presenter) -> (Context, super::ParityWindow) {
        let ctx = headless();
        let mut window = build_window(&ctx, presenter);
        ctx.run_ui(sized(RawInput::default()), |_| window.host.pass(&ctx))
            .drop_without_applying_deltas();
        (ctx, window)
    }

    /// Walk every node of `tree`, collecting the kinds it declares.
    fn kinds(tree: &ViewNode, out: &mut std::collections::HashSet<NodeKind>) {
        out.insert(tree.kind);
        for child in &tree.children {
            kinds(child, out);
        }
    }

    /// The page's own tree is acceptable.
    ///
    /// First, because a refused tree is replaced wholesale by the host's
    /// refusal view, and every other test here would then be asserting against
    /// an error message instead of against this page.
    #[test]
    fn the_parity_tree_is_accepted() {
        let tree = Parity::default().view();
        let mut registry = gorgon_petra::tree::Registry::with_vocabulary(
            gorgon_petra::token::standard_vocabulary(),
        );
        registry.register_custom_kind(CUSTOM_KIND);
        gorgon_petra::anim::shipped_registry().declare_into(&mut registry);
        if let Err(errors) = gorgon_petra::tree::validate(&tree, &registry) {
            panic!("the parity page's own tree is not acceptable: {errors}");
        }
    }

    /// T060's structural claim: every container kind is on the page.
    ///
    /// `Collection` is the one kind that does not appear in the *placed* frame
    /// as itself — it materialises rows — so this asserts against the declared
    /// tree rather than against placements. Eleven of the twelve are declared
    /// with the modal shut; the twelfth, `Image`, is here twice.
    #[test]
    fn every_node_kind_is_declared() {
        let mut app = Parity {
            modal: true,
            ..Parity::default()
        };
        let tree = app.view();
        let mut found = std::collections::HashSet::new();
        kinds(&tree, &mut found);
        for kind in [
            NodeKind::Stack,
            NodeKind::Grid,
            NodeKind::Overlay,
            NodeKind::Scroll,
            NodeKind::Collection,
            NodeKind::Surface,
            NodeKind::Text,
            NodeKind::Image,
            NodeKind::Input,
            NodeKind::Spacer,
            NodeKind::Separator,
            NodeKind::Custom,
        ] {
            assert!(
                found.contains(&kind),
                "the parity page must declare {kind:?}; it is the whole subject of \
                 T060 that no kind is left out of the web build"
            );
        }
    }

    /// The page renders under both shipped themes with nothing unresolved and
    /// no slot the painter does not know.
    ///
    /// Two themes, one loop, because "it works in dark" is half a claim: the
    /// light theme assigns different colours to the same names, and a name
    /// only one theme declares would resolve in one and land in the unresolved
    /// set in the other.
    #[test]
    fn the_page_renders_under_both_shipped_themes() {
        for (what, presenter) in [
            ("dark", Presenter::new(dark())),
            ("light", Presenter::new(light())),
        ] {
            let (_ctx, window) = settled(presenter);
            let report = window.host.report().expect("a paint report");
            assert!(report.is_complete(), "{what}: {report:?}");
            assert!(!report.desynced, "{what}: {report:?}");
            assert!(
                report.unresolved_tokens.is_empty(),
                "{what}: every token this page binds must exist in the shipped theme: \
                 {:?}",
                report.unresolved_tokens
            );
            assert!(
                report.unknown_slots.is_empty(),
                "{what}: the page must not bind a slot the painter does not know: {:?}",
                report.unknown_slots
            );
        }
    }

    /// The registered painter draws, and the unregistered image source does
    /// not — the two halves of the hosted-content contract, asserted together
    /// so a change that silently disabled the dispatch could not pass by
    /// making both sides quiet.
    #[test]
    fn the_registered_painter_draws_and_the_missing_image_is_named() {
        let (_ctx, window) = settled(Presenter::new(dark()));
        let report = window.host.report().expect("a paint report");
        assert_eq!(
            report.customs, 1,
            "the meter placement must have been drawn by the registered painter: \
             {report:?}"
        );
        assert!(
            report.images >= 1,
            "the registered swatch must have reached the GPU: {report:?}"
        );
        // `image:` prefixed, because the undrawn set holds custom kinds under
        // their own names too and a bare source could collide with one. The
        // prefix is the paint pass's, not this file's — asserting on the whole
        // string is what makes this a test of the contract rather than of a
        // substring.
        assert!(
            report.undrawn.contains(&format!("image:{MISSING_SOURCE}")),
            "the unregistered source must be named in the undrawn set: {:?}",
            report.undrawn
        );
    }

    /// The font stack a browser gets, measured here where no browser is
    /// needed.
    ///
    /// `wasm32-unknown-unknown` has no filesystem, so
    /// `fonts::install_desktop_fallbacks` cannot run there and the web build
    /// gets exactly what `eframe`'s `default_fonts` embeds. A plain
    /// `egui::Context` on this machine has the same four faces and nothing
    /// else, which makes this test the offline half of the same question the
    /// `petra-parity` lane asks a real browser: **which scripts can the web
    /// build draw without boxes?**
    ///
    /// Measured on 2026-08-24 against egui 0.36.1's bundled faces:
    /// `latin=26/26`, `emoji=5/5`, and `cjk`, `arabic`, `devanagari` and
    /// `hebrew` at `0` of 15, 10, 11 and 7. The assertion below is that
    /// split, not those counts: the counts move whenever
    /// `fonts::script_samples` is edited, and the split is the fact the lane
    /// depends on. `gorgon/xtask/src/parity/glyphs.rs` (private-monorepo-only)
    /// holds the same split as the browser-side requirement, and its
    /// `WEB_REQUIRED_SCRIPTS` is
    /// what goes red if this one ever changes without that one changing too.
    #[test]
    fn the_stack_a_browser_gets_draws_latin_and_emoji_and_nothing_else() {
        let report = glyph_report().expect("the probe builds on the bundled font stack");
        assert!(report.starts_with(GLYPH_HEADER), "{report}");

        let mut complete = Vec::new();
        let mut empty = Vec::new();
        for line in report.lines().filter_map(|l| l.strip_prefix("GLYPH ")) {
            let (script, counts) = line.split_once('=').expect("GLYPH <script>=<n>/<n>");
            let (covered, total) = counts.split_once('/').expect("<covered>/<total>");
            let covered: usize = covered.parse().expect("a covered count");
            let total: usize = total.parse().expect("a total count");
            assert!(total > 0, "script {script:?} measured no codepoints");
            if covered == total {
                complete.push(script.to_owned());
            } else if covered == 0 {
                empty.push(script.to_owned());
            } else {
                panic!(
                    "script {script:?} is partly covered ({covered}/{total}) on the bundled \
                     stack; the lane's two-way split in xtask/src/parity/glyphs.rs assumes \
                     every script is all or nothing and needs re-measuring:\n{report}"
                );
            }
        }
        assert_eq!(
            complete,
            ["latin", "emoji"],
            "the scripts a browser can draw have changed:\n{report}"
        );
        assert_eq!(
            empty,
            ["cjk", "arabic", "devanagari", "hebrew"],
            "the scripts a browser cannot draw have changed:\n{report}"
        );
    }

    /// The bundle bar's button id, read out of a real settled frame rather
    /// than spelled here, so this test follows the page instead of pinning a
    /// path that could stop existing while the test stayed green.
    fn bump_node(window: &super::ParityWindow) -> String {
        window
            .host
            .frame()
            .expect("a settled frame")
            .placements
            .as_slice()
            .iter()
            .map(|placement| placement.id.clone())
            .find(|id| id.ends_with("/bump"))
            .expect("the page declares a `bump` button")
    }

    fn primary_press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::default(),
        }
    }

    /// A frozen page moves by nothing on a click that moves the live one.
    ///
    /// This is the 2026-08-24 `petra-parity` failure in a unit test. The
    /// desktop half of that lane is a real window on a live X server: a click
    /// that lands on `bump` while the window is up moves the bundle bar, the
    /// headless browser canvas has no such click, and the comparator reports
    /// a 209/255 delta at the bar's own pixels. Both halves are asserted here
    /// — the interactive page must still move, or this test would pass
    /// against a page that ignores input for some other reason.
    #[test]
    fn a_frozen_page_does_not_move_on_the_click_that_would_move_it() {
        let (_ctx, window) = settled(Presenter::new(dark()));
        let node = bump_node(&window);
        let route = Route::Pointer { node: node.clone() };
        let frame = window.host.frame().expect("a settled frame");
        let start = Parity::default().progress;

        let mut interactive = Parity::default();
        interactive.handle(&primary_press(), &route, Some(frame));
        assert!(
            (interactive.progress - start).abs() > f32::EPSILON,
            "the interactive page must still act on a click, or this test proves nothing \
             about the frozen one: {start} -> {}",
            interactive.progress
        );

        let mut frozen = Parity {
            frozen: true,
            ..Parity::default()
        };
        frozen.handle(&primary_press(), &route, Some(frame));
        assert_eq!(
            frozen.progress, start,
            "a frozen page moved on a click; the desktop capture is no longer a pure \
             function of the build and the parity comparison is a coin toss"
        );
        assert_eq!(
            frozen.last_event, "",
            "a frozen page must not move even the echo: `last_event` is drawn through a \
             wrapping text node, and a second line would paint below the declared region \
             that exists to excuse the first"
        );
    }

    /// Only a real opt-in freezes the page.
    #[test]
    fn frozen_from_env_reads_only_a_real_opt_in() {
        use std::ffi::OsString;
        assert!(!super::frozen_from_env(None));
        assert!(!super::frozen_from_env(Some(OsString::from(""))));
        assert!(!super::frozen_from_env(Some(OsString::from("0"))));
        assert!(super::frozen_from_env(Some(OsString::from("1"))));
    }
}
