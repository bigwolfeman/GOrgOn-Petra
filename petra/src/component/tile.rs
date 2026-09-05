//! Carbon Tile. Slice-e.
//!
//! Four kinds, four constructors. Carbon documents them as one component
//! with four structural variants, not four sizes of the same chrome:
//!
//! 1. [`tile`] — base: a static container. Enabled only, no role, no
//!    interactions.
//! 2. [`clickable_tile`] — the whole tile is one target (`Role::Button`).
//! 3. [`selectable_tile`] — an option. Single- vs multi-select is caller
//!    grouping, the same line [`super::radio`] draws. Selection is never
//!    colour alone: `Semantics.selected` plus a checkbox-shaped mark at the
//!    tile's top-right corner — an empty box when off, an accent box with
//!    [`IconMark::Check`] when on (Carbon's `CheckboxCheckedFilled`).
//! 4. [`expandable_tile`] — reveals a below-the-fold body. Click anywhere
//!    on the tile (this constructor has no inner controls). The header
//!    ends in a [`super::caret`], down when open and right when shut;
//!    `Semantics.expanded` plus the revealed body are the channels a
//!    reader who cannot see the caret still gets (FR-026). Until
//!    2026-09-04 the caret was the literal word `expanded` / `collapsed`.
//!
//! Geometry is the SCSS floor, not a size ramp: `min-inline-size` 128
//! (8rem), `min-block-size` 64 (4rem), padding `$spacing-05` on both axes.
//! There is no sm/md/lg. There is no invalid, warning, or skeleton state.
//!
//! # One box, four kinds
//!
//! Every kind is the same box: a [`SURFACE_RAISED`] fill, no resting edge,
//! and the width of whatever it is placed in. That is Carbon's own form
//! without the `enable-tile-contrast` flag ("interactive tiles have no
//! border at all", slice-e) and it is what `35-tile.png` shows: four tiles
//! at one width, one fill, no outline. Before 2026-09-04 the three
//! interactive kinds drew a [`super::tokens::BORDER_STRONG`] edge as a
//! second channel and every kind hugged its own sentence, so the row read
//! as two visual languages in four widths. What tells the kinds apart now
//! is what Carbon uses: the hover fill on the interactive ones, the mark
//! on the selectable one, the caret on the expandable one.
//!
//! The box is a single-column `Weight` grid rather than a stack because a
//! stack settles at its content width; a `Weight` track claims the width
//! it is offered, which is how a tile fills its column the way Carbon's
//! block-level tile fills its grid cell.
//!
//! Fill is [`SURFACE_RAISED`]. Carbon calls a tile un-elevated — they mean
//! relative to a card, and Carbon has no Card. Petra's card is
//! [`super::section`], which already spends `surface.raised` *and*
//! `shadow.raised`. A tile takes the fill and leaves the shadow, so the two
//! stay distinct on the same page.

use super::icon::{IconMark, icon};
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_STRONG, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_NONE,
    SPACING_03, SPACING_05, SURFACE_RAISED, t,
};
use super::{CaretDirection, caret, pad, stack, swatch};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, Interaction, Key, NodeKind, Props, Role, TrackSize, ViewNode,
};

/// Carbon `.cds--tile` `min-inline-size: 8rem`.
const MIN_INLINE: f32 = 128.0;
/// Carbon `.cds--tile` `min-block-size: 4rem`.
const MIN_BLOCK: f32 = 64.0;
/// The selectable tile's checkbox-shaped mark: Carbon's 16px icon.
const MARK: f32 = 16.0;

const INTERACTIVE: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

fn floor() -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(MIN_INLINE),
            max: None,
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(MIN_BLOCK),
            max: None,
            priority: 0,
        },
    }
}

/// Shared box: raised fill, `$spacing-05` padding, Carbon min size, and
/// the full width it is offered. One `FitContent` row per child, gapped
/// by `$spacing-03`; `align: Stretch` so a header row inside it also
/// spans the tile.
///
/// No border, no role, no hover. [`tile`] returns this as-is.
/// Interactive constructors add the state fills on top.
fn shell(key: impl Into<Key>, children: Vec<ViewNode>) -> ViewNode {
    let rows = vec![TrackSize::FitContent; children.len().max(1)];
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            rows,
            row_spacing: Some(t(SPACING_03)),
            align: Some(Align::Stretch),
            padding: Some(pad(SPACING_05, SPACING_05)),
            ..Props::default()
        })
        .with_children(children);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.with_constraints(floor())
}

/// The interactive-tile chrome: a hover fill and — when the kind can be
/// selected — the four-fill set [`super::list_row`] pioneered.
///
/// `Hover` has to be declared on the node that binds `background@hover`,
/// or the engine never hit-tests it and the token is a name nothing reads.
fn with_interactive_chrome(mut node: ViewNode, selectable: bool) -> ViewNode {
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    if selectable {
        node.props
            .tokens
            .insert("background@selected".into(), t(LAYER_SELECTED));
        node.props
            .tokens
            .insert("background@selected-hover".into(), t(LAYER_SELECTED_HOVER));
    }
    node
}

/// A row with text on the left and a fixed mark pinned to the right end:
/// `[Weight, FitContent]` columns, stretched, so the text takes the room
/// and the mark sits at the tile's top-right corner the way Carbon's
/// checkbox and chevron icons do.
fn header(key: &'static str, leading: ViewNode, trailing: ViewNode) -> ViewNode {
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }, TrackSize::FitContent],
            rows: vec![TrackSize::FitContent],
            column_spacing: Some(t(SPACING_03)),
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![leading, trailing])
}

/// A base tile: static container, enabled only.
///
/// No role. `Role::Pane` would make a region out of a box that does not
/// collect anything; a container with no actions is outside
/// `ActionableNeedsRoleAndLabel`'s reach, so FR-058 has nothing to enforce
/// here. Callers that need interactive children put them in themselves —
/// this constructor does not grow a children argument, because the
/// inventory's base tile is a box around text, not a layout primitive.
pub fn tile(key: impl Into<Key>, body: impl Into<String>) -> ViewNode {
    shell(key, vec![text("body", body.into())])
}

/// A clickable tile: the whole surface is one activation target.
///
/// `label` is the accessible name (FR-058, required, not an `Option`).
/// `body` is the visible text. They are two strings because Carbon's
/// clickable tile is an `<a>` whose content is not always its name.
pub fn clickable_tile(
    key: impl Into<Key>,
    label: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    with_interactive_chrome(shell(key, vec![text("body", body.into())]), false).interactive(
        Role::Button,
        label,
        INTERACTIVE,
    )
}

/// A selectable tile: one option in a caller-grouped set.
///
/// Multi-select vs radio is not two components. The caller groups these
/// the way it groups [`super::radio`]. What this constructor guarantees
/// is that selected is a declared fact (`Semantics.selected`) with a
/// second visual channel (the mark's shape: empty box against filled box
/// with a check), not a fill swap a colour-blind reader cannot recover.
pub fn selectable_tile(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let label = label.into();
    let row = header(
        "row",
        text("label", label.clone()),
        selection_mark(selected),
    );
    let node = with_interactive_chrome(shell(key, vec![row]), true);
    let mut node = node.interactive(Role::Button, label, INTERACTIVE);
    node.semantics.selected = selected;
    node
}

/// The selectable tile's checkbox-shaped mark. Off: a 16-unit box with a
/// [`BORDER_STRONG`] edge and no fill, the same reason the checkbox's own
/// box keeps its edge — with nothing inside it, the outline is the whole
/// control. On: an [`ACCENT_PRIMARY`] box with [`IconMark::Check`] centred
/// in it, the way [`super::progress_indicator`]'s complete mark is built.
/// Sharp corners ([`SHAPE_NONE`]), matching the checkbox.
fn selection_mark(selected: bool) -> ViewNode {
    if !selected {
        return swatch(
            "box",
            MARK,
            MARK,
            None,
            Some(BORDER_STRONG),
            Some(SHAPE_NONE),
        );
    }
    let tick = icon("mark", IconMark::Check);
    let inset = ((MARK - tick.constraints.horizontal.min.unwrap_or(0.0)) * 0.5).max(0.0);
    let mut node = stack(
        "box",
        Axis::Horizontal,
        None,
        vec![
            swatch("inset-start", inset, MARK, None, None, None),
            tick,
            swatch("inset-end", inset, MARK, None, None, None),
        ],
    );
    node.props.align = Some(Align::Center);
    node.props
        .tokens
        .insert("background".into(), t(ACCENT_PRIMARY));
    node.props.tokens.insert("radius".into(), t(SHAPE_NONE));
    node.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(MARK),
            max: Some(MARK),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(MARK),
            max: Some(MARK),
            priority: 0,
        },
    })
}

/// An expandable tile: a labelled header that reveals `body` below the fold.
///
/// Clicking anywhere on the tile toggles it — this constructor has no
/// inner controls, so it does not need Carbon's "only the chevron
/// button toggles" sub-form. The caret at the header's right end points
/// down when open and right when shut; `Semantics.expanded` and the
/// revealed body are the fact a reader who cannot see it still gets.
/// Carbon seats its chevron at the tile's bottom-right corner; this one
/// sits on the header's line, because the tile has no fixed above-the-
/// fold height to pin a bottom to.
pub fn expandable_tile(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    body: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let disclosure = caret(
        "caret",
        if expanded {
            CaretDirection::Down
        } else {
            CaretDirection::Right
        },
    );
    let mut children = vec![header("header", text("label", label.clone()), disclosure)];
    if expanded {
        children.push(text("body", body.into()));
    }

    let mut node = with_interactive_chrome(shell(key, children), false).interactive(
        Role::Button,
        label,
        INTERACTIVE,
    );
    node.semantics.expanded = Some(expanded);
    node
}

#[cfg(test)]
mod tests {
    use super::super::tokens::{
        ACCENT_PRIMARY, BORDER_STRONG, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER,
        SURFACE_RAISED,
    };
    use super::{
        MARK, MIN_BLOCK, MIN_INLINE, clickable_tile, expandable_tile, selectable_tile, tile,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Rect, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn token<'a>(node: &'a crate::tree::ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn has_canvas(node: &crate::tree::ViewNode) -> bool {
        node.kind == NodeKind::Canvas || node.children.iter().any(|child| has_canvas(child))
    }

    #[test]
    fn tile_exists_and_is_not_interactive() {
        let node = tile("base", "Grouped content");
        assert_eq!(node.key.as_str(), "base");
        assert!(
            !node.is_interactive(),
            "a base tile is a container, not a control"
        );
        assert!(node.interactions.is_empty());
        assert!(
            node.semantics.role.is_none(),
            "base tile prefers no role; it is not a Pane"
        );
        assert!(!node.semantics.selected);
        assert_eq!(node.semantics.expanded, None);
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), None);
        assert_eq!(token(&node, "background@hover"), None);
    }

    #[test]
    fn tile_min_constraints_are_the_carbon_floor() {
        for (label, node) in [
            ("base", tile("t", "x")),
            ("clickable", clickable_tile("t", "Open", "x")),
            ("selectable", selectable_tile("t", "Plan A", false)),
            ("expandable", expandable_tile("t", "Details", false, "x")),
        ] {
            assert_eq!(
                node.constraints.horizontal.min,
                Some(MIN_INLINE),
                "{label}: min-inline-size 128"
            );
            assert_eq!(
                node.constraints.vertical.min,
                Some(MIN_BLOCK),
                "{label}: min-block-size 64"
            );
            assert_eq!(node.constraints.horizontal.max, None);
            assert_eq!(node.constraints.vertical.max, None);
        }
        assert_eq!(MIN_INLINE, 128.0);
        assert_eq!(MIN_BLOCK, 64.0);
    }

    /// Row 35's defect: the base tile had a fill and no edge while the
    /// three interactive kinds had a fill *and* an edge, two visual
    /// languages in one row. Every kind is now the same box.
    #[test]
    fn every_kind_is_the_same_box_with_no_resting_edge() {
        for (label, node) in [
            ("base", tile("t", "x")),
            ("clickable", clickable_tile("t", "Open", "x")),
            ("selectable", selectable_tile("t", "Plan A", false)),
            ("expandable", expandable_tile("t", "Details", false, "x")),
        ] {
            assert_eq!(node.kind, NodeKind::Grid, "{label}");
            assert_eq!(token(&node, "background"), Some(SURFACE_RAISED), "{label}");
            assert_eq!(
                token(&node, "border"),
                None,
                "{label}: a tile draws no resting edge (Carbon without the contrast flag)"
            );
        }
    }

    #[test]
    fn clickable_tile_is_interactive_with_role_and_label() {
        let node = clickable_tile("open", "Open project", "Project Alpha");
        assert!(node.is_interactive());
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Open project"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(
            node.interactions.contains(&Interaction::Hover),
            "Hover must be declared on the node that binds background@hover"
        );
        assert_eq!(token(&node, "background@hover"), Some(LAYER_HOVER));
        assert_eq!(
            token(&node, "background@selected"),
            None,
            "a clickable tile has no selected state"
        );
        assert!(!node.semantics.selected);
    }

    /// Selection is a shape: an empty box off, an accent box with a check
    /// on. Both sit in the header's trailing cell, so the mark is at the
    /// tile's top-right in either state and the label does not move.
    #[test]
    fn selectable_tile_declares_selected_and_draws_a_check() {
        let on = selectable_tile("plan", "Plan A", true);
        assert_eq!(on.semantics.role, Some(Role::Button));
        assert_eq!(on.semantics.label.as_deref(), Some("Plan A"));
        assert!(on.semantics.selected, "selected is a declared fact");
        assert!(has_canvas(&on), "a selected tile draws IconMark::Check");
        let on_box = named(&on, "box");
        assert_eq!(token(on_box, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(on_box.constraints.horizontal.min, Some(MARK));
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(
            token(&on, "background@selected-hover"),
            Some(LAYER_SELECTED_HOVER)
        );

        let off = selectable_tile("plan", "Plan A", false);
        assert!(!off.semantics.selected);
        assert!(!has_canvas(&off), "an unselected tile draws no check");
        let off_box = named(&off, "box");
        assert_eq!(token(off_box, "border"), Some(BORDER_STRONG));
        assert_eq!(token(off_box, "background"), None);
        assert_eq!(off_box.constraints.horizontal.min, Some(MARK));
        assert!(off.is_interactive());
        assert_eq!(MARK, 16.0);
    }

    #[test]
    fn expandable_tile_declares_expanded_as_a_second_channel() {
        let open = expandable_tile("more", "Details", true, "the rest");
        assert!(open.is_interactive());
        assert_eq!(open.semantics.role, Some(Role::Button));
        assert_eq!(open.semantics.label.as_deref(), Some("Details"));
        assert_eq!(open.semantics.expanded, Some(true));
        let open_caret = named(&open, "caret");
        assert_eq!(
            open_caret.kind,
            NodeKind::Canvas,
            "disclosure is a caret shape"
        );
        assert!(
            open_caret.props.text.is_none(),
            "the caret is a shape, not the word `expanded`"
        );
        assert_eq!(
            named(&open, "body").props.text.as_deref(),
            Some("the rest"),
            "an open tile mounts its body"
        );

        let shut = expandable_tile("more", "Details", false, "the rest");
        assert_eq!(shut.semantics.expanded, Some(false));
        let shut_caret = named(&shut, "caret");
        assert_ne!(
            open_caret.props.canvas, shut_caret.props.canvas,
            "open and shut carets point different ways"
        );
        assert!(
            !shut.children.iter().any(|c| c.key.as_str() == "body"),
            "a shut tile does not mount its body"
        );
        assert_eq!(token(&open, "background@hover"), Some(LAYER_HOVER));
    }

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        petrify_column(vec![node])
    }

    fn petrify_column(nodes: Vec<ViewNode>) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .with_children(nodes);
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

    /// Row 35 on the placed frame: four tiles in one column place at one
    /// width — the column's, not each sentence's — and the selectable
    /// tile's mark sits at the tile's top-right, inside its padding.
    #[test]
    fn four_kinds_in_one_column_place_at_the_columns_width() {
        let frame = petrify_column(vec![
            tile("base", "A static tile holds related content."),
            clickable_tile("click", "Open", "Short."),
            selectable_tile("sel", "Select this option", false),
            expandable_tile("exp", "More detail", false, "Below the fold."),
        ]);
        let widths: Vec<f32> = ["/base", "/click", "/sel", "/exp"]
            .iter()
            .map(|k| rect_of(&frame, k).w)
            .collect();
        assert!(
            widths.iter().all(|w| (w - widths[0]).abs() < 0.01),
            "tiles hug their own text instead of filling the column: {widths:?}"
        );
        assert_eq!(widths[0], VIEWPORT.w, "a tile is as wide as its column");
        let sel = rect_of(&frame, "/sel");
        let mark = rect_of(&frame, "/sel/row/box");
        assert_eq!(mark.w, MARK);
        assert_eq!(
            mark.x + mark.w,
            sel.x + sel.w - 16.0,
            "the mark sits at the tile's right edge, inside the 16 padding"
        );
        assert_eq!(
            mark.y,
            sel.y + 16.0,
            "the mark sits at the tile's top, inside the padding"
        );
        let label = rect_of(&frame, "/sel/row/label");
        assert!(label.x < mark.x, "label leads, mark trails");
    }

    /// Check C/D across all four kinds: nothing overflows, nothing leaves
    /// its parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("base", tile("t", "Grouped content")),
            (
                "clickable",
                clickable_tile("t", "Open project", "Project Alpha"),
            ),
            ("selectable-on", selectable_tile("t", "Plan A", true)),
            ("selectable-off", selectable_tile("t", "Plan A", false)),
            (
                "expandable-open",
                expandable_tile("t", "Details", true, "the rest"),
            ),
            (
                "expandable-shut",
                expandable_tile("t", "Details", false, "the rest"),
            ),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
                    p.id
                );
                assert!(
                    !p.paint.truncated,
                    "{label}: {} was truncated to fit its parent",
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
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check F: the base tile is a container with no interactions and is
    /// never reachable; the three interactive kinds declare `Focus` and
    /// are.
    #[test]
    fn interactive_kinds_are_reachable_and_the_base_tile_is_not() {
        for (label, node, should_be_focusable) in [
            ("base", tile("t", "Grouped content"), false),
            (
                "clickable",
                clickable_tile("t", "Open project", "Project Alpha"),
                true,
            ),
            ("selectable", selectable_tile("t", "Plan A", false), true),
            (
                "expandable",
                expandable_tile("t", "Details", false, "the rest"),
                true,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/t"))
                .expect("the tile is placed");
            let reachable = focus.order().iter().any(|o| o == &placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the body/label text against the tile's own resting fill,
    /// across all four kinds, in both themes.
    #[test]
    fn tile_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node, keys) in [
                ("base", tile("t", "Grouped content"), vec!["body"]),
                (
                    "clickable",
                    clickable_tile("t", "Open project", "Project Alpha"),
                    vec!["body"],
                ),
                (
                    "selectable",
                    selectable_tile("t", "Plan A", true),
                    vec!["label"],
                ),
                (
                    "expandable",
                    expandable_tile("t", "Details", true, "the rest"),
                    vec!["label", "body"],
                ),
            ] {
                let tile_bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: tile binds a resting background"));
                let tile_bg = color(&theme, tile_bg_name.as_str());
                for key in keys {
                    let text_node = named(&node, key);
                    let fg_name = text_node
                        .props
                        .tokens
                        .get("foreground")
                        .unwrap_or_else(|| panic!("{label}: {key} binds a foreground"));
                    let opacity = text_node.props.opacity.unwrap_or(1.0);
                    let fg = color(&theme, fg_name.as_str()).faded(opacity).over(tile_bg);
                    let ratio = fg.contrast_ratio(tile_bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{label} {key} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                        tile_bg_name.as_str()
                    );
                }
            }
        }
    }
}
