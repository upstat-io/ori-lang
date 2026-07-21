//! Validation and freezing of the closed executable-program artifact.

use ori_arc::{
    ArcFunction, ClosureAdapterPlan, FreshSelfAllocationFacts, FunctionCallableFacts,
    FunctionEffectFacts, MemoryContract, ParamDisjointnessFacts, RetainPlanTable,
};
use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use super::{
    call_targets, callable_facts, closure_adapters, drop_plan, effect_facts, external,
    function_contracts, function_families, method_targets, parameter_facts, return_facts,
    validation, CallableTarget, ExecutableDropPlan, ExecutableProgram, ExecutableProgramParts,
    FunctionId, RealizationError, EXECUTABLE_PROGRAM_VERSION,
};

pub(super) fn validate(
    mut parts: ExecutableProgramParts,
) -> Result<ExecutableProgram, RealizationError> {
    validate_version(parts.version)?;
    validate_function_symbols(&parts.functions, &parts.symbols)?;
    validation::validate_function_metadata(&parts.functions, &parts.pool, &parts.symbols)?;
    parts.functions.sort_by(|left, right| {
        parts
            .symbols
            .lookup(left.name)
            .cmp(parts.symbols.lookup(right.name))
            .then_with(|| left.name.raw().cmp(&right.name.raw()))
    });
    let function_ids = build_function_ids(&parts.functions)?;
    freeze_executable_program(parts, function_ids)
}

fn freeze_executable_program(
    mut parts: ExecutableProgramParts,
    function_ids: FxHashMap<Name, FunctionId>,
) -> Result<ExecutableProgram, RealizationError> {
    let FrozenFunctionMetadata {
        function_families,
        user_drop_plan,
        function_contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters,
        retain_plans,
    } = freeze_function_metadata(&mut parts, &function_ids)?;
    let roots = freeze_roots(
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
    let (external_functions, external_ids) =
        external::freeze_external_callables(parts.externals, &parts.pool)?;
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
    parts.repr_plan.close_yield_runtime_header_requirements(
        &parts.functions,
        |function, destination| {
            function_ids.get(&function).is_some_and(|function_id| {
                matches!(
                    direct_call_targets.get(&(*function_id, destination)),
                    Some(CallableTarget::Runtime(_))
                )
            })
        },
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

struct FrozenFunctionMetadata {
    function_families: function_families::FrozenFunctionFamilies,
    user_drop_plan: ExecutableDropPlan,
    function_contracts: Box<[MemoryContract]>,
    function_effects: Box<[FunctionEffectFacts]>,
    fresh_return_facts: Box<[FreshSelfAllocationFacts]>,
    param_disjointness: Box<[ParamDisjointnessFacts]>,
    callable_facts: Box<[FunctionCallableFacts]>,
    closure_adapters: Box<[Option<ClosureAdapterPlan>]>,
    retain_plans: RetainPlanTable,
}

fn freeze_function_metadata(
    parts: &mut ExecutableProgramParts,
    function_ids: &FxHashMap<Name, FunctionId>,
) -> Result<FrozenFunctionMetadata, RealizationError> {
    let function_families = function_families::freeze_function_families(
        std::mem::take(&mut parts.function_families),
        &parts.functions,
        function_ids,
    )?;
    let user_drop_plan = drop_plan::freeze_user_drop_plan(
        std::mem::take(&mut parts.user_drop_bindings),
        &parts.type_registry,
        &parts.pool,
        &parts.functions,
        function_ids,
    )?;
    let function_contracts = function_contracts::freeze_function_contracts(
        &parts.functions,
        std::mem::take(&mut parts.contracts),
        &parts.symbols,
    )?;
    let function_effects = effect_facts::freeze_function_effects(
        &parts.functions,
        &function_contracts,
        std::mem::take(&mut parts.function_effects),
        &parts.symbols,
    )?;
    let fresh_return_facts = return_facts::freeze_fresh_return_facts(
        &parts.functions,
        &function_contracts,
        std::mem::take(&mut parts.fresh_return_facts),
        &parts.symbols,
    )?;
    let param_disjointness = parameter_facts::freeze_param_disjointness(
        &parts.functions,
        std::mem::take(&mut parts.param_disjointness),
        &parts.symbols,
    )?;
    let callable_facts = callable_facts::freeze_callable_facts(
        &parts.functions,
        std::mem::take(&mut parts.callable_facts),
        &parts.symbols,
    )?;
    let (closure_adapters, retain_plans) = closure_adapters::freeze_closure_adapters(
        &parts.functions,
        &function_contracts,
        &parts.closure_adapters,
        std::mem::take(&mut parts.retain_plans),
        &parts.symbols,
    )?;
    Ok(FrozenFunctionMetadata {
        function_families,
        user_drop_plan,
        function_contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters,
        retain_plans,
    })
}

fn validate_version(version: u32) -> Result<(), RealizationError> {
    if version == EXECUTABLE_PROGRAM_VERSION {
        Ok(())
    } else {
        Err(RealizationError::UnsupportedVersion {
            found: version,
            expected: EXECUTABLE_PROGRAM_VERSION,
        })
    }
}

fn validate_function_symbols(
    functions: &[ArcFunction],
    symbols: &StringInterner,
) -> Result<(), RealizationError> {
    for function in functions {
        if symbols.try_lookup(function.name).is_none() {
            return Err(RealizationError::UnknownFunctionName {
                name: function.name,
            });
        }
    }
    Ok(())
}

fn build_function_ids(
    functions: &[ArcFunction],
) -> Result<FxHashMap<Name, FunctionId>, RealizationError> {
    let mut ids = FxHashMap::default();
    for (index, function) in functions.iter().enumerate() {
        let id = FunctionId::from_index(index)?;
        if ids.insert(function.name, id).is_some() {
            return Err(RealizationError::DuplicateFunction {
                name: function.name,
            });
        }
    }
    Ok(ids)
}

fn freeze_roots(
    roots: &[Name],
    cli_entry: Option<Name>,
    function_ids: &FxHashMap<Name, FunctionId>,
    function_families: &function_families::FrozenFunctionFamilies,
) -> Result<Box<[FunctionId]>, RealizationError> {
    if roots.is_empty() {
        if function_ids.is_empty() && cli_entry.is_none() {
            return Ok(Box::new([]));
        }
        return Err(RealizationError::MissingProgramRoots);
    }
    let mut seen = rustc_hash::FxHashSet::default();
    let mut frozen = Vec::with_capacity(roots.len());
    for &name in roots {
        if !seen.insert(name) {
            return Err(RealizationError::DuplicateProgramRoot { name });
        }
        let function = function_ids
            .get(&name)
            .copied()
            .ok_or(RealizationError::MissingProgramRoot { name })?;
        if function_families.is_lambda(function) {
            return Err(RealizationError::ProgramRootIsLambda { name });
        }
        frozen.push(function);
    }
    if let Some(entry) = cli_entry {
        if !seen.contains(&entry) {
            return Err(RealizationError::CliEntryNotRoot { name: entry });
        }
    }
    Ok(frozen.into_boxed_slice())
}
