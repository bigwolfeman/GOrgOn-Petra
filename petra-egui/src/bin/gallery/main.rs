//! The Carbon component catalog: 42 inventory rows, one page each.
//!
//! Run it: `cargo run -p gorgon-petra-egui --bin gallery`
//!
//! # Why a binary, and why a second one
//!
//! `contracts/component-anatomy.md` §8 requires every built component, in every
//! variant, size and declared state, to be reachable from **one** renderable
//! surface — that surface is what `contracts/parity-harness.md`'s capture loop
//! walks, and a combination the surface cannot reach cannot be captured and so
//! cannot be gated. The 42 rows carry 323 parity cells before themes multiply
//! them. That does not fit in an `examples/` target: an example is one file
//! Cargo will not let you split into modules, and the existing
//! `examples/gallery.rs` is already 2,766 lines of it.
//!
//! So this is a `src/bin/` directory, which can hold modules, and the example
//! stays exactly where it is. Both exist on purpose right now:
//!
//! * `examples/gallery.rs` — the **working** page over the thirteen components
//!   Petra ships today, with its honesty counters. It renders. A later task
//!   migrates it here.
//! * `src/bin/gallery/` — **this**, the Carbon catalog over all 42 rows. It
//!   opens a window. Wave 1 rows are built. The rest name their slice
//!   file and refuse to draw a stand-in.

mod cat;
mod catalog;
mod cell;
#[cfg(test)]
mod font_spike;
mod inventory;
mod page;
#[cfg(test)]
mod shots;

use std::process::ExitCode;

use cell::{Cell, tally};

fn main() -> ExitCode {
    let roster = Cell::roster();
    let (built, total) = tally(&roster);

    println!("Petra component gallery — the 42 Carbon inventory rows");
    println!("{built} of {total} cells build a component.\n");
    for cell in &roster {
        let status = if cell.is_built() { "built" } else { "unbuilt" };
        println!(
            "{:>2}  {:<24} slice {}  {status}",
            cell.row.number,
            cell.row.component,
            cell.row.slice.letter()
        );
    }

    // The one thing in this binary that does draw besides the catalog
    // window: spec 005's draw-list acceptance scene (T133). It is not a
    // Carbon inventory row, so it is reported beside the tally rather than
    // inside it — a cat is not one of the 42, and counting it as one would
    // be exactly the kind of borrowed credit this catalog's own pages
    // refuse.
    let cat = cat::drawing();
    println!(
        "\nacceptance scene: the cat, {} draw-list command(s), {} path verb(s), \
         geometry only (references_assets = {})",
        cat.len(),
        cat.path_verbs(),
        cat.references_assets()
    );

    if built == 0 {
        eprintln!(
            "\ngallery: no cell builds a component, so there is no gallery to open. This is \
             the scaffold from spec 005 T004 — cell identity only. Building the components is \
             phase 5 (T063 onward); the working page over the thirteen components Petra ships \
             today is still `cargo run -p gorgon-petra-egui --example gallery`."
        );
        return ExitCode::FAILURE;
    }

    println!("\ngallery: opening the Carbon catalog window ({built} of {total} built).");
    match catalog::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("gallery: window failed: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cell::{Cell, tally};

    /// The catalog walks every row it claims to. A roster that silently lost
    /// a cell is the failure this whole file exists to prevent.
    #[test]
    fn the_roster_covers_the_whole_inventory() {
        let roster = Cell::roster();
        let (_, total) = tally(&roster);
        assert_eq!(total, crate::inventory::ROWS.len());
        assert_eq!(total, 42);
    }
}
