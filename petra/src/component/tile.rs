//! Carbon Tile. Slice-e.
//!
//! Four kinds, four constructors. Carbon documents them as one component
//! with four structural variants, not four sizes of the same chrome:
//!
//! 1. [`tile`] — base: a static container. Enabled only, no role, no
//!    border, no interactions.
//! 2. [`clickable_tile`] — the whole tile is one target (`Role::Button`).
//! 3. [`selectable_tile`] — an option. Single- vs multi-select is caller
//!    grouping, the same line [`super::radio`] draws. Selection is never
//!    colour alone: `Semantics.selected` plus [`IconMark::Check`] when on.
//! 4. [`expandable_tile`] — reveals a below-the-fold body. Click anywhere
//!    on the tile (this constructor has no inner controls). Expansion is
//!    never colour alone: `Semantics.expanded` plus the word
//!    `"expanded"` / `"collapsed"`.
//!
//! Geometry is the SCSS floor, not a size ramp: `min-inline-size` 128
//! (8rem), `min-block-size` 64 (4rem), padding `$spacing-05` on both axes.
//! There is no sm/md/lg. There is no invalid, warning, or skeleton state.
//!
//! Fill is [`SURFACE_RAISED`]. Carbon calls a tile un-elevated — they mean
//! relative to a card, and Carbon has no Card. Petra's card is
//! [`super::section`], which already spends `surface.raised` *and*
//! `shadow.raised`. A tile takes the fill and leaves the shadow, so the two
//! stay distinct on the same page.
//!
//! Interactive tiles take [`BORDER_SUBTLE`] as their edge. Carbon's
//! `$border-tile` is a feature-flag token we do not ship; inventing it
//! here would put a colour decision in a component. The border is the
//! second channel beside the hover fill (`layer-hover`), the same job
//! Carbon's flag-gated 1px line does: mark the tile as a target without
//! waiting for a pointer.

use super::icon::{IconMark, icon};
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_03, SPACING_05,
    SURFACE_RAISED, t,
};
use super::{pad, stack};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

/// Carbon `.cds--tile` `min-inline-size: 8rem`.
const MIN_INLINE: f32 = 128.0;
/// Carbon `.cds--tile` `min-block-size: 4rem`.
const MIN_BLOCK: f32 = 64.0;

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

/// Shared box: raised fill, `$spacing-05` padding, Carbon min size.
///
/// No border, no role, no hover. [`tile`] returns this as-is.
/// Interactive constructors add the edge and the state fills on top.
fn shell(key: impl Into<Key>, axis: Axis, children: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, axis, Some(SPACING_03), children);
    node.props.padding = Some(pad(SPACING_05, SPACING_05));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.with_constraints(floor())
}

/// The interactive-tile chrome: a border, a hover fill, and — when the
/// kind can be selected — the four-fill set [`super::list_row`] pioneered.
///
/// `Hover` has to be declared on the node that binds `background@hover`,
/// or the engine never hit-tests it and the token is a name nothing reads.
fn with_interactive_chrome(mut node: ViewNode, selectable: bool) -> ViewNode {
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
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

/// A base tile: static container, enabled only.
///
/// No role. `Role::Pane` would make a region out of a box that does not
/// collect anything; a container with no actions is outside
/// `ActionableNeedsRoleAndLabel`'s reach, so FR-058 has nothing to enforce
/// here. Callers that need interactive children put them in themselves —
/// this constructor does not grow a children argument, because the
/// inventory's base tile is a box around text, not a layout primitive.
pub fn tile(key: impl Into<Key>, body: impl Into<String>) -> ViewNode {
    shell(key, Axis::Vertical, vec![text("body", body.into())])
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
    with_interactive_chrome(
        shell(key, Axis::Vertical, vec![text("body", body.into())]),
        false,
    )
    .interactive(Role::Button, label, INTERACTIVE)
}

/// A selectable tile: one option in a caller-grouped set.
///
/// Multi-select vs radio is not two components. The caller groups these
/// the way it groups [`super::radio`]. What this constructor guarantees
/// is that selected is a declared fact (`Semantics.selected`) with a
/// second visual channel ([`IconMark::Check`] when on), not a fill swap
/// a colour-blind reader cannot recover.
pub fn selectable_tile(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let label = label.into();
    let mut parts = Vec::new();
    if selected {
        parts.push(icon("mark", IconMark::Check));
    }
    parts.push(text("label", label.clone()));

    let mut node = with_interactive_chrome(shell(key, Axis::Horizontal, parts), true);
    node.props.align = Some(Align::Center);
    let mut node = node.interactive(Role::Button, label, INTERACTIVE);
    node.semantics.selected = selected;
    node
}

/// An expandable tile: a labelled header that reveals `body` below the fold.
///
/// Clicking anywhere on the tile toggles it — this constructor has no
/// inner controls, so it does not need Carbon's "only the chevron
/// button toggles" sub-form. The chevron's job is the second channel:
/// the word `"expanded"` or `"collapsed"` sits beside the label. A
/// canvas chevron without a word would be an icon carrying state alone
/// (FR-026). `Semantics.expanded` is the fact a reader who cannot see
/// either channel still gets.
pub fn expandable_tile(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    body: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let disclosure = if expanded { "expanded" } else { "collapsed" };
    let mut header = stack(
        "header",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![text("label", label.clone()), text("disclosure", disclosure)],
    );
    header.props.align = Some(Align::Center);

    let mut children = vec![header];
    if expanded {
        children.push(text("body", body.into()));
    }

    let mut node = with_interactive_chrome(shell(key, Axis::Vertical, children), false)
        .interactive(Role::Button, label, INTERACTIVE);
    node.semantics.expanded = Some(expanded);
    node
}

#[cfg(test)]
mod tests {
    use super::super::tokens::{
        BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SURFACE_RAISED,
    };
    use super::{MIN_BLOCK, MIN_INLINE, clickable_tile, expandable_tile, selectable_tile, tile};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
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
        assert_eq!(
            token(&node, "border"),
            None,
            "base tile has no interactive edge"
        );
        assert_eq!(token(&node, "background@hover"), None);
    }

    #[test]
    fn tile_min_constraints_are_the_carbon_floor() {
        let nodes = [
            tile("b", "body"),
            clickable_tile("c", "Open", "body"),
            selectable_tile("s", "Plan A", false),
            expandable_tile("e", "Details", false, "more"),
        ];
        for node in nodes {
            assert_eq!(
                node.constraints.horizontal.min,
                Some(MIN_INLINE),
                "{:?} inline min",
                node.key
            );
            assert_eq!(
                node.constraints.vertical.min,
                Some(MIN_BLOCK),
                "{:?} block min",
                node.key
            );
            assert_eq!(node.constraints.horizontal.max, None);
            assert_eq!(node.constraints.vertical.max, None);
        }
    }

    #[test]
    fn clickable_tile_is_interactive_with_role_and_label() {
        let node = clickable_tile("go", "Open project", "Project Alpha");
        assert!(node.is_interactive());
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Open project"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(
            node.interactions.contains(&Interaction::Hover),
            "Hover is what makes background@hover reachable"
        );
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert_eq!(token(&node, "background@hover"), Some(LAYER_HOVER));
        assert_eq!(
            token(&node, "background@selected"),
            None,
            "clickable is not a selected kind; do not bind a slot nothing ranks into"
        );
        assert!(!node.semantics.selected);
    }

    #[test]
    fn selectable_tile_declares_selected_and_draws_a_check() {
        let on = selectable_tile("plan", "Plan A", true);
        assert_eq!(on.semantics.role, Some(Role::Button));
        assert_eq!(on.semantics.label.as_deref(), Some("Plan A"));
        assert!(
            on.semantics.selected,
            "selection is a declared fact, not a fill"
        );
        assert!(
            has_canvas(&on),
            "IconMark::Check is the second channel when selected"
        );
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(
            token(&on, "background@selected-hover"),
            Some(LAYER_SELECTED_HOVER)
        );
        assert_eq!(token(&on, "border"), Some(BORDER_SUBTLE));

        let off = selectable_tile("plan", "Plan A", false);
        assert!(!off.semantics.selected);
        assert!(
            !has_canvas(&off),
            "the check is the on-state mark, not a permanent glyph"
        );
        assert!(off.is_interactive());
    }

    #[test]
    fn expandable_tile_declares_expanded_as_a_second_channel() {
        let open = expandable_tile("more", "Details", true, "the rest");
        assert!(open.is_interactive());
        assert_eq!(open.semantics.role, Some(Role::Button));
        assert_eq!(open.semantics.label.as_deref(), Some("Details"));
        assert_eq!(open.semantics.expanded, Some(true));
        assert!(
            open.children.len() > 1,
            "expanded body sits below the fold as a child"
        );

        let shut = expandable_tile("more", "Details", false, "the rest");
        assert_eq!(shut.semantics.expanded, Some(false));
        assert_eq!(
            shut.children.len(),
            1,
            "collapsed tile keeps the header and drops the body"
        );
        assert_eq!(token(&open, "border"), Some(BORDER_SUBTLE));
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

    /// Check C/D across all four kinds. `selectable_tile`'s check mark
    /// (`IconMark::Check`) and `expandable_tile`'s `"expanded"`/
    /// `"collapsed"` word are the class-4 suspects named in this group's
    /// brief; neither carries its own `Constraints`, unlike Modal's
    /// `close_button` or Number input's stepper, so this is the
    /// frame-level proof neither overflows, not a substitute for reading
    /// the code.
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
                    vec!["label", "disclosure", "body"],
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
