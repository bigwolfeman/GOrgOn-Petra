//! Throwaway visual spike: debossed hairline rules and box bevels.
//!
//! Not shipped code. Every literal colour value in this file is a spike
//! value, marked as such at its use site, and would need to become a named
//! token before anything here shipped. This module (and the one line that
//! declares it in `shots.rs`) is deleted once the operator has picked a
//! direction from the pictures it produces.
//!
//! # Why a stack of 1px strokes, not a gradient
//!
//! `gorgon_petra::token::slot` (`slot.rs:160`) says the `gradient-stop-1`/
//! `-2` slots were retired with no painter ever built for them, and none
//! exists today. So every soft edge here is a column of thin
//! `background`-filled bars, each bound to its own literal colour, stacked
//! with zero row spacing — cheap, digest-visible, and the shape that
//! module's doc names as "very likely how we would ship it anyway".
//!
//! # Why bars, not the four border-edge slots, for rules
//!
//! `gorgon-petra-egui`'s `paint.rs` always draws `border-top`/`-right`/
//! `-bottom`/`-left` at exactly one *device* pixel
//! (`device_snapped_width(1.0, scale)`), snapped to the device grid,
//! regardless of the node's own size — it cannot draw a 2- or 4-device-
//! pixel-wide edge on its own. A `background`-filled bar with an explicit
//! fixed height sidesteps that: every bar height in this file is expressed
//! in **device pixels** (`px / scale` logical points), so the same
//! specimen geometry, captured at a different `scale`, occupies a coarser
//! or finer device grid — which is the axis the 1x/2x pair exists to
//! probe. Box specimens use the border-edge slots instead (see
//! [`box_node`]), because a box needs four independently coloured edges
//! plus a corner radius, and whether that combination even renders
//! correctly is one of the things this spike is checking.

use gorgon_petra::component::{field, primary_button, tag, text};
use gorgon_petra::geom::Align;
use gorgon_petra::token::{
    ColorValue, ShapeValue, Theme, ThemeMode, TokenName, TokenValue, dark, light,
    standard_vocabulary,
};
use gorgon_petra::tree::{
    AxisConstraint, Constraints, InsetRefs, NodeKind, Props, TrackSize, ViewNode,
};

use crate::page::common::{filled_body, row, sp, tok, wrapped};

use super::Camera;

/// Panel width every sheet is built at: narrow enough to read on a phone,
/// wide enough that a 200-unit box specimen sits inside it with margin.
const PANEL_WIDTH: f32 = 380.0;

// ===================================================================
// Colour: blend the theme's own surface toward black or white.
// ===================================================================

/// Blend `base` toward black (`t < 0`) or white (`t > 0`) by `|t|`, `t`
/// clamped to `[-1, 1]`.
///
/// A naive linear blend in the colour's own (linear-light) channels, not a
/// perceptual model — good enough to tell two strokes apart in a
/// screenshot, which is this spike's entire job. Every call site names the
/// amount as a comment because every one of these numbers is a spike
/// literal that would need a name (`border.bevel-shadow`, or whatever the
/// operator picks) before it could ship.
fn shade(base: ColorValue, t: f32) -> ColorValue {
    let t = t.clamp(-1.0, 1.0);
    let target = if t < 0.0 { 0.0 } else { 1.0 };
    let amount = t.abs();
    ColorValue {
        r: base.r + (target - base.r) * amount,
        g: base.g + (target - base.g) * amount,
        b: base.b + (target - base.b) * amount,
        a: base.a,
    }
}

/// The shipped theme for `mode`.
fn base_theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::Light => light(),
        ThemeMode::Dark => dark(),
    }
}

/// The value already bound to `name` in `theme`, as a colour.
///
/// # Panics
/// If `theme` does not bind `name`, or binds it to something that is not a
/// colour — both authoring mistakes in this spike file, never a fact about
/// the shipped theme.
fn theme_color(theme: &Theme, name: &str) -> ColorValue {
    let key = TokenName::new(name).expect("shipped token names are well-formed");
    match theme.value(&key) {
        Some(TokenValue::Color(c)) => *c,
        other => panic!("{name}: expected a colour token, found {other:?}"),
    }
}

/// `base_theme(mode)`, extended with `extra` literal spike colours under
/// new names.
///
/// `Theme::build` only requires every *vocabulary-declared* name to be
/// assigned — a name `values` carries that the vocabulary does not declare
/// is not an error (see `theme.rs`'s own doc on `Theme::build`). And
/// `gorgon-petra-egui`'s `Host` composes the vocabulary it validates trees
/// against as `Vocabulary::from_theme(theme)` (`host.rs`'s
/// `composed_vocabulary`), which walks the theme's own `values`, not
/// `standard_vocabulary()` — so an extra name landed here is legal for
/// `Theme::build` and reachable from a tree this host will accept, with no
/// vocabulary edited anywhere.
///
/// # Panics
/// If adding `extra` somehow broke completeness against
/// `standard_vocabulary()`, which would be a bug in this function, not in
/// the caller.
fn extend_theme(mode: ThemeMode, extra: Vec<(String, ColorValue)>) -> Theme {
    extend_theme_values(
        mode,
        extra
            .into_iter()
            .map(|(name, color)| (name, TokenValue::Color(color)))
            .collect(),
    )
}

/// [`extend_theme`], generalised to any [`TokenValue`] — added 2026-09-07
/// for the radius sweep, which needs spike `Shape` tokens
/// (`TokenValue::Shape`) rather than colours. `extend_theme` is now a
/// thin wrapper over this so its existing callers and their
/// already-verified behaviour are unchanged.
fn extend_theme_values(mode: ThemeMode, extra: Vec<(String, TokenValue)>) -> Theme {
    let base = base_theme(mode);
    let mut values = base.values().clone();
    for (name, value) in extra {
        let key = TokenName::new(name).expect("spike token names are well-formed");
        values.insert(key, value);
    }
    Theme::build(mode, &standard_vocabulary(), values)
        .expect("extending a complete theme with extra names cannot make it incomplete")
}

// ===================================================================
// Shared tree helpers.
// ===================================================================

/// A muted caption line: what the specimen below it is.
fn label(key: &str, content: impl Into<String>) -> ViewNode {
    let mut node = text(key, content);
    node.props
        .tokens
        .insert("foreground".into(), tok("text.muted"));
    node
}

/// One filled strip, `px` **device** pixels tall regardless of `scale` —
/// see the module doc for why rule bars are built this way instead of
/// through the border-edge slots.
fn bar(key: &str, px: f32, scale: f32, color: TokenName) -> ViewNode {
    let node = ViewNode::new(NodeKind::Grid, key).with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        ..Props::default()
    });
    let mut node = node;
    node.props.tokens.insert("background".into(), color);
    node.with_constraints(Constraints {
        vertical: AxisConstraint {
            min: Some(px / scale),
            max: Some(px / scale),
            priority: 0,
        },
        ..Constraints::default()
    })
}

/// A column of [`bar`]s, stacked with zero gap and stretched to the full
/// specimen width — the "rule" itself.
fn rule(key: &str, bars: Vec<ViewNode>) -> ViewNode {
    filled_body(key, None, bars)
}

/// One labelled specimen: caption, a line of body text, the rule, another
/// line of body text — a rule judged in the context it separates, never
/// floating alone.
fn specimen(key: &str, caption: &str, the_rule: ViewNode) -> ViewNode {
    filled_body(
        key,
        sp("spacing.2xs"),
        vec![
            label(&format!("{key}-caption"), caption.to_owned()),
            body_line(
                &format!("{key}-above"),
                "The quick brown fox jumps over the lazy dog.",
            ),
            the_rule,
            body_line(
                &format!("{key}-below"),
                "Pack my box with five dozen liquor jugs.",
            ),
        ],
    )
}

fn body_line(key: &str, content: impl Into<String>) -> ViewNode {
    wrapped(key, content)
}

/// A sheet: a title caption over a column of specimens, at [`PANEL_WIDTH`],
/// on `ground` (a real shipped token — the page's own colour, not a spike
/// literal).
fn sheet(key: &str, title: &str, specimens: Vec<ViewNode>, ground: TokenName) -> ViewNode {
    sheet_with_gap(key, title, specimens, ground, sp("spacing.2xl"))
}

/// [`sheet`], with the gap between specimens exposed — [`box_sheet`] has
/// seven specimens under the engine's fixed 900-unit capture viewport
/// (`catalog::WINDOW`, which every [`Camera`] pass re-stamps the frame to;
/// see that sheet's own doc for how this was found and why a taller
/// viewport was not the fix) and needs a tighter one than the five- and
/// three-specimen sheets do.
fn sheet_with_gap(
    key: &str,
    title: &str,
    specimens: Vec<ViewNode>,
    ground: TokenName,
    gap: Option<TokenName>,
) -> ViewNode {
    let mut children = vec![label(&format!("{key}-title"), title.to_owned())];
    children.extend(specimens);
    let mut page = filled_body(key, gap, children).with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(PANEL_WIDTH),
            max: Some(PANEL_WIDTH),
            priority: 0,
        },
        ..Constraints::default()
    });
    page.props.padding = Some(InsetRefs::all(tok("spacing.xl")));
    page.props.tokens.insert("background".into(), ground);
    page
}

// ===================================================================
// Sheet 1/2: material — hard bevel vs. soft band.
// ===================================================================

/// The five material specimens (flat, bevel-1, bevel-2, band-4, band-6),
/// on `mode`'s shipped `surface.layer-one`.
///
/// Not `surface.base`: a first render on `surface.base` found that in
/// light mode it is pure `#ffffff`, so "blend toward white" for a
/// highlight is a no-op — every material's highlight bar was literally
/// invisible, white-on-white, and the sheet was silently comparing shadow
/// lines only. `surface.layer-one` (`#f2f2f2` light / `#262626` dark) has
/// real headroom in both directions in both themes, so the material
/// question (hard bevel vs. soft band) gets a fair hearing. This is the
/// light-theme mirror of the dark-theme "no headroom" problem the operator
/// already named; see the report for the numbers. [`dark_strategies_sheet`]
/// keeps `surface.base` on purpose — that sheet's whole point is the
/// near-black, no-headroom case.
fn materials_sheet(mode: ThemeMode, scale: f32) -> (ViewNode, Theme) {
    let base = base_theme(mode);
    let surface = theme_color(&base, "surface.layer-one");

    // Spike literal: the hard-bevel shadow, 35% of the way from the
    // surface toward black. Would become `border.bevel-shadow` (or
    // whatever name the operator picks) if bevel-1/bevel-2 ship.
    let shadow = shade(surface, -0.35);
    // Spike literal: the hard-bevel highlight, 50% toward white. On a
    // light surface (already white) this clamps to white — see the
    // report's note on the light-theme highlight ceiling.
    let highlight = shade(surface, 0.5);

    // Spike literals: band-4's four steps, spread across amplitude 0.45.
    let band4_t = [-0.45_f32, -0.15, 0.15, 0.45];
    // Spike literals: band-6's six steps, a wider spread (amplitude 0.6)
    // — "wider falloff" per the brief.
    let band6_t = [-0.6_f32, -0.36, -0.12, 0.12, 0.36, 0.6];

    let mut extra: Vec<(String, ColorValue)> = vec![
        ("spike.mat-shadow".into(), shadow),
        ("spike.mat-highlight".into(), highlight),
    ];
    for (i, t) in band4_t.iter().enumerate() {
        extra.push((format!("spike.mat-quad-{i:02}"), shade(surface, *t)));
    }
    for (i, t) in band6_t.iter().enumerate() {
        extra.push((format!("spike.mat-hex-{i:02}"), shade(surface, *t)));
    }
    let theme = extend_theme(mode, extra);

    let t = |name: &str| TokenName::new(name.to_owned()).expect("spike name");

    let flat = specimen(
        "flat-1px",
        "1. flat-1px — today's single stroke (the control)",
        rule(
            "flat-1px-rule",
            vec![bar("b0", 1.0, scale, tok("border.subtle"))],
        ),
    );
    let bevel1 = specimen(
        "bevel-1",
        "2. bevel-1 — 1px shadow, 1px highlight (2px total)",
        rule(
            "bevel-1-rule",
            vec![
                bar("b0", 1.0, scale, t("spike.mat-shadow")),
                bar("b1", 1.0, scale, t("spike.mat-highlight")),
            ],
        ),
    );
    let bevel2 = specimen(
        "bevel-2",
        "3. bevel-2 — 2px shadow, 2px highlight (4px total)",
        rule(
            "bevel-2-rule",
            vec![
                bar("b0", 2.0, scale, t("spike.mat-shadow")),
                bar("b1", 2.0, scale, t("spike.mat-highlight")),
            ],
        ),
    );
    let band4 = specimen(
        "band-4",
        "4. band-4 — 4 stacked 1px strokes, dark to light",
        rule(
            "band-4-rule",
            (0..4)
                .map(|i| {
                    bar(
                        &format!("b{i}"),
                        1.0,
                        scale,
                        t(&format!("spike.mat-quad-{i:02}")),
                    )
                })
                .collect(),
        ),
    );
    let band6 = specimen(
        "band-6",
        "5. band-6 — 6 stacked 1px strokes, wider falloff",
        rule(
            "band-6-rule",
            (0..6)
                .map(|i| {
                    bar(
                        &format!("b{i}"),
                        1.0,
                        scale,
                        t(&format!("spike.mat-hex-{i:02}")),
                    )
                })
                .collect(),
        ),
    );

    let title = match mode {
        ThemeMode::Light => "Material — light",
        ThemeMode::Dark => "Material — dark",
    };
    let root = sheet(
        "materials",
        title,
        vec![flat, bevel1, bevel2, band4, band6],
        tok("surface.layer-one"),
    );
    (root, theme)
}

// ===================================================================
// Sheet 3: dark strategies — bevel-1 geometry, three highlight/shadow
// arrangements.
// ===================================================================

fn dark_strategies_sheet(scale: f32) -> (ViewNode, Theme) {
    let base = base_theme(ThemeMode::Dark);
    let surface = theme_color(&base, "surface.base");

    // Spike literal: same-order's shadow, compressed to 15% toward black
    // — "amplitudes compressed toward the surface value" per the brief.
    // Near-black already; this is close to a no-op, which is the point.
    let shadow_compressed = shade(surface, -0.15);
    // Spike literal: same-order's (and highlight-only's, and ridge's)
    // highlight, 35% toward white — the one channel with real headroom on
    // a dark surface, so it carries the effect.
    let highlight = shade(surface, 0.35);

    let theme = extend_theme(
        ThemeMode::Dark,
        vec![
            ("spike.dark-shadow".into(), shadow_compressed),
            ("spike.dark-highlight".into(), highlight),
        ],
    );
    let t = |name: &str| TokenName::new(name.to_owned()).expect("spike name");

    let same_order = specimen(
        "same-order",
        "1. same-order — shadow above, highlight below, both compressed",
        rule(
            "same-order-rule",
            vec![
                bar("b0", 1.0, scale, t("spike.dark-shadow")),
                bar("b1", 1.0, scale, t("spike.dark-highlight")),
            ],
        ),
    );
    let highlight_only = specimen(
        "highlight-only",
        "2. highlight-only — shadow dropped, one lighter stroke",
        rule(
            "highlight-only-rule",
            vec![bar("b0", 1.0, scale, t("spike.dark-highlight"))],
        ),
    );
    let ridge = specimen(
        "ridge",
        "3. ridge — inverted: light above, dark below (reads raised)",
        rule(
            "ridge-rule",
            vec![
                bar("b0", 1.0, scale, t("spike.dark-highlight")),
                bar("b1", 1.0, scale, t("spike.dark-shadow")),
            ],
        ),
    );

    let root = sheet(
        "dark-strategies",
        "Dark strategies (bevel-1 geometry)",
        vec![same_order, highlight_only, ridge],
        tok("surface.base"),
    );
    (root, theme)
}

// ===================================================================
// Sheets 3/4: full-box bevels.
// ===================================================================

/// One bevelled box: shadow on the top and left edges, highlight on the
/// bottom and right edges — top-left light azimuth, pressed in.
///
/// Border-edge slots, not stacked bars: a box needs four independently
/// coloured edges and (for the radius specimens) a rounded corner, and the
/// border-edge slots are the only primitive that draws a coloured edge
/// with a `radius` binding in play at all — whether the two compose
/// correctly is exactly what the radius specimens are checking, and
/// `paint.rs`'s `edge_segment` does not take `corner_radius` as a
/// parameter, so the answer may be "no"; see the report.
/// The four tokens that describe a box's surface, bundled because they are
/// one concept and because eight positional arguments is one too many.
struct BoxSkin {
    radius: TokenName,
    background: TokenName,
    shadow: TokenName,
    highlight: TokenName,
}

fn box_node(key: &str, w: f32, h: f32, skin: BoxSkin, child: Option<ViewNode>) -> ViewNode {
    let BoxSkin {
        radius,
        background,
        shadow,
        highlight,
    } = skin;
    let mut node = ViewNode::new(NodeKind::Grid, key).with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    });
    node.props.tokens.insert("background".into(), background);
    node.props.tokens.insert("radius".into(), radius);
    node.props
        .tokens
        .insert("border-top".into(), shadow.clone());
    node.props.tokens.insert("border-left".into(), shadow);
    node.props
        .tokens
        .insert("border-bottom".into(), highlight.clone());
    node.props.tokens.insert("border-right".into(), highlight);
    if let Some(kid) = child {
        node = node.with_children(vec![kid]);
    }
    node.with_constraints(Constraints {
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
    })
}

/// A labelled box specimen: caption above, the box below. No flanking body
/// text — a card-sized box is judged as a shape, not as something dividing
/// prose.
fn box_specimen(key: &str, caption: &str, the_box: ViewNode) -> ViewNode {
    filled_body(
        key,
        sp("spacing.xs"),
        vec![
            label(&format!("{key}-caption"), caption.to_owned()),
            the_box,
        ],
    )
}

/// The seven box specimens (amplitude x3, radius x2 extra, asymmetry,
/// nesting — see the module doc), on `mode`.
fn box_sheet(mode: ThemeMode) -> (ViewNode, Theme) {
    let base = base_theme(mode);
    // Boxes sit on a raised panel (`surface.layer-one`), not the page's
    // own `surface.base`: a "card-sized box... not a thin rule" is a tile
    // on a page, and — unlike `surface.base` in dark mode, which is
    // already near-black — layer-one has real headroom in both directions
    // in both themes, so the amplitude/asymmetry specimens are not
    // pre-decided by having nowhere to go before the strategy in question
    // is even tested. `dark-strategies` sheet still uses `surface.base`
    // on purpose: that sheet's whole point is the near-black no-headroom
    // case.
    let panel = theme_color(&base, "surface.layer-one");
    let inner_panel = theme_color(&base, "surface.layer-two");

    // Spike literals: three amplitudes, each producing a symmetric
    // shadow/highlight pair around `panel`.
    const LOW: f32 = 0.06;
    const BASE: f32 = 0.30;
    const HIGH: f32 = 0.75;
    // Spike literal: the asymmetric specimen's highlight, half of `BASE`
    // — "highlight at roughly half the shadow's delta" per the brief.
    const ASYM_HIGHLIGHT: f32 = BASE / 2.0;

    let mut extra: Vec<(String, ColorValue)> = vec![
        ("spike.box-low-shadow".into(), shade(panel, -LOW)),
        ("spike.box-low-highlight".into(), shade(panel, LOW)),
        ("spike.box-base-shadow".into(), shade(panel, -BASE)),
        ("spike.box-base-highlight".into(), shade(panel, BASE)),
        ("spike.box-high-shadow".into(), shade(panel, -HIGH)),
        ("spike.box-high-highlight".into(), shade(panel, HIGH)),
        (
            "spike.box-asym-highlight".into(),
            shade(panel, ASYM_HIGHLIGHT),
        ),
        ("spike.box-inner-shadow".into(), shade(inner_panel, -BASE)),
        ("spike.box-inner-highlight".into(), shade(inner_panel, BASE)),
    ];
    // The nested specimen's inner box reuses the base amplitude on the
    // inner panel tone, already pushed above; nothing else to add.
    let theme = extend_theme(mode, std::mem::take(&mut extra));
    let t = |name: &str| TokenName::new(name.to_owned()).expect("spike name");

    // 200x80, not the 200x140 first tried: the engine's fixed 900-unit-tall
    // capture viewport (`catalog::WINDOW`, re-stamped onto every pass by
    // `Camera`'s private `step`, which this spike could not override
    // without editing shared, non-spike code) does not fit seven
    // 140-tall specimens plus a caption and page chrome — the first render
    // silently clipped specimens 5-7 off-frame with no error, since
    // `shoot()`'s only assertion is "captured successfully", not "captured
    // everything". Caught by counting specimens in the picture, not by any
    // assertion.
    const W: f32 = 200.0;
    const H: f32 = 80.0;

    let baseline = box_specimen(
        "box-baseline",
        "1. baseline — amp 0.30 symmetric, radius 0, single bevel",
        box_node(
            "box-baseline-box",
            W,
            H,
            BoxSkin {
                radius: tok("shape.corner-none"),
                background: tok("surface.layer-one"),
                shadow: t("spike.box-base-shadow"),
                highlight: t("spike.box-base-highlight"),
            },
            None,
        ),
    );
    let amp_low = box_specimen(
        "box-amp-low",
        "2. amplitude: low (0.06) — machined-aluminium test",
        box_node(
            "box-amp-low-box",
            W,
            H,
            BoxSkin {
                radius: tok("shape.corner-none"),
                background: tok("surface.layer-one"),
                shadow: t("spike.box-low-shadow"),
                highlight: t("spike.box-low-highlight"),
            },
            None,
        ),
    );
    let amp_high = box_specimen(
        "box-amp-high",
        "3. amplitude: high (0.75) — Win95-extreme test",
        box_node(
            "box-amp-high-box",
            W,
            H,
            BoxSkin {
                radius: tok("shape.corner-none"),
                background: tok("surface.layer-one"),
                shadow: t("spike.box-high-shadow"),
                highlight: t("spike.box-high-highlight"),
            },
            None,
        ),
    );
    let radius_md = box_specimen(
        "box-radius-md",
        "4. corner radius: shape.corner-md (8px)",
        box_node(
            "box-radius-md-box",
            W,
            H,
            BoxSkin {
                radius: tok("shape.corner-md"),
                background: tok("surface.layer-one"),
                shadow: t("spike.box-base-shadow"),
                highlight: t("spike.box-base-highlight"),
            },
            None,
        ),
    );
    let radius_lg = box_specimen(
        "box-radius-lg",
        "5. corner radius: shape.corner-lg (12px)",
        box_node(
            "box-radius-lg-box",
            W,
            H,
            BoxSkin {
                radius: tok("shape.corner-lg"),
                background: tok("surface.layer-one"),
                shadow: t("spike.box-base-shadow"),
                highlight: t("spike.box-base-highlight"),
            },
            None,
        ),
    );
    let asymmetric = box_specimen(
        "box-asymmetric",
        "6. asymmetry: highlight at half shadow delta (0.30 / 0.15)",
        box_node(
            "box-asymmetric-box",
            W,
            H,
            BoxSkin {
                radius: tok("shape.corner-none"),
                background: tok("surface.layer-one"),
                shadow: t("spike.box-base-shadow"),
                highlight: t("spike.box-asym-highlight"),
            },
            None,
        ),
    );
    let inner = box_node(
        "box-nested-inner",
        W - 2.0 * 12.0,
        H - 2.0 * 12.0,
        BoxSkin {
            radius: tok("shape.corner-none"),
            background: tok("surface.layer-two"),
            shadow: t("spike.box-inner-shadow"),
            highlight: t("spike.box-inner-highlight"),
        },
        None,
    );
    let mut outer = box_node(
        "box-nested-outer",
        W,
        H,
        BoxSkin {
            radius: tok("shape.corner-none"),
            background: tok("surface.layer-one"),
            shadow: t("spike.box-base-shadow"),
            highlight: t("spike.box-base-highlight"),
        },
        Some(inner),
    );
    outer.props.padding = Some(InsetRefs::all(tok("spacing.md")));
    let nested = box_specimen(
        "box-nested",
        "7. nesting: outer + inner bevel (the banned combo)",
        outer,
    );

    let title = match mode {
        ThemeMode::Light => "Box bevels — light",
        ThemeMode::Dark => "Box bevels — dark",
    };
    let root = sheet_with_gap(
        "boxes",
        title,
        vec![
            baseline, amp_low, amp_high, radius_md, radius_lg, asymmetric, nested,
        ],
        tok("surface.base"),
        sp("spacing.2xs"),
    );
    (root, theme)
}

// ===================================================================
// Sheets: container A/B — full-box deboss vs. horizontal-only rules.
// ===================================================================
//
// Added 2026-09-07 as a direct correction to the earlier box-matrix scope:
// the operator has NOT decided the deboss applies to full box borders —
// it is a candidate under test against the alternative of no box border
// at all, shadow-lifted off the ground instead. These two functions build
// identical content under two container treatments so the only variable
// in the comparison is the treatment.

/// The card's inner content: a title, a header rule, three rows each
/// followed by a row-divider rule (bar bevel-1 geometry, the material
/// sheet's own pick) — "real content", not a blank box. Shared between
/// treatment A and treatment B so the only difference between the two
/// pictures is the container.
fn card_content(scale: f32, rule_shadow: TokenName, rule_highlight: TokenName) -> ViewNode {
    let seam = |key: &str| {
        rule(
            key,
            vec![
                bar("b0", 1.0, scale, rule_shadow.clone()),
                bar("b1", 1.0, scale, rule_highlight.clone()),
            ],
        )
    };
    let mut node = filled_body(
        "card-inner",
        sp("spacing.sm"),
        vec![
            label("card-title", "Panel title"),
            seam("card-header-rule"),
            body_line("card-row1", "First row of realistic content in the panel."),
            seam("card-row1-rule"),
            body_line("card-row2", "Second row of realistic content in the panel."),
            seam("card-row2-rule"),
            body_line("card-row3", "Third row of realistic content in the panel."),
        ],
    );
    node.props.padding = Some(InsetRefs::all(tok("spacing.md")));
    node
}

const CARD_W: f32 = 260.0;
const CARD_H: f32 = 230.0;
/// The amplitude used for both treatments' edges/rules — chosen after
/// looking at the box-variation matrix (`box_sheet`), see the report for
/// which specimen this reproduces and why `shape.corner-md` rather than
/// `shape.corner-none` or `-lg`.
const AB_AMPLITUDE: f32 = 0.30;

fn ab_theme(mode: ThemeMode) -> (Theme, TokenName, TokenName) {
    let base = base_theme(mode);
    let panel = theme_color(&base, "surface.layer-one");
    let shadow = shade(panel, -AB_AMPLITUDE);
    let highlight = shade(panel, AB_AMPLITUDE);
    let theme = extend_theme(
        mode,
        vec![
            ("spike.ab-shadow".into(), shadow),
            ("spike.ab-highlight".into(), highlight),
        ],
    );
    (
        theme,
        TokenName::new("spike.ab-shadow").expect("spike name"),
        TokenName::new("spike.ab-highlight").expect("spike name"),
    )
}

/// Treatment A: full four-sided deboss. Light from top-left, pressed in —
/// shadow on top+left, highlight on bottom+right, the same rule
/// `box_node`'s doc states for every box specimen in this file.
fn container_a_fullbox(mode: ThemeMode, scale: f32) -> (ViewNode, Theme) {
    let (theme, shadow, highlight) = ab_theme(mode);
    let content = card_content(scale, shadow.clone(), highlight.clone());
    let card = box_node(
        "card-a",
        CARD_W,
        CARD_H,
        BoxSkin {
            radius: tok("shape.corner-md"),
            background: tok("surface.layer-one"),
            shadow,
            highlight,
        },
        Some(content),
    );
    let title = match mode {
        ThemeMode::Light => "Container A — full-box deboss (light)",
        ThemeMode::Dark => "Container A — full-box deboss (dark)",
    };
    let root = sheet("container-a", title, vec![card], tok("surface.base"));
    (root, theme)
}

/// Treatment B: no box border at all. Separated from the ground by
/// `surface.layer-one` plus a real shipped `shadow.raised` — the same
/// token `component::section` binds for a raised panel — and the only
/// debossed lines anywhere in the picture are the horizontal rules inside
/// it (header + row dividers), at the same amplitude as A's edges.
fn container_b_horizontal(mode: ThemeMode, scale: f32) -> (ViewNode, Theme) {
    let (theme, shadow, highlight) = ab_theme(mode);
    let content = card_content(scale, shadow, highlight);
    let card = floating_card(content);
    let title = match mode {
        ThemeMode::Light => "Container B — horizontal rules only, no box border (light)",
        ThemeMode::Dark => "Container B — horizontal rules only, no box border (dark)",
    };
    let root = sheet("container-b", title, vec![card], tok("surface.base"));
    (root, theme)
}

/// The treatment-B card shell: background, radius and a real `shadow.raised`
/// binding, and deliberately **no** `border-*`/`border` slot bound at all —
/// absent means no rule, per `paint.rs`'s own doc on [`UNDERLINE_SLOT`]'s
/// sibling slots.
fn floating_card(content: ViewNode) -> ViewNode {
    let mut card = ViewNode::new(NodeKind::Grid, "card-b").with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    });
    card.props
        .tokens
        .insert("background".into(), tok("surface.layer-one"));
    card.props
        .tokens
        .insert("radius".into(), tok("shape.corner-md"));
    card.props
        .tokens
        .insert("shadow".into(), tok("shadow.raised"));
    card.with_children(vec![content])
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(CARD_W),
                max: Some(CARD_W),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(CARD_H),
                max: Some(CARD_H),
                priority: 0,
            },
        })
}

/// Both treatments, same content, side by side in one wide frame — whether
/// a screen full of bevelled boxes gets busy is not answerable from a
/// single card in isolation. Deliberately **not** [`PANEL_WIDTH`]: this is
/// the one sheet in this file that is wide on purpose.
fn ab_adjacent_sheet(mode: ThemeMode, scale: f32) -> (ViewNode, Theme) {
    let (theme, shadow, highlight) = ab_theme(mode);
    let content_a = card_content(scale, shadow.clone(), highlight.clone());
    let card_a = box_node(
        "card-a",
        CARD_W,
        CARD_H,
        BoxSkin {
            radius: tok("shape.corner-md"),
            background: tok("surface.layer-one"),
            shadow: shadow.clone(),
            highlight: highlight.clone(),
        },
        Some(content_a),
    );
    let content_b = card_content(scale, shadow, highlight);
    let card_b = floating_card(content_b);
    let pair = row("ab-pair", sp("spacing.xl"), vec![card_a, card_b]);
    let title = match mode {
        ThemeMode::Light => "A vs. B, side by side (light)",
        ThemeMode::Dark => "A vs. B, side by side (dark)",
    };
    let mut page = filled_body(
        "ab-adjacent",
        sp("spacing.xl"),
        vec![label("ab-title", title.to_owned()), pair],
    )
    .with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(600.0),
            max: Some(600.0),
            priority: 0,
        },
        ..Constraints::default()
    });
    page.props.padding = Some(InsetRefs::all(tok("spacing.xl")));
    page.props
        .tokens
        .insert("background".into(), tok("surface.base"));
    (page, theme)
}

// ===================================================================
// Ground sweep, added 2026-09-07 in response to a correction: the earlier
// A/B read fixated on `shadow.raised`'s (correctly measured, wrongly
// weighted) near-zero delta and missed that the fill-vs-page *value step*
// is what actually carries container B — confirmed by looking at
// `ab-adjacent-{light,dark}` rather than at one isolated channel. The
// operator has picked B; a four-sided border is off the table and gets no
// more pictures.
//
// What is still open is quantitative: the whole materials/box investigation
// found that a symmetric blend-from-surface bevel only ever shows one of
// its two strokes, because Carbon's surfaces sit close to their own
// extreme. This sweep asks how far a surface has to move off that extreme
// before both strokes of a plain `bevel-1` show at once, at an amplitude
// that is not garish.
// ===================================================================

/// The amplitude every sweep step blends at — the same "moderate, not
/// Win95" value [`box_sheet`]'s baseline and [`AB_AMPLITUDE`] both use, so
/// a step that passes here is directly comparable to everything already
/// photographed.
const SWEEP_AMPLITUDE: f32 = 0.30;

/// An arbitrary point on the sweep's own luminance axis: a flat sRGB grey
/// at `pct` of full white. Not a shipped surface and not meant to become
/// one — the sweep is characterising headroom in the abstract, and
/// [`realistic_panel_at_ground`] is where a winning point turns into an
/// actual page/panel pair.
fn ground_color(pct: f32) -> ColorValue {
    let v = (pct.clamp(0.0, 1.0) * 255.0).round() as u8;
    ColorValue::from_srgb8(v, v, v, 255)
}

/// One `bevel-1` swatch at a single flat ground (page and panel are the
/// same value here, deliberately — "move both together" is the whole
/// point of a headroom sweep, see the module doc above). `ground`,
/// `shadow` and `highlight` are already-declared spike token names.
fn sweep_specimen(
    key: &str,
    caption: String,
    ground: TokenName,
    shadow: TokenName,
    highlight: TokenName,
    scale: f32,
) -> ViewNode {
    let mut node = filled_body(
        key,
        sp("spacing.2xs"),
        vec![
            label(&format!("{key}-caption"), caption),
            rule(
                &format!("{key}-rule"),
                vec![
                    bar("b0", 1.0, scale, shadow),
                    bar("b1", 1.0, scale, highlight),
                ],
            ),
        ],
    );
    node.props.tokens.insert("background".into(), ground);
    node.props.padding = Some(InsetRefs::all(tok("spacing.sm")));
    node
}

/// A sheet of [`sweep_specimen`]s, one per `(label, pct)` in `steps`.
///
/// `label` is also the token-name segment each step's three spike colours
/// are filed under, so it has to already be a legal [`TokenName`] segment
/// — the "NNpct" shape every call site uses passes: a leading digit run is
/// allowed, so "07pct"/"95pct" are fine, but a bare "7" or "95" (no
/// trailing letters) is not, the same rule that bit the material sheet's
/// first `band4`/`band6` naming attempt.
fn headroom_sweep_sheet(mode: ThemeMode, steps: &[(&str, f32)], scale: f32) -> (ViewNode, Theme) {
    let mut extra: Vec<(String, ColorValue)> = Vec::new();
    let mut specimens = Vec::new();
    for &(step_label, pct) in steps {
        let ground = ground_color(pct);
        let shadow = shade(ground, -SWEEP_AMPLITUDE);
        let highlight = shade(ground, SWEEP_AMPLITUDE);
        let ground_name = format!("spike.sweep-{step_label}-ground");
        let shadow_name = format!("spike.sweep-{step_label}-shadow");
        let highlight_name = format!("spike.sweep-{step_label}-highlight");
        extra.push((ground_name.clone(), ground));
        extra.push((shadow_name.clone(), shadow));
        extra.push((highlight_name.clone(), highlight));
        let v = (pct.clamp(0.0, 1.0) * 255.0).round() as u8;
        let caption = format!(
            "{step_label} of white \u{2014} ground #{v:02x}{v:02x}{v:02x}, bevel-1 at amplitude \u{b1}{SWEEP_AMPLITUDE:.2}"
        );
        specimens.push(sweep_specimen(
            &format!("sweep-{step_label}"),
            caption,
            TokenName::new(ground_name).expect("spike name"),
            TokenName::new(shadow_name).expect("spike name"),
            TokenName::new(highlight_name).expect("spike name"),
            scale,
        ));
    }
    let theme = extend_theme(mode, extra);
    let title = match mode {
        ThemeMode::Light => "Ground sweep \u{2014} light (bevel-1 only)",
        ThemeMode::Dark => "Ground sweep \u{2014} dark (bevel-1 only)",
    };
    let root = sheet_with_gap(
        "sweep",
        title,
        specimens,
        tok("surface.base"),
        sp("spacing.sm"),
    );
    (root, theme)
}

/// The inverse of `from_srgb8`'s transfer function, one channel at a time
/// (the standard sRGB piecewise curve) — needed only so
/// [`shipped_panel_offset_srgb8`] can read the *byte* gap between two
/// shipped colours; nothing paints through this.
fn linear_to_srgb8_channel(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "s is in [0, 1] by construction, so s * 255 rounds into u8"
    )]
    {
        (s * 255.0).round() as u8
    }
}

/// The page-vs-panel offset container B already ships with, in sRGB8
/// bytes: `surface.layer-one` against `surface.base`, which is -13 in
/// light (`#f2f2f2` vs. `#ffffff`) and +16 in dark (`#222222` vs.
/// `#121212`) — read from the running theme, not hand-copied from
/// `token/shipped.rs`'s doc comments, so it cannot drift from what the
/// theme actually ships. [`realistic_panel_at_ground`] carries this same
/// byte offset to the winning sweep ground, because "move the page and
/// panel together" (the sweep's framing) means keep the relationship, not
/// collapse it to one flat colour — a page with no panel step at all is
/// not "what the app looks like", it is a different, unasked question.
fn shipped_panel_offset_srgb8(mode: ThemeMode) -> i16 {
    let base = base_theme(mode);
    let page = theme_color(&base, "surface.base");
    let panel = theme_color(&base, "surface.layer-one");
    i16::from(linear_to_srgb8_channel(panel.r)) - i16::from(linear_to_srgb8_channel(page.r))
}

/// One realistic panel — [`card_content`] under [`floating_card`]'s own
/// shell (no border, `shape.corner-md`, a real `shadow.raised`) — at a
/// page/panel pair built from `page_pct` (the sweep ground that won) and
/// [`shipped_panel_offset`] rather than from `surface.base`/`-layer-one`.
/// This is the actual deliverable the sweep was for: a delta table says
/// nothing about what the rest of the app would look like if every
/// surface moved this far off today's extreme, and that cost is what the
/// operator has to weigh it against.
fn realistic_panel_at_ground(mode: ThemeMode, scale: f32, page_pct: f32) -> (ViewNode, Theme) {
    let page_ground = ground_color(page_pct);
    let page_v = i16::from(linear_to_srgb8_channel(page_ground.r));
    let panel_v = (page_v + shipped_panel_offset_srgb8(mode)).clamp(0, 255);
    // `panel_v` is clamped into [0, 255] on the line above, so the cast is
    // total. Clippy proves this itself now; an `#[expect]` here would be
    // unfulfilled.
    let panel_v = panel_v as u8;
    let panel_ground = ColorValue::from_srgb8(panel_v, panel_v, panel_v, 255);
    let rule_shadow = shade(panel_ground, -AB_AMPLITUDE);
    let rule_highlight = shade(panel_ground, AB_AMPLITUDE);
    let theme = extend_theme(
        mode,
        vec![
            ("spike.realistic-page".into(), page_ground),
            ("spike.realistic-panel".into(), panel_ground),
            ("spike.realistic-rule-shadow".into(), rule_shadow),
            ("spike.realistic-rule-highlight".into(), rule_highlight),
        ],
    );
    let t = |name: &str| TokenName::new(name.to_owned()).expect("spike name");
    let content = card_content(
        scale,
        t("spike.realistic-rule-shadow"),
        t("spike.realistic-rule-highlight"),
    );
    let mut card = ViewNode::new(NodeKind::Grid, "card-realistic").with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    });
    card.props
        .tokens
        .insert("background".into(), t("spike.realistic-panel"));
    card.props
        .tokens
        .insert("radius".into(), tok("shape.corner-md"));
    card.props
        .tokens
        .insert("shadow".into(), tok("shadow.raised"));
    let card = card
        .with_children(vec![content])
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(CARD_W),
                max: Some(CARD_W),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(CARD_H),
                max: Some(CARD_H),
                priority: 0,
            },
        });
    let title = match mode {
        ThemeMode::Light => "Realistic panel at threshold ground (light)",
        ThemeMode::Dark => "Realistic panel at threshold ground (dark)",
    };
    let root = sheet("realistic", title, vec![card], t("spike.realistic-page"));
    (root, theme)
}

// ===================================================================
// Paper-tone candidates, added 2026-09-07: the operator saw both
// realistic panels and called the light one too grey, and asked for
// warmer, more natural paper tones instead. New context that frees this
// sheet from the sweep's constraint: rule shadow/highlight are now
// **locked** — see [`LOCKED_RULE_SHADOW`] — so a candidate's ground no
// longer has to clear any headroom floor. Ground lightness here is a pure
// aesthetic choice, covering a wide range rather than staying near the
// sweep's 80% crossover.
//
// One sheet, one image, eight specimens stacked vertically: the same
// [`card_content`] on eight candidate grounds, panel offset from each by
// the same real `surface.layer-one` byte delta [`realistic_panel_at_ground`]
// used, so the ground is the only thing that changes specimen to specimen.
// ===================================================================

/// The rule shadow/highlight this sheet locks across every candidate: the
/// exact byte pair [`headroom_sweep_sheet`] measured as "both strokes
/// clear a ~15-level floor" at the 80% grey crossover, reused unmodified
/// rather than re-derived per candidate — which is the point of "locked,
/// independent tokens": a rule that no longer depends on blend headroom
/// from whatever ground it happens to sit on, so the ground is free to
/// move for reasons that have nothing to do with the rule at all.
const LOCKED_RULE_SHADOW: (u8, u8, u8) = (174, 174, 174);
/// See [`LOCKED_RULE_SHADOW`].
const LOCKED_RULE_HIGHLIGHT: (u8, u8, u8) = (221, 221, 221);

/// One candidate ground: `hex` and `note` are the specimen's own caption,
/// `rgb` is what actually gets painted.
struct PaperCandidate {
    /// Letters only, no digits: used to build both the node keys and the
    /// spike token names, and `TokenName`'s grammar refuses a segment
    /// with an embedded (non-leading) digit — `spike.paper-cream95-page`
    /// would panic the same way the material sheet's first `band4`/`band6`
    /// naming attempt did.
    slug: &'static str,
    hex: &'static str,
    note: &'static str,
    rgb: (u8, u8, u8),
}

/// The eight candidates the brief suggested, unmodified — each checked
/// (see the report) for R and G moving *together* against B, the safe
/// blue-yellow warmth axis for red-green colour blindness, rather than
/// either channel shifting against the other.
const PAPER_CANDIDATES: [PaperCandidate; 8] = [
    PaperCandidate {
        slug: "white",
        hex: "#ffffff",
        note: "today's white \u{2014} control",
        rgb: (255, 255, 255),
    },
    PaperCandidate {
        slug: "greycontrol",
        hex: "#cccccc",
        note: "neutral grey \u{2014} the rejected control",
        rgb: (204, 204, 204),
    },
    PaperCandidate {
        slug: "creamnf",
        hex: "#f5f1e8",
        note: "warm cream, ~95%",
        rgb: (245, 241, 232),
    },
    PaperCandidate {
        slug: "creamnt",
        hex: "#efe9dd",
        note: "warm cream, ~90%",
        rgb: (239, 233, 221),
    },
    PaperCandidate {
        slug: "bone",
        hex: "#e8e2d4",
        note: "bone / manila, ~87%",
        rgb: (232, 226, 212),
    },
    PaperCandidate {
        slug: "greige",
        hex: "#e8e4dc",
        note: "greige, restrained, ~90%",
        rgb: (232, 228, 220),
    },
    PaperCandidate {
        slug: "newsprint",
        hex: "#eae9e4",
        note: "cool paper / newsprint, ~91%",
        rgb: (234, 233, 228),
    },
    PaperCandidate {
        slug: "warmtip",
        hex: "#f0e6d2",
        note: "warmer, more saturated, ~94%",
        rgb: (240, 230, 210),
    },
];

/// `channel + `[`shipped_panel_offset_srgb8`]`(Light)`, clamped — applying
/// the *same real byte offset* `surface.layer-one` carries from
/// `surface.base` to a chromatic channel rather than only to a neutral
/// grey, so a warm candidate's panel stays the same hue as its page,
/// merely a shade darker, instead of drifting grey.
fn offset_channel(c: u8, offset: i16) -> u8 {
    let v = i16::from(c) + offset;
    // The clamp makes the cast total, and clippy proves it, so an
    // `#[expect]` here would be unfulfilled.
    v.clamp(0, 255) as u8
}

/// `candidate`'s panel fill: its own ground offset by the real
/// `surface.layer-one`-minus-`surface.base` byte delta, applied per
/// channel. Factored out of [`paper_specimen`] (which now calls this) so
/// [`paper_tone_corrected_sheet`] can compute a candidate's panel colour
/// *before* building its specimen, since the corrected rule's colours are
/// derived from that panel, not from a shared locked pair.
fn panel_rgb_for(candidate: &PaperCandidate) -> (u8, u8, u8) {
    let offset = shipped_panel_offset_srgb8(ThemeMode::Light);
    let (r, g, b) = candidate.rgb;
    (
        offset_channel(r, offset),
        offset_channel(g, offset),
        offset_channel(b, offset),
    )
}

/// The coordinator's fix, 2026-09-07: a rule's shadow and highlight as
/// **signed sRGB level offsets from the panel fill it sits on**, clamped
/// per channel to `[0, 255]`, instead of the fixed absolute pair
/// [`LOCKED_RULE_SHADOW`]/[`LOCKED_RULE_HIGHLIGHT`] every earlier sheet
/// used.
///
/// The bug the fixed pair had: whether the pair reads as a groove depends
/// entirely on the surface under it. On the sweep's 191 panel, 174 sits
/// below it and 221 sits above it, so it reads as a real groove. On the
/// white candidate's 242 panel, 221 *also* sits below it — the
/// "highlight" is a second, fainter shadow, and no groove is reachable at
/// all. A signed offset cannot make that mistake: the shadow is always
/// `panel - 28` and the highlight is always `panel + 22`, so the shadow
/// is always below the surface and the highlight always above it, up to
/// whatever headroom the panel has left to clamp into. `-28`/`+22` are
/// the coordinator's own deltas. Applied per channel, not to one scalar
/// level, so a chromatic candidate's rule stays the same hue as its
/// panel, only a shade darker or lighter — the same reasoning
/// [`offset_channel`] already carries for the page-to-panel step.
fn rule_stroke_colors(panel: (u8, u8, u8)) -> ((u8, u8, u8), (u8, u8, u8)) {
    let (r, g, b) = panel;
    let shadow = (
        offset_channel(r, -28),
        offset_channel(g, -28),
        offset_channel(b, -28),
    );
    let highlight = (
        offset_channel(r, 22),
        offset_channel(g, 22),
        offset_channel(b, 22),
    );
    (shadow, highlight)
}

/// Why this sheet has its own compact content builder instead of reusing
/// [`card_content`]: eight full header-plus-three-row cards do not fit a
/// 900-unit capture at `card_content`'s own (shared, unmodified)
/// `spacing.sm`/`spacing.md` rhythm — `paper_tone_sheet_fits_the_capture_viewport`
/// caught that by printing every specimen's placed rect, not by guessing at
/// pixel budgets by hand: specimens 6-8 were landing at zero height,
/// silently collapsed rather than clipped. Tightening `card_content` itself
/// would have re-flowed `realistic-panel-*` and the container A/B shots too,
/// which are already verified and out of scope this round.
///
/// The card's own compact content: a title that doubles as this
/// specimen's label (hex + note — no separate caption line, which is the
/// other half of the space this sheet needed), two rules between three
/// rows. Same three sentences [`card_content`] uses, same structure
/// (header, three rows, rules between them), tighter rhythm only.
fn paper_card_content(
    candidate: &PaperCandidate,
    shadow: TokenName,
    highlight: TokenName,
    scale: f32,
    gap: Option<TokenName>,
) -> ViewNode {
    let seam = |key: &str| {
        rule(
            key,
            vec![
                bar("b0", 1.0, scale, shadow.clone()),
                bar("b1", 1.0, scale, highlight.clone()),
            ],
        )
    };
    let mut node = filled_body(
        "card-inner",
        gap.clone(),
        vec![
            label(
                "card-title",
                format!("{} \u{2014} {}", candidate.hex, candidate.note),
            ),
            body_line("card-row1", "First row of realistic content in the panel."),
            seam("card-row1-rule"),
            body_line("card-row2", "Second row of realistic content in the panel."),
            seam("card-row2-rule"),
            body_line("card-row3", "Third row of realistic content in the panel."),
        ],
    );
    node.props.padding = Some(InsetRefs::all(
        gap.clone().unwrap_or_else(|| tok("spacing.2xs")),
    ));
    node
}

fn paper_specimen(
    candidate: &PaperCandidate,
    shadow: TokenName,
    highlight: TokenName,
    scale: f32,
    gap: Option<TokenName>,
    extra: &mut Vec<(String, ColorValue)>,
) -> ViewNode {
    let key = candidate.slug;
    let (pr, pg, pb) = candidate.rgb;
    let panel_rgb = panel_rgb_for(candidate);
    let page_name = format!("spike.paper-{key}-page");
    let panel_name = format!("spike.paper-{key}-panel");
    extra.push((page_name.clone(), ColorValue::from_srgb8(pr, pg, pb, 255)));
    extra.push((
        panel_name.clone(),
        ColorValue::from_srgb8(panel_rgb.0, panel_rgb.1, panel_rgb.2, 255),
    ));
    let t_page = TokenName::new(page_name).expect("spike name");
    let t_panel = TokenName::new(panel_name).expect("spike name");

    let content = paper_card_content(candidate, shadow, highlight, scale, gap.clone());
    let mut card = ViewNode::new(NodeKind::Grid, format!("{key}-card")).with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    });
    card.props.tokens.insert("background".into(), t_panel);
    card.props
        .tokens
        .insert("radius".into(), tok("shape.corner-md"));
    card.props
        .tokens
        .insert("shadow".into(), tok("shadow.raised"));
    let card = card.with_children(vec![content]);

    let mut swatch = filled_body(key, None, vec![card]);
    swatch.props.tokens.insert("background".into(), t_page);
    swatch.props.padding = Some(InsetRefs::all(gap.unwrap_or_else(|| tok("spacing.2xs"))));
    swatch
}

/// Panel width for this sheet only, wider than [`PANEL_WIDTH`]: the row
/// text is unchanged (same content in every specimen is the whole point),
/// and a few more logical units keeps it to one line instead of two.
const PAPER_PANEL_WIDTH: f32 = 460.0;

/// The whole sheet: one tall column, eight [`paper_specimen`]s over one
/// locked shadow/highlight pair.
fn paper_tone_sheet(scale: f32) -> (ViewNode, Theme) {
    let mut extra: Vec<(String, ColorValue)> = vec![
        (
            "spike.paper-rule-shadow".into(),
            ColorValue::from_srgb8(
                LOCKED_RULE_SHADOW.0,
                LOCKED_RULE_SHADOW.1,
                LOCKED_RULE_SHADOW.2,
                255,
            ),
        ),
        (
            "spike.paper-rule-highlight".into(),
            ColorValue::from_srgb8(
                LOCKED_RULE_HIGHLIGHT.0,
                LOCKED_RULE_HIGHLIGHT.1,
                LOCKED_RULE_HIGHLIGHT.2,
                255,
            ),
        ),
    ];
    let shadow_tok = TokenName::new("spike.paper-rule-shadow").expect("spike name");
    let highlight_tok = TokenName::new("spike.paper-rule-highlight").expect("spike name");

    let specimens: Vec<ViewNode> = PAPER_CANDIDATES
        .iter()
        .map(|candidate| {
            paper_specimen(
                candidate,
                shadow_tok.clone(),
                highlight_tok.clone(),
                scale,
                None,
                &mut extra,
            )
        })
        .collect();

    let theme = extend_theme(ThemeMode::Light, extra);
    let mut children = vec![label(
        "paper-title",
        "Paper-tone candidates (light) \u{2014} locked rule, ground varies".to_owned(),
    )];
    children.extend(specimens);
    let mut page =
        filled_body("paper", sp("spacing.2xs"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(PAPER_PANEL_WIDTH),
                max: Some(PAPER_PANEL_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    page.props
        .tokens
        .insert("background".into(), tok("surface.base"));
    (page, theme)
}

// ===================================================================
// Corrected rule tokens, added 2026-09-07: the coordinator found that
// LOCKED_RULE_SHADOW/HIGHLIGHT were fixed absolute values, so whether the
// pair read as a groove depended on the panel under it — it worked on the
// 191 grey-control panel and failed on the 242 white panel, where the
// "highlight" (221) also sat below the surface. The fix is
// [`rule_stroke_colors`]: signed offsets from each candidate's own panel
// fill. Two sheets exercise it.
// ===================================================================

/// Sheet 1 of the fix: the same eight grounds and the same panel content
/// as [`paper_tone_sheet`] — [`paper_specimen`] itself is untouched — but
/// each specimen's rule is computed by [`rule_stroke_colors`] from *that
/// specimen's own panel*, rather than sharing one locked absolute pair.
/// Flip this image against `paper-tone-candidates-light-2x` to see the
/// same eight grounds with only the rule's colour source changed.
fn paper_tone_corrected_sheet(scale: f32) -> (ViewNode, Theme) {
    let mut extra: Vec<(String, ColorValue)> = Vec::new();

    let specimens: Vec<ViewNode> = PAPER_CANDIDATES
        .iter()
        .map(|candidate| {
            let panel = panel_rgb_for(candidate);
            let (shadow_rgb, highlight_rgb) = rule_stroke_colors(panel);
            let shadow_name = format!("spike.paper-corrected-{}-shadow", candidate.slug);
            let highlight_name = format!("spike.paper-corrected-{}-highlight", candidate.slug);
            extra.push((
                shadow_name.clone(),
                ColorValue::from_srgb8(shadow_rgb.0, shadow_rgb.1, shadow_rgb.2, 255),
            ));
            extra.push((
                highlight_name.clone(),
                ColorValue::from_srgb8(highlight_rgb.0, highlight_rgb.1, highlight_rgb.2, 255),
            ));
            let shadow_tok = TokenName::new(shadow_name).expect("spike name");
            let highlight_tok = TokenName::new(highlight_name).expect("spike name");
            paper_specimen(
                candidate,
                shadow_tok,
                highlight_tok,
                scale,
                None,
                &mut extra,
            )
        })
        .collect();

    let theme = extend_theme(ThemeMode::Light, extra);
    let mut children = vec![label(
        "paper-corrected-title",
        "Paper-tone candidates (light) \u{2014} corrected: rule = signed offset from panel"
            .to_owned(),
    )];
    children.extend(specimens);
    let mut page =
        filled_body("paper-corrected", sp("spacing.2xs"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(PAPER_PANEL_WIDTH),
                max: Some(PAPER_PANEL_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    page.props
        .tokens
        .insert("background".into(), tok("surface.base"));
    (page, theme)
}

// ===================================================================
// Sheet 2: four rule treatments on the white ground only — the case the
// operator has said matters most, since white is where the fixed pair
// failed and white is what he prefers as a control.
// ===================================================================

/// Sheet 2's card content: the same three-row structure
/// [`paper_card_content`] uses, but the title is a free string (a rule
/// treatment's name, not a ground candidate) and the seam between rows is
/// supplied by `make_seam` instead of hardcoded to a two-bar groove — so
/// this sheet can put a flat single bar, a shadow-only bar, or the
/// corrected two-bar groove in that slot without editing
/// `paper_card_content` (already verified by `paper_tone_sheet`) or
/// `card_content` (shared with `realistic-panel-*` and the container A/B
/// shots, out of scope this round).
fn rule_treatment_card_content(
    title: &str,
    scale: f32,
    make_seam: impl Fn(&str, f32) -> ViewNode,
) -> ViewNode {
    let mut node = filled_body(
        "card-inner",
        None,
        vec![
            label("card-title", title.to_owned()),
            body_line("card-row1", "First row of realistic content in the panel."),
            make_seam("card-row1-rule", scale),
            body_line("card-row2", "Second row of realistic content in the panel."),
            make_seam("card-row2-rule", scale),
            body_line("card-row3", "Third row of realistic content in the panel."),
        ],
    );
    node.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    node
}

/// One rule-treatment specimen: `title` and `make_seam`'s rule inside the
/// same rounded, shadowed, borderless shell every other panel-tone
/// specimen uses, on the **real shipped** `surface.base`/`surface.layer-one`
/// pair — not a spike literal — because this sheet's whole point is
/// today's actual white control, not a stand-in for it.
fn rule_treatment_specimen(
    key: &str,
    title: &str,
    scale: f32,
    make_seam: impl Fn(&str, f32) -> ViewNode,
) -> ViewNode {
    let content = rule_treatment_card_content(title, scale, make_seam);
    let mut card = ViewNode::new(NodeKind::Grid, format!("{key}-card")).with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    });
    card.props
        .tokens
        .insert("background".into(), tok("surface.layer-one"));
    card.props
        .tokens
        .insert("radius".into(), tok("shape.corner-md"));
    card.props
        .tokens
        .insert("shadow".into(), tok("shadow.raised"));
    let card = card.with_children(vec![content]);

    let mut swatch = filled_body(key, None, vec![card]);
    swatch
        .props
        .tokens
        .insert("background".into(), tok("surface.base"));
    swatch.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    swatch
}

/// The whole sheet: four [`rule_treatment_specimen`]s, all on
/// `surface.base`/`surface.layer-one`, differing only in how the rule
/// between rows is built.
fn rule_on_white_sheet(scale: f32) -> (ViewNode, Theme) {
    let panel = panel_rgb_for(&PAPER_CANDIDATES[0]);
    debug_assert_eq!(PAPER_CANDIDATES[0].slug, "white");
    let (shadow_rgb, highlight_rgb) = rule_stroke_colors(panel);
    let extra: Vec<(String, ColorValue)> = vec![
        (
            "spike.rulecmp-shadow".into(),
            ColorValue::from_srgb8(shadow_rgb.0, shadow_rgb.1, shadow_rgb.2, 255),
        ),
        (
            "spike.rulecmp-highlight".into(),
            ColorValue::from_srgb8(highlight_rgb.0, highlight_rgb.1, highlight_rgb.2, 255),
        ),
    ];
    let shadow_tok = TokenName::new("spike.rulecmp-shadow").expect("spike name");
    let highlight_tok = TokenName::new("spike.rulecmp-highlight").expect("spike name");

    let (s1, h1) = (shadow_tok.clone(), highlight_tok.clone());
    let s2 = shadow_tok.clone();
    let s3 = shadow_tok;

    let specimens = vec![
        rule_treatment_specimen(
            "control",
            "1. flat border.subtle (control)",
            scale,
            move |key, scale| rule(key, vec![bar("b0", 1.0, scale, tok("border.subtle"))]),
        ),
        rule_treatment_specimen(
            "groove",
            "2. signed-offset groove (-28 / +22 clamped)",
            scale,
            move |key, scale| {
                rule(
                    key,
                    vec![
                        bar("b0", 1.0, scale, s1.clone()),
                        bar("b1", 1.0, scale, h1.clone()),
                    ],
                )
            },
        ),
        rule_treatment_specimen(
            "shadow1px",
            "3. shadow-only, 1px stroke at -28",
            scale,
            move |key, scale| rule(key, vec![bar("b0", 1.0, scale, s2.clone())]),
        ),
        rule_treatment_specimen(
            "shadow2px",
            "4. shadow-only, 2px stroke at -28",
            scale,
            move |key, scale| rule(key, vec![bar("b0", 2.0, scale, s3.clone())]),
        ),
    ];

    let theme = extend_theme(ThemeMode::Light, extra);
    let mut children = vec![label(
        "rulecmp-title",
        "Rule treatments on white ground (light) \u{2014} same panel, four strokes".to_owned(),
    )];
    children.extend(specimens);
    let mut page =
        filled_body("rulecmp", sp("spacing.2xs"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(PAPER_PANEL_WIDTH),
                max: Some(PAPER_PANEL_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    page.props
        .tokens
        .insert("background".into(), tok("surface.base"));
    (page, theme)
}

// ===================================================================
// Depth-matched rule matrix, added 2026-09-07 (round 3 of the rule-token
// fix): the coordinator's own review of `rule-on-white-light-2x` found a
// confound in that sheet's own conclusion — it varied construction (flat
// vs groove vs doubled) *and* depth (control at -47, all three new
// options at -28) at the same time, so "flat beat groove" may only have
// meant "deeper beat shallower". This matrix holds depth fixed within
// each A/B/C/D/E label and only lets construction vary, run once on
// white (no headroom) and once on a reference ground with full headroom
// on both sides, so a depth-matched comparison is finally possible. Also
// switches to a true 1x capture (`Camera::spike`'s `scale` sets
// `pixels_per_point` directly — see that constructor's own doc) instead
// of 2x, because the operator's viewer downscales the sheet and a 2x
// capture's 1-device-pixel rule is already a half-logical-pixel line
// before that downscale even happens.
// ===================================================================

/// Width for this matrix only: narrower than [`PAPER_PANEL_WIDTH`] per
/// the coordinator's "roughly 760 units" target, now that the sheet is
/// captured at 1x and viewed at native resolution rather than being
/// downscaled from a 2x capture.
const DEPTH_MATCHED_WIDTH: f32 = 760.0;

/// One depth-matched specimen: [`rule_treatment_card_content`]'s same
/// three-row structure, under a shell whose page/panel ground is passed
/// in rather than hardcoded — [`rule_treatment_specimen`] (last round's
/// white-only sheet) always used the real `surface.base`/`surface.layer-one`
/// pair, which this matrix's second sheet cannot do, since 219/206 is not
/// a real shipped token pair.
fn depth_matched_specimen(
    key: &str,
    title: &str,
    scale: f32,
    page_tok: TokenName,
    panel_tok: TokenName,
    make_seam: impl Fn(&str, f32) -> ViewNode,
) -> ViewNode {
    let content = rule_treatment_card_content(title, scale, make_seam);
    let mut card = ViewNode::new(NodeKind::Grid, format!("{key}-card")).with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    });
    card.props.tokens.insert("background".into(), panel_tok);
    card.props
        .tokens
        .insert("radius".into(), tok("shape.corner-md"));
    card.props
        .tokens
        .insert("shadow".into(), tok("shadow.raised"));
    let card = card.with_children(vec![content]);

    let mut swatch = filled_body(key, None, vec![card]);
    swatch.props.tokens.insert("background".into(), page_tok);
    swatch.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    swatch
}

/// The five-specimen depth-matched matrix, on one ground.
///
/// `shadow47` is the **shared** shadow for A, C and E — on the white
/// sheet this is the real `border.subtle` token itself (today's actual
/// control, byte-identical across all three uses of it, not a
/// recomputed stand-in), because `-47` and `border.subtle`'s own value
/// happen to coincide exactly on the 242 panel. On the reference sheet
/// there is no real token that means "-47 on a 206 panel", so the caller
/// passes a spike literal computed the same way B/C/D's own offsets are.
/// `panel_val` is that panel's own achieved sRGB level, needed to compute
/// B, C and D's `-28`/`+22` offsets.
/// A sheet's ground: the page it sits on, the panel drawn over it, that
/// panel's own achieved sRGB level, and any spike colours the theme needs.
/// Bundled because these four always travel together and because the sheet
/// builders were carrying eight and nine positional arguments without it.
struct SheetGround {
    page: TokenName,
    panel: TokenName,
    /// The panel's achieved sRGB level, from which every stroke offset is
    /// computed. Sampled from the render, never assumed.
    panel_val: u8,
    extra: Vec<(String, ColorValue)>,
}

fn rule_depth_matched_sheet(
    scale: f32,
    sheet_key: &str,
    ground_title: &str,
    shadow47: TokenName,
    ground: SheetGround,
) -> (ViewNode, Theme) {
    let SheetGround {
        page: page_tok,
        panel: panel_tok,
        panel_val,
        mut extra,
    } = ground;
    let shadow28_val = offset_channel(panel_val, -28);
    let highlight22_val = offset_channel(panel_val, 22);
    // Letters only, no digits: `TokenName`'s grammar refuses a segment
    // with a non-leading digit, which a name like `shadow28` would be —
    // the same rule the material and paper-tone sheets' naming already
    // ran into.
    let shadow28_name = format!("spike.depthmatch-{sheet_key}-shadowmid");
    let highlight_name = format!("spike.depthmatch-{sheet_key}-highlightmid");
    extra.push((
        shadow28_name.clone(),
        ColorValue::from_srgb8(shadow28_val, shadow28_val, shadow28_val, 255),
    ));
    extra.push((
        highlight_name.clone(),
        ColorValue::from_srgb8(highlight22_val, highlight22_val, highlight22_val, 255),
    ));
    let shadow28_tok = TokenName::new(shadow28_name).expect("spike name");
    let highlight_tok = TokenName::new(highlight_name).expect("spike name");

    let shadow47_a = shadow47.clone();
    let shadow47_c = shadow47.clone();
    let shadow47_e = shadow47;
    let shadow28_b = shadow28_tok.clone();
    let shadow28_d = shadow28_tok;
    let highlight_c = highlight_tok.clone();
    let highlight_d = highlight_tok;

    let specimens = vec![
        depth_matched_specimen(
            "a",
            "A. shadow -47 only, 1px (today's border.subtle depth)",
            scale,
            page_tok.clone(),
            panel_tok.clone(),
            move |key, scale| rule(key, vec![bar("b0", 1.0, scale, shadow47_a.clone())]),
        ),
        depth_matched_specimen(
            "b",
            "B. shadow -28 only, 1px",
            scale,
            page_tok.clone(),
            panel_tok.clone(),
            move |key, scale| rule(key, vec![bar("b0", 1.0, scale, shadow28_b.clone())]),
        ),
        depth_matched_specimen(
            "c",
            "C. shadow -47 + highlight +22 (clamped), 1px each",
            scale,
            page_tok.clone(),
            panel_tok.clone(),
            move |key, scale| {
                rule(
                    key,
                    vec![
                        bar("b0", 1.0, scale, shadow47_c.clone()),
                        bar("b1", 1.0, scale, highlight_c.clone()),
                    ],
                )
            },
        ),
        depth_matched_specimen(
            "d",
            "D. shadow -28 + highlight +22 (clamped), 1px each",
            scale,
            page_tok.clone(),
            panel_tok.clone(),
            move |key, scale| {
                rule(
                    key,
                    vec![
                        bar("b0", 1.0, scale, shadow28_d.clone()),
                        bar("b1", 1.0, scale, highlight_d.clone()),
                    ],
                )
            },
        ),
        depth_matched_specimen(
            "e",
            "E. shadow -47 doubled to 2px, no highlight",
            scale,
            page_tok.clone(),
            panel_tok.clone(),
            move |key, scale| rule(key, vec![bar("b0", 2.0, scale, shadow47_e.clone())]),
        ),
    ];

    let theme = extend_theme(ThemeMode::Light, extra);
    let mut children = vec![label("title", ground_title.to_owned())];
    children.extend(specimens);
    let mut page =
        filled_body(sheet_key, sp("spacing.2xs"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(DEPTH_MATCHED_WIDTH),
                max: Some(DEPTH_MATCHED_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    page.props.tokens.insert("background".into(), page_tok);
    (page, theme)
}

/// Sheet 1 of the depth-matched round: the real shipped
/// `surface.base`/`surface.layer-one` pair (255/242), `shadow47` is the
/// real `border.subtle` token — the true control, not a recomputed
/// stand-in.
fn rule_depth_matched_white_sheet(scale: f32) -> (ViewNode, Theme) {
    rule_depth_matched_sheet(
        scale,
        "depthwhite",
        "Depth-matched rule matrix \u{2014} white ground (surface.base 255 / surface.layer-one 242, no headroom above the panel)",
        tok("border.subtle"),
        SheetGround {
            page: tok("surface.base"),
            panel: tok("surface.layer-one"),
            panel_val: 242,
            extra: Vec::new(),
        },
    )
}

/// Sheet 2 of the depth-matched round: the coordinator's reference
/// ground, 219 page / 206 panel — chosen so `-28`/`+22` both land with no
/// clamping on either side, the case white cannot produce. Not a real
/// shipped token pair, so page, panel and the shared `-47` shadow are all
/// spike literals here — `shadow47` is `panel - 47` computed the same way
/// B/C/D's own offsets are, since there is no real token that means
/// "-47 on a 206 panel" the way `border.subtle` happens to mean "-47 on
/// a 242 panel".
fn rule_depth_matched_bone_sheet(scale: f32) -> (ViewNode, Theme) {
    const PAGE_VAL: u8 = 219;
    const PANEL_VAL: u8 = 206;
    let shadow47_val = offset_channel(PANEL_VAL, -47);
    let extra: Vec<(String, ColorValue)> = vec![
        (
            "spike.depthbone-page".into(),
            ColorValue::from_srgb8(PAGE_VAL, PAGE_VAL, PAGE_VAL, 255),
        ),
        (
            "spike.depthbone-panel".into(),
            ColorValue::from_srgb8(PANEL_VAL, PANEL_VAL, PANEL_VAL, 255),
        ),
        (
            // Letters only — see the same-shaped comment in
            // `rule_depth_matched_sheet`.
            "spike.depthbone-shadowdeep".into(),
            ColorValue::from_srgb8(shadow47_val, shadow47_val, shadow47_val, 255),
        ),
    ];
    let page_tok = TokenName::new("spike.depthbone-page").expect("spike name");
    let panel_tok = TokenName::new("spike.depthbone-panel").expect("spike name");
    let shadow47_tok = TokenName::new("spike.depthbone-shadowdeep").expect("spike name");
    rule_depth_matched_sheet(
        scale,
        "depthbone",
        "Depth-matched rule matrix \u{2014} bone-reference ground (219 page / 206 panel, full headroom both sides)",
        shadow47_tok,
        SheetGround {
            page: page_tok,
            panel: panel_tok,
            panel_val: PANEL_VAL,
            extra,
        },
    )
}

// ===================================================================
// Ratio field, added 2026-09-07: the operator picked D (shadow 47,
// highlight 22, ratio 2.14) on the bone-reference ground and D from the
// white sheet's own labelling (shadow 28, highlight 13, ratio 2.15) —
// see `docs/experiments/successes/2026-09-07-deboss-rule-contrast-ratio.md`,
// the committed protocol this section implements. That file's hypothesis:
// the groove holds a fixed shadow-to-highlight ratio near 2.15 regardless
// of ground, with the clamped stroke taking whatever headroom exists and
// the free stroke sized from it. Dark is the test, because it inverts
// which stroke clamps (shadow has only 34 levels of room on the shipped
// dark panel, where light's highlight was the clamped one) and because
// sRGB gamma stretches a fixed step more near black than near white — the
// plan predicts dark wants smaller absolute deltas than light's 28/13,
// not the same numbers transplanted.
//
// Two sheets, same seven specimens (A the real `border.subtle` control,
// B-G spanning ratios on both sides of 2.15 plus one highlight-dominant
// inversion), differing only in ground — the plan's own confound guard,
// since the light-theme round that produced the hypothesis varied depth
// and construction at once. Labels carry offsets, never ratios: the
// operator judges without seeing the arithmetic.
// ===================================================================

/// Width for the ratio sheets: the plan asks for "roughly 1200 wide", but
/// the capture viewport itself (`catalog::WINDOW`) is exactly 1200
/// logical units wide, so this sheet's own outer padding would push a
/// content box fixed to the full 1200 past that edge. 1160 leaves the
/// slack that padding needs while still reading as "about 1200" per the
/// coordinator's own wording.
const RATIO_SHEET_WIDTH: f32 = 1160.0;

/// B through G's offsets, deliberately spanning both sides of the 2.15
/// ratio the plan is testing, per the plan's own field requirements: at
/// least one specimen well inside the range and at least two on each
/// side. Labels show the two offsets and never the ratio.
const RATIO_ROWS: [(&str, &str, i16, i16); 6] = [
    ("b", "B. shadow 28, highlight 13", 28, 13),
    ("c", "C. shadow 20, highlight 9", 20, 9),
    ("d", "D. shadow 13, highlight 6", 13, 6),
    ("e", "E. shadow 26, highlight 6", 26, 6),
    ("f", "F. shadow 13, highlight 11", 13, 11),
    ("g", "G. shadow 6, highlight 13", 6, 13),
];

/// The seven-specimen ratio field, on one ground. `border_subtle_tok` is
/// always the real `border.subtle` token from `theme_mode`'s shipped
/// theme — including on the mid-dark reference sheet, where the ground
/// is a spike literal but the control specimen is still the actual
/// shipped token, unmodified, shown against a lighter panel than it
/// ships on. `panel_val` is that panel's own achieved sRGB level, used to
/// compute B-G's offsets; reuses [`depth_matched_specimen`] for the
/// per-specimen shell, unchanged from the depth-matched round.
fn rule_ratio_sheet(
    scale: f32,
    sheet_key: &str,
    ground_title: &str,
    theme_mode: ThemeMode,
    border_subtle_tok: TokenName,
    ground: SheetGround,
) -> (ViewNode, Theme) {
    let SheetGround {
        page: page_tok,
        panel: panel_tok,
        panel_val,
        mut extra,
    } = ground;
    let mut specimens = Vec::with_capacity(7);
    specimens.push(depth_matched_specimen(
        "a",
        "A. shipped flat border.subtle (control)",
        scale,
        page_tok.clone(),
        panel_tok.clone(),
        move |key, scale| rule(key, vec![bar("b0", 1.0, scale, border_subtle_tok.clone())]),
    ));

    for (key, label_text, shadow_off, highlight_off) in RATIO_ROWS {
        let shadow_val = offset_channel(panel_val, -shadow_off);
        let highlight_val = offset_channel(panel_val, highlight_off);
        let shadow_name = format!("spike.ratio-{sheet_key}-{key}-shadow");
        let highlight_name = format!("spike.ratio-{sheet_key}-{key}-highlight");
        extra.push((
            shadow_name.clone(),
            ColorValue::from_srgb8(shadow_val, shadow_val, shadow_val, 255),
        ));
        extra.push((
            highlight_name.clone(),
            ColorValue::from_srgb8(highlight_val, highlight_val, highlight_val, 255),
        ));
        let shadow_tok = TokenName::new(shadow_name).expect("spike name");
        let highlight_tok = TokenName::new(highlight_name).expect("spike name");
        specimens.push(depth_matched_specimen(
            key,
            label_text,
            scale,
            page_tok.clone(),
            panel_tok.clone(),
            move |k, scale| {
                rule(
                    k,
                    vec![
                        bar("b0", 1.0, scale, shadow_tok.clone()),
                        bar("b1", 1.0, scale, highlight_tok.clone()),
                    ],
                )
            },
        ));
    }

    let theme = extend_theme(theme_mode, extra);
    let mut children = vec![label("title", ground_title.to_owned())];
    children.extend(specimens);
    let mut page =
        filled_body(sheet_key, sp("spacing.2xs"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(RATIO_SHEET_WIDTH),
                max: Some(RATIO_SHEET_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    page.props.tokens.insert("background".into(), page_tok);
    (page, theme)
}

/// Sheet 1: the shipped dark theme, real `surface.base`/`surface.layer-one`
/// (18/34) and the real `border.subtle` token for A.
fn rule_ratio_dark_sheet(scale: f32) -> (ViewNode, Theme) {
    rule_ratio_sheet(
        scale,
        "ratiodark",
        "Rule ratio field \u{2014} shipped dark ground (surface.base 18 / surface.layer-one 34)",
        ThemeMode::Dark,
        tok("border.subtle"),
        SheetGround {
            page: tok("surface.base"),
            panel: tok("surface.layer-one"),
            panel_val: 34,
            extra: Vec::new(),
        },
    )
}

/// Sheet 2: a mid-dark reference ground with headroom on both sides of
/// the panel, page `#3a3a3a` (58) / panel `#4a4a4a` (74) — spike
/// literals, not a colour proposal, the same disclosure the bone ground
/// carried in the depth-matched round. A still uses the real shipped
/// dark `border.subtle` token (unmodified), so the only variable between
/// this sheet and the shipped-dark one is the ground itself.
fn rule_ratio_dark_reference_sheet(scale: f32) -> (ViewNode, Theme) {
    const PAGE_VAL: u8 = 58;
    const PANEL_VAL: u8 = 74;
    let extra: Vec<(String, ColorValue)> = vec![
        (
            "spike.ratioref-page".into(),
            ColorValue::from_srgb8(PAGE_VAL, PAGE_VAL, PAGE_VAL, 255),
        ),
        (
            "spike.ratioref-panel".into(),
            ColorValue::from_srgb8(PANEL_VAL, PANEL_VAL, PANEL_VAL, 255),
        ),
    ];
    let page_tok = TokenName::new("spike.ratioref-page").expect("spike name");
    let panel_tok = TokenName::new("spike.ratioref-panel").expect("spike name");
    rule_ratio_sheet(
        scale,
        "ratioref",
        "Rule ratio field \u{2014} mid-dark reference ground (58 page / 74 panel, neutral grey, not a colour proposal)",
        ThemeMode::Dark,
        tok("border.subtle"),
        SheetGround {
            page: page_tok,
            panel: panel_tok,
            panel_val: PANEL_VAL,
            extra,
        },
    )
}

// ===================================================================
// Height field, added 2026-09-07: the ratio round is decided (see
// `docs/experiments/successes/2026-09-07-deboss-rule-contrast-ratio.md`
// — the operator picked B, shadow 28 / highlight 13, ratio 2.15, on both
// dark grounds; the gamma prediction was falsified, dark wanted light's
// exact numbers). The operator now wants to see that winner rendered
// taller. This holds the two *levels* fixed at the winning offsets and
// varies only stroke thickness in device pixels.
//
// There is a question buried in the field, named here rather than left
// implicit: the 2.15 ratio was established with both strokes one device
// pixel tall, where level ratio and ink (area) ratio are the same number.
// Once thickness varies independently per stroke they come apart — B is
// 2.15 by level but 4.3 by ink, D is 2.15 by level but 6.5 by ink, while
// C and F stay 2.15 by both because both strokes thicken together. A
// symmetric pick (A, C, F) is consistent with "ratio governs area, not
// just level, and thickness is free within that constraint." An
// asymmetric pick (B, D, E) is consistent with "ratio governs level only,
// and thickness is a free variable the ratio says nothing about." Labels
// below carry only pixel counts, the same way the ratio round's labels
// carried only offsets — the arithmetic stays out of what the operator
// sees while judging.
// ===================================================================

/// A through F's stroke thicknesses in device pixels, at the winning
/// fixed levels (shadow 28, highlight 13). Labels show pixel counts only.
const RULE_HEIGHT_ROWS: [(&str, &str, f32, f32); 6] = [
    ("a", "A. shadow 1px, highlight 1px (control)", 1.0, 1.0),
    ("b", "B. shadow 2px, highlight 1px", 2.0, 1.0),
    ("c", "C. shadow 2px, highlight 2px", 2.0, 2.0),
    ("d", "D. shadow 3px, highlight 1px", 3.0, 1.0),
    ("e", "E. shadow 3px, highlight 2px", 3.0, 2.0),
    ("f", "F. shadow 3px, highlight 3px", 3.0, 3.0),
];

/// The six-specimen height field, on one ground, both strokes at the
/// winning fixed offsets (shadow `-28`, highlight `+13`) with only
/// thickness varying per specimen. Reuses [`depth_matched_specimen`] for
/// the shell, unchanged since the depth-matched round.
fn rule_height_sheet(
    scale: f32,
    sheet_key: &str,
    ground_title: &str,
    theme_mode: ThemeMode,
    page_tok: TokenName,
    panel_tok: TokenName,
    panel_val: u8,
) -> (ViewNode, Theme) {
    let shadow_val = offset_channel(panel_val, -28);
    let highlight_val = offset_channel(panel_val, 13);
    let shadow_name = format!("spike.height-{sheet_key}-shadow");
    let highlight_name = format!("spike.height-{sheet_key}-highlight");
    let extra: Vec<(String, ColorValue)> = vec![
        (
            shadow_name.clone(),
            ColorValue::from_srgb8(shadow_val, shadow_val, shadow_val, 255),
        ),
        (
            highlight_name.clone(),
            ColorValue::from_srgb8(highlight_val, highlight_val, highlight_val, 255),
        ),
    ];
    let shadow_tok = TokenName::new(shadow_name).expect("spike name");
    let highlight_tok = TokenName::new(highlight_name).expect("spike name");

    let mut specimens = Vec::with_capacity(6);
    for (key, label_text, shadow_px, highlight_px) in RULE_HEIGHT_ROWS {
        let s_tok = shadow_tok.clone();
        let h_tok = highlight_tok.clone();
        specimens.push(depth_matched_specimen(
            key,
            label_text,
            scale,
            page_tok.clone(),
            panel_tok.clone(),
            move |k, scale| {
                rule(
                    k,
                    vec![
                        bar("b0", shadow_px, scale, s_tok.clone()),
                        bar("b1", highlight_px, scale, h_tok.clone()),
                    ],
                )
            },
        ));
    }

    let theme = extend_theme(theme_mode, extra);
    let mut children = vec![label("title", ground_title.to_owned())];
    children.extend(specimens);
    let mut page =
        filled_body(sheet_key, sp("spacing.2xs"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(RATIO_SHEET_WIDTH),
                max: Some(RATIO_SHEET_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    page.props.tokens.insert("background".into(), page_tok);
    (page, theme)
}

/// Sheet 1: shipped light, real `surface.base`/`surface.layer-one`
/// (255/242) — the highlight clamps to 255 at every thickness here, since
/// `+13` from 242 already hits the ceiling at 1px.
fn rule_height_light_sheet(scale: f32) -> (ViewNode, Theme) {
    rule_height_sheet(
        scale,
        "heightlight",
        "Rule height field \u{2014} shipped light ground (surface.base 255 / surface.layer-one 242)",
        ThemeMode::Light,
        tok("surface.base"),
        tok("surface.layer-one"),
        242,
    )
}

/// Sheet 2: shipped dark, real `surface.base`/`surface.layer-one`
/// (18/34) — shadow sits at 6, close to black, at every thickness.
fn rule_height_dark_sheet(scale: f32) -> (ViewNode, Theme) {
    rule_height_sheet(
        scale,
        "heightdark",
        "Rule height field \u{2014} shipped dark ground (surface.base 18 / surface.layer-one 34)",
        ThemeMode::Dark,
        tok("surface.base"),
        tok("surface.layer-one"),
        34,
    )
}

// ===================================================================
// The settled rule, fixed 2026-09-07: material is fully decided — a
// two-stroke groove, shadow 2 device pixels at panel fill -28, highlight
// 2 device pixels at panel fill +13, 4 absolute device pixels total,
// identical in both themes. It is no longer a variable; every sheet
// below builds it exactly this way and sweeps something else instead
// (radius, or ground).
// ===================================================================

/// The settled shadow offset, sRGB levels from the panel it sits on. No
/// longer swept.
const SETTLED_SHADOW_OFFSET: i16 = -28;
/// The settled highlight offset. No longer swept.
const SETTLED_HIGHLIGHT_OFFSET: i16 = 13;
/// The settled stroke thickness, device pixels, both strokes. No longer
/// swept.
const SETTLED_STROKE_PX: f32 = 2.0;

/// The settled rule's shadow/highlight tokens for one **neutral** panel
/// value, declared under a letters-only spike name scoped by `id` so
/// multiple grounds in one sheet do not collide.
fn settled_rule_tokens(
    id: &str,
    panel_val: u8,
    extra: &mut Vec<(String, TokenValue)>,
) -> (TokenName, TokenName) {
    let shadow_val = offset_channel(panel_val, SETTLED_SHADOW_OFFSET);
    let highlight_val = offset_channel(panel_val, SETTLED_HIGHLIGHT_OFFSET);
    let shadow_name = format!("spike.settled-{id}-shadow");
    let highlight_name = format!("spike.settled-{id}-highlight");
    extra.push((
        shadow_name.clone(),
        TokenValue::Color(ColorValue::from_srgb8(
            shadow_val, shadow_val, shadow_val, 255,
        )),
    ));
    extra.push((
        highlight_name.clone(),
        TokenValue::Color(ColorValue::from_srgb8(
            highlight_val,
            highlight_val,
            highlight_val,
            255,
        )),
    ));
    (
        TokenName::new(shadow_name).expect("spike name"),
        TokenName::new(highlight_name).expect("spike name"),
    )
}

/// [`settled_rule_tokens`], for a **tinted** (chromatic) panel — each
/// channel offset independently rather than assuming `R = G = B`, because
/// the light-ground sheet's whole point is that the tint carries through
/// the rule's own strokes, not only through the fill.
fn settled_rule_tokens_tinted(
    id: &str,
    panel_rgb: (u8, u8, u8),
    extra: &mut Vec<(String, TokenValue)>,
) -> (TokenName, TokenName) {
    let (pr, pg, pb) = panel_rgb;
    let shadow_rgb = (
        offset_channel(pr, SETTLED_SHADOW_OFFSET),
        offset_channel(pg, SETTLED_SHADOW_OFFSET),
        offset_channel(pb, SETTLED_SHADOW_OFFSET),
    );
    let highlight_rgb = (
        offset_channel(pr, SETTLED_HIGHLIGHT_OFFSET),
        offset_channel(pg, SETTLED_HIGHLIGHT_OFFSET),
        offset_channel(pb, SETTLED_HIGHLIGHT_OFFSET),
    );
    let shadow_name = format!("spike.settled-{id}-shadow");
    let highlight_name = format!("spike.settled-{id}-highlight");
    extra.push((
        shadow_name.clone(),
        TokenValue::Color(ColorValue::from_srgb8(
            shadow_rgb.0,
            shadow_rgb.1,
            shadow_rgb.2,
            255,
        )),
    ));
    extra.push((
        highlight_name.clone(),
        TokenValue::Color(ColorValue::from_srgb8(
            highlight_rgb.0,
            highlight_rgb.1,
            highlight_rgb.2,
            255,
        )),
    ));
    (
        TokenName::new(shadow_name).expect("spike name"),
        TokenName::new(highlight_name).expect("spike name"),
    )
}

/// One settled groove: shadow then highlight, [`SETTLED_STROKE_PX`] each.
fn settled_seam(key: &str, scale: f32, shadow: TokenName, highlight: TokenName) -> ViewNode {
    rule(
        key,
        vec![
            bar("b0", SETTLED_STROKE_PX, scale, shadow),
            bar("b1", SETTLED_STROKE_PX, scale, highlight),
        ],
    )
}

/// A spike `Shape` token at a literal corner radius, under a letters-only
/// name scoped by `id`.
fn radius_token(id: &str, radius: f32, extra: &mut Vec<(String, TokenValue)>) -> TokenName {
    let name = format!("spike.radius-{id}");
    extra.push((
        name.clone(),
        TokenValue::Shape(ShapeValue {
            corner_radius: radius,
        }),
    ));
    TokenName::new(name).expect("spike name")
}

// ===================================================================
// Sheet 1, `radius-compound-1x`: six realistic dialog-sized cards on the
// shipped light ground, radius swept 4/6/8/10/12/16, everything else
// (fill, the settled rule) fixed. Labels carry the bare radius number
// only, per this round's instruction to keep commentary off the
// specimens.
// ===================================================================

/// The six radii under test, spelled out — `TokenName`'s grammar refuses
/// a non-leading digit in a segment, so `radius-4` as a token segment is
/// out, the same wall the material and paper-tone sheets hit.
const RADIUS_COMPOUND_ROWS: [(&str, &str, f32); 6] = [
    ("four", "4", 4.0),
    ("six", "6", 6.0),
    ("eight", "8", 8.0),
    ("ten", "10", 10.0),
    ("twelve", "12", 12.0),
    ("sixteen", "16", 16.0),
];

/// Columns in the radius-compound grid; six cards at three per row.
const RADIUS_GRID_COLUMNS: usize = 3;

/// One dialog-sized card's content: a title, two content rows, one
/// settled groove between them — a title-plus-body compound sized like a
/// small dialog, per the brief, not an empty swatch.
fn radius_card_content(scale: f32, shadow: TokenName, highlight: TokenName) -> ViewNode {
    let mut node = filled_body(
        "card-inner",
        sp("spacing.sm"),
        vec![
            label("card-title", "Dialog title"),
            body_line("card-row1", "First row of dialog content."),
            settled_seam("card-seam", scale, shadow, highlight),
            body_line("card-row2", "Second row of dialog content."),
        ],
    );
    node.props.padding = Some(InsetRefs::all(tok("spacing.md")));
    node
}

fn radius_compound_specimen(
    key: &str,
    label_text: &str,
    radius: f32,
    scale: f32,
    shadow: TokenName,
    highlight: TokenName,
    extra: &mut Vec<(String, TokenValue)>,
) -> ViewNode {
    let radius_tok = radius_token(key, radius, extra);
    let content = radius_card_content(scale, shadow, highlight);
    let mut card = ViewNode::new(NodeKind::Grid, format!("{key}-card")).with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    });
    card.props
        .tokens
        .insert("background".into(), tok("surface.layer-one"));
    card.props.tokens.insert("radius".into(), radius_tok);
    card.props
        .tokens
        .insert("shadow".into(), tok("shadow.raised"));
    let card = card.with_children(vec![content]);

    let caption = label(&format!("{key}-caption"), label_text.to_owned());
    let mut cell = filled_body(key, sp("spacing.2xs"), vec![caption, card]);
    cell.props.padding = Some(InsetRefs::all(tok("spacing.xs")));
    cell
}

fn radius_compound_sheet(scale: f32) -> (ViewNode, Theme) {
    let mut extra: Vec<(String, TokenValue)> = Vec::new();
    let (shadow_tok, highlight_tok) = settled_rule_tokens("radiuscompound", 242, &mut extra);

    let cells: Vec<ViewNode> = RADIUS_COMPOUND_ROWS
        .iter()
        .map(|(key, label_text, radius)| {
            radius_compound_specimen(
                key,
                label_text,
                *radius,
                scale,
                shadow_tok.clone(),
                highlight_tok.clone(),
                &mut extra,
            )
        })
        .collect();

    let theme = extend_theme_values(ThemeMode::Light, extra);

    let grid = ViewNode::new(NodeKind::Grid, "cards")
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }; RADIUS_GRID_COLUMNS],
            rows: vec![TrackSize::FitContent; 2],
            align: Some(Align::Stretch),
            column_spacing: sp("spacing.md"),
            row_spacing: sp("spacing.md"),
            ..Props::default()
        })
        .with_children(cells);

    let mut children = vec![label(
        "title",
        "Radius compound field (light) \u{2014} shipped surface.layer-one, settled rule".to_owned(),
    )];
    children.push(grid);
    let mut page =
        filled_body("radiuscompound", sp("spacing.sm"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(RATIO_SHEET_WIDTH),
                max: Some(RATIO_SHEET_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.md")));
    page.props
        .tokens
        .insert("background".into(), tok("surface.base"));
    (page, theme)
}

// ===================================================================
// Sheet 2, `radius-atomic-1x`: five rows on the shipped light ground,
// radius swept 2/4/6/8/10, each row holding a filled button, a text
// input, a checkbox and a tag side by side at that radius. Button, field
// and tag are the real shipped builders with their own `radius` token
// overridden afterward — a genuine settable-radius path for all three,
// though field's own real anatomy has no radius at all (see
// `radius_atomic_row_specimen`'s doc). Checkbox is a hand-built stand-in
// — see [`checkbox_standin`]'s doc for why the real one is not reachable.
// ===================================================================

/// The five radii under test. Labels carry the bare number only.
const RADIUS_ATOMIC_ROWS: [(&str, &str, f32); 5] = [
    ("two", "2", 2.0),
    ("four", "4", 4.0),
    ("six", "6", 6.0),
    ("eight", "8", 8.0),
    ("ten", "10", 10.0),
];

/// Fixed width every atomic cell gets, so the four controls line up
/// column-for-column across rows regardless of each one's own natural
/// size — `field()` in particular has no natural width of its own (its
/// leaf carries no literal text children, only a placeholder), so
/// without this it would collapse to zero.
const ATOMIC_CELL_WIDTH: f32 = 180.0;

/// Carbon's real checkbox mark size, confirmed from the shipped source's
/// own `component::controls::CHECKBOX_BOX` constant (16.0, `pub(crate)`
/// so this crate cannot read it directly — the value is copied here, not
/// guessed).
const CHECKBOX_BOX_PX: f32 = 16.0;

fn atomic_cell(key: &str, child: ViewNode) -> ViewNode {
    let cell = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(ATOMIC_CELL_WIDTH),
                max: Some(ATOMIC_CELL_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    cell.with_children(vec![child])
}

/// A hand-built stand-in for Carbon's checkbox, at a settable radius.
///
/// Unlike the button and tag below, the real `checkbox()`'s 16×16 mark is
/// not on the node it returns: it is built by private helper functions
/// (`empty_mark`/`marked_box` in `gorgon_petra::component::controls`) on
/// a child keyed `"box"`, and `checkbox_box` — the one function that
/// exposes that mark alone — is `pub(crate)` to `gorgon-petra`, which
/// this crate is not inside. There is no accessible path to override its
/// radius, faithful or otherwise, without editing shipped component
/// source, which this spike does not do. This stand-in matches its
/// visible anatomy — a square mark plus a label — at the real 16px size
/// ([`CHECKBOX_BOX_PX`]), with a border and the swept radius, but **it is
/// not the shipped component tree** the way the button, tag and (with a
/// separate caveat) field specimens are.
fn checkbox_standin(key: &str, radius: TokenName) -> ViewNode {
    let mut mark = ViewNode::new(NodeKind::Grid, format!("{key}-mark"))
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(CHECKBOX_BOX_PX),
                max: Some(CHECKBOX_BOX_PX),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(CHECKBOX_BOX_PX),
                max: Some(CHECKBOX_BOX_PX),
                priority: 0,
            },
        });
    mark.props
        .tokens
        .insert("background".into(), tok("surface.layer-one"));
    mark.props
        .tokens
        .insert("border".into(), tok("border-strong"));
    mark.props.tokens.insert("radius".into(), radius);
    row(
        key,
        sp("spacing.xs"),
        vec![mark, label(&format!("{key}-label"), "Checked".to_owned())],
    )
}

/// One radius-atomic row: a label, then a button, a field, a checkbox
/// stand-in and a tag, all forced to the same literal radius.
///
/// **On `field()` specifically:** Carbon's real text input has no radius
/// channel at all — `bind_field_chrome` never sets one, and the module's
/// own doc says so outright ("Square corners: Carbon's field has no
/// radius"). Forcing a `radius` token onto it here renders (the paint
/// path resolves the slot for any node kind), but it is testing a
/// variant that contradicts the shipped design, not exercising a real
/// settable-radius path the way button and tag are. Included because the
/// brief asked for all four atomics at every radius; treat this one
/// specimen skeptically.
fn radius_atomic_row_specimen(
    key: &str,
    label_text: &str,
    radius: f32,
    extra: &mut Vec<(String, TokenValue)>,
) -> ViewNode {
    let radius_tok = radius_token(key, radius, extra);

    let mut btn = primary_button(format!("{key}-button"), "Save");
    btn.props.tokens.insert("radius".into(), radius_tok.clone());

    let mut input = field(format!("{key}-field"), "Label");
    input
        .props
        .tokens
        .insert("radius".into(), radius_tok.clone());

    let check = checkbox_standin(&format!("{key}-checkbox"), radius_tok.clone());

    let mut chip = tag(format!("{key}-tag"), "Tag");
    chip.props.tokens.insert("radius".into(), radius_tok);

    let cells = vec![
        atomic_cell(&format!("{key}-button-cell"), btn),
        atomic_cell(&format!("{key}-field-cell"), input),
        atomic_cell(&format!("{key}-checkbox-cell"), check),
        atomic_cell(&format!("{key}-tag-cell"), chip),
    ];
    let content = row(&format!("{key}-row"), sp("spacing.md"), cells);
    let caption = label(&format!("{key}-caption"), label_text.to_owned());
    row(key, sp("spacing.lg"), vec![caption, content])
}

fn radius_atomic_sheet() -> (ViewNode, Theme) {
    let mut extra: Vec<(String, TokenValue)> = Vec::new();

    let rows: Vec<ViewNode> = RADIUS_ATOMIC_ROWS
        .iter()
        .map(|(key, label_text, radius)| {
            radius_atomic_row_specimen(key, label_text, *radius, &mut extra)
        })
        .collect();

    let theme = extend_theme_values(ThemeMode::Light, extra);
    let mut children = vec![label(
        "title",
        "Radius atomic field (light) \u{2014} button / field / checkbox stand-in / tag".to_owned(),
    )];
    children.extend(rows);
    let mut page =
        filled_body("radiusatomic", sp("spacing.md"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(RATIO_SHEET_WIDTH),
                max: Some(RATIO_SHEET_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.md")));
    page.props
        .tokens
        .insert("background".into(), tok("surface.base"));
    (page, theme)
}

// ===================================================================
// Sheet 3, `light-ground-1x`: the light-ground question, re-asked with
// the settled rule. Five candidates from the earlier paper-tone field.
// For each, the page AND the card carry the candidate's tint (the same
// per-channel `surface.layer-one` offset the paper-tone rounds used,
// [`panel_rgb_for`]), and the settled rule's own two strokes are derived
// from that tinted card fill, per channel, so the rule stays the same
// hue as the card rather than reading as a neutral grey line laid over a
// tinted surface.
// ===================================================================

/// Five of the earlier paper-tone field's eight candidates. Labels carry
/// each candidate's own hex, no descriptive note, matching this round's
/// no-commentary discipline for the radius sheets.
const LIGHT_GROUND_CANDIDATES: [PaperCandidate; 5] = [
    PaperCandidate {
        slug: "white",
        hex: "#ffffff",
        note: "today's white",
        rgb: (255, 255, 255),
    },
    PaperCandidate {
        slug: "creamnt",
        hex: "#efe9dd",
        note: "warm cream, ~90%",
        rgb: (239, 233, 221),
    },
    PaperCandidate {
        slug: "bone",
        hex: "#e8e2d4",
        note: "bone / manila, ~87%",
        rgb: (232, 226, 212),
    },
    PaperCandidate {
        slug: "greige",
        hex: "#e8e4dc",
        note: "greige, ~90%",
        rgb: (232, 228, 220),
    },
    PaperCandidate {
        slug: "newsprint",
        hex: "#eae9e4",
        note: "cool newsprint, ~91%",
        rgb: (234, 233, 228),
    },
];

/// One light-ground card's content: the candidate's own hex as the
/// title, three rows, two settled grooves — `card_content`'s shape, not
/// `card_content` itself, since this sheet needs a fresh per-candidate
/// tinted rule rather than `card_content`'s caller-supplied (but
/// untinted, 1px) pair.
fn light_ground_card_content(
    hex: &str,
    shadow: TokenName,
    highlight: TokenName,
    scale: f32,
) -> ViewNode {
    let mut node = filled_body(
        "card-inner",
        None,
        vec![
            label("card-title", hex.to_owned()),
            body_line("card-row1", "First row of realistic content in the panel."),
            settled_seam("card-row1-rule", scale, shadow.clone(), highlight.clone()),
            body_line("card-row2", "Second row of realistic content in the panel."),
            settled_seam("card-row2-rule", scale, shadow, highlight),
            body_line("card-row3", "Third row of realistic content in the panel."),
        ],
    );
    node.props.padding = Some(InsetRefs::all(tok("spacing.sm")));
    node
}

fn light_ground_specimen(
    candidate: &PaperCandidate,
    scale: f32,
    extra: &mut Vec<(String, TokenValue)>,
) -> ViewNode {
    let key = candidate.slug;
    let (pr, pg, pb) = candidate.rgb;
    let panel_rgb = panel_rgb_for(candidate);

    let page_name = format!("spike.lightground-{key}-page");
    let panel_name = format!("spike.lightground-{key}-panel");
    extra.push((
        page_name.clone(),
        TokenValue::Color(ColorValue::from_srgb8(pr, pg, pb, 255)),
    ));
    extra.push((
        panel_name.clone(),
        TokenValue::Color(ColorValue::from_srgb8(
            panel_rgb.0,
            panel_rgb.1,
            panel_rgb.2,
            255,
        )),
    ));
    let page_tok = TokenName::new(page_name).expect("spike name");
    let panel_tok = TokenName::new(panel_name).expect("spike name");

    let (shadow_tok, highlight_tok) = settled_rule_tokens_tinted(key, panel_rgb, extra);

    let content = light_ground_card_content(candidate.hex, shadow_tok, highlight_tok, scale);
    let mut card = ViewNode::new(NodeKind::Grid, format!("{key}-card")).with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    });
    card.props.tokens.insert("background".into(), panel_tok);
    card.props
        .tokens
        .insert("radius".into(), tok("shape.corner-md"));
    card.props
        .tokens
        .insert("shadow".into(), tok("shadow.raised"));
    let card = card.with_children(vec![content]);

    let mut swatch = filled_body(key, None, vec![card]);
    swatch.props.tokens.insert("background".into(), page_tok);
    swatch.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    swatch
}

/// Width for this sheet: wider than [`PAPER_PANEL_WIDTH`] since the row
/// sentences are unchanged and this sheet's card padding is a touch
/// roomier (`spacing.sm`, not `spacing.2xs`) than the paper-tone sheets'.
const LIGHT_GROUND_WIDTH: f32 = 700.0;

fn light_ground_sheet(scale: f32) -> (ViewNode, Theme) {
    let mut extra: Vec<(String, TokenValue)> = Vec::new();
    let specimens: Vec<ViewNode> = LIGHT_GROUND_CANDIDATES
        .iter()
        .map(|candidate| light_ground_specimen(candidate, scale, &mut extra))
        .collect();

    let theme = extend_theme_values(ThemeMode::Light, extra);
    let mut children = vec![label(
        "title",
        "Light ground field (light) \u{2014} settled rule, tint carried through page and card"
            .to_owned(),
    )];
    children.extend(specimens);
    let mut page =
        filled_body("lightground", sp("spacing.2xs"), children).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(LIGHT_GROUND_WIDTH),
                max: Some(LIGHT_GROUND_WIDTH),
                priority: 0,
            },
            ..Constraints::default()
        });
    page.props.padding = Some(InsetRefs::all(tok("spacing.2xs")));
    page.props
        .tokens
        .insert("background".into(), tok("surface.base"));
    (page, theme)
}

// ===================================================================
// The App and the Camera constructor that drives it.
// ===================================================================

struct SpikeApp {
    root: ViewNode,
    pending: Option<Theme>,
}

impl gorgon_petra::layout::RowSource for SpikeApp {
    fn rows(
        &mut self,
        _source: &str,
        _range: std::ops::Range<usize>,
    ) -> Vec<std::sync::Arc<gorgon_petra::tree::ViewNode>> {
        Vec::new()
    }
}

impl gorgon_petra_egui::host::App for SpikeApp {
    fn view(&mut self) -> ViewNode {
        self.root.clone()
    }

    fn handle(
        &mut self,
        _event: &gorgon_petra::input::InputEvent,
        _route: &gorgon_petra::input::Route,
        _frame: Option<&gorgon_petra::frame::PetrifiedFrame>,
    ) {
    }

    fn take_changes(&mut self) -> gorgon_petra::layout::ChangeSet {
        gorgon_petra::layout::ChangeSet::All
    }

    fn theme_request(&mut self) -> Option<Theme> {
        self.pending.take()
    }
}

impl Camera<SpikeApp> {
    /// Host `root` under `theme`, at `scale`, and settle it — the same
    /// two-pass-then-assert shape as [`Camera::<Fixture>::fixture`], with
    /// `theme` supplied directly (this spike needs its own extended
    /// themes, not just light/dark) and `scale` exposed (the catalog's
    /// `Camera::on`/`at_scale` split, which [`Camera::<Fixture>::fixture`]
    /// does not have).
    fn spike(root: ViewNode, theme: Theme, scale: f32) -> Self {
        let mode = theme.mode();
        let app = SpikeApp {
            root,
            pending: Some(theme),
        };
        let ctx = super::headless();
        ctx.set_pixels_per_point(scale);
        let mut host = gorgon_petra_egui::host::Host::new(
            &ctx,
            app,
            gorgon_petra_egui::host::default_presenter(),
        );
        // The radius-atomic sheet (2026-09-07) is the first spike specimen
        // to use a real interactive component (`primary_button`) rather
        // than a hand-built leaf, and a button declares a `button-press`
        // transition tree acceptance checks against the host's own
        // registry. `Camera::<Fixture>::fixture` never needed this because
        // its subjects are never real interactive components; the real
        // `Catalog` registers the same call. Registering it here is a
        // superset of every earlier sheet's needs, not a behaviour change
        // for any of them.
        host.set_transitions(gorgon_petra::anim::shipped_registry());
        ctx.run_ui(super::sized(egui::RawInput::default()), |_| host.pass(&ctx))
            .drop_without_applying_deltas();
        host.set_reduced_motion(true);
        let clock = ctx.input(|input| input.time);
        let mut cam = Self {
            ctx,
            host,
            shooter: gorgon_petra_testkit::snapshot::Snapshotter::new(),
            page: format!("spike({mode:?}@{scale}x)"),
            clipboard: Vec::new(),
            clock,
            pointer: None,
        };
        // Two passes: the first delivers the theme request, the second
        // lays out and paints under it — `Camera::<Fixture>::fixture`'s
        // own comment explains why one pass is not enough.
        cam.settle();
        cam.settle();
        cam
    }
}

// ===================================================================
// Shots.
// ===================================================================

#[cfg(test)]
mod tests {
    use super::Camera;
    use super::{
        ThemeMode, ab_adjacent_sheet, box_sheet, container_a_fullbox, container_b_horizontal,
        dark_strategies_sheet, headroom_sweep_sheet, light_ground_sheet, materials_sheet,
        paper_tone_corrected_sheet, paper_tone_sheet, radius_atomic_sheet, radius_compound_sheet,
        realistic_panel_at_ground, rule_depth_matched_bone_sheet, rule_depth_matched_white_sheet,
        rule_height_dark_sheet, rule_height_light_sheet, rule_on_white_sheet,
        rule_ratio_dark_reference_sheet, rule_ratio_dark_sheet,
    };

    #[test]
    fn materials_light_2x() {
        let (root, theme) = materials_sheet(ThemeMode::Light, 2.0);
        Camera::spike(root, theme, 2.0).shoot("materials-light-2x");
    }

    #[test]
    fn materials_dark_2x() {
        let (root, theme) = materials_sheet(ThemeMode::Dark, 2.0);
        Camera::spike(root, theme, 2.0).shoot("materials-dark-2x");
    }

    #[test]
    fn materials_light_1x() {
        let (root, theme) = materials_sheet(ThemeMode::Light, 1.0);
        Camera::spike(root, theme, 1.0).shoot("materials-light-1x");
    }

    #[test]
    fn materials_dark_1x() {
        let (root, theme) = materials_sheet(ThemeMode::Dark, 1.0);
        Camera::spike(root, theme, 1.0).shoot("materials-dark-1x");
    }

    #[test]
    fn dark_strategies_2x() {
        let (root, theme) = dark_strategies_sheet(2.0);
        Camera::spike(root, theme, 2.0).shoot("dark-strategies-2x");
    }

    #[test]
    fn box_light_2x() {
        let (root, theme) = box_sheet(ThemeMode::Light);
        Camera::spike(root, theme, 2.0).shoot("box-light-2x");
    }

    #[test]
    fn box_dark_2x() {
        let (root, theme) = box_sheet(ThemeMode::Dark);
        Camera::spike(root, theme, 2.0).shoot("box-dark-2x");
    }

    #[test]
    fn box_light_1x() {
        let (root, theme) = box_sheet(ThemeMode::Light);
        Camera::spike(root, theme, 1.0).shoot("box-light-1x");
    }

    #[test]
    fn container_a_fullbox_light() {
        let (root, theme) = container_a_fullbox(ThemeMode::Light, 2.0);
        Camera::spike(root, theme, 2.0).shoot("container-a-fullbox-light");
    }

    #[test]
    fn container_a_fullbox_dark() {
        let (root, theme) = container_a_fullbox(ThemeMode::Dark, 2.0);
        Camera::spike(root, theme, 2.0).shoot("container-a-fullbox-dark");
    }

    #[test]
    fn container_b_horizontal_light() {
        let (root, theme) = container_b_horizontal(ThemeMode::Light, 2.0);
        Camera::spike(root, theme, 2.0).shoot("container-b-horizontal-light");
    }

    #[test]
    fn container_b_horizontal_dark() {
        let (root, theme) = container_b_horizontal(ThemeMode::Dark, 2.0);
        Camera::spike(root, theme, 2.0).shoot("container-b-horizontal-dark");
    }

    #[test]
    fn ab_adjacent_light() {
        let (root, theme) = ab_adjacent_sheet(ThemeMode::Light, 2.0);
        Camera::spike(root, theme, 2.0).shoot("ab-adjacent-light");
    }

    #[test]
    fn ab_adjacent_dark() {
        let (root, theme) = ab_adjacent_sheet(ThemeMode::Dark, 2.0);
        Camera::spike(root, theme, 2.0).shoot("ab-adjacent-dark");
    }

    #[test]
    fn headroom_sweep_light_2x() {
        let steps: [(&str, f32); 5] = [
            ("100pct", 1.0),
            ("95pct", 0.95),
            ("90pct", 0.90),
            ("85pct", 0.85),
            ("80pct", 0.80),
        ];
        let (root, theme) = headroom_sweep_sheet(ThemeMode::Light, &steps, 2.0);
        Camera::spike(root, theme, 2.0).shoot("headroom-sweep-light-2x");
    }

    #[test]
    fn headroom_sweep_dark_2x() {
        // Extended past the brief's suggested 32% top: the shadow stroke
        // was still under a 15-level floor at 32% (see the report), so two
        // more steps were added to find where it actually crosses rather
        // than stopping at the suggested range and guessing.
        let steps: [(&str, f32); 7] = [
            ("07pct", 18.0 / 255.0),
            ("12pct", 0.12),
            ("18pct", 0.18),
            ("25pct", 0.25),
            ("32pct", 0.32),
            ("40pct", 0.40),
            ("48pct", 0.48),
        ];
        let (root, theme) = headroom_sweep_sheet(ThemeMode::Dark, &steps, 2.0);
        Camera::spike(root, theme, 2.0).shoot("headroom-sweep-dark-2x");
    }

    #[test]
    fn realistic_panel_light_2x() {
        // 80% of white (#cccccc): the sweep's first light-mode step where
        // both bevel-1 strokes clear a ~15-sRGB8-level floor at once
        // (shadow Δ30, highlight Δ17 against the ground) — see the report.
        let (root, theme) = realistic_panel_at_ground(ThemeMode::Light, 2.0, 0.80);
        Camera::spike(root, theme, 2.0).shoot("realistic-panel-light-2x");
    }

    #[test]
    fn realistic_panel_dark_2x() {
        // 40% of white (#666666): the sweep's first dark-mode step where
        // both strokes clear the same floor (shadow Δ16, highlight Δ66) —
        // past the brief's suggested 32% top; see the report for why.
        let (root, theme) = realistic_panel_at_ground(ThemeMode::Dark, 2.0, 0.40);
        Camera::spike(root, theme, 2.0).shoot("realistic-panel-dark-2x");
    }

    /// Not a shot — a calibration check. Eight full [`card_content`]
    /// specimens stacked in one image risk the exact silent-clipping bug
    /// `box_sheet` had (the capture viewport is a fixed 900 units tall,
    /// re-stamped on every pass, and a `shoot` that ran past it reports
    /// nothing wrong). This reads the last specimen's own placed rect
    /// before any screenshot is trusted, and fails loudly if its bottom
    /// edge would fall outside the frame.
    #[test]
    fn paper_tone_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = paper_tone_sheet(2.0);
        let cam = Camera::spike(root, theme, 2.0);
        for slug in [
            "white",
            "greycontrol",
            "creamnf",
            "creamnt",
            "bone",
            "greige",
            "newsprint",
            "warmtip",
        ] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("warmtip-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 8th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn paper_tone_light_2x() {
        let (root, theme) = paper_tone_sheet(2.0);
        Camera::spike(root, theme, 2.0).shoot("paper-tone-candidates-light-2x");
    }

    /// Same calibration approach as [`paper_tone_sheet_fits_the_capture_viewport`],
    /// for the corrected-rule sheet — its specimens are the same size as
    /// the uncorrected sheet's (only the rule's colour source changed), so
    /// this is expected to pass by the same margin, and is checking that
    /// expectation rather than guessing at it.
    #[test]
    fn paper_tone_corrected_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = paper_tone_corrected_sheet(2.0);
        let cam = Camera::spike(root, theme, 2.0);
        for slug in [
            "white",
            "greycontrol",
            "creamnf",
            "creamnt",
            "bone",
            "greige",
            "newsprint",
            "warmtip",
        ] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("warmtip-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 8th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn paper_tone_corrected_light_2x() {
        let (root, theme) = paper_tone_corrected_sheet(2.0);
        Camera::spike(root, theme, 2.0).shoot("paper-tone-corrected-light-2x");
    }

    /// Calibration for sheet 2 (four rule treatments, white ground only):
    /// same 900-unit-viewport concern as the eight-candidate sheets, this
    /// time for four specimens built with the compact
    /// [`rule_treatment_card_content`] rhythm rather than letting anything
    /// silently collapse.
    #[test]
    fn rule_on_white_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = rule_on_white_sheet(2.0);
        let cam = Camera::spike(root, theme, 2.0);
        for slug in ["control", "groove", "shadow1px", "shadow2px"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("shadow2px-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 4th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn rule_on_white_light_2x() {
        let (root, theme) = rule_on_white_sheet(2.0);
        Camera::spike(root, theme, 2.0).shoot("rule-on-white-light-2x");
    }

    /// Calibration for the depth-matched white sheet at a true 1x capture
    /// (`scale` sets `pixels_per_point` directly — see [`Camera::spike`]'s
    /// own doc): five specimens under the same fixed 900-logical-unit
    /// viewport every other sheet in this file has had to fit.
    #[test]
    fn rule_depth_matched_white_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = rule_depth_matched_white_sheet(1.0);
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["a", "b", "c", "d", "e"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("e-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 5th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn rule_depth_matched_white_1x() {
        let (root, theme) = rule_depth_matched_white_sheet(1.0);
        Camera::spike(root, theme, 1.0).shoot("rule-depth-matched-white-1x");
    }

    /// Same calibration, bone-reference ground.
    #[test]
    fn rule_depth_matched_bone_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = rule_depth_matched_bone_sheet(1.0);
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["a", "b", "c", "d", "e"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("e-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 5th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn rule_depth_matched_bone_1x() {
        let (root, theme) = rule_depth_matched_bone_sheet(1.0);
        Camera::spike(root, theme, 1.0).shoot("rule-depth-matched-bone-1x");
    }

    /// Calibration for the seven-specimen ratio field at 1x: same
    /// fixed-900-unit-viewport concern every sheet in this file has had.
    #[test]
    fn rule_ratio_dark_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = rule_ratio_dark_sheet(1.0);
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["a", "b", "c", "d", "e", "f", "g"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("g-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 7th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn rule_ratio_dark_1x() {
        let (root, theme) = rule_ratio_dark_sheet(1.0);
        Camera::spike(root, theme, 1.0).shoot("rule-ratio-dark-1x");
    }

    /// Same calibration, mid-dark reference ground.
    #[test]
    fn rule_ratio_dark_reference_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = rule_ratio_dark_reference_sheet(1.0);
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["a", "b", "c", "d", "e", "f", "g"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("g-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 7th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn rule_ratio_dark_reference_1x() {
        let (root, theme) = rule_ratio_dark_reference_sheet(1.0);
        Camera::spike(root, theme, 1.0).shoot("rule-ratio-dark-reference-1x");
    }

    /// Calibration for the six-specimen height field at 1x: same fixed
    /// 900-unit-viewport concern every sheet in this file has had, worth
    /// re-checking here specifically because F's strokes are 3px each,
    /// the tallest anything in this file has built a seam from.
    #[test]
    fn rule_height_light_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = rule_height_light_sheet(1.0);
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["a", "b", "c", "d", "e", "f"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("f-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 6th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn rule_height_light_1x() {
        let (root, theme) = rule_height_light_sheet(1.0);
        Camera::spike(root, theme, 1.0).shoot("rule-height-light-1x");
    }

    /// Same calibration, dark ground.
    #[test]
    fn rule_height_dark_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = rule_height_dark_sheet(1.0);
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["a", "b", "c", "d", "e", "f"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("f-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 6th specimen's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn rule_height_dark_1x() {
        let (root, theme) = rule_height_dark_sheet(1.0);
        Camera::spike(root, theme, 1.0).shoot("rule-height-dark-1x");
    }

    /// Calibration for the radius-compound sheet: six cards in a 3x2
    /// grid, checked against the fixed 900-unit-tall capture viewport.
    #[test]
    fn radius_compound_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = radius_compound_sheet(1.0);
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["four", "six", "eight", "ten", "twelve", "sixteen"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("sixteen-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the bottom-row cards' bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn radius_compound_light_1x() {
        let (root, theme) = radius_compound_sheet(1.0);
        Camera::spike(root, theme, 1.0).shoot("radius-compound-1x");
    }

    /// Calibration for the radius-atomic sheet: five rows of four
    /// controls each.
    #[test]
    fn radius_atomic_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = radius_atomic_sheet();
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["two", "four", "six", "eight", "ten"] {
            let r = cam.rect(slug);
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("ten");
        assert!(
            last.y + last.h < WINDOW[1],
            "the last row's bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn radius_atomic_light_1x() {
        let (root, theme) = radius_atomic_sheet();
        Camera::spike(root, theme, 1.0).shoot("radius-atomic-1x");
    }

    /// Calibration for the light-ground sheet: five candidate cards.
    #[test]
    fn light_ground_sheet_fits_the_capture_viewport() {
        use super::super::WINDOW;
        let (root, theme) = light_ground_sheet(1.0);
        let cam = Camera::spike(root, theme, 1.0);
        for slug in ["white", "creamnt", "bone", "greige", "newsprint"] {
            let r = cam.rect(&format!("{slug}-card"));
            eprintln!("{slug}: y={} h={} bottom={}", r.y, r.h, r.y + r.h);
        }
        let last = cam.rect("newsprint-card");
        assert!(
            last.y + last.h < WINDOW[1],
            "the 5th candidate's card bottom ({}) falls outside the {}-unit capture \
             viewport — it would be silently clipped from the screenshot",
            last.y + last.h,
            WINDOW[1]
        );
    }

    #[test]
    fn light_ground_light_1x() {
        let (root, theme) = light_ground_sheet(1.0);
        Camera::spike(root, theme, 1.0).shoot("light-ground-1x");
    }
}
