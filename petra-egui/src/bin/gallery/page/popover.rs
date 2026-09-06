//! Inventory row 24, Popover.

use gorgon_petra::component::{
    button, heading, popover_with, popover_with_placement, primary_button, section,
};
use gorgon_petra::geom::{Align as CrossAlign, Axis};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{Align, AxisConstraint, Constraints, Edge, NodeKind, Props, ViewNode};

use super::Page;
use super::common::{body, column, dismisses, path_has, row, sp, wrapped};

const ANCHOR: &str = "pop-anchor";
const NOTE: &str = "pop-note";
const SHUT: &str = "pop-shut";

/// All twelve of Carbon's named placements: `_popover.scss`'s
/// `--<edge>`, `--<edge>-start` and `--<edge>-end`, for each of four edges.
///
/// [`placement_grid`] filters this list by edge to build one row per edge, so
/// what the order here decides is the order *within* a row. `Align::Center`
/// sits in the middle of each three, so `Start` and `End` read as the same
/// bubble nudged either way rather than as two unrelated pictures.
const PLACEMENTS: [(Edge, Align, &str); 12] = [
    (Edge::Top, Align::Start, "top-start"),
    (Edge::Top, Align::Center, "top"),
    (Edge::Top, Align::End, "top-end"),
    (Edge::Left, Align::Start, "left-start"),
    (Edge::Left, Align::Center, "left"),
    (Edge::Left, Align::End, "left-end"),
    (Edge::Right, Align::Start, "right-start"),
    (Edge::Right, Align::Center, "right"),
    (Edge::Right, Align::End, "right-end"),
    (Edge::Bottom, Align::Start, "bottom-start"),
    (Edge::Bottom, Align::Center, "bottom"),
    (Edge::Bottom, Align::End, "bottom-end"),
];

/// One trigger-and-bubble pair for the placement grid, keyed by `tag`
/// (`PLACEMENTS`' third field, e.g. `"top-start"`) so all twelve pairs on
/// the page carry distinct ids.
///
/// The trigger's own label is the fixed word "Trigger", not `tag`: an
/// earlier version put `tag` on both the trigger and the bubble, so every
/// pair photographed as two boxes reading the same word and a caret hunting
/// between them read as a rendering bug rather than a placement. Only the
/// bubble names the placement, so a person reads "Trigger" once per pair
/// and the word that changes is always the one the caret is pointing at.
fn placement_pair(edge: Edge, align: Align, tag: &str) -> ViewNode {
    let anchor_key = format!("pop-grid-{tag}");
    let bubble_key = format!("pop-grid-{tag}-bubble");
    // A `Left` bubble grows leftward out of its trigger, and a stack packs
    // its children at the main-axis start, so a left-facing trigger sitting
    // at the cell's own left edge has nowhere to put its bubble: it paints
    // off the grid entirely (measured at x 195 against a grid starting at
    // 292). The lead-in pushes those triggers across their cell so the
    // bubble grows into the cell rather than out of the page.
    let mut children = Vec::new();
    if edge == Edge::Left {
        children.push(lead_in(&format!("pop-grid-{tag}-lead")));
    }
    children.extend([
        button(anchor_key.clone(), "Trigger"),
        popover_with_placement(
            bubble_key,
            format!("{tag} placement"),
            anchor_key,
            edge,
            align,
            vec![wrapped(&format!("pop-grid-{tag}-body"), tag)],
        ),
    ]);
    let cell = row(&format!("pop-grid-{tag}-cell"), None, children);
    // A bubble is an *overlay*: it paints outside its parent's box and the
    // layout pass reserves nothing for it. So a grid of twelve pairs laid
    // out on spacing alone photographs as triggers sitting on top of their
    // neighbours' bubbles — which is exactly what the first shot of this
    // page showed, and it reads as a rendering fault rather than as twelve
    // placements. The cell reserves the room the overlay is going to use.
    //
    // These are layout declarations, which `literal-style`'s module doc
    // names as numeric by spec and never inspects. They are the demo's
    // furniture, not styling.
    cell.with_props(Props {
        axis: Some(Axis::Horizontal),
        align: Some(trigger_seat(edge)),
        ..Props::default()
    })
    .with_constraints(cell_box())
}

/// Where a cell parks its trigger on the vertical.
///
/// A bubble grows away from the trigger's named edge, so a trigger sitting
/// in the middle of its cell sends half of that growth out of the cell and
/// into the row below or above. Park the trigger against the edge the
/// bubble grows *away* from and the whole bubble lands inside the cell:
/// a `Top` trigger at the cell's foot has its bubble above it, a `Bottom`
/// trigger at the cell's head has its bubble below it. A `Left` or `Right`
/// bubble grows sideways and only ever spans 52 units on the vertical
/// whichever way it is aligned, so those triggers stay centred.
///
/// This is what lets the whole grid be 104 units per row instead of 160.
/// The difference is not cosmetic: four 148-unit rows starting at y 432
/// ran to y 1024 against a 900-unit viewport, so the `left-*` row sat
/// entirely below the fold and all three of its bubbles clamped to the
/// viewport's bottom edge at 900 — stacked on the `bottom-*` row's bubbles
/// and no longer beside their own triggers.
fn trigger_seat(edge: Edge) -> CrossAlign {
    match edge {
        Edge::Top => CrossAlign::End,
        Edge::Bottom => CrossAlign::Start,
        Edge::Left | Edge::Right => CrossAlign::Center,
    }
}

/// An empty block that pushes a left-facing trigger across its cell.
///
/// Fixed rather than flexible on purpose: an empty stack measures zero
/// against both a zero and an unbounded offer, so it has no flexibility for
/// the container to hand space to, and a spacer that cannot grow has to
/// name its own width.
fn lead_in(key: &str) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key).with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(150.0),
            max: Some(150.0),
            priority: 0,
        },
        ..Constraints::default()
    })
}

/// The room one placement cell reserves for its bubble.
///
/// Every cell is the same size, so a bubble has the same room whichever
/// way it grows. That uniformity is the whole point. The first two
/// attempts at this grid shaped each group to the direction it grew —
/// `Top` and `Bottom` in wide rows, `Left` and `Right` in tall columns —
/// and both photographed as triggers sitting on their neighbours'
/// bubbles, because a bubble is an overlay: it paints outside its parent's
/// box and the layout pass reserves nothing for it. A layout tuned per
/// direction has four ways to be wrong. One that is symmetric has none.
/// What varies per direction is only where the trigger sits inside the
/// cell, which [`trigger_seat`] settles.
///
/// 280 by 104, measured rather than guessed.
///
/// The width: the page's content column is 868 units wide, so three cells
/// have 289 each to live in; an earlier 420 asked for 1260, and the stack
/// answered by crushing the third column to 28 units and clipping its
/// trigger off the page. A `Left` cell needs the lead-in (150) plus the
/// trigger (77) — 227, which fits, and leaves the widest bubble (97) its
/// room to the trigger's left.
///
/// The height: the trigger (40) plus the anchor gap (8) plus a 52-tall
/// bubble is 100, and the trigger parks against the edge that keeps the
/// bubble inside the cell, so 104 holds a whole pair with a little over.
/// Four rows of 104 start at y 432 and end at y 848, inside a 900-unit
/// viewport. Four rows of 148 ended at 1024 and put the last row under
/// the fold, where `ClampRule::Flip` measures against the viewport and
/// clamped all three of its bubbles onto the row above.
///
/// These are layout declarations, which `literal-style`'s module doc names
/// as numeric by spec and never inspects.
fn cell_box() -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(280.0),
            max: Some(280.0),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(104.0),
            max: Some(104.0),
            priority: 0,
        },
    }
}

/// T0.3's acceptance shot: the engine reaches all twelve of Carbon's named
/// placements from this component, not only the one `pop-pair` above
/// demonstrates (`Edge::Bottom`/`Align::Center`, Carbon's default).
///
/// Four rows of three: one row per edge, the row's three cells its three
/// alignments. Every cell is the same box ([`cell_box`]) and differs only in
/// where it parks its trigger ([`trigger_seat`]), so each bubble grows into
/// its own cell and no cell has to know what its neighbours are doing.
///
/// Two earlier arrangements shaped each group to the direction it grew —
/// `Top` and `Bottom` as wide rows, `Left` and `Right` as tall columns — and
/// both photographed as triggers sitting on their neighbours' bubbles. A
/// bubble is an overlay: it paints outside its parent's box and the layout
/// pass reserves nothing for it, so a layout tuned per direction has four
/// ways to be wrong and a symmetric one has none.
///
/// The whole grid has to fit above the fold, which is why the cell is 104
/// tall rather than 160. [`ClampRule::Flip`] measures a bubble against the
/// **viewport**, not the scroll content, so a row below the fold does not
/// come good when the page is scrolled: its bubbles clamp there and stay
/// clamped. The `shots` test named for these twelve bubbles is what holds
/// that: it checks containment on **both** axes, and that each bubble lands
/// on the side it is named for.
fn placement_grid() -> ViewNode {
    let edge_row = |edge: Edge| {
        let cells = PLACEMENTS
            .iter()
            .filter(|(e, ..)| *e == edge)
            .map(|(edge, align, tag)| placement_pair(*edge, *align, tag))
            .collect();
        row(&format!("pop-grid-row-{}", edge.as_str()), None, cells)
    };
    // A vertical `Stack`, not `common::column`. That helper is a `Grid`
    // with one weighted column, so it hands each of the four edge rows an
    // equal *share* of the section's height — measured at a 300 pitch for
    // rows that asked for 180 — and the grid then ran past the bottom of the
    // page. `ClampRule::Flip` did exactly what it should with that: the
    // `bottom-*` bubbles had no room below, so they flipped above their
    // triggers and the row photographed as a second `top-*` row. A stack
    // packs the rows at their own height and the flip stops firing.
    ViewNode::new(NodeKind::Stack, "pop-grid")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .with_children(vec![
            edge_row(Edge::Top),
            edge_row(Edge::Right),
            edge_row(Edge::Bottom),
            edge_row(Edge::Left),
        ])
}

/// Live state of the Popover page: whether the panel is showing.
///
/// It starts **open**. A closed disclosure is one button and no
/// disclosure, which is what the operator walked past on rows 24 and 37;
/// the resting capture has to photograph the thing itself.
pub struct Popover {
    open: bool,
}

impl Default for Popover {
    fn default() -> Self {
        Self { open: true }
    }
}

impl Page for Popover {
    fn row(&self) -> &'static str {
        "Popover"
    }

    fn body(&self) -> ViewNode {
        let mut pair = vec![button(ANCHOR, "Filters")];
        if self.open {
            // A popover is the *container* Toggletip, Tooltip, Dropdown and
            // Menu all compose on, so the only thing that demonstrates it
            // is what lives inside: a title, a line of prose that has to
            // wrap at the 368 ceiling, and two real controls. Row 37's
            // bubble is 288 wide, a different tone, and holds a note.
            pair.push(popover_with(
                NOTE,
                "Filter fibers",
                ANCHOR,
                vec![
                    heading("pop-title", "Filter fibers"),
                    wrapped(
                        "pop-body",
                        "A popover is the anchored surface every other overlay \
                         in this library is built on. Its contents are the \
                         caller's, and they keep their own roles.",
                    ),
                    row(
                        "pop-actions",
                        sp("spacing.sm"),
                        vec![
                            button("pop-cancel", "Cancel"),
                            primary_button("pop-apply", "Apply"),
                        ],
                    ),
                ],
            ));
        }
        section(
            "pop",
            "Popover",
            vec![body("po", sp("spacing.md"), {
                // The placement grid leads. `ClampRule::Flip` measures a
                // bubble against the **viewport**, not the scroll content,
                // so a row near the fold flips whichever way it has to and
                // no amount of scrolling undoes it: with the `po-pair`
                // demo above it the grid began at y 432 with 468 units of
                // viewport left for four rows that need 640, and the
                // `bottom-*` row photographed as a second `top-*` row.
                // Leading with the grid gives it the room, and the `po-pair`
                // panel below grows down into space nothing else wants.
                // One overlay demo at a time, and the trigger swaps them.
                //
                // A popover reserves no layout space and `ClampRule::Flip`
                // measures against the **viewport**, not the scroll content
                // (`layout/overlay_surface.rs`'s `flip_axis` takes
                // `vp_origin`/`vp_extent`), so two large overlay demos on
                // one page cannot both be open however the page is scrolled
                // or ordered: whichever one ends up at the fold flips onto
                // the other. Photographed in both orders before this split.
                //
                // So the panel is the resting state — a closed disclosure is
                // one button and no disclosure, which is what the operator
                // walked past on rows 24 and 37 — and pressing the trigger
                // shuts it and shows the twelve placements instead.
                let mut cells = vec![];
                cells.push(column("po-pair", None, pair));
                cells.extend([
                    column("po-shut", None, vec![button(SHUT, "Closed trigger")]),
                    wrapped(
                        "po-note",
                        "Press either trigger. The panel is 368 wide on \
                             surface.raised; a toggletip is 288 on the ramp's \
                             last rung.",
                    ),
                ]);
                if !self.open {
                    // T0.3: all twelve of Carbon's named placements are
                    // reachable through `popover_with_placement`, not only
                    // the `Edge::Bottom`/`Align::Center` default the panel
                    // shows.
                    //
                    // It sits below the trigger, not above it, and the two
                    // constraints that pins it there are worth stating. Put
                    // the grid first and it fills the viewport, pushing
                    // `po-pair`'s own trigger below the fold where no test
                    // and no hand can press it. Leave the panel open beside
                    // it and the panel flips onto the grid. So: the grid
                    // follows the trigger, and it only appears when the
                    // panel is shut.
                    cells.push(heading("pop-grid-title", "All twelve placements"));
                    cells.push(placement_grid());
                }
                cells
            })],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, ANCHOR) || path_has(node, SHUT) {
            self.open = !self.open;
            return true;
        }
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, NOTE) {
            self.open = false;
        }
    }
}
