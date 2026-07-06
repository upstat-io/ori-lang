//! Population pass for the per-`(ArcVarId, FieldPath)` birth-site partition.
//!
//! Walks the ARC IR once, admitting edges per the T1 partition calculus
//! (`AimsProof.Partition`). Tier-1 unconditional same-allocation edges union
//! directly: Let-Var whole-var aliases, Project field-path composition,
//! Construct field funding, and contract-proven Apply/Invoke result aliases
//! (`ApplyAliasSource::Direct` / `::Project`). Block-param merges and
//! `ApplyAliasSource::Conditional` aliases are admitted ONLY under the
//! singleton birth-site witness, iterated to a fixpoint so chained merges
//! compose. Birth sites are minted per `Construct` site; every other fresh
//! definition stays UNKNOWN, so a merge over it refuses conservatively.
//! COW-mutating uses (`Set` / `SetTag`) taint their class as a boundary.
//! Scalar / immortal vars carry no allocation and are skipped.

use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::birth_site_partition::{BirthSiteId, BirthSitePartition, FieldPath, NodeIdx};
use super::state_map::{AimsStateMap, ApplyAliasSource};

/// Build the birth-site partition for `func` from its converged state map.
///
/// Reads `state_map` for the scalar/immortal exclusion set and the pre-walk
/// `apply_result_aliases` side table; writes nothing. The caller installs
/// the result via [`AimsStateMap::set_birth_site_partition`]; the table is
/// read-only thereafter (PL-5).
pub(crate) fn compute_birth_site_partition(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> BirthSitePartition {
    let mut partition = BirthSitePartition::new();
    let mut next_site: u32 = 0;

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { dst, args, .. } => {
                    if state_map.is_excluded(*dst) {
                        continue;
                    }
                    let dst_node = whole_node(&mut partition, *dst);
                    partition.set(dst_node, BirthSiteId::new(next_site));
                    next_site += 1;
                    // Field funding: constructor arg positions ARE the field
                    // indices `Project { field }` reads back.
                    for (position, &arg) in args.iter().enumerate() {
                        if state_map.is_excluded(arg) {
                            continue;
                        }
                        let Ok(field) = u32::try_from(position) else {
                            unreachable!("constructor arg position exceeds u32::MAX");
                        };
                        let field_node = partition.register_node(*dst, FieldPath::single(field));
                        let arg_node = whole_node(&mut partition, arg);
                        partition.union_tier1(field_node, arg_node);
                    }
                }
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    if state_map.is_excluded(*dst) || state_map.is_excluded(*src) {
                        continue;
                    }
                    let dst_node = whole_node(&mut partition, *dst);
                    let src_node = whole_node(&mut partition, *src);
                    partition.union_tier1(dst_node, src_node);
                }
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Literal(_),
                    ..
                } => {
                    // A non-excluded heap literal (a non-empty string;
                    // scalars and immortals are state-map-excluded) is a
                    // fresh allocation site, minted like a Construct.
                    if state_map.is_excluded(*dst) {
                        continue;
                    }
                    let dst_node = whole_node(&mut partition, *dst);
                    partition.set(dst_node, BirthSiteId::new(next_site));
                    next_site += 1;
                }
                ArcInstr::Project {
                    dst, value, field, ..
                } => {
                    if state_map.is_excluded(*dst) {
                        continue;
                    }
                    // Single-hop composition: multi-hop chains compose
                    // transitively through the union-find, never through
                    // hand-built multi-hop paths.
                    let dst_node = whole_node(&mut partition, *dst);
                    let field_node = partition.register_node(*value, FieldPath::single(*field));
                    partition.union_tier1(dst_node, field_node);
                }
                ArcInstr::Set { base, field, .. } => {
                    if state_map.is_excluded(*base) {
                        continue;
                    }
                    let base_node = whole_node(&mut partition, *base);
                    partition.mark_cow_boundary(base_node);
                    let field_node = partition.register_node(*base, FieldPath::single(*field));
                    partition.mark_cow_boundary(field_node);
                }
                ArcInstr::SetTag { base, .. } => {
                    if state_map.is_excluded(*base) {
                        continue;
                    }
                    let base_node = whole_node(&mut partition, *base);
                    partition.mark_cow_boundary(base_node);
                }
                _ => {}
            }
        }
    }

    let mut merges = collect_contract_alias_edges(&mut partition, state_map);
    collect_block_param_edges(&mut partition, func, state_map, &mut merges);
    admit_witnessed_merges(&mut partition, &merges);

    partition
}

/// Intern the whole-variable node for `var`.
fn whole_node(partition: &mut BirthSitePartition, var: ArcVarId) -> NodeIdx {
    partition.register_node(var, FieldPath::whole_var())
}

/// Admit contract-proven Apply/Invoke result aliases.
///
/// `Direct` (whole-var passthrough) and `Project` (single-field-of-param
/// return) are tier-1: the contract proves the result names the SAME
/// allocation. `Wrapped` is containment — the result is a SEPARATE
/// allocation — so no edge. `Conditional` is a merge over candidates
/// (the result aliases ONE of N at runtime) and joins the returned
/// witnessed-merge set. Sorted by destination for deterministic unions.
fn collect_contract_alias_edges(
    partition: &mut BirthSitePartition,
    state_map: &AimsStateMap,
) -> Vec<(ArcVarId, Vec<ArcVarId>)> {
    let mut aliases: Vec<(ArcVarId, &ApplyAliasSource)> = state_map
        .apply_result_aliases()
        .iter()
        .map(|(&dst, source)| (dst, source))
        .collect();
    aliases.sort_by_key(|&(dst, _)| dst.raw());

    let mut merges: Vec<(ArcVarId, Vec<ArcVarId>)> = Vec::new();
    for (dst, source) in aliases {
        if state_map.is_excluded(dst) {
            continue;
        }
        match source {
            ApplyAliasSource::Direct(arg) => {
                if state_map.is_excluded(*arg) {
                    continue;
                }
                let dst_node = whole_node(partition, dst);
                let arg_node = whole_node(partition, *arg);
                partition.union_tier1(dst_node, arg_node);
            }
            ApplyAliasSource::Project { arg, field } => {
                if state_map.is_excluded(*arg) {
                    continue;
                }
                let dst_node = whole_node(partition, dst);
                let field_node = partition.register_node(*arg, FieldPath::single(*field));
                partition.union_tier1(dst_node, field_node);
            }
            ApplyAliasSource::Wrapped(_) => {}
            ApplyAliasSource::Conditional { candidates } => {
                merges.push((dst, candidates.clone()));
            }
        }
    }
    merges
}

/// Route Jump-arg -> block-param edges.
///
/// A single-predecessor param is a pure renaming of its one arg (tier-1);
/// a multi-predecessor param is a phi merge and joins the witnessed set.
fn collect_block_param_edges(
    partition: &mut BirthSitePartition,
    func: &ArcFunction,
    state_map: &AimsStateMap,
    merges: &mut Vec<(ArcVarId, Vec<ArcVarId>)>,
) {
    let mut incoming: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut param_order: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for &(param, _) in &block.params {
            param_order.push(param);
        }
    }
    for block in &func.blocks {
        let ArcTerminator::Jump { target, args } = &block.terminator else {
            continue;
        };
        let Some(target_block) = func.blocks.get(target.index()) else {
            continue;
        };
        for (&arg, &(param, _)) in args.iter().zip(target_block.params.iter()) {
            incoming.entry(param).or_default().push(arg);
        }
    }

    for param in param_order {
        if state_map.is_excluded(param) {
            continue;
        }
        let Some(args) = incoming.remove(&param) else {
            continue;
        };
        if let [only_arg] = args.as_slice() {
            if state_map.is_excluded(*only_arg) {
                continue;
            }
            let param_node = whole_node(partition, param);
            let arg_node = whole_node(partition, *only_arg);
            partition.union_tier1(param_node, arg_node);
        } else {
            merges.push((param, args));
        }
    }
}

/// Admit the witnessed merges to a fixpoint.
///
/// Per merge, predecessors already in the merge node's class are dropped
/// first: a threaded-back self value holds the merge's own prior-iteration
/// value, so (by induction over iterations) its birth site equals the
/// admitted class's — the loop-invariant back-edge shape
/// (`T1_items_header_unified_across_backedge`). The singleton witness runs
/// over the remaining predecessors; an admission can enable a chained
/// merge, so the loop repeats until no admission fires.
fn admit_witnessed_merges(
    partition: &mut BirthSitePartition,
    merges: &[(ArcVarId, Vec<ArcVarId>)],
) {
    let mut changed = true;
    while changed {
        changed = false;
        for (merge_var, args) in merges {
            let merge_node = whole_node(partition, *merge_var);
            let mut preds: Vec<NodeIdx> = Vec::with_capacity(args.len());
            for &arg in args {
                let arg_node = whole_node(partition, arg);
                if partition.same_rep(arg_node, merge_node) {
                    continue;
                }
                preds.push(arg_node);
            }
            if preds.is_empty() {
                continue;
            }
            if partition.union_phi_witnessed(merge_node, &preds) {
                changed = true;
            }
        }
    }
}

#[cfg(test)]
mod tests;
