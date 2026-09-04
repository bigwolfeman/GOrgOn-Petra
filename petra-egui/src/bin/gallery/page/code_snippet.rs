//! Inventory row 6, Code snippet.

use gorgon_petra::component::{code_snippet, code_snippet_inline, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Code snippet page. It holds no live state.
pub struct CodeSnippet;

impl Page for CodeSnippet {
    fn row(&self) -> &'static str {
        "Code snippet"
    }

    fn body(&self) -> ViewNode {
        section(
            "snippets",
            "Single, inline",
            vec![body(
                "code",
                sp("spacing.md"),
                vec![
                    code_snippet("snip", "pcargo test -p gorgon-petra --lib"),
                    code_snippet_inline("snip-in", "Role::List"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
