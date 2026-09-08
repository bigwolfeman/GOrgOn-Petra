//! Parameter shapes for the component registry.
//!
//! 180 of the component library's public constructors take a key first and
//! then a small, repeating set of arguments: 42 are `(key, label)`, 20 are
//! `(key, label, selected)`, 12 are `(key, children)`. This module names those
//! repeating shapes once so a registry entry can say which one it wants
//! instead of parsing arguments by hand.
//!
//! # Why these are types and not a bag of optional fields
//!
//! A shape is what makes the generated Luau stub typed. [`ParamShape::LUAU`]
//! is the signature `gorgon/kernel-lua/plugin.d.luau` renders for every
//! constructor bound to that shape, so an agent calling `ui.tag { key = 1 }`
//! gets a type error at the call site rather than a refusal at mount. A struct
//! with every field optional could not produce that signature, and
//! `deny_unknown_fields` on a named shape is what turns a misspelled parameter
//! into a precise message naming the parameter.

use serde::Deserialize;

use crate::tree::{Key, ViewNode};

/// A parameter shape: how it deserializes, and how it reads in Luau.
pub trait ParamShape: for<'de> Deserialize<'de> {
    /// The Luau table type this shape renders as in `plugin.d.luau`.
    const LUAU: &'static str;
}

/// What went wrong turning a wire `params` table into a shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamError {
    /// The constructor that was asked for.
    pub component: String,
    /// The operator-facing reason, naming the parameter where it can.
    pub reason: String,
}

macro_rules! shapes {
    ($($(#[$m:meta])* $name:ident { $($(#[$f:meta])* $field:ident : $ty:ty),* $(,)? } => $luau:literal;)*) => {
        $(
            $(#[$m])*
            #[derive(Debug, Clone, Deserialize)]
            #[serde(deny_unknown_fields)]
            pub struct $name {
                $($(#[$f])* pub $field: $ty,)*
            }
            impl ParamShape for $name {
                const LUAU: &'static str = $luau;
            }
        )*
    };
}

shapes! {
    /// 42 constructors. The dominant shape.
    KeyLabel {
        /// This node's key, unique among its siblings.
        key: Key,
        /// The visible text.
        label: String,
    }
        => "{ key: string, label: string }";
    /// 20 constructors.
    KeyLabelSelected {
        /// This node's key, unique among its siblings.
        key: Key,
        /// The visible text.
        label: String,
        /// Whether this node reads as selected.
        selected: bool,
    }
        => "{ key: string, label: string, selected: boolean }";
    /// 12 constructors.
    KeyChildren {
        /// This node's key, unique among its siblings.
        key: Key,
        /// Child nodes, in paint order.
        ///
        /// `#[serde(default)]` because the Lua factory omits an empty table:
        /// Lua cannot distinguish an empty list from an empty map, so sending
        /// one would arrive as `{}` and fail to deserialize.
        #[serde(default)]
        children: Vec<ViewNode>,
    }
        => "{ key: string, children: { ViewNode } }";
    /// 7 constructors.
    KeyChildrenSelected {
        /// This node's key, unique among its siblings.
        key: Key,
        /// Child nodes, in paint order.
        ///
        /// `#[serde(default)]` because the Lua factory omits an empty table:
        /// Lua cannot distinguish an empty list from an empty map, so sending
        /// one would arrive as `{}` and fail to deserialize.
        #[serde(default)]
        children: Vec<ViewNode>,
        /// Whether this node reads as selected.
        selected: bool,
    }
        => "{ key: string, children: { ViewNode }, selected: boolean }";
    /// 19 constructors.
    KeyLabelValue {
        /// This node's key, unique among its siblings.
        key: Key,
        /// The visible text.
        label: String,
        /// The node's current value, as text.
        value: String,
    }
        => "{ key: string, label: string, value: string }";
    /// 15 constructors.
    KeyLabelSelectedValue {
        /// This node's key, unique among its siblings.
        key: Key,
        /// The visible text.
        label: String,
        /// Whether this node reads as selected.
        selected: bool,
        /// The node's current value, as text.
        value: String,
    }
        => "{ key: string, label: string, selected: boolean, value: string }";
    /// 7 constructors.
    KeyLabelChildren {
        /// This node's key, unique among its siblings.
        key: Key,
        /// The visible text.
        label: String,
        /// Child nodes, in paint order.
        ///
        /// `#[serde(default)]` because the Lua factory omits an empty table:
        /// Lua cannot distinguish an empty list from an empty map, so sending
        /// one would arrive as `{}` and fail to deserialize.
        #[serde(default)]
        children: Vec<ViewNode>,
    }
        => "{ key: string, label: string, children: { ViewNode } }";
    /// 5 constructors.
    KeyLabelSelectedChildren {
        /// This node's key, unique among its siblings.
        key: Key,
        /// The visible text.
        label: String,
        /// Whether this node reads as selected.
        selected: bool,
        /// Child nodes, in paint order.
        ///
        /// `#[serde(default)]` because the Lua factory omits an empty table:
        /// Lua cannot distinguish an empty list from an empty map, so sending
        /// one would arrive as `{}` and fail to deserialize.
        #[serde(default)]
        children: Vec<ViewNode>,
    }
        => "{ key: string, label: string, selected: boolean, children: { ViewNode } }";
    /// 4 constructors. `f32` in Rust; Luau has one number type.
    KeyLabelNumber {
        /// This node's key, unique among its siblings.
        key: Key,
        /// The visible text.
        label: String,
        /// The node's current value, as a number.
        value: f32,
    }
        => "{ key: string, label: string, value: number }";
    /// 2 constructors.
    KeyOnly {
        /// This node's key, unique among its siblings.
        key: Key,
    }
        => "{ key: string }";
}
