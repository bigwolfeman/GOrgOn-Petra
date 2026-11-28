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
    /// `"warning-alt-filled"` on the wire, [`IconMark::WarningAltFilled`] in the tree.
    WarningAltFilled,
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
    /// `"folder"` on the wire, [`IconMark::Folder`] in the tree.
    Folder,
    /// `"folder-open"` on the wire, [`IconMark::FolderOpen`] in the tree.
    FolderOpen,
    /// `"document"` on the wire, [`IconMark::Document`] in the tree.
    Document,
    /// `"launch"` on the wire, [`IconMark::Launch`] in the tree.
    Launch,
    /// `"launch-outside"` on the wire, [`IconMark::LaunchOutside`] in the tree.
    LaunchOutside,
    /// `"unlink"` on the wire, [`IconMark::Unlink`] in the tree.
    Unlink,
    /// `"locked"` on the wire, [`IconMark::Locked`] in the tree.
    Locked,
    /// `"view-off"` on the wire, [`IconMark::ViewOff`] in the tree.
    ViewOff,
    /// `"folder-dashed"` on the wire, [`IconMark::FolderDashed`] in the tree.
    FolderDashed,
    /// `"branch"` on the wire, [`IconMark::Branch`] in the tree.
    Branch,
    /// `"branch-worktree"` on the wire, [`IconMark::BranchWorktree`] in the tree.
    BranchWorktree,
    /// `"branch-boxed"` on the wire, [`IconMark::BranchBoxed`] in the tree.
    BranchBoxed,
    /// `"unknown"` on the wire, [`IconMark::Unknown`] in the tree.
    Unknown,
    /// `"document-invalid"` on the wire, [`IconMark::DocumentInvalid`] in the tree.
    DocumentInvalid,
    /// `"circle"` on the wire, [`IconMark::Circle`] in the tree.
    Circle,
    /// `"circle-half"` on the wire, [`IconMark::CircleHalf`] in the tree.
    CircleHalf,
    /// `"circle-filled"` on the wire, [`IconMark::CircleFilled`] in the tree.
    CircleFilled,
    /// `"square-bar"` on the wire, [`IconMark::SquareBar`] in the tree.
    SquareBar,
    /// `"square-bar-half"` on the wire, [`IconMark::SquareBarHalf`] in the tree.
    SquareBarHalf,
    /// `"square-bar-filled"` on the wire, [`IconMark::SquareBarFilled`] in the tree.
    SquareBarFilled,
    /// `"triangle-alert"` on the wire, [`IconMark::TriangleAlert`] in the tree.
    TriangleAlert,
    /// `"triangle-alert-half"` on the wire, [`IconMark::TriangleAlertHalf`] in the tree.
    TriangleAlertHalf,
    /// `"triangle-alert-filled"` on the wire, [`IconMark::TriangleAlertFilled`] in the tree.
    TriangleAlertFilled,
    /// `"squares-arrow"` on the wire, [`IconMark::SquaresArrow`] in the tree.
    SquaresArrow,
    /// `"squares-arrow-half"` on the wire, [`IconMark::SquaresArrowHalf`] in the tree.
    SquaresArrowHalf,
    /// `"squares-arrow-filled"` on the wire, [`IconMark::SquaresArrowFilled`] in the tree.
    SquaresArrowFilled,
    /// `"square-plus"` on the wire, [`IconMark::SquarePlus`] in the tree.
    SquarePlus,
    /// `"square-plus-dashed"` on the wire, [`IconMark::SquarePlusDashed`] in the tree.
    SquarePlusDashed,
    /// `"square-in-square"` on the wire, [`IconMark::SquareInSquare`] in the tree.
    SquareInSquare,
    /// `"arrow-up"` on the wire, [`IconMark::ArrowUp`] in the tree.
    ArrowUp,
    /// `"arrow-down"` on the wire, [`IconMark::ArrowDown`] in the tree.
    ArrowDown,
    /// `"arrow-question"` on the wire, [`IconMark::ArrowQuestion`] in the tree.
    ArrowQuestion,
    /// `"arrow-cross"` on the wire, [`IconMark::ArrowCross`] in the tree.
    ArrowCross,
    /// `"arrow-dashed"` on the wire, [`IconMark::ArrowDashed`] in the tree.
    ArrowDashed,
    /// `"arrows-loop"` on the wire, [`IconMark::ArrowsLoop`] in the tree.
    ArrowsLoop,
    /// `"square-ellipsis"` on the wire, [`IconMark::SquareEllipsis`] in the tree.
    SquareEllipsis,
    /// `"sieve"` on the wire, [`IconMark::Sieve`] in the tree.
    Sieve,
    /// `"hourglass"` on the wire, [`IconMark::Hourglass`] in the tree.
    Hourglass,
    /// `"octagon-cross"` on the wire, [`IconMark::OctagonCross`] in the tree.
    OctagonCross,
    /// `"octagon-bar"` on the wire, [`IconMark::OctagonBar`] in the tree.
    OctagonBar,
    /// `"folder-cracked"` on the wire, [`IconMark::FolderCracked`] in the tree.
    FolderCracked,
    /// `"folder-person"` on the wire, [`IconMark::FolderPerson`] in the tree.
    FolderPerson,
    /// `"cloud"` on the wire, [`IconMark::Cloud`] in the tree.
    Cloud,
    /// `"circle-exclamation"` on the wire, [`IconMark::CircleExclamation`] in the tree.
    CircleExclamation,
    /// `"diamond-filled"` on the wire, [`IconMark::DiamondFilled`] in the tree.
    DiamondFilled,
    /// `"disc-on-bar"` on the wire, [`IconMark::DiscOnBar`] in the tree.
    DiscOnBar,
    /// `"document-add"` on the wire, [`IconMark::DocumentAdd`] in the tree.
    DocumentAdd,
    /// `"folder-add"` on the wire, [`IconMark::FolderAdd`] in the tree.
    FolderAdd,
    /// `"folder-move-to"` on the wire, [`IconMark::FolderMoveTo`] in the tree.
    FolderMoveTo,
    /// `"trash-can"` on the wire, [`IconMark::TrashCan`] in the tree.
    TrashCan,
    /// `"undo"` on the wire, [`IconMark::Undo`] in the tree.
    Undo,
    /// `"collapse-all"` on the wire, [`IconMark::CollapseAll`] in the tree.
    CollapseAll,
    /// `"replace"` on the wire, [`IconMark::Replace`] in the tree.
    Replace,
    /// `"replace-all"` on the wire, [`IconMark::ReplaceAll`] in the tree.
    ReplaceAll,
    /// `"stop"` on the wire, [`IconMark::Stop`] in the tree.
    Stop,
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
            IconMarkParam::WarningAltFilled => IconMark::WarningAltFilled,
            IconMarkParam::WarningFilled => IconMark::WarningFilled,
            IconMarkParam::InformationFilled => IconMark::InformationFilled,
            IconMarkParam::CheckmarkFilled => IconMark::CheckmarkFilled,
            IconMarkParam::CaretDown => IconMark::CaretDown,
            IconMarkParam::Edit => IconMark::Edit,
            IconMarkParam::BulletDisc => IconMark::BulletDisc,
            IconMarkParam::BulletCircle => IconMark::BulletCircle,
            IconMarkParam::BulletSquare => IconMark::BulletSquare,
            IconMarkParam::BulletDash => IconMark::BulletDash,
            IconMarkParam::Folder => IconMark::Folder,
            IconMarkParam::FolderOpen => IconMark::FolderOpen,
            IconMarkParam::Document => IconMark::Document,
            IconMarkParam::Launch => IconMark::Launch,
            IconMarkParam::LaunchOutside => IconMark::LaunchOutside,
            IconMarkParam::Unlink => IconMark::Unlink,
            IconMarkParam::Locked => IconMark::Locked,
            IconMarkParam::ViewOff => IconMark::ViewOff,
            IconMarkParam::FolderDashed => IconMark::FolderDashed,
            IconMarkParam::Branch => IconMark::Branch,
            IconMarkParam::BranchWorktree => IconMark::BranchWorktree,
            IconMarkParam::BranchBoxed => IconMark::BranchBoxed,
            IconMarkParam::Unknown => IconMark::Unknown,
            IconMarkParam::DocumentInvalid => IconMark::DocumentInvalid,
            IconMarkParam::Circle => IconMark::Circle,
            IconMarkParam::CircleHalf => IconMark::CircleHalf,
            IconMarkParam::CircleFilled => IconMark::CircleFilled,
            IconMarkParam::SquareBar => IconMark::SquareBar,
            IconMarkParam::SquareBarHalf => IconMark::SquareBarHalf,
            IconMarkParam::SquareBarFilled => IconMark::SquareBarFilled,
            IconMarkParam::TriangleAlert => IconMark::TriangleAlert,
            IconMarkParam::TriangleAlertHalf => IconMark::TriangleAlertHalf,
            IconMarkParam::TriangleAlertFilled => IconMark::TriangleAlertFilled,
            IconMarkParam::SquaresArrow => IconMark::SquaresArrow,
            IconMarkParam::SquaresArrowHalf => IconMark::SquaresArrowHalf,
            IconMarkParam::SquaresArrowFilled => IconMark::SquaresArrowFilled,
            IconMarkParam::SquarePlus => IconMark::SquarePlus,
            IconMarkParam::SquarePlusDashed => IconMark::SquarePlusDashed,
            IconMarkParam::SquareInSquare => IconMark::SquareInSquare,
            IconMarkParam::ArrowUp => IconMark::ArrowUp,
            IconMarkParam::ArrowDown => IconMark::ArrowDown,
            IconMarkParam::ArrowQuestion => IconMark::ArrowQuestion,
            IconMarkParam::ArrowCross => IconMark::ArrowCross,
            IconMarkParam::ArrowDashed => IconMark::ArrowDashed,
            IconMarkParam::ArrowsLoop => IconMark::ArrowsLoop,
            IconMarkParam::SquareEllipsis => IconMark::SquareEllipsis,
            IconMarkParam::Sieve => IconMark::Sieve,
            IconMarkParam::Hourglass => IconMark::Hourglass,
            IconMarkParam::OctagonCross => IconMark::OctagonCross,
            IconMarkParam::OctagonBar => IconMark::OctagonBar,
            IconMarkParam::FolderCracked => IconMark::FolderCracked,
            IconMarkParam::FolderPerson => IconMark::FolderPerson,
            IconMarkParam::Cloud => IconMark::Cloud,
            IconMarkParam::CircleExclamation => IconMark::CircleExclamation,
            IconMarkParam::DiamondFilled => IconMark::DiamondFilled,
            IconMarkParam::DiscOnBar => IconMark::DiscOnBar,
            IconMarkParam::DocumentAdd => IconMark::DocumentAdd,
            IconMarkParam::FolderAdd => IconMark::FolderAdd,
            IconMarkParam::FolderMoveTo => IconMark::FolderMoveTo,
            IconMarkParam::TrashCan => IconMark::TrashCan,
            IconMarkParam::Undo => IconMark::Undo,
            IconMarkParam::CollapseAll => IconMark::CollapseAll,
            IconMarkParam::Replace => IconMark::Replace,
            IconMarkParam::ReplaceAll => IconMark::ReplaceAll,
            IconMarkParam::Stop => IconMark::Stop,
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
         \"error-filled\" | \"warning-alt-filled\" | \"warning-filled\" | \"information-filled\" | \"checkmark-filled\" | \
         \"caret-down\" | \"edit\" | \"bullet-disc\" | \"bullet-circle\" | \"bullet-square\" | \
         \"bullet-dash\" | \"folder\" | \"folder-open\" | \"document\" | \"launch\" | \
         \"launch-outside\" | \"unlink\" | \"locked\" | \"view-off\" | \"folder-dashed\" | \
         \"branch\" | \"branch-worktree\" | \"branch-boxed\" | \"unknown\" | \
         \"document-invalid\" | \"circle\" | \"circle-half\" | \"circle-filled\" | \
         \"square-bar\" | \"square-bar-half\" | \"square-bar-filled\" | \"triangle-alert\" | \
         \"triangle-alert-half\" | \"triangle-alert-filled\" | \"squares-arrow\" | \
         \"squares-arrow-half\" | \"squares-arrow-filled\" | \"square-plus\" | \
         \"square-plus-dashed\" | \"square-in-square\" | \"arrow-up\" | \"arrow-down\" | \"arrow-question\" | \"arrow-cross\" | \"arrow-dashed\" | \
         \"arrows-loop\" | \"square-ellipsis\" | \"sieve\" | \"hourglass\" | \
         \"octagon-cross\" | \"octagon-bar\" | \"folder-cracked\" | \"folder-person\" | \
         \"cloud\" | \"circle-exclamation\" | \"diamond-filled\" | \"disc-on-bar\" | \
         \"document-add\" | \"folder-add\" | \"folder-move-to\" | \"trash-can\" | \"undo\" | \
         \"collapse-all\" | \"replace\" | \"replace-all\" | \"stop\""
    };
}
