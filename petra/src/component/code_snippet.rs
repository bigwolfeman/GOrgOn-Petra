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
    LINK_PRIMARY, SHAPE_SM, SIZE_MD, SPACING_02, SPACING_03, SPACING_05, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_CODE, t,
};
use crate::geom::{Align, Axis};
use crate::token::TokenName;
use crate::tree::{
    AxisConstraint, Constraints, InsetRefs, Interaction, Justify, Key, Role, TextRun, ViewNode,
};

/// Carbon `.cds--snippet--multi` `min-block-size`.
const MULTI_MIN: f32 = 288.0;
/// Carbon `.cds--snippet--inline` container height.
const INLINE_HEIGHT: f32 = 16.0;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MULTI_MIN == 288.0);
const _: () = assert!(INLINE_HEIGHT == 16.0);

const COPY_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click];

/// Single-line snippet. Height 40. Copy button labelled `"Copy"`.
pub fn code_snippet(key: impl Into<Key>, code: impl Into<String>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![code_text(code.into()), copy_button()],
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
        vec![copy_row(), code_text(code.into())],
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
    let mut node = stack(key, Axis::Horizontal, None, vec![code_text(code.into())]);
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

fn code_text(code: String) -> ViewNode {
    let mut node = text("code", code);
    // Carbon sets every snippet in `$code-01` / `$code-02`, which is IBM
    // Plex Mono. This bound nothing until 2026-09-05, so the catalog's code
    // snippet was set in the sans body face -- a code snippet whose columns
    // do not line up, which is the first thing a reader notices and the
    // hardest to name. `typography.code` is the ramp's one MONO step and it
    // shipped from the start.
    node.props.style = Some(t(TYPOGRAPHY_CODE));
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
        "copy",
        Axis::Horizontal,
        None,
        vec![icon_toned("copy-icon", IconMark::Copy, IconTone::Primary)],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_03, SPACING_02));
    node.interactive(Role::Button, "Copy", COPY_INTENTS)
}

fn paint_well(mut node: ViewNode) -> ViewNode {
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
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
        CodeInk, INLINE_HEIGHT, MULTI_MIN, SHAPE_SM, SIZE_MD, SURFACE_RAISED, code_runs,
        code_snippet, code_snippet_inline, code_snippet_multi,
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
