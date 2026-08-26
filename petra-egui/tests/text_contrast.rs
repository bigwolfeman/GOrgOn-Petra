//! A text-contrast gate that reads through `Props.opacity` (FR-010).
//!
//! # Why this could not be a token-value check
//!
//! Every contrast assertion in `gorgon_petra::token::shipped` compares two
//! *token values*: `text.primary` against `surface.raised` is 10.7:1 in the
//! light theme, and that number is true about the theme. It stops being true
//! about the screen the moment anything upstream composites the node —
//! `Props.opacity` fades a whole subtree at paint time, so a label declared at
//! 10.7:1 and drawn at 45% opacity reaches the reader at about 3.4:1 and every
//! value-level check still passes.
//!
//! That is not hypothetical. `examples/gallery.rs` expressed "this button is
//! unavailable" by fading the subtree, the suite could not see it, and the
//! only reason it was defensible is that WCAG exempts inactive components.
//! The exemption is real; the blindness was the problem, and this file is the
//! eye.
//!
//! # What it measures
//!
//! For every placement that draws text: the ink token the painter would
//! resolve (through `gorgon_petra::token::state`'s precedence chain, so a
//! hovered or disabled node is measured in the family it is actually painted
//! in), composited over the nearest ancestor's ground, **both faded by the
//! cumulative `Placement::opacity` the painter sets on its own painter
//! handle**. That is the pair a reader sees.
//!
//! Disabled and skeleton nodes are exempt from the 4.5:1 floor and are held
//! to a different rule instead: WCAG 2.1 SC 1.4.3 exempts inactive components,
//! and FR-010 requires the state to be carried by something other than colour
//! anyway.

use gorgon_petra::component::{button, disabled, primary_button, text};
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Size};
use gorgon_petra::layout::LayoutState;
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::{
    ColorValue, DerivedState, InteractionRank, Theme, ThemeMode, TokenName, TokenValue, light,
    resolve_slot, resolve_state,
};
use gorgon_petra::tree::{NodeKind, Props, ViewNode};
use gorgon_petra_egui::paint::{BACKGROUND_SLOT, DEFAULT_TEXT_TOKEN, FOREGROUND_SLOT};

/// WCAG 2.1 SC 1.4.3 at AA for body text.
const FLOOR: f32 = 4.5;

/// The ground a page paints on when nothing above a node binds one.
const PAGE: &str = "surface.base";

fn colour(theme: &Theme, token: &str) -> ColorValue {
    let name = TokenName::new(token).unwrap_or_else(|err| panic!("`{token}`: {err}"));
    match theme.value(&name) {
        Some(TokenValue::Color(c)) => *c,
        other => panic!("`{token}` is not a colour in this theme: {other:?}"),
    }
}

/// One node's resolved ink and ground, already faded and composited — what a
/// reader actually sees.
fn drawn_pair(frame: &PetrifiedFrame, theme: &Theme, index: usize) -> (ColorValue, ColorValue) {
    let placement = &frame.placements[index];
    let state = DerivedState::of(&placement.semantics);
    let ink = colour(
        theme,
        resolve_slot(&frame.content[index].tokens, FOREGROUND_SLOT, state)
            .unwrap_or(DEFAULT_TEXT_TOKEN),
    );

    // The ground is the nearest ancestor that binds one, resolved in *its*
    // own state — a hovered row repaints its fill, and its label is read
    // against the fill that is actually there. Walking rather than assuming
    // the parent, because a label's parent is often a bare layout box.
    let mut ground = colour(theme, PAGE);
    let mut ground_opacity = 1.0;
    let mut cursor = Some(index);
    while let Some(at) = cursor {
        let node = &frame.placements[at];
        if let Some(token) = resolve_slot(
            &frame.content[at].tokens,
            BACKGROUND_SLOT,
            DerivedState::of(&node.semantics),
        ) {
            ground = colour(theme, token);
            ground_opacity = node.opacity;
            break;
        }
        cursor = node.parent;
    }

    let page = colour(theme, PAGE);
    let ground = ground.faded(ground_opacity).over(page);
    let ink = ink.faded(placement.opacity).over(ground);
    (ink, ground)
}

/// Every text node whose ink cannot be read against its ground, as a
/// human-readable line each.
fn contrast_violations(frame: &PetrifiedFrame, theme: &Theme) -> Vec<String> {
    let mut out = Vec::new();
    for (index, placement) in frame.placements.iter().enumerate() {
        if frame.content[index].text.is_none() {
            continue;
        }
        let rank = resolve_state(DerivedState::of(&placement.semantics));
        if matches!(rank, InteractionRank::Disabled | InteractionRank::Skeleton) {
            // Exempt from the floor, and held to FR-010 instead — see
            // `a_disabled_control_is_identified_without_colour` below.
            continue;
        }
        let (ink, ground) = drawn_pair(frame, theme, index);
        let ratio = ink.contrast_ratio(ground);
        if ratio < FLOOR {
            out.push(format!(
                "{}: {ratio:.2}:1 at opacity {:.2}, under the {FLOOR}:1 floor",
                placement.id, placement.opacity
            ));
        }
    }
    out
}

fn place(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    harness.state = LayoutState::default();
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(600.0, 200.0), ThemeMode::Light),
        TransitionActivity::default(),
    )
}

/// A row of the library's own controls, one of them unavailable.
fn page() -> ViewNode {
    ViewNode::new(NodeKind::Stack, "app")
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            ..Props::default()
        })
        .child(text("caption", "Fibers waiting"))
        .child(button("cancel", "Cancel"))
        .child(primary_button("save", "Save"))
        .child(disabled(button("retire", "Retire")))
}

/// The shipped component library, measured the way it is drawn.
#[test]
fn every_live_label_clears_the_contrast_floor_as_drawn() {
    let theme = light();
    let frame = place(&page());
    let violations = contrast_violations(&frame, &theme);
    assert!(
        violations.is_empty(),
        "labels a reader cannot read:\n  {}",
        violations.join("\n  ")
    );
    // Not vacuous: the frame really does draw text.
    assert!(
        frame.content.iter().filter(|c| c.text.is_some()).count() >= 4,
        "the fixture drew no labels, so the gate measured nothing"
    );
}

/// The gate has teeth: a live label faded the way `gallery.rs` used to fade
/// one is caught.
///
/// This is the exact construction the gate exists for — `Props.opacity` on a
/// subtree whose token bindings are all perfectly legal — and the token
/// values are untouched, so a value-level check passes it. Only compositing
/// through the opacity finds it.
#[test]
fn a_faded_live_label_fails_the_contrast_gate() {
    let theme = light();
    let mut faded = button("cancel", "Cancel");
    faded.props.opacity = Some(0.2);
    let tree = ViewNode::new(NodeKind::Stack, "app")
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            ..Props::default()
        })
        .child(faded);
    let frame = place(&tree);

    let violations = contrast_violations(&frame, &theme);
    assert!(
        violations.iter().any(|v| v.contains("/app/cancel")),
        "a label faded to 20% passed a contrast gate: {violations:?}"
    );

    // And the same tree at full opacity does not, so what the gate found is
    // the fade and not the colours.
    let mut solid = button("cancel", "Cancel");
    solid.props.opacity = Some(1.0);
    let frame = place(
        &ViewNode::new(NodeKind::Stack, "app")
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            })
            .child(solid),
    );
    assert!(contrast_violations(&frame, &theme).is_empty());
}

/// A disabled control is identified without colour, which is why it may be
/// exempt from the floor at all (FR-010).
///
/// Two channels, both asserted here because the exemption is only defensible
/// if they exist:
///
/// * the **declaration** reaches the whole subtree, so the label is disabled
///   too and resolves the disabled ink family rather than the live one;
/// * the **elevation** is dropped by the painter for a disabled rank, which is
///   the channel a red-green colourblind reader is left with.
///
/// The second is asserted as the painter's rule rather than through a pixel
/// capture: `paint_one` skips the shadow block when the rank is disabled, and
/// what this file can check is that the rank is what the painter will see.
#[test]
fn a_disabled_control_is_identified_without_colour() {
    let frame = place(&page());
    let retire: Vec<&str> = frame
        .placements
        .iter()
        .filter(|p| p.id.starts_with("/app/retire"))
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(
        retire,
        vec!["/app/retire", "/app/retire/retire-label"],
        "the fixture must place both halves of the button"
    );

    for id in &retire {
        let placement = frame.placement(id).expect("placed");
        assert!(
            placement.semantics.disabled,
            "{id} is not declared disabled, so the painter will draw it live"
        );
        assert_eq!(
            resolve_state(DerivedState::of(&placement.semantics)),
            InteractionRank::Disabled
        );
        assert_eq!(
            placement.opacity, 1.0,
            "{id} is faded; the state must be carried by tokens and elevation, \
             not by a compositing property nothing can read"
        );
    }

    // The label resolves the disabled ink family, not the live one.
    let index = frame
        .placements
        .iter()
        .position(|p| p.id == "/app/retire/retire-label")
        .expect("placed");
    let placement = &frame.placements[index];
    let live = resolve_slot(
        &frame.content[index].tokens,
        FOREGROUND_SLOT,
        DerivedState::default(),
    );
    let drawn = resolve_slot(
        &frame.content[index].tokens,
        FOREGROUND_SLOT,
        DerivedState::of(&placement.semantics),
    );
    assert!(drawn.is_some() && drawn != live, "{live:?} vs {drawn:?}");
}
