//! Resolving bound properties: the fold, the closed expression evaluator,
//! and the retained state `Host::apply_slot_changes` drives.
//!
//! Resolution is one pure pass over one tree: every openable property's
//! [`PropVal`](super::binding::PropVal) is evaluated against a map of slot
//! values and folded into its ordinary literal field, so the output of
//! [`ResolvedTree::refold`] is an ordinary literal tree — no `bound` map, no
//! new wire shape, and a frame digest identical to the literal tree carrying
//! the same values (`contracts/frame-identity.md`). Nothing here reads a
//! property: every expression addresses slots
//! ([`super::binding::DeriveExpr`] leaves are slot names), so one node's
//! bindings fold in any order and a fold needs no second pass.
//!
//! Every refusal here names its cause and the site that caused it. A slot
//! with no value, a value of the wrong type, a malformed template, an
//! unusable sort direction — each is an error, never a defaulted field or a
//! silent skip.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use super::binding::{
    BoundProps, BoundSite, DeriveExpr, PropKey, PropType, PropVal, ReverseIndex, Rows, SlotChange,
    SlotKey, SlotValue,
};
use super::key::KeyPath;
use super::node::ViewNode;

/// The values a fold resolves bindings against: slot name → current value.
///
/// What the host reads out of its table (and its predicted overlay) before
/// asking for a fold. A [`super::binding::PropVal::Bind`] naming a slot this
/// map does not hold is a refusal, not a blank: the shell delivers the
/// table's snapshot first (design §8), and a tree folded against missing
/// values would draw a picture that lies.
pub type ResolveInputs = BTreeMap<String, SlotValue>;

/// What a source fills: a node property, or an openable component
/// parameter (`ComponentRef::bound`). A refusal names it after the site id,
/// so `/ui:7/tg.params.selected` reads apart from `/ui:7/tg.selected`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindTarget {
    /// A node property.
    Prop(PropKey),
    /// A component parameter, by the name the openable table declares.
    Param(&'static str),
}

impl From<PropKey> for BindTarget {
    fn from(value: PropKey) -> Self {
        Self::Prop(value)
    }
}

impl fmt::Display for BindTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prop(prop) => write!(f, "{prop}"),
            Self::Param(param) => write!(f, "params.{param}"),
        }
    }
}

/// Why a fold refused, named to the property that caused it.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolveError {
    /// A binding names a slot the input map does not hold.
    UnknownSlot {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// The slot nothing delivered.
        slot: String,
    },
    /// A value reached a property that cannot hold it.
    TypeMismatch {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// What produced the value ("the slot", "fmt", "sort_by", …).
        via: &'static str,
        /// What the property holds.
        expected: super::binding::PropType,
        /// What arrived.
        found: super::binding::PropType,
    },
    /// The property declares a literal and a binding. One source per
    /// property; `validate` refuses this too, and the fold refuses it for
    /// the paths that never reach `validate` with declarations intact.
    TwoSources {
        /// Canonical id of the bound node.
        node: String,
        /// The property claiming two sources.
        prop: PropKey,
    },
    /// A `fmt` template is malformed.
    BadFormat {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// The template as authored.
        format: String,
        /// What is wrong with it.
        reason: String,
    },
    /// A `fmt` template reads `{name}` and no argument of that name exists.
    MissingFormatArgument {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// The placeholder name.
        name: String,
    },
    /// A `fmt` argument no placeholder ever reads.
    UnusedFormatArgument {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// The argument name.
        name: String,
    },
    /// `sort_by`'s direction slot holds something other than `"asc"` or
    /// `"desc"`.
    SortDirection {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// What the direction slot held.
        got: String,
    },
    /// `sort_by`'s field holds rows; only scalars order.
    SortFieldHoldsRows {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// The field name the field slot named.
        field: String,
    },
    /// `total_count` wants a whole, non-negative count and got something
    /// else.
    NotWhole {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// What the number was.
        got: f64,
    },
    /// `opacity` holds an `f32` and the value does not fit in one finitely.
    NotFinite {
        /// Canonical id of the bound node.
        node: String,
        /// The property (or component parameter) being folded.
        prop: BindTarget,
        /// What the number was.
        got: f64,
    },
    /// The deferred half of `TextRunsDoNotCoverTheText`: acceptance cannot
    /// judge `runs` against a text that does not exist yet, so the fold that
    /// lands the text judges the coverage instead.
    RunsDoNotCoverText {
        /// Canonical id of the bound node.
        node: String,
        /// What the runs claim, in bytes.
        claimed: usize,
        /// What the resolved text holds, in bytes.
        text_len: usize,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSlot { node, prop, slot } => write!(
                f,
                "{node}.{prop}: slot `{slot}` has no value — deliver it through \
                 `apply_slot_changes` before this tree is folded"
            ),
            Self::TypeMismatch {
                node,
                prop,
                via,
                expected,
                found,
            } => write!(
                f,
                "{node}.{prop}: {via} produces {found}, this property holds {expected}"
            ),
            Self::TwoSources { node, prop } => write!(
                f,
                "{node}.{prop}: a property has exactly one source, and this one declares a \
                 literal and a binding"
            ),
            Self::BadFormat {
                node,
                prop,
                format,
                reason,
            } => write!(
                f,
                "{node}.{prop}: `fmt` template {format:?} is malformed: {reason}"
            ),
            Self::MissingFormatArgument { node, prop, name } => write!(
                f,
                "{node}.{prop}: `fmt` reads {{{name}}} but declares no argument of that name"
            ),
            Self::UnusedFormatArgument { node, prop, name } => write!(
                f,
                "{node}.{prop}: `fmt` declares argument `{name}` that no placeholder reads"
            ),
            Self::SortDirection { node, prop, got } => write!(
                f,
                "{node}.{prop}: `sort_by` direction slot holds {got:?}; it must hold \"asc\" or \
                 \"desc\""
            ),
            Self::SortFieldHoldsRows { node, prop, field } => write!(
                f,
                "{node}.{prop}: `sort_by` field `{field}` holds rows; only scalars order"
            ),
            Self::NotWhole { node, prop, got } => {
                write!(f, "{node}.{prop}: {got} is not a whole non-negative count")
            }
            Self::NotFinite { node, prop, got } => {
                write!(
                    f,
                    "{node}.{prop}: {got} is not finite in this property's number type"
                )
            }
            Self::RunsDoNotCoverText {
                node,
                claimed,
                text_len,
            } => write!(
                f,
                "{node}.text: the runs claim {claimed} bytes but the resolved text is {text_len} \
                 bytes"
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Why a [`ResolvedTree::apply_slot_changes`] refused a batch.
#[derive(Clone, Debug, PartialEq)]
pub enum ApplyError {
    /// A change is not strictly newer than the value already held, or its
    /// batch orders two versions of one slot backwards. The whole batch is
    /// refused: a half-applied batch is a picture nobody ordered.
    StaleVersion {
        /// The slot that refused.
        slot: String,
        /// The version offered.
        offered: u32,
        /// The version already held.
        held: u32,
    },
    /// The reverse index names a node the retained tree does not hold. The
    /// map and the tree are rebuilt together, so this is an engine bug, and
    /// it is reported rather than skipped.
    SiteLost {
        /// Canonical id the index named.
        node: String,
        /// The property it named.
        prop: PropKey,
    },
    /// Re-computing an affected property refused.
    Resolve(ResolveError),
    /// Re-expanding a component whose bound parameter moved refused: the
    /// constructor rejected the new parameter table, or the regrown tree
    /// failed acceptance. The whole batch is refused with it.
    Regrow {
        /// Canonical id of the component reference, relative to its tree.
        component: String,
        /// Why, as the constructor or `validate` said it.
        reason: String,
    },
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleVersion {
                slot,
                offered,
                held,
            } => write!(
                f,
                "slot `{slot}` already holds version {held}; version {offered} is not newer and \
                 the batch was refused whole"
            ),
            Self::SiteLost { node, prop } => write!(
                f,
                "the reverse index names {node}.{prop} but the retained tree holds no such node; \
                 the map and the tree are rebuilt together, so this is an engine bug"
            ),
            Self::Resolve(err) => write!(f, "re-computing a bound property refused: {err}"),
            Self::Regrow { component, reason } => write!(
                f,
                "re-expanding component {component} against the batch refused, and the batch was \
                 refused whole: {reason}"
            ),
        }
    }
}

impl std::error::Error for ApplyError {}

impl From<ResolveError> for ApplyError {
    fn from(value: ResolveError) -> Self {
        Self::Resolve(value)
    }
}

/// What produced a value, for the message a refusal prints.
fn via(src: &PropVal) -> &'static str {
    match src {
        PropVal::Lit(_) => "the literal",
        PropVal::Bind(_) => "the slot",
        PropVal::Derive(expr) => match expr {
            DeriveExpr::Not { .. } => "not",
            DeriveExpr::Eq { .. } => "eq",
            DeriveExpr::If { .. } => "if",
            DeriveExpr::Fmt { .. } => "fmt",
            DeriveExpr::Len { .. } => "len",
            DeriveExpr::SortBy { .. } => "sort_by",
        },
    }
}

/// Every slot `src` reads, for the reverse index: a change to any of them
/// re-computes the property this source fills.
pub(crate) fn slots_read(src: &PropVal, out: &mut Vec<SlotKey>) {
    match src {
        PropVal::Lit(_) => {}
        PropVal::Bind(slot) => out.push(slot.clone()),
        PropVal::Derive(expr) => match expr {
            DeriveExpr::Not { on } => slots_read(on, out),
            DeriveExpr::Eq { slot, .. } => out.push(slot.clone()),
            DeriveExpr::If { on, then, else_ } => {
                slots_read(on, out);
                slots_read(then, out);
                slots_read(else_, out);
            }
            DeriveExpr::Fmt { args, .. } => {
                for arg in args.values() {
                    slots_read(arg, out);
                }
            }
            DeriveExpr::Len { of } => out.push(of.clone()),
            DeriveExpr::SortBy {
                rows,
                field,
                direction,
            } => {
                out.push(rows.clone());
                out.push(field.clone());
                out.push(direction.clone());
            }
        },
    }
}

fn lookup<'v>(
    node: &str,
    prop: BindTarget,
    slot: &SlotKey,
    inputs: &'v ResolveInputs,
) -> Result<&'v SlotValue, ResolveError> {
    inputs
        .get(slot.as_str())
        .ok_or_else(|| ResolveError::UnknownSlot {
            node: node.to_owned(),
            prop,
            slot: slot.as_str().to_owned(),
        })
}

/// Evaluate `src` to a value. Total and pure over `inputs`.
pub(crate) fn eval_source(
    node: &str,
    prop: BindTarget,
    src: &PropVal,
    inputs: &ResolveInputs,
) -> Result<SlotValue, ResolveError> {
    match src {
        PropVal::Lit(value) => Ok(value.clone()),
        PropVal::Bind(slot) => Ok(lookup(node, prop, slot, inputs)?.clone()),
        PropVal::Derive(expr) => eval_expr(node, prop, expr, inputs),
    }
}

fn eval_expr(
    node: &str,
    prop: BindTarget,
    expr: &DeriveExpr,
    inputs: &ResolveInputs,
) -> Result<SlotValue, ResolveError> {
    match expr {
        DeriveExpr::Not { on } => {
            let value = eval_source(node, prop, on, inputs)?;
            match value {
                SlotValue::Bool(flag) => Ok(SlotValue::Bool(!flag)),
                other => Err(type_mismatch(node, prop, "not", PropType::Bool, other)),
            }
        }
        DeriveExpr::Eq { slot, value } => {
            let held = lookup(node, prop, slot, inputs)?;
            if held.prop_type() != value.prop_type() {
                return Err(type_mismatch(
                    node,
                    prop,
                    "eq",
                    value.prop_type(),
                    held.clone(),
                ));
            }
            Ok(SlotValue::Bool(held == value))
        }
        DeriveExpr::If { on, then, else_ } => {
            let value = eval_source(node, prop, on, inputs)?;
            match value {
                SlotValue::Bool(true) => eval_source(node, prop, then, inputs),
                SlotValue::Bool(false) => eval_source(node, prop, else_, inputs),
                other => Err(type_mismatch(node, prop, "if", PropType::Bool, other)),
            }
        }
        DeriveExpr::Fmt { format, args } => {
            let mut values = BTreeMap::new();
            for (name, arg) in args {
                values.insert(name.clone(), eval_source(node, prop, arg, inputs)?);
            }
            render(node, prop, format, &values).map(SlotValue::Str)
        }
        DeriveExpr::Len { of } => match lookup(node, prop, of, inputs)? {
            SlotValue::Rows(rows) => Ok(SlotValue::Num(rows.len() as f64)),
            other => Err(type_mismatch(
                node,
                prop,
                "len",
                PropType::Rows,
                other.clone(),
            )),
        },
        DeriveExpr::SortBy {
            rows,
            field,
            direction,
        } => {
            let mut rows = match lookup(node, prop, rows, inputs)? {
                SlotValue::Rows(rows) => rows.clone(),
                other => {
                    return Err(type_mismatch(
                        node,
                        prop,
                        "sort_by",
                        PropType::Rows,
                        other.clone(),
                    ));
                }
            };
            let field = match lookup(node, prop, field, inputs)? {
                SlotValue::Str(name) => name.clone(),
                other => {
                    return Err(type_mismatch(
                        node,
                        prop,
                        "sort_by",
                        PropType::Str,
                        other.clone(),
                    ));
                }
            };
            let descending = match lookup(node, prop, direction, inputs)? {
                SlotValue::Str(dir) if dir == "asc" => false,
                SlotValue::Str(dir) if dir == "desc" => true,
                other => {
                    return Err(match other {
                        SlotValue::Str(got) => ResolveError::SortDirection {
                            node: node.to_owned(),
                            prop,
                            got: got.clone(),
                        },
                        value => type_mismatch(node, prop, "sort_by", PropType::Str, value.clone()),
                    });
                }
            };
            sort_rows(node, prop, &mut rows, &field, descending)?;
            Ok(SlotValue::Rows(rows))
        }
    }
}

pub(crate) fn type_mismatch(
    node: &str,
    prop: BindTarget,
    via: &'static str,
    expected: super::binding::PropType,
    value: SlotValue,
) -> ResolveError {
    ResolveError::TypeMismatch {
        node: node.to_owned(),
        prop,
        via,
        expected,
        found: value.prop_type(),
    }
}

/// Order two sort keys: absent first, then by type rank, then within type.
fn cmp_keys(a: Option<&SlotValue>, b: Option<&SlotValue>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => {
            let rank = |v: &SlotValue| match v {
                SlotValue::Bool(_) => 0,
                SlotValue::Num(_) => 1,
                SlotValue::Str(_) => 2,
                SlotValue::Rows(_) => 3,
            };
            rank(a).cmp(&rank(b)).then_with(|| match (a, b) {
                (SlotValue::Bool(a), SlotValue::Bool(b)) => a.cmp(b),
                (SlotValue::Num(a), SlotValue::Num(b)) => a.total_cmp(b),
                (SlotValue::Str(a), SlotValue::Str(b)) => a.cmp(b),
                (SlotValue::Rows(a), SlotValue::Rows(b)) => a.len().cmp(&b.len()),
                _ => Ordering::Equal,
            })
        }
    }
}

fn sort_rows(
    node: &str,
    prop: BindTarget,
    rows: &mut Rows,
    field: &str,
    descending: bool,
) -> Result<(), ResolveError> {
    for row in rows.iter() {
        if matches!(row.get(field), Some(SlotValue::Rows(_))) {
            return Err(ResolveError::SortFieldHoldsRows {
                node: node.to_owned(),
                prop,
                field: field.to_owned(),
            });
        }
    }
    rows.sort_by(|a, b| {
        let ord = cmp_keys(a.get(field), b.get(field));
        if descending { ord.reverse() } else { ord }
    });
    Ok(())
}

/// The `fmt` core: substitute `{name}` placeholders from evaluated `args`.
fn render(
    node: &str,
    prop: BindTarget,
    format: &str,
    args: &BTreeMap<String, SlotValue>,
) -> Result<String, ResolveError> {
    let bad = |reason: &str| ResolveError::BadFormat {
        node: node.to_owned(),
        prop,
        format: format.to_owned(),
        reason: reason.to_owned(),
    };
    let mut out = String::with_capacity(format.len());
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '}' => {
                return Err(bad(
                    "unmatched `}` — `}}` is how a literal brace is written",
                ));
            }
            '{' => {
                let mut name = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == '}' {
                        closed = true;
                        break;
                    }
                    name.push(ch);
                }
                if !closed {
                    return Err(bad("a placeholder is never closed"));
                }
                if name.is_empty() {
                    return Err(bad("placeholders are named — `{name}`, not `{}`"));
                }
                let Some(value) = args.get(&name) else {
                    return Err(ResolveError::MissingFormatArgument {
                        node: node.to_owned(),
                        prop,
                        name,
                    });
                };
                used.insert(name);
                match value {
                    SlotValue::Str(text) => out.push_str(text),
                    SlotValue::Num(number) => out.push_str(&number.to_string()),
                    SlotValue::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
                    SlotValue::Rows(_) => {
                        return Err(type_mismatch(
                            node,
                            prop,
                            "fmt",
                            super::binding::PropType::Str,
                            value.clone(),
                        ));
                    }
                }
            }
            other => out.push(other),
        }
    }
    for name in args.keys() {
        if !used.contains(name) {
            return Err(ResolveError::UnusedFormatArgument {
                node: node.to_owned(),
                prop,
                name: name.clone(),
            });
        }
    }
    Ok(out)
}

/// Land one computed value in the node field `prop` names.
fn land(
    node: &mut ViewNode,
    prop: PropKey,
    value: SlotValue,
    site: &str,
    via: &'static str,
) -> Result<(), ResolveError> {
    match (prop, value) {
        (PropKey::Text, SlotValue::Str(text)) => node.props.text = Some(text),
        (PropKey::Label, SlotValue::Str(text)) => node.semantics.label = Some(text),
        (PropKey::Value, SlotValue::Str(text)) => node.semantics.value = Some(text),
        (PropKey::Placeholder, SlotValue::Str(text)) => node.props.placeholder = Some(text),
        (PropKey::Image, SlotValue::Str(text)) => node.props.image = Some(text),
        (PropKey::Source, SlotValue::Str(text)) => node.props.source = Some(text),
        (PropKey::Disabled, SlotValue::Bool(flag)) => node.semantics.disabled = flag,
        (PropKey::Selected, SlotValue::Bool(flag)) => node.semantics.selected = flag,
        (PropKey::Expanded, SlotValue::Bool(flag)) => node.semantics.expanded = Some(flag),
        (PropKey::Opacity, SlotValue::Num(number)) => {
            let opacity = number as f32;
            if !opacity.is_finite() {
                return Err(ResolveError::NotFinite {
                    node: site.to_owned(),
                    prop: prop.into(),
                    got: number,
                });
            }
            node.props.opacity = Some(opacity);
        }
        (PropKey::TotalCount, SlotValue::Num(number)) => {
            if !(number.is_finite() && number >= 0.0 && number.fract() == 0.0) {
                return Err(ResolveError::NotWhole {
                    node: site.to_owned(),
                    prop: prop.into(),
                    got: number,
                });
            }
            node.props.total_count = Some(number as usize);
        }
        (PropKey::Items, SlotValue::Rows(rows)) => node.props.items = Some(rows),
        (_, value) => {
            return Err(type_mismatch(
                site,
                prop.into(),
                via,
                prop.prop_type(),
                value,
            ));
        }
    }
    Ok(())
}

/// Whether `node` declares a literal for `prop`.
///
/// A `false` flag is *not* a declaration: `Semantics::disabled` and
/// `Semantics::selected` serialize away when false, so `false` is the wire's
/// own "not declared" and a binding may supply the value. `validate` runs
/// this too, to refuse a two-sourced property at acceptance.
pub(crate) fn literal_declared(node: &ViewNode, prop: PropKey) -> bool {
    match prop {
        PropKey::Text => node.props.text.is_some(),
        PropKey::Label => node.semantics.label.is_some(),
        PropKey::Value => node.semantics.value.is_some(),
        PropKey::Placeholder => node.props.placeholder.is_some(),
        PropKey::Image => node.props.image.is_some(),
        PropKey::Source => node.props.source.is_some(),
        PropKey::Disabled => node.semantics.disabled,
        PropKey::Selected => node.semantics.selected,
        PropKey::Expanded => node.semantics.expanded.is_some(),
        PropKey::Opacity => node.props.opacity.is_some(),
        PropKey::TotalCount => node.props.total_count.is_some(),
        PropKey::Items => node.props.items.is_some(),
    }
}

/// One node's declared sources, in [`PropKey::ALL`] order.
fn sources_of(node: &ViewNode) -> Vec<(PropKey, PropVal)> {
    let mut out = Vec::new();
    for prop in PropKey::ALL {
        if let Some(source) = node.bound.get(prop) {
            out.push((prop, source.clone()));
        }
    }
    out
}

/// Fold one tree into literals against `inputs`, recording every binding in
/// `index` and walking `path`/`at` to the canonical ids and child-index
/// paths the index carries.
fn fold_tree(
    node: &ViewNode,
    path: &mut KeyPath,
    at: &mut Vec<usize>,
    inputs: &ResolveInputs,
    index: &mut ReverseIndex,
) -> Result<ViewNode, ResolveError> {
    path.push(node.key.clone());
    let site = path.id();
    let mut folded = node.clone();
    let sources = sources_of(node);
    for (prop, _) in &sources {
        if literal_declared(node, *prop) {
            path.pop();
            return Err(ResolveError::TwoSources {
                node: site,
                prop: *prop,
            });
        }
    }
    for (prop, source) in &sources {
        let value = eval_source(&site, (*prop).into(), source, inputs)?;
        land(&mut folded, *prop, value, &site, via(source))?;
        let mut slots = Vec::new();
        slots_read(source, &mut slots);
        for slot in slots {
            index.record(
                slot,
                BoundSite {
                    node: site.clone(),
                    prop: *prop,
                    source: source.clone(),
                    at: at.clone(),
                },
            );
        }
    }
    folded.bound = BoundProps::default();
    if sources.iter().any(|(prop, _)| *prop == PropKey::Text) && !node.props.runs.is_empty() {
        let claimed: usize = node.props.runs.iter().map(|run| run.len).sum();
        let text_len = folded.props.text.as_deref().unwrap_or_default().len();
        if claimed != text_len {
            path.pop();
            return Err(ResolveError::RunsDoNotCoverText {
                node: site,
                claimed,
                text_len,
            });
        }
    }
    let mut children = Vec::with_capacity(node.children.len());
    for (ix, child) in node.children.iter().enumerate() {
        at.push(ix);
        let folded_child = fold_tree(child, path, at, inputs, index);
        at.pop();
        children.push(Arc::new(folded_child?));
    }
    folded.children = children;
    path.pop();
    Ok(folded)
}

fn node_at_mut<'t>(root: &'t mut ViewNode, at: &[usize]) -> Option<&'t mut ViewNode> {
    let mut node = root;
    for &ix in at {
        node = Arc::make_mut(node.children.get_mut(ix)?);
    }
    Some(node)
}

fn node_at<'t>(root: &'t ViewNode, at: &[usize]) -> Option<&'t ViewNode> {
    let mut node = root;
    for &ix in at {
        node = node.children.get(ix)?.as_ref();
    }
    Some(node)
}

/// Whether every change in `batch` can land against the versions in `held`
/// (slot name → version on file).
///
/// The one definition of "newer" for slot data: strictly greater than what is
/// held, and strictly greater than every earlier change in the same batch for
/// the same slot. A host holding several trees checks them all before landing
/// any of it, one commit group applied atomically (design §1). Read-only;
/// nothing lands here.
pub fn check_slot_versions(
    held: &BTreeMap<String, u32>,
    changes: &[SlotChange],
) -> Result<(), ApplyError> {
    let mut effective: BTreeMap<&str, u32> = BTreeMap::new();
    for change in changes {
        let key = change.slot.as_str();
        let held = effective
            .get(key)
            .copied()
            .or_else(|| held.get(key).copied())
            .unwrap_or(0);
        if change.version <= held {
            return Err(ApplyError::StaleVersion {
                slot: key.to_owned(),
                offered: change.version,
                held,
            });
        }
        effective.insert(key, change.version);
    }
    Ok(())
}

/// What a [`SlotChange`] applied, and to which version.
#[derive(Clone, Debug, PartialEq)]
struct Held {
    value: SlotValue,
    version: u32,
}

/// The retained binding state: the folded tree, the reverse index, and the
/// slot values with their versions.
///
/// Built by [`ResolvedTree::resolve`] (or [`ResolvedTree::new`] and then
/// [`ResolvedTree::refold`]) and driven by
/// [`ResolvedTree::apply_slot_changes`]. Values and versions survive a
/// re-fold — a host that re-builds its tree every frame (design §5's
/// structure-patch model) keeps one of these across frames and folds each new
/// tree into it.
#[derive(Clone, Debug)]
pub struct ResolvedTree {
    tree: ViewNode,
    index: ReverseIndex,
    values: BTreeMap<String, Held>,
}

impl Default for ResolvedTree {
    /// Nothing folded yet. [`ResolvedTree::refold`] replaces the stand-in
    /// root, and values delivered before the first fold survive into it.
    fn default() -> Self {
        Self::new()
    }
}

impl ResolvedTree {
    /// Nothing folded yet — see [`Default`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            tree: ViewNode::new(super::node::NodeKind::Stack, "petra-unfolded"),
            index: ReverseIndex::default(),
            values: BTreeMap::new(),
        }
    }

    /// Fold `declared` against `inputs`, keeping nothing.
    pub fn resolve(declared: &ViewNode, inputs: &ResolveInputs) -> Result<Self, ResolveError> {
        let mut index = ReverseIndex::default();
        let mut path = KeyPath::root();
        let tree = fold_tree(declared, &mut path, &mut Vec::new(), inputs, &mut index)?;
        let values = inputs
            .iter()
            .map(|(slot, value)| {
                (
                    slot.clone(),
                    Held {
                        value: value.clone(),
                        version: 0,
                    },
                )
            })
            .collect();
        Ok(Self {
            tree,
            index,
            values,
        })
    }

    /// Fold a new declared tree in place of the last one, keeping values and
    /// versions.
    ///
    /// Atomic: a refusal leaves the previous tree, index and values exactly
    /// as they were, so a refused re-fold shows the last good picture and the
    /// refusal side by side rather than half a new state.
    pub fn refold(&mut self, declared: &ViewNode) -> Result<(), ResolveError> {
        let inputs = self.inputs();
        let mut index = ReverseIndex::default();
        let mut path = KeyPath::root();
        let tree = fold_tree(declared, &mut path, &mut Vec::new(), &inputs, &mut index)?;
        self.tree = tree;
        self.index = index;
        Ok(())
    }

    /// Fold `declared` in place of the subtree at child-index path `at`,
    /// keeping values, versions and every binding outside that subtree.
    ///
    /// What a re-expanded component lands through: the regrown subtree is
    /// folded against the held values, grafted onto the subtree it replaces
    /// (every unchanged descendant keeps its `Arc`, so its placements and
    /// Merkle hash are reused), its sites replace the old subtree's in the
    /// reverse index, and nothing outside it is re-folded. Atomic — a
    /// refusal (a slot the regrown subtree reads with no value, a path the
    /// tree does not hold) leaves the tree and the index as they were.
    ///
    /// Returns the canonical ids (this tree's own id space) of the nodes
    /// whose content changed, in pre-order: what a host names in
    /// `ChangeSet::Nodes`. Empty when the regrowth drew the same subtree.
    pub fn splice(&mut self, at: &[usize], declared: &ViewNode) -> Result<Vec<String>, ApplyError> {
        let lost = || ApplyError::Regrow {
            component: format!("{at:?}"),
            reason: "the child-index path names no node in the retained tree; the splice path \
                     and the tree are built together, so this is an engine bug"
                .to_owned(),
        };
        let mut path = KeyPath::root();
        if let Some((_, parents)) = at.split_last() {
            let mut node = &self.tree;
            path.push(node.key.clone());
            for &ix in parents {
                node = node.children.get(ix).ok_or_else(lost)?;
                path.push(node.key.clone());
            }
            node_at(&self.tree, at).ok_or_else(lost)?;
        }
        let inputs = self.inputs();
        let mut index = ReverseIndex::default();
        let folded = fold_tree(declared, &mut path, &mut at.to_vec(), &inputs, &mut index)?;
        let mut changed = Vec::new();
        let grafted = match at.split_last() {
            None => {
                let old = Arc::new(self.tree.clone());
                let kept = super::graft::graft(Some(&old), folded, &mut path, &mut changed);
                Arc::unwrap_or_clone(kept)
            }
            Some((&last, parents)) => {
                let parent = node_at(&self.tree, parents).ok_or_else(lost)?;
                let old = parent.children.get(last).ok_or_else(lost)?;
                let kept = super::graft::graft(Some(old), folded, &mut path, &mut changed);
                if Arc::ptr_eq(&kept, old) {
                    // Nothing moved: leave every `Arc` on the path alone, so
                    // the ancestors keep their identity too.
                    self.index.replace_under(at, index);
                    return Ok(changed);
                }
                let parent = node_at_mut(&mut self.tree, parents).ok_or_else(lost)?;
                parent.children[last] = kept;
                self.index.replace_under(at, index);
                return Ok(changed);
            }
        };
        self.tree = grafted;
        self.index.replace_under(at, index);
        Ok(changed)
    }

    /// The folded tree: literals only, no `bound` map.
    #[must_use]
    pub fn tree(&self) -> &ViewNode {
        &self.tree
    }

    /// The reverse index: slot name → every `(node, property)` that binds it.
    #[must_use]
    pub fn index(&self) -> &ReverseIndex {
        &self.index
    }

    /// The value held for `slot`, if any has been delivered.
    #[must_use]
    pub fn value(&self, slot: &str) -> Option<&SlotValue> {
        self.values.get(slot).map(|held| &held.value)
    }

    /// The version held for `slot`, if any has been delivered.
    #[must_use]
    pub fn version(&self, slot: &str) -> Option<u32> {
        self.values.get(slot).map(|held| held.version)
    }

    /// The current values, as a fold sees them — what a bound component
    /// re-expands against once a batch has landed.
    #[must_use]
    pub fn inputs(&self) -> ResolveInputs {
        self.values
            .iter()
            .map(|(slot, held)| (slot.clone(), held.value.clone()))
            .collect()
    }

    /// Whether every change in `batch` can land against the versions this
    /// tree holds.
    ///
    /// The validation half of [`Self::apply_slot_changes`], split out so a
    /// host holding several trees can check them all before landing any of
    /// it — one commit group applied atomically across every retained tree
    /// (design §1). Read-only; nothing lands here.
    pub fn check_slot_changes(&self, changes: &[SlotChange]) -> Result<(), ApplyError> {
        let versions: BTreeMap<String, u32> = self
            .values
            .iter()
            .map(|(slot, held)| (slot.clone(), held.version))
            .collect();
        check_slot_versions(&versions, changes)
    }

    /// Apply a batch of slot changes: values first, then exactly the
    /// properties the reverse index names for the slots that moved.
    ///
    /// Last-writer-wins under a strictly increasing per-key version (design
    /// §1). The batch is atomic — one stale or out-of-order change refuses
    /// the whole batch ([`Self::check_slot_changes`] runs before anything
    /// lands), and a batch whose values cannot be landed (a type that broke a
    /// binding) refuses the whole batch too: every landing is proven on the
    /// side before one of them is written. The answer is the
    /// `(node, property)` list that was re-computed, deduplicated and
    /// sorted. A version folded in through [`ResolvedTree::resolve`]'s inputs
    /// counts as 0, so delivered changes start at 1.
    pub fn apply_slot_changes(
        &mut self,
        changes: &[SlotChange],
    ) -> Result<Vec<(String, PropKey)>, ApplyError> {
        self.check_slot_changes(changes)?;
        // The values the batch leaves behind — what every re-computation
        // judges against, whether or not its own slot moved.
        let mut inputs = self.inputs();
        for change in changes {
            inputs.insert(change.slot.as_str().to_owned(), change.value.clone());
        }

        let mut seen: BTreeSet<(String, PropKey)> = BTreeSet::new();
        let mut affected: Vec<(String, PropKey)> = Vec::new();
        let mut recomputed: Vec<BoundSite> = Vec::new();
        for change in changes {
            for site in self.index.get(change.slot.as_str()) {
                if seen.insert((site.node.clone(), site.prop)) {
                    affected.push((site.node.clone(), site.prop));
                    recomputed.push(site.clone());
                }
            }
        }
        // Prove every landing before one is written. `land` judges the value
        // against the node (types, whole counts, run coverage) and writes the
        // field it names, so it runs against a scratch copy here and the
        // real nodes only once all of them have agreed to land.
        let mut planned: Vec<(BoundSite, SlotValue)> = Vec::new();
        for site in &recomputed {
            let held = node_at(&self.tree, &site.at).ok_or_else(|| ApplyError::SiteLost {
                node: site.node.clone(),
                prop: site.prop,
            })?;
            let value = eval_source(&site.node, site.prop.into(), &site.source, &inputs)?;
            let mut scratch = held.clone();
            land(
                &mut scratch,
                site.prop,
                value.clone(),
                &site.node,
                via(&site.source),
            )?;
            planned.push((site.clone(), value));
        }
        // Commit. Phase one proved these landings against the same node
        // content and the same values; an error here would mean `land` is not
        // deterministic in what it judges, and it is reported rather than
        // swallowed.
        for change in changes {
            self.values.insert(
                change.slot.as_str().to_owned(),
                Held {
                    value: change.value.clone(),
                    version: change.version,
                },
            );
        }
        for (site, value) in planned {
            let node =
                node_at_mut(&mut self.tree, &site.at).ok_or_else(|| ApplyError::SiteLost {
                    node: site.node.clone(),
                    prop: site.prop,
                })?;
            land(node, site.prop, value, &site.node, via(&site.source))?;
        }
        affected.sort();
        Ok(affected)
    }
}

/// Whether any node in `root` declares a binding or derivation.
///
/// The cheap gate the host asks before folding: a tree with no bindings folds
/// to itself, so nothing pays for a fold (or a re-fold per pass) that cannot
/// change a single property.
pub fn carries_bindings(root: &ViewNode) -> bool {
    if !root.bound.is_empty() {
        return true;
    }
    root.children
        .iter()
        .any(|child| carries_bindings(child.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::super::binding::{
        DeriveExpr, PropKey, PropVal, Row, ShapeFault, SlotChange, SlotKey, SlotValue, shape_for,
    };
    use super::*;
    use crate::tree::NodeKind;

    fn lit(value: SlotValue) -> PropVal {
        PropVal::Lit(value)
    }

    fn text_node(bind: Option<PropVal>) -> ViewNode {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        if let Some(source) = bind {
            node.bound.set(PropKey::Text, source);
        }
        node
    }

    fn str_value(text: &str) -> SlotValue {
        SlotValue::Str(text.to_owned())
    }

    #[test]
    fn a_bind_resolves_to_its_slot_value() {
        let mut inputs = ResolveInputs::new();
        inputs.insert("title".into(), str_value("Fibers"));
        let tree = ResolvedTree::resolve(
            &text_node(Some(PropVal::Bind(SlotKey::new("title")))),
            &inputs,
        )
        .unwrap();
        assert_eq!(tree.tree().props.text.as_deref(), Some("Fibers"));
    }

    #[test]
    fn not_negates_a_flag() {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        node.bound.set(
            PropKey::Disabled,
            PropVal::Derive(DeriveExpr::Not {
                on: Box::new(PropVal::Bind(SlotKey::new("ready"))),
            }),
        );
        let mut inputs = ResolveInputs::new();
        inputs.insert("ready".into(), SlotValue::Bool(false));
        let tree = ResolvedTree::resolve(&node, &inputs).unwrap();
        assert!(tree.tree().semantics.disabled);
    }

    #[test]
    fn eq_compares_a_slot_against_a_constant() {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        node.bound.set(
            PropKey::Disabled,
            PropVal::Derive(DeriveExpr::Eq {
                slot: SlotKey::new("status"),
                value: str_value("running"),
            }),
        );
        let mut inputs = ResolveInputs::new();
        inputs.insert("status".into(), str_value("running"));
        let tree = ResolvedTree::resolve(&node, &inputs).unwrap();
        assert!(tree.tree().semantics.disabled);
    }

    #[test]
    fn if_takes_the_condition_and_only_evaluates_the_taken_branch() {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        node.bound.set(
            PropKey::Text,
            PropVal::Derive(DeriveExpr::If {
                on: Box::new(PropVal::Bind(SlotKey::new("on"))),
                then: Box::new(lit(str_value("shown"))),
                else_: Box::new(PropVal::Bind(SlotKey::new("never-delivered"))),
            }),
        );
        let mut inputs = ResolveInputs::new();
        inputs.insert("on".into(), SlotValue::Bool(true));
        // The `else` branch names a slot nobody delivered; laziness is what
        // lets an untaken branch hold a binding this fold cannot resolve.
        let tree = ResolvedTree::resolve(&node, &inputs).unwrap();
        assert_eq!(tree.tree().props.text.as_deref(), Some("shown"));
    }

    #[test]
    fn fmt_substitutes_named_arguments() {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        let mut args = BTreeMap::new();
        args.insert(
            "count".into(),
            PropVal::Derive(DeriveExpr::Len { of: "rows".into() }),
        );
        args.insert("who".into(), PropVal::Bind(SlotKey::new("who")));
        node.bound.set(
            PropKey::Text,
            PropVal::Derive(DeriveExpr::Fmt {
                format: "{who} has {{count}} {count} rows".to_owned(),
                args,
            }),
        );
        let mut inputs = ResolveInputs::new();
        inputs.insert("who".into(), str_value("Fen"));
        inputs.insert(
            "rows".into(),
            SlotValue::Rows(vec![Row::new(), Row::new(), Row::new()]),
        );
        let tree = ResolvedTree::resolve(&node, &inputs).unwrap();
        assert_eq!(
            tree.tree().props.text.as_deref(),
            Some("Fen has {count} 3 rows")
        );
    }

    #[test]
    fn fmt_refuses_a_placeholder_no_argument_names() {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        node.bound.set(
            PropKey::Text,
            PropVal::Derive(DeriveExpr::Fmt {
                format: "{missing}".to_owned(),
                args: BTreeMap::new(),
            }),
        );
        let err = ResolvedTree::resolve(&node, &ResolveInputs::new()).unwrap_err();
        assert!(
            matches!(err, ResolveError::MissingFormatArgument { ref name, .. } if name == "missing"),
            "{err}"
        );
    }

    #[test]
    fn len_counts_rows() {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        node.bound.set(
            PropKey::TotalCount,
            PropVal::Derive(DeriveExpr::Len { of: "rows".into() }),
        );
        let mut inputs = ResolveInputs::new();
        inputs.insert("rows".into(), SlotValue::Rows(vec![Row::new()]));
        let tree = ResolvedTree::resolve(&node, &inputs).unwrap();
        assert_eq!(tree.tree().props.total_count, Some(1));
    }

    fn sorted(direction: &str) -> ViewNode {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        node.bound.set(
            PropKey::Items,
            PropVal::Derive(DeriveExpr::SortBy {
                rows: "rows".into(),
                field: "col".into(),
                direction: "dir".into(),
            }),
        );
        let mut inputs = ResolveInputs::new();
        let row = |name: &str| {
            let mut row = Row::new();
            row.insert("name".into(), str_value(name));
            row
        };
        inputs.insert("rows".into(), SlotValue::Rows(vec![row("b"), row("a")]));
        inputs.insert("col".into(), str_value("name"));
        inputs.insert("dir".into(), str_value(direction));
        ResolvedTree::resolve(&node, &inputs)
            .unwrap()
            .tree()
            .clone()
    }

    #[test]
    fn sort_by_orders_rows_in_the_named_direction() {
        let names = |tree: &ViewNode| -> Vec<String> {
            tree.props
                .items
                .as_ref()
                .unwrap()
                .iter()
                .map(|row| match row.get("name") {
                    Some(SlotValue::Str(name)) => name.clone(),
                    other => panic!("row name is {other:?}"),
                })
                .collect()
        };
        assert_eq!(names(&sorted("asc")), ["a", "b"]);
        assert_eq!(names(&sorted("desc")), ["b", "a"]);
    }

    #[test]
    fn sort_by_refuses_a_direction_that_is_not_asc_or_desc() {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        node.bound.set(
            PropKey::Items,
            PropVal::Derive(DeriveExpr::SortBy {
                rows: "rows".into(),
                field: "col".into(),
                direction: "dir".into(),
            }),
        );
        let mut inputs = ResolveInputs::new();
        inputs.insert("rows".into(), SlotValue::Rows(Vec::new()));
        inputs.insert("col".into(), str_value("name"));
        inputs.insert("dir".into(), str_value("sideways"));
        let err = ResolvedTree::resolve(&node, &inputs).unwrap_err();
        assert!(
            matches!(err, ResolveError::SortDirection { ref got, .. } if got == "sideways"),
            "{err}"
        );
    }

    #[test]
    fn a_slot_with_no_value_is_refused_by_name() {
        let err = ResolvedTree::resolve(
            &text_node(Some(PropVal::Bind("title".into()))),
            &ResolveInputs::new(),
        )
        .unwrap_err();
        assert!(
            matches!(err, ResolveError::UnknownSlot { ref slot, .. } if slot == "title"),
            "{err}"
        );
    }

    #[test]
    fn a_property_with_two_sources_is_refused() {
        let mut node = text_node(Some(lit(str_value("bound"))));
        node.props.text = Some("literal".into());
        let err = ResolvedTree::resolve(&node, &ResolveInputs::new()).unwrap_err();
        assert!(
            matches!(
                err,
                ResolveError::TwoSources {
                    prop: PropKey::Text,
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn the_reverse_index_lists_every_binding_for_a_slot() {
        let mut root = ViewNode::new(NodeKind::Stack, "root");
        let mut left = ViewNode::new(NodeKind::Text, "left");
        left.bound.set(PropKey::Text, PropVal::Bind("k".into()));
        left.bound.set(PropKey::Label, PropVal::Bind("k".into()));
        let mut right = ViewNode::new(NodeKind::Text, "right");
        right.bound.set(
            PropKey::Value,
            PropVal::Derive(DeriveExpr::Fmt {
                format: "<{x}>".to_owned(),
                args: BTreeMap::from([("x".to_owned(), PropVal::Bind("k".into()))]),
            }),
        );
        root = root.child(left).child(right);
        let mut inputs = ResolveInputs::new();
        inputs.insert("k".into(), str_value("yes"));
        let tree = ResolvedTree::resolve(&root, &inputs).unwrap();
        let sites: Vec<(String, PropKey)> = tree
            .index()
            .get("k")
            .iter()
            .map(|site| (site.node.clone(), site.prop))
            .collect();
        assert_eq!(
            sites,
            [
                ("/root/left".to_owned(), PropKey::Text),
                ("/root/left".to_owned(), PropKey::Label),
                ("/root/right".to_owned(), PropKey::Value),
            ]
        );
    }

    #[test]
    fn apply_slot_changes_updates_exactly_the_bound_properties() {
        let mut root = ViewNode::new(NodeKind::Stack, "root");
        let mut bound = ViewNode::new(NodeKind::Text, "bound");
        bound.bound.set(PropKey::Text, PropVal::Bind("k".into()));
        let mut other = ViewNode::new(NodeKind::Text, "other");
        other.bound.set(PropKey::Label, PropVal::Bind("j".into()));
        other.props.text = Some("stays".into());
        root = root.child(bound).child(other);
        let mut inputs = ResolveInputs::new();
        inputs.insert("k".into(), str_value("before"));
        inputs.insert("j".into(), str_value("same"));
        let mut tree = ResolvedTree::resolve(&root, &inputs).unwrap();
        let before = tree.tree().clone();

        let affected = tree
            .apply_slot_changes(&[SlotChange::new("k", 1, str_value("after"))])
            .unwrap();
        assert_eq!(
            affected,
            [("/root/bound".to_owned(), PropKey::Text)],
            "only the property bound to `k` is re-computed"
        );
        let after = tree.tree();
        assert_eq!(after.children[0].props.text.as_deref(), Some("after"));
        assert_eq!(after.children[1], before.children[1], "nothing else moved");
        assert_eq!(after.props, before.props, "nothing on the root moved");
    }

    #[test]
    fn a_stale_version_refuses_the_whole_batch() {
        let node = text_node(Some(PropVal::Bind("k".into())));
        let mut inputs = ResolveInputs::new();
        inputs.insert("k".into(), str_value("v1"));
        let mut tree = ResolvedTree::resolve(&node, &inputs).unwrap();
        tree.apply_slot_changes(&[SlotChange::new("k", 5, str_value("v5"))])
            .unwrap();
        let err = tree
            .apply_slot_changes(&[
                SlotChange::new("k", 6, str_value("v6")),
                SlotChange::new("k", 4, str_value("v4")),
            ])
            .unwrap_err();
        assert!(
            matches!(
                err,
                ApplyError::StaleVersion {
                    offered: 4,
                    held: 6,
                    ..
                }
            ),
            "{err}"
        );
        assert_eq!(
            tree.tree().props.text.as_deref(),
            Some("v5"),
            "the refused batch changed nothing"
        );
    }

    #[test]
    fn refold_keeps_versions_across_trees() {
        let mut tree = ResolvedTree::new();
        tree.apply_slot_changes(&[SlotChange::new("k", 9, str_value("kept"))])
            .unwrap();
        tree.refold(&text_node(Some(PropVal::Bind("k".into()))))
            .unwrap();
        assert_eq!(tree.value("k"), Some(&str_value("kept")));
        assert_eq!(tree.version("k"), Some(9));
        assert_eq!(tree.tree().props.text.as_deref(), Some("kept"));
    }

    #[test]
    fn a_declared_fmt_argument_no_placeholder_reads_is_refused() {
        let mut node = ViewNode::new(NodeKind::Text, "t");
        let mut args = BTreeMap::new();
        args.insert("unused".into(), lit(str_value("x")));
        node.bound.set(
            PropKey::Text,
            PropVal::Derive(DeriveExpr::Fmt {
                format: "plain".to_owned(),
                args,
            }),
        );
        let err = ResolvedTree::resolve(&node, &ResolveInputs::new()).unwrap_err();
        assert!(
            matches!(err, ResolveError::UnusedFormatArgument { ref name, .. } if name == "unused"),
            "{err}"
        );
    }

    #[test]
    fn runs_against_a_bound_text_are_judged_when_the_text_lands() {
        use crate::tree::TextRun;
        let mut node = text_node(Some(PropVal::Bind("k".into())));
        node.props.runs = vec![TextRun {
            len: 3,
            foreground: None,
        }];
        let mut inputs = ResolveInputs::new();
        inputs.insert("k".into(), str_value("four"));
        let err = ResolvedTree::resolve(&node, &inputs).unwrap_err();
        assert!(
            matches!(
                err,
                ResolveError::RunsDoNotCoverText {
                    claimed: 3,
                    text_len: 4,
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn shape_refuses_a_derive_that_cannot_produce_the_property_type() {
        assert!(matches!(
            shape_for(
                &PropVal::Derive(DeriveExpr::SortBy {
                    rows: "r".into(),
                    field: "c".into(),
                    direction: "d".into(),
                }),
                PropKey::Text
            ),
            Err(ShapeFault::TypeMismatch { .. })
        ));
    }
}
