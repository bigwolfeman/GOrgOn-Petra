//! `progress` — Carbon determinate progress bar: a labelled track plus fill.
//!
//! Anatomy (slice-d, SCSS `_progress-bar.scss`; T070 prefers SCSS):
//! 1. Label (required, `$text-primary`) — a visual `text` child and the
//!    accessible name on [`Role::Progress`].
//! 2. Helper text (optional) — [`progress_with_helper`] only; the three-arg
//!    constructor does not take one.
//! 3. Track (`$border-subtle`) — child key `"track"`.
//! 4. Bar indicator (`$interactive` → [`ACCENT_PRIMARY`]) — child key `"fill"`.
//!
//! Carbon's fill is `transform: scaleX`. Petra keeps weighted [`Grid`]
//! columns so the existing fill-width tests can read placed rects. Tree
//! acceptance refuses a zero [`TrackSize::Weight`], so 0% and 100% still
//! carry [`MIN_WEIGHT`] on the empty side.

use super::swatch;
use super::text::text;
use super::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, SPACING_03, TEXT_MUTED, TEXT_PRIMARY, t};
use crate::geom::Align;
use crate::tree::{
    AxisConstraint, Constraints, GridSpan, Key, NodeKind, Props, Role, Semantics, TrackSize,
    ViewNode,
};

/// Carbon **big** (default, `--big` and the unmodified class): 8px track.
/// MEASURED `_progress-bar.scss:46,52`.
const BAR_HEIGHT: f32 = 8.0;

/// Carbon **small** (`--small`): 4px track. MEASURED `_progress-bar.scss:56`.
const BAR_HEIGHT_SM: f32 = 4.0;

/// Carbon minimum track/label width. MEASURED `_progress-bar.scss:34,48`.
const MIN_TRACK_WIDTH: f32 = 48.0;

/// Smallest weight either bar track may carry.
///
/// Tree acceptance refuses a `TrackSize::Weight` that is not finite and
/// greater than zero (`Violation::ValueOutOfRange`), so a 0% or 100% bar
/// cannot express its empty side as weight `0.0` — it expresses it as a
/// weight small enough to round away instead.
const MIN_WEIGHT: f32 = 0.001;

/// Track thickness. Two-step Carbon scale, not sm/md/lg.
#[derive(Clone, Copy)]
enum ProgressSize {
    Big,
    Small,
}

impl ProgressSize {
    fn height(self) -> f32 {
        match self {
            Self::Big => BAR_HEIGHT,
            Self::Small => BAR_HEIGHT_SM,
        }
    }
}

/// One cell of the bar: a box that takes its width from the grid track it
/// sits in, coloured when it has something of its own to say.
///
/// [`swatch`] pins both axes to the extents it is handed, which is what a
/// checkbox box or a status dot wants — a fixed square. A progress cell is
/// the opposite case: the whole point of the grid's weighted columns is that
/// the fill is *as wide as the value says*, so the horizontal pin is dropped
/// here and the grid's [`Align::Stretch`] fills the track. The vertical pin
/// stays: the bar's thickness belongs to the component, not to the row it
/// lands in.
///
/// Do not pass `0.0` as a *kept* horizontal max. That was the original bug:
/// both cells declared `min = max = 0.0`, the weights were right, and the
/// placed fill was zero pixels wide.
fn bar_cell(key: &'static str, height: f32, background: Option<&str>) -> ViewNode {
    let mut cell = swatch(key, 0.0, height, background, None, None);
    cell.constraints.horizontal = AxisConstraint::default();
    cell
}

fn normalise(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Label and helper span both weighted columns so they sit above/below the
/// whole track, not in the fill column alone.
fn full_row_span() -> GridSpan {
    GridSpan {
        columns: 2,
        rows: 1,
    }
}

/// A progress bar.
///
/// `value` is normalised into `[0, 1]` once, here, before it drives either
/// the drawn fill (the grid's column weights) or the reported percentage
/// (`Semantics.value`) — so the two can never read as disagreeing with each
/// other the way two independently-rounded copies of the same number could.
/// That normalisation includes the non-finite cases: `f32::clamp` propagates
/// `NaN` while `f32::max` scrubs it, so a `NaN` that reached the two paths
/// separately announced "NaN%" over a bar drawn exactly half full — the one
/// input on which the guarantee above mattered was the one input that broke
/// it.
///
/// A non-finite `value` becomes `0.0` rather than a refusal. This library's
/// posture is that an unrepresentable state should be unrepresentable, and
/// where a constructor can say so it does ([`crate::token::StatusToken::new`]
/// and [`crate::token::TokenName::new`] both return `Result`). `progress`
/// returns a `ViewNode`, so refusal here would have to be a panic, and the
/// usual source of a `NaN` is a caller's `done / total` with `total == 0` —
/// the empty case of a real data source, not a programming error worth
/// taking the frame down for. `0.0` and not `1.0` because it is the reading
/// that claims nothing: a bar that says "0%" over a number nobody could
/// compute is wrong in the direction that does not announce a finished job
/// which never ran.
///
/// Carbon paints the fill with `$interactive`. Petra has no interactive
/// token; [`ACCENT_PRIMARY`] is the shipped interactive hue. The track is
/// `$border-subtle` ([`BORDER_SUBTLE`]). Status finishes (`$support-success`
/// / `$support-error`) are not in the component token list and are not
/// invented here.
///
/// Indeterminate (Carbon: 1400ms infinite linear sliding block) is omitted:
/// the shipped animation registry has no such motion, and naming a new one
/// from this module would be inventing a motion the host cannot resolve.
pub fn progress(key: impl Into<Key>, label: impl Into<String>, value: f32) -> ViewNode {
    progress_sized(key, label, value, ProgressSize::Big, None)
}

/// Carbon **small** progress bar: 4px track. Same anatomy as [`progress`];
/// FR-058 still requires a label argument.
pub fn progress_sm(key: impl Into<Key>, label: impl Into<String>, value: f32) -> ViewNode {
    progress_sized(key, label, value, ProgressSize::Small, None)
}

/// Determinate bar plus Carbon helper text (`$text-secondary` → [`TEXT_MUTED`]).
///
/// The three-arg [`progress`] constructor does not take a helper. Error
/// helper colour (`$text-error`) is not in the token list and is not
/// invented here.
pub fn progress_with_helper(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: f32,
    helper: impl Into<String>,
) -> ViewNode {
    progress_sized(key, label, value, ProgressSize::Big, Some(helper.into()))
}

fn progress_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: f32,
    size: ProgressSize,
    helper: Option<String>,
) -> ViewNode {
    let key = key.into();
    let label = label.into();
    let done = normalise(value);
    let rest = (1.0 - done).max(MIN_WEIGHT);
    let height = size.height();

    let bar_props = Props {
        columns: vec![
            TrackSize::Weight {
                weight: done.max(MIN_WEIGHT),
            },
            TrackSize::Weight { weight: rest },
        ],
        align: Some(Align::Stretch),
        // Label margin-bottom and helper margin-top are both `$spacing-03`
        // (8px). One row gap covers both Carbon numbers.
        row_spacing: Some(t(SPACING_03)),
        ..Props::default()
    };
    // The grid itself does not paint. A rail fill here would sit behind the
    // label (and helper), which Carbon draws on the page, not on the track.
    // Radius without a fill is `Outcome::Silent` in the paint pass, so the
    // grid stays Empty — a position for its children — and the track cell
    // carries `$border-subtle` itself.

    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    caption.props.span = Some(full_row_span());

    let fill = bar_cell("fill", height, Some(ACCENT_PRIMARY));
    let track = bar_cell("track", height, Some(BORDER_SUBTLE));

    let mut children = vec![caption, fill, track];
    if let Some(helper) = helper {
        let mut helper_node = text("helper", helper);
        helper_node
            .props
            .tokens
            .insert("foreground".into(), t(TEXT_MUTED));
        helper_node.props.span = Some(full_row_span());
        children.push(helper_node);
    }

    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(bar_props)
        .with_children(children)
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(MIN_TRACK_WIDTH),
                max: None,
                priority: 0,
            },
            ..Constraints::default()
        });
    node.semantics = Semantics {
        role: Some(Role::Progress),
        label: Some(label),
        value: Some(format!("{:.0}%", done * 100.0)),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::{
        BAR_HEIGHT, BAR_HEIGHT_SM, MIN_TRACK_WIDTH, MIN_WEIGHT, progress, progress_sm,
        progress_with_helper,
    };
    use crate::component::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{NodeKind, Props, Registry, Role, TrackSize, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .unwrap_or_else(|| panic!("no direct child keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn fill_weight(node: &ViewNode) -> f32 {
        match node.props.columns.first() {
            Some(TrackSize::Weight { weight }) => *weight,
            other => panic!("fill column must be a Weight, got {other:?}"),
        }
    }

    #[test]
    fn default_track_is_eight_px() {
        let node = progress("p", "Rebuild", 0.62);
        let fill = child(&node, "fill");
        assert_eq!(fill.constraints.vertical.min, Some(BAR_HEIGHT));
        assert_eq!(fill.constraints.vertical.max, Some(BAR_HEIGHT));
        assert_eq!(BAR_HEIGHT, 8.0);
        let track = child(&node, "track");
        assert_eq!(track.constraints.vertical.min, Some(8.0));
        assert_eq!(track.constraints.vertical.max, Some(8.0));
    }

    #[test]
    fn small_track_is_four_px() {
        let node = progress_sm("p", "Rebuild", 0.62);
        let fill = child(&node, "fill");
        assert_eq!(fill.constraints.vertical.min, Some(BAR_HEIGHT_SM));
        assert_eq!(fill.constraints.vertical.max, Some(BAR_HEIGHT_SM));
        assert_eq!(BAR_HEIGHT_SM, 4.0);
        assert_eq!(node.semantics.role, Some(Role::Progress));
    }

    #[test]
    fn fill_and_track_keys_are_present() {
        let node = progress("p", "Rebuild", 0.62);
        assert_eq!(node.kind, NodeKind::Grid);
        let _ = child(&node, "fill");
        let _ = child(&node, "track");
        let _ = child(&node, "label");
    }

    #[test]
    fn role_is_progress_and_the_label_is_inside_the_function() {
        let node = progress("p", "Rebuild", 0.62);
        assert_eq!(node.semantics.role, Some(Role::Progress));
        assert_eq!(node.semantics.label.as_deref(), Some("Rebuild"));
        assert_eq!(child(&node, "label").props.text.as_deref(), Some("Rebuild"));
        assert_eq!(
            token(child(&node, "label"), "foreground"),
            Some(TEXT_PRIMARY)
        );
    }

    #[test]
    fn a_value_of_zero_point_six_two_produces_a_non_zero_fill_weight() {
        let node = progress("p", "Rebuild", 0.62);
        let weight = fill_weight(&node);
        assert!(
            weight > 0.0,
            "a 0.62 bar produced fill weight {weight}, which is zero"
        );
        assert!(
            (weight - 0.62).abs() < f32::EPSILON,
            "fill weight {weight} is not 0.62"
        );
        let fill = child(&node, "fill");
        assert!(
            fill.constraints.horizontal.max != Some(0.0),
            "fill is pinned to zero width; dropping the swatch horizontal pin is the fix"
        );
        assert_eq!(
            fill.constraints.horizontal.min, None,
            "fill must take its width from the weighted column, not a min pin"
        );
    }

    #[test]
    fn carbon_colours_and_min_width() {
        let node = progress("p", "Rebuild", 0.62);
        assert_eq!(
            token(child(&node, "fill"), "background"),
            Some(ACCENT_PRIMARY)
        );
        assert_eq!(
            token(child(&node, "track"), "background"),
            Some(BORDER_SUBTLE)
        );
        assert_eq!(node.constraints.horizontal.min, Some(MIN_TRACK_WIDTH));
        assert_eq!(MIN_TRACK_WIDTH, 48.0);
        assert!(!node.props.tokens.contains_key("background"));
        assert!(!node.props.tokens.contains_key("border"));
    }

    #[test]
    fn nan_and_out_of_range_normalise_once() {
        assert_eq!(
            progress("p", "Rebuild", f32::NAN)
                .semantics
                .value
                .as_deref(),
            Some("0%")
        );
        assert_eq!(
            progress("p", "Rebuild", f32::INFINITY)
                .semantics
                .value
                .as_deref(),
            Some("0%")
        );
        assert_eq!(
            progress("p", "Rebuild", -0.5).semantics.value.as_deref(),
            Some("0%")
        );
        assert_eq!(
            progress("p", "Rebuild", 1.5).semantics.value.as_deref(),
            Some("100%")
        );
        assert_eq!(fill_weight(&progress("p", "Rebuild", 0.0)), MIN_WEIGHT);
        let full = fill_weight(&progress("p", "Rebuild", 1.0));
        assert!(full >= 1.0 - MIN_WEIGHT);
    }

    #[test]
    fn helper_is_optional_and_does_not_change_the_three_arg_signature() {
        let node = progress_with_helper("p", "Rebuild", 0.5, "About a minute left");
        assert_eq!(node.semantics.role, Some(Role::Progress));
        let helper = child(&node, "helper");
        assert_eq!(helper.props.text.as_deref(), Some("About a minute left"));
        assert_eq!(token(helper, "foreground"), Some(TEXT_MUTED));
        let _ = child(&node, "fill");
        let _ = child(&node, "track");
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Class 2's exact suspect: `fill` and `track` are childless swatches
    /// (`bar_cell`'s own doc), the same shape as the Accordion divider that
    /// petrified 0px wide. This module's own fix predates this audit (see
    /// `a_value_of_zero_point_six_two_produces_a_non_zero_fill_weight`
    /// above), but that test reads the *weight* prop, never a placed rect —
    /// this is the frame-level check the audit plan requires, at 0%, a mid
    /// value, and 100%, where the empty side's weight is
    /// [`MIN_WEIGHT`] (0.001), the case most likely to round away to a
    /// zero-pixel placement.
    #[test]
    fn fill_and_track_place_with_a_real_nonzero_rect_at_every_value() {
        for (label, value) in [("0%", 0.0), ("62%", 0.62), ("100%", 1.0)] {
            let frame = petrify_lone(progress("p", "Rebuild", value));
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            let fill = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/fill"))
                .unwrap_or_else(|| panic!("{label}: no placement ending /fill"));
            let track = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/track"))
                .unwrap_or_else(|| panic!("{label}: no placement ending /track"));
            assert!(
                fill.rect.w > 0.0 && fill.rect.h > 0.0,
                "{label}: fill placed with a degenerate rect {:?}",
                fill.rect
            );
            assert!(
                track.rect.w > 0.0 && track.rect.h > 0.0,
                "{label}: track placed with a degenerate rect {:?}",
                track.rect
            );
            for p in &frame.placements {
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
                    p.id
                );
                if let Some(parent_idx) = p.parent {
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check E: the label and helper text against the page ground the bar
    /// sits on (the bar itself binds no `background` — its own doc says the
    /// grid stays `Empty`), in both themes.
    #[test]
    fn label_and_helper_text_clear_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            let node = progress_with_helper("p", "Rebuild", 0.5, "About a minute left");
            for label_key in ["label", "helper"] {
                let label = child(&node, label_key);
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label text binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label_key} at {ratio:.2}:1 against page ground fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }
}
