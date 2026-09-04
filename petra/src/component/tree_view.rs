//! Carbon Tree view (slice-f). No drag-to-reorder.
//!
//! Anatomy (`_treeview.scss` + usage page):
//! 1. [`tree_view`] — the hierarchy container (`Role::Tree`).
//! 2. Branch / leaf [`tree_item`] — `Role::TreeItem`.
//! 3. Caret as the word `"expanded"` / `"collapsed"`, never an icon-only
//!    mark (FR-026). Present on branches only.
//! 4. Node label.
//!
//! Nested children exist in the tree only while `expanded` is true.
//! Selection is `Semantics.selected` plus [`LAYER_SELECTED`], never colour
//! alone. Carbon does not ship drag-to-reorder; items do not declare
//! [`Interaction::Drag`].
//!
//! Default node height is Carbon small: 32. Extra-small 24 is
//! [`tree_item_xs`].

use super::stack;
use super::text::text;
use super::tokens::{
    LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_03, SPACING_05, SURFACE_BASE, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    InsetRefs, Interaction, Key, NodeKind, Props, Role, Semantics, TrackSize, ViewNode,
};

/// Carbon small / default node height.
const HEIGHT: f32 = 32.0;
/// Carbon extra-small node height.
const HEIGHT_XS: f32 = 24.0;

const _: () = assert!(HEIGHT == 32.0);
const _: () = assert!(HEIGHT_XS == 24.0);

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
    let mut row_parts = Vec::new();
    if is_branch {
        let disclosure = if expanded { "expanded" } else { "collapsed" };
        row_parts.push(text("chevron", disclosure));
    }
    row_parts.push(text("label", label.clone()));

    let mut row = stack("row", Axis::Horizontal, Some(SPACING_03), row_parts);
    row.props.align = Some(Align::Center);
    row.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    row.constraints.vertical.min = Some(height);

    let mut parts = vec![row];
    if expanded && is_branch {
        let mut nest = stack("children", Axis::Vertical, None, children);
        nest.props.align = Some(Align::Stretch);
        nest.props.padding = Some(InsetRefs {
            left: Some(t(SPACING_05)),
            ..InsetRefs::default()
        });
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

#[cfg(test)]
mod tests {
    use super::{HEIGHT, HEIGHT_XS, tree_item, tree_item_xs, tree_view};
    use crate::component::disabled;
    use crate::component::tokens::LAYER_SELECTED;
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
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
        assert_eq!(
            named(&on, "chevron").props.text.as_deref(),
            Some("expanded")
        );
        assert_eq!(named(&on, "label").props.text.as_deref(), Some("src"));
        assert!(has_key(&on, "lib"));
        assert_eq!(named(&on, "row").constraints.vertical.min, Some(HEIGHT));
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
        assert_eq!(
            named(&off, "chevron").props.text.as_deref(),
            Some("collapsed")
        );
        assert!(
            !has_key(&off, "lib"),
            "collapsed branch must not mount children"
        );
        assert!(
            !has_key(&off, "children"),
            "collapsed branch must not keep an empty nest"
        );
    }

    #[test]
    fn tree_items_do_not_declare_drag() {
        let node = tree_view(
            "fs",
            vec![tree_item(
                "src",
                "src",
                true,
                false,
                vec![tree_item("lib", "lib.rs", false, true, vec![])],
            )],
        );
        no_drag(&node);
        let leaf = tree_item("leaf", "README", false, false, vec![]);
        no_drag(&leaf);
        assert!(!has_key(&leaf, "chevron"), "a leaf has no expand caret");
    }

    #[test]
    fn extra_small_item_is_24() {
        let node = tree_item_xs("xs", "tiny", false, false, vec![]);
        assert_eq!(
            named(&node, "row").constraints.vertical.min,
            Some(HEIGHT_XS)
        );
        assert_eq!(HEIGHT_XS, 24.0);
        no_drag(&node);
    }

    // Chevron is inline text ("expanded"/"collapsed"), never pinned inside
    // a `Constraints` box the way Modal's `close_button` or Number input's
    // `stepper` are — `row`'s own `Constraints` pin only the block-size
    // (height), never the inline-size (width) — so Class 4 (an icon-only
    // hit box pinned around a word) does not apply structurally here.

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

    // Colour-channel finding for the operator (red-green colour blind),
    // recorded not fixed — adding an icon channel is a feature, not an
    // audit fix, matching group 5's identical finding on `selectable_tag`
    // and `structured_list_row`.
    //
    // `tree_item`'s `selected` state is carried by `background@selected`
    // (`LAYER_SELECTED`) alone: no icon, shape, or text-weight channel
    // accompanies it (unlike Carbon's own tab, which changes typography
    // weight as well as colour — the pattern `tabs.rs`'s `tab` already
    // follows). `Semantics.selected` is set (`tree_item_declares_expanded_
    // and_selected_never_colour_only` above already proves this much for
    // the accessibility tree), so an assistive-technology reader is fine;
    // a sighted, colour-blind reader looking at two rows filled with
    // `LAYER_SELECTED` vs. resting `SURFACE_BASE` — a fill-tone difference
    // only — is the same shape the operator flagged in group 5's UI shell
    // sibling components below.
    #[test]
    fn selected_state_is_carried_by_fill_tone_alone_recorded_for_the_operator() {
        let on = tree_item("l", "main.rs", false, true, vec![]);
        let off = tree_item("l", "lib.rs", false, false, vec![]);
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(
            token(&off, "background@selected"),
            Some(LAYER_SELECTED),
            "both bind the same selected fill token; only `Semantics.selected` \
             tells them apart, and no icon/shape/text channel does"
        );
    }
}
