//! Registry rows: wave-1 atomics that are not in the original 42.
//!
//! Textarea, avatar, rating, toggle-button, input-group, and OTP live in
//! this crate but not in a `GROUPS` file. Those five files scan sibling
//! sources of the original 42 with `include_str`; these constructors are
//! not those files, and a second `include_str` group cannot see compound
//! constructors in `gorgon-petra-compound` either.
//! [`super::register_external`] is the hook; this crate pushes this slice
//! through it before the first lookup.
//!
//! Tag extras (`tag_with_avatar`, `tag_status`) and `code_snippet_multi_capped`
//! stay in `feedback.rs` / `containment.rs` so those groups' source-scan
//! coverage tests still match the files they already own.
//!
//! Every `pub fn ... -> ViewNode` in `textarea.rs`, `avatar.rs`,
//! `rating.rs`, `toggle_button.rs`, `input_group.rs`, and `otp.rs` has
//! exactly one row below — `every_view_node_constructor_has_exactly_one_row`
//! proves it by scanning those six files' own source text.

use serde::Deserialize;

use super::Entry;
use crate::component as lib;
use crate::component::params::{
    KeyChildren, KeyLabel, KeyLabelMessage, KeyLabelOptionalMessage, ParamShape,
};
use crate::component::registry::IconMarkParam;
use crate::token::StatusToken;
use crate::tree::{Key, ViewNode};

macro_rules! status_token_luau {
    () => {
        "{ name: string, shape: \"circle\" | \"triangle\" | \"square\" | \"diamond\" | \
         \"octagon\", text: string }"
    };
}

// ---------------------------------------------------------------------
// One-off shapes. Field names match the Rust constructors.
// ---------------------------------------------------------------------

/// `avatar` / `avatar_xs` / `avatar_md` / `avatar_lg`(key, initials).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyInitials {
    key: Key,
    initials: String,
}
impl ParamShape for KeyInitials {
    const LUAU: &'static str = "{ key: string, initials: string }";
}

/// `avatar_with_image(key, initials, src)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyInitialsSrc {
    key: Key,
    initials: String,
    src: String,
}
impl ParamShape for KeyInitialsSrc {
    const LUAU: &'static str = "{ key: string, initials: string, src: string }";
}

/// `avatar_with_status(key, initials, status)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyInitialsStatus {
    key: Key,
    initials: String,
    status: StatusToken,
}
impl ParamShape for KeyInitialsStatus {
    const LUAU: &'static str = concat!(
        "{ key: string, initials: string, status: ",
        status_token_luau!(),
        " }"
    );
}

/// `avatar_with(key, initials, image?, status?)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyInitialsOptionalImageStatus {
    key: Key,
    initials: String,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    status: Option<StatusToken>,
}
impl ParamShape for KeyInitialsOptionalImageStatus {
    const LUAU: &'static str = concat!(
        "{ key: string, initials: string, image: string?, status: ",
        status_token_luau!(),
        "? }"
    );
}

/// `rating(key, value, max, mark)`. `value` and `max` are `f32` on the
/// wire because Luau has one number type (same reason `KeyLabelNumber`
/// is `f32`, not `u32`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingParams {
    key: Key,
    value: f32,
    max: f32,
    mark: IconMarkParam,
}
impl ParamShape for RatingParams {
    const LUAU: &'static str = concat!(
        "{ key: string, value: number, max: number, mark: ",
        crate::icon_mark_luau!(),
        " }"
    );
}

/// `toggle_button(key, label, pressed)`. `pressed`, not `selected`: the
/// constructor's own argument name.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelPressed {
    key: Key,
    label: String,
    pressed: bool,
}
impl ParamShape for KeyLabelPressed {
    const LUAU: &'static str = "{ key: string, label: string, pressed: boolean }";
}

/// `toggle_button_icon(key, label, pressed, mark)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelPressedMark {
    key: Key,
    label: String,
    pressed: bool,
    mark: IconMarkParam,
}
impl ParamShape for KeyLabelPressedMark {
    const LUAU: &'static str = concat!(
        "{ key: string, label: string, pressed: boolean, mark: ",
        crate::icon_mark_luau!(),
        " }"
    );
}

/// `avatar_group(key, avatars, overflow)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AvatarGroupParams {
    key: Key,
    // An empty Lua table crosses as `{}`, a JSON object, because Lua cannot
    // tell an empty list from an empty map; `#[serde(default)]` supplies the
    // empty `Vec` when the field is missing.
    #[serde(default)]
    avatars: Vec<ViewNode>,
    overflow: u32,
}
impl ParamShape for AvatarGroupParams {
    const LUAU: &'static str = "{ key: string, avatars: { ViewNode }, overflow: number }";
}

/// `input_group_with_addon(key, addon, field)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyAddonField {
    key: Key,
    addon: ViewNode,
    field: ViewNode,
}
impl ParamShape for KeyAddonField {
    const LUAU: &'static str = "{ key: string, addon: ViewNode, field: ViewNode }";
}

/// `otp(key, length, value)`. `length` is `u32` on the wire; Luau has one
/// number type, same as pagination's page counts.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OtpParams {
    key: Key,
    length: u32,
    value: String,
}
impl ParamShape for OtpParams {
    const LUAU: &'static str = "{ key: string, length: number, value: string }";
}

// ---------------------------------------------------------------------
// This group's constructors.
// ---------------------------------------------------------------------
pub const ENTRIES: &[Entry] = &[
    row!("textarea", KeyLabel, |p| lib::textarea(p.key, p.label)),
    row!("textarea_invalid", KeyLabelMessage, |p| {
        lib::textarea_invalid(p.key, p.label, p.message)
    }),
    row!("textarea_warning", KeyLabelMessage, |p| {
        lib::textarea_warning(p.key, p.label, p.message)
    }),
    row!("textarea_validated", KeyLabelOptionalMessage, |p| {
        lib::textarea_validated(p.key, p.label, p.message)
    }),
    row!("avatar", KeyInitials, |p| lib::avatar(p.key, p.initials)),
    row!("avatar_xs", KeyInitials, |p| lib::avatar_xs(
        p.key, p.initials
    )),
    row!("avatar_md", KeyInitials, |p| lib::avatar_md(
        p.key, p.initials
    )),
    row!("avatar_lg", KeyInitials, |p| lib::avatar_lg(
        p.key, p.initials
    )),
    row!("avatar_with_image", KeyInitialsSrc, |p| {
        lib::avatar_with_image(p.key, p.initials, p.src)
    }),
    row!("avatar_with_status", KeyInitialsStatus, |p| {
        lib::avatar_with_status(p.key, p.initials, &p.status)
    }),
    row!("avatar_with", KeyInitialsOptionalImageStatus, |p| {
        lib::avatar_with(p.key, p.initials, p.image.as_deref(), p.status.as_ref())
    }),
    row!("avatar_group", AvatarGroupParams, |p| lib::avatar_group(
        p.key, p.avatars, p.overflow
    )),
    row!("rating", RatingParams, |p| lib::rating(
        p.key,
        p.value.max(0.0) as u32,
        p.max.max(0.0) as u32,
        p.mark.into()
    )),
    row!("toggle_button", KeyLabelPressed, |p| lib::toggle_button(
        p.key, p.label, p.pressed
    )),
    row!("toggle_button_icon", KeyLabelPressedMark, |p| {
        lib::toggle_button_icon(p.key, p.label, p.pressed, p.mark.into())
    }),
    row!("toggle_button_group", KeyChildren, |p| {
        lib::toggle_button_group(p.key, p.children)
    }),
    row!("input_group", KeyChildren, |p| lib::input_group(
        p.key, p.children
    )),
    row!("input_group_with_addon", KeyAddonField, |p| {
        lib::input_group_with_addon(p.key, p.addon, p.field)
    }),
    row!("input_group_seamless", KeyChildren, |p| {
        lib::input_group_seamless(p.key, p.children)
    }),
    row!("input_group_with_addon_seamless", KeyAddonField, |p| {
        lib::input_group_with_addon_seamless(p.key, p.addon, p.field)
    }),
    row!("otp", OtpParams, |p| lib::otp(p.key, p.length, &p.value)),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::ENTRIES;

    /// Every `pub fn ... -> ViewNode` in the six files this group owns has
    /// exactly one row, found by scanning the files' own text rather than
    /// trusting a hand-written list — a constructor added later and never
    /// registered fails this the moment it lands, not at review time.
    ///
    /// The heuristic matches `registry/form.rs`: a `pub fn` declaration in
    /// these files never has a `{` in its signature before the one that
    /// opens its own body, so the text between `pub fn ` and the next `{`
    /// is the whole signature, and it names a `ViewNode` constructor
    /// exactly when that text contains `-> ViewNode`.
    fn public_view_node_constructors(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = source;
        while let Some(at) = rest.find("pub fn ") {
            rest = &rest[at + "pub fn ".len()..];
            let Some(open) = rest.find('(') else { break };
            let name = rest[..open].trim().to_owned();
            let Some(brace) = rest.find('{') else { break };
            let signature = &rest[..brace];
            if signature.contains("-> ViewNode") {
                out.push(name);
            }
            rest = &rest[brace..];
        }
        out
    }

    #[test]
    fn every_view_node_constructor_has_exactly_one_row() {
        let sources: &[(&str, &str)] = &[
            ("textarea.rs", include_str!("../textarea.rs")),
            ("avatar.rs", include_str!("../avatar.rs")),
            ("rating.rs", include_str!("../rating.rs")),
            ("toggle_button.rs", include_str!("../toggle_button.rs")),
            ("input_group.rs", include_str!("../input_group.rs")),
            ("otp.rs", include_str!("../otp.rs")),
        ];
        let registered: BTreeSet<&str> = ENTRIES.iter().map(|e| e.name).collect();
        assert_eq!(
            registered.len(),
            ENTRIES.len(),
            "two rows in this file share one name"
        );
        let mut missing = Vec::new();
        let mut source_names: BTreeSet<String> = BTreeSet::new();
        for (file, source) in sources {
            for name in public_view_node_constructors(source) {
                if !registered.contains(name.as_str()) {
                    missing.push(format!("{name} ({file})"));
                }
                source_names.insert(name);
            }
        }
        assert!(
            missing.is_empty(),
            "these `pub fn ... -> ViewNode` constructors have no registry row: {missing:?}"
        );
        let extra: Vec<&str> = registered
            .iter()
            .filter(|name| !source_names.contains(**name))
            .copied()
            .collect();
        assert!(
            extra.is_empty(),
            "these rows name no `pub fn ... -> ViewNode` in the six files this group owns: {extra:?}"
        );
    }
}
