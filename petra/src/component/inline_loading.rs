//! `inline_loading` — Carbon Inline loading (slice-b).
//!
//! Status of an in-flight action. Carbon's active state is the small
//! Loading spinner — a 16×16 track ring with a turning 48% arc, the same
//! markup `loading.scss` draws (`_inline-loading.scss` `@use '../loading'`)
//! — so the active mark here is [`super::loading`]'s small spinner at the
//! host clock `now`, and it turns for the same reason and by the same
//! mechanism. Finished adds [`IconMark::Check`] on a disc.
//!
//! Spinner size 16 is MEASURED `_loading.scss` `--small`. Container floor
//! 32 (`min-block-size: 2rem`, `_inline-loading.scss:30`). Gap spinner-to-
//! label is [`SPACING_03`] (8, `margin-inline-end` on `__animation`).
//!
//! # The finished disc is the accent, not Carbon's green
//!
//! Carbon fills `CheckmarkFilled` in `$support-success`. Petra's
//! `support-success` aliases `status.ok`, a near-white mint in both themes
//! (`token::shipped`), and a glyph in that tone on the light theme's ground
//! is about 1.2:1 — the status palette carries its meaning through a shape
//! and a word, not through a glyph. So the disc spends the accent, and the
//! check is the channel: a disc with a tick against a ring with an arc.

use super::icon::{IconMark, icon};
use super::loading::spinner_small;
use super::stack;
use super::text::text;
use super::tokens::{ACCENT_PRIMARY, SPACING_03, TEXT_MUTED, t};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{AxisConstraint, Constraints, Justify, Key, Role, Semantics, ViewNode};

/// Carbon small loading spinner, the only size Inline loading uses.
const SPINNER: f32 = 16.0;
/// Container `min-block-size: 2rem`. MEASURED `_inline-loading.scss:30`.
const MIN_BLOCK: f32 = 32.0;

/// An action in flight: the small spinner at the host clock `now`
/// (seconds), then `label`.
///
/// `label` is required ([`Role::Status`] needs a label).
/// [`Semantics.value`] is `"loading"`.
pub fn inline_loading(key: impl Into<Key>, label: impl Into<String>, now: f64) -> ViewNode {
    row(key, label, spinner_small("mark", now), "loading")
}

/// An action that finished: a check on an accent disc, then `label`.
/// [`Semantics.value`] is `"finished"`.
pub fn inline_loading_finished(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let mut badge = stack(
        "mark",
        Axis::Horizontal,
        None,
        vec![icon("tick", IconMark::Check)],
    );
    // `align` is the *cross* axis and `justify` is the main one. With only
    // `align` set, the 10-unit tick packed against the disc's leading edge
    // — 3 units left of centre inside the 16 disc, measured off
    // `14-inline-loading.png` on 2026-09-05 — and read as a mark falling out
    // of its own badge. That is the same one-line defect
    // `every_header_icon_button_centres_its_glyph` already pins on the shell
    // header's 20-unit glyphs.
    badge.props.align = Some(Align::Center);
    badge.props.justify = Some(Justify::Center);
    badge
        .props
        .tokens
        .insert("background".into(), t(ACCENT_PRIMARY));
    // FR-022: a spinner ring is a disc at whatever size it ships, so it
    // says `CornerRole::Pill` rather than leaning on the half-edge clause.
    // `Floating`'s 8 would also reach `shape.corner-full` at today's 16
    // units, and would silently stop the day the ring grew.
    badge
        .props
        .tokens
        .insert("radius".into(), t(corner_for(CornerRole::Pill, SPINNER)));
    let badge = badge.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(SPINNER),
            max: Some(SPINNER),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(SPINNER),
            max: Some(SPINNER),
            priority: 0,
        },
    });
    row(key, label, badge, "finished")
}

fn row(key: impl Into<Key>, label: impl Into<String>, mark: ViewNode, value: &str) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));

    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![mark, caption]);
    node.props.align = Some(Align::Center);
    node.constraints.vertical.min = Some(MIN_BLOCK);
    node.semantics = Semantics {
        role: Some(Role::Status),
        label: Some(label),
        value: Some(value.into()),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::{MIN_BLOCK, SPINNER, inline_loading, inline_loading_finished};
    use crate::component::icon::IconMark;
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::draw::Command;

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    /// Row 14: the active mark is the small spinner — an open arc that
    /// turns with the clock on an ambient canvas — not a still ring.
    #[test]
    fn active_is_status_loading_with_a_turning_arc() {
        let node = inline_loading("save", "Saving", 0.0);
        assert_eq!(node.semantics.role, Some(Role::Status));
        assert_eq!(node.semantics.label.as_deref(), Some("Saving"));
        assert_eq!(node.semantics.value.as_deref(), Some("loading"));
        assert!(!node.is_interactive());
        assert_eq!(node.constraints.vertical.min, Some(MIN_BLOCK));
        assert_eq!(MIN_BLOCK, 32.0);
        let mark = child(&node, "mark");
        assert_eq!(mark.kind, NodeKind::Canvas);
        assert!(mark.ambient, "the spinner canvas is rebuilt every frame");
        assert_eq!(mark.constraints.horizontal.min, Some(SPINNER));
        assert_eq!(mark.constraints.vertical.min, Some(SPINNER));
        assert_eq!(SPINNER, 16.0);
        let list = mark.props.canvas.as_ref().expect("the spinner is a canvas");
        assert!(
            list.commands()
                .iter()
                .any(|c| matches!(c, Command::Path { closed: false, .. })),
            "an arc with a gap, not a closed ring: {:?}",
            list.commands()
        );
        assert_ne!(
            child(&inline_loading("save", "Saving", 0.1), "mark")
                .props
                .canvas,
            mark.props.canvas,
            "a later clock is a different picture"
        );
        assert_eq!(child(&node, "label").props.text.as_deref(), Some("Saving"));
    }

    #[test]
    fn finished_adds_a_check_mark() {
        let node = inline_loading_finished("save", "Saved");
        assert_eq!(node.semantics.role, Some(Role::Status));
        assert_eq!(node.semantics.label.as_deref(), Some("Saved"));
        assert_eq!(node.semantics.value.as_deref(), Some("finished"));
        let mark = child(&node, "mark");
        assert!(!mark.ambient, "a finished mark is still");
        let tick = child(mark, "tick");
        assert_eq!(tick.kind, NodeKind::Canvas);
        let list = tick.props.canvas.as_ref().expect("Check draws a path");
        assert!(list.path_verbs() > 0);
        assert!(
            list.commands()
                .iter()
                .any(|c| matches!(c, Command::Path { .. }))
        );
        let _ = IconMark::Check;
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: both the active and the finished mark place with a real
    /// rect, none of them outside the row.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        for (label, node) in [
            ("active", inline_loading("save", "Saving", 0.0)),
            ("finished", inline_loading_finished("save", "Saved")),
        ] {
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
                    assert_fits_parent(label, p, &frame.placements[parent_idx]);
                }
            }
        }
    }

    /// Check F: `inline_loading` declares no interaction — it is a status
    /// readout, never a control — so neither state contributes anything to
    /// the focus order.
    #[test]
    fn declares_no_interaction_so_nothing_enters_focus_order() {
        for node in [
            inline_loading("save", "Saving", 0.0),
            inline_loading_finished("save", "Saved"),
        ] {
            assert!(!node.is_interactive());
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            assert!(
                focus.order().is_empty(),
                "a non-interactive status readout must contribute nothing \
                 to the focus order, found {:?}",
                focus.order()
            );
        }
    }

    /// The finished tick sits in the middle of its own disc.
    ///
    /// Asserted on the **placed rects**, because the defect it pins is a
    /// layout one and every `ViewNode`-level fact about it was already true
    /// while the mark sat 3 units left of centre: the badge declared its
    /// 16x16, the tick declared its 10x10, and neither says where the tick
    /// landed. Measured off `14-inline-loading.png` on 2026-09-05 before the
    /// fix, the tick's ink ran from 2 to 7.5 across a 16 disc.
    #[test]
    fn the_finished_tick_is_centred_in_its_disc() {
        let frame = petrify_lone(inline_loading_finished("save", "Saved"));
        let placed = |tail: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(tail))
                .unwrap_or_else(|| {
                    panic!(
                        "{tail} is not placed; placed: {:?}",
                        frame
                            .placements
                            .iter()
                            .map(|p| p.id.as_str())
                            .collect::<Vec<_>>()
                    )
                })
                .rect
        };
        let disc = placed("/mark");
        let tick = placed("/mark/tick");
        assert_eq!((disc.w, disc.h), (SPINNER, SPINNER));
        let dx = (tick.x + tick.w / 2.0) - (disc.x + disc.w / 2.0);
        let dy = (tick.y + tick.h / 2.0) - (disc.y + disc.h / 2.0);
        assert!(
            dx.abs() < 0.01 && dy.abs() < 0.01,
            "the tick sits {dx:+} , {dy:+} off the centre of its own disc              ({disc:?} vs {tick:?})"
        );
    }

    /// Check E: the label against the page ground it is read on
    /// (`surface.base` — `inline_loading` sets no fill of its own), read
    /// through `Props.opacity`, at both states.
    #[test]
    fn label_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            for node in [
                inline_loading("save", "Saving", 0.0),
                inline_loading_finished("save", "Saved"),
            ] {
                let label = child(&node, "label");
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "label at {ratio:.2}:1 against {SURFACE_BASE} fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }
}
