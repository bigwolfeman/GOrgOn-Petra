//! A fixed-pitch cell grid: the size of one character cell for a typography
//! token, and the arithmetic between cells and points.
//!
//! A custom painter that draws a grid of characters (a console, a hex view,
//! a fixed-width table) needs three facts the frame does not carry: how wide
//! one cell is, how tall one row is, and the font to draw with. All three
//! follow from one typography token, so this module derives them from the
//! token the theme already declares (`typography.code`, say) rather than
//! from literals in each painter. The width is the face's advance for one
//! glyph plus the token's letter spacing, which is exactly what a run laid
//! out with [`CellMetrics::text_format`] advances by, so text drawn as a run
//! stays on the grid.

use egui::text::TextFormat;
use egui::{Color32, Context, FontId, Pos2, Rect, Vec2};
use gorgon_petra::token::{TokenValue, TypographyFamily, TypographyValue};

use crate::paint::TokenSource;
use crate::text::FontFaces;

/// The glyph whose advance sets the cell width. Any glyph serves in a
/// fixed-pitch face; a digit is in every face's basic set.
const PROBE: char = '0';

/// One cell of a fixed-pitch grid, in logical points.
#[derive(Clone, Debug, PartialEq)]
pub struct CellMetrics {
    /// The face and size to draw with.
    pub font: FontId,
    /// Horizontal advance of one cell, letter spacing included.
    pub width: f32,
    /// Height of one row: the token's line height.
    pub height: f32,
    /// The token's letter spacing, already inside `width`. Kept so
    /// [`CellMetrics::text_format`] lays a run out at the same advance.
    pub letter_spacing: f32,
}

impl CellMetrics {
    /// The cell for `value`, drawn through `faces`.
    ///
    /// Reads glyph metrics from `ctx`, so it needs the fonts egui loads on
    /// its first pass: call it from a painter, or after one pass has run.
    /// A proportional token gives a proportional face's `0` advance, which
    /// is a grid but not a faithful one; [`CellMetrics::from_token`] refuses
    /// a token that is not [`TypographyFamily::Mono`].
    #[must_use]
    pub fn of(ctx: &Context, value: &TypographyValue, faces: &FontFaces) -> Self {
        let font = FontId::new(value.size, faces.family(value.family, value.weight).clone());
        let advance = ctx.fonts_mut(|fonts| fonts.glyph_width(&font, PROBE));
        Self {
            font,
            width: advance + value.letter_spacing,
            height: value.line_height,
            letter_spacing: value.letter_spacing,
        }
    }

    /// The cell for the typography token `name` in `tokens`, or `None` when
    /// the theme has no such token or it is not a fixed-pitch token.
    #[must_use]
    pub fn from_token(
        ctx: &Context,
        tokens: &dyn TokenSource,
        name: &str,
        faces: &FontFaces,
    ) -> Option<Self> {
        match tokens.value(name)? {
            TokenValue::Typography(value) if value.family == TypographyFamily::Mono => {
                Some(Self::of(ctx, value, faces))
            }
            _ => None,
        }
    }

    /// How many whole columns and rows fit in `area`, at least one of each.
    ///
    /// Floors, so the last partial cell is left empty rather than clipped.
    /// At least one, because a grid program told it has zero columns has no
    /// defined behaviour, and a pane squeezed to nothing still has a size.
    #[must_use]
    pub fn grid(&self, area: Vec2) -> (u16, u16) {
        (fit(area.x, self.width), fit(area.y, self.height))
    }

    /// The rect of cell (`col`, `row`) in a grid whose top-left is `origin`.
    #[must_use]
    pub fn cell(&self, origin: Pos2, col: u16, row: u16) -> Rect {
        Rect::from_min_size(
            Pos2::new(
                origin.x + f32::from(col) * self.width,
                origin.y + f32::from(row) * self.height,
            ),
            Vec2::new(self.width, self.height),
        )
    }

    /// The cell under `pos` in a grid of `grid` cells whose top-left is
    /// `origin`, or `None` when `pos` is outside it.
    #[must_use]
    pub fn cell_at(&self, origin: Pos2, grid: (u16, u16), pos: Pos2) -> Option<(u16, u16)> {
        let x = (pos.x - origin.x) / self.width;
        let y = (pos.y - origin.y) / self.height;
        if !(x >= 0.0 && y >= 0.0) {
            return None;
        }
        // Non-negative and finite here, and a cell index past `u16::MAX`
        // saturates the cast and then fails the bound below.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (col, row) = (x.floor() as u16, y.floor() as u16);
        (x < f32::from(grid.0) && y < f32::from(grid.1)).then_some((col, row))
    }

    /// A text format that lays a run out on this grid: the font, the row
    /// height, and the letter spacing, in `color`.
    #[must_use]
    pub fn text_format(&self, color: Color32) -> TextFormat {
        TextFormat {
            font_id: self.font.clone(),
            extra_letter_spacing: self.letter_spacing,
            line_height: Some(self.height),
            color,
            ..TextFormat::default()
        }
    }
}

/// Whole `cell`s in `extent`, at least one and at most `u16::MAX`.
fn fit(extent: f32, cell: f32) -> u16 {
    // NaN in either compares false everywhere, so it is named here.
    if cell.is_nan() || extent.is_nan() || cell <= 0.0 || extent < cell {
        return 1;
    }
    let whole = (extent / cell).floor();
    // Clamped above, so the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = whole.min(f32::from(u16::MAX)) as u16;
    whole.max(1)
}

#[cfg(test)]
mod tests {
    use super::{CellMetrics, fit};
    use crate::text::FontFaces;
    use egui::{Context, Pos2, Vec2};
    use gorgon_petra::token::{ThemeSnapshot, dark};

    /// A context whose fonts are loaded, which needs one pass.
    fn loaded() -> Context {
        crate::testing::headless()
    }

    /// The shipped code token gives a fixed-pitch cell: every glyph in the
    /// face advances by the same amount, and the cell is that advance plus
    /// the token's letter spacing. Kill line: dropping `letter_spacing` from
    /// `width` in `CellMetrics::of`.
    #[test]
    fn the_code_token_gives_a_fixed_pitch_cell() {
        let ctx = loaded();
        let tokens = ThemeSnapshot::new(dark(), 1);
        let cell = CellMetrics::from_token(&ctx, &tokens, "typography.code", &FontFaces::default())
            .expect("the shipped theme has a mono code token");
        let (narrow, wide) = ctx.fonts_mut(|fonts| {
            (
                fonts.glyph_width(&cell.font, 'i'),
                fonts.glyph_width(&cell.font, 'W'),
            )
        });
        assert!(
            (narrow - wide).abs() < 1e-3,
            "a fixed-pitch face: {narrow} vs {wide}"
        );
        assert!(
            (cell.width - (wide + cell.letter_spacing)).abs() < 1e-3,
            "{cell:?}"
        );
        assert!(
            cell.letter_spacing > 0.0,
            "code-01 tracks at 0.32: {cell:?}"
        );
        assert!(
            (cell.height - 16.0).abs() < 1e-3,
            "code-01's 16pt line: {cell:?}"
        );
    }

    /// A proportional token is refused rather than giving a grid whose
    /// cells do not match its glyphs; an absent token is refused too.
    #[test]
    fn a_proportional_or_missing_token_has_no_cell() {
        let ctx = loaded();
        let tokens = ThemeSnapshot::new(dark(), 1);
        let faces = FontFaces::default();
        assert!(CellMetrics::from_token(&ctx, &tokens, "typography.body", &faces).is_none());
        assert!(CellMetrics::from_token(&ctx, &tokens, "typography.nope", &faces).is_none());
    }

    /// Whole cells, floored, never zero.
    #[test]
    fn the_grid_floors_and_never_reaches_zero() {
        assert_eq!(fit(100.0, 10.0), 10);
        assert_eq!(fit(109.9, 10.0), 10);
        assert_eq!(fit(5.0, 10.0), 1);
        assert_eq!(fit(0.0, 10.0), 1);
        assert_eq!(fit(100.0, 0.0), 1);
        assert_eq!(fit(f32::NAN, 10.0), 1);
        assert_eq!(fit(1.0e9, 1.0), u16::MAX);
    }

    /// `cell_at` inverts `cell`, and a point outside the grid names nothing.
    #[test]
    fn cell_at_inverts_cell() {
        let cell = CellMetrics {
            font: egui::FontId::monospace(12.0),
            width: 7.5,
            height: 16.0,
            letter_spacing: 0.0,
        };
        let origin = Pos2::new(10.0, 20.0);
        let grid = cell.grid(Vec2::new(80.0 * 7.5, 24.0 * 16.0));
        assert_eq!(grid, (80, 24));
        for (col, row) in [(0, 0), (79, 23), (40, 7)] {
            let rect = cell.cell(origin, col, row);
            assert_eq!(cell.cell_at(origin, grid, rect.center()), Some((col, row)));
        }
        assert_eq!(cell.cell_at(origin, grid, Pos2::new(9.0, 25.0)), None);
        assert_eq!(
            cell.cell_at(origin, grid, Pos2::new(10.0 + 80.0 * 7.5, 25.0)),
            None
        );
        assert_eq!(cell.cell_at(origin, grid, Pos2::new(f32::NAN, 25.0)), None);
    }
}
