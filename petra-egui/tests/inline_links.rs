//! A link inside a paragraph, driven through the real host with real faces:
//! Tab lands on it, Enter and a click on its words fire its intent under its
//! own id, and a click beside it inside the same paragraph fires nothing.
//!
//! The engine's half (fragments, order, hit test, semantics) is
//! `petra/tests/inline_links.rs` against fixed-width metrics. This file is the
//! half that needs a shaper: the fragments the host reports are where the
//! glyphs are painted. Set `PETRA_SHOT_DIR` to write the focused paragraph.

use std::ops::Range;
use std::sync::Arc;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect as EguiRect, vec2};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    Intent, Key as NodeKey, NodeKind, Props, RunLink, TextRun, TextWrap, ViewNode,
};
use gorgon_petra_egui::host::{App, FiredIntent, Host};
use gorgon_petra_egui::testing::{boot, step};

const WINDOW: [f32; 2] = [360.0, 200.0];
const PROSE: &str = "Read the specification before you open the plan.";

#[derive(Default)]
struct Page {
    fired: Vec<FiredIntent>,
}

impl RowSource for Page {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

fn link_run(len: usize, key: &str) -> TextRun {
    TextRun {
        len,
        underline: true,
        foreground: Some(TokenName::new("link-primary").expect("token")),
        link: Some(RunLink {
            key: NodeKey::new(key),
            target: Some(format!("{key}.md")),
        }),
        ..TextRun::default()
    }
}

impl App for Page {
    fn view(&mut self) -> ViewNode {
        let spec = PROSE.find("specification").expect("word");
        let plan = PROSE.find("plan").expect("word");
        let paragraph = ViewNode::new(NodeKind::Text, "p").with_props(Props {
            text: Some(PROSE.to_owned()),
            wrap: Some(TextWrap::Wrap),
            runs: vec![
                TextRun {
                    len: spec,
                    ..TextRun::default()
                },
                link_run("specification".len(), "spec"),
                TextRun {
                    len: plan - spec - "specification".len(),
                    ..TextRun::default()
                },
                link_run("plan".len(), "plan"),
                TextRun {
                    len: PROSE.len() - plan - "plan".len(),
                    ..TextRun::default()
                },
            ],
            ..Props::default()
        });
        let mut page = ViewNode::new(NodeKind::Stack, "root").child(paragraph);
        page.props.tokens.insert(
            "background".into(),
            TokenName::new("surface.base").expect("token"),
        );
        gorgon_petra::component::padded(
            page,
            gorgon_petra::tree::InsetRefs::all(TokenName::new("spacing-05").expect("token")),
        )
    }

    fn handle(&mut self, _: &InputEvent, _: &Route, _: Option<&PetrifiedFrame>) {}

    fn intent(&mut self, fired: &FiredIntent) {
        self.fired.push(fired.clone());
    }

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

fn key(key: Key) -> RawInput {
    let mut input = RawInput::default();
    for pressed in [true, false] {
        input.events.push(Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
    }
    sized(input)
}

fn click(at: Pos2) -> Vec<RawInput> {
    let press = |pressed| {
        let mut input = RawInput::default();
        input.events.push(Event::PointerMoved(at));
        input.events.push(Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        sized(input)
    };
    vec![press(true), press(false)]
}

fn settle(ctx: &egui::Context, host: &mut Host<Page>) {
    for _ in 0..30 {
        step(ctx, host, sized(RawInput::default()));
    }
}

fn link_rect(host: &Host<Page>, id: &str) -> (EguiRect, Vec<EguiRect>) {
    let frame = host.frame().expect("a frame");
    let p = frame
        .placement(id)
        .unwrap_or_else(|| panic!("{id} is placed"));
    let r = |x: f32, y: f32, w: f32, h: f32| EguiRect::from_min_size(Pos2::new(x, y), vec2(w, h));
    let fragments = p
        .semantics
        .fragments
        .iter()
        .map(|f| r(p.rect.x + f.x, p.rect.y + f.y, f.w, f.h))
        .collect();
    (r(p.rect.x, p.rect.y, p.rect.w, p.rect.h), fragments)
}

#[test]
fn tab_enter_and_a_click_reach_a_link_inside_a_paragraph() {
    let (ctx, mut host) = boot(Page::default());
    host.set_reduced_motion(true);
    settle(&ctx, &mut host);

    // The host seats focus on the first focusable node on its own, so the
    // first link is reached with no Tab or with one, never more.
    for _ in 0..2 {
        if host.state().focused.as_deref() == Some("/root/p/spec") {
            break;
        }
        step(&ctx, &mut host, key(Key::Tab));
        settle(&ctx, &mut host);
    }
    assert_eq!(
        host.state().focused.as_deref(),
        Some("/root/p/spec"),
        "the first link is the first stop"
    );
    if let Some(dir) = std::env::var_os("PETRA_SHOT_DIR") {
        let output = ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx));
        let frame = host.frame().expect("a frame");
        let shot = gorgon_petra_testkit::snapshot::Snapshotter::new()
            .capture(&ctx, &output, frame, None)
            .expect("capture");
        output.drop_without_applying_deltas();
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("inline-link-focused.png"), &shot.png).expect("png");
    }

    step(&ctx, &mut host, key(Key::Enter));
    settle(&ctx, &mut host);
    assert!(
        host.app()
            .fired
            .iter()
            .any(|f| f.node == "/root/p/spec" && f.intent == Intent::Activate),
        "Enter activates the focused link: {:?}",
        host.app().fired
    );

    step(&ctx, &mut host, key(Key::Tab));
    settle(&ctx, &mut host);
    assert_eq!(host.state().focused.as_deref(), Some("/root/p/plan"));

    let (_, fragments) = link_rect(&host, "/root/p/plan");
    let words = fragments.first().copied().expect("the plan is drawn");
    host.app_mut().fired.clear();
    for input in click(words.center()) {
        step(&ctx, &mut host, input);
    }
    settle(&ctx, &mut host);
    assert!(
        host.app().fired.iter().any(|f| f.node == "/root/p/plan"),
        "a click on the words activates that link: {:?}",
        host.app().fired
    );

    // Beside the link, on the paragraph's own words: nothing fires.
    let text = host.frame().unwrap().placement("/root/p").unwrap().rect;
    host.app_mut().fired.clear();
    for input in click(Pos2::new(text.x + 4.0, text.y + 4.0)) {
        step(&ctx, &mut host, input);
    }
    settle(&ctx, &mut host);
    assert!(
        host.app().fired.is_empty(),
        "a click on plain words fires no link: {:?}",
        host.app().fired
    );
}
