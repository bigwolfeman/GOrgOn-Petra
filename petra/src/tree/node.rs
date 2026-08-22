//! The view node: plain nested data, no closures, no state, no toolkit types.

use serde::{Deserialize, Serialize};

use crate::tree::key::Key;
use crate::tree::props::Props;

/// What a node is.
///
/// Twelve built-ins. `Custom` defers measurement and painting to a registered
/// implementation named by `props.custom_kind`; an unregistered name is a
/// tree-acceptance error, never a render-time surprise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// One-axis distribution container.
    Stack,
    /// Two-axis track container.
    Grid,
    /// Children share the container's rect; z-order decides paint order.
    Overlay,
    /// Clips and offsets its content along one axis.
    Scroll,
    /// A virtualized row range read from a store-side source.
    Collection,
    /// An overlay surface (popup, toast, modal, frame-wide seat).
    Surface,
    /// A text run.
    Text,
    /// A bitmap or vector image.
    Image,
    /// An editable text field.
    Input,
    /// Empty, flexible space.
    Spacer,
    /// A one-logical-unit rule.
    Separator,
    /// Host-registered measurement and painting.
    Custom,
}

impl NodeKind {
    /// Whether this kind lays out children.
    #[must_use]
    pub fn is_container(self) -> bool {
        matches!(
            self,
            Self::Stack
                | Self::Grid
                | Self::Overlay
                | Self::Scroll
                | Self::Collection
                | Self::Surface
        )
    }

    /// The kind's wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stack => "stack",
            Self::Grid => "grid",
            Self::Overlay => "overlay",
            Self::Scroll => "scroll",
            Self::Collection => "collection",
            Self::Surface => "surface",
            Self::Text => "text",
            Self::Image => "image",
            Self::Input => "input",
            Self::Spacer => "spacer",
            Self::Separator => "separator",
            Self::Custom => "custom",
        }
    }
}

/// Per-axis clamp and flexibility priority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AxisConstraint {
    /// Lower bound on the response, logical units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    /// Upper bound on the response, logical units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
    /// Distribution priority. Higher groups negotiate first and give last.
    #[serde(skip_serializing_if = "is_zero")]
    pub priority: i32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &i32) -> bool {
    *v == 0
}

impl AxisConstraint {
    /// Clamp `value` into this constraint. `min` wins over `max` when the two
    /// contradict, so a node never reports less than its declared minimum.
    #[must_use]
    pub fn clamp(self, value: f32) -> f32 {
        let mut out = value;
        if let Some(max) = self.max {
            out = out.min(max);
        }
        if let Some(min) = self.min {
            out = out.max(min);
        }
        out
    }
}

/// Per-axis constraints on a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Constraints {
    /// Horizontal clamp and priority.
    #[serde(skip_serializing_if = "is_default_axis")]
    pub horizontal: AxisConstraint,
    /// Vertical clamp and priority.
    #[serde(skip_serializing_if = "is_default_axis")]
    pub vertical: AxisConstraint,
}

fn is_default_axis(v: &AxisConstraint) -> bool {
    *v == AxisConstraint::default()
}

impl Constraints {
    /// The constraint on `axis`.
    #[must_use]
    pub fn axis(self, axis: crate::geom::Axis) -> AxisConstraint {
        match axis {
            crate::geom::Axis::Horizontal => self.horizontal,
            crate::geom::Axis::Vertical => self.vertical,
        }
    }

    /// Clamp a size on both axes.
    #[must_use]
    pub fn clamp_size(self, size: crate::geom::Size) -> crate::geom::Size {
        crate::geom::Size::new(self.horizontal.clamp(size.w), self.vertical.clamp(size.h))
    }
}

/// An interaction intent a node emits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Interaction {
    /// Primary activation.
    Click,
    /// Press-move-release.
    Drag,
    /// Pointer entry and exit.
    Hover,
    /// Can take keyboard focus.
    Focus,
    /// Accepts text input while focused.
    TextEdit,
    /// Accepts scroll deltas.
    Scroll,
    /// Accepts key events while focused.
    Key,
}

/// What a node is, for the semantic tree.
///
/// The wire form is the role name; a host role prints as `custom:<name>`
/// (`contracts/semantic-tree.md`). Additive-only after first release.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A region of the screen.
    Pane,
    /// An ordered collection.
    List,
    /// One entry in a list.
    ListItem,
    /// An activatable control.
    Button,
    /// An editable text field.
    TextInput,
    /// Static text.
    Label,
    /// A state readout. Always carries a label: the shape-plus-text rule.
    Status,
    /// One tab in a tab list.
    Tab,
    /// A row of tabs.
    TabList,
    /// A scrollable region.
    Scroll,
    /// A layer above the main content.
    Overlay,
    /// A transient message.
    Toast,
    /// A focus-trapping surface.
    Dialog,
    /// A visual rule.
    Separator,
    /// A picture.
    Image,
    /// A determinate or indeterminate progress readout.
    Progress,
    /// A hierarchy.
    Tree,
    /// One entry in a tree.
    TreeItem,
    /// A grid of rows and cells.
    Table,
    /// One row of a table.
    Row,
    /// One cell of a row.
    Cell,
    /// A host-defined role, exposed under its own name.
    Custom(String),
}

/// Every built-in role name, in declaration order. Gates and documentation
/// read this; [`Role::parse`] and [`Role::as_wire`] are the two halves that
/// must agree with it, and `role_names_round_trip` is what makes them.
pub const ROLE_NAMES: &[&str] = &[
    "pane",
    "list",
    "listitem",
    "button",
    "textinput",
    "label",
    "status",
    "tab",
    "tablist",
    "scroll",
    "overlay",
    "toast",
    "dialog",
    "separator",
    "image",
    "progress",
    "tree",
    "treeitem",
    "table",
    "row",
    "cell",
];

impl Role {
    /// The role's wire name. `Custom("gutter")` prints as `custom:gutter`.
    #[must_use]
    pub fn as_wire(&self) -> String {
        match self {
            Self::Custom(name) => format!("custom:{name}"),
            other => (*other.builtin_name().expect("non-custom role has a name")).to_owned(),
        }
    }

    /// The static wire name of a built-in role, or `None` for [`Role::Custom`].
    #[must_use]
    pub fn builtin_name(&self) -> Option<&'static str> {
        let name = match self {
            Self::Pane => "pane",
            Self::List => "list",
            Self::ListItem => "listitem",
            Self::Button => "button",
            Self::TextInput => "textinput",
            Self::Label => "label",
            Self::Status => "status",
            Self::Tab => "tab",
            Self::TabList => "tablist",
            Self::Scroll => "scroll",
            Self::Overlay => "overlay",
            Self::Toast => "toast",
            Self::Dialog => "dialog",
            Self::Separator => "separator",
            Self::Image => "image",
            Self::Progress => "progress",
            Self::Tree => "tree",
            Self::TreeItem => "treeitem",
            Self::Table => "table",
            Self::Row => "row",
            Self::Cell => "cell",
            Self::Custom(_) => return None,
        };
        Some(name)
    }

    /// Parse a wire name.
    ///
    /// # Errors
    /// Returns the unrecognised name. `custom:` with an empty suffix is
    /// rejected: an unnamed host role would collide with every other one.
    pub fn parse(wire: &str) -> Result<Self, String> {
        if let Some(name) = wire.strip_prefix("custom:") {
            return if name.is_empty() {
                Err(wire.to_owned())
            } else {
                Ok(Self::Custom(name.to_owned()))
            };
        }
        match wire {
            "pane" => Ok(Self::Pane),
            "list" => Ok(Self::List),
            "listitem" => Ok(Self::ListItem),
            "button" => Ok(Self::Button),
            "textinput" => Ok(Self::TextInput),
            "label" => Ok(Self::Label),
            "status" => Ok(Self::Status),
            "tab" => Ok(Self::Tab),
            "tablist" => Ok(Self::TabList),
            "scroll" => Ok(Self::Scroll),
            "overlay" => Ok(Self::Overlay),
            "toast" => Ok(Self::Toast),
            "dialog" => Ok(Self::Dialog),
            "separator" => Ok(Self::Separator),
            "image" => Ok(Self::Image),
            "progress" => Ok(Self::Progress),
            "tree" => Ok(Self::Tree),
            "treeitem" => Ok(Self::TreeItem),
            "table" => Ok(Self::Table),
            "row" => Ok(Self::Row),
            "cell" => Ok(Self::Cell),
            other => Err(other.to_owned()),
        }
    }
}

impl Serialize for Role {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.as_wire())
    }
}

impl<'de> Deserialize<'de> for Role {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let wire = String::deserialize(de)?;
        Self::parse(&wire).map_err(|bad| {
            serde::de::Error::custom(format!(
                "unknown semantic role {bad:?}; expected one of {} or custom:<name>",
                ROLE_NAMES.join(", ")
            ))
        })
    }
}

/// Semantic declarations carried by a node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Semantics {
    /// What this node is. Required on interactive nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// Human-readable name. Required on interactive nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Current value for inputs and status readouts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Declared disabled state.
    #[serde(skip_serializing_if = "is_false")]
    pub disabled: bool,
    /// Declared selected state.
    #[serde(skip_serializing_if = "is_false")]
    pub selected: bool,
    /// Declared expanded state, for expandables only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// Declared staleness: the projection behind this node is past its
    /// freshness bound.
    #[serde(skip_serializing_if = "is_false")]
    pub stale: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(v: &bool) -> bool {
    !*v
}

/// A reference to a transition definition by name.
///
/// The tree holds a name, not a definition: `contracts/view-tree.md` keeps the
/// tree plain data, and the registry that resolves the name lives in
/// [`crate::anim`]. An unresolvable name is a tree-acceptance error.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransitionRef(pub String);

impl TransitionRef {
    /// A reference to the named definition.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The definition name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// One element of a surface description.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNode {
    /// What this node is.
    pub kind: NodeKind,
    /// Identity within the parent. Duplicate sibling keys refuse the tree.
    pub key: Key,
    /// Kind-specific parameters.
    #[serde(default, skip_serializing_if = "is_default_props")]
    pub props: Props,
    /// Clamps and distribution priority.
    #[serde(default, skip_serializing_if = "is_default_constraints")]
    pub constraints: Constraints,
    /// Declared animation for this node's animatable changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionRef>,
    /// Interaction intents this node emits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interactions: Vec<Interaction>,
    /// Semantic declarations.
    #[serde(default, skip_serializing_if = "is_default_semantics")]
    pub semantics: Semantics,
    /// Marks a deliberately endless animation, excluded from settle.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ambient: bool,
    /// Ordered children. Container kinds only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ViewNode>,
}

fn is_default_props(v: &Props) -> bool {
    *v == Props::default()
}

fn is_default_constraints(v: &Constraints) -> bool {
    *v == Constraints::default()
}

fn is_default_semantics(v: &Semantics) -> bool {
    *v == Semantics::default()
}

impl ViewNode {
    /// A node of `kind` with `key` and nothing else declared.
    pub fn new(kind: NodeKind, key: impl Into<Key>) -> Self {
        Self {
            kind,
            key: key.into(),
            props: Props::default(),
            constraints: Constraints::default(),
            transition: None,
            interactions: Vec::new(),
            semantics: Semantics::default(),
            ambient: false,
            children: Vec::new(),
        }
    }

    /// Replace the props.
    #[must_use]
    pub fn with_props(mut self, props: Props) -> Self {
        self.props = props;
        self
    }

    /// Replace the constraints.
    #[must_use]
    pub fn with_constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Replace the children.
    #[must_use]
    pub fn with_children(mut self, children: Vec<ViewNode>) -> Self {
        self.children = children;
        self
    }

    /// Append one child.
    #[must_use]
    pub fn child(mut self, child: ViewNode) -> Self {
        self.children.push(child);
        self
    }

    /// Declare interactions and the semantics they require.
    #[must_use]
    pub fn interactive(
        mut self,
        role: Role,
        label: impl Into<String>,
        intents: &[Interaction],
    ) -> Self {
        self.semantics.role = Some(role);
        self.semantics.label = Some(label.into());
        self.interactions = intents.to_vec();
        self
    }

    /// Replace the semantics.
    #[must_use]
    pub fn with_semantics(mut self, semantics: Semantics) -> Self {
        self.semantics = semantics;
        self
    }

    /// Name a transition definition.
    #[must_use]
    pub fn with_transition(mut self, name: impl Into<String>) -> Self {
        self.transition = Some(TransitionRef::new(name));
        self
    }

    /// Mark this node as hosting a deliberately endless animation.
    #[must_use]
    pub fn with_ambient(mut self, ambient: bool) -> Self {
        self.ambient = ambient;
        self
    }

    /// Whether this node declares any interaction.
    #[must_use]
    pub fn is_interactive(&self) -> bool {
        !self.interactions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{Constraints, Interaction, NodeKind, Role, ViewNode};
    use crate::tree::props::Props;

    /// The wire form is the shape a Lua table produces: declared keys only.
    #[test]
    fn a_bare_node_serializes_to_two_keys() {
        let node = ViewNode::new(NodeKind::Stack, "root");
        let json = serde_json::to_string(&node).unwrap();
        assert_eq!(json, r#"{"kind":"stack","key":"root"}"#);
    }

    #[test]
    fn a_populated_tree_round_trips() {
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                spacing: Some(8.0),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Text, "title").with_props(Props {
                text: Some("Fibers".into()),
                ..Props::default()
            }))
            .child(ViewNode::new(NodeKind::Input, "filter").interactive(
                Role::TextInput,
                "Filter fibers",
                &[Interaction::Focus, Interaction::TextEdit],
            ));
        let json = serde_json::to_string(&tree).unwrap();
        let back: ViewNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tree);
    }

    #[test]
    fn unknown_node_fields_are_refused() {
        let err = serde_json::from_str::<ViewNode>(r#"{"kind":"stack","key":"a","kids":[]}"#)
            .unwrap_err();
        assert!(err.to_string().contains("kids"), "{err}");
    }

    #[test]
    fn custom_roles_carry_their_name() {
        assert_eq!(Role::Button.as_wire(), "button");
        assert_eq!(Role::Custom("gutter".into()).as_wire(), "custom:gutter");
        assert_eq!(
            Role::parse("custom:gutter"),
            Ok(Role::Custom("gutter".into()))
        );
        assert!(Role::parse("custom:").is_err());
        assert!(Role::parse("widget").is_err());
    }

    /// `as_wire` and `parse` are two hand-written halves of one table. This is
    /// what stops them drifting: every name in `ROLE_NAMES` must parse, and
    /// every parse must print back to the same name.
    #[test]
    fn role_names_round_trip() {
        for name in super::ROLE_NAMES {
            let role = Role::parse(name).unwrap_or_else(|_| panic!("{name} must parse"));
            assert_eq!(&role.as_wire(), name);
            assert_eq!(role.builtin_name(), Some(*name));
        }
        assert_eq!(super::ROLE_NAMES.len(), 21);
        assert!(Role::Custom("x".into()).builtin_name().is_none());
    }

    #[test]
    fn min_beats_max_when_they_contradict() {
        let c = Constraints {
            horizontal: super::AxisConstraint {
                min: Some(50.0),
                max: Some(10.0),
                priority: 0,
            },
            vertical: super::AxisConstraint::default(),
        };
        assert_eq!(c.horizontal.clamp(30.0), 50.0);
    }

    #[test]
    fn container_kinds_are_the_six_that_place_children() {
        let containers: Vec<&str> = [
            NodeKind::Stack,
            NodeKind::Grid,
            NodeKind::Overlay,
            NodeKind::Scroll,
            NodeKind::Collection,
            NodeKind::Surface,
            NodeKind::Text,
            NodeKind::Image,
            NodeKind::Input,
            NodeKind::Spacer,
            NodeKind::Separator,
            NodeKind::Custom,
        ]
        .into_iter()
        .filter(|k| k.is_container())
        .map(NodeKind::as_str)
        .collect();
        assert_eq!(
            containers,
            [
                "stack",
                "grid",
                "overlay",
                "scroll",
                "collection",
                "surface"
            ]
        );
    }
}
