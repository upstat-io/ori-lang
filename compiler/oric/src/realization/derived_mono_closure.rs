//! Lowering of the type-checker-closed monomorphized-function inventory.

use ori_ir::canon::CanonResult;
use ori_ir::StringInterner;
use ori_repr::monomorphize::MonoFunction;
use ori_types::{AcceptedDerivedImpl, DerivedCallPlan, Pool};

use super::repr::lower_mono_function_for_analysis;

/// Lower every local body in the already-closed mono inventory.
///
/// The type checker owns generated-call closure and records exact producer
/// selections in `derived_call_plans`. Realization consumes that frozen
/// inventory without redispatching by receiver shape or method spelling.
pub(crate) fn lower_mono_functions_for_analysis(
    mono_functions: &[MonoFunction],
    accepted_derives: &[AcceptedDerivedImpl],
    derived_call_plans: &[DerivedCallPlan],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Vec<super::ArcFunctionGroup> {
    mono_functions
        .iter()
        .filter(|mono| !mono.is_imported)
        .filter_map(|mono| {
            lower_mono_function_for_analysis(
                mono,
                accepted_derives,
                derived_call_plans,
                canon,
                interner,
                pool,
                problems,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
