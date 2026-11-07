//! The Petra amendments of spec 016 (T037 to T041), photographed.
//!
//! Two fixtures, each an [`App`] hosted by [`Camera::hosting`], so they
//! render headlessly like the catalog pages and touch no window:
//!
//! * **Tree states.** An eager tree of every row state, beside the
//!   virtualized `tree_rows` list serving the same rows flat, so the two
//!   forms can be read against each other. The list holds 20,000 rows and
//!   builds seven.
//! * **Syntax well.** One code well painted in every `syntax.*` token.
//!
//! These are fixtures and not catalog rows. The Lua mirror of the catalog
//! (`lua_parity`) proves every page body is expressible from Lua, and the
//! long-form `TreeItem` (slots, dimmed state, place) takes nodes that a
//! wire constructor cannot carry, so a catalog row built on it would break
//! that proof for a reason that is not drift.
//!
//! Read the PNGs. The assertions only hold the fixtures together.

use std::ops::Range;
use std::sync::Arc;

use gorgon_petra::component::{
    IconMark, IconTone, TreeBranch, TreeItem, TreePlace, code_runs, code_snippet_multi_capped,
    icon_toned, text, tree_rows, tree_view,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::{SYNTAX_CLASSES, Theme, syntax_token};
use gorgon_petra::tree::{AxisConstraint, InsetRefs, TextRun, ViewNode};
use gorgon_petra_egui::host::App;

use crate::page::common::{body, row, sp, tok};
use crate::shots::Camera;

/// What sits at a row's trailing edge.
#[derive(Clone, Copy)]
enum Mark {
    None,
    /// The word, muted.
    Ignored,
    /// A glyph and its word.
    Submodule,
}

/// One row of the states list: every state the tree draws, flattened
/// depth-first. The eager "states" tree nests these rows by `level`, and the
/// virtualized list serves the same rows flat with each row's place, so the
/// two draw one picture and the page can be read for any difference.
struct StateRow {
    label: &'static str,
    level: u32,
    branch: TreeBranch,
    selected: bool,
    dimmed: bool,
    mark: Mark,
}

const fn state(label: &'static str, level: u32, branch: TreeBranch, mark: Mark) -> StateRow {
    StateRow {
        label,
        level,
        branch,
        selected: false,
        dimmed: false,
        mark,
    }
}

const LONG: &str = "a-directory-name-too-long-for-this-column-to-hold";

/// Open and closed, an unloaded directory, an empty one, a leaf, four
/// levels, a dimmed ignored row with its word, a trailing glyph mark, and a
/// name that ends in an ellipsis beside its mark.
const STATES: [StateRow; 13] = [
    state("src", 1, TreeBranch::Open, Mark::None),
    StateRow {
        selected: true,
        ..state("main.rs", 2, TreeBranch::Leaf, Mark::None)
    },
    state("lib.rs", 2, TreeBranch::Leaf, Mark::None),
    state("nested", 2, TreeBranch::Open, Mark::None),
    state("deeper", 3, TreeBranch::Open, Mark::None),
    state("leaf.rs", 4, TreeBranch::Leaf, Mark::None),
    state("docs", 1, TreeBranch::Closed, Mark::None),
    state("empty", 1, TreeBranch::Open, Mark::None),
    state("README.md", 1, TreeBranch::Leaf, Mark::None),
    StateRow {
        dimmed: true,
        ..state("target", 1, TreeBranch::Closed, Mark::Ignored)
    },
    StateRow {
        dimmed: true,
        ..state("debug.log", 1, TreeBranch::Leaf, Mark::Ignored)
    },
    state("petra", 1, TreeBranch::Closed, Mark::Submodule),
    state(LONG, 1, TreeBranch::Closed, Mark::Submodule),
];

/// Rows the virtualized list says it holds. A 20,000-entry project is the
/// size spec 016 SC-003 budgets for.
const VIRTUAL_TOTAL: usize = 20_000;
/// The source name the virtualized tree is declared with.
const VIRTUAL_SOURCE: &str = "tv-files";
/// Key prefix of a virtualized row.
const VIRTUAL_PREFIX: &str = "tvf-";
/// The width every tree on the page is pinned to, so its rows fill one band.
const TREE_WIDTH: f32 = 360.0;
/// Seven 32-unit rows: the window the virtualized list shows.
const VIRTUAL_HEIGHT: f32 = 224.0;

/// A row's icon: the Carbon glyph for its branch state.
fn row_icon(branch: TreeBranch) -> ViewNode {
    icon_toned("g", branch.icon_mark(), IconTone::Secondary)
}

fn mark_node(mark: Mark) -> Option<ViewNode> {
    match mark {
        Mark::None => None,
        Mark::Ignored => {
            let mut word = text("m", "ignored");
            word.props
                .tokens
                .insert("foreground".into(), tok("text.muted"));
            Some(word)
        }
        Mark::Submodule => Some(row(
            "m",
            sp("spacing-02"),
            vec![
                icon_toned("g", IconMark::Copy, IconTone::Secondary),
                text("w", "submodule"),
            ],
        )),
    }
}

/// The builder for one states row, without nesting or place.
fn state_item(key: &str, spec: &StateRow) -> TreeItem {
    let mut item = TreeItem::new(key, spec.label)
        .branch(spec.branch)
        .selected(spec.selected)
        .icon(row_icon(spec.branch));
    if let Some(mark) = mark_node(spec.mark) {
        item = item.mark(mark);
    }
    if spec.dimmed {
        item = item.dimmed("ignored");
    }
    item
}

/// The key of states row `index`, in both forms.
fn state_key(index: usize) -> String {
    format!("tvs-{index}")
}

/// The states list as an eager tree: each row nested under the nearest
/// row above it with a lower level.
fn eager_states(from: usize, level: u32) -> Vec<ViewNode> {
    let mut out = Vec::new();
    let mut i = from;
    while i < STATES.len() && STATES[i].level >= level {
        if STATES[i].level == level {
            let mut end = i + 1;
            while end < STATES.len() && STATES[end].level > level {
                end += 1;
            }
            let children = eager_states(i + 1, level + 1);
            let nested: Vec<ViewNode> = if STATES[i].branch.is_open() {
                children
            } else {
                Vec::new()
            };
            out.push(state_item(&state_key(i), &STATES[i]).build(nested));
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// Where states row `index` sits among its siblings.
fn place_in_states(index: usize) -> TreePlace {
    let level = STATES[index].level;
    // The parent is the nearest row above with a lower level. Siblings are
    // the rows at this level until the next row of a lower level.
    let start = (0..index)
        .rev()
        .find(|&i| STATES[i].level < level)
        .map_or(0, |parent| parent + 1);
    let mut pos = 0;
    let mut size = 0;
    for (i, row) in STATES.iter().enumerate().skip(start) {
        if row.level < level {
            break;
        }
        if row.level == level {
            size += 1;
            if i <= index {
                pos += 1;
            }
        }
    }
    TreePlace {
        level,
        pos_in_set: pos,
        set_size: size,
    }
}

/// [`row`] with its children seated at the top, so trees of different
/// heights share one top edge.
fn row_top(
    key: &str,
    spacing: Option<gorgon_petra::token::TokenName>,
    children: Vec<ViewNode>,
) -> ViewNode {
    let mut node = row(key, spacing, children);
    node.props.align = Some(gorgon_petra::geom::Align::Start);
    node
}

/// Pin a tree to [`TREE_WIDTH`] so its rows fill one band.
fn pinned(mut node: ViewNode) -> ViewNode {
    node.constraints.horizontal = AxisConstraint {
        min: Some(TREE_WIDTH),
        max: Some(TREE_WIDTH),
        priority: 0,
    };
    node
}

/// Key of the syntax well.
const SNIP_SYNTAX: &str = "snip-syntax";

/// A realistic Rust function, each piece with the highlight class the
/// daemon would send for it. `None` is plain ink.
const SAMPLE: &[(&str, Option<&str>)] = &[
    ("/// Add two points.\n", Some("comment.doc")),
    ("#[inline]", Some("attribute")),
    ("\n", None),
    ("pub fn", Some("keyword")),
    (" ", None),
    ("add", Some("function")),
    ("(", Some("punctuation.bracket")),
    ("a", Some("variable.parameter")),
    (": ", Some("punctuation.delimiter")),
    ("Point", Some("type")),
    (", ", Some("punctuation.delimiter")),
    ("b", Some("variable.parameter")),
    (": ", Some("punctuation.delimiter")),
    ("Point", Some("type")),
    (") ", Some("punctuation.bracket")),
    ("->", Some("operator")),
    (" ", None),
    ("Point", Some("type")),
    (" {\n    ", None),
    ("let", Some("keyword")),
    (" x ", Some("variable")),
    ("=", Some("operator")),
    (" a", Some("variable")),
    (".", Some("punctuation.delimiter")),
    ("x", Some("property")),
    (" + ", Some("operator")),
    ("b", Some("variable")),
    (".", Some("punctuation.delimiter")),
    ("x", Some("property")),
    (";\n    ", None),
    ("// sum", Some("comment")),
    ("\n    ", None),
    ("println!", Some("function.macro")),
    ("(", Some("punctuation.bracket")),
    ("\"sum ", Some("string")),
    ("{}", Some("string.special")),
    ("\\n", Some("string.escape")),
    ("\"", Some("string")),
    (", ", Some("punctuation.delimiter")),
    ("x", Some("variable")),
    (");\n    ", None),
    ("if", Some("keyword.control")),
    (" x ", Some("variable")),
    (">", Some("operator")),
    (" ", None),
    ("0", Some("number")),
    (" { ", Some("punctuation.bracket")),
    ("return", Some("keyword.control")),
    (" ", None),
    ("None", Some("constant.builtin")),
    (";", Some("punctuation.delimiter")),
    (" }\n    ", Some("punctuation.bracket")),
    ("Point", Some("constructor")),
    (" { x ", Some("punctuation.bracket")),
    ("}\n}\n\n", Some("punctuation.bracket")),
    ("// markup\n", Some("comment")),
    ("# Title", Some("markup.heading")),
    (" ", None),
    ("**bold**", Some("markup.bold")),
    (" ", None),
    ("_italic_", Some("markup.italic")),
    (" ", None),
    ("[link](url)", Some("markup.link")),
    (" ", None),
    ("`raw`", Some("markup.raw")),
    ("\n", None),
    ("- item", Some("markup.list")),
    ("  ", None),
    ("> quote", Some("markup.quote")),
    ("\n\n", None),
    (
        "// every class: self, label, module, tag\n",
        Some("comment"),
    ),
    ("self", Some("variable.builtin")),
    (" ", None),
    ("'a", Some("label")),
    (" ", None),
    ("std::io", Some("module")),
    (" ", None),
    ("<div>", Some("tag")),
    (" ", None),
    ("u32", Some("type.builtin")),
    (" ", None),
    ("MAX", Some("constant")),
    (" ", None),
    ("assert", Some("function.builtin")),
    (" ", None),
    ("...", Some("punctuation")),
    ("\n\n", None),
    ("+ a line a patch added", Some("diff.plus")),
    ("\n", None),
    ("- a line a patch removed", Some("diff.minus")),
    ("\n", None),
];

/// The sample as one string and the runs that tile it.
///
/// Every class of `SYNTAX_CLASSES` appears in [`SAMPLE`] at least once, so
/// the well is the picture of the whole palette. A class with no token is a
/// panic here and a failing test below, not a plain-ink fallback.
fn syntax_sample() -> (String, Vec<TextRun>) {
    let mut code = String::new();
    let mut runs = Vec::new();
    for (piece, class) in SAMPLE {
        code.push_str(piece);
        runs.push(TextRun {
            len: piece.len(),
            foreground: class.map(|c| {
                syntax_token(c).unwrap_or_else(|| panic!("{c:?} is not a highlight class"))
            }),
        });
    }
    (code, runs)
}

/// Window height that fits all 13 eager rows (13 x 32) plus the page inset,
/// so no row is squeezed below its 32 unit height.
const TREE_WINDOW_H: f32 = 520.0;

/// The eager states tree beside the virtualized list, on `surface.base`,
/// the ground a file-tree panel sits on.
struct TreeStates {
    theme: Option<Theme>,
}

impl TreeStates {
    fn virtual_row(index: usize) -> Arc<ViewNode> {
        let key = format!("{VIRTUAL_PREFIX}{index}");
        let item = if index < STATES.len() {
            state_item(&key, &STATES[index]).at(place_in_states(index))
        } else {
            // Generated entries: depth walks 1, 2, 3, 2 so every level from
            // 1 to 3 shows as the list scrolls, and a third are closed
            // branches.
            const DEPTHS: [u32; 4] = [1, 2, 3, 2];
            let branch = if index.is_multiple_of(3) {
                TreeBranch::Closed
            } else {
                TreeBranch::Leaf
            };
            TreeItem::new(key.as_str(), format!("entry-{index}"))
                .branch(branch)
                .icon(row_icon(branch))
                .at(TreePlace {
                    level: DEPTHS[index % DEPTHS.len()],
                    pos_in_set: u32::try_from(index % 4).unwrap_or(0) + 1,
                    set_size: 4,
                })
        };
        // Row 1 is selected, like the eager tree's `main.rs`.
        Arc::new(item.selected(index == 1).build(vec![]))
    }
}

impl RowSource for TreeStates {
    fn rows(&mut self, source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        if source != VIRTUAL_SOURCE {
            return Vec::new();
        }
        let end = range.end.min(VIRTUAL_TOTAL);
        (range.start.min(end)..end).map(Self::virtual_row).collect()
    }
}

/// A body column on `surface.base` with a generous inset.
fn on_base(children: Vec<ViewNode>) -> ViewNode {
    let mut page = body("body", sp("spacing.md"), children);
    page.props
        .tokens
        .insert("background".into(), tok("surface.base"));
    page.props.padding = Some(InsetRefs::all(tok("spacing.xl")));
    page
}

impl App for TreeStates {
    fn view(&mut self) -> ViewNode {
        let mut list = tree_rows("tv-virtual", "Files", VIRTUAL_SOURCE, VIRTUAL_TOTAL);
        list.constraints.vertical = AxisConstraint {
            min: Some(VIRTUAL_HEIGHT),
            max: Some(VIRTUAL_HEIGHT),
            priority: 0,
        };
        on_base(vec![row_top(
            "trees",
            sp("spacing.xl"),
            vec![
                pinned(tree_view("states", eager_states(0, 1))),
                pinned(list),
            ],
        )])
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }

    fn theme_request(&mut self) -> Option<Theme> {
        self.theme.take()
    }
}

/// The syntax well on `surface.base`.
struct SyntaxWell {
    theme: Option<Theme>,
}

impl RowSource for SyntaxWell {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for SyntaxWell {
    fn view(&mut self) -> ViewNode {
        let (code, runs) = syntax_sample();
        on_base(vec![code_runs(
            code_snippet_multi_capped(SNIP_SYNTAX, code, "sample.rs", "Rust", true),
            runs,
        )])
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }

    fn theme_request(&mut self) -> Option<Theme> {
        self.theme.take()
    }
}

fn theme_named(name: &str) -> Theme {
    match name {
        "dark" => gorgon_petra::token::dark(),
        "light" => gorgon_petra::token::light(),
        other => panic!("no shipped theme is named {other:?}"),
    }
}

/// How many rows of the virtualized list the frame placed.
fn virtual_heads(cam: &Camera<TreeStates>) -> usize {
    cam.frame()
        .placements
        .iter()
        .filter(|p| p.id.contains("/tv-virtual/") && p.id.ends_with("/row"))
        .count()
}

/// Every state on one page, in both themes: an open branch, a closed
/// branch with unloaded children, an empty folder, a leaf, four levels,
/// dimmed ignored rows with their word, a trailing glyph mark, a long name
/// beside its mark, and the virtualized list beside them.
///
/// The assertions hold the fixture together: the rows exist, and the
/// virtualized list is a window and not the list. The picture is the check.
#[test]
fn every_tree_state_and_the_virtualized_list_render() {
    for theme in ["dark", "light"] {
        let mut cam = Camera::hosting(
            TreeStates {
                theme: Some(theme_named(theme)),
            },
            "tree states",
            [900.0, TREE_WINDOW_H],
        );
        for key in [
            "tvs-0", "tvs-5", "tvs-6", "tvs-7", "tvs-9", "tvs-11", "tvs-12",
        ] {
            assert!(cam.has(key), "{theme}: {key} is not on the page");
        }
        assert!(cam.has("tvf-0"));
        // No eager row is squeezed, and a virtual row is the height of an
        // eager one: the two forms draw at one row height.
        for index in 0..STATES.len() {
            let eager = cam.rect(&format!("tvs-{index}"));
            if eager.h > 32.0 {
                continue; // a branch that holds its children is taller
            }
            assert_eq!(
                eager.h, 32.0,
                "{theme}: tvs-{index} is squeezed to {}",
                eager.h
            );
        }
        assert_eq!(cam.rect("tvf-0").h, 32.0);
        assert!(!cam.has("tvf-19999"));
        let heads = virtual_heads(&cam);
        assert!(
            (7..=14).contains(&heads),
            "{theme}: {heads} virtualized rows placed"
        );
        cam.shoot(&format!("tree-states-{theme}"));
    }
}

/// Scrolling the virtualized list moves its window down the list: later
/// rows arrive, earlier ones leave, and the frame stays the same size.
#[test]
fn scrolling_the_virtualized_tree_moves_its_window() {
    let mut cam = Camera::hosting(
        TreeStates {
            theme: Some(theme_named("dark")),
        },
        "tree states",
        [900.0, TREE_WINDOW_H],
    );
    let before = cam.shoot("tree-states-virtual-top");
    cam.scroll("tv-virtual", -3200.0);
    assert!(
        !cam.has("tvf-0"),
        "the first row is still mounted after the scroll"
    );
    let after = cam.shoot("tree-states-virtual-scrolled");
    assert_ne!(before, after, "the scroll changed no pixel");
    let heads = virtual_heads(&cam);
    assert!((7..=14).contains(&heads), "{heads} virtualized rows placed");
}

/// The syntax well, whole, in both themes. Read `syntax-well-*.png`.
#[test]
fn the_syntax_well_paints_every_class() {
    for theme in ["dark", "light"] {
        let mut cam = Camera::hosting(
            SyntaxWell {
                theme: Some(theme_named(theme)),
            },
            "syntax well",
            [760.0, 560.0],
        );
        assert!(cam.has(SNIP_SYNTAX));
        cam.shoot(&format!("syntax-well-{theme}"));
    }
}

/// The sample paints every highlight class, so the picture is the picture
/// of the whole palette. A class added to the contract and not to the
/// sample fails here by name.
#[test]
fn the_syntax_sample_paints_every_highlight_class() {
    for class in SYNTAX_CLASSES {
        assert!(
            SAMPLE.iter().any(|(_, c)| *c == Some(class)),
            "the sample never paints {class}"
        );
    }
}

/// The runs tile the code exactly, which tree acceptance requires.
#[test]
fn the_syntax_runs_tile_the_sample() {
    let (code, runs) = syntax_sample();
    assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), code.len());
}
