//! The one presenter that publishes theme snapshots, atomically
//! (FR-014, FR-016).
//!
//! Atomicity is a property of the type, not a rule written in a doc
//! comment: [`Presenter::publish`] takes `&self`, builds the entire new
//! [`ThemeSnapshot`] before touching any shared state, and then performs
//! exactly one write — replacing the `Arc` a `RwLock` guards. A reader
//! calls [`Presenter::current`], which clones that `Arc` under a brief read
//! lock and returns; the clone it walks away with is a complete, immutable
//! snapshot regardless of what `publish` does afterward, because there is
//! no field inside a published `ThemeSnapshot` that publish, or anyone
//! else, ever mutates in place. There is therefore no window in which a
//! reader can observe half of one theme and half of another.
//!
//! "One presenter" is enforced by usage, not by a private constructor: a
//! `Presenter` is ordinary owned data (not `Clone`, not a global), so the
//! host wires up exactly one and hands `Arc<Presenter>` (or a `&Presenter`)
//! to everything that reads it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::token::snapshot::ThemeSnapshot;
use crate::token::theme::Theme;

/// Publishes [`ThemeSnapshot`]s. See the module doc for the atomicity
/// argument.
pub struct Presenter {
    current: RwLock<Arc<ThemeSnapshot>>,
    next_revision: AtomicU64,
}

impl Presenter {
    /// Start presenting `initial` at revision 0. Revision 0 matches
    /// `Viewport::new`'s and `ThemeSnapshot`'s own zero-valued default, so
    /// a viewport built before any publication and one built just after
    /// agree on "no theme has been published yet".
    #[must_use]
    pub fn new(initial: Theme) -> Self {
        Self {
            current: RwLock::new(Arc::new(ThemeSnapshot::new(initial, 0))),
            next_revision: AtomicU64::new(1),
        }
    }

    /// The snapshot currently in force. Cheap: a lock-guarded `Arc` clone
    /// (a refcount bump), not a copy of the theme's token map.
    #[must_use]
    pub fn current(&self) -> Arc<ThemeSnapshot> {
        Arc::clone(
            &self
                .current
                .read()
                .expect("presenter lock poisoned by a panicking publisher"),
        )
    }

    /// Publish `theme` as the new current snapshot. Assigns the next
    /// revision, builds the full snapshot off to the side, then swaps it in
    /// with one write. Returns the new revision.
    pub fn publish(&self, theme: Theme) -> u64 {
        let revision = self.next_revision.fetch_add(1, Ordering::SeqCst);
        let snapshot = Arc::new(ThemeSnapshot::new(theme, revision));
        *self
            .current
            .write()
            .expect("presenter lock poisoned by a panicking publisher") = snapshot;
        revision
    }
}

#[cfg(test)]
mod tests {
    use super::Presenter;
    use crate::token::ThemeMode;
    use crate::token::name::TokenName;
    use crate::token::shipped::{dark, light};
    use crate::token::value::TokenValue;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn revisions_start_at_zero_and_increase_by_one_on_every_publish() {
        let presenter = Presenter::new(light());
        assert_eq!(presenter.current().revision(), 0);
        assert_eq!(presenter.publish(dark()), 1);
        assert_eq!(presenter.current().revision(), 1);
        assert_eq!(presenter.publish(light()), 2);
        assert_eq!(presenter.current().revision(), 2);
    }

    #[test]
    fn every_publication_changes_the_revision_even_when_the_theme_content_repeats() {
        // Publishing the same mode twice in a row is a legitimate
        // re-publication (e.g. a token value changed within the same
        // mode); the revision must still move, because a frame's identity
        // depends on the revision number, not on whether the content
        // happened to differ.
        let presenter = Presenter::new(light());
        let first = presenter.publish(light());
        let second = presenter.publish(light());
        assert_ne!(first, second);
        assert!(second > first);
    }

    #[test]
    fn a_reader_holding_an_old_snapshot_never_sees_it_change_underneath() {
        let presenter = Presenter::new(light());
        let held = presenter.current();
        assert_eq!(held.mode(), ThemeMode::Light);

        presenter.publish(dark());
        presenter.publish(light());
        presenter.publish(dark());

        // `held` is a separate `Arc` this thread cloned before any of
        // those publications; nothing about it can have moved.
        assert_eq!(held.revision(), 0);
        assert_eq!(held.mode(), ThemeMode::Light);

        // A fresh read after the publications sees the latest snapshot
        // whole.
        let fresh = presenter.current();
        assert_eq!(fresh.revision(), 3);
        assert_eq!(fresh.mode(), ThemeMode::Dark);
    }

    #[test]
    fn a_second_thread_never_observes_a_torn_publish() {
        // The property under test: every value a concurrent reader ever
        // sees from `current()` belongs entirely to one snapshot — never a
        // colour from the new theme paired with a mode from the old one,
        // or vice versa. We check this by reading two tokens together on
        // every poll and asserting they always agree on which theme they
        // came from.
        let surface = TokenName::new("surface.base").unwrap();
        let light_value = *light().value(&surface).unwrap();
        let dark_value = *dark().value(&surface).unwrap();
        assert_ne!(
            light_value, dark_value,
            "the two shipped themes must actually differ for this test to prove anything"
        );

        let presenter = Arc::new(Presenter::new(light()));
        let reached_dark = Arc::new(AtomicBool::new(false));

        let reader = {
            let presenter = Arc::clone(&presenter);
            let reached_dark = Arc::clone(&reached_dark);
            std::thread::spawn(move || {
                let mut observations = 0u32;
                // Poll until the publishing thread has definitely finished
                // (bounded, so a broken implementation fails the test
                // instead of hanging it).
                while observations < 200_000 {
                    let snapshot = presenter.current();
                    let mode = snapshot.mode();
                    let value: TokenValue = *snapshot.value(&surface).unwrap();
                    let expected = if mode == ThemeMode::Light {
                        light_value
                    } else {
                        dark_value
                    };
                    assert_eq!(
                        value, expected,
                        "mode and value disagree on which theme they came from"
                    );
                    if mode == ThemeMode::Dark {
                        reached_dark.store(true, Ordering::SeqCst);
                        break;
                    }
                    observations += 1;
                }
            })
        };

        presenter.publish(dark());
        reader.join().expect("reader thread panicked");
        assert!(
            reached_dark.load(Ordering::SeqCst),
            "reader never observed the published switch to dark"
        );
    }
}
