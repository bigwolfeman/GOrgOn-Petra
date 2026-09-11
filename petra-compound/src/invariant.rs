//! Package-owned invariant companion for `gorgon-petra-compound`.
//!
//! `gorgon-petra-compound` has a real runtime invariant, so this companion
//! asserts it instead of opting out: **every shipped [`crate::Compound`]'s
//! own view, built from its own `init`, carries no
//! [`gorgon_petra::tree::Violation`] that [`validate`] can judge without a
//! host [`Registry`]** — the same
//! [`gorgon_petra::tree::Violation::judgeable_standalone`] filter
//! `gorgon/gorgond/src/ui.rs`'s `registry_free_violations` runs on a
//! contributed subtree before the daemon ever answers it. A registry-scoped
//! question (an unregistered custom kind, an unregistered transition, an
//! unknown token) genuinely depends on host state this crate does not own
//! and a compound author cannot settle alone, so it is out of scope here on
//! purpose — checking against one hand-picked registry would make this
//! companion pass or fail on facts about `gorgond`, not about the compound.
//! A duplicate sibling key needs no registry at all.
//!
//! The five shipped compounds (`calendar`, `combobox`, `command`,
//! `data_table`, `selection_palette`) build their trees by composing
//! `gorgon_petra::component` kit constructors under hand-written keys — a
//! row id doubling as a node key, a column id doubling as a header key, an
//! option index doubling as `opt-{n}`. A copy-paste duplicate among those
//! keys is exactly the failure `petra-egui`'s own
//! `src/bin/gallery/page/pagination.rs` already documents: mounting the
//! same hardcoded inner key twice makes `Camera::click` panic, because
//! ambiguous hit-testing has no sibling left to pick between.
//!
//! Nothing in this crate's own test suite calls `validate` on a real
//! compound's tree today: every `update` test drives state transitions
//! only, and the `view` tests that do exist walk the tree for one named
//! node without ever accepting or refusing the whole tree. A duplicate key
//! introduced while composing a compound's `view` therefore compiles,
//! passes every existing test in this crate unnoticed, and only surfaces
//! the moment a driver clicks the tree or the daemon happens to be asked to
//! contribute it live — long after the crate that shipped it. This
//! companion closes that gap.

use gorgon_petra::tree::{Registry, ViewNode, validate};

use crate::calendar::{Mode, Props as CalendarProps};
use crate::combobox::Props as ComboboxProps;
use crate::command::{Item, Props as CommandProps};
use crate::data_table::{Column, Props as DataTableProps, Row};
use crate::selection_palette::Props as PaletteProps;
use crate::{Calendar, Combobox, Command, Compound, DataTable, SelectionPalette};

/// Assert every shipped compound's own initial view carries no
/// registry-free [`gorgon_petra::tree::Violation`].
///
/// Panics naming the compound and quoting every such violation `validate`
/// found — a duplicate key among them the one this companion exists for,
/// but any other context-free structural mistake `validate` catches
/// applies equally.
pub fn install() {
    check::<Calendar>("calendar", &calendar_props());
    check::<Combobox>("combobox", &combobox_props());
    check::<Command>("command", &command_props());
    check::<DataTable>("data_table", &data_table_props());
    check::<SelectionPalette>("selection_palette", &PaletteProps);
}

/// One compound's own `init` then `view`, checked for standalone violations.
///
/// `Registry::new()` declares no custom kinds, transitions or vocabulary on
/// purpose: every violation that would need one of those is filtered out by
/// [`gorgon_petra::tree::Violation::judgeable_standalone`] before this ever
/// looks at it, so which registry is passed cannot change the outcome.
fn check<C: Compound>(name: &str, props: &C::Props) {
    let state = C::init(props);
    let tree = C::view(&state, props);
    standalone_violations(&tree, name);
}

fn standalone_violations(tree: &ViewNode, name: &str) {
    let registry = Registry::new();
    let Err(errors) = validate(tree, &registry) else {
        return;
    };
    let standalone: Vec<String> = errors
        .as_slice()
        .iter()
        .filter(|err| err.violation.judgeable_standalone())
        .map(ToString::to_string)
        .collect();
    if !standalone.is_empty() {
        panic!(
            "gorgon-petra-compound: {name}'s own view tree carries {} registry-free \
             violation(s), most likely a hand-written duplicate key among its composed kit \
             constructors: {}; that sits here silently until a driver's `Camera::click` \
             panics on it (see `petra-egui/src/bin/gallery/page/pagination.rs`) or the daemon \
             refuses the live contribution",
            standalone.len(),
            standalone.join("; "),
        );
    }
}

/// Non-empty, non-default: `Mode::Multi` still runs
/// `date_picker_showing_selection`'s full grid, which is where the
/// hand-written day keys live.
fn calendar_props() -> CalendarProps {
    CalendarProps {
        label: "When".into(),
        mode: Mode::Multi,
    }
}

/// Three items, so `view` actually mounts the option rows whose keys
/// (`opt-{n}`) are the ones a copy-paste could collide on.
fn combobox_props() -> ComboboxProps {
    ComboboxProps {
        label: "Theme".into(),
        items: vec!["alpha".into(), "bravo".into(), "alpine".into()],
    }
}

fn command_props() -> CommandProps {
    CommandProps {
        items: vec![
            Item::new("open", "Open file"),
            Item::new("save", "Save"),
            Item::new("close", "Close window"),
        ],
    }
}

/// Three rows including one expandable body, so `view` mounts the row keys,
/// the header keys, and the expansion-body key together.
fn data_table_props() -> DataTableProps {
    DataTableProps {
        columns: vec![Column::new("name", "Name"), Column::new("status", "Status")],
        rows: vec![
            Row::new("r0", ["charlie", "ready"]),
            Row::new("r1", ["alpha", "busy"]).with_body("more"),
            Row::new("r2", ["bravo", "ready"]),
        ],
        batch_actions: Vec::new(),
        row_actions: Vec::new(),
        loading: false,
    }
}

#[cfg(test)]
mod tests {
    use gorgon_petra::tree::{NodeKind, Registry, ViewNode, validate};

    /// The five shipped compounds pass their own check.
    #[test]
    fn install_accepts_the_shipped_compounds() {
        super::install();
    }

    /// The same `validate` call this module relies on, driven by a
    /// hand-built duplicate key, so this fails the day `install` is loosened
    /// to "the tree exists" instead of "the tree carries no standalone
    /// violation".
    #[test]
    fn a_duplicate_sibling_key_is_refused_and_is_standalone() {
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "dup"))
            .child(ViewNode::new(NodeKind::Text, "dup"));
        let err = validate(&tree, &Registry::new()).unwrap_err();
        assert_eq!(err.as_slice().len(), 1);
        assert!(err.as_slice()[0].violation.judgeable_standalone());
        assert!(err.to_string().contains("must be unique"), "{err}");
    }
}
