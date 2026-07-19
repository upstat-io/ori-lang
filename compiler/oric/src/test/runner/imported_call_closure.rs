//! Lexical target rewriting and reachability closure for LLVM JIT imports.

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator, CtorKind};
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::realization::ArcFunctionGroup;

/// Rewrite source-visible call operands to collision-proof module identities.
///
/// Imported canons own independent mono-instance tables, and one unspecialized
/// generic body can be lowered for several concrete host instances. Numeric
/// dispatch IDs are therefore cleared on those bodies; the shared resolver
/// selects the target from specialized types and producer-qualified facts.
pub(super) fn rewrite_group(
    group: ArcFunctionGroup,
    targets: &FxHashMap<Name, Name>,
    clear_dispatch_ids: bool,
) -> ArcFunctionGroup {
    let (mut parent, mut lambdas) = group.into_parts();
    rewrite_function(&mut parent, targets, clear_dispatch_ids);
    for lambda in &mut lambdas {
        rewrite_function(lambda, targets, clear_dispatch_ids);
    }
    ArcFunctionGroup::new(parent, lambdas)
}

pub(super) fn rewrite_lowered_body(
    (mut parent, mut lambdas): (ArcFunction, Vec<ArcFunction>),
    targets: &FxHashMap<Name, Name>,
    clear_dispatch_ids: bool,
) -> (ArcFunction, Vec<ArcFunction>) {
    rewrite_function(&mut parent, targets, clear_dispatch_ids);
    for lambda in &mut lambdas {
        rewrite_function(lambda, targets, clear_dispatch_ids);
    }
    (parent, lambdas)
}

pub(super) fn rewrite_function(
    function: &mut ArcFunction,
    targets: &FxHashMap<Name, Name>,
    clear_dispatch_ids: bool,
) {
    let typed_dispatch_destinations: FxHashSet<_> = function
        .method_call_facts
        .iter()
        .map(|fact| fact.destination)
        .chain(
            function
                .operator_call_facts
                .iter()
                .map(|fact| fact.destination),
        )
        .collect();
    for block in &mut function.blocks {
        for instruction in &mut block.body {
            match instruction {
                ArcInstr::Apply {
                    dst,
                    func,
                    mono_instance_id,
                    ..
                } => {
                    if !typed_dispatch_destinations.contains(dst) {
                        if let Some(&target) = targets.get(func) {
                            *func = target;
                        }
                    }
                    if clear_dispatch_ids {
                        *mono_instance_id = None;
                    }
                }
                ArcInstr::PartialApply { func, .. }
                | ArcInstr::Construct {
                    ctor: CtorKind::Closure { func },
                    ..
                } => {
                    if let Some(&target) = targets.get(func) {
                        *func = target;
                    }
                }
                _ => {}
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func,
            mono_instance_id,
            ..
        } = &mut block.terminator
        {
            if !typed_dispatch_destinations.contains(dst) {
                if let Some(&target) = targets.get(func) {
                    *func = target;
                }
            }
            if clear_dispatch_ids {
                *mono_instance_id = None;
            }
        }
    }
}

/// Retain exactly the imported parent families reachable from the root batch.
pub(super) fn close_reachable_imports(
    roots: &[ArcFunctionGroup],
    candidates: Vec<ArcFunctionGroup>,
) -> (Vec<ArcFunctionGroup>, FxHashSet<Name>) {
    let candidate_indices: FxHashMap<_, _> = candidates
        .iter()
        .enumerate()
        .map(|(index, group)| (group.parent_name(), index))
        .collect();
    let mut pending = Vec::new();
    for function in roots.iter().flat_map(ArcFunctionGroup::bodies) {
        collect_direct_targets(function, &mut pending);
    }

    let mut reachable = FxHashSet::default();
    while let Some(target) = pending.pop() {
        let Some(&index) = candidate_indices.get(&target) else {
            continue;
        };
        if !reachable.insert(target) {
            continue;
        }
        for function in candidates[index].bodies() {
            collect_direct_targets(function, &mut pending);
        }
    }

    let retained = candidates
        .into_iter()
        .filter(|group| reachable.contains(&group.parent_name()))
        .collect();
    (retained, reachable)
}

fn collect_direct_targets(function: &ArcFunction, targets: &mut Vec<Name>) {
    for block in &function.blocks {
        for instruction in &block.body {
            match instruction {
                ArcInstr::Apply { func, .. }
                | ArcInstr::PartialApply { func, .. }
                | ArcInstr::Construct {
                    ctor: CtorKind::Closure { func },
                    ..
                } => targets.push(*func),
                _ => {}
            }
        }
        if let ArcTerminator::Invoke { func, .. } = block.terminator {
            targets.push(func);
        }
    }
}

#[cfg(test)]
mod tests;
