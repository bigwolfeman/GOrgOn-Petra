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
pub use props::{
    Anchor, ClampRule, CollectionProps, Edge, GridProps, InputPolicy, Layer, Props, ScrollProps,
    StackProps, SurfaceProps, TextProps, TextWrap, TrackSize,
};
pub use validate::{Registry, TreeError, TreeErrors, ValidatedTree, Violation, validate};
