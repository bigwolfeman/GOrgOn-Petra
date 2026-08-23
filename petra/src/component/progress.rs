//! `progress` — a determinate readout: a filled track plus its role, label,
//! and formatted value.

use super::swatch;
use super::tokens::{SHAPE_FULL, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, t};
use crate::geom::Align;
use crate::tree::{AxisConstraint, Key, NodeKind, Props, Role, Semantics, TrackSize, ViewNode};

/// Thickness of the drawn bar, logical units. The one number the component
/// still owns: its width is the grid's, its colours are the theme's.
const BAR_HEIGHT: f32 = 10.0;

/// Smallest weight either bar track may carry.
///
/// Tree acceptance refuses a `TrackSize::Weight` that is not finite and
/// greater than zero (`Violation::ValueOutOfRange`), so a 0% or 100% bar
/// cannot express its empty side as weight `0.0` — it expresses it as a
/// weight small enough to round away instead.
const MIN_WEIGHT: f32 = 0.001;

/// One cell of the bar: a coloured box that takes its width from the grid
/// track it sits in.
///
/// [`swatch`] pins both axes to the extents it is handed, which is what a
/// checkbox box or a status dot wants — a fixed square. A progress cell is
/// the opposite case: the whole point of the grid's weighted columns is that
/// the fill is *as wide as the value says*, so the horizontal pin is dropped
/// here and the grid's [`Align::Stretch`] fills the track (the same
/// "`Stretch` is the declaration that says fill the track" idiom the
/// gallery's two-column form uses for its fields). The vertical pin stays:
/// the bar's thickness belongs to the component, not to the row it lands in.
fn bar_cell(key: &'static str, background: &str) -> ViewNode {
    let mut cell = swatch(key, 0.0, BAR_HEIGHT, Some(background), None, None);
    cell.constraints.horizontal = AxisConstraint::default();
    cell
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
/// The fill and track colours are ink-on-paper (`TEXT_PRIMARY` on
/// `SURFACE_RAISED`), not a status colour: a `progress` bar says how much
/// of something is done, not whether that something is healthy, and
/// spending a status hue on it would blur that distinction the day an
/// author puts a `status` component next to one.
pub fn progress(key: impl Into<Key>, label: impl Into<String>, value: f32) -> ViewNode {
    let key = key.into();
    let label = label.into();
    let done = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let rest = (1.0 - done).max(MIN_WEIGHT);

    let mut bar_props = Props {
        columns: vec![
            TrackSize::Weight {
                weight: done.max(MIN_WEIGHT),
            },
            TrackSize::Weight { weight: rest },
        ],
        align: Some(Align::Stretch),
        ..Props::default()
    };
    bar_props.tokens.insert("border".into(), t(TEXT_MUTED));
    bar_props.tokens.insert("radius".into(), t(SHAPE_FULL));

    let fill = bar_cell("fill", TEXT_PRIMARY);
    let track = bar_cell("track", SURFACE_RAISED);

    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(bar_props)
        .child(fill)
        .child(track);
    node.semantics = Semantics {
        role: Some(Role::Progress),
        label: Some(label),
        value: Some(format!("{:.0}%", done * 100.0)),
        ..Semantics::default()
    };
    node
}
