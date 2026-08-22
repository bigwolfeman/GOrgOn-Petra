//! Kind-specific node parameters.
//!
//! `props` is deliberately one flat bag of optional plain values rather than a
//! Rust enum: the wire form must be the shape a Lua table produces
//! (`contracts/view-tree.md`, FR-011), and a table has no tag. Coherence
//! between `kind` and `props` is enforced once, at tree acceptance
//! ([`crate::tree::validate`]), so container code reads through the typed
//! views below and never re-checks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geom::{Align, Axis, Insets};

/// How a text node handles content it cannot fit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextWrap {
    /// Wrap at word boundaries, then clip.
    #[default]
    Wrap,
    /// Single line, ellipsis at the truncation point.
    Ellipsis,
    /// Single line, hard clip with no marker.
    Clip,
}

impl TextWrap {
    /// The policy's wire name.
    ///
    /// The frame digest hashes this string (`contracts/frame-identity.md` §3),
    /// so it is part of a serialization format, not a debug label: changing a
    /// name here changes every digest and needs the domain-prefix bump that
    /// [`crate::frame::digest::DOMAIN`] documents. The names are the same ones
    /// serde emits, and a test below pins that.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wrap => "wrap",
            Self::Ellipsis => "ellipsis",
            Self::Clip => "clip",
        }
    }
}

/// How one grid track is sized.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum TrackSize {
    /// A fixed logical extent.
    Fixed {
        /// The extent.
        value: f32,
    },
    /// A share of the leftover space, proportional to `weight`.
    Weight {
        /// Relative share. Non-positive weights are a tree-acceptance error.
        weight: f32,
    },
    /// The track's own content extent, probed with `Unspecified`.
    FitContent,
}

/// Which surface layer an overlay lives on. Higher layers paint later.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    /// Anchored popup (menu, completion list).
    Popup,
    /// Transient message, does not take focus.
    Toast,
    /// Focus-trapping dialog.
    Modal,
    /// A frame-wide seat above everything (drag ghost, command palette scrim).
    FrameWide,
}

impl Layer {
    /// The base z-order for this layer. Nodes inside a layer order by
    /// `props.z`, then by child order.
    #[must_use]
    pub fn base_z(self) -> i32 {
        match self {
            Self::Popup => 1_000,
            Self::Toast => 2_000,
            Self::Modal => 3_000,
            Self::FrameWide => 4_000,
        }
    }
}

/// Where an overlay surface attaches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum Anchor {
    /// Attached to another node's rect, by that node's key-path id.
    Node {
        /// Semantic id of the anchor node.
        id: String,
        /// Which edge of the anchor the surface prefers.
        edge: Edge,
    },
    /// Attached to a point in the viewport.
    Point {
        /// Horizontal position, logical units.
        x: f32,
        /// Vertical position, logical units.
        y: f32,
    },
    /// Centred in the viewport.
    Viewport,
}

/// Preferred side of an anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    /// Above the anchor.
    Top,
    /// Below the anchor.
    Bottom,
    /// Left of the anchor.
    Left,
    /// Right of the anchor.
    Right,
}

/// What an overlay does when its preferred placement does not fit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClampRule {
    /// Try the opposite edge, then shrink.
    #[default]
    Flip,
    /// Keep the edge, reduce the extent to fit.
    Shrink,
    /// Keep the extent, make the surface scroll.
    Scroll,
}

/// How an overlay treats input aimed past it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputPolicy {
    /// Swallow everything outside the surface; focus is trapped inside.
    Block,
    /// Let input through to what is underneath.
    Passthrough,
    /// Let input through, and close on the first outside press.
    #[default]
    DismissOutside,
}

/// Every kind-specific parameter a built-in node can carry.
///
/// Absent fields serialize away, so the JSON of a plain stack is three keys,
/// not thirty.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Props {
    /// Main axis for `stack`, scrolling axis for `scroll`, run direction for
    /// `separator`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<Axis>,
    /// Gap between stack children, reserved before distribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f32>,
    /// Cross-axis alignment of children.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    /// Grid column tracks, leading to trailing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<TrackSize>,
    /// Grid row tracks, leading to trailing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<TrackSize>,
    /// Gap between grid columns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_spacing: Option<f32>,
    /// Gap between grid rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_spacing: Option<f32>,
    /// Extra logical extent materialized past each end of a scroll viewport.
    ///
    /// Declared on the **`scroll`**, which is what has a viewport and an
    /// offset. A `collection` inside one reads its ancestor's value off the
    /// scroll context the walk carries (`layout::ScrollFrame`), and tree
    /// acceptance refuses `overscan` on a `collection` that has a `scroll`
    /// ancestor rather than silently ignoring it
    /// ([`crate::tree::Violation::ScrollParamOwnedByAncestor`]).
    ///
    /// A `collection` with no `scroll` ancestor keeps its own: nothing can
    /// scroll it, so nothing else can own the value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overscan: Option<f32>,
    /// Total row count of a `collection`, including unmaterialized rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<usize>,
    /// Name of the store-side row source a `collection` reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Estimated main-axis extent of one collection row, used to size the
    /// scroll range before rows are materialized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_extent: Option<f32>,
    /// Text content for `text` and initial content for `input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Truncation policy for `text`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<TextWrap>,
    /// Maximum rendered lines before truncation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<usize>,
    /// Typography token name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Image source identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Placeholder text for `input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Surface layer for `surface`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
    /// Anchor for `surface`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
    /// Off-screen rule for `surface`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clamp: Option<ClampRule>,
    /// Outside-input rule for `surface`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_policy: Option<InputPolicy>,
    /// Registered painter name for `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_kind: Option<String>,
    /// Explicit z-order within the node's layer. Ties break on child order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<i32>,
    /// Paint opacity in `[0, 1]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    /// Content insets from this node's own edges. Honoured only by container
    /// kinds (`NodeKind::is_container`) — a leaf has no children to inset,
    /// and `padding` on a leaf is a tree-acceptance violation rather than a
    /// silently ignored declaration
    /// ([`crate::tree::Violation::PaddingOnLeafKind`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<Insets>,
    /// Token references by role name (`background`, `foreground`, `border`, …).
    ///
    /// Values are token names, never literal styles: FR-013's gate reads this
    /// map and rejects anything that parses as a colour or a number.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tokens: BTreeMap<String, String>,
}

/// Resolved `stack` parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackProps {
    /// Distribution axis.
    pub axis: Axis,
    /// Gap reserved between adjacent children.
    pub spacing: f32,
    /// Cross-axis alignment.
    pub align: Align,
}

/// Resolved `grid` parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct GridProps {
    /// Column tracks.
    pub columns: Vec<TrackSize>,
    /// Row tracks. Empty means rows are implicit and `FitContent`.
    pub rows: Vec<TrackSize>,
    /// Gap between columns.
    pub column_spacing: f32,
    /// Gap between rows.
    pub row_spacing: f32,
    /// Cross-axis alignment inside a cell.
    pub align: Align,
}

/// Resolved `scroll` parameters.
///
/// Read by the `scroll` itself and, through the scroll context the layout
/// walk carries, by any `collection` inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollProps {
    /// The axis that scrolls. The other axis passes the parent's proposal
    /// through unchanged.
    pub axis: Axis,
    /// Extra logical extent a virtualized child materializes past each end
    /// of the viewport.
    pub overscan: f32,
}

/// Resolved `collection` parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectionProps {
    /// Rows in the whole collection, materialized or not.
    pub total_count: usize,
    /// Store-side row source name.
    pub source: String,
    /// Estimated extent of one row along the scrolling axis.
    pub estimated_extent: f32,
}

/// Resolved `text` parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct TextProps<'a> {
    /// The content.
    pub text: &'a str,
    /// Truncation policy.
    pub wrap: TextWrap,
    /// Line cap, or `None` for unlimited.
    pub max_lines: Option<usize>,
    /// Typography token name, or `None` for the theme's body style.
    pub style: Option<&'a str>,
}

/// Resolved `surface` parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceProps<'a> {
    /// Which layer.
    pub layer: Layer,
    /// Where it attaches.
    pub anchor: &'a Anchor,
    /// What it does when it does not fit.
    pub clamp: ClampRule,
    /// What it does with outside input.
    pub input_policy: InputPolicy,
}

/// Default gap between stack children when none is declared.
pub const DEFAULT_SPACING: f32 = 0.0;
/// Default overscan for a scroll container, in logical units.
pub const DEFAULT_OVERSCAN: f32 = 64.0;
/// Fallback row extent when a collection declares none.
pub const DEFAULT_ROW_EXTENT: f32 = 24.0;

impl Props {
    /// Resolved `stack` parameters. Valid for any node that passed acceptance
    /// as a `stack`; defaults are the documented ones, never a guess about a
    /// missing required field.
    #[must_use]
    pub fn stack(&self) -> StackProps {
        StackProps {
            axis: self.axis.unwrap_or(Axis::Vertical),
            spacing: self.spacing.unwrap_or(DEFAULT_SPACING),
            align: self.align.unwrap_or_default(),
        }
    }

    /// Resolved `grid` parameters.
    #[must_use]
    pub fn grid(&self) -> GridProps {
        GridProps {
            columns: self.columns.clone(),
            rows: self.rows.clone(),
            column_spacing: self.column_spacing.unwrap_or(DEFAULT_SPACING),
            row_spacing: self.row_spacing.unwrap_or(DEFAULT_SPACING),
            align: self.align.unwrap_or_default(),
        }
    }

    /// Resolved `scroll` parameters.
    ///
    /// `overscan` falls back to [`DEFAULT_OVERSCAN`] for an absent value and
    /// for a present-but-unusable one (negative or non-finite): it widens a
    /// materialization window below, and a stray author input must not be
    /// able to turn that window inside out.
    #[must_use]
    pub fn scroll(&self) -> ScrollProps {
        ScrollProps {
            overscan: self
                .overscan
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(DEFAULT_OVERSCAN),
            axis: self.axis.unwrap_or(Axis::Vertical),
        }
    }

    /// Resolved content insets. Absent declares no padding, which resolves to
    /// `Insets::NONE` — the same "absence is the documented default" rule
    /// every other resolver here follows (`DEFAULT_SPACING`, `DEFAULT_OVERSCAN`).
    #[must_use]
    pub fn padding(&self) -> Insets {
        self.padding.unwrap_or(Insets::NONE)
    }

    /// Resolved `collection` parameters, or `None` when the node is not a
    /// collection that passed acceptance.
    #[must_use]
    pub fn collection(&self) -> Option<CollectionProps> {
        Some(CollectionProps {
            total_count: self.total_count?,
            source: self.source.clone()?,
            estimated_extent: self.estimated_extent.unwrap_or(DEFAULT_ROW_EXTENT),
        })
    }

    /// Resolved `text` parameters.
    #[must_use]
    pub fn text(&self) -> TextProps<'_> {
        TextProps {
            text: self.text.as_deref().unwrap_or(""),
            wrap: self.wrap.unwrap_or_default(),
            max_lines: self.max_lines,
            style: self.style.as_deref(),
        }
    }

    /// Resolved `surface` parameters, or `None` when no anchor is declared.
    #[must_use]
    pub fn surface(&self) -> Option<SurfaceProps<'_>> {
        Some(SurfaceProps {
            layer: self.layer?,
            anchor: self.anchor.as_ref()?,
            clamp: self.clamp.unwrap_or_default(),
            input_policy: self.input_policy.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Layer, Props, TextWrap, TrackSize};
    use crate::geom::{Axis, Insets};

    /// One vocabulary for one enum. The digest hashes
    /// [`TextWrap::as_str`] and the wire form uses serde's name; if the two
    /// drift, a frame received over the driver protocol and a frame petrified
    /// locally would hash the same policy to two different strings and the
    /// SC-004 cross-target claim would be false with nothing to show for it.
    #[test]
    fn the_wrap_policy_has_one_name_in_the_digest_and_on_the_wire() {
        for wrap in [TextWrap::Wrap, TextWrap::Ellipsis, TextWrap::Clip] {
            let json = serde_json::to_string(&wrap).unwrap();
            assert_eq!(
                json,
                format!("\"{}\"", wrap.as_str()),
                "{wrap:?} serializes as {json} but digests as {:?}",
                wrap.as_str()
            );
            let back: TextWrap = serde_json::from_str(&json).unwrap();
            assert_eq!(back, wrap);
        }
    }

    #[test]
    fn an_empty_props_serializes_to_an_empty_table() {
        let json = serde_json::to_string(&Props::default()).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn declared_fields_round_trip() {
        let mut props = Props {
            axis: Some(Axis::Horizontal),
            spacing: Some(8.0),
            columns: vec![
                TrackSize::Fixed { value: 100.0 },
                TrackSize::Weight { weight: 1.0 },
            ],
            wrap: Some(TextWrap::Ellipsis),
            layer: Some(Layer::Modal),
            padding: Some(Insets::symmetric(4.0, 8.0)),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), "surface.raised".into());
        let json = serde_json::to_string(&props).unwrap();
        let back: Props = serde_json::from_str(&json).unwrap();
        assert_eq!(back, props);
        assert!(json.contains("\"axis\":\"horizontal\""), "{json}");
        assert!(
            json.contains("\"type\":\"fit-content\"") || json.contains("\"weight\""),
            "{json}"
        );
    }

    /// A misspelled prop is a typo the author wants to hear about, not a
    /// silently ignored field that makes a panel lay out wrong.
    #[test]
    fn unknown_props_are_refused() {
        let err = serde_json::from_str::<Props>(r#"{"spaceing": 8}"#).unwrap_err();
        assert!(err.to_string().contains("spaceing"), "{err}");
    }

    #[test]
    fn defaults_are_the_documented_ones() {
        let props = Props::default();
        assert_eq!(props.stack().axis, Axis::Vertical);
        assert_eq!(props.stack().spacing, 0.0);
        assert_eq!(props.scroll().axis, Axis::Vertical);
        assert!(props.collection().is_none());
        assert!(props.surface().is_none());
        assert_eq!(props.text().text, "");
        assert_eq!(props.padding(), Insets::NONE);
    }

    /// `Props.padding` round-trips through serde the same way every other
    /// declared field does — pinned separately from `declared_fields_round_trip`
    /// because the field is new and its serde shape (an `Insets` struct, not a
    /// plain scalar) is worth checking on its own.
    #[test]
    fn padding_round_trips_through_serde() {
        let props = Props {
            padding: Some(Insets::symmetric(4.0, 8.0)),
            ..Props::default()
        };
        let json = serde_json::to_string(&props).unwrap();
        assert!(json.contains("\"padding\""), "{json}");
        let back: Props = serde_json::from_str(&json).unwrap();
        assert_eq!(back, props);
        assert_eq!(back.padding(), Insets::symmetric(4.0, 8.0));

        // Absent padding serializes away entirely, and resolves to NONE.
        let bare = Props::default();
        let bare_json = serde_json::to_string(&bare).unwrap();
        assert!(!bare_json.contains("padding"), "{bare_json}");
        assert_eq!(bare.padding(), Insets::NONE);
    }

    #[test]
    fn layer_z_bases_are_ordered() {
        assert!(Layer::Popup.base_z() < Layer::Toast.base_z());
        assert!(Layer::Toast.base_z() < Layer::Modal.base_z());
        assert!(Layer::Modal.base_z() < Layer::FrameWide.base_z());
    }
}
