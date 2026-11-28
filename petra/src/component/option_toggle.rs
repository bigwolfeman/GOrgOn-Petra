//! `option_toggle`: a field's own option, drawn inside the field as a short
//! glyph — an editor's `Aa`, `ab` and `.*` in its find well.
//!
//! The glyph is text, set in the label type, so it stays crisp at every
//! scale and needs no drawn icon. `label` names the option for a reader
//! ("Match case"); the glyph is what the eye reads.
//!
//! On is two cues and neither is a hue: the selected fill, a lightness step
//! off the well, and a one-unit outline in the accent ink round the glyph's
//! box. Off has no outline and the muted ink. A reader who cannot tell red
//! from green, or any hue, still sees a boxed glyph against a bare one.
//!
//! [`field_with_options`] is the well that carries them: a workbench find
//! field with its options at the trailing end, and its message under it
//! while the value is refused.
//!
//! The outline is the four-sided box slot, which `petra-no-box-border`
//! allows only for a control mark; this file is on its list for that, and
//! for the refused well's box, the same state `field.rs` draws.

use super::field::{bind_field_chrome, bind_validation_chrome, invalid_helper, warning_helper};
use super::inline_field::FieldState;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, CHROME_HOVER, CHROME_SELECTED, SPACING_01, SPACING_02, SUPPORT_ERROR,
    SUPPORT_WARNING, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT, TYPOGRAPHY_LABEL, t,
};
use super::{IconMark, chrome_icon_button, pin_block, stack};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Behaviour, FocusFigure, FocusShownOn, InsetRefs, Intent, Interaction, Justify,
    Key, NodeKind, Phase, Props, Role, TextCursor, TextRun, ViewNode,
};

/// The height of [`field_with_options`]'s well: the workbench find field.
pub const OPTION_FIELD_HEIGHT: f32 = 28.0;

/// The toggle's box on both axes: it sits inside a 32-unit well with room
/// round it, as an editor's option glyphs do.
pub const OPTION_TOGGLE_SIZE: f32 = 22.0;

const INTERACTIVE: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

const TOGGLES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Toggle,
    phase: Phase::OnRelease,
};

/// One option of a field: its key, its glyph, its name, and whether it is
/// on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldOption {
    key: String,
    glyph: String,
    label: String,
    on: bool,
    underline: bool,
}

impl FieldOption {
    /// The option `key`, drawn as `glyph`, named `label`, `on` or off.
    #[must_use]
    pub fn new(
        key: impl Into<String>,
        glyph: impl Into<String>,
        label: impl Into<String>,
        on: bool,
    ) -> Self {
        Self {
            key: key.into(),
            glyph: glyph.into(),
            label: label.into(),
            on,
            underline: false,
        }
    }

    /// Underline the glyph: the whole-word option's `ab` reads as a word
    /// with its edges marked.
    #[must_use]
    pub fn underlined(mut self) -> Self {
        self.underline = true;
        self
    }

    /// The option as a toggle.
    #[must_use]
    pub fn build(&self) -> ViewNode {
        option_toggle(self)
    }
}

/// `option` as a pressable glyph box.
#[must_use]
pub fn option_toggle(option: &FieldOption) -> ViewNode {
    let mut glyph = text("glyph", option.glyph.clone());
    glyph.props.style = Some(t(TYPOGRAPHY_LABEL));
    glyph.props.tokens.insert(
        "foreground".into(),
        t(if option.on { TEXT_PRIMARY } else { TEXT_MUTED }),
    );
    if option.underline {
        glyph.props.runs = vec![TextRun {
            len: option.glyph.len(),
            underline: true,
            ..TextRun::default()
        }];
    }
    let mut node = stack(option.key.clone(), Axis::Horizontal, None, vec![glyph]);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node.props
        .tokens
        .insert("background@hover".into(), t(CHROME_HOVER));
    node.props
        .tokens
        .insert("background@selected".into(), t(CHROME_SELECTED));
    if option.on {
        node.props.tokens.insert("border".into(), t(ACCENT_PRIMARY));
    }
    let mut node = node
        .with_constraints(pin_block(OPTION_TOGGLE_SIZE))
        .interactive(Role::Button, option.label.clone(), INTERACTIVE)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .owning_its_text()
        .with_focus_figure(FocusFigure::BarUnder);
    node.constraints.horizontal = AxisConstraint {
        min: Some(OPTION_TOGGLE_SIZE),
        max: Some(OPTION_TOGGLE_SIZE),
        priority: 0,
    };
    node.semantics.selected = option.on;
    node
}

/// An icon button the size of an [`option_toggle`]: an action that sits in a
/// find well's trailing column (an editor's replace all) or at the trailing
/// edge of a dense results row (replace this file, this match). A
/// [`chrome_icon_button`] pinned to [`OPTION_TOGGLE_SIZE`] on both axes, so
/// it lines up under the toggles of the well above it and fits a 22-unit
/// row without growing it. Like a toggle, it has no fill until the pointer
/// is on it.
#[must_use]
pub fn field_icon_button(
    key: impl Into<Key>,
    mark: IconMark,
    label: impl Into<String>,
) -> ViewNode {
    let mut node =
        chrome_icon_button(key, mark, label).with_constraints(pin_block(OPTION_TOGGLE_SIZE));
    // No resting fill: it sits on a well's fill or a selected row's band,
    // and the side bar's own fill there is a dark square. The pointer still
    // lights its hover fill, as a toggle's.
    node.props.tokens.remove("background");
    node.constraints.horizontal = AxisConstraint {
        min: Some(OPTION_TOGGLE_SIZE),
        max: Some(OPTION_TOGGLE_SIZE),
        priority: 0,
    };
    node
}

/// A find well named `label`, holding `value` with `cursor`, with
/// `options` at its trailing end and, while `state` refuses or warns, its
/// message under it (glyph and word, `Invalid:` or `Warning:`).
///
/// The well's input is keyed `input` and the options keep their own keys,
/// so a press on either routes by key. The message is in the flow, not a
/// popup: a form has room for it, and a reader meets it after the field.
#[must_use]
pub fn field_with_options(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    cursor: Option<TextCursor>,
    options: &[FieldOption],
    state: &FieldState,
) -> ViewNode {
    let trailing = options.iter().map(FieldOption::build).collect();
    field_with_trailing(key, label, value, cursor, trailing, state)
}

/// [`field_with_options`] with any controls at the trailing end: option
/// toggles, an icon button that acts on the value (an editor's replace
/// all), or both. Two wells built from this are the same width whatever
/// each carries, so a find well and a replace well stack flush.
///
/// Focus shows on the whole well, as one bar along its inner bottom edge
/// across the text and the controls. While `state` refuses or warns, the
/// state's box is the only indicator ([`FocusFigure::Quiet`]).
#[must_use]
pub fn field_with_trailing(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    cursor: Option<TextCursor>,
    trailing: Vec<ViewNode>,
    state: &FieldState,
) -> ViewNode {
    let label = label.into();
    let value = value.into();
    let mut props = Props {
        placeholder: Some(label.clone()),
        text: Some(value.clone()),
        cursor,
        style: Some(t(TYPOGRAPHY_BODY_COMPACT)),
        ..Props::default()
    };
    props.tokens.insert(
        "foreground".into(),
        t(if value.is_empty() {
            TEXT_MUTED
        } else {
            TEXT_PRIMARY
        }),
    );
    let mut input = ViewNode::new(NodeKind::Input, "input")
        .with_props(props)
        .interactive(Role::TextInput, label, super::field::EDITABLE_TEXT_INTENTS);
    input.semantics.focus_shown_on = FocusShownOn::OnWell;
    let mut parts = vec![input];
    parts.extend(trailing);
    let mut well = stack("well", Axis::Horizontal, Some(SPACING_01), parts);
    well.props.align = Some(Align::Center);
    well.props.padding = Some(InsetRefs {
        right: Some(t(SPACING_02)),
        ..InsetRefs::default()
    });
    match state {
        FieldState::Valid => bind_field_chrome(&mut well.props),
        FieldState::Invalid(_) => bind_validation_chrome(&mut well.props, SUPPORT_ERROR),
        FieldState::Warning(_) => bind_validation_chrome(&mut well.props, SUPPORT_WARNING),
    }
    well.semantics.focus_shown_on = FocusShownOn::Well;
    well.semantics.focus_run = true;
    well.semantics.focus_figure = match state {
        FieldState::Valid => FocusFigure::BarInside,
        FieldState::Invalid(_) | FieldState::Warning(_) => FocusFigure::Quiet,
    };
    well.constraints.vertical = AxisConstraint {
        min: Some(OPTION_FIELD_HEIGHT),
        max: Some(OPTION_FIELD_HEIGHT),
        priority: 0,
    };
    let mut children = vec![well];
    match state {
        FieldState::Valid => {}
        FieldState::Invalid(message) => children.push(invalid_helper(message.clone())),
        FieldState::Warning(message) => children.push(warning_helper(message.clone())),
    }
    let mut node = stack(key, Axis::Vertical, Some(SPACING_02), children);
    node.props.align = Some(Align::Stretch);
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_is_an_outline_and_a_fill_and_off_is_neither() {
        let on = FieldOption::new("case", "Aa", "Match case", true).build();
        assert_eq!(on.semantics.label.as_deref(), Some("Match case"));
        assert!(on.semantics.selected);
        assert_eq!(
            on.props.tokens.get("border").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        let off = FieldOption::new("case", "Aa", "Match case", false).build();
        assert!(!off.semantics.selected);
        assert!(
            !off.props.tokens.contains_key("border"),
            "off has no outline"
        );
        assert_eq!(off.constraints.horizontal.min, Some(OPTION_TOGGLE_SIZE));
        let glyph = |n: &ViewNode| {
            n.children
                .iter()
                .find(|c| c.key.as_str() == "glyph")
                .and_then(|c| {
                    c.props
                        .tokens
                        .get("foreground")
                        .map(|t| t.as_str().to_owned())
                })
        };
        assert_eq!(glyph(&on).as_deref(), Some(TEXT_PRIMARY));
        assert_eq!(glyph(&off).as_deref(), Some(TEXT_MUTED));
    }

    /// The action is a toggle's size on both axes and has no resting fill,
    /// so it sits on a well or a selected row without a dark square.
    #[test]
    fn a_field_icon_button_is_a_toggles_size_with_no_resting_fill() {
        let node = field_icon_button("all", IconMark::ReplaceAll, "Replace all");
        assert_eq!(node.constraints.horizontal.max, Some(OPTION_TOGGLE_SIZE));
        assert_eq!(node.constraints.vertical.max, Some(OPTION_TOGGLE_SIZE));
        assert!(!node.props.tokens.contains_key("background"));
        assert!(node.props.tokens.contains_key("background@hover"));
        assert_eq!(node.semantics.label.as_deref(), Some("Replace all"));
    }

    #[test]
    fn the_whole_word_glyph_is_underlined() {
        let word = FieldOption::new("word", "ab", "Match whole word", false)
            .underlined()
            .build();
        let glyph = word
            .children
            .iter()
            .find(|c| c.key.as_str() == "glyph")
            .expect("glyph");
        assert!(glyph.props.runs.iter().all(|r| r.underline));
        assert_eq!(glyph.props.runs.iter().map(|r| r.len).sum::<usize>(), 2);
    }

    #[test]
    fn the_well_holds_its_input_then_its_options_and_a_refusal_says_why_under_it() {
        let options = [
            FieldOption::new("case", "Aa", "Match case", false),
            FieldOption::new("regex", ".*", "Use regular expression", true),
        ];
        let node = field_with_options(
            "query",
            "Search",
            "a(",
            None,
            &options,
            &FieldState::Invalid("unclosed group".into()),
        );
        let well = &node.children[0];
        let keys: Vec<&str> = well.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["input", "case", "regex"]);
        assert_eq!(
            well.props.tokens.get("border").map(|t| t.as_str()),
            Some(SUPPORT_ERROR),
            "a refused well is boxed in the error hue"
        );
        let words = format!("{:?}", node.children[1]);
        assert!(
            words.contains("Invalid") && words.contains("unclosed group"),
            "{words}"
        );
        let valid = field_with_options("query", "Search", "", None, &options, &FieldState::Valid);
        assert_eq!(valid.children.len(), 1, "a legal value has no message");
        assert!(!valid.children[0].props.tokens.contains_key("border"));
        // Focus shows on the whole well, and a refused well shows only its
        // state box.
        let input = &valid.children[0].children[0];
        assert_eq!(input.semantics.focus_shown_on, FocusShownOn::OnWell);
        assert_eq!(
            valid.children[0].semantics.focus_figure,
            FocusFigure::BarInside
        );
        assert_eq!(well.semantics.focus_figure, FocusFigure::Quiet);
    }
}
