//! A runtime theme change re-resolves every surface in the next frame
//! (spec 005 FR-051, US7 scenario 3, task T120).
//!
//! Drives the shipped `Host` through all four themes in a ring
//! (White, Gray 10, Gray 90, Gray 100, back to White) with one surface the
//! application owns and one a fiber contributed. After the pass that takes
//! the request, the tessellated frame must carry the new theme's colour for
//! every token the tree binds, and no colour the old theme gave a token the
//! new theme changed. That second half is the "no stale colour survives"
//! clause: a surface that kept its old resolution would still pass a check
//! that only looked for the new colours somewhere on screen.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use egui::{Color32, Context, Pos2, RawInput, Rgba};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::semantic::ContributionId;
use gorgon_petra::token::{Theme, ThemeId, TokenName, TokenValue, shipped};
use gorgon_petra::tree::{NodeKind, Props, ViewNode};
use gorgon_petra_egui::host::{App, Contribution, Host, default_presenter};

const WINDOW: [f32; 2] = [640.0, 360.0];
const SLOT: &str = "shell.panel";

/// The tokens the application's own tree binds, slot by slot.
const APP_TOKENS: [&str; 4] = [
    "surface.base",
    "surface.layer-one",
    "text.primary",
    "border-strong",
];
/// The tokens the contributed tree binds.
const CONTRIBUTED_TOKENS: [&str; 2] = ["surface.layer-two", "text.muted"];

/// An application that asks for a theme when told to, and otherwise only
/// redraws what the host tells it to.
#[derive(Default)]
struct Shell {
    pending: Option<Theme>,
}

impl RowSource for Shell {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Shell {
    fn view(&mut self) -> ViewNode {
        let card = filled("card", "surface.layer-one")
            .with_border("border-strong")
            .child(text("title", "Theme", "text.primary"));
        filled("root", "surface.base")
            .child(card)
            .child(ViewNode::new(NodeKind::Stack, SLOT))
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        // The shell's own tree never changes: anything that moves on screen
        // after a theme request moved because the host re-resolved it.
        ChangeSet::None
    }

    fn theme_request(&mut self) -> Option<Theme> {
        self.pending.take()
    }
}

trait Bordered {
    fn with_border(self, token: &str) -> Self;
}

impl Bordered for ViewNode {
    fn with_border(mut self, token: &str) -> Self {
        self.props.tokens.insert("border".to_owned(), name(token));
        self
    }
}

fn name(token: &str) -> TokenName {
    TokenName::new(token).expect("a shipped token name")
}

fn filled(key: &str, background: &str) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key).with_props(Props {
        tokens: [("background".to_owned(), name(background))]
            .into_iter()
            .collect(),
        ..Props::default()
    })
}

fn text(key: &str, body: &str, foreground: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key).with_props(Props {
        text: Some(body.to_owned()),
        tokens: [("foreground".to_owned(), name(foreground))]
            .into_iter()
            .collect(),
        ..Props::default()
    })
}

fn contribution() -> Contribution {
    Contribution {
        id: ContributionId::new(7),
        revision: 1,
        slot: SLOT.to_owned(),
        tree: filled("surface", "surface.layer-two").child(text("note", "fiber", "text.muted")),
    }
}

fn sized() -> RawInput {
    RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(WINDOW[0], WINDOW[1]),
        )),
        ..RawInput::default()
    }
}

/// Run one pass and return every opaque vertex colour the frame tessellated
/// to: rect fills, strokes and glyph ink alike.
fn pass(ctx: &Context, host: &mut Host<Shell>) -> BTreeSet<[u8; 4]> {
    let owned = ctx.clone();
    let output = owned.run_ui(sized(), |_| host.pass(&owned));
    let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
    output.drop_without_applying_deltas();
    let mut colours = BTreeSet::new();
    for clipped in primitives {
        if let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive {
            for vertex in mesh.vertices {
                if vertex.color.a() == 255 {
                    colours.insert(vertex.color.to_array());
                }
            }
        }
    }
    colours
}

/// The colour `theme` resolves `token` to, as the painter converts it.
fn resolved(theme: &Theme, token: &str) -> [u8; 4] {
    match theme.value(&name(token)) {
        Some(TokenValue::Color(c)) => {
            Color32::from(Rgba::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)).to_array()
        }
        other => panic!("{token} is not a colour: {other:?}"),
    }
}

#[test]
fn a_theme_change_re_resolves_every_surface_in_the_next_frame() {
    let ctx = Context::default();
    ctx.run_ui(sized(), |_| {}).drop_without_applying_deltas();
    let mut host = Host::new(&ctx, Shell::default(), default_presenter());
    host.set_contributions(vec![contribution()]);

    let ring = [
        ThemeId::White,
        ThemeId::Gray10,
        ThemeId::Gray90,
        ThemeId::Gray100,
        ThemeId::White,
    ];
    host.app_mut().pending = Some(shipped(ring[0]));
    pass(&ctx, &mut host);

    for pair in ring.windows(2) {
        let (from, to) = (shipped(pair[0]), shipped(pair[1]));
        host.app_mut().pending = Some(to.clone());
        let colours = pass(&ctx, &mut host);

        assert_eq!(
            host.presenter().current().id(),
            Some(pair[1]),
            "the pass that took the request did not publish {}",
            pair[1].as_str()
        );
        for token in APP_TOKENS.iter().chain(&CONTRIBUTED_TOKENS) {
            let now = resolved(&to, token);
            assert!(
                colours.contains(&now),
                "{} -> {}: {token} should paint {now:?} in the first frame \
                 after the change; the frame carries {colours:?}",
                pair[0].as_str(),
                pair[1].as_str(),
            );
            let before = resolved(&from, token);
            let moved = before != now;
            let still_live = APP_TOKENS
                .iter()
                .chain(&CONTRIBUTED_TOKENS)
                .any(|t| resolved(&to, t) == before);
            assert!(
                !moved || still_live || !colours.contains(&before),
                "{} -> {}: {token}'s old colour {before:?} survived the change",
                pair[0].as_str(),
                pair[1].as_str(),
            );
        }
    }
}
