//! The Carbon catalog window: one inventory row per page.
//!
//! `examples/gallery.rs` is the working page over the thirteen components
//! Petra ships today. This binary is the 42-row Carbon inventory. Wave 1
//! rows (Wave 1 and Wave 2) are built constructors. The rest say so in
//! words rather than drawing a stand-in.

use std::ops::Range;
use std::sync::Arc;

use egui::ViewportBuilder;
use gorgon_petra::component::{
    accordion, accordion_item, ai_label, ai_label_revert, breadcrumb, breadcrumb_item, button,
    button_lg, button_sm, checkbox, checkbox_group, checkbox_indeterminate, checkbox_readonly,
    clickable_tile, code_snippet, code_snippet_inline, contained_list, contained_tab,
    contained_tab_bar, content_switcher, content_switcher_item, danger_button, data_table,
    data_table_row, date_picker, dismissible_tag, dropdown, expandable_tile, field, field_invalid,
    field_lg, field_readonly, field_sm, file_uploader, file_uploader_item, form, ghost_button,
    heading, inline_loading, link, list_item, list_item_with, list_row, loading, loading_sm,
    menu_button, menu_item, modal, notification, number_input, ordered_list, pagination,
    primary_button, progress, progress_indicator, progress_sm, progress_step, progress_with_helper,
    radio, radio_group, search, section, select, selectable_tag, selectable_tile, slider,
    structured_list, structured_list_row, tab, tab_bar, tag, tertiary_button, text, tile, toggle,
    toggle_sm, toggletip, tree_item, tree_view, ui_shell_header, ui_shell_header_action,
    ui_shell_header_menu_trigger, ui_shell_header_nav_item, ui_shell_left_panel,
    ui_shell_left_panel_item, ui_shell_left_panel_subitem, ui_shell_right_panel_divider,
    ui_shell_switcher_item, unordered_list, vertical_tab, vertical_tab_bar,
};
use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::input::{InputEvent, KeyCode, PointerButton, Route, activates};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    AxisConstraint, Constraints, InsetRefs, NodeKind, Props, TextWrap, TrackSize, ViewNode,
};
use gorgon_petra_egui::host::{App, Host, default_presenter};

use crate::cell::{Cell, Content};

/// Inner size an interactive catalog window asks for. Not a layout pin: the
/// host lays out at whatever the compositor grants.
const WINDOW: [f32; 2] = [1200.0, 900.0];

const PREV: &str = "prev";
const NEXT: &str = "next";
const TOGGLE_DEFAULT_OFF: &str = "toggle-default-off";
const TOGGLE_DEFAULT_ON: &str = "toggle-default-on";
const TOGGLE_SM_OFF: &str = "toggle-sm-off";
const TOGGLE_SM_ON: &str = "toggle-sm-on";
const CHECK_A: &str = "check-a";
const CHECK_B: &str = "check-b";
const RADIO_A: &str = "radio-a";
const RADIO_B: &str = "radio-b";
const TAB_LINE_0: &str = "tab-line-0";
const TAB_LINE_1: &str = "tab-line-1";
const TAB_CONT_0: &str = "tab-cont-0";
const TAB_CONT_1: &str = "tab-cont-1";
const TILE_SEL: &str = "tile-sel";
const TILE_EXP: &str = "tile-exp";
const ACC_0: &str = "acc-0";
const SW_0: &str = "sw-0";
const SW_1: &str = "sw-1";
const TAG_SEL: &str = "tag-sel";
/// Key prefix for a row in the left index (`idx-36` is Toggle).
const IDX: &str = "idx-";
/// Width of the scrolling index pane, logical units.
const INDEX_WIDTH: f32 = 240.0;

/// A spacing token reference, for the styling props that take one (FR-053).
fn sp(name: &str) -> Option<TokenName> {
    Some(TokenName::new(name).expect("catalog spacing tokens are well-formed"))
}

/// A colour or type token reference, for `props.tokens` values.
fn tok(name: &str) -> TokenName {
    TokenName::new(name).expect("catalog style tokens are well-formed")
}

/// Symmetric padding from two spacing steps, horizontal first — the same
/// argument order [`InsetRefs::symmetric`] uses.
fn pad(horizontal: &str, vertical: &str) -> InsetRefs {
    InsetRefs::symmetric(tok(horizontal), tok(vertical))
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

fn path_has(node: &str, key: &str) -> bool {
    node.split('/').any(|part| part == key)
}

fn path_idx(node: &str) -> Option<u8> {
    node.split('/')
        .find_map(|part| part.strip_prefix(IDX)?.parse().ok())
}

/// The catalog application: paging chrome plus live control state.
pub struct Catalog {
    roster: Vec<Cell>,
    page: usize,
    default_off: bool,
    default_on: bool,
    small_off: bool,
    small_on: bool,
    check_a: bool,
    check_b: bool,
    radio: u8,
    line_tab: u8,
    contained_tab: u8,
    tile_sel: bool,
    tile_exp: bool,
    acc_open: bool,
    switcher: u8,
    tag_sel: bool,
    pager: u32,
}

impl Default for Catalog {
    fn default() -> Self {
        let roster = Cell::roster();
        let page = roster
            .iter()
            .position(|cell| cell.row.component == "Toggle")
            .expect("the Toggle row is in the inventory");
        Self {
            roster,
            page,
            default_off: false,
            default_on: true,
            small_off: false,
            small_on: true,
            check_a: true,
            check_b: false,
            radio: 0,
            line_tab: 0,
            contained_tab: 0,
            tile_sel: false,
            tile_exp: false,
            acc_open: true,
            switcher: 0,
            tag_sel: false,
            pager: 1,
        }
    }
}

impl Catalog {
    fn current(&self) -> &Cell {
        &self.roster[self.page]
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
                list_row(key, label, i == self.page)
            })
            .collect();
        let list = Self::column("rows", sp("spacing.2xs"), rows);
        let mut scroll = ViewNode::new(NodeKind::Scroll, "index").with_props(Props {
            axis: Some(Axis::Vertical),
            overscan: Some(64.0),
            ..Props::default()
        });
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

    /// A vertical run of blocks, each as tall as it needs to be.
    ///
    /// A single-column `Grid`, not a `Stack`. A vertical `Stack` placed at
    /// an exact height divides that height among its children by equal
    /// share; a `Grid`'s implicit rows are `FitContent`. See
    /// `examples/gallery.rs` `Gallery::column`.
    fn column(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Grid, key)
            .with_props(Props {
                columns: vec![TrackSize::Weight { weight: 1.0 }],
                row_spacing: spacing,
                ..Props::default()
            })
            .with_children(children)
    }

    fn row(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                spacing,
                align: Some(Align::Center),
                ..Props::default()
            })
            .with_children(children)
    }

    /// The body of a [`section`]: a column that outranks the section's title
    /// when the section divides its height.
    fn body(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
        Self::column(key, spacing, children).with_constraints(Constraints {
            vertical: AxisConstraint {
                min: None,
                max: None,
                priority: 1,
            },
            ..Constraints::default()
        })
    }

    fn wrapped(key: &str, content: impl Into<String>) -> ViewNode {
        let mut node = text(key, content);
        node.props.wrap = Some(TextWrap::Wrap);
        node
    }

    fn page_body(&self) -> ViewNode {
        let cell = self.current();
        match cell.content {
            Content::Unbuilt => {
                let letter = cell.row.slice.letter().to_ascii_lowercase();
                Self::wrapped(
                    "unbuilt",
                    format!(
                        "This inventory row is unbuilt. No Petra component exists for {}. \
                         Anatomy, variants, sizes, and states live in slice-{letter}.md. \
                         This page does not draw a stand-in.",
                        cell.row.component
                    ),
                )
            }
            Content::Accordion => section(
                "items",
                "Items",
                vec![Self::body(
                    "accordion",
                    sp("spacing.md"),
                    vec![accordion(
                        "acc",
                        vec![
                            accordion_item(ACC_0, "First section", self.acc_open, "Hidden body."),
                            accordion_item("acc-1", "Second section", false, "Still closed."),
                        ],
                    )],
                )],
            ),
            Content::AiLabel => section(
                "ai",
                "Triggers (closed)",
                vec![Self::body(
                    "ai-body",
                    sp("spacing.md"),
                    vec![
                        // Closed form only: `open: true` mounts the
                        // explainability popover via `Anchor::Node`, which the
                        // catalog cannot validate nested in a page (see the
                        // Popover/Toggletip/Tooltip pages below for the same
                        // workaround).
                        //
                        // `ai_label_inline` is NOT shown here: its trigger
                        // (`ai_label.rs::inline_trigger`) sets `props.padding`
                        // on the "AI" caption, which is a `text` leaf —
                        // `validate` refuses a leaf declaring padding it has
                        // no children to apply. This is a pre-existing defect
                        // in `ai_label.rs`, which this integration task does
                        // not own or edit; reported separately.
                        ai_label(
                            "ai-default",
                            "Confidence score",
                            false,
                            "Trained on ticket history.",
                        ),
                        ai_label_revert("ai-revert", "Revert to AI suggestion"),
                    ],
                )],
            ),
            Content::Breadcrumb => section(
                "trail",
                "Trail",
                vec![Self::body(
                    "crumbs",
                    sp("spacing.md"),
                    vec![breadcrumb(
                        "crumbs",
                        vec![
                            breadcrumb_item("bc-0", "Workspace"),
                            breadcrumb_item("bc-1", "Fibers"),
                            breadcrumb_item("bc-2", "Rebuild"),
                        ],
                    )],
                )],
            ),
            Content::Button => section(
                "variants",
                "Variants and sizes",
                vec![Self::body(
                    "buttons",
                    sp("spacing.md"),
                    vec![
                        Self::row(
                            "kinds",
                            sp("spacing.md"),
                            vec![
                                primary_button("btn-primary", "Primary"),
                                button("btn-default", "Default"),
                                tertiary_button("btn-tertiary", "Tertiary"),
                                ghost_button("btn-ghost", "Ghost"),
                                danger_button("btn-danger", "Danger"),
                            ],
                        ),
                        Self::row(
                            "sizes",
                            sp("spacing.md"),
                            vec![
                                button_sm("btn-sm", "Small 32"),
                                button("btn-md", "Medium 40"),
                                button_lg("btn-lg", "Large 48"),
                            ],
                        ),
                    ],
                )],
            ),
            Content::CodeSnippet => section(
                "snippets",
                "Single, inline",
                vec![Self::body(
                    "code",
                    sp("spacing.md"),
                    vec![
                        code_snippet("snip", "pcargo test -p gorgon-petra --lib"),
                        code_snippet_inline("snip-in", "Role::List"),
                    ],
                )],
            ),
            Content::ContainedList => section(
                "on-page",
                "On-page header",
                vec![Self::body(
                    "contained",
                    sp("spacing.md"),
                    vec![contained_list(
                        "cl",
                        "Recent",
                        vec![list_item("cl-0", "Trace"), list_item("cl-1", "Store")],
                    )],
                )],
            ),
            Content::ContentSwitcher => section(
                "switcher",
                "Switcher",
                vec![Self::body(
                    "switch",
                    sp("spacing.md"),
                    vec![content_switcher(
                        "sw",
                        vec![
                            content_switcher_item(SW_0, "List", self.switcher == 0),
                            content_switcher_item(SW_1, "Grid", self.switcher == 1),
                        ],
                    )],
                )],
            ),
            Content::DataTable => section(
                "table",
                "Data table",
                vec![Self::body(
                    "dt",
                    sp("spacing.md"),
                    vec![data_table(
                        "dt",
                        vec![text("h0", "Name"), text("h1", "Kind")],
                        vec![
                            data_table_row(
                                "dt-0",
                                vec![text("c0", "kernel"), text("c1", "runtime")],
                                false,
                            ),
                            data_table_row(
                                "dt-1",
                                vec![text("c0", "petra"), text("c1", "layout")],
                                true,
                            ),
                        ],
                    )],
                )],
            ),
            Content::DatePicker => section(
                "date",
                "Closed date picker",
                vec![Self::body(
                    "dp",
                    sp("spacing.md"),
                    vec![date_picker("when", "Date", "2026-08-30")],
                )],
            ),
            Content::Dropdown => section(
                "drop",
                "Closed dropdown",
                vec![Self::body(
                    "dd",
                    sp("spacing.md"),
                    vec![dropdown("dd", "Theme", "Dark")],
                )],
            ),
            Content::FileUploader => section(
                "files",
                "Uploader",
                vec![Self::body(
                    "fu",
                    sp("spacing.md"),
                    vec![
                        file_uploader("fu", "Drop files here"),
                        file_uploader_item("fu-0", "trace.ndjson", true),
                    ],
                )],
            ),
            Content::Form => section(
                "form",
                "Form",
                vec![Self::body(
                    "form-body",
                    sp("spacing.md"),
                    vec![form(
                        "demo-form",
                        "Fiber",
                        vec![
                            field("form-name", "Name"),
                            checkbox("form-ok", "Enabled", true),
                        ],
                    )],
                )],
            ),
            Content::InlineLoading => section(
                "inline",
                "Inline loading",
                vec![Self::body(
                    "il",
                    sp("spacing.md"),
                    vec![
                        inline_loading("il-on", "Saving", true),
                        inline_loading("il-off", "Saved", false),
                    ],
                )],
            ),
            Content::Link => section(
                "links",
                "Link",
                vec![Self::body(
                    "link-row",
                    sp("spacing.md"),
                    vec![link("docs", "Open the spec")],
                )],
            ),
            Content::Checkbox => section(
                "states",
                "States",
                vec![Self::body(
                    "checks",
                    sp("spacing.md"),
                    vec![checkbox_group(
                        "check-group",
                        "Notifications",
                        vec![
                            checkbox(CHECK_A, "Email", self.check_a),
                            checkbox(CHECK_B, "Push", self.check_b),
                            checkbox_indeterminate("check-mixed", "Mixed"),
                            checkbox_readonly("check-ro", "Read only", true),
                        ],
                    )],
                )],
            ),
            Content::Loading => section(
                "spinner",
                "Loading",
                vec![Self::body(
                    "load",
                    sp("spacing.md"),
                    vec![
                        loading("load-lg", "Working"),
                        loading_sm("load-sm", "Working"),
                    ],
                )],
            ),
            Content::List => section(
                "kinds",
                "Ordered, unordered, nested",
                vec![Self::body(
                    "lists",
                    sp("spacing.md"),
                    vec![
                        unordered_list(
                            "ul",
                            vec![
                                list_item("ul-0", "Inbox"),
                                list_item_with(
                                    "ul-1",
                                    "Archive",
                                    Some(unordered_list(
                                        "ul-nested",
                                        vec![
                                            list_item("ul-1-0", "2025"),
                                            list_item("ul-1-1", "2026"),
                                        ],
                                    )),
                                ),
                            ],
                        ),
                        ordered_list(
                            "ol",
                            vec![
                                list_item("ol-0", "Clone"),
                                list_item("ol-1", "Build"),
                                list_item("ol-2", "Run"),
                            ],
                        ),
                    ],
                )],
            ),
            Content::Menu => section(
                "menu",
                "Menu items",
                vec![Self::body(
                    "mn",
                    sp("spacing.md"),
                    vec![menu_item("mn-0", "Rename"), menu_item("mn-1", "Delete")],
                )],
            ),
            Content::MenuButtons => section(
                "mb",
                "Menu button",
                vec![Self::body(
                    "mb-body",
                    sp("spacing.md"),
                    vec![menu_button(
                        "mb",
                        "More",
                        false,
                        vec![menu_item("mb-0", "Duplicate")],
                    )],
                )],
            ),
            Content::Modal => section(
                "dialog",
                "Modal",
                vec![Self::body(
                    "md",
                    sp("spacing.md"),
                    vec![modal("md", "Confirm rebuild", "This unloads the fiber.")],
                )],
            ),
            Content::Notification => section(
                "note",
                "Notification",
                vec![Self::body(
                    "nt",
                    sp("spacing.md"),
                    vec![notification(
                        "nt",
                        "Rebuild finished",
                        "12 fibers reloaded.",
                    )],
                )],
            ),
            Content::NumberInput => section(
                "number",
                "Number input",
                vec![Self::body(
                    "num",
                    sp("spacing.md"),
                    vec![number_input("n-md", "Count", "12")],
                )],
            ),
            Content::Pagination => section(
                "pager",
                "Pagination",
                vec![Self::body(
                    "pages",
                    sp("spacing.md"),
                    vec![pagination("pager", self.pager, 5)],
                )],
            ),
            Content::Popover => section(
                "pop",
                "Popover chrome",
                vec![Self::body(
                    "po",
                    sp("spacing.md"),
                    vec![
                        button("pop-anchor", "Anchor"),
                        Self::wrapped("pop-body", "Anchored note (constructor sets Anchor::Node)."),
                    ],
                )],
            ),
            Content::ProgressBar => section(
                "bars",
                "Determinate",
                vec![Self::body(
                    "progress",
                    sp("spacing.md"),
                    vec![
                        progress("prog-big", "Rebuild", 0.62),
                        progress_sm("prog-sm", "Upload", 0.25),
                        progress_with_helper("prog-help", "Index", 1.0, "Complete"),
                    ],
                )],
            ),
            Content::ProgressIndicator => section(
                "steps",
                "Steps",
                vec![Self::body(
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
            ),
            Content::RadioButton => section(
                "group",
                "Group",
                vec![Self::body(
                    "radios",
                    sp("spacing.md"),
                    vec![radio_group(
                        "radio-group",
                        "Theme",
                        vec![
                            radio(RADIO_A, "Dark", self.radio == 0),
                            radio(RADIO_B, "Light", self.radio == 1),
                        ],
                    )],
                )],
            ),
            Content::Search => section(
                "search",
                "Search",
                vec![Self::body(
                    "q",
                    sp("spacing.md"),
                    vec![search("q", "Filter fibers")],
                )],
            ),
            Content::Select => section(
                "select",
                "Closed select",
                vec![Self::body(
                    "sel",
                    sp("spacing.md"),
                    vec![select("theme", "Theme", "Dark")],
                )],
            ),
            Content::Slider => section(
                "slide",
                "Slider",
                vec![Self::body(
                    "slid",
                    sp("spacing.md"),
                    vec![slider("vol", "Volume", 0.4)],
                )],
            ),
            Content::StructuredList => section(
                "table",
                "Structured list",
                vec![Self::body(
                    "sl",
                    sp("spacing.md"),
                    vec![structured_list(
                        "sl",
                        vec![text("h0", "Name"), text("h1", "Role")],
                        vec![
                            structured_list_row(
                                "sl-0",
                                vec![text("c0", "kernel"), text("c1", "runtime")],
                                false,
                            ),
                            structured_list_row(
                                "sl-1",
                                vec![text("c0", "petra"), text("c1", "layout")],
                                true,
                            ),
                        ],
                    )],
                )],
            ),
            Content::Tabs => section(
                "strips",
                "Line, contained, vertical",
                vec![Self::body(
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
                        vertical_tab_bar(
                            "vert-strip",
                            vec![
                                vertical_tab("tab-vert-0", "North", true),
                                vertical_tab("tab-vert-1", "South", false),
                            ],
                        ),
                    ],
                )],
            ),
            Content::Tag => section(
                "tags",
                "Tags",
                vec![Self::body(
                    "tag-row",
                    sp("spacing.md"),
                    vec![
                        tag("tag-ro", "Read only"),
                        dismissible_tag("tag-x", "Filter"),
                        selectable_tag(TAG_SEL, "Selectable", self.tag_sel),
                    ],
                )],
            ),
            Content::TextInput => section(
                "fields",
                "Default sizes and states",
                vec![Self::body(
                    "inputs",
                    sp("spacing.md"),
                    vec![
                        field("field-md", "Fiber name"),
                        field_sm("field-sm", "Small"),
                        field_lg("field-lg", "Large"),
                        field_invalid("field-bad", "Port", "must be a number"),
                        field_readonly("field-ro", "Read only"),
                    ],
                )],
            ),
            Content::Tile => section(
                "kinds",
                "Base, clickable, selectable, expandable",
                vec![Self::body(
                    "tiles",
                    sp("spacing.md"),
                    vec![
                        tile("tile-base", "A static tile holds related content."),
                        clickable_tile(
                            "tile-click",
                            "Open workspace",
                            "Clickable tile — one target.",
                        ),
                        selectable_tile(TILE_SEL, "Select this option", self.tile_sel),
                        expandable_tile(
                            TILE_EXP,
                            "More detail",
                            self.tile_exp,
                            "Below-the-fold body.",
                        ),
                    ],
                )],
            ),
            Content::Toggletip => section(
                "tt",
                "Toggletip",
                vec![Self::body(
                    "tt-body",
                    sp("spacing.md"),
                    vec![toggletip("tt", "Why", false, "Because the spec says so.")],
                )],
            ),
            Content::Tooltip => section(
                "tip",
                "Tooltip",
                vec![Self::body(
                    "tip-body",
                    sp("spacing.md"),
                    vec![Self::wrapped("tip-text", "Save writes the composition.")],
                )],
            ),
            Content::TreeView => section(
                "tree",
                "Tree view",
                vec![Self::body(
                    "tv",
                    sp("spacing.md"),
                    vec![tree_view(
                        "tv",
                        vec![tree_item(
                            "tv-0",
                            "gorgon",
                            true,
                            false,
                            vec![tree_item("tv-0-0", "petra", false, true, vec![])],
                        )],
                    )],
                )],
            ),
            Content::UiShellHeader => section(
                "shell-header-section",
                "Header",
                vec![Self::body(
                    "shell-header-body",
                    sp("spacing.md"),
                    vec![ui_shell_header(
                        "shell-header",
                        "GOrgOn",
                        Some(ui_shell_header_menu_trigger("shell-menu", false)),
                        vec![
                            ui_shell_header_nav_item("shell-nav-overview", "Overview", true),
                            ui_shell_header_nav_item("shell-nav-fibers", "Fibers", false),
                        ],
                        vec![
                            ui_shell_header_action("shell-action-notify", "Notifications", false),
                            ui_shell_header_action("shell-action-switcher", "App switcher", false),
                        ],
                    )],
                )],
            ),
            Content::UiShellLeftPanel => section(
                "shell-left-section",
                "Fixed panel",
                vec![Self::body(
                    "shell-left-body",
                    sp("spacing.md"),
                    vec![ui_shell_left_panel(
                        "shell-left",
                        vec![
                            ui_shell_left_panel_item(
                                "shell-left-kernel",
                                "Kernel",
                                true,
                                false,
                                vec![ui_shell_left_panel_subitem(
                                    "shell-left-fibers",
                                    "Fibers",
                                    false,
                                )],
                            ),
                            ui_shell_left_panel_item(
                                "shell-left-petra",
                                "Petra",
                                false,
                                true,
                                vec![],
                            ),
                        ],
                    )],
                )],
            ),
            Content::UiShellRightPanel => section(
                "shell-right-section",
                "Switcher trigger and items",
                vec![Self::body(
                    "shell-right-body",
                    sp("spacing.md"),
                    vec![
                        // `ui_shell_right_panel`/`ui_shell_switcher` build a
                        // `Surface` anchored via `Anchor::Node`, the same
                        // open-anchored-overlay shape the catalog cannot
                        // validate nested in a page (see the Popover page
                        // above). The trigger and the switcher's own rows
                        // (plain buttons, no anchor) are shown standalone
                        // instead of inside the anchored panel.
                        ui_shell_header_action("shell-switcher-trigger", "App switcher", false),
                        ui_shell_switcher_item("shell-switcher-petra", "Petra"),
                        ui_shell_right_panel_divider("shell-switcher-div"),
                        ui_shell_switcher_item("shell-switcher-inspector", "Inspector"),
                    ],
                )],
            ),
            Content::Toggle => section(
                "states",
                "States",
                vec![Self::body(
                    "toggles",
                    sp("spacing.md"),
                    vec![
                        toggle(TOGGLE_DEFAULT_OFF, "Default off", self.default_off),
                        toggle(TOGGLE_DEFAULT_ON, "Default on", self.default_on),
                        toggle_sm(TOGGLE_SM_OFF, "Small off", self.small_off),
                        toggle_sm(TOGGLE_SM_ON, "Small on", self.small_on),
                    ],
                )],
            ),
        }
    }
}

impl RowSource for Catalog {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Catalog {
    fn view(&mut self) -> ViewNode {
        let cell = self.current();
        let title = format!("{}  {}", cell.row.number, cell.row.component);
        let slice = format!("slice {}", cell.row.slice.letter());
        let status = if cell.is_built() { "BUILT" } else { "UNBUILT" };

        let mut main = Self::column(
            "main",
            sp("spacing.xl"),
            vec![
                heading("title", title),
                Self::column(
                    "meta",
                    sp("spacing.sm"),
                    vec![text("slice", slice), text("status", status)],
                ),
                Self::row(
                    "nav",
                    sp("spacing.md"),
                    vec![button(PREV, "Prev"), button(NEXT, "Next")],
                ),
                self.page_body(),
            ],
        );
        main.props.padding = Some(pad("spacing.xl", "spacing.lg"));

        let mut shell = ViewNode::new(NodeKind::Grid, "shell").with_props(Props {
            columns: vec![
                TrackSize::Fixed { value: INDEX_WIDTH },
                TrackSize::Weight { weight: 1.0 },
            ],
            column_spacing: sp("spacing.md"),
            ..Props::default()
        });
        shell = shell.child(self.index_pane()).child(main);
        shell
            .props
            .tokens
            .insert("background".into(), tok("surface.base"));

        let mut page = Props {
            axis: Some(Axis::Vertical),
            overscan: Some(64.0),
            ..Props::default()
        };
        page.tokens.insert("background".into(), tok("surface.base"));
        ViewNode::new(NodeKind::Scroll, "page")
            .with_props(page)
            .child(shell)
    }

    fn handle(&mut self, event: &InputEvent, route: &Route) {
        if let InputEvent::Key {
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
            Route::Unrouted { .. } => return,
        };
        if !activated(event) {
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
        if path_has(node, "pager") && path_has(node, "previous") {
            if self.pager > 1 {
                self.pager -= 1;
            }
        } else if path_has(node, "pager") && path_has(node, "next") {
            if self.pager < 5 {
                self.pager += 1;
            }
        } else if path_has(node, PREV) {
            self.prev();
        } else if path_has(node, NEXT) {
            self.next();
        } else if path_has(node, TOGGLE_DEFAULT_OFF) {
            self.default_off = !self.default_off;
        } else if path_has(node, TOGGLE_DEFAULT_ON) {
            self.default_on = !self.default_on;
        } else if path_has(node, TOGGLE_SM_OFF) {
            self.small_off = !self.small_off;
        } else if path_has(node, TOGGLE_SM_ON) {
            self.small_on = !self.small_on;
        } else if path_has(node, CHECK_A) {
            self.check_a = !self.check_a;
        } else if path_has(node, CHECK_B) {
            self.check_b = !self.check_b;
        } else if path_has(node, RADIO_A) {
            self.radio = 0;
        } else if path_has(node, RADIO_B) {
            self.radio = 1;
        } else if path_has(node, TAB_LINE_0) {
            self.line_tab = 0;
        } else if path_has(node, TAB_LINE_1) {
            self.line_tab = 1;
        } else if path_has(node, TAB_CONT_0) {
            self.contained_tab = 0;
        } else if path_has(node, TAB_CONT_1) {
            self.contained_tab = 1;
        } else if path_has(node, TILE_SEL) {
            self.tile_sel = !self.tile_sel;
        } else if path_has(node, TILE_EXP) {
            self.tile_exp = !self.tile_exp;
        } else if path_has(node, ACC_0) {
            self.acc_open = !self.acc_open;
        } else if path_has(node, SW_0) {
            self.switcher = 0;
        } else if path_has(node, SW_1) {
            self.switcher = 1;
        } else if path_has(node, TAG_SEL) {
            self.tag_sel = !self.tag_sel;
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

/// Wraps the host so the first pass can steal OS focus for Tab.
struct CatalogWindow {
    host: Host<Catalog>,
    grabbed_focus: bool,
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
    options.wgpu_options = eframe::WgpuConfiguration::default()
        .with_surface_config(eframe::SurfaceConfig::HIGH_THROUGHPUT);
    eframe::run_native(
        "Petra Carbon catalog",
        options,
        Box::new(|cc| {
            Ok(Box::new(CatalogWindow {
                host: Host::new(&cc.egui_ctx, Catalog::default(), default_presenter()),
                grabbed_focus: false,
            }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        Catalog, Cell, Content, NEXT, PREV, TOGGLE_DEFAULT_OFF, TOGGLE_DEFAULT_ON, TOGGLE_SM_OFF,
        TOGGLE_SM_ON, WINDOW,
    };
    use egui::{Context, Pos2, RawInput};
    use gorgon_petra::geom::Point;
    use gorgon_petra::input::{InputEvent, KeyCode, Modifiers, PointerButton, Route};
    use gorgon_petra::tree::ViewNode;
    use gorgon_petra_egui::host::{App, Host, default_presenter};

    fn find<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|child| find(child, key))
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
        app.handle(
            &InputEvent::PointerPressed {
                pos: Point::ZERO,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            &Route::Pointer {
                node: format!("/page/root/{tail}"),
            },
        );
    }

    fn key(app: &mut Catalog, code: KeyCode) {
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
        );
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
        assert!(find(&tree, TOGGLE_DEFAULT_OFF).is_some());
        assert!(find(&tree, TOGGLE_DEFAULT_ON).is_some());
        assert!(find(&tree, TOGGLE_SM_OFF).is_some());
        assert!(find(&tree, TOGGLE_SM_ON).is_some());
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
    fn clicking_a_toggle_flips_application_state() {
        let mut app = Catalog::default();
        assert!(
            !find(&app.view(), TOGGLE_DEFAULT_OFF)
                .unwrap()
                .semantics
                .selected
        );
        assert!(
            find(&app.view(), TOGGLE_DEFAULT_ON)
                .unwrap()
                .semantics
                .selected
        );
        assert!(!find(&app.view(), TOGGLE_SM_OFF).unwrap().semantics.selected);
        assert!(find(&app.view(), TOGGLE_SM_ON).unwrap().semantics.selected);

        press(&mut app, TOGGLE_DEFAULT_OFF);
        press(&mut app, TOGGLE_DEFAULT_ON);
        press(&mut app, TOGGLE_SM_OFF);
        press(&mut app, TOGGLE_SM_ON);

        assert!(
            find(&app.view(), TOGGLE_DEFAULT_OFF)
                .unwrap()
                .semantics
                .selected
        );
        assert!(
            !find(&app.view(), TOGGLE_DEFAULT_ON)
                .unwrap()
                .semantics
                .selected
        );
        assert!(find(&app.view(), TOGGLE_SM_OFF).unwrap().semantics.selected);
        assert!(!find(&app.view(), TOGGLE_SM_ON).unwrap().semantics.selected);
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
        assert!(find(&tree, TOGGLE_DEFAULT_OFF).is_none());
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

    /// A press on a child of the toggle (the knob) must still flip it.
    #[test]
    fn a_press_on_the_knob_flips_the_toggle() {
        let mut app = Catalog::default();
        assert!(!app.default_off);
        app.handle(
            &InputEvent::PointerPressed {
                pos: Point::ZERO,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            &Route::Pointer {
                node: "/page/shell/main/states/toggles/toggle-default-off/appearance/track/knob"
                    .into(),
            },
        );
        assert!(app.default_off, "a press on the knob must flip the control");
        app.handle(
            &InputEvent::PointerPressed {
                pos: Point::ZERO,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            &Route::Pointer {
                node: "/page/shell/main/states/toggles/toggle-default-off/appearance/track/knob"
                    .into(),
            },
        );
        assert!(
            !app.default_off,
            "a second press on the same knob must flip again"
        );
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
            let mut host = Host::new(&ctx, app, default_presenter());
            // Two passes: the first registers the font atlas, the second is
            // the one with content to photograph.
            step(&ctx, &mut host, RawInput::default());
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
