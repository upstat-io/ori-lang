use ori_arc::ir::{
    ArcInstr, ArcTerminator, ArcValue, ArcVarId, YieldAllocationLocality, YieldExtent,
};
use ori_arc::{ArcClassification, ArcFunction};
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::{CallableTarget, ExecutableDropPlan, FunctionId};

pub(super) fn analyze_length_projections(
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
                Some(CallableTarget::Runtime(super::super::RuntimeCall::Length))
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
