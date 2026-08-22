//! The vocabulary and the two themes Petra ships: `standard_vocabulary`
//! declares the names every theme must define, and `light`/`dark` assign
//! them. Both themes are built through [`Theme::build`], so an author who
//! adds a name here and forgets one theme's assignment gets a compile-time-
//! adjacent test failure (see the tests module), not a silently incomplete
//! theme shipped to a feature author.

use std::collections::BTreeMap;

use crate::token::ThemeMode;
use crate::token::name::TokenName;
use crate::token::status::{StatusShape, StatusToken};
use crate::token::theme::Theme;
use crate::token::value::{
    ColorValue, MotionEasing, MotionValue, ShapeValue, TokenKind, TokenValue, TypographyValue,
    TypographyWeight,
};
use crate::token::vocabulary::{DesignToken, Vocabulary};

fn name(n: &str) -> TokenName {
    TokenName::new(n)
        .unwrap_or_else(|err| panic!("shipped vocabulary name {n:?} must be well-formed: {err}"))
}

/// The full set of tokens the shipped light and dark themes must define.
///
/// Deliberately small: enough surface/text/status/spacing/typography/
/// motion/shape coverage to exercise every [`TokenKind`], not an attempt at
/// a production-scale design system.
#[must_use]
pub fn standard_vocabulary() -> Vocabulary {
    let mut vocab = Vocabulary::new();

    vocab
        .declare(DesignToken::new(name("surface.base"), TokenKind::Color))
        .declare(DesignToken::new(name("surface.raised"), TokenKind::Color))
        .declare(DesignToken::new(name("text.primary"), TokenKind::Color))
        .declare(DesignToken::new(name("text.muted"), TokenKind::Color))
        .declare(DesignToken::new(name("spacing.sm"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.md"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.lg"), TokenKind::Spacing))
        .declare(DesignToken::new(
            name("typography.body"),
            TokenKind::Typography,
        ))
        .declare(DesignToken::new(
            name("typography.heading"),
            TokenKind::Typography,
        ))
        .declare(DesignToken::new(name("motion.fast"), TokenKind::Motion))
        .declare(DesignToken::new(name("motion.slow"), TokenKind::Motion))
        .declare(DesignToken::new(name("shape.corner-sm"), TokenKind::Shape))
        .declare(DesignToken::new(name("shape.corner-lg"), TokenKind::Shape));

    // The status subset (FR-015): colour, shape, and text together. Shape
    // and text are mode-independent — only the colour painted into the
    // shape changes between `light()` and `dark()`.
    vocab
        .declare_status(
            StatusToken::new(name("status.ok"), StatusShape::Circle, "OK")
                .expect("non-empty literal text"),
        )
        .declare_status(
            StatusToken::new(name("status.degraded"), StatusShape::Triangle, "Degraded")
                .expect("non-empty literal text"),
        )
        .declare_status(
            StatusToken::new(name("status.down"), StatusShape::Square, "Down")
                .expect("non-empty literal text"),
        );

    vocab
}

/// The shipped light theme, checked complete against [`standard_vocabulary`]
/// at call time (`Theme::build`'s `.expect` below is the completeness
/// proof: it can only panic if this function's own assignments fall out of
/// step with the vocabulary above, which the module's tests catch).
#[must_use]
pub fn light() -> Theme {
    let vocab = standard_vocabulary();
    let mut values = BTreeMap::new();

    values.insert(
        name("surface.base"),
        TokenValue::Color(ColorValue::from_srgb8(0xff, 0xff, 0xff, 0xff)),
    );
    values.insert(
        name("surface.raised"),
        TokenValue::Color(ColorValue::from_srgb8(0xf2, 0xf2, 0xf2, 0xff)),
    );
    values.insert(
        name("text.primary"),
        TokenValue::Color(ColorValue::from_srgb8(0x1a, 0x1a, 0x1a, 0xff)),
    );
    values.insert(
        name("text.muted"),
        TokenValue::Color(ColorValue::from_srgb8(0x5c, 0x5c, 0x5c, 0xff)),
    );
    values.insert(name("spacing.sm"), TokenValue::Spacing(4.0));
    values.insert(name("spacing.md"), TokenValue::Spacing(8.0));
    values.insert(name("spacing.lg"), TokenValue::Spacing(16.0));
    values.insert(
        name("typography.body"),
        TokenValue::Typography(TypographyValue {
            size: 14.0,
            line_height: 20.0,
            weight: TypographyWeight::Regular,
        }),
    );
    values.insert(
        name("typography.heading"),
        TokenValue::Typography(TypographyValue {
            size: 20.0,
            line_height: 28.0,
            weight: TypographyWeight::Bold,
        }),
    );
    values.insert(
        name("motion.fast"),
        TokenValue::Motion(MotionValue {
            duration_ms: 120.0,
            easing: MotionEasing::EaseOut,
        }),
    );
    values.insert(
        name("motion.slow"),
        TokenValue::Motion(MotionValue {
            duration_ms: 320.0,
            easing: MotionEasing::EaseInOut,
        }),
    );
    values.insert(
        name("shape.corner-sm"),
        TokenValue::Shape(ShapeValue { corner_radius: 4.0 }),
    );
    values.insert(
        name("shape.corner-lg"),
        TokenValue::Shape(ShapeValue {
            corner_radius: 12.0,
        }),
    );

    // Status colours: distinguishable in hue *and* lightness, on top of the
    // shape/text channels the vocabulary already pairs them with — a
    // colourblind reader is never left with hue as the only signal.
    values.insert(
        name("status.ok"),
        TokenValue::Color(ColorValue::from_srgb8(0x1e, 0x7d, 0x32, 0xff)),
    );
    values.insert(
        name("status.degraded"),
        TokenValue::Color(ColorValue::from_srgb8(0xb2, 0x6a, 0x00, 0xff)),
    );
    values.insert(
        name("status.down"),
        TokenValue::Color(ColorValue::from_srgb8(0xb0, 0x00, 0x20, 0xff)),
    );

    Theme::build(ThemeMode::Light, &vocab, values).expect("shipped light theme must be complete")
}

/// The shipped dark theme. See [`light`] for the completeness note.
#[must_use]
pub fn dark() -> Theme {
    let vocab = standard_vocabulary();
    let mut values = BTreeMap::new();

    values.insert(
        name("surface.base"),
        TokenValue::Color(ColorValue::from_srgb8(0x12, 0x12, 0x12, 0xff)),
    );
    values.insert(
        name("surface.raised"),
        TokenValue::Color(ColorValue::from_srgb8(0x22, 0x22, 0x22, 0xff)),
    );
    values.insert(
        name("text.primary"),
        TokenValue::Color(ColorValue::from_srgb8(0xf2, 0xf2, 0xf2, 0xff)),
    );
    values.insert(
        name("text.muted"),
        TokenValue::Color(ColorValue::from_srgb8(0xa3, 0xa3, 0xa3, 0xff)),
    );
    values.insert(name("spacing.sm"), TokenValue::Spacing(4.0));
    values.insert(name("spacing.md"), TokenValue::Spacing(8.0));
    values.insert(name("spacing.lg"), TokenValue::Spacing(16.0));
    values.insert(
        name("typography.body"),
        TokenValue::Typography(TypographyValue {
            size: 14.0,
            line_height: 20.0,
            weight: TypographyWeight::Regular,
        }),
    );
    values.insert(
        name("typography.heading"),
        TokenValue::Typography(TypographyValue {
            size: 20.0,
            line_height: 28.0,
            weight: TypographyWeight::Bold,
        }),
    );
    values.insert(
        name("motion.fast"),
        TokenValue::Motion(MotionValue {
            duration_ms: 120.0,
            easing: MotionEasing::EaseOut,
        }),
    );
    values.insert(
        name("motion.slow"),
        TokenValue::Motion(MotionValue {
            duration_ms: 320.0,
            easing: MotionEasing::EaseInOut,
        }),
    );
    values.insert(
        name("shape.corner-sm"),
        TokenValue::Shape(ShapeValue { corner_radius: 4.0 }),
    );
    values.insert(
        name("shape.corner-lg"),
        TokenValue::Shape(ShapeValue {
            corner_radius: 12.0,
        }),
    );

    values.insert(
        name("status.ok"),
        TokenValue::Color(ColorValue::from_srgb8(0x4c, 0xaf, 0x50, 0xff)),
    );
    values.insert(
        name("status.degraded"),
        TokenValue::Color(ColorValue::from_srgb8(0xff, 0xa7, 0x26, 0xff)),
    );
    values.insert(
        name("status.down"),
        TokenValue::Color(ColorValue::from_srgb8(0xff, 0x52, 0x52, 0xff)),
    );

    Theme::build(ThemeMode::Dark, &vocab, values).expect("shipped dark theme must be complete")
}

#[cfg(test)]
mod tests {
    use super::{dark, light, standard_vocabulary};
    use crate::token::ThemeMode;

    #[test]
    fn the_shipped_light_theme_is_complete() {
        let theme = light();
        assert_eq!(theme.mode(), ThemeMode::Light);
        for name in standard_vocabulary().names() {
            assert!(theme.value(name).is_some(), "light theme is missing {name}");
        }
    }

    #[test]
    fn the_shipped_dark_theme_is_complete() {
        let theme = dark();
        assert_eq!(theme.mode(), ThemeMode::Dark);
        for name in standard_vocabulary().names() {
            assert!(theme.value(name).is_some(), "dark theme is missing {name}");
        }
    }

    #[test]
    fn every_shipped_status_token_carries_a_shape_and_non_empty_text() {
        let vocab = standard_vocabulary();
        let statuses: Vec<_> = vocab.statuses().collect();
        assert_eq!(
            statuses.len(),
            3,
            "expected status.ok, status.degraded, status.down"
        );
        for status in statuses {
            assert!(
                !status.text().trim().is_empty(),
                "{} has no text channel",
                status.name()
            );
            // `shape()` returns a `StatusShape`, not an `Option`: simply
            // calling it is proof a shape exists, since there is no other
            // way to have constructed this `StatusToken`.
            let _ = status.shape();
        }
    }
}
