use ori_ir::canon::{CanId, CanNamedExpr};
use ori_ir::{Name, StringInterner};
use ori_patterns::{ControlAction, EvalError, Value};

/// Returns a pre-evaluated property selected by interned `Name`.
pub(super) fn find_prop_value(
    values: &[(Name, Value)],
    name: Name,
    interner: &StringInterner,
) -> Result<Value, ControlAction> {
    values
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, value)| value.clone())
        .ok_or_else(|| missing_property(name, interner))
}

/// Returns an unevaluated property's `CanId` for lazy evaluation.
pub(super) fn find_prop_can_id(
    named: &[CanNamedExpr],
    name: Name,
    interner: &StringInterner,
) -> Result<CanId, ControlAction> {
    named
        .iter()
        .find(|expression| expression.name == name)
        .map(|expression| expression.value)
        .ok_or_else(|| missing_property(name, interner))
}

fn missing_property(name: Name, interner: &StringInterner) -> ControlAction {
    EvalError::new(format!(
        "missing required property: {}",
        interner.lookup(name)
    ))
    .into()
}
