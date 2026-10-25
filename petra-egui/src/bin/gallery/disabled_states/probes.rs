//! T027's measurement and T028's interaction/focus assertions, over every
//! disabled cell the gallery exhibits.
//!
//! The per-state cases (`slice_a.rs` … `slice_f.rs`) read the treatment off
//! the built cells as data. These two probes drive it: one measures what a
//! reader actually sees (contrast as drawn, through whatever opacity is in
//! force — FR-023), and one pushes at the input surface to show a disabled
//! node takes no interaction state and cannot be focused (T028).

use std::collections::BTreeMap;

use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Point, Size};
use gorgon_petra::input::{InputEvent, KeyCode, Modifiers, Route, hit_test, route};
use gorgon_petra::layout::LayoutState;
use gorgon_petra::testing::{Harness, extended_vocabulary, validated_with};
use gorgon_petra::token::measure::label_contrasts;
use gorgon_petra::token::{DerivedState, ThemeMode, dark, light};
use gorgon_petra::tree::{Interaction, Registry, ViewNode};

use super::{FLOOR, cells};

/// Every component the manifest marks with a disabled state (T023's
/// enumeration): its 29 `| state | disabled |` rows. Form's second row,
/// `disabled-fieldset`, is covered by its own case in `slice_b.rs`.
const DISABLED_COMPONENTS: [&str; 29] = [
    "Accordion",
    "Button",
    "Checkbox",
    "Code snippet",
    "Contained list",
    "Content switcher",
    "Data table",
    "Date picker",
    "Dropdown",
    "File uploader",
    "Form",
    "Link",
    "Menu",
    "Menu buttons",
    "Notification",
    "Number input",
    "Pagination",
    "Progress indicator",
    "Radio button",
    "Search",
    "Select",
    "Slider",
    "Structured list",
    "Tabs",
    "Tag",
    "Text input",
    "Tile",
    "Toggle",
    "Tree view",
];

/// Petrify one cell the way `Host::new` would accept it: its own vocabulary
/// plus every shipped transition name (`text_contrast.rs`'s `place`, for the
/// same reason — a bare vocabulary refuses a cell that names a shipped
/// animation, and declaring the shipped set keeps an invented name refused).
fn place(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    harness.state = LayoutState::default();
    let mut registry = Registry::with_vocabulary(extended_vocabulary(tree));
    gorgon_petra::anim::shipped_registry().declare_into(&mut registry);
    petrify(
        1,
        validated_with(tree, &registry),
        &mut harness.ctx(),
        Viewport::new(Size::new(600.0, 400.0), ThemeMode::Light),
        TransitionActivity::default(),
    )
}

/// T027: every disabled label in the gallery, measured **through** whatever
/// opacity the treatment applies (FR-023) — the hole spec 005 FR-010 named
/// at `gallery.rs:497`, closed.
///
/// The measurement is `gorgon_petra::token::measure`'s, the same one
/// `petra-egui/tests/text_contrast.rs` gates on: the resolved ink (through
/// the state slot the painter would use) faded by the cumulative placement
/// opacity, composited over the ground actually under it. One line per
/// label is printed, so the run records the numbers and not just the
/// verdict. Where a label fails, the fix is the treatment — the binding —
/// never this floor.
#[test]
fn every_disabled_label_in_the_gallery_clears_the_floor_as_drawn() {
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for (theme_name, theme) in [("light", light()), ("dark", dark())] {
        for component in DISABLED_COMPONENTS {
            for (at, cell) in cells(component).iter().enumerate() {
                let frame = place(cell);
                for (id, ratio, opacity) in label_contrasts(&frame, &theme) {
                    let line = format!(
                        "{component}[{at}] {theme_name} {id}: {ratio:.2}:1 \
                         at opacity {opacity:.2}"
                    );
                    if ratio < FLOOR {
                        failures.push(line);
                    } else {
                        report.push(line);
                    }
                }
            }
        }
    }
    for line in &report {
        eprintln!("{line}");
    }
    assert!(
        failures.is_empty(),
        "disabled labels under the {FLOOR}:1 floor as drawn:\n  {}",
        failures.join("\n  ")
    );
    assert!(
        report.len() >= 29,
        "the probe measured only {} labels across 29 components, so it \
         measured nothing",
        report.len()
    );
}

/// T028: a disabled node takes no interaction state.
///
/// Two halves, because "takes none" is a fact about both the node and the
/// state machine: the built cells declare no interaction at all (nothing to
/// take — hover, press, capture and focus all need a declaration), and
/// forcing hover and active onto the derived state anyway does not move the
/// resolved rank off Disabled or a single slot off the disabled family.
#[test]
fn a_disabled_node_takes_no_interaction_state() {
    for component in DISABLED_COMPONENTS {
        for (at, cell) in cells(component).iter().enumerate() {
            let frame = place(cell);
            for (index, placement) in frame.placements.iter().enumerate() {
                if !placement.semantics.disabled {
                    continue;
                }
                let where_ = format!("{component}[{at}] {}", placement.id);
                assert!(
                    placement.semantics.actions.is_empty(),
                    "{where_}: disabled but reachable through {:?}",
                    placement.semantics.actions
                );
                let tokens = &frame.content[index].tokens;
                let plain = DerivedState {
                    disabled: true,
                    ..DerivedState::default()
                };
                let forced = DerivedState {
                    disabled: true,
                    hovered: true,
                    active: true,
                    ..DerivedState::default()
                };
                for slot in ["foreground", "background", "border", "icon"] {
                    assert_eq!(
                        gorgon_petra::token::resolve_slot(tokens, slot, plain),
                        gorgon_petra::token::resolve_slot(tokens, slot, forced),
                        "{where_}: forcing hover and active changed what \
                         `{slot}` resolves to, so the node took an \
                         interaction state it must refuse"
                    );
                }
            }
        }
    }
}

/// T028: a disabled node is not focusable.
///
/// Focus in this engine is routed to (keyboard), hit-tested (pointer), or
/// tabbed to through [`FocusTree`]'s order, and all three are pushed here
/// against the built cells: no placement inside a disabled subtree is in the
/// focus order, none is ever the answer to a hit test, and a keyboard route
/// aimed at one climbs past it rather than delivering.
#[test]
fn a_disabled_node_is_not_focusable() {
    let key = InputEvent::Key {
        key: KeyCode::Tab,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    for component in DISABLED_COMPONENTS {
        for (at, cell) in cells(component).iter().enumerate() {
            let frame = place(cell);
            let disabled: Vec<usize> = frame
                .placements
                .iter()
                .enumerate()
                .filter(|(_, p)| p.semantics.disabled)
                .map(|(index, _)| index)
                .collect();
            let focus = FocusTree::from_placements(&frame.placements, &BTreeMap::new());
            let order = focus.order();
            for &index in &disabled {
                let placement = &frame.placements[index];
                let where_ = format!("{component}[{at}] {}", placement.id);
                assert!(
                    !order.contains(&placement.id),
                    "{where_}: a disabled node is in the focus order {order:?}"
                );
                let centre = Point::new(
                    placement.rect.x + placement.rect.w / 2.0,
                    placement.rect.y + placement.rect.h / 2.0,
                );
                for interaction in [Interaction::Click, Interaction::Hover, Interaction::Focus] {
                    if let Some(hit) = hit_test(&frame, centre, interaction) {
                        assert!(
                            !hit.semantics.disabled,
                            "{where_}: hit test landed on a disabled node \
                             for {interaction:?}"
                        );
                    }
                }
                let answer = route(&frame, Some(placement.id.as_str()), &key);
                match answer {
                    Route::Keyboard { node } | Route::Pointer { node } => assert!(
                        !frame.placement(&node).is_some_and(|p| p.semantics.disabled),
                        "{where_}: a keyboard route delivered to disabled \
                         `{node}`"
                    ),
                    Route::Raw { .. } | Route::Reserved { .. } => panic!(
                        "{where_}: plain `route` produced a verbatim-delivery \
                         route, which its docs forbid"
                    ),
                    Route::Unrouted { .. } => {}
                }
            }
        }
    }
}
