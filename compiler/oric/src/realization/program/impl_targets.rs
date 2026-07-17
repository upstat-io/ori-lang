//! Exact impl-target rewrites for typed method-call sites.

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator};
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use super::super::method_receiver_key;

pub(crate) fn rewrite_impl_targets(
    functions: &mut [ArcFunction],
    targets: &FxHashMap<(Idx, Name), Name>,
    pool: &Pool,
) {
    for function in functions {
        let method_call_facts = &function.method_call_facts;
        for block in &mut function.blocks {
            for instruction in &mut block.body {
                if let ArcInstr::Apply { dst, func, .. } = instruction {
                    rewrite_impl_target(func, *dst, method_call_facts, targets, pool);
                }
            }
            if let ArcTerminator::Invoke { dst, func, .. } = &mut block.terminator {
                rewrite_impl_target(func, *dst, method_call_facts, targets, pool);
            }
        }
    }
}

fn rewrite_impl_target(
    target: &mut Name,
    destination: ori_arc::ArcVarId,
    method_call_facts: &[ori_arc::MethodCallFact],
    impl_targets: &FxHashMap<(Idx, Name), Name>,
    pool: &Pool,
) {
    let Some(fact) = method_call_facts
        .iter()
        .find(|fact| fact.destination == destination)
    else {
        return;
    };
    let key = (method_receiver_key(pool, fact.receiver_type), *target);
    if let Some(&qualified) = impl_targets.get(&key) {
        *target = qualified;
    }
}
