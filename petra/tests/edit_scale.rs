//! The editor model at scale (spec 006 SC-003, plan "Performance plan").
//!
//! Release builds only: a debug build measures the debug build. A
//! 100,000-line buffer takes random single-character edits across the file
//! through the whole command path (`EditorState::apply`, transaction,
//! history, edit records), and line lookups at random lines. One line of
//! `EDITOR_SCALE_BUFFER …` goes to stdout for the `editor-scale` lane.
#![cfg(not(debug_assertions))]

use std::time::Instant;

use gorgon_petra::edit::{EditCommand, EditorConfig, EditorState, Motion};

const LINES: usize = 100_000;
const EDITS: usize = 10_000;
/// Budget per keystroke for the model alone, at the 99th percentile. The
/// whole keystroke (model, reparse, highlight, frame) has 8 ms (SC-003);
/// the model must leave most of it to the rest.
const MODEL_P99_US: u128 = 500;

fn synthetic() -> String {
    let mut out = String::with_capacity(LINES * 40);
    for i in 0..LINES {
        match i % 5 {
            0 => out.push_str(&format!("fn item_{i}(x: u32) -> u32 {{\n")),
            1 => out.push_str("    // a comment with some words in it\n"),
            2 => out.push_str(&format!(
                "    let value = x * {i} + \"text\".len() as u32;\n"
            )),
            3 => out.push_str("    value\n"),
            _ => out.push_str("}\n"),
        }
    }
    out
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

#[test]
fn keystrokes_on_a_100k_line_buffer_stay_inside_the_model_budget() {
    let text = synthetic();
    let started = Instant::now();
    let mut state = EditorState::new(&text, EditorConfig::default());
    let load_ms = started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(state.buffer().len_lines(), LINES + 1);

    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut times = Vec::with_capacity(EDITS);
    for k in 0..EDITS {
        let line = (next() as usize) % LINES;
        let at = state.buffer().line_start(line) + 2;
        state.apply(EditCommand::PlaceCaret { at, extend: false }, 0);
        let command = match k % 4 {
            0 | 1 => EditCommand::Insert("x".into()),
            2 => EditCommand::Backspace,
            _ => EditCommand::Newline,
        };
        let t = Instant::now();
        state.apply(command, k as u64);
        let _ = state.take_records();
        times.push(t.elapsed().as_micros());
    }
    times.sort_unstable();
    let (p50, p99, max) = (
        percentile(&times, 0.5),
        percentile(&times, 0.99),
        *times.last().unwrap_or(&0),
    );

    let mut lookups = Vec::with_capacity(EDITS);
    for _ in 0..EDITS {
        let line = (next() as usize) % LINES;
        let t = Instant::now();
        let start = state.buffer().line_start(line);
        let back = state.buffer().line_of(start);
        lookups.push(t.elapsed().as_nanos());
        assert_eq!(back, line);
    }
    lookups.sort_unstable();

    let t = Instant::now();
    for _ in 0..1000 {
        state.apply(
            EditCommand::Move {
                motion: Motion::Down,
                extend: false,
            },
            0,
        );
    }
    let move_us = t.elapsed().as_micros() / 1000;

    println!(
        "EDITOR_SCALE_BUFFER lines={LINES} load_ms={load_ms:.1} keystroke_p50_us={p50} \
         keystroke_p99_us={p99} keystroke_max_us={max} lookup_p99_ns={} move_down_us={move_us}",
        percentile(&lookups, 0.99)
    );
    assert!(
        p99 <= MODEL_P99_US,
        "keystroke p99 {p99} µs is over the {MODEL_P99_US} µs model budget"
    );
}
