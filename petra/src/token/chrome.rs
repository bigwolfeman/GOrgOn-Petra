//! The chrome set: the flat surfaces of an application frame.
//!
//! A workbench (an editor, a file manager, any tool with a side bar, a tab
//! strip and a status line around one working surface) is not a stack of
//! Carbon layers. Its frame is two near-equal greys split by a hairline, so
//! the working surface reads as the page and everything around it as quiet
//! furniture. The layer set ([`super::shipped::LAYER_TOKENS`]) steps eight
//! CIE L\* units per layer, which is right for a card on a page and too loud
//! for a frame: a side bar one layer up reads as a second page beside the
//! first.
//!
//! Five names:
//!
//! * `chrome.ground`: the working surface (an editor's text area).
//! * `chrome.side`: the frame around it (side bar, tab strip, status line).
//!   One small step off the ground, darker in a dark theme and darker in a
//!   light one, as the reference editors draw it.
//! * `chrome.rule`: the hairline between frame parts.
//! * `chrome.hover` and `chrome.selected`: a row's resting-to-hovered and
//!   selected fills on `chrome.side`, stepping away from it.
//!
//! The values are measured, not derived: see
//! `.agents/notes/implemented/feature/2026-11-22-ide-chrome-follows-the-editor-convention.md`
//! in the GOrgOn repository for the reference capture. Every text tone a
//! theme ships keeps the 4.5:1 body floor on every chrome surface
//! (`every_text_tone_clears_the_body_floor_on_every_chrome_surface`).

use std::collections::BTreeMap;

use crate::token::ThemeMode;
use crate::token::name::TokenName;
use crate::token::theme::ThemeId;
use crate::token::value::{ColorValue, TokenKind, TokenValue};
use crate::token::vocabulary::{DesignToken, Vocabulary};

/// The working surface.
pub const CHROME_GROUND: &str = "chrome.ground";
/// The frame around the working surface.
pub const CHROME_SIDE: &str = "chrome.side";
/// The hairline between frame parts.
pub const CHROME_RULE: &str = "chrome.rule";
/// A hovered row on [`CHROME_SIDE`].
pub const CHROME_HOVER: &str = "chrome.hover";
/// A selected row on [`CHROME_SIDE`].
pub const CHROME_SELECTED: &str = "chrome.selected";

/// Every chrome name, in [`chrome_set`] order.
pub const CHROME_TOKENS: [&str; 5] = [
    CHROME_GROUND,
    CHROME_SIDE,
    CHROME_RULE,
    CHROME_HOVER,
    CHROME_SELECTED,
];

/// The five chrome tones of `id`, in [`CHROME_TOKENS`] order.
///
/// Gray 100 is the reference editor's own pair (`#181818` working surface,
/// `#141414` frame). White is the light reference pair (`#ffffff`,
/// `#f8f8f8`). Gray 90 and Gray 10 keep their Carbon ground as the working
/// surface's neighbourhood and take the same small step for the frame.
#[must_use]
pub const fn chrome_set(id: ThemeId) -> [[u8; 3]; 5] {
    match id {
        ThemeId::Gray100 => [
            [0x18, 0x18, 0x18],
            [0x14, 0x14, 0x14],
            [0x26, 0x26, 0x26],
            [0x23, 0x23, 0x23],
            [0x30, 0x30, 0x30],
        ],
        ThemeId::Gray90 => [
            [0x26, 0x26, 0x26],
            [0x21, 0x21, 0x21],
            [0x33, 0x33, 0x33],
            [0x2d, 0x2d, 0x2d],
            [0x38, 0x38, 0x38],
        ],
        ThemeId::White => [
            [0xff, 0xff, 0xff],
            [0xf8, 0xf8, 0xf8],
            [0xe5, 0xe5, 0xe5],
            [0xee, 0xee, 0xee],
            [0xe2, 0xe2, 0xe2],
        ],
        ThemeId::Gray10 => [
            [0xf4, 0xf4, 0xf4],
            [0xeb, 0xeb, 0xeb],
            [0xd9, 0xd9, 0xd9],
            [0xe2, 0xe2, 0xe2],
            [0xd8, 0xd8, 0xd8],
        ],
    }
}

fn name(n: &str) -> TokenName {
    TokenName::new(n).unwrap_or_else(|err| panic!("chrome name {n:?} must be well-formed: {err}"))
}

/// Declare the five chrome names.
pub(crate) fn declare(vocab: &mut Vocabulary) {
    for token in CHROME_TOKENS {
        vocab.declare(DesignToken::new(name(token), TokenKind::Color));
    }
}

/// Assign the five chrome tones of `id`.
pub(crate) fn insert(values: &mut BTreeMap<TokenName, TokenValue>, id: ThemeId) {
    for (token, [r, g, b]) in CHROME_TOKENS.iter().zip(chrome_set(id)) {
        values.insert(
            name(token),
            TokenValue::Color(ColorValue::from_srgb8(r, g, b, 0xff)),
        );
    }
}

/// Which way "away from the frame" goes: lighter on a dark theme, darker on
/// a light one.
#[must_use]
pub const fn away(mode: ThemeMode) -> i8 {
    match mode {
        ThemeMode::Dark => 1,
        ThemeMode::Light => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::measure::contrast;
    use crate::token::shipped::shipped;

    const IDS: [ThemeId; 4] = [
        ThemeId::White,
        ThemeId::Gray10,
        ThemeId::Gray90,
        ThemeId::Gray100,
    ];

    fn colour(theme: &crate::token::Theme, token: &str) -> ColorValue {
        match theme.value(&name(token)) {
            Some(TokenValue::Color(c)) => *c,
            other => panic!("{token} is not a colour: {other:?}"),
        }
    }

    #[test]
    fn every_shipped_theme_assigns_the_chrome_set() {
        for id in IDS {
            let theme = shipped(id);
            for (token, [r, g, b]) in CHROME_TOKENS.iter().zip(chrome_set(id)) {
                assert_eq!(
                    colour(&theme, token),
                    ColorValue::from_srgb8(r, g, b, 0xff),
                    "{} {token}",
                    id.as_str()
                );
            }
        }
    }

    /// Text on the frame is body text: a tree's names, a tab's title, a
    /// status line. Every tone, the faint one included, keeps 4.5:1 on
    /// every chrome surface it can sit on.
    #[test]
    fn every_text_tone_clears_the_body_floor_on_every_chrome_surface() {
        for id in IDS {
            let theme = shipped(id);
            for surface in [CHROME_GROUND, CHROME_SIDE, CHROME_HOVER, CHROME_SELECTED] {
                for ink in ["text.primary", "text.muted", "text.faint"] {
                    let ratio = contrast(colour(&theme, ink), colour(&theme, surface));
                    assert!(
                        ratio >= 4.5,
                        "{} {ink} on {surface} is {ratio:.2}:1",
                        id.as_str()
                    );
                }
            }
        }
    }

    /// The frame is one small step off the working surface, never the same
    /// grey and never a layer step: the side bar must not read as a second
    /// page. Hover and selected step away from the frame, in order, so
    /// selection is the stronger of the two.
    #[test]
    fn the_frame_steps_once_and_row_states_step_away_from_it() {
        for id in IDS {
            let [ground, side, rule, hover, selected] = chrome_set(id);
            let level = |c: [u8; 3]| i16::from(c[0]);
            let dir = i16::from(away(id.mode()));
            // Darker in both polarities: the frame recedes, the page leads.
            let step = level(ground) - level(side);
            assert!(
                (1..=12).contains(&step),
                "{}: the frame is {step} levels off the ground",
                id.as_str()
            );
            assert!(
                (level(hover) - level(side)) * dir > 0,
                "{}: hover steps away from the frame",
                id.as_str()
            );
            assert!(
                (level(selected) - level(hover)) * dir > 0,
                "{}: selected steps past hover",
                id.as_str()
            );
            assert!(
                rule != ground && rule != side,
                "{}: the rule is its own grey",
                id.as_str()
            );
            assert!(
                (level(rule) - level(ground)) * dir > 0,
                "{}: the rule sits on the far side of the ground from the frame",
                id.as_str()
            );
        }
    }
}
