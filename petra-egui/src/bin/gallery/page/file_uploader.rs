//! Inventory row 12, File uploader.

use gorgon_petra::component::{file_uploader, file_uploader_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The File uploader page. It holds no live state.
pub struct FileUploader;

impl Page for FileUploader {
    fn row(&self) -> &'static str {
        "File uploader"
    }

    fn body(&self) -> ViewNode {
        section(
            "files",
            "Uploader",
            vec![body(
                "fu",
                sp("spacing.md"),
                vec![
                    // The heading, not the zone prompt. `file_uploader`
                    // draws "Drop files here" inside the zone itself, so
                    // passing that same string here printed it twice on
                    // `12-file-uploader.png`, once as a heading and once
                    // in the box below it.
                    file_uploader("fu", "Upload a trace"),
                    file_uploader_item("fu-0", "trace.ndjson", true),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
