//! An inline field: a dense text input that stands where a row's label
//! stood, with its validity in a popup under the well.
//!
//! An editor names a thing where the thing is drawn. The explorer of every
//! workbench editor renames a file by turning its row into a field, and makes
//! a new one by opening a field row in the folder it goes in. A centred
//! dialog holding only a text box loses the one fact the operator needs most,
//! which entry the name is for, and it says nothing while he types.
//!
//! Anatomy:
//! 1. Well — a [`NodeKind::Input`] keyed `input`, [`INLINE_FIELD_HEIGHT`]
//!    tall so it sits inside a 22-unit workbench row with a unit to spare
//!    above and below. Its edge is one unit on all four sides, in
//!    [`ACCENT_PRIMARY`] while the value is legal (the workbench's "this is
//!    where the keys go" edge), and in [`SUPPORT_ERROR`] or
//!    [`SUPPORT_WARNING`] while it is not.
//! 2. Message — while the value breaks a rule, a [`NodeKind::Surface`] keyed
//!    `message` on [`Layer::Popup`], anchored under the well at its leading
//!    edge, at least as broad as it ([`Fit::Anchor`]), flipping above when
//!    the window has no room below. It holds the form family's state helper
//!    (`field.rs`): the state's glyph and `Invalid: …` or `Warning: …` in
//!    the primary ink. Its edge takes the state's hue as well.
//!
//! Colour is never the only channel. The operator is red-green colour
//! blind, so an error and a warning differ in their word and their glyph's
//! silhouette (circle-with-slash against triangle-with-bang), and a legal
//! value differs from both by having no message at all.
//!
//! The message floats and takes no room in flow, so a row that grows a
//! message keeps its height and the rows under it stay put, which is what a
//! virtualized list needs: its rows have one pitch.
//!
//! The constructor builds the well with the editable intents, like every
//! other field. An application that owns the typing (the field is drawn, the
//! app hears the keys) clears them, as it does for any field it drives.

use super::field::{invalid_helper, warning_helper};
use super::stack;
use super::tokens::{
    ACCENT_PRIMARY, SPACING_02, SPACING_03, SUPPORT_ERROR, SUPPORT_WARNING, SURFACE_BASE,
    SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT, t,
};
use crate::geom::{Align as CrossAlign, Axis};
use crate::tree::{
    Align, Anchor, AxisConstraint, ClampRule, Constraints, Edge, Fit, InputPolicy, InsetRefs,
    Interaction, Key, Layer, NodeKind, Props, Role, Semantics, TextCursor, Tip, TrackSize,
    ViewNode,
};

/// The well's height: a 22-unit workbench row less a unit above and below.
pub const INLINE_FIELD_HEIGHT: f32 = 20.0;

/// What a value is worth against the rules its field checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FieldState {
    /// The value can be committed.
    #[default]
    Valid,
    /// The value can be committed, and the reader should know this first.
    Warning(String),
    /// The value cannot be committed, for this reason.
    Invalid(String),
}

impl FieldState {
    /// True when the value cannot be committed.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid(_))
    }

    /// The sentence the message shows, when there is one.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Valid => None,
            Self::Warning(words) | Self::Invalid(words) => Some(words),
        }
    }
}

/// What an editable well declares. The same list as `field.rs`'s, which is
/// private to that module.
const EDITABLE: &[Interaction] = &[
    Interaction::Focus,
    Interaction::Click,
    Interaction::Key,
    Interaction::TextEdit,
];

/// An inline field keyed `key`: the well, named `label` for a reader and
/// holding `value` with `cursor` drawn in it, and the message `state` asks
/// for.
///
/// The node is a one-column grid that stretches its well across whatever
/// it is offered, so a caller puts it where a label went and gives it the
/// rest of the row.
#[must_use]
pub fn field_inline(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    cursor: TextCursor,
    state: &FieldState,
) -> ViewNode {
    let label = label.into();
    let edge = match state {
        FieldState::Valid => ACCENT_PRIMARY,
        FieldState::Warning(_) => SUPPORT_WARNING,
        FieldState::Invalid(_) => SUPPORT_ERROR,
    };
    let value = value.into();
    // The painter has one ink for a well; an empty well paints its
    // placeholder, which reads as a hint in the muted ink.
    let ink = if value.is_empty() {
        TEXT_MUTED
    } else {
        TEXT_PRIMARY
    };
    let mut props = Props {
        placeholder: Some(label.clone()),
        text: Some(value),
        cursor: Some(cursor),
        style: Some(t(TYPOGRAPHY_BODY_COMPACT)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(ink));
    // The well is one step darker than the row's own ground, as a workbench
    // input is, so it reads as a hole to type into rather than a raised chip.
    props.tokens.insert("background".into(), t(SURFACE_BASE));
    props.tokens.insert("border".into(), t(edge));
    let well = ViewNode::new(NodeKind::Input, "input")
        .with_props(props)
        .interactive(Role::TextInput, label, EDITABLE)
        .with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(INLINE_FIELD_HEIGHT),
                max: Some(INLINE_FIELD_HEIGHT),
                priority: 0,
            },
            ..Constraints::default()
        });

    let mut children = vec![well];
    if let Some(message) = message(state) {
        children.push(message);
    }
    // A vertical stack stretches the well across its own width; the message
    // floats and adds no height.
    let mut body = stack("body", Axis::Vertical, None, children);
    body.props.align = Some(CrossAlign::Stretch);
    // One weight column, as a tree row's detail holder is: offered the rest
    // of a row it takes all of it, so the well runs to the row's end instead
    // of hugging the name typed so far.
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            rows: vec![TrackSize::FitContent],
            align: Some(CrossAlign::Stretch),
            ..Props::default()
        })
        .with_children(vec![body])
}

/// The popup under the well, or nothing for a legal value.
fn message(state: &FieldState) -> Option<ViewNode> {
    let (helper, edge) = match state {
        FieldState::Valid => return None,
        FieldState::Warning(words) => (warning_helper(words.clone()), SUPPORT_WARNING),
        FieldState::Invalid(words) => (invalid_helper(words.clone()), SUPPORT_ERROR),
    };
    let words = state.message().unwrap_or_default().to_owned();
    let mut node = ViewNode::new(NodeKind::Surface, "message")
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Sibling {
                key: Key::new("input"),
                edge: Edge::Bottom,
                align: Align::Start,
                offset: None,
            }),
            clamp: Some(ClampRule::Flip),
            // The message is read, never pressed: a press on it lands on
            // whatever is under it, as if it were not there.
            input_policy: Some(InputPolicy::Passthrough),
            tip: Some(Tip::Flush),
            fit: Some(Fit::Anchor),
            padding: Some(InsetRefs {
                left: Some(t(SPACING_03)),
                right: Some(t(SPACING_03)),
                top: Some(t(SPACING_02)),
                bottom: Some(t(SPACING_02)),
            }),
            ..Props::default()
        })
        .child(helper);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(edge));
    node.semantics = Semantics {
        role: Some(Role::Status),
        label: Some(words),
        ..Semantics::default()
    };
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::NodeKind;

    fn find<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|child| find(child, key))
    }

    fn texts(node: &ViewNode) -> Vec<String> {
        let mut out = Vec::new();
        if node.kind == NodeKind::Text
            && let Some(text) = &node.props.text
        {
            out.push(text.clone());
        }
        for child in &node.children {
            out.extend(texts(child));
        }
        out
    }

    #[test]
    fn a_legal_value_has_an_accent_edge_and_no_message() {
        let node = field_inline(
            "name",
            "Name",
            "notes.md",
            TextCursor::spanning(0..5),
            &FieldState::Valid,
        );
        let well = find(&node, "input").expect("a well");
        assert_eq!(well.kind, NodeKind::Input);
        assert_eq!(well.props.text.as_deref(), Some("notes.md"));
        assert_eq!(well.props.cursor, Some(TextCursor::spanning(0..5)));
        assert_eq!(
            well.props.tokens.get("border").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        assert_eq!(well.constraints.vertical.max, Some(INLINE_FIELD_HEIGHT));
        assert!(find(&node, "message").is_none());
    }

    /// Error and warning differ in the word and the glyph, never the hue
    /// alone, and the well keeps its key whatever the state, so a value
    /// that turns legal does not re-key the node under the caret.
    #[test]
    fn a_broken_rule_shows_its_word_and_glyph_under_the_well() {
        for (state, word, edge) in [
            (
                FieldState::Invalid("`a.rs` already exists".into()),
                "Invalid: `a.rs` already exists",
                SUPPORT_ERROR,
            ),
            (
                FieldState::Warning("leading space".into()),
                "Warning: leading space",
                SUPPORT_WARNING,
            ),
        ] {
            let node = field_inline("name", "Name", "x", TextCursor::at(1), &state);
            let well = find(&node, "input").expect("the well keeps its key");
            assert_eq!(
                well.props.tokens.get("border").map(|t| t.as_str()),
                Some(edge)
            );
            let message = find(&node, "message").expect("a message");
            assert_eq!(message.kind, NodeKind::Surface);
            assert_eq!(message.semantics.role, Some(Role::Status));
            assert!(
                matches!(
                    message.props.anchor,
                    Some(Anchor::Sibling { ref key, edge: Edge::Bottom, .. }) if key.as_str() == "input"
                ),
                "anchored under the well"
            );
            assert_eq!(message.props.input_policy, Some(InputPolicy::Passthrough));
            assert!(find(message, "mark").is_some(), "the state's glyph");
            assert!(
                texts(message).iter().any(|t| t == word),
                "{:?}",
                texts(message)
            );
        }
    }

    /// The message of a field in a virtualized row hangs under the well.
    /// Rows are made by the row source during placement, where the anchor
    /// harvest cannot see them, so the surface once centred in the window.
    #[test]
    fn a_message_in_a_collection_row_hangs_under_its_well() {
        use crate::component::{TreeItem, tree_rows_compact};
        use crate::frame::{TransitionActivity, Viewport, petrify};
        use crate::geom::Size;
        use crate::testing::{Harness, MonoContent, validated};
        use crate::token::ThemeMode;
        use std::sync::Arc;

        struct OneEditingRow;
        impl crate::layout::RowSource for OneEditingRow {
            fn rows(&mut self, _source: &str, range: std::ops::Range<usize>) -> Vec<Arc<ViewNode>> {
                range
                    .filter(|i| *i < 3)
                    .map(|i| {
                        let item = TreeItem::new(format!("r{i}"), format!("row {i}")).compact();
                        let item = if i == 1 {
                            item.editing(field_inline(
                                "name",
                                "Name",
                                "a/b",
                                TextCursor::at(3),
                                &FieldState::Invalid("A name cannot contain /".into()),
                            ))
                        } else {
                            item
                        };
                        Arc::new(item.build(Vec::new()))
                    })
                    .collect()
            }
        }
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(tree_rows_compact("files", "Files", "rows", 3));
        let mut harness = Harness::with(MonoContent::default(), OneEditingRow);
        let frame = petrify(
            1,
            validated(&tree),
            &mut harness.ctx(),
            Viewport::new(Size::new(800.0, 600.0), ThemeMode::Dark),
            TransitionActivity::default(),
        );
        let rect = |tail: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(tail))
                .unwrap_or_else(|| panic!("no {tail}"))
                .rect
        };
        let well = rect("/r1/row/name/body/input");
        let message = rect("/r1/row/name/body/message");
        assert!(
            (message.y - (well.y + well.h)).abs() < 0.5 && (message.x - well.x).abs() < 0.5,
            "the message hangs under the well: {message:?} under {well:?}"
        );
    }
}
