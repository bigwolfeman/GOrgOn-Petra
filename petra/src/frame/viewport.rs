//! The viewport a frame is negotiated against.

use crate::geom::{Scale, Size};
use crate::token::ThemeMode;

/// Window size, scale, and theme identity at petrify time. Every field is a
/// digest input (`contracts/frame-identity.md` §1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Logical size of the drawable area.
    pub size: Size,
    /// Device pixels per logical unit.
    pub scale: Scale,
    /// Revision of the theme snapshot in force.
    pub theme_rev: u64,
    /// Resolved light or dark mode.
    pub theme_mode: ThemeMode,
}

impl Viewport {
    /// A viewport at unit scale with theme revision zero.
    #[must_use]
    pub fn new(size: Size, mode: ThemeMode) -> Self {
        Self {
            size,
            scale: Scale::ONE,
            theme_rev: 0,
            theme_mode: mode,
        }
    }

    /// This viewport at `scale`.
    #[must_use]
    pub fn with_scale(mut self, scale: Scale) -> Self {
        self.scale = scale;
        self
    }

    /// This viewport at theme revision `rev`.
    #[must_use]
    pub fn with_theme_rev(mut self, rev: u64) -> Self {
        self.theme_rev = rev;
        self
    }
}
