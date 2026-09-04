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
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
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
    use crate::tree::{Interaction, NodeKind, Role, ViewNode};

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
            named(&zone, "prompt").props.text.as_deref(),
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
}
