//! The flag parameters opened for the Lua gallery's radio, tag, tile, UI
//! shell header, toggle-button, left-panel and right-panel pages, judged on
//! the frame
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).
//!
//! Each row must fold to exactly the picture the literal reference draws in
//! both flag states (digest parity), before and after a commit, and a
//! commit regrows only the bound reference: its literal twin beside it
//! keeps its node.

#[allow(dead_code, reason = "a shared support module; this file uses a subset")]
#[path = "bound_components/support.rs"]
mod support;

use std::sync::Arc;

use gorgon_petra::component::registry::expand;
use gorgon_petra::tree::{NodeKind, ResolveInputs, SlotChange, ViewNode};
use serde_json::{Value, json};
use support::{commit, flag, frame, reference, seeded};

/// `(component, flag parameter, the other parameters)`.
fn rows() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("radio", "selected", json!({ "label": "Dark" })),
        ("selectable_tag", "selected", json!({ "label": "Pick" })),
        ("selectable_tile", "selected", json!({ "label": "Pick" })),
        (
            "expandable_tile",
            "expanded",
            json!({ "label": "More", "body": "Below the fold." }),
        ),
        ("ui_shell_header_menu_trigger", "open", json!({})),
        (
            "ui_shell_header_nav_item",
            "selected",
            json!({ "label": "Fibers" }),
        ),
    ]
}

/// A page holding the reference under test (bound to `on`, or literal when
/// `lit` is `Some`) and a literal twin that is always on.
fn page(name: &str, param: &str, rest: &Value, lit: Option<bool>) -> ViewNode {
    let params = |key: &str, value: Option<bool>| {
        let mut params = rest.clone();
        params["key"] = json!(key);
        if let Some(value) = value {
            params[param] = json!(value);
        }
        params
    };
    let subject = match lit {
        Some(value) => reference("subject", name, params("subject", Some(value)), &[]),
        None => reference("subject", name, params("subject", None), &[(param, "on")]),
    };
    ViewNode::new(NodeKind::Stack, "page")
        .child(subject)
        .child(reference("twin", name, params("twin", Some(true)), &[]))
}

/// The property every opened flag owes (`name.param` bound to `on`):
/// digests exactly the picture the literal reference draws in both states,
/// before and after each commit, and a commit regrows only the bound
/// reference — its literal twin beside it keeps its node.
fn flag_reexpansion(name: &str, param: &str, rest: &Value) {
    let literal =
        |on: bool| frame(&expand(&page(name, param, rest, Some(on))).expect("the literal expands"));
    let mut values: ResolveInputs = [("on".into(), flag(false))].into_iter().collect();
    let (mut expanded, mut resolved) = seeded(&page(name, param, rest, None), &values);
    assert_eq!(
        frame(resolved.tree()).digest,
        literal(false).digest,
        "`{name}.{param}` bound off digests like the literal off"
    );
    let twin = Arc::clone(&resolved.tree().children[1]);
    for (version, on) in [(2, true), (4, false)] {
        let named = commit(
            &mut expanded,
            &mut resolved,
            &mut values,
            &[SlotChange::new("on", version, flag(on))],
        );
        assert!(
            named.iter().all(|id| id.starts_with("/page/subject")),
            "`{name}`: only the bound reference may be named, named {named:?}"
        );
        assert_eq!(
            frame(resolved.tree()).digest,
            literal(on).digest,
            "`{name}.{param}` bound {on} digests like the literal {on}"
        );
    }
    assert!(
        Arc::ptr_eq(&twin, &resolved.tree().children[1]),
        "`{name}`: the literal twin keeps its node"
    );
}

#[test]
fn every_opened_flag_digests_the_literal_in_both_states_and_grafts_alone() {
    for (name, param, rest) in rows() {
        flag_reexpansion(name, param, &rest);
    }
}

/// `toggle_button.pressed` is the group button's own press flag
/// (`KeyLabelPressed::pressed: bool`): the state the toggle-button page
/// drives from its choice of List / Grid / Both.
/// Falsified 2026-10-07 by removing `("toggle_button", &[("pressed",
/// PropType::Bool)])` from `OPENABLE`, which is what that table shipped
/// before the row joined:
///
/// ```text
/// thread 'a_bound_toggle_button_press_digests_the_literal_in_both_states_and_grafts_alone' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "toggle_button", reason: "parameter `pressed` is not openable: a component expands from literal parameters, and `toggle_button` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_toggle_button_press_digests_the_literal_in_both_states_and_grafts_alone() {
    flag_reexpansion("toggle_button", "pressed", &json!({ "label": "List" }));
}

/// `toggle_button_icon.pressed` — the same flag on the mark-carrying
/// button (`KeyLabelPressedMark::pressed: bool`), which the toggle-button
/// page's "Both" control binds just as its two plain siblings bind theirs.
/// Falsified 2026-10-07 by removing `("toggle_button_icon", &[("pressed",
/// PropType::Bool)])` from `OPENABLE`, which is what that table shipped
/// before the row joined:
///
/// ```text
/// thread 'a_bound_toggle_button_icon_press_digests_the_literal_in_both_states_and_grafts_alone' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "toggle_button_icon", reason: "parameter `pressed` is not openable: a component expands from literal parameters, and `toggle_button_icon` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_toggle_button_icon_press_digests_the_literal_in_both_states_and_grafts_alone() {
    flag_reexpansion(
        "toggle_button_icon",
        "pressed",
        &json!({ "label": "Both", "mark": "menu" }),
    );
}

/// `ui_shell_left_panel_icon_item.selected` marks the current page's row
/// (`LeftPanelIconItemParams::selected: bool`); `marked` derives from it
/// beside any selected child, so a commit must regrow the row's own
/// picture exactly.
/// Falsified 2026-10-07 by removing the `("ui_shell_left_panel_icon_item",
/// &[("expanded", ...), ("selected", ...)])` row from `OPENABLE`, which is
/// what that table shipped before the row joined:
///
/// ```text
/// thread 'a_bound_left_panel_icon_item_selection_digests_the_literal_in_both_states_and_grafts_alone' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "ui_shell_left_panel_icon_item", reason: "parameter `selected` is not openable: a component expands from literal parameters, and `ui_shell_left_panel_icon_item` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_left_panel_icon_item_selection_digests_the_literal_in_both_states_and_grafts_alone() {
    flag_reexpansion(
        "ui_shell_left_panel_icon_item",
        "selected",
        &json!({ "label": "Petra", "mark": "edit", "expanded": false }),
    );
}

/// `ui_shell_left_panel_icon_subitem.selected` — the nested row's own
/// current-page flag (`KeyLabelSelected::selected: bool`), the state the
/// left-panel page drives from its `current` choice.
/// Falsified 2026-10-07 by removing the `("ui_shell_left_panel_icon_subitem",
/// &[("selected", PropType::Bool)])` row from `OPENABLE`, which is what that
/// table shipped before the row joined:
///
/// ```text
/// thread 'a_bound_left_panel_icon_subitem_selection_digests_the_literal_in_both_states_and_grafts_alone' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "ui_shell_left_panel_icon_subitem", reason: "parameter `selected` is not openable: a component expands from literal parameters, and `ui_shell_left_panel_icon_subitem` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_left_panel_icon_subitem_selection_digests_the_literal_in_both_states_and_grafts_alone() {
    flag_reexpansion(
        "ui_shell_left_panel_icon_subitem",
        "selected",
        &json!({ "label": "Fibers" }),
    );
}

/// `ui_shell_switcher.open` is the right panel's whole open/shut state:
/// `ui_shell_switcher` builds `right_panel`, which mounts the content
/// either way and only pins the width (`pin_inline(if open {
/// RIGHT_PANEL_WIDTH } else { 0.0 })` — "Shut is a zero-width panel that
/// is still mounted, never an unmounted one"). A flag row, not a
/// disclosure: the right-panel page's switcher and its items are always
/// declared and only the picture moves.
/// Falsified 2026-10-07 by removing `("ui_shell_switcher", &[("open",
/// PropType::Bool)])` from `OPENABLE`, which is what that table shipped
/// before the row joined:
///
/// ```text
/// thread 'a_bound_switcher_open_digests_the_literal_in_both_states_and_grafts_alone' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "ui_shell_switcher", reason: "parameter `open` is not openable: a component expands from literal parameters, and `ui_shell_switcher` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_switcher_open_digests_the_literal_in_both_states_and_grafts_alone() {
    flag_reexpansion(
        "ui_shell_switcher",
        "open",
        &json!({ "label": "App switcher" }),
    );
}

/// `ui_shell_switcher_item.selected` marks the current app's row
/// (`KeyLabelSelected::selected: bool`): it recolours the caption and
/// declares `semantics.selected`, and the row's body is a fixed caption —
/// no membership rides it.
/// Falsified 2026-10-07 by removing the `("ui_shell_switcher_item",
/// &[("selected", PropType::Bool)])` row from `OPENABLE`, which is what
/// that table shipped before the row joined:
///
/// ```text
/// thread 'a_bound_switcher_item_selection_digests_the_literal_in_both_states_and_grafts_alone' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "ui_shell_switcher_item", reason: "parameter `selected` is not openable: a component expands from literal parameters, and `ui_shell_switcher_item` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_switcher_item_selection_digests_the_literal_in_both_states_and_grafts_alone() {
    flag_reexpansion(
        "ui_shell_switcher_item",
        "selected",
        &json!({ "label": "Petra" }),
    );
}

/// `ui_shell_header_action.selected` is the action's persistent "its panel
/// is open" condition (`KeyLabelSelected::selected: bool`, `active` in the
/// constructor: glyph tone plus `$layer` fill) — the right-panel page
/// binds it on the switcher trigger.
/// Falsified 2026-10-07 by removing the `("ui_shell_header_action",
/// &[("selected", PropType::Bool)])` row from `OPENABLE`, which is what
/// that table shipped before the row joined:
///
/// ```text
/// thread 'a_bound_header_action_selection_digests_the_literal_in_both_states_and_grafts_alone' panicked at petra/tests/bound_components/support.rs:110:50:
/// the page expands: Param(ParamError { component: "ui_shell_header_action", reason: "parameter `selected` is not openable: a component expands from literal parameters, and `ui_shell_header_action` opens only [] to a slot" })
/// ```
///
/// Restored byte-identical afterwards and re-run green.
#[test]
fn a_bound_header_action_selection_digests_the_literal_in_both_states_and_grafts_alone() {
    flag_reexpansion(
        "ui_shell_header_action",
        "selected",
        &json!({ "label": "App switcher" }),
    );
}
