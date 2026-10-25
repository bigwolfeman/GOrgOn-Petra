//! Contrast as drawn, not as declared (spec 005 FR-010, spec 008 FR-023).
//!
//! Every contrast assertion that compares two *token values* is true about
//! the theme and stops being true about the screen the moment anything
//! upstream composites the node: `Props.opacity` fades a whole subtree at
//! paint time, so a label declared at 10.7:1 and drawn at 45% opacity
//! reaches the reader at about 3.4:1 and every value-level check still
//! passes. This module is the measurement that reads through that fade —
//! and through the state slot the painter would resolve, so a hovered or
//! disabled node is measured in the family it is actually painted in.
//!
//! One home for the compositing, used by `petra-egui`'s text-contrast gate
//! and by the gallery's disabled-state cases (`spec 008` T027): a gate and a
//! per-state test that computed their own fades could disagree about what
//! "as drawn" means while both stayed green.
//!
//! # The two holes this closes
//!
//! * spec 005 FR-010: `examples/gallery.rs` expressed "unavailable" by
//!   fading a subtree. The suite could not see it.
//! * spec 008 FR-023: disabled labels were exempt from the floor and held
//!   to a different rule, which meant nothing measured them at all. The
//!   disabled treatment binds `text.muted` (`contracts/disabled.md`), so
//!   they now clear 4.5:1 *as drawn* and the exemption can go.

use std::collections::BTreeMap;

use crate::frame::PetrifiedFrame;
use crate::token::state::{DerivedState, resolve_slot};
use crate::token::theme::Theme;
use crate::token::value::{ColorValue, TokenValue};

/// The ink slot a text placement paints from.
const FOREGROUND_SLOT: &str = "foreground";

/// The ground slot a node paints its fill from.
const BACKGROUND_SLOT: &str = "background";

/// What the painter draws when a text placement binds no ink slot.
const DEFAULT_TEXT_TOKEN: &str = "text.primary";

/// The ground a page paints on when nothing above a node binds one.
const PAGE: &str = "surface.base";

fn colour(theme: &Theme, token: &str) -> ColorValue {
    let name = crate::token::TokenName::new(token).unwrap_or_else(|err| panic!("`{token}`: {err}"));
    match theme.value(&name) {
        Some(TokenValue::Color(c)) => *c,
        other => panic!("`{token}` is not a colour in this theme: {other:?}"),
    }
}

/// One node's resolved ink and ground, already faded and composited — what a
/// reader actually sees.
///
/// The ground is the nearest ancestor that binds one, resolved in *its* own
/// state — a hovered row repaints its fill, and its label is read against the
/// fill that is actually there. Walking rather than assuming the parent,
/// because a label's parent is often a bare layout box. Both halves are
/// faded by the cumulative opacity the painter composited them under.
#[must_use]
pub fn drawn_pair(frame: &PetrifiedFrame, theme: &Theme, index: usize) -> (ColorValue, ColorValue) {
    let placement = &frame.placements[index];
    let state = DerivedState::of(&placement.semantics);
    let ink = colour(
        theme,
        resolve_slot(&frame.content[index].tokens, FOREGROUND_SLOT, state)
            .unwrap_or(DEFAULT_TEXT_TOKEN),
    );

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

/// Every text placement, measured as [`drawn_pair`] measures it: `(id,
/// ratio, opacity)` per label, in frame order.
#[must_use]
pub fn label_contrasts(frame: &PetrifiedFrame, theme: &Theme) -> Vec<(String, f32, f32)> {
    let mut out = Vec::new();
    for (index, placement) in frame.placements.iter().enumerate() {
        if frame.content[index].text.is_none() {
            continue;
        }
        let (ink, ground) = drawn_pair(frame, theme, index);
        out.push((
            placement.id.clone(),
            ink.contrast_ratio(ground),
            placement.opacity,
        ));
    }
    out
}

/// Convenience for gates that hold a map rather than a frame: the token
/// names one node binds, keyed by slot, as [`resolve_slot`] wants them.
#[must_use]
pub fn slot_map(tokens: &BTreeMap<String, crate::token::TokenName>) -> BTreeMap<String, String> {
    tokens
        .iter()
        .map(|(slot, name)| (slot.clone(), name.as_str().to_owned()))
        .collect()
}
