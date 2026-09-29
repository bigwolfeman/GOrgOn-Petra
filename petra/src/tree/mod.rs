//! The view tree: plain nested data describing a surface.
//!
//! A tree is authored (in Rust today, in a Lua table later — the wire shape is
//! the same), validated once at acceptance, then negotiated and petrified. No
//! node holds a closure, a piece of state, or a toolkit type.

pub mod binding;
pub mod binding_eval;
pub(crate) mod graft;
pub mod key;
pub mod node;
pub mod props;
pub mod validate;

// Property-value sources (`Lit`/`Bind`/`Derive`) and their fold — the
// retained-tree seam `Host::apply_slot_changes` drives (design §5 of
// `.agents/notes/proposed/architecture/2026-09-27-bound-slot-table-ui-model.md`).
pub use binding::{
    BoundProps, BoundSite, DeriveExpr, DirtyClass, PropKey, PropType, PropVal, ReverseIndex, Row,
    Rows, ShapeFault, SlotChange, SlotKey, SlotValue, result_shape, shape_for,
};
pub use binding_eval::{
    ApplyError, BindTarget, ResolveError, ResolveInputs, ResolvedTree, carries_bindings,
    check_slot_versions,
};
pub use key::{Key, KeyPath};
pub use node::{
    AxisConstraint, Behaviour, ComponentRef, Constraints, FocusFigure, FocusShownOn, Intent,
    Interaction, NodeKind, Phase, ROLE_NAMES, Role, Semantics, TransitionRef, ViewNode,
};
// `Align` here is the anchor alignment `props` declares, not
// `crate::geom::Align`, which is a child's cross-axis placement inside its
// parent. Two vocabularies over one word, kept apart by the module they are
// reached through; `props::Align`'s own doc comment says which is which.
pub use props::{
    Align, Anchor, ClampRule, CollectionProps, Edge, Fit, GridProps, GridSpan, InputPolicy,
    InsetRefs, Justify, Layer, NodeAnchor, Props, ScrollProps, StackProps, SurfaceProps, TextProps,
    TextRun, TextWrap, Tip, TrackSize, resolve_insets, resolve_spacing,
};
// `ANCHOR_PROP_KINDS` / `PADDING_PROP_KINDS` / `TOKEN_PROP_KINDS` and their
// reader `token_prop_refs` are re-exported here because they are read from
// outside this crate: the daemon's stage-1 `ui` acceptance
// (`gorgon/gorgond/src/ui.rs`) walks the same pairing `validate` walks,
// rather than keeping a second copy of it.
pub use validate::{
    ANCHOR_PROP_KINDS, PADDING_PROP_KINDS, Prerequisites, Registry, TOKEN_PROP_KINDS, TreeError,
    TreeErrors, ValidatedTree, Violation, token_prop_refs, validate,
};
