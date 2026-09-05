//! Inventory row 21, Notification.

use gorgon_petra::component::{
    NotificationKind, notification_actionable_kind, notification_inline_kind, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

/// The toast's action button, keyed by [`gorgon_petra::component`]'s own
/// `action_button`, so a press names this segment.
const ACTION: &str = "action";

/// The four kinds in Carbon's own order (`Notification.js`'s icon map).
const KINDS: [(NotificationKind, &str, &str, &str); 4] = [
    (
        NotificationKind::Error,
        "nt-error",
        "Rebuild failed",
        "Fiber 7 did not come back.",
    ),
    (
        NotificationKind::Warning,
        "nt-warning",
        "Disk filling",
        "Trace volume is at 80%.",
    ),
    (
        NotificationKind::Info,
        "nt-info",
        "Rebuild finished",
        "12 fibers reloaded.",
    ),
    (
        NotificationKind::Success,
        "nt-success",
        "Supervisor restarted",
        "Worker 3 came back.",
    ),
];

/// Index of [`NotificationKind::Info`] in [`KINDS`]: what the toast opens
/// on, so the resting page is the quiet one.
const OPENS_ON: usize = 2;

/// Live state of the Notification page: which of [`KINDS`] the toast is
/// showing.
///
/// The page held no state at all until 2026-09-05, so the operator had no
/// way to see the icon field he asked for do anything. Pressing the toast's
/// action steps it through all four kinds, and the glyph, the title and the
/// message all change together — the toast never shows an error ring over
/// the word "finished".
pub struct Notification {
    kind: usize,
}

impl Default for Notification {
    fn default() -> Self {
        Self { kind: OPENS_ON }
    }
}

impl Notification {
    fn showing(&self) -> (NotificationKind, &'static str, &'static str, &'static str) {
        KINDS[self.kind % KINDS.len()]
    }
}

impl Page for Notification {
    fn row(&self) -> &'static str {
        "Notification"
    }

    fn body(&self) -> ViewNode {
        let inline: Vec<ViewNode> = KINDS
            .iter()
            .map(|(kind, key, title, message)| {
                notification_inline_kind(*key, *kind, *title, *message)
            })
            .collect();
        let (kind, _, title, message) = self.showing();
        section(
            "note",
            "Notification",
            vec![body(
                "nt-inline",
                sp("spacing.md"),
                vec![
                    // Not stretched. The component carries Carbon's band —
                    // `min-inline-size: 288px` and the ramp's `md` cap of
                    // 608 — and every message on this page is shorter than
                    // the floor, so all four cards land on 288 exactly and
                    // the column is one column. Round 3 photographed a
                    // staircase here because the floor did not exist yet.
                    // Stretching to the page instead would also line them up,
                    // at 550-odd wide with a lot of empty band; Carbon's own
                    // reference for this row is the narrow card.
                    body("kinds", sp("spacing.md"), inline),
                    // The toast, keyed as it has been since round 2 so its
                    // `nt/panel` path stays the one `shots.rs` photographs.
                    notification_actionable_kind("nt", kind, title, message, "Next kind"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, ACTION) {
            self.kind = (self.kind + 1) % KINDS.len();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTION, KINDS, Notification, OPENS_ON};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::component::NotificationKind;
    use gorgon_petra::geom::Point;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    fn press(page: &mut Notification, node: &str) -> bool {
        page.handle(
            &InputEvent::PointerPressed {
                pos: Point::ZERO,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            node,
        )
    }

    /// The status field is live: a press on the toast's action steps the
    /// kind, and the glyph the card draws changes with it. Read off the
    /// glyph's own draw list, because the kind's channel is the picture.
    #[test]
    fn pressing_the_toast_action_steps_the_kind_and_redraws_the_glyph() {
        let mut page = Notification::default();
        // Scoped to the toast: four inline cards on this page also key a
        // `glyph`, and an unscoped `find` returns the first of those, which
        // never changes and would make this test pass on a dead field.
        let glyph = |page: &Notification| {
            let tree = page.body();
            let toast = find(&tree, "nt").expect("the toast is on the page");
            find(toast, "glyph")
                .expect("the toast card draws a status glyph")
                .props
                .canvas
                .clone()
                .expect("the glyph is a canvas")
        };
        let mut seen = vec![glyph(&page)];
        for step in 1..KINDS.len() {
            assert!(
                press(&mut page, &format!("/page/body/nt/panel/{ACTION}")),
                "step {step}: the action is this page's own control"
            );
            let next = glyph(&page);
            assert!(
                !seen.contains(&next),
                "step {step}: the kind advanced but the glyph is one \
                 already drawn — the icon field does not reach the picture"
            );
            seen.push(next);
        }
        assert_eq!(seen.len(), KINDS.len());
    }

    /// All four kinds are on the page at once, each with its own glyph, so
    /// the four marks can be told apart side by side with no hue.
    #[test]
    fn the_four_kinds_draw_four_different_glyphs() {
        let page = Notification::default();
        let tree = page.body();
        let mut lists = Vec::new();
        for (kind, key, _, _) in KINDS {
            let card = find(&tree, key).unwrap_or_else(|| panic!("{key} is not on the page"));
            assert_eq!(
                card.semantics.value.as_deref(),
                Some(kind.word()),
                "{key} must carry its kind as a word, the channel that \
                 outlives both the hue and the picture"
            );
            let list = find(card, "glyph")
                .unwrap_or_else(|| panic!("{key} draws no glyph"))
                .props
                .canvas
                .clone()
                .expect("the glyph is a canvas");
            assert!(
                !lists.contains(&list),
                "{key} draws a glyph another kind already drew"
            );
            lists.push(list);
        }
        assert_eq!(lists.len(), 4);
        assert_eq!(
            KINDS[OPENS_ON].0,
            NotificationKind::Info,
            "the toast opens on the quiet kind"
        );
    }
}
