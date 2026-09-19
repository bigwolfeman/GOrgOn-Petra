//! The wire vocabulary for [`IconMark`], shared by every registry group
//! that has a constructor taking an icon.
//!
//! [`IconMark`] deliberately derives no serde support: `icon.rs`'s closed
//! vocabulary is a drawing decision, not a wire format, and giving it
//! `Deserialize` would make every future variant a silent wire change. So
//! the registry carries a wire mirror instead.
//!
//! That mirror used to be copied, byte for byte, into `registry/navigation.rs`,
//! `registry/new_atomics.rs` and `registry/atoms.rs` — 30 variants, a 30-arm
//! `From`, and a 30-name Luau union macro, three times. Each of those files
//! said in a comment that extracting it was a later simplification; the third
//! one said "the third file to say so". Spec 014 is the leaf that needed a
//! fourth copy, in `gorgon-petra-compound` (`command_compound`'s items and
//! modes carry icons), and a fourth copy in a second crate is where the cost
//! stopped being theoretical. One definition now, and it is `pub` because
//! that second crate is the reason it exists.
//!
//! Adding a variant to [`IconMark`] means adding it here, to the `From`, and
//! to [`icon_mark_luau`] — three edits in one file rather than nine across
//! four.

use serde::Deserialize;

use crate::component::IconMark;

/// Wire form of [`IconMark`]: the shipped enum has no serde support.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IconMarkParam {
    /// `"check"` on the wire, [`IconMark::Check`] in the tree.
    Check,
    /// `"calendar"` on the wire, [`IconMark::Calendar`] in the tree.
    Calendar,
    /// `"chevron-down"` on the wire, [`IconMark::ChevronDown`] in the tree.
    ChevronDown,
    /// `"chevron-up"` on the wire, [`IconMark::ChevronUp`] in the tree.
    ChevronUp,
    /// `"chevron-left"` on the wire, [`IconMark::ChevronLeft`] in the tree.
    ChevronLeft,
    /// `"chevron-right"` on the wire, [`IconMark::ChevronRight`] in the tree.
    ChevronRight,
    /// `"close"` on the wire, [`IconMark::Close`] in the tree.
    Close,
    /// `"copy"` on the wire, [`IconMark::Copy`] in the tree.
    Copy,
    /// `"add"` on the wire, [`IconMark::Add`] in the tree.
    Add,
    /// `"subtract"` on the wire, [`IconMark::Subtract`] in the tree.
    Subtract,
    /// `"search"` on the wire, [`IconMark::Search`] in the tree.
    Search,
    /// `"menu"` on the wire, [`IconMark::Menu`] in the tree.
    Menu,
    /// `"notification"` on the wire, [`IconMark::Notification`] in the tree.
    Notification,
    /// `"switcher"` on the wire, [`IconMark::Switcher`] in the tree.
    Switcher,
    /// `"caret-left"` on the wire, [`IconMark::CaretLeft`] in the tree.
    CaretLeft,
    /// `"caret-right"` on the wire, [`IconMark::CaretRight`] in the tree.
    CaretRight,
    /// `"checkmark-outline"` on the wire, [`IconMark::CheckmarkOutline`] in the tree.
    CheckmarkOutline,
    /// `"circle-dash"` on the wire, [`IconMark::CircleDash`] in the tree.
    CircleDash,
    /// `"incomplete"` on the wire, [`IconMark::Incomplete`] in the tree.
    Incomplete,
    /// `"checkmark"` on the wire, [`IconMark::Checkmark`] in the tree.
    Checkmark,
    /// `"error-filled"` on the wire, [`IconMark::ErrorFilled`] in the tree.
    ErrorFilled,
    /// `"warning-filled"` on the wire, [`IconMark::WarningFilled`] in the tree.
    WarningFilled,
    /// `"information-filled"` on the wire, [`IconMark::InformationFilled`] in the tree.
    InformationFilled,
    /// `"checkmark-filled"` on the wire, [`IconMark::CheckmarkFilled`] in the tree.
    CheckmarkFilled,
    /// `"caret-down"` on the wire, [`IconMark::CaretDown`] in the tree.
    CaretDown,
    /// `"edit"` on the wire, [`IconMark::Edit`] in the tree.
    Edit,
    /// `"bullet-disc"` on the wire, [`IconMark::BulletDisc`] in the tree.
    BulletDisc,
    /// `"bullet-circle"` on the wire, [`IconMark::BulletCircle`] in the tree.
    BulletCircle,
    /// `"bullet-square"` on the wire, [`IconMark::BulletSquare`] in the tree.
    BulletSquare,
    /// `"bullet-dash"` on the wire, [`IconMark::BulletDash`] in the tree.
    BulletDash,
}

impl From<IconMarkParam> for IconMark {
    fn from(m: IconMarkParam) -> Self {
        match m {
            IconMarkParam::Check => IconMark::Check,
            IconMarkParam::Calendar => IconMark::Calendar,
            IconMarkParam::ChevronDown => IconMark::ChevronDown,
            IconMarkParam::ChevronUp => IconMark::ChevronUp,
            IconMarkParam::ChevronLeft => IconMark::ChevronLeft,
            IconMarkParam::ChevronRight => IconMark::ChevronRight,
            IconMarkParam::Close => IconMark::Close,
            IconMarkParam::Copy => IconMark::Copy,
            IconMarkParam::Add => IconMark::Add,
            IconMarkParam::Subtract => IconMark::Subtract,
            IconMarkParam::Search => IconMark::Search,
            IconMarkParam::Menu => IconMark::Menu,
            IconMarkParam::Notification => IconMark::Notification,
            IconMarkParam::Switcher => IconMark::Switcher,
            IconMarkParam::CaretLeft => IconMark::CaretLeft,
            IconMarkParam::CaretRight => IconMark::CaretRight,
            IconMarkParam::CheckmarkOutline => IconMark::CheckmarkOutline,
            IconMarkParam::CircleDash => IconMark::CircleDash,
            IconMarkParam::Incomplete => IconMark::Incomplete,
            IconMarkParam::Checkmark => IconMark::Checkmark,
            IconMarkParam::ErrorFilled => IconMark::ErrorFilled,
            IconMarkParam::WarningFilled => IconMark::WarningFilled,
            IconMarkParam::InformationFilled => IconMark::InformationFilled,
            IconMarkParam::CheckmarkFilled => IconMark::CheckmarkFilled,
            IconMarkParam::CaretDown => IconMark::CaretDown,
            IconMarkParam::Edit => IconMark::Edit,
            IconMarkParam::BulletDisc => IconMark::BulletDisc,
            IconMarkParam::BulletCircle => IconMark::BulletCircle,
            IconMarkParam::BulletSquare => IconMark::BulletSquare,
            IconMarkParam::BulletDash => IconMark::BulletDash,
        }
    }
}

/// The Luau union for [`IconMarkParam`], spliced with `concat!` into every
/// shape that carries an icon so the two never drift apart.
///
/// `#[macro_export]`, so it is `gorgon_petra::icon_mark_luau!()` from
/// another crate and `crate::icon_mark_luau!()` from inside this one.
/// `gorgon-petra-compound`'s `command_compound` row is the outside caller.
#[macro_export]
macro_rules! icon_mark_luau {
    () => {
        "\"check\" | \"calendar\" | \"chevron-down\" | \"chevron-up\" | \"chevron-left\" | \
         \"chevron-right\" | \"close\" | \"copy\" | \"add\" | \"subtract\" | \"search\" | \
         \"menu\" | \"notification\" | \"switcher\" | \"caret-left\" | \"caret-right\" | \
         \"checkmark-outline\" | \"circle-dash\" | \"incomplete\" | \"checkmark\" | \
         \"error-filled\" | \"warning-filled\" | \"information-filled\" | \"checkmark-filled\" | \
         \"caret-down\" | \"edit\" | \"bullet-disc\" | \"bullet-circle\" | \"bullet-square\" | \
         \"bullet-dash\""
    };
}
