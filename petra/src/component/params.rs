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

use crate::tree::{Intent, Key, Phase, ViewNode};

/// A parameter shape: how it deserializes, and how it reads in Luau.
pub trait ParamShape: for<'de> Deserialize<'de> {
    /// The Luau table type this shape renders as in `plugin.d.luau`.
    const LUAU: &'static str;
}

/// Spec 010 FR-015: [`Intent`] and [`Phase`] render as Luau string-literal
/// unions, the same mechanism [`ParamShape::LUAU`] gives every table shape
/// above, so a plugin author gets a type error at the call site for
/// `"nagivate"` instead of a silent no-op at mount.
///
/// Neither is a *table* shape — `ParamShape::LUAU` calls that "the Luau
/// table type this shape renders as", and a string-literal union is not a
/// table — but the trait itself asks only for a rendered Luau type string
/// and a `Deserialize` impl, both of which `Intent` and `Phase` already
/// have (`tree::node`'s `#[serde(rename_all = "kebab-case")]`). Neither is
/// registered as a component constructor's own top-level parameter shape —
/// `crate::component::registry::entries` walks those, not this trait's
/// every implementor — so `xtask::ui_stubs`'s `every_shape_is_a_table_type`
/// never sees either impl and never needs to. What DOES need to see them is
/// whatever renders a `behaviour: Behaviour` parameter's Luau type once a
/// component constructor exposes one: a table type built from
/// `Intent::LUAU` and `Phase::LUAU` (`{ intent: <Intent::LUAU>, phase:
/// <Phase::LUAU> }`) rather than a second, hand-copied pair of string lists
/// that the vocabulary in `tree::node` can drift out from under.
impl ParamShape for Intent {
    const LUAU: &'static str = "\"activate\" | \"toggle\" | \"select\" | \"adjust\"";
}

impl ParamShape for Phase {
    const LUAU: &'static str = "\"on-press\" | \"on-change\" | \"on-release\"";
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

#[cfg(test)]
mod tests {
    use super::ParamShape;
    use crate::tree::{Intent, Phase};

    /// FR-015: a misspelled `Intent`/`Phase` name must be a Luau type
    /// error at the call site, not a silent no-op at mount — so
    /// `ParamShape::LUAU` must render both as a string-literal union naming
    /// every wire variant, never as the unchecked `string` a closed
    /// vocabulary has no business widening to.
    ///
    /// The expected union is built from `serde_json`'s own rendering of
    /// each variant, in declaration order, rather than a second hand-typed
    /// copy of the four/three wire strings: this is the test that would
    /// catch `Intent::LUAU` or `Phase::LUAU` drifting from
    /// `tree::node`'s `#[serde(rename_all = "kebab-case")]` vocabulary,
    /// which a second hand-typed literal could not.
    #[test]
    fn intent_and_phase_render_as_string_literal_unions() {
        let wire = |json: &str| json.to_owned();
        let intents = [
            Intent::Activate,
            Intent::Toggle,
            Intent::Select,
            Intent::Adjust,
        ];
        let expected_intent = intents
            .iter()
            .map(|i| wire(&serde_json::to_string(i).unwrap()))
            .collect::<Vec<_>>()
            .join(" | ");
        assert_eq!(<Intent as ParamShape>::LUAU, expected_intent);
        assert_eq!(
            <Intent as ParamShape>::LUAU,
            "\"activate\" | \"toggle\" | \"select\" | \"adjust\""
        );

        let phases = [Phase::OnPress, Phase::OnChange, Phase::OnRelease];
        let expected_phase = phases
            .iter()
            .map(|p| wire(&serde_json::to_string(p).unwrap()))
            .collect::<Vec<_>>()
            .join(" | ");
        assert_eq!(<Phase as ParamShape>::LUAU, expected_phase);
        assert_eq!(
            <Phase as ParamShape>::LUAU,
            "\"on-press\" | \"on-change\" | \"on-release\""
        );
    }
}
