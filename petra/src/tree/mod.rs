//! The view tree: plain nested data describing a surface.
//!
//! A tree is authored (in Rust today, in a Lua table later — the wire shape is
//! the same), validated once at acceptance, then negotiated and petrified. No
//! node holds a closure, a piece of state, or a toolkit type.

pub mod key;
pub mod node;
pub mod props;
pub mod validate;

pub use key::{Key, KeyPath};
pub use node::{
    AxisConstraint, Constraints, Interaction, NodeKind, ROLE_NAMES, Role, Semantics, TransitionRef,
    ViewNode,
};
// `Align` here is the anchor alignment `props` declares, not
// `crate::geom::Align`, which is a child's cross-axis placement inside its
// parent. Two vocabularies over one word, kept apart by the module they are
// reached through; `props::Align`'s own doc comment says which is which.
pub use props::{
    Align, Anchor, ClampRule, CollectionProps, Edge, GridProps, GridSpan, InputPolicy, InsetRefs,
    Layer, Props, ScrollProps, StackProps, SurfaceProps, TextProps, TextWrap, TrackSize,
    resolve_insets, resolve_spacing,
};
pub use validate::{Registry, TreeError, TreeErrors, ValidatedTree, Violation, validate};
