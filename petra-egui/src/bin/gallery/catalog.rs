//! The Carbon catalog window: one inventory row per page.
//!
//! `examples/gallery.rs` is the working page over the thirteen components
//! Petra ships today. This binary is the 42-row Carbon inventory. Wave 1
//! rows (Wave 1 and Wave 2) are built constructors. The rest say so in
//! words rather than drawing a stand-in.
//!
//! This file is the chrome only: the roster, the open page, the index pane,
//! Prev/Next and the page header. Every row's own page — its state, node
//! ids, body and handler — is a module under `page/`, reached through
//! [`Page`].

use std::ops::Range;
use std::sync::Arc;

use egui::ViewportBuilder;
use gorgon_petra::component::{button, heading, list_row, on_layer, text};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::input::{InputEvent, KeyCode, PointerButton, Route, activates};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::{Theme, ThemeMode};
use gorgon_petra::tree::{
    AxisConstraint, Constraints, InsetRefs, Interaction, NodeKind, Props, Role, TrackSize, ViewNode,
};
use gorgon_petra_egui::host::{App, Host, default_presenter};

use crate::cell::Cell;
use crate::page::common::{column, path_has, row, sp, tok, wrapped};
use crate::page::{self, Page};

/// Inner size an interactive catalog window asks for. Not a layout pin: the
/// host lays out at whatever the compositor grants.
pub(crate) const WINDOW: [f32; 2] = [1200.0, 900.0];

const PREV: &str = "prev";
const NEXT: &str = "next";
/// Key prefix for a row in the left index (`idx-36` is Toggle).
const IDX: &str = "idx-";
/// Width of the scrolling index pane, logical units.
pub(crate) const INDEX_WIDTH: f32 = 240.0;

/// Symmetric padding from two spacing steps, horizontal first — the same
/// argument order [`InsetRefs::symmetric`] uses.
fn pad(horizontal: &str, vertical: &str) -> InsetRefs {
    InsetRefs::symmetric(tok(horizontal), tok(vertical))
}

/// One row of the 42-row index pane.
///
/// [`list_row`] with the pane's own density put back over it. The index is
/// the gallery's chrome, not a Carbon contained list on display, and the two
/// want different things: Carbon's item is `padding: $spacing-04 $spacing-05`
/// so a list of three reads as a list of three, and this pane has 42 rows to
/// fit in 900 logical units. They were one decision until `list_row` was
/// given Carbon's real inset for row 7, at which point six rows fell off the
/// bottom of the pane and two of this module's own tests went red.
///
/// Only the padding is restated. The four selection-state fills stay in the
/// component, where a fifth state would be added once rather than twice.
fn index_row(key: String, label: String, selected: bool) -> ViewNode {
    let mut node = list_row(key, label, selected);
    node.props.padding = Some(pad("spacing-03", "spacing-02"));
    node
}

/// Whether `event` should act on the node it routed to.
///
/// [`activates`] answers only for Enter or Space on the focused node. A
/// press that reaches [`Route::Pointer`] has already passed hit-test for
/// [`gorgon_petra::tree::Interaction::Click`], so acting on it is the point.
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

fn path_idx(node: &str) -> Option<u8> {
    node.split('/')
        .find_map(|part| part.strip_prefix(IDX)?.parse().ok())
}

/// The catalog application: the paging chrome over the 42 pages.
///
/// Holds no control state of its own. Every control lives in the page
/// module that draws it (`page/`), and this struct reaches it only through
/// [`Page`].
pub struct Catalog {
    roster: Vec<Cell>,
    /// Index into `roster` of the open row.
    page: usize,
    /// One page per row, from [`page::all`], found by [`Page::row`].
    pages: Vec<Box<dyn Page>>,
    /// Dismissals the host reported for the event now being handled,
    /// held until the open page has seen the press. See
    /// [`Page::dismissed`] for why the order is inverted from the host's.
    pending_dismiss: Vec<String>,
}

impl Default for Catalog {
    fn default() -> Self {
        let roster = Cell::roster();
        let open = roster
            .iter()
            .position(|cell| cell.row.component == "Toggle")
            .expect("the Toggle row is in the inventory");
        Self {
            roster,
            page: open,
            pages: page::all(),
            pending_dismiss: Vec::new(),
        }
    }
}

/// Test-only page selection by inventory row name.
///
/// `Catalog`'s fields are private and its `Default` opens the Toggle page, so
/// a driver outside this module has no way to say which page it wants. This
/// is that way, kept test-only because the shipped window pages by input and
/// never by name.
#[cfg(test)]
impl Catalog {
    /// Open the page whose inventory row is named `component`.
    ///
    /// # Panics
    /// If no row carries that name. The message lists every row name, because
    /// the usual cause is a spelling that differs from Carbon's by one capital
    /// letter and is otherwise invisible at a call site.
    pub(crate) fn on_page(component: &str) -> Self {
        let mut app = Self::default();
        app.page = app
            .roster
            .iter()
            .position(|cell| cell.row.component == component)
            .unwrap_or_else(|| {
                panic!(
                    "no inventory row is named {component:?}. Rows:\n  {}",
                    app.roster
                        .iter()
                        .map(|cell| cell.row.component)
                        .collect::<Vec<_>>()
                        .join("\n  ")
                )
            });
        app
    }

    /// The open page's body as the page builds it now, unseated.
    ///
    /// A driver reads state back from here when the frame cannot carry it:
    /// a placement has an id, a rect and its semantics, but not the text a
    /// leaf paints, so "the panel now reads South" is a question for the
    /// tree. Never a substitute for the picture — a tree that says so and a
    /// picture that does not is the defect this catalog exists to catch.
    pub(crate) fn open_page_body(&self) -> ViewNode {
        self.page_body_raw()
    }
}

impl Catalog {
    pub(crate) fn current(&self) -> &Cell {
        &self.roster[self.page]
    }

    /// Where in `pages` the open row's page is, or `None` when the row is
    /// unbuilt and there is no page to show.
    ///
    /// # Panics
    /// If the row is built and no module in [`page::all`] claims it. That
    /// is a wiring error `page::tests` catches before it can happen live,
    /// and drawing nothing in its place would hide it.
    fn open_page_index(&self) -> Option<usize> {
        let cell = self.current();
        if !cell.is_built() {
            return None;
        }
        let index = self
            .pages
            .iter()
            .position(|page| page.row() == cell.row.component)
            .unwrap_or_else(|| {
                panic!(
                    "row {} ({}) is built but no module in page::all claims it",
                    cell.row.number, cell.row.component
                )
            });
        Some(index)
    }

    fn open_page(&self) -> Option<&dyn Page> {
        let index = self.open_page_index()?;
        Some(self.pages[index].as_ref())
    }

    fn open_page_mut(&mut self) -> Option<&mut dyn Page> {
        let index = self.open_page_index()?;
        Some(self.pages[index].as_mut())
    }

    fn prev(&mut self) {
        self.page = if self.page == 0 {
            self.roster.len() - 1
        } else {
            self.page - 1
        };
    }

    fn next(&mut self) {
        self.page = (self.page + 1) % self.roster.len();
    }

    fn go_to(&mut self, number: u8) {
        if let Some(i) = self.roster.iter().position(|c| c.row.number == number) {
            self.page = i;
        }
    }

    fn index_pane(&self) -> ViewNode {
        let rows: Vec<ViewNode> = self
            .roster
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let key = format!("{IDX}{}", cell.row.number);
                let label = format!("{:>2}  {}", cell.row.number, cell.row.component);
                index_row(key, label, i == self.page)
            })
            .collect();
        let mut list = column("rows", sp("spacing.2xs"), rows);
        // Without this, `rows` is a `Grid` whose single column is a weighted
        // track (fills `INDEX_WIDTH`) but whose *children* still align
        // `Start` in it — each row is measured at its own text's width, not
        // stretched to the column, so "1 Accordion" comes out narrower than
        // "8 Content switcher" and the sidebar stair-steps.
        list.props.align = Some(Align::Stretch);
        let mut scroll = ViewNode::new(NodeKind::Scroll, "index")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                overscan: Some(64.0),
                ..Props::default()
            })
            // Without `Interaction::Scroll`, `route`'s `hit_test` finds no
            // placement at the pointer that accepts a wheel event — not
            // even this node itself — and the event is dropped before it
            // ever reaches a scroll offset (`input.rs`'s `hit_test` checks
            // `semantics.actions`, which is `node.interactions`, which
            // `with_props` never sets).
            .interactive(Role::Scroll, "Component index", &[Interaction::Scroll]);
        scroll
            .props
            .tokens
            .insert("background".into(), tok("surface.raised"));
        scroll = scroll.child(list);
        scroll.with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(INDEX_WIDTH),
                max: Some(INDEX_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        })
    }

    /// Re-seat a mounted component's fills against the card it sits on.
    ///
    /// Components that mean "disappear into the page ground" bind
    /// `surface.base`, and `component::on_layer` is the documented way a
    /// caller re-seats that against the surface it actually placed the
    /// control on. This catalog puts every component inside a
    /// `surface.raised` card and never called it, so `15-link.png` drew a
    /// dark patch behind "Open the spec" and `18-menu.png` drew one behind
    /// each menu item: base-coloured fills sitting on a raised ground.
    ///
    /// `on_layer` re-seats one node, so this walks. Depth is a flat 1 for
    /// the whole subtree rather than counting nesting, which is honest
    /// about what it is: the card is one step up from the page, and a
    /// component that nests its own surfaces deeper needs the `Surface`
    /// node kind `on_layer`'s own doc names as the real fix.
    fn seated(mut node: ViewNode, depth: usize) -> ViewNode {
        node.children = node
            .children
            .into_iter()
            .map(|child| Arc::new(Self::seated(ViewNode::clone(&child), depth)))
            .collect();
        on_layer(node, depth)
    }

    /// The card's own contents, re-seated; the card itself keeps its fill.
    fn seat_card(mut card: ViewNode) -> ViewNode {
        card.children = card
            .children
            .into_iter()
            .map(|child| Arc::new(Self::seated(ViewNode::clone(&child), 1)))
            .collect();
        card
    }

    fn page_body(&self) -> ViewNode {
        Self::seat_card(self.page_body_raw())
    }

    fn page_body_raw(&self) -> ViewNode {
        let cell = self.current();
        match self.open_page() {
            None => {
                let letter = cell.row.slice.letter().to_ascii_lowercase();
                wrapped(
                    "unbuilt",
                    format!(
                        "This inventory row is unbuilt. No Petra component exists for {}. \
                         Anatomy, variants, sizes, and states live in slice-{letter}.md. \
                         This page does not draw a stand-in.",
                        cell.row.component
                    ),
                )
            }
            Some(page) => page.body(),
        }
    }
}

impl RowSource for Catalog {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Catalog {
    fn tick(&mut self, now: f64) {
        if let Some(page) = self.open_page_mut() {
            page.tick(now);
        }
    }

    /// Forward the open page's theme choice, converted here.
    ///
    /// The page names a [`ThemeMode`] and the chrome turns it into a
    /// [`Theme`], so `token::light()` and `token::dark()` are named in one
    /// place rather than in whichever page happens to offer the switch. The
    /// host publishes it; see `App::theme_request`.
    fn theme_request(&mut self) -> Option<Theme> {
        let mode = self.open_page_mut()?.theme_request()?;
        Some(match mode {
            ThemeMode::Light => gorgon_petra::token::light(),
            ThemeMode::Dark => gorgon_petra::token::dark(),
        })
    }

    fn view(&mut self) -> ViewNode {
        let cell = self.current();
        let title = format!("{}  {}", cell.row.number, cell.row.component);
        let slice = format!("slice {}", cell.row.slice.letter());
        let status = if cell.is_built() { "BUILT" } else { "UNBUILT" };

        let mut main = column(
            "main",
            sp("spacing.xl"),
            vec![
                heading("title", title),
                column(
                    "meta",
                    sp("spacing.sm"),
                    vec![text("slice", slice), text("status", status)],
                ),
                row(
                    "nav",
                    sp("spacing.md"),
                    vec![button(PREV, "Prev"), button(NEXT, "Next")],
                ),
                self.page_body(),
            ],
        );
        main.props.padding = Some(pad("spacing.xl", "spacing.lg"));

        let mut main_scroll = ViewNode::new(NodeKind::Scroll, "main-scroll")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                overscan: Some(64.0),
                ..Props::default()
            })
            .interactive(Role::Scroll, "Page content", &[Interaction::Scroll]);
        main_scroll = main_scroll.child(main);

        // Both rows of `shell` are `Weight`, not the implicit `FitContent`
        // an unspecified `rows` would give: a `FitContent` row takes its
        // content's own natural height, and this cell's content is 42 nav
        // rows — taller than the window — so the cell (and the scroll pane
        // inside it) would just grow to fit all 42 rather than being handed
        // a bounded viewport to scroll *within*. `Weight` makes the row's
        // height come from what `page` offers instead, which is what lets
        // `index_pane`'s `Scroll` report a real, shorter-than-content
        // viewport (`layout/scroll.rs`: "an `Exact` offer is honoured
        // exactly") and therefore actually have something to scroll.
        let mut shell = ViewNode::new(NodeKind::Grid, "shell").with_props(Props {
            columns: vec![
                TrackSize::Fixed { value: INDEX_WIDTH },
                TrackSize::Weight { weight: 1.0 },
            ],
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            column_spacing: sp("spacing.md"),
            ..Props::default()
        });
        shell = shell.child(self.index_pane()).child(main_scroll);
        shell
            .props
            .tokens
            .insert("background".into(), tok("surface.base"));

        // `page` used to be the `Scroll`: one region, `shell` (nav and main
        // together) as its content, so scrolling moved both — the nav list
        // could never scroll on its own, only drag the whole page's main
        // content along with it. A `Scroll`'s own doc is explicit that it
        // offers its child `Unbounded` on the scrolling axis
        // (`layout/scroll.rs`), which is exactly what defeated `shell`'s
        // `Weight` row above: a row cannot divide an unbounded offer. `page`
        // is now a plain 1x1 `Grid` that passes the window's own bounded
        // size straight through to `shell`, and `index_pane` and
        // `main_scroll` each scroll their own content independently.
        let mut page = Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            ..Props::default()
        };
        page.tokens.insert("background".into(), tok("surface.base"));
        ViewNode::new(NodeKind::Grid, "page")
            .with_props(page)
            .child(shell)
    }

    fn handle(&mut self, event: &InputEvent, route: &Route, frame: Option<&PetrifiedFrame>) {
        self.route_event(event, route, frame);
        // The dismissals the host reported for this same event, delivered
        // now that the page has seen the press. `Page::dismissed` says why
        // this order and not the host's.
        let ids = std::mem::take(&mut self.pending_dismiss);
        if !ids.is_empty()
            && let Some(page) = self.open_page_mut()
        {
            page.dismissed(&ids);
        }
    }

    fn dismissed(&mut self, ids: &[String]) {
        self.pending_dismiss.extend_from_slice(ids);
    }

    fn focus_changed(&mut self, focused: Option<&str>) {
        if let Some(page) = self.open_page_mut() {
            page.focused(focused);
        }
    }

    fn take_changes(&mut self) -> ChangeSet {
        // The catalog rebuilds its whole tree every pass, so `All` is the
        // only honest answer. Naming individual nodes while handing back
        // fresh `Arc`s is the under-declaration
        // `ReuseState::verify_declaration` panics on in a debug build.
        ChangeSet::All
    }
}

impl Catalog {
    /// [`App::handle`] proper: the chrome's own shortcuts, then the open
    /// page's gesture hook, then the activation filter, then the page's
    /// handler, then the chrome's Prev/Next. Split from `handle` so every
    /// early return here still lands on the dismissal delivery there.
    fn route_event(&mut self, event: &InputEvent, route: &Route, frame: Option<&PetrifiedFrame>) {
        // Whether this keystroke is aimed at something that accepts text.
        // The chrome pages on `[`, `]` and the arrows, and a text field has
        // to be able to *contain* those characters — a page shortcut that
        // eats what the operator is typing is a worse bug than no shortcut.
        // So the field wins whenever the route names a `Role::TextInput`,
        // and the shortcut keeps working everywhere else.
        let editing = match route {
            Route::Keyboard { node } => frame.is_some_and(|frame| {
                frame
                    .placements
                    .iter()
                    .any(|p| p.id == *node && p.semantics.role == Some(Role::TextInput))
            }),
            _ => false,
        };
        if !editing
            && let InputEvent::Key {
                key, pressed: true, ..
            } = event
        {
            match key {
                KeyCode::Left | KeyCode::Char('[') => {
                    self.prev();
                    return;
                }
                KeyCode::Right | KeyCode::Char(']') => {
                    self.next();
                    return;
                }
                _ => {}
            }
        }

        let node = match route {
            Route::Pointer { node } | Route::Keyboard { node } => node.as_str(),
            // A pointer move or pointer-exit that landed on nothing is still
            // news to a page whose surface is revealed by hover: the pointer
            // has left the trigger for empty ground, and the tooltip has to
            // hear that or it never closes. The empty path matches no key
            // (`Page::gesture`), so no other page acts on it. Every other
            // unrouted event is dropped here as before.
            Route::Unrouted { .. } => match event {
                InputEvent::PointerMoved { .. } | InputEvent::PointerLeft => "",
                _ => return,
            },
        };
        // A gesture reaches the open page before the activation filter
        // below, with the frame it was routed against: a pointer move under
        // capture is not an activation, and a page turning it into a value
        // needs a rect the route does not carry (`Page::gesture`). The route
        // named a node, so the frame is there; `App::handle`'s doc says
        // `None` comes only with an `Unrouted` route, and a first-pass
        // unrouted move has no frame to offer.
        if let Some(frame) = frame
            && self
                .open_page_mut()
                .is_some_and(|page| page.gesture(event, node, frame))
        {
            return;
        }
        // Typing is not an activation, so it never passes the filter below
        // and never reached a page. Deliver it here, and return either way:
        // an edit must never fall through to the chrome's navigation, or a
        // keystroke meant for a field would page the catalog.
        if matches!(event, InputEvent::Text(_))
            || matches!(
                event,
                InputEvent::Key {
                    key: KeyCode::Backspace,
                    pressed: true,
                    ..
                }
            )
        {
            if !node.is_empty()
                && let Some(page) = self.open_page_mut()
            {
                page.handle(event, node);
            }
            return;
        }
        if node.is_empty() || !activated(event) {
            return;
        }
        // A press on a child (knob, label, state text) still names that
        // child in the route. Matching only the last segment dropped
        // every click that was not exactly on the interactive root, so
        // a focused toggle looked dead after the first hit.
        if let Some(number) = path_idx(node) {
            self.go_to(number);
            return;
        }
        // The open page is asked before the chrome's own Prev/Next, not
        // after. Pagination's own next button routes as `pager/next` and
        // `NEXT` is `next`, so a chrome-first `path_has` would page the
        // catalog on a press meant for the pager. That is the precedence
        // the one `else if` chain had before the split into `page/`, kept
        // rather than re-decided.
        if self
            .open_page_mut()
            .is_some_and(|page| page.handle(event, node))
        {
            return;
        }
        if path_has(node, PREV) {
            self.prev();
        } else if path_has(node, NEXT) {
            self.next();
        }
    }
}

/// Move keyboard focus onto the sidebar row for the page now on screen, if
/// that row is currently visible.
///
/// [`gorgon_petra_egui::focus_caret::FocusCaret`] — the flying bar in the
/// index pane — tracks keyboard focus, not [`Catalog::page`]. Prev/Next and
/// the `[`/`]` shortcuts change the page without ever routing a pointer or
/// Tab event to the sidebar, so left alone the bar stays wherever it last
/// was (row 1, on a fresh [`Host`], since [`gorgon_petra::focus::FocusTree`]
/// defaults to the first focusable node) while the row's own highlight jumps
/// to the new page — two channels for "which page is open," disagreeing.
/// The operator is red-green colour blind: a fill a shade lighter is the
/// weak channel, the bar's position is the one that has to be trusted, so
/// the two must never disagree. This re-seats focus onto the open page's row
/// every time it is called; callers only call it when [`Catalog::page`]
/// actually changed, so a user tabbing around inside the page body is left
/// alone the rest of the time.
///
/// Looks the placement id up by its `idx-N` suffix rather than assuming the
/// full path, so a later reshuffle of `index_pane`'s own nesting cannot
/// silently turn this into a no-op.
///
/// **Does nothing for a row past the sidebar's fold.**
/// [`gorgon_petra::focus::FocusTree::focus`] refuses anything not focusable
/// this frame (`focus/mod.rs`'s doc: a placement is focusable only when
/// [`Placement::is_visible`] holds), and calling it here for such a row
/// would simply return its error. `gorgon-petra-egui`'s `Host` now carries a
/// scroll-into-view primitive (`Host::step_focus`, built for Tab), but nothing
/// wires it into *this* call path: Prev/Next changes `Catalog::page` without
/// ever routing a keystroke through `Host::deliver_input`, which is the only
/// place `step_focus` runs. Writing an offset here directly and hoping it
/// lands would still race `paint.rs`'s `PaintReport::blind_focus` — a
/// focused-but-invisible placement is exactly what that check exists to
/// catch (`gorgon-petra-egui/src/host.rs`'s `debug_assert!(report.is_complete())`
/// fires on it) — because a bare offset write is not synchronized with
/// petrify's own reconciliation pass the way `step_focus`'s deferred
/// `pending_focus` is. Giving Prev/Next the same primitive is future work,
/// out of `catalog.rs`'s own scope; see `.agents/notes/implemented/bug-fix/
/// 2026-09-03-no-input-path-writes-a-scroll-offset.md`.
pub(crate) fn seat_index_focus(host: &mut Host<Catalog>) {
    let Some(frame) = host.frame() else {
        return;
    };
    let suffix = format!("/{IDX}{}", host.app().current().row.number);
    let Some(row) = frame
        .placements
        .iter()
        .find(|p| p.id.ends_with(suffix.as_str()))
    else {
        return;
    };
    if !row.is_visible() {
        return;
    }
    let id = row.id.clone();
    let _ = host.focus_mut().focus(&id);
}

/// Wraps the host so the first pass can steal OS focus for Tab, and so the
/// flying focus bar can be re-seated exactly when the open page changes.
struct CatalogWindow {
    host: Host<Catalog>,
    grabbed_focus: bool,
    /// The page [`seat_index_focus`] last ran for. `None` until the first
    /// pass has placed a frame for [`seat_index_focus`] to search.
    focus_seated_for: Option<usize>,
}

impl eframe::App for CatalogWindow {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.safe_area_insets = Some(egui::SafeAreaInsets::default());
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.grabbed_focus {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.grabbed_focus = true;
        }
        self.host.pass_in_window(&ctx, frame);
        let page = self.host.app().page;
        if self.focus_seated_for != Some(page) {
            seat_index_focus(&mut self.host);
            self.focus_seated_for = Some(page);
        }
    }
}

/// Open the catalog window. `DISPLAY` is the caller's problem.
///
/// # Errors
/// Whatever [`eframe::run_native`] reports: no display, a wgpu failure, or
/// a compositor that refuses the window.
pub fn run() -> eframe::Result<()> {
    let mut options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(WINDOW)
            .with_active(true),
        ..eframe::NativeOptions::default()
    };
    // Latency 1, not eframe's `HIGH_THROUGHPUT` 2.
    //
    // The operator, row 30: *"Slider is laggy when moving compared to the
    // cursor under it. They should render in the same way."* The compositor
    // draws his pointer with no queue at all, so anything this window queues
    // is a gap he can see, and a slider handle is the one control whose whole
    // job is to sit under that pointer.
    //
    // `HIGH_THROUGHPUT` was chosen for the older example gallery so a caret
    // hop had a frame queued ahead and never stalled. That trade is right for
    // a *self-driven* animation, which has nothing to be measured against,
    // and wrong for *pointer tracking*, which is measured against the
    // hardware cursor on every frame. `SurfaceConfig::LOW_LATENCY` is
    // eframe's own recommendation for "GUIs with very little (or no) extra
    // GPU work", which is what a 2D component catalog is; the 160 ms caret
    // hop is about ten frames at 60 Hz and needs the host to keep asking for
    // repaints, which `FocusCaret::is_moving` already does, not a queued
    // frame.
    //
    // One frame of latency is left and cannot be removed by an application
    // that draws its own control: the pass that reads the move is the pass
    // that paints it, and that painting reaches the screen at the next vsync.
    options.wgpu_options = eframe::WgpuConfiguration::default()
        .with_surface_config(eframe::SurfaceConfig::LOW_LATENCY);
    eframe::run_native(
        "Petra Carbon catalog",
        options,
        Box::new(|cc| {
            Ok(Box::new(CatalogWindow {
                host: Host::new(&cc.egui_ctx, Catalog::default(), default_presenter()),
                grabbed_focus: false,
                focus_seated_for: None,
            }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{Catalog, Cell, IDX, NEXT, PREV, WINDOW, seat_index_focus};
    use crate::cell::Content;
    use crate::page::common::find;
    use egui::{Context, Pos2, RawInput};
    use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use gorgon_petra::geom::{Point, Size};
    use gorgon_petra::input::{InputEvent, KeyCode, Modifiers, PointerButton, Route};
    use gorgon_petra::testing::{Harness, validated_with};
    use gorgon_petra::token::{ThemeMode, standard_vocabulary};
    use gorgon_petra::tree::{Registry, ViewNode};
    use gorgon_petra_egui::host::{App, Host, default_presenter};
    use gorgon_petra_egui::inject::{Action, Target, inject_action};

    /// The frame a route into `app` would have been computed against: its
    /// own view, petrified at the catalog window. The tests below fabricate
    /// routes by path rather than by hit test, so what `handle` needs is a
    /// frame from this tree, not a route that was actually found in it.
    fn frame_of(app: &mut Catalog) -> PetrifiedFrame {
        let root = app.view();
        // The shipped transition registry, as `Host::new` installs it: the
        // toggle page names `toggle-knob`, and a registry without it refuses
        // the tree.
        let mut registry = Registry::with_vocabulary(standard_vocabulary());
        gorgon_petra::anim::shipped_registry().declare_into(&mut registry);
        let mut harness = Harness::new();
        let viewport = Viewport::new(Size::new(WINDOW[0], WINDOW[1]), ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn tree_contains_text(node: &ViewNode, needle: &str) -> bool {
        node.props
            .text
            .as_deref()
            .is_some_and(|text| text.contains(needle))
            || node
                .children
                .iter()
                .any(|child| tree_contains_text(child, needle))
    }

    fn press(app: &mut Catalog, tail: &str) {
        let frame = frame_of(app);
        app.handle(
            &InputEvent::PointerPressed {
                pos: Point::ZERO,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            &Route::Pointer {
                node: format!("/page/root/{tail}"),
            },
            Some(&frame),
        );
    }

    fn key(app: &mut Catalog, code: KeyCode) {
        let frame = frame_of(app);
        app.handle(
            &InputEvent::Key {
                key: code,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            &Route::Unrouted {
                reason: "catalog test",
            },
            Some(&frame),
        );
    }

    /// One Tab keystroke as the platform reports it — the real path a
    /// physical keyboard or a driver's synthetic key takes, unlike [`key`]
    /// above (which calls [`Catalog::handle`] directly and never reaches
    /// [`gorgon_petra_egui::host::Host::deliver_input`]'s own traversal).
    fn tab_key(shift: bool) -> RawInput {
        let mut input = RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: if shift {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            },
        });
        input
    }

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

    fn step(ctx: &Context, host: &mut Host<Catalog>, input: RawInput) {
        ctx.run_ui(sized(input), |_| host.pass(ctx))
            .drop_without_applying_deltas();
    }

    fn accepted(tree: &ViewNode) {
        let mut registry = gorgon_petra::tree::Registry::with_vocabulary(
            gorgon_petra::token::standard_vocabulary(),
        );
        gorgon_petra::anim::shipped_registry().declare_into(&mut registry);
        if let Err(errors) = gorgon_petra::tree::validate(tree, &registry) {
            panic!("the catalog's own tree is not acceptable: {errors}");
        }
    }

    #[test]
    fn the_catalog_opens_on_the_toggle_page() {
        let app = Catalog::default();
        assert_eq!(app.current().row.component, "Toggle");
        assert_eq!(app.current().row.number, 36);
        assert!(app.current().is_built());
        let tree = Catalog::default().view();
        assert!(tree_contains_text(&tree, "36  Toggle"));
        assert!(tree_contains_text(&tree, "slice F"));
        assert!(tree_contains_text(&tree, "BUILT"));
        // The page body is the Toggle page's: `page::toggle` owns these ids.
        assert!(find(&tree, "toggle-default-off").is_some());
        assert!(find(&tree, "toggle-default-on").is_some());
        assert!(find(&tree, "toggle-sm-off").is_some());
        assert!(find(&tree, "toggle-sm-on").is_some());
    }

    #[test]
    fn prev_and_next_wrap_through_all_forty_two_rows() {
        let mut app = Catalog::default();
        assert_eq!(app.current().row.component, "Toggle");
        press(&mut app, NEXT);
        assert_eq!(app.current().row.component, "Toggletip");
        press(&mut app, PREV);
        assert_eq!(app.current().row.component, "Toggle");
        press(&mut app, PREV);
        assert_eq!(app.current().row.component, "Tile");

        app.page = 0;
        press(&mut app, PREV);
        assert_eq!(app.current().row.number, 42);
        press(&mut app, NEXT);
        assert_eq!(app.current().row.number, 1);
    }

    #[test]
    fn left_and_right_keys_page_the_catalog() {
        let mut app = Catalog::default();
        key(&mut app, KeyCode::Right);
        assert_eq!(app.current().row.component, "Toggletip");
        key(&mut app, KeyCode::Left);
        assert_eq!(app.current().row.component, "Toggle");
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.current().row.component, "Toggletip");
        key(&mut app, KeyCode::Char('['));
        assert_eq!(app.current().row.component, "Toggle");
    }

    #[test]
    fn an_unbuilt_page_names_the_slice_file_and_does_not_fake_a_component() {
        // All 42 inventory rows are built constructors as of this change
        // (`cell::BUILT` covers every row), so there is no live unbuilt row
        // left for the catalog to page to. The `Content::Unbuilt` arm in
        // `page_body` and the "UNBUILT" status text are still real code
        // paths (a future inventory addition lands unbuilt first), so this
        // test drives them directly by substituting an unbuilt `Cell` for a
        // real one rather than asserting on a fake component.
        let mut app = Catalog::default();
        let row = app.roster[0].row;
        assert_eq!(row.component, "Accordion");
        app.roster[0] = Cell {
            row,
            content: Content::Unbuilt,
        };
        app.page = 0;
        assert!(!app.current().is_built());
        let tree = app.view();
        assert!(tree_contains_text(&tree, "1  Accordion"));
        assert!(tree_contains_text(&tree, "UNBUILT"));
        assert!(tree_contains_text(&tree, "slice-a.md"));
        assert!(tree_contains_text(&tree, "unbuilt"));
        assert!(find(&tree, "toggle-default-off").is_none());
        accepted(&tree);
    }

    #[test]
    fn the_toggle_page_tree_is_accepted() {
        accepted(&Catalog::default().view());
    }

    #[test]
    fn every_built_page_tree_is_accepted() {
        let mut app = Catalog::default();
        for index in 0..app.roster.len() {
            app.page = index;
            if !app.current().is_built() {
                continue;
            }
            accepted(&app.view());
        }
    }

    #[test]
    fn the_index_lists_every_inventory_row() {
        let tree = Catalog::default().view();
        assert!(find(&tree, "idx-1").is_some());
        assert!(find(&tree, "idx-36").is_some());
        assert!(find(&tree, "idx-42").is_some());
        assert!(
            find(&tree, "idx-36").unwrap().semantics.selected,
            "the open page is selected in the index"
        );
        assert!(!find(&tree, "idx-1").unwrap().semantics.selected);
    }

    #[test]
    fn clicking_an_index_row_opens_that_page() {
        let mut app = Catalog::default();
        press(&mut app, "idx-1");
        assert_eq!(app.current().row.component, "Accordion");
        press(&mut app, "idx-36");
        assert_eq!(app.current().row.component, "Toggle");
    }

    /// `seat_index_focus` must land the flying focus bar on the row for the
    /// page a fresh [`Host`] actually opened, not row 1 — the default
    /// [`gorgon_petra::focus::FocusTree`] falls back to on an untouched
    /// tree. A caret still parked on row 1 while row 36's fill is selected
    /// is exactly the two-channels-disagree defect this exists to close.
    #[test]
    fn seat_index_focus_moves_the_caret_off_row_one_onto_the_open_page() {
        // Page 23, Pagination — the exact row the audit's own before-picture
        // named ("the highlight is correctly on 23 Pagination but the blue
        // bar is still under 1 Accordion") and, unlike the default Toggle
        // page, still inside the sidebar's un-scrolled fold, so this proves
        // the fix without also depending on scrolling (`seat_index_focus`
        // does not scroll; see its doc).
        let mut app = Catalog::default();
        app.go_to(23);
        let ctx = headless();
        let mut host = Host::new(&ctx, app, default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert!(
            host.focus()
                .current()
                .is_some_and(|id| id.ends_with("/idx-1")),
            "a fresh host defaults focus to the first nav row, got {:?}",
            host.focus().current()
        );

        seat_index_focus(&mut host);

        assert!(
            host.focus()
                .current()
                .is_some_and(|id| id.ends_with("/idx-23")),
            "focus must move to Pagination's own row (idx-23), the page \
             this host actually opened; got {:?}",
            host.focus().current()
        );
    }

    /// `index_pane`'s `Scroll` must have a *bounded* viewport of its own to
    /// scroll within — shorter than 42 rows — rather than growing to fit
    /// all of them, which is what let row 42 through the window's bottom
    /// edge with nothing to scroll. Before `shell`'s `rows` track was made
    /// `Weight` (this file's own fix), this node measured at its full
    /// content height instead.
    #[test]
    fn the_index_pane_has_a_bounded_viewport_shorter_than_its_content() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Catalog::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());

        let frame = host.frame().expect("a completed pass has a frame");
        let index = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/index"))
            .expect("the index pane is placed");
        let last_row = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}42")))
            .expect("row 42 is placed");
        assert!(
            index.rect.h < last_row.rect.y + last_row.rect.h,
            "the index pane's own viewport ({:?}) must be shorter than its \
             content (row 42 ends at {}), or it has nothing to scroll",
            index.rect,
            last_row.rect.y + last_row.rect.h
        );
        assert!(
            !last_row.is_visible(),
            "row 42 must start outside that viewport, or this test proves \
             nothing"
        );
    }

    /// Writing a new offset into [`gorgon_petra_egui::host::Host::state_mut`]
    /// — the mechanism [`seat_index_focus`] itself uses once a row is
    /// scrolled out of view — must bring row 42 into the index pane's
    /// viewport on the next pass. This is *not* a test of the mouse wheel:
    /// [`gorgon_petra_egui::inject::Action::Scroll`] was tried here first
    /// and never moved `state().scroll_offsets` at all — nothing in
    /// `gorgon-petra-egui`'s `host.rs` translates a routed
    /// [`gorgon_petra::input::InputEvent::Scroll`] into a scroll-offset
    /// write (`grep scroll_offsets` across the crate turns up only test
    /// harnesses setting it directly, this one included). That gap is
    /// outside `catalog.rs`; see this audit's report.
    #[test]
    fn writing_the_index_pane_scroll_offset_reveals_row_forty_two() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Catalog::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());

        let index_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with("/index"))
            .expect("the index pane is placed")
            .id
            .clone();
        // A fresh host's focus defaults to row 1, which this offset is
        // about to scroll out of view. Move focus to the always-visible
        // Next button first: a placement that is focused this pass and then
        // goes invisible next pass is exactly `PaintReport::blind_focus`,
        // which `host.rs`'s own `debug_assert!(report.is_complete())` would
        // otherwise catch on the very next `step`.
        let next_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{NEXT}")))
            .expect("the Next button is placed")
            .id
            .clone();
        host.focus_mut()
            .focus(&next_id)
            .expect("the Next button is focusable");
        host.state_mut().scroll_offsets.insert(index_id, 100_000.0);
        step(&ctx, &mut host, RawInput::default());

        let row_42 = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}42")))
            .expect("row 42 is still placed after scrolling");
        assert!(
            row_42.is_visible(),
            "a large enough offset must bring row 42 into the index pane's \
             viewport"
        );
    }

    /// The real path a physical mouse takes — [`inject_action`]'s
    /// `Action::Scroll` over the index pane — now does what the test above
    /// did by writing `state_mut().scroll_offsets` directly.
    /// `Host::apply_scroll` is the piece that used to be missing; see its
    /// doc in `gorgon-petra-egui`'s `host.rs`.
    #[test]
    fn wheeling_over_the_index_pane_writes_a_scroll_offset_and_shifts_the_frame() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Catalog::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());

        let index_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with("/index"))
            .expect("the index pane is placed")
            .id
            .clone();
        let next_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{NEXT}")))
            .expect("the Next button is placed")
            .id
            .clone();
        // Same reason as the direct-write test above: row 1 holds focus by
        // default, and a modest scroll can carry it out of view.
        host.focus_mut()
            .focus(&next_id)
            .expect("the Next button is focusable");
        let row_1_y_before = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}1")))
            .expect("row 1 is placed")
            .rect
            .y;

        let mut raw = RawInput::default();
        // egui's own doc on `Event::MouseWheel`: a *negative* Y moves the
        // content *up* — scrolling down the list, toward row 42.
        inject_action(
            &mut host,
            &Target::NodeId(index_id.clone()),
            &Action::Scroll {
                delta: Size::new(0.0, -200.0),
            },
            &mut raw,
        )
        .expect("the index pane resolves a point to scroll from");
        step(&ctx, &mut host, raw);

        assert_eq!(
            host.state().scroll_offsets.get(&index_id).copied(),
            Some(200.0),
            "a 200-unit wheel delta must write a 200-unit offset — nothing \
             this small should have clamped"
        );
        let row_1_y_after = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}1")))
            .expect("row 1 is still placed")
            .rect
            .y;
        assert!(
            row_1_y_after < row_1_y_before,
            "the index pane's content must have shifted up: row 1 was at \
             {row_1_y_before}, is now at {row_1_y_after}"
        );
    }

    /// The offset a wheel writes clamps at both ends: it cannot scroll past
    /// row 42, and scrolling back up cannot go negative.
    #[test]
    fn wheeling_past_either_end_of_the_index_pane_clamps_the_offset() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Catalog::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());

        let index_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with("/index"))
            .expect("the index pane is placed")
            .id
            .clone();
        let next_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{NEXT}")))
            .expect("the Next button is placed")
            .id
            .clone();
        host.focus_mut()
            .focus(&next_id)
            .expect("the Next button is focusable");

        let mut down = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(index_id.clone()),
            &Action::Scroll {
                delta: Size::new(0.0, -1_000_000.0),
            },
            &mut down,
        )
        .unwrap();
        step(&ctx, &mut host, down);
        let offset_at_bottom = host
            .state()
            .scroll_offsets
            .get(&index_id)
            .copied()
            .expect("a huge downward wheel must have written an offset");
        let row_42 = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}42")))
            .expect("row 42 is placed");
        assert!(
            row_42.is_visible(),
            "the far end of a million-unit scroll must show the last row"
        );

        // Scrolling the same enormous amount again must land on the exact
        // same offset — proof this is a clamp, not a number so large the
        // picture merely happens to still show row 42.
        let mut down_again = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(index_id.clone()),
            &Action::Scroll {
                delta: Size::new(0.0, -1_000_000.0),
            },
            &mut down_again,
        )
        .unwrap();
        step(&ctx, &mut host, down_again);
        assert_eq!(
            host.state().scroll_offsets.get(&index_id).copied(),
            Some(offset_at_bottom),
            "a second identical over-scroll must not move the offset past \
             its clamp"
        );

        let mut up = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(index_id.clone()),
            &Action::Scroll {
                delta: Size::new(0.0, 1_000_000.0),
            },
            &mut up,
        )
        .unwrap();
        step(&ctx, &mut host, up);
        let offset_at_top = host
            .state()
            .scroll_offsets
            .get(&index_id)
            .copied()
            .unwrap_or(0.0);
        assert_eq!(
            offset_at_top, 0.0,
            "a million-unit scroll back up must clamp at zero, not go negative"
        );
        let row_1 = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}1")))
            .expect("row 1 is placed");
        assert!(
            row_1.is_visible(),
            "scrolling back to zero must show row 1 again"
        );
    }

    /// FR-025's reachability promise, for the one part of the catalog large
    /// enough to hide behind its own scroll: Tab from row 30 — the last row
    /// visible on a fresh load — must reach row 31, and the index pane must
    /// have scrolled by the time it does.
    #[test]
    fn tab_from_the_last_visible_row_scrolls_the_index_pane_to_reveal_row_thirty_one() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Catalog::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());

        let row_30_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}30")))
            .expect("row 30 is placed and visible on a fresh load")
            .id
            .clone();
        host.focus_mut()
            .focus(&row_30_id)
            .expect("row 30 is focusable before any scroll");
        assert!(
            host.frame()
                .unwrap()
                .placements
                .iter()
                .find(|p| p.id.ends_with(&format!("/{IDX}31")))
                .is_some_and(|row_31| !row_31.is_visible()),
            "row 31 must start outside the viewport, or this test proves \
             nothing"
        );

        step(&ctx, &mut host, tab_key(false));

        assert!(
            host.focus()
                .current()
                .is_some_and(|id| id.ends_with(&format!("/{IDX}31"))),
            "Tab from row 30 must land on row 31, got {:?}",
            host.focus().current()
        );
        let row_31 = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}31")))
            .expect("row 31 is placed after the scroll");
        assert!(
            row_31.is_visible(),
            "row 31 must be on screen once Tab has moved to it, not merely \
             focused off screen"
        );

        // One more empty pass: `Host::step_focus` defers the actual
        // `FocusTree::focus` call to after this pass's own reconciliation
        // (`Host::pending_focus`'s doc), so the ring itself lands one frame
        // later — the same "seat it after petrify" shape
        // `enter_open_modal` already uses. This pass must settle without
        // tripping `PaintReport::blind_focus`'s `debug_assert`.
        step(&ctx, &mut host, RawInput::default());
        assert!(
            host.focus()
                .current()
                .is_some_and(|id| id.ends_with(&format!("/{IDX}31"))),
            "focus must still be on row 31 after the settling pass"
        );
    }

    /// Tab alone must be able to walk every one of the 42 index rows,
    /// scrolling the sidebar as it goes — not just the one hop across the
    /// fold the test above pins down.
    #[test]
    fn tab_walks_all_forty_two_index_rows_scrolling_as_it_goes() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Catalog::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert!(
            host.focus()
                .current()
                .is_some_and(|id| id.ends_with(&format!("/{IDX}1"))),
            "a fresh host starts focus on row 1"
        );

        for expected in 2..=42 {
            step(&ctx, &mut host, tab_key(false));
            let suffix = format!("/{IDX}{expected}");
            assert!(
                host.focus()
                    .current()
                    .is_some_and(|id| id.ends_with(suffix.as_str())),
                "Tab number {expected} landed on {:?}, not row {expected}",
                host.focus().current()
            );
            let row = host
                .frame()
                .unwrap()
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix.as_str()))
                .unwrap_or_else(|| panic!("row {expected} is placed"));
            assert!(
                row.is_visible(),
                "row {expected} must be visible once Tab reaches it"
            );
        }
    }

    /// Rasterizes the index pane after a real wheel scroll has carried it to
    /// its far end, so a human or an agent can look at the picture rather
    /// than trust the frame record — the same discipline
    /// `every_built_page_rasterizes_to_more_than_one_colour` holds every
    /// catalog page to. Set `PETRA_SHOT_DIR` to write the PNG there.
    #[test]
    fn the_scrolled_index_pane_rasterizes_with_row_forty_two_on_screen() {
        let dir = std::env::var_os("PETRA_SHOT_DIR").map(std::path::PathBuf::from);
        if let Some(dir) = &dir {
            std::fs::create_dir_all(dir).expect("shot dir");
        }

        let ctx = headless();
        let mut host = Host::new(&ctx, Catalog::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());

        let index_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with("/index"))
            .expect("the index pane is placed")
            .id
            .clone();
        let next_id = host
            .frame()
            .unwrap()
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{NEXT}")))
            .expect("the Next button is placed")
            .id
            .clone();
        host.focus_mut()
            .focus(&next_id)
            .expect("the Next button is focusable");

        let mut raw = RawInput::default();
        inject_action(
            &mut host,
            &Target::NodeId(index_id),
            &Action::Scroll {
                delta: Size::new(0.0, -1_000_000.0),
            },
            &mut raw,
        )
        .unwrap();

        host.set_reduced_motion(true);
        let output = ctx.run_ui(sized(raw), |_| host.pass(&ctx));
        let frame = host.frame().expect("the scrolling pass produced a frame");
        let row_42 = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(&format!("/{IDX}42")))
            .expect("row 42 is placed");
        assert!(
            row_42.is_visible(),
            "the frame about to be photographed must actually show row 42, \
             or the picture proves nothing"
        );

        let mut shooter = gorgon_petra_testkit::snapshot::Snapshotter::new();
        let shot = shooter
            .capture(&ctx, &output, frame, None)
            .expect("capture refused");
        output.drop_without_applying_deltas();

        if let Some(dir) = &dir {
            std::fs::write(dir.join("scrolled-index-pane.png"), &shot.png).expect("write shot");
        }

        let image = image::load_from_memory(&shot.png)
            .expect("shot is not a PNG")
            .to_rgba8();
        let mut seen: std::collections::HashSet<[u8; 4]> = std::collections::HashSet::new();
        for px in image.pixels() {
            seen.insert(px.0);
            if seen.len() > 1 {
                break;
            }
        }
        assert!(
            seen.len() > 1,
            "the scrolled index pane rasterized to a single flat colour"
        );
    }

    /// A routed press inside the open page reaches that page's handler,
    /// through the real full path a physical pointer produces and on a
    /// descendant (the knob), not the interactive root. This is the
    /// delegation seam the split into `page/` introduced; that the page
    /// itself flips is `page::toggle`'s own test.
    #[test]
    fn a_routed_press_inside_the_open_page_reaches_that_page() {
        let mut app = Catalog::default();
        let knob =
            "/page/shell/main-scroll/main/states/toggles/toggle-default-off/appearance/track/knob";
        assert!(
            !find(&app.view(), "toggle-default-off")
                .unwrap()
                .semantics
                .selected
        );
        let frame = frame_of(&mut app);
        app.handle(
            &InputEvent::PointerPressed {
                pos: Point::ZERO,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            &Route::Pointer { node: knob.into() },
            Some(&frame),
        );
        assert!(
            find(&app.view(), "toggle-default-off")
                .unwrap()
                .semantics
                .selected,
            "the chrome accepted the press but never handed it to the open page"
        );
    }

    /// Pagination's own next button routes as `.../pager/next`, and the
    /// chrome's Next is `next`. The page must win, or a press on the pager
    /// turns the page of the catalog instead. Pins the precedence the split
    /// preserved from the old `else if` chain.
    #[test]
    fn a_press_on_the_pagers_own_next_does_not_page_the_catalog() {
        let mut app = Catalog::on_page("Pagination");
        press(&mut app, "pager/pages/pager/next");
        assert_eq!(app.current().row.component, "Pagination");
        press(&mut app, NEXT);
        assert_eq!(app.current().row.component, "Popover");
    }

    #[test]
    fn a_headless_pass_over_the_toggle_page_does_not_open_a_window() {
        let ctx = headless();
        let mut host = Host::new(&ctx, Catalog::default(), default_presenter());
        step(&ctx, &mut host, RawInput::default());
        assert!(
            host.frame().is_some(),
            "a headless pass must still produce a petrified frame"
        );
    }

    /// `every_built_page_tree_is_accepted` above only proves a page's tree is
    /// structurally valid (`tree::validate`) — it never paints a pixel. A
    /// page whose interaction-state tokens do not resolve at rest (Accordion's
    /// item headers used to bind only `background@hover`, with no resting
    /// `background`; Modal's close button had the same defect) sailed through
    /// that gate and every other gate, and only surfaced live, as
    /// `gorgon-petra-egui::host::Host::pass`'s
    /// `report.is_complete()` `debug_assert!` firing the first time an
    /// operator opened the page. This test runs the real paint pass over
    /// every built page and names every one that leaves a placement
    /// undrawn-but-declared — the gate that should have caught the bug this
    /// test guards against.
    ///
    /// `catch_unwind` is required, not merely convenient: in a debug build
    /// `Host::pass` panics via that same `debug_assert!` before
    /// `Host::report()` is ever populated for the offending pass, so the
    /// panic message is the only way to read that pass's counts. A release
    /// build compiles the `debug_assert!` out, so the `Ok` arm below checks
    /// `PaintReport::is_complete` directly — either way a silent placement
    /// fails this test and names its page.
    #[test]
    fn every_built_page_paints_with_nothing_silent() {
        let mut offenders: Vec<String> = Vec::new();
        for index in 0..Catalog::default().roster.len() {
            let app = Catalog {
                page: index,
                ..Catalog::default()
            };
            if !app.current().is_built() {
                continue;
            }
            let component = app.current().row.component.to_string();
            let ctx = headless();
            let mut host = Host::new(&ctx, app, default_presenter());
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                step(&ctx, &mut host, RawInput::default());
            }));
            match outcome {
                Ok(()) => {
                    let report = host
                        .report()
                        .expect("a completed pass without a panic records a report");
                    if !report.is_complete() {
                        offenders.push(format!(
                            "{index} {component}: {} drawn + {} clipped + {} empty + \
                             {} silent of {} placement(s), desynced={}",
                            report.drawn,
                            report.skipped_clipped,
                            report.empty,
                            report.silent,
                            report.placements,
                            report.desynced,
                        ));
                    }
                }
                Err(payload) => {
                    let msg = payload
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                        .unwrap_or_else(|| "<non-string panic payload>".to_owned());
                    offenders.push(format!("{index} {component}: {msg}"));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these catalog pages paint at least one silent placement:\n{}",
            offenders.join("\n")
        );
    }

    /// Rasterize every catalog page on the GPU and assert it is not blank.
    ///
    /// Everything else in this file, and every frame-level test the component
    /// audit added, reads the *frame record*: rects, tokens, semantics. None
    /// of it looks at a pixel. A page whose record is perfect and whose
    /// painter draws one flat rectangle passes all of them. This test is the
    /// one that cannot: it runs the real pass, hands the tessellated output to
    /// a real `wgpu` device through
    /// [`gorgon_petra_testkit::snapshot::Snapshotter`], and reads the PNG back.
    ///
    /// The assertion is deliberately weak and therefore honest: a page must
    /// contain more than one distinct colour. That catches a blank page, a
    /// page painted entirely in its own background, and a page whose content
    /// resolved to the ground it sits on. It does **not** catch bad spacing,
    /// wrong alignment, or an ugly layout, and it is not meant to.
    ///
    /// Set `PETRA_SHOT_DIR` to also write each page's PNG there, one file per
    /// row, for a human or an agent to look at.
    #[test]
    fn every_built_page_rasterizes_to_more_than_one_colour() {
        let dir = std::env::var_os("PETRA_SHOT_DIR").map(std::path::PathBuf::from);
        if let Some(dir) = &dir {
            std::fs::create_dir_all(dir).expect("shot dir");
        }
        let mut shooter = gorgon_petra_testkit::snapshot::Snapshotter::new();
        let mut flat: Vec<String> = Vec::new();

        for index in 0..Catalog::default().roster.len() {
            let app = Catalog {
                page: index,
                ..Catalog::default()
            };
            if !app.current().is_built() {
                continue;
            }
            let row = app.current().row;
            let name = row.component.to_owned();
            let number = row.number;

            let ctx = headless();
            // Two device pixels per point. The catalog is read by people
            // looking for defects a few pixels across — a snapped mark, a
            // missing hairline, two cells touching — and a 1x page has to be
            // magnified to see any of them, which is a resample and a
            // resample is where a one-pixel difference goes to die. It also
            // matches the IBM Carbon reference app under `ignored/carbon-ref/`,
            // which captures at device pixel ratio 2, so a Petra shot and a
            // Carbon shot of the same component are the same size and can be
            // put side by side without touching either.
            ctx.set_pixels_per_point(2.0);
            let mut host = Host::new(&ctx, app, default_presenter());
            // Two passes: the first registers the font atlas. A fresh `Host`
            // defaults keyboard focus to the first focusable placement, row
            // 1 in the index, whatever `page` actually is; `seat_index_focus`
            // corrects that for pages 1-30 (still inside the sidebar's own
            // fold). Pages 31-42's rows are past it, and `seat_index_focus`
            // deliberately does nothing there rather than risk a focused
            // placement going invisible under a scroll it did not settle —
            // see that function's doc and this audit's report. The second
            // pass is the one photographed; reduced motion makes it snap
            // rather than fly to wherever focus landed, since a static shot
            // cannot show a settled position for a bar still mid-flight.
            step(&ctx, &mut host, RawInput::default());
            seat_index_focus(&mut host);
            host.set_reduced_motion(true);
            let output = ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx));
            let shot = shooter
                .capture(
                    &ctx,
                    &output,
                    host.frame()
                        .unwrap_or_else(|| panic!("{name}: the pass produced no frame")),
                    None,
                )
                .unwrap_or_else(|err| panic!("{name}: capture refused: {err:?}"));
            output.drop_without_applying_deltas();

            let image = image::load_from_memory(&shot.png)
                .unwrap_or_else(|err| panic!("{name}: shot is not a PNG: {err}"))
                .to_rgba8();
            let mut seen: std::collections::HashSet<[u8; 4]> = std::collections::HashSet::new();
            for px in image.pixels() {
                seen.insert(px.0);
                if seen.len() > 1 {
                    break;
                }
            }
            if seen.len() < 2 {
                flat.push(format!("row {number} {name}"));
            }

            if let Some(dir) = &dir {
                let slug = name.to_lowercase().replace(' ', "-");
                std::fs::write(dir.join(format!("{number:02}-{slug}.png")), &shot.png)
                    .expect("write shot");
            }
        }

        assert!(
            flat.is_empty(),
            "these pages rasterized to a single flat colour, so whatever their \
             frame record says, nothing reached the screen:\n{}",
            flat.join("\n")
        );
    }
}
