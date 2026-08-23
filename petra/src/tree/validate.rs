//! Tree acceptance.
//!
//! Every violation here is refused before any layout runs
//! (`contracts/view-tree.md`). The point is that a malformed tree fails with a
//! sentence naming the node, not with a panel that quietly lays out wrong or a
//! render-time surprise three frames later.

use std::collections::{BTreeSet, HashSet};
use std::fmt;

use crate::geom::Axis;
use crate::tree::key::{Key, KeyPath};
use crate::tree::node::{NodeKind, Role, ViewNode};
use crate::tree::props::{ScrollProps, TrackSize, max_row_tracks};

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
    /// A `collection` declares a scrolling parameter its `scroll` ancestor
    /// owns.
    ///
    /// The axis and the overscan belong to the container that has a viewport
    /// and an offset, and the layout walk reads both off the nearest
    /// enclosing `scroll` (`crate::layout::ScrollFrame`). A declaration here
    /// would be ignored, and an ignored declaration is worse than a refused
    /// one: the author sees a list that scrolls the wrong way, or
    /// materializes the wrong window, with a prop in the tree that says
    /// otherwise. A `collection` outside every `scroll` keeps its own values
    /// and is never refused.
    ScrollParamOwnedByAncestor {
        /// The `props` field name.
        prop: &'static str,
        /// What the `collection` declared, as text.
        declared: String,
        /// Canonical id of the `scroll` that owns the parameter.
        scroll: String,
        /// What that `scroll` resolves the parameter to, as text.
        owner: String,
    },
    /// A leaf kind declares `padding`.
    ///
    /// A leaf has no children to offer an inset rect to, and `Placement`
    /// carries exactly one `Rect` per node — there is no second "content
    /// rect" a leaf's own drawn content could sit inside while its outer
    /// rect stays put for a bound `background` token to paint. Refused for
    /// the same reason `ScrollParamOwnedByAncestor` is refused rather than
    /// silently ignored: a declaration here would be dead, and a dead
    /// declaration is worse than a refused one.
    PaddingOnLeafKind {
        /// The leaf kind that declared it.
        kind: NodeKind,
    },
    /// A grid child declares a span of no tracks, or of more tracks than the
    /// grid has on that axis.
    ///
    /// Refused, not clamped: `contracts/view-tree.md` says so in as many
    /// words, and the reason is that a shortened span is a silent relocation.
    /// An author who writes "this event covers four rows" and gets two has a
    /// panel that looks plausible and says the wrong thing, which is the
    /// failure `contracts/view-tree.md` spends FR-005's whole concession order
    /// avoiding.
    ///
    /// The bound is the axis's own track count, because that is what "runs
    /// past the last track" means for a span whose leading cell the grid
    /// chooses. A five-column span in a four-column grid fits nowhere on any
    /// row; a four-column one fits, and the grid's seating is what decides
    /// which row it lands on.
    GridSpanOutOfRange {
        /// `span.columns` or `span.rows`.
        prop: &'static str,
        /// The declared count.
        count: usize,
        /// How many tracks the grid has on that axis. Rows include the
        /// implicit ones the grid grows so every child has a row to sit in.
        tracks: usize,
    },
    /// A node declares `props.span` but its parent is not a `grid`.
    ///
    /// Refused for the reason [`Violation::PaddingOnLeafKind`] is refused: a
    /// span is read by the `grid` that seats the child, so anywhere else the
    /// declaration is dead, and a dead declaration is worse than a refused
    /// one.
    SpanOutsideGrid {
        /// The parent's kind, or `None` when the node is the root.
        parent: Option<NodeKind>,
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
            Self::ScrollParamOwnedByAncestor {
                prop,
                declared,
                scroll,
                owner,
            } => write!(
                f,
                "props.{prop} is {declared} on a `collection`, but its scroll ancestor `{scroll}` owns that parameter and resolves it to {owner}; declare it on the scroll (a collection outside every scroll keeps its own)"
            ),
            Self::PaddingOnLeafKind { kind } => write!(
                f,
                "kind `{}` is a leaf and declares props.padding; a leaf has no children to inset, so the declaration would be ignored — declare padding on a container ancestor instead",
                kind.as_str()
            ),
            Self::GridSpanOutOfRange {
                prop,
                count,
                tracks,
            } => {
                if *count == 0 {
                    write!(
                        f,
                        "props.{prop} is 0; a span counts the tracks a child covers, and a child always covers at least the one it sits in"
                    )
                } else {
                    write!(
                        f,
                        "props.{prop} is {count}, which runs past the last of this grid's {tracks} track(s); a span that fits nowhere is refused, never shortened"
                    )
                }
            }
            Self::SpanOutsideGrid { parent } => match parent {
                Some(kind) => write!(
                    f,
                    "declares props.span but its parent is a `{}`, not a `grid`; only a grid seats children in tracks, so the declaration would be ignored",
                    kind.as_str()
                ),
                None => write!(
                    f,
                    "declares props.span but is the root, so it has no grid to be seated in; the declaration would be ignored"
                ),
            },
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

/// A tree that has passed [`validate`].
///
/// The only way to get one of these is `validate` returning `Ok`: the field
/// is private to this module, there is no public constructor, and there is
/// no `From` impl. [`crate::frame::petrify`] and
/// [`crate::frame::petrify_with_memo`] accept only this type, so a host
/// cannot reach layout with a tree that skipped acceptance by forgetting a
/// call — the compiler refuses the program, not a debug assertion that
/// might not be compiled in. `gorgon-petra-egui` is a separate crate, so
/// module privacy alone would not be enough; the field is private to this
/// crate and stays that way.
///
/// Borrows rather than owns: every caller already has a `&ViewNode` it keeps
/// alive for the pass (the host holds its tree locally for the duration of
/// one frame), so borrowing costs nothing and avoids a clone of a tree that
/// can be large. [`std::ops::Deref`] to [`ViewNode`] means existing code that
/// takes `&ViewNode` — `surface_scopes`, the reuse verifier — keeps working
/// unchanged against `&validated_tree`.
#[derive(Clone, Copy, Debug)]
pub struct ValidatedTree<'a>(&'a ViewNode);

impl<'a> std::ops::Deref for ValidatedTree<'a> {
    type Target = ViewNode;

    fn deref(&self) -> &ViewNode {
        self.0
    }
}

/// Accept a tree, or refuse it with every violation it carries.
///
/// The whole tree is refused, not the offending subtree: a panel missing one
/// label is a bug to fix, and rendering the rest would hide it.
///
/// On success, mints a [`ValidatedTree`]: the token [`crate::frame::petrify`]
/// requires, and the only way to produce one.
///
/// # Errors
/// Returns every violation found, in tree pre-order.
pub fn validate<'a>(
    root: &'a ViewNode,
    registry: &Registry,
) -> Result<ValidatedTree<'a>, TreeErrors> {
    let mut errors = Vec::new();
    let mut path = KeyPath::root();
    walk(root, registry, &mut path, None, None, &mut errors);
    if errors.is_empty() {
        Ok(ValidatedTree(root))
    } else {
        Err(TreeErrors(errors))
    }
}

/// The nearest enclosing `scroll`: its canonical id and its resolved
/// parameters. `None` outside every scroll, and inside a `surface`, which is
/// anchored in viewport coordinates and so is not scrolled by anything it was
/// declared inside (`crate::layout::LayoutCtx::outside_scroll`).
type ScrollAncestor<'a> = Option<(&'a str, ScrollProps)>;

fn walk(
    node: &ViewNode,
    registry: &Registry,
    path: &mut KeyPath,
    parent: Option<NodeKind>,
    scroll: ScrollAncestor<'_>,
    errors: &mut Vec<TreeError>,
) {
    path.push(node.key.clone());
    check_node(node, registry, path, parent, scroll, errors);
    if node.kind == NodeKind::Grid {
        // A parent-level check: a span is declared on the child but only means
        // anything against this grid's tracks and this grid's flow order, so
        // the grid is the only place that can judge one.
        check_grid_spans(node, path, errors);
    }
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
    // The same rule the layout walk follows: a `scroll` becomes the context
    // for everything under it, a `surface` clears it, everything else passes
    // its own context through.
    let entered = (node.kind == NodeKind::Scroll).then(|| path.id());
    let child_scroll = match (&entered, node.kind) {
        (Some(id), _) => Some((id.as_str(), node.props.scroll())),
        (None, NodeKind::Surface) => None,
        (None, _) => scroll,
    };
    for child in &node.children {
        walk(child, registry, path, Some(node.kind), child_scroll, errors);
    }
    path.pop();
}

/// Every grid child's span, against the tracks this grid actually has.
///
/// A span is a count of cells, and cells are finite, so a count larger than
/// the axis has tracks names a run that fits on no row of this grid at all.
/// That is the "runs past the last track" the contract refuses. Where the run
/// then *sits* is the grid's business, not the author's: `layout::grid`'s
/// seating walks the cells and takes the first free run wide enough, so a span
/// that fits somewhere always finds a home, and no two children ever share a
/// cell. There is nothing left here for an overlap check to catch.
///
/// The row bound is [`max_row_tracks`], not the declared row count: rows grow
/// to fit content, so a grid that declares none still has as many as its
/// children need, and a row span can legitimately ask for a row no child's
/// index names. What it may not do is ask for rows nothing in the tree
/// justifies at all.
///
/// Returns immediately when no child declares a span: a default span is one
/// cell, which always fits. A tree written before FR-061 existed pays nothing
/// here.
fn check_grid_spans(node: &ViewNode, path: &KeyPath, errors: &mut Vec<TreeError>) {
    let ncols = node.props.columns.len();
    // A grid with no columns already carries `MissingRequiredProp`; it has no
    // tracks to measure a span against, so nothing more is said here.
    if ncols == 0 || !node.children.iter().any(|c| c.props.span.is_some()) {
        return;
    }
    let nrows = max_row_tracks(node.props.rows.len(), node.children.len());
    for child in &node.children {
        let span = child.props.span();
        for (prop, count, tracks) in [
            ("span.columns", span.columns, ncols),
            ("span.rows", span.rows, nrows),
        ] {
            if count == 0 || count > tracks {
                errors.push(TreeError {
                    path: path.child(&child.key).id(),
                    violation: Violation::GridSpanOutOfRange {
                        prop,
                        count,
                        tracks,
                    },
                });
            }
        }
    }
}

fn check_node(
    node: &ViewNode,
    registry: &Registry,
    path: &KeyPath,
    parent: Option<NodeKind>,
    scroll: ScrollAncestor<'_>,
    errors: &mut Vec<TreeError>,
) {
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

    // A span is read by the `grid` that seats this node. Anywhere else it is a
    // dead declaration, and the counts themselves are checked by that grid in
    // `check_grid_spans`, which is the only place that knows the tracks.
    if node.props.span.is_some() && parent != Some(NodeKind::Grid) {
        push(Violation::SpanOutsideGrid { parent });
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
            if let Some((scroll_id, scroll)) = scroll {
                if let Some(overscan) = node.props.overscan {
                    push(Violation::ScrollParamOwnedByAncestor {
                        prop: "overscan",
                        declared: format!("{overscan}"),
                        scroll: scroll_id.to_owned(),
                        owner: format!("{}", scroll.overscan),
                    });
                }
                // An agreeing declaration is allowed: an author may spell out
                // the axis a list runs along. Only a disagreement is refused,
                // because only a disagreement would be overridden.
                if let Some(axis) = node.props.axis
                    && axis != scroll.axis
                {
                    push(Violation::ScrollParamOwnedByAncestor {
                        prop: "axis",
                        declared: Axis::as_str(axis).to_owned(),
                        scroll: scroll_id.to_owned(),
                        owner: scroll.axis.as_str().to_owned(),
                    });
                }
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

    // `spacing` and every edge of `padding` used to be range-checked here.
    // They are token references now (FR-053), so there is no number in the
    // tree left to range-check: a `TokenName` has no sign and cannot be NaN,
    // and serde refuses a bare number in the slot before acceptance ever
    // runs. The check did not disappear — it moved to where the number is now
    // written, which is `token::Theme::build`, and it refuses a theme that
    // assigns a negative or non-finite extent to any name.
    if node.props.padding.is_some() && !node.kind.is_container() {
        push(Violation::PaddingOnLeafKind { kind: node.kind });
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

    // A literal in a token slot (FR-013) used to be refused here: a
    // validate-time walk over `node.props.tokens` calling `is_style_literal`
    // on each `String` value, pushing `Violation::LiteralStyleValue` on a
    // hit. Both are gone. `node.props.tokens`'s value type is now
    // `TokenName` (contract C15), and `TokenName::new` — which its
    // `Deserialize` impl calls — already refuses a hex colour, a bare
    // number, and an `rgb(...)`/`hsl(...)` call
    // (`token::name::looks_like_style_literal`). A `Props` carrying a
    // literal in a token slot therefore cannot deserialize at all, so there
    // is nothing left for acceptance to catch: the tree cannot be built in
    // the first place. `a_literal_in_a_token_slot_never_deserializes` below
    // is the proof, at the boundary where a literal could actually arrive —
    // parsing a wire payload — rather than at construction in Rust, where
    // the type system already makes it unrepresentable.
}

#[cfg(test)]
mod tests {
    use super::{Registry, TreeError, Violation, validate};
    use crate::geom::Axis;
    use crate::testing::gap_token;
    use crate::tree::node::{Interaction, NodeKind, Role, Semantics, ViewNode};
    use crate::tree::props::{Anchor, Edge, GridSpan, InsetRefs, Layer, Props, TrackSize};

    fn stack(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
    }

    /// A `collection` that declares nothing about scrolling, for the
    /// scroll-ownership tests below.
    fn rows(props: Props) -> ViewNode {
        ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(10),
            source: Some("fibers".into()),
            ..props
        })
    }

    fn scroll(key: &str, props: Props) -> ViewNode {
        ViewNode::new(NodeKind::Scroll, key).with_props(props)
    }

    /// A grid `ncols` wide with no declared rows.
    fn grid(ncols: usize, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::FitContent; ncols],
                ..Props::default()
            })
            .with_children(children)
    }

    fn spanning(key: &str, columns: usize, rows: usize) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, key).with_props(Props {
            span: Some(GridSpan { columns, rows }),
            ..Props::default()
        })
    }

    /// A span counts the tracks a child covers, and a child always covers the
    /// one it sits in. Zero is not "no span", it is a contradiction, and
    /// reading it as one would make the wire form have two spellings for the
    /// default.
    #[test]
    fn a_span_of_no_tracks_is_refused_on_either_axis() {
        let tree = grid(3, vec![spanning("wide", 0, 1), spanning("tall", 1, 0)]);
        let err = validate(&tree, &Registry::new()).unwrap_err();
        let props: Vec<&str> = err
            .as_slice()
            .iter()
            .map(|e| match &e.violation {
                Violation::GridSpanOutOfRange { prop, count, .. } => {
                    assert_eq!(*count, 0, "only the zero counts should be refused here");
                    *prop
                }
                other => panic!("expected a span violation, got {other:?}"),
            })
            .collect();
        assert_eq!(props, ["span.columns", "span.rows"]);
        assert!(
            err.to_string()
                .contains("covers at least the one it sits in"),
            "the message must say why zero is wrong: {err}"
        );
    }

    /// Refused, not shortened. A four-column run in a three-column grid fits
    /// on no row of it, and quietly handing the author a three-column run
    /// gives them a panel that looks plausible and says something they did not
    /// write.
    #[test]
    fn a_column_span_wider_than_the_grid_is_refused_not_clamped() {
        let tree = grid(3, vec![spanning("wide", 4, 1)]);
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(
            err.as_slice()[0],
            TreeError {
                path: "/g/wide".to_owned(),
                violation: Violation::GridSpanOutOfRange {
                    prop: "span.columns",
                    count: 4,
                    tracks: 3,
                },
            }
        );
        assert!(
            err.to_string().contains("refused, never shortened"),
            "the message must rule out the clamp reading: {err}"
        );
    }

    /// A run exactly as wide as the grid is legal: it fits, on its own row.
    /// The bound is "more tracks than exist", not "more than one".
    #[test]
    fn a_column_span_the_full_width_of_the_grid_is_accepted() {
        let tree = grid(3, vec![spanning("banner", 3, 1), spanning("a", 1, 1)]);
        assert!(validate(&tree, &Registry::new()).is_ok());
    }

    /// Rows grow to fit content and always have, so a row span may ask for a
    /// row no child's index names. What it may not do is ask for rows nothing
    /// in the tree justifies at all — a `usize` typo would otherwise
    /// materialize every one of them.
    #[test]
    fn a_row_span_may_outrun_the_declared_rows_but_not_the_whole_tree() {
        // Two children, no declared rows: the ceiling is one row per child.
        let ok = grid(2, vec![spanning("tall", 1, 2), spanning("b", 1, 1)]);
        assert!(validate(&ok, &Registry::new()).is_ok());

        let too_tall = grid(2, vec![spanning("tall", 1, 3), spanning("b", 1, 1)]);
        let err = validate(&too_tall, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::GridSpanOutOfRange {
                prop: "span.rows",
                count: 3,
                tracks: 2,
            }
        );

        // Declared rows raise the ceiling: the week view's ninety-six.
        let week = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::FitContent; 7],
                rows: vec![TrackSize::Fixed { value: 12.0 }; 96],
                ..Props::default()
            })
            .child(spanning("event", 1, 4));
        assert!(validate(&week, &Registry::new()).is_ok());
    }

    /// The largest span there is must not wrap its way back into range.
    #[test]
    fn a_span_of_usize_max_is_refused_rather_than_wrapping_into_range() {
        let tree = grid(2, vec![spanning("absurd", usize::MAX, usize::MAX)]);
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 2, "both axes are out of range: {err}");
    }

    /// A span is read by the `grid` that seats the child. Anywhere else the
    /// declaration is dead, and a dead declaration is worse than a refused one
    /// — the same rule `PaddingOnLeafKind` follows.
    #[test]
    fn a_span_under_a_parent_that_is_not_a_grid_is_refused() {
        let tree = stack("root").child(spanning("child", 2, 1));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(
            err.as_slice()[0],
            TreeError {
                path: "/root/child".to_owned(),
                violation: Violation::SpanOutsideGrid {
                    parent: Some(NodeKind::Stack),
                },
            }
        );
        assert!(err.to_string().contains("would be ignored"), "{err}");
    }

    /// The root has no parent at all, so it has no grid to be seated in.
    #[test]
    fn a_span_on_the_root_is_refused_and_says_it_is_the_root() {
        let tree = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            span: Some(GridSpan {
                columns: 2,
                rows: 1,
            }),
            ..Props::default()
        });
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::SpanOutsideGrid { parent: None }
        );
        assert!(err.to_string().contains("is the root"), "{err}");
    }

    /// The check that must never cost an existing tree anything: a grid whose
    /// children declare no span at all is accepted, whatever shape it is.
    #[test]
    fn a_grid_that_declares_no_span_anywhere_is_accepted() {
        for children in [0_usize, 1, 3, 4, 7, 12] {
            let tree = grid(
                3,
                (0..children)
                    .map(|i| ViewNode::new(NodeKind::Spacer, format!("c{i}")))
                    .collect(),
            );
            assert!(
                validate(&tree, &Registry::new()).is_ok(),
                "{children} unspanned children must validate"
            );
        }
    }

    /// An explicit one-cell span is the default written out, and must be
    /// accepted exactly where an absent one is — including on a grid with a
    /// single column, where the ceiling is at its tightest.
    #[test]
    fn an_explicit_one_cell_span_is_accepted_wherever_an_absent_one_is() {
        let tree = grid(1, vec![spanning("a", 1, 1), spanning("b", 1, 1)]);
        assert!(validate(&tree, &Registry::new()).is_ok());
    }

    /// Overscan belongs to the `scroll`. Declaring it on the list inside
    /// would be silently ignored by layout, so it is refused here instead.
    #[test]
    fn a_collection_may_not_declare_the_overscan_its_scroll_owns() {
        let tree = scroll(
            "list",
            Props {
                overscan: Some(200.0),
                ..Props::default()
            },
        )
        .child(rows(Props {
            overscan: Some(96.0),
            ..Props::default()
        }));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err.as_slice()[0].path, "/list/rows");
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::ScrollParamOwnedByAncestor {
                prop: "overscan",
                declared: "96".into(),
                scroll: "/list".into(),
                owner: "200".into(),
            }
        );
        assert!(
            err.to_string().contains("declare it on the scroll"),
            "{err}"
        );
    }

    /// An axis that agrees is a legal restatement; one that disagrees is not,
    /// because the scroll's is the one the rows are laid out along.
    #[test]
    fn a_collection_may_restate_its_scrolls_axis_but_not_contradict_it() {
        let agreeing = scroll(
            "list",
            Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            },
        )
        .child(rows(Props {
            axis: Some(Axis::Horizontal),
            ..Props::default()
        }));
        assert!(validate(&agreeing, &Registry::new()).is_ok());

        let contradicting = scroll(
            "list",
            Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            },
        )
        .child(rows(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        }));
        let err = validate(&contradicting, &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::ScrollParamOwnedByAncestor {
                prop: "axis",
                declared: "vertical".into(),
                scroll: "/list".into(),
                owner: "horizontal".into(),
            }
        );
    }

    /// The ancestor chain is a chain, not a parent check: a list two
    /// containers below its scroll is still the scroll's.
    #[test]
    fn the_scroll_ancestor_is_found_through_intervening_containers() {
        let tree = scroll("list", Props::default()).child(stack("body").child(
            stack("inner").child(rows(Props {
                overscan: Some(96.0),
                ..Props::default()
            })),
        ));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(err.as_slice()[0].path, "/list/body/inner/rows");
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::ScrollParamOwnedByAncestor {
                prop: "overscan",
                declared: "96".into(),
                scroll: "/list".into(),
                // The scroll declares none, so it resolves to the default,
                // and that default is what would have won.
                owner: "64".into(),
            }
        );
    }

    /// Outside every scroll a `collection` owns its own scrolling
    /// parameters, and a `surface` puts it outside every scroll.
    #[test]
    fn a_collection_outside_every_scroll_keeps_its_own_parameters() {
        let bare = stack("root").child(rows(Props {
            overscan: Some(96.0),
            axis: Some(Axis::Horizontal),
            ..Props::default()
        }));
        assert!(validate(&bare, &Registry::new()).is_ok());

        let in_a_popup = scroll(
            "list",
            Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            },
        )
        .child(
            ViewNode::new(NodeKind::Surface, "popup")
                .with_props(Props {
                    layer: Some(Layer::Popup),
                    anchor: Some(Anchor::Viewport),
                    ..Props::default()
                })
                .child(rows(Props {
                    overscan: Some(96.0),
                    axis: Some(Axis::Horizontal),
                    ..Props::default()
                })),
        );
        assert!(
            validate(&in_a_popup, &Registry::new()).is_ok(),
            "a surface is anchored, not scrolled, so it starts a fresh chain"
        );
    }

    /// A leaf has no children to offer an inset rect to, so `padding` there
    /// is refused rather than silently ignored — the same policy
    /// `ScrollParamOwnedByAncestor` already applies to a `collection` that
    /// declares a parameter its `scroll` ancestor owns.
    #[test]
    fn a_leaf_kind_refuses_padding() {
        let node = ViewNode::new(NodeKind::Text, "t").with_props(Props {
            padding: Some(InsetRefs::all(gap_token(4.0))),
            ..Props::default()
        });
        let err = validate(&node, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 1, "{err}");
        assert_eq!(err.as_slice()[0].path, "/t");
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::PaddingOnLeafKind {
                kind: NodeKind::Text,
            }
        );
        assert!(
            err.to_string().contains("has no children to inset"),
            "{err}"
        );
    }

    /// A container kind is exactly where `padding` is meaningful, and it must
    /// be accepted there with no violation.
    #[test]
    fn a_container_kind_may_declare_padding() {
        let tree = stack("root").with_props(Props {
            padding: Some(InsetRefs::symmetric(gap_token(4.0), gap_token(8.0))),
            ..Props::default()
        });
        assert!(validate(&tree, &Registry::new()).is_ok());
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
            ..Props::default()
        });
        let err = validate(&node, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 1, "{err}");

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

    /// `spacing` and `padding` used to be range-checked here, edge by edge.
    /// They are token references now (FR-053), so the number an author could
    /// have got wrong is not in the tree any more — and this test pins where
    /// each half of that check went, so neither half can be lost quietly.
    ///
    /// The *shape* half is enforced before acceptance, by `TokenName`: a bare
    /// number, a hex string, or a negative literal in a styling slot fails to
    /// deserialize, so no such `Props` can be constructed to validate.
    /// The *range* half moved to `token::Theme::build`, which is where the
    /// number is now written.
    #[test]
    fn a_number_in_a_styling_slot_never_reaches_acceptance() {
        for bad in [
            r#"{"spacing":-1}"#,
            r#"{"spacing":0}"#,
            r#"{"column_spacing":7.5}"#,
            r#"{"padding":{"top":-1,"right":3}}"#,
        ] {
            let err = serde_json::from_str::<Props>(bad).unwrap_err();
            assert!(
                err.to_string().contains("invalid type") || err.to_string().contains("token name"),
                "{bad} should be refused as a non-name, got: {err}"
            );
        }

        // And the range half, at its new home. A theme is where the extent
        // is written, so a theme is what refuses a bad one.
        let mut values = crate::token::light().values().clone();
        values.insert(
            crate::token::TokenName::new("spacing.md").unwrap(),
            crate::token::TokenValue::Spacing(-1.0),
        );
        let err = crate::token::Theme::build(
            crate::token::ThemeMode::Light,
            &crate::token::standard_vocabulary(),
            values,
        )
        .unwrap_err();
        assert_eq!(err.unusable().len(), 1, "{err}");
        assert_eq!(err.unusable()[0].why, "negative");
    }

    /// C16's proof. FR-013 used to be a validate-time refusal
    /// (`is_style_literal` walking `Props.tokens`, pushing
    /// `Violation::LiteralStyleValue` on a hit); both are deleted. C15 makes
    /// `Props.tokens`'s value type `TokenName`, and `TokenName::new` — which
    /// its `Deserialize` impl calls — already refuses every shape
    /// `is_style_literal` refused: a hex colour, a bare number, an
    /// `rgb(...)`/`hsl(...)` call, and an empty or whitespace-only string.
    /// That refusal now fires at the wire boundary, before a `Props` — let
    /// alone a `ViewNode` `validate` could inspect — exists at all, which is
    /// strictly earlier than a validate-time check can ever run. Proved by
    /// deserializing the wire form, the same boundary
    /// `a_number_in_a_styling_slot_never_reaches_acceptance` uses for
    /// `spacing`/`padding`, rather than by constructing a `Props` in Rust
    /// directly — the type system already makes that construction not
    /// compile, so there is no runtime path left to exercise there.
    #[test]
    fn a_literal_in_a_token_slot_never_deserializes() {
        for literal in ["#ff0000", "12", "rgb(1,2,3)", " ", ""] {
            let json = format!(r#"{{"tokens":{{"background":{literal:?}}}}}"#);
            let err = serde_json::from_str::<Props>(&json).unwrap_err();
            assert!(
                err.to_string().contains("style value") || err.to_string().contains("empty"),
                "{literal:?} should be refused as a non-name, got: {err}"
            );
        }

        // A well-formed name still deserializes, and the tree it lands in
        // still accepts — the stronger check does not refuse more than the
        // old one did.
        let json = r#"{"tokens":{"background":"surface.raised"}}"#;
        let props: Props = serde_json::from_str(json).expect("a real token name deserializes");
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
