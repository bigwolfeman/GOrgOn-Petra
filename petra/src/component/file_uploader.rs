//! Carbon File uploader (slice-b). OS drop is T140 / T148.
//!
//! Anatomy (usage page + `_file-uploader.scss`):
//! 1. Heading — the constructor `label` (Carbon default "Upload files").
//! 2. Drop zone — [`Role::Button`] that conceptually opens a file picker.
//!    Visible text is `"Drop files here"`. Border is solid
//!    [`BORDER_SUBTLE`]: Carbon draws a dashed `$border-strong`; Petra
//!    has no dashed stroke, and does not invent one.
//! 3. [`file_uploader_item`] — one selected-file row: the name plus
//!    either a loading mark or a check. No file bytes are stored here.
//!
//! The drop zone does **not** declare [`Interaction::Drag`]. OS file-drop
//! is a windowing event (T140), not pointer capture.

use super::icon::{IconMark, icon};
use super::pad;
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, LAYER_HOVER, SHAPE_FULL, SPACING_03, SPACING_05, SURFACE_BASE,
    SURFACE_RAISED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

/// MEASURED `_file-uploader.scss:425` drop-container `block-size`.
const DROP_HEIGHT: f32 = 96.0;
/// MEASURED uploaded-file width `18rem`.
const ITEM_WIDTH: f32 = 288.0;
/// MEASURED default selected-file `min-block-size` (`$spacing-09`).
const ITEM_HEIGHT: f32 = 48.0;
/// Small loading disc, same 16 as Inline loading's spinner.
const MARK: f32 = 16.0;

const _: () = assert!(DROP_HEIGHT == 96.0);
const _: () = assert!(ITEM_WIDTH == 288.0);
const _: () = assert!(ITEM_HEIGHT == 48.0);

const ZONE_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Drop-zone file uploader. `label` is the heading and the button name.
///
/// Visible zone copy is `"Drop files here"`. Clicking the zone is the
/// file-picker activation; dropped OS files are out of this constructor.
pub fn file_uploader(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut heading = text("heading", label.clone());
    heading
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let mut caption = text("prompt", "Drop files here");
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let mut zone = stack("zone", Axis::Vertical, None, vec![caption]);
    zone.props.align = Some(Align::Center);
    zone.props.padding = Some(pad(SPACING_05, SPACING_05));
    zone.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    zone.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    zone.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    zone.constraints.vertical.min = Some(DROP_HEIGHT);
    let zone = zone.interactive(Role::Button, label.clone(), ZONE_INTENTS);

    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), vec![heading, zone]);
    node.semantics.label = Some(label);
    node
}

/// One selected-file row. `complete` chooses check vs loading mark.
///
/// No file bytes. The row is not a drop target.
pub fn file_uploader_item(
    key: impl Into<Key>,
    name: impl Into<String>,
    complete: bool,
) -> ViewNode {
    let name = name.into();
    let mark = if complete {
        let mut badge = stack(
            "mark",
            Axis::Horizontal,
            None,
            vec![icon("tick", IconMark::Check)],
        );
        badge.props.align = Some(Align::Center);
        badge
            .props
            .tokens
            .insert("background".into(), t(ACCENT_PRIMARY));
        badge.props.tokens.insert("radius".into(), t(SHAPE_FULL));
        badge.with_constraints(pin_mark())
    } else {
        swatch(
            "mark",
            MARK,
            MARK,
            None,
            Some(BORDER_SUBTLE),
            Some(SHAPE_FULL),
        )
    };
    let status = if complete { "complete" } else { "uploading" };
    let mut status_text = text("status", status);
    status_text
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut filename = text("name", name.clone());
    filename
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![filename, mark, status_text],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    // MEASURED `_file-uploader.scss`: `.cds--file__selected-file` is
    // `background-color: $layer` (→ `SURFACE_RAISED`) and nothing else —
    // no border. Only the drop zone (`.cds--file__drop-container`) is
    // bordered. A `border` binding here drew an edge Carbon never draws:
    // this row is a "list strip"-shaped container (no interaction of its
    // own), and the fill alone is its whole boundary, the same class as
    // `list_row` and `tab` (see `containers_take_a_tone_and_controls_take_an_edge`'s
    // own doc for why a strip segment does not get an edge of its own).
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.constraints.horizontal.min = Some(ITEM_WIDTH);
    node.constraints.horizontal.max = Some(ITEM_WIDTH);
    node.constraints.vertical.min = Some(ITEM_HEIGHT);
    node.semantics.label = Some(name);
    node.semantics.value = Some(status.into());
    node
}

fn pin_mark() -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(MARK),
            max: Some(MARK),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(MARK),
            max: Some(MARK),
            priority: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{DROP_HEIGHT, ITEM_HEIGHT, ITEM_WIDTH, file_uploader, file_uploader_item};
    use crate::component::tokens::BORDER_SUBTLE;
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn no_drag(node: &ViewNode) {
        assert!(
            !node.interactions.contains(&Interaction::Drag),
            "file uploader `{}` declared Drag; OS drop is T140",
            node.key
        );
        for child in &node.children {
            no_drag(child);
        }
    }

    #[test]
    fn file_uploader_has_a_label_and_a_drop_zone() {
        let node = file_uploader("up", "Upload files");
        assert_eq!(node.semantics.label.as_deref(), Some("Upload files"));
        assert_eq!(
            named(&node, "heading").props.text.as_deref(),
            Some("Upload files")
        );
        let zone = named(&node, "zone");
        assert_eq!(zone.semantics.role, Some(Role::Button));
        assert_eq!(zone.semantics.label.as_deref(), Some("Upload files"));
        assert!(zone.interactions.contains(&Interaction::Click));
        assert!(zone.interactions.contains(&Interaction::Focus));
        assert!(!zone.interactions.contains(&Interaction::Drag));
        assert_eq!(
            named(zone, "prompt").props.text.as_deref(),
            Some("Drop files here")
        );
        assert_eq!(zone.constraints.vertical.min, Some(DROP_HEIGHT));
        assert_eq!(DROP_HEIGHT, 96.0);
        assert_eq!(token(zone, "border"), Some(BORDER_SUBTLE));
        no_drag(&node);
    }

    #[test]
    fn item_shows_check_when_complete_and_a_mark_when_not() {
        let done = file_uploader_item("f0", "notes.txt", true);
        assert_eq!(done.semantics.label.as_deref(), Some("notes.txt"));
        assert_eq!(done.semantics.value.as_deref(), Some("complete"));
        assert_eq!(
            named(&done, "name").props.text.as_deref(),
            Some("notes.txt")
        );
        assert_eq!(
            named(&done, "status").props.text.as_deref(),
            Some("complete")
        );
        assert_eq!(named(&done, "tick").kind, NodeKind::Canvas);
        assert!(
            token(&done, "border").is_none(),
            "MEASURED `_file-uploader.scss`: `.cds--file__selected-file` is \
             fill-only, no border; a strip segment does not draw an edge of \
             its own"
        );
        assert_eq!(done.constraints.horizontal.min, Some(ITEM_WIDTH));
        assert_eq!(done.constraints.horizontal.max, Some(ITEM_WIDTH));
        assert_eq!(ITEM_WIDTH, 288.0);
        assert_eq!(done.constraints.vertical.min, Some(ITEM_HEIGHT));
        assert_eq!(ITEM_HEIGHT, 48.0);
        no_drag(&done);

        let busy = file_uploader_item("f1", "notes.txt", false);
        assert_eq!(busy.semantics.value.as_deref(), Some("uploading"));
        assert_eq!(
            named(&busy, "status").props.text.as_deref(),
            Some("uploading")
        );
        let mark = named(&busy, "mark");
        assert_eq!(token(mark, "border"), Some(BORDER_SUBTLE));
        assert_eq!(
            token(mark, "radius"),
            Some(crate::component::tokens::SHAPE_FULL)
        );
        no_drag(&busy);
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

    /// Check C/D: the drop zone and both selected-file states place with a
    /// real rect, none of them outside their own row.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("zone", file_uploader("up", "Upload files")),
            ("item-complete", file_uploader_item("f0", "notes.txt", true)),
            (
                "item-uploading",
                file_uploader_item("f1", "notes.txt", false),
            ),
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

    /// Check F: the drop zone declares `Focus` and is reachable when
    /// enabled, and not when disabled. `file_uploader_item` declares no
    /// interaction of its own (no delete/retry control in this anatomy), so
    /// there is no positive case for it.
    #[test]
    fn the_drop_zone_is_reachable_unless_disabled() {
        let frame = petrify_lone(file_uploader("up", "Upload files"));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let zone = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/zone"))
            .expect("the zone is placed");
        assert!(
            focus.order().iter().any(|o| o == &zone.id),
            "the drop zone declares Focus but is not in focus order"
        );

        let disabled_frame = petrify_lone(crate::component::disabled(file_uploader(
            "up",
            "Upload files",
        )));
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_zone = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/zone"))
            .expect("the disabled zone is placed");
        assert!(
            !disabled_focus
                .order()
                .iter()
                .any(|o| o == &disabled_zone.id),
            "a disabled drop zone must not be reachable"
        );
    }

    /// Check E: the zone's prompt (against the page ground it sits on,
    /// `surface.base`) and a selected-file row's name/status text (against
    /// its own resting `surface.raised` fill), read through
    /// `Props.opacity`, in both themes.
    #[test]
    fn text_clears_aa_contrast_against_its_own_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let page_bg = color(&theme, SURFACE_BASE);
            let uploader = file_uploader("up", "Upload files");
            let prompt = named(&uploader, "prompt");
            let fg_name = prompt
                .props
                .tokens
                .get("foreground")
                .expect("prompt text binds a foreground");
            let opacity = prompt.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str()).faded(opacity).over(page_bg);
            let ratio = fg.contrast_ratio(page_bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "prompt at {ratio:.2}:1 against {SURFACE_BASE} fails AA {MIN_TEXT_CONTRAST}:1"
            );

            for complete in [true, false] {
                let item = file_uploader_item("f0", "notes.txt", complete);
                let bg_name = item
                    .props
                    .tokens
                    .get("background")
                    .expect("item binds a resting background");
                let bg = color(&theme, bg_name.as_str());
                for label_key in ["name", "status"] {
                    let label = named(&item, label_key);
                    let fg_name = label
                        .props
                        .tokens
                        .get("foreground")
                        .expect("label text binds a foreground");
                    let opacity = label.props.opacity.unwrap_or(1.0);
                    let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "complete={complete} {label_key} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                        bg_name.as_str()
                    );
                }
            }
        }
    }
}
