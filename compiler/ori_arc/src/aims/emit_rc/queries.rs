//! Post-emission query passes: added-owner-credit tracking.
//!
//! Produces auxiliary data used by COW annotation and drop-hint passes.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

#[cfg(test)]
mod tests;

/// Collects logical storage identities that have a competing owner credit.
///
/// Credits propagate from `RcInc` and sharing-view calls through aliases,
/// block parameters, selections, and frozen birth-site classes. COW and drop
/// decisions cannot treat the returned identities as single-owner values.
pub(crate) fn collect_rc_incremented_vars(
    func: &ArcFunction,
    birth_site_partition: Option<&BirthSitePartition>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashSet<ArcVarId> {
    use crate::ir::ArcTerminator;

    let mut incremented = FxHashSet::default();
    let mut alias_edges: Vec<Option<ArcVarId>> = vec![None; func.var_types.len()];
    let mut select_aliases: Vec<(ArcVarId, ArcVarId, ArcVarId)> = Vec::new();

    // Why: Representative lookup path-compresses, while the converged side table stays immutable.
    let birth_class_members = collect_birth_class_members(birth_site_partition);

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } => seed_sharing_view_owners(*dst, *callee, args, contracts, &mut incremented),
                ArcInstr::RcInc { var, .. } => {
                    incremented.insert(*var);
                }
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    alias_edges[dst.index()] = Some(*src);
                }
                ArcInstr::Select {
                    dst,
                    true_val,
                    false_val,
                    ..
                } => {
                    select_aliases.push((*dst, *true_val, *false_val));
                }
                _ => {}
            }
        }

        // INVARIANT: A block parameter is credited when any incoming jump argument is credited.
        match &block.terminator {
            ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } => seed_sharing_view_owners(*dst, *callee, args, contracts, &mut incremented),
            ArcTerminator::Jump { target, args } => {
                let target_idx = target.index();
                if target_idx < func.blocks.len() {
                    for (i, &arg) in args.iter().enumerate() {
                        if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                            if let Some(existing) = alias_edges[param_var.index()] {
                                // INVARIANT: Existing and new predecessor sources both contribute.
                                if incremented.contains(&arg) || incremented.contains(&existing) {
                                    incremented.insert(param_var);
                                }
                            } else {
                                alias_edges[param_var.index()] = Some(arg);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    propagate_incremented_vars(
        &alias_edges,
        &select_aliases,
        &birth_class_members,
        &mut incremented,
    );

    incremented
}

fn propagate_incremented_vars(
    alias_edges: &[Option<ArcVarId>],
    select_aliases: &[(ArcVarId, ArcVarId, ArcVarId)],
    birth_class_members: &[Vec<ArcVarId>],
    incremented: &mut FxHashSet<ArcVarId>,
) {
    loop {
        let mut changed = false;
        for (i, alias) in alias_edges.iter().enumerate() {
            if let Some(src) = alias {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "ARC IR var counts fit in u32"
                )]
                let dst = ArcVarId::new(i as u32);
                if incremented.contains(src) && incremented.insert(dst) {
                    changed = true;
                }
                if incremented.contains(&dst) && incremented.insert(*src) {
                    changed = true;
                }
            }
        }
        for &(dst, true_val, false_val) in select_aliases {
            if incremented.contains(&dst) {
                if incremented.insert(true_val) {
                    changed = true;
                }
                if incremented.insert(false_val) {
                    changed = true;
                }
            }
        }
        for members in birth_class_members {
            if members.iter().any(|var| incremented.contains(var)) {
                for &var in members {
                    if incremented.insert(var) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

fn collect_birth_class_members(
    birth_site_partition: Option<&BirthSitePartition>,
) -> Vec<Vec<ArcVarId>> {
    birth_site_partition.map_or_else(Vec::new, |partition| {
        let mut partition = partition.clone();
        let mut by_rep: FxHashMap<NodeIdx, Vec<ArcVarId>> = FxHashMap::default();
        for (var, path, node) in partition.nodes_snapshot() {
            if path.is_whole_var() {
                by_rep.entry(partition.rep_of(node)).or_default().push(var);
            }
        }
        by_rep.into_values().collect()
    })
}

/// Seeds a sharing-view result and every borrowed argument with an owner credit.
///
/// `returns_sharing_view` does not identify which borrowed argument backs the result.
fn seed_sharing_view_owners(
    dst: ArcVarId,
    callee: Name,
    args: &[ArcVarId],
    contracts: &FxHashMap<Name, MemoryContract>,
    incremented: &mut FxHashSet<ArcVarId>,
) {
    use crate::aims::lattice::AccessClass;

    let Some(contract) = contracts
        .get(&callee)
        .filter(|contract| contract.return_info.returns_sharing_view)
    else {
        return;
    };
    incremented.insert(dst);
    for (&arg, param) in args.iter().zip(&contract.params) {
        if param.access == AccessClass::Borrowed {
            incremented.insert(arg);
        }
    }
}

/// Collects borrowed parameters and aliases that transitively derive from them.
///
/// The result prevents unique-drop while the caller retains a reference.
pub(crate) fn collect_param_borrowed_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    use crate::ownership::Ownership;

    let mut result = FxHashSet::default();
    for param in &func.params {
        if param.ownership == Ownership::Borrowed {
            result.insert(param.var);
        }
    }
    if result.is_empty() {
        return result;
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if result.contains(src) && result.insert(*dst) {
                        changed = true;
                    }
                }
            }
            let targets: Vec<(usize, &[ArcVarId])> = match &block.terminator {
                crate::ir::ArcTerminator::Jump { target, args } => {
                    vec![(target.index(), args)]
                }
                _ => vec![],
            };
            for (target_idx, args) in targets {
                if target_idx >= func.blocks.len() {
                    continue;
                }
                let target_block = &func.blocks[target_idx];
                for (arg_pos, arg_var) in args.iter().enumerate() {
                    if result.contains(arg_var) && arg_pos < target_block.params.len() {
                        let (param_var, _) = target_block.params[arg_pos];
                        if result.insert(param_var) {
                            changed = true;
                        }
                    }
                }
            }
        }
    }
    result
}
