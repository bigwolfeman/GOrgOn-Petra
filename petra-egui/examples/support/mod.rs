//! The example pages' shared authoring helpers: the token references, the
//! text roles the component library does not carry, and the primitive shapes
//! C13 leaves to the primitives.
//!
//! `gallery.rs` and `parity.rs` each include this file with
//! `#[path = "support/mod.rs"] mod support;`, so every example compiles its
//! own copy and stays one self-contained build target — the wasm canary
//! cannot stop compiling because its neighbour was edited, which is the
//! property `parity.rs` exists to protect. Sharing is possible at all
//! because of that seam: before it, these helpers were restated in each
//! file, and each file said why in as many words ("restated here rather than
//! shared because two examples cannot share a private module without
//! inventing a crate to hold it"). The seam is that shared private module,
//! without the crate. `.agents/notes/implemented/simplification/2026-10-03-dd10-example-twins-owner.md`
//! records the fold.
//!
//! # No literal style values (T078, SC-011)
//!
//! This module contains **no literal style value**. Every gap is a step of
//! the shipped spacing ramp, every corner a step of the shipped shape ramp,
//! every colour and type step a name from
//! [`gorgon_petra::token::standard_vocabulary`]. `cargo xtask
//! verify-literal-style` scans this file (its corpus is the `examples/`
//! tree, recursively), and the scan is text-based, so **every token name
//! below is spelled as a literal at its own call site on purpose**. Hiding
//! the names behind `const`s the way `petra/src/component/tokens.rs` does
//! would move them out of the lane's reach and turn a real gate into a
//! rubber stamp. The repetition here is the gate's fixture.

use std::ops::Range;
use std::sync::Arc;

use gorgon_petra::component::{
    button, disabled, heading, list_row, on_layer, section, status, text,
};
use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::input::{InputEvent, PointerButton, activates};
use gorgon_petra::layout::ChangeSet;
use gorgon_petra::token::{StatusToken, TokenName, standard_vocabulary};
use gorgon_petra::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, InputPolicy, InsetRefs, Layer, NodeKind, Props,
    Role, Semantics, TextWrap, TrackSize, ViewNode,
};

// ---------------------------------------------------------------------------
// Token helpers: every name reaches the scan as a literal at its own call site
// ---------------------------------------------------------------------------

/// A spacing token reference, for the styling props that take one (FR-053).
pub fn sp(name: &str) -> Option<TokenName> {
    Some(TokenName::new(name).expect("spacing tokens are well-formed"))
}

/// A colour, typography or shape token reference, for `props.tokens` values,
/// `props.style` and the edges of an [`InsetRefs`].
pub fn tok(name: &str) -> TokenName {
    TokenName::new(name).expect("style tokens are well-formed")
}

/// Symmetric padding from two spacing steps, horizontal first — the same
/// argument order [`InsetRefs::symmetric`] uses.
///
/// `parity.rs` called this `inset`; one owner keeps one name.
pub fn pad(horizontal: &str, vertical: &str) -> InsetRefs {
    InsetRefs::symmetric(tok(horizontal), tok(vertical))
}

// ---------------------------------------------------------------------------
// The three places the component library does not reach
// ---------------------------------------------------------------------------

/// Body text in the muted tone.
///
/// **Library gap 1.** [`text`] binds `text.primary` and takes no tone
/// argument, so there is no muted run in C13's set. Rather than hand-rolling
/// a `NodeKind::Text` — which would re-decide the kind, the typography step
/// and the wrap policy this has no business re-deciding — this composes
/// the component and rebinds the one slot it needs. The kind, the type step
/// and the structure still come from the library; only the tone is ours.
pub fn muted(key: &str, content: &str) -> ViewNode {
    let mut node = text(key, content);
    node.props
        .tokens
        .insert("foreground".into(), tok("text.muted"));
    node
}

/// A muted run that is allowed to wrap.
///
/// Every explanatory line on either page goes through here. Without
/// `TextWrap::Wrap` a note wider than its card is truncated — honestly, but
/// truncated — and a page full of sentences ending in an ellipsis is the
/// failure these harnesses exist to make visible, not one to commit.
pub fn note(key: &str, content: &str) -> ViewNode {
    let mut node = muted(key, content);
    node.props.wrap = Some(TextWrap::Wrap);
    node
}

/// A field label or a column header: muted, body-size, set in capitals.
///
/// **Library gap 2.** The type ramp has four steps and the library exposes
/// two of them ([`text`] and
/// [`heading`](gorgon_petra::component::heading)); there is no "label"
/// component and no smaller step to build one from. Capitals plus the muted
/// tone is the fifth typographic role these pages need and the ramp does not
/// carry — it costs no new token, and it is legible to a reader who cannot
/// separate the hues. It is also the only one of the five that survives the
/// renderer's broken typography map, since it is a property of the string
/// rather than of the font.
///
/// The uppercasing happens **here**, not at the call sites. A helper that
/// only renamed [`muted`] and trusted every caller to type in capitals would
/// be a wrapper around nothing, and the first caller to forget would leave a
/// label that is a different typographic role from its neighbours with
/// nothing to catch it.
pub fn caption(key: &str, content: &str) -> ViewNode {
    muted(key, &content.to_uppercase())
}

/// A button that declares itself unavailable, seated on the card it sits on.
///
/// One library call each for the two facts, and nothing else. This function
/// used to be forty lines of hand composition — clear the interactions,
/// declare `Semantics.disabled`, delete the `shadow` binding, and then fade
/// the whole subtree through `Props.opacity` at 45% because the label was
/// out of reach. Every one of those four is now somewhere it can be gated:
///
/// * the flag and the cleared interactions are
///   [`gorgon_petra::component::disabled`], which applies them to the whole
///   subtree so [`button`]'s label is unavailable too;
/// * the dropped elevation is the painter's, for any node whose resolved rank
///   is disabled — the depth channel, and the one that survives a reader who
///   cannot separate the colours at all;
/// * the faded ink is [`button`]'s own `foreground@disabled` binding,
///   resolved through the precedence chain, which is a *token* a theme can
///   retune and a gate can measure.
///
/// **The opacity is gone and its job is done twice over.** A subtree faded at
/// paint time is a state nothing can read: no digest flag, no semantic tree
/// entry, no driver query, and a contrast gate that reads token values sees
/// `text.primary` at 10.7:1 while the screen shows 3.4:1
/// (`contracts/interaction-state.md` §5, FR-010).
///
/// **The tonal step is kept**, and an early version of this got that wrong.
/// It forced `surface.base` as well, on the reasoning that a disabled control
/// should sit flush with its ground. With the border gone that left nothing:
/// no step, no shadow, no edge, so "Retire" drew no shape whatsoever and read
/// as a line of grey text between two buttons. A disabled control still has
/// to look like a control; what it must not look like is a *pressable* one,
/// and the missing elevation is what says that.
// Only the gallery has a disabled control to paint; `parity.rs`'s copy of
// this module would otherwise report it dead. Each example compiles its own
// copy of the palette and paints with a different subset of it.
#[allow(dead_code)]
pub fn disabled_button(key: &str, label: &str, depth: usize) -> ViewNode {
    disabled(on_layer(button(key, label), depth))
}

// ---------------------------------------------------------------------------
// Primitive helpers: the shapes C13 deliberately leaves to the primitives
// ---------------------------------------------------------------------------

/// A horizontal stack. `align` is explicit at every call site rather than
/// defaulted, because a control row and a text row want different answers
/// and a defaulted one is a decision nobody made.
pub fn row(
    key: &str,
    spacing: Option<TokenName>,
    align: Align,
    children: Vec<ViewNode>,
) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            spacing,
            align: Some(align),
            ..Props::default()
        })
        .with_children(children)
}

/// A vertical run of blocks, each as tall as it needs to be.
///
/// A single-column `Grid`, not a `Stack`, and the difference is the
/// reason this helper exists at all. A vertical `Stack` placed at an
/// exact height divides that height among its children by **equal
/// share** (`layout/stack.rs::distribute`), and a child offered less
/// than it needs answers with the squeezed size rather than with its
/// natural one — so one tall block in a run of short ones is compressed
/// and its text truncates. It is honest truncation, and nobody asked for
/// it. Measured on the gallery page before the change: the Controls card
/// came out 188 units tall against a natural 224, every button label was
/// cut to zero height, and the tab strip was eight units tall.
///
/// A `Grid`'s implicit rows are `FitContent`, and a `FitContent` track
/// under a closed probe is measured with `Proposal::Unspecified` and
/// clamped only to what is left (`layout/grid.rs::distribute_tracks`) —
/// the natural extent, never a share of the total. That is the sizing a
/// vertical run of blocks wants, and asking for it by choosing the
/// container is better than asking for it by tuning every block's
/// constraints until the share arithmetic happens to come out.
pub fn column(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    tracks(
        key,
        vec![TrackSize::Weight { weight: 1.0 }],
        spacing,
        children,
    )
}

/// A vertical run whose one column is a fixed measure rather than the
/// room available.
///
/// The width has to be the *track's*, not a clamp on each child: a text
/// node measures its wrapped height against the width it is offered, and
/// a constraint applied after that measurement narrows the box without
/// re-wrapping the run inside it — which reads as a paragraph that has
/// lost its last two lines.
pub fn measure_column(
    key: &str,
    width: f32,
    spacing: Option<TokenName>,
    children: Vec<ViewNode>,
) -> ViewNode {
    tracks(
        key,
        vec![TrackSize::Fixed { value: width }],
        spacing,
        children,
    )
}

/// The shared shape of [`column`] and [`measure_column`].
pub fn tracks(
    key: &str,
    columns: Vec<TrackSize>,
    spacing: Option<TokenName>,
    children: Vec<ViewNode>,
) -> ViewNode {
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns,
            row_spacing: spacing,
            ..Props::default()
        })
        .with_children(children)
}

/// The body of a card: a [`column`] that outranks the card's own
/// title when the card divides its height.
///
/// [`section`] is a vertical `Stack`, which is the one container these
/// pages use whose sizing the page cannot choose — so the title and the
/// body go through the equal-share distribution described on [`column`],
/// and with two children of very different heights the body is the one
/// that loses. `AxisConstraint::priority` is the declared way out:
/// `distribute` walks priority groups highest-first and reserves only
/// the *lower*-priority children's floors, so a body at priority 1 is
/// offered everything the title's floor does not need, and answers with
/// its natural height. One line of declaration instead of a height
/// guessed per card.
pub fn body(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    column(key, spacing, children).with_constraints(Constraints {
        vertical: AxisConstraint {
            min: None,
            max: None,
            priority: 1,
        },
        ..Constraints::default()
    })
}

/// A drawn horizontal rule.
///
/// A bare `Separator` paints **nothing**: the painter knows four token
/// slots, and a node that binds none of them declares no content, so it
/// is counted `empty` and leaves no mark. Every separator these pages had
/// before the rewrite was invisible for exactly that reason. Binding
/// `background` is what makes a rule a rule.
///
/// **The tone is `border.subtle`, not `text.muted`.** A `Separator` has
/// no `border` slot to bind, so for this node kind `background` *is* the
/// edge-drawing mechanism — functionally the same thing an outline is on
/// a filled box, and subject to the same rule. It bound `text.muted`
/// until 2026-08-25, and the first capture after the borders came off
/// made the cost obvious: with every card outline gone, the gallery's four
/// rules were measured at byte 212 on a byte-34 card, the loudest marks
/// left on the page and louder than the prose they divided. That is the
/// same conscription BORDERS.md names everywhere else (R2).
///
/// A divider is **decorative**, and the line that used to end this
/// paragraph said it was "a component boundary (WCAG 2.1 SC 1.4.11,
/// 3:1)". That clause covers information identifying a *component*, and
/// a rule between two blocks identifies none; holding every rule to it
/// is what set the tone of every divider in the catalog. The floor is
/// Carbon's own 1.3:1 now -- see `gorgon_petra::token::shipped`'s
/// `BORDER_TOKEN`.
pub fn rule(key: &str) -> ViewNode {
    let mut props = Props::default();
    props
        .tokens
        .insert("background".into(), tok("border.subtle"));
    ViewNode::new(NodeKind::Separator, key).with_props(props)
}

/// The shared shape of [`exact`] and [`at_least`].
pub fn clamp(axis: Axis, min: Option<f32>, max: Option<f32>) -> Constraints {
    let clamp = AxisConstraint {
        min,
        max,
        priority: 0,
    };
    match axis {
        Axis::Horizontal => Constraints {
            horizontal: clamp,
            ..Constraints::default()
        },
        Axis::Vertical => Constraints {
            vertical: clamp,
            ..Constraints::default()
        },
    }
}

/// A hard clamp on one axis, so a node is measured against an extent the
/// window cannot change out from under it.
///
/// Layout declaration, not styling: `xtask`'s literal-style lane never
/// inspects `constraints`, and says so in as many words, because a track
/// size and a text measure are numeric by spec.
pub fn exact(axis: Axis, value: f32) -> Constraints {
    clamp(axis, Some(value), Some(value))
}

/// A floor on one axis with no ceiling: what a control that must be big
/// enough to look like a control, but may grow, declares.
pub fn at_least(axis: Axis, value: f32) -> Constraints {
    clamp(axis, Some(value), None)
}

/// A rect of exactly `width` by `height`, for a hosted content well.
///
/// Only `parity.rs` hosts fixed wells today; the gallery's hosted card
/// registers painters instead. Same one-sided rule as [`disabled_button`].
#[allow(dead_code)]
pub fn well(width: f32, height: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(width),
            max: Some(width),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(height),
            max: Some(height),
            priority: 0,
        },
    }
}

/// The card treatment: raised fill, an elevation shadow, `shape.corner-md`.
///
/// [`section`] applies exactly this to every titled block (down to the
/// same `shadow.raised` token, for the same reason: BORDERS.md R2/R3 —
/// no `border` may bind a text tone, and depth is drawn as elevation, not
/// as an outline, once there is a fill to spend). The telemetry band is
/// the one card on either page that is a `Grid` rather than a titled
/// column, so it cannot go through `section` — and a card that was
/// *nearly* a section would be the "round buttons in a square layout"
/// failure in miniature. This function is how the two stay one object.
pub fn card(mut node: ViewNode) -> ViewNode {
    node.props.padding = Some(pad("spacing.lg", "spacing.md"));
    node.props
        .tokens
        .insert("background".into(), tok("surface.raised"));
    // Depth, not an outline. This bound `border` to `text.muted` until
    // 2026-08-25, which drew a hairline at 10.7:1 against the fill it was
    // separating -- as loud as body text, and carrying no information the
    // fill was not already carrying. A card is not a wireframe.
    node.props
        .tokens
        .insert("shadow".into(), tok("shadow.raised"));
    node.props
        .tokens
        .insert("radius".into(), tok("shape.corner-md"));
    node
}

/// A floating surface: a modal, a popup or a toast — the things on these
/// pages with `shape.corner-lg`.
///
/// `shadow.overlay`, not `shadow.raised`: `SHADOW_GEOMETRY`'s own doc
/// draws the line at "has left the page entirely" for the deeper of the
/// two elevations, and a viewport-anchored dialog or toast is exactly
/// that — unlike [`card`], which is still part of the page's own flow.
pub fn surface(key: &str, layer: Layer, policy: InputPolicy) -> ViewNode {
    let mut props = Props {
        layer: Some(layer),
        anchor: Some(Anchor::Viewport),
        clamp: Some(ClampRule::Shrink),
        input_policy: Some(policy),
        // One child, always: every caller hands this a single
        // `column`. A `surface` stacks its children the way a
        // vertical `Stack` does, and stacking four blocks of different
        // heights inside one is the equal-share squeeze all over again —
        // measured, on the first capture with the gallery's modal open:
        // the heading and the body were flattened to nothing and the rule
        // inflated into a 28-unit grey bar across the dialog.
        axis: Some(Axis::Vertical),
        padding: Some(pad("spacing.xl", "spacing.lg")),
        ..Props::default()
    };
    props
        .tokens
        .insert("background".into(), tok("surface.raised"));
    props.tokens.insert("shadow".into(), tok("shadow.overlay"));
    props.tokens.insert("radius".into(), tok("shape.corner-lg"));
    ViewNode::new(NodeKind::Surface, key).with_props(props)
}

/// The three track-sizing rules, a spacer and a rule — the `Grid`
/// vocabulary C13 leaves to the primitives, carrying `Role::Table`
/// because a grid of labelled cells is what a table is.
pub fn layout_card() -> ViewNode {
    let grid = ViewNode::new(NodeKind::Grid, "tracks")
        .with_props(Props {
            columns: vec![
                TrackSize::Fixed { value: 90.0 },
                TrackSize::FitContent,
                TrackSize::Weight { weight: 1.0 },
            ],
            column_spacing: sp("spacing.md"),
            row_spacing: sp("spacing.xs"),
            ..Props::default()
        })
        .with_semantics(Semantics {
            role: Some(Role::Table),
            ..Semantics::default()
        })
        .child(caption("h-a", "fixed 90"))
        .child(caption("h-b", "fitcontent"))
        .child(caption("h-c", "weight(1)"))
        .child(text("g-a", "clamped"))
        .child(text("g-b", "wider cell here"))
        .child(text("g-c", "takes the rest"));

    let spacer_row = row(
        "spacer-row",
        sp("spacing.sm"),
        Align::Center,
        vec![
            text("left", "left"),
            // Height-clamped on purpose. An unconstrained `Spacer` takes
            // everything offered on *both* axes (`layout/leaf.rs`), so
            // one in a horizontal row claims the row's whole height and
            // opens a hole the size of the window. SwiftUI's `Spacer`
            // expands only along its stack's axis; Petra's does not, and
            // `an_unconstrained_spacer_claims_the_cross_axis_too` (the
            // gallery's own tests) pins that so the difference is a
            // decision rather than a surprise.
            ViewNode::new(NodeKind::Spacer, "gap").with_constraints(exact(Axis::Vertical, 0.0)),
            text("right", "pushed right by a Spacer"),
        ],
    );

    section(
        "layout",
        "Layout primitives",
        vec![body(
            "body",
            sp("spacing.md"),
            vec![grid, rule("layout-rule"), spacer_row],
        )],
    )
}

// ---------------------------------------------------------------------------
// The shared card bodies and the `view` scaffold (DD4's copy-accident half)
// ---------------------------------------------------------------------------

/// The one line both text cards measure wrapping and elision against.
///
/// The whole point of a text node is that the engine measures it against
/// the width it is offered rather than trusting whoever wrote the string,
/// and a sentence that fits on one line demonstrates neither.
const LONG_LINE: &str = "A long line that has to wrap, because the whole point of a text \
                         node is that the engine measures it against the width it is offered \
                         rather than trusting whoever wrote the string.";

/// The text card: one long line twice — wrapped, then elided at one line —
/// plus whatever else a page has to say about text.
///
/// Both runs share one measure, and the measure is the *track's* width
/// rather than a clamp on each node — see [`measure_column`] for why the
/// difference is the whole paragraph. 360 logical units at the shipped body
/// size is about sixty characters, which is the readable range, so this is
/// a typographic decision rather than a workaround. It is also what makes
/// the two nodes differ at all: at the card's full width the "wrapped"
/// string fits on one line and the "elided" one never truncates, and then
/// the card demonstrates neither.
pub fn text_card(extra: Vec<ViewNode>) -> ViewNode {
    let mut wrapped = Props {
        text: Some(LONG_LINE.to_owned()),
        wrap: Some(TextWrap::Wrap),
        style: Some(tok("typography.body")),
        ..Props::default()
    };
    wrapped
        .tokens
        .insert("foreground".into(), tok("text.primary"));

    let mut elided = Props {
        text: Some(LONG_LINE.to_owned()),
        wrap: Some(TextWrap::Ellipsis),
        max_lines: Some(1),
        style: Some(tok("typography.body")),
        ..Props::default()
    };
    elided.tokens.insert("foreground".into(), tok("text.muted"));

    let mut children = vec![
        ViewNode::new(NodeKind::Text, "wrapped").with_props(wrapped),
        caption("elide-note", "the same string, capped at one line"),
        ViewNode::new(NodeKind::Text, "elided").with_props(elided),
    ];
    children.extend(extra);

    section(
        "type",
        "Text",
        vec![body(
            "body",
            sp("spacing.md"),
            vec![measure_column("measure", 360.0, sp("spacing.sm"), children)],
        )],
    )
}

/// Every shipped status, through colour, shape and the word: the FR-015
/// card.
///
/// `fibers` is `(key, status token, detail line)` per row — the data is
/// each page's own (it is the page's subject); only the construction is
/// shared. The token names are looked up in [`standard_vocabulary`] at
/// build time rather than reassembled here, because [`status`] takes a
/// whole [`StatusToken`] — colour, shape and text together, FR-015 — and
/// there is deliberately no way to hand it a colour on its own.
pub fn status_card(fibers: &[(&str, &str, &str)], message: &str) -> ViewNode {
    let vocabulary = standard_vocabulary();
    // The state leads. A `FitContent` state column on the right sits
    // wherever the widest name leaves it — measured at about five hundred
    // units away from the row it belongs to, which reads as two unrelated
    // lists. On the left the three dots line up in a column the eye can run
    // down, and the names start on a common edge because `FitContent` takes
    // the widest of the three.
    let mut grid = ViewNode::new(NodeKind::Grid, "fibers").with_props(Props {
        columns: vec![TrackSize::FitContent, TrackSize::Weight { weight: 1.0 }],
        column_spacing: sp("spacing.md"),
        row_spacing: sp("spacing.sm"),
        ..Props::default()
    });
    for &(fiber, token, detail) in fibers {
        let declared: &StatusToken = vocabulary
            .status(&tok(token))
            .unwrap_or_else(|| panic!("{token} must be a declared status"));
        grid = grid
            .child(status(format!("{fiber}-state"), declared))
            .child(column(
                fiber,
                sp("spacing.2xs"),
                vec![text("name", fiber), note("detail", detail)],
            ));
    }

    section(
        "status",
        "Fibers",
        vec![body(
            "body",
            sp("spacing.md"),
            vec![grid, note("note", message)],
        )],
    )
}

/// A `Scroll` over a `Collection` that claims `total` rows.
///
/// The collection asks [`RowSource`](gorgon_petra::layout::RowSource) only
/// for the window it can see, so the row count is a claim about the store
/// and not about work this frame did — watch the placements readout: it
/// does not grow with `total`. `extent` is one row's declared height:
/// a `list_row` is a padded stack around a 20-unit body line, and
/// declaring 28 rather than the bare line height is what keeps the rows
/// contiguous instead of leaving a gap of exactly the padding under each
/// one. `height` clamps the scroll viewport on the page, and `caption` is
/// the claim the card prints above it.
pub fn collection_card(
    source: &str,
    total: usize,
    extent: f32,
    height: f32,
    caption_text: &str,
) -> ViewNode {
    let list = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
        source: Some(source.to_owned()),
        total_count: Some(total),
        estimated_extent: Some(extent),
        axis: Some(Axis::Vertical),
        ..Props::default()
    });

    section(
        "store",
        "Store",
        vec![body(
            "body",
            sp("spacing.md"),
            vec![
                caption("note", caption_text),
                ViewNode::new(NodeKind::Scroll, "scroll")
                    .with_props(Props {
                        axis: Some(Axis::Vertical),
                        // On the scroll, not on the collection inside
                        // it. Tree acceptance refuses `overscan` on a
                        // nested collection rather than silently
                        // ignoring it, because the scroll is what
                        // actually resolves the value.
                        overscan: Some(64.0),
                        ..Props::default()
                    })
                    .with_constraints(exact(Axis::Vertical, height))
                    .child(list),
            ],
        )],
    )
}

/// The modal scaffold: a blocking dialog with a title, a rule, a note and
/// the caller's action buttons.
///
/// The button nodes are the caller's own — their ids are what each page's
/// `handle` dispatches on, so they stay data like every other node id
/// (DD4 §5.2).
pub fn modal_surface(
    label: &str,
    title: &str,
    message: &str,
    button_nodes: Vec<ViewNode>,
) -> ViewNode {
    let mut actions = vec![
        // The pusher, height-clamped exactly like `layout_card`'s: an
        // unconstrained `Spacer` claims the cross axis and would make the
        // actions row as tall as the dialog.
        ViewNode::new(NodeKind::Spacer, "push").with_constraints(exact(Axis::Vertical, 0.0)),
    ];
    actions.extend(button_nodes);
    surface("modal", Layer::Modal, InputPolicy::Block)
        .with_semantics(Semantics {
            role: Some(Role::Dialog),
            label: Some(label.to_owned()),
            ..Semantics::default()
        })
        .child(column(
            "body",
            sp("spacing.md"),
            vec![
                heading("title", title),
                rule("rule"),
                note("note", message),
                row("actions", sp("spacing.md"), Align::Center, actions),
            ],
        ))
}

/// The page scaffold: a masthead, the telemetry band, a rule and the body
/// content, on the page-coloured root grid.
///
/// The page itself is a [`column`] too — a single-column `Grid`, carrying
/// the page margin and the page fill. Its regions have very different
/// heights, and a `Stack` would share the window's height out between them
/// instead of letting each be as tall as it is. See [`column`] for the
/// measurement behind that.
pub fn page_root(masthead: ViewNode, telemetry: ViewNode, content: ViewNode) -> ViewNode {
    let mut page = Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        row_spacing: sp("spacing.2xl"),
        padding: Some(pad("spacing.3xl", "spacing.2xl")),
        ..Props::default()
    };
    page.tokens.insert("background".into(), tok("surface.base"));

    ViewNode::new(NodeKind::Grid, "root")
        .with_props(page)
        .child(masthead)
        .child(telemetry)
        .child(rule("masthead-rule"))
        .child(content)
}

/// The body grid both pages hang their cards on: two weight-1 columns.
pub fn two_column_grid(left: Vec<ViewNode>, right: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Grid, "columns")
        .with_props(Props {
            columns: vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Weight { weight: 1.0 },
            ],
            column_spacing: sp("spacing.xl"),
            row_spacing: sp("spacing.xl"),
            ..Props::default()
        })
        .child(column("left", sp("spacing.xl"), left))
        .child(column("right", sp("spacing.xl"), right))
}

/// The page inside its scroll — the node that covers the viewport.
///
/// The page scrolls. Without this the root grid is offered exactly the
/// window's height and divides it among its regions, so adding a card does
/// not make the page longer — it makes every existing card shorter, down to
/// text squeezed to a few points tall. A vertical grid distributes the
/// height it is given; something has to give it an unbounded one, and a
/// scroll is that something.
///
/// The page fill lives on the *scroll*, not only on the content inside it:
/// a page shorter than the window leaves the rest of the frame painted by
/// whatever cleared it, and the seam between the two blacks is visible. The
/// scroll is the node that covers the whole viewport, so it is the one that
/// has to carry the colour.
pub fn scrolled_page(root: ViewNode) -> ViewNode {
    let mut scroll = Props {
        axis: Some(Axis::Vertical),
        overscan: Some(64.0),
        ..Props::default()
    };
    scroll
        .tokens
        .insert("background".into(), tok("surface.base"));
    ViewNode::new(NodeKind::Scroll, "page")
        .with_props(scroll)
        .child(root)
}

/// The top-level shell: the page, with every open surface beside it.
///
/// Surfaces sit beside the page, not inside it. They are anchored to the
/// viewport, and a viewport-anchored thing inside scrolling content is
/// placed in the wrong coordinate space — a popup that moves when the page
/// moves, and hit-testing that disagrees with what is on screen. `Overlay`
/// is the container for this: every child gets the container's own
/// proposal, z-order by child order.
pub fn overlay_shell(page: ViewNode, surfaces: Vec<ViewNode>) -> ViewNode {
    let mut shell = ViewNode::new(NodeKind::Overlay, "shell").child(page);
    for surface_node in surfaces {
        shell = shell.child(surface_node);
    }
    shell
}

/// The window of rows a virtualized collection asks its [`RowSource`]
/// (gorgon_petra::layout::RowSource) for.
///
/// Keyed by the row's own index, not by its position in this window, so
/// scrolling does not renumber what is on screen. `source` is what the
/// engine asked for and `wanted` is the one source this page serves; any
/// other name gets nothing rather than someone else's rows.
pub fn window_rows(
    source: &str,
    wanted: &str,
    range: Range<usize>,
    selected: Option<usize>,
) -> Vec<Arc<ViewNode>> {
    if source != wanted {
        return Vec::new();
    }
    range
        .map(|i| {
            Arc::new(on_layer(
                list_row(
                    format!("row-{i}"),
                    format!("supervisor/worker-{i:05}"),
                    selected == Some(i),
                ),
                1,
            ))
        })
        .collect()
}

/// [`ChangeSet::All`] for a page that rebuilds its whole tree every pass.
///
/// The only honest answer. Naming individual nodes while handing back
/// fresh `Arc`s is exactly the under-declaration
/// `ReuseState::verify_declaration` panics on in a debug build.
pub fn whole_tree_changes() -> ChangeSet {
    ChangeSet::All
}

/// Whether `event` should act on the node it routed to.
///
/// [`activates`] answers only for the keyboard stand-in — Enter or Space on
/// the focused node — because that is the part `gorgon_petra::input::route`
/// has to widen to keep a `Click`-only button operable from the keyboard
/// (FR-025). It is deliberately *not* an answer about the pointer, and the
/// gallery page used to guard on it alone: every mouse press on that page was
/// reported "not an activation" and dropped, so the tab strip, the surface
/// buttons and the row selection were all decorative. A press that reaches
/// [`gorgon_petra::input::Route::Pointer`] has already passed `hit_test` for
/// [`gorgon_petra::tree::Interaction::Click`], so the node it names is a node
/// that asked to be clicked; acting on it is the whole point.
pub fn activated(event: &InputEvent) -> bool {
    activates(event)
        || matches!(
            event,
            InputEvent::PointerPressed {
                button: PointerButton::Primary,
                ..
            }
        )
}
