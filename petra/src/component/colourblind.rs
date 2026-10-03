//! The colour-blindness lane at the level of a rendered component.
//!
//! Test-only. [`crate::token::colourblind`] holds the arithmetic; this module
//! holds the questions a name-level sweep structurally cannot ask.
//!
//! The token gates in [`crate::token::shipped`] walk the shipped status and
//! support families and prove every pair stays ΔE\*ab
//! [`MIN_STATUS_SEPARATION`] apart to a deuteranope and to a protanope. That
//! is a statement about a vocabulary. It is not a statement about a screen,
//! and three gaps sit between the two:
//!
//! 1. **A component binds a name.** Nothing in a vocabulary sweep sees a
//!    component that paints its status in `accent.primary`, or in a fresh
//!    Carbon hex, because such a component never puts a `status.*` name
//!    anywhere for the sweep to find.
//! 2. **A colour is only ever half a status here.** FR-015 pairs every status
//!    colour with a shape and a text, and [`crate::token::StatusToken`] makes
//!    the pairing unrepresentable to omit — but a component is free to accept
//!    the token and then render the colour alone, and the type system has
//!    nothing more to say about that.
//! 3. **The ground is the component's, not the gate's.** The token sweep
//!    measures every status against `surface.raised` because it has no other
//!    ground to measure against. A component knows the fill it actually paints
//!    its mark on, and that fill is in the tree it returns.
//!
//! Each test below is one of those three, asked of every status-carrying
//! component and every status in the live vocabulary rather than of a list
//! written here.
//!
//! Every test in this file was proved live before it was trusted: on
//! 2026-09-11 each one was reverted against a real production edit and the
//! failure it printed recorded alongside the reverts
//! (`.agents/notes/implemented/testing/2026-09-11-the-colourblindness-lane-reaches-component-instances.md`).

use crate::component::icon::{IconBox, IconMark, IconTone, icon_in, icon_toned};
use crate::component::{
    avatar_with_status, checkbox_warning, field_warning, file_uploader_item_warning,
    number_input_warning, radio_warning, status, tag_status, textarea_warning,
};
use crate::token::colourblind::{
    MIN_STATUS_SEPARATION, MIN_SURFACE_CONTRAST, VISIONS, contrast, separation, status_family,
    theme_color,
};
use crate::token::shipped::{dark, light};
use crate::token::{StatusShape, StatusToken, TokenName};
use crate::tree::{Role, ViewNode};

/// The slot a component paints a status colour into.
const MARK_FILL: &str = "background";
/// The slot carrying the status's second visual channel.
const SILHOUETTE: &str = "silhouette";

/// The five shapes, so a sweep covers the whole channel rather than the one
/// shape a hand-written fixture happened to pick.
const SHAPES: [StatusShape; 5] = [
    StatusShape::Circle,
    StatusShape::Triangle,
    StatusShape::Square,
    StatusShape::Diamond,
    StatusShape::Octagon,
];

fn descendant<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
    if node.key.as_str() == key {
        return Some(node);
    }
    node.children.iter().find_map(|c| descendant(c, key))
}

fn slot<'a>(node: &'a ViewNode, name: &str) -> Option<&'a str> {
    node.props.tokens.get(name).map(TokenName::as_str)
}

/// The status mark a rendered component shows: the node carrying
/// [`Role::Status`]'s own colour. Every status-carrying component in this
/// crate keys it `dot`, and a component that stops doing so fails here rather
/// than silently dropping out of the sweep.
fn mark<'a>(node: &'a ViewNode, who: &str) -> &'a ViewNode {
    descendant(node, "dot").unwrap_or_else(|| panic!("{who} shows no status mark keyed `dot`"))
}

/// Every node in `node`'s tree claiming [`Role::Status`].
fn announcing_status(node: &ViewNode) -> Vec<&ViewNode> {
    let mut found = Vec::new();
    if node.semantics.role == Some(Role::Status) {
        found.push(node);
    }
    for child in &node.children {
        found.extend(announcing_status(child));
    }
    found
}

/// One status token per name in the live vocabulary, cycling the five shapes
/// so the sweep never measures one shape five times.
fn every_shipped_status() -> Vec<StatusToken> {
    status_family()
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            StatusToken::new(
                TokenName::new(&name).unwrap(),
                SHAPES[i % SHAPES.len()],
                name,
            )
            .unwrap()
        })
        .collect()
}

/// Every component that takes a [`StatusToken`], rendered, labelled by which
/// one it is.
fn status_carrying(token: &StatusToken) -> Vec<(&'static str, ViewNode)> {
    vec![
        ("status", status("s", token)),
        ("tag_status", tag_status("s", token)),
        ("avatar_with_status", avatar_with_status("s", "JD", token)),
    ]
}

/// Gap 1: a component paints the colour it was handed, and that colour is one
/// the separation gate actually sweeps.
///
/// Without this, `status.rs`'s `dot` could bind `accent.primary` — a real
/// colour, a real token, gate C1-8 satisfied, nothing invented — and the
/// whole separation lane would go on measuring three names no component
/// paints.
#[test]
fn every_status_a_component_paints_is_a_colour_the_separation_gate_sweeps() {
    let family = status_family();
    for token in every_shipped_status() {
        for (who, node) in status_carrying(&token) {
            let fill = slot(mark(&node, who), MARK_FILL)
                .unwrap_or_else(|| panic!("{who} paints no {MARK_FILL} on its status mark"));
            assert_eq!(
                fill,
                token.name().as_str(),
                "{who} was handed {} and painted {fill}; a component that \
                 substitutes its own hue leaves the token sweep measuring a \
                 colour nothing draws",
                token.name(),
            );
            assert!(
                family.iter().any(|n| n == fill),
                "{who} paints {fill}, which is outside the status and support \
                 families `status_family` sweeps, so no separation floor is \
                 ever applied to it"
            );
        }
    }
}

/// Gap 1, measured rather than inferred: the colours the components actually
/// bind stay apart, pair by pair, under both simulations.
///
/// The membership check above makes this follow from the token gate. It is
/// still run, and run on the colour read back out of the rendered tree,
/// because "follows from" is an argument and this is a measurement — and the
/// two disagree the moment a component binds an alias whose value drifts.
///
/// A single-token palette revert does not trip this: moving `status.degraded`
/// alone to Carbon's `#f1c21b` leaves every pair above the floor. The collapse
/// needs two Carbon hues at once — exactly the mistake the gate is here for.
#[test]
fn two_statuses_a_component_paints_stay_apart_to_a_dichromat() {
    let tokens = every_shipped_status();
    for (label, theme) in [("light", light()), ("dark", dark())] {
        for (i, a) in tokens.iter().enumerate() {
            for b in tokens.iter().skip(i + 1) {
                let (rendered_a, rendered_b) = (tag_status("s", a), tag_status("s", b));
                let fill_a = slot(mark(&rendered_a, "tag_status"), MARK_FILL).unwrap();
                let fill_b = slot(mark(&rendered_b, "tag_status"), MARK_FILL).unwrap();
                let (ca, cb) = (theme_color(&theme, fill_a), theme_color(&theme, fill_b));
                // Two names for one colour is the point of the `support-*`
                // aliases, not a failure: `support-error` *is* `status.down`.
                if ca == cb {
                    continue;
                }
                for (vision, matrix) in VISIONS {
                    let d = separation(ca, cb, matrix);
                    assert!(
                        d >= MIN_STATUS_SEPARATION,
                        "{label}: a tag showing {fill_a} and a tag showing \
                         {fill_b} are only ΔE*ab {d:.1} apart to a {vision} \
                         reader (floor is {MIN_STATUS_SEPARATION})"
                    );
                }
            }
        }
    }
}

/// Gap 2: the colour is never the only channel, in any component that shows a
/// status, for any shape.
///
/// [`StatusToken`] guarantees a shape and a text *exist*. This guarantees they
/// are *drawn*: the mark carries a silhouette slot, and exactly one node in
/// the rendered tree announces [`Role::Status`] carrying the token's own
/// words.
///
/// **Where those two channels sit differs by component, and both placements
/// are right.** [`status`] puts the role and the label on its wrapper and
/// shows the words as a visible `label` child beside the mark, because a
/// readout has room for a caption. [`avatar_with_status`] puts them on the pip
/// itself, because a 10-unit presence dot in the corner of a 32-unit avatar
/// has nowhere to put a word. So this sweep asks who announces the status
/// rather than demanding a fixed node, and the first cut of it — which
/// demanded the role on the mark — failed on [`status`] for exactly that
/// reason.
#[test]
fn a_status_mark_reads_without_its_colour_in_every_component_that_shows_one() {
    for shape in SHAPES {
        let token = StatusToken::new(
            TokenName::new("status.ok").unwrap(),
            shape,
            "fiber is running",
        )
        .unwrap();
        for (who, node) in status_carrying(&token) {
            assert!(
                slot(mark(&node, who), SILHOUETTE).is_some(),
                "{who} draws {shape:?} with no {SILHOUETTE}; to a reader who \
                 cannot separate the hues that mark is a blank swatch"
            );
            let announcers: Vec<&ViewNode> = announcing_status(&node);
            assert_eq!(
                announcers.len(),
                1,
                "{who} has {} nodes claiming Role::Status; a reader hears the \
                 status once or not at all, never twice",
                announcers.len()
            );
            assert_eq!(
                announcers[0].semantics.label.as_deref(),
                Some("fiber is running"),
                "{who} drops the token's text channel, so the status has no \
                 reading at all once colour and shape are both unavailable"
            );
        }
    }
}

/// Gap 2, the divergence that makes it worth sweeping: `avatar.rs` keeps its
/// own copy of `status.rs`'s shape table (its own doc says so). Two tables
/// agree until one is edited.
///
/// Checked through the rendered marks rather than by calling either table,
/// because the observable claim is that one status looks like itself wherever
/// it is shown, not that two private functions share a body.
#[test]
fn the_avatar_and_the_readout_agree_on_every_status_silhouette() {
    for shape in SHAPES {
        let token = StatusToken::new(TokenName::new("status.ok").unwrap(), shape, "here").unwrap();
        let readout = status("s", &token);
        let avatar = avatar_with_status("s", "JD", &token);
        let (a, b) = (
            mark(&readout, "status"),
            mark(&avatar, "avatar_with_status"),
        );
        assert_eq!(
            slot(a, SILHOUETTE),
            slot(b, SILHOUETTE),
            "{shape:?} is one figure in a status readout and a different one \
             on an avatar; avatar.rs's copy of the shape table has drifted"
        );
        assert_eq!(
            slot(a, "radius"),
            slot(b, "radius"),
            "{shape:?} is rounded one way in a status readout and another on \
             an avatar"
        );
    }
}

/// Gap 3: the mark is visible on the ground its own component paints under
/// it, not on the `surface.raised` the token gate assumes.
///
/// The ground is read out of the tree, so this measures what the component
/// composes rather than what this file guesses it composes.
#[test]
fn a_status_mark_clears_the_contrast_floor_against_its_own_components_ground() {
    for (label, theme) in [("light", light()), ("dark", dark())] {
        for token in every_shipped_status() {
            for (who, node, ground_key) in [
                ("tag_status", tag_status("s", &token), "s"),
                (
                    "avatar_with_status",
                    avatar_with_status("s", "JD", &token),
                    "face",
                ),
            ] {
                let fill = slot(mark(&node, who), MARK_FILL).unwrap();
                let Some(ground) = descendant(&node, ground_key).and_then(|g| slot(g, MARK_FILL))
                else {
                    panic!(
                        "{who} paints no {MARK_FILL} on `{ground_key}`, so this gate cannot \
                            tell what its mark sits on"
                    );
                };
                let ratio = contrast(theme_color(&theme, fill), theme_color(&theme, ground));
                // `status.ok` in light is a near-white mint tint on a
                // near-white raised surface; the token gate exempts the same
                // pair for the same reason, and the circle and the word carry
                // the mark there.
                if label == "light" && (fill == "status.ok" || fill == "support-success") {
                    continue;
                }
                assert!(
                    ratio >= MIN_SURFACE_CONTRAST,
                    "{label}: {who} paints {fill} on {ground} at {ratio:.2}:1, \
                     under the {MIN_SURFACE_CONTRAST}:1 floor — the mark the \
                     shape channel lives on is not visible to anybody"
                );
            }
        }
    }
}

/// The four alert glyphs are four pictures, at one tone.
///
/// Carbon separates an error, a warning, an information and a success
/// notification by hue first. Petra's rule is that the silhouette carries it,
/// and for a notification the silhouette *is* the glyph. Holding
/// [`IconTone`] fixed removes colour from the comparison entirely, so a pair
/// that matches here matches on screen for every reader.
#[test]
fn the_four_alert_marks_are_four_different_pictures_at_one_tone() {
    let alerts = [
        IconMark::ErrorFilled,
        IconMark::WarningFilled,
        IconMark::InformationFilled,
        IconMark::CheckmarkFilled,
    ];
    let drawn: Vec<_> = alerts
        .iter()
        .map(|m| icon_toned("mark", *m, IconTone::Primary))
        .collect();
    for (i, a) in alerts.iter().enumerate() {
        let list_a = drawn[i]
            .props
            .canvas
            .as_ref()
            .unwrap_or_else(|| panic!("{a:?} draws no canvas"));
        for (j, b) in alerts.iter().enumerate().skip(i + 1) {
            let list_b = drawn[j].props.canvas.as_ref().unwrap();
            assert_ne!(
                list_a.commands(),
                list_b.commands(),
                "{a:?} and {b:?} are the same picture at one tone, so the two \
                 severities differ only in hue"
            );
        }
    }
}

/// Every warning variant says the word and draws the glyph.
///
/// Each of the six already has its own test naming its own chrome. This is
/// the sweep beside them: it holds the *shared* claim — a warning is a word
/// plus a picture, never a hue — in one place, so a seventh variant added
/// next to these six either joins the sweep or is visibly missing from it.
#[test]
fn every_warning_variant_says_warning_in_words_and_a_glyph() {
    let want_glyph = icon_in(
        "mark",
        IconMark::WarningFilled,
        IconBox::Glyph,
        IconTone::Primary,
    );
    let want_commands = want_glyph.props.canvas.as_ref().unwrap().commands();

    let variants: [(&str, ViewNode); 6] = [
        ("field_warning", field_warning("f", "Name", "check it")),
        (
            "textarea_warning",
            textarea_warning("t", "Notes", "check it"),
        ),
        (
            "number_input_warning",
            number_input_warning("n", "Count", "7", "check it"),
        ),
        (
            "checkbox_warning",
            checkbox_warning("c", "Agree", false, "check it"),
        ),
        (
            "radio_warning",
            radio_warning("r", "Other", false, "check it"),
        ),
        (
            "file_uploader_item_warning",
            file_uploader_item_warning("u", "run.log", "check it"),
        ),
    ];

    for (who, node) in variants {
        let helper = descendant(&node, "helper")
            .unwrap_or_else(|| panic!("{who} has no `helper` row, so it carries no warning text"));
        let message = descendant(helper, "message")
            .unwrap_or_else(|| panic!("{who}'s helper has no `message`"));
        assert_eq!(
            message.props.text.as_deref(),
            Some("Warning: check it"),
            "{who} does not say the word"
        );
        let glyph =
            descendant(helper, "mark").unwrap_or_else(|| panic!("{who}'s helper has no `mark`"));
        assert_eq!(
            glyph.props.canvas.as_ref().map(|l| l.commands()),
            Some(want_commands),
            "{who}'s helper draws something other than the warning glyph, so \
             the severity rests on the text alone"
        );
    }
}
