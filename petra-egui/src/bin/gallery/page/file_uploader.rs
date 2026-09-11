//! Inventory row 12, File uploader.

use gorgon_petra::component::{
    file_uploader_item, file_uploader_item_edit, file_uploader_item_invalid,
    file_uploader_item_warning, file_uploader_with, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp, wrapped};
use gorgon_petra_egui::host::FilePick;

/// The two files the page starts holding. Each row's key is its index, so
/// a press on `fu-0/remove` names the row it landed on and nothing else.
const START: [&str; 2] = ["trace.ndjson", "fiber-dump.ndjson"];

/// The extensions the zone's dialog offers, Carbon's `accept` prop. A hint
/// to the portal and never a guarantee, so [`FileUploader::files_dropped`]
/// still takes whatever it is handed.
const ACCEPTS: [&str; 2] = ["ndjson", "yaml"];

/// Live state of the File uploader page: which files are still listed, and
/// whether the zone has asked for a dialog.
///
/// The list is real in both directions now. Pressing a row's remove control
/// drops that row; pressing the zone opens the system file dialog, and the
/// files that come back — from the dialog or from a drop on the window —
/// join the list.
pub struct FileUploader {
    files: Vec<String>,
    /// Set by a press on the zone, taken by the chrome on the same pass.
    /// Take-and-clear, the shape `clipboard_request` established: a request
    /// that outlived its pass would reopen the dialog every frame.
    wants_dialog: bool,
}

impl Default for FileUploader {
    fn default() -> Self {
        Self {
            files: START.iter().map(|name| (*name).to_owned()).collect(),
            wants_dialog: false,
        }
    }
}

impl Page for FileUploader {
    fn row(&self) -> &'static str {
        "File uploader"
    }

    fn body(&self) -> ViewNode {
        let mut rows: Vec<ViewNode> = vec![
            // The heading, not the zone prompt. `file_uploader` draws
            // "Drop files here or click to upload" inside the zone itself,
            // so passing that same string here printed it twice on
            // `12-file-uploader.png`, once as a heading and once in the box.
            file_uploader_with("fu", "Upload a trace", "NDJSON or YAML, up to 5 MB"),
        ];
        for (index, name) in self.files.iter().enumerate() {
            rows.push(file_uploader_item_edit(format!("fu-{index}"), name.clone()));
        }
        rows.push(file_uploader_item("fu-busy", "kernel.yaml", false));
        rows.push(file_uploader_item("fu-done", "notes.txt", true));
        rows.push(file_uploader_item_invalid(
            "fu-bad",
            "core.dump",
            "File is over 5 MB",
        ));
        rows.push(file_uploader_item_warning(
            "fu-warn",
            "stale.ndjson",
            "older than the session",
        ));
        rows.push(wrapped(
            "note",
            "Pressing a row's remove control drops that row. Pressing the \
             zone opens the system file dialog, and a file dropped on the \
             window arrives the same way — both reach the page as paths.",
        ));
        section(
            "files",
            "Uploader",
            vec![body("fu-body", sp("spacing.md"), rows)],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // The two branches are disjoint by construction: `path_has` matches
        // whole segments, a remove control's path ends `fu-<n>/remove`, and
        // the zone's ends `fu/zone`. Neither key appears in the other path,
        // so the order here is readability and nothing else.
        if path_has(node, "remove") {
            for index in 0..self.files.len() {
                if path_has(node, &format!("fu-{index}")) {
                    self.files.remove(index);
                    return true;
                }
            }
            return false;
        }
        if path_has(node, "zone") {
            self.wants_dialog = true;
            return true;
        }
        false
    }

    fn file_request(&mut self) -> Option<FilePick> {
        if !std::mem::take(&mut self.wants_dialog) {
            return None;
        }
        Some(FilePick {
            multiple: true,
            extensions: ACCEPTS.iter().map(|ext| (*ext).to_owned()).collect(),
        })
    }

    /// Names, not paths. The list draws a file name and the operator's home
    /// directory is not this catalog's to put on screen; a path with no file
    /// name at all (a directory, `..`) is dropped rather than shown as an
    /// empty row.
    fn files_dropped(&mut self, paths: &[std::path::PathBuf]) {
        for path in paths {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                self.files.push(name.to_owned());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileUploader;
    use crate::page::Page;
    use gorgon_petra::geom::Point;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    /// A press on one node path, the way the chrome routes one.
    fn press(page: &mut FileUploader, node: &str) -> bool {
        page.handle(
            &InputEvent::PointerPressed {
                pos: Point::ZERO,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            node,
        )
    }

    /// Pressing the drop zone asks the host for a dialog, exactly once.
    ///
    /// The dialog itself cannot be driven — it is the operator's window
    /// manager — so what is asserted is the request that reaches the host,
    /// and that it is **taken and cleared**. A request that outlived its
    /// pass would reopen the picker on every frame, which is a bug a person
    /// discovers by being unable to close it.
    ///
    /// Read off the page rather than the raster on purpose: opening a
    /// portal dialog changes nothing about what Petra draws, so there is no
    /// picture to read, and pretending otherwise would be the theatre this
    /// repo bans.
    #[test]
    fn pressing_the_drop_zone_asks_for_a_file_dialog_once() {
        let mut page = FileUploader::default();
        assert!(
            page.file_request().is_none(),
            "a page that has not been pressed is asking for a dialog"
        );

        let pressed = press(&mut page, "/page/files/fu-body/fu/zone");
        assert!(pressed, "the press on the zone was not consumed");

        let pick = page.file_request().expect("the press asked for no dialog");
        assert!(pick.multiple, "Carbon's uploader takes more than one file");
        assert!(
            pick.extensions.iter().any(|ext| ext == "ndjson"),
            "the dialog offers no filter for the format this page names: {:?}",
            pick.extensions
        );
        assert!(
            page.file_request().is_none(),
            "the request survived the pass that took it, so the dialog \
             would reopen every frame"
        );
    }

    /// A press on a row's remove control drops the row it names.
    ///
    /// The loop walks every index and matches `fu-<index>` against the path,
    /// so the bug this pins is the off-by-one family: dropping the first row
    /// whatever was pressed. Pressing the *second* row is what tells those
    /// apart, which is why row 0 is not the one under test.
    #[test]
    fn removing_a_row_drops_the_row_that_was_pressed() {
        let mut page = FileUploader::default();
        let pressed = press(&mut page, "/page/files/fu-body/fu-1/remove");
        assert!(pressed, "the remove press was not consumed");
        assert_eq!(
            page.files,
            vec!["trace.ndjson".to_owned()],
            "pressing the second row's remove control dropped the wrong row"
        );
        assert!(
            page.file_request().is_none(),
            "removing a row also asked the host to open a file dialog"
        );
    }
}
