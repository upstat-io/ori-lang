//! Closed-program projection for list results observed only through their length.

use ori_arc::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, YieldAllocationLocality, YieldExtent,
};
use ori_arc::{ArcClassification, ArcClassifier};
use ori_ir::{Name, StringInterner};
use ori_repr::executable::{CallableTarget, ExecutableProgram, FunctionId, RuntimeCall};
use rustc_hash::{FxHashMap, FxHashSet};

/// A call-site redirect plus the returned yield identity virtualized in its clone.
pub(super) struct LengthProjections {
    pub(super) calls: FxHashMap<(Name, ArcVarId), Name>,
    pub(super) yields: FxHashMap<Name, ArcVarId>,
}

/// Find direct list-returning calls whose result is used only by `length` and release.
///
/// The ordinary callee remains intact. LLVM emits a private physical clone for the
/// qualified call sites, preserving the source ABI while replacing only the returned
/// yield payload with its element count. Every unsupported use fails closed.
pub(super) fn analyze_length_projections(
    program: &ExecutableProgram,
    interner: &StringInterner,
) -> LengthProjections {
    let observers = length_observers(program, interner);
    let classifier = ArcClassifier::new(program.pool());

    let mut yields = FxHashMap::default();
    for function in program.functions() {
        let returned = function.yield_allocations.iter().filter(|fact| {
            fact.locality == YieldAllocationLocality::Escaping
                && matches!(fact.extent, YieldExtent::StaticExact(_))
                && classifier.is_scalar(fact.elem_ty)
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
    for caller in program.functions() {
        let caller_id = program
            .function_id(caller.name)
            .unwrap_or_else(|| unreachable!("validated executable function has no identity"));
        for block in &caller.blocks {
            for instruction in &block.body {
                let ArcInstr::Apply { dst, .. } = instruction else {
                    continue;
                };
                let Some(CallableTarget::Function(callee)) =
                    program.direct_call_target(caller_id, *dst)
                else {
                    continue;
                };
                let callee_name = program.function(callee).name;
                if yields.contains_key(&callee_name)
                    && result_is_length_only(program, caller_id, caller, *dst, &observers)
                {
                    calls.insert((caller.name, *dst), callee_name);
                }
            }
            let ArcTerminator::Invoke { dst, .. } = &block.terminator else {
                continue;
            };
            let Some(CallableTarget::Function(callee)) =
                program.direct_call_target(caller_id, *dst)
            else {
                continue;
            };
            let callee_name = program.function(callee).name;
            if yields.contains_key(&callee_name)
                && result_is_length_only(program, caller_id, caller, *dst, &observers)
            {
                calls.insert((caller.name, *dst), callee_name);
            }
        }
    }

    yields.retain(|callee, _| calls.values().any(|target| target == callee));
    LengthProjections { calls, yields }
}

fn length_observers(
    program: &ExecutableProgram,
    interner: &StringInterner,
) -> FxHashSet<FunctionId> {
    program
        .functions()
        .iter()
        .filter_map(|function| {
            let id = program.function_id(function.name)?;
            is_length_observer(program, id, function, interner).then_some(id)
        })
        .collect()
}

fn is_length_observer(
    program: &ExecutableProgram,
    function_id: FunctionId,
    function: &ArcFunction,
    interner: &StringInterner,
) -> bool {
    let [parameter] = function.params.as_slice() else {
        return false;
    };
    if program
        .pool()
        .tag(program.pool().resolve_fully(parameter.ty))
        != ori_types::Tag::List
    {
        return false;
    }

    let aliases = alias_closure(function, parameter.var);
    let Some(length_result) =
        exact_length_observer_result(program, function_id, function, interner, &aliases)
    else {
        return false;
    };
    length_result_is_returned_directly(function, length_result)
}

fn exact_length_observer_result(
    program: &ExecutableProgram,
    function_id: FunctionId,
    function: &ArcFunction,
    interner: &StringInterner,
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
            let ArcInstr::Apply {
                dst, func, args, ..
            } = instruction
            else {
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
            let runtime_length = matches!(
                program.direct_call_target(function_id, *dst),
                Some(CallableTarget::Runtime(RuntimeCall::Length))
            );
            tracing::trace!(
                observer = %interner.lookup(function.name),
                callee = %interner.lookup(*func),
                target = ?program.direct_call_target(function_id, *dst),
                runtime_length,
                "examined length-observer call"
            );
            if !runtime_length
                || args.len() != 1
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
    let length_aliases = alias_closure(function, length_result);

    for block in &function.blocks {
        for instruction in &block.body {
            if !instruction
                .used_vars()
                .iter()
                .any(|var| length_aliases.contains(var))
            {
                continue;
            }
            if !matches!(
                instruction,
                ArcInstr::Let {
                    value: ArcValue::Var(source),
                    ..
                } if length_aliases.contains(source)
            ) {
                return false;
            }
        }
    }

    let mut return_count = 0;
    for block in &function.blocks {
        match &block.terminator {
            ArcTerminator::Return { value } if length_aliases.contains(value) => {
                return_count += 1;
            }
            ArcTerminator::Return { .. } => return false,
            terminator
                if terminator
                    .used_vars()
                    .iter()
                    .any(|var| length_aliases.contains(var)) =>
            {
                return false;
            }
            _ => {}
        }
    }
    return_count == 1
}

fn result_is_length_only(
    program: &ExecutableProgram,
    caller_id: FunctionId,
    caller: &ArcFunction,
    result: ArcVarId,
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
                        program, caller_id, *dst, args, &aliases, observers,
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
                        program, caller_id, *dst, args, &aliases, observers,
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
    program: &ExecutableProgram,
    caller: FunctionId,
    destination: ArcVarId,
    args: &[ArcVarId],
    aliases: &FxHashSet<ArcVarId>,
    observers: &FxHashSet<FunctionId>,
) -> bool {
    args.len() == 1
        && aliases.contains(&args[0])
        && matches!(
            program.direct_call_target(caller, destination),
            Some(CallableTarget::Function(target)) if observers.contains(&target)
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

/// Mint the clone identity only after the closed analysis has qualified a callee.
pub(super) fn projection_name(interner: &StringInterner, callee: Name) -> Name {
    interner.intern(&format!("{}$length_only", interner.lookup(callee)))
}
