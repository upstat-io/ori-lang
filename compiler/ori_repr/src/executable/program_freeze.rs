//! Validation and freezing of the closed executable-program artifact.

use ori_arc::ir::{
    ArcInstr, ArcTerminator, ArcValue, ArcVarId, YieldAllocationLocality, YieldExtent,
};
use ori_arc::{
    ArcClassification, ArcFunction, ClosureAdapterPlan, FreshSelfAllocationFacts,
    FunctionCallableFacts, FunctionEffectFacts, MemoryContract, ParamDisjointnessFacts,
    RetainPlanTable,
};
use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

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
        &parts.pool,
        |function, destination| {
            let function_id = function_ids.get(&function)?;
            let CallableTarget::Runtime(operation) =
                direct_call_targets.get(&(*function_id, destination))?
            else {
                return None;
            };
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
        },
    );
    let (length_projection_calls, length_projection_yields) = analyze_length_projections(
        &parts.functions,
        &parts.pool,
        &user_drop_plan,
        &function_ids,
        &direct_call_targets,
    );
    parts
        .repr_plan
        .set_length_projections(length_projection_calls, length_projection_yields);
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

fn analyze_length_projections(
    functions: &[ArcFunction],
    pool: &ori_types::Pool,
    user_drop_plan: &ExecutableDropPlan,
    function_ids: &FxHashMap<Name, FunctionId>,
    direct_targets: &FxHashMap<(FunctionId, ArcVarId), CallableTarget>,
) -> (FxHashMap<(Name, ArcVarId), Name>, FxHashMap<Name, ArcVarId>) {
    let observers = length_observers(functions, pool, function_ids, direct_targets);
    let classifier = ori_arc::ArcClassifier::new(pool);
    let mut yields = FxHashMap::default();
    for function in functions {
        let returned = function.yield_allocations.iter().filter(|fact| {
            fact.locality == YieldAllocationLocality::Escaping
                && matches!(fact.extent, YieldExtent::StaticExact(_))
                && classifier.is_scalar(fact.elem_ty)
                && user_drop_plan.get(fact.elem_ty, pool).is_none()
                && ori_arc::push_receiver_lineage_returned(function, fact.result)
        });
        let mut returned = returned.map(|fact| fact.result);
        let Some(result) = returned.next() else {
            continue;
        };
        if returned.next().is_none() {
            yields.insert(function.name, result);
        }
    }

    let mut calls = FxHashMap::default();
    for caller in functions {
        let caller_id = function_ids
            .get(&caller.name)
            .copied()
            .unwrap_or_else(|| unreachable!("validated executable function has no identity"));
        for block in &caller.blocks {
            for instruction in &block.body {
                let ArcInstr::Apply { dst, .. } = instruction else {
                    continue;
                };
                let Some(CallableTarget::Function(callee)) =
                    direct_targets.get(&(caller_id, *dst)).copied()
                else {
                    continue;
                };
                let callee_name = functions[callee.index()].name;
                if yields.contains_key(&callee_name)
                    && result_is_length_only(caller, caller_id, *dst, direct_targets, &observers)
                {
                    calls.insert((caller.name, *dst), callee_name);
                }
            }
            let ArcTerminator::Invoke { dst, .. } = &block.terminator else {
                continue;
            };
            let Some(CallableTarget::Function(callee)) =
                direct_targets.get(&(caller_id, *dst)).copied()
            else {
                continue;
            };
            let callee_name = functions[callee.index()].name;
            if yields.contains_key(&callee_name)
                && result_is_length_only(caller, caller_id, *dst, direct_targets, &observers)
            {
                calls.insert((caller.name, *dst), callee_name);
            }
        }
    }

    yields.retain(|callee, _| calls.values().any(|target| target == callee));
    (calls, yields)
}

fn length_observers(
    functions: &[ArcFunction],
    pool: &ori_types::Pool,
    function_ids: &FxHashMap<Name, FunctionId>,
    direct_targets: &FxHashMap<(FunctionId, ArcVarId), CallableTarget>,
) -> FxHashSet<FunctionId> {
    functions
        .iter()
        .filter_map(|function| {
            let id = function_ids.get(&function.name).copied()?;
            is_length_observer(function, id, pool, direct_targets).then_some(id)
        })
        .collect()
}

fn is_length_observer(
    function: &ArcFunction,
    function_id: FunctionId,
    pool: &ori_types::Pool,
    direct_targets: &FxHashMap<(FunctionId, ArcVarId), CallableTarget>,
) -> bool {
    let [parameter] = function.params.as_slice() else {
        return false;
    };
    if pool.tag(pool.resolve_fully(parameter.ty)) != ori_types::Tag::List {
        return false;
    }
    let aliases = alias_closure(function, parameter.var);
    let Some(length_result) =
        exact_length_observer_result(function, function_id, direct_targets, &aliases)
    else {
        return false;
    };
    length_result_is_returned_directly(function, length_result)
}

fn exact_length_observer_result(
    function: &ArcFunction,
    function_id: FunctionId,
    direct_targets: &FxHashMap<(FunctionId, ArcVarId), CallableTarget>,
    parameter_aliases: &FxHashSet<ArcVarId>,
) -> Option<ArcVarId> {
    let mut length_result = None;
    for block in &function.blocks {
        for instruction in &block.body {
            if !instruction
                .used_vars()
                .iter()
                .any(|var| parameter_aliases.contains(var))
            {
                continue;
            }
            let ArcInstr::Apply { dst, args, .. } = instruction else {
                if matches!(
                    instruction,
                    ArcInstr::Let {
                        value: ArcValue::Var(source),
                        ..
                    } if parameter_aliases.contains(source)
                ) {
                    continue;
                }
                return None;
            };
            if !matches!(
                direct_targets.get(&(function_id, *dst)),
                Some(CallableTarget::Runtime(super::RuntimeCall::Length))
            ) || args.len() != 1
                || !parameter_aliases.contains(&args[0])
                || length_result.replace(*dst).is_some()
            {
                return None;
            }
        }
        if block
            .terminator
            .used_vars()
            .iter()
            .any(|var| parameter_aliases.contains(var))
        {
            return None;
        }
    }
    length_result
}

fn length_result_is_returned_directly(function: &ArcFunction, length_result: ArcVarId) -> bool {
    let aliases = alias_closure(function, length_result);
    for block in &function.blocks {
        for instruction in &block.body {
            if !instruction
                .used_vars()
                .iter()
                .any(|var| aliases.contains(var))
            {
                continue;
            }
            if !matches!(
                instruction,
                ArcInstr::Let {
                    value: ArcValue::Var(source),
                    ..
                } if aliases.contains(source)
            ) {
                return false;
            }
        }
    }
    let mut return_count = 0;
    for block in &function.blocks {
        match &block.terminator {
            ArcTerminator::Return { value } if aliases.contains(value) => return_count += 1,
            ArcTerminator::Return { .. } => return false,
            terminator
                if terminator
                    .used_vars()
                    .iter()
                    .any(|var| aliases.contains(var)) =>
            {
                return false;
            }
            _ => {}
        }
    }
    return_count == 1
}

fn result_is_length_only(
    caller: &ArcFunction,
    caller_id: FunctionId,
    result: ArcVarId,
    direct_targets: &FxHashMap<(FunctionId, ArcVarId), CallableTarget>,
    observers: &FxHashSet<FunctionId>,
) -> bool {
    let aliases = alias_closure(caller, result);
    let mut observed = false;
    for block in &caller.blocks {
        for instruction in &block.body {
            if !instruction
                .used_vars()
                .iter()
                .any(|var| aliases.contains(var))
            {
                continue;
            }
            match instruction {
                ArcInstr::Let {
                    value: ArcValue::Var(source),
                    ..
                } if aliases.contains(source) => {}
                ArcInstr::RcDec { var, .. } | ArcInstr::BurdenDec { var }
                    if aliases.contains(var) => {}
                ArcInstr::Apply { dst, args, .. }
                    if call_is_length_observer(
                        caller_id,
                        *dst,
                        args,
                        &aliases,
                        observers,
                        direct_targets,
                    ) =>
                {
                    observed = true;
                }
                _ => return false,
            }
        }
        if block
            .terminator
            .used_vars()
            .iter()
            .any(|var| aliases.contains(var))
        {
            match &block.terminator {
                ArcTerminator::Invoke { dst, args, .. }
                    if call_is_length_observer(
                        caller_id,
                        *dst,
                        args,
                        &aliases,
                        observers,
                        direct_targets,
                    ) =>
                {
                    observed = true;
                }
                _ => return false,
            }
        }
    }
    observed
}

fn call_is_length_observer(
    caller: FunctionId,
    destination: ArcVarId,
    args: &[ArcVarId],
    aliases: &FxHashSet<ArcVarId>,
    observers: &FxHashSet<FunctionId>,
    direct_targets: &FxHashMap<(FunctionId, ArcVarId), CallableTarget>,
) -> bool {
    args.len() == 1
        && aliases.contains(&args[0])
        && matches!(
            direct_targets.get(&(caller, destination)),
            Some(CallableTarget::Function(target)) if observers.contains(target)
        )
}

fn alias_closure(function: &ArcFunction, seed: ArcVarId) -> FxHashSet<ArcVarId> {
    let mut aliases = FxHashSet::default();
    aliases.insert(seed);
    loop {
        let before = aliases.len();
        for block in &function.blocks {
            for instruction in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(source),
                    ..
                } = instruction
                {
                    if aliases.contains(source) || aliases.contains(dst) {
                        aliases.insert(*source);
                        aliases.insert(*dst);
                    }
                }
            }
        }
        if aliases.len() == before {
            return aliases;
        }
    }
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
