//! Inventory row 6, Code snippet.

use gorgon_petra::component::{code_snippet, code_snippet_inline, code_snippet_multi, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

/// The single-line sample. A const because the Copy button has to hand back
/// the same string the well shows, and two copies of it would drift.
const SINGLE: &str = "pcargo test -p gorgon-petra --lib";
/// Key of the single-line snippet.
const SNIP: &str = "snip";
/// Key of the multi-line snippet.
const SNIP_MULTI: &str = "snip-multi";
/// Key of the copy control inside either snippet (`code_snippet.rs`).
const COPY: &str = "copy";

/// The multi-line sample: the gates this catalog is held to, as the shell
/// runs them. Long enough that the 288-tall well has lines to show.
const MULTI: &str = "\
# The gates, in the order the wave brief lists them.
pcargo test -p gorgon-petra --lib
pcargo test -p gorgon-petra-egui --lib
pcargo test -p gorgon-petra-egui --bin gallery
pcargo test -p gorgon-petra --test anchored_determinism
pcargo test -p gorgon-petra --test incremental_frames
pcargo fmt --all -- --check
pcargo clippy --workspace --all-targets -- -D warnings
pcargo xtask verify-notes

# Then look at the pictures.
PETRA_SHOT_DIR=/tmp/wave pcargo test -p gorgon-petra-egui --bin gallery \\
    every_built_page_rasterizes_to_more_than_one_colour
magick /tmp/wave/06-code-snippet.png -crop 900x700+480+240 +repage \\
    -filter point -resize 400% /tmp/c.png";

/// The Code snippet page.
///
/// The only state is the string a press on Copy has queued. The operator:
/// *"copy button does not work"* — it did not, because `handle` returned
/// `false` for every activation and a page holds no handle to the window.
/// `Page::clipboard_request` is that channel; the host answers it with
/// `egui::Context::copy_text`.
#[derive(Default)]
pub struct CodeSnippet {
    /// Taken by the chrome on the pass that handled the press.
    pending: Option<String>,
}

impl Page for CodeSnippet {
    fn row(&self) -> &'static str {
        "Code snippet"
    }

    fn body(&self) -> ViewNode {
        section(
            "snippets",
            "Single, multi-line, inline",
            vec![filled_body(
                "code",
                sp("spacing.md"),
                vec![
                    code_snippet(SNIP, SINGLE),
                    code_snippet_multi(SNIP_MULTI, MULTI),
                    code_snippet_inline("snip-in", "cargo xtask gates"),
                ],
            )],
        )
    }

    fn clipboard_request(&mut self) -> Option<String> {
        self.pending.take()
    }

    /// Copy puts the well's own text on the clipboard.
    ///
    /// Matched on both segments, because both snippets name their control
    /// `copy` and only the snippet's own key says which well was pressed.
    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if !path_has(node, COPY) {
            return false;
        }
        let text = if path_has(node, SNIP_MULTI) {
            MULTI
        } else if path_has(node, SNIP) {
            SINGLE
        } else {
            return false;
        };
        self.pending = Some(text.to_owned());
        true
    }
}
