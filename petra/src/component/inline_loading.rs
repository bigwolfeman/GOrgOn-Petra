//! `inline_loading` — Carbon Inline loading (slice-b).
//!
//! Status of an in-flight action. Carbon's active state is a 16×16 spinning
//! ring; the shipped animation registry has no spinner track, so the active
//! mark is a static outlined disc. Finished adds [`IconMark::Check`].
//!
//! Spinner size 16 is MEASURED `_loading.scss` `--small`. Container floor
//! 32 (`min-block-size: 2rem`, `_inline-loading.scss:30`). Gap spinner-to-
//! label is [`SPACING_03`] (8).

use super::icon::{IconMark, icon};
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, SHAPE_FULL, SPACING_03, TEXT_MUTED, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Key, Role, Semantics, ViewNode};

/// Carbon small loading spinner, the only size Inline loading uses.
const SPINNER: f32 = 16.0;
/// Container `min-block-size: 2rem`. MEASURED `_inline-loading.scss:30`.
const MIN_BLOCK: f32 = 32.0;

/// In-place status of an action: spinning (static disc) or finished (check).
///
/// `label` is required ([`Role::Status`] needs a label). `active` selects
/// the mark and [`Semantics.value`]: `"loading"` vs `"finished"`.
pub fn inline_loading(key: impl Into<Key>, label: impl Into<String>, active: bool) -> ViewNode {
    let label = label.into();
    let mark = if active {
        swatch(
            "mark",
            SPINNER,
            SPINNER,
            None,
            Some(BORDER_SUBTLE),
            Some(SHAPE_FULL),
        )
    } else {
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
        badge.with_constraints(Constraints {
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
        })
    };

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
        value: Some(if active {
            "loading".into()
        } else {
            "finished".into()
        }),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::{MIN_BLOCK, SPINNER, inline_loading};
    use crate::component::icon::IconMark;
    use crate::draw::Command;
    use crate::tree::{NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    #[test]
    fn active_is_status_loading_with_the_label() {
        let node = inline_loading("save", "Saving", true);
        assert_eq!(node.semantics.role, Some(Role::Status));
        assert_eq!(node.semantics.label.as_deref(), Some("Saving"));
        assert_eq!(node.semantics.value.as_deref(), Some("loading"));
        assert!(!node.is_interactive());
        assert_eq!(node.constraints.vertical.min, Some(MIN_BLOCK));
        assert_eq!(MIN_BLOCK, 32.0);
        let mark = child(&node, "mark");
        assert_eq!(mark.constraints.horizontal.min, Some(SPINNER));
        assert_eq!(mark.constraints.vertical.min, Some(SPINNER));
        assert_eq!(SPINNER, 16.0);
        assert_eq!(child(&node, "label").props.text.as_deref(), Some("Saving"));
        assert!(
            node.transition.is_none(),
            "no spinner track is registered; motion is omitted"
        );
    }

    #[test]
    fn finished_adds_a_check_mark() {
        let node = inline_loading("save", "Saved", false);
        assert_eq!(node.semantics.role, Some(Role::Status));
        assert_eq!(node.semantics.label.as_deref(), Some("Saved"));
        assert_eq!(node.semantics.value.as_deref(), Some("finished"));
        let mark = child(&node, "mark");
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
}
