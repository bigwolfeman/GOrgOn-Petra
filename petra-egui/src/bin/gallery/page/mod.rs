//! One module per catalog page.
//!
//! Each inventory row's page owns its own state, its own node-id constants,
//! its body and its handler, so two agents fixing two rows edit two files
//! and never the same `match`. `catalog.rs` keeps the chrome — the roster,
//! the open page, the index pane, Prev/Next — and reaches a page only
//! through [`Page`]. Adding a row is one new file here, one line in
//! [`all`], and the row in `inventory.rs`.
//!
//! Rationale and the alternatives weighed:
//! `.agents/notes/implemented/architecture/2026-09-04-one-module-per-catalog-page.md`.

pub mod common;

mod accordion;
mod ai_label;
mod breadcrumb;
mod button;
mod checkbox;
mod code_snippet;
mod contained_list;
mod content_switcher;
mod data_table;
mod date_picker;
mod dropdown;
mod file_uploader;
mod form;
mod inline_loading;
mod link;
mod list;
mod loading;
mod menu;
mod menu_buttons;
mod modal;
mod notification;
mod number_input;
mod pagination;
mod popover;
mod progress_bar;
mod progress_indicator;
mod radio_button;
mod search;
mod select;
mod slider;
mod structured_list;
mod tabs;
mod tag;
mod text_input;
mod tile;
mod toggle;
mod toggletip;
mod tooltip;
mod tree_view;
mod ui_shell_header;
mod ui_shell_left_panel;
mod ui_shell_right_panel;

use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

/// One catalog page: the state, body and handler for one inventory row.
///
/// The catalog rebuilds its whole tree every pass, so `body` is called once
/// per frame and builds from the page's own state; `handle` is called for
/// an activation the chrome has already accepted and did not consume.
pub trait Page {
    /// The inventory row name this renders, verbatim from `inventory.rs`.
    /// The chrome finds the open page by it.
    fn row(&self) -> &'static str;

    /// Build the body from this page's own state.
    fn body(&self) -> ViewNode;

    /// React to an activation routed to `node`, the full id path of the node
    /// the press or key landed on. Returns whether the page consumed it.
    ///
    /// The whole path, not its last segment: a press on a knob or a label
    /// names that child in the route, and a page's own control can be two
    /// segments (`pager` and `next`), so an implementation matches with
    /// [`common::path_has`].
    fn handle(&mut self, event: &InputEvent, node: &str) -> bool;
}

/// Every page, in inventory order. One line per row.
pub fn all() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(accordion::Accordion::default()),
        Box::new(ai_label::AiLabel),
        Box::new(breadcrumb::Breadcrumb),
        Box::new(button::Button),
        Box::new(checkbox::Checkbox::default()),
        Box::new(code_snippet::CodeSnippet),
        Box::new(contained_list::ContainedList),
        Box::new(content_switcher::ContentSwitcher::default()),
        Box::new(data_table::DataTable),
        Box::new(date_picker::DatePicker),
        Box::new(dropdown::Dropdown),
        Box::new(file_uploader::FileUploader),
        Box::new(form::Form),
        Box::new(inline_loading::InlineLoading),
        Box::new(link::Link),
        Box::new(list::List),
        Box::new(loading::Loading),
        Box::new(menu::Menu),
        Box::new(menu_buttons::MenuButtons),
        Box::new(modal::Modal),
        Box::new(notification::Notification),
        Box::new(number_input::NumberInput),
        Box::new(pagination::Pagination::default()),
        Box::new(popover::Popover),
        Box::new(progress_bar::ProgressBar),
        Box::new(progress_indicator::ProgressIndicator),
        Box::new(radio_button::RadioButton::default()),
        Box::new(search::Search),
        Box::new(select::Select),
        Box::new(slider::Slider),
        Box::new(structured_list::StructuredList),
        Box::new(tabs::Tabs::default()),
        Box::new(tag::Tag::default()),
        Box::new(text_input::TextInput),
        Box::new(tile::Tile::default()),
        Box::new(toggle::Toggle::default()),
        Box::new(toggletip::Toggletip),
        Box::new(tooltip::Tooltip),
        Box::new(tree_view::TreeView),
        Box::new(ui_shell_header::UiShellHeader),
        Box::new(ui_shell_left_panel::UiShellLeftPanel),
        Box::new(ui_shell_right_panel::UiShellRightPanel),
    ]
}

#[cfg(test)]
mod tests {
    use super::all;
    use crate::inventory::ROWS;

    /// `all` names every inventory row exactly once, in inventory order, so
    /// the chrome's lookup by `Page::row` can never miss a built row and no
    /// two modules can claim the same one.
    #[test]
    fn every_inventory_row_has_exactly_one_page_in_inventory_order() {
        let pages = all();
        assert_eq!(pages.len(), ROWS.len());
        for (page, row) in pages.iter().zip(ROWS.iter()) {
            assert_eq!(page.row(), row.component);
        }
    }
}
