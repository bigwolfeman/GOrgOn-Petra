//! Tree acceptance.
//!
//! Every violation here is refused before any layout runs
//! (`contracts/view-tree.md`). The point is that a malformed tree fails with a
//! sentence naming the node, not with a panel that quietly lays out wrong or a
//! render-time surprise three frames later.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

use crate::geom::Axis;
use crate::token::{SlotSchema, TokenKind, TokenName, Vocabulary, standard_slots};
use crate::tree::key::{Key, KeyPath};
use crate::tree::node::{NodeKind, Role, ViewNode};
use crate::tree::props::{Anchor, ScrollProps, TrackSize, max_row_tracks};

/// Names the host has registered: custom node kinds, transition
/// definitions, and the design-token vocabulary. All three are populated by
/// the host before the first tree is accepted; an unregistered reference is
/// a violation, never a fallback.
///
/// The vocabulary is theme-independent (C6): it is the set of names a theme
/// must resolve, not a resolved theme itself, so `validate` never takes a
/// `ThemeSnapshot` — a tree accepted against this registry is accepted
/// under every theme built complete against the same vocabulary
/// (`token::Theme::build`).
#[derive(Clone, Debug)]
pub struct Registry {
    custom_kinds: BTreeSet<String>,
    transitions: BTreeSet<String>,
    vocabulary: Vocabulary,
    slots: SlotSchema,
}

impl Default for Registry {
    /// Empty on every host-configured axis, but carrying the **shipped** slot
    /// schema.
    ///
    /// The asymmetry is deliberate. Custom kinds, transitions and the
    /// vocabulary are things a particular host declares, so declaring none is
    /// the honest empty state. Which `TokenKind` the `background` slot takes
    /// is not host configuration — it is a fact about the design system, true
    /// of every host that uses the shipped painter. A host with its own
    /// painter replaces it through [`Registry::with_slots`].
    fn default() -> Self {
        Self {
            custom_kinds: BTreeSet::new(),
            transitions: BTreeSet::new(),
            vocabulary: Vocabulary::default(),
            slots: standard_slots(),
        }
    }
}

impl Registry {
    /// An empty registry: no custom kinds, no transitions, and no declared
    /// token vocabulary — so every styling token reference in a tree
    /// validated against it is refused as unknown. A host wires up its real
    /// vocabulary through [`Registry::with_vocabulary`]; this constructor is
    /// for the cases upstream of that (or for a tree that references no
    /// tokens at all).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry seeded with `vocabulary`, with no custom kinds or
    /// transitions registered yet — chain [`Registry::register_custom_kind`]
    /// and [`Registry::register_transition`] as needed.
    #[must_use]
    pub fn with_vocabulary(vocabulary: Vocabulary) -> Self {
        Self {
            vocabulary,
            ..Self::default()
        }
    }

    /// Replace the slot schema, for a host whose painter draws slots the
    /// shipped one does not.
    ///
    /// An empty schema turns the slot kind check off entirely: a slot the
    /// schema does not declare carries a token of any kind, by design.
    #[must_use]
    pub fn with_slots(mut self, slots: SlotSchema) -> Self {
        self.slots = slots;
        self
    }

    /// The slot schema this registry accepts trees against.
    #[must_use]
    pub fn slots(&self) -> &SlotSchema {
        &self.slots
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

    /// The declared token vocabulary this registry validates styling
    /// references against.
    #[must_use]
    pub fn vocabulary(&self) -> &Vocabulary {
        &self.vocabulary
    }

    /// The vocabulary, mutably — for a host that builds its `Registry` once
    /// (through, say, a UI toolkit's own constructor) and declares its
    /// tokens afterward, the same way [`Registry::register_custom_kind`] and
    /// [`Registry::register_transition`] are called after construction
    /// rather than threaded through it.
    pub fn vocabulary_mut(&mut self) -> &mut Vocabulary {
        &mut self.vocabulary
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
    /// A styling prop names a token the vocabulary does not declare at all
    /// (FR-056): unknown, misspelled, or simply never registered.
    ///
    /// Never falls back to a default and never renders — the only outcomes
    /// for a node carrying this violation are refusal here, at tree
    /// acceptance, before any theme or layout pass sees the name.
    UnknownTokenRef {
        /// The `props` field the name was given to (`"spacing"`,
        /// `"padding.top"`, `"tokens.background"`, …).
        prop: String,
        /// The offending name, exactly as declared.
        name: TokenName,
        /// The names that *would* have been legal here: every vocabulary
        /// entry of the kind this prop expects, not the whole vocabulary —
        /// a spacing prop was never going to accept a colour name, so
        /// listing one would not help the author find the fix.
        legal: Vec<TokenName>,
    },
    /// A styling prop names a token the vocabulary declares, but at a
    /// [`TokenKind`] this prop does not accept — `spacing` naming a token
    /// declared [`TokenKind::Color`], for instance.
    ///
    /// Distinct from [`Violation::UnknownTokenRef`] because the fix is
    /// different: the name is not a typo, it is a real token that belongs to
    /// a different slot.
    TokenKindMismatch {
        /// The `props` field the name was given to.
        prop: String,
        /// The offending name.
        name: TokenName,
        /// The kind this prop's slot requires.
        expected: TokenKind,
        /// The kind the vocabulary actually declares `name` at.
        found: TokenKind,
        /// Every vocabulary entry actually declared at `expected` — the
        /// legal set for this slot.
        legal: Vec<TokenName>,
    },
    /// A `surface`'s anchor names an id no node in the tree has: an
    /// [`Anchor::Node`] whose `id` matches no node, or an
    /// [`Anchor::Sibling`] whose `key` no sibling of the surface carries.
    ///
    /// Refused rather than resolved as [`Anchor::Viewport`]
    /// (`contracts/anchored-placement.md` §2a). A silent fallback would put a
    /// menu in the middle of the window instead of under the button that
    /// opened it, and there is nothing on screen to say a fallback was taken
    /// — the same reason `UnregisteredCustomKind` is refused instead of
    /// drawn as an empty box. It is also half of what makes the resolution
    /// scheme total: an anchor that names nothing has no rect to harvest, so
    /// the harvest walk would have to invent one.
    AnchorTargetMissing {
        /// The canonical id the anchor resolved to: the `id` an
        /// [`Anchor::Node`] declared verbatim, or the id an
        /// [`Anchor::Sibling`]'s key produced against the surface's own
        /// parent path ([`crate::tree::Anchor::target_id`]). For a sibling
        /// anchor this is the one candidate that was tried, so an author
        /// can see exactly which child list the key was looked for in.
        id: String,
        /// The ids the tree does have, for an author who mistyped one. Every
        /// surface anchor in a real tree names a node the author just wrote,
        /// so the list is what turns "that id is wrong" into "you meant this
        /// one".
        known: Vec<String>,
    },
    /// A `surface`'s anchor depends, directly or transitively, on its own
    /// placement.
    ///
    /// A surface anchored into a node that lives inside another anchored
    /// surface is legal and costs one more harvest walk
    /// (`contracts/anchored-placement.md` §1.4). What is refused here is the
    /// case where following that dependency comes back to where it started:
    /// the anchor rect of `A` cannot be known until `B` is placed, and `B`'s
    /// cannot be known until `A` is. This is the *other* half of what makes
    /// the resolution scheme total — the scheme carries no cycle detection
    /// because no cycle survives acceptance.
    AnchorCycle {
        /// The dependency chain, starting and ending at this surface.
        through: Vec<String>,
    },
    /// `props.state_tokens` keys on a state name the engine never resolves.
    ///
    /// Refused for the reason [`Violation::PaddingOnLeafKind`] is refused: no
    /// state by that name is ever entered, so the override would be a dead
    /// declaration — an author's `hovered:` block that never once paints,
    /// with nothing on screen to say why.
    UnknownStateName {
        /// The declared key.
        name: String,
        /// The names the dispatcher actually resolves.
        legal: Vec<&'static str>,
    },
}

/// A [`TokenKind`] rendered the way a refusal message names it. Not
/// [`std::fmt::Display`] on [`TokenKind`] itself: that type lives in
/// `token::value`, which this module does not own, and the wording here is
/// specific to how a violation names a kind mismatch.
fn kind_word(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Color => "color",
        TokenKind::Spacing => "spacing",
        TokenKind::Typography => "typography",
        TokenKind::Motion => "motion",
        TokenKind::Spring => "spring",
        TokenKind::Shape => "shape",
        TokenKind::Silhouette => "silhouette",
        TokenKind::Coverage => "coverage",
    }
}

/// `legal`, rendered as the message body names it: `[a, b, c]`, or
/// `[none declared]` for a vocabulary that declares nothing of the kind a
/// prop needed — which is itself informative, not a blank the author has to
/// wonder about.
fn legal_set(legal: &[TokenName]) -> String {
    if legal.is_empty() {
        return "none declared".to_owned();
    }
    legal
        .iter()
        .map(TokenName::as_str)
        .collect::<Vec<_>>()
        .join(", ")
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
            Self::UnknownTokenRef { prop, name, legal } => write!(
                f,
                "props.{prop} names token `{name}`, which the vocabulary does not declare; legal: [{}]",
                legal_set(legal)
            ),
            Self::AnchorTargetMissing { id, known } => write!(
                f,
                "props.anchor names node `{id}`, which this tree has no node for; an anchor that names nothing has no rect to be placed against, and is refused rather than centred in the viewport; known ids: [{}]",
                known.join(", ")
            ),
            Self::AnchorCycle { through } => write!(
                f,
                "props.anchor depends on this surface's own placement, through {}; a surface may be anchored into a node inside another anchored surface, but not into one whose own anchor leads back here",
                through.join(" -> ")
            ),
            Self::UnknownStateName { name, legal } => write!(
                f,
                "props.state_tokens keys on state {name:?}, which the placement dispatcher never enters, so the override would never paint; legal: [{}]",
                legal.join(", ")
            ),
            Self::TokenKindMismatch {
                prop,
                name,
                expected,
                found,
                legal,
            } => write!(
                f,
                "props.{prop} names token `{name}`, which is a {} token, but props.{prop} takes a {} token; legal: [{}]",
                kind_word(*found),
                kind_word(*expected),
                legal_set(legal)
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
/// Returns every violation found: first every per-node one, in tree
/// pre-order, then the anchor violations [`check_anchors`] finds. Those two
/// groups are two passes because they are two different questions — whether
/// one node is well formed, and whether the tree's `surface` anchors name
/// each other in a way that can be resolved at all — and the second cannot be
/// asked until every node's id is known.
pub fn validate<'a>(
    root: &'a ViewNode,
    registry: &Registry,
) -> Result<ValidatedTree<'a>, TreeErrors> {
    let mut errors = Vec::new();
    let mut path = KeyPath::root();
    walk(root, registry, &mut path, None, None, &mut errors);
    check_anchors(root, &mut errors);
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

    // Every rule that turns on which kind of node this is, one named check
    // per kind, in this order; a kind that carries no rules of its own falls
    // through. A `collection` rule therefore has exactly one place to land,
    // and a reader asking what a `surface` must declare finds the answer by
    // name instead of by counting braces. Nothing here is order-sensitive
    // across arms: a node has one kind, so at most one arm ever runs.
    match node.kind {
        NodeKind::Custom => check_custom_kind(node, registry, &mut push),
        NodeKind::Grid => check_grid_tracks(node, &mut push),
        NodeKind::Collection => check_collection(node, scroll, &mut push),
        NodeKind::Surface => check_surface(node, &mut push),
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
    // assigns a negative or non-finite extent to any name in `values`.
    //
    // "In `values`" is load-bearing and was got wrong once. This note used to
    // say "any name", while `Theme::build` walked `vocabulary.names()` to
    // decide what to inspect — so a value under an undeclared name was never
    // checked, and `Vocabulary::from_theme` declared it anyway. The
    // replacement was narrower than the note claimed, which is the one way a
    // deletion note can be worse than no note. `Theme::build` now walks
    // `values`, and `an_extent_under_a_name_the_vocabulary_never_declared_is_checked_too`
    // pins it.
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

    // FR-056: every styling token reference this node declares must name a
    // vocabulary entry, and — for the props whose slot has one fixed shape —
    // at the kind that slot requires. Checked unconditionally, independent
    // of `node.kind`: a reference is either right or refused, the same way
    // `opacity` above is range-checked whether or not this kind of node ever
    // reads it. A field this kind does not use is usually caught by its own
    // violation already (`PaddingOnLeafKind`, `ScrollParamOwnedByAncestor`);
    // that a declaration is dead is a different fact from whether the name
    // in it is real, and both are worth refusing.
    let vocabulary = registry.vocabulary();
    for (prop, expected, reference) in [
        ("spacing", TokenKind::Spacing, &node.props.spacing),
        (
            "column_spacing",
            TokenKind::Spacing,
            &node.props.column_spacing,
        ),
        ("row_spacing", TokenKind::Spacing, &node.props.row_spacing),
        ("style", TokenKind::Typography, &node.props.style),
    ] {
        if let Some(name) = reference {
            check_token_ref(vocabulary, prop, name, expected, &mut push);
        }
    }
    if let Some(refs) = &node.props.padding {
        for (prop, reference) in [
            ("padding.top", &refs.top),
            ("padding.right", &refs.right),
            ("padding.bottom", &refs.bottom),
            ("padding.left", &refs.left),
        ] {
            if let Some(name) = reference {
                check_token_ref(vocabulary, prop, name, TokenKind::Spacing, &mut push);
            }
        }
    }
    // `props.tokens`'s keys are paint slots, not design-token kinds
    // (`tree::props`'s own doc comment on the field: the slot is "decided by
    // the painter", e.g. `gorgon_petra_egui::paint::KNOWN_SLOTS`). This
    // module does not — and must not — **hardcode** which slot wants which
    // `TokenKind`, because a host can register a painter with slots this
    // crate has never heard of, and a slot a *shipped* painter does not know
    // yet (an invented name, proven live by
    // `gorgon_petra_egui::paint::tests::a_genuinely_unknown_token_slot_is_recorded`)
    // can legitimately carry a token of any kind — that mismatch is the
    // painter's `unknown_slots` report to make, not tree acceptance's.
    //
    // It reads the pairing from `registry.slots()` instead, which is the
    // whole reason `token::SlotSchema` exists and is the difference between
    // "must not hardcode" and "must not check". A slot the schema declares is
    // checked against its declared kind; a slot the schema does not declare
    // is checked for existence only, exactly as before, so a host painter's
    // invented slot is unaffected.
    //
    // Existence-only was the shipped behaviour and it let a `Spacing` token
    // bind to the `background` colour slot: the tree was accepted,
    // `semantic::audit` reported nothing, and the marker painted no colour at
    // all. For a status indicator that is the inverse of the FR-015 incident
    // — not colour carrying meaning alone, but the colour channel silently
    // going missing while the shape and word channels stay.
    // The gap between an anchor and the surface it carries is a spacing
    // token for the reason `props.spacing` is one (FR-053), so it is checked
    // exactly the way `props.spacing` is — a literal cannot arrive here at
    // all, because the field's type is `TokenName`.
    if let Some(name) = node.props.anchor.as_ref().and_then(Anchor::offset) {
        check_token_ref(
            vocabulary,
            "anchor.offset",
            name,
            TokenKind::Spacing,
            &mut push,
        );
    }
    // A state-decorated key names a state the resolver will actually enter,
    // or it is refused here.
    //
    // `crate::token::resolve_slot` builds its candidates from the flags on a
    // placement, so a key it never builds is never looked up: `background@hovr`
    // is not an error at paint time, it is silence. That is the one failure
    // mode this authoring shape has that a typed map would not, so tree
    // acceptance is where it gets closed. The legal set is the resolver's own
    // chain, not a second list that can drift from it.
    for slot in node.props.tokens.keys() {
        let Some((_, state)) = slot.split_once(crate::token::STATE_SEPARATOR) else {
            continue;
        };
        if !legal_state_suffix(state) {
            push(Violation::UnknownStateName {
                name: state.to_owned(),
                legal: legal_state_suffixes(),
            });
        }
    }
    for (slot, name) in &node.props.tokens {
        match vocabulary.kind_of(name) {
            None => push(Violation::UnknownTokenRef {
                prop: format!("tokens.{slot}"),
                name: name.clone(),
                legal: vocabulary.names().cloned().collect(),
            }),
            Some(found) => {
                if let Some(spec) = registry.slots().get(slot.as_str())
                    && spec.kind() != found
                {
                    push(Violation::TokenKindMismatch {
                        prop: format!("tokens.{slot}"),
                        name: name.clone(),
                        expected: spec.kind(),
                        found,
                        legal: vocabulary
                            .names()
                            .filter(|n| vocabulary.kind_of(n) == Some(spec.kind()))
                            .cloned()
                            .collect(),
                    });
                }
            }
        }
    }
}

/// A `custom` node names a kind the host has registered.
///
/// The whole point of the kind is that this crate has never heard of it, so
/// the registry is the only authority there is: an unregistered name — or no
/// name at all — is refused rather than drawn as an empty box, and the
/// refusal carries the registered set, so an author who misspelled one can
/// see what they were reaching for.
fn check_custom_kind(node: &ViewNode, registry: &Registry, push: &mut impl FnMut(Violation)) {
    match node.props.custom_kind.as_deref() {
        Some(name) if registry.has_custom_kind(name) => {}
        other => push(Violation::UnregisteredCustomKind {
            name: other.map(str::to_owned),
            registered: registry
                .custom_kinds()
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }),
    }
}

/// A `grid`'s own tracks: at least one column, and every declared weight a
/// real ratio.
///
/// Columns are required because a grid with none has no tracks to seat
/// anything against. [`check_grid_spans`] — the sibling check, run from
/// [`walk`] because only the grid knows its own tracks — says nothing at all
/// in that case, so this is the violation that has to name the cause. Rows
/// are not required: they grow to fit content ([`max_row_tracks`]).
///
/// A `Weight` track takes a share of the leftover axis in proportion, so
/// zero, negative, and non-finite are one bug with three spellings — a share
/// of nothing, or of a total that is itself NaN. Both axes go through the one
/// loop, and each carries its own prop name: a bad row weight said
/// `columns[].weight` until this check was extracted, which sent the author
/// to the wrong list.
fn check_grid_tracks(node: &ViewNode, push: &mut impl FnMut(Violation)) {
    if node.props.columns.is_empty() {
        push(Violation::MissingRequiredProp {
            kind: NodeKind::Grid,
            prop: "columns",
        });
    }
    let axes = [
        ("columns[].weight", &node.props.columns),
        ("rows[].weight", &node.props.rows),
    ];
    for (prop, tracks) in axes {
        for track in tracks {
            if let TrackSize::Weight { weight } = track
                && !(weight.is_finite() && *weight > 0.0)
            {
                push(Violation::ValueOutOfRange {
                    prop,
                    value: format!("{weight}"),
                    expected: "a finite weight greater than zero",
                });
            }
        }
    }
}

/// A `collection`'s required props, and the scroll parameters it may not take
/// back from an enclosing `scroll`.
///
/// `total_count` and `source` are what virtualisation runs on: the count
/// fixes the scrollable extent before a single item is built, and the source
/// names where the items come from. Neither has a defensible default, so
/// neither is optional.
///
/// `overscan` and `axis` belong to the nearest enclosing `scroll`
/// ([`ScrollAncestor`]), which resolves them once for everything beneath it.
/// A declaration here would be quietly overridden, so it is refused instead,
/// and the refusal names both the value written and the value that wins.
fn check_collection(node: &ViewNode, scroll: ScrollAncestor<'_>, push: &mut impl FnMut(Violation)) {
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
        // An agreeing declaration is allowed: an author may spell out the
        // axis a list runs along. Only a disagreement is refused, because
        // only a disagreement would be overridden.
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

/// A `surface` declares both of the things that place it.
///
/// A surface is anchored in viewport coordinates rather than laid out by its
/// parent, so `anchor` decides where it sits and `layer` decides what it sits
/// in front of. Defaulting either one would put a popover somewhere the
/// author did not choose, in front of or behind something they did not
/// choose, with nothing on screen to say a default had been taken.
fn check_surface(node: &ViewNode, push: &mut impl FnMut(Violation)) {
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

/// One `surface` whose anchor names another node, as [`check_anchors`] needs
/// it: pre-order position, its own canonical id, and the id it names.
struct AnchoredSurface {
    /// This surface's own canonical key path.
    id: String,
    /// The canonical id its anchor names, already resolved: an
    /// [`Anchor::Node`]'s `id` as declared, or an [`Anchor::Sibling`]'s key
    /// against this surface's own parent path. Past this point the two
    /// spellings are one thing ([`Anchor::target_id`]).
    target: String,
}

/// The `surface` anchors of the whole tree, judged together.
///
/// Two refusals, and both are what make `contracts/anchored-placement.md`
/// §1's resolution scheme *total* rather than merely usual:
///
/// * An anchor naming no node — an [`Anchor::Node`] with an unknown `id`,
///   or an [`Anchor::Sibling`] whose key no sibling carries — has no rect
///   to harvest ([`Violation::AnchorTargetMissing`]).
/// * A surface whose anchor leads, through the surfaces enclosing it, back to
///   itself can never be placed at all ([`Violation::AnchorCycle`]).
///
/// Neither can be judged one node at a time, which is why this is a second
/// pass rather than an arm of [`check_node`]: the first needs every id in the
/// tree, and the second needs every anchor.
///
/// The dependency edge is *not* "surface names node": it is "surface `A`
/// needs the placement of surface `B`", where `B` is the nearest anchored
/// surface enclosing `A`'s anchor target. A target in ordinary flow has no
/// such enclosing surface and so contributes no edge, which is why an
/// ordinary popover — the overwhelming majority of trees — reaches nothing
/// here at all. Each surface has at most one outgoing edge, so following the
/// chain is the whole of cycle detection.
///
/// Returns immediately for a tree with no node anchor anywhere: the walk
/// that collects the ids is the only cost, and it is not paid twice.
///
/// There is one anchor system here, not two. A sibling anchor is turned into
/// the canonical id it means at the moment the walk reaches its surface
/// ([`collect_anchors`]), and everything after that — the id lookup, the
/// refusal, the cycle scan — reads the resolved id and never asks which
/// spelling produced it.
fn check_anchors(root: &ViewNode, errors: &mut Vec<TreeError>) {
    let mut ids: BTreeSet<String> = BTreeSet::new();
    let mut surfaces: Vec<AnchoredSurface> = Vec::new();
    // Node id -> the nearest *anchored* surface enclosing it. A surface
    // anchored to a point or to the viewport is placed without reference to
    // any other node, so it does not enclose its subtree in a dependency;
    // only one that names a node does.
    let mut enclosing: BTreeMap<String, String> = BTreeMap::new();
    collect_anchors(
        root,
        &mut KeyPath::root(),
        None,
        &mut ids,
        &mut surfaces,
        &mut enclosing,
    );
    if surfaces.is_empty() {
        return;
    }

    let mut edge: BTreeMap<&str, &str> = BTreeMap::new();
    for surface in &surfaces {
        if !ids.contains(&surface.target) {
            errors.push(TreeError {
                path: surface.id.clone(),
                violation: Violation::AnchorTargetMissing {
                    id: surface.target.clone(),
                    known: ids.iter().cloned().collect(),
                },
            });
            continue;
        }
        if let Some(owner) = enclosing.get(&surface.target) {
            edge.insert(surface.id.as_str(), owner.as_str());
        }
    }

    for surface in &surfaces {
        // At most one edge leaves each surface, so a chain longer than the
        // surface count has revisited something; the start is what we are
        // asking about, so it is a cycle through this surface exactly when
        // this surface is what it comes back to.
        let mut chain = vec![surface.id.as_str()];
        let mut at = surface.id.as_str();
        for _ in 0..surfaces.len() {
            let Some(next) = edge.get(at).copied() else {
                break;
            };
            chain.push(next);
            if next == surface.id.as_str() {
                errors.push(TreeError {
                    path: surface.id.clone(),
                    violation: Violation::AnchorCycle {
                        through: chain.iter().map(|s| (*s).to_owned()).collect(),
                    },
                });
                break;
            }
            at = next;
        }
    }
}

/// One level of [`check_anchors`]'s walk: every node's id, every anchored
/// surface, and which anchored surface (if any) each node is inside.
fn collect_anchors(
    node: &ViewNode,
    path: &mut KeyPath,
    owner: Option<&str>,
    ids: &mut BTreeSet<String>,
    surfaces: &mut Vec<AnchoredSurface>,
    enclosing: &mut BTreeMap<String, String>,
) {
    path.push(node.key.clone());
    let id = path.id();
    if let Some(owner) = owner {
        enclosing.insert(id.clone(), owner.to_owned());
    }
    // `path` is this node's own full path here, which is exactly the context
    // a sibling anchor resolves against.
    let target = node
        .props
        .anchor
        .as_ref()
        .and_then(|anchor| anchor.target_id(path));
    let anchored = if let Some(target) = target {
        surfaces.push(AnchoredSurface {
            id: id.clone(),
            target,
        });
        // An anchored surface owns *itself*, not just its subtree: a surface
        // anchored to its own rect is as circular as one anchored to a node
        // inside it, and the map is what the cycle scan reads.
        enclosing.insert(id.clone(), id.clone());
        Some(id.clone())
    } else {
        None
    };
    ids.insert(id);
    let child_owner = anchored.as_deref().or(owner);
    for child in &node.children {
        collect_anchors(child, path, child_owner, ids, surfaces, enclosing);
    }
    path.pop();
}

/// One paint-slot binding against the vocabulary and the registry's slot
/// schema: the same rule the `props.tokens` loop in [`check_node`] applies,
/// so a per-state override can never be checked more loosely than the base
/// binding it overrides.
///
/// `prop` is what a refusal names (`state_tokens.hover.background`); `slot` is
/// the paint slot itself, which is what the schema is keyed on. They differ,
/// which is why both are passed: a message naming `background` would not tell
/// an author which of their state blocks to fix.
/// Every state suffix `crate::token::resolve_slot` can look for, in the order
/// its chain tries them.
///
/// Derived from [`crate::token::InteractionRank`] rather than written out, so
/// a rank added to the resolver cannot leave this list behind. `Enabled` has
/// no suffix — it *is* the undecorated slot — so it contributes nothing here.
fn legal_state_suffixes() -> Vec<&'static str> {
    use crate::token::InteractionRank;
    const RANKS: [InteractionRank; 4] = [
        InteractionRank::Skeleton,
        InteractionRank::Disabled,
        InteractionRank::Active,
        InteractionRank::Hover,
    ];
    let mut out = vec!["selected"];
    for rank in RANKS {
        if let Some(name) = rank.as_str() {
            out.push(name);
        }
    }
    out
}

/// Whether `suffix` is one the resolver's chain can produce, including the
/// `selected-<rank>` combinations Carbon names as their own tokens.
fn legal_state_suffix(suffix: &str) -> bool {
    let bare = suffix.strip_prefix("selected-").unwrap_or(suffix);
    if bare != suffix {
        // `selected-<rank>`: the combination form. `selected-enabled` is not
        // one — that combination is spelled `selected`.
        return legal_state_suffixes()
            .iter()
            .any(|name| *name == bare && *name != "selected");
    }
    legal_state_suffixes().contains(&suffix)
}

/// One styling prop's declared token name against the vocabulary: refused if
/// the name is not declared at all
/// ([`Violation::UnknownTokenRef`]), refused if it is declared but at the
/// wrong [`TokenKind`] for this prop's slot
/// ([`Violation::TokenKindMismatch`]), otherwise silent. A name is one or the
/// other, never both, so `push` is called at most once per reference.
fn check_token_ref(
    vocabulary: &Vocabulary,
    prop: &str,
    name: &TokenName,
    expected: TokenKind,
    push: &mut impl FnMut(Violation),
) {
    match vocabulary.kind_of(name) {
        None => push(Violation::UnknownTokenRef {
            prop: prop.to_owned(),
            name: name.clone(),
            legal: vocabulary
                .names_of_kind(expected)
                .into_iter()
                .cloned()
                .collect(),
        }),
        Some(found) if found != expected => push(Violation::TokenKindMismatch {
            prop: prop.to_owned(),
            name: name.clone(),
            expected,
            found,
            legal: vocabulary
                .names_of_kind(expected)
                .into_iter()
                .cloned()
                .collect(),
        }),
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{Registry, TreeError, Violation, validate};
    use crate::geom::Axis;
    use crate::testing::gap_token;
    use crate::token::{
        DesignToken, SlotSchema, TokenKind, TokenName, Vocabulary, light, standard_slots,
        standard_vocabulary,
    };
    use crate::tree::node::{Interaction, NodeKind, Role, Semantics, ViewNode};
    use crate::tree::props::{Anchor, Edge, GridSpan, InsetRefs, Layer, Props, TrackSize};

    fn stack(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Stack, key)
    }

    /// A registry whose vocabulary declares exactly the two fixture gaps the
    /// padding tests below reference (`gap_token(4.0)`, `gap_token(8.0)`),
    /// each at [`TokenKind::Spacing`] — just enough for those tests to stay
    /// about padding placement, not about the token vocabulary FR-056 checks
    /// against.
    fn spacing_registry() -> Registry {
        let mut vocab = Vocabulary::new();
        for units in [4.0, 8.0] {
            vocab.declare(DesignToken::new(gap_token(units), TokenKind::Spacing));
        }
        Registry::with_vocabulary(vocab)
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
        let err = validate(&node, &spacing_registry()).unwrap_err();
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
        assert!(validate(&tree, &spacing_registry()).is_ok());
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

        // The other half of the surface rule. Every other surface fixture in
        // this crate supplies `layer`, so without this case the `layer` push
        // could be deleted and the whole suite would stay green.
        //
        // Both fixtures below sit beside a real anchor node, because an
        // `Anchor::Node` naming nothing is now its own violation
        // (`AnchorTargetMissing`) and would mask the one under test.
        let unlayered = ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "button"))
            .child(ViewNode::new(NodeKind::Surface, "s").with_props(Props {
                anchor: Some(anchored_at("/root/button")),
                ..Props::default()
            }));
        let err = validate(&unlayered, &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::MissingRequiredProp {
                kind: NodeKind::Surface,
                prop: "layer"
            }
        );

        let ok = ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "button"))
            .child(ViewNode::new(NodeKind::Surface, "s").with_props(Props {
                layer: Some(Layer::Popup),
                anchor: Some(anchored_at("/root/button")),
                ..Props::default()
            }));
        assert!(validate(&ok, &Registry::new()).is_ok());
    }

    /// A `bottom`-edge node anchor on `id`, centred with no offset — the
    /// shape every anchored-surface fixture in this module wants.
    fn anchored_at(id: &str) -> Anchor {
        Anchor::Node {
            id: id.to_owned(),
            edge: Edge::Bottom,
            align: crate::tree::Align::Center,
            offset: None,
        }
    }

    #[test]
    fn out_of_range_values_are_named() {
        let node = ViewNode::new(NodeKind::Text, "t").with_props(Props {
            opacity: Some(1.5),
            ..Props::default()
        });
        let err = validate(&node, &Registry::new()).unwrap_err();
        assert_eq!(err.len(), 1, "{err}");

        // Both axes are checked, and each names its own list. Asserting the
        // prop rather than the variant is what makes a row weight reported as
        // `columns[].weight` a failure here.
        let grid = ViewNode::new(NodeKind::Grid, "g").with_props(Props {
            columns: vec![TrackSize::Weight { weight: 0.0 }],
            ..Props::default()
        });
        let err = validate(&grid, &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::ValueOutOfRange {
                prop: "columns[].weight",
                value: "0".into(),
                expected: "a finite weight greater than zero",
            }
        );

        let rows = ViewNode::new(NodeKind::Grid, "g").with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            rows: vec![TrackSize::Weight { weight: f32::NAN }],
            ..Props::default()
        });
        let err = validate(&rows, &Registry::new()).unwrap_err();
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::ValueOutOfRange {
                prop: "rows[].weight",
                value: "NaN".into(),
                expected: "a finite weight greater than zero",
            }
        );
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
        // still accepts — against a registry that actually declares it
        // (FR-056: an *undeclared* well-formed name is refused too, which is
        // exactly what the rest of this module's FR-056 tests cover — this
        // one is about the deserialize boundary, not the vocabulary one).
        let json = r#"{"tokens":{"background":"surface.raised"}}"#;
        let props: Props = serde_json::from_str(json).expect("a real token name deserializes");
        assert!(
            validate(
                &ViewNode::new(NodeKind::Text, "t").with_props(props),
                &Registry::with_vocabulary(standard_vocabulary())
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

    // ---------------------------------------------------------- FR-056: the
    // vocabulary. An unknown, misspelled, or ill-typed token reference is
    // refused naming the value, the parameter, and the legal set — never a
    // fallback, never a render.

    /// The refusal names all three things FR-056 requires, proved on the
    /// *rendered* message (gate B4-2b) rather than by reading the struct's
    /// fields: a `Display` impl that forgot to print `legal` would still
    /// pass a field-level assertion, and this is the test built to catch
    /// exactly that.
    #[test]
    fn an_unknown_token_reference_names_value_parameter_and_legal_set() {
        let mut vocab = Vocabulary::new();
        vocab
            .declare(DesignToken::new(
                TokenName::new("spacing.md").unwrap(),
                TokenKind::Spacing,
            ))
            .declare(DesignToken::new(
                TokenName::new("spacing.sm").unwrap(),
                TokenKind::Spacing,
            ));
        let tree = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            spacing: Some(TokenName::new("spacing.bogus").unwrap()),
            ..Props::default()
        });
        let err = validate(&tree, &Registry::with_vocabulary(vocab)).unwrap_err();
        assert_eq!(err.len(), 1, "{err}");
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::UnknownTokenRef {
                prop: "spacing".to_owned(),
                name: TokenName::new("spacing.bogus").unwrap(),
                legal: vec![
                    TokenName::new("spacing.md").unwrap(),
                    TokenName::new("spacing.sm").unwrap(),
                ],
            }
        );
        let message = err.to_string();
        assert!(message.contains("spacing.bogus"), "the value: {message}");
        assert!(
            message.contains("props.spacing"),
            "the parameter: {message}"
        );
        assert!(
            message.contains("spacing.md") && message.contains("spacing.sm"),
            "the legal set: {message}"
        );
    }

    /// A name of the wrong *kind* for its slot is refused too — `spacing`
    /// naming a token the vocabulary declares as [`TokenKind::Color`] — and
    /// the message names both kinds plus the legal set for the one the slot
    /// actually wanted, proved on the rendered message (B4-2b).
    #[test]
    fn a_token_declared_at_the_wrong_kind_is_refused_naming_both_kinds() {
        let mut vocab = Vocabulary::new();
        vocab
            .declare(DesignToken::new(
                TokenName::new("surface.raised").unwrap(),
                TokenKind::Color,
            ))
            .declare(DesignToken::new(
                TokenName::new("spacing.md").unwrap(),
                TokenKind::Spacing,
            ));
        let tree = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            spacing: Some(TokenName::new("surface.raised").unwrap()),
            ..Props::default()
        });
        let err = validate(&tree, &Registry::with_vocabulary(vocab)).unwrap_err();
        assert_eq!(err.len(), 1, "{err}");
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::TokenKindMismatch {
                prop: "spacing".to_owned(),
                name: TokenName::new("surface.raised").unwrap(),
                expected: TokenKind::Spacing,
                found: TokenKind::Color,
                legal: vec![TokenName::new("spacing.md").unwrap()],
            }
        );
        let message = err.to_string();
        assert!(message.contains("surface.raised"), "the value: {message}");
        assert!(
            message.contains("props.spacing"),
            "the parameter: {message}"
        );
        assert!(message.contains("color"), "the found kind: {message}");
        assert!(message.contains("spacing.md"), "the legal set: {message}");
    }

    /// The same declared name is legal in one slot and refused in another —
    /// `surface.raised` fits `tokens.background` (a [`TokenKind::Color`]
    /// slot) but not `spacing` (a [`TokenKind::Spacing`] one). Fitness is a
    /// property of the pairing, not of the name alone.
    #[test]
    fn a_name_legal_in_one_slot_is_refused_in_another() {
        let mut vocab = Vocabulary::new();
        vocab.declare(DesignToken::new(
            TokenName::new("surface.raised").unwrap(),
            TokenKind::Color,
        ));
        let registry = Registry::with_vocabulary(vocab);

        let legal_slot = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            tokens: [(
                "background".to_owned(),
                TokenName::new("surface.raised").unwrap(),
            )]
            .into_iter()
            .collect(),
            ..Props::default()
        });
        assert!(
            validate(&legal_slot, &registry).is_ok(),
            "surface.raised is a declared Color, and tokens.background \
             checks existence only"
        );

        let wrong_slot = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            spacing: Some(TokenName::new("surface.raised").unwrap()),
            ..Props::default()
        });
        let err = validate(&wrong_slot, &registry).unwrap_err();
        assert!(
            matches!(
                err.as_slice()[0].violation,
                Violation::TokenKindMismatch { .. }
            ),
            "the same name is a Color, and spacing wants Spacing: {err}"
        );
    }

    /// A slot the schema does **not** declare is checked for existence only.
    ///
    /// This module does not know, and must not guess, which [`TokenKind`] an
    /// invented slot wants: a host can register a painter with slots this
    /// crate has never heard of, and the mismatch — if any — is that
    /// painter's `unknown_slots` report to make, not tree acceptance's.
    ///
    /// This test used to name the slot `shadow` while its own doc called that
    /// "an invented slot name". `shadow` is declared in
    /// [`crate::token::standard_slots`], so the example contradicted the
    /// sentence explaining it, and the test passed only because nothing
    /// consulted the schema at all. It now uses a slot name that really is
    /// invented, which is what it always meant to say.
    #[test]
    fn a_tokens_map_entry_under_an_undeclared_slot_is_checked_for_existence_only() {
        let mut vocab = Vocabulary::new();
        vocab.declare(DesignToken::new(
            TokenName::new("shape.corner-lg").unwrap(),
            TokenKind::Shape,
        ));
        let node = ViewNode::new(NodeKind::Text, "t").with_props(Props {
            tokens: [(
                "aurora-wash".to_owned(),
                TokenName::new("shape.corner-lg").unwrap(),
            )]
            .into_iter()
            .collect(),
            ..Props::default()
        });
        assert!(
            standard_slots().get("aurora-wash").is_none(),
            "the fixture slot must be one the shipped schema does not declare"
        );
        assert!(
            validate(&node, &Registry::with_vocabulary(vocab)).is_ok(),
            "a declared token under an unrecognised slot name is still a \
             real, declared token"
        );
    }

    /// A slot the schema **does** declare is checked against its declared
    /// kind, and this is the hole that shipped.
    ///
    /// A `Spacing` token bound to the `background` colour slot was accepted,
    /// `semantic::audit` reported nothing, and the marker painted no colour.
    /// For a status indicator that is the inverse of the incident FR-015
    /// exists to prevent: not colour carrying meaning alone, but the colour
    /// channel silently going missing while the shape and word channels stay.
    #[test]
    fn a_spacing_token_in_the_background_colour_slot_is_refused() {
        let node = ViewNode::new(NodeKind::Text, "t").with_props(Props {
            tokens: [(
                "background".to_owned(),
                TokenName::new("spacing.md").unwrap(),
            )]
            .into_iter()
            .collect(),
            ..Props::default()
        });
        let err = validate(&node, &Registry::with_vocabulary(standard_vocabulary()))
            .expect_err("a Spacing token in a Color slot is not a paintable binding");
        assert!(
            err.as_slice()
                .iter()
                .any(|e| matches!(e.violation, Violation::TokenKindMismatch { .. })),
            "expected a kind mismatch on tokens.background, got: {err}"
        );
    }

    /// Turning the schema off turns the check off, so a host with its own
    /// painter is never fought by a schema written for a different one.
    #[test]
    fn an_empty_slot_schema_accepts_any_kind_in_any_slot() {
        let node = ViewNode::new(NodeKind::Text, "t").with_props(Props {
            tokens: [(
                "background".to_owned(),
                TokenName::new("spacing.md").unwrap(),
            )]
            .into_iter()
            .collect(),
            ..Props::default()
        });
        let registry =
            Registry::with_vocabulary(standard_vocabulary()).with_slots(SlotSchema::new());
        assert!(validate(&node, &registry).is_ok());
    }

    /// The other half of the same coin: a `tokens` map entry naming
    /// something the vocabulary does not declare at all is refused, exactly
    /// like every other styling prop.
    #[test]
    fn an_unknown_tokens_map_entry_is_refused() {
        let vocab = standard_vocabulary();
        let node = ViewNode::new(NodeKind::Text, "t").with_props(Props {
            tokens: [(
                "background".to_owned(),
                TokenName::new("surface.invented").unwrap(),
            )]
            .into_iter()
            .collect(),
            ..Props::default()
        });
        let err = validate(&node, &Registry::with_vocabulary(vocab.clone())).unwrap_err();
        assert_eq!(err.len(), 1, "{err}");
        assert_eq!(
            err.as_slice()[0].violation,
            Violation::UnknownTokenRef {
                prop: "tokens.background".to_owned(),
                name: TokenName::new("surface.invented").unwrap(),
                legal: vocab.names().cloned().collect(),
            }
        );
    }

    /// C6/C5's totality guarantee, proved rather than left as a comment: for
    /// any tree `validate` accepts, every token name it declared resolves
    /// under a theme built complete against the same vocabulary
    /// (`token::Theme::build`). `validate` refuses every name the vocabulary
    /// does not declare; `Theme::build` refuses a theme that fails to assign
    /// a value to any name the vocabulary *does* declare. The two proofs
    /// meet in the middle, and this test is the meeting point: nothing an
    /// accepted tree references is left unresolved, so `resolve_spacing`'s
    /// `panic!` (`tree::props`) is never reached for a tree that passed
    /// here.
    #[test]
    fn resolution_is_total_for_an_accepted_tree() {
        let vocab = standard_vocabulary();
        let theme = crate::token::Theme::build(
            crate::token::ThemeMode::Light,
            &vocab,
            light().values().clone(),
        )
        .expect("the shipped light theme is complete against the shipped vocabulary");

        let tree = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                spacing: Some(TokenName::new("spacing.md").unwrap()),
                padding: Some(InsetRefs::all(TokenName::new("spacing.sm").unwrap())),
                tokens: [
                    (
                        "background".to_owned(),
                        TokenName::new("surface.base").unwrap(),
                    ),
                    (
                        "radius".to_owned(),
                        TokenName::new("shape.corner-md").unwrap(),
                    ),
                ]
                .into_iter()
                .collect(),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Text, "t").with_props(Props {
                style: Some(TokenName::new("typography.body").unwrap()),
                ..Props::default()
            }));

        assert!(validate(&tree, &Registry::with_vocabulary(vocab)).is_ok());

        for name in [
            "spacing.md",
            "spacing.sm",
            "surface.base",
            "shape.corner-md",
            "typography.body",
        ] {
            assert!(
                theme.value(&TokenName::new(name).unwrap()).is_some(),
                "{name} must resolve in a theme complete against the \
                 vocabulary that accepted it"
            );
        }
    }

    /// `vocabulary_mut` lets a `Registry` built once declare tokens
    /// afterward, the same way `register_custom_kind` already does for
    /// custom kinds — the accessor a host actually has after construction is
    /// `Registry::vocabulary_mut`, not a rebuild through `with_vocabulary`.
    #[test]
    fn vocabulary_mut_lets_a_registry_declare_tokens_after_construction() {
        let mut registry = Registry::new();
        registry.vocabulary_mut().declare(DesignToken::new(
            TokenName::new("spacing.md").unwrap(),
            TokenKind::Spacing,
        ));
        let tree = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            spacing: Some(TokenName::new("spacing.md").unwrap()),
            ..Props::default()
        });
        assert!(validate(&tree, &registry).is_ok());
    }

    /// FR-056: a refused reference never falls back to a default. The other
    /// half of FR-056 — a refused reference never renders — is a type-level
    /// guarantee this test cannot exercise dynamically: [`super::ValidatedTree`]
    /// (what `frame::petrify` requires) has no public constructor other than
    /// this function's `Ok` arm, so a tree this test refuses cannot reach
    /// `petrify` at all; there is no runtime path left to call. What remains
    /// to prove is refusal itself — that an unknown name is not silently
    /// swapped for [`crate::tree::props::DEFAULT_SPACING`] or any other
    /// default.
    #[test]
    fn a_refused_token_reference_never_defaults() {
        let tree = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            spacing: Some(TokenName::new("spacing.nonexistent").unwrap()),
            ..Props::default()
        });
        assert!(
            validate(&tree, &Registry::new()).is_err(),
            "an unknown name must refuse the tree, not resolve to a default"
        );
    }

    // -- Anchored surfaces: the two refusals that make resolution total. ----

    /// A `surface` on `key` anchored to `target`.
    fn popover(key: &str, target: &str) -> ViewNode {
        ViewNode::new(NodeKind::Surface, key).with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(anchored_at(target)),
            ..Props::default()
        })
    }

    /// An anchor that names no node is refused, never resolved as
    /// `Anchor::Viewport` (`contracts/anchored-placement.md` §2a).
    #[test]
    fn an_anchor_naming_no_node_is_refused_rather_than_centred() {
        let tree = stack("root")
            .child(ViewNode::new(NodeKind::Text, "button"))
            .child(popover("popup", "/root/buton"));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        let violation = &err.as_slice()[0].violation;
        let Violation::AnchorTargetMissing { id, known } = violation else {
            panic!("wrong violation: {violation:?}");
        };
        assert_eq!(id, "/root/buton");
        assert!(
            known.contains(&"/root/button".to_owned()),
            "the refusal must name the ids that do exist, so a typo is \
             findable: {known:?}"
        );
        assert_eq!(err.as_slice()[0].path, "/root/popup");
    }

    // -- Anchor::Sibling: a bare key, resolved against the surface's own parent. --

    /// A `bottom`-edge sibling anchor on `key`, centred with no offset: what
    /// every component constructor in `crate::component` builds.
    fn beside(key: &str) -> Anchor {
        Anchor::Sibling {
            key: key.into(),
            edge: Edge::Bottom,
            align: crate::tree::Align::Center,
            offset: None,
        }
    }

    fn sibling_popover(key: &str, target: &str) -> ViewNode {
        ViewNode::new(NodeKind::Surface, key).with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(beside(target)),
            ..Props::default()
        })
    }

    /// The pair a constructor builds — a trigger and a popover side by side
    /// — wrapped in `depth` extra containers. Mount point is the caller's
    /// business; the pair never changes.
    fn pair_at_depth(depth: usize) -> ViewNode {
        let mut node = stack("host")
            .child(ViewNode::new(NodeKind::Text, "trigger"))
            .child(sibling_popover("popup", "trigger"));
        for level in 0..depth {
            node = stack(&format!("level{level}")).child(node);
        }
        stack("root").child(node)
    }

    /// The defect this variant exists for: the same pair is accepted at the
    /// root and three containers down, because a sibling key is resolved
    /// against the surface's own parent path rather than compared to the
    /// canonical id verbatim.
    #[test]
    fn a_sibling_anchor_is_accepted_at_every_depth() {
        for depth in 0..4 {
            let tree = pair_at_depth(depth);
            assert!(
                validate(&tree, &Registry::new()).is_ok(),
                "depth {depth}: {:?}",
                validate(&tree, &Registry::new()).err()
            );
        }
    }

    /// A key no sibling carries is refused as `AnchorTargetMissing`, and the
    /// refusal names the one canonical id that was tried — the surface's
    /// parent path plus the key — so an author sees which child list the
    /// trigger was missing from.
    #[test]
    fn a_sibling_anchor_naming_no_sibling_is_refused_with_the_id_it_tried() {
        let tree = stack("root").child(
            stack("host")
                .child(ViewNode::new(NodeKind::Text, "trigger"))
                .child(sibling_popover("popup", "triger")),
        );
        let err = validate(&tree, &Registry::new()).unwrap_err();
        let violation = &err.as_slice()[0].violation;
        let Violation::AnchorTargetMissing { id, known } = violation else {
            panic!("wrong violation: {violation:?}");
        };
        assert_eq!(id, "/root/host/triger");
        assert!(
            known.contains(&"/root/host/trigger".to_owned()),
            "{known:?}"
        );
        assert_eq!(err.as_slice()[0].path, "/root/host/popup");
    }

    /// A sibling key reaches the surface's own child list and nothing else:
    /// a `trigger` one level up, or one level down, is not a sibling, and
    /// the tree is refused rather than resolved to the nearest match. This
    /// is what makes two components at different depths that both name
    /// `"trigger"` unable to reach each other's.
    #[test]
    fn a_sibling_anchor_never_resolves_to_a_cousin() {
        let above = stack("root")
            .child(ViewNode::new(NodeKind::Text, "trigger"))
            .child(stack("host").child(sibling_popover("popup", "trigger")));
        let err = validate(&above, &Registry::new()).unwrap_err();
        assert!(
            matches!(
                &err.as_slice()[0].violation,
                Violation::AnchorTargetMissing { id, .. } if id == "/root/host/trigger"
            ),
            "{err}"
        );

        let below = stack("root").child(
            stack("host")
                .child(stack("inner").child(ViewNode::new(NodeKind::Text, "trigger")))
                .child(sibling_popover("popup", "trigger")),
        );
        let err = validate(&below, &Registry::new()).unwrap_err();
        assert!(
            matches!(
                &err.as_slice()[0].violation,
                Violation::AnchorTargetMissing { id, .. } if id == "/root/host/trigger"
            ),
            "{err}"
        );
    }

    /// Two pairs that both key their trigger `"trigger"` coexist in one
    /// tree: each popover resolves to the trigger beside it, and nothing
    /// else, so the tree is accepted with no ambiguity to detect.
    #[test]
    fn two_sibling_anchors_sharing_a_key_at_different_depths_are_both_accepted() {
        let tree = stack("root")
            .child(
                stack("a")
                    .child(ViewNode::new(NodeKind::Text, "trigger"))
                    .child(sibling_popover("popup", "trigger")),
            )
            .child(
                stack("b").child(
                    stack("deeper")
                        .child(ViewNode::new(NodeKind::Text, "trigger"))
                        .child(sibling_popover("popup", "trigger")),
                ),
            );
        assert!(validate(&tree, &Registry::new()).is_ok());
    }

    /// A sibling anchor is one end of a dependency edge exactly as a node
    /// anchor is: a cycle spelled through a sibling key is still a cycle.
    /// Here a popover names its sibling `b`, and `b` is a surface anchored
    /// into the first popover's subtree.
    #[test]
    fn a_cycle_through_a_sibling_anchor_is_refused() {
        let tree = stack("root")
            .child(sibling_popover("a", "b").child(ViewNode::new(NodeKind::Text, "item")))
            .child(popover("b", "/root/a/item"));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert!(
            err.as_slice()
                .iter()
                .any(|e| matches!(e.violation, Violation::AnchorCycle { .. })),
            "{err}"
        );
    }

    /// The root has no siblings. A sibling anchor on the top-level node
    /// resolves to `/key`, which is the root itself only when the key
    /// matches — and that is a self-anchor, refused as a cycle — and
    /// otherwise names nothing.
    #[test]
    fn a_sibling_anchor_on_the_root_names_nothing_usable() {
        let missing = sibling_popover("root", "other");
        let err = validate(&missing, &Registry::new()).unwrap_err();
        assert!(
            matches!(
                &err.as_slice()[0].violation,
                Violation::AnchorTargetMissing { id, .. } if id == "/other"
            ),
            "{err}"
        );
        let itself = sibling_popover("root", "root");
        let err = validate(&itself, &Registry::new()).unwrap_err();
        assert!(
            matches!(&err.as_slice()[0].violation, Violation::AnchorCycle { .. }),
            "{err}"
        );
    }

    /// `anchor.offset` on a sibling anchor is a spacing token checked the
    /// same way a node anchor's is.
    #[test]
    fn a_sibling_anchor_offset_naming_no_token_is_refused() {
        let tree = stack("root")
            .child(ViewNode::new(NodeKind::Text, "button"))
            .child(ViewNode::new(NodeKind::Surface, "popup").with_props(Props {
                layer: Some(Layer::Popup),
                anchor: Some(Anchor::Sibling {
                    key: "button".into(),
                    edge: Edge::Bottom,
                    align: crate::tree::Align::Center,
                    offset: Some(gap_token(9.0)),
                }),
                ..Props::default()
            }));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert!(
            err.as_slice().iter().any(|e| matches!(
                &e.violation,
                Violation::UnknownTokenRef { prop, .. } if prop == "anchor.offset"
            )),
            "{err}"
        );
        assert!(validate(&tree, &spacing_registry_with(9.0)).is_ok());
    }

    /// A surface anchored to its own rect is circular: its placement is what
    /// decides the rect its placement is resolved against.
    #[test]
    fn a_surface_anchored_to_itself_is_refused() {
        let tree = stack("root").child(popover("popup", "/root/popup"));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        let violation = &err.as_slice()[0].violation;
        let Violation::AnchorCycle { through } = violation else {
            panic!("wrong violation: {violation:?}");
        };
        assert_eq!(through, &["/root/popup", "/root/popup"]);
    }

    /// The mutual case: each surface is anchored to a node inside the other.
    ///
    /// Both are named, because both are unplaceable and an author fixing one
    /// end wants to see the other.
    #[test]
    fn two_surfaces_anchored_into_each_others_subtrees_are_refused() {
        let tree = stack("root")
            .child(popover("a", "/root/b/inner").child(ViewNode::new(NodeKind::Text, "inner")))
            .child(popover("b", "/root/a/inner").child(ViewNode::new(NodeKind::Text, "inner")));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        let cycles: Vec<&TreeError> = err
            .as_slice()
            .iter()
            .filter(|e| matches!(e.violation, Violation::AnchorCycle { .. }))
            .collect();
        assert_eq!(cycles.len(), 2, "both ends are refused: {err}");
        let Violation::AnchorCycle { through } = &cycles[0].violation else {
            unreachable!("filtered above")
        };
        assert_eq!(through, &["/root/a", "/root/b", "/root/a"]);
    }

    /// One-way nesting is *not* a cycle and is not refused: it is the case
    /// `contracts/anchored-placement.md` §1.4 spends an extra harvest walk
    /// on. Refusing it here would make the depth rule dead code.
    #[test]
    fn a_surface_anchored_into_another_anchored_surface_is_accepted() {
        let tree = stack("root")
            .child(ViewNode::new(NodeKind::Text, "button"))
            .child(popover("a", "/root/button").child(ViewNode::new(NodeKind::Text, "item")))
            .child(popover("b", "/root/a/item"));
        assert!(validate(&tree, &Registry::new()).is_ok());
    }

    /// A surface inside a non-anchored surface contributes no dependency
    /// either: an `Anchor::Viewport` surface is placed without reference to
    /// any other node, so nothing about it can loop.
    #[test]
    fn an_anchor_into_a_viewport_anchored_surface_is_accepted() {
        let modal = ViewNode::new(NodeKind::Surface, "modal")
            .with_props(Props {
                layer: Some(Layer::Modal),
                anchor: Some(Anchor::Viewport),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Text, "field"))
            .child(popover("hint", "/root/modal/field"));
        let tree = stack("root").child(modal);
        assert!(validate(&tree, &Registry::new()).is_ok());
    }

    /// `anchor.offset` is a spacing token and is checked like every other
    /// one: a name the vocabulary does not declare is refused.
    #[test]
    fn an_anchor_offset_naming_no_token_is_refused() {
        let tree = stack("root")
            .child(ViewNode::new(NodeKind::Text, "button"))
            .child(ViewNode::new(NodeKind::Surface, "popup").with_props(Props {
                layer: Some(Layer::Popup),
                anchor: Some(Anchor::Node {
                    id: "/root/button".into(),
                    edge: Edge::Bottom,
                    align: crate::tree::Align::Center,
                    offset: Some(gap_token(9.0)),
                }),
                ..Props::default()
            }));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert!(
            err.as_slice().iter().any(|e| matches!(
                &e.violation,
                Violation::UnknownTokenRef { prop, .. } if prop == "anchor.offset"
            )),
            "{err}"
        );
        // The same tree against a vocabulary that declares the gap accepts.
        assert!(validate(&tree, &spacing_registry_with(9.0)).is_ok());
    }

    /// As [`spacing_registry`], for one gap named by the caller.
    fn spacing_registry_with(units: f32) -> Registry {
        let mut vocab = Vocabulary::new();
        vocab.declare(DesignToken::new(gap_token(units), TokenKind::Spacing));
        Registry::with_vocabulary(vocab)
    }

    // -- State-decorated token bindings. ------------------------------------

    /// A state suffix the resolver never builds is refused at acceptance.
    ///
    /// This is the one failure mode the decorated-key shape has that a typed
    /// per-state map would not: `resolve_slot` builds its candidates from the
    /// placement's flags, so a key it never builds is not an error at paint
    /// time — it is silence. `background@hovered` would simply never win, and
    /// the author would see a control that does nothing on hover with no
    /// diagnostic anywhere. Acceptance is where that gets closed.
    #[test]
    fn a_state_suffix_outside_the_resolver_chain_is_refused() {
        let tree = ViewNode::new(NodeKind::Text, "t").with_props(Props {
            tokens: [(
                "background@hovered".to_owned(),
                TokenName::new("surface.raised").unwrap(),
            )]
            .into_iter()
            .collect(),
            ..Props::default()
        });
        let err = validate(&tree, &Registry::with_vocabulary(standard_vocabulary())).unwrap_err();
        let violation = &err.as_slice()[0].violation;
        let Violation::UnknownStateName { name, legal } = violation else {
            panic!("wrong violation: {violation:?}");
        };
        assert_eq!(name, "hovered", "the spelling that is right is `hover`");
        assert!(legal.contains(&"hover"), "{legal:?}");
    }

    /// The combination forms Carbon names are legal; the one that is not a
    /// combination is not.
    #[test]
    fn a_named_combination_is_legal_and_selected_enabled_is_not() {
        let registry = Registry::with_vocabulary(standard_vocabulary());
        let with = |key: &str| {
            ViewNode::new(NodeKind::Text, "t").with_props(Props {
                tokens: [(key.to_owned(), TokenName::new("surface.raised").unwrap())]
                    .into_iter()
                    .collect(),
                ..Props::default()
            })
        };

        for key in [
            "background@hover",
            "background@selected",
            "background@selected-hover",
            "background@selected-disabled",
        ] {
            assert!(
                validate(&with(key), &registry).is_ok(),
                "{key} is a candidate `resolve_slot` builds"
            );
        }

        // `Enabled` contributes no suffix — it *is* the undecorated slot — so
        // the selected-and-resting combination is spelled `@selected`, and
        // `@selected-enabled` names nothing the chain will ever look for.
        let err = validate(&with("background@selected-enabled"), &registry).unwrap_err();
        assert!(
            matches!(
                &err.as_slice()[0].violation,
                Violation::UnknownStateName { .. }
            ),
            "{err}"
        );
    }

    /// A decorated binding is checked against the vocabulary and the slot
    /// schema exactly as the base binding is: a colour slot may not take a
    /// spacing token just because it was declared under `@hover`.
    #[test]
    fn a_decorated_binding_is_checked_like_the_base_binding() {
        let registry = Registry::with_vocabulary(standard_vocabulary());
        let with = |token: TokenName| {
            ViewNode::new(NodeKind::Text, "t").with_props(Props {
                tokens: [("background@hover".to_owned(), token)]
                    .into_iter()
                    .collect(),
                ..Props::default()
            })
        };

        let unknown = with(TokenName::new("surface.invented").unwrap());
        let err = validate(&unknown, &registry).unwrap_err();
        assert!(
            matches!(
                &err.as_slice()[0].violation,
                Violation::UnknownTokenRef { prop, .. } if prop == "tokens.background@hover"
            ),
            "{err}"
        );

        let wrong_kind = with(TokenName::new("spacing.md").unwrap());
        let err = validate(&wrong_kind, &registry).unwrap_err();
        assert!(
            matches!(
                &err.as_slice()[0].violation,
                Violation::TokenKindMismatch { prop, expected, .. }
                    if prop == "tokens.background@hover" && *expected == TokenKind::Color
            ),
            "{err}"
        );
    }
}
