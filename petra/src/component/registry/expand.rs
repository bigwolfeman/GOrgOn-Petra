//! The expansion walk: component references → the primitives their
//! constructors build, with bound parameters folded to literals first.
//!
//! One walk serves both callers. [`super::expand`] passes no values, and a
//! reference carrying `bound` sources is refused there by name — it cannot
//! be expanded honestly without them. [`super::bound::expand_with`] passes
//! the slot values, so each openable parameter's source is evaluated,
//! type-checked against the openable table and written into the parameter
//! table before the constructor runs, and every slot read is collected so
//! the caller knows what re-expands this subtree.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::openable::openable_param;
use super::{MAX_DEPTH, build};
use crate::component::params::ParamError;
use crate::tree::binding_eval::{eval_source, slots_read, type_mismatch};
use crate::tree::{
    BindTarget, NodeKind, PropType, PropVal, ResolveError, ResolveInputs, SlotValue, ViewNode,
};

/// Why an expansion refused: a parameter table the constructor rejects (a
/// defect no slot value repairs), or a bound parameter whose value could not
/// be computed (a slot with no value yet, a value of the wrong type).
#[derive(Clone, Debug, PartialEq)]
pub enum ExpandError {
    /// The reference, its parameters or its sources are malformed.
    Param(ParamError),
    /// A bound parameter's source did not evaluate.
    Resolve(ResolveError),
}

impl std::fmt::Display for ExpandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Param(err) => write!(f, "{}: {}", err.component, err.reason),
            Self::Resolve(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ExpandError {}

impl From<ParamError> for ExpandError {
    fn from(value: ParamError) -> Self {
        Self::Param(value)
    }
}

/// The values a walk folds bound parameters against, and what it read.
pub(crate) struct Fold<'a> {
    inputs: Option<&'a ResolveInputs>,
    /// Every slot a bound parameter under the walk's root reads.
    pub(crate) read: BTreeSet<String>,
}

impl<'a> Fold<'a> {
    /// No values: a bound parameter is refused.
    pub(crate) fn literal() -> Self {
        Self {
            inputs: None,
            read: BTreeSet::new(),
        }
    }

    /// Fold bound parameters against `inputs`.
    pub(crate) fn with(inputs: &'a ResolveInputs) -> Self {
        Self {
            inputs: Some(inputs),
            read: BTreeSet::new(),
        }
    }
}

pub(crate) fn too_deep(what: &str) -> ParamError {
    ParamError {
        component: what.to_owned(),
        reason: format!(
            "contributed tree nests deeper than {MAX_DEPTH}; expansion refuses it rather than \
             recursing further"
        ),
    }
}

pub(crate) fn expand_node(
    node: &ViewNode,
    depth: usize,
    fold: &mut Fold<'_>,
) -> Result<ViewNode, ExpandError> {
    if depth >= MAX_DEPTH {
        return Err(too_deep(node.key.as_str()).into());
    }
    if node.kind == NodeKind::Component {
        let reference = node.component.as_ref().ok_or_else(|| ParamError {
            component: node.key.as_str().to_owned(),
            reason: "kind is `component` but no component reference is declared; `validate` \
                     should have refused this tree before expansion"
                .to_owned(),
        })?;
        if !node.bound.is_empty() {
            // Expansion replaces this node with the constructor's subtree,
            // so a `bound` map here would be dropped without a word and the
            // property would silently draw its empty literal. Binding an
            // openable parameter (`component.bound`), a node in `params`
            // (which survives expansion) or the tree around this reference
            // is the way to bind.
            return Err(ParamError {
                component: node.key.as_str().to_owned(),
                reason:
                    "a component reference carries property-value sources (`bound`); expansion \
                         replaces the node with the constructor's subtree and would drop them — \
                         bind an openable parameter (`component.bound`), a node in `params`, or \
                         the tree around the reference instead"
                        .to_owned(),
            }
            .into());
        }
        let mut params = expand_params(&reference.params, depth + 1, fold)?;
        if !reference.bound.is_empty() {
            fold_bound(node, &reference.name, &reference.bound, &mut params, fold)?;
        }
        // The built subtree is primitives: a constructor in `crate::component`
        // cannot name another component, so there is nothing left to expand
        // and no second walk to pay for.
        return Ok(build(&reference.name, &params)?);
    }
    let mut out = node.clone();
    out.children = node
        .children
        .iter()
        .map(|child| expand_node(child, depth + 1, fold).map(Arc::new))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}

/// Write each bound parameter's current value into `params`.
///
/// A parameter the component does not declare openable is refused by name,
/// as is a parameter declared both literally and bound. Without values
/// ([`Fold::literal`]) any `bound` entry is refused: the reference cannot be
/// expanded honestly without the slot values it reads.
fn fold_bound(
    node: &ViewNode,
    component: &str,
    bound: &std::collections::BTreeMap<String, PropVal>,
    params: &mut serde_json::Value,
    fold: &mut Fold<'_>,
) -> Result<(), ExpandError> {
    let site = node.key.as_str();
    let refuse = |reason: String| {
        ExpandError::Param(ParamError {
            component: component.to_owned(),
            reason,
        })
    };
    let Some(inputs) = fold.inputs else {
        return Err(refuse(format!(
            "reference `{site}` binds parameters ({}) and was expanded without slot values; \
             expand it through `registry::bound::expand_with`, which folds them first",
            bound.keys().cloned().collect::<Vec<_>>().join(", ")
        )));
    };
    if params.is_null() {
        *params = serde_json::Value::Object(serde_json::Map::new());
    }
    let Some(table) = params.as_object_mut() else {
        return Err(refuse(format!(
            "reference `{site}` binds parameters but its parameter table is not a table"
        )));
    };
    for (param, source) in bound {
        let Some((name, ty)) = openable_param(component, param) else {
            return Err(refuse(format!(
                "parameter `{param}` is not openable: a component expands from literal \
                 parameters, and `{component}` opens only [{}] to a slot",
                super::openable::openable(component)
                    .iter()
                    .map(|(name, _)| *name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };
        if table.contains_key(param) {
            return Err(refuse(format!(
                "parameter `{param}` of `{site}` has exactly one source, and this one declares \
                 a literal and a binding"
            )));
        }
        let target = BindTarget::Param(name);
        let value = eval_source(site, target, source, inputs).map_err(ExpandError::Resolve)?;
        if value.prop_type() != ty {
            return Err(ExpandError::Resolve(type_mismatch(
                site, target, "the slot", ty, value,
            )));
        }
        table.insert(param.clone(), to_json(site, name, ty, value)?);
        let mut slots = Vec::new();
        slots_read(source, &mut slots);
        fold.read
            .extend(slots.into_iter().map(|slot| slot.as_str().to_owned()));
    }
    Ok(())
}

fn to_json(
    site: &str,
    param: &'static str,
    ty: PropType,
    value: SlotValue,
) -> Result<serde_json::Value, ExpandError> {
    Ok(match value {
        SlotValue::Bool(flag) => serde_json::Value::Bool(flag),
        SlotValue::Str(text) => serde_json::Value::String(text),
        SlotValue::Num(number) => serde_json::Number::from_f64(number)
            .map(serde_json::Value::Number)
            .ok_or_else(|| {
                ExpandError::Resolve(ResolveError::NotFinite {
                    node: site.to_owned(),
                    prop: BindTarget::Param(param),
                    got: number,
                })
            })?,
        rows @ SlotValue::Rows(_) => {
            // No openable parameter holds rows (the table's own test refuses
            // one), so this is a table defect, reported where it bites.
            return Err(ExpandError::Resolve(type_mismatch(
                site,
                BindTarget::Param(param),
                "the slot",
                ty,
                rows,
            )));
        }
    })
}

/// Expand any component references embedded in a parameter table.
///
/// Parameters are still JSON here, so a nested node is an object carrying
/// `"kind": "component"`. Walking JSON rather than deserializing first is what
/// lets one function serve every parameter shape: a shape holding
/// `Vec<ViewNode>` and one holding a single `ViewNode` need no separate case.
fn expand_params(
    params: &serde_json::Value,
    depth: usize,
    fold: &mut Fold<'_>,
) -> Result<serde_json::Value, ExpandError> {
    if depth >= MAX_DEPTH {
        return Err(too_deep("<params>").into());
    }
    match params {
        serde_json::Value::Object(fields) => {
            if fields.get("kind").and_then(serde_json::Value::as_str) == Some("component") {
                let node: ViewNode =
                    serde_json::from_value(params.clone()).map_err(|err| ParamError {
                        component: "<nested>".to_owned(),
                        reason: format!("nested component reference does not deserialize: {err}"),
                    })?;
                let built = expand_node(&node, depth, fold)?;
                return Ok(serde_json::to_value(built).map_err(|err| ParamError {
                    component: "<nested>".to_owned(),
                    reason: format!("expanded subtree does not re-serialize: {err}"),
                })?);
            }
            let mut out = serde_json::Map::with_capacity(fields.len());
            for (name, value) in fields {
                out.insert(name.clone(), expand_params(value, depth + 1, fold)?);
            }
            Ok(serde_json::Value::Object(out))
        }
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| expand_params(item, depth + 1, fold))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        other => Ok(other.clone()),
    }
}
