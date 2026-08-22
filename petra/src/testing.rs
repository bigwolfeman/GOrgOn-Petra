//! Deterministic doubles for the boundaries the engine cannot cross alone.
//!
//! [`crate::layout::ContentMeasure`] and [`crate::layout::RowSource`] are
//! implemented outside this crate — by `gorgon-petra-egui` against shaped
//! galleys, and by a host against its store. Layout tests need neither a GPU
//! nor a font, so they use the fakes here.
//!
//! These are shipped rather than hidden behind `#[cfg(test)]` on purpose:
//! integration tests, benches, and downstream crates' layout tests all need the
//! same fake, and five hand-rolled copies would drift into five different
//! definitions of "how wide is this text". Nothing in a production path
//! constructs them.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use crate::geom::{Scale, Size};
use crate::layout::{
    ContentMeasure, LayoutCtx, LayoutState, MeasureCache, RowSource, ScrollStack, SizeProposal,
    TextMeasurement, TextRequest,
};
use crate::tree::{Key, NodeKind, Props, Registry, TextWrap, ValidatedTree, ViewNode, validate};

/// Accept `tree` against an empty [`Registry`] and mint the token
/// [`crate::frame::petrify`] requires, panicking with the named violations if
/// it does not validate.
///
/// This is the one helper every test and bench in the workspace mints a
/// [`ValidatedTree`] through, rather than each of dozens of call sites
/// repeating its own `validate(...).expect("valid")` — see the project's
/// util rule. Reach for [`validated_with`] instead when the tree under test
/// declares a custom kind or a transition name that a real registry would
/// need to know about.
///
/// # Panics
/// Panics naming the violations when `tree` does not accept.
#[must_use]
pub fn validated(tree: &ViewNode) -> ValidatedTree<'_> {
    validated_with(tree, &Registry::new())
}

/// [`validated`], against a caller-supplied [`Registry`] rather than an empty
/// one.
///
/// # Panics
/// Panics naming the violations when `tree` does not accept.
#[must_use]
pub fn validated_with<'a>(tree: &'a ViewNode, registry: &Registry) -> ValidatedTree<'a> {
    match validate(tree, registry) {
        Ok(v) => v,
        Err(errors) => panic!("tree used in a test did not pass acceptance:\n{errors}"),
    }
}

/// A fixed-pitch text measurer: every character is [`MonoContent::char_w`]
/// wide, every line is [`MonoContent::line_h`] tall.
///
/// Real shaping is proportional, script-dependent, and font-dependent. This
/// fake is none of those things, which is the point: a layout test that fails
/// should fail because the distribution algorithm is wrong, not because a font
/// changed. Anything that depends on real metrics belongs in a
/// `gorgon-petra-egui` test instead.
#[derive(Clone, Debug)]
pub struct MonoContent {
    /// Width of one character in logical units.
    pub char_w: f32,
    /// Height of one line in logical units.
    pub line_h: f32,
    /// Natural size an image reports at any proposal.
    pub image_size: Size,
    /// Natural size a custom node reports at any proposal.
    pub custom_size: Size,
    /// How many measurement calls have been served. Tests that assert the
    /// measurement cache is doing its job read this.
    pub calls: usize,
}

impl Default for MonoContent {
    fn default() -> Self {
        Self {
            char_w: 8.0,
            line_h: 16.0,
            image_size: Size::new(64.0, 64.0),
            custom_size: Size::new(32.0, 32.0),
            calls: 0,
        }
    }
}

impl MonoContent {
    /// A measurer with the default metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The unwrapped width of `text`.
    #[must_use]
    pub fn run_width(&self, text: &str) -> f32 {
        text.chars().count() as f32 * self.char_w
    }
}

impl ContentMeasure for MonoContent {
    fn text(&mut self, req: &TextRequest<'_>) -> TextMeasurement {
        self.calls += 1;
        let full = self.run_width(req.text);
        let Some(available) = req.available_width else {
            // An open probe: the full unwrapped run, one line.
            return TextMeasurement {
                size: Size::new(full, self.line_h),
                truncated: false,
                lines: 1,
            };
        };
        let per_line = (available / self.char_w).floor().max(0.0) as usize;
        if per_line == 0 {
            return TextMeasurement {
                size: Size::new(0.0, self.line_h),
                truncated: !req.text.is_empty(),
                lines: 1,
            };
        }
        let chars = req.text.chars().count();
        let (lines, truncated) = match req.wrap {
            TextWrap::Wrap => {
                let needed = chars.div_ceil(per_line).max(1);
                match req.max_lines {
                    Some(cap) if needed > cap => (cap, true),
                    _ => (needed, false),
                }
            }
            TextWrap::Ellipsis | TextWrap::Clip => (1, chars > per_line),
        };
        let widest = if lines == 1 {
            full.min(available)
        } else {
            available
        };
        TextMeasurement {
            size: Size::new(widest, lines as f32 * self.line_h),
            truncated,
            lines,
        }
    }

    fn image(&mut self, _source: &str, proposal: SizeProposal) -> Size {
        self.calls += 1;
        let _ = proposal;
        self.image_size
    }

    fn custom(&mut self, _name: &str, proposal: SizeProposal) -> Size {
        self.calls += 1;
        let _ = proposal;
        self.custom_size
    }
}

/// A row source with no rows. Answers every request with nothing, which is what
/// a tree with no `collection` node needs.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoRows;

impl RowSource for NoRows {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

/// A row source that synthesizes rows on demand.
///
/// Holds no rows itself, so a 100 000-row virtualization test measures the
/// engine's materialization bound rather than the fixture's allocation.
#[derive(Clone, Debug)]
pub struct GeneratedRows {
    /// Rows available per source name.
    pub totals: BTreeMap<String, usize>,
    /// Highest row index ever requested, for asserting the window is bounded.
    pub max_index_seen: usize,
    /// Rows served since construction.
    pub served: usize,
}

impl GeneratedRows {
    /// A source named `name` with `total` rows.
    #[must_use]
    pub fn new(name: &str, total: usize) -> Self {
        let mut totals = BTreeMap::new();
        totals.insert(name.to_owned(), total);
        Self {
            totals,
            max_index_seen: 0,
            served: 0,
        }
    }

    /// The node this source synthesizes for row `index`.
    #[must_use]
    pub fn row(index: usize) -> ViewNode {
        ViewNode::new(NodeKind::Text, Key::new(format!("row-{index}"))).with_props(Props {
            text: Some(format!("row {index}")),
            ..Props::default()
        })
    }
}

impl RowSource for GeneratedRows {
    fn rows(&mut self, source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        let total = self.totals.get(source).copied().unwrap_or(0);
        let end = range.end.min(total);
        if range.start >= end {
            return Vec::new();
        }
        self.max_index_seen = self.max_index_seen.max(end.saturating_sub(1));
        self.served += end - range.start;
        (range.start..end).map(|i| Arc::new(Self::row(i))).collect()
    }
}

/// Everything a layout pass needs, owned in one place so a test can build a
/// [`LayoutCtx`] in one line.
#[derive(Debug)]
pub struct Harness<C: ContentMeasure, R: RowSource> {
    /// Content measurement double.
    pub content: C,
    /// Row materialization double.
    pub rows: R,
    /// The measurement cache under test.
    pub cache: MeasureCache,
    /// The state snapshot this pass reads.
    pub state: LayoutState,
    /// Theme snapshot revision.
    pub theme_rev: u64,
    /// Display scale.
    pub scale: Scale,
}

impl Default for Harness<MonoContent, NoRows> {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness<MonoContent, NoRows> {
    /// A harness with fixed-pitch text and no rows.
    #[must_use]
    pub fn new() -> Self {
        Self::with(MonoContent::new(), NoRows)
    }
}

impl<C: ContentMeasure, R: RowSource> Harness<C, R> {
    /// A harness over the given doubles.
    pub fn with(content: C, rows: R) -> Self {
        Self {
            content,
            rows,
            cache: MeasureCache::new(),
            state: LayoutState::default(),
            theme_rev: 1,
            scale: Scale::ONE,
        }
    }

    /// A layout context borrowing this harness.
    pub fn ctx(&mut self) -> LayoutCtx<'_> {
        LayoutCtx {
            content: &mut self.content,
            rows: &mut self.rows,
            cache: &mut self.cache,
            state: &self.state,
            theme_rev: self.theme_rev,
            scale: self.scale,
            scroll: ScrollStack::new(),
            reuse: None,
        }
    }

    /// Set a scroll offset for a node id.
    pub fn set_scroll(&mut self, id: &str, offset: f32) {
        self.state.scroll_offsets.insert(id.to_owned(), offset);
    }
}

#[cfg(test)]
mod tests {
    use super::{GeneratedRows, Harness, MonoContent};
    use crate::geom::Size;
    use crate::layout::{ContentMeasure, RowSource, TextRequest};
    use crate::tree::TextWrap;

    fn req<'a>(text: &'a str, width: Option<f32>, wrap: TextWrap) -> TextRequest<'a> {
        TextRequest {
            text,
            style: None,
            wrap,
            max_lines: None,
            available_width: width,
        }
    }

    #[test]
    fn an_open_probe_returns_the_full_unwrapped_run() {
        let mut c = MonoContent::new();
        let m = c.text(&req("hello", None, TextWrap::Wrap));
        assert_eq!(m.size, Size::new(40.0, 16.0));
        assert_eq!(m.lines, 1);
        assert!(!m.truncated);
    }

    #[test]
    fn wrapping_grows_line_count_and_reports_no_truncation() {
        let mut c = MonoContent::new();
        let m = c.text(&req("abcdefgh", Some(32.0), TextWrap::Wrap));
        assert_eq!(m.lines, 2);
        assert_eq!(m.size, Size::new(32.0, 32.0));
        assert!(!m.truncated);
    }

    #[test]
    fn a_line_cap_truncates_rather_than_growing() {
        let mut c = MonoContent::new();
        let m = c.text(&TextRequest {
            max_lines: Some(1),
            ..req("abcdefgh", Some(32.0), TextWrap::Wrap)
        });
        assert_eq!(m.lines, 1);
        assert!(m.truncated);
    }

    #[test]
    fn clip_and_ellipsis_stay_on_one_line() {
        let mut c = MonoContent::new();
        for wrap in [TextWrap::Clip, TextWrap::Ellipsis] {
            let m = c.text(&req("abcdefgh", Some(32.0), wrap));
            assert_eq!(m.lines, 1, "{wrap:?}");
            assert!(m.truncated, "{wrap:?}");
            assert_eq!(m.size.w, 32.0);
        }
    }

    #[test]
    fn a_zero_width_offer_truncates_anything_non_empty() {
        let mut c = MonoContent::new();
        assert!(c.text(&req("x", Some(0.0), TextWrap::Wrap)).truncated);
        assert!(!c.text(&req("", Some(0.0), TextWrap::Wrap)).truncated);
    }

    /// The generated source must never serve past the declared total, or a
    /// virtualization test would pass while the engine fabricated rows.
    #[test]
    fn generated_rows_stop_at_the_declared_total() {
        let mut rows = GeneratedRows::new("fibers", 10);
        assert_eq!(rows.rows("fibers", 8..20).len(), 2);
        assert_eq!(rows.rows("fibers", 20..30).len(), 0);
        assert_eq!(rows.rows("other", 0..5).len(), 0);
        assert_eq!(rows.max_index_seen, 9);
        assert_eq!(rows.served, 2);
    }

    #[test]
    fn the_harness_hands_out_a_usable_context() {
        let mut h = Harness::new();
        h.set_scroll("/list", 40.0);
        let ctx = h.ctx();
        assert_eq!(ctx.state.scroll_offset("/list"), 40.0);
        assert_eq!(ctx.theme_rev, 1);
    }
}
