//! Gate C1-5, the real acceptance: a tree built from every shipped
//! component passes `semantic::audit` with zero findings.
//!
//! This is not a smoke test that the tree petrifies — every layout test in
//! the workspace already proves that much for a plain `ViewNode` tree. It
//! is the specific claim FR-058 makes: that a surface an author composes
//! entirely out of this module's thirteen names cannot end up with an
//! unlabelled interactive node or a colour-only status, because the audit
//! that would catch either one finds nothing to report.

use crate::frame::{TransitionActivity, Viewport, petrify};
use crate::geom::{Axis, Size};
use crate::semantic::{audit, project};
use crate::testing::{Harness, validated_with};
use crate::token::{StatusShape, StatusToken, ThemeMode, TokenName, standard_vocabulary};
use crate::tree::{Interaction, NodeKind, Props, Registry, ViewNode};

use super::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, TEXT_ON_ACCENT};
use super::{
    MAX_LAYER_DEPTH, accordion, accordion_item, ai_label, ai_label_inline, breadcrumb,
    breadcrumb_item, button, checkbox, clickable_tile, code_snippet, code_snippet_inline,
    code_snippet_multi, contained_list, contained_list_disclosed, contained_tab, contained_tab_bar,
    content_switcher, content_switcher_item, data_table, data_table_row, data_table_row_expandable,
    data_table_sort_header, date_picker, date_picker_open, disabled, dismissible_tag, dropdown,
    dropdown_open, dropdown_option, expandable_tile, field, field_fluid, field_labeled, field_lg,
    field_readonly, field_sm, file_uploader, file_uploader_item, form, heading, inline_loading,
    inline_loading_finished, layer_tokens, link, list_item, list_item_with, list_row, loading,
    loading_sm, menu, menu_button, menu_item, modal, notification_actionable, notification_inline,
    notification_toast, number_input, on_layer, ordered_list, pagination, popover, primary_button,
    progress, progress_indicator, progress_sm, progress_step, radio, search, section, select,
    select_lg, select_sm, selectable_tag, selectable_tile, slider, slider_readonly, status,
    structured_list, structured_list_row, tab, tab_bar, tag, tag_lg, tag_sm, text, tile, toggle,
    toggle_sm, toggletip, tooltip, tree_item, tree_view, ui_shell_header, ui_shell_header_action,
    ui_shell_header_menu_trigger, ui_shell_header_nav_item, ui_shell_left_panel,
    ui_shell_left_panel_divider, ui_shell_left_panel_item, ui_shell_left_panel_rail,
    ui_shell_left_panel_subitem, ui_shell_right_panel, ui_shell_right_panel_divider,
    ui_shell_switcher, ui_shell_switcher_item, unordered_list, vertical_tab, vertical_tab_bar,
};

const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

/// One tree exercising all thirteen shipped components, each at least once,
/// nested under a plain `Stack` root (a primitive, per gate C1-10 — the
/// library composes from the primitives, it does not replace the root).
fn full_gallery() -> ViewNode {
    let ok = StatusToken::new(
        TokenName::new("status.ok").unwrap(),
        StatusShape::Circle,
        "OK",
    )
    .unwrap();
    let degraded = StatusToken::new(
        TokenName::new("status.degraded").unwrap(),
        StatusShape::Triangle,
        "Degraded",
    )
    .unwrap();

    let controls = section(
        "controls",
        "Controls",
        vec![
            checkbox("check", "Checked", true),
            checkbox("check-off", "Unchecked", false),
            disabled(checkbox("check-disabled", "Unavailable", false)),
            radio("radio", "Chosen", false),
            toggle("toggle", "On", true),
            primary_button("primary", "Save"),
            button("secondary", "Cancel"),
            disabled(button("secondary-disabled", "Unavailable")),
            field("name", "Fiber name"),
        ],
    );

    // Group 1's own seven (accordion, AI label, breadcrumb, code snippet,
    // contained list; button and checkbox already sit in `controls` above).
    // AI label appears closed and open: the open form's explainability
    // panel names its trigger by bare sibling key (`Anchor::Sibling`), so
    // it mounts at this depth like everything else.
    let carbon = section(
        "carbon",
        "Carbon (group 1)",
        vec![
            breadcrumb(
                "trail",
                vec![
                    breadcrumb_item("home", "Home"),
                    breadcrumb_item("docs", "Docs"),
                    breadcrumb_item("here", "Here"),
                ],
            ),
            accordion(
                "acc",
                vec![
                    accordion_item("acc-open", "Open section", true, "The panel body."),
                    accordion_item("acc-shut", "Closed section", false, "hidden"),
                ],
            ),
            ai_label("ai-default", "Confidence score", false, "80% confident"),
            ai_label("ai-open", "Confidence score", true, "80% confident"),
            ai_label_inline("ai-inline", "Ask AI", false, "Trained on ticket history."),
            code_snippet("snippet-single", "fn main() {}"),
            code_snippet_inline("snippet-inline", "ViewNode"),
            code_snippet_multi("snippet-multi", "line 1\nline 2\nline 3"),
            contained_list(
                "cl-onpage",
                "Related",
                vec![text("cl-r0", "Alpha"), text("cl-r1", "Bravo")],
            ),
            contained_list_disclosed("cl-disclosed", "Menu", vec![text("cl-r2", "Charlie")]),
        ],
    );

    // Group 2's own seven (content switcher, data table, date picker,
    // dropdown, file uploader, form, inline loading). Date picker and
    // dropdown appear closed and open; the open forms' popovers name the
    // `field` beside them by bare sibling key (`Anchor::Sibling`).
    // `dropdown_option` is not itself anchored, so it is also included
    // standalone the way a caller would place it inside the menu.
    let carbon2 = section(
        "carbon2",
        "Carbon (group 2)",
        vec![
            content_switcher(
                "cs",
                vec![
                    content_switcher_item("cs-list", "List", true),
                    content_switcher_item("cs-grid", "Grid", false),
                    disabled(content_switcher_item("cs-detail", "Detail", false)),
                ],
            ),
            data_table(
                "dt-jobs",
                vec![
                    data_table_sort_header("dt-h0", "Name", true),
                    text("dt-h1", "Status"),
                ],
                vec![
                    data_table_row(
                        "dt-r0",
                        vec![text("dt-n0", "alpha"), text("dt-s0", "ready")],
                        false,
                    ),
                    data_table_row(
                        "dt-r1",
                        vec![text("dt-n1", "bravo"), text("dt-s1", "ready")],
                        true,
                    ),
                    data_table_row_expandable(
                        "dt-r2",
                        vec![text("dt-n2", "charlie")],
                        false,
                        true,
                        "more detail",
                    ),
                    disabled(data_table_row("dt-r3", vec![text("dt-n3", "delta")], false)),
                ],
            ),
            date_picker("dp-due", "Due date", "2026-08-30"),
            dropdown("dd-theme", "Theme", "Dark"),
            dropdown_option("dd-opt-a", "Dark", true),
            dropdown_option("dd-opt-b", "Light", false),
            dropdown_open(
                "dd-open",
                "Theme",
                "Dark",
                vec![
                    dropdown_option("dd-open-dark", "Dark", true),
                    dropdown_option("dd-open-light", "Light", false),
                ],
            ),
            date_picker_open("dp-open", "Due date", "2026-08-30"),
            file_uploader("fu-up", "Upload files"),
            file_uploader_item("fu-f0", "notes.txt", true),
            file_uploader_item("fu-f1", "report.pdf", false),
            form("fm-signup", "Account", vec![field("fm-name", "Name")]),
            inline_loading("il-save", "Saving", 0.0),
            inline_loading_finished("il-saved", "Saved"),
        ],
    );

    // Group 3's own seven (link, list, loading, menu, menu buttons, modal,
    // notification). `menu` IS the anchored surface and names a sibling
    // keyed `trigger` (`Anchor::Sibling`), so it sits beside a button
    // carrying that key; `menu_button` appears closed and open. `menu_item`
    // itself is not anchored, so a standalone item (and a disabled one) is
    // included here the way `dropdown_option` is above. Modal and
    // Notification are `Anchor::Viewport` surfaces, placed with no
    // reference to any sibling.
    let carbon3 = section(
        "carbon3",
        "Carbon (group 3)",
        vec![
            link("lnk-docs", "Open docs"),
            disabled(link("lnk-archived", "Archived project")),
            unordered_list(
                "ul-topics",
                vec![
                    list_item("ul-alpha", "Alpha"),
                    list_item("ul-bravo", "Bravo"),
                ],
            ),
            ordered_list(
                "ol-steps",
                vec![
                    list_item("ol-first", "First"),
                    list_item("ol-second", "Second"),
                ],
            ),
            list_item_with(
                "li-nested",
                "Parent",
                Some(unordered_list(
                    "li-nested-inner",
                    vec![list_item("li-nested-child", "Child")],
                )),
            ),
            loading("ld-large", "Loading fibers", 0.0),
            loading_sm("ld-small", "Saving", 0.0),
            menu_item("mi-rename", "Rename"),
            disabled(menu_item("mi-delete", "Delete")),
            menu_button(
                "mb-more",
                "More actions",
                false,
                vec![menu_item("mb-rename", "Rename")],
            ),
            menu_button(
                "mb-open",
                "More actions",
                true,
                vec![menu_item("mb-open-rename", "Rename")],
            ),
            button("trigger", "Actions"),
            menu(
                "mn-actions",
                "Actions",
                vec![
                    menu_item("mn-rename", "Rename"),
                    menu_item("mn-delete", "Delete"),
                ],
            ),
            modal(
                "md-retire",
                "Retire fiber",
                "Its children are retired with it.",
                "Retire",
            ),
            notification_toast("nt-restart", "Supervisor restarted", "Worker 3 came back."),
            notification_inline("nt-warn", "Disk filling", "Trace volume is at 80%."),
            notification_actionable(
                "nt-action",
                "Update available",
                "A new build is ready.",
                "Reload",
            ),
        ],
    );

    // Group 4's own seven (number input, pagination, popover, progress
    // bar, progress indicator, radio button, search). Popover has no
    // closed form — every constructor it exports IS the anchored surface —
    // so it sits beside the button its `Anchor::Sibling` names.
    // Number input's invalid form binds an accent `border` (`ACCENT_PRIMARY`,
    // same pattern as `field_invalid`, which for the identical reason is
    // also absent from this tree) and so is audited only inside
    // `number_input.rs`'s own module, to keep `containers_take_a_tone_and_
    // controls_take_an_edge` a single-tone invariant.
    let carbon4 = section(
        "carbon4",
        "Carbon (group 4)",
        vec![
            number_input("ni-count", "Replicas", "3"),
            disabled(number_input("ni-disabled", "Replicas", "3")),
            pagination("pg-first", 1, 4),
            button("pop-anchor", "Anchor"),
            popover("pop", "Filter help", "pop-anchor", "Narrow the list."),
            progress_sm("pb-empty", "Queued", 0.0),
            progress("pb-mid", "Rebuilding", 0.5),
            progress_sm("pb-full", "Done", 1.0),
            progress_indicator(
                "pi-steps",
                vec![
                    progress_step("pi-choose", "Choose", true, false),
                    progress_step("pi-configure", "Configure", false, true),
                    progress_step("pi-review", "Review", false, false),
                    disabled(progress_step("pi-confirm", "Confirm", false, false)),
                ],
            ),
            radio("radio-checked", "Chosen", true),
            radio("radio-unchecked", "Not chosen", false),
            disabled(radio("radio-disabled", "Unavailable", false)),
            search("srch-filter", "Filter fibers"),
        ],
    );

    // Group 5's own seven (Select, Slider, Structured list, Tabs, Tag,
    // Text input, Tile). Select ships no invalid-state constructor at all
    // (scope gap, recorded in the plan) so only the closed and disabled
    // forms appear here. The `tabs` section below already carries Line
    // selected/unselected, so this section covers Contained and Vertical
    // instead. `field_invalid` binds an accent `border` (same pattern as
    // Number input's invalid form, carbon4's own comment above) and so is
    // audited only inside `field.rs`'s own module, to keep
    // `containers_take_a_tone_and_controls_take_an_edge` a single-tone
    // invariant.
    let carbon5 = section(
        "carbon5",
        "Carbon (group 5)",
        vec![
            select("sel-theme", "Theme", "Dark"),
            select_sm("sel-sm", "Theme", "Dark"),
            select_lg("sel-lg", "Theme", "Dark"),
            disabled(select("sel-disabled", "Theme", "Dark")),
            slider("sl-min", "Volume", 0.0),
            slider("sl-mid", "Volume", 0.5),
            slider("sl-max", "Volume", 1.0),
            slider_readonly("sl-readonly", "Volume", 0.5),
            structured_list(
                "stl-plans",
                vec![text("stl-h0", "Plan"), text("stl-h1", "Price")],
                vec![
                    structured_list_row(
                        "stl-r0",
                        vec![text("stl-p0", "Basic"), text("stl-c0", "$12")],
                        false,
                    ),
                    structured_list_row(
                        "stl-r1",
                        vec![text("stl-p1", "Pro"), text("stl-c1", "$24")],
                        true,
                    ),
                ],
            ),
            contained_tab_bar(
                "tabs-contained",
                vec![
                    contained_tab("tc-selected", "Fibers", true),
                    contained_tab("tc-unselected", "Trace", false),
                    disabled(contained_tab("tc-disabled", "Archive", false)),
                ],
            ),
            vertical_tab_bar(
                "tabs-vertical",
                vec![
                    vertical_tab("tv-selected", "Fibers", true),
                    vertical_tab("tv-unselected", "Trace", false),
                ],
            ),
            tag("tag-ro", "prod"),
            tag_sm("tag-sm", "prod"),
            tag_lg("tag-lg", "prod"),
            dismissible_tag("tag-dismiss", "prod"),
            selectable_tag("tag-select-on", "prod", true),
            selectable_tag("tag-select-off", "prod", false),
            field("txt-default", "Fiber name"),
            disabled(field("txt-disabled", "Fiber name")),
            field_readonly("txt-readonly", "Fiber name"),
            field_labeled("txt-labeled", "Fiber name"),
            field_fluid("txt-fluid", "Fiber name"),
            field_sm("txt-sm", "Fiber name"),
            field_lg("txt-lg", "Fiber name"),
            clickable_tile("tile-click", "Open project", "Project Alpha"),
            selectable_tile("tile-select-on", "Plan A", true),
            selectable_tile("tile-select-off", "Plan A", false),
            expandable_tile("tile-expand-open", "Details", true, "the rest"),
            expandable_tile("tile-expand-shut", "Details", false, "the rest"),
        ],
    );

    // Group 6's own seven (Toggle, Toggletip, Tooltip, Tree view, UI shell
    // header, UI shell left panel, UI shell right panel). Toggle already has
    // one instance in `controls` above; this section adds the states that
    // instance does not cover (off, disabled, small). Toggletip appears
    // closed and open. Tooltip and both UI shell right panels IS the
    // anchored surface with no closed form, so each sits beside the control
    // its `Anchor::Sibling` names: tooltip's is keyed `trigger` by
    // construction, the panels' are whatever key the caller passes.
    let carbon6 = section(
        "carbon6",
        "Carbon (group 6)",
        vec![
            toggle("tog-on", "Autosave", true),
            toggle("tog-off", "Autosave", false),
            disabled(toggle("tog-disabled", "Autosave", false)),
            toggle_sm("tog-sm", "Compact mode", true),
            toggletip("help", "About filters", false, "Narrow the list."),
            toggletip("help-open", "About filters", true, "Narrow the list."),
            button("trigger", "Copy"),
            tooltip("tip", "Copied", "Copied to clipboard"),
            ui_shell_header_action("panel-trigger", "Notifications", true),
            ui_shell_right_panel(
                "shell-right",
                "Notifications",
                "panel-trigger",
                vec![text("shell-note", "No new notifications.")],
            ),
            ui_shell_header_action("apps", "App switcher", true),
            ui_shell_switcher(
                "shell-switcher",
                "App switcher",
                "apps",
                vec![
                    ui_shell_switcher_item("sw-a", "Petra"),
                    ui_shell_right_panel_divider("sw-d1"),
                    ui_shell_switcher_item("sw-b", "Inspector"),
                ],
            ),
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
            ),
            ui_shell_header(
                "shell-header",
                "GOrgOn",
                Some(ui_shell_header_menu_trigger("trigger", false)),
                vec![
                    ui_shell_header_nav_item("overview", "Overview", true),
                    ui_shell_header_nav_item("fibers", "Fibers", false),
                ],
                vec![
                    ui_shell_header_action("notify", "Notifications", true),
                    ui_shell_header_action("search-action", "Search", false),
                ],
            ),
            ui_shell_left_panel(
                "shell-left",
                vec![
                    ui_shell_left_panel_item("home", "Home", false, true, vec![]),
                    ui_shell_left_panel_item(
                        "kernel",
                        "Kernel",
                        true,
                        false,
                        vec![
                            ui_shell_left_panel_subitem("fibers-item", "Fibers", true),
                            ui_shell_left_panel_subitem("trace-item", "Trace", false),
                        ],
                    ),
                    ui_shell_left_panel_divider("rule"),
                    ui_shell_left_panel_item("settings", "Settings", false, false, vec![]),
                ],
            ),
            ui_shell_left_panel_rail("shell-rail", vec![]),
        ],
    );

    let tabs = section(
        "tabs",
        "Tabs",
        vec![tab_bar(
            "strip",
            vec![
                tab("t-fibers", "Fibers", true),
                tab("t-trace", "Trace", false),
            ],
        )],
    );

    let readouts = section(
        "readouts",
        "Readouts",
        vec![
            progress("rebuild", "Rebuild", 0.62),
            status("s-ok", &ok),
            status("s-degraded", &degraded),
        ],
    );

    let list = section(
        "list",
        "List",
        vec![
            list_row("row-0", "row 0", true),
            list_row("row-1", "row 1", false),
            unordered_list(
                "ul",
                vec![list_item("ul-0", "Inbox"), list_item("ul-1", "Archive")],
            ),
            tile("tile", "A static tile."),
        ],
    );

    let mut root = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
        axis: Some(Axis::Vertical),
        ..Props::default()
    });
    root = root
        .child(heading("title", "Component gallery"))
        .child(text("intro", "every shipped component, once"))
        .child(controls)
        .child(carbon)
        .child(carbon2)
        .child(carbon3)
        .child(carbon4)
        .child(carbon5)
        .child(carbon6)
        .child(tabs)
        .child(readouts)
        .child(list);
    root
}

#[test]
fn every_component_in_one_tree_passes_the_audit_with_zero_findings() {
    let tree = full_gallery();
    // The component library binds real design-token names (`spacing-04`,
    // `text.primary`, ...), so — unlike most layout fixtures, which name no
    // token at all — this tree needs a `Registry` that actually declares the
    // shipped vocabulary; `crate::testing::validated`'s empty `Registry::new()`
    // would refuse every one of them as unknown (`Violation::UnknownTokenRef`).
    let registry = accepting_registry();
    let mut harness = Harness::new();
    let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
    harness.scale = viewport.scale;
    let frame = petrify(
        1,
        validated_with(&tree, &registry),
        &mut harness.ctx(),
        viewport,
        TransitionActivity::default(),
    );
    let projected = project(&frame).expect("a petrified frame with a root has a projection");
    let violations = audit(&projected, &frame);
    assert!(
        violations.is_empty(),
        "a tree built only from the component library must pass the audit clean, found: {violations:#?}"
    );
}

/// [`button`], [`checkbox`], [`radio`], [`toggle`], [`tab`], [`field`], and
/// [`list_row`] all declare at least one interaction; every one of them
/// must carry a role and a non-empty label; this is FR-058 measured through
/// the same audit rule the sabotage run (gate C1-12) targets.
/// A node that accepts typing must also accept a press.
///
/// The rule a whole round of "the text field does not work" came down to.
/// `input::hit_test` aims a pointer press at `Interaction::Click`, and
/// `Host::seat_pointer_focus` runs only on a `Route::Pointer`. So an input
/// that declares `TextEdit` without `Click` cannot be focused by a hand: the
/// press hits nothing, focus stays where it was, and every later keystroke
/// routes somewhere else. It looks perfectly correct in any test that moves
/// focus itself.
///
/// Swept over [`full_gallery`], so a fourth text-entry component added later
/// is covered without touching this test.
#[test]
fn anything_that_accepts_typing_can_also_be_clicked_into() {
    fn walk(node: &ViewNode, bad: &mut Vec<String>) {
        if node.interactions.contains(&Interaction::TextEdit)
            && !node.interactions.contains(&Interaction::Click)
        {
            bad.push(node.key.as_str().to_owned());
        }
        for child in &node.children {
            walk(child, bad);
        }
    }
    let mut bad = Vec::new();
    walk(&full_gallery(), &mut bad);
    assert!(
        bad.is_empty(),
        "these accept typing but no pointer press can reach them, so clicking \
         one does not put the caret in it: {bad:?}"
    );
}

#[test]
fn every_interactive_component_declares_a_role_and_a_label() {
    let nodes = [
        button("b", "Save"),
        checkbox("c", "Checked", true),
        radio("r", "Chosen", false),
        toggle("t", "On", true),
        toggle_sm("ts", "On", true),
        tab("tb", "Fibers", true),
        field("f", "Fiber name"),
        list_row("l", "row", false),
    ];
    for node in nodes {
        assert!(
            node.is_interactive(),
            "{:?} declares no interaction",
            node.key
        );
        assert!(
            node.semantics.role.is_some(),
            "{:?} is interactive but carries no role",
            node.key
        );
        assert!(
            node.semantics
                .label
                .as_ref()
                .is_some_and(|label| !label.trim().is_empty()),
            "{:?} is interactive but carries no non-empty label",
            node.key
        );
    }
}

/// [`status`] cannot be built without a shape and a non-empty text channel,
/// because its only parameter is a [`StatusToken`] and `StatusToken::new`
/// already refuses one with empty text (gate C1-6). This test measures the
/// component's own output rather than `StatusToken` a second time: the
/// projected node's role and label are what the audit actually reads.
#[test]
fn status_always_carries_role_and_label_from_its_status_token() {
    let token = StatusToken::new(
        TokenName::new("status.down").unwrap(),
        StatusShape::Square,
        "Down",
    )
    .unwrap();
    let node = status("s", &token);
    assert_eq!(node.semantics.role, Some(crate::tree::Role::Status));
    assert_eq!(node.semantics.label.as_deref(), Some("Down"));
}

/// A control's `selected` state is declared in `Semantics`, never carried by
/// its fill colour alone (gate C1-6's sibling claim, for the binary
/// controls rather than `status`).
#[test]
fn selection_is_declared_state_not_only_a_fill_colour() {
    assert!(checkbox("c", "Checked", true).semantics.selected);
    assert!(!checkbox("c", "Unchecked", false).semantics.selected);
    assert!(toggle("t", "On", true).semantics.selected);
    assert!(toggle_sm("ts", "On", true).semantics.selected);
    assert!(!toggle_sm("ts", "Off", false).semantics.selected);
    assert!(tab("tb", "Fibers", true).semantics.selected);
    assert!(list_row("l", "row", true).semantics.selected);
}

/// On-state marks fill with the accent, not ink. Off stays empty so the
/// outline is the control. `Semantics.selected` still carries the state
/// (FR-015).
#[test]
fn selected_marks_fill_with_accent() {
    use super::tokens::ACCENT_PRIMARY;

    let box_bg = |node: &ViewNode, child: &str| -> Option<String> {
        named(node, child)
            .props
            .tokens
            .get("background")
            .map(|t| t.as_str().to_owned())
    };
    assert_eq!(
        box_bg(&checkbox("c", "Checked", true), "box").as_deref(),
        Some(ACCENT_PRIMARY)
    );
    assert_eq!(box_bg(&checkbox("c", "Unchecked", false), "box"), None);
    assert_eq!(
        box_bg(&radio("r", "Chosen", true), "box").as_deref(),
        Some(ACCENT_PRIMARY)
    );
    assert_eq!(
        box_bg(&toggle("t", "On", true), "track").as_deref(),
        Some(ACCENT_PRIMARY)
    );
    assert_eq!(
        box_bg(&toggle("t", "Off", false), "track").as_deref(),
        Some(super::tokens::SURFACE_RAISED)
    );
    assert_eq!(
        box_bg(&toggle_sm("ts", "On", true), "track").as_deref(),
        Some(ACCENT_PRIMARY)
    );
    assert_eq!(
        box_bg(&toggle_sm("ts", "Off", false), "track").as_deref(),
        Some(super::tokens::SURFACE_RAISED)
    );
}

/// The knob's identity is the same on and off, with pads on both sides, so
/// a flip is a position trajectory rather than a child reorder.
#[test]
fn a_toggle_keeps_both_pads_so_the_knob_can_slide() {
    let keys = |node: ViewNode| -> Vec<String> {
        named(&node, "track")
            .children
            .iter()
            .map(|c| c.key.as_str().to_owned())
            .collect()
    };
    assert_eq!(
        keys(toggle("t", "On", true)),
        ["pad-start", "knob", "pad-end"]
    );
    assert_eq!(
        keys(toggle("t", "On", false)),
        ["pad-start", "knob", "pad-end"]
    );
    assert_eq!(
        keys(toggle_sm("ts", "On", true)),
        ["pad-start", "knob", "pad-end"]
    );
    assert_eq!(
        keys(toggle_sm("ts", "On", false)),
        ["pad-start", "knob", "pad-end"]
    );
    let on = toggle("t", "On", true);
    let knob = named(&on, "knob");
    assert_eq!(
        knob.transition.as_ref().map(|t| t.name()),
        Some(crate::anim::TOGGLE_KNOB)
    );
    let sm_on = toggle_sm("ts", "On", true);
    let sm_knob = named(&sm_on, "knob");
    assert_eq!(
        sm_knob.transition.as_ref().map(|t| t.name()),
        Some(crate::anim::TOGGLE_KNOB)
    );
}

/// Off sits the knob at the start of the track; on sits it at the end.
/// Carbon default travel is 24 (`translateX(24px)`).
#[test]
fn an_on_toggle_places_the_knob_to_the_right_of_an_off_toggle() {
    let x = |on: bool| {
        petrify_lone(toggle("t", "On", on))
            .placements
            .iter()
            .find(|p| p.id.ends_with("/track/knob"))
            .expect("the knob is placed")
            .rect
            .x
    };
    let on_x = x(true);
    let off_x = x(false);
    let travel = on_x - off_x;
    assert!(
        on_x > off_x,
        "on-knob x {on_x} must sit to the right of off-knob x {off_x}"
    );
    assert!(
        (travel - 24.0).abs() < 1.0,
        "default travel {travel} must be within 1 of 24"
    );
}

/// Carbon small travel is 16 (`translateX(16px)`).
#[test]
fn a_small_on_toggle_travels_sixteen() {
    let x = |on: bool| {
        petrify_lone(toggle_sm("t", "On", on))
            .placements
            .iter()
            .find(|p| p.id.ends_with("/track/knob"))
            .expect("the small knob is placed")
            .rect
            .x
    };
    let travel = x(true) - x(false);
    assert!(
        (travel - 16.0).abs() < 1.0,
        "small travel {travel} must be within 1 of 16"
    );
}

/// Carbon track sizes, read off placed rects: default 48×24, small 32×16.
#[test]
fn toggle_tracks_match_carbon_geometry() {
    let size_of = |node: ViewNode| {
        petrify_lone(node)
            .placements
            .iter()
            .find(|p| p.id.ends_with("/track") && !p.id.contains("/track/"))
            .expect("the track is placed")
            .rect
    };
    let default = size_of(toggle("t", "On", false));
    assert_eq!(default.w, 48.0, "default track width");
    assert_eq!(default.h, 24.0, "default track height");
    let small = size_of(toggle_sm("t", "On", false));
    assert_eq!(small.w, 32.0, "small track width");
    assert_eq!(small.h, 16.0, "small track height");
    let default_on = size_of(toggle("t", "On", true));
    assert_eq!(default_on.w, 48.0);
    assert_eq!(default_on.h, 24.0);
}

#[test]
fn marker_and_label_share_a_midline() {
    use crate::geom::Align;
    assert_eq!(
        checkbox("c", "Checked", false).props.align,
        Some(Align::Center)
    );
    assert_eq!(radio("r", "Chosen", false).props.align, Some(Align::Center));
    assert_eq!(
        named(&toggle("t", "On", false), "appearance").props.align,
        Some(Align::Center)
    );
    assert_eq!(
        named(&toggle("t", "On", false), "track").props.align,
        Some(Align::Center)
    );
    let down = StatusToken::new(
        TokenName::new("status.down").unwrap(),
        StatusShape::Square,
        "Down",
    )
    .unwrap();
    assert_eq!(status("s", &down).props.align, Some(Align::Center));

    // Props.align is the declaration. Checkbox/radio/status are 12-vs-20
    // (10-vs-20 for status); Start would place the marker ~4 units above
    // the words. A toggle's label sits *above* the switch (Carbon anatomy);
    // the midline that must match is track vs state text, 24-vs-20.
    // Petrify and compare the placed midlines so a layout that ignores
    // align cannot stay green.
    let mid = |suffix: &str, frame: &crate::frame::PetrifiedFrame| {
        let rect = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("{suffix} is missing from the petrified frame"))
            .rect;
        rect.y + rect.h * 0.5
    };
    let cases: &[(&str, ViewNode, &str, &str)] = &[
        (
            "checkbox",
            checkbox("c", "Checked", false),
            "/root/c/box",
            "/root/c/label",
        ),
        (
            "radio",
            radio("r", "Chosen", false),
            "/root/r/box",
            "/root/r/label",
        ),
        (
            "toggle",
            toggle("t", "On", false),
            "/root/t/appearance/track",
            "/root/t/appearance/state",
        ),
        ("status", status("s", &down), "/root/s/dot", "/root/s/label"),
    ];
    for (name, node, marker, label) in cases {
        let frame = petrify_lone(node.clone());
        let marker_mid = mid(marker, &frame);
        let label_mid = mid(label, &frame);
        assert!(
            (marker_mid - label_mid).abs() < 0.5,
            "{name}: marker midline {marker_mid} vs label midline {label_mid}"
        );
    }
}

/// `size-md` is 40 in the ramp; a field that spelled 40.0 at the call site
/// would drift the first time the ramp moved. The constraint is the
/// declaration; the placed rect is the claim.
#[test]
fn a_field_is_as_tall_as_size_md() {
    assert_eq!(super::tokens::SIZE_MD, 40.0);
    let node = field("name", "Fiber name");
    assert_eq!(node.constraints.vertical.min, Some(super::tokens::SIZE_MD));
    let theme = crate::token::light();
    let value = theme
        .value(&TokenName::new("size-md").unwrap())
        .expect("size-md is in the vocabulary");
    match value {
        crate::token::TokenValue::Spacing(units) => {
            assert_eq!(*units, super::tokens::SIZE_MD);
        }
        other => panic!("size-md should be a spacing value, got {other:?}"),
    }
    let frame = petrify_lone(node);
    let placed = frame
        .placements
        .iter()
        .find(|p| p.id.ends_with("/root/name"))
        .expect("the field is missing from the petrified frame");
    assert_eq!(placed.rect.h, super::tokens::SIZE_MD);
}

/// **No shipped component spells an icon as its name.** Until 2026-09-04
/// `IconMark` had one variant, so every other Carbon glyph in this library
/// was drawn as its word in body text — `closed`, `Dismiss`, `Increment` —
/// which is most of what made the catalog read as unfinished. The word now
/// lives in `Semantics.label` (or in `Semantics.expanded`, for a state), and
/// the eye gets the glyph.
///
/// Every constructor that used to spell one is built here in every state
/// that chose a different word, and no text node anywhere under any of them
/// may be one of the literals. `select`, `tile` and `tree_view` are not in
/// this fixture: their carets are `component::caret` and `select`'s word is
/// another wave's row (see `icon.rs`'s module doc).
///
/// Falsify by putting one word back — `text("chevron", "closed")` in
/// `dropdown::closed_field` — and this names the path it reappeared at.
#[test]
fn no_shipped_component_spells_an_icon_as_its_name() {
    const ICON_NAMES: [&str; 15] = [
        "calendar",
        "closed",
        "open",
        "Copy",
        "Decrement",
        "Increment",
        "Search",
        "Dismiss",
        "collapsed",
        "expanded",
        "Open",
        "Close",
        "Notifications",
        "App switcher",
        "Menu",
    ];
    let fixture = section(
        "icons",
        "Every part that used to be a word",
        vec![
            accordion(
                "acc",
                vec![
                    accordion_item("open", "Open section", true, "body"),
                    accordion_item("shut", "Closed section", false, "body"),
                ],
            ),
            data_table(
                "dt",
                vec![data_table_sort_header("h", "Name", false)],
                vec![
                    data_table_row_expandable("r-open", vec![text("c", "a")], false, true, "more"),
                    data_table_row_expandable("r-shut", vec![text("c", "a")], false, false, "more"),
                ],
            ),
            date_picker("dp", "Due date", "2026-09-04"),
            date_picker_open("dp-open", "Due date", "2026-09-04"),
            dropdown("dd", "Theme", "Dark"),
            dropdown_open(
                "dd-open",
                "Theme",
                "Dark",
                vec![dropdown_option("dark", "Dark", true)],
            ),
            menu_button("mb", "More", false, vec![menu_item("rename", "Rename")]),
            menu_button("mb-open", "More", true, vec![menu_item("rename", "Rename")]),
            code_snippet("snip", "let x = 1;"),
            code_snippet_multi("snip-multi", "a\nb"),
            number_input("n", "Count", "12"),
            search("q", "Filter fibers"),
            dismissible_tag("tag-x", "Filter"),
            selectable_tag("tag-on", "Selected", true),
            ui_shell_header(
                "header",
                "GOrgOn",
                Some(ui_shell_header_menu_trigger("trigger-shut", false)),
                vec![ui_shell_header_nav_item("overview", "Overview", true)],
                vec![
                    ui_shell_header_action("notify", "Notifications", false),
                    ui_shell_header_action("search", "Search", true),
                    ui_shell_header_action("apps", "App switcher", false),
                ],
            ),
            ui_shell_header_menu_trigger("trigger-open", true),
            ui_shell_left_panel(
                "nav",
                vec![
                    ui_shell_left_panel_item(
                        "kernel-open",
                        "Kernel",
                        true,
                        false,
                        vec![ui_shell_left_panel_subitem("fibers", "Fibers", false)],
                    ),
                    ui_shell_left_panel_item(
                        "kernel-shut",
                        "Kernel",
                        false,
                        false,
                        vec![ui_shell_left_panel_subitem("fibers", "Fibers", false)],
                    ),
                ],
            ),
        ],
    );

    let mut spelled: Vec<String> = Vec::new();
    walk(&fixture, "", &mut |path, props| {
        if let Some(word) = props.text.as_deref()
            && ICON_NAMES.contains(&word)
        {
            spelled.push(format!("{path}: {word:?}"));
        }
    });
    assert!(
        spelled.is_empty(),
        "an icon is spelled as its name in body text; the word belongs in \
         `Semantics.label` and the eye gets an `IconMark`:\n{}",
        spelled.join("\n")
    );
}

/// A component with more than one visual part composes it from a primitive
/// container (`NodeKind::Stack` or `NodeKind::Grid`), never by inventing a
/// new node kind — gate C1-10, checked structurally rather than by reading
/// the source.
#[test]
fn multi_part_components_are_built_from_primitive_container_kinds() {
    assert_eq!(button("b", "Save").kind, NodeKind::Stack);
    assert_eq!(section("s", "T", vec![]).kind, NodeKind::Stack);
    assert_eq!(progress("p", "Rebuild", 0.5).kind, NodeKind::Grid);
    assert_eq!(tab_bar("tb", vec![]).kind, NodeKind::Stack);
}

/// Walk `node` and everything under it, handing `visit` each node's key path
/// and its bound tokens.
fn walk(node: &ViewNode, path: &str, visit: &mut impl FnMut(&str, &crate::tree::Props)) {
    let here = if path.is_empty() {
        node.key.as_str().to_owned()
    } else {
        format!("{path}/{}", node.key.as_str())
    };
    visit(&here, &node.props);
    for child in &node.children {
        walk(child, &here, visit);
    }
}

/// **The regression guard for the 2026-08-25 design pass.** Exactly the set
/// of nodes below draws an edge, every one of them in the border tone, and
/// nothing else in the library draws one at all.
///
/// # The rule, stated once
///
/// **Containers get a tone. Controls get an edge.**
///
/// A card, a well, a progress rail, an image frame and a list strip are
/// separated from what is behind them by a *fill* — one layer of the shipped
/// set sitting on another, or an elevation shadow. Every one of those also
/// drew a `text.muted` outline before this pass: 10.73:1 against the fill it
/// was separating, decoration over a shape that already had a boundary, and
/// the reason the page read as a wireframe. Those are gone.
///
/// A `button`, a `field`, a checkbox box, a radio box and a toggle track
/// keep one, for two different reasons that both come down to a
/// measurement:
///
/// - The binary controls' marks have **no fill at all** when they are off.
///   `box_control` passes `None` as the background of an unchecked box, so
///   taking the outline away does not quieten the control, it deletes it.
/// - `field` has a fill and it is not enough. [`on_layer`] steps it one
///   layer ahead of the card it sits on, which measures **1.26:1 in dark and
///   1.12:1 in light** against WCAG 2.1 SC 1.4.11's 3:1 floor. A field is a
///   place to *put* something rather than a thing to press, and it carries
///   no label of its own until somebody types one — an empty well with no
///   boundary does not read as an input at all.
///
/// **`button` and `primary_button` were on this list and came off it.** They
/// are elevated instead: every button casts `shadow.raised`, which is a
/// weaker boundary by measurement (~1.6:1 in light against the border's
/// 3.34:1) and is the call Material 3 and Apple's HIG both make for a filled
/// button. `labelled`'s doc carries the full table and the reasoning; this
/// test is only the pin. If buttons ever draw an edge again, this fails, and
/// that is the intended behaviour rather than an inconvenience.
///
/// `list_row` and `tab` are the deliberate omission: each is one segment of
/// a strip rather than a free-standing control, the strip is what identifies
/// it, and a tab bar of five outlined boxes is a wireframe again.
///
/// # Why one test and not two
///
/// Each half alone is defeatable. A future edit that puts a border back on
/// the card would bind the right *token* and pass a tone check. An edit that
/// repaints the checkbox in `text.muted` would leave the same *set* of nodes
/// bordered and pass a membership check. Both halves are asserted here,
/// against an exact set rather than an allow-list, so adding a border
/// anywhere fails just as loudly as removing one.
///
/// # What it cannot reach
///
/// Only what [`full_gallery`] composes, which is why that fixture is the
/// all-thirteen tree rather than a hand-picked subset. And contrast is not
/// the only thing that makes an edge shout: stroke width lives in the
/// painter (`gorgon-petra-egui`'s `device_snapped_width`), not here. A
/// capture owns that.
#[test]
fn containers_take_a_tone_and_controls_take_an_edge() {
    /// Every node in [`full_gallery`] that may draw a border, by the key
    /// path it appears at. An exact set: a node missing from here that draws
    /// one fails, and a node listed here that stops drawing one fails too.
    const DRAWS_AN_EDGE: [&str; 55] = [
        // `field`: an empty well with no boundary does not read as a place
        // to type. See `field`'s own doc for why it keeps one when `button`
        // does not.
        "root/controls/name",
        // The binary controls' marks: no fill at all when they are off.
        // `check`, `check-off` and `check-disabled` are the same box shape
        // at three different `Semantics` states — `empty_mark`/`marked_box`
        // bind `border` unconditionally, so all three carry it regardless
        // of checked or disabled.
        "root/controls/check/box",
        "root/controls/check-off/box",
        "root/controls/check-disabled/box",
        "root/controls/radio/box",
        "root/controls/toggle/appearance/track",
        // Accordion item used to be here, binding `border` on the whole
        // item as an approximation of Carbon's own `border-top: 1px solid
        // $border-subtle` divider between rows (`_accordion.scss`). A3
        // audit, `01-accordion.png`: unlike a lone box, the item's own
        // `header` child paints an opaque fill *after* the item's border
        // (children paint over their parent) and hid the top edge
        // entirely, while the transparent `panel` child let the left/right
        // edges show through underneath it as two stray hairlines with no
        // visible top — the same defect class `pagination`'s old
        // `nav_button` and `ui_shell`'s old header binding drew, both
        // fixed the same way (V4/V6, see their own comments below). The
        // fix is identical: the binding is gone, and `accordion_item`'s own
        // `divider` child — already a real element, not new machinery — is
        // the one edge Carbon actually draws. See
        // `accordion_item_owns_a_one_px_border_subtle_divider`.
        //
        // AI label's default-variant trigger: Carbon's `border-inverse`
        // (`.cds--ai-label`, MEASURED SCSS; see `ai_label.rs`'s module doc
        // for why `BORDER_SUBTLE` stands in for the missing token). Unlike
        // a filled `button`, which trades its edge for `shadow.raised`, this
        // trigger has no fill loud enough to read as a boundary on its own —
        // its background is `SURFACE_BASE`, the page's own ground — so the
        // edge is the only thing that identifies it as a control at rest.
        // The inline variant (`ai-inline`) is the sibling case that does
        // *not* draw one: its leading bullet dot is the boundary instead.
        "root/carbon/ai-default/trigger",
        // The open AI label's trigger is the same `trigger_button` as the
        // closed one — `open` changes `expanded` and nothing it draws — and
        // its explainability `panel` is a popover: see the popover class
        // under `root/carbon4/pop` below.
        "root/carbon/ai-open/panel",
        "root/carbon/ai-open/trigger",
        // Content switcher's own row: Carbon's `.cds--content-switcher`
        // 1px `$border-subtle` outline (MEASURED `_content-switcher.scss`;
        // see `content_switcher.rs`'s module doc, anatomy #1). The row is a
        // container, but this is a real measured boundary, not decoration —
        // the same class as `field` rather than a card or a strip.
        "root/carbon2/cs",
        // Dropdown's and date picker's closed fields: `SURFACE_RAISED` +
        // `BORDER_SUBTLE`, the same `field`-class pairing and the same
        // reason — a raised fill one layer ahead of its ground is not
        // enough contrast on its own (`field`'s own doc has the
        // measurement), and these are input-shaped wells before anything is
        // typed or chosen.
        // The open forms' `field` is the same `closed_field` as the closed
        // form (only the chevron word and `expanded` change), and their
        // `menu`/`calendar` is a popover — see `root/carbon4/pop` below.
        "root/carbon2/dd-open/field",
        "root/carbon2/dd-open/menu",
        "root/carbon2/dd-theme",
        "root/carbon2/dp-due",
        "root/carbon2/dp-open/calendar",
        "root/carbon2/dp-open/field",
        // Data table rows used to be here, binding the four-sided `border`
        // as an approximation of Carbon's row-bottom rule
        // (`td { border-block-end }`, slice-b:76). On adjacent rows that
        // drew every seam twice and every column edge once — the "grid of
        // boxes" the operator named in both rounds of the 2026-09-04 walk
        // (`.agents/carbon-waves/ROUND2-DEFECTS.md` row 9). The rows now
        // bind `border-bottom`, the one edge Carbon draws, which this test
        // does not audit; see `data_table.rs`'s
        // `header_is_accent_in_compact_heading_and_rows_draw_one_bottom_rule`.
        // What the rows do carry is the selection column's checkbox box —
        // `controls::checkbox_box`, the exact same outline-is-the-mark
        // shape as `root/controls/check/box` above — one per body row and
        // the header's select-all.
        "root/carbon2/dt-jobs/dt-r0/select/box",
        "root/carbon2/dt-jobs/dt-r1/select/box",
        "root/carbon2/dt-jobs/dt-r2/cells/select/box",
        "root/carbon2/dt-jobs/dt-r3/select/box",
        "root/carbon2/dt-jobs/header/select/select-all/box",
        // Form's field child: the exact same `field()` component as
        // `root/controls/name`, for the exact same reason.
        "root/carbon2/fm-signup/fm-name",
        // File uploader's incomplete-item mark: a static ring standing in
        // for Carbon's spinning loader. Like the binary controls' marks
        // above, this shape has **no fill at all** — the border is not
        // decoration on top of a boundary, it is the whole ring. Inline
        // loading's active mark used to sit beside it and no longer does:
        // it is `loading::spinner_small`, a drawn canvas whose track and
        // arc are strokes in a draw list, so it binds no `border`.
        "root/carbon2/fu-f1/mark",
        // File uploader's drop zone: Carbon draws `border: 1px dashed
        // $border-strong` (MEASURED `_file-uploader.scss:425`); Petra has no
        // dashed stroke and substitutes a solid `BORDER_SUBTLE` rather than
        // inventing one — see `file_uploader.rs`'s module doc. The zone is
        // also a click target with no fill loud enough to read as a
        // boundary on its own (`SURFACE_BASE`, the page's own ground), the
        // same shape as AI label's trigger above.
        "root/carbon2/fu-up/zone",
        // Modal draws no edge since 2026-09-04. Carbon's "Container
        // border: 1px `$border-subtle-01`" (SOURCED style page, slice-c)
        // was bound on the dialog shell until the operator's walk of the
        // catalog refused it ("borders are a no no"); the container is a
        // raised tone on a scrim now, which is the "containers take a
        // tone" rule this test is named for, applied to the one container
        // that had been exempt. See `modal.rs`'s module doc.
        // Menu and the open menu button's menu are popovers — see
        // `root/carbon4/pop` below.
        "root/carbon3/mb-open/menu",
        "root/carbon3/mn-actions",
        // Number input's well: Carbon's `border-bottom: 1px solid
        // $border-strong` on `.cds--number` (slice-d, "Field ...
        // `border-bottom` `$border-strong`"), the same `field`-class
        // input-well pairing as `field`, Dropdown and Date picker above —
        // Petra approximates the directional bottom rule as the same full
        // `border.subtle` outline those three already use, rather than
        // inventing a directional token. `ni-disabled` carries it too:
        // `disabled()` only clears interactions and sets
        // `Semantics.disabled`, it never touches a token binding.
        "root/carbon4/ni-count",
        "root/carbon4/ni-disabled",
        // Pagination's own bar was here, binding a four-sided `border` to
        // approximate Carbon's one-sided `border-block-start: 1px solid
        // $border-subtle` (slice-d). W8 audit, `23-pagination.png`: the
        // bar's own children fill its height and painted over the inset
        // edge, so the top and bottom rules showed only in the gaps
        // between cells — three boxes that did not close. The same fix as
        // the header and the accordion item: the one edge Carbon draws is
        // now a real 1-unit `rule` element (`pagination.rs`'s `bar`), which
        // binds `background`, and the bar binds no `border` at all.
        // Pagination's Previous/Next used to approximate Carbon's
        // directional `border-inline-start: 1px solid $border-subtle`
        // (slice-d) as a full 4-sided `border` token, the same trade-off
        // as the well above. V4 audit, `23-pagination.png`: on a lone box
        // that trade-off is harmless, but on two adjacent boxed buttons it
        // drew a real defect — Next's own right edge plus the container's
        // own right edge bracketed the container's trailing padding into
        // what read as an empty fourth pagination cell. The fix is a real
        // 1px divider element (`pagination.rs`'s `nav_divider`, the same
        // technique `accordion`'s own `divider` uses) standing in for the
        // one edge Carbon actually draws, so `previous`/`next` no longer
        // bind `border` at all — the divider binds `background`, which
        // this test does not audit.
        // Progress indicator's three step glyphs are all drawn canvases now
        // (Carbon's `CheckmarkOutline`, `Incomplete` and `CircleDash`, off
        // their own paths), so no step binds `border`. The not-started
        // step's icon was a bordered empty swatch until 2026-09-04 and sat
        // on this list as `pi-review` and `pi-confirm`.
        // Popover, and every surface built on `popover_with` (Menu, Menu
        // button open, Dropdown open, Date picker open, Toggletip open,
        // Tooltip, AI label open) plus the UI shell right panels, which
        // bind the same pair by hand: `SURFACE_RAISED` + `BORDER_SUBTLE`.
        // A floating surface's raised fill is one layer ahead of the page
        // it floats over — the same 1.26:1 / 1.12:1 `field` measures
        // against its card — so the edge is what separates the surface
        // from the page underneath it, the same class as Modal's dialog
        // shell above. These were unreachable from this tree until
        // `Anchor::Sibling` let an open form mount at depth; nothing about
        // what they draw changed.
        "root/carbon4/pop",
        // Radio's own mark: the same `empty_mark`/`marked_box` class as
        // the checkbox boxes above, for the same reason — an unselected
        // radio is nothing but its outline, and the selected/disabled
        // forms keep the same border so the ring does not resize between
        // states.
        "root/carbon4/radio-checked/box",
        "root/carbon4/radio-disabled/box",
        "root/carbon4/radio-unchecked/box",
        // Search's own well: Carbon's `border-block-end: 1px solid
        // $border-strong` on `.cds--search-input` (slice-d anatomy #1),
        // the same directional-rule-approximated-as-a-full-outline class
        // as Number input's well above.
        "root/carbon4/srch-filter",
        // Select's closed field: Carbon's `border-block-end: 1px solid
        // $border-strong` on `.cds--select-input` (slice-e, Select
        // anatomy #3), the same `field`-class input-well pairing as
        // `field`, Dropdown, Date picker, Number input and Search above.
        // Every size (sm/md/lg) and the disabled form all bind it the
        // same way `select_sized` does — `disabled()` never touches a
        // token binding, same precedent as `ni-disabled` above.
        "root/carbon5/sel-disabled",
        "root/carbon5/sel-lg",
        "root/carbon5/sel-sm",
        "root/carbon5/sel-theme",
        // Structured list rows used to be here for the same reason and
        // with the same defect as the Data table rows above (round 2 row
        // 31, "has all the problems of the data table"). Each data row now
        // binds `border-top` and the last one `border-bottom` (slice-e:98);
        // the header binds no edge at all. See `structured_list.rs`'s
        // `rows_draw_one_top_rule_the_last_closes_and_the_header_is_bare`.
        // Tag: only Selectable and Operational carry a Border — slice-e,
        // "Selectable and Operational additionally have a Border (E) that
        // read-only/dismissible tags do not have ... the border is the
        // at-a-glance signal that a tag has increased interactivity."
        // `tag-dismiss` was on this list until this group's audit: `shell`
        // bound the edge for any interactive tag, not just the
        // Selectable/Operational ones Carbon actually draws it on (Class 3,
        // fixed in `tag.rs`'s own `shell` doc). Read-only tags
        // (`tag-ro`/`tag-sm`/`tag-lg`, not on this list either) have no
        // edge, matching the same line.
        "root/carbon5/tag-select-off",
        "root/carbon5/tag-select-on",
        // Tile: no kind draws a resting edge any more. The three
        // interactive kinds used to take `BORDER_SUBTLE` as a second
        // channel beside the hover fill, which put a fill-and-no-edge base
        // tile in a row with three fill-and-edge ones — two visual
        // languages, W8 audit, `35-tile.png`. Carbon's own form without
        // the contrast flag is "interactive tiles have no border at all"
        // (slice-e) and its reference shot draws none, so every kind is
        // now the same box. The one tile edge left is the unselected
        // selectable tile's checkbox-shaped mark: an empty box whose
        // outline is the whole mark, the checkbox's own class above. The
        // selected one (`tile-select-on`) is an accent-filled box with a
        // check and binds no border.
        "root/carbon5/tile-select-off/row/box",
        // Text input (`field`): the exact same `field()`/`field_sm`/
        // `field_lg`/`field_readonly`/`field_fluid` well as
        // `root/controls/name` above, for the exact same reason.
        // `field_labeled`'s wrapper carries no token of its own — its
        // `input` child is where the edge lives, the same anatomy as
        // Form's `fm-name` above. `field_invalid` binds an accent border
        // instead and is deliberately absent from this tree (see this
        // section's own comment above `carbon5`).
        "root/carbon5/txt-default",
        "root/carbon5/txt-disabled",
        "root/carbon5/txt-fluid",
        "root/carbon5/txt-labeled/input",
        "root/carbon5/txt-lg",
        "root/carbon5/txt-readonly",
        "root/carbon5/txt-sm",
        // Toggle: the same `root/controls/toggle/appearance/track` argument
        // as above, for the three additional states this group's own
        // section adds (off, disabled, small) — `toggle_sized` binds
        // `border` unconditionally on the track regardless of `on` or the
        // size variant, the same precedent `check`/`radio`'s boxes set.
        // Toggletip open, both UI shell right panels and Tooltip: the
        // popover class, see `root/carbon4/pop` above.
        "root/carbon6/help-open/tip",
        "root/carbon6/shell-right",
        "root/carbon6/shell-switcher",
        "root/carbon6/tip",
        "root/carbon6/tog-disabled/appearance/track",
        "root/carbon6/tog-off/appearance/track",
        "root/carbon6/tog-on/appearance/track",
        "root/carbon6/tog-sm/appearance/track",
        // UI shell header used to be on this list, binding `border`
        // directly on the header bar for Carbon's `border-block-end: 1px
        // solid $border-subtle` (slice-f "UI shell header" Key numbers).
        // V6 audit, `40-ui-shell-header.png`: the shared `"border"` token
        // always paints a 4-sided box (see Pagination's own comment
        // above), and with the header's children packed edge-to-edge
        // (`None` spacing) and most of them opaque, that box was invisible
        // everywhere a child's own background covered it and showed
        // through as a stray hairline above AND below the nav row only
        // where a child painted no background of its own (`name`,
        // `spacer`) — never the single bottom line Carbon actually draws.
        // The same fix Pagination already took: a real 1px divider element
        // (`ui_shell.rs`'s `divider`, built with `accent_mark`) stands in
        // for the one edge Carbon draws, so the header no longer binds
        // `border` at all — the divider binds `background`, which this
        // test does not audit.
    ];

    // `status` is not on that list, and the omission is measured rather than
    // an oversight: a status mark carries a `silhouette` and a filled
    // status colour, so its shape *is* its boundary and every one of the
    // three shipped colours already clears 3:1 on the layers it can be
    // painted on (`shipped.rs`'s own status gates). Adding an outline would
    // put a grey ring around the one channel that is deliberately not grey.
    // `primary_button` is absent for the reason in this test's doc; its
    // label child is a `Text` node and draws no box at all.

    let mut bordered: Vec<String> = Vec::new();
    walk(&full_gallery(), "", &mut |path, props| {
        let Some(token) = props.tokens.get("border") else {
            return;
        };
        assert_eq!(
            token.as_str(),
            BORDER_SUBTLE,
            "{path} draws its edge in `{}`. A border binds the border tone; \
             binding a text tone is how this library came to look like a \
             wireframe, and it fails no contrast floor on the way.",
            token.as_str()
        );
        bordered.push(path.to_owned());
    });
    bordered.sort();

    let mut expected: Vec<String> = DRAWS_AN_EDGE.iter().map(|s| (*s).to_owned()).collect();
    expected.sort();
    assert_eq!(
        bordered, expected,
        "the set of nodes drawing an edge changed. Containers take a tone \
         and controls take an edge -- read this test's doc before widening \
         the list, and if a node genuinely needs an edge, say which of the \
         two measured reasons applies to it."
    );
}

/// [`on_layer`] seats a control one tone ahead of the ground it is placed
/// on, and a control that means to disappear takes the ground's own tone.
///
/// Both halves are checked against the *shipped colours*, not only against
/// the token names: two names that resolved to one grey would pass a name
/// comparison and paint an invisible control, which is precisely the failure
/// the four-layer set was extended to fix.
#[test]
fn on_layer_steps_a_control_one_tone_ahead_of_its_ground() {
    use crate::token::{LAYER_TOKENS, TokenValue, dark, light};

    for depth in 0..=MAX_LAYER_DEPTH {
        let raised = on_layer(button("b", "Save"), depth);
        let flush = on_layer(tab("t", "Trace", false), depth);

        assert_eq!(
            raised.props.tokens.get("background").map(TokenName::as_str),
            Some(LAYER_TOKENS[depth + 1]),
            "a raised control seated on layer {depth} must take the tone one \
             step ahead of it"
        );
        assert_eq!(
            flush.props.tokens.get("background").map(TokenName::as_str),
            Some(LAYER_TOKENS[depth]),
            "a control that means to sit flush with its ground must take the \
             ground's own tone"
        );

        for (label, theme) in [("light", light()), ("dark", dark())] {
            let colour = |token: &str| match theme.value(&TokenName::new(token).unwrap()) {
                Some(TokenValue::Color(c)) => *c,
                other => panic!("{token} is not a colour: {other:?}"),
            };
            assert_ne!(
                colour(LAYER_TOKENS[depth]),
                colour(LAYER_TOKENS[depth + 1]),
                "{label}: layers {depth} and {} resolve to the same colour, \
                 so a control seated here has no edge at all — the tonal cue \
                 is the whole depth cue now that the borders are gone",
                depth + 1
            );
        }
    }
}

/// A seat deeper than the layer set can express is clamped rather than
/// allowed to run off the end of it.
///
/// The end of `LAYER_TOKENS` is `surface.layer-three`, so an unclamped
/// `depth + 1` would either panic on the index or — worse, if someone
/// "fixed" it with a saturating index — resolve the control and its ground
/// to the same grey. That second failure is silent: the token resolves, the
/// painter reports a fill, and the control is invisible.
#[test]
fn a_seat_deeper_than_the_ramp_is_clamped_and_still_has_a_step_in_it() {
    use crate::token::LAYER_TOKENS;

    let deep = on_layer(button("b", "Save"), MAX_LAYER_DEPTH + 40);
    assert_eq!(
        deep.props.tokens.get("background").map(TokenName::as_str),
        Some(LAYER_TOKENS[MAX_LAYER_DEPTH + 1]),
        "an over-deep seat must land on the deepest step the ramp can \
         express, not past the end of it"
    );
}

/// [`on_layer`] rewrites the two tones it is about and nothing else, and it
/// never adds an edge.
///
/// The accent case is the one with a measurement behind it: `shipped.rs`'s
/// `DEEPEST_ACCENT_LAYER` records that dark's accent is under the 3:1 fill
/// floor on `surface.layer-three`, so a re-seating pass that treated an
/// accent fill as "a background, therefore mine to move" could walk a
/// primary button onto a ground its own colour cannot carry.
/// An ordinal layer name is re-seated too, by the same amount.
///
/// Every one of these names is a distance from the ground, so re-seating has
/// to move all of them together. It used to move only the two lowest, which
/// made the operator non-composable: a component that stepped its own two
/// surfaces had them collapsed onto one tone the moment its mount point
/// re-seated the outer one. The modal is where that showed — its dialog and
/// its Cancel button rendered as the same `#333333`.
#[test]
fn on_layer_moves_an_ordinal_layer_by_the_same_step_as_the_two_below_it() {
    fn seat(name: &str, depth: usize) -> String {
        let mut node = ViewNode::new(NodeKind::Stack, "n");
        node.props
            .tokens
            .insert("background".into(), TokenName::new(name).unwrap());
        on_layer(node, depth)
            .props
            .tokens
            .get("background")
            .expect("the binding survives")
            .as_str()
            .to_owned()
    }
    // Two nodes a step apart stay a step apart after a re-seat.
    assert_eq!(seat("surface.raised", 1), "surface.layer-two");
    assert_eq!(seat("surface.layer-two", 1), "surface.layer-three");
    // And the ramp's own ceiling is where that stops. Four steps is four
    // steps; this test states the limit rather than pretending there is none.
    assert_eq!(seat("surface.layer-three", 1), "surface.layer-three");
}

#[test]
fn on_layer_leaves_every_other_fill_alone_and_never_touches_an_edge() {
    for depth in 0..=MAX_LAYER_DEPTH {
        let primary = on_layer(primary_button("p", "Save"), depth);
        assert_eq!(
            primary
                .props
                .tokens
                .get("background")
                .map(TokenName::as_str),
            Some(ACCENT_PRIMARY),
            "an accent fill is a deliberate choice by whoever bound it, not a \
             surface tone for this pass to step"
        );

        for (before, after) in [
            (button("b", "Save"), on_layer(button("b", "Save"), depth)),
            (
                field("f", "Fiber name"),
                on_layer(field("f", "Fiber name"), depth),
            ),
            (
                tab("t", "Trace", false),
                on_layer(tab("t", "Trace", false), depth),
            ),
            (
                list_row("l", "row", true),
                on_layer(list_row("l", "row", true), depth),
            ),
            (primary_button("p", "Save"), primary),
        ] {
            assert_eq!(
                before.props.tokens.get("border"),
                after.props.tokens.get("border"),
                "{:?}'s edge changed when it was re-seated. Which components \
                 draw an edge, and in what tone, is decided once by the \
                 component (see `containers_take_a_tone_and_controls_take_an_edge`) \
                 and is not a depth question. An operator that added one here \
                 would put an outline on a card; one that removed it would take \
                 the boundary off every control on a card.",
                after.key
            );
        }
    }
}

/// The primary button spends the accent, and its label is the ink that goes
/// with it.
///
/// `text.on-accent` is not a stylistic pick: `shipped.rs` measures both
/// shipped text tones on both accents at 1.95:1 to 3.48:1, all four under
/// the 4.5:1 AA floor. A caller — or a later edit — that "simplifies" the
/// label back to `text.primary` fails here.
#[test]
fn the_primary_button_spends_the_accent_and_the_ink_that_goes_with_it() {
    let node = primary_button("p", "Save");
    assert_eq!(
        node.props.tokens.get("background").map(TokenName::as_str),
        Some(ACCENT_PRIMARY)
    );
    assert!(
        node.props.tokens.contains_key("shadow"),
        "a button sits *on* the surface, and depth is what says so"
    );
    assert!(
        !node.props.tokens.contains_key("border"),
        "the accent is the emphasis; an edge on top of it is the wireframe again"
    );

    let label = node
        .children
        .first()
        .expect("primary_button carries its label as a child node");
    assert_eq!(
        label.props.tokens.get("foreground").map(TokenName::as_str),
        Some(TEXT_ON_ACCENT),
        "neither shipped text tone clears AA on either accent — see \
         `ON_ACCENT_TOKEN`'s own measurements"
    );

    // The plain button is elevated too -- the two are told apart by their
    // fill, not by their depth, because that is where the contrast is. What
    // it must not do is pick up the accent.
    let plain = button("b", "Cancel");
    assert!(
        plain.props.tokens.contains_key("shadow"),
        "every button lifts off the surface; only the fill ranks them"
    );
    assert_ne!(
        plain.props.tokens.get("background").map(TokenName::as_str),
        Some(ACCENT_PRIMARY),
        "an accent that appears on every button is not an accent"
    );
    assert!(
        !plain.props.tokens.contains_key("border"),
        "a default button is separated by tone and depth, not by an outline"
    );
}

/// Keeps [`full_gallery`]'s two status calls pinned to distinct shapes, so
/// the acceptance test above is not silently exercising `Circle` twice.
#[test]
fn the_fixture_status_tokens_are_shaped_differently() {
    let a = StatusToken::new(
        TokenName::new("status.ok").unwrap(),
        StatusShape::Circle,
        "OK",
    )
    .unwrap();
    let b = StatusToken::new(
        TokenName::new("status.degraded").unwrap(),
        StatusShape::Triangle,
        "Degraded",
    )
    .unwrap();
    assert_ne!(a.shape(), b.shape());
}

/// First node in `node` (inclusive) whose key is `key`.
fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
    fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|child| walk(child, key))
    }
    walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
}

/// Petrify a single component under a vertical stack root.
fn accepting_registry() -> Registry {
    let mut registry = Registry::with_vocabulary(standard_vocabulary());
    crate::anim::shipped_registry().declare_into(&mut registry);
    registry
}

fn petrify_lone(child: ViewNode) -> crate::frame::PetrifiedFrame {
    let root = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(child);
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

/// Petrify a lone [`progress`] at `value` and return the widths the frame
/// actually drew: `(bar, fill, track)`.
///
/// The tests above read `ViewNode` props, which is exactly how a zero-width
/// fill shipped: the column weights were right, the two cells were pinned to
/// zero extent, and nothing in this file looked at a rect. This helper is the
/// geometry channel — it runs the same `petrify` the acceptance test above
/// runs and reports placed rects, not declarations.
fn bar_widths(value: f32) -> (f32, f32, f32) {
    let root = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(progress("bar", "Rebuild", value));
    let registry = Registry::with_vocabulary(standard_vocabulary());
    let mut harness = Harness::new();
    let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
    harness.scale = viewport.scale;
    let frame = petrify(
        1,
        validated_with(&root, &registry),
        &mut harness.ctx(),
        viewport,
        TransitionActivity::default(),
    );
    let width_of = |suffix: &str| {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("{suffix} is missing from the petrified frame"))
            .rect
            .w
    };
    (
        width_of("/root/bar"),
        width_of("/root/bar/fill"),
        width_of("/root/bar/track"),
    )
}

/// The percentage [`progress`] announces to a screen reader, as a number.
fn announced_percent(value: f32) -> f32 {
    let reported = progress("bar", "Rebuild", value)
        .semantics
        .value
        .expect("progress always reports a value");
    reported
        .strip_suffix('%')
        .unwrap_or_else(|| panic!("{reported:?} is not a percentage"))
        .parse()
        .unwrap_or_else(|e| panic!("{reported:?} does not parse as a percentage: {e}"))
}

/// The fraction of the bar the fill actually covers, in percent.
fn drawn_percent(value: f32) -> f32 {
    let (bar, fill, _) = bar_widths(value);
    assert!(bar > 0.0, "the bar itself was drawn at zero width");
    fill / bar * 100.0
}

/// A 62% bar is drawn 62% full, and the two cells tile the bar exactly.
///
/// Both halves matter. The weights alone were already right when the fill
/// was zero pixels wide, so the claim under test is the placed rect: the
/// fill occupies its track rather than declining it.
#[test]
fn the_fill_is_drawn_at_the_width_its_value_asks_for() {
    let (bar, fill, track) = bar_widths(0.62);
    assert!(fill > 0.0, "the fill of a 62% bar was drawn {fill} wide");
    assert!(
        (fill / bar - 0.62).abs() < 0.005,
        "a 0.62 value drew {fill} of {bar} ({:.1}%), not ~62%",
        fill / bar * 100.0
    );
    assert!(
        (fill + track - bar).abs() < 0.5,
        "fill {fill} + track {track} does not tile the {bar}-wide bar"
    );
}

/// The ends: an empty bar draws no meaningful fill, a complete one draws
/// almost nothing but fill, and the fill grows with the value in between.
///
/// Neither end lands on exactly 0 or exactly `bar`: tree acceptance refuses
/// a `Weight` track of zero, so the empty side of the bar carries
/// `MIN_WEIGHT` (0.001) instead — a tenth of a percent, which rounds away on
/// screen and in the announced percentage alike.
#[test]
fn an_empty_bar_and_a_complete_bar_are_both_drawn() {
    let (bar_0, fill_0, track_0) = bar_widths(0.0);
    assert!(
        fill_0 / bar_0 < 0.01,
        "a 0.0 value drew {fill_0} of {bar_0} — an empty bar must read empty"
    );
    assert!(
        track_0 / bar_0 > 0.99,
        "a 0.0 value left only {track_0} of {bar_0} for the track"
    );

    let (bar_1, fill_1, track_1) = bar_widths(1.0);
    assert!(
        fill_1 / bar_1 > 0.99,
        "a 1.0 value drew {fill_1} of {bar_1} — a complete bar must read full"
    );
    assert!(
        track_1 / bar_1 < 0.01,
        "a 1.0 value left {track_1} of {bar_1} still unfilled"
    );

    let fill_62 = bar_widths(0.62).1;
    assert!(
        fill_0 < fill_62 && fill_62 < fill_1,
        "fill is not monotone in value: {fill_0} / {fill_62} / {fill_1}"
    );
}

/// The label and the bar are two readers of one number, so they must never
/// disagree — including on the inputs that are not numbers.
///
/// `f32::clamp` propagates `NaN` and `f32::max` scrubs it, so before the
/// single normalisation in `progress` a `NaN` announced "NaN%" over two
/// equal weights: a bar drawn half full. Out-of-range and non-finite input
/// is normalised once, ahead of both channels; the announced percentage and
/// the drawn percentage are compared here against each other, not against a
/// hardcoded pair, so neither channel can be fixed alone.
#[test]
fn the_announced_percentage_and_the_drawn_fill_always_agree() {
    for value in [
        0.0,
        0.62,
        1.0,
        -0.5,
        1.5,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        let announced = announced_percent(value);
        let drawn = drawn_percent(value);
        assert!(
            (announced - drawn).abs() < 0.5,
            "value {value:?} announces {announced}% but draws {drawn:.2}%"
        );
    }
    assert_eq!(
        announced_percent(f32::NAN),
        0.0,
        "a value nobody could compute must not announce a finished job"
    );
}

/// FR-004a's two layering rules, read off every seat the ramp can express.
///
/// **The border rule is the one this exists for.** Carbon states it as
/// *"border tokens pair with its same number, for example `$field-03` pairs
/// with `$border-strong-03`"* — the field's number, not the background's — and
/// stated in prose it is the kind of rule that survives one implementation and
/// then quietly becomes "the border pairs with the background" in the next.
/// Stated as arithmetic it is two ordinals that either match or do not.
///
/// The ordinals are parsed back out of the token names rather than compared to
/// a table written here, so this fails if `layer_tokens` starts returning a
/// consistent-looking triple that has slipped a step — which a table would
/// have to be edited to notice.
#[test]
fn the_layer_triple_puts_the_field_one_ahead_and_the_border_beside_it() {
    /// The trailing two-digit ordinal of a Carbon-spelled name, if it has one.
    fn ordinal(token: &str) -> Option<usize> {
        token.rsplit('-').next()?.parse().ok()
    }

    for depth in 0..=MAX_LAYER_DEPTH {
        let seat = layer_tokens(depth);

        // The background carries no ordinal of its own — the layer set spells
        // its steps as words — so the seat number is the source of truth for
        // it, and the other two are checked against that.
        let field = ordinal(seat.field)
            .unwrap_or_else(|| panic!("`{}` carries no ordinal to pair a border with", seat.field));
        let border =
            ordinal(seat.border).unwrap_or_else(|| panic!("`{}` carries no ordinal", seat.border));

        assert_eq!(
            field,
            depth + 1,
            "a field on {} must be field-0{}, one layer ahead, not `{}`",
            seat.background,
            depth + 1,
            seat.field
        );
        assert_eq!(
            border, field,
            "`{}` pairs with `{}`: a border takes the *field's* number, not \
             the background's. This is the rule that gets lost.",
            seat.border, seat.field
        );
    }

    // Past the end of the ramp the seat clamps rather than running off it,
    // for `MAX_LAYER_DEPTH`'s reason: the alternative resolves a node and its
    // ground to the same colour and reports success.
    assert_eq!(
        layer_tokens(MAX_LAYER_DEPTH + 7),
        layer_tokens(MAX_LAYER_DEPTH),
        "a deeper seat than the ramp can express must clamp, not wrap or panic"
    );
}

/// Every name `layer_tokens` can emit is one the shipped vocabulary declares.
///
/// Without this the operators would be a well-formed arithmetic over names
/// that resolve to nothing: a tree built from them is refused at acceptance,
/// at runtime, in whichever component reaches the deepest seat first.
#[test]
fn every_layering_operator_name_is_in_the_standard_vocabulary() {
    let vocab = standard_vocabulary();
    for depth in 0..=MAX_LAYER_DEPTH {
        let seat = layer_tokens(depth);
        for token in [seat.background, seat.field, seat.border] {
            let name = TokenName::new(token)
                .unwrap_or_else(|err| panic!("`{token}` is not a well-formed token name: {err}"));
            assert!(
                vocab.contains(&name),
                "seat {depth} emits `{token}`, which standard_vocabulary() does \
                 not declare"
            );
        }
    }
}

/// **A state-decorated binding needs a resting one underneath it.**
///
/// This is the rule the Accordion and Modal crash taught, generalised. A node
/// that binds `background@hover` and no plain `background` resolves to nothing
/// at rest: `resolve_slot` walks `slot@state -> slot`, and if neither exists it
/// returns `None`. The painter then counts the placement **silent** rather than
/// empty, because `PaintContent::is_empty()` only asks whether `tokens` is
/// empty — and it is not, the `@hover` entry is sitting right there. A silent
/// placement trips the paint-accounting assertion and takes the window down.
///
/// The catalog's `every_built_page_paints_with_nothing_silent` catches this too,
/// but only for what a catalog page happens to mount. This one is structural:
/// it needs no paint pass, no window, and no page, so it also covers a
/// component reachable only inside another component.
///
/// The fix is never to delete the `@hover` binding. It is to bind the resting
/// slot to whatever surface the control sits on, the way `button.rs`'s Ghost
/// variant already does.
#[test]
fn a_state_decorated_token_always_has_a_resting_binding() {
    use crate::token::state::STATE_SEPARATOR;

    let tree = full_gallery();
    let mut offences: Vec<String> = Vec::new();
    walk(&tree, "", &mut |path, props| {
        for key in props.tokens.keys() {
            let Some((slot, state)) = key.split_once(STATE_SEPARATOR) else {
                continue;
            };
            if !props.tokens.contains_key(slot) {
                offences.push(format!(
                    "{path}: binds {slot}{STATE_SEPARATOR}{state} with no resting {slot}, \
                     so it resolves to nothing and paints silent"
                ));
            }
        }
    });
    assert!(
        offences.is_empty(),
        "every state-decorated binding needs a resting one underneath it:\n{}",
        offences.join("\n")
    );
}

// -- Anchored overlays mount at depth. ---------------------------------------
//
// A constructor cannot know the canonical key-path it will be mounted at, so
// every anchored overlay in this module names its trigger by bare sibling
// key (`Anchor::Sibling`), and `crate::tree::validate` resolves that key
// against the surface's *own* parent path. The tests below are the claim
// that made this necessary: an open form has to be accepted at the depth the
// gallery catalog actually mounts pages at, not only at the tree root.
// See `.agents/notes/implemented/architecture/
// 2026-09-03-anchored-components-cannot-name-their-own-anchor.md`.

/// `children`, mounted the way `gorgon-petra-egui`'s gallery catalog mounts
/// every page: a body column inside a [`section`] inside the page root. Two
/// containers stand between the root and the component, so a bare key that
/// only resolved at the root would fail here.
pub(super) fn mounted_like_the_catalog(children: Vec<ViewNode>) -> ViewNode {
    let body = ViewNode::new(NodeKind::Stack, "body")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .with_children(children);
    let page = section("page", "Page", vec![body]);
    ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(page)
}

/// `validate`, with the shipped vocabulary, accepts `children` mounted at
/// the catalog's depth. Panics with every violation otherwise, so a refusal
/// names the anchor it could not resolve.
pub(super) fn assert_mounts_at_catalog_depth(what: &str, children: Vec<ViewNode>) {
    let tree = mounted_like_the_catalog(children);
    let registry = Registry::with_vocabulary(standard_vocabulary());
    if let Err(errors) = crate::tree::validate(&tree, &registry) {
        panic!("{what}: mounted two levels deep, `validate` refused the tree:\n{errors}");
    }
}

/// The acceptance test for the whole change: `dropdown_open`, whose popover
/// names its `field` sibling by bare key, is accepted two containers below
/// the root with the shipped `Registry`. Before `Anchor::Sibling` existed,
/// this was refused as `AnchorTargetMissing` because `field` was compared
/// against `/root/page/body/theme/field` verbatim.
#[test]
fn dropdown_open_validates_when_mounted_at_catalog_depth() {
    assert_mounts_at_catalog_depth(
        "dropdown_open",
        vec![super::dropdown_open(
            "theme",
            "Theme",
            "Dark",
            vec![
                dropdown_option("dark", "Dark", true),
                dropdown_option("light", "Light", false),
            ],
        )],
    );
}
