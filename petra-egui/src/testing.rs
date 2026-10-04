//! Test boot support: one home for the `headless` → `Host::new` → first-frame
//! procedure that `src/host.rs`'s and `src/bin/gallery/catalog.rs`'s test mods
//! used to copy at every boot site.
//!
//! The two test mods sit in different build targets (the lib's own tests and
//! the `gallery` bin's tests), so a `#[cfg(test)]` module reaches one and not
//! the other; this is a plain `pub mod` each test mod imports — `crate::testing`
//! from the lib's, `gorgon_petra_egui::testing` from the bin's. The `_in` spine
//! takes the environment (context, first input) because the catalog pins its
//! window (`WINDOW` = 1200×900) and enters with a one-line sizing; the
//! conveniences without `_in` are this crate's default environment.
//!
//! The helpers return the two **owned** handles — `(Context, Host<A>)` — and
//! hold no fixtures: [`Host::new`] clones its `&Context`, so nothing outlives
//! the call. Call-site setup and every assertion stay at the call site.
//! Lifetime reasoning and the fold record: DD9,
//! `.agents/notes/implemented/simplification/2026-10-03-dd9-test-boot-triple.md`
//! (GOrgOn repo root).

use std::time::Duration;

use egui::{Context, RawInput};
use gorgon_petra::token::Presenter;

use crate::host::{App, Host};

/// A context with no frame behind it and no pending deltas in it: one empty
/// pass, deltas dropped, so a host's first pass is not also egui's.
#[must_use]
pub fn headless() -> Context {
    headless_with(RawInput::default())
}

/// [`headless`] over a caller-supplied first input — where an environment that
/// sizes its inputs (the catalog's `sized`) enters.
#[must_use]
pub fn headless_with(init: RawInput) -> Context {
    let ctx = Context::default();
    ctx.run_ui(init, |_| {}).drop_without_applying_deltas();
    ctx
}

/// `Host::new` over a context the caller built, and no pass — a site that
/// asserts on a brand-new host must see its own setup and nothing a frame
/// would add.
#[must_use]
pub fn construct_in<A: App>(ctx: Context, app: A, presenter: Presenter) -> (Context, Host<A>) {
    let host = Host::new(&ctx, app, presenter);
    (ctx, host)
}

/// [`construct_in`] over [`headless`]: construct, no pass.
#[must_use]
pub fn construct_with<A: App>(app: A, presenter: Presenter) -> (Context, Host<A>) {
    construct_in(headless(), app, presenter)
}

/// [`construct_with`] over [`crate::host::default_presenter`].
#[must_use]
pub fn construct<A: App>(app: A) -> (Context, Host<A>) {
    construct_with(app, crate::host::default_presenter())
}

/// [`construct_in`] plus one first frame over `first`: the boot triple, for
/// the sites that boot *for* that frame.
#[must_use]
pub fn boot_in<A: App>(
    ctx: Context,
    app: A,
    presenter: Presenter,
    first: RawInput,
) -> (Context, Host<A>) {
    let (ctx, mut host) = construct_in(ctx, app, presenter);
    step(&ctx, &mut host, first);
    (ctx, host)
}

/// [`boot_in`] over [`headless`] and an empty first input.
#[must_use]
pub fn boot_with<A: App>(app: A, presenter: Presenter) -> (Context, Host<A>) {
    boot_in(headless(), app, presenter, RawInput::default())
}

/// [`boot_with`] over [`crate::host::default_presenter`].
#[must_use]
pub fn boot<A: App>(app: A) -> (Context, Host<A>) {
    boot_with(app, crate::host::default_presenter())
}

/// One frame, the way `eframe` drives it: the host's pass runs *inside* an
/// egui pass, because that is the only place a layer painter's shapes are
/// collected and a repaint request is observed.
///
/// The `Duration` is the smallest repaint delay that pass asked for
/// (`Duration::MAX` when it asked for none) — what settle-style loops read.
pub fn step<A: App>(ctx: &Context, host: &mut Host<A>, input: RawInput) -> Duration {
    let out = ctx.run_ui(input, |_| host.pass(ctx));
    let delay = out
        .viewport_output
        .values()
        .map(|v| v.repaint_delay)
        .min()
        .unwrap_or(Duration::MAX);
    out.drop_without_applying_deltas();
    delay
}
