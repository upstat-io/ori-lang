//! Lowering of the type-checker-closed monomorphized-function inventory.

use ori_ir::Name;
use ori_repr::monomorphize::{MonoFunction, MonoTargetMaps};
use ori_types::{AcceptedDerivedImpl, DerivedCallPlan};
use rustc_hash::FxHashSet;

use super::repr::lower_mono_function_for_analysis;
use crate::arc_lowering::ArcLoweringContext;

/// Lower every local body in the already-closed mono inventory.
///
/// The type checker owns generated-call closure and records exact producer
/// selections in `derived_call_plans`. Realization consumes that frozen
/// inventory without redispatching by receiver shape or method spelling.
pub(crate) fn lower_mono_functions_for_analysis(
    mono_functions: &[MonoFunction],
    accepted_derives: &[AcceptedDerivedImpl],
    derived_call_plans: &[DerivedCallPlan],
    context: &mut ArcLoweringContext<'_>,
) -> Vec<super::ArcFunctionGroup> {
    lower_selected_mono_functions_for_analysis(
        mono_functions,
        None,
        accepted_derives,
        derived_call_plans,
        context,
    )
}

/// Lower only newly discovered local mono bodies while resolving their calls
/// against the complete final target inventory.
pub(crate) fn lower_new_mono_functions_for_analysis(
    mono_functions: &[MonoFunction],
    selected: &FxHashSet<Name>,
    accepted_derives: &[AcceptedDerivedImpl],
    derived_call_plans: &[DerivedCallPlan],
    context: &mut ArcLoweringContext<'_>,
) -> Vec<super::ArcFunctionGroup> {
    lower_selected_mono_functions_for_analysis(
        mono_functions,
        Some(selected),
        accepted_derives,
        derived_call_plans,
        context,
    )
}

fn lower_selected_mono_functions_for_analysis(
    mono_functions: &[MonoFunction],
    selected: Option<&FxHashSet<Name>>,
    accepted_derives: &[AcceptedDerivedImpl],
    derived_call_plans: &[DerivedCallPlan],
    context: &mut ArcLoweringContext<'_>,
) -> Vec<super::ArcFunctionGroup> {
    let mono_targets = MonoTargetMaps::new(mono_functions, context.pool);
    mono_functions
        .iter()
        .filter(|mono| !mono.is_imported)
        .filter(|mono| selected.is_none_or(|names| names.contains(&mono.mangled_name)))
        .filter_map(|mono| {
            lower_mono_function_for_analysis(
                mono,
                &mono_targets,
                accepted_derives,
                derived_call_plans,
                context,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
