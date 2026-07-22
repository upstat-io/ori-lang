//! Validation and freezing of the closed executable-program artifact.

mod length_projection;
mod metadata;

use super::{
    call_targets, external, method_targets, validation, CallableTarget, ExecutableDropPlan,
    ExecutableProgram, ExecutableProgramParts, FunctionId, RealizationError,
};
use ori_arc::ir::ArcVarId;
use ori_arc::ArcFunction;
use ori_ir::Name;
use rustc_hash::FxHashMap;

pub(super) fn validate(
    mut parts: ExecutableProgramParts,
) -> Result<ExecutableProgram, RealizationError> {
    metadata::validate_version(parts.version)?;
    metadata::validate_function_symbols(&parts.functions, &parts.symbols)?;
    validation::validate_function_metadata(&parts.functions, &parts.pool, &parts.symbols)?;
    parts.functions.sort_by(|left, right| {
        parts
            .symbols
            .lookup(left.name)
            .cmp(parts.symbols.lookup(right.name))
            .then_with(|| left.name.raw().cmp(&right.name.raw()))
    });
    let function_ids = metadata::build_function_ids(&parts.functions)?;
    freeze_executable_program(parts, function_ids)
}

fn freeze_executable_program(
    mut parts: ExecutableProgramParts,
    function_ids: FxHashMap<Name, FunctionId>,
) -> Result<ExecutableProgram, RealizationError> {
    let metadata::FrozenFunctionMetadata {
        function_families,
        user_drop_plan,
        function_contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters,
        retain_plans,
    } = metadata::freeze_function_metadata(&mut parts, &function_ids)?;

    let roots = metadata::freeze_roots(
        &parts.roots,
        parts.cli_entry,
        &function_ids,
        &function_families,
    )?;

    let cli_entry = parts
        .cli_entry
        .map(|name| {
            function_ids
                .get(&name)
                .copied()
                .ok_or(RealizationError::MissingEntryPoint { name })
        })
        .transpose()?;
    let (external_functions, external_ids) = external::freeze_external_callables(parts.externals)?;
    call_targets::validate_external_symbols(&external_functions, &parts.symbols, &function_ids)?;
    let method_targets = method_targets::freeze_method_targets(
        std::mem::take(&mut parts.method_targets),
        &parts.pool,
        &parts.symbols,
        &function_ids,
        &external_ids,
        &function_families,
    )?;

    let call_targets::ResolvedCallTargets {
        call_targets,
        direct_call_targets,
    } = call_targets::build_call_targets(
        &parts.functions,
        &function_ids,
        &external_functions,
        &external_ids,
        &parts.symbols,
        &parts.pool,
    )?;

    close_yield_lineage_facts(
        &mut parts.repr_plan,
        &parts.functions,
        &parts.pool,
        &user_drop_plan,
        &function_ids,
        &direct_call_targets,
    );
    Ok(ExecutableProgram {
        version: parts.version,
        symbols: parts.symbols,
        pool: parts.pool,
        functions: parts.functions.into_boxed_slice(),
        function_ids,
        function_family_lambdas: function_families.lambdas_by_parent,
        function_contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters,
        retain_plans,
        call_targets,
        direct_call_targets,
        roots,
        cli_entry,
        external_functions,
        external_ids,
        method_targets,
        user_drop_plan,
        repr_plan: parts.repr_plan,
        type_registry: parts.type_registry,
    })
}

/// Freezes runtime-header requirements and length-projection pairs together.
fn close_yield_lineage_facts(
    repr_plan: &mut crate::plan::ReprPlan,
    functions: &[ArcFunction],
    pool: &ori_types::Pool,
    user_drop_plan: &ExecutableDropPlan,
    function_ids: &FxHashMap<Name, FunctionId>,
    direct_call_targets: &FxHashMap<(FunctionId, ArcVarId), CallableTarget>,
) {
    repr_plan.close_yield_runtime_header_requirements(functions, pool, |function, destination| {
        let function_id = function_ids.get(&function)?;
        let CallableTarget::Runtime(operation) =
            direct_call_targets.get(&(*function_id, destination))?
        else {
            return None;
        };
        yield_lineage_runtime_call(*operation)
    });
    let (length_projection_calls, length_projection_yields) =
        length_projection::analyze_length_projections(
            functions,
            pool,
            user_drop_plan,
            function_ids,
            direct_call_targets,
        );
    repr_plan.set_length_projections(length_projection_calls, length_projection_yields);
}

/// Classify a resolved runtime call for yield-lineage header accounting.
fn yield_lineage_runtime_call(
    operation: super::RuntimeCall,
) -> Option<crate::plan::YieldLineageRuntimeCall> {
    match operation {
        super::RuntimeCall::Index
        | super::RuntimeCall::Length
        | super::RuntimeCall::Protocol(
            ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index,
        )
        | super::RuntimeCall::RegisteredMethod(ori_registry::MethodRuntime::Length) => {
            Some(crate::plan::YieldLineageRuntimeCall::BorrowedRead)
        }
        super::RuntimeCall::ListSet
        | super::RuntimeCall::RegisteredMethod(ori_registry::MethodRuntime::ListSet) => {
            Some(crate::plan::YieldLineageRuntimeCall::StaticUniqueListSet)
        }
        _ => None,
    }
}
