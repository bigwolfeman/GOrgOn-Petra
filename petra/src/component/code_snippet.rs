//! Carbon Code snippet (slice-a).
//!
//! Three variants, three constructors. No named sm/md/lg scale; each
//! variant has its own fixed numbers (style page Structure; T070 prefers
//! SCSS where they disagree):
//!
//! - [`code_snippet`] — single line, height 40, fill [`SURFACE_RAISED`].
//! - [`code_snippet_multi`] — multi-line, min-height 288.
//! - [`code_snippet_inline`] — inline, height 16, radius [`SHAPE_SM`]
//!   (SCSS 4px; style-page 2px is stale).
//!
//! Ink is [`super::tokens::TEXT_PRIMARY`]. Do not invent syntax colours.
//! Copy is a labelled [`Role::Button`] (`"Copy"`) drawing
//! [`IconMark::Copy`] in [`IconTone::Primary`] (`.cds--snippet__icon`,
//! `fill: $icon-primary`, 16×16); the label is its name, so the glyph is
//! never the only channel (FR-058, FR-026).

use std::sync::Arc;

use super::icon::{IconMark, IconTone, icon_toned};
use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    LINK_PRIMARY, SHAPE_SM, SIZE_MD, SPACING_02, SPACING_03, SPACING_05, SURFACE_LAYER_THREE,
    SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_CODE, t,
};
use super::tooltip::tooltip_anchored;
use crate::geom::{Align, Axis};
use crate::token::TokenName;
use crate::tree::{
    AxisConstraint, Constraints, FocusFigure, FocusShownOn, InsetRefs, Interaction, Justify, Key,
    Role, TextRun, ViewNode,
};

/// Carbon `.cds--snippet--multi` `min-block-size`.
const MULTI_MIN: f32 = 288.0;
/// Carbon `.cds--snippet--inline` container height.
const INLINE_HEIGHT: f32 = 16.0;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MULTI_MIN == 288.0);
const _: () = assert!(INLINE_HEIGHT == 16.0);

const COPY_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click];

/// What a copied snippet says.
///
/// Carbon's `CopyButton`/`Copy` `feedback` prop, whose default is exactly
/// this string (`@carbon/react/lib/components/Copy/Copy.js:30`,
/// `CopyButton.js:30`). It is a **word**, which is the whole point: the
/// operator is red-green colourblind, so a copy control that answered with
/// a fill step would answer him with nothing.
pub const COPY_FEEDBACK: &str = "Copied!";

/// How long a copied snippet says it, in seconds.
///
/// Carbon's `feedbackTimeout` default is `2e3` milliseconds
/// (`@carbon/react/lib/components/Copy/Copy.js:30`). Seconds because that
/// is the unit the host clock reaches an application in
/// (`gorgon_petra_egui::host::App::tick`), and the conversion belongs at the
/// one place the number is written down rather than at each caller.
pub const COPY_FEEDBACK_SECONDS: f64 = 2.0;

/// Key of the feedback bubble [`code_snippet_copied`] mounts.
pub const COPY_FEEDBACK_KEY: &str = "copied";

/// Key of the copy control inside any snippet that has one.
const COPY_KEY: &str = "copy";
/// Key of the glyph inside the copy control, which is what the feedback
/// bubble anchors to. See [`code_snippet_copied`].
const COPY_ICON_KEY: &str = "copy-icon";

const _: () = assert!(COPY_FEEDBACK_SECONDS == 2.0);

/// Single-line snippet. Height 40. Copy button labelled `"Copy"`.
pub fn code_snippet(key: impl Into<Key>, code: impl Into<String>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![code_text(code.into(), true), copy_button()],
    );
    node.props.align = Some(Align::Center);
    // Carbon pins the control to the trailing edge in every variant
    // (`.cds--snippet-button` is absolutely positioned right). Ours sat
    // immediately after the text on the single line and at the top *left* of
    // the multi-line well, which is what the operator meant by "inconsistent
    // placement": the same control in two different corners.
    node.props.justify = Some(Justify::SpaceBetween);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    paint_well(node).with_constraints(pin_height(SIZE_MD))
}

/// Multi-line snippet. Min-height 288. Copy button labelled `"Copy"`.
pub fn code_snippet_multi(key: impl Into<Key>, code: impl Into<String>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Vertical,
        Some(SPACING_03),
        vec![copy_row(), code_text(code.into(), true)],
    );
    node.props.align = Some(Align::Stretch);
    node.props.padding = Some(pad(SPACING_05, SPACING_05));
    paint_well(node).with_constraints(Constraints {
        vertical: AxisConstraint {
            min: Some(MULTI_MIN),
            max: None,
            priority: 0,
        },
        ..Constraints::default()
    })
}

/// Inline snippet. Height 16, radius sm. Display only — no copy button.
pub fn code_snippet_inline(key: impl Into<Key>, code: impl Into<String>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Horizontal,
        None,
        vec![code_text(code.into(), false)],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_03)),
        right: Some(t(SPACING_03)),
        ..InsetRefs::default()
    });
    let mut node = paint_well(node);
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.with_constraints(pin_height(INLINE_HEIGHT))
}

/// What a stretch of code *is*, so the theme can decide what colour it takes.
///
/// The seam the operator asked for on 2026-09-05: *"it needs to support
/// colorization for later LSP integration"*. Three facts have to live
/// somewhere and they belong in three different places:
///
/// 1. **"this stretch is a comment"** — the source knows it, and later an
///    LSP `textDocument/semanticTokens` response says it.
/// 2. **"comments are the muted ink here"** — the design system knows it.
/// 3. **"the muted ink is `#6f6f6f`"** — the theme knows it.
///
/// This enum is the second. It names no colour and invents no token; it
/// chooses among names [`super::tokens`] already ships, the same way
/// [`super::field`] chooses `surface.raised` for a well. A caller classifies
/// (1) and never has to decide (2), so two callers cannot disagree about
/// what a comment looks like.
///
/// **Carbon ships no syntax palette.** `.cds--snippet` sets every variant in
/// one ink and the inventory records no others, so every mapping below is
/// this library's own choice and is a departure by addition rather than by
/// contradiction. Four classes because that is what the catalog's own sample
/// needs; an LSP's two dozen semantic types map onto them until there is a
/// reason to grow the set, and growing it is adding a variant here rather
/// than teaching every caller a new token name.
///
/// The four are told apart by **lightness as well as hue** — muted grey,
/// white, blue, and the error tone — because the operator is red-green
/// colourblind and a palette separated only by hue would be one channel he
/// does not have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CodeInk {
    /// Ordinary code: the snippet's own ink, whatever the well decided.
    #[default]
    Plain,
    /// A comment. [`TEXT_MUTED`] — quieter than the code around it, which is
    /// the one thing every syntax theme agrees on.
    Comment,
    /// A keyword, a command name, the word that says what the line does.
    /// [`LINK_PRIMARY`], the ink this library already spends on "the
    /// actionable word".
    Keyword,
    /// A literal — a string, a number, a flag's value. [`TEXT_PRIMARY`], so
    /// it reads as the brightest thing on the line.
    Literal,
}

impl CodeInk {
    /// The token this class takes, or `None` for the snippet's own ink.
    #[must_use]
    pub fn token(self) -> Option<TokenName> {
        match self {
            Self::Plain => None,
            Self::Comment => Some(t(TEXT_MUTED)),
            Self::Keyword => Some(t(LINK_PRIMARY)),
            Self::Literal => Some(t(TEXT_PRIMARY)),
        }
    }

    /// This class over `len` bytes, as a run [`code_runs`] takes.
    #[must_use]
    pub fn over(self, len: usize) -> TextRun {
        TextRun {
            len,
            foreground: self.token(),
        }
    }
}

/// Lay colour runs over a snippet's code, for syntax highlighting.
///
/// Takes any of the three snippet constructors and reaches the node keyed
/// `"code"` inside it — by key, not by kind, so it can never land on a copy
/// button's label. Returns the node unchanged if there is none.
///
/// # This library ships no syntax palette, and this function does not invent
/// one
///
/// Carbon has no syntax colours: `.cds--snippet` sets every variant in one
/// ink, and the inventory records none. So the runs come from the **caller**
/// and name tokens the caller's theme declares. That is not a gap left open
/// by accident — it is the seam the operator asked for, *"support
/// colorization for later LSP integration"*. An LSP's
/// `textDocument/semanticTokens` response is a token type per stretch; a
/// consumer maps those types to theme token names and hands the result here.
/// Nothing in this file has to know what a keyword is.
///
/// A run naming no token takes the snippet's own ink, so a highlighter names
/// only the stretches it has an opinion about.
///
/// The runs must tile the code exactly; a list that does not is refused by
/// tree acceptance ([`crate::tree::Violation::TextRunsDoNotCoverTheText`])
/// rather than silently recoloured.
#[must_use]
pub fn code_runs(mut node: ViewNode, runs: Vec<TextRun>) -> ViewNode {
    fn fill(node: &mut ViewNode, runs: &[TextRun]) -> bool {
        if node.key.as_str() == "code" {
            node.props.runs = runs.to_vec();
            return true;
        }
        for child in &mut node.children {
            let mut owned = Arc::unwrap_or_clone(Arc::clone(child));
            if fill(&mut owned, runs) {
                *child = Arc::new(owned);
                return true;
            }
        }
        false
    }
    fill(&mut node, &runs);
    node
}

/// Say a snippet was copied, or stop saying it.
///
/// Takes any of the three snippet constructors, the way [`code_runs`] does,
/// and reaches the control keyed `"copy"` inside it. A snippet with no copy
/// control — the inline variant — comes back unchanged.
///
/// # What Carbon does
///
/// Pressing `CopyButton` shows a tooltip reading [`COPY_FEEDBACK`] for
/// [`COPY_FEEDBACK_SECONDS`] and swaps the button's own accessible name to
/// the same string (`@carbon/react/lib/components/Copy/Copy.js:30,56,61`).
/// The bubble is not a component of its own over there either: it is the
/// tooltip caret and content mixins on a `<span>` **inside** the button
/// (`@carbon/styles/scss/components/copy-button/_copy-button.scss:44,50`),
/// which is why this mounts it as the button's own child rather than beside
/// it.
///
/// Both channels move together, and that is deliberate rather than
/// belt-and-braces. The operator this library is built for is red-green
/// colourblind: feedback carried by a tint is feedback he does not receive,
/// so the affordance is a word on the screen and the same word in the
/// accessible name, and neither is a decoration of the other.
///
/// # Why the button and not the snippet
///
/// A [`crate::tree::NodeKind::Surface`] measures `Size::ZERO`, but a stack
/// still counts it when it spreads a [`Justify::SpaceBetween`] row's
/// leftover into the gaps (`layout::stack`, `gaps = children - 1`). Hung off
/// the single-line snippet the bubble would therefore be a third child of a
/// two-child row, and the copy control the operator just pressed would jump
/// to the middle of the well the moment it answered him. Inside the button
/// the row has no spacing and no justify, so the zero-size child moves
/// nothing — and it is where Carbon puts it anyway.
///
/// The bubble anchors to the glyph rather than to the button, because
/// [`crate::tree::Anchor::Sibling`] resolves among siblings and the glyph is
/// the one sibling it has. The two are concentric — the glyph is the only
/// thing in the button, centred — so the bubble hangs under the middle of
/// the control either way.
#[must_use]
pub fn code_snippet_copied(mut node: ViewNode, copied: bool) -> ViewNode {
    if !copied {
        return node;
    }
    fn mark(node: &mut ViewNode) -> bool {
        if node.key.as_str() == COPY_KEY {
            node.semantics.label = Some(COPY_FEEDBACK.to_owned());
            node.children.push(Arc::new(tooltip_anchored(
                COPY_FEEDBACK_KEY,
                COPY_ICON_KEY,
                COPY_FEEDBACK,
            )));
            return true;
        }
        for child in &mut node.children {
            let mut owned = Arc::unwrap_or_clone(Arc::clone(child));
            if mark(&mut owned) {
                *child = Arc::new(owned);
                return true;
            }
        }
        false
    }
    mark(&mut node);
    node
}

/// What the well's code run declares so the operator can select inside it.
///
/// `Drag` is what wins the pointer capture — a selection *is* a
/// press-move-release, and `crate::input::PointerState` hit-tests for exactly
/// this intent before granting one. `Focus` and `Key` are what make the copy
/// chord reachable: a selection nobody can copy is decoration.
const CODE_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Drag, Interaction::Key];

/// The accessible name of the well's code run.
///
/// Carbon's container falls back to the literal string `"code-snippet"`
/// (`@carbon/react/lib/components/CodeSnippet/CodeSnippet.js:124`); this is
/// the same name written the way a person reads it.
const CODE_LABEL: &str = "Code snippet";

fn code_text(code: String, selectable: bool) -> ViewNode {
    let mut node = text("code", code);
    // Carbon sets every snippet in `$code-01` / `$code-02`, which is IBM
    // Plex Mono. This bound nothing until 2026-09-05, so the catalog's code
    // snippet was set in the sans body face -- a code snippet whose columns
    // do not line up, which is the first thing a reader notices and the
    // hardest to name. `typography.code` is the ramp's one MONO step and it
    // shipped from the start.
    node.props.style = Some(t(TYPOGRAPHY_CODE));
    if selectable {
        selectable_code(node)
    } else {
        node
    }
}

/// Declare a code run selectable, and say what a selection looks like on it.
///
/// The operator, round 4: *"code snippet: I cant highlight text inside the
/// code snippet blocks"*. Carbon's answer is that the snippet container is a
/// read-only `textbox` the browser lets you drag across
/// (`@carbon/react/lib/components/CodeSnippet/CodeSnippet.js:120-124`:
/// `role="textbox"`, `tabIndex=0`, `aria-readonly`). Petra has no browser
/// under it, so the same three facts have to be declared:
///
/// 1. **The intents** ([`CODE_INTENTS`]). Without `Drag` a press on the code
///    never wins the capture and the drag is somebody else's.
/// 2. **The role, the name and the read-only flag**, which is FR-058's
///    requirement for any node that declares an interaction and is also
///    Carbon's own trio.
/// 3. **What a selection is painted in.** The engine carries the selected
///    byte range and the host fills a rectangle behind those glyphs; which
///    colour it fills is a design-system decision and so it is named here, in
///    the component, never in the painter.
///
/// # Why a selection is two tokens and not one
///
/// A ground dark enough to see moves the ground the ink was measured
/// against. `link-primary` — the keyword class — clears AA on
/// `surface.raised` in the light theme at 4.63:1 against a 4.5 floor, so
/// **every** visible highlight sinks it there, and a ground pale enough to
/// keep it is a ground nobody can see. So the selected stretch takes
/// [`TEXT_PRIMARY`] for as long as it is selected, which is what
/// `::selection { color }` does in a browser: the syntax colouring is
/// suspended inside the selection rather than being made illegible under it.
/// `snippet_selection_is_legible_in_both_themes` measures the pair.
///
/// # Why the ground is the top of the ramp and not `layer-selected`
///
/// `layer-selected` is the obvious name and it does not work, measured on
/// the catalog: the well binds `surface.raised`, [`super::on_layer`] re-seats
/// it to `surface.layer-two` (`#333333` dark) because every component in the
/// catalog sits on a card, and `layer-selected` is a step off layer *one* —
/// `#313131`. Two of 255 apart from the ground it would sit on, which is the
/// same invisible-highlight defect the tag's selected fill was found to have.
/// A relative name that followed the re-seat would be the real fix and it is
/// not a slot: `on_layer` rewrites `background` and nothing else, by design.
///
/// So this takes [`super::tooltip`]'s answer to the same problem, for the
/// same reason it gave: a surface that is dragged over whatever happens to be
/// under it cannot know what that is, and the ramp's last rung is a step away
/// from every rung below it. `#444444` on `#333333` is seventeen of 255.
///
/// **The limit that comes with it, stated rather than discovered later:** a
/// well already seated at the top of the ramp gets no visible highlight. That
/// is `on_layer`'s own documented four-step limit — *"a component that stacks
/// three surfaces of its own and is then mounted one step up loses its
/// topmost separation"* — and nothing at this layer can paper over it.
fn selectable_code(mut node: ViewNode) -> ViewNode {
    node.props
        .tokens
        .insert("selection".into(), t(SURFACE_LAYER_THREE));
    node.props
        .tokens
        .insert("selection-ink".into(), t(TEXT_PRIMARY));
    let mut node = node.interactive(Role::TextInput, CODE_LABEL, CODE_INTENTS);
    // The run holds focus; the well shows it. See `paint_well`.
    node.semantics.focus_shown_on = FocusShownOn::OnWell;
    // The same figure the well declares. The host reads the figure off the
    // node focus is *shown on*, so this line draws nothing — but a reader
    // finding `OnWell` here and no figure has to go and look at the well to
    // learn what shape this control wears, and a reader finding a
    // *different* figure here would be told something false.
    // `a_control_and_the_node_it_shows_focus_on_agree_about_the_figure`
    // holds the pair together.
    node.semantics.focus_figure = FocusFigure::Sides;
    // Carbon's `aria-readonly`. A code well takes a selection and a copy and
    // never a keystroke, and a reader told it is an editable field would be
    // told something false about every one of these forty-two rows.
    node.semantics.read_only = true;
    node
}

/// The multi-line well's copy control, pushed to the trailing edge.
///
/// A row with the button and `Justify::End`, rather than the button alone:
/// Carbon positions `.cds--snippet-button` absolutely at the top right, and
/// the nearest thing this layout engine has is a full-width row that spends
/// its leftover before its one child.
fn copy_row() -> ViewNode {
    let mut row = stack("copy-row", Axis::Horizontal, None, vec![copy_button()]);
    row.props.justify = Some(Justify::End);
    row
}

fn copy_button() -> ViewNode {
    let mut node = stack(
        COPY_KEY,
        Axis::Horizontal,
        None,
        vec![icon_toned(COPY_ICON_KEY, IconMark::Copy, IconTone::Primary)],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_03, SPACING_02));
    node.interactive(Role::Button, "Copy", COPY_INTENTS)
        // A button: `Sides`, per the operator's rule. See `component::button`.
        .with_focus_figure(FocusFigure::Sides)
}

fn paint_well(mut node: ViewNode) -> ViewNode {
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    // The well is the hull the code run's focus is shown on. Carbon treats
    // focus inside a snippet at the container: `.cds--snippet--single
    // :focus-within .cds--snippet-container` (`_code-snippet.scss:500`), and
    // its `focus-outline('outline')` calls sit on the snippet and its
    // buttons, never on the text run inside.
    //
    // Without this the run rings itself, because `FocusFigure::BarUnder` is
    // the default and the run is focusable — it declares `Interaction::Drag`
    // so a selection gesture can reach it. The picture was a blue box drawn
    // around the whole line at the moment the operator dragged across part
    // of it, fighting the selection band inside it.
    node.semantics.focus_shown_on = FocusShownOn::Well;
    // `Sides`, the same figure `component::field` wears: a code well is a
    // region a person drags a text selection through, so it should read as a
    // well and not as a pressed control. The bar under would also collide
    // with the multi-line well's own expand row.
    node.semantics.focus_figure = FocusFigure::Sides;
    node
}

fn pin_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        COPY_FEEDBACK, COPY_FEEDBACK_KEY, COPY_FEEDBACK_SECONDS, CodeInk, INLINE_HEIGHT, MULTI_MIN,
        SHAPE_SM, SIZE_MD, SURFACE_LAYER_THREE, SURFACE_RAISED, code_runs, code_snippet,
        code_snippet_copied, code_snippet_inline, code_snippet_multi,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        descendant(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn descendant<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children
            .iter()
            .find_map(|child| descendant(child, key))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn code_snippet_exists_at_height_forty() {
        let node = code_snippet("s", "fn main() {}");
        assert_eq!(node.key.as_str(), "s");
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert!(!node.is_interactive());
        assert_eq!(
            named(&node, "code").props.text.as_deref(),
            Some("fn main() {}")
        );
    }

    #[test]
    fn code_snippet_copy_is_a_labelled_button() {
        let node = code_snippet("s", "let x = 1;");
        let copy = named(&node, "copy");
        assert_eq!(copy.semantics.role, Some(Role::Button));
        assert_eq!(copy.semantics.label.as_deref(), Some("Copy"));
        let glyph = named(copy, "copy-icon");
        assert_eq!(glyph.kind, crate::tree::NodeKind::Canvas);
        assert_eq!(
            glyph.props.text, None,
            "the copy control is a glyph, not the word"
        );
        assert!(copy.interactions.contains(&Interaction::Focus));
        assert!(copy.interactions.contains(&Interaction::Click));
        assert!(copy.is_interactive());
    }

    #[test]
    fn code_snippet_multi_has_min_height_288() {
        let node = code_snippet_multi("s", "line 1\nline 2");
        assert_eq!(node.constraints.vertical.min, Some(MULTI_MIN));
        assert_eq!(node.constraints.vertical.max, None);
        assert_eq!(MULTI_MIN, 288.0);
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        named(&node, "copy");
        named(&node, "code");
    }

    #[test]
    fn code_snippet_inline_is_sixteen_tall_with_sm_radius() {
        let node = code_snippet_inline("s", "ViewNode");
        assert_eq!(node.constraints.vertical.min, Some(INLINE_HEIGHT));
        assert_eq!(node.constraints.vertical.max, Some(INLINE_HEIGHT));
        assert_eq!(INLINE_HEIGHT, 16.0);
        assert_eq!(token(&node, "radius"), Some(SHAPE_SM));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert!(!node.is_interactive());
        assert!(
            node.children
                .iter()
                .all(|child| child.semantics.role != Some(Role::Button)),
            "inline snippet has no copy button"
        );
    }

    #[test]
    fn code_snippet_does_not_invent_syntax_colours() {
        let node = code_snippet("s", "fn main() {}");
        let code = named(&node, "code");
        assert_eq!(
            token(code, "foreground"),
            Some(super::super::tokens::TEXT_PRIMARY)
        );
        assert!(
            code.props
                .tokens
                .keys()
                .all(|k| k != "syntax" && !k.starts_with("syntax.")),
            "no invented syntax colour slots"
        );
        assert!(
            code.props.runs.is_empty(),
            "a bare snippet carries no runs; colour is something a caller adds"
        );

        // And the palette `CodeInk` does ship invents nothing either: every
        // class resolves to a name the shipped vocabulary already declares.
        // This is the check that stops the enum growing a colour of its own.
        let vocabulary = standard_vocabulary();
        for ink in [
            CodeInk::Plain,
            CodeInk::Comment,
            CodeInk::Keyword,
            CodeInk::Literal,
        ] {
            let Some(name) = ink.token() else { continue };
            assert!(
                vocabulary.names().any(|declared| declared == &name),
                "{ink:?} names {}, which no shipped theme declares",
                name.as_str()
            );
        }
    }

    /// `code_runs` reaches the code and nothing else.
    ///
    /// By key, not by kind: a snippet's copy button carries a `label` text
    /// node, and a modifier that took the first `NodeKind::Text` it found
    /// would colour the word "Copy" on the single-line and multi-line forms.
    #[test]
    fn code_runs_colours_the_code_and_never_the_copy_label() {
        let runs = vec![CodeInk::Comment.over(2), CodeInk::Plain.over(2)];
        for label in ["single", "multi", "inline"] {
            let bare = match label {
                "single" => code_snippet("s", "abcd"),
                "multi" => code_snippet_multi("s", "abcd"),
                _ => code_snippet_inline("s", "abcd"),
            };
            let node = code_runs(bare, runs.clone());
            assert_eq!(
                named(&node, "code").props.runs,
                runs,
                "{label}: the runs did not reach the code"
            );
            if let Some(caption) = descendant(&node, "label") {
                assert!(
                    caption.props.runs.is_empty(),
                    "{label}: the copy button's own caption was coloured"
                );
            }
        }

        // Nothing keyed `code` means nothing to colour, and that is a no-op
        // rather than a panic, the same as `valued` on a tree with no input.
        let plain = crate::component::text("t", "abcd");
        assert_eq!(code_runs(plain.clone(), runs), plain);
    }

    /// A copied snippet says so in words, in both channels.
    ///
    /// Carbon's `feedback` default is `"Copied!"` and its `feedbackTimeout`
    /// default is 2000 ms
    /// (`@carbon/react/lib/components/Copy/Copy.js:30`); the button's own
    /// accessible name becomes the same string while it is up
    /// (`Copy.js:56,61`). Both are asserted, because the operator is
    /// red-green colourblind and a control that answered only in a tint
    /// would not have answered him at all.
    #[test]
    fn a_copied_snippet_says_copied_in_the_picture_and_in_its_name() {
        assert_eq!(COPY_FEEDBACK, "Copied!");
        assert!((COPY_FEEDBACK_SECONDS - 2.0).abs() < f64::EPSILON);

        for (label, bare) in [
            ("single", code_snippet("s", "fn main() {}")),
            ("multi", code_snippet_multi("s", "line 1\nline 2")),
        ] {
            let resting = code_snippet_copied(bare.clone(), false);
            assert_eq!(
                resting, bare,
                "{label}: a snippet nobody copied must come back untouched"
            );
            assert_eq!(
                named(&resting, "copy").semantics.label.as_deref(),
                Some("Copy")
            );
            assert!(
                descendant(&resting, COPY_FEEDBACK_KEY).is_none(),
                "{label}: a snippet nobody copied says nothing"
            );

            let said = code_snippet_copied(bare, true);
            let copy = named(&said, "copy");
            let bubble = descendant(copy, COPY_FEEDBACK_KEY)
                .unwrap_or_else(|| panic!("{label}: no feedback bubble under the copy control"));
            assert_eq!(
                named(bubble, "body").props.text.as_deref(),
                Some(COPY_FEEDBACK),
                "{label}: the bubble carries Carbon's own feedback string"
            );
            assert_eq!(
                copy.semantics.label.as_deref(),
                Some(COPY_FEEDBACK),
                "{label}: and so does the accessible name, as Carbon's does"
            );
            assert!(
                bubble.interactions.is_empty(),
                "{label}: the feedback is a message, not a control"
            );
        }
    }

    /// The well's code run declares everything a selection gesture needs.
    ///
    /// Four facts, and the gesture is dead without any one of them:
    /// `Interaction::Drag` wins the press its capture, `Focus` and `Key` make
    /// the copy chord reachable, the role and name satisfy FR-058 and match
    /// Carbon's own read-only `textbox`, and the two token slots are the
    /// design system saying what a selection looks like — the painter draws
    /// nothing without them and never invents a colour of its own.
    #[test]
    fn the_code_run_declares_what_a_selection_needs() {
        for (label, node) in [
            ("single", code_snippet("s", "fn main() {}")),
            ("multi", code_snippet_multi("s", "line 1\nline 2")),
        ] {
            let code = named(&node, "code");
            assert_eq!(code.semantics.role, Some(Role::TextInput), "{label}");
            assert_eq!(
                code.semantics.label.as_deref(),
                Some("Code snippet"),
                "{label}"
            );
            assert!(
                code.semantics.read_only,
                "{label}: a code well takes a selection and never a keystroke"
            );
            for intent in [Interaction::Drag, Interaction::Focus, Interaction::Key] {
                assert!(
                    code.interactions.contains(&intent),
                    "{label}: the code run does not declare {intent:?}, so \
                     the selection gesture never reaches it"
                );
            }
            assert_eq!(
                token(code, "selection"),
                Some(SURFACE_LAYER_THREE),
                "{label}: no selection ground, so the painter draws no highlight"
            );
            assert_eq!(
                token(code, "selection-ink"),
                Some(super::super::tokens::TEXT_PRIMARY),
                "{label}: no selection ink, and the pair is never used by halves"
            );
        }

        // The inline form is a `<span>` in Carbon with no role and no tab
        // stop, and it is not a control here either.
        let inline = code_snippet_inline("s", "ViewNode");
        let code = named(&inline, "code");
        assert!(
            code.interactions.is_empty(),
            "the inline run is not a control"
        );
        assert_eq!(token(code, "selection"), None);
    }

    /// The code run holds focus and the **well** shows it.
    ///
    /// Carbon treats focus inside a snippet at the container:
    /// `.cds--snippet--single:focus-within .cds--snippet-container`
    /// (`_code-snippet.scss:500`), and every `focus-outline('outline')` in
    /// that file is on the snippet or one of its buttons, never on the text
    /// run.
    ///
    /// This is not decoration. The run is focusable — it declares
    /// `Interaction::Drag` so a selection gesture can reach it — so without
    /// the pointing the run marks *itself*. The picture that produced was a
    /// figure drawn on the whole line at the moment a fragment of it was
    /// dragged over, fighting the selection band inside it. `paint.rs` takes
    /// the figure from the node focus is shown on, so pointing the run at
    /// its well is what puts the mark on the container.
    ///
    /// # How this goes red
    ///
    /// Drop either assignment in `selectable_code` or `paint_well`.
    #[test]
    fn a_code_run_shows_its_focus_on_the_well_and_never_on_itself() {
        for (label, node) in [
            ("single", code_snippet("s", "pcargo test")),
            ("multi", code_snippet_multi("m", "pcargo test")),
        ] {
            assert_eq!(
                node.semantics.focus_shown_on,
                crate::tree::FocusShownOn::Well,
                "{label}: the well is not the hull, so a descendant \
                 pointing at it has nothing to be shown on"
            );
            let code = named(&node, "code");
            assert_eq!(
                code.semantics.focus_shown_on,
                crate::tree::FocusShownOn::OnWell,
                "{label}: the code run shows focus on itself, so dragging a \
                 selection across it rings the whole line"
            );
        }
    }

    /// The selection ground is a step a reader can see, in both themes, and
    /// the ink on it clears AA for every class the highlighter can produce.
    ///
    /// This is the measurement that chose the pair. `layer-selected` is the
    /// obvious ground and it lands two of 255 from the re-seated well; and a
    /// ground that *is* visible drops `link-primary` under AA in the light
    /// theme, which is why the selected run takes its own ink at all.
    #[test]
    fn snippet_selection_is_legible_in_both_themes() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        // The catalog seats every component one layer up, so the well the
        // highlight sits on is the ramp's second rung, not `surface.raised`.
        const SEATED_WELL: &str = "surface.layer-two";
        for theme in [crate::token::light(), crate::token::dark()] {
            let ground = color(&theme, SURFACE_LAYER_THREE);
            let ink = color(&theme, super::super::tokens::TEXT_PRIMARY);
            let ratio = ink.over(ground).contrast_ratio(ground);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "selected text reads at {ratio:.2}:1 on its own highlight, \
                 under the {MIN_TEXT_CONTRAST}:1 floor"
            );
            // And the band is a luminance step off the well, which is the
            // operator's channel: he is red-green colourblind, so a
            // highlight told apart by hue is one he does not receive.
            let well = color(&theme, SEATED_WELL);
            let step = (ground.relative_luminance() - well.relative_luminance()).abs();
            assert!(
                step > 0.01,
                "the highlight is {step:.4} of relative luminance from the \
                 well it sits on, which is not a step a reader can use"
            );
        }
    }

    /// The inline snippet has no copy control, so there is nothing to say.
    #[test]
    fn an_inline_snippet_has_no_copy_control_and_no_feedback() {
        let bare = code_snippet_inline("s", "ViewNode");
        assert_eq!(code_snippet_copied(bare.clone(), true), bare);
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D across all three variants: no degenerate rect, no child
    /// placed outside its parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("single", code_snippet("s", "fn main() {}")),
            ("multi", code_snippet_multi("s", "line 1\nline 2")),
            ("inline", code_snippet_inline("s", "ViewNode")),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
                    p.id
                );
                if let Some(parent_idx) = p.parent {
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// The control does not move when it answers.
    ///
    /// The trap this is here for: a `Surface` measures `Size::ZERO`, but
    /// `layout::stack` still counts it when it spreads a
    /// `Justify::SpaceBetween` row's leftover into the gaps
    /// (`gaps = children - 1`). Hung off the single-line snippet rather than
    /// off the button, the feedback bubble is a third child of a two-child
    /// row — and the control the operator just pressed jumps to the middle
    /// of the well at the moment it answers him. That is worse than no
    /// feedback, so it is pinned by rect and not by inspection.
    #[test]
    fn saying_copied_does_not_move_the_control_that_said_it() {
        for (label, bare) in [
            ("single", code_snippet("s", "fn main() {}")),
            ("multi", code_snippet_multi("s", "line 1\nline 2")),
        ] {
            let copy_rect = |node: ViewNode| {
                let frame = petrify_lone(node);
                frame
                    .placements
                    .iter()
                    .find(|p| p.id.ends_with("/copy"))
                    .unwrap_or_else(|| panic!("{label}: no copy control placed"))
                    .rect
            };
            let resting = copy_rect(code_snippet_copied(bare.clone(), false));
            let saying = copy_rect(code_snippet_copied(bare, true));
            assert_eq!(
                resting, saying,
                "{label}: the copy control moved from {resting:?} to {saying:?} \
                 when it answered the press"
            );
        }
    }

    /// Check F: the copy button on `single`/`multi` declares `Focus` and is
    /// reachable; `inline` has no copy button and is not interactive at all.
    #[test]
    fn copy_button_is_reachable_in_focus_order() {
        for (label, node) in [
            ("single", code_snippet("s", "fn main() {}")),
            ("multi", code_snippet_multi("s", "line 1\nline 2")),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let copy = frame
                .placements
                .iter()
                .find(|p| p.semantics.role == Some(Role::Button))
                .unwrap_or_else(|| panic!("{label}: no copy button placed"));
            assert!(
                focus.order().iter().any(|id| id == &copy.id),
                "{label}: copy button declares Focus but is not in focus order"
            );
        }
    }

    /// Check E: code text and the copy label against the snippet's own
    /// well fill, in both themes, read through `Props.opacity`.
    ///
    /// **Colour runs are checked too**, and they are the reason this walk
    /// grew: a run is a second ink channel over the same fill, and a guard
    /// that read only `tokens["foreground"]` would pass a snippet whose
    /// comments were unreadable. Every [`CodeInk`] class is in the fixture
    /// list, so adding a class to that enum without a legible token turns
    /// this red.
    #[test]
    fn snippet_and_copy_text_clear_aa_contrast_against_the_well_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        let every_ink = code_runs(
            code_snippet_multi("s", "abcd"),
            vec![
                CodeInk::Plain.over(1),
                CodeInk::Comment.over(1),
                CodeInk::Keyword.over(1),
                CodeInk::Literal.over(1),
            ],
        );
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node) in [
                ("single", code_snippet("s", "fn main() {}")),
                ("multi", code_snippet_multi("s", "line 1\nline 2")),
                ("inline", code_snippet_inline("s", "ViewNode")),
                ("every ink", every_ink.clone()),
            ] {
                let bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: snippet has no resting background"));
                let bg = color(&theme, bg_name.as_str());
                fn walk_text(
                    node: &ViewNode,
                    bg: ColorValue,
                    theme: &Theme,
                    min: f32,
                    label: &str,
                ) {
                    if node.props.text.is_some() {
                        let opacity = node.props.opacity.unwrap_or(1.0);
                        // The node's own ink, then every run's. A run naming
                        // no token takes the node's, which the first pass
                        // already measured.
                        let inks =
                            node.props.tokens.get("foreground").into_iter().chain(
                                node.props.runs.iter().filter_map(|r| r.foreground.as_ref()),
                            );
                        for fg_name in inks {
                            let fg = color(theme, fg_name.as_str()).faded(opacity).over(bg);
                            let ratio = fg.contrast_ratio(bg);
                            assert!(
                                ratio >= min,
                                "{label}: {:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                                node.key,
                                fg_name.as_str()
                            );
                        }
                    }
                    for child in &node.children {
                        walk_text(child, bg, theme, min, label);
                    }
                }
                walk_text(&node, bg, &theme, MIN_TEXT_CONTRAST, label);
            }
        }
    }
}
