//! Inventory row 35, Tile.

use gorgon_petra::component::{clickable_tile, expandable_tile, section, selectable_tile, tile};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const TILE_SEL: &str = "tile-sel";
const TILE_EXP: &str = "tile-exp";

/// Live state of the Tile page.
#[derive(Default)]
pub struct Tile {
    tile_sel: bool,
    tile_exp: bool,
}

impl Page for Tile {
    fn row(&self) -> &'static str {
        "Tile"
    }

    fn body(&self) -> ViewNode {
        section(
            "kinds",
            "Base, clickable, selectable, expandable",
            vec![body(
                "tiles",
                sp("spacing.md"),
                vec![
                    tile("tile-base", "A static tile holds related content."),
                    clickable_tile(
                        "tile-click",
                        "Open workspace",
                        "Clickable tile — one target.",
                    ),
                    selectable_tile(TILE_SEL, "Select this option", self.tile_sel),
                    expandable_tile(
                        TILE_EXP,
                        "More detail",
                        self.tile_exp,
                        "Below-the-fold body.",
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, TILE_SEL) {
            self.tile_sel = !self.tile_sel;
        } else if path_has(node, TILE_EXP) {
            self.tile_exp = !self.tile_exp;
        } else {
            return false;
        }
        true
    }
}
