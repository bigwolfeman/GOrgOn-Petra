//! Inventory row 12, File uploader.

use gorgon_petra::component::{
    file_uploader_item, file_uploader_item_edit, file_uploader_item_invalid, file_uploader_with,
    section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp, wrapped};

/// The two files the page starts holding. Each row's key is its index, so
/// a press on `fu-0/remove` names the row it landed on and nothing else.
const START: [&str; 2] = ["trace.ndjson", "fiber-dump.ndjson"];

/// Live state of the File uploader page: which files are still listed.
///
/// The list is real. Pressing a row's remove control drops that row and
/// nothing else, which is the only part of this component a person can
/// drive today — see the section caption, and this row's Agent Note, for
/// the seam a native picker and an OS drop still need.
pub struct FileUploader {
    files: Vec<String>,
}

impl Default for FileUploader {
    fn default() -> Self {
        Self {
            files: START.iter().map(|name| (*name).to_owned()).collect(),
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
        rows.push(wrapped(
            "note",
            "Pressing a row's remove control drops that row. Pressing the \
             zone opens nothing: a page cannot ask the host for a file \
             dialog yet, and an OS drop has no channel to arrive on.",
        ));
        section(
            "files",
            "Uploader",
            vec![body("fu-body", sp("spacing.md"), rows)],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if !path_has(node, "remove") {
            return false;
        }
        for index in 0..self.files.len() {
            if path_has(node, &format!("fu-{index}")) {
                self.files.remove(index);
                return true;
            }
        }
        false
    }
}
