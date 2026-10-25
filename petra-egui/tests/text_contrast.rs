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
//! Disabled and skeleton nodes are held to different rules than the floor:
//!
//! * **skeleton** nodes are exempt from the 4.5:1 floor. They are
//!   placeholders for content that has not arrived, and Phase 3's own gate
//!   holds them to the skeleton rules instead.
//! * **disabled** labels are **not** exempt (spec 008 FR-023). WCAG 2.1 SC
//!   1.4.3 exempts inactive components, and FR-010 requires the state to be
//!   carried by something other than colour anyway — but "exempt from the
//!   floor" was being read as "never measured", and a disabled label nobody
//!   can read is still a label nobody can read. The disabled treatment binds
//!   `text.muted` (`specs/008-petra-carbon-variants/contracts/disabled.md`)
//!   and clears 4.5:1 as drawn, so the floor holds with nothing riding on
//!   the exemption.
//!
//! The measurement itself is `gorgon_petra::token::measure`, shared with the
//! gallery's disabled-state cases so the gate and the per-state tests cannot
//! disagree about what "as drawn" means.

use gorgon_petra::component::{button, disabled, primary_button, text};
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Size};
use gorgon_petra::layout::LayoutState;
use gorgon_petra::testing::{Harness, extended_vocabulary, validated_with};
use gorgon_petra::token::measure::drawn_pair;
use gorgon_petra::token::{
    DerivedState, InteractionRank, Theme, ThemeMode, light, resolve_slot, resolve_state,
};
use gorgon_petra::tree::{NodeKind, Props, Registry, ViewNode};
use gorgon_petra_egui::paint::FOREGROUND_SLOT;

/// WCAG 2.1 SC 1.4.3 at AA for body text.
const FLOOR: f32 = 4.5;

/// Every text node whose ink cannot be read against its ground, as a
/// human-readable line each.
fn contrast_violations(frame: &PetrifiedFrame, theme: &Theme) -> Vec<String> {
    let mut out = Vec::new();
    for (index, placement) in frame.placements.iter().enumerate() {
        if frame.content[index].text.is_none() {
            continue;
        }
        let rank = resolve_state(DerivedState::of(&placement.semantics));
        if rank == InteractionRank::Skeleton {
            // Exempt from the floor and held to Phase 3's rules instead.
            // Disabled is not exempt — see the module docs (FR-023).
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
    // The tree's own vocabulary plus every shipped transition name, the way
    // `Host::new` builds a registry. `testing::validated`'s bare vocabulary
    // refuses the page below, because a `button` names
    // `anim::BUTTON_PRESS`. Declaring the shipped set rather than whatever
    // the tree says keeps acceptance honest here: an invented transition
    // name is still refused.
    let mut registry = Registry::with_vocabulary(extended_vocabulary(tree));
    gorgon_petra::anim::shipped_registry().declare_into(&mut registry);
    petrify(
        1,
        validated_with(tree, &registry),
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

/// The shipped component library, measured the way it is drawn — disabled
/// labels included (FR-023).
#[test]
fn every_label_clears_the_contrast_floor_as_drawn() {
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

/// A disabled label is measured through whatever opacity is applied to it
/// (FR-023) — the hole spec 005 FR-010 named, closed.
///
/// WCAG exempts inactive components, and this gate used to read that as
/// "skip disabled nodes", so a disabled subtree faded to nothing passed
/// every check here. The floor is the reader's, not the component's: a
/// label nobody can read is a label nobody can read. The disabled treatment
/// binds `text.muted` and applies no opacity of its own, so the unfaded
/// control clears the floor on its own numbers — and when a caller fades
/// one anyway, the gate says so instead of looking away.
#[test]
fn a_disabled_label_is_measured_through_the_applied_opacity() {
    let theme = light();
    let wrap = |node: ViewNode| {
        ViewNode::new(NodeKind::Stack, "app")
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            })
            .child(node)
    };

    let mut faded = disabled(button("retire", "Retire"));
    faded.props.opacity = Some(0.2);
    let frame = place(&wrap(faded));
    let violations = contrast_violations(&frame, &theme);
    assert!(
        violations.iter().any(|v| v.contains("/app/retire")),
        "a disabled label faded to 20% passed the contrast gate: {violations:?}"
    );

    // And the unfaded control clears the floor with room to spare, so what
    // the gate found is the fade and not the treatment. The number in the
    // message is the measurement FR-023 asks for: the label as a reader
    // meets it, through whatever opacity is in force.
    let frame = place(&wrap(disabled(button("retire", "Retire"))));
    let index = frame
        .placements
        .iter()
        .position(|p| p.id == "/app/retire/retire-label")
        .expect("placed");
    let (ink, ground) = drawn_pair(&frame, &theme, index);
    let ratio = ink.contrast_ratio(ground);
    assert!(
        ratio >= FLOOR,
        "a disabled label as drawn is {ratio:.2}:1, under the {FLOOR}:1 floor"
    );
    assert!(
        contrast_violations(&frame, &theme).is_empty(),
        "the unfaded disabled control must clear the gate on its own"
    );
}

/// A disabled control is identified without colour, which is why a muted
/// ink is defensible at all (FR-010) — and the muted ink still clears the
/// floor, so nothing here rests on an exemption (FR-023).
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
