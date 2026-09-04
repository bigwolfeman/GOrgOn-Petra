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
//! [`indent`] rather than as padding on the nested container, so a nested
//! row's fill and accent bar still span the whole tree.
//! Inside a row, left to right: the 4-unit accent bar (its fill only when
//! selected, its width always, so nothing shifts on selection), 12 of
//! lead (`$spacing-04`), the 16-unit caret box, 8 (`$spacing-03`), then
//! the label — so a level-1 label starts at 40 and a level-2 label at 56,
//! Carbon's own numbers for a text-only tree. Right padding is 16.

use std::sync::Arc;

use super::tokens::{
    ACCENT_PRIMARY, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_05, SURFACE_BASE, t,
};
use super::{CARET_SIZE, CaretDirection, caret, stack, swatch};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, InsetRefs, Interaction, Key, NodeKind, Props, Role, Semantics, TrackSize,
    ViewNode,
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

const _: () = assert!(HEIGHT == 32.0);
const _: () = assert!(HEIGHT_XS == 24.0);
const _: () = assert!(ACCENT_BAR + LEAD == 16.0);
const _: () = assert!(LABEL_START == 40.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A tree of items. `Role::Tree`, no interactions.
///
/// V6: `align = Align::Stretch` — without it every top-level [`tree_item`]
/// measured to its own label width instead of the tree's, so a selected
/// item's fill (bound on the item's own outer node) stopped at its own text
/// rather than spanning the row, the same content-sized-instead-of-full-
/// width class V2 fixed one level up in `data_table`'s row list.
pub fn tree_view(key: impl Into<Key>, nodes: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Vertical, None, nodes);
    node.props.align = Some(Align::Stretch);
    node.semantics = Semantics {
        role: Some(Role::Tree),
        ..Semantics::default()
    };
    node
}

/// One tree item at Carbon small (32).
///
/// `label` is required (FR-058). Nested `children` are mounted only when
/// `expanded` is true. Leaves still record `Semantics.expanded` because
/// the constructor takes the flag; they do not grow a caret.
pub fn tree_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    tree_item_sized(key, label, expanded, selected, children, HEIGHT)
}

/// [`tree_item`] at Carbon extra-small (24).
pub fn tree_item_xs(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    tree_item_sized(key, label, expanded, selected, children, HEIGHT_XS)
}

fn tree_item_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
    height: f32,
) -> ViewNode {
    let label = label.into();
    let is_branch = !children.is_empty();

    // The bar keeps its width in every state; only its fill comes and
    // goes, so the label never shifts when the selection moves.
    let bar_fill = selected.then_some(ACCENT_PRIMARY);
    let disclosure = if is_branch {
        caret(
            "caret",
            if expanded {
                CaretDirection::Down
            } else {
                CaretDirection::Right
            },
        )
    } else {
        swatch("caret-slot", CARET_SIZE, CARET_SIZE, None, None, None)
    };
    let mut row = stack(
        "row",
        Axis::Horizontal,
        None,
        vec![
            swatch("bar", ACCENT_BAR, height, bar_fill, None, None),
            swatch("lead", LEAD, height, None, None, None),
            disclosure,
            swatch("gap", CARET_GAP, height, None, None, None),
            super::text::text("label", label.clone()),
        ],
    );
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

    let mut parts = vec![row];
    if expanded && is_branch {
        // The level's indent goes into each child's row, not onto this
        // container as padding: a padded container would start the
        // child's fill (and its accent bar) 16 in, where Carbon's node
        // spans the whole tree and only its content steps in.
        let children = children
            .into_iter()
            .map(|mut child| {
                indent(&mut child, LEVEL_INDENT);
                child
            })
            .collect();
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
    let row_count = parts.len();
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            rows: vec![TrackSize::FitContent; row_count],
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
    let mut node = node.interactive(Role::TreeItem, label, ITEM_INTENTS);
    node.semantics.selected = selected;
    node.semantics.expanded = Some(expanded);
    node
}

/// Step `item`'s row content in by `by`, and everything already nested
/// under it by the same amount, by widening the row's `lead` spacer. The
/// item's own rect is untouched, so its fill and accent bar keep spanning
/// the tree. A child is built before its parent, so this is the parent's
/// stamp on it, the same shape as `list.rs`'s `stamp_items`.
fn indent(item: &mut ViewNode, by: f32) {
    for child in &mut item.children {
        let child = Arc::make_mut(child);
        match child.key.as_str() {
            "row" => {
                for part in &mut child.children {
                    if part.key.as_str() == "lead" {
                        let lead = Arc::make_mut(part);
                        let width = lead.constraints.horizontal.min.unwrap_or(0.0) + by;
                        lead.constraints.horizontal.min = Some(width);
                        lead.constraints.horizontal.max = Some(width);
                    }
                }
            }
            "children" => {
                for grandchild in &mut child.children {
                    indent(Arc::make_mut(grandchild), by);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT_BAR, ACCENT_PRIMARY, HEIGHT, HEIGHT_XS, LABEL_START, tree_item, tree_item_xs,
        tree_view,
    };
    use crate::component::disabled;
    use crate::component::tokens::LAYER_SELECTED;
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Rect, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

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

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
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
                let parent = &frame.placements[parent_idx];
                let fits = p.rect.x >= parent.rect.x - 0.01
                    && p.rect.y >= parent.rect.y - 0.01
                    && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                    && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                assert!(
                    fits,
                    "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                    p.id, p.rect, parent.id, parent.rect
                );
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
}
