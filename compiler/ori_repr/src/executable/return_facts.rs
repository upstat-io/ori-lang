//! Final backend-neutral RL-29 facts frozen by stable function identity.

use ori_arc::{ArcFunction, FreshSelfAllocationFacts, MemoryContract};
use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use super::RealizationError;

/// Validate exact coverage and order already-realized RL-29 facts by slot.
pub(super) fn freeze_fresh_return_facts(
    functions: &[ArcFunction],
    contracts: &[MemoryContract],
    mut facts: FxHashMap<Name, FreshSelfAllocationFacts>,
    symbols: &StringInterner,
) -> Result<Box<[FreshSelfAllocationFacts]>, RealizationError> {
    let mut frozen = Vec::with_capacity(functions.len());
    for (function, contract) in functions.iter().zip(contracts) {
        let function_symbol: Box<str> = symbols.lookup(function.name).into();
        let function_facts = facts.remove(&function.name).ok_or_else(|| {
            RealizationError::MissingFreshReturnFacts {
                function: function.name,
                function_symbol: function_symbol.clone(),
            }
        })?;
        if function_facts.is_proven() != contract.return_info.returns_fresh_self_alloc {
            return Err(RealizationError::FreshReturnFactsMismatch {
                function: function.name,
                function_symbol,
                contract: contract.return_info.returns_fresh_self_alloc,
                facts: function_facts.is_proven(),
            });
        }
        frozen.push(function_facts);
    }
    if let Some(function) = facts.keys().min_by_key(|name| name.raw()).copied() {
        return Err(RealizationError::UnexpectedFreshReturnFacts { function });
    }
    Ok(frozen.into_boxed_slice())
}
