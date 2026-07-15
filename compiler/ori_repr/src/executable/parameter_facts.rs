//! Backend-neutral AIMS facts frozen into the executable artifact.

use ori_arc::{ArcFunction, ParamDisjointnessFacts};
use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use super::RealizationError;

/// Validate exact coverage and order already-realized RL-31 facts by slot.
pub(super) fn freeze_param_disjointness(
    functions: &[ArcFunction],
    mut facts: FxHashMap<Name, ParamDisjointnessFacts>,
    symbols: &StringInterner,
) -> Result<Box<[ParamDisjointnessFacts]>, RealizationError> {
    let mut frozen = Vec::with_capacity(functions.len());
    for function in functions {
        let function_symbol: Box<str> = symbols.lookup(function.name).into();
        let function_facts = facts.remove(&function.name).ok_or_else(|| {
            RealizationError::MissingParamDisjointnessFacts {
                function: function.name,
                function_symbol: function_symbol.clone(),
            }
        })?;
        if function_facts.type_disjointness().len() != function.params.len() {
            return Err(RealizationError::ParamDisjointnessArity {
                function: function.name,
                function_symbol,
                parameters: function.params.len(),
                fact_parameters: function_facts.type_disjointness().len(),
            });
        }
        frozen.push(function_facts);
    }
    if let Some(function) = facts.keys().min_by_key(|name| name.raw()).copied() {
        return Err(RealizationError::UnexpectedParamDisjointnessFacts { function });
    }
    Ok(frozen.into_boxed_slice())
}
