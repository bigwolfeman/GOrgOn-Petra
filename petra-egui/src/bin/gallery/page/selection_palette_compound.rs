//! Spec 009's Selection palette compound, hosted directly through the
//! [`Compound`] triple.
//!
//! A real text selection is spec 006; this page's trigger stands in for
//! it and sends [`Intent::Move`], the same substitution the compound's own
//! doc names. The palette itself has no closed form — `view` always draws
//! the overlay — and its own Bold/Italic controls are pressable here
//! exactly as they would be once a real selection hosts them.

use gorgon_petra::Point;
use gorgon_petra::component::{button, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;
use gorgon_petra_compound::Compound;
use gorgon_petra_compound::selection_palette::{
    Intent, Props, SelectionPalette as SelectionPaletteCompound, State,
};

use super::Page;
use super::common::{body, column, path_has, sp, wrapped};

/// The page's own trigger: not part of the compound, standing in for the
/// real selection move spec 006 will send.
const MOVE_TRIGGER: &str = "move-selection";
/// The two points [`MOVE_TRIGGER`] alternates the overlay between, so a
/// press visibly relocates it rather than moving it by a pixel no
/// screenshot could tell apart from a rounding accident.
///
/// `Anchor::Point` is viewport-relative, not page-body-relative, and the
/// catalog's own index pane occupies roughly the left quarter of the
/// 1200x900 logical window (`catalog::WINDOW`). A first version of this
/// page anchored at `(24, 96)`: comfortably inside the page body by every
/// other page's convention, and squarely on top of the index pane's own
/// row 4/5 labels once actually rasterized. Both points below sit inside
/// the section body's own content column instead.
const POINT_A: (f32, f32) = (320.0, 420.0);
const POINT_B: (f32, f32) = (720.0, 520.0);

/// Live state of the Selection palette (compound) page.
pub struct SelectionPaletteCompoundPage {
    props: Props,
    state: State,
}

impl Default for SelectionPaletteCompoundPage {
    fn default() -> Self {
        let props = Props;
        let mut state = SelectionPaletteCompound::init(&props);
        SelectionPaletteCompound::update(
            &mut state,
            Intent::Move {
                point: Point::new(POINT_A.0, POINT_A.1),
            },
        );
        SelectionPaletteCompound::update(&mut state, Intent::ToggleBold);
        Self { props, state }
    }
}

impl Page for SelectionPaletteCompoundPage {
    fn row(&self) -> &'static str {
        "Selection palette (compound)"
    }

    fn body(&self) -> ViewNode {
        section(
            "palette-compound",
            "Selection palette (compound)",
            vec![body(
                "spc-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "spc-note",
                        "The same triple as spec 009 T028a. A real \
                         selection is spec 006; this trigger stands in for \
                         it with `Intent::Move`. Bold and Italic below are \
                         the compound's own toggle buttons.",
                    ),
                    column(
                        "spc-col",
                        sp("spacing.md"),
                        vec![
                            button(MOVE_TRIGGER, "Move to another spot"),
                            SelectionPaletteCompound::view(&self.state, &self.props),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, "bold") {
            SelectionPaletteCompound::update(&mut self.state, Intent::ToggleBold);
            return true;
        }
        if path_has(node, "italic") {
            SelectionPaletteCompound::update(&mut self.state, Intent::ToggleItalic);
            return true;
        }
        if path_has(node, MOVE_TRIGGER) {
            let at_a = self.state.point == Point::new(POINT_A.0, POINT_A.1);
            let next = if at_a { POINT_B } else { POINT_A };
            SelectionPaletteCompound::update(
                &mut self.state,
                Intent::Move {
                    point: Point::new(next.0, next.1),
                },
            );
            return true;
        }
        false
    }
}
