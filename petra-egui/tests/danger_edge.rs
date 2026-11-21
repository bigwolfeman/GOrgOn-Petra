//! The danger button on every layer of every theme, photographed headless and
//! measured off the pixels: its edge must clear SC 1.4.11's 3:1 against the
//! layer it sits on, wherever the red fill cannot
//! (`.agents/notes/implemented/architecture/2026-11-11-gray90-danger-fill-cannot-clear-layer-one.md`).
//!
//! The token test (`token::shipped::tests::the_danger_edge_finds_the_button_on_every_layer`)
//! holds the values. This one holds the painted picture: an edge that
//! resolved but did not paint, or painted under the fill, passes the token
//! test and fails here. Set `PETRA_SHOT_DIR` to write one PNG per theme and
//! look at them.

use std::ops::Range;
use std::sync::Arc;

use egui::{Context, Pos2, RawInput, Rect as EguiRect, vec2};
use gorgon_petra::component::{danger_button, padded};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::{LAYER_TOKENS, Presenter, ThemeId, TokenName, shipped};
use gorgon_petra::tree::{AxisConstraint, Constraints, InsetRefs, NodeKind, Props, ViewNode};
use gorgon_petra_egui::host::{App, Host};

/// One panel per layer, side by side.
const PANEL: [f32; 2] = [200.0, 120.0];
const WINDOW: [f32; 2] = [PANEL[0] * 4.0, PANEL[1]];

fn tok(name: &str) -> TokenName {
    TokenName::new(name).expect("well-formed token name")
}

fn fixed(w: f32, h: f32) -> Constraints {
    let axis = |v: f32| AxisConstraint {
        min: Some(v),
        max: Some(v),
        priority: 0,
    };
    Constraints {
        horizontal: axis(w),
        vertical: axis(h),
    }
}

/// Four panels, `surface.base` to `surface.layer-three`, each holding one
/// danger button.
struct Layers;

impl RowSource for Layers {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Layers {
    fn view(&mut self) -> ViewNode {
        let panels = LAYER_TOKENS
            .iter()
            .enumerate()
            .map(|(depth, layer)| {
                let mut props = Props::default();
                props.tokens.insert("background".into(), tok(layer));
                let mut panel = ViewNode::new(NodeKind::Stack, format!("layer-{depth}"))
                    .with_props(props)
                    .child(danger_button(format!("delete-{depth}"), "Delete"));
                panel.constraints = fixed(PANEL[0], PANEL[1]);
                padded(panel, InsetRefs::all(tok("spacing.xl")))
            })
            .collect();
        let props = Props {
            axis: Some(gorgon_petra::geom::Axis::Horizontal),
            ..Props::default()
        };
        let mut root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(props)
            .with_children(panels);
        root.constraints = fixed(WINDOW[0], WINDOW[1]);
        root
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

fn sized(mut input: RawInput) -> RawInput {
    input.screen_rect = Some(EguiRect::from_min_size(
        Pos2::ZERO,
        vec2(WINDOW[0], WINDOW[1]),
    ));
    input
}

fn luminance(p: [u8; 3]) -> f32 {
    let lin = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(p[0]) + 0.7152 * lin(p[1]) + 0.0722 * lin(p[2])
}

fn contrast(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (x, y) = (luminance(a) + 0.05, luminance(b) + 0.05);
    x.max(y) / x.min(y)
}

/// Every theme, every layer: the button's outermost painted ring clears
/// 3:1 against the panel just outside it.
///
/// Falsify by dropping `border` from `Variant::Danger`'s chrome: Gray 90
/// fails on layer one and Gray 100 on layers two and three.
#[test]
fn the_danger_button_is_findable_on_every_layer_of_every_theme() {
    for id in ThemeId::ALL {
        let ctx = Context::default();
        ctx.run_ui(sized(RawInput::default()), |_| {})
            .drop_without_applying_deltas();
        let mut host = Host::new(&ctx, Layers, Presenter::new(shipped(id)));
        host.set_reduced_motion(true);
        let output = ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx));
        let frame = host.frame().expect("a frame");
        let mut shooter = gorgon_petra_testkit::snapshot::Snapshotter::new();
        let shot = shooter
            .capture(&ctx, &output, frame, None)
            .expect("capture refused");
        output.drop_without_applying_deltas();
        if let Some(dir) = std::env::var_os("PETRA_SHOT_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).expect("shot dir");
            std::fs::write(
                dir.join(format!("danger-edge-{}.png", id.as_str())),
                &shot.png,
            )
            .expect("write shot");
        }
        let img = image::load_from_memory(&shot.png)
            .expect("a PNG")
            .to_rgba8();
        let scale = img.width() as f32 / WINDOW[0];
        let at = |x: f32, y: f32| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let p = img.get_pixel((x * scale) as u32, (y * scale) as u32).0;
            [p[0], p[1], p[2]]
        };
        for depth in 0..LAYER_TOKENS.len() {
            let button = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(&format!("/delete-{depth}")))
                .unwrap_or_else(|| panic!("{}: no button on layer {depth}", id.as_str()))
                .rect;
            let mid = button.y + button.h / 2.0;
            // Half a device pixel inside the left edge, and twelve units
            // outside it on the panel.
            let edge = at(button.x + 0.5 / scale, mid);
            let panel = at(button.x - 12.0, mid);
            let ratio = contrast(edge, panel);
            assert!(
                ratio >= 2.9,
                "{}: the danger button on layer {depth} paints {edge:?} at its edge \
                 against the panel's {panel:?}, {ratio:.2}:1, under the 3:1 floor",
                id.as_str()
            );
        }
    }
}
