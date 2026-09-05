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

use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::InputEvent;
use gorgon_petra::token::ThemeMode;
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

    /// The host's clock, in seconds, for the pass about to call
    /// [`Page::body`]. Forwarded from `App::tick` to the open page.
    ///
    /// A page whose picture turns with time — the two loading rows — keeps
    /// it and builds from it; the default drops it, for the forty pages
    /// that draw nothing time-dependent.
    fn tick(&mut self, now: f64) {
        let _ = now;
    }

    /// Build the body from this page's own state.
    fn body(&self) -> ViewNode;

    /// A theme this page wants the whole catalog switched to, taken and
    /// cleared.
    ///
    /// Row 27's radio group is labelled Theme and offers Dark and Light, and
    /// until 2026-09-05 choosing Light moved a dot and nothing else. The
    /// operator: *"in here you have dark and light as options, actually
    /// implement that so I can see the light theme version."*
    ///
    /// A theme is application-wide, so a page cannot publish one; it asks,
    /// the chrome converts the mode into a `Theme`, and `App::theme_request`
    /// hands it to the host, which owns the `Presenter`. The default asks for
    /// nothing, which is forty-one of the forty-two rows.
    fn theme_request(&mut self) -> Option<ThemeMode> {
        None
    }

    /// Text this page wants on the system clipboard, taken and cleared.
    ///
    /// Row 6's Copy button had nowhere to put a string: `Page::handle` sees a
    /// route and an event and holds no handle to the window, so the control
    /// looked like it worked and dropped every press. The chrome forwards
    /// this to `App::clipboard_request`, which the host answers with
    /// `egui::Context::copy_text`.
    ///
    /// The default copies nothing.
    fn clipboard_request(&mut self) -> Option<String> {
        None
    }

    /// React to an activation routed to `node`, the full id path of the node
    /// the press or key landed on. Returns whether the page consumed it.
    ///
    /// The whole path, not its last segment: a press on a knob or a label
    /// names that child in the route, and a page's own control can be two
    /// segments (`pager` and `next`), so an implementation matches with
    /// [`common::path_has`].
    fn handle(&mut self, event: &InputEvent, node: &str) -> bool;

    /// React to a pointer gesture — a press, a move or release under
    /// capture, or the `GestureEnded` report — routed to `node`, with the
    /// frame the route was computed against. Returns whether the page
    /// consumed it; a consumed event is not offered to [`Page::handle`].
    ///
    /// Separate from `handle` because the chrome offers `handle` only
    /// activations (a primary press, Enter, Space), and a drag is mostly
    /// moves. The frame is here and not on `handle` because a gesture is
    /// the one thing that needs geometry: a move is a window position, and
    /// the value it names is that position along some rect only the frame
    /// has (`gorgon_petra::component::slider_value_at`).
    ///
    /// `node` is empty for a pointer move or pointer-exit that routed
    /// nowhere — the pointer is over nothing that accepts hover. A page
    /// whose surface is revealed by hover (row 38, Tooltip) closes it on
    /// that; [`common::path_has`] on an empty path matches no key, so every
    /// other page ignores it exactly as it ignores a stranger's node.
    ///
    /// The default consumes nothing, so the forty pages with no drag and no
    /// hover-revealed surface declare none. A page that declares
    /// `Interaction::Drag` on a control overrides this, or the control is
    /// dead — exactly what row 30 was.
    fn gesture(&mut self, event: &InputEvent, node: &str, frame: &PetrifiedFrame) -> bool {
        let _ = (event, node, frame);
        false
    }

    /// Close every surface named in `ids`, each an
    /// `InputPolicy::DismissOutside` surface a press landed outside of.
    ///
    /// The ids are canonical placement ids, so a page matches its own
    /// surface with [`common::dismisses`] rather than by equality: the
    /// chrome's mount path is not the page's business.
    ///
    /// The chrome delivers this **after** [`Page::handle`] has seen the
    /// press, not before as the host does (`App::dismissed`). The order
    /// matters for the one control every popup page has: a trigger that
    /// toggles. Delivered first, a press on the open trigger would close the
    /// surface and then `handle` would toggle it straight back open; the
    /// press that closes a menu by landing on its own trigger would leave
    /// the menu open. Delivered second, `handle` toggles it shut and the
    /// dismissal finds it already closed.
    ///
    /// The default ignores dismissals, for the pages with no such surface.
    fn dismissed(&mut self, ids: &[String]) {
        let _ = ids;
    }

    /// Keyboard focus moved: `node` is the canonical placement id that now
    /// holds it, `None` when nothing does. Delivered from the host's own
    /// focus tree (`App::focus_changed`), so a page never keeps a second
    /// copy of who is focused; it reacts to the move.
    ///
    /// Focus is host-owned and painted by the engine, so the forty pages
    /// with nothing revealed by focus declare none. The one that does is
    /// row 38 (Tooltip): Carbon reveals the bubble on hover *and* on focus,
    /// and the bubble is a node only the page can mount.
    fn focused(&mut self, node: Option<&str>) {
        let _ = node;
    }
}

/// Every page, in inventory order. One line per row.
pub fn all() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(accordion::Accordion::default()),
        Box::new(ai_label::AiLabel),
        Box::new(breadcrumb::Breadcrumb),
        Box::new(button::Button),
        Box::new(checkbox::Checkbox::default()),
        Box::new(code_snippet::CodeSnippet::default()),
        Box::new(contained_list::ContainedList::default()),
        Box::new(content_switcher::ContentSwitcher::default()),
        Box::new(data_table::DataTable::default()),
        Box::new(date_picker::DatePicker::default()),
        Box::new(dropdown::Dropdown::default()),
        Box::new(file_uploader::FileUploader),
        Box::new(form::Form),
        Box::new(inline_loading::InlineLoading::default()),
        Box::new(link::Link),
        Box::new(list::List),
        Box::new(loading::Loading::default()),
        Box::new(menu::Menu::default()),
        Box::new(menu_buttons::MenuButtons::default()),
        Box::new(modal::Modal::default()),
        Box::new(notification::Notification::default()),
        Box::new(number_input::NumberInput::default()),
        Box::new(pagination::Pagination::default()),
        Box::new(popover::Popover::default()),
        Box::new(progress_bar::ProgressBar),
        Box::new(progress_indicator::ProgressIndicator),
        Box::new(radio_button::RadioButton::default()),
        Box::new(search::Search::default()),
        Box::new(select::Select::default()),
        Box::new(slider::Slider::default()),
        Box::new(structured_list::StructuredList::default()),
        Box::new(tabs::Tabs::default()),
        Box::new(tag::Tag::default()),
        Box::new(text_input::TextInput::default()),
        Box::new(tile::Tile::default()),
        Box::new(toggle::Toggle::default()),
        Box::new(toggletip::Toggletip::default()),
        Box::new(tooltip::Tooltip::default()),
        Box::new(tree_view::TreeView),
        Box::new(ui_shell_header::UiShellHeader::default()),
        Box::new(ui_shell_left_panel::UiShellLeftPanel::default()),
        Box::new(ui_shell_right_panel::UiShellRightPanel::default()),
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
