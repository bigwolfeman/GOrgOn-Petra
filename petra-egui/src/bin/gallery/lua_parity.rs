//! The Lua/Rust tree-equality harness for the gallery's pages (spec 013,
//! T008/T008a).
//!
//! # Where this lives, and why here specifically
//!
//! `petra/` is a git submodule published on its own as `GOrgOn-Petra`; the
//! Lua VM (`gorgon-kernel-lua`) exists only in the private GOrgOn monorepo.
//! So this harness cannot live in `gorgon-kernel-lua` (it would need
//! `gorgon-petra`'s gallery pages, which it cannot see — `page` is
//! `mod page;` in the `gallery` **binary**, reachable from no other crate
//! regardless of visibility) and it cannot be a normal dependency of
//! `gorgon-petra-egui` (that would need `gorgon-kernel-lua` in production,
//! breaking `cargo xtask petra-boundary` in spirit and shipping a Lua VM
//! nothing in this crate's actual product needs). It lives here, as a
//! `#[cfg(test)]` module beside `shots`, with `gorgon-kernel-lua` added as a
//! monorepo-only **dev**-dependency — see that dependency's own comment in
//! `Cargo.toml` for the full reasoning and its precedent
//! (`petra/README.md`'s Provenance section: the same trade `gorgon-petra-
//! testkit`'s three removed `gorgon-inspector` integration tests made).
//!
//! # What SC-002 actually proves — read this before trusting a green run
//!
//! Every `ctx.ui.<name>` wrapper phases 1 and 2 of this spec registered
//! builds a `NodeKind::Component` wire node, not a literal tree
//! (`C.<name> = component("<name>")`). [`expand`] is what turns that
//! reference back into real primitives, by calling the exact same Rust
//! constructor the page's own `body()` calls directly
//! (`registry::expand` -> `build` -> `(entry.ctor)(params)`). So wherever
//! both sides of this test route through the registry, the assertion is
//! comparing `f(x)` to `f(x)`: it **cannot fail on drift**, because there is
//! only one implementation on that path, not two.
//!
//! What it still proves, and what nothing else in this tree proves:
//!
//! 1. **Reachability** — every constructor a page needs has a registry row
//!    and a `ctx.ui` wrapper; a missing one means the Lua side cannot name
//!    it at all.
//! 2. **Param fidelity** — every argument crosses the wire inside its param
//!    shape; a constructor taking something a wire value cannot carry (a
//!    `&PetrifiedFrame`, say) fails here by construction.
//! 3. **Transcription** — keys, nesting and child order match what the page
//!    actually built.
//! 4. **Real drift, in the one place it can occur** — `lua_parity/
//!    common.lua`'s `body`/`row`/`column`/`filled_body`, the hand-written
//!    mirror of `page/common.rs`'s four scaffolding builders. Those have no
//!    registry row (they are not `gorgon_petra::component` constructors,
//!    just raw `Grid`/`Stack` assembly private to the gallery binary), so
//!    they are the one genuine second implementation this test exercises —
//!    see that file's own module doc.
//!
//! This is an **expressibility gate**, not an implementation gate. A page
//! whose Lua source passes this test has been proven reachable, well-typed
//! and correctly transcribed — not proven correct on its own terms, because
//! for 56 of the 57 pages there are not two terms, there is one constructor
//! wearing two hats.
//!
//! # T010: the chrome, and what its test adds
//!
//! `lua_catalog_chrome_builds_the_same_tree_as_the_rust_one` extends the
//! same comparison one level up, from a page's `body()` to the chrome that
//! frames it: `catalog.rs`'s `Catalog::view()`. Its Lua side is
//! `lua_parity/catalog.lua`, the one file here that also transcribes
//! *derived* values — `inventory.rs`'s 57-row roster, the index labels, the
//! header strings — and it composes `common.lua` and the open page's own
//! T009-proven body rather than rebuilding either. What the test does NOT
//! cover is host-side behaviour — `seat_index_focus`, scrolling, event
//! routing — because no Lua tree contributes those, by construction.
//!
//! # Why a page is one `.lua` file under `lua_parity/pages/`, not one match arm
//!
//! [`every_lua_page_source_builds_the_same_tree_as_its_rust_page`] discovers
//! its pages by walking `lua_parity/pages/` at run time and matching each
//! `<slug>.lua` file's stem against `page::all()`'s `Page::row()`, slugged
//! the same way every existing `page/*.rs` module is already named
//! ([`slug`]'s doc explains the mapping). Nothing here is a compile-time
//! list, a `match`, or a table another leaf's page would need to add a row
//! to: landing row 31's proof after this one is one new file,
//! `lua_parity/pages/<slug>.lua`, and nothing else in this crate changes.
//! That is deliberate — T009 is 57 pages, several leaves will work it at
//! once, and a shared file every leaf must edit is a shared file every leaf
//! collides on.
//!
//! # Falsified, 2026-09-18
//!
//! With `lua_parity/pages/slider.lua`'s `value = "40"` changed to `"41"`
//! (one wire value, nothing else), the assertion fired. The real panic,
//! trimmed — `ViewNode`'s `Debug` has no diff view, so `assert_eq!` dumps
//! both whole trees rather than pointing at the one field that moved, and
//! that is worth knowing before trusting this across 57 pages:
//!
//! ```text
//! thread '...' panicked at petra/petra-egui/src/bin/gallery/lua_parity.rs:240:9:
//! assertion `left == right` failed: .../lua_parity/pages/slider.lua ("Slider") has \
//!   drifted from its Rust page's body()
//!   left: ViewNode { kind: Stack, key: Key("slide"), ... }
//!  right: ViewNode { kind: Stack, key: Key("slide"), ... }
//! ```
//! — several kilobytes of both trees follow. The outer message does name the
//! file and the inventory row, so a reader knows which page broke without
//! reading either dump; finding *where* inside the tree still means reading
//! (or `diff`ing) the two Debug blocks — confirmed here by grepping the
//! captured output for `text: Some("4` and getting exactly one `"40"` and
//! one `"41"`, the input's text and nothing else. Restored byte-identical
//! afterward (`diff` against a saved copy, empty), then re-verified green.
//! A future leaf that wants a pointed diff rather than two dumps should
//! reach for `pretty_assertions::assert_eq!` rather than accept this as
//! final; not done here because it is a dev-dependency this task did not
//! need to add to prove the harness works.

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};

use gorgon_kernel_lua::convert::lua_to_json;
use gorgon_kernel_lua::ui::install;
use gorgon_petra::component::registry::expand;
use gorgon_petra::geom::Point;
use gorgon_petra::input::{InputEvent, Modifiers, PointerButton, Route};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::ViewNode;
use gorgon_petra_egui::host::App;
use mlua::{Function, Lua, Table, Value};

use crate::catalog::{Catalog, THEME_DARK, THEME_LIGHT};
use crate::page;

/// Slug a `Page::row()` display name into the file stem its Lua proof and
/// its Rust module both use: lowercase, every run of non-alphanumeric
/// characters becomes one `_`, no leading or trailing `_`.
///
/// Checked against every one of the 57 `inventory.rs` rows before relying on
/// it: it reproduces `page/*.rs`'s own file names exactly, including the
/// non-mechanical ones — `"AI label"` -> `ai_label` (`ai_label.rs`),
/// `"Combobox (compound)"` -> `combobox_compound` (`combobox_compound.rs`).
/// That match is not a coincidence to preserve carefully; it is what lets a
/// leaf name a page's Lua proof after the page without inventing a second
/// naming scheme.
fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut at_boundary = true; // suppresses a leading `_`
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            at_boundary = false;
        } else if !at_boundary {
            out.push('_');
            at_boundary = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

fn lua_parity_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin/gallery/lua_parity")
}

/// A fresh VM with `ui` bound the way `ctx.ui` would be for a real plugin,
/// plus `common` bound to `lua_parity/common.lua`'s module table (T008a) —
/// the same shape `gorgon_kernel_lua::ui::mod.rs`'s own `vm()` sets up for
/// its single-node precedent, generalised with the one extra global every
/// page source here needs.
fn vm() -> Lua {
    let lua = Lua::new();
    let ui: Table = install(&lua).expect("gorgon-kernel-lua's builders.lua loads and evaluates");
    let common_path = lua_parity_dir().join("common.lua");
    let common_source = fs::read_to_string(&common_path)
        .unwrap_or_else(|e| panic!("{}: {e}", common_path.display()));
    let common_factory: Function = lua
        .load(&common_source)
        .set_name("gallery.lua_parity.common")
        .eval()
        .unwrap_or_else(|e| panic!("{}: {e}", common_path.display()));
    let common: Table = common_factory.call(ui.clone()).unwrap_or_else(|e| {
        panic!(
            "{}: common.lua's factory refused `ui`: {e}",
            common_path.display()
        )
    });
    lua.globals().set("ui", ui).expect("set the ui global");
    lua.globals()
        .set("common", common)
        .expect("set the common global");
    lua
}

/// Run `src`, convert its return value to the wire JSON `ctx.ui:contribute`
/// would send, deserialize it into a [`ViewNode`], and [`expand`] every
/// component reference it carries into the primitives its constructor
/// actually builds — the step the single-node precedent
/// (`a_lua_chrome_strip_is_the_same_tree_as_the_rust_one`) does not need,
/// because a hand-ported `chrome_strip`/`rule` never emits a component
/// reference in the first place. Every page source here does, because every
/// `ctx.ui.<component>` wrapper is `component("<name>")`.
fn build_expanded(lua: &Lua, src: &str, path: &Path) -> ViewNode {
    let value: Value = lua
        .load(src)
        .set_name(path.display().to_string())
        .eval()
        .unwrap_or_else(|e| panic!("{}: {e}\n-- source --\n{src}", path.display()));
    let json = lua_to_json(&value).unwrap_or_else(|e| {
        panic!(
            "{}: this page's return value does not convert to json: {e}",
            path.display()
        )
    });
    let node: ViewNode = serde_json::from_value(json.clone())
        .unwrap_or_else(|e| panic!("{}: {e}\n-- json was --\n{json}", path.display()));
    expand(&node).unwrap_or_else(|e| {
        panic!(
            "{}: registry::expand refused this tree for `{}`: {}",
            path.display(),
            e.component,
            e.reason
        )
    })
}

/// Put `gorgon-petra-compound`'s five rows in `gorgon_petra`'s merged
/// component table, so a page that names one expands instead of failing by
/// name (spec 014 A3).
///
/// `gorgond`'s `boot::run` and `gorgon-inspector`'s `main` each make this
/// call as their first act, for the same reason. This test process makes
/// none of them, so it has to make its own: registration is per-process and
/// the merged table is a `OnceLock`.
///
/// **Call this before the first thing in this binary that touches the
/// registry — `expand`, `build`, `lookup`, `entries` or `shape_of`.**
/// Whichever of those runs first freezes the table, and `register_external`
/// panics rather than silently dropping a slice that arrives after
/// (`petra/petra/src/component/registry/mod.rs`'s own doc). Today
/// [`build_expanded`]'s `expand` is the only registry call in the `gallery`
/// binary, and the walk below is its only caller, so one call at the top of
/// that walk is enough. A test added here later that reaches the registry
/// by another road has to call this too — `gorgon_petra_compound::registry
/// ::register` is idempotent, so doing it defensively costs a `Once` check.
///
/// Falsified 2026-09-19 by deleting the call from the walk below, which is
/// the state this file shipped in while spec 013's five compound pages were
/// blocked:
///
/// ```text
/// thread 'lua_parity::every_lua_page_source_builds_the_same_tree_as_its_rust_page'
/// panicked at petra/petra-egui/src/bin/gallery/lua_parity.rs:196:9:
/// .../lua_parity/pages/calendar_compound.lua: registry::expand refused this tree for
/// `calendar_compound`: no component named `calendar_compound`; see docs/catalogs/components.md
/// ```
///
/// Restored byte-identical afterwards and re-run green.
fn register_compound_rows() {
    gorgon_petra_compound::registry::register();
}

/// Walk `lua_parity/pages/`, and for every `<slug>.lua` file there, build it
/// through Lua and compare it against the matching `page::all()` entry's own
/// `body()`. See this module's own doc for what a green result here does and
/// does not prove, and [`slug`]'s doc for the file-name mapping.
///
/// One page's file, one page's proof: adding row 31's file here never
/// touches this function or any other page's file, which is what makes
/// T009's 56 remaining pages splittable across parallel leaves.
#[test]
fn every_lua_page_source_builds_the_same_tree_as_its_rust_page() {
    register_compound_rows();
    let pages_dir = lua_parity_dir().join("pages");
    let mut entries: Vec<PathBuf> = fs::read_dir(&pages_dir)
        .unwrap_or_else(|e| panic!("{}: {e}", pages_dir.display()))
        .map(|entry| entry.expect("a directory entry reads").path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "lua"))
        .collect();
    entries.sort();
    assert!(
        !entries.is_empty(),
        "{} has no .lua page sources; T008/T008a landed with none proven",
        pages_dir.display()
    );

    let pages = page::all();
    let lua = vm();
    let mut checked = Vec::new();
    for path in entries {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("{}: file stem is not valid UTF-8", path.display()))
            .to_owned();
        let page = pages
            .iter()
            .find(|p| slug(p.row()) == stem)
            .unwrap_or_else(|| {
                panic!(
                    "{} names no inventory row: slug {stem:?} matches no Page::row() in \
                 page::all(). Every page/*.rs module's file name already reproduces \
                 this slug — check the spelling against inventory.rs's ROWS.",
                    path.display()
                )
            });

        let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let from_lua = build_expanded(&lua, &src, &path);
        let from_rust = page.body();
        if from_lua != from_rust {
            panic!(
                "{} (\"{}\") has drifted from its Rust page's body()\n{}",
                path.display(),
                page.row(),
                difference(&from_lua, &from_rust)
            );
        }
        checked.push(page.row());
    }
    eprintln!(
        "lua_parity: {} of {} inventory rows proven equal to their Rust body(): {checked:?}",
        checked.len(),
        pages.len()
    );
}

/// The wire spelling [`ThemeMode`] takes in `catalog.lua`'s `state.theme`.
/// One place, so the test cases and the seeded global cannot drift apart.
fn theme_wire(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
    }
}

/// Reach the theme the way the driver does: a press on the chrome's own
/// switcher segment (`catalog.rs`'s `route_event` chain), taken by the host
/// on the next pass (`App::theme_request`). Deterministic whatever
/// `initial_theme_mode()` read from the environment, because
/// `Catalog::set_theme` only no-ops when the mode is already the one asked
/// for.
fn press_theme_segment(catalog: &mut Catalog, mode: ThemeMode) {
    let id = match mode {
        ThemeMode::Dark => THEME_DARK,
        ThemeMode::Light => THEME_LIGHT,
    };
    catalog.handle(
        &InputEvent::PointerPressed {
            pos: Point::ZERO,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        },
        &Route::Pointer {
            node: format!("/page/root/{id}"),
        },
        None,
    );
    let _ = catalog.theme_request();
}

/// T010's proof: the Lua chrome mirror (`lua_parity/catalog.lua`) builds the
/// same tree `catalog.rs`'s [`Catalog::view`] builds — roster, index pane,
/// page header, Prev/Next and the theme switcher, around the open page's own
/// body — for the open pages and themes the driver can actually land on.
///
/// The state is seeded through `catalog.lua`'s `state` global (its module
/// doc): the open row by name, the theme, and the body as
/// `pages/<slug>.lua` itself builds it (already proven equal to `body()` by
/// the test above), so this comparison is whole-`view()` against whole-`view()`
/// with nothing stubbed on either side.
///
/// What a green run here does and does not prove is the same shape as the
/// page walk's — reachability, param fidelity, transcription, plus real
/// drift in the hand-typed raw nodes (`common.lua`, `catalog.lua`) — and it
/// explicitly does NOT prove `seat_index_focus`, scrolling or event
/// routing: those are host-side (`Host`, `route`, the focus tree) and no Lua
/// tree contributes them.
///
/// Falsified 2026-09-26 by changing `catalog.lua`'s `index` scroll's
/// `overscan` from 64.0 to 32.0 and restoring it byte-identical afterwards
/// (`diff` against a saved copy, empty, then re-verified green). The real
/// panic, pointing at the field, both values, and the case that caught it:
///
/// ```text
/// thread 'lua_parity::lua_catalog_chrome_builds_the_same_tree_as_the_rust_one'
/// panicked at petra/petra-egui/src/bin/gallery/lua_parity.rs:417:13:
/// .../lua_parity/catalog.lua ("Toggle", dark) has drifted from catalog.rs's view()
///   at children[0].children[0].props.overscan
///     lua:  32.0
///     rust: 64.0
/// ```
#[test]
fn lua_catalog_chrome_builds_the_same_tree_as_the_rust_one() {
    register_compound_rows();
    let path = lua_parity_dir().join("catalog.lua");
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let cases = [
        ("Toggle", ThemeMode::Dark),
        ("Toggle", ThemeMode::Light),
        ("Modal", ThemeMode::Dark),
        ("Avatar", ThemeMode::Light),
    ];
    for (component, mode) in cases {
        // A fresh VM per case: `state` is a global, and a leaked one from
        // the previous case would silently answer the next.
        let lua = vm();
        let body_path = lua_parity_dir()
            .join("pages")
            .join(format!("{}.lua", slug(component)));
        let body_src = fs::read_to_string(&body_path)
            .unwrap_or_else(|e| panic!("{}: {e}", body_path.display()));
        let body: Value = lua
            .load(&body_src)
            .set_name(body_path.display().to_string())
            .eval()
            .unwrap_or_else(|e| panic!("{}: {e}", body_path.display()));
        let state: Table = lua.create_table().expect("a state table");
        state.set("open", component).expect("set state.open");
        state
            .set("theme", theme_wire(mode))
            .expect("set state.theme");
        state.set("body", body).expect("set state.body");
        lua.globals()
            .set("state", state)
            .expect("set the state global");
        let from_lua = build_expanded(&lua, &src, &path);

        let mut catalog = Catalog::on_page(component);
        press_theme_segment(&mut catalog, mode);
        let from_rust = catalog.view();
        if from_lua != from_rust {
            panic!(
                "{} (\"{}\", {}) has drifted from catalog.rs's view()\n{}",
                path.display(),
                component,
                theme_wire(mode),
                difference(&from_lua, &from_rust)
            );
        }
    }
}

/// Name the FIRST place two page trees differ, as a path and a pair of
/// values, instead of printing both trees whole.
///
/// `assert_eq!` on a `ViewNode` dumps two multi-kilobyte `Debug` renderings
/// and leaves the reader to spot the one field that moved. That is tolerable
/// for one page and not for fifty-seven: spec 013 T009 hands the remaining
/// pages to several agents at once, and each one has to be able to read its
/// own failure without grepping a wall of text.
///
/// The walk is over `serde_json::Value` rather than over `ViewNode`'s fields
/// by hand, for one reason: `Props` alone has 34 fields and `Semantics` 14,
/// and a hand-written comparison would silently stop covering whichever
/// field someone adds next. Serializing both sides means every field present
/// on the wire is compared, forever, with no list to maintain. `ViewNode`
/// skips empty fields when it serializes, so a field set on one side and
/// absent on the other shows up here as a key only one object has, which is
/// reported as such rather than as a value mismatch.
///
/// Falsified 2026-09-18 against `slider.lua`, both branches, each restored
/// byte-identical afterwards. A changed scalar:
///
/// ```text
/// .../lua_parity/pages/slider.lua ("Slider") has drifted from its Rust page's body()
///   at children[1].children[0].children[1].children[3].props.text
///     lua:  "41"
///     rust: "40"
/// ```
///
/// and a child the Rust side does not have:
///
/// ```text
///   at children[1].children
///     lua:  2 element(s)
///     rust: 1 element(s)
/// ```
///
/// Both name the file, the inventory row, the path and the two values. That
/// is the bar: a leaf porting one page has to read its own failure without
/// grepping a wall of `Debug` output.
fn difference(lua: &ViewNode, rust: &ViewNode) -> String {
    let (a, b) = match (serde_json::to_value(lua), serde_json::to_value(rust)) {
        (Ok(a), Ok(b)) => (a, b),
        _ => return "  (both trees failed to serialize, so no path could be named)".to_owned(),
    };
    match walk(&a, &b, String::new()) {
        Some(found) => found,
        // Unreachable through the caller above, which only calls this after
        // `!=`. Kept as a real answer rather than an `unreachable!` because
        // a `ViewNode` that compares unequal while serializing identically
        // would be a genuine bug in this tree's serde, and printing that
        // sentence is more useful to whoever hits it than a panic inside the
        // panic path.
        None => "  the trees compare unequal but serialize identically — \
                 suspect a field that `Serialize` skips"
            .to_owned(),
    }
}

/// One step of [`difference`]'s walk. `at` is the path so far, in the
/// spelling a reader can follow back into the Lua source.
fn walk(a: &serde_json::Value, b: &serde_json::Value, at: String) -> Option<String> {
    use serde_json::Value;
    let here = if at.is_empty() {
        "<root>".to_owned()
    } else {
        at.clone()
    };
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort_unstable();
            keys.dedup();
            for key in keys {
                let path = if at.is_empty() {
                    key.clone()
                } else {
                    format!("{at}.{key}")
                };
                match (x.get(key), y.get(key)) {
                    (Some(l), Some(r)) => {
                        if let Some(found) = walk(l, r, path) {
                            return Some(found);
                        }
                    }
                    (Some(l), None) => {
                        return Some(format!("  at {path}\n    lua:  {l}\n    rust: (absent)"));
                    }
                    (None, Some(r)) => {
                        return Some(format!("  at {path}\n    lua:  (absent)\n    rust: {r}"));
                    }
                    (None, None) => {}
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!(
                    "  at {here}\n    lua:  {} element(s)\n    rust: {} element(s)",
                    x.len(),
                    y.len()
                ));
            }
            for (i, (l, r)) in x.iter().zip(y).enumerate() {
                if let Some(found) = walk(l, r, format!("{at}[{i}]")) {
                    return Some(found);
                }
            }
            None
        }
        _ if a == b => None,
        _ => Some(format!("  at {here}\n    lua:  {a}\n    rust: {b}")),
    }
}

#[cfg(test)]
mod tests {
    use super::slug;

    /// [`slug`] must reproduce every existing `page/*.rs` file stem exactly,
    /// including the three shapes that are not a mechanical lowercase-and-
    /// underscore of the display name: a parenthesised suffix, a run of two
    /// spaces' worth of punctuation, and a name that is already one word.
    /// Falsified by keeping a trailing separator: with the trim loop
    /// removed, `"Slider".to_string()` still passes, but `"AI label "`
    /// (trailing space) — no row has one, but the case it stands in for is
    /// real — would come out `ai_label_`, not `ai_label`.
    #[test]
    fn slug_reproduces_every_page_module_file_name() {
        let cases = [
            ("Accordion", "accordion"),
            ("AI label", "ai_label"),
            ("Code snippet", "code_snippet"),
            ("Data table", "data_table"),
            ("UI shell left panel", "ui_shell_left_panel"),
            ("Combobox (compound)", "combobox_compound"),
            ("Selection palette (compound)", "selection_palette_compound"),
            ("Slider", "slider"),
        ];
        for (name, expected) in cases {
            assert_eq!(slug(name), expected, "slug({name:?})");
        }
    }
}
