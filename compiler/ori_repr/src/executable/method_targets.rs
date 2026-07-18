//! Frozen receiver-qualified callable targets used by physical projections.

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use super::function_families::FrozenFunctionFamilies;
use super::{CallableTarget, ExternalFunctionId, FunctionId, RealizationError};

pub(super) fn freeze_method_targets(
    targets: FxHashMap<(Idx, Name), Name>,
    pool: &Pool,
    symbols: &StringInterner,
    function_ids: &FxHashMap<Name, FunctionId>,
    external_ids: &FxHashMap<Name, ExternalFunctionId>,
    function_families: &FrozenFunctionFamilies,
) -> Result<FxHashMap<(Idx, Name), CallableTarget>, RealizationError> {
    let mut frozen = FxHashMap::default();
    for ((receiver, method), target) in targets {
        if !pool.is_valid_idx(receiver) {
            return Err(RealizationError::InvalidMethodTargetReceiver { receiver, method });
        }
        if symbols.try_lookup(method).is_none() {
            return Err(RealizationError::UnknownMethodTargetName { method });
        }
        let callable = if let Some(&function) = function_ids.get(&target) {
            if function_families.is_lambda(function) {
                return Err(RealizationError::MethodTargetIsLambda {
                    receiver,
                    method,
                    target,
                });
            }
            CallableTarget::Function(function)
        } else if let Some(&external) = external_ids.get(&target) {
            CallableTarget::External(external)
        } else {
            return Err(RealizationError::MissingMethodTarget {
                receiver,
                method,
                target,
            });
        };
        frozen.insert((receiver, method), callable);
    }
    Ok(frozen)
}
