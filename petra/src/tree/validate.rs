//! Tree acceptance.
//!
//! Every violation here is refused before any layout runs
//! (`contracts/view-tree.md`). The point is that a malformed tree fails with a
//! sentence naming the node, not with a panel that quietly lays out wrong or a
//! render-time surprise three frames later.

use std::collections::{BTreeSet, HashSet};
use std::fmt;

use crate::tree::key::{Key, KeyPath};
use crate::tree::node::{NodeKind, Role, ViewNode};
use crate::tree::props::TrackSize;

/// Names the host has registered: custom node kinds and transition
/// definitions. Both are populated by the host before the first tree is
/// accepted; an unregistered reference is a violation, never a fallback.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    custom_kinds: BTreeSet<String>,
    transitions: BTreeSet<String>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a custom node kind name.
    pub fn register_custom_kind(&mut self, name: impl Into<String>) {
        self.custom_kinds.insert(name.into());
    }

    /// Register a transition definition name.
    pub fn register_transition(&mut self, name: impl Into<String>) {
        self.transitions.insert(name.into());
    }

    /// Whether a custom kind name is registered.
    #[must_use]
    pub fn has_custom_kind(&self, name: &str) -> bool {
        self.custom_kinds.contains(name)
    }

    /// Whether a transition name is registered.
    #[must_use]
    pub fn has_transition(&self, name: &str) -> bool {
        self.transitions.contains(name)
    }

    /// Registered custom kind names, sorted.
    #[must_use]
    pub fn custom_kinds(&self) -> Vec<&str> {
        self.custom_kinds.iter().map(String::as_str).collect()
    }

    /// Registered transition names, sorted.
    #[must_use]
    pub fn transitions(&self) -> Vec<&str> {
        self.transitions.iter().map(String::as_str).collect()
    }
}

/// What is wrong with one node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Violation {
    /// Two children of one parent share a key.
    DuplicateSiblingKey {
        /// The repeated key.
        key: Key,
    },
    /// A node declares interactions but no semantic role, no label, or neither.
    InteractiveWithoutSemantics {
        /// Whether `semantics.role` is present.
        has_role: bool,
        /// Whether `semantics.label` is present.
        has_label: bool,
    },
    /// A node carries a status-bearing role with no text channel.
    ///
    /// Separate from [`Violation::InteractiveWithoutSemantics`] because it
    /// catches the *non*-interactive case that one deliberately skips: a
    /// status readout is usually not clickable, so nothing else in this
    /// module ever looked at it, and a red dot with no label validated clean.
    StatusRoleWithoutLabel {
        /// The role that requires a label. Named in the message so the author
        /// knows which of the status-bearing roles tripped it.
        role: String,
    },
    /// A `custom` node names no painter, or names one that is not registered.
    UnregisteredCustomKind {
        /// The declared name, or `None` when `props.custom_kind` is absent.
        name: Option<String>,
        /// What the registry does hold.
        registered: Vec<String>,
    },
    /// A node names a transition definition the registry does not hold.
    UnregisteredTransition {
        /// The declared name.
        name: String,
        /// What the registry does hold.
        registered: Vec<String>,
    },
    /// A leaf kind carries children.
    LeafWithChildren {
        /// The leaf kind.
        kind: NodeKind,
        /// How many children it declared.
        count: usize,
    },
    /// A container kind is missing a parameter it cannot lay out without.
    MissingRequiredProp {
        /// The kind that needs it.
        kind: NodeKind,
        /// The `props` field name.
        prop: &'static str,
    },
    /// A declared value is outside its legal range.
    ValueOutOfRange {
        /// The `props` field name.
        prop: &'static str,
        /// What was declared, as text.
        value: String,
        /// The legal range, as text.
        expected: &'static str,
    },
    /// A style literal appears where a token name is required (FR-013).
    LiteralStyleValue {
        /// The token slot.
        slot: String,
        /// The literal that was found.
        value: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSiblingKey { key } => {
                write!(
                    f,
                    "two children share the key {key:?}; keys identify a node inside its parent and must be unique there"
                )
            }
            Self::InteractiveWithoutSemantics {
                has_role,
                has_label,
            } => {
                let missing = match (has_role, has_label) {
                    (false, false) => "semantics.role and semantics.label",
                    (false, true) => "semantics.role",
                    _ => "semantics.label",
                };
                write!(
                    f,
                    "declares interactions but no {missing}; an interactive node that cannot be named cannot be driven or heard"
                )
            }
            Self::StatusRoleWithoutLabel { role } => {
                write!(
                    f,
                    "role `{role}` carries no semantics.label; a status conveys its state through colour, shape, and text together (FR-015), and without a label colour is the only channel left"
                )
            }
            Self::UnregisteredCustomKind { name, registered } => match name {
                None => write!(
                    f,
                    "kind `custom` without props.custom_kind; registered: [{}]",
                    registered.join(", ")
                ),
                Some(name) => write!(
                    f,
                    "custom kind {name:?} is not registered; registered: [{}]",
                    registered.join(", ")
                ),
            },
            Self::UnregisteredTransition { name, registered } => {
                write!(
                    f,
                    "transition {name:?} is not registered; registered: [{}]",
                    registered.join(", ")
                )
            }
            Self::LeafWithChildren { kind, count } => {
                write!(
                    f,
                    "kind `{}` is a leaf but declares {count} child(ren)",
                    kind.as_str()
                )
            }
            Self::MissingRequiredProp { kind, prop } => {
                write!(f, "kind `{}` requires props.{prop}", kind.as_str())
            }
            Self::ValueOutOfRange {
                prop,
                value,
                expected,
            } => write!(f, "props.{prop} is {value}; expected {expected}"),
            Self::LiteralStyleValue { slot, value } => write!(
                f,
                "tokens.{slot} is the literal {value:?}; features reference token names, never values"
            ),
        }
    }
}

/// One violation, with the key path of the node that carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeError {
    /// Canonical id of the offending node.
    pub path: String,
    /// What is wrong.
    pub violation: Violation,
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.violation)
    }
}

/// Every violation in one tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeErrors(Vec<TreeError>);

impl TreeErrors {
    /// The violations, in tree pre-order.
    #[must_use]
    pub fn as_slice(&self) -> &[TreeError] {
        &self.0
    }

    /// How many violations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the list is empty. Never true for a returned `Err`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for TreeErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "petra: {} tree-acceptance violation(s):", self.0.len())?;
        for err in &self.0 {
            writeln!(f, "  {err}")?;
        }
        Ok(())
    }
}

impl std::error::Error for TreeErrors {}

/// Accept a tree, or refuse it with every violation it carries.
///
/// The whole tree is refused, not the offending subtree: a panel missing one
/// label is a bug to fix, and rendering the rest would hide it.
///
/// # Errors
/// Returns every violation found, in tree pre-order.
pub fn validate(root: &ViewNode, registry: &Registry) -> Result<(), TreeErrors> {
    let mut errors = Vec::new();
    let mut path = KeyPath::root();
    walk(root, registry, &mut path, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(TreeErrors(errors))
    }
}

fn walk(node: &ViewNode, registry: &Registry, path: &mut KeyPath, errors: &mut Vec<TreeError>) {
    path.push(node.key.clone());
    check_node(node, registry, path, errors);
    let mut seen: HashSet<&Key> = HashSet::with_capacity(node.children.len());
    for child in &node.children {
        if !seen.insert(&child.key) {
            errors.push(TreeError {
                path: path.child(&child.key).id(),
                violation: Violation::DuplicateSiblingKey {
                    key: child.key.clone(),
                },
            });
        }
    }
    for child in &node.children {
        walk(child, registry, path, errors);
    }
    path.pop();
}

fn check_node(node: &ViewNode, registry: &Registry, path: &KeyPath, errors: &mut Vec<TreeError>) {
    let id = path.id();
    let mut push = |violation| {
        errors.push(TreeError {
            path: id.clone(),
            violation,
        });
    };

    let has_label = node
        .semantics
        .label
        .as_ref()
        .is_some_and(|l| !l.trim().is_empty());

    if node.is_interactive() {
        let has_role = node.semantics.role.is_some();
        if !has_role || !has_label {
            push(Violation::InteractiveWithoutSemantics {
                has_role,
                has_label,
            });
        }
    }

    // A status-bearing role must carry its text channel, whether or not the
    // node is interactive. The interactive check above deliberately does not
    // cover this: a health dot or a progress bar is usually not clickable, so
    // before this check existed a `Text` node with `role: status` and a
    // `status.down` colour token validated clean — colour as the only channel,
    // which is the one thing FR-015 exists to prevent.
    if !has_label
        && let Some(role) = &node.semantics.role
        && matches!(role, Role::Status | Role::Progress)
    {
        push(Violation::StatusRoleWithoutLabel {
            role: role.as_wire(),
        });
    }

    if !node.kind.is_container() && !node.children.is_empty() {
        push(Violation::LeafWithChildren {
            kind: node.kind,
            count: node.children.len(),
        });
    }

    match node.kind {
        NodeKind::Custom => match node.props.custom_kind.as_deref() {
            Some(name) if registry.has_custom_kind(name) => {}
            other => push(Violation::UnregisteredCustomKind {
                name: other.map(str::to_owned),
                registered: registry
                    .custom_kinds()
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            }),
        },
        NodeKind::Grid => {
            if node.props.columns.is_empty() {
                push(Violation::MissingRequiredProp {
                    kind: NodeKind::Grid,
                    prop: "columns",
                });
            }
            for track in node.props.columns.iter().chain(node.props.rows.iter()) {
                if let TrackSize::Weight { weight } = track
                    && !(weight.is_finite() && *weight > 0.0)
                {
                    push(Violation::ValueOutOfRange {
                        prop: "columns[].weight",
                        value: format!("{weight}"),
                        expected: "a finite weight greater than zero",
                    });
                }
            }
        }
        NodeKind::Collection => {
            if node.props.total_count.is_none() {
                push(Violation::MissingRequiredProp {
                    kind: NodeKind::Collection,
                    prop: "total_count",
                });
            }
            if node.props.source.is_none() {
                push(Violation::MissingRequiredProp {
                    kind: NodeKind::Collection,
                    prop: "source",
                });
            }
        }
        NodeKind::Surface => {
            if node.props.layer.is_none() {
                push(Violation::MissingRequiredProp {
                    kind: NodeKind::Surface,
                    prop: "layer",
                });
            }
            if node.props.anchor.is_none() {
                push(Violation::MissingRequiredProp {
                    kind: NodeKind::Surface,
                    prop: "anchor",
                });
            }
        }
        _ => {}
    }

    if let Some(opacity) = node.props.opacity
        && !(opacity.is_finite() && (0.0..=1.0).contains(&opacity))
    {
        push(Violation::ValueOutOfRange {
            prop: "opacity",
            value: format!("{opacity}"),
            expected: "a finite value in [0, 1]",
        });
    }

    if let Some(spacing) = node.props.spacing
        && !(spacing.is_finite() && spacing >= 0.0)
    {
        push(Violation::ValueOutOfRange {
            prop: "spacing",
            value: format!("{spacing}"),
            expected: "a finite value of zero or more",
        });
    }

    if let Some(name) = node.transition.as_ref().map(|t| t.name().to_owned())
        && !registry.has_transition(&name)
    {
        push(Violation::UnregisteredTransition {
            name,
            registered: registry
                .transitions()
                .into_iter()
                .map(str::to_owned)
                .collect(),
        });
    }

    for (slot, value) in &node.props.tokens {
        if is_style_literal(value) {
            push(Violation::LiteralStyleValue {
                slot: slot.clone(),
                value: value.clone(),
            });
        }
    }
}

/// Whether a token slot holds a value instead of a name.
///
/// The two shapes a literal takes in practice are a CSS-style hex colour and a
/// bare number. A token name is `surface.raised` or `text-muted`; neither
/// parses as either.
fn is_style_literal(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.starts_with('#') {
        return true;
    }
    if trimmed.parse::<f64>().is_ok() {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    ["rgb(", "rgba(", "hsl(", "hsla("]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::{Registry, TreeError, Violation, validate};
    use crate::tree::node::{Interaction, NodeKind, Role, Semantics, ViewNode};
    use crate::tree::props::{Anchor, Edge, Layer, Props, TrackSize};

    fn stack(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
    }

    #[test]
    fn a_clean_tree_is_accepted() {
        let tree = stack("root")
            .child(ViewNode::new(NodeKind::Text, "a"))
            .child(ViewNode::new(NodeKind::Text, "b"));
        assert!(validate(&tree, &Registry::new()).is_ok());
    }

    #[test]
    fn duplicate_sibling_keys_refuse_the_tree() {
        let tree = stack("root")
            .child(ViewNode::new(NodeKind::Text, "a"))
            .child(ViewNode::new(NodeKind::Text, "a"));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err.as_slice()[0].path, "/root/a");
        assert!(matches!(
            err.as_slice()[0].violation,
            Violation::DuplicateSiblingKey { .. }
        ));
        assert!(err.to_string().contains("must be unique"), "{err}");
    }

    #[test]
    fn an_interactive_node_needs_role_and_label() {
        let mut node = ViewNode::new(NodeKind::Input, "field");
        node.interactions = vec![Interaction::Click];
        let err = validate(&stack("root").child(node), &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::InteractiveWithoutSemantics {
                has_role: false,
                has_label: false
            }
        );
    }

    /// A whitespace label is the shape that would pass a naive `is_some`
    /// check and still leave a button nameless to a screen reader.
    #[test]
    fn a_blank_label_does_not_count_as_a_label() {
        let node = ViewNode::new(NodeKind::Input, "field").interactive(
            Role::Button,
            "   ",
            &[Interaction::Click],
        );
        let err = validate(&stack("root").child(node), &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::InteractiveWithoutSemantics {
                has_role: true,
                has_label: false
            }
        );
    }

    #[test]
    fn a_custom_kind_must_be_registered() {
        let node = ViewNode::new(NodeKind::Custom, "gauge").with_props(Props {
            custom_kind: Some("gauge".into()),
            ..Props::default()
        });
        let err = validate(&stack("root").child(node.clone()), &Registry::new()).unwrap_err();
        assert!(matches!(
            err.as_slice()[0].violation,
            Violation::UnregisteredCustomKind { .. }
        ));
        let mut registry = Registry::new();
        registry.register_custom_kind("gauge");
        assert!(validate(&stack("root").child(node), &registry).is_ok());
    }

    #[test]
    fn a_transition_must_be_registered() {
        let node = ViewNode::new(NodeKind::Text, "t").with_transition("slide");
        let err = validate(&stack("root").child(node.clone()), &Registry::new()).unwrap_err();
        assert!(matches!(
            err.as_slice()[0].violation,
            Violation::UnregisteredTransition { .. }
        ));
        let mut registry = Registry::new();
        registry.register_transition("slide");
        assert!(validate(&stack("root").child(node), &registry).is_ok());
    }

    #[test]
    fn leaves_may_not_carry_children() {
        let leaf = ViewNode::new(NodeKind::Text, "t").child(ViewNode::new(NodeKind::Text, "c"));
        let err = validate(&stack("root").child(leaf), &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::LeafWithChildren {
                kind: NodeKind::Text,
                count: 1
            }
        );
    }

    #[test]
    fn containers_declare_what_they_cannot_lay_out_without() {
        let grid = ViewNode::new(NodeKind::Grid, "g");
        let err = validate(&grid, &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::MissingRequiredProp {
                kind: NodeKind::Grid,
                prop: "columns"
            }
        );

        let collection = ViewNode::new(NodeKind::Collection, "c");
        let err = validate(&collection, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 2, "{err}");

        let surface = ViewNode::new(NodeKind::Surface, "s").with_props(Props {
            layer: Some(Layer::Popup),
            ..Props::default()
        });
        let err = validate(&surface, &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::MissingRequiredProp {
                kind: NodeKind::Surface,
                prop: "anchor"
            }
        );

        let ok = ViewNode::new(NodeKind::Surface, "s").with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Node {
                id: "/root".into(),
                edge: Edge::Bottom,
            }),
            ..Props::default()
        });
        assert!(validate(&ok, &Registry::new()).is_ok());
    }

    #[test]
    fn out_of_range_values_are_named() {
        let node = ViewNode::new(NodeKind::Text, "t").with_props(Props {
            opacity: Some(1.5),
            spacing: Some(-1.0),
            ..Props::default()
        });
        let err = validate(&node, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 2, "{err}");

        let grid = ViewNode::new(NodeKind::Grid, "g").with_props(Props {
            columns: vec![TrackSize::Weight { weight: 0.0 }],
            ..Props::default()
        });
        let err = validate(&grid, &Registry::new()).unwrap_err();
        assert!(matches!(
            err.as_slice()[0].violation,
            Violation::ValueOutOfRange { .. }
        ));
    }

    #[test]
    fn literal_styles_are_refused_where_tokens_belong() {
        for literal in ["#ff0000", "12", "rgb(1,2,3)", " "] {
            let mut props = Props::default();
            props.tokens.insert("background".into(), literal.into());
            let node = ViewNode::new(NodeKind::Text, "t").with_props(props);
            let err = validate(&node, &Registry::new()).unwrap_err();
            assert!(
                matches!(
                    err.as_slice()[0].violation,
                    Violation::LiteralStyleValue { .. }
                ),
                "{literal:?} was accepted"
            );
        }
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), "surface.raised".into());
        assert!(
            validate(
                &ViewNode::new(NodeKind::Text, "t").with_props(props),
                &Registry::new()
            )
            .is_ok()
        );
    }

    /// Violations are reported for the whole tree, not just the first one, so
    /// one run of the gate names every fix.
    /// The hole FR-015 had. A status readout is not interactive, so the
    /// role-and-label check above skipped it entirely, and a node whose only
    /// state channel was a `status.down` colour token passed acceptance.
    #[test]
    fn a_status_role_without_a_label_is_refused_even_though_it_is_not_interactive() {
        let node = ViewNode::new(NodeKind::Text, "health").with_semantics(Semantics {
            role: Some(Role::Status),
            ..Semantics::default()
        });
        assert!(
            !node.is_interactive(),
            "the test is only meaningful if the interactive check does not \
             already cover this node"
        );
        let errors = validate(&node, &Registry::default()).unwrap_err();
        assert_eq!(
            errors.0,
            vec![TreeError {
                path: "/health".to_string(),
                violation: Violation::StatusRoleWithoutLabel {
                    role: "status".to_string()
                },
            }]
        );
        assert!(
            errors
                .to_string()
                .contains("colour is the only channel left"),
            "the message must say why, not just what: {errors}"
        );
    }

    /// Whitespace is not a text channel, the same way it is not a label for an
    /// interactive node.
    #[test]
    fn a_whitespace_only_status_label_does_not_count() {
        let node = ViewNode::new(NodeKind::Text, "health").with_semantics(Semantics {
            role: Some(Role::Status),
            label: Some("  \t ".to_string()),
            ..Semantics::default()
        });
        let errors = validate(&node, &Registry::default()).unwrap_err();
        assert_eq!(
            errors.0[0].violation,
            Violation::StatusRoleWithoutLabel {
                role: "status".to_string()
            }
        );
    }

    /// `progress` carries the same obligation for the same reason: a bar whose
    /// only signal is how much of it is painted a particular colour.
    #[test]
    fn progress_carries_the_same_obligation_as_status() {
        let node = ViewNode::new(NodeKind::Custom, "bar")
            .with_props(Props {
                custom_kind: Some("bar".to_string()),
                ..Props::default()
            })
            .with_semantics(Semantics {
                role: Some(Role::Progress),
                ..Semantics::default()
            });
        let mut registry = Registry::default();
        registry.register_custom_kind("bar");
        let errors = validate(&node, &registry).unwrap_err();
        assert_eq!(
            errors.0[0].violation,
            Violation::StatusRoleWithoutLabel {
                role: "progress".to_string()
            }
        );
    }

    /// The accepting case, so the check cannot be satisfied by refusing
    /// everything.
    #[test]
    fn a_labelled_status_is_accepted() {
        let node = ViewNode::new(NodeKind::Text, "health").with_semantics(Semantics {
            role: Some(Role::Status),
            label: Some("Down".to_string()),
            ..Semantics::default()
        });
        validate(&node, &Registry::default()).unwrap();
    }

    /// Roles that are not status-bearing keep working without a label — the
    /// check must not quietly become "every node needs a label".
    #[test]
    fn a_non_status_role_still_needs_no_label() {
        let node = ViewNode::new(NodeKind::Text, "heading").with_semantics(Semantics {
            role: Some(Role::Label),
            ..Semantics::default()
        });
        validate(&node, &Registry::default()).unwrap();
    }

    #[test]
    fn every_violation_is_reported_in_pre_order() {
        let mut bad_a = ViewNode::new(NodeKind::Input, "a");
        bad_a.interactions = vec![Interaction::Focus];
        let mut bad_b = ViewNode::new(NodeKind::Input, "b");
        bad_b.interactions = vec![Interaction::Focus];
        let tree = stack("root").child(bad_a).child(bad_b);
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 2);
        assert_eq!(err.as_slice()[0].path, "/root/a");
        assert_eq!(err.as_slice()[1].path, "/root/b");
    }
}
