//! Carbon Menu (slice-c). Floating action list on a list box.
//!
//! Anatomy (`_menu.scss`):
//! 1. Menu container — [`super::list_box::list_box`], [`crate::tree::Role::Overlay`]:
//!    `$layer` fill, `0 2px 6px 0 rgba(0,0,0,.2)` shadow, **no caret**,
//!    flush under its trigger and at least as wide as it. Until 2026-09-04
//!    this was [`super::popover::popover_with`], which put a beak on it,
//!    padded it 16 on every side and centred the rows in the box — what
//!    the operator read as "menu button does the same thing as popover".
//! 2. Action item — [`menu_item`] / [`menu_item_with`]: the shared list-box
//!    row in [`super::list_box`]. This module re-exports both so
//!    `menu::menu_item` stays the call site. The overlay here only adds
//!    width bounds and `takes_focus`. [`menu_flyout`] is the same overlay
//!    opening on the trigger's trailing edge, fitted to its own content.
//!
//! Container width is min 160 / max 288 (SCSS `$supported-sizes` map and
//! style-page).
//!
//! # Phase 8 (spec 008 T050): the documented modifier set
//!
//! * `--border` — [`menu_border`], the 1px `$border-subtle` outline the
//!   style page names ("optional `outline: 1px solid $border-subtle` for
//!   `--border`").
//! * `--with-icons` — [`menu_with_icons`], the 12rem/192px minimum and the
//!   leading icon column each row carries through
//!   [`menu_item_with`]'s `icon` (slice-c: "reserves a leading icon
//!   column, widens min-width to 12rem/192px").
//! * `--with-selectable-items` — [`menu_item_selectable`]'s selection
//!   column, with `--with-icons --with-selectable-items` (both columns)
//!   the same row with `icon: Some(..)` beside its checkmark.
//! * `menu-item-group` / `menu-item-radio-group` — [`menu_item_group`] /
//!   [`menu_item_radio_group`], the two structural sub-parts, and the only
//!   rows a menu does not re-pin itself (see [`menu_at`]).
//! * `menu-item-divider` — [`menu_item_divider`], the 1px `$border-subtle`
//!   rule with `$spacing-02` above and below that splits a menu's rows into
//!   groups. Not a row either, so the menu leaves its height alone.
//!
//! ## Submenu (manifest row 257)
//!
//! [`menu_submenu`] is a row with a trailing chevron and, while open, a
//! second menu anchored to that row's trailing edge *inside the first
//! menu's surface*. That nesting is spec 005's anchored placement doing its
//! documented job: the harvest runs one extra walk per level of surface
//! nesting, so the inner menu resolves against where the outer one was
//! actually placed, and the flip ladder moves it to the row's leading edge
//! when the window has no room on the trailing one. A press inside the
//! submenu is inside the menu that contains it, so opening the submenu
//! does not dismiss its parent (`input::route_with_surfaces`).
//!
//! ## What is still refused (FR-006)
//!
//! * **Danger hover** (rows 262/263) is refused on the item's own terms:
//!   there is no danger item kind in this family (`menu_item_with` has no
//!   `kind` parameter), so the hover states of a kind that does not exist
//!   are not a viewport of anything. That reasoning is recorded as
//!   exclusion rows in `VARIANT-EXCLUSIONS.md` (T057, FR-006) rather than
//!   left as a doc comment.

use std::sync::Arc;

use super::list_box::{Dividers, ListBoxSize, list_box};
use super::pin_block;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, SPACING_02, SPACING_05, SURFACE_RAISED, TEXT_MUTED, TYPOGRAPHY_LABEL, t,
};
use crate::geom::Align as CrossAlign;
use crate::geom::Axis;
use crate::tree::{
    Align, Anchor, AxisConstraint, Edge, Fit, InsetRefs, Key, Role, Semantics, ViewNode,
};

#[doc(inline)]
pub use super::list_box::{menu_item, menu_item_selectable, menu_item_with};

/// The role a [`menu_item_group`] declares: ARIA's own `group` name.
pub(crate) const GROUP_ROLE: &str = "group";
/// The role a [`menu_item_radio_group`] declares: ARIA's own `radiogroup`.
pub(crate) const RADIO_GROUP_ROLE: &str = "radiogroup";
/// The role a [`menu_submenu`]'s wrapper declares. Not an ARIA role (ARIA
/// puts `aria-haspopup` on the row itself, which [`Semantics::expanded`]
/// carries here); it exists so the enclosing menu re-pins the row inside it
/// and leaves the nested surface alone.
pub(crate) const SUBMENU_ROLE: &str = "submenu";
/// The role a [`menu_item_divider`] declares: ARIA's own `separator`. Like
/// the groups, it is not a row, so [`re_pin`] leaves its height alone.
pub(crate) const DIVIDER_ROLE: &str = "separator";
/// A [`menu_item_divider`]'s line: Carbon's `block-size: 1px`.
const DIVIDER_THICKNESS: f32 = 1.0;
/// The key of the row inside a [`menu_submenu`], the one its flyout anchors
/// to.
const SUBMENU_ROW: &str = "row";

/// Carbon menu min-inline (`10rem`). Also the menu button trigger's
/// minimum, which is how Carbon's shot has the two the same width.
pub(crate) const MIN_INLINE: f32 = 160.0;
/// Carbon menu min-inline under `--with-icons` (`12rem`, style page).
pub(crate) const MIN_INLINE_ICONS: f32 = 192.0;
/// Carbon menu max-inline (`18rem`). `pub(crate)` so the menu-button
/// family can assert its halves add up to the family bounds (T051).
pub(crate) const MAX_INLINE: f32 = 288.0;

const _: () = assert!(MIN_INLINE == 160.0);
const _: () = assert!(MIN_INLINE_ICONS == 192.0);
const _: () = assert!(MAX_INLINE == 288.0);

/// A floating menu of `items`, hosted on a list box, rows at Carbon md
/// (40).
///
/// Anchored to a sibling keyed `"trigger"` (the key [`super::menu_button`]
/// uses for its button). `label` is the accessible name of the overlay.
/// The min/max here bound the content; a trigger wider than 160 still gets
/// a menu its own width (`Fit::Anchor`).
pub fn menu(key: impl Into<Key>, label: impl Into<String>, items: Vec<ViewNode>) -> ViewNode {
    menu_at(key, label, items, ListBoxSize::Md)
}

/// The menu at Carbon xs: rows 24.
pub fn menu_xs(key: impl Into<Key>, label: impl Into<String>, items: Vec<ViewNode>) -> ViewNode {
    menu_at(key, label, items, ListBoxSize::Xs)
}

/// The menu at Carbon sm: rows 32.
pub fn menu_sm(key: impl Into<Key>, label: impl Into<String>, items: Vec<ViewNode>) -> ViewNode {
    menu_at(key, label, items, ListBoxSize::Sm)
}

/// The menu at Carbon lg: rows 48.
pub fn menu_lg(key: impl Into<Key>, label: impl Into<String>, items: Vec<ViewNode>) -> ViewNode {
    menu_at(key, label, items, ListBoxSize::Lg)
}

/// The menu owns the row height. Slice-c "Sizes": "four menu-item heights,
/// driven by the trigger that opened the menu" — xs 24 / sm 32 / md 40 /
/// lg 48, the same ramp [`ListBoxSize`] names — so whatever row a caller
/// builds, the menu re-pins it to the step the trigger chose (the same
/// rule [`super::content_switcher`] applies to its tabs). This replaces
/// the wave-1–4 placeholder pin (`65c3c43`) that froze rows at md and
/// refused the other three documented steps (FR-026).
///
/// A group is not a row: [`menu_item_group`] and
/// [`menu_item_radio_group`] keep their own height and only their
/// `Role::Button` rows take the step, or a grouped menu at lg would pin
/// the group — label and rows together — to one row's 48.
pub(crate) fn menu_at(
    key: impl Into<Key>,
    label: impl Into<String>,
    mut items: Vec<ViewNode>,
    size: ListBoxSize,
) -> ViewNode {
    let height = size.height();
    for item in &mut items {
        re_pin(item, height);
    }
    let mut node = list_box(key, label, "trigger", items, Dividers::None);
    node.constraints.horizontal = AxisConstraint {
        min: Some(MIN_INLINE),
        max: Some(MAX_INLINE),
        priority: 0,
    };
    // Carbon's Menu takes keyboard focus for as long as it is open
    // (`Menu.js`'s `handleOpen`/`focusItem`/`handleClose`), and it is the
    // only list box in this crate that does: Dropdown and Combo box leave
    // focus on the trigger. Without this the trigger stays focused under an
    // open menu, and the cover rule then withholds its underline because the
    // menu sits on the strip the bar would occupy — no indicator anywhere.
    node.props.takes_focus = Some(true);
    node
}

/// A submenu flyout: the same overlay as [`menu`], opening on the trigger's
/// trailing edge and sized to its items rather than the trigger height.
///
/// [`Fit::Anchor`] would inherit a 40-unit trigger as the flyout's width.
/// [`Fit::Content`] lets the 160/288 min/max bound the items instead.
pub fn menu_flyout(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    flyout_on(key, label, items, "trigger")
}

/// [`menu`] opening on the trailing edge of the sibling keyed `anchor`,
/// fitted to its own items: the one shape [`menu_flyout`] and
/// [`menu_submenu`] share.
fn flyout_on(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
    anchor: &str,
) -> ViewNode {
    let mut node = menu(key, label, items);
    node.props.anchor = Some(Anchor::Sibling {
        key: anchor.into(),
        edge: Edge::Right,
        align: Align::Start,
        offset: None,
    });
    node.props.fit = Some(Fit::Content);
    node
}

/// A menu row that opens a submenu (Carbon `MenuItem` with children, manifest
/// row 257): the row with its trailing chevron, and while `open` a nested
/// [`menu`] of `items` hanging off the row's trailing edge, top-aligned to
/// it, flipping to the leading edge when the window has no room.
///
/// The row says whether it is open through [`Semantics::expanded`], so a
/// driver and a screen reader read the state the chevron and the flyout
/// show. The application owns `open` the way it owns a menu's own: Carbon
/// opens a submenu on hover, Enter, Space or ArrowRight and closes it on
/// ArrowLeft or Escape, and every one of those arrives as an event naming
/// the row.
///
/// Goes in a menu's `items` like any row; the menu re-pins the row to its
/// step ([`SUBMENU_ROLE`]).
pub fn menu_submenu(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
    open: bool,
) -> ViewNode {
    let label = label.into();
    let mut row = menu_item_with(SUBMENU_ROW, label.clone(), None, None, true);
    row.semantics.expanded = Some(open);
    let mut children = vec![row];
    if open {
        children.push(flyout_on("submenu", label.clone(), items, SUBMENU_ROW));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.props.align = Some(CrossAlign::Stretch);
    node.semantics = Semantics {
        role: Some(Role::Custom(SUBMENU_ROLE.to_owned())),
        label: Some(label),
        ..Semantics::default()
    };
    node
}

/// Re-pin one menu child to the trigger's step. A row takes the step whole;
/// a group (its own role, see [`GROUP_ROLE`]) is **not** a row — only its
/// `Role::Button` rows take the step, its label and its column keep the
/// height the group built them with. That is what keeps a grouped menu at
/// lg from pinning the group — label and rows together — to one row's 48.
fn re_pin(node: &mut ViewNode, height: f32) {
    match node.semantics.role {
        Some(Role::Custom(ref name))
            if name == GROUP_ROLE || name == RADIO_GROUP_ROLE || name == SUBMENU_ROLE =>
        {
            for child in &mut node.children {
                let child = Arc::make_mut(child);
                if child.semantics.role == Some(Role::Button) {
                    child.constraints = pin_block(height);
                }
            }
        }
        Some(Role::Custom(ref name)) if name == DIVIDER_ROLE => {}
        _ => node.constraints = pin_block(height),
    }
}

/// `menu-item-divider`: Carbon's `.cds--menu-item-divider`, the rule that
/// splits a menu into groups of related rows. A 1px [`BORDER_SUBTLE`] line
/// across the menu's whole width with [`SPACING_02`] above and below
/// (`block-size: 1px; margin: $spacing-02 0`). It declares ARIA's
/// `separator` role ([`DIVIDER_ROLE`]): a driver reads it as a boundary and
/// skips it, and the menu does not pin it to the row height.
pub fn menu_item_divider(key: impl Into<Key>) -> ViewNode {
    // The line is a childless strip one unit tall, not a `Separator`: a
    // separator answers the whole offer along its axis, so inside a menu
    // that fits its content it pushed the menu out to its 288 maximum. A
    // childless strip has no width of its own; the `Stretch` below gives
    // it the menu's.
    let mut line = stack("rule", Axis::Vertical, None, Vec::new());
    line.constraints.vertical = AxisConstraint {
        min: Some(DIVIDER_THICKNESS),
        max: Some(DIVIDER_THICKNESS),
        priority: 0,
    };
    line.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));
    let mut node = stack(key, Axis::Vertical, None, vec![line]);
    node.props.align = Some(CrossAlign::Stretch);
    node.props.padding = Some(InsetRefs {
        top: Some(t(SPACING_02)),
        bottom: Some(t(SPACING_02)),
        ..InsetRefs::default()
    });
    // The strip wears the menu's own fill. The overlay's fill does not reach
    // between rows (each row paints its own), so a bare strip showed the
    // window behind it as a dark band around the line.
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.semantics = Semantics {
        role: Some(Role::Custom(DIVIDER_ROLE.to_owned())),
        ..Semantics::default()
    };
    node
}

/// `menu-border`: [`menu`] wearing the 1px `$border-subtle` outline the
/// style page names as optional ("optional `outline: 1px solid
/// $border-subtle` for `--border`"). The border is one token binding on the
/// overlay — the same channel [`super::tag::tag`]'s outline uses — so it is
/// in the digest and the frame record like every other chrome decision.
pub fn menu_border(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    let mut node = menu(key, label, items);
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node
}

/// `menu-with-icons`: [`menu`] at the widened `--with-icons` minimum
/// (`12rem`/192px, style page) instead of 160. The leading icon column is
/// each row's own — [`menu_item_with`]'s `icon`, or
/// [`menu_item_selectable`]'s `icon` beside its checkmark — so the modifier
/// owns the width and the rows own their columns.
pub fn menu_with_icons(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    let mut node = menu(key, label, items);
    node.constraints.horizontal.min = Some(MIN_INLINE_ICONS);
    node
}

/// `menu-item-group`: a labelled column of rows inside a menu — Carbon's
/// first structural sub-part ("grouped item collections"). The group is a
/// vertical run with [`GROUP_ROLE`] (ARIA's own `group` name) and a
/// [`TYPOGRAPHY_LABEL`] caption in [`TEXT_MUTED`], indented to the rows'
/// [`SPACING_05`] margin and titled above them at [`SPACING_02`]. The rows
/// are whatever the caller builds; the menu re-pins them to the trigger's
/// step and leaves the group itself alone (see [`re_pin`]).
pub fn menu_item_group(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    menu_item_group_at(key, label, items, GROUP_ROLE)
}

/// `menu-item-radio-group`: [`menu_item_group`] declaring
/// [`RADIO_GROUP_ROLE`] (ARIA's `radiogroup`) — a single-choice collection
/// whose chosen row is marked by [`menu_item_selectable`]'s checkmark and
/// `Semantics::selected`. The role is the whole difference between the two
/// sub-parts: it is what tells a driver the collection picks one, while the
/// group's role says it picks none.
pub fn menu_item_radio_group(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    menu_item_group_at(key, label, items, RADIO_GROUP_ROLE)
}

/// The structural sub-part both group constructors build: a labelled
/// vertical run under `role`. One shape, two roles — the two Carbon
/// sub-parts differ in what the collection means, not in how it is drawn.
fn menu_item_group_at(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
    role: &str,
) -> ViewNode {
    let label = label.into();
    let mut caption_text = text("label-text", label.clone());
    caption_text.props.style = Some(t(TYPOGRAPHY_LABEL));
    caption_text
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    // The caption's inset is declared on its own container, not on the
    // text: tree acceptance refuses padding on a leaf — a leaf has no
    // children to inset — so the margin the caption needs has to live on
    // a node that has them.
    let mut caption = stack("label", Axis::Vertical, None, vec![caption_text]);
    caption.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        top: Some(t(SPACING_02)),
        bottom: Some(t(SPACING_02)),
    });
    let mut children = vec![caption];
    children.extend(items);
    let mut node = stack(key, Axis::Vertical, None, children);
    node.props.align = Some(CrossAlign::Stretch);
    node.semantics = Semantics {
        role: Some(Role::Custom(role.to_owned())),
        label: Some(label),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::{
        BORDER_SUBTLE, GROUP_ROLE, MAX_INLINE, MIN_INLINE, MIN_INLINE_ICONS, RADIO_GROUP_ROLE,
        SUBMENU_ROLE, TEXT_MUTED, TYPOGRAPHY_LABEL, menu, menu_border, menu_flyout, menu_item,
        menu_item_group, menu_item_radio_group, menu_item_selectable, menu_lg, menu_submenu,
        menu_with_icons, pin_block,
    };
    use crate::component::disabled;
    use crate::component::icon::IconMark;
    use crate::component::tests::{assert_fits_parent, petrify_lone, petrify_lone_at};

    use crate::geom::Axis;

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{Anchor, Edge, Fit, NodeKind, Props, Role, Tip, TrackSize, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    #[test]
    fn menu_is_a_flush_list_box_not_a_popover() {
        let node = menu(
            "actions",
            "Actions",
            vec![menu_item("rename", "Rename"), menu_item("delete", "Delete")],
        );
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Actions"));
        assert!(node.interactions.is_empty());
        match &node.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "trigger"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        assert_eq!(node.constraints.horizontal.min, Some(MIN_INLINE));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MIN_INLINE, 160.0);
        assert_eq!(MAX_INLINE, 288.0);
        assert_eq!(
            node.props.tip,
            Some(Tip::Flush),
            "Carbon's menu has no beak"
        );
        assert_eq!(node.props.fit, Some(Fit::Anchor));
        assert_eq!(
            node.props.takes_focus,
            Some(true),
            "Carbon's Menu moves focus into itself when it opens"
        );
        assert!(node.props.padding.is_none(), "rows run edge to edge");
        let content = child(&node, "content");
        assert!(
            content.children.iter().all(|c| c.key.as_str() != "caret"),
            "no caret word in a menu"
        );
        let rename = child(content, "rename");
        assert_eq!(rename.semantics.role, Some(Role::Button));
        assert_eq!(rename.semantics.label.as_deref(), Some("Rename"));
        assert_eq!(
            child(content, "delete").semantics.label.as_deref(),
            Some("Delete")
        );
    }

    /// A flyout opens to the right of its trigger and sizes to its items,
    /// not the trigger's 40-unit height.
    #[test]
    fn menu_flyout_opens_to_the_right_and_fits_content() {
        let node = menu_flyout("actions", "Actions", vec![menu_item("rename", "Rename")]);
        match &node.props.anchor {
            Some(Anchor::Sibling { key, edge, .. }) => {
                assert_eq!(key.as_str(), "trigger");
                assert_eq!(*edge, Edge::Right);
            }
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        assert_eq!(node.props.fit, Some(Fit::Content));
        assert_eq!(node.props.takes_focus, Some(true));
        assert_eq!(node.constraints.horizontal.min, Some(MIN_INLINE));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
    }

    /// `menu()` IS the anchored surface, naming a `trigger` sibling by bare
    /// key (`Anchor::Sibling`). Placed beside a control keyed `trigger`, it
    /// is accepted wherever the caller mounts the pair — here two
    /// containers below the root, the gallery catalog's own depth.
    #[test]
    fn menu_validates_beside_its_trigger_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "menu",
            vec![
                crate::component::button("trigger", "Actions"),
                menu(
                    "actions",
                    "Actions",
                    vec![menu_item("rename", "Rename"), menu_item("delete", "Delete")],
                ),
            ],
        );
    }

    /// Manifest row 257: an open submenu hangs off its row's trailing edge,
    /// top-aligned to it, *where the outer menu was actually placed*. The
    /// outer menu sits under its trigger, so the row is nowhere near the
    /// viewport's origin; a submenu resolved against a rect the first walk
    /// had not placed yet would land beside the wrong place. In a window too
    /// narrow for the trailing side, the flip puts it on the leading side.
    #[test]
    fn an_open_submenu_hangs_off_its_row_inside_the_placed_menu() {
        let pair = |open: bool| {
            ViewNode::new(NodeKind::Stack, "pair")
                .with_props(Props {
                    axis: Some(Axis::Vertical),
                    ..Props::default()
                })
                .child(crate::component::button("trigger", "Actions"))
                .child(menu(
                    "actions",
                    "Actions",
                    vec![
                        menu_item("rename", "Rename"),
                        menu_submenu(
                            "more",
                            "More",
                            vec![menu_item("copy", "Copy"), menu_item("move", "Move")],
                            open,
                        ),
                    ],
                ))
        };
        let shut = petrify_lone(pair(false));
        assert!(
            !shut.placements.iter().any(|p| p.id.ends_with("/submenu")),
            "a shut submenu places nothing"
        );
        let row_of = |frame: &crate::frame::PetrifiedFrame| {
            let row = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/more/row"))
                .expect("the submenu row is placed");
            assert_eq!(
                row.semantics.expanded,
                Some(
                    frame
                        .placements
                        .iter()
                        .any(|p| p.id.ends_with("/more/submenu"))
                )
            );
            row.rect
        };
        assert!(row_of(&shut).h > 0.0);

        let frame = petrify_lone_at(pair(true), crate::geom::Size::new(900.0, 600.0));
        let row = row_of(&frame);
        let sub = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/more/submenu"))
            .expect("the open submenu is placed")
            .rect;
        assert!(
            row.y > 40.0,
            "the row sits in the menu under the trigger: {row:?}"
        );
        assert!(
            (sub.x - row.right()).abs() < 0.5,
            "trailing edge: row {row:?}, submenu {sub:?}"
        );
        assert!(
            (sub.y - row.y).abs() < 0.5,
            "top-aligned: row {row:?}, submenu {sub:?}"
        );
        for key in ["/submenu/content/copy", "/submenu/content/move"] {
            let item = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(key))
                .unwrap_or_else(|| panic!("{key} is placed"));
            assert!(sub.contains(crate::geom::Point::new(
                item.rect.x + 1.0,
                item.rect.y + 1.0
            )));
        }

        // Pushed to the window's right edge by a fixed 400 column, the
        // trailing side has no room (400 + 160 + 160 > 600) and the leading
        // side does, so the flip puts the submenu there.
        let pushed = ViewNode::new(NodeKind::Grid, "pushed")
            .with_props(Props {
                columns: vec![TrackSize::Fixed { value: 400.0 }, TrackSize::FitContent],
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Stack, "spacer"))
            .child(pair(true));
        let flipped = petrify_lone_at(pushed, crate::geom::Size::new(600.0, 600.0));
        let row = row_of(&flipped);
        let sub = flipped
            .placements
            .iter()
            .find(|p| p.id.ends_with("/more/submenu"))
            .unwrap()
            .rect;
        assert!(row.x >= 399.5, "the column pushed the menu right: {row:?}");
        assert!(
            (sub.right() - row.x).abs() < 0.5,
            "no room on the trailing side, so it flips leading: row {row:?}, submenu {sub:?}"
        );
        assert!(
            (sub.y - row.y).abs() < 0.5,
            "still top-aligned: row {row:?}, submenu {sub:?}"
        );
    }

    /// The submenu's row is re-pinned to the trigger's step like any row,
    /// and says whether it is open.
    #[test]
    fn a_submenu_row_takes_the_menu_step_and_reports_expanded() {
        let built = menu_lg(
            "actions",
            "Actions",
            vec![menu_submenu(
                "more",
                "More",
                vec![menu_item("copy", "Copy")],
                true,
            )],
        );
        let wrapper = &built.children[0].children[0];
        assert_eq!(
            wrapper.semantics.role,
            Some(Role::Custom(SUBMENU_ROLE.into()))
        );
        let row = &wrapper.children[0];
        assert_eq!(row.semantics.expanded, Some(true));
        assert_eq!(row.constraints.vertical.min, Some(48.0), "lg step");
        let submenu = &wrapper.children[1];
        assert_eq!(submenu.kind, NodeKind::Surface);
        assert_eq!(
            submenu.constraints.vertical.min, None,
            "the nested surface is not a row"
        );
    }

    // `menu_item` carries no anchor, so — unlike `date_picker`'s
    // `day_button`, which is reachable only inside the anchored calendar —
    // it is audited below with a real `petrify_lone`, the same as any
    // standalone component.

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: two menu items, one of them disabled, place with real
    /// rects, none of them outside their parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let items = ViewNode::new(NodeKind::Stack, "items")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(menu_item("rename", "Rename"))
            .child(disabled(menu_item("delete", "Delete")));
        let frame = petrify_lone(items);
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
                assert_fits_parent("", p, &frame.placements[parent_idx]);
            }
        }
    }

    /// Check F: an enabled item is reachable; a disabled one is not.
    #[test]
    fn an_enabled_item_is_reachable_and_a_disabled_one_is_not() {
        let frame = petrify_lone(menu_item("rename", "Rename"));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let item = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/rename"))
            .expect("the item is placed");
        assert!(
            order.iter().any(|o| o == &item.id),
            "the item declares Focus but is not in focus order"
        );

        let disabled_frame = petrify_lone(disabled(menu_item("delete", "Delete")));
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_order = disabled_focus.order();
        let disabled_item = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/delete"))
            .expect("the disabled item is placed");
        assert!(
            !disabled_order.iter().any(|o| o == &disabled_item.id),
            "a disabled item must not be reachable"
        );
    }

    /// Check E: the item's label against the item's own resting fill, in
    /// both themes.
    #[test]
    fn item_label_clears_aa_contrast_against_its_own_resting_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = menu_item("rename", "Rename");
            let bg_name = node
                .props
                .tokens
                .get("background")
                .expect("menu_item binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            let caption = child(&node, "label");
            let fg_name = caption
                .props
                .tokens
                .get("foreground")
                .expect("label binds a foreground");
            let opacity = caption.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
            let ratio = fg.contrast_ratio(bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "menu item label at {ratio:.2}:1 against {} fails AA \
                 {MIN_TEXT_CONTRAST}:1",
                fg_name.as_str()
            );
        }
    }

    /// `--border` is the style page's optional 1px `$border-subtle`
    /// outline; the plain menu has none. One token binding on the overlay,
    /// so the digest and the frame record carry it like any other chrome.
    #[test]
    fn menu_border_wears_the_one_pixel_outline_the_style_page_names() {
        let bordered = menu_border("actions", "Actions", vec![menu_item("rename", "Rename")]);
        assert_eq!(
            bordered.props.tokens.get("border").map(|v| v.as_str()),
            Some(BORDER_SUBTLE),
            "the --border menu binds $border-subtle"
        );
        let plain = menu("actions", "Actions", vec![menu_item("rename", "Rename")]);
        assert!(
            !plain.props.tokens.contains_key("border"),
            "the plain menu wears no outline"
        );
    }

    /// `--with-icons` widens the menu's minimum to 12rem/192px (style
    /// page) and leaves the 18rem maximum alone.
    #[test]
    fn menu_with_icons_widens_the_minimum_to_twelve_rem() {
        assert_eq!(MIN_INLINE_ICONS, 192.0, "12rem at 16 units per rem");
        let node = menu_with_icons("actions", "Actions", vec![menu_item("open", "Open")]);
        assert_eq!(node.constraints.horizontal.min, Some(MIN_INLINE_ICONS));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
    }

    /// The selection column is reserved whether or not it holds a
    /// checkmark: both labels land 40 units in (16 padding + 16 column +
    /// 8 gap) in the placed frame, selected and unselected alike —
    /// measured, not eyeballed.
    #[test]
    fn a_selectable_item_reserves_its_column_whether_or_not_it_is_selected() {
        let items = ViewNode::new(NodeKind::Stack, "items")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(menu_item_selectable("keep", "Keep", None, false))
            .child(menu_item_selectable("share", "Share", None, true));
        let frame = petrify_lone(items);
        let x = |suffix: &str| -> f32 {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("nothing placed for {suffix}"))
                .rect
                .x
        };
        for row in ["keep", "share"] {
            assert_eq!(
                x(&format!("/{row}/label")) - x(&format!("/{row}")),
                40.0,
                "{row}'s label must start 40 units in (16 padding + 16 column + 8 gap)"
            );
        }
        assert_eq!(
            x("/keep/label"),
            x("/share/label"),
            "selected and unselected labels share one offset"
        );
    }

    /// Selection is a declared fact plus a checkmark, never a fill: both
    /// rows keep the same `background` binding, and the column is a pinned
    /// 16-unit slot — empty when not selected, the glyph when selected.
    #[test]
    fn selection_is_a_checkmark_and_a_semantic_flag_never_a_fill() {
        let unselected = menu_item_selectable("keep", "Keep", None, false);
        let selected = menu_item_selectable("share", "Share", None, true);
        assert!(!unselected.semantics.selected);
        assert!(selected.semantics.selected);
        assert_eq!(
            unselected.props.tokens.get("background"),
            selected.props.tokens.get("background"),
            "selection must not recolour the row"
        );
        let slot = child(&unselected, "selection");
        assert_eq!(slot.kind, NodeKind::Stack, "an unselected column is empty");
        assert_eq!(slot.constraints.horizontal.min, Some(16.0));
        assert_eq!(slot.constraints.horizontal.max, Some(16.0));
        assert_eq!(slot.constraints.vertical.min, Some(16.0));
        assert_eq!(slot.constraints.vertical.max, Some(16.0));
        let mark = child(&selected, "selection");
        assert_eq!(
            mark.kind,
            NodeKind::Canvas,
            "a selected column is the checkmark glyph"
        );
        assert_eq!(mark.constraints.horizontal.min, Some(16.0));
        assert_eq!(mark.constraints.vertical.min, Some(16.0));
    }

    /// The combined `--with-icons --with-selectable-items` row: the
    /// selection column first, the icon column beside it, then the label —
    /// both columns on one row.
    #[test]
    fn the_combined_row_carries_the_selection_column_and_the_icon_column() {
        let node = menu_item_selectable("edit", "Edit", Some(IconMark::Edit), true);
        let keys: Vec<&str> = node.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["selection", "icon", "label"]);
    }

    /// The two structural sub-parts differ in what the collection means:
    /// `group` picks none, `radiogroup` picks one (ARIA's own names). Both
    /// are a labelled column — muted label caption above `Role::Button`
    /// rows the menu re-pins.
    #[test]
    fn the_group_is_a_labelled_collection_and_the_radio_group_says_one_choice() {
        let groups = [
            (
                "View",
                menu_item_group("view", "View", vec![menu_item("zoom-in", "Zoom in")]),
                GROUP_ROLE,
            ),
            (
                "Size",
                menu_item_radio_group("size", "Size", vec![menu_item("sm", "Small")]),
                RADIO_GROUP_ROLE,
            ),
        ];
        for (name, node, role) in groups {
            assert_eq!(
                node.semantics.role,
                Some(Role::Custom(role.to_owned())),
                "{name} declares its ARIA role"
            );
            assert_eq!(node.semantics.label.as_deref(), Some(name));
            let caption = child(&node, "label");
            let words = child(caption, "label-text");
            assert_eq!(
                words.props.style.as_ref().map(|v| v.as_str()),
                Some(TYPOGRAPHY_LABEL),
                "{name}'s caption is label-styled"
            );
            assert_eq!(
                words.props.tokens.get("foreground").map(|v| v.as_str()),
                Some(TEXT_MUTED),
                "{name}'s caption is muted ink"
            );
            assert!(
                node.children
                    .iter()
                    .skip(1)
                    .all(|row| row.semantics.role == Some(Role::Button)),
                "{name} is a column of button rows"
            );
        }
    }

    /// The menu owns the row height, but a group is not a row: only its
    /// `Role::Button` rows take the trigger's step — here lg 48 — while
    /// the group and its label keep the height they built.
    #[test]
    fn the_menu_re_pins_group_rows_to_the_trigger_step_but_not_the_group() {
        let node = menu_lg(
            "actions",
            "Actions",
            vec![
                menu_item("rename", "Rename"),
                menu_item_group("view", "View", vec![menu_item("zoom-in", "Zoom in")]),
            ],
        );
        let content = child(&node, "content");
        assert_eq!(child(content, "rename").constraints, pin_block(48.0));
        let group = child(content, "view");
        assert_ne!(group.constraints, pin_block(48.0), "a group is not a row");
        assert_eq!(
            child(group, "zoom-in").constraints,
            pin_block(48.0),
            "its rows take the trigger's step"
        );
        assert_ne!(
            child(group, "label").constraints,
            pin_block(48.0),
            "its label does not"
        );
    }
}
