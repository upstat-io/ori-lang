use ori_arc::{
    ArcFunction, ClosureAdapterPlan, FreshSelfAllocationFacts, FunctionCallableFacts,
    FunctionEffectFacts, MemoryContract, ParamDisjointnessFacts, RetainPlanTable,
};
use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::{
    callable_facts, closure_adapters, drop_plan, effect_facts, function_contracts,
    function_families, parameter_facts, return_facts, ExecutableDropPlan, ExecutableProgramParts,
    FunctionId, RealizationError, EXECUTABLE_PROGRAM_VERSION,
};

pub(super) struct FrozenFunctionMetadata {
    pub(super) function_families: function_families::FrozenFunctionFamilies,
    pub(super) user_drop_plan: ExecutableDropPlan,
    pub(super) function_contracts: Box<[MemoryContract]>,
    pub(super) function_effects: Box<[FunctionEffectFacts]>,
    pub(super) fresh_return_facts: Box<[FreshSelfAllocationFacts]>,
    pub(super) param_disjointness: Box<[ParamDisjointnessFacts]>,
    pub(super) callable_facts: Box<[FunctionCallableFacts]>,
    pub(super) closure_adapters: Box<[Option<ClosureAdapterPlan>]>,
    pub(super) retain_plans: RetainPlanTable,
}

pub(super) fn freeze_function_metadata(
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

pub(super) fn validate_version(version: u32) -> Result<(), RealizationError> {
    if version == EXECUTABLE_PROGRAM_VERSION {
        Ok(())
    } else {
        Err(RealizationError::UnsupportedVersion {
            found: version,
            expected: EXECUTABLE_PROGRAM_VERSION,
        })
    }
}

pub(super) fn validate_function_symbols(
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

pub(super) fn build_function_ids(
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

pub(super) fn freeze_roots(
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
    let mut seen = FxHashSet::default();
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
