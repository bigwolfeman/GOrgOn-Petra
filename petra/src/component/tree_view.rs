//! Carbon Tree view (slice-f). No drag-to-reorder.
//!
//! Anatomy (`_treeview.scss` + usage page + `39-tree-view.png`):
//! 1. [`tree_view`] — the hierarchy container (`Role::Tree`).
//! 2. Branch / leaf [`tree_item`] — `Role::TreeItem`, a full-width row
//!    pinned to 32 (small, the default) or 24 ([`tree_item_xs`]).
//! 3. Caret — a [`super::caret`] on branches, down when expanded and
//!    right when collapsed. Leaves keep the caret's slot empty so their
//!    label lines up with a sibling branch's (docs style page: branch L1
//!    indent 16, leaf L1 indent 40 — the 24 difference is the caret's
//!    box). Until 2026-09-04 the caret was the literal word `expanded` or
//!    `collapsed` in front of the label.
//! 4. Node label.
//! 5. Accent bar — Carbon's `.cds--tree-node--active` 4-unit
//!    [`ACCENT_PRIMARY`] bar at the row's inline-start edge, full row
//!    height, drawn on the selected row so selection is a shape as well
//!    as a fill.
//!
//! Nested children exist in the tree only while `expanded` is true, and
//! that revealed content plus `Semantics.expanded` is the channel a
//! reader who cannot see the caret still gets (FR-026). Selection is
//! `Semantics.selected` plus [`LAYER_SELECTED`] plus the accent bar, never
//! colour alone. Carbon does not ship drag-to-reorder; items do not
//! declare [`Interaction::Drag`].
//!
//! # Indent arithmetic
//!
//! One level is `$spacing-05` (16), added to each nested row's lead by
//! [`step_in`] rather than as padding on the nested container, so a nested
//! row's fill and accent bar still span the whole tree.
//! Inside a row, left to right: the 4-unit accent bar (its fill only when
//! selected, its width always, so nothing shifts on selection), 12 of
//! lead (`$spacing-04`), the 16-unit caret box, 8 (`$spacing-03`), an
//! optional icon and 8, then the label — so a level-1 label starts at 40
//! and a level-2 label at 56, Carbon's own numbers for a text-only tree.
//! An optional mark follows the label, pushed to the trailing edge by a
//! spacer. Right padding is 16.
//!
//! # Long form, slots, and the virtualized form
//!
//! [`TreeItem`] is the builder behind [`tree_item`]. It adds what a row
//! needs beyond a label: [`TreeBranch`] (a caret on a directory whose
//! children are not loaded, or that has none), an icon slot, a trailing
//! mark slot, a dimmed state, and a place in a flattened list
//! ([`TreePlace`]). [`tree_rows`] is [`tree_view`] over a
//! [`crate::tree::NodeKind::Collection`], and a row built with
//! [`TreeItem::at`] draws the pixels the eager tree draws at that depth.
//! Both forms declare `level`, `pos_in_set`, and `set_size`.

use std::sync::Arc;

use super::tokens::{
    ACCENT_PRIMARY, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_05, SURFACE_BASE,
    TEXT_MUTED, t,
};
use super::{CARET_SIZE, CaretDirection, IconMark, caret, stack, swatch};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Behaviour, FocusFigure, FocusShownOn, InsetRefs, Intent, Interaction, Key,
    NodeKind, Phase, Props, Role, Semantics, TextWrap, TrackSize, ViewNode,
};

/// Spec 010: exclusive among siblings.
const SELECTS_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Select,
    phase: Phase::OnRelease,
};

/// Carbon small / default node height.
const HEIGHT: f32 = 32.0;
/// Carbon extra-small node height.
const HEIGHT_XS: f32 = 24.0;
/// Carbon `.cds--tree-node--active` bar width.
const ACCENT_BAR: f32 = 4.0;
/// Lead between the accent bar and the caret box, so bar + lead is the
/// branch's 16-unit level-1 indent.
const LEAD: f32 = 12.0;
/// Gap between the caret box and the label (`$spacing-03`).
const CARET_GAP: f32 = 8.0;
/// Where a level-1 label starts: Carbon's leaf L1 indent (`$spacing-08`).
const LABEL_START: f32 = ACCENT_BAR + LEAD + CARET_SIZE + CARET_GAP;
/// One level of nesting (`$spacing-05`), added to a nested row's lead.
const LEVEL_INDENT: f32 = 16.0;
/// Gap between a row's icon and its label, and between its label and its
/// mark (`$spacing-03`).
const SLOT_GAP: f32 = 8.0;
/// The ink opacity of a dimmed row's icon. A lightness cue, not a hue cue,
/// so it holds for a red-green colour-blind reader. The label is not
/// faded: it takes the `text.muted` token instead, which has a measured
/// contrast floor, where a faded label would drop below it.
const DIMMED_ICON_OPACITY: f32 = 0.5;

const _: () = assert!(HEIGHT == 32.0);
const _: () = assert!(HEIGHT_XS == 24.0);
const _: () = assert!(ACCENT_BAR + LEAD == 16.0);
const _: () = assert!(LABEL_START == 40.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// What a tree item is to its children, as one value.
///
/// Whether a row draws a caret is a fact about the row, not something to
/// infer from whether its children happen to be mounted. A directory that
/// has not been read yet has no mounted children and still opens; a
/// directory that was read and holds nothing also has none and also opens
/// (to nothing). Both are branches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TreeBranch {
    /// No caret. Nothing to open.
    #[default]
    Leaf,
    /// A caret that points right. The children, if any, are not mounted.
    Closed,
    /// A caret that points down. The children passed to the builder mount.
    Open,
}

impl TreeBranch {
    /// The branch a bare child list implies, which is what [`tree_item`]
    /// has always done: no children is a leaf, children that are hidden
    /// are closed, children that show are open.
    #[must_use]
    pub fn implied(has_children: bool, expanded: bool) -> Self {
        match (has_children, expanded) {
            (false, _) => Self::Leaf,
            (true, false) => Self::Closed,
            (true, true) => Self::Open,
        }
    }

    /// Whether the row draws a caret.
    #[must_use]
    pub fn is_branch(self) -> bool {
        self != Self::Leaf
    }

    /// Whether the children mount.
    #[must_use]
    pub fn is_open(self) -> bool {
        self == Self::Open
    }

    /// The Carbon glyph a file tree draws for this state: [`IconMark::Document`]
    /// for a leaf and [`IconMark::Folder`] for a branch, open or closed. The
    /// caret already says open or closed, as Carbon's tree view does, so the
    /// glyph does not repeat it. Pass it to [`icon_toned`](super::icon_toned) and
    /// seat the node with [`TreeItem::icon`]. A tree of something other than
    /// files picks its own mark.
    #[must_use]
    pub const fn icon_mark(self) -> IconMark {
        match self {
            Self::Leaf => IconMark::Document,
            Self::Closed | Self::Open => IconMark::Folder,
        }
    }
}

/// Where a flat row sits in its hierarchy, for [`TreeItem::at`].
///
/// All three numbers are 1-based, as WAI-ARIA counts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TreePlace {
    /// Depth, 1 at the top.
    pub level: u32,
    /// Position among siblings.
    pub pos_in_set: u32,
    /// Number of siblings.
    pub set_size: u32,
}

/// One row of a tree, described before it is built.
///
/// [`tree_item`] and [`tree_item_xs`] are the short forms. This is the
/// long form, for a row that needs a slot or a state: an icon, a trailing
/// mark, a dimmed look, or a place in a flattened list ([`tree_rows`]).
///
/// Every part is a value the caller hands in, and none is specific to one
/// kind of tree. The icon and the mark are any [`ViewNode`]. The container
/// seats them and gives them stable keys (`icon`, `mark`), so a row's node
/// paths do not depend on what was passed.
///
/// ```ignore
/// TreeItem::new("target", "target")
///     .branch(TreeBranch::Closed)
///     .icon(folder_glyph)
///     .mark(submodule_text)
///     .dimmed("ignored")
///     .build(vec![])
/// ```
#[derive(Clone, Debug)]
pub struct TreeItem {
    key: Key,
    label: String,
    branch: TreeBranch,
    selected: bool,
    height: f32,
    icon: Option<ViewNode>,
    mark: Option<ViewNode>,
    dimmed: Option<String>,
    place: Option<TreePlace>,
}

impl TreeItem {
    /// A leaf at Carbon small (32), not selected, with no slots.
    #[must_use]
    pub fn new(key: impl Into<Key>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            branch: TreeBranch::Leaf,
            selected: false,
            height: HEIGHT,
            icon: None,
            mark: None,
            dimmed: None,
            place: None,
        }
    }

    /// Whether the row has a caret, and which way it points.
    #[must_use]
    pub fn branch(mut self, branch: TreeBranch) -> Self {
        self.branch = branch;
        self
    }

    /// Whether the row is the selected one.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Carbon extra-small (24) instead of small (32).
    #[must_use]
    pub fn extra_small(mut self) -> Self {
        self.height = HEIGHT_XS;
        self
    }

    /// A node between the caret and the label. The caller sizes it. A
    /// dimmed row draws it at half opacity.
    #[must_use]
    pub fn icon(mut self, icon: ViewNode) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A node at the row's trailing edge, right-aligned. It never shrinks:
    /// a long label gives way to it.
    ///
    /// A mark must carry a glyph shape or text of its own. A colour alone
    /// is not a channel the operator can read.
    #[must_use]
    pub fn mark(mut self, mark: ViewNode) -> Self {
        self.mark = Some(mark);
        self
    }

    /// The dimmed state: a quiet row that is still there. The icon draws at
    /// half opacity, the label takes `text.muted`, and `state` joins the
    /// semantic label, so a reader who cannot see the lightness still
    /// hears it (`"target, ignored"`).
    #[must_use]
    pub fn dimmed(mut self, state: impl Into<String>) -> Self {
        self.dimmed = Some(state.into());
        self
    }

    /// Place this row in a flattened hierarchy: indent it to `level`, and
    /// declare `level`, `pos_in_set`, and `set_size` in its semantics.
    ///
    /// An eager tree does not need this, because [`tree_item`] nests its
    /// children and stamps the same numbers itself. A virtualized tree
    /// ([`tree_rows`]) holds no nesting, so each row says where it sits.
    #[must_use]
    pub fn at(mut self, place: TreePlace) -> Self {
        self.place = Some(place);
        self
    }

    /// Build the row. `children` mount only when the branch is
    /// [`TreeBranch::Open`].
    #[must_use]
    pub fn build(self, children: Vec<ViewNode>) -> ViewNode {
        let Self {
            key,
            label,
            branch,
            selected,
            height,
            icon,
            mark,
            dimmed,
            place,
        } = self;

        // The bar keeps its width in every state; only its fill comes and
        // goes, so the label never shifts when the selection moves.
        let bar_fill = selected.then_some(ACCENT_PRIMARY);
        let disclosure = if branch.is_branch() {
            caret(
                "caret",
                if branch.is_open() {
                    CaretDirection::Down
                } else {
                    CaretDirection::Right
                },
            )
        } else {
            swatch("caret-slot", CARET_SIZE, CARET_SIZE, None, None, None)
        };

        let mut label_node = super::text::text("label", label.clone()).with_focus_run();
        // One line. A long name ends in an ellipsis instead of wrapping into
        // a second line that the pinned row height would clip.
        label_node.props.wrap = Some(TextWrap::Ellipsis);
        if dimmed.is_some() {
            label_node
                .props
                .tokens
                .insert("foreground".into(), t(TEXT_MUTED));
        }

        let level_lead = place.map_or(0.0, |p| p.level.saturating_sub(1) as f32 * LEVEL_INDENT);
        let mut lead = swatch("lead", LEAD, height, None, None, None);
        if level_lead > 0.0 {
            pin_width(&mut lead, LEAD + level_lead);
        }

        let mut parts = vec![
            swatch("bar", ACCENT_BAR, height, bar_fill, None, None),
            lead,
            disclosure,
            swatch("gap", CARET_GAP, height, None, None, None),
        ];
        if let Some(mut icon) = icon {
            icon.key = Key::new("icon");
            if dimmed.is_some() {
                icon.props.opacity = Some(DIMMED_ICON_OPACITY);
            }
            parts.push(icon);
            parts.push(swatch("icon-gap", SLOT_GAP, height, None, None, None));
        }
        if let Some(mut mark) = mark {
            mark.key = Key::new("mark");
            // Priority orders who is satisfied first (`contracts/view-tree.md`
            // steps 3.a and 3.b): the mark, then the label, then the rest. The mark
            // keeps its whole width and a long name gives way to it.
            //
            // The label needs a priority of its own because of the spacer.
            // In an equal-share walk the label and the spacer split the room
            // the mark leaves, and a spacer takes any share it is offered, so
            // a label that fit with room to spare stopped at half the row.
            // As the only member of its group the label is offered every
            // unit the mark and the fixed parts leave, takes what it needs,
            // and the spacer gets the rest.
            label_node.constraints.horizontal.priority = 1;
            mark.constraints.horizontal.priority = 2;
            parts.push(label_node);
            parts.push(ViewNode::new(NodeKind::Spacer, "spacer"));
            parts.push(swatch("mark-gap", SLOT_GAP, height, None, None, None));
            parts.push(mark);
        } else {
            parts.push(label_node);
        }

        let mut row = stack("row", Axis::Horizontal, None, parts);
        row.props.align = Some(Align::Center);
        row.props.padding = Some(InsetRefs {
            right: Some(t(SPACING_05)),
            ..InsetRefs::default()
        });
        row.constraints.vertical = AxisConstraint {
            min: Some(height),
            max: Some(height),
            priority: 0,
        };
        // The item below holds focus and its rect spans the whole expanded
        // subtree, so its own underline would land beneath the last grandchild.
        // Carbon puts the ring here instead — `.cds--tree-node:focus >
        // .cds--tree-node__label`, `_treeview.scss:59` — and this pair of
        // declarations is that selector.
        row.semantics.focus_shown_on = FocusShownOn::Head;
        // The figure is read off the node focus is *shown on*, not the node that
        // holds it (`gorgon-petra-egui`'s `focused_caret_target`), so the head
        // row carries it and the item's own declaration below is only there to
        // keep the two from disagreeing if the `OnHead` link is ever dropped.
        // `BarInside` for the reason the item gives: tree rows stack flush.
        row.semantics.focus_figure = FocusFigure::BarInside;

        let mut parts = vec![row];
        if branch.is_open() && !children.is_empty() {
            // The level's indent goes into each child's row, not onto this
            // container as padding: a padded container would start the
            // child's fill (and its accent bar) 16 in, where Carbon's node
            // spans the whole tree and only its content steps in.
            let mut children: Vec<ViewNode> = children;
            for child in &mut children {
                step_in(child);
            }
            stamp_set(&mut children);
            let mut nest = stack("children", Axis::Vertical, None, children);
            nest.props.align = Some(Align::Stretch);
            parts.push(nest);
        }

        // V6: a single-column `Grid` with a `Weight` track, not a plain
        // `Stack` with `Align::Stretch`. Re-capturing after the `Stretch`-only
        // attempt showed the fill widen (65px to 130px, matching its sibling
        // "gorgon" row) but stop well short of the tree's own width: a Stack's
        // cross-axis `Stretch` only offers a child up to the available extent
        // as a maximum, it does not force consumption of it, so a plain-Stack
        // item still settles at its content width. A `Weight` grid track does
        // force it — the exact shape `ui_shell_left_panel_row`'s `body` cell
        // already proves in this same file's sibling `ui_shell.rs` (its
        // selected fill measurably spans the full 256px panel, not its own
        // text). `rows` are `FitContent`: only the column needs to claim the
        // full width, the two stacked rows (`row`, optional `children`) stay
        // sized to their own content height.
        //
        // R8: `align: Align::Stretch` on that grid, added 2026-09-05. The
        // `Weight` track made the *item* span the tree, which is where the
        // `background@selected` fill is bound, so the highlighted band was
        // full width. The head `row` inside the cell was not: a grid places a
        // non-`Stretch` child by *measuring* it against the cell and offsetting
        // the answer inside the box (`layout/grid.rs::place_in_cell`), and a
        // Horizontal stack measures to its content. So the row settled at its
        // label's width, and the focus indicator — which is drawn on the row,
        // because the item declares `FocusShownOn::OnHead` — stopped short of
        // the band the operator can see. `Stretch` is the arm that skips the
        // measurement and imposes the cell outright. The existing
        // `rows_pin_their_height_and_labels_align_across_branch_and_leaf`
        // asserted `item.w == tree.w`, which is the *fill's* node and was
        // already true; it never looked at the row.
        // `the_head_row_spans_the_band_the_selection_fills` does.
        let row_count = parts.len();
        let mut node = ViewNode::new(NodeKind::Grid, key)
            .with_props(Props {
                columns: vec![TrackSize::Weight { weight: 1.0 }],
                rows: vec![TrackSize::FitContent; row_count],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(parts);
        for (slot, token) in [
            ("background", SURFACE_BASE),
            ("background@hover", LAYER_HOVER),
            ("background@selected", LAYER_SELECTED),
            ("background@selected-hover", LAYER_SELECTED_HOVER),
        ] {
            node.props.tokens.insert(slot.into(), t(token));
        }
        let semantic_label = match &dimmed {
            Some(state) => format!("{label}, {state}"),
            None => label,
        };
        let mut node = node
            .interactive(Role::TreeItem, semantic_label, ITEM_INTENTS)
            .with_behaviour(SELECTS_ON_RELEASE);
        node.semantics.focus_shown_on = FocusShownOn::OnHead;
        // `BarInside`: tree rows stack flush, so the default bar — five units
        // below the bottom edge — would land on the row below. This is the same
        // underline seated on the row's own bottom edge, so it cannot.
        //
        // It also stays clear of the selected row's own mark, which a ring did
        // not: the accent bar runs down the row's **leading** edge, and a ring's
        // left band runs down exactly those pixels.
        //
        // "Stays clear" is the `with_focus_run` on the label above, and it was
        // not always. Until 2026-09-06 this comment claimed a stripe on the
        // bottom edge "shares nothing" with the accent bar, on the strength of
        // the stripe being as wide as the label and so starting well to the
        // right of it. `focus::marked_rect` then changed to span the control's
        // **content run**, and the row's leading `swatch("bar", ...)` -- the
        // selection mark itself -- is a childless leaf and so counted as
        // content. The stripe started on top of the accent bar, the two marks
        // met at the row's bottom-left corner, and a row that was both selected
        // and focused wore one L-shaped bracket instead of two marks.
        //
        // The operator photographed it the same day: a 4-wide bar down rows
        // 130..158 and a 3-tall stripe across rows 159..161, both starting at
        // x = 52. A selection indicator is not content, and no geometric rule
        // can know that -- a checkbox's box is also a childless leaf and must
        // be included, or the bar jumps 24 units when the box is ticked. So the
        // row declares it, which is what `Semantics::focus_run` is for.
        node.semantics.focus_figure = FocusFigure::BarInside;
        node.semantics.selected = selected;
        // A leaf has nothing to expand, so it declares no state, as
        // WAI-ARIA has it. A branch says open or closed.
        node.semantics.expanded = branch.is_branch().then_some(branch.is_open());
        match place {
            Some(place) => {
                node.semantics.level = Some(place.level);
                node.semantics.pos_in_set = Some(place.pos_in_set);
                node.semantics.set_size = Some(place.set_size);
            }
            // An eager item is level 1 until a parent nests it.
            None => node.semantics.level = Some(1),
        }
        node
    }
}

/// A tree of items. `Role::Tree`, no interactions.
///
/// V6: `align = Align::Stretch` — without it every top-level [`tree_item`]
/// measured to its own label width instead of the tree's, so a selected
/// item's fill (bound on the item's own outer node) stopped at its own text
/// rather than spanning the row, the same content-sized-instead-of-full-
/// width class V2 fixed one level up in `data_table`'s row list.
///
/// Items declare their place among siblings (`pos_in_set`, `set_size`) the
/// way [`tree_item`] declares it for the items it nests.
pub fn tree_view(key: impl Into<Key>, mut nodes: Vec<ViewNode>) -> ViewNode {
    stamp_set(&mut nodes);
    let mut node = stack(key, Axis::Vertical, None, nodes);
    node.props.align = Some(Align::Stretch);
    node.semantics = Semantics {
        role: Some(Role::Tree),
        ..Semantics::default()
    };
    node
}

/// The virtualized form of [`tree_view`]: a `Role::Tree` scroll over a
/// [`NodeKind::Collection`] of `total_count` flat rows, so only the rows
/// near the viewport are built.
///
/// `label` names the tree for a reader ("Files"). `source` names the host's row source ([`crate::layout::RowSource`]). It
/// answers each row as `TreeItem::at(..).build(vec![])`. Those rows draw the
/// same pixels the eager tree draws for the same rows, and they declare
/// `level`, `pos_in_set`, and `set_size`, because a flat list holds no
/// nesting for a reader to walk.
///
/// The host owns the flattening: which rows are open, and what the total
/// is. The collection draws the total as its extent, so a total that
/// disagrees with the source draws a short tree.
pub fn tree_rows(
    key: impl Into<Key>,
    label: impl Into<String>,
    source: impl Into<String>,
    total_count: usize,
) -> ViewNode {
    tree_rows_sized(key, label, source, total_count, HEIGHT)
}

/// The optional parts of a [`TreeItem`] row, gathered in one value so a
/// registered constructor can take them as one argument.
///
/// Every field is a [`TreeItem`] method: [`TreeItem::icon`],
/// [`TreeItem::mark`], [`TreeItem::dimmed`], [`TreeItem::at`]. A `None` leaves
/// the row without that part.
#[derive(Clone, Debug, Default)]
pub struct TreeAdornments {
    /// The node between the caret and the label.
    pub icon: Option<ViewNode>,
    /// The node at the trailing edge.
    pub mark: Option<ViewNode>,
    /// The state text of a dimmed row.
    pub dimmed: Option<String>,
    /// Where a flat row sits in its hierarchy.
    pub place: Option<TreePlace>,
}

/// One tree row with every option [`TreeItem`] has, as a plain function.
///
/// **The registered form of [`TreeItem`].** A Lua plugin cannot hold a Rust
/// builder, so this is what `ui.tree_row{...}` calls. It is how a plugin
/// builds a closed branch with nothing mounted, a dimmed row, a row with a
/// trailing mark, or one flat row of a [`tree_rows`] source. `extra_small`
/// is Carbon extra-small (24) and not small (32). `children` mount only when
/// `branch` is [`TreeBranch::Open`].
#[must_use]
pub fn tree_row(
    key: impl Into<Key>,
    label: impl Into<String>,
    branch: TreeBranch,
    selected: bool,
    extra_small: bool,
    adornments: TreeAdornments,
    children: Vec<ViewNode>,
) -> ViewNode {
    let TreeAdornments {
        icon,
        mark,
        dimmed,
        place,
    } = adornments;
    let mut item = TreeItem::new(key, label).branch(branch).selected(selected);
    if extra_small {
        item = item.extra_small();
    }
    if let Some(icon) = icon {
        item = item.icon(icon);
    }
    if let Some(mark) = mark {
        item = item.mark(mark);
    }
    if let Some(state) = dimmed {
        item = item.dimmed(state);
    }
    if let Some(place) = place {
        item = item.at(place);
    }
    item.build(children)
}

/// [`tree_rows`] for rows built with [`TreeItem::extra_small`] (24).
pub fn tree_rows_xs(
    key: impl Into<Key>,
    label: impl Into<String>,
    source: impl Into<String>,
    total_count: usize,
) -> ViewNode {
    tree_rows_sized(key, label, source, total_count, HEIGHT_XS)
}

fn tree_rows_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    source: impl Into<String>,
    total_count: usize,
    row_extent: f32,
) -> ViewNode {
    let rows = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
        total_count: Some(total_count),
        source: Some(source.into()),
        estimated_extent: Some(row_extent),
        ..Props::default()
    });
    // A `Scroll` node takes the wheel only when it declares it, so the
    // virtualized tree does, and a declared interaction needs a name.
    ViewNode::new(NodeKind::Scroll, key)
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(rows)
        .interactive(Role::Tree, label, &[Interaction::Scroll])
}

/// One tree item at Carbon small (32).
///
/// `label` is required (FR-058). Nested `children` are mounted only when
/// `expanded` is true. A node with no children is a leaf and grows no
/// caret. A branch the caller knows is a branch, with nothing mounted
/// under it, is [`TreeItem`] with [`TreeBranch::Closed`].
pub fn tree_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    implied_item(key, label, expanded, selected, children, HEIGHT)
}

/// [`tree_item`] at Carbon extra-small (24).
pub fn tree_item_xs(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    implied_item(key, label, expanded, selected, children, HEIGHT_XS)
}

fn implied_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
    height: f32,
) -> ViewNode {
    let mut item = TreeItem::new(key, label)
        .branch(TreeBranch::implied(!children.is_empty(), expanded))
        .selected(selected);
    item.height = height;
    item.build(children)
}

/// Pin a leaf spacer's width.
fn pin_width(node: &mut ViewNode, width: f32) {
    node.constraints.horizontal.min = Some(width);
    node.constraints.horizontal.max = Some(width);
}

/// Declare each tree item in `nodes` as one of a set, numbering them
/// from 1. Nodes that are not tree items take no number and are not
/// counted.
fn stamp_set(nodes: &mut [ViewNode]) {
    let size = nodes
        .iter()
        .filter(|n| n.semantics.role == Some(Role::TreeItem))
        .count();
    let mut pos = 0u32;
    for node in nodes
        .iter_mut()
        .filter(|n| n.semantics.role == Some(Role::TreeItem))
    {
        pos += 1;
        node.semantics.pos_in_set = Some(pos);
        node.semantics.set_size = Some(u32::try_from(size).unwrap_or(u32::MAX));
    }
}

/// Step `item` in one level: widen its row's `lead` spacer by one
/// level of indent and add one to its `level`, and do the same for
/// everything already nested under it. The item's own rect is untouched,
/// so its fill and accent bar keep spanning the tree. A child is built
/// before its parent, so this is the parent's stamp on it, the same shape
/// as `list.rs`'s `stamp_items`.
fn step_in(item: &mut ViewNode) {
    if let Some(level) = &mut item.semantics.level {
        *level += 1;
    }
    for child in &mut item.children {
        let child = Arc::make_mut(child);
        match child.key.as_str() {
            "row" => {
                for part in &mut child.children {
                    if part.key.as_str() == "lead" {
                        let lead = Arc::make_mut(part);
                        let width = lead.constraints.horizontal.min.unwrap_or(0.0) + LEVEL_INDENT;
                        pin_width(lead, width);
                    }
                }
            }
            "children" => {
                for grandchild in &mut child.children {
                    step_in(Arc::make_mut(grandchild));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT_BAR, ACCENT_PRIMARY, DIMMED_ICON_OPACITY, FocusShownOn, HEIGHT, HEIGHT_XS,
        LABEL_START, TreeBranch, TreeItem, TreePlace, tree_item, tree_item_xs, tree_rows,
        tree_view,
    };
    use crate::component::disabled;
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::LAYER_SELECTED;
    use crate::frame::PetrifiedFrame;
    use crate::geom::Rect;

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{Behaviour, Intent, Interaction, NodeKind, Phase, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn has_key(node: &ViewNode, key: &str) -> bool {
        if node.key.as_str() == key {
            return true;
        }
        node.children.iter().any(|child| has_key(child, key))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn no_drag(node: &ViewNode) {
        assert!(
            !node.interactions.contains(&Interaction::Drag),
            "tree item `{}` declared Drag; Carbon ships no reorder",
            node.key
        );
        for child in &node.children {
            no_drag(child);
        }
    }

    #[test]
    fn tree_view_sets_role_tree() {
        let node = tree_view("fs", vec![tree_item("root", "src", true, false, vec![])]);
        assert_eq!(node.semantics.role, Some(Role::Tree));
        assert!(node.interactions.is_empty());
        assert_eq!(child_role(&node, "root"), Some(Role::TreeItem));
    }

    fn child_role(node: &ViewNode, key: &str) -> Option<Role> {
        named(node, key).semantics.role.clone()
    }

    /// The focus ring is declared on the item and drawn on its head row.
    ///
    /// Carbon's selector is `.cds--tree-node:focus > .cds--tree-node__label`
    /// (`_treeview.scss:59`), with `:focus { outline: none }` on the node
    /// itself the line above. This pair of figures is that selector, and it
    /// matters because an expanded item's rect spans its whole subtree: the
    /// item's own underline would land under the last grandchild.
    ///
    /// Every item declares it, leaf or branch. A leaf's rect is its row, so
    /// the two agree there and the declaration costs nothing; a leaf that
    /// later grows children would otherwise start pointing at its subtree.
    #[test]
    fn a_tree_item_shows_focus_on_its_head_row() {
        for (expanded, children) in [
            (false, vec![]),
            (true, vec![tree_item("lib", "lib.rs", false, false, vec![])]),
        ] {
            let node = tree_item("src", "src", expanded, false, children);
            assert_eq!(
                node.semantics.focus_shown_on,
                FocusShownOn::OnHead,
                "the item points at its head row (expanded {expanded})"
            );
            assert_eq!(
                named(&node, "row").semantics.focus_shown_on,
                FocusShownOn::Head,
                "the head row is the one pointed at (expanded {expanded})"
            );
        }
    }

    /// Row 39's defect: disclosure was the literal word `expanded` placed
    /// before the label. A branch now draws a caret canvas that points a
    /// different way in each state, a leaf keeps the slot empty, and
    /// `Semantics.expanded` carries the fact either way.
    #[test]
    fn tree_item_declares_expanded_and_selected_never_colour_only() {
        let on = tree_item(
            "src",
            "src",
            true,
            true,
            vec![tree_item("lib", "lib.rs", false, false, vec![])],
        );
        assert_eq!(on.semantics.role, Some(Role::TreeItem));
        assert_eq!(on.semantics.label.as_deref(), Some("src"));
        assert_eq!(on.semantics.expanded, Some(true));
        assert!(on.semantics.selected);
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        let open_caret = named(named(&on, "row"), "caret");
        assert_eq!(open_caret.kind, NodeKind::Canvas, "a branch draws a caret");
        assert!(
            open_caret.props.text.is_none(),
            "the caret is a shape, not the word `expanded`"
        );
        assert_eq!(named(&on, "label").props.text.as_deref(), Some("src"));
        assert!(has_key(&on, "lib"));
        assert_eq!(named(&on, "row").constraints.vertical.min, Some(HEIGHT));
        assert_eq!(named(&on, "row").constraints.vertical.max, Some(HEIGHT));
        assert_eq!(HEIGHT, 32.0);

        let off = tree_item(
            "src",
            "src",
            false,
            false,
            vec![tree_item("lib", "lib.rs", false, false, vec![])],
        );
        assert_eq!(off.semantics.expanded, Some(false));
        assert!(!off.semantics.selected);
        let shut_caret = named(named(&off, "row"), "caret");
        assert_eq!(shut_caret.kind, NodeKind::Canvas);
        assert_ne!(
            open_caret.props.canvas, shut_caret.props.canvas,
            "expanded and collapsed carets point different ways"
        );
        assert!(
            !has_key(&off, "lib"),
            "a collapsed branch does not mount its children"
        );
        no_drag(&on);
        no_drag(&off);
    }

    /// Selection is a shape as well as a fill: the 4-unit accent bar at
    /// the row's start is filled on a selected row and empty otherwise,
    /// and it keeps its width in both states so the label does not move.
    #[test]
    fn a_selected_row_carries_an_accent_bar_beside_its_fill() {
        let on = tree_item("l", "main.rs", false, true, vec![]);
        let off = tree_item("l", "lib.rs", false, false, vec![]);
        let on_bar = named(&on, "bar");
        let off_bar = named(&off, "bar");
        assert_eq!(token(on_bar, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(token(off_bar, "background"), None);
        assert_eq!(on_bar.constraints.horizontal.min, Some(ACCENT_BAR));
        assert_eq!(off_bar.constraints.horizontal.min, Some(ACCENT_BAR));
        assert_eq!(on_bar.constraints.vertical.min, Some(HEIGHT));
        assert_eq!(ACCENT_BAR, 4.0);
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
    }

    #[test]
    fn tree_items_do_not_declare_drag() {
        let tree = tree_view(
            "fs",
            vec![
                tree_item(
                    "src",
                    "src",
                    true,
                    false,
                    vec![tree_item("main", "main.rs", false, true, vec![])],
                ),
                tree_item("docs", "docs", false, false, vec![]),
            ],
        );
        no_drag(&tree);
        let leaf = tree_item("leaf", "README", false, false, vec![]);
        assert!(!has_key(&leaf, "caret"), "a leaf has no expand caret");
        assert!(
            has_key(&leaf, "caret-slot"),
            "a leaf keeps the caret's slot so its label lines up with a branch's"
        );
    }

    #[test]
    fn extra_small_item_is_24() {
        let node = tree_item_xs("n", "node", false, false, vec![]);
        assert_eq!(
            named(&node, "row").constraints.vertical.min,
            Some(HEIGHT_XS)
        );
        assert_eq!(
            named(&node, "row").constraints.vertical.max,
            Some(HEIGHT_XS)
        );
        assert_eq!(HEIGHT_XS, 24.0);
        assert_eq!(
            named(&node, "bar").constraints.vertical.min,
            Some(HEIGHT_XS)
        );
    }

    fn rect_of(frame: &PetrifiedFrame, suffix: &str) -> Rect {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("no placement ending in {suffix}"))
            .rect
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// A two-deep tree exercising every documented state at once: an
    /// expanded branch (`src`) holding a selected leaf (`main`) and a plain
    /// leaf (`lib`), a collapsed branch (`build`, so its own child never
    /// mounts), and a disabled leaf (`archived`).
    fn sample_tree() -> ViewNode {
        tree_view(
            "fs",
            vec![
                tree_item(
                    "src",
                    "src",
                    true,
                    false,
                    vec![
                        tree_item("main", "main.rs", false, true, vec![]),
                        tree_item("lib", "lib.rs", false, false, vec![]),
                    ],
                ),
                tree_item(
                    "build",
                    "build",
                    false,
                    false,
                    vec![tree_item("out", "out.o", false, false, vec![])],
                ),
                disabled(tree_item("archived", "archived", false, false, vec![])),
            ],
        )
    }

    /// Row 39 on the placed frame: every row is exactly 32 tall and as
    /// wide as the tree; a level-1 label starts 40 in whether its row is a
    /// branch or a leaf, a level-2 label 16 further; the selected row's
    /// accent bar is 4 wide, full row height, at the row's start.
    #[test]
    fn rows_pin_their_height_and_labels_align_across_branch_and_leaf() {
        let frame = petrify_lone(sample_tree());
        let tree = rect_of(&frame, "/fs");
        for row in [
            "/src/row",
            "/main/row",
            "/lib/row",
            "/build/row",
            "/archived/row",
        ] {
            let r = rect_of(&frame, row);
            assert_eq!(r.h, HEIGHT, "{row}: row height is pinned");
        }
        for item in ["/fs/src", "/fs/build", "/fs/archived"] {
            let r = rect_of(&frame, item);
            assert_eq!(r.w, tree.w, "{item}: a row spans the tree's width");
        }
        let branch_label = rect_of(&frame, "/src/row/label");
        let leaf_label = rect_of(&frame, "/archived/row/label");
        assert_eq!(
            branch_label.x - tree.x,
            LABEL_START,
            "a level-1 branch label starts at Carbon's 40"
        );
        assert_eq!(
            leaf_label.x, branch_label.x,
            "a level-1 leaf label lines up with a level-1 branch label"
        );
        let nested_label = rect_of(&frame, "/main/row/label");
        assert_eq!(
            nested_label.x - branch_label.x,
            16.0,
            "one level of nesting indents by spacing-05"
        );
        let caret = rect_of(&frame, "/src/row/caret");
        assert_eq!(
            caret.x - tree.x,
            16.0,
            "the caret box starts at the branch indent"
        );
        let selected = rect_of(&frame, "/fs/src/children/main");
        assert_eq!(
            selected.w, tree.w,
            "a nested row spans the tree; only its content steps in"
        );
        assert_eq!(selected.x, tree.x);
        let bar = rect_of(&frame, "/main/row/bar");
        assert_eq!(bar.x, tree.x, "the accent bar sits at the tree's edge");
        assert_eq!(bar.w, ACCENT_BAR);
        assert_eq!(bar.h, HEIGHT, "the accent bar spans the full row height");
    }

    /// R8: the head row spans the same band the selection fill covers, so
    /// an indicator drawn on that row lines up with the highlight.
    ///
    /// The operator: "In tree view the cursor doesn't position itself well
    /// on any of these elements" (2026-09-05). The item declares
    /// `FocusShownOn::OnHead`, so every figure is drawn on `"row"` and not
    /// on the item; the fill is bound on the item. Before `Align::Stretch`
    /// went on the item's grid, the row measured to its own content and the
    /// indicator stopped short of the band by whatever the label left over.
    ///
    /// `rows_pin_their_height_and_labels_align_across_branch_and_leaf`
    /// already asserted `item.w == tree.w`. That is the *fill's* node and it
    /// was true the whole time; it never looked at the row. This does, and
    /// asserts both edges rather than the width alone, because two rects of
    /// equal width can still be offset from one another.
    ///
    /// Falsify by dropping `align: Some(Align::Stretch)` from
    /// `tree_item_sized`'s grid.
    #[test]
    fn the_head_row_spans_the_band_the_selection_fills() {
        let frame = petrify_lone(sample_tree());
        for item in ["/fs/src", "/fs/build", "/fs/archived"] {
            let fill = rect_of(&frame, item);
            let row = rect_of(&frame, &format!("{item}/row"));
            assert_eq!(
                row.x, fill.x,
                "{item}: the head row starts left of the band the fill covers"
            );
            assert_eq!(
                row.right(),
                fill.right(),
                "{item}: the head row stops {} short of the band's right edge",
                fill.right() - row.right()
            );
        }
        // A nested row too: the indent widens `lead` rather than padding the
        // container, so a level-2 row must still reach both edges.
        let nested_fill = rect_of(&frame, "/fs/src/children/main");
        let nested_row = rect_of(&frame, "/fs/src/children/main/row");
        assert_eq!(nested_row.x, nested_fill.x);
        assert_eq!(nested_row.right(), nested_fill.right());
        assert_eq!(
            nested_row.h, HEIGHT,
            "stretching the cell must not stretch the row past its pinned height"
        );
    }

    /// The item points at its head row, and the head row is the marker it
    /// points at. Two declarations, one selector.
    #[test]
    fn a_tree_item_shows_its_focus_on_its_head_row() {
        let item = tree_item(
            "src",
            "src",
            true,
            false,
            vec![tree_item("a", "a.rs", false, false, vec![])],
        );
        assert_eq!(item.semantics.focus_shown_on, FocusShownOn::OnHead);
        assert_eq!(
            named(&item, "row").semantics.focus_shown_on,
            FocusShownOn::Head
        );
        let xs = tree_item_xs("leaf", "leaf", false, false, vec![]);
        assert_eq!(xs.semantics.focus_shown_on, FocusShownOn::OnHead);
        assert_eq!(
            named(&xs, "row").semantics.focus_shown_on,
            FocusShownOn::Head
        );
    }

    /// Check C/D: every placement across a mixed expanded/collapsed/
    /// selected/disabled tree at two depths places with a real rect and
    /// draws no content larger than it.
    #[test]
    fn tree_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(sample_tree());
        assert!(!frame.placements.is_empty(), "nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{} drew content larger than its own rect",
                p.id
            );
            assert!(
                !p.paint.truncated,
                "{} was truncated to fit its parent",
                p.id
            );
            if let Some(parent_idx) = p.parent {
                assert_fits_parent("", p, &frame.placements[parent_idx]);
            }
        }
    }

    /// Check F: every enabled item (branch or leaf, selected or not)
    /// declares `Focus` and is reachable; the disabled leaf is not, and a
    /// collapsed branch's never-mounted child is simply absent, not merely
    /// unreachable.
    #[test]
    fn tree_focus_reachability_matches_disabled_state() {
        let frame = petrify_lone(sample_tree());
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let reachable = |suffix: &str| {
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"));
            focus.order().iter().any(|o| o == &placement.id)
        };
        assert!(reachable("/src"), "expanded branch must be reachable");
        assert!(reachable("/main"), "selected leaf must be reachable");
        assert!(reachable("/lib"), "plain leaf must be reachable");
        assert!(reachable("/build"), "collapsed branch must be reachable");
        assert!(
            !frame.placements.iter().any(|p| p.id.ends_with("/out")),
            "a collapsed branch's child must not mount at all"
        );
        assert!(
            !reachable("/archived"),
            "disabled leaf must not be focus reachable"
        );
    }

    /// Check E: every item's label against its own resting fill
    /// ([`SURFACE_BASE`]), in both themes — selected and unselected alike,
    /// since `LAYER_SELECTED` still needs to clear AA for the label sitting
    /// on it.
    #[test]
    fn tree_item_labels_clear_aa_contrast_against_their_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node) in [
                (
                    "collapsed-branch",
                    tree_item("b", "src", false, false, vec![]),
                ),
                (
                    "expanded-branch",
                    tree_item(
                        "b",
                        "src",
                        true,
                        false,
                        vec![tree_item("c", "child", false, false, vec![])],
                    ),
                ),
                (
                    "selected-leaf",
                    tree_item("l", "main.rs", false, true, vec![]),
                ),
                ("plain-leaf", tree_item("l", "lib.rs", false, false, vec![])),
            ] {
                let bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: item binds a resting background"));
                let bg = color(&theme, bg_name.as_str());
                let row = named(&node, "row");
                let text_label = named(row, "label");
                let fg_name = text_label
                    .props
                    .tokens
                    .get("foreground")
                    .unwrap_or_else(|| panic!("{label}: label binds a foreground"));
                let opacity = text_label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    bg_name.as_str()
                );
            }
        }
    }

    /// Spec 010: a tree item declares Select / OnRelease.
    ///
    /// Falsified by dropping `.with_behaviour(...)` from the item builder:
    ///
    /// ```text
    /// tree_item declared behaviour None
    /// ```
    #[test]
    fn tree_item_declares_select_on_release() {
        let node = tree_item("leaf", "Kernel", false, false, Vec::new());
        assert_eq!(
            node.behaviour,
            Some(Behaviour {
                intent: Intent::Select,
                phase: Phase::OnRelease,
            }),
            "tree_item declared behaviour {:?}",
            node.behaviour
        );
    }

    // ---------------------------------------------------------------
    // The slots, the dimmed state, and the flat form (spec 016, T038, T040)
    // ---------------------------------------------------------------

    /// A 16-square stand-in for any icon a caller brings.
    fn glyph(key: &str) -> ViewNode {
        crate::component::swatch(
            key,
            16.0,
            16.0,
            Some(crate::component::tokens::ICON_PRIMARY),
            None,
            None,
        )
    }

    /// A text mark: a word, so it carries a channel that is not colour.
    fn word(key: &str, text: &str) -> ViewNode {
        crate::component::text(key, text)
    }

    /// The four grounds a tree row paints on.
    const ROW_GROUNDS: [&str; 4] = [
        crate::component::tokens::SURFACE_BASE,
        crate::component::tokens::LAYER_HOVER,
        crate::component::tokens::LAYER_SELECTED,
        crate::component::tokens::LAYER_SELECTED_HOVER,
    ];

    /// A dimmed row is a lightness cue in three channels, and the word.
    ///
    /// The icon fades through `Props.opacity`. The label takes the
    /// `text.muted` token (not an opacity, because a faded label has no
    /// measured floor). The semantic label carries the state text. An
    /// undimmed row has none of the three, so the test also holds the
    /// default.
    ///
    /// Falsify by dropping the `dimmed.is_some()` branch for the icon: the
    /// opacity assertion fails with `None`.
    #[test]
    fn a_dimmed_row_fades_its_icon_mutes_its_label_and_says_why() {
        let quiet = TreeItem::new("target", "target")
            .branch(TreeBranch::Closed)
            .icon(glyph("whatever"))
            .dimmed("ignored")
            .build(vec![]);
        let loud = TreeItem::new("src", "src")
            .branch(TreeBranch::Closed)
            .icon(glyph("whatever"))
            .build(vec![]);

        let icon = named(&quiet, "icon");
        assert_eq!(
            icon.props.opacity,
            Some(DIMMED_ICON_OPACITY),
            "the dimmed icon draws below full opacity"
        );
        const { assert!(DIMMED_ICON_OPACITY < 1.0) };
        assert_eq!(
            token(named(&quiet, "label"), "foreground"),
            Some(crate::component::tokens::TEXT_MUTED),
            "the dimmed label takes text.muted"
        );
        assert_eq!(
            named(&quiet, "label").props.opacity,
            None,
            "the label is muted by token, never faded"
        );
        assert_eq!(quiet.semantics.label.as_deref(), Some("target, ignored"));
        assert_eq!(
            named(&quiet, "label").props.text.as_deref(),
            Some("target"),
            "the drawn text stays the name"
        );

        assert_eq!(named(&loud, "icon").props.opacity, None);
        assert_eq!(
            token(named(&loud, "label"), "foreground"),
            Some(crate::component::tokens::TEXT_PRIMARY)
        );
        assert_eq!(loud.semantics.label.as_deref(), Some("src"));
    }

    /// Muted text keeps the token gates' minimum on every ground a row can
    /// sit on: resting, hovered, selected, selected and hovered.
    ///
    /// Petra ships two themes. `light` carries the White and Gray 10 values
    /// and `dark` the Gray 90 and Gray 100 values (`token-vocabulary.md`
    /// section 2), so sweeping both covers the four Carbon themes' polarity
    /// and tone. The floor is 4.5, the same number
    /// `tree_item_labels_clear_aa_contrast_against_their_own_fill` holds.
    #[test]
    fn dimmed_muted_text_clears_the_text_contrast_floor_on_every_row_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        let dimmed = TreeItem::new("target", "target")
            .dimmed("ignored")
            .build(vec![]);
        let fg_name = token(named(&dimmed, "label"), "foreground").expect("a foreground");
        for (mode, theme) in [
            ("light", crate::token::light()),
            ("dark", crate::token::dark()),
        ] {
            let fg = color(&theme, fg_name);
            for ground in ROW_GROUNDS {
                let bg = color(&theme, ground);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{mode}: dimmed label ({fg_name}) on {ground} is {ratio:.2}:1, \
                     under the {MIN_TEXT_CONTRAST}:1 floor"
                );
            }
        }
    }

    /// The slots take any element and give it a stable key, so a row's
    /// node paths do not depend on what the caller passed.
    #[test]
    fn the_icon_and_mark_slots_seat_any_element_under_fixed_keys() {
        let row = TreeItem::new("n", "name")
            .icon(glyph("something-else"))
            .mark(word("anything", "submodule"))
            .build(vec![]);
        let parts: Vec<&str> = named(&row, "row")
            .children
            .iter()
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(
            parts,
            [
                "bar",
                "lead",
                "caret-slot",
                "gap",
                "icon",
                "icon-gap",
                "label",
                "spacer",
                "mark-gap",
                "mark"
            ]
        );
        let bare = TreeItem::new("n", "name").build(vec![]);
        let parts: Vec<&str> = named(&bare, "row")
            .children
            .iter()
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(
            parts,
            ["bar", "lead", "caret-slot", "gap", "label"],
            "a row with no slot has no slot parts, so it draws what it always drew"
        );
    }

    /// A mark sits at the row's trailing edge, right-aligned, whole, and a
    /// long label gives way to it with an ellipsis instead of pushing it
    /// out of the row.
    #[test]
    fn a_mark_is_right_aligned_and_never_clipped_by_a_long_label() {
        use crate::geom::Size;
        let long = "a-very-long-directory-name-that-cannot-possibly-fit-in-this-narrow-row-at-all";
        let tree = tree_view(
            "fs",
            vec![
                TreeItem::new("short", "short")
                    .mark(word("m", "submodule"))
                    .build(vec![]),
                TreeItem::new("long", long)
                    .mark(word("m", "submodule"))
                    .build(vec![]),
            ],
        );
        let frame = crate::component::tests::petrify_lone_at(tree, Size::new(260.0, 200.0));
        for who in ["short", "long"] {
            let row = rect_of(&frame, &format!("/{who}/row"));
            let mark = rect_of(&frame, &format!("/{who}/row/mark"));
            assert_eq!(
                mark.right(),
                row.right() - 16.0,
                "{who}: the mark stops at the row's 16 right padding"
            );
            assert!(mark.w > 0.0, "{who}: the mark has a width");
            let label = rect_of(&frame, &format!("/{who}/row/label"));
            assert!(
                label.right() <= mark.x,
                "{who}: the label ({label:?}) overlaps the mark ({mark:?})"
            );
        }
        let long_label = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/long/row/label"))
            .expect("long label placed");
        assert!(
            !long_label.paint.overflowed,
            "the long label overflowed its rect instead of ending in an ellipsis"
        );
        for p in &frame.placements {
            if let Some(parent) = p.parent {
                assert_fits_parent("", p, &frame.placements[parent]);
            }
        }
    }

    /// The label takes every unit the mark does not need, and no more.
    ///
    /// The row is `... label, spacer, mark-gap, mark`. An equal-share walk
    /// offered the label and the bare spacer half of the room each, so a name
    /// that fit with room to spare still ended in an ellipsis in the middle
    /// of the row, with a wide gap before its mark. Three widths pin it: a
    /// row that fits the label with a little room left (no ellipsis, the
    /// full natural width), a row too narrow (the label stops one mark-gap
    /// before the mark), and the same name in a wide row.
    #[test]
    fn a_label_uses_all_the_room_the_mark_leaves_and_truncates_only_at_the_mark() {
        use crate::geom::Size;
        let name = "a-directory-name-too-long-for-this-column-to-hold";
        let build = || {
            tree_view(
                "fs",
                vec![
                    TreeItem::new("r", name)
                        .mark(word("m", "submodule"))
                        .build(vec![]),
                ],
            )
        };
        let rects = |width: f32| {
            let frame = crate::component::tests::petrify_lone_at(build(), Size::new(width, 80.0));
            let label = rect_of(&frame, "/r/row/label");
            let gap = rect_of(&frame, "/r/row/mark-gap");
            let mark = rect_of(&frame, "/r/row/mark");
            let row = rect_of(&frame, "/r/row");
            let truncated = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/r/row/label"))
                .expect("label placed")
                .paint
                .truncated;
            (row, label, gap, mark, truncated)
        };

        // Wide: the label's natural width.
        let (_, label, _, _, truncated) = rects(900.0);
        assert!(!truncated, "no ellipsis when the name fits");
        let natural = label.w;

        // A row with 8 units to spare past the label, the gap and the mark.
        let (row, label, gap, mark, _) = rects(900.0);
        let fixed = (label.x - row.x) + gap.w + mark.w + 16.0;
        let (_, label, gap, mark, truncated) = rects(fixed + natural + 8.0);
        assert!(
            !truncated,
            "a name that fits with 8 units to spare must not end in an ellipsis"
        );
        assert_eq!(label.w, natural, "the label keeps its natural width");
        assert!(label.right() + gap.w <= mark.x + 0.01);

        // Too narrow: the label is cut, and its right edge plus the gap is
        // the mark's left edge.
        let (_, label, gap, mark, truncated) = rects(fixed + natural - 40.0);
        assert!(truncated, "a name that does not fit ends in an ellipsis");
        assert!(
            (label.right() + gap.w - mark.x).abs() < 0.01,
            "label right {} + gap {} must meet the mark at {}",
            label.right(),
            gap.w,
            mark.x
        );
    }

    /// A leaf is a document, a closed branch a folder, an open one an open
    /// folder.
    #[test]
    fn a_branch_state_names_its_carbon_glyph() {
        assert_eq!(
            TreeBranch::Leaf.icon_mark(),
            crate::component::IconMark::Document
        );
        assert_eq!(
            TreeBranch::Closed.icon_mark(),
            crate::component::IconMark::Folder
        );
        assert_eq!(
            TreeBranch::Open.icon_mark(),
            crate::component::IconMark::Folder
        );
    }

    /// A branch row, a leaf row, and a deep row line up the way the eager
    /// tree does: carets one level apart, labels one level apart.
    #[test]
    fn a_flat_row_at_a_level_draws_what_the_eager_row_draws_at_that_depth() {
        // The eager tree: three levels, one branch each.
        let eager = tree_view(
            "fs",
            vec![tree_item(
                "a",
                "a",
                true,
                false,
                vec![tree_item(
                    "b",
                    "b",
                    true,
                    false,
                    vec![tree_item("c", "c", false, true, vec![])],
                )],
            )],
        );
        // The same three rows, flat, each saying where it sits.
        let flat = tree_view(
            "fs",
            vec![
                TreeItem::new("a", "a")
                    .branch(TreeBranch::Open)
                    .at(TreePlace {
                        level: 1,
                        pos_in_set: 1,
                        set_size: 1,
                    })
                    .build(vec![]),
                TreeItem::new("b", "b")
                    .branch(TreeBranch::Open)
                    .at(TreePlace {
                        level: 2,
                        pos_in_set: 1,
                        set_size: 1,
                    })
                    .build(vec![]),
                TreeItem::new("c", "c")
                    .selected(true)
                    .at(TreePlace {
                        level: 3,
                        pos_in_set: 1,
                        set_size: 1,
                    })
                    .build(vec![]),
            ],
        );
        let eager_frame = petrify_lone(eager);
        let flat_frame = petrify_lone(flat);
        let e_tree = rect_of(&eager_frame, "/fs");
        let f_tree = rect_of(&flat_frame, "/fs");
        for (who, parts) in [
            ("a", ["bar", "lead", "caret", "gap", "label"]),
            ("b", ["bar", "lead", "caret", "gap", "label"]),
            ("c", ["bar", "lead", "caret-slot", "gap", "label"]),
        ] {
            let e_row = rect_of(&eager_frame, &format!("/{who}/row"));
            let f_row = rect_of(&flat_frame, &format!("/{who}/row"));
            assert_eq!(e_row.w, f_row.w, "{who}: row width");
            assert_eq!(e_row.h, f_row.h, "{who}: row height");
            for part in parts {
                let e = rect_of(&eager_frame, &format!("/{who}/row/{part}"));
                let f = rect_of(&flat_frame, &format!("/{who}/row/{part}"));
                assert_eq!(
                    e.x - e_tree.x,
                    f.x - f_tree.x,
                    "{who}/{part}: the flat row sits at another x than the eager row"
                );
                assert_eq!(e.w, f.w, "{who}/{part}: width");
                assert_eq!(e.h, f.h, "{who}/{part}: height");
            }
        }
    }

    /// An eager tree declares what a flat one does: depth from 1, and a
    /// position among its siblings, so the two forms read the same to a
    /// screen reader.
    #[test]
    fn an_eager_tree_stamps_level_position_and_set_size_on_every_item() {
        let tree = tree_view(
            "fs",
            vec![
                tree_item(
                    "src",
                    "src",
                    true,
                    false,
                    vec![
                        tree_item("main", "main.rs", false, false, vec![]),
                        tree_item("lib", "lib.rs", false, false, vec![]),
                    ],
                ),
                tree_item("docs", "docs", false, false, vec![]),
            ],
        );
        let at = |key: &str| {
            let s = &named(&tree, key).semantics;
            (s.level, s.pos_in_set, s.set_size)
        };
        assert_eq!(at("src"), (Some(1), Some(1), Some(2)));
        assert_eq!(at("docs"), (Some(1), Some(2), Some(2)));
        assert_eq!(at("main"), (Some(2), Some(1), Some(2)));
        assert_eq!(at("lib"), (Some(2), Some(2), Some(2)));
    }

    /// Three levels: the grandchild is level 3 although it was built as a
    /// level-1 item, because each parent steps its children in by one.
    #[test]
    fn nesting_adds_one_level_per_parent() {
        let tree = tree_item(
            "a",
            "a",
            true,
            false,
            vec![tree_item(
                "b",
                "b",
                true,
                false,
                vec![tree_item("c", "c", false, false, vec![])],
            )],
        );
        assert_eq!(named(&tree, "a").semantics.level, Some(1));
        assert_eq!(named(&tree, "b").semantics.level, Some(2));
        assert_eq!(named(&tree, "c").semantics.level, Some(3));
    }

    /// `tree_rows` is a `Role::Tree` scroll over one collection that
    /// declares the total, so only the host's window of rows is ever built.
    #[test]
    fn tree_rows_is_a_tree_scroll_over_a_collection_of_the_declared_total() {
        let node = tree_rows("files", "Files", "project-files", 20_000);
        assert_eq!(node.kind, NodeKind::Scroll);
        assert_eq!(node.semantics.role, Some(Role::Tree));
        assert_eq!(node.semantics.label.as_deref(), Some("Files"));
        assert!(
            node.interactions.contains(&Interaction::Scroll),
            "a scroll takes the wheel only when it declares it"
        );
        let rows = named(&node, "rows");
        assert_eq!(rows.kind, NodeKind::Collection);
        assert_eq!(rows.props.total_count, Some(20_000));
        assert_eq!(rows.props.source.as_deref(), Some("project-files"));
        assert_eq!(rows.props.estimated_extent, Some(HEIGHT));
        let xs = super::tree_rows_xs("files", "Files", "s", 3);
        assert_eq!(
            named(&xs, "rows").props.estimated_extent,
            Some(HEIGHT_XS),
            "an extra-small tree estimates its rows at 24"
        );
        assert!(
            crate::tree::validate(
                &node,
                &crate::tree::Registry::with_vocabulary(crate::token::standard_vocabulary())
            )
            .is_ok(),
            "tree_rows is an acceptable tree"
        );
    }
}
