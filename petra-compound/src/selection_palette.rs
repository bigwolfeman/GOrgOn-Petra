//! Selection palette as a [`Compound`]: a floating format strip at a point.
//!
//! Field on the fiber that owns the selection (`view-fiber.md` §4.5). `view`
//! is a raised overlay at [`Anchor::Point`] hosting
//! [`toggle_button_group`]. A real text selection as the anchor is spec 006;
//! the host supplies the [`Point`].
//!
//! Binding: spec 009 T028a.

use gorgon_petra::Point;
use gorgon_petra::component::{toggle_button, toggle_button_group};
use gorgon_petra::token::{CornerRole, TokenName, corner_for};
use gorgon_petra::tree::{
    Anchor, ClampRule, FocusFigure, InputPolicy, Intent as EngineIntent, Layer, NodeKind, Phase,
    Props as NodeProps, Role, Semantics, Tip, ViewNode,
};
use serde::{Deserialize, Serialize};

use crate::Compound;

/// Namespace over the selection-palette triple. Never constructed as a value.
pub struct SelectionPalette;

/// Root key of the node [`SelectionPalette::view`] returns.
const PALETTE_KEY: &str = "palette";

/// Author-supplied configuration. Marks and the point live in [`State`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Props;

/// Which marks are pressed, and where the overlay sits.
///
/// Round-trips through reload as a whole. The point is a gallery fake until
/// spec 006 hands a real selection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    /// Bold mark is on.
    pub bold: bool,
    /// Italic mark is on.
    pub italic: bool,
    /// Overlay origin, logical units, top-left.
    pub point: Point,
}

/// Closed set of things that can happen to the palette.
///
/// `Deserialize` so [`gorgon_view_fiber::ViewFiber`] can decode it off the
/// `ui:intent` bus (`ViewFiber`'s `C::Intent: DeserializeOwned` bound).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Intent {
    /// Flip [`State::bold`].
    ToggleBold,
    /// Flip [`State::italic`].
    ToggleItalic,
    /// Move the overlay. The host sends this as the selection moves.
    Move {
        /// New origin.
        point: Point,
    },
}

impl Compound for SelectionPalette {
    type Props = Props;
    type State = State;
    type Intent = Intent;
    type Request = ();

    fn init(_props: &Self::Props) -> Self::State {
        State::default()
    }

    fn update(state: &mut Self::State, intent: Self::Intent) -> Vec<Self::Request> {
        match intent {
            Intent::ToggleBold => {
                state.bold = !state.bold;
            }
            Intent::ToggleItalic => {
                state.italic = !state.italic;
            }
            Intent::Move { point } => {
                state.point = point;
            }
        }
        Vec::new()
    }

    fn intent_from_fire(
        _state: &Self::State,
        node: &str,
        intent: EngineIntent,
        phase: Phase,
        _value: Option<f64>,
        _weights: Option<&[f64]>,
    ) -> Option<Self::Intent> {
        if phase != Phase::OnRelease || intent != EngineIntent::Toggle {
            return None;
        }
        match node {
            "bold" => Some(Intent::ToggleBold),
            "italic" => Some(Intent::ToggleItalic),
            _ => None,
        }
    }

    fn view(state: &Self::State, _props: &Self::Props) -> ViewNode {
        // A mark sits flush against the palette's own edge, so the two bars
        // `toggle_button` seats with `FocusFigure::Sides` would be drawn
        // seven units outside the palette, on the page behind it. `BarInside`
        // is the figure for a flush control. Every mark takes it, not only
        // the first and last: the figure a control wears should not change
        // with how many siblings happen to sit beside it.
        let group = toggle_button_group(
            "marks",
            vec![
                flush_mark(toggle_button("bold", "Bold", state.bold)),
                flush_mark(toggle_button("italic", "Italic", state.italic)),
            ],
        );
        let mut node = ViewNode::new(NodeKind::Surface, PALETTE_KEY)
            .with_props(NodeProps {
                layer: Some(Layer::Popup),
                anchor: Some(Anchor::Point {
                    x: state.point.x,
                    y: state.point.y,
                }),
                clamp: Some(ClampRule::Flip),
                input_policy: Some(InputPolicy::DismissOutside),
                tip: Some(Tip::Flush),
                ..NodeProps::default()
            })
            .child(group);
        node.props
            .tokens
            .insert("background".into(), token("surface.raised"));
        node.props
            .tokens
            .insert("shadow".into(), token("shadow.overlay"));
        // Floating overlay card. Short edge is the toggle-button md height
        // the marks use; `corner_for` picks the ramp step, never a literal.
        node.props.tokens.insert(
            "radius".into(),
            token(corner_for(CornerRole::Floating, 40.0)),
        );
        node.semantics = Semantics {
            role: Some(Role::Overlay),
            label: Some("Format".into()),
            ..Semantics::default()
        };
        node
    }
}

fn token(name: &'static str) -> TokenName {
    TokenName::new(name).unwrap_or_else(|err| panic!("shipped token {name:?}: {err}"))
}

/// One mark of the palette, re-seated for a container with no padding.
///
/// See the call site in [`SelectionPalette::view`] for why `Sides` cannot be
/// worn here.
fn flush_mark(mut node: ViewNode) -> ViewNode {
    node.semantics.focus_figure = FocusFigure::BarInside;
    node
}

#[cfg(test)]
mod tests {
    use gorgon_petra::Point;
    use gorgon_petra::token::{CornerRole, corner_for};
    use gorgon_petra::tree::{Anchor, Layer, Role, Tip, ViewNode};

    use crate::Compound;

    use super::{Intent, PALETTE_KEY, Props, SelectionPalette, token};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    #[test]
    fn init_is_unpressed_at_the_origin() {
        let state = SelectionPalette::init(&Props);
        assert!(!state.bold);
        assert!(!state.italic);
        assert_eq!(state.point, Point::ZERO);
    }

    #[test]
    fn toggles_flip_and_move_sets_the_point() {
        let mut state = SelectionPalette::init(&Props);
        assert!(SelectionPalette::update(&mut state, Intent::ToggleBold).is_empty());
        assert!(state.bold);
        SelectionPalette::update(&mut state, Intent::ToggleBold);
        assert!(!state.bold);
        SelectionPalette::update(&mut state, Intent::ToggleItalic);
        assert!(state.italic);
        let point = Point::new(24.0, 80.0);
        SelectionPalette::update(&mut state, Intent::Move { point });
        assert_eq!(state.point, point);
    }

    #[test]
    fn view_is_a_flush_overlay_at_the_point_hosting_the_group() {
        let mut state = SelectionPalette::init(&Props);
        state.bold = true;
        state.point = Point::new(12.0, 34.0);
        let node = SelectionPalette::view(&state, &Props);
        assert_eq!(node.key.as_str(), PALETTE_KEY);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.props.layer, Some(Layer::Popup));
        assert_eq!(node.props.tip, Some(Tip::Flush));
        assert_eq!(node.props.anchor, Some(Anchor::Point { x: 12.0, y: 34.0 }));
        assert!(
            node.props.tokens.contains_key("radius"),
            "the palette is a floating overlay card"
        );
        assert_eq!(
            node.props.tokens.get("radius"),
            Some(&token(corner_for(CornerRole::Floating, 40.0))),
        );
        assert!(
            !node.props.tokens.contains_key("border"),
            "the palette is a raised overlay, not a box"
        );
        named(&node, "marks");
        assert!(named(&node, "bold").semantics.selected);
        assert!(!named(&node, "italic").semantics.selected);
    }
}
