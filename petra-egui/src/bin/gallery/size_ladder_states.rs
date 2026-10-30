//! Spec 008 US6 (FR-025) wave 1: the size ladders of the six pages that
//! grew steps — Date picker, Modal, Structured list, Tabs, Data table's
//! toolbar tier, UI shell left panel.
//!
//! Probed the way
//! `pagination_nav_states.rs::the_variant_builds_at_three_steps_on_the_page`
//! probes: the placed inner node, not a root that includes a rule, a
//! scrim or a wrapper column. Greyscale is read by eye from
//! `PETRA_SHOT_DIR`'s PNGs; the probes below are what make that reading a
//! foregone conclusion rather than the only evidence.

use super::shots::Camera;

/// Register rows #157/#159: the dated well carries the field ramp's three
/// heights (sm 32 / md 40 / lg 48). Probing the well (`size-dp-*/field`),
/// not the column around it.
#[test]
fn the_date_picker_well_builds_at_three_steps_on_the_page() {
    let cam = Camera::on("Date picker");
    for (tail, want) in [
        ("size-dp-sm/field", 32.0),
        ("size-dp-md/field", 40.0),
        ("size-dp-lg/field", 48.0),
    ] {
        assert!(cam.has(tail), "no size cell for {tail}");
        let well = cam.rect(tail);
        assert!(
            (well.h - want).abs() < 1.0,
            "{tail} is {} tall, not {want}",
            well.h
        );
    }
}

/// Register rows #471–473: the contained strip carries the md/lg/xl
/// ladder — 40 / 48 / 64 — one tab node per step, probed on the tab
/// itself.
#[test]
fn the_contained_strip_carries_md_lg_and_xl_on_the_page() {
    let cam = Camera::on("Tabs");
    for (tail, want) in [
        ("size-ct-md", 40.0),
        ("size-ct-lg", 48.0),
        ("size-ct-xl", 64.0),
    ] {
        assert!(cam.has(tail), "no size cell for {tail}");
        let tab = cam.rect(tail);
        assert!(
            (tab.h - want).abs() < 1.0,
            "{tail} is {} tall, not {want}",
            tab.h
        );
    }
}

/// Register row #455: the condensed row is exactly one padding step
/// tighter than the default's (`padding-td` 16+24 → `padding-td--condensed`
/// 8+8 is 24), and each row's text sits at its own top inset (16 vs 8).
/// The totals are emergent from padding plus the text line, so the claim
/// is the delta and the insets, not a magic total.
#[test]
fn the_condensed_row_is_one_padding_step_tighter_on_the_page() {
    let cam = Camera::on("Structured list");
    let default_row = cam.rect("size-sl-default/size-row-default");
    let condensed_row = cam.rect("size-sl-condensed/size-row-condensed");
    assert!(
        (default_row.h - condensed_row.h - 24.0).abs() < 1.0,
        "condensed is not 24 tighter than default: {} vs {}",
        condensed_row.h,
        default_row.h
    );
    let default_text = cam.rect("size-name");
    let condensed_text = cam.rect("size-cname");
    assert!(
        (default_text.y - default_row.y - 16.0).abs() < 1.0,
        "the default row's text is not 16 under its top"
    );
    assert!(
        (condensed_text.y - condensed_row.y - 8.0).abs() < 1.0,
        "the condensed row's text is not 8 under its top"
    );
}

/// Register row #612: every left-panel row kind carries slice-f's two
/// heights — default 32 and `--item--large` 48 — and a large branch's
/// mounted sub-menu rows are re-pinned with it (`size-sub-lg/row`), so one
/// panel is never half a step.
#[test]
fn the_left_panel_rows_carry_default_and_item_large_on_the_page() {
    let cam = Camera::on("UI shell left panel");
    for (tail, want) in [
        ("size-item-d/row", 32.0),
        ("size-icon-item-d/row", 32.0),
        ("size-sub-d/row", 32.0),
        ("size-item-lg/row", 48.0),
        ("size-icon-item-lg/row", 48.0),
        ("size-sub-lg/row", 48.0),
        ("size-icon-sub-lg/row", 48.0),
    ] {
        assert!(cam.has(tail), "no size cell for {tail}");
        let row = cam.rect(tail);
        assert!(
            (row.h - want).abs() < 1.0,
            "{tail} is {} tall, not {want}",
            row.h
        );
    }
}

/// Register rows #604–607: the toolbar and the batch action bar each
/// carry both documented heights (large 48, small 32), and the toolbar's
/// settings trigger follows its bar's step rather than floating 16 units
/// taller than a small one.
#[test]
fn the_toolbar_and_batch_bar_show_both_steps_on_the_page() {
    let cam = Camera::on("Data table");
    for (tail, want) in [
        ("size-toolbar-large", 48.0),
        ("size-toolbar-small", 32.0),
        ("size-batch-large", 48.0),
        ("size-batch-small", 32.0),
        ("size-menu-lg/trigger", 48.0),
        ("size-menu-sm/trigger", 32.0),
    ] {
        assert!(cam.has(tail), "no size cell for {tail}");
        let bar = cam.rect(tail);
        assert!(
            (bar.h - want).abs() < 1.0,
            "{tail} is {} tall, not {want}",
            bar.h
        );
    }
}

/// Register rows #299–302: each modal step takes its documented width
/// share of the window (xs 32% / sm 42% / md 60% / lg 84%, the
/// `lg`-breakpoint column of slice-c's table) and stays centred in it.
/// Driven one step at a time — an open dialog blocks the page, so the
/// trigger for the next step cannot be reached until this one closes —
/// and photographed per step for the greyscale read.
#[test]
fn each_modal_step_takes_its_documented_width_share_on_the_page() {
    let mut cam = Camera::on("Modal");
    for (trigger, step, share) in [
        ("size-open-xs", "size-dlg-xs", 0.32),
        ("size-open-sm", "size-dlg-sm", 0.42),
        ("size-open-md", "size-dlg-md", 0.6),
        ("size-open-lg", "size-dlg-lg", 0.84),
    ] {
        cam.click(trigger);
        let scrim = cam.rect(step);
        let dialog = cam.rect(&format!("{step}/frame/seat/dialog"));
        assert!(
            (dialog.w - scrim.w * share).abs() < 1.0,
            "the {step} dialog is {} wide at share {share}, not {}",
            dialog.w,
            scrim.w * share
        );
        assert!(
            (dialog.x + dialog.w / 2.0 - (scrim.x + scrim.w / 2.0)).abs() < 1.0,
            "the {step} dialog is not centred in the window"
        );
        cam.shoot(&format!("20-modal-{step}"));
        cam.click(&format!("{step}/frame/seat/dialog/header/close"));
    }
}
