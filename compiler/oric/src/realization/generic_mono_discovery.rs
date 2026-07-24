//! Generic call and function-value use discovery over specialized ARC probes.

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator, CtorKind};
use ori_ir::Name;
use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashMap;

use super::generic_mono_closure::{GenericSignature, GenericUse};
use super::ArcFunctionGroup;

pub(super) fn collect_generic_uses(
    groups: &[ArcFunctionGroup],
    signatures: &FxHashMap<Name, GenericSignature<'_>>,
    pool: &Pool,
) -> Vec<GenericUse> {
    let mut uses = Vec::new();
    for function in groups.iter().flat_map(ArcFunctionGroup::bodies) {
        for block in &function.blocks {
            for instruction in &block.body {
                collect_instruction_use(function, instruction, signatures, pool, &mut uses);
            }
            if let ArcTerminator::Invoke {
                dst,
                ty,
                func,
                args,
                ..
            } = &block.terminator
            {
                collect_direct_use(function, *dst, *ty, *func, args, signatures, &mut uses);
            }
        }
    }
    uses.sort_by_key(|use_| generic_use_key(use_, pool));
    uses.dedup();
    uses
}

fn collect_instruction_use(
    function: &ArcFunction,
    instruction: &ArcInstr,
    signatures: &FxHashMap<Name, GenericSignature<'_>>,
    pool: &Pool,
    uses: &mut Vec<GenericUse>,
) {
    match instruction {
        ArcInstr::Apply {
            dst,
            ty,
            func,
            args,
            ..
        } => collect_direct_use(function, *dst, *ty, *func, args, signatures, uses),
        ArcInstr::PartialApply { ty, func, args, .. }
        | ArcInstr::Construct {
            ty,
            ctor: CtorKind::Closure { func },
            args,
            ..
        } => collect_function_value_use(function, *ty, *func, args, signatures, pool, uses),
        _ => {}
    }
}

fn collect_direct_use(
    function: &ArcFunction,
    destination: ori_arc::ArcVarId,
    return_type: Idx,
    callee: Name,
    arguments: &[ori_arc::ArcVarId],
    signatures: &FxHashMap<Name, GenericSignature<'_>>,
    uses: &mut Vec<GenericUse>,
) {
    if !signatures.contains_key(&callee)
        || function.method_call_fact(destination).is_some()
        || function
            .operator_call_facts
            .iter()
            .any(|fact| fact.destination == destination)
    {
        return;
    }
    uses.push(GenericUse {
        callee,
        param_types: arguments
            .iter()
            .map(|argument| function.var_type(*argument))
            .collect(),
        return_type,
    });
}

fn collect_function_value_use(
    function: &ArcFunction,
    function_type: Idx,
    callee: Name,
    captured: &[ori_arc::ArcVarId],
    signatures: &FxHashMap<Name, GenericSignature<'_>>,
    pool: &Pool,
    uses: &mut Vec<GenericUse>,
) {
    let Some(signature) = signatures.get(&callee).map(|source| source.signature) else {
        return;
    };
    let function_type = pool.resolve_fully(function_type);
    if pool.tag(function_type) != Tag::Function {
        return;
    }
    let mut param_types: Vec<_> = captured
        .iter()
        .map(|argument| function.var_type(*argument))
        .collect();
    param_types.extend_from_slice(&pool.function_params(function_type));
    if param_types.len() != signature.param_types.len() {
        return;
    }
    uses.push(GenericUse {
        callee,
        param_types,
        return_type: pool.function_return(function_type),
    });
}

fn generic_use_key(generic_use: &GenericUse, pool: &Pool) -> (u32, Vec<u64>, u64) {
    (
        generic_use.callee.raw(),
        generic_use
            .param_types
            .iter()
            .map(|&ty| pool.hash(pool.resolve_fully(ty)))
            .collect(),
        pool.hash(pool.resolve_fully(generic_use.return_type)),
    )
}
