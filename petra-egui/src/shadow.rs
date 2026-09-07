//! A soft rectangular shadow, built as a mesh instead of asked of epaint.
//!
//! # Why this module exists
//!
//! `epaint::Shadow::as_shape` cannot draw a shadow on a bar, and the failure
//! is visible rather than theoretical. It renders a shadow as a `RectShape`
//! whose *feathering* carries the blur, and `tessellator.rs:1866` then reads:
//!
//! ```text
//! // The tessellator can't handle blurring/feathering larger than the
//! // smallest side of the rect.
//! blur_width = blur_width.at_most(rect.size().min_elem() - eps - ...);
//! corner_radius += 0.5 * blur_width;
//! ```
//!
//! Two things follow for a focus bar, which is
//! [`gorgon_petra::token::FocusRing::thickness`] — three units — on its short
//! side against a `shadow.raised` blur of six.
//!
//! 1. **The blur is clamped to 2.9.** The declared six is never drawn. The
//!    bar's shadow and a card's shadow are not the same shadow, whatever the
//!    token says, and nothing in the token layer can tell.
//! 2. **The corner radius becomes 1.45 on a 3-unit side**, so the two corner
//!    arcs nearly meet and their feather skirts overlap. The semi-transparent
//!    shadow is then blended twice along the corner's 45° bisector, and the
//!    result is a one-pixel diagonal streak, four pixels long, darker than
//!    anything else in the shadow.
//!
//! Measured on the light theme at 2 device px per point, under a tag's
//! `BarUnder`, reading across the shadow's rows near its left end. The
//! numbers are the red channel; the mirrored column is given beside each
//! offender:
//!
//! ```text
//! row 643   ... 238 228 218 209 199 [117] 185 ...   mirror reads 190
//! row 644   ... 242 233 222 213 [143] 199 195 ...   mirror reads 203
//! row 645   ... 242 236 227 [173] 213 209 207 ...   mirror reads 216
//! row 646   ... 242 240 [210] 227 222 218 217 ...   mirror reads 231
//! ```
//!
//! The operator found it on a `Sides` bar, where it is worst: that bar is
//! three units wide, so a four-pixel diagonal is as long as the bar is broad
//! and reads as a hook hanging off the foot. The same streak is on every
//! `BarUnder` in the catalog, at one end, where it passes for a stray pixel.
//!
//! Surfaces are not affected and this module is not for them. A menu is 160
//! by 80 against a blur of 12, so nothing is clamped, no corner radius is
//! invented, and both of its bottom corners measure as exact mirrors of each
//! other. `paint`'s elevation block still uses `epaint::Shadow` and should.
//!
//! # What is drawn instead
//!
//! A box blurred by a box kernel is **separable**: the result is the product
//! of a one-dimensional profile in x and the same in y. Each profile is a
//! trapezoid — it rises over `min(w, blur)`, holds, and falls again — and its
//! plateau is `min(1, w / blur)`, which is below one exactly when the caster
//! is thinner than the blur. That is the whole model, and it is correct at
//! every aspect ratio, including the two this crate needs: 115 by 3 and 3 by
//! 96.
//!
//! [`profile`] returns one axis's samples. [`box_shadow`] takes the outer
//! product of two of them and emits a grid of quads whose vertex alpha is
//! that product. There are no corners to round, so there is nothing to
//! overlap, and the mesh is symmetric by construction because the sample
//! positions are.
//!
//! [`STEPS`] says why the ramps are subdivided rather than drawn as one quad.

use egui::epaint::{Color32, Mesh, Rect, Vertex, WHITE_UV, pos2};

use gorgon_petra::token::ShadowGeometry;

/// How many quads each ramp of the trapezoid is cut into.
///
/// One quad per ramp would be wrong, and the reason is that a GPU
/// interpolates a *triangle* linearly while the field being drawn is
/// bilinear. Over one cell the two differ most at the middle of the shared
/// diagonal, by a quarter of the product of the cell's two weight steps. Cut
/// each ramp into `STEPS`, and that error falls as `STEPS²`: at four steps it
/// is `1/64` of the shadow's peak alpha, which for `shadow.raised` in the
/// light theme is a quarter of one level out of 255 and cannot be seen.
///
/// The cost is a mesh of about a hundred vertices per bar per frame, against
/// the ten a single quad would need. A focus caret draws at most two bars.
const STEPS: usize = 4;

/// The sample positions and weights of one axis of the trapezoid.
///
/// `lo` and `hi` bound the caster on this axis and `blur` is the token's,
/// unclamped. Returns `(position, weight)` in increasing position, weights
/// starting and ending at zero and holding `min(1, (hi - lo) / blur)` between.
///
/// Written out rather than folded into [`box_shadow`] so it can be tested on
/// its own: the thin case, where the plateau drops below one, is the case
/// epaint gets wrong and is worth naming in a test rather than inferring
/// from a picture.
///
/// A `blur` of zero has no ramp at all, and the caller handles that before
/// calling; this function would divide by it.
fn profile(lo: f32, hi: f32, blur: f32) -> Vec<(f32, f32)> {
    debug_assert!(blur > 0.0, "profile is undefined without a blur");
    let width = (hi - lo).max(0.0);
    // The convolution of a box of width `width` with a box kernel of width
    // `blur`: support `width + blur` wide, plateau `|width - blur|` wide,
    // both centred on the caster's own centre.
    let centre = 0.5 * (lo + hi);
    let support = 0.5 * (width + blur);
    let plateau = 0.5 * (width - blur).abs();
    let peak = (width / blur).min(1.0);
    let (s0, s1) = (centre - support, centre - plateau);
    let (s2, s3) = (centre + plateau, centre + support);

    let mut out = Vec::with_capacity(2 * STEPS + 2);
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        out.push((s0 + (s1 - s0) * t, peak * t));
    }
    // Skipped when the caster is exactly as wide as the blur, where the
    // trapezoid degenerates to a triangle and `s1` already stands at `s2`.
    if s2 > s1 {
        out.push((s2, peak));
    }
    for i in 1..=STEPS {
        let t = i as f32 / STEPS as f32;
        out.push((s2 + (s3 - s2) * t, peak * (1.0 - t)));
    }
    out
}

/// The mesh of `geometry`'s shadow cast by `caster`, in `color`.
///
/// `color` is the token's, at its own alpha; the mesh scales it down across
/// the falloff and never up. `offset` displaces the caster and `spread` grows
/// it, both before the blur, which is the order CSS uses and the order
/// `epaint::Shadow::as_shape` uses.
///
/// An empty mesh comes back when there is nothing to draw — a caster with no
/// area, or a geometry with no blur and no spread — so a caller can add it
/// unconditionally.
#[must_use]
pub fn box_shadow(caster: Rect, geometry: ShadowGeometry, color: Color32) -> Mesh {
    let ShadowGeometry {
        offset,
        blur,
        spread,
    } = geometry;
    let rect = caster
        .translate(egui::vec2(f32::from(offset[0]), f32::from(offset[1])))
        .expand(f32::from(spread));
    let mut mesh = Mesh::default();
    if !rect.is_positive() || color.a() == 0 {
        return mesh;
    }
    if blur == 0 {
        // No falloff to build. One flat quad, which is what a zero blur is.
        mesh.add_colored_rect(rect, color);
        return mesh;
    }
    let blur = f32::from(blur);
    let xs = profile(rect.min.x, rect.max.x, blur);
    let ys = profile(rect.min.y, rect.max.y, blur);

    mesh.vertices.reserve(xs.len() * ys.len());
    for &(y, wy) in &ys {
        for &(x, wx) in &xs {
            mesh.vertices.push(Vertex {
                pos: pos2(x, y),
                uv: WHITE_UV,
                color: color.gamma_multiply(wx * wy),
            });
        }
    }
    let stride = xs.len() as u32;
    mesh.indices.reserve(6 * (xs.len() - 1) * (ys.len() - 1));
    for j in 0..ys.len() as u32 - 1 {
        for i in 0..stride - 1 {
            let (a, b) = (j * stride + i, j * stride + i + 1);
            let (c, d) = (a + stride, b + stride);
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(b, d, c);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `profile` is symmetric about the caster's centre, which is the
    /// property epaint loses at a corner and the reason this module exists.
    ///
    /// Falsify by nudging one of the two ramps: the pairing below is exact,
    /// not approximate, because both ramps are built from the same `t`.
    #[test]
    fn a_profile_mirrors_itself_about_the_casters_centre() {
        for (lo, hi, blur) in [(0.0, 3.0, 6.0), (0.0, 115.0, 6.0), (0.0, 6.0, 6.0)] {
            let p = profile(lo, hi, blur);
            let centre = 0.5 * (lo + hi);
            for (front, back) in p.iter().zip(p.iter().rev()) {
                assert!(
                    (front.0 - centre + (back.0 - centre)).abs() < 1e-4,
                    "{lo}..{hi} blur {blur}: {front:?} does not mirror {back:?}"
                );
                assert!(
                    (front.1 - back.1).abs() < 1e-6,
                    "{lo}..{hi} blur {blur}: weights {front:?} and {back:?} differ"
                );
            }
        }
    }

    /// A caster thinner than its blur cannot reach full alpha, and the
    /// plateau says by how much: a 3-unit bar under a 6-unit blur peaks at a
    /// half.
    ///
    /// This is the arithmetic epaint replaces with a clamp, and it is what
    /// makes the difference between a bar's shadow and a card's honest
    /// rather than hidden.
    #[test]
    fn a_caster_thinner_than_its_blur_peaks_below_one() {
        let thin = profile(0.0, 3.0, 6.0);
        let peak = thin.iter().map(|&(_, w)| w).fold(0.0_f32, f32::max);
        assert!(
            (peak - 0.5).abs() < 1e-6,
            "a 3-unit caster under a 6-unit blur should peak at 0.5, got {peak}"
        );
        let wide = profile(0.0, 115.0, 6.0);
        let peak = wide.iter().map(|&(_, w)| w).fold(0.0_f32, f32::max);
        assert!(
            (peak - 1.0).abs() < 1e-6,
            "a caster wider than its blur should reach full alpha, got {peak}"
        );
    }

    /// The support is `width + blur` wide and centred on the caster, so the
    /// shadow reaches exactly `blur / 2` past each edge — the reach
    /// `epaint::Shadow::margin` declares and `paint::indicator_outset`
    /// budgets for.
    #[test]
    fn a_profile_reaches_half_the_blur_past_each_edge() {
        let p = profile(10.0, 13.0, 6.0);
        assert!((p.first().expect("a profile has samples").0 - 7.0).abs() < 1e-4);
        assert!((p.last().expect("a profile has samples").0 - 16.0).abs() < 1e-4);
        assert!(p.first().expect("a profile has samples").1 == 0.0);
        assert!(p.last().expect("a profile has samples").1 == 0.0);
    }

    /// Every quad in the mesh is wound the same way and no vertex is
    /// orphaned, which is the mesh-level version of "there is nothing to
    /// overlap".
    #[test]
    fn the_mesh_covers_its_grid_once_with_no_stray_vertices() {
        let geometry = ShadowGeometry {
            offset: [0, 2],
            blur: 6,
            spread: 0,
        };
        let bar = Rect::from_min_size(pos2(100.0, 200.0), egui::vec2(3.0, 40.0));
        let mesh = box_shadow(bar, geometry, Color32::from_black_alpha(64));
        assert!(
            mesh.is_valid(),
            "the mesh indexes a vertex it does not have"
        );
        let cells = (2 * STEPS + 1) * (2 * STEPS + 1);
        assert_eq!(
            mesh.indices.len(),
            6 * cells,
            "the grid should be {cells} quads of two triangles each"
        );
        let mut seen = vec![false; mesh.vertices.len()];
        for &i in &mesh.indices {
            seen[i as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "a vertex is in no triangle");
    }

    /// The mesh is symmetric about the caster's own vertical centre line, so
    /// no corner can be darker than the one facing it. This is the
    /// regression the operator reported, stated as an assertion.
    ///
    /// Falsify by drawing the shadow through `epaint::Shadow::as_shape`
    /// again: its corner arcs are generated in path order and the seam lands
    /// at one of them.
    #[test]
    fn the_mesh_has_no_corner_darker_than_its_mirror() {
        let geometry = ShadowGeometry {
            offset: [0, 2],
            blur: 6,
            spread: 0,
        };
        let bar = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(3.0, 40.0));
        let mesh = box_shadow(bar, geometry, Color32::from_black_alpha(64));
        let axis = 1.5; // the bar's own centre in x
        for v in &mesh.vertices {
            let twin = mesh
                .vertices
                .iter()
                .find(|w| {
                    (w.pos.x - (2.0 * axis - v.pos.x)).abs() < 1e-3
                        && (w.pos.y - v.pos.y).abs() < 1e-3
                })
                .expect("every vertex has a mirror across the caster's centre");
            assert_eq!(
                v.color, twin.color,
                "{:?} is not the colour of its mirror {:?}",
                v.pos, twin.pos
            );
        }
    }

    /// A degenerate caster draws nothing rather than a stray triangle.
    #[test]
    fn a_caster_with_no_area_draws_nothing() {
        let geometry = ShadowGeometry {
            offset: [0, 2],
            blur: 6,
            spread: 0,
        };
        let flat = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(0.0, 40.0));
        assert!(
            box_shadow(flat, geometry, Color32::from_black_alpha(64))
                .indices
                .is_empty()
        );
        let bar = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(3.0, 40.0));
        assert!(
            box_shadow(bar, geometry, Color32::TRANSPARENT)
                .indices
                .is_empty(),
            "a transparent shadow is no shadow"
        );
    }
}
