//! Bound view properties: a property is a literal, a `Bind`, or a `Derive`.
//!
//! The vocabulary of `contracts/view-tree.md`'s bound-properties amendment
//! (2026-09-27), from
//! `.agents/notes/proposed/architecture/2026-09-27-bound-slot-table-ui-model.md`
//! §1 and §5: every openable property carries a [`PropVal`] —
//! [`PropVal::Lit`], [`PropVal::Bind`] to a [`SlotKey`] in the host's slot
//! table, or [`PropVal::Derive`] of one expression in the closed set — and
//! the host feeds values back through
//! [`crate::tree::binding_eval::ResolvedTree::apply_slot_changes`].
//!
//! **This is not [`crate::tree::node::Binding`].** That one is the keymap
//! table (spec 010); `ViewNode::bindings` is where a keystroke is claimed and
//! it stays literal for ever. These are *value* bindings, they live on
//! [`ViewNode::bound`], and nothing here can ever name a key.
//!
//! # The closed set, on purpose
//!
//! [`DeriveExpr`] has exactly six operators — `not`, `eq(slot, const)`, `if`,
//! `fmt`, `len`, `sort_by(rows, field_slot, dir_slot)` — the design's §5
//! list. It is evaluated in Rust (`binding_eval.rs`) and it does not grow: a
//! binding that needs more is a Lua reaction (design §7), not a seventh
//! operator. Growing the set is an amendment to `view-tree.md` plus a note,
//! never a drive-by variant.
//!
//! # One source per property
//!
//! A property is either declared literal or declared bound, never both: the
//! wire says which, `validate` refuses the overlap
//! ([`crate::tree::Violation::PropertyWithTwoSources`]), and the fold
//! (`binding_eval.rs`) refuses it too. A bound property's computed value is
//! folded into its literal field by resolution, so a resolved tree is an
//! ordinary literal tree that carries no `bound` map at all — which is what
//! keeps the frame digest of a bound tree identical to the digest of the
//! literal tree carrying the same values.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A slot in the host's table, by name.
///
/// The daemon-side table addresses slots by `SlotId(u32)` after publishing
/// (design §1); Petra never sees those. Petra sees names — `SlotKey` — because
/// it must not depend on the shared-memory crate (design §13), and a name is
/// the one address that survives the two being different objects. The host
/// maps names to whatever its table calls a slot before handing values over.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SlotKey(String);

impl SlotKey {
    /// A key from anything string-like.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The name, as authored.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SlotKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SlotKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for SlotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One keyed record of a [`SlotValue::Rows`] value.
pub type Row = BTreeMap<String, SlotValue>;

/// Keyed records, the shape a list property binds.
pub type Rows = Vec<Row>;

/// One value in the slot table, as much of it as a property can carry.
///
/// Deliberately narrower than the daemon's `Register` scalars (design §3):
/// `Enum` arrives as [`SlotValue::Str`], `I64` and `F64` both as
/// [`SlotValue::Num`], and `Bytes`, `Text` and `Lease` have no property that
/// accepts them, so they have no variant here. A value a property cannot
/// hold is refused by name at the fold, not stored in a catch-all.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotValue {
    /// Text. `Enum` wire values arrive here.
    Str(String),
    /// A number. Both `I64` and `F64` wire values arrive here, and a
    /// property that wants an `i64` count says so when the fold lands it.
    Num(f64),
    /// A flag.
    Bool(bool),
    /// Keyed records. The value `sort_by` reorders and `len` counts.
    Rows(Rows),
}

impl SlotValue {
    /// The value's type, which is what property it may land on.
    #[must_use]
    pub fn prop_type(&self) -> PropType {
        match self {
            Self::Str(_) => PropType::Str,
            Self::Num(_) => PropType::Num,
            Self::Bool(_) => PropType::Bool,
            Self::Rows(_) => PropType::Rows,
        }
    }
}

/// The delta unit: one slot's new value at one version (design §1).
///
/// The same shape the design's `Commit` groups carry; Petra's copy is its own
/// type so the crate stays free of `gorgon-slot-table` (design §13). A batch
/// of changes applies atomically and last-writer-wins under a strictly
/// increasing per-key version — see
/// [`crate::tree::binding_eval::ResolvedTree::apply_slot_changes`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotChange {
    /// Which slot moved.
    pub slot: SlotKey,
    /// The value's version. Applies must be strictly newer than what is
    /// held; a replayed or reordered change is refused, not applied quietly.
    pub version: u32,
    /// The new value.
    pub value: SlotValue,
}

impl SlotChange {
    /// A change of `slot` to `value` at `version`.
    #[must_use]
    pub fn new(slot: impl Into<SlotKey>, version: u32, value: SlotValue) -> Self {
        Self {
            slot: slot.into(),
            version,
            value,
        }
    }
}

/// One openable property, by name — the `(node, property)` half of the
/// reverse index (design §5) and the key a [`BoundProps`] declares under.
///
/// The wire name is this enum's snake-case name and it matches the
/// [`BoundProps`] field it fills, so a spelling that is not openable fails
/// decode rather than being dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropKey {
    /// [`crate::tree::Props::text`] — the string a `text` or `input` draws.
    Text,
    /// [`crate::tree::Semantics::label`] — the accessible name.
    Label,
    /// [`crate::tree::Semantics::value`] — the value a control reports.
    Value,
    /// [`crate::tree::Props::placeholder`] — an empty input's hint.
    Placeholder,
    /// [`crate::tree::Props::image`] — the image source identifier.
    Image,
    /// [`crate::tree::Props::source`] — the row source a `collection` reads.
    Source,
    /// [`crate::tree::Semantics::disabled`] — the greyed-out flag.
    Disabled,
    /// [`crate::tree::Semantics::selected`] — the selection flag.
    Selected,
    /// [`crate::tree::Semantics::expanded`] — the open/closed flag.
    Expanded,
    /// [`crate::tree::Props::opacity`] — the paint multiplier.
    Opacity,
    /// [`crate::tree::Props::total_count`] — a collection's row count.
    TotalCount,
    /// [`crate::tree::Props::items`] — the rows a keyed template renders.
    Items,
}

impl PropKey {
    /// Every openable property, in declaration order.
    pub const ALL: [Self; 12] = [
        Self::Text,
        Self::Label,
        Self::Value,
        Self::Placeholder,
        Self::Image,
        Self::Source,
        Self::Disabled,
        Self::Selected,
        Self::Expanded,
        Self::Opacity,
        Self::TotalCount,
        Self::Items,
    ];

    /// The wire name, identical to the [`BoundProps`] field this fills.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Label => "label",
            Self::Value => "value",
            Self::Placeholder => "placeholder",
            Self::Image => "image",
            Self::Source => "source",
            Self::Disabled => "disabled",
            Self::Selected => "selected",
            Self::Expanded => "expanded",
            Self::Opacity => "opacity",
            Self::TotalCount => "total_count",
            Self::Items => "items",
        }
    }

    /// The value type this property accepts.
    #[must_use]
    pub fn prop_type(self) -> PropType {
        match self {
            Self::Text
            | Self::Label
            | Self::Value
            | Self::Placeholder
            | Self::Image
            | Self::Source => PropType::Str,
            Self::Disabled | Self::Selected | Self::Expanded => PropType::Bool,
            Self::Opacity | Self::TotalCount => PropType::Num,
            Self::Items => PropType::Rows,
        }
    }

    /// What a change to this property costs: the dirty class of design §5.
    ///
    /// Fixed per property kind, as §5 requires — a host never classifies a
    /// change at runtime, it looks the property up here.
    #[must_use]
    pub const fn dirty_class(self) -> DirtyClass {
        match self {
            // *paint* (colour, selected, checked, opacity): damage only.
            Self::Disabled | Self::Selected | Self::Opacity | Self::Image => DirtyClass::Paint,
            // *layout* (text, size, visibility, child count): re-measure the
            // affected subtree.
            Self::Text
            | Self::Label
            | Self::Value
            | Self::Placeholder
            | Self::Source
            | Self::Expanded
            | Self::TotalCount
            | Self::Items => DirtyClass::Layout,
        }
    }
}

/// A property change's dirty class (design §5), the cost a change carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirtyClass {
    /// Appearance only: damage the node's rect and repaint, never
    /// re-measure. §5's *paint* class (colour, selected, checked, opacity);
    /// [`PropKey::Image`] joins it because an image's rect comes from
    /// constraints and its source string is paint payload, not measure
    /// input (`contracts/frame-identity.md` "Paint payload hash").
    Paint,
    /// Changes what the affected subtree measures: re-measure it through a
    /// keyed [`crate::layout::ChangeSet::Nodes`], never
    /// [`crate::layout::ChangeSet::All`] (design §5's *layout* class: text,
    /// size, visibility, child count).
    Layout,
}

impl fmt::Display for PropKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The kinds a property can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropType {
    /// Text.
    Str,
    /// A flag.
    Bool,
    /// A number.
    Num,
    /// Keyed records.
    Rows,
}

impl PropType {
    /// The name a refusal message prints.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Str => "text",
            Self::Bool => "flag",
            Self::Num => "number",
            Self::Rows => "rows",
        }
    }
}

impl fmt::Display for PropType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What one property's value is: a literal, a slot binding, or a derive.
///
/// Serialized by name — `{"lit": v}`, `{"bind": "slot"}`,
/// `{"derive": {"op": …}}` — so a literal tree's encoding is byte-identical
/// to the encoding it had before this type existed, and the pinned golden
/// vectors of `contracts/frame-identity.md` do not move.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropVal {
    /// A declared constant.
    Lit(SlotValue),
    /// The named slot's current value.
    Bind(SlotKey),
    /// A closed expression over slot values. See [`DeriveExpr`].
    Derive(DeriveExpr),
}

/// The closed derive set (design §5), evaluated in Rust.
///
/// `not`, `eq(slot, const)`, `if`, `fmt`, `len`, `sort_by(rows, field_slot,
/// dir_slot)` — and nothing else. Operands in composition position are
/// themselves [`PropVal`]s so `if(eq(…), …, …)` and `fmt("{} items",
/// len(rows))` compose one level; every leaf address is a slot name. The set
/// does not grow into a language (design §5's rule; growing it is an
/// amendment plus a note).
///
/// Evaluation is total and pure over the input map (`binding_eval.rs`):
/// no expression reads a property, so a fold needs one pass over one node and
/// no ordering between a node's own bindings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DeriveExpr {
    /// `not(x)` — the negation of a flag.
    Not {
        /// The flag to negate.
        on: Box<PropVal>,
    },
    /// `eq(slot, const)` — whether the slot currently equals the constant.
    ///
    /// The two sides must be the same value type; a comparison across types
    /// is a refusal naming both, not a quiet `false`.
    Eq {
        /// The slot to compare.
        slot: SlotKey,
        /// The constant to compare it against.
        value: SlotValue,
    },
    /// `if(c, a, b)` — `a` when the condition holds, `b` otherwise.
    ///
    /// Only the taken branch is evaluated, so the untaken one may reference a
    /// slot the caller never had to deliver.
    If {
        /// The condition; a flag when evaluated.
        on: Box<PropVal>,
        /// The value when the condition holds.
        then: Box<PropVal>,
        /// The value when it does not.
        #[serde(rename = "else")]
        else_: Box<PropVal>,
    },
    /// `fmt(template, …)` — the template with each `{name}` replaced by the
    /// argument of that name.
    ///
    /// `{{` and `}}` are literal braces. Placeholders are named, every
    /// declared argument is used exactly once, and both directions of drift
    /// are refusals: a template referencing an argument nobody declared is
    /// as broken as a declared argument no template ever reads.
    Fmt {
        /// The template, e.g. `"{count} items"`.
        format: String,
        /// The arguments, by placeholder name.
        args: BTreeMap<String, PropVal>,
    },
    /// `len(rows)` — how many records a rows slot holds, as a number.
    Len {
        /// The rows slot to count.
        of: SlotKey,
    },
    /// `sort_by(rows, field_slot, dir_slot)` — the rows slot's records,
    /// ordered by the field `field_slot` names, in the direction
    /// `dir_slot` names (`"asc"` or `"desc"`).
    ///
    /// The sort is stable, records missing the field sort before records
    /// holding it, and a field holding rows cannot be compared — only
    /// scalars order.
    SortBy {
        /// The rows slot to order.
        rows: SlotKey,
        /// The slot naming the field to order by.
        field: SlotKey,
        /// The slot naming the direction, `"asc"` or `"desc"`.
        direction: SlotKey,
    },
}

/// Why a [`PropVal`] cannot produce the value a [`PropKey`] accepts.
///
/// Shown at acceptance ([`crate::tree::Violation::IllTypedBinding`]); the
/// same arithmetic backs the fold's value-time refusals, so a tree cannot
/// slip past one boundary and die at the other with a different story.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeFault {
    /// The source produces the wrong type for this property.
    TypeMismatch {
        /// What the property accepts.
        expected: PropType,
        /// What the source produces.
        found: PropType,
    },
    /// `if`'s two branches produce different types, so the expression has no
    /// one type to promise the property.
    BranchesDisagree {
        /// The `then` branch's type.
        then: PropType,
        /// The `else` branch's type.
        other: PropType,
    },
    /// A `fmt` argument produces rows, which no template can print.
    NotFormattable {
        /// What the argument produces.
        found: PropType,
    },
}

impl fmt::Display for ShapeFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeMismatch { expected, found } => {
                write!(
                    f,
                    "this property holds {expected}, the binding produces {found}"
                )
            }
            Self::BranchesDisagree { then, other } => write!(
                f,
                "the `if` branches disagree: `then` produces {then}, `else` produces {other}"
            ),
            Self::NotFormattable { found } => {
                write!(
                    f,
                    "a `fmt` argument produces {found}, which no template can print"
                )
            }
        }
    }
}

/// The type `src` produces, or why it has none it can promise.
///
/// `Ok(None)` is a [`PropVal::Bind`]: the type is whatever the slot table
/// delivers, so it is judged when the value lands, not here. Everything else
/// is decided now.
pub fn result_shape(src: &PropVal) -> Result<Option<PropType>, ShapeFault> {
    match src {
        PropVal::Lit(value) => Ok(Some(value.prop_type())),
        PropVal::Bind(_) => Ok(None),
        PropVal::Derive(expr) => match expr {
            DeriveExpr::Not { on } => {
                flag_shape(on)?;
                Ok(Some(PropType::Bool))
            }
            DeriveExpr::Eq { .. } => Ok(Some(PropType::Bool)),
            DeriveExpr::If { on, then, else_ } => {
                flag_shape(on)?;
                match (result_shape(then)?, result_shape(else_)?) {
                    (None, other) | (other, None) => Ok(other),
                    (Some(a), Some(b)) if a == b => Ok(Some(a)),
                    (Some(a), Some(b)) => Err(ShapeFault::BranchesDisagree { then: a, other: b }),
                }
            }
            DeriveExpr::Fmt { args, .. } => {
                for arg in args.values() {
                    match result_shape(arg)? {
                        None | Some(PropType::Str) | Some(PropType::Num) | Some(PropType::Bool) => {
                        }
                        Some(found) => return Err(ShapeFault::NotFormattable { found }),
                    }
                }
                Ok(Some(PropType::Str))
            }
            DeriveExpr::Len { .. } => Ok(Some(PropType::Num)),
            DeriveExpr::SortBy { .. } => Ok(Some(PropType::Rows)),
        },
    }
}

fn flag_shape(src: &PropVal) -> Result<(), ShapeFault> {
    match result_shape(src)? {
        None | Some(PropType::Bool) => Ok(()),
        Some(found) => Err(ShapeFault::TypeMismatch {
            expected: PropType::Bool,
            found,
        }),
    }
}

/// Whether `src` can fill `prop`, as far as static shape says.
pub fn shape_for(src: &PropVal, prop: PropKey) -> Result<(), ShapeFault> {
    match result_shape(src)? {
        None => Ok(()),
        Some(found) if found == prop.prop_type() => Ok(()),
        Some(found) => Err(ShapeFault::TypeMismatch {
            expected: prop.prop_type(),
            found,
        }),
    }
}

/// A property's declared source, for the properties this tree opens up.
///
/// Twelve `Option<PropVal>`s, one per [`PropKey`], serialized by those exact
/// names with unknown names refused at decode — the contract's "names that
/// are not openable fail decode". `None` is "declared literal" and the
/// literal lives in its ordinary field: one source per property, and the
/// fields below are the only ones that have a second source at all.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundProps {
    /// Binding for [`PropKey::Text`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<PropVal>,
    /// Binding for [`PropKey::Label`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<PropVal>,
    /// Binding for [`PropKey::Value`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<PropVal>,
    /// Binding for [`PropKey::Placeholder`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<PropVal>,
    /// Binding for [`PropKey::Image`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<PropVal>,
    /// Binding for [`PropKey::Source`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PropVal>,
    /// Binding for [`PropKey::Disabled`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<PropVal>,
    /// Binding for [`PropKey::Selected`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<PropVal>,
    /// Binding for [`PropKey::Expanded`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<PropVal>,
    /// Binding for [`PropKey::Opacity`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<PropVal>,
    /// Binding for [`PropKey::TotalCount`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_count: Option<PropVal>,
    /// Binding for [`PropKey::Items`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<PropVal>,
}

impl BoundProps {
    /// Whether nothing is bound here — the state every tree was in before
    /// bindings existed, which is why it serializes away entirely.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.visit(|_, _| {})
    }

    /// The declared source for `prop`, if it has one.
    #[must_use]
    pub fn get(&self, prop: PropKey) -> Option<&PropVal> {
        match prop {
            PropKey::Text => self.text.as_ref(),
            PropKey::Label => self.label.as_ref(),
            PropKey::Value => self.value.as_ref(),
            PropKey::Placeholder => self.placeholder.as_ref(),
            PropKey::Image => self.image.as_ref(),
            PropKey::Source => self.source.as_ref(),
            PropKey::Disabled => self.disabled.as_ref(),
            PropKey::Selected => self.selected.as_ref(),
            PropKey::Expanded => self.expanded.as_ref(),
            PropKey::Opacity => self.opacity.as_ref(),
            PropKey::TotalCount => self.total_count.as_ref(),
            PropKey::Items => self.items.as_ref(),
        }
    }

    /// Declare `source` as the value of `prop`, replacing any declaration.
    pub fn set(&mut self, prop: PropKey, source: PropVal) {
        let slot = match prop {
            PropKey::Text => &mut self.text,
            PropKey::Label => &mut self.label,
            PropKey::Value => &mut self.value,
            PropKey::Placeholder => &mut self.placeholder,
            PropKey::Image => &mut self.image,
            PropKey::Source => &mut self.source,
            PropKey::Disabled => &mut self.disabled,
            PropKey::Selected => &mut self.selected,
            PropKey::Expanded => &mut self.expanded,
            PropKey::Opacity => &mut self.opacity,
            PropKey::TotalCount => &mut self.total_count,
            PropKey::Items => &mut self.items,
        };
        *slot = Some(source);
    }

    /// Walk every declared source, in [`PropKey::ALL`] order.
    ///
    /// Returns whether any declaration was seen, which is what `is_empty`
    /// answers and what the fold's index walk records against.
    fn visit(&self, mut f: impl FnMut(PropKey, &PropVal)) -> bool {
        let mut any = false;
        for prop in PropKey::ALL {
            if let Some(source) = self.get(prop) {
                f(prop, source);
                any = true;
            }
        }
        any
    }
}

/// One `(node, property)` pair that binds a slot — a row of the reverse index
/// (design §5).
///
/// The public half is exactly the pair the design names; `source` and `at`
/// are how the retained tree is re-computed in place, kept crate-private so
/// no caller outside the fold can pair a source with the wrong node.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundSite {
    /// Canonical id of the bound node ([`crate::tree::KeyPath::id`] form).
    pub node: String,
    /// The property whose computed value this binding produces.
    pub prop: PropKey,
    pub(crate) source: PropVal,
    pub(crate) at: Vec<usize>,
}

/// The reverse index: slot name → every `(node, property)` that binds it
/// (design §5).
///
/// "The host keeps a reverse index from slot to a list of `(node, prop)`. A
/// slot change dirties exactly those properties." Built by the fold and read
/// by [`crate::tree::binding_eval::ResolvedTree::apply_slot_changes`]; R-2's
/// keyed invalidation reads the same list to dirty exactly those subtrees.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReverseIndex {
    sites: BTreeMap<SlotKey, Vec<BoundSite>>,
}

impl ReverseIndex {
    /// Every site bound to `slot`, in tree pre-order.
    #[must_use]
    pub fn get(&self, slot: &str) -> &[BoundSite] {
        self.sites
            .get(&SlotKey::new(slot))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Every `(slot, sites)` pair, ordered by slot name.
    pub fn iter(&self) -> impl Iterator<Item = (&SlotKey, &[BoundSite])> {
        self.sites
            .iter()
            .map(|(slot, sites)| (slot, sites.as_slice()))
    }

    /// How many slots have at least one binding.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sites.len()
    }

    /// Whether nothing binds anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }

    pub(crate) fn record(&mut self, slot: SlotKey, site: BoundSite) {
        self.sites.entry(slot).or_default().push(site);
    }

    /// Drop every site at or under the child-index path `at` and take
    /// `with`'s sites in their place, keeping each slot's list in tree
    /// pre-order (a child-index path orders lexicographically in pre-order).
    pub(crate) fn replace_under(&mut self, at: &[usize], with: ReverseIndex) {
        let mut touched: Vec<SlotKey> = Vec::new();
        for (slot, sites) in &mut self.sites {
            let before = sites.len();
            sites.retain(|site| !site.at.starts_with(at));
            if sites.len() != before {
                touched.push(slot.clone());
            }
        }
        for (slot, sites) in with.sites {
            touched.push(slot.clone());
            self.sites.entry(slot).or_default().extend(sites);
        }
        for slot in touched {
            if let Some(sites) = self.sites.get_mut(&slot) {
                sites.sort_by(|a, b| a.at.cmp(&b.at));
            }
        }
        self.sites.retain(|_, sites| !sites.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::{DirtyClass, PropKey};

    /// The design §5 table, property by property, over the closed set.
    ///
    /// Walks [`PropKey::ALL`] rather than naming the twelve, so a thirteenth
    /// property does not compile past here until its dirty class is decided
    /// — the same "no rest pattern" shape `frame-identity.md` pins its leaf
    /// stream with.
    #[test]
    fn every_property_has_the_dirty_class_design_5_gives_it() {
        let class_of = |key: PropKey| key.dirty_class();
        let layout = [
            PropKey::Text,
            PropKey::Label,
            PropKey::Value,
            PropKey::Placeholder,
            PropKey::Source,
            PropKey::Expanded,
            PropKey::TotalCount,
            PropKey::Items,
        ];
        let paint = [
            PropKey::Disabled,
            PropKey::Selected,
            PropKey::Opacity,
            PropKey::Image,
        ];
        for key in PropKey::ALL {
            let expected = if layout.contains(&key) {
                DirtyClass::Layout
            } else {
                assert!(paint.contains(&key), "{key} is in neither class");
                DirtyClass::Paint
            };
            assert_eq!(class_of(key), expected, "{key}");
        }
    }

    /// The wire names `contracts/view-tree.md` pins: `{"lit": v}`,
    /// `{"bind": "slot"}`, `{"derive": {"op": …}}` — lowercase, which is what
    /// a Lua table spells and what the shell decodes.
    #[test]
    fn a_prop_val_serializes_by_the_contract_names() {
        use super::{DeriveExpr, PropVal, SlotKey, SlotValue};
        let wire = |v: &PropVal| serde_json::to_string(v).expect("encode");
        assert_eq!(
            wire(&PropVal::Lit(SlotValue::Bool(true))),
            r#"{"lit":{"bool":true}}"#
        );
        assert_eq!(wire(&PropVal::Bind(SlotKey::new("on"))), r#"{"bind":"on"}"#);
        let derive = PropVal::Derive(DeriveExpr::Len {
            of: SlotKey::new("rows"),
        });
        assert_eq!(wire(&derive), r#"{"derive":{"op":"len","of":"rows"}}"#);
        let back: PropVal = serde_json::from_str(r#"{"bind":"on"}"#).expect("decode");
        assert_eq!(back, PropVal::Bind(SlotKey::new("on")));
    }
}
