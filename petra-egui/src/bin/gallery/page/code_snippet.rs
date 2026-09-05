//! Inventory row 6, Code snippet.

use gorgon_petra::component::{code_snippet, code_snippet_inline, code_snippet_multi, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

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

/// The Code snippet page. It holds no live state.
pub struct CodeSnippet;

impl Page for CodeSnippet {
    fn row(&self) -> &'static str {
        "Code snippet"
    }

    fn body(&self) -> ViewNode {
        section(
            "snippets",
            "Single, multi-line, inline",
            vec![body(
                "code",
                sp("spacing.md"),
                vec![
                    code_snippet("snip", "pcargo test -p gorgon-petra --lib"),
                    code_snippet_multi("snip-multi", MULTI),
                    code_snippet_inline("snip-in", "cargo xtask gates"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
