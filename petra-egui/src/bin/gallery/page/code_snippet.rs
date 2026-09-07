//! Inventory row 6, Code snippet.

use gorgon_petra::component::{
    COPY_FEEDBACK_SECONDS, CodeInk, code_runs, code_snippet, code_snippet_copied,
    code_snippet_inline, code_snippet_multi, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{TextRun, ViewNode};

use super::Page;
use super::common::{filled_body, path_has, sp};

/// The single-line sample. A const because the Copy button has to hand back
/// the same string the well shows, and two copies of it would drift.
const SINGLE: &str = "pcargo test -p gorgon-petra --lib";
/// Key of the single-line snippet.
const SNIP: &str = "snip";
/// Key of the multi-line snippet.
const SNIP_MULTI: &str = "snip-multi";
/// Key of the inline chip, which is a copy control in its own right.
const SNIP_INLINE: &str = "snip-in";
/// The inline sample. A const for the same reason [`SINGLE`] is one: the
/// chip hands this exact string back when it is pressed.
const INLINE: &str = "cargo xtask gates";
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

/// Classify a shell script, line by line, into [`CodeInk`] classes.
///
/// The operator's row-6 line was *"it needs to support colorization for
/// later LSP integration"*. The library half of that is `Props::runs`,
/// `code_runs` and `CodeInk`; this is a stand-in for the half an LSP will
/// eventually do, so the catalog row shows the mechanism carrying real
/// colour rather than a page asserting that it could.
///
/// **It is a classifier, not a parser, and it is deliberately small.** Four
/// rules, which is all this page's sample needs: a line whose first
/// non-blank character is `#` is a comment; a word starting with `-` is a
/// flag; the first word of a command is the command; and a line following
/// one that ended in `\` is a continuation, so its first word is an
/// argument and not a second command. That last rule is here because the
/// picture said so — without it the sample's two wrapped lines wore the
/// command ink on `every_built_page_...` and `-filter`, which is exactly the
/// kind of plausible-but-wrong colouring a highlighter has to be looked at
/// to catch.
///
/// It still knows nothing about quoting, here-docs or substitution, and it
/// is not meant to — the moment an LSP is wired, `shell_runs` is deleted and
/// its `Vec<TextRun>` comes off the wire instead. Nothing else on the page
/// changes, which is the point of the seam.
///
/// The runs tile the string exactly, including the newlines, because tree
/// acceptance refuses anything else
/// (`Violation::TextRunsDoNotCoverTheText`).
fn shell_runs(code: &str) -> Vec<TextRun> {
    let mut runs: Vec<TextRun> = Vec::new();
    let mut push = |ink: CodeInk, len: usize| {
        if len == 0 {
            return;
        }
        // Merge with the run before it when the class is the same, so the
        // list stays as short as the colouring actually is.
        match runs.last_mut() {
            Some(last) if last.foreground == ink.token() => last.len += len,
            _ => runs.push(ink.over(len)),
        }
    };
    let mut continued = false;
    for line in code.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let newline = line.len() - body.len();
        let indent = body.len() - body.trim_start().len();
        push(CodeInk::Plain, indent);
        let rest = &body[indent..];
        let carries_on = body.trim_end().ends_with('\\');
        if rest.starts_with('#') {
            push(CodeInk::Comment, rest.len());
        } else {
            let mut at = 0usize;
            for (word, gap) in words(rest) {
                let ink = if at == 0 && !continued && !word.is_empty() {
                    CodeInk::Keyword
                } else if word.starts_with('-') {
                    CodeInk::Literal
                } else {
                    CodeInk::Plain
                };
                push(ink, word.len());
                push(CodeInk::Plain, gap);
                at += word.len() + gap;
            }
            debug_assert_eq!(at, rest.len(), "the words did not tile the line");
        }
        push(CodeInk::Plain, newline);
        // A comment cannot be continued, whatever it ends with.
        continued = carries_on && !rest.starts_with('#');
    }
    debug_assert_eq!(
        runs.iter().map(|r| r.len).sum::<usize>(),
        code.len(),
        "the runs do not tile the code"
    );
    runs
}

/// `line` split into `(word, following whitespace)` pairs that tile it.
fn words(line: &str) -> Vec<(&str, usize)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < line.len() {
        let rest = &line[at..];
        let word_len = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let after = &rest[word_len..];
        let gap = after.len() - after.trim_start().len();
        out.push((&rest[..word_len], gap));
        at += word_len + gap;
    }
    out
}

/// Which well a press landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Well {
    /// The single-line snippet, keyed [`SNIP`].
    Single,
    /// The multi-line snippet, keyed [`SNIP_MULTI`].
    Multi,
    /// The inline chip, keyed [`SNIP_INLINE`]. It has no copy *button*; the
    /// chip is the control (`component::code_snippet::code_snippet_inline`).
    Inline,
}

/// The Code snippet page.
///
/// Two pieces of state, both about one press on Copy.
///
/// The first is the string it queued. The operator: *"copy button does not
/// work"* — it did not, because `handle` returned `false` for every
/// activation and a page holds no handle to the window.
/// `Page::clipboard_request` is that channel; the host answers it with
/// `egui::Context::copy_text`.
///
/// The second is *which well was pressed and when*, which is round 4's ask:
/// *"when clicking the copy text button it should give me some feed back
/// that it copied"*. The well says `Copied!` for
/// [`COPY_FEEDBACK_SECONDS`] and then stops, so the page has to hold the
/// press time and compare it against the host clock — and it has to ask the
/// host for a pass at the deadline, because an idle Petra window paints
/// nothing and the message would otherwise outlive its own timer. That is
/// [`Page::wake_at`].
///
/// The well is recorded beside the time rather than a bare flag: both wells
/// name their control `copy`, and a flag would light the bubble on whichever
/// one the tree happened to build first.
#[derive(Default)]
pub struct CodeSnippet {
    /// Taken by the chrome on the pass that handled the press.
    pending: Option<String>,
    /// The host clock for the pass now being built.
    now: f64,
    /// Which well was copied, and at what time, while the feedback is up.
    copied: Option<(Well, f64)>,
}

impl CodeSnippet {
    /// Whether `well` is inside its feedback window.
    ///
    /// A press restarts the window rather than extending it, which is what
    /// Carbon's debounced `handleFadeOut` does
    /// (`@carbon/react/lib/components/Copy/Copy.js:37`).
    fn saying_copied(&self, well: Well) -> bool {
        self.copied
            .is_some_and(|(at, when)| at == well && self.now - when < COPY_FEEDBACK_SECONDS)
    }
}

impl Page for CodeSnippet {
    fn row(&self) -> &'static str {
        "Code snippet"
    }

    fn tick(&mut self, now: f64) {
        self.now = now;
    }

    /// The moment the feedback stops being true, or nothing when none is up.
    ///
    /// One deadline, not a repeating request: the pass this wakes finds the
    /// window elapsed, drops the bubble, and answers `None`, so the window
    /// goes straight back to idle.
    fn wake_at(&mut self) -> Option<f64> {
        let (_, when) = self.copied?;
        let ends = when + COPY_FEEDBACK_SECONDS;
        (ends > self.now).then_some(ends)
    }

    fn body(&self) -> ViewNode {
        section(
            "snippets",
            "Single, multi-line, inline",
            vec![filled_body(
                "code",
                sp("spacing.md"),
                vec![
                    code_snippet_copied(
                        code_runs(code_snippet(SNIP, SINGLE), shell_runs(SINGLE)),
                        self.saying_copied(Well::Single),
                    ),
                    code_snippet_copied(
                        code_runs(code_snippet_multi(SNIP_MULTI, MULTI), shell_runs(MULTI)),
                        self.saying_copied(Well::Multi),
                    ),
                    code_snippet_copied(
                        code_snippet_inline(SNIP_INLINE, INLINE),
                        self.saying_copied(Well::Inline),
                    ),
                ],
            )],
        )
    }

    fn clipboard_request(&mut self) -> Option<String> {
        self.pending.take()
    }

    /// Copy puts the well's own text on the clipboard, and the well says so.
    ///
    /// Matched on both segments, because both snippets name their control
    /// `copy` and only the snippet's own key says which well was pressed.
    /// `SNIP_MULTI` is tested first: `snip-multi` is not `snip`, but a
    /// segment test that asked the short question first would answer it for
    /// both wells.
    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // The inline chip is asked about first and is asked a different
        // question: it has no `copy` segment on its path, because it has no
        // copy button — it *is* one. Testing `COPY` before this would answer
        // `false` for it and the chip would be a control that does nothing.
        let (well, text) = if path_has(node, SNIP_INLINE) {
            (Well::Inline, INLINE)
        } else if !path_has(node, COPY) {
            return false;
        } else if path_has(node, SNIP_MULTI) {
            (Well::Multi, MULTI)
        } else if path_has(node, SNIP) {
            (Well::Single, SINGLE)
        } else {
            return false;
        };
        self.pending = Some(text.to_owned());
        self.copied = Some((well, self.now));
        true
    }
}
