//! Carbon Modal (slice-c). Focus-trapping dialog, not a popover.
//!
//! Anatomy (`_modal.scss`, and `ignored/carbon-ref/shots/20-modal.png`):
//! 1. Overlay — the scrim. Carbon's `.cds--modal` is a `position: fixed`
//!    box covering the whole viewport with `background-color: $overlay`;
//!    the dialog sits inside it. Here that box *is* the [`NodeKind::Surface`]
//!    this module returns: [`Layer::Modal`], [`Anchor::Viewport`],
//!    [`InputPolicy::Block`], and a declared minimum wider than any window
//!    so [`ClampRule::Shrink`] hands it the window exactly
//!    (`layout::overlay_surface`, "a surface that asks for more than the
//!    window gets the window"). It paints [`OVERLAY_SCRIM`] through
//!    `background` like any other fill, and it carries [`Role::Dialog`] and
//!    the accessible name.
//! 2. Container — the dialog. 60% of the window wide (Carbon `md` at the
//!    `lg` breakpoint, the reference app's own size), centred both ways,
//!    [`SURFACE_RAISED`] with [`SHADOW_RAISED`]. **No border and no
//!    radius**: Carbon's container is square, and its 1px `$border-subtle`
//!    edge is the outline the operator refused on 2026-09-04 ("borders are
//!    a no no").
//! 3. Header — the required title (`heading-03`), padded 16 on top and
//!    inline, 16 below; and Close in a 48×48 hit box at the top-right.
//! 4. Body — caller-supplied text, wrapped, padded 8 above and 48 below.
//! 5. Footer — Carbon's transactional two-button bar: full-bleed, 64 tall,
//!    Cancel (secondary) and the primary action at 50%/50%. [`modal_passive`]
//!    omits it (Carbon `passiveModal`: dismiss-only).
//! 6. Close — a [`Role::Button`] labelled `"Close"`, never an icon-only
//!    mark (FR-026). Hit box 48 (Carbon `3rem`). The close icon itself is
//!    **20×20** (SCSS, T070); the style page's 16×16 is design intent.
//!
//! # Why the scrim is the surface
//!
//! The scrim is not decoration: it is the one visual channel for "the page
//! behind this is inert", and the operator is red-green colour blind, so a
//! state shown only by a change of hue is a state he cannot see. Making the
//! window-covering box the `Block` surface binds the picture and the input
//! rule to one node — nothing is ever outside it, and
//! `input::route_with_surfaces` makes a `Block` surface opaque over its
//! rect, so a press on the dimmed page reaches nothing and a press on the
//! footer reaches the button. A dialog-sized surface with a separate scrim
//! behind it would have needed two nodes to agree about one fact.
//!
//! # What this does not match
//!
//! Carbon autofocuses the primary button on open; the host's
//! `enter_open_modal` seats focus on the first focusable node in the trap,
//! which is Close. Carbon's `md` width is 84% below the `lg` breakpoint and
//! 48% above `xlg`; the 60% here is not bucketed by window size. Carbon's
//! body copy is held to 80% of the container by an extra inline-end pad;
//! this body wraps at the full padded width.
//!
//! A viewport-centred surface is not a popover; this module does not call
//! [`super::popover`].

use super::button::{button, primary_button};
use super::icon::{IconBox, IconMark, IconTone, icon_in};
use super::on_layer;
use super::stack;
use super::text::{heading, text};
use super::tokens::{
    LAYER_HOVER, OVERLAY_SCRIM, SHADOW_RAISED, SHAPE_NONE, SPACING_03, SPACING_05, SPACING_09,
    SURFACE_RAISED, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, FocusFigure, InputPolicy, InsetRefs,
    Interaction, Justify, Key, Layer, NodeKind, Props, Role, Semantics, TextWrap, TrackSize,
    ViewNode,
};

/// Carbon close-button hit box (`3rem`).
const CLOSE_HIT: f32 = 48.0;
/// Carbon close icon, SCSS `convert.to-rem(20px)`. Style page says 16; T070
/// prefers SCSS. Recorded, not painted: the visible channel is the word
/// `"Close"`.
const CLOSE_ICON: f32 = 20.0;
/// MEASURED `_modal.scss` footer: `block-size: 4rem`.
const FOOTER_HEIGHT: f32 = 64.0;
/// A minimum no window can satisfy, so the `Shrink` ladder places the
/// surface on the window rect exactly. `f32::MAX`, not infinity: an
/// infinite extent is not "sane" to `geom::Size::sane` and collapses to
/// zero, which is the opposite of covering the window.
const COVER_WINDOW: f32 = f32::MAX;
/// Column weights of the frame that seats the dialog: side, dialog, side.
/// `1 : 3 : 1` is 20% / 60% / 20% — Carbon `md` at the `lg` breakpoint.
const SIDE_SHARE: f32 = 1.0;
/// See [`SIDE_SHARE`].
const DIALOG_SHARE: f32 = 3.0;

const _: () = assert!(CLOSE_HIT == 48.0);
const _: () = assert!(CLOSE_ICON == 20.0);
const _: () = assert!(FOOTER_HEIGHT == 64.0);
const _: () = assert!(COVER_WINDOW.is_finite());
const _: () = assert!(DIALOG_SHARE / (SIDE_SHARE + DIALOG_SHARE + SIDE_SHARE) == 0.6);

const CLOSE_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A transactional dialog: title, body, Close, and a Cancel / `primary`
/// footer. `title` is the accessible name and the visible heading.
///
/// The Cancel label is Carbon's own default for the secondary action and is
/// not a parameter: a second label parameter that every caller passes as
/// `"Cancel"` is noise, and the one caller that wants something else wants
/// a different modal variant (Carbon's three-button "progress" footer),
/// which this module does not ship.
pub fn modal(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
    primary: impl Into<String>,
) -> ViewNode {
    build(key.into(), title.into(), body.into(), Some(primary.into()))
}

/// A passive dialog (Carbon `passiveModal`): title, body and Close, with no
/// footer. The reader dismisses it; there is nothing to confirm.
pub fn modal_passive(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    build(key.into(), title.into(), body.into(), None)
}

fn build(key: Key, title: String, body: String, primary: Option<String>) -> ViewNode {
    // The dialog, centred on the cross axis of a stack that fills the middle
    // column: `justify` centres it vertically in the whole window height,
    // `Stretch` gives it the column's full 60%.
    let mut seat = stack(
        "seat",
        Axis::Vertical,
        None,
        vec![dialog(&title, body, primary)],
    );
    seat.props.justify = Some(Justify::Center);
    seat.props.align = Some(Align::Stretch);

    // One row the height of the window, three columns: the dialog's width
    // is a share of the window rather than a number, which is the only way
    // to say "60%" in a tree whose constraints are absolute.
    let frame = ViewNode::new(NodeKind::Grid, "frame")
        .with_props(Props {
            columns: vec![
                TrackSize::Weight { weight: SIDE_SHARE },
                TrackSize::Weight {
                    weight: DIALOG_SHARE,
                },
                TrackSize::Weight { weight: SIDE_SHARE },
            ],
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![
            ViewNode::new(NodeKind::Spacer, "l"),
            seat,
            ViewNode::new(NodeKind::Spacer, "r"),
        ]);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Modal),
            anchor: Some(Anchor::Viewport),
            clamp: Some(ClampRule::Shrink),
            input_policy: Some(InputPolicy::Block),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(COVER_WINDOW),
                max: None,
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(COVER_WINDOW),
                max: None,
                priority: 0,
            },
        })
        .child(frame);
    node.props
        .tokens
        .insert("background".into(), t(OVERLAY_SCRIM));
    node.semantics = Semantics {
        role: Some(Role::Dialog),
        label: Some(title),
        ..Semantics::default()
    };
    node
}

/// The container: header, body, and the footer when there is one.
fn dialog(title: &str, body: String, primary: Option<String>) -> ViewNode {
    let mut rows = vec![header(title), content(body)];
    if let Some(primary) = primary {
        rows.push(footer(primary));
    }
    let mut node = stack("dialog", Axis::Vertical, None, rows);
    node.props.align = Some(Align::Stretch);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
    node
}

/// Title on the left, Close in the top-right corner. A grid rather than a
/// horizontal stack so the close control sits at the trailing edge whatever
/// the title's width, and so a long title wraps in the room that is left.
fn header(title: &str) -> ViewNode {
    let mut heading = heading("title", title);
    heading.props.wrap = Some(TextWrap::Wrap);
    // Carbon: `padding-block-start: $spacing-05; padding-inline: $spacing-05
    // $spacing-09; margin-block-end: $spacing-05`. The 48 inline-end is the
    // room Carbon's absolutely positioned close button takes; here the
    // close control has its own column, so the title's own inline-end pad
    // is the plain 16.
    let mut seat = stack("title-seat", Axis::Vertical, None, vec![heading]);
    seat.props.padding = Some(InsetRefs {
        top: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        bottom: Some(t(SPACING_05)),
        left: Some(t(SPACING_05)),
    });
    ViewNode::new(NodeKind::Grid, "header")
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }, TrackSize::FitContent],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![seat, close_button()])
}

/// The body copy. Carbon: `padding-block: $spacing-03 $spacing-09;
/// padding-inline: $spacing-05`.
fn content(body: String) -> ViewNode {
    let mut copy = text("body", body);
    copy.props.wrap = Some(TextWrap::Wrap);
    let mut seat = stack("content", Axis::Vertical, None, vec![copy]);
    seat.props.align = Some(Align::Stretch);
    seat.props.padding = Some(InsetRefs {
        top: Some(t(SPACING_03)),
        right: Some(t(SPACING_05)),
        bottom: Some(t(SPACING_09)),
        left: Some(t(SPACING_05)),
    });
    seat
}

/// Carbon's two-button footer: 64 tall, full-bleed, each button half the
/// width, Cancel on the left and the primary action on the right.
fn footer(primary: String) -> ViewNode {
    ViewNode::new(NodeKind::Grid, "footer")
        .with_props(Props {
            columns: vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Weight { weight: 1.0 },
            ],
            rows: vec![TrackSize::Fixed {
                value: FOOTER_HEIGHT,
            }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![
            // One tone ahead of the container it sits on (`on_layer`):
            // Carbon's `$button-secondary` is a visibly lighter grey than
            // the dialog, and a Cancel in the dialog's own fill reads as a
            // label, not a control.
            footer_button(on_layer(button("cancel", "Cancel"), 1)),
            footer_button(primary_button("primary", primary)),
        ])
}

/// A library button re-seated as a footer cell: Carbon's `.cds--modal-footer
/// .cds--btn` is 64 tall with its label at the top-left
/// (`padding-block-start: $spacing-05; align-items: flex-start`), square,
/// and casts no shadow of its own — the footer is part of the container's
/// silhouette, not four raised controls on it.
fn footer_button(mut node: ViewNode) -> ViewNode {
    node.constraints.vertical = AxisConstraint {
        min: Some(FOOTER_HEIGHT),
        max: Some(FOOTER_HEIGHT),
        priority: 0,
    };
    // `Border`, overriding the `Sides` a library button carries, and it is
    // the one place in this library where a box beats an underline on both
    // counts at once.
    //
    // A footer button is full-bleed in **both** axes. Across: Cancel's
    // leading edge *is* the dialog's leading edge and the primary action's
    // trailing edge is the dialog's, so a bar standing `FocusRing::hug_gap`
    // + `FocusRing::thickness` = 7 units outside either of them paints on
    // the page behind the dialog — measured on row 20, Cancel's left bar at
    // x 233 and the primary's right bar at x 964 against a dialog spanning
    // 240 to 960. Down: the button's bottom edge *is* the dialog's bottom
    // edge, so `BarInside` — which was the answer to the first problem —
    // seats its stripe on the dialog's last three units with no ground under
    // it at all. The operator, 2026-09-06: *"on modal the underbar is still
    // too close."* Measured off his capture: dialog fill to y 278, accent on
    // 279..281, page scrim from 282.
    //
    // And the primary action is worse than cramped. Its fill is
    // `ACCENT_PRIMARY` and `focus.ring` is byte-identical to it, so a stripe
    // painted on that fill is invisible — *"it disappears when in the
    // colored one."* His capture reads `(15, 98, 254)` for both.
    //
    // `Border` answers all three. It is contained, so neither axis can
    // overflow; it carries the ground-coloured halo band
    // (`focus::HALO_TOKEN`) that exists for exactly this collision and says
    // so in its own doc; and it is what Carbon puts here
    // (`.cds--modal-footer .cds--btn:focus` takes `focus-outline('outline')`).
    // The operator's standing rule is that underlines beat boxes *except*
    // where an underline sticks too far off and looks bad; an underline that
    // vanishes is the same complaint with the volume turned up.
    node.semantics.focus_figure = FocusFigure::Border;
    node.props.align = Some(Align::Start);
    node.props.padding = Some(InsetRefs {
        top: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        bottom: None,
        left: Some(t(SPACING_05)),
    });
    node.props.tokens.insert("radius".into(), t(SHAPE_NONE));
    node.props.tokens.remove("shadow");
    node
}

fn close_button() -> ViewNode {
    let mut node = stack(
        "close",
        Axis::Horizontal,
        None,
        vec![icon_in(
            "glyph",
            IconMark::Close,
            IconBox::Glyph,
            IconTone::Primary,
        )],
    );
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    // A flat glyph, no boundary — `component::file_uploader`'s `remove`
    // control, which is the one the operator named on 2026-09-06: *"the
    // close button border looks terrible... for close try just a flat x like
    // we are using on the file uploader modals"*.
    //
    // This replaces the word "Close" and a `BORDER_STRONG` box, both added
    // the day before for a reason that was real: the resting fill is the
    // dialog's own surface, so with neither a border nor a glyph the control
    // appeared only under the pointer. A glyph answers that without a box —
    // it is visible at rest, which is the whole job the border was doing.
    //
    // **It is a departure from FR-026**, which says an icon must be a second
    // channel and never the only one, and from this module's own doc rule 6
    // ("never an icon-only mark"). The accessible name is still "Close" and
    // `no_shipped_component_spells_an_icon_as_its_name` still holds, but for
    // a sighted reader the glyph is now the only channel. Recorded rather
    // than dressed up: the operator asked for it by name, citing a control
    // this library already ships the same way.
    //
    // Resting fill matches the dialog surface it sits on ([`SURFACE_RAISED`])
    // rather than binding no `background` at all: an unbound slot with only
    // `background@hover` beside it declares content the paint pass cannot
    // resolve at rest, which the accounting counts as silent
    // (`gorgon_petra_egui::paint::PaintReport::silent`).
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(Constraints {
        // Pinned both ways now, where the word "Close" needed a floor and no
        // ceiling. Carbon's 48 is the hit box for exactly this: an icon-only
        // close glyph.
        horizontal: AxisConstraint {
            min: Some(CLOSE_HIT),
            max: Some(CLOSE_HIT),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(CLOSE_HIT),
            max: Some(CLOSE_HIT),
            priority: 0,
        },
    })
    .interactive(Role::Button, "Close", CLOSE_INTENTS)
    .owning_its_text()
    // `BarUnder`, the operator's call of 2026-09-06 for this control.
    //
    // `Sides` was the library default and spilled: this button sits hard
    // against the dialog's trailing edge and a side bar stands `hug_gap` +
    // `thickness` = 7 units outside the rect, so the right-hand one painted
    // on the page behind the modal. `BarInside` replaced it and put the
    // stripe inside the button's own fill, which is what the operator is
    // rejecting here. The header has the dialog's whole body under it, so
    // the five units a bar needs are there and the bar stays in the dialog.
    .with_focus_figure(FocusFigure::BarUnder)
}

#[cfg(test)]
mod tests {
    use super::{CLOSE_HIT, CLOSE_ICON, COVER_WINDOW, FOOTER_HEIGHT, modal, modal_passive};
    use crate::component::tokens::{LAYER_HOVER, OVERLAY_SCRIM, SURFACE_RAISED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Point, Rect, Size};
    use crate::input::{InputEvent, Modifiers, PointerButton, Route, route_with_surfaces};
    use crate::layout::overlay_surface::surface_scopes;
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::FocusFigure;
    use crate::tree::{
        Anchor, ClampRule, InputPolicy, Interaction, Layer, NodeKind, Props, Registry, Role,
        TrackSize, ViewNode,
    };

    fn descendant<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|c| walk(c, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("missing descendant {key}"))
    }

    fn has_descendant(node: &ViewNode, key: &str) -> bool {
        node.key.as_str() == key || node.children.iter().any(|c| has_descendant(c, key))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn rebuild() -> ViewNode {
        modal(
            "retire",
            "Retire fiber",
            "Its children are retired with it.",
            "Retire",
        )
    }

    #[test]
    fn modal_is_a_labelled_dialog_not_an_overlay() {
        let node = rebuild();
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Dialog));
        assert_ne!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.semantics.label.as_deref(), Some("Retire fiber"));
        assert_eq!(node.props.layer, Some(Layer::Modal));
        assert_eq!(node.props.input_policy, Some(InputPolicy::Block));
        assert_eq!(node.props.anchor, Some(Anchor::Viewport));
        assert_eq!(node.props.clamp, Some(ClampRule::Shrink));
        assert!(
            node.interactions.is_empty(),
            "the dialog is not a click target"
        );
    }

    /// The surface is the scrim: it asks for more than any window so the
    /// `Shrink` ladder gives it the window, and it paints the scrim token.
    /// The container inside it is the raised card, with no edge drawn.
    #[test]
    fn the_surface_is_a_window_covering_scrim_and_the_container_has_no_border() {
        let node = rebuild();
        assert_eq!(node.constraints.horizontal.min, Some(COVER_WINDOW));
        assert_eq!(node.constraints.vertical.min, Some(COVER_WINDOW));
        assert_eq!(token(&node, "background"), Some(OVERLAY_SCRIM));
        let dialog = descendant(&node, "dialog");
        assert_eq!(token(dialog, "background"), Some(SURFACE_RAISED));
        assert_eq!(
            token(dialog, "border"),
            None,
            "borders are a no no (operator, 2026-09-04): Carbon's 1px \
             `$border-subtle` container edge is not ported"
        );
        assert_eq!(
            token(dialog, "radius"),
            None,
            "Carbon's container is square"
        );
        assert_eq!(token(&node, "border"), None);
    }

    #[test]
    fn modal_chrome_is_title_body_close_and_a_two_button_footer() {
        let node = rebuild();
        assert_eq!(
            descendant(&node, "title").props.text.as_deref(),
            Some("Retire fiber")
        );
        assert_eq!(
            descendant(&node, "body").props.text.as_deref(),
            Some("Its children are retired with it.")
        );
        let close = descendant(&node, "close");
        assert_eq!(close.semantics.role, Some(Role::Button));
        assert_eq!(close.semantics.label.as_deref(), Some("Close"));
        // A glyph, not the word, since 2026-09-06. The accessible name above
        // still carries "Close" and is the only place that word now appears,
        // which is the FR-026 departure this control's own comment records.
        assert!(
            has_descendant(close, "glyph"),
            "the close control draws no glyph"
        );
        assert!(
            !has_descendant(close, "label"),
            "the word came back beside the glyph; if that is wanted, the \
             FR-026 note on `close_button` is what needs revisiting first"
        );
        assert!(close.interactions.contains(&Interaction::Click));
        assert!(close.interactions.contains(&Interaction::Focus));
        assert_eq!(close.constraints.horizontal.min, Some(CLOSE_HIT));
        assert_eq!(close.constraints.vertical.min, Some(CLOSE_HIT));
        assert_eq!(
            close.constraints.horizontal.max,
            Some(CLOSE_HIT),
            "an icon-only close is pinned square at Carbon's hit box; the \
             open ceiling was there to let the word decide the width"
        );
        assert_eq!(
            close.semantics.focus_figure,
            FocusFigure::BarUnder,
            "the header has the dialog's body under it, so the bar goes \
             below the button rather than inside its fill"
        );
        assert_eq!(
            close.props.tokens.get("border"),
            None,
            "the flat glyph carries no boundary"
        );
        assert_eq!(CLOSE_HIT, 48.0);
        assert_eq!(CLOSE_ICON, 20.0, "T070: SCSS 20, not the style-page 16");
        assert!(
            close.props.text.is_none(),
            "Close is a labelled control, not an icon-only leaf"
        );
        assert_eq!(token(close, "background@hover"), Some(LAYER_HOVER));
        assert_eq!(
            token(close, "background"),
            Some(SURFACE_RAISED),
            "a resting `background` must be bound alongside `background@hover`, \
             or the close button paints nothing when it is not hovered — the \
             paint pass counts that as silent, not empty"
        );

        let footer = descendant(&node, "footer");
        assert_eq!(
            footer.props.rows,
            vec![TrackSize::Fixed {
                value: FOOTER_HEIGHT
            }]
        );
        assert_eq!(FOOTER_HEIGHT, 64.0);
        match &footer.props.columns[..] {
            [
                TrackSize::Weight { weight: a },
                TrackSize::Weight { weight: b },
            ] => {
                assert_eq!(a, b, "50% / 50%")
            }
            other => panic!("footer columns: {other:?}"),
        }
        let cancel = descendant(footer, "cancel");
        let primary = descendant(footer, "primary");
        assert_eq!(cancel.semantics.label.as_deref(), Some("Cancel"));
        assert_eq!(primary.semantics.label.as_deref(), Some("Retire"));
        for (name, b) in [("cancel", cancel), ("primary", primary)] {
            assert_eq!(b.semantics.role, Some(Role::Button), "{name}");
            assert!(b.interactions.contains(&Interaction::Click), "{name}");
            assert_eq!(b.constraints.vertical.min, Some(FOOTER_HEIGHT), "{name}");
            assert_eq!(b.constraints.vertical.max, Some(FOOTER_HEIGHT), "{name}");
            assert_eq!(token(b, "shadow"), None, "{name}: the footer is flat");
        }
        assert_ne!(
            token(cancel, "background"),
            token(primary, "background"),
            "Cancel is the secondary fill, the action is the accent: two \
             channels (fill and position) tell them apart, never hue alone"
        );
        assert_ne!(
            token(cancel, "background"),
            token(descendant(&node, "dialog"), "background"),
            "Cancel is a tone ahead of the container, not the container's own fill"
        );
    }

    #[test]
    fn a_passive_modal_has_title_body_and_close_but_no_footer() {
        let node = modal_passive("info", "About", "Version 3.");
        assert!(has_descendant(&node, "title"));
        assert!(has_descendant(&node, "body"));
        assert!(has_descendant(&node, "close"));
        assert!(!has_descendant(&node, "footer"));
        assert!(!has_descendant(&node, "cancel"));
        assert!(!has_descendant(&node, "primary"));
        assert_eq!(node.semantics.label.as_deref(), Some("About"));
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        // Every shipped transition name is declared here for the same reason
        // `component::tests`'s own harness declares them: a `button` names
        // `crate::anim::BUTTON_PRESS` and a `toggle` knob names `TOGGLE_KNOB`,
        // and tree acceptance refuses a name the registry has not been told
        // about. A host does this in `Host::new`; a test that builds its own
        // registry has to do it too.
        let mut registry = Registry::with_vocabulary(standard_vocabulary());
        crate::anim::shipped_registry().declare_into(&mut registry);
        registry
    }

    /// The modal mounted the way an application mounts it: inside a card,
    /// inside a page column, never at the root.
    fn page_around(node: ViewNode) -> ViewNode {
        let card = ViewNode::new(NodeKind::Stack, "card")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(crate::component::text(
                "intro",
                "The page behind the dialog.",
            ))
            .child(node);
        ViewNode::new(NodeKind::Grid, "root")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 240.0 },
                    TrackSize::Weight { weight: 1.0 },
                ],
                ..Props::default()
            })
            .child(crate::component::button("nav", "Next"))
            .child(card)
    }

    fn petrify_page(root: &ViewNode) -> PetrifiedFrame {
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        petrify_page(&page_around(node))
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    fn placed<'a>(frame: &'a PetrifiedFrame, tail: &str) -> &'a crate::frame::Placement {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(tail))
            .unwrap_or_else(|| panic!("no placement ends with {tail}"))
    }

    /// Row 20's defect, at the frame level: mounted inside a card in the
    /// right-hand column of a page, the surface still covers the whole
    /// window, and the container is 60% of the window wide and centred in
    /// it — not centred in the card, and not the card's width.
    #[test]
    fn mounted_inside_a_card_the_modal_covers_the_window_and_centres_a_sixty_percent_dialog() {
        let frame = petrify_lone(rebuild());
        let window = Rect::new(0.0, 0.0, VIEWPORT.w, VIEWPORT.h);
        let scrim = placed(&frame, "/card/retire");
        assert_eq!(scrim.rect, window, "the scrim is the window");
        let dialog = placed(&frame, "/retire/frame/seat/dialog");
        assert!(
            (dialog.rect.x - VIEWPORT.w * 0.2).abs() < 0.5,
            "dialog starts at 20% of the window, got x={}",
            dialog.rect.x
        );
        assert!(
            (dialog.rect.w - VIEWPORT.w * 0.6).abs() < 0.5,
            "dialog is 60% of the window wide, got w={}",
            dialog.rect.w
        );
        let centre_y = dialog.rect.y + dialog.rect.h / 2.0;
        assert!(
            (centre_y - VIEWPORT.h / 2.0).abs() < 1.0,
            "dialog is vertically centred in the window, centre_y={centre_y}"
        );
        assert!(
            dialog.rect.h > 64.0 + 48.0,
            "header, body and a 64-tall footer: got h={}",
            dialog.rect.h
        );
        let footer = placed(&frame, "/dialog/footer");
        assert_eq!(footer.rect.h, FOOTER_HEIGHT);
        assert_eq!(footer.rect.bottom(), dialog.rect.bottom(), "full-bleed");
        let cancel = placed(&frame, "/footer/cancel");
        let primary = placed(&frame, "/footer/primary");
        assert!((cancel.rect.w - primary.rect.w).abs() < 0.5, "50% / 50%");
        assert_eq!(cancel.rect.h, FOOTER_HEIGHT);
        assert_eq!(cancel.rect.x, dialog.rect.x);
        assert!((primary.rect.right() - dialog.rect.right()).abs() < 0.5);
        assert!(
            cancel.rect.x < primary.rect.x,
            "Cancel on the left, the action on the right"
        );
        let close = placed(&frame, "/header/close");
        assert!(
            (close.rect.right() - dialog.rect.right()).abs() < 0.5,
            "Close is flush with the container's right edge"
        );
        assert_eq!(close.rect.y, dialog.rect.y, "and its top");
    }

    /// The card that declares the modal is not made taller by it: the
    /// surface takes no flow room, so the intro paragraph's rect is the
    /// card's whole content.
    #[test]
    fn the_modal_takes_no_room_in_the_card_that_declares_it() {
        let with = petrify_lone(rebuild());
        let without = petrify_page(&page_around(crate::component::text("stub", "")));
        let intro_with = placed(&with, "/card/intro").rect;
        let intro_without = placed(&without, "/card/intro").rect;
        assert_eq!(intro_with, intro_without);
    }

    /// The input side of the same picture: a press on the dimmed page
    /// reaches nothing, a press on the footer's primary button reaches it,
    /// and a press on the page's own button behind the scrim is swallowed.
    #[test]
    fn a_press_on_the_dimmed_page_lands_nowhere_and_a_press_on_the_footer_lands_on_its_button() {
        let root = page_around(rebuild());
        let frame = petrify_page(&root);
        let scopes = surface_scopes(&root);
        let press = |pos: Point| InputEvent::PointerPressed {
            pos,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        let nav = placed(&frame, "/root/nav").rect;
        let on_nav = route_with_surfaces(
            &frame,
            None,
            &press(Point::new(nav.x + nav.w / 2.0, nav.y + nav.h / 2.0)),
            &scopes,
            None,
        );
        assert!(
            matches!(on_nav.route, Route::Unrouted { .. }),
            "the page's Next button is behind the scrim and must not be \
             reached, got {:?}",
            on_nav.route
        );
        let primary = placed(&frame, "/footer/primary");
        let on_primary = route_with_surfaces(
            &frame,
            None,
            &press(Point::new(
                primary.rect.x + primary.rect.w / 2.0,
                primary.rect.y + primary.rect.h / 2.0,
            )),
            &scopes,
            None,
        );
        assert_eq!(
            on_primary.route,
            Route::Pointer {
                node: primary.id.clone()
            }
        );
    }

    /// Check C/D: the scrim, the container, its header, body, footer and
    /// every button place with real rects, none of them outside their
    /// parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(rebuild());
        assert!(!frame.placements.is_empty(), "nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{} drew content larger than its own rect",
                p.id
            );
            if let Some(parent_idx) = p.parent {
                let parent = &frame.placements[parent_idx];
                // The scrim is the one placement allowed past its parent:
                // it is placed against the window, not the card.
                if p.kind == NodeKind::Surface {
                    continue;
                }
                let fits = p.rect.x >= parent.rect.x - 0.01
                    && p.rect.y >= parent.rect.y - 0.01
                    && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                    && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                assert!(
                    fits,
                    "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                    p.id, p.rect, parent.id, parent.rect
                );
            }
        }
    }

    /// Check F: every button in the dialog declares Focus and is reachable,
    /// and all of them sit in the modal's own focus scope.
    #[test]
    fn close_cancel_and_primary_are_reachable_inside_the_trap() {
        let root = page_around(rebuild());
        let frame = petrify_page(&root);
        let focus =
            crate::focus::FocusTree::from_placements(&frame.placements, &surface_scopes(&root));
        let order = focus.order();
        for tail in ["/header/close", "/footer/cancel", "/footer/primary"] {
            let id = placed(&frame, tail).id.clone();
            assert!(
                order.iter().any(|o| o == &id),
                "{tail} declares Focus but is not in focus order"
            );
            assert_eq!(
                focus.scope_of(&id),
                Some(placed(&frame, "/card/retire").id.as_str()),
                "{tail} is inside the modal's focus trap"
            );
        }
    }

    /// Check E: title, body and every button label against the surface
    /// each actually sits on — the container's fill for title/body, each
    /// button's own resting fill for its label — in both themes.
    #[test]
    fn dialog_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = rebuild();
            let dialog = descendant(&node, "dialog");
            let dialog_bg_name = dialog
                .props
                .tokens
                .get("background")
                .expect("the container binds a resting background");
            let dialog_bg = color(&theme, dialog_bg_name.as_str());
            fn walk_text(
                node: &ViewNode,
                inherited_bg: ColorValue,
                theme: &Theme,
                min: f32,
                get_color: &impl Fn(&Theme, &str) -> ColorValue,
            ) {
                let bg = match node.props.tokens.get("background") {
                    Some(name) => get_color(theme, name.as_str()),
                    None => inherited_bg,
                };
                if node.props.text.is_some()
                    && let Some(fg_name) = node.props.tokens.get("foreground")
                {
                    let opacity = node.props.opacity.unwrap_or(1.0);
                    let fg = get_color(theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= min,
                        "{:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                        node.key,
                        fg_name.as_str()
                    );
                }
                for child in &node.children {
                    walk_text(child, bg, theme, min, get_color);
                }
            }
            walk_text(dialog, dialog_bg, &theme, MIN_TEXT_CONTRAST, &color);
        }
    }
}
