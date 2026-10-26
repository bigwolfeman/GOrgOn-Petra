//! Inventory row 13, Form.

use gorgon_petra::component::{
    checkbox, field, form, form_fluid, form_two_column, heading, hinted, labeled, section, valued,
};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

/// What the empty Name field shows. An example of the value, never the
/// label beside it — the label already says "Name", and a placeholder that
/// repeats it says nothing and reads as a duplicated word.
const PLACEHOLDER: &str = "fiber-7";

/// The editable field's key.
const NAME: &str = "form-name";
/// The checkbox's key.
const ENABLED: &str = "form-ok";

/// The Form page.
///
/// # What "broken elements" was
///
/// The operator's round-3 word for this row. It was three separate things,
/// all visible in `ignored/carbon-ref/shots/13-form.png` beside our own:
///
/// 1. **The page held no state.** `Form` was a unit struct whose `handle`
///    returned `false`, so the field took no keystroke and the checkbox
///    never toggled. Cross-cutting cause 4 in `ROUND3-DEFECTS.md`.
/// 2. **The field had no label.** Carbon's form item is a `.cds--label`
///    above the well (slice-b:227, anatomy 2a); this page passed the word
///    "Name" as the *placeholder*, so the well showed its own name until
///    the first keystroke wiped it. A bare well with its name inside it is
///    a search box. The label went in on 2026-09-05 but the placeholder
///    stayed "Name" beside it, which read as the word printed twice; the
///    placeholder is an example of the value now, which is the only thing
///    it is for (slice-e:184, "the user's entered content, styled via
///    `::placeholder` ... before anything is typed").
/// 3. **The items were 16 apart where Carbon puts 32.** Fixed in
///    `component/form.rs`, which owns the number.
pub struct Form {
    /// What the Name field holds. Empty on open, so the picture shows the
    /// placeholder, and typed into by [`Form::handle`].
    name: String,
    /// The checkbox's state. Carbon's Form has no state of its own; the
    /// controls in it do, and this is the one control on this page that
    /// carries any.
    enabled: bool,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
        }
    }
}

impl Page for Form {
    fn row(&self) -> &'static str {
        "Form"
    }

    /// The two documented disabled states (spec 008 T025, register rows 221
    /// and 223): `Form | state | disabled` is a disabled **child** — the
    /// child's own family is the whole treatment and the shell recolours
    /// nothing (slice-b States: the list describes the children) — and
    /// `Form | state | disabled-fieldset` flags the shell, whose legend then
    /// reads in `text.muted` (`fieldset[disabled] .cds--label`).
    fn disabled(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::disabled;

        vec![
            // The fieldset case (manifest row 223): a live form holding one
            // disabled field.
            form(
                "disabled-fieldset",
                "Fibers",
                vec![disabled(field("disabled-field", "Fiber name"))],
            ),
            // The whole form unavailable (row 221).
            disabled(form(
                "disabled-form",
                "Fibers",
                vec![field("disabled-form-field", "Fiber name")],
            )),
        ]
    }

    fn body(&self) -> ViewNode {
        section(
            "form",
            "Form",
            vec![filled_body(
                "form-body",
                sp("spacing.md"),
                vec![
                    form(
                        "demo-form",
                        "Fiber",
                        vec![
                            labeled(
                                "name-item",
                                "Name",
                                valued(hinted(field(NAME, "Name"), PLACEHOLDER), self.name.clone()),
                            ),
                            checkbox(ENABLED, "Enabled", self.enabled),
                        ],
                    ),
                    heading("fluid-title", "Fluid"),
                    // T056's fluid variant: items flush against each other,
                    // hairline rules between them, one legend above.
                    form_fluid(
                        "fluid-form",
                        "Fiber",
                        vec![
                            field("fluid-name", "Name"),
                            checkbox("fluid-ok", "Enabled", true),
                            field("fluid-port", "Port"),
                        ],
                    ),
                    heading("grid-title", "Two columns"),
                    // T056's two-column variant: items flowing row-major
                    // over two equal columns at Carbon's form margins.
                    form_two_column(
                        "grid-form",
                        "Fiber",
                        vec![
                            field("grid-name", "Name"),
                            field("grid-port", "Port"),
                            field("grid-host", "Host"),
                            field("grid-role", "Role"),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if path_has(node, ENABLED) {
            self.enabled = !self.enabled;
            return true;
        }
        if !path_has(node, NAME) {
            return false;
        }
        match event {
            InputEvent::Text(typed) => {
                self.name.push_str(typed);
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } => {
                self.name.pop();
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Form;
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::tree::{NodeKind, TrackSize};

    /// T056's fluid variant cell: the fluid form is on the page with its
    /// three flush items under one legend.
    #[test]
    fn the_page_carries_the_fluid_form_with_its_items() {
        let tree = Form::default().body();
        let fluid = find(&tree, "fluid-form").expect("the fluid form is on the page");
        for key in ["fluid-name", "fluid-ok", "fluid-port"] {
            assert!(find(fluid, key).is_some(), "the fluid form carries {key}");
        }
    }

    /// T056's two-column variant cell: the item grid is a two-track grid,
    /// both tracks weighted — the layout the variant's whole name is about.
    #[test]
    fn the_page_carries_the_two_column_form_as_a_two_track_grid() {
        let tree = Form::default().body();
        let grid = find(&tree, "grid-form-items").expect("the two-column grid is on the page");
        assert_eq!(grid.kind, NodeKind::Grid);
        assert!(
            matches!(
                grid.props.columns.as_slice(),
                [TrackSize::Weight { .. }, TrackSize::Weight { .. }]
            ),
            "two weighted columns, got {:?}",
            grid.props.columns
        );
    }
}
