//! Carbon File uploader (slice-b). OS drop and the native picker are T140.
//!
//! Anatomy (usage page + `_file-uploader.scss`), in order:
//!
//! 1. **Heading** — `.cds--file--label`, `heading-compact-01`,
//!    [`TEXT_PRIMARY`]: the constructor's `label`.
//! 2. **Description** — `.cds--label-description`, `body-compact-01`,
//!    `$text-secondary` ([`TEXT_MUTED`]): the size and format limits.
//!    Optional; [`file_uploader`] omits it, [`file_uploader_with`] takes it.
//! 3. **Drop zone** — `.cds--file__drop-container` inside
//!    `.cds--file-browse-btn`: a [`Role::Button`] [`ZONE_WIDTH`] wide and
//!    [`DROP_HEIGHT`] tall, padded [`SPACING_05`], holding one prompt run
//!    in [`LINK_PRIMARY`] reading `"Drop files here or click to upload"`.
//!    Content is **centred** on both axes, which is a departure from
//!    Carbon recorded in the third divergence below.
//! 4. [`file_uploader_item`] and its two siblings — one selected-file row,
//!    [`ITEM_WIDTH`] wide, [`ITEM_HEIGHT`] tall, `$layer` fill, laid out
//!    `1fr auto` so the name starts at the leading edge and the state
//!    container is flush right. Carbon's state container is
//!    [`STATE_BOX`] wide with [`SPACING_04`] of trailing pad, and holds
//!    **exactly one** control per status: a loader while uploading, a
//!    `CheckmarkFilled` when complete, an `ErrorFilled` when invalid, and a
//!    `Close` button when the row is editable.
//!
//! # Three divergences, all recorded rather than faked
//!
//! **The dashed border.** Carbon draws `border: 1px dashed $border-strong`
//! (MEASURED `_file-uploader.scss:424`). This painter has one border slot
//! and [`crate::draw::Stroke`] carries a width and a colour and no dash
//! pattern, so a dashed edge is not expressible: adding one is a
//! `draw::VERSION` v1 → v2 change plus a `paint.rs` change, not a component
//! edit. Drawing the dashes as a row of child rectangles is not open either
//! — [`crate::tree::NodeKind::Canvas`] places no child, so a canvas cannot
//! *contain* the prompt, and there is no absolute positioning outside the
//! anchored-surface machinery. The zone therefore keeps a **solid**
//! [`BORDER_SUBTLE`], which is also what
//! `component::tests::containers_take_a_tone_and_controls_take_an_edge`
//! requires of every edge in this library.
//!
//! **The prompt is centred in the zone; Carbon's is in the corner.**
//! MEASURED `_file-uploader.scss:415`: `.cds--file__drop-container` is
//! `display: flex; align-items: flex-start; justify-content:
//! space-between; padding: $spacing-05; block-size: 96px`. One child under
//! `space-between` sits at the main-axis start, so Carbon's own prompt is
//! in the box's top-left corner and 60 of the 96 units below it are empty.
//! Round 4, row 12, is the operator on that picture: *"file uploader: the
//! text needs to be centered in the ui"*. The zone therefore takes
//! [`Align::Center`] across and [`Justify::Center`] along, and the measured
//! offset it closes is 40.1 units horizontally and 23 vertically
//! (`shots::tests::the_drop_zone_is_a_carbon_box_and_a_file_row_can_be_removed`,
//! which now holds the two centres together to within a point).
//!
//! This is an **open** departure. Carbon's corner alignment is what leaves
//! room for the second element its drop container is built to take — the
//! `space-between` is there for a trailing control, not for a lone label —
//! so a later form of this component that grows one has to revisit the
//! pair rather than keep centring around it.
//!
//! **The prompt's underline is at rest, not on hover.** Carbon underlines
//! `.cds--file-browse-btn` on hover. The hover flag belongs to the placement
//! the pointer is over, which here is the **zone**, not the text leaf inside
//! it — `link.rs`'s own doc walks the same trap — so an `underline@hover`
//! slot on the prompt would never resolve. The prompt binds a resting
//! `underline` instead, the [`super::link_inline`] form, which is also the
//! form to reach for when a link must be found without a pointer.
//!
//! # No file bytes, and no picker
//!
//! Nothing here stores a file, opens a dialog, or hears an OS drop. The zone
//! declares [`Interaction::Click`] because activating it is what opens a
//! picker; the picker itself needs a channel from `Page` to the host that
//! does not exist yet (see this row's Agent Note). The zone does **not**
//! declare [`Interaction::Drag`]: an OS file-drop is a windowing event, not
//! pointer capture.

use super::field::warning_helper;
use super::icon::{IconBox, IconMark, IconTone, icon_in};
use super::kit;
use super::pad;
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    BORDER_STRONG, LAYER_HOVER, LINK_PRIMARY, SPACING_03, SPACING_04, SPACING_05, SURFACE_BASE,
    SURFACE_RAISED, TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT, TYPOGRAPHY_HEADING_SM, TYPOGRAPHY_LABEL,
    t,
};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    AxisConstraint, Behaviour, Constraints, FocusFigure, Intent, Interaction, Justify, Key,
    NodeKind, Phase, Role, ViewNode,
};

/// Spec 010: drop zone / remove — one shot.
const ACTIVATES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Activate,
    phase: Phase::OnRelease,
};

/// MEASURED `_file-uploader.scss:425` drop-container `block-size`.
const DROP_HEIGHT: f32 = 96.0;
/// MEASURED `_file-uploader.scss:70` — `.cds--file-browse-btn` is
/// `inline-size: 100%` capped at `max-inline-size: 320px`, and the
/// reference capture (`ignored/carbon-ref/shots/12-file-uploader.png`,
/// device x 64..703 at dpr 2) renders it at exactly that cap.
const ZONE_WIDTH: f32 = 320.0;
/// MEASURED `_file-uploader.scss:155` `.cds--file__selected-file`
/// `max-inline-size`. **This was 288 until 2026-09-05**, from the docs'
/// `18rem`; T070 gives the SCSS the last word and the reference renders the
/// row at the same 320 the zone above it uses, which is why the two boxes
/// line up in Carbon's picture and did not in ours.
const ITEM_WIDTH: f32 = 320.0;
/// MEASURED default selected-file `min-block-size` (`$spacing-09`).
const ITEM_HEIGHT: f32 = 48.0;
/// MEASURED `_file-uploader.scss:390` — `.cds--file-close` and
/// `.cds--file-complete` are `$spacing-06` (24) on both axes, and
/// `.cds--file__state-container` is `min-inline-size: 1.5rem` (24).
const STATE_BOX: f32 = 24.0;
/// Small loading disc: the still ring standing in for Carbon's spinner.
const MARK: f32 = 16.0;

const _: () = assert!(DROP_HEIGHT == 96.0);
const _: () = assert!(ZONE_WIDTH == 320.0);
const _: () = assert!(ITEM_WIDTH == 320.0);
const _: () = assert!(ITEM_HEIGHT == 48.0);
const _: () = assert!(STATE_BOX == 24.0);

/// Carbon's `.cds--file-browse-btn` copy, and the operator's own complaint:
/// the resting prompt has to say that a click opens a picker, because a
/// drop target that only says "drop files here" reads as a tile.
const PROMPT: &str = "Drop files here or click to upload";

const ZONE_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];
/// The remove control answers to the same three a button does.
const REMOVE_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Drop-zone file uploader. `label` is the heading and the button name.
///
/// See [`file_uploader_with`] for the described form.
pub fn file_uploader(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    build_uploader(key, label, None)
}

/// [`file_uploader`] with Carbon's `.cds--label-description` line under the
/// heading — the place the size and format limits go.
pub fn file_uploader_with(
    key: impl Into<Key>,
    label: impl Into<String>,
    description: impl Into<String>,
) -> ViewNode {
    build_uploader(key, label, Some(description.into()))
}

fn build_uploader(
    key: impl Into<Key>,
    label: impl Into<String>,
    description: Option<String>,
) -> ViewNode {
    let label = label.into();
    let mut heading = text("heading", label.clone());
    heading.props.style = Some(t(TYPOGRAPHY_HEADING_SM));
    heading
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let mut prompt = text("prompt", PROMPT);
    prompt.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    prompt
        .props
        .tokens
        .insert("foreground".into(), t(LINK_PRIMARY));
    prompt
        .props
        .tokens
        .insert("underline".into(), t(LINK_PRIMARY));

    let mut zone = stack("zone", Axis::Vertical, None, vec![prompt]);
    // **Centred on both axes, against Carbon, on the operator's
    // instruction.** See the module doc's third divergence for the
    // measurement and the argument; the short version is that Carbon's
    // `align-items: flex-start` leaves 60 of the 96 units empty under one
    // line of text, and round 4 row 12 reads *"the text needs to be
    // centered in the ui"*.
    //
    // `Align` is the cross axis and `Justify` the main one, and this stack
    // is vertical, so the pair is "centre horizontally, centre vertically"
    // in that order. Both are safe here only because `zone.constraints`
    // pins **both** extents four lines down: `Align::Center` on an
    // unconstrained box is what collapsed the zone to the 121 points the
    // operator photographed in round 3, and a centred prompt inside a
    // collapsed box is centred in nothing.
    zone.props.align = Some(Align::Center);
    zone.props.justify = Some(Justify::Center);
    zone.props.padding = Some(pad(SPACING_05, SPACING_05));
    zone.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    zone.props.tokens.insert("border".into(), t(BORDER_STRONG));
    zone.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    zone.constraints = pin(ZONE_WIDTH, DROP_HEIGHT);
    let zone = zone
        .interactive(Role::Button, label.clone(), ZONE_INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .owning_its_text();

    let mut children = vec![heading];
    if let Some(description) = description {
        children.push(kit::description("description", &description));
    }
    children.push(zone);

    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), children);
    node.semantics.label = Some(label);
    node
}

/// One selected-file row. `complete` chooses the check vs the loading ring.
///
/// No file bytes. The row is not a drop target. See
/// [`file_uploader_item_edit`] for the form that carries a remove control
/// and [`file_uploader_item_invalid`] for the one that carries an error.
pub fn file_uploader_item(
    key: impl Into<Key>,
    name: impl Into<String>,
    complete: bool,
) -> ViewNode {
    let mark = if complete {
        // Carbon `.cds--file-complete { fill: $interactive }` — the accent,
        // not a disc with a tick punched out of it. `CheckmarkFilled` is the
        // glyph the reference draws (`12-file-uploader.png`: a 14-unit blue
        // ring with a light tick, flush right).
        icon_in(
            "mark",
            IconMark::CheckmarkFilled,
            IconBox::Glyph,
            IconTone::Accent,
        )
    } else {
        // A still ring standing in for `.cds--file-loading`'s spinner. It
        // keeps its `border` binding on purpose: this shape has no fill at
        // all, so the ring *is* the mark, and
        // `component::tests::containers_take_a_tone_and_controls_take_an_edge`
        // names `fu-f1/mark` as one of the library's measured edges.
        // FR-022: the ring is a disc at whatever size it ships, so it says
        // `CornerRole::Pill` rather than leaning on the half-edge clause,
        // which `Floating`'s 8 clears at today's 16 units and would stop
        // clearing the day the ring grew.
        swatch(
            "mark",
            MARK,
            MARK,
            None,
            Some(BORDER_STRONG),
            Some(corner_for(CornerRole::Pill, MARK)),
        )
    };
    let status = if complete { "complete" } else { "uploading" };
    item_row(key, name, mark, status, None)
}

/// The editable form: Carbon's `status="edit"`, whose state container is a
/// `Close` button that removes the row.
///
/// The glyph is the whole control and the word lives in `Semantics.label`,
/// which is this library's settled shape for an affordance that used to be
/// spelled out — see
/// `component::tests::no_shipped_component_spells_an_icon_as_its_name`.
pub fn file_uploader_item_edit(key: impl Into<Key>, name: impl Into<String>) -> ViewNode {
    let name = name.into();
    let mut remove = stack(
        "remove",
        Axis::Horizontal,
        None,
        vec![icon_in(
            "glyph",
            IconMark::Close,
            IconBox::Glyph,
            IconTone::Primary,
        )],
    );
    remove.props.align = Some(Align::Center);
    remove.props.justify = Some(Justify::Center);
    // A resting fill as well as a hover one. Carbon's `.cds--file-close` is
    // `background-color: transparent`; the row's own `$layer` is what
    // "transparent" means here, and a state-decorated slot with no resting
    // binding is a placement that declares content and paints nothing —
    // `PaintReport::silent`, which is exactly what the catalog's
    // `every_built_page_paints_with_nothing_silent` caught on the first run
    // of this control.
    remove
        .props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    remove
        .props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let remove = remove
        .with_constraints(pin(STATE_BOX, STATE_BOX))
        .interactive(Role::Button, format!("Remove {name}"), REMOVE_INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .owning_its_text()
        // A button: `Sides`, per the operator's rule. See `component::button`.
        .with_focus_figure(FocusFigure::Sides);
    item_row(key, name, remove, "ready", None)
}

/// The invalid form: Carbon's `.cds--file__selected-file--invalid`, an
/// `ErrorFilled` mark plus a `.cds--form-requirement` line under the name.
///
/// The message is [`TEXT_PRIMARY`] and not a red ink, because
/// [`super::tokens::SUPPORT_ERROR`] measures 3.79:1 on this row's own fill
/// in the dark theme and AA wants 4.5 for text. The kind travels on the
/// glyph's silhouette — a ring with a slash, which no other state in this
/// component draws — and on `Semantics.value`.
pub fn file_uploader_item_invalid(
    key: impl Into<Key>,
    name: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    let mark = icon_in(
        "mark",
        IconMark::ErrorFilled,
        IconBox::Glyph,
        IconTone::Primary,
    );
    let mut requirement = text("message", message.into());
    requirement.props.style = Some(t(TYPOGRAPHY_LABEL));
    requirement
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    item_row(key, name, mark, "invalid", Some(requirement))
}

/// The warning form: a [`IconMark::WarningFilled`] mark plus a warning
/// helper line under the name.
///
/// Colour is not the only channel. The kind travels on the glyph's
/// silhouette (a ring around a bang, which no other state in this
/// component draws) and on [`warning_helper`]'s `Warning: {message}` line.
/// The row itself takes no four-sided container border.
#[must_use]
pub fn file_uploader_item_warning(
    key: impl Into<Key>,
    name: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    let mark = icon_in(
        "mark",
        IconMark::WarningFilled,
        IconBox::Glyph,
        IconTone::Primary,
    );
    item_row(key, name, mark, "warning", Some(warning_helper(message)))
}

/// The shared row: `1fr auto`, the name at the leading edge and one state
/// control flush right.
///
/// # Why the control is a direct child and not inside a `state` wrapper
///
/// Carbon's `.cds--file__state-container` is a 24-wide box with
/// `padding-inline-end: 12px`, which puts its 14-unit glyph **17** units in
/// from the row's trailing edge (MEASURED on
/// `ignored/carbon-ref/shots/12-file-uploader.png`: glyph right edge at
/// device x 669, row right edge at 703, dpr 2). Ours is a 16-unit mark
/// against the row's own `$spacing-05` trailing pad, so it lands at **16**
/// — one logical unit short of Carbon's, and a direct child of the row so
/// the mark keeps the canonical id `<row>/mark` that
/// `component::tests::containers_take_a_tone_and_controls_take_an_edge`
/// names. A wrapper would buy the missing unit and cost that id.
///
/// The invalid and warning forms nest, because each has a second row
/// (`.cds--form-requirement`, or the shared warning helper) under the name
/// and needs a `line` to hold the first one.
fn item_row(
    key: impl Into<Key>,
    name: impl Into<String>,
    control: ViewNode,
    status: &str,
    helper: Option<ViewNode>,
) -> ViewNode {
    let key = key.into();
    let name = name.into();
    let mut filename = text("name", name.clone());
    filename.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    filename
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    // Carbon's grid is `1fr auto`; a stack has no weighted track, so the
    // free space is a child, the same answer `accordion_item`'s header
    // reached for the chevron at its trailing edge.
    let line_children = vec![filename, ViewNode::new(NodeKind::Spacer, "spacer"), control];

    let mut node = if let Some(helper) = helper {
        let mut line = stack("line", Axis::Horizontal, Some(SPACING_03), line_children);
        line.props.align = Some(Align::Center);
        line.props.justify = Some(Justify::SpaceBetween);
        // `gap: 12px 0` between a selected file's own grid rows.
        stack(key, Axis::Vertical, Some(SPACING_04), vec![line, helper])
    } else {
        let mut row = stack(key, Axis::Horizontal, Some(SPACING_03), line_children);
        row.props.align = Some(Align::Center);
        row.props.justify = Some(Justify::SpaceBetween);
        row
    };
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    // MEASURED `_file-uploader.scss`: `.cds--file__selected-file` is
    // `background-color: $layer` (→ `SURFACE_RAISED`) and nothing else —
    // no border. Only the drop zone (`.cds--file__drop-container`) is
    // bordered. A `border` binding here drew an edge Carbon never draws:
    // this row is a "list strip"-shaped container (no interaction of its
    // own), and the fill alone is its whole boundary, the same class as
    // `list_row` and `tab` (see `containers_take_a_tone_and_controls_take_an_edge`'s
    // own doc for why a strip segment does not get an edge of its own).
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.constraints.horizontal.min = Some(ITEM_WIDTH);
    node.constraints.horizontal.max = Some(ITEM_WIDTH);
    node.constraints.vertical.min = Some(ITEM_HEIGHT);
    node.semantics.label = Some(name);
    node.semantics.value = Some(status.into());
    node
}

fn pin(w: f32, h: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(w),
            max: Some(w),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DROP_HEIGHT, ITEM_HEIGHT, ITEM_WIDTH, MARK, PROMPT, STATE_BOX, ZONE_WIDTH, file_uploader,
        file_uploader_item, file_uploader_item_edit, file_uploader_item_invalid,
        file_uploader_item_warning, file_uploader_with,
    };
    use crate::component::tokens::{
        ACCENT_PRIMARY, BORDER_STRONG, LINK_PRIMARY, SUPPORT_ERROR, TEXT_MUTED,
    };
    use crate::component::{IconMark, IconTone, icon_toned};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Align, Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{
        ColorValue, CornerRole, Theme, ThemeMode, TokenName, TokenValue, corner_for,
        standard_vocabulary,
    };
    use crate::tree::{
        Behaviour, Intent, Interaction, Justify, NodeKind, Phase, Props, Registry, Role, ViewNode,
    };

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn no_drag(node: &ViewNode) {
        assert!(
            !node.interactions.contains(&Interaction::Drag),
            "file uploader `{}` declared Drag; OS drop is T140",
            node.key
        );
        for child in &node.children {
            no_drag(child);
        }
    }

    /// Carbon's zone is a 320 x 96 box. Ours was a 121-wide box with grey
    /// body text on its midline: `zone` set only `vertical.min`, so the box
    /// collapsed to the width of its caption, and `Align::Center` put the
    /// caption in the middle of what was left.
    ///
    /// **The two halves of that are separable, and round 4 separated
    /// them.** What went wrong was the missing horizontal constraint, not
    /// the centring; a centred caption in a box that cannot collapse is
    /// centred in a 320 x 96 box. The constraint assertions below are the
    /// guard that used to be carried by "align is None", and they come
    /// first here for that reason. The alignment is now `Center` on both
    /// axes, on the operator's instruction — see the module doc's third
    /// divergence, and
    /// `shots::tests::the_drop_zone_is_a_carbon_box_and_a_file_row_can_be_removed`
    /// for the picture.
    ///
    /// Falsify by dropping `zone.constraints` back to a lone
    /// `vertical.min`, which fails on the horizontal pin before it reaches
    /// the alignment.
    #[test]
    fn the_drop_zone_is_a_320_by_96_box_with_a_centred_prompt() {
        let node = file_uploader("up", "Upload files");
        assert_eq!(node.semantics.label.as_deref(), Some("Upload files"));
        assert_eq!(
            named(&node, "heading").props.text.as_deref(),
            Some("Upload files")
        );
        assert_eq!(
            named(&node, "heading")
                .props
                .style
                .as_ref()
                .map(|s| s.as_str()),
            Some(crate::component::tokens::TYPOGRAPHY_HEADING_SM),
            "Carbon's `.cds--file--label` is `heading-compact-01`"
        );
        let zone = named(&node, "zone");
        assert_eq!(zone.semantics.role, Some(Role::Button));
        assert_eq!(zone.semantics.label.as_deref(), Some("Upload files"));
        assert!(zone.interactions.contains(&Interaction::Click));
        assert!(zone.interactions.contains(&Interaction::Focus));
        assert!(!zone.interactions.contains(&Interaction::Drag));
        assert_eq!(zone.constraints.horizontal.min, Some(ZONE_WIDTH));
        assert_eq!(zone.constraints.horizontal.max, Some(ZONE_WIDTH));
        assert_eq!(zone.constraints.vertical.min, Some(DROP_HEIGHT));
        assert_eq!(zone.constraints.vertical.max, Some(DROP_HEIGHT));
        assert_eq!(ZONE_WIDTH, 320.0);
        assert_eq!(DROP_HEIGHT, 96.0);
        // Both axes centred, and both extents pinned four lines up. The
        // pin is the precondition: a centred child inside an unconstrained
        // box is what collapsed this zone to 121 points in round 3, so
        // these two assertions are only safe *because* those four hold.
        assert_eq!(
            zone.props.align,
            Some(Align::Center),
            "the drop zone's prompt is centred across the box. Carbon is \
             `align-items: flex-start` (`_file-uploader.scss:421`); this is \
             a recorded departure, see the module doc."
        );
        assert_eq!(
            zone.props.justify,
            Some(Justify::Center),
            "the drop zone's prompt is centred along the box too. Carbon is \
             `justify-content: space-between`, which puts a lone child at \
             the start and leaves 60 of the 96 units empty under it."
        );
        assert_eq!(token(zone, "border"), Some(BORDER_STRONG));

        let prompt = named(zone, "prompt");
        assert_eq!(prompt.props.text.as_deref(), Some(PROMPT));
        assert_eq!(
            prompt.props.text.as_deref(),
            Some("Drop files here or click to upload"),
            "the prompt has to say that a click opens a picker"
        );
        assert_eq!(token(prompt, "foreground"), Some(LINK_PRIMARY));
        assert_eq!(
            token(prompt, "underline"),
            Some(LINK_PRIMARY),
            "a resting underline, not `underline@hover`: the hover flag \
             belongs to the zone placement, so a state slot on this leaf \
             would never resolve"
        );
        assert!(
            token(prompt, "underline@hover").is_none(),
            "the slot that cannot resolve here is not bound"
        );
        no_drag(&node);
    }

    #[test]
    fn the_described_form_adds_carbon_s_label_description_line() {
        let plain = file_uploader("up", "Upload files");
        assert!(
            crate::component::file_uploader::tests::find(&plain, "description").is_none(),
            "the undescribed form draws no description line"
        );
        let described = file_uploader_with("up", "Upload files", "Max 5 MB, .ndjson only");
        let line = named(&described, "description");
        assert_eq!(line.props.text.as_deref(), Some("Max 5 MB, .ndjson only"));
        assert_eq!(token(line, "foreground"), Some(TEXT_MUTED));
    }

    /// One state control per row, flush right, and **no status word**:
    /// Carbon's selected file shows a glyph and nothing else, and ours
    /// printed "complete" beside the mark. The three states draw three
    /// different silhouettes, which is the channel that survives a reader
    /// who cannot use hue.
    #[test]
    fn each_row_state_draws_one_mark_flush_right_and_no_status_word() {
        let done = file_uploader_item("f0", "notes.txt", true);
        assert_eq!(done.semantics.label.as_deref(), Some("notes.txt"));
        assert_eq!(done.semantics.value.as_deref(), Some("complete"));
        assert_eq!(
            named(&done, "name").props.text.as_deref(),
            Some("notes.txt")
        );
        assert!(
            crate::component::file_uploader::tests::find(&done, "status").is_none(),
            "Carbon's file row carries no status word; the glyph and \
             `Semantics.value` carry the state"
        );
        assert_eq!(named(&done, "mark").kind, NodeKind::Canvas);
        assert_eq!(done.props.justify, Some(crate::tree::Justify::SpaceBetween));
        assert!(
            token(&done, "border").is_none(),
            "MEASURED `_file-uploader.scss`: `.cds--file__selected-file` is \
             fill-only, no border; a strip segment does not draw an edge of \
             its own"
        );
        assert_eq!(done.constraints.horizontal.min, Some(ITEM_WIDTH));
        assert_eq!(done.constraints.horizontal.max, Some(ITEM_WIDTH));
        assert_eq!(
            ITEM_WIDTH, 320.0,
            "MEASURED `_file-uploader.scss:155`; the docs' 18rem is the \
             number T070 tells us to ignore"
        );
        assert_eq!(done.constraints.vertical.min, Some(ITEM_HEIGHT));
        assert_eq!(ITEM_HEIGHT, 48.0);
        no_drag(&done);

        let busy = file_uploader_item("f1", "notes.txt", false);
        assert_eq!(busy.semantics.value.as_deref(), Some("uploading"));
        let mark = named(&busy, "mark");
        assert_eq!(
            mark.kind,
            NodeKind::Spacer,
            "the uploading ring is the one mark with no fill, so the border \
             is the whole shape and `fu-f1/mark` stays on this library's \
             measured-edge list"
        );
        assert_eq!(token(mark, "border"), Some(BORDER_STRONG));
        assert_eq!(
            token(mark, "radius"),
            Some(corner_for(CornerRole::Pill, MARK))
        );
        no_drag(&busy);
    }

    /// Carbon's `status="edit"` row: the state container is a `Close`
    /// button that removes the file. The word is in `Semantics.label` and
    /// the eye gets the glyph, which is this library's settled shape for an
    /// affordance that used to be spelled out.
    #[test]
    fn the_editable_row_carries_a_named_remove_button() {
        let row = file_uploader_item_edit("f2", "trace.ndjson");
        assert_eq!(row.semantics.value.as_deref(), Some("ready"));
        let remove = named(&row, "remove");
        assert_eq!(remove.semantics.role, Some(Role::Button));
        assert_eq!(
            remove.semantics.label.as_deref(),
            Some("Remove trace.ndjson"),
            "the accessible name says which file, not just `Remove`"
        );
        assert!(remove.interactions.contains(&Interaction::Click));
        assert!(remove.interactions.contains(&Interaction::Focus));
        assert_eq!(remove.constraints.horizontal.min, Some(STATE_BOX));
        assert_eq!(remove.constraints.vertical.min, Some(STATE_BOX));
        assert_eq!(STATE_BOX, 24.0);
        assert_eq!(named(remove, "glyph").kind, NodeKind::Canvas);
        no_drag(&row);
    }

    /// The invalid row is the only one with a second line, so it is the
    /// only one that nests: `line` holds the name and the mark, and
    /// `.cds--form-requirement` sits under both.
    #[test]
    fn the_invalid_row_carries_a_message_under_its_name() {
        let row = file_uploader_item_invalid("f3", "huge.bin", "File is over 5 MB");
        assert_eq!(row.semantics.value.as_deref(), Some("invalid"));
        assert_eq!(
            named(&row, "message").props.text.as_deref(),
            Some("File is over 5 MB")
        );
        assert_eq!(named(&row, "line").props.axis, Some(Axis::Horizontal));
        assert_eq!(named(&row, "mark").kind, NodeKind::Canvas);
        assert!(
            token(named(&row, "message"), "foreground")
                != Some(crate::component::tokens::SUPPORT_ERROR),
            "`support-error` measures 3.79:1 on this row's fill in the dark \
             theme; AA wants 4.5, so the kind travels on the glyph and not \
             on a red ink this library does not have"
        );
        no_drag(&row);
    }

    /// Colour is not the only channel: the helper says `Warning: {message}`
    /// and sits a WarningFilled mark beside it. The state mark is
    /// WarningFilled too. The row itself is not a four-sided box.
    #[test]
    fn file_uploader_item_warning_says_so_in_words_and_a_glyph() {
        let row = file_uploader_item_warning("f4", "notes.txt", "check the value");
        assert_eq!(row.semantics.value.as_deref(), Some("warning"));
        assert_eq!(named(&row, "line").props.axis, Some(Axis::Horizontal));
        assert!(
            token(&row, "border").is_none(),
            "a selected-file row is a strip, not a boxed well; a four-sided \
             container border is T053's finding"
        );
        assert_ne!(token(&row, "border"), Some(SUPPORT_ERROR));
        assert_ne!(token(&row, "border"), Some(ACCENT_PRIMARY));
        let helper = named(&row, "helper");
        let message = named(helper, "message");
        assert_eq!(message.kind, NodeKind::Text);
        assert!(
            message
                .props
                .text
                .as_deref()
                .is_some_and(|t| t.contains("Warning:")),
            "the helper text must contain `Warning:`"
        );
        assert_eq!(
            message.props.text.as_deref(),
            Some("Warning: check the value")
        );
        let expected = icon_toned("mark", IconMark::WarningFilled, IconTone::Primary);
        let helper_mark = named(helper, "mark");
        assert_eq!(helper_mark.kind, NodeKind::Canvas);
        assert_eq!(
            helper_mark.props.canvas, expected.props.canvas,
            "the helper's glyph is WarningFilled, not ErrorFilled and not a \
             swatch"
        );
        let state_mark = named(named(&row, "line"), "mark");
        assert_eq!(state_mark.kind, NodeKind::Canvas);
        assert_eq!(
            state_mark.props.canvas, expected.props.canvas,
            "the state mark is WarningFilled, matching Carbon's warn row"
        );
        no_drag(&row);
    }

    /// Depth-first lookup that answers `None` instead of panicking, for the
    /// negative halves above.
    fn find<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|child| find(child, key))
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

    /// Check C/D: the drop zone and both selected-file states place with a
    /// real rect, none of them outside their own row.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("zone", file_uploader("up", "Upload files")),
            ("item-complete", file_uploader_item("f0", "notes.txt", true)),
            (
                "item-uploading",
                file_uploader_item("f1", "notes.txt", false),
            ),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
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

    /// Check F: the drop zone declares `Focus` and is reachable when
    /// enabled, and not when disabled. `file_uploader_item` declares no
    /// interaction of its own (no delete/retry control in this anatomy), so
    /// there is no positive case for it.
    #[test]
    fn the_drop_zone_is_reachable_unless_disabled() {
        let frame = petrify_lone(file_uploader("up", "Upload files"));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let zone = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/zone"))
            .expect("the zone is placed");
        assert!(
            focus.order().iter().any(|o| o == &zone.id),
            "the drop zone declares Focus but is not in focus order"
        );

        let disabled_frame = petrify_lone(crate::component::disabled(file_uploader(
            "up",
            "Upload files",
        )));
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_zone = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/zone"))
            .expect("the disabled zone is placed");
        assert!(
            !disabled_focus
                .order()
                .iter()
                .any(|o| o == &disabled_zone.id),
            "a disabled drop zone must not be reachable"
        );
    }

    /// Check E: the zone's prompt (against the page ground it sits on,
    /// `surface.base`) and a selected-file row's name/status text (against
    /// its own resting `surface.raised` fill), read through
    /// `Props.opacity`, in both themes.
    #[test]
    fn text_clears_aa_contrast_against_its_own_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let page_bg = color(&theme, SURFACE_BASE);
            let uploader = file_uploader("up", "Upload files");
            let prompt = named(&uploader, "prompt");
            let fg_name = prompt
                .props
                .tokens
                .get("foreground")
                .expect("prompt text binds a foreground");
            let opacity = prompt.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str()).faded(opacity).over(page_bg);
            let ratio = fg.contrast_ratio(page_bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "prompt at {ratio:.2}:1 against {SURFACE_BASE} fails AA {MIN_TEXT_CONTRAST}:1"
            );

            for complete in [true, false] {
                let item = file_uploader_item("f0", "notes.txt", complete);
                let bg_name = item
                    .props
                    .tokens
                    .get("background")
                    .expect("item binds a resting background");
                let bg = color(&theme, bg_name.as_str());
                {
                    let label_key = "name";
                    let label = named(&item, label_key);
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
                        "complete={complete} {label_key} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                        bg_name.as_str()
                    );
                }
            }
        }
    }

    /// Spec 010: the drop zone declares Activate / OnRelease.
    ///
    /// Falsified by dropping `.with_behaviour(...)` from the zone builder:
    ///
    /// ```text
    /// file_uploader zone declared behaviour None
    /// ```
    #[test]
    fn file_uploader_zone_declares_activate_on_release() {
        let node = file_uploader("up", "Upload files");
        assert_eq!(
            named(&node, "zone").behaviour,
            Some(Behaviour {
                intent: Intent::Activate,
                phase: Phase::OnRelease,
            }),
            "file_uploader zone declared behaviour {:?}",
            named(&node, "zone").behaviour
        );
    }
}
