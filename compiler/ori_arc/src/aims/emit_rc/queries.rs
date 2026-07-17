//! Post-emission query passes: added-owner-credit tracking.
//!
//! Produces auxiliary data used by downstream pipeline steps (COW annotations,
//! drop hints).

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

#[cfg(test)]
mod tests;

/// Collect all variables whose logical storage identity received an added
/// owner credit through the transitional `RcInc` carrier or a sharing-view
/// call boundary whose runtime retains the borrowed backing storage.
///
/// Returns a set containing:
/// - Every variable `v` that has an `RcInc { var: v }` instruction
/// - The destination and every borrowed argument of an Apply/Invoke whose
///   contract carries `returns_sharing_view`. The retain is internal to the
///   producer runtime (for example `ori_list_slice`), so no ARC `RcInc`
///   spells it even though the source and returned view are competing owners.
/// - Every variable on a `Let { dst, value: Var(src) }` alias edge whose
///   OTHER end is in the set — BOTH directions, transitive through alias
///   chains. `dst` and `src` name the same logical storage identity, so an
///   added credit on either applies to both aliases: forward (src incremented →
///   dst incremented) covers reads through later aliases; backward (dst
///   incremented → src incremented, re-distributed to every sibling by the
///   forward pass) covers a kept duplication-alias inc (the RL-1 funded
///   call-arg / store families keep the inc on the ALIAS while the root and
///   its other aliases stay live — classifying those as un-incremented would
///   promote a later COW site to `StaticUnique` despite a competing owner)
/// - Every block parameter that receives an incremented variable through
///   a `Jump { target, args }` terminator (phi-edge propagation)
/// - Every Select operand (`true_val` / `false_val`) when the `dst` is
///   incremented. `Select dst` is one of `{true_val, false_val}`, so an added
///   credit on `dst` belongs to the chosen operand's logical identity. The
///   pre-event `Unique` classification is no longer valid for that identity.
///   Propagation conservatively marks both operands because either can be
///   chosen.
/// - Every whole-variable member of the same frozen birth-site class. This
///   carries a credit across Construct-field / Project identities: an inc on
///   the value moved into a struct field also invalidates pre-event uniqueness
///   on a later projection of that field. The aggregate's whole variable is
///   not in the field class and is therefore not conflated with its payload.
///
/// Used by both COW annotations and drop hints: after logical event
/// realization, a variable in this set may have a competing owner credit. Its
/// pre-event AIMS uniqueness state therefore cannot authorize single-owner
/// mutation or cleanup.
pub(crate) fn collect_rc_incremented_vars(
    func: &ArcFunction,
    birth_site_partition: Option<&BirthSitePartition>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashSet<ArcVarId> {
    use crate::ir::ArcTerminator;

    let mut incremented = FxHashSet::default();
    let mut alias_edges: Vec<Option<ArcVarId>> = vec![None; func.var_types.len()];
    // Select operand aliases: dst → (true_val, false_val). Propagation in the
    // fixed-point loop below mirrors `Let { Var(src) }` but edges TWO ways
    // because a Select's dst aliases EITHER operand at runtime.
    let mut select_aliases: Vec<(ArcVarId, ArcVarId, ArcVarId)> = Vec::new();

    // Freeze whole-variable members by birth-site representative up front.
    // `BirthSitePartition` uses path-compressing queries, so work on a clone;
    // the converged side table remains immutable per PL-5.
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

        // Phi-edge propagation: Jump args map to target block params.
        // A block param may receive values from multiple predecessors
        // (e.g., loop header from entry + back-edge). If ANY source is
        // incremented, the param inherits the flag. Single-source cases
        // record an `alias_edges` entry; multi-source uses direct insertion.
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
                                // Multi-predecessor: param already has an alias
                                // edge. If the new arg OR the existing source is
                                // incremented, mark the param directly. This
                                // handles loop headers where one predecessor
                                // passes an incremented var and the back-edge
                                // passes the param itself.
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

/// Seed the physical competing-owner fact created inside a sharing-view
/// producer. `returns_sharing_view` is path-universal typed provenance: the
/// destination receives a new owner credit backed by one of the borrowed
/// inputs. The contract does not yet name which borrowed parameter supplies
/// that backing identity, so mark every borrowed parameter conservatively.
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

/// Collect borrowed function parameter variables and their Let-alias
/// transitive closure.
///
/// Returns a set containing every function parameter with
/// `Ownership::Borrowed` plus all `Let { dst, value: Var(src) }` aliases
/// that transitively derive from them. Used by `decide_drop_hint` to
/// prevent unique-drop on borrowed params (the caller retains a reference,
/// so the buffer is never uniquely owned by the callee).
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
    // Transitive closure through Let aliases AND block params (Jump/Branch
    // args → target block params). Borrowed-param values flow through the
    // loop header and exit block as block parameters.
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            // Let aliases: %3 = %0 where %0 is a borrowed param.
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
            // Block param propagation: Jump args map to target block params.
            // If arg N is in the result set, add target's block param N.
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
