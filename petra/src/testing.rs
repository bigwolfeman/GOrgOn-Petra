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
use std::sync::{Arc, OnceLock};

use crate::geom::{Insets, Scale, Size};
use crate::layout::{
    AnchorRects, ContentMeasure, LayoutCtx, LayoutState, MeasureCache, RowSource, ScrollStack,
    SizeProposal, TextMeasurement, TextRequest,
};
use crate::token::{
    DesignToken, Theme, ThemeSnapshot, TokenKind, TokenName, TokenValue, Vocabulary, light,
    standard_vocabulary,
};
use crate::tree::{
    Anchor, InsetRefs, Key, NodeKind, Props, Registry, TextWrap, ValidatedTree, ViewNode, validate,
};

/// Accept `tree` against [`extended_vocabulary`]`(tree)` — the shipped
/// vocabulary plus this tree's own [`gap_token`] spacings — and mint the
/// token [`crate::frame::petrify`] requires, panicking with the named
/// violations if it does not validate.
///
/// This is the one helper every test and bench in the workspace mints a
/// [`ValidatedTree`] through, rather than each of dozens of call sites
/// repeating its own `validate(...).expect("valid")` — see the project's
/// util rule. Reach for [`validated_with`] instead when the tree under test
/// declares a custom kind or a transition name a real registry would need to
/// know about, or when it references tokens outside the shipped vocabulary
/// and the fixture gap scale — [`extended_vocabulary`] documents exactly what
/// this function does and does not cover.
///
/// # Panics
/// Panics naming the violations when `tree` does not accept.
#[must_use]
pub fn validated(tree: &ViewNode) -> ValidatedTree<'_> {
    validated_with(tree, &Registry::with_vocabulary(extended_vocabulary(tree)))
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

/// The whole-unit gaps every [`Harness`] resolves without being asked:
/// `spacing.0units` through `spacing.64units`.
///
/// 64 because [`crate::tree::props::DEFAULT_OVERSCAN`] is 64, and almost
/// every fixture in the workspace reserves less than the largest default the
/// engine ships. It is a *pre-binding* range, not a limit on what
/// [`gap_token`] can name: a fixture outside it names its gap the same way
/// and calls [`Harness::bind_tree_gaps`] or [`Harness::bind_spacings`].
pub const MAX_PREBOUND_GAP: u32 = 64;

/// The token name for a fixture gap of exactly `units` logical units.
///
/// Layout fixtures and design tokens want opposite things from a number. The
/// shipped ramp ([`crate::token::standard_vocabulary`]) is eight steps chosen
/// so a page hangs together; a fixture asserts "four children and a gap of 9
/// measure 27 units of gap", and its expectation is arithmetic on that 9.
/// Rounding the fixture onto a ramp step would make it agree with the design
/// system and stop measuring the container — the container is what is under
/// test, not the taste.
///
/// So fixtures get their own names, and the name **encodes the gap**. A whole
/// number spells itself (`spacing.9units`), because that is what a fixture
/// author reads in a diff; anything else spells its bit pattern
/// (`spacing.1097654321bits`), because a swept `14.189328` still has to name
/// itself exactly and no decimal spelling survives [`TokenName`]'s
/// segment rule. [`gap_units`] reads either form back.
///
/// Encoding rather than registering is what lets a *generated* tree carry a
/// complete theme: [`Harness::bind_tree_gaps`] binds what the tree declares,
/// so a proptest never has to keep a second list of the gaps it swept.
///
/// # Panics
/// If `units` is negative or not finite. A gap is neither.
#[must_use]
pub fn gap_token(units: f32) -> TokenName {
    assert!(
        units.is_finite() && units >= 0.0,
        "a fixture gap is a finite, non-negative extent, not {units}"
    );
    let spelled = if units.fract() == 0.0 && units <= f64::from(u32::MAX) as f32 {
        format!("spacing.{}units", units as u32)
    } else {
        format!("spacing.{}bits", units.to_bits())
    };
    TokenName::new(spelled).expect("a fixture gap spells a well-formed token name")
}

/// The gap `name` encodes, or `None` when `name` is not one [`gap_token`]
/// produced.
///
/// Exact in both directions: the whole-unit form round-trips through a `u32`
/// and the general form through `f32::to_bits`, so a fixture's declared gap
/// and its resolved gap are the same float, not two floats that print alike.
#[must_use]
pub fn gap_units(name: &TokenName) -> Option<f32> {
    let rest = name.as_str().strip_prefix("spacing.")?;
    if let Some(digits) = rest.strip_suffix("units") {
        return digits.parse::<u32>().ok().map(|whole| whole as f32);
    }
    rest.strip_suffix("bits")
        .and_then(|digits| digits.parse::<u32>().ok())
        .map(f32::from_bits)
}

/// [`gap_token`] wrapped for a props field, which is what every call site
/// wants: `spacing: gap(9.0)`.
///
/// Zero is a token here, not an absence. A fixture that means "this stack
/// declares a gap and the gap is nothing" is making a different statement
/// from one that declares no gap at all, and both statements have tests.
///
/// # Panics
/// As [`gap_token`].
#[must_use]
pub fn gap(units: f32) -> Option<TokenName> {
    Some(gap_token(units))
}

/// The [`InsetRefs`] that names each edge of `insets` on the fixture gap
/// scale, so a fixture can keep saying what it means in numbers
/// (`gap_insets(Insets::symmetric(4.0, 8.0))`) while the tree it builds
/// carries token references like every other tree.
///
/// # Panics
/// As [`gap_token`].
#[must_use]
pub fn gap_insets(insets: Insets) -> InsetRefs {
    InsetRefs {
        top: gap(insets.top),
        right: gap(insets.right),
        bottom: gap(insets.bottom),
        left: gap(insets.left),
    }
}

/// Every spacing-shaped token name `tree` declares: `spacing`,
/// `column_spacing`, `row_spacing`, and every edge of `padding`, walked over
/// the whole tree.
///
/// Shared by [`Harness::bind_tree_gaps`] (which extends a *theme* with these
/// names) and [`extended_vocabulary`] (which extends a *vocabulary* with
/// them) — one walk, so the two can never drift into declaring a different
/// set of names than the one a tree actually references.
fn spacing_refs(node: &ViewNode, out: &mut Vec<TokenName>) {
    let padding = node
        .props
        .padding
        .iter()
        .flat_map(|refs| [&refs.top, &refs.right, &refs.bottom, &refs.left].into_iter());
    // An anchor offset is a spacing reference like any other, and it is the
    // only one that does not sit on a `props.*_spacing` field. Missing it here
    // would let a fixture name an offset token outside the pre-bound gap ramp
    // and have `extended_vocabulary` quietly not declare it — the theme would
    // then refuse a name the tree legitimately uses.
    let anchor_offset = node.props.anchor.as_ref().and_then(Anchor::offset);
    for name in [
        &node.props.spacing,
        &node.props.column_spacing,
        &node.props.row_spacing,
    ]
    .into_iter()
    .chain(padding)
    .flatten()
    .chain(anchor_offset)
    {
        out.push(name.clone());
    }
    for child in &node.children {
        spacing_refs(child, out);
    }
}

/// The vocabulary [`validated`] and [`validated_with`]'s callers implicitly
/// build a theme against: [`standard_vocabulary`], the whole-unit fixture
/// gap scale ([`gap_token`], `0..=`[`MAX_PREBOUND_GAP`] — the same range
/// [`fixture_theme`] pre-binds), and every [`gap_token`]-shaped name `tree`
/// itself declares outside that range ([`gap_units`] recognises the shape;
/// anything else is left alone).
///
/// This is one vocabulary, not a second, looser rule set for fixtures (C18):
/// a fixture that names a token neither shipped nor a [`gap_token`] spelling
/// is refused exactly as it would be under a host's real vocabulary — the
/// only names this function adds are ones [`gap_token`] itself would have
/// produced, each declared at the one kind [`gap_token`] ever means,
/// [`TokenKind::Spacing`]. A fixture that invents an arbitrary spacing name
/// through [`Harness::bind_spacings`] rather than [`gap_token`] is not
/// covered — those names are for the layout-module white-box tests that
/// call `measure`/`place` directly and never pass through [`validate`] at
/// all; a fixture that does validate should spell its gaps with
/// [`gap_token`].
#[must_use]
pub fn extended_vocabulary(tree: &ViewNode) -> Vocabulary {
    let mut vocab = standard_vocabulary();
    for units in 0..=MAX_PREBOUND_GAP {
        vocab.declare(DesignToken::new(
            gap_token(units as f32),
            TokenKind::Spacing,
        ));
    }
    let mut declared = Vec::new();
    spacing_refs(tree, &mut declared);
    for name in declared {
        if gap_units(&name).is_some() {
            vocab.declare(DesignToken::new(name, TokenKind::Spacing));
        }
    }
    vocab
}

/// The theme every [`Harness`] starts on: the shipped light theme, plus one
/// [`gap_token`] per whole unit up to [`MAX_PREBOUND_GAP`].
///
/// Built once and cloned, which is cheaper than the `light()` call each
/// harness used to make on its own — the sRGB conversions and the map inserts
/// happen one time for the whole process.
fn fixture_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let mut vocab = standard_vocabulary();
        let mut values = light().values().clone();
        for units in 0..=MAX_PREBOUND_GAP {
            let name = gap_token(units as f32);
            vocab.declare(DesignToken::new(name.clone(), TokenKind::Spacing));
            values.insert(name, TokenValue::Spacing(units as f32));
        }
        Theme::build(crate::token::ThemeMode::Light, &vocab, values)
            .expect("the shipped light theme plus a whole-unit gap scale is complete")
    })
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
    /// The theme snapshot this pass resolves token names against. Owned here
    /// because [`LayoutCtx::theme`] borrows one for the whole pass. Starts on
    /// the shipped [`light`] theme plus the whole-unit fixture gap scale
    /// ([`gap_token`]) at revision 1, which is the `theme_rev` below; swap it
    /// with [`Harness::set_theme`], which keeps the two in step, or extend it
    /// with [`Harness::bind_spacings`].
    pub theme: ThemeSnapshot,
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
            theme: ThemeSnapshot::new(fixture_theme().clone(), 1),
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
            theme: &self.theme,
            theme_rev: self.theme_rev,
            scale: self.scale,
            scroll: ScrollStack::new(),
            reuse: None,
            anchors: AnchorRects::new(),
        }
    }

    /// Extend this harness's theme with spacing names it does not already
    /// define, at exact values.
    ///
    /// For a gap outside the pre-bound whole-unit range
    /// ([`MAX_PREBOUND_GAP`]) under a name the fixture chooses for itself.
    /// A fixture that names its gaps with [`gap_token`] wants
    /// [`Harness::bind_tree_gaps`] instead, which reads the numbers off the
    /// tree rather than making the author list them again.
    ///
    /// The theme this builds is a real one, built through [`Theme::build`]
    /// against a vocabulary that declares every name the theme assigns — so
    /// it is complete and kind-checked exactly like a shipped theme, and
    /// resolution through it takes the path production takes. Bindings
    /// accumulate: the vocabulary is derived from what the current theme
    /// already assigns, so a second call does not erase the first, and
    /// calling this after [`Harness::set_theme`] extends *that* theme rather
    /// than reverting to the shipped one.
    ///
    /// The revision is left where it was, so binding a fixture gap does not
    /// look like a theme switch to [`crate::layout::MeasureCache`].
    ///
    /// # Panics
    /// If a name is not a well-formed [`TokenName`], or if the resulting
    /// theme is not complete — both are bugs in the fixture, not conditions
    /// to recover from.
    pub fn bind_spacings(&mut self, extra: &[(&str, f32)]) {
        let mut values = self.theme.theme().values().clone();
        // Whatever the current theme already assigns, at the kind it assigns
        // it: that is what keeps earlier bindings (and a swapped-in theme's
        // own extras) declared rather than dropped on the next build.
        let mut vocab = Vocabulary::from_theme(self.theme.theme());
        for (raw, units) in extra {
            let name = TokenName::new(*raw)
                .unwrap_or_else(|err| panic!("fixture spacing name {raw:?}: {err}"));
            vocab.declare(DesignToken::new(name.clone(), TokenKind::Spacing));
            values.insert(name, TokenValue::Spacing(*units));
        }
        let theme = Theme::build(self.theme.mode(), &vocab, values)
            .expect("a fixture theme extended with spacing names is complete");
        self.theme = ThemeSnapshot::new(theme, self.theme.revision());
    }

    /// Bind every [`gap_token`] gap that `tree` declares and this harness's
    /// theme does not already define.
    ///
    /// The name encodes the number ([`gap_token`]), so the tree is a complete
    /// statement of what its theme has to answer. A fixture that generates
    /// gaps — a proptest sweep — therefore needs no second list of the values
    /// it generated, and cannot let the two drift.
    ///
    /// Names the theme already defines are left alone, so the shipped ramp
    /// and the pre-bound whole-unit range are never re-declared, and a name
    /// that is not a fixture gap (a shipped `spacing-04`, say) is ignored
    /// rather than guessed at.
    ///
    /// # Panics
    /// If the resulting theme is not complete, which would be a bug here
    /// rather than in the fixture.
    pub fn bind_tree_gaps(&mut self, tree: &ViewNode) {
        let mut declared = Vec::new();
        spacing_refs(tree, &mut declared);
        let wanted: Vec<(String, f32)> = declared
            .into_iter()
            .filter(|name| self.theme.value(name).is_none())
            .filter_map(|name| gap_units(&name).map(|units| (name.as_str().to_owned(), units)))
            .collect();
        if wanted.is_empty() {
            return;
        }
        let pairs: Vec<(&str, f32)> = wanted
            .iter()
            .map(|(name, units)| (name.as_str(), *units))
            .collect();
        self.bind_spacings(&pairs);
    }

    /// Swap in a different theme snapshot, the way a running host does when
    /// the operator switches theme.
    ///
    /// `theme_rev` follows the snapshot's own revision, because the pair has
    /// to agree for the measure cache to be invalidated by the switch it is
    /// supposed to be invalidated by. A fixture that wants the two to
    /// disagree — `crate::layout::reuse::FrameMemo::adopt` documents why some
    /// do — still assigns the field directly.
    pub fn set_theme(&mut self, theme: ThemeSnapshot) {
        self.theme_rev = theme.revision();
        self.theme = theme;
    }

    /// Set a scroll offset for a node id.
    pub fn set_scroll(&mut self, id: &str, offset: f32) {
        self.state.scroll_offsets.insert(id.to_owned(), offset);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GeneratedRows, Harness, MAX_PREBOUND_GAP, MonoContent, extended_vocabulary, gap_token,
    };
    use crate::geom::Size;
    use crate::layout::{ContentMeasure, RowSource, TextRequest};
    use crate::tree::{Anchor, Edge, NodeKind, Props, TextWrap, ViewNode};

    /// An anchor offset is the one spacing reference that does not sit on a
    /// `props.*_spacing` field. Until `spacing_refs` walked it, a fixture
    /// naming an offset outside the pre-bound ramp got a vocabulary that did
    /// not declare it, and the tree it legitimately wrote was refused.
    #[test]
    fn extended_vocabulary_declares_an_anchor_offset_outside_the_prebound_ramp() {
        let far = gap_token((MAX_PREBOUND_GAP + 7) as f32);
        let mut node = ViewNode::new(NodeKind::Stack, "root");
        node.props = Props {
            anchor: Some(Anchor::Node {
                id: "/target".to_owned(),
                edge: Edge::Bottom,
                align: crate::tree::Align::default(),
                offset: Some(far.clone()),
            }),
            ..Props::default()
        };
        let vocab = extended_vocabulary(&node);
        assert!(
            vocab.contains(&far),
            "the offset token {far} must be declared, or the tree that names it cannot validate"
        );
    }

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

#[cfg(test)]
mod escape_hatch {
    use super::{extended_vocabulary, gap, gap_token};
    use crate::token::TokenName;
    use crate::tree::{NodeKind, Props, Registry, ViewNode, validate};

    /// The fixture harness widens the vocabulary so a generated tree can spell
    /// its own gaps ([`gap_token`]), and that widening is the one thing that
    /// could quietly turn every fixture in the workspace into a proof of
    /// nothing. So it is deliberately narrow: [`extended_vocabulary`] declares
    /// only names `gap_token` itself would have produced. A fixture that
    /// invents any other name is refused exactly like production code is.
    ///
    /// This is true by construction today — `extended_vocabulary` filters on
    /// `gap_units(&name).is_some()`. The test exists because "by construction"
    /// stays true only until somebody edits the construction.
    #[test]
    fn the_fixture_vocabulary_is_not_an_escape_hatch() {
        let minted = gap_token(9.0);
        let invented = TokenName::new("spacing.whatever").expect("well-formed, just undeclared");

        for (name, must_be_accepted) in [(minted, true), (invented, false)] {
            let tree = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
                spacing: Some(name.clone()),
                ..Props::default()
            });
            let registry = Registry::with_vocabulary(extended_vocabulary(&tree));
            let accepted = validate(&tree, &registry).is_ok();
            assert_eq!(
                accepted, must_be_accepted,
                "`{name}` acceptance under the fixture vocabulary"
            );
        }
    }

    /// The gap the harness mints is a *spacing* token, so naming it in a
    /// styling slot of another kind is still a kind mismatch. Widening the
    /// vocabulary must not widen what a name may be used for.
    #[test]
    fn a_minted_gap_is_still_only_a_spacing_token() {
        let tree = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            spacing: gap(9.0),
            style: Some(gap_token(9.0)),
            ..Props::default()
        });
        let registry = Registry::with_vocabulary(extended_vocabulary(&tree));
        let err =
            validate(&tree, &registry).expect_err("a spacing token is not a typography token");
        assert!(
            err.to_string().contains("props.style"),
            "the refusal must name the offending parameter, got: {err}"
        );
    }
}
