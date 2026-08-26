//! A theme: token → value assignments for one [`ThemeMode`], checked
//! complete against a [`Vocabulary`] at construction time.
//!
//! "Complete" is [`Theme::build`]'s whole job: every vocabulary-declared
//! token must be assigned a value, at the declared kind, or the theme is
//! refused with every offending name listed — never a partial `Theme` that
//! silently falls back for the tokens it forgot.

use std::collections::BTreeMap;
use std::fmt;

use crate::token::ThemeMode;
use crate::token::name::TokenName;
use crate::token::value::{TokenKind, TokenValue};
use crate::token::vocabulary::Vocabulary;

/// Token → value assignments for one mode, guaranteed complete against the
/// [`Vocabulary`] it was built with.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    mode: ThemeMode,
    values: BTreeMap<TokenName, TokenValue>,
}

impl Theme {
    /// Build a theme, refusing it unless `values` assigns every token
    /// `vocabulary` declares, each at its declared kind, and each at a value
    /// layout can use.
    ///
    /// The third condition is why a styling prop can be a bare token
    /// reference. `Props.spacing` and `Props.padding` used to carry numbers,
    /// and tree acceptance range-checked them (`tree::validate`): a negative
    /// or NaN gap is not a gap. Those props now carry names, so the number
    /// arrives from here instead — and the check has to arrive with it, or
    /// nothing checks it at all. A theme is the one place the number is
    /// written, so a theme is where "a gap is a finite, non-negative extent"
    /// is proved. Layout can then resolve a name without a range arm, the
    /// same way it resolves one without a fallback arm.
    ///
    /// # Errors
    /// Returns [`ThemeError`] naming every missing token, every
    /// kind-mismatched token, and every unusable value — all of them, not
    /// just the first found.
    pub fn build(
        mode: ThemeMode,
        vocabulary: &Vocabulary,
        values: BTreeMap<TokenName, TokenValue>,
    ) -> Result<Self, ThemeError> {
        let mut missing = Vec::new();
        let mut mismatched = Vec::new();
        let mut unusable = Vec::new();
        // Completeness is asked of the vocabulary: every declared name must be
        // assigned, at its declared kind. A name `values` carries that the
        // vocabulary does not declare cannot be asked either question -- there
        // is no declared kind to compare against -- so it is not an error here.
        for name in vocabulary.names() {
            let declared = vocabulary
                .kind_of(name)
                .expect("name was just yielded by vocabulary.names()");
            match values.get(name) {
                None => missing.push(name.clone()),
                Some(value) if value.kind() != declared => {
                    mismatched.push(ThemeMismatch {
                        name: name.clone(),
                        declared,
                        found: value.kind(),
                    });
                }
                Some(_) => {}
            }
        }
        // Usability is asked of `values`, and this loop is deliberately not
        // folded into the one above.
        //
        // Walking `vocabulary.names()` for the extent check is what let a
        // negative or NaN gap through: a value under a name the vocabulary did
        // not declare was never inspected, and `Vocabulary::from_theme` --
        // which walks `values`, not the vocabulary -- then declared it anyway.
        // A host that built a theme against `standard_vocabulary()` with extra
        // names in `values` got those names declared, unchecked, and
        // `resolve_spacing` placed a child at `x=-500` for a `-500` gap and at
        // `x=NaN` for a NaN one, with nothing reported.
        //
        // So the rule is: every extent this theme can hand to layout is proved
        // finite and non-negative, whether or not a vocabulary declared it.
        // That is the claim `tree::validate` gave up when styling props became
        // token references, and it is only true if it covers the same set of
        // numbers layout can reach.
        for (name, value) in &values {
            if let Some(why) = unusable_extent(value) {
                unusable.push(ThemeUnusable {
                    name: name.clone(),
                    why,
                });
            }
        }
        if missing.is_empty() && mismatched.is_empty() && unusable.is_empty() {
            Ok(Self { mode, values })
        } else {
            Err(ThemeError {
                mode,
                missing,
                mismatched,
                unusable,
            })
        }
    }

    /// This theme's mode.
    #[must_use]
    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// The value assigned to `name`, if this theme defines it.
    #[must_use]
    pub fn value(&self, name: &TokenName) -> Option<&TokenValue> {
        self.values.get(name)
    }

    /// All token → value assignments.
    #[must_use]
    pub fn values(&self) -> &BTreeMap<TokenName, TokenValue> {
        &self.values
    }
}

/// Why a value that is the right *kind* is still not a value layout can lay
/// anything out with, or `None` when it is fine.
///
/// Most of the extent-valued kinds are checked for the two properties every
/// consumer of an extent assumes: finite, and not negative. A colour out of
/// `[0, 1]` clips at paint time and a motion duration is not a distance, so
/// neither is this function's business.
///
/// [`TokenValue::Coverage`] is the one exception: it is checked against its
/// own legal range, `[1.0, 4.0]`
/// (`ignored/builds/2026-08-24-text-pipeline-port/SPEC.md` §1.1, §6.1),
/// rather than "not negative". A pass count of `0.0` composites nothing —
/// it is not a smaller version of the effect, it is off — and a pass count
/// above `4.0` is past ai-macs' own `maxTextSharpness` ceiling, never
/// measured by either project. "Refuse outside the range, never clamp into
/// it" is the same posture ai-macs' env parser takes for its three knobs
/// (SPEC.md §1.1: "falls to the default, never to zero"); here the
/// equivalent is refusing the whole theme, because a token has no env
/// default to fall back to.
fn unusable_extent(value: &TokenValue) -> Option<&'static str> {
    match value {
        TokenValue::Spacing(units) => finite_and_nonnegative(*units),
        TokenValue::Shape(shape) => finite_and_nonnegative(shape.corner_radius),
        TokenValue::Coverage(coverage) => finite_and_in_range(coverage.passes, 1.0, 4.0),
        _ => None,
    }
}

/// `extent` must be finite and not negative — the shared rule
/// [`TokenValue::Spacing`] and [`TokenValue::Shape`] both check.
fn finite_and_nonnegative(extent: f32) -> Option<&'static str> {
    if !extent.is_finite() {
        Some("not a finite number")
    } else if extent < 0.0 {
        Some("negative")
    } else {
        None
    }
}

/// `value` must be finite and fall inside `[lo, hi]` — the rule
/// [`TokenValue::Coverage`] checks, whose legal range is not "non-negative"
/// (see [`unusable_extent`]'s doc comment for why).
fn finite_and_in_range(value: f32, lo: f32, hi: f32) -> Option<&'static str> {
    if !value.is_finite() {
        Some("not a finite number")
    } else if value < lo || value > hi {
        Some("outside its legal range")
    } else {
        None
    }
}

/// One token assigned a value of the right kind that layout still cannot
/// use: a negative gap, a NaN corner radius.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeUnusable {
    /// The token name.
    pub name: TokenName,
    /// What is wrong with the value, in one phrase.
    pub why: &'static str,
}

/// One token whose assigned value's kind disagrees with its vocabulary
/// declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeMismatch {
    /// The token name.
    pub name: TokenName,
    /// What the vocabulary declared.
    pub declared: TokenKind,
    /// What kind the assigned value actually carried.
    pub found: TokenKind,
}

/// A theme that does not define every vocabulary token, or defines one at
/// the wrong kind. [`Theme::build`] never hands back a `Theme` with this
/// problem silently patched over — a missing token stays missing until an
/// author fixes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeError {
    mode: ThemeMode,
    missing: Vec<TokenName>,
    mismatched: Vec<ThemeMismatch>,
    unusable: Vec<ThemeUnusable>,
}

impl ThemeError {
    /// Every token missing a value, in the order the vocabulary was
    /// iterated (its declaration order).
    #[must_use]
    pub fn missing(&self) -> &[TokenName] {
        &self.missing
    }

    /// Every token assigned a value of the wrong kind.
    #[must_use]
    pub fn mismatched(&self) -> &[ThemeMismatch] {
        &self.mismatched
    }

    /// Every token assigned an extent layout cannot use.
    #[must_use]
    pub fn unusable(&self) -> &[ThemeUnusable] {
        &self.unusable
    }
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "petra: {} theme is incomplete:", self.mode.as_str())?;
        for name in &self.missing {
            writeln!(f, "  missing: {name}")?;
        }
        for mismatch in &self.mismatched {
            writeln!(
                f,
                "  {}: expected {:?}, found {:?}",
                mismatch.name, mismatch.declared, mismatch.found
            )?;
        }
        for bad in &self.unusable {
            writeln!(f, "  {}: extent is {}", bad.name, bad.why)?;
        }
        Ok(())
    }
}

impl std::error::Error for ThemeError {}

#[cfg(test)]
mod tests {
    use super::Theme;
    use crate::token::ThemeMode;
    use crate::token::name::TokenName;
    use crate::token::value::{ColorValue, TokenValue};
    use crate::token::vocabulary::{DesignToken, Vocabulary};
    use std::collections::BTreeMap;

    fn two_token_vocabulary() -> Vocabulary {
        let mut vocab = Vocabulary::new();
        vocab.declare(DesignToken::new(
            TokenName::new("surface.raised").unwrap(),
            crate::token::value::TokenKind::Color,
        ));
        vocab.declare(DesignToken::new(
            TokenName::new("text.muted").unwrap(),
            crate::token::value::TokenKind::Color,
        ));
        vocab
    }

    #[test]
    fn a_theme_missing_tokens_is_refused_and_names_every_one() {
        let vocab = two_token_vocabulary();
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("surface.raised").unwrap(),
            TokenValue::Color(ColorValue::TRANSPARENT),
        );
        // text.muted deliberately absent.
        let err = Theme::build(ThemeMode::Light, &vocab, values).unwrap_err();
        assert_eq!(err.missing(), &[TokenName::new("text.muted").unwrap()]);
        assert!(err.to_string().contains("text.muted"), "{err}");
    }

    #[test]
    fn a_theme_missing_every_token_names_every_one_not_just_the_first() {
        let vocab = two_token_vocabulary();
        let err = Theme::build(ThemeMode::Dark, &vocab, BTreeMap::new()).unwrap_err();
        assert_eq!(err.missing().len(), 2);
        let text = err.to_string();
        assert!(
            text.contains("surface.raised") && text.contains("text.muted"),
            "{text}"
        );
    }

    #[test]
    fn a_value_at_the_wrong_kind_is_refused() {
        let vocab = two_token_vocabulary();
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("surface.raised").unwrap(),
            TokenValue::Spacing(8.0),
        );
        values.insert(
            TokenName::new("text.muted").unwrap(),
            TokenValue::Color(ColorValue::TRANSPARENT),
        );
        let err = Theme::build(ThemeMode::Light, &vocab, values).unwrap_err();
        assert!(err.missing().is_empty());
        assert_eq!(err.mismatched().len(), 1);
        assert_eq!(
            err.mismatched()[0].name,
            TokenName::new("surface.raised").unwrap()
        );
    }

    fn gap_vocabulary() -> Vocabulary {
        let mut vocab = Vocabulary::new();
        vocab.declare(DesignToken::new(
            TokenName::new("spacing-04").unwrap(),
            crate::token::value::TokenKind::Spacing,
        ));
        vocab.declare(DesignToken::new(
            TokenName::new("shape.corner-sm").unwrap(),
            crate::token::value::TokenKind::Shape,
        ));
        vocab
    }

    /// The same check, on a name the vocabulary does **not** declare.
    ///
    /// This is the case that shipped broken. `Theme::build` walked
    /// `vocabulary.names()` to decide what to inspect, so a value under an
    /// undeclared name was skipped entirely — and
    /// [`Vocabulary::from_theme`], which walks `values` rather than the
    /// vocabulary, then declared it. A host building against
    /// `standard_vocabulary()` with an extra name in `values` got that name
    /// declared with its extent never checked, and layout placed a child at
    /// `x=-500` for a `-500` gap.
    ///
    /// The pair of assertions is the point: the theme is refused **and**
    /// `from_theme` would have declared the name, so skipping it was not
    /// harmless.
    #[test]
    fn an_extent_under_a_name_the_vocabulary_never_declared_is_checked_too() {
        for bad in [
            TokenValue::Spacing(-500.0),
            TokenValue::Spacing(f32::NAN),
            TokenValue::Shape(crate::token::value::ShapeValue {
                corner_radius: f32::NEG_INFINITY,
            }),
        ] {
            let undeclared = TokenName::new("spacing.app-gutter").unwrap();
            assert!(
                gap_vocabulary().kind_of(&undeclared).is_none(),
                "the fixture must not declare the name this test is about"
            );

            let mut values = BTreeMap::new();
            values.insert(
                TokenName::new("spacing-04").unwrap(),
                TokenValue::Spacing(12.0),
            );
            values.insert(
                TokenName::new("shape.corner-sm").unwrap(),
                TokenValue::Shape(crate::token::value::ShapeValue { corner_radius: 4.0 }),
            );
            values.insert(undeclared.clone(), bad);

            let err = Theme::build(ThemeMode::Light, &gap_vocabulary(), values)
                .expect_err("an unusable extent is refused whoever declared the name");
            assert!(
                err.unusable.iter().any(|u| u.name == undeclared),
                "{undeclared:?} carrying {bad:?} was not named unusable: {err:?}"
            );
            assert!(
                err.missing.is_empty() && err.mismatched.is_empty(),
                "the theme is complete; only the extent is wrong: {err:?}"
            );
        }
    }

    /// The reason the test above matters: `from_theme` declares every name in
    /// `values`, so any name `Theme::build` declines to inspect becomes a
    /// declared token that layout will resolve.
    #[test]
    fn from_theme_declares_names_build_was_never_asked_about() {
        let undeclared = TokenName::new("spacing.app-gutter").unwrap();
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("spacing-04").unwrap(),
            TokenValue::Spacing(12.0),
        );
        values.insert(
            TokenName::new("shape.corner-sm").unwrap(),
            TokenValue::Shape(crate::token::value::ShapeValue { corner_radius: 4.0 }),
        );
        values.insert(undeclared.clone(), TokenValue::Spacing(20.0));

        let theme = Theme::build(ThemeMode::Light, &gap_vocabulary(), values)
            .expect("a usable extent under an extra name is fine");
        let rebuilt = Vocabulary::from_theme(&theme);
        assert!(
            rebuilt.kind_of(&undeclared).is_some(),
            "from_theme did not declare the extra name, so this test no longer \
             guards what it was written for"
        );
    }

    /// The check `tree::validate` used to run on `Props.spacing` and every
    /// edge of `Props.padding`, now run where the number is actually
    /// written.
    ///
    /// This is not defence in depth — it is the *only* place the number is
    /// checked any more, because a styling prop is a name and a name has no
    /// sign. A negative gap makes a stack lay its children on top of each
    /// other; a NaN one poisons every extent downstream of it.
    #[test]
    fn a_negative_or_non_finite_extent_is_refused_and_named() {
        for (bad, why) in [
            (TokenValue::Spacing(-1.0), "negative"),
            (TokenValue::Spacing(f32::NAN), "not a finite number"),
            (TokenValue::Spacing(f32::INFINITY), "not a finite number"),
        ] {
            let mut values = BTreeMap::new();
            values.insert(TokenName::new("spacing-04").unwrap(), bad);
            values.insert(
                TokenName::new("shape.corner-sm").unwrap(),
                TokenValue::Shape(crate::token::value::ShapeValue { corner_radius: 4.0 }),
            );
            let err = Theme::build(ThemeMode::Light, &gap_vocabulary(), values).unwrap_err();
            assert!(err.missing().is_empty(), "{err}");
            assert!(err.mismatched().is_empty(), "{err}");
            assert_eq!(err.unusable().len(), 1, "{bad:?} was accepted: {err}");
            assert_eq!(
                err.unusable()[0].name,
                TokenName::new("spacing-04").unwrap()
            );
            assert_eq!(err.unusable()[0].why, why);
            assert!(err.to_string().contains(why), "{err}");
        }
    }

    /// A corner radius is an extent too, and gets the same treatment — the
    /// rule is about what the number has to be, not about which prop reads
    /// it.
    #[test]
    fn an_unusable_corner_radius_is_refused_as_well() {
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("spacing-04").unwrap(),
            TokenValue::Spacing(12.0),
        );
        values.insert(
            TokenName::new("shape.corner-sm").unwrap(),
            TokenValue::Shape(crate::token::value::ShapeValue {
                corner_radius: -2.0,
            }),
        );
        let err = Theme::build(ThemeMode::Light, &gap_vocabulary(), values).unwrap_err();
        assert_eq!(err.unusable().len(), 1, "{err}");
        assert_eq!(
            err.unusable()[0].name,
            TokenName::new("shape.corner-sm").unwrap()
        );
    }

    /// Zero is a gap an author may want (a flush run of cells), so the floor
    /// is "not negative", not "positive".
    #[test]
    fn a_zero_extent_is_usable() {
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("spacing-04").unwrap(),
            TokenValue::Spacing(0.0),
        );
        values.insert(
            TokenName::new("shape.corner-sm").unwrap(),
            TokenValue::Shape(crate::token::value::ShapeValue { corner_radius: 0.0 }),
        );
        assert!(Theme::build(ThemeMode::Light, &gap_vocabulary(), values).is_ok());
    }

    fn coverage_vocabulary() -> Vocabulary {
        let mut vocab = Vocabulary::new();
        vocab.declare(DesignToken::new(
            TokenName::new("text.coverage-curve").unwrap(),
            crate::token::value::TokenKind::Coverage,
        ));
        vocab
    }

    /// D4: a pass count outside `[1.0, 4.0]`, and NaN, are refused at
    /// `Theme::build` — the same "refuse, never clamp" posture
    /// [`a_negative_or_non_finite_extent_is_refused_and_named`] proves for
    /// spacing, but against `Coverage`'s own legal range rather than
    /// "not negative".
    #[test]
    fn a_coverage_pass_count_outside_its_legal_range_is_refused() {
        for bad in [0.0_f32, 0.999, 4.001, 10.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut values = BTreeMap::new();
            values.insert(
                TokenName::new("text.coverage-curve").unwrap(),
                TokenValue::Coverage(crate::token::value::CoverageValue {
                    passes: bad,
                    snap: false,
                }),
            );
            let err = Theme::build(ThemeMode::Light, &coverage_vocabulary(), values)
                .expect_err(&format!("passes: {bad} must be refused"));
            assert!(err.missing().is_empty(), "{err}");
            assert!(err.mismatched().is_empty(), "{err}");
            assert_eq!(err.unusable().len(), 1, "{bad}: {err}");
            assert_eq!(
                err.unusable()[0].name,
                TokenName::new("text.coverage-curve").unwrap()
            );
        }
    }

    /// The far side of the same rule: every value in the legal range,
    /// including both closed endpoints, is accepted.
    #[test]
    fn a_coverage_pass_count_inside_its_legal_range_is_usable() {
        for ok in [1.0_f32, 1.5, 2.0, 3.0, 4.0] {
            let mut values = BTreeMap::new();
            values.insert(
                TokenName::new("text.coverage-curve").unwrap(),
                TokenValue::Coverage(crate::token::value::CoverageValue {
                    passes: ok,
                    snap: false,
                }),
            );
            assert!(
                Theme::build(ThemeMode::Light, &coverage_vocabulary(), values).is_ok(),
                "passes: {ok} must be usable"
            );
        }
    }

    #[test]
    fn a_complete_theme_is_accepted_and_answers_lookups() {
        let vocab = two_token_vocabulary();
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("surface.raised").unwrap(),
            TokenValue::Color(ColorValue::TRANSPARENT),
        );
        values.insert(
            TokenName::new("text.muted").unwrap(),
            TokenValue::Color(ColorValue::TRANSPARENT),
        );
        let theme = Theme::build(ThemeMode::Light, &vocab, values).unwrap();
        assert_eq!(theme.mode(), ThemeMode::Light);
        assert!(
            theme
                .value(&TokenName::new("surface.raised").unwrap())
                .is_some()
        );
        assert!(
            theme
                .value(&TokenName::new("status.unknown").unwrap())
                .is_none()
        );
    }
}
