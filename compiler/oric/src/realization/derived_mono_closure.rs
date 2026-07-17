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
mod tests {
    use ori_ir::canon::CanonResult;
    use ori_ir::StringInterner;
    use ori_repr::monomorphize::{MonoFunction, MonoFunctionOrigin};
    use ori_types::{FunctionSig, Idx, Pool};
    use rustc_hash::FxHashMap;

    use super::lower_mono_functions_for_analysis;

    #[test]
    fn imported_metadata_is_not_lowered_against_host_canon() {
        let interner = StringInterner::new();
        let name = interner.intern("imported_identity$m$3_int");
        let monos = vec![MonoFunction {
            mangled_name: name,
            original_name: interner.intern("imported_identity"),
            origin: MonoFunctionOrigin::Source,
            sig: FunctionSig::simple(name, vec![Idx::INT], Idx::INT),
            body_type_map: FxHashMap::default(),
            instance_ids: Vec::new(),
            is_imported: true,
            receiver_type: None,
            receiver_type_name: None,
        }];
        let mut problems = Vec::new();

        let groups = lower_mono_functions_for_analysis(
            &monos,
            &[],
            &[],
            &CanonResult::empty(),
            &interner,
            &Pool::new(),
            &mut problems,
        );

        assert!(groups.is_empty());
        assert!(problems.is_empty());
    }
}
