//! Final backend-neutral AIMS contracts frozen by stable function identity.

use ori_arc::{ArcFunction, MemoryContract};
use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use super::RealizationError;

/// Validate exact function coverage and order contracts by executable slot.
pub(super) fn freeze_function_contracts(
    functions: &[ArcFunction],
    mut contracts: FxHashMap<Name, MemoryContract>,
    symbols: &StringInterner,
) -> Result<Box<[MemoryContract]>, RealizationError> {
    let mut frozen = Vec::with_capacity(functions.len());
    for function in functions {
        let function_symbol: Box<str> = symbols.lookup(function.name).into();
        let contract = contracts.remove(&function.name).ok_or_else(|| {
            RealizationError::MissingFunctionContract {
                function: function.name,
                function_symbol: function_symbol.clone(),
            }
        })?;
        if contract.params.len() != function.params.len() {
            return Err(RealizationError::FunctionContractArity {
                function: function.name,
                function_symbol,
                parameters: function.params.len(),
                contract_parameters: contract.params.len(),
            });
        }
        frozen.push(contract);
    }
    if let Some(function) = contracts.keys().min_by_key(|name| name.raw()).copied() {
        return Err(RealizationError::UnexpectedFunctionContract { function });
    }
    Ok(frozen.into_boxed_slice())
}
