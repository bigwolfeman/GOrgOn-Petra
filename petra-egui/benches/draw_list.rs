//! What one canvas costs at the draw-list bounds (`contracts/draw-list.md`
//! §5, spec 005 task T169).
//!
//! `MAX_DRAW_COMMANDS = 4096` and `MAX_DRAW_PATH_SEGMENTS = 16384` were reasoned,
//! not measured. A bound is right when the most expensive list it admits
//! still fits a frame with room for everything else on screen, and wrong in
//! the other direction when it refuses lists that would have been cheap. So
//! this measures the whole per-frame path a canvas pays, at and around each
//! bound, and nothing else:
//!
//! 1. `DrawList::new` — validation: the counts, `Push`/`Pop` balance and the
//!    convexity test on filled closed paths. Paid once per submitted list.
//! 2. `hash_draw_list` — the canvas's share of the frame digest. Paid once per
//!    submitted list.
//! 3. `paint_canvas` plus `egui`'s tessellation of the shapes it emitted —
//!    paid every frame the canvas is painted.
//!
//! Each list is the expensive end of what its bound admits: filled and
//! stroked rects for the command bound; for the verb bound, a stroked
//! polyline, stroked cubics (each flattened into many points), and one
//! filled convex polygon (the convexity test and the fan fill at once).
//!
//! Run with `cargo bench -p gorgon-petra-egui --bench draw_list`. The numbers
//! are machine-specific; `contracts/draw-list.md` §5 records the run it
//! quotes and the machine it ran on.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use egui::Color32;
use gorgon_petra::draw::{
    ColorRef, Command, Corners, DrawList, MAX_DRAW_COMMANDS, MAX_DRAW_PATH_SEGMENTS, Paint,
    PathVerb, Stroke, Width,
};
use gorgon_petra::frame::digest::hash_draw_list;
use gorgon_petra::geom::{Point, Rect, Scale};
use gorgon_petra::token::DerivedState;
use gorgon_petra_egui::draw::{Ink, NoAssets, paint_canvas};
use gorgon_petra_egui::paint::TokenSource;

/// One 60 Hz frame.
const FRAME: Duration = Duration::from_micros(16_667);
const REPS: usize = 15;
const CANVAS: f32 = 1024.0;

struct Colours;

impl TokenSource for Colours {
    fn color(&self, token: &str) -> Option<Color32> {
        match token {
            "fill" => Some(Color32::from_rgb(40, 40, 48)),
            "line" => Some(Color32::from_rgb(220, 220, 228)),
            _ => None,
        }
    }
}

fn line(width: f32) -> Stroke {
    Stroke {
        width: Width::Logical(width),
        color: ColorRef::Token("line".to_owned()),
    }
}

fn rects(n: usize) -> Vec<Command> {
    let side = (n as f32).sqrt().ceil();
    let cell = CANVAS / side;
    (0..n)
        .map(|i| {
            let (x, y) = ((i as f32 % side) * cell, (i as f32 / side).floor() * cell);
            Command::Rect {
                rect: Rect::new(x, y, cell * 0.8, cell * 0.8),
                radius: Corners::SQUARE,
                snap: false,
                paint: Paint {
                    fill: Some(ColorRef::Token("fill".to_owned())),
                    stroke: Some(line(1.0)),
                },
            }
        })
        .collect()
}

fn polyline(verbs: usize) -> Vec<Command> {
    let mut path = vec![PathVerb::MoveTo(Point::new(0.0, CANVAS / 2.0))];
    for i in 1..verbs {
        let x = i as f32 * CANVAS / verbs as f32;
        let y = CANVAS / 2.0 + (i as f32 * 0.37).sin() * CANVAS / 3.0;
        path.push(PathVerb::LineTo(Point::new(x, y)));
    }
    vec![Command::Path {
        verbs: path,
        closed: false,
        paint: Paint::stroked(line(1.5)),
    }]
}

fn cubics(verbs: usize) -> Vec<Command> {
    let mut path = vec![PathVerb::MoveTo(Point::new(0.0, CANVAS / 2.0))];
    let step = CANVAS / verbs as f32;
    for i in 1..verbs {
        let x = i as f32 * step;
        path.push(PathVerb::CubicTo {
            c1: Point::new(x - step * 0.66, 0.0),
            c2: Point::new(x - step * 0.33, CANVAS),
            to: Point::new(x, CANVAS / 2.0),
        });
    }
    vec![Command::Path {
        verbs: path,
        closed: false,
        paint: Paint::stroked(line(1.5)),
    }]
}

fn convex(verbs: usize) -> Vec<Command> {
    let centre = CANVAS / 2.0;
    let at = |i: usize| {
        let a = i as f32 / verbs as f32 * std::f32::consts::TAU;
        Point::new(
            centre + a.cos() * centre * 0.9,
            centre + a.sin() * centre * 0.9,
        )
    };
    let mut path = vec![PathVerb::MoveTo(at(0))];
    path.extend((1..verbs).map(|i| PathVerb::LineTo(at(i))));
    vec![Command::Path {
        verbs: path,
        closed: true,
        paint: Paint {
            fill: Some(ColorRef::Token("fill".to_owned())),
            stroke: Some(line(1.0)),
        },
    }]
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

struct Cost {
    build: Duration,
    hash: Duration,
    paint: Duration,
    tessellate: Duration,
    vertices: usize,
}

fn measure(commands: &[Command]) -> Result<Cost, String> {
    let (mut build, mut hash, mut paint, mut tessellate) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut vertices = 0;
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |_| {})
        .drop_without_applying_deltas();
    let colours = Colours;
    let bindings = BTreeMap::new();
    for _ in 0..REPS {
        let t = Instant::now();
        let list = DrawList::new(commands.to_vec()).map_err(|err| err.to_string())?;
        build.push(t.elapsed());

        let t = Instant::now();
        std::hint::black_box(hash_draw_list(&list));
        hash.push(t.elapsed());

        let mut painted = Duration::ZERO;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            let t = Instant::now();
            let report = paint_canvas(
                &painter,
                &list,
                Rect::new(0.0, 0.0, CANVAS, CANVAS),
                Scale::new(1.0).unwrap(),
                Ink::new(&colours, &bindings, DerivedState::default()),
                &mut NoAssets,
            );
            painted = t.elapsed();
            assert!(report.shapes > 0, "the canvas drew nothing");
        });
        paint.push(painted);
        let t = Instant::now();
        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        tessellate.push(t.elapsed());
        output.drop_without_applying_deltas();
        vertices = primitives
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
                egui::epaint::Primitive::Callback(_) => 0,
            })
            .sum();
    }
    Ok(Cost {
        build: median(build),
        hash: median(hash),
        paint: median(paint),
        tessellate: median(tessellate),
        vertices,
    })
}

fn row(label: &str, n: usize, cost: &Result<Cost, String>) {
    let cost = match cost {
        Ok(cost) => cost,
        Err(err) => {
            println!("| {label} | {n} | refused: {err} | | | | | | |");
            return;
        }
    };
    let per_frame = cost.paint + cost.tessellate;
    let once = cost.build + cost.hash;
    println!(
        "| {label} | {n} | {:.0} | {:.0} | {:.0} | {:.0} | {:.1}% | {:.0} | {} |",
        cost.build.as_secs_f64() * 1e6,
        cost.hash.as_secs_f64() * 1e6,
        cost.paint.as_secs_f64() * 1e6,
        cost.tessellate.as_secs_f64() * 1e6,
        per_frame.as_secs_f64() / FRAME.as_secs_f64() * 100.0,
        once.as_secs_f64() * 1e6,
        cost.vertices,
    );
}

fn main() {
    println!(
        "bounds: MAX_DRAW_COMMANDS = {MAX_DRAW_COMMANDS}, MAX_DRAW_PATH_SEGMENTS = {MAX_DRAW_PATH_SEGMENTS}; \
         median of {REPS}; times in µs"
    );
    println!(
        "| list | n | build | hash | paint | tessellate | paint+tess / 16.7 ms | build+hash | vertices |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for n in [256, 1024, MAX_DRAW_COMMANDS] {
        row("filled+stroked rects", n, &measure(&rects(n)));
    }
    for (label, build) in [
        ("stroked polyline", polyline as fn(usize) -> Vec<Command>),
        ("stroked cubics", cubics),
        ("filled convex polygon", convex),
    ] {
        for n in [1024, 4096, MAX_DRAW_PATH_SEGMENTS] {
            row(label, n, &measure(&build(n)));
        }
    }
    // The most of the bench's long cubics the segment bound admits: the
    // expensive end of what the bound lets a curve-heavy list do.
    let (mut fits, mut over) = (2, MAX_DRAW_PATH_SEGMENTS);
    while over - fits > 1 {
        let mid = (fits + over) / 2;
        if DrawList::new(cubics(mid)).is_ok() {
            fits = mid;
        } else {
            over = mid;
        }
    }
    let admitted = fits;
    row(
        "stroked cubics, most admitted",
        admitted,
        &measure(&cubics(admitted)),
    );
}
