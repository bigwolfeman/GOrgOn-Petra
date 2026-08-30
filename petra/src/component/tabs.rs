//! `tab` and `tab_bar` — one selectable strip.

use super::text::text;
use super::tokens::{
    SHAPE_SM, SPACING_02, SPACING_03, SPACING_04, SURFACE_BASE, SURFACE_RAISED, TEXT_MUTED,
    TEXT_PRIMARY, t,
};
use super::{pad, stack};
use crate::geom::{Align, Axis};
use crate::tree::{Interaction, Key, Role, Semantics, ViewNode};

/// One tab. `selected` swaps its fill *and* is declared in `Semantics`, so
/// which tab is current never rests on colour alone.
pub fn tab(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let key = key.into();
    let label = label.into();

    let mut label_node = text("label", label.clone());
    label_node.props.tokens.insert(
        "foreground".into(),
        t(if selected { TEXT_PRIMARY } else { TEXT_MUTED }),
    );

    let mut node = stack(key, Axis::Horizontal, None, vec![label_node]);
    node.props.align = Some(Align::Center);
    // 8 vertical / 12 horizontal. The previous 4-unit vertical pad plus
    // `shape.corner-sm` (also 4) put the rounded fill through the glyphs.
    node.props.padding = Some(pad(SPACING_04, SPACING_03));
    node.props.tokens.insert(
        "background".into(),
        t(if selected {
            SURFACE_RAISED
        } else {
            SURFACE_BASE
        }),
    );
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));

    let mut node = node.interactive(Role::Tab, label, &[Interaction::Focus, Interaction::Click]);
    node.semantics.selected = selected;
    node
}

/// The strip: wraps already-built [`tab`] children under `Role::TabList`.
///
/// Carries no label of its own. A `TabList` declares no interactions, so
/// FR-058's role-and-label obligation — which is about *actionable* nodes —
/// does not reach it; each `tab` inside already carries its own, which is
/// what [`crate::semantic::AuditRule::ActionableNeedsRoleAndLabel`] checks
/// per node with actions, not per ancestor.
pub fn tab_bar(key: impl Into<Key>, tabs: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_02), tabs);
    node.semantics = Semantics {
        role: Some(Role::TabList),
        ..Semantics::default()
    };
    node
}
