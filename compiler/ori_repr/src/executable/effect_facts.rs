//! Final backend-neutral RL-30 facts frozen by stable function identity.

use ori_arc::{ArcFunction, FunctionEffectFacts, MemoryAccessClass, MemoryContract};
use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use super::RealizationError;

/// Validate exact coverage and order already-realized RL-30 facts by slot.
pub(super) fn freeze_function_effects(
    functions: &[ArcFunction],
    contracts: &[MemoryContract],
    mut facts: FxHashMap<Name, FunctionEffectFacts>,
    symbols: &StringInterner,
) -> Result<Box<[FunctionEffectFacts]>, RealizationError> {
    let mut frozen = Vec::with_capacity(functions.len());
    for (function, contract) in functions.iter().zip(contracts) {
        let function_symbol: Box<str> = symbols.lookup(function.name).into();
        let function_facts = facts.remove(&function.name).ok_or_else(|| {
            RealizationError::MissingFunctionEffectFacts {
                function: function.name,
                function_symbol: function_symbol.clone(),
            }
        })?;
        if function_facts.effects() != contract.effects {
            return Err(RealizationError::FunctionEffectFactsMismatch {
                function: function.name,
                function_symbol,
                contract: contract.effects,
                facts: function_facts.effects(),
            });
        }
        if function_facts.may_write_inaccessible()
            && function_facts.memory_access() == MemoryAccessClass::ReadOnly
        {
            return Err(RealizationError::InvalidFunctionEffectFacts {
                function: function.name,
                function_symbol,
            });
        }
        frozen.push(function_facts);
    }
    if let Some(function) = facts.keys().min_by_key(|name| name.raw()).copied() {
        return Err(RealizationError::UnexpectedFunctionEffectFacts { function });
    }
    Ok(frozen.into_boxed_slice())
}
