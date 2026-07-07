//! Population pass for the per-`(ArcVarId, FieldPath)` birth-site partition.
//!
//! Walks the ARC IR once, admitting edges per the T1 partition calculus
//! (`AimsProof.Partition`). Tier-1 unconditional same-allocation edges union
//! directly: Let-Var whole-var aliases, Project field-path composition,
//! Construct field funding, and contract-proven Apply/Invoke result aliases
//! (`ApplyAliasSource::Direct` / `::Project`). Block-param merges and
//! `ApplyAliasSource::Conditional` aliases are admitted ONLY under the
//! singleton birth-site witness, iterated to a fixpoint so chained merges
//! compose. Birth sites are minted per `Construct` site, per heap-producing
//! literal/PrimOp `Let`, and per NON-aliased call result (an alias-free
//! `Apply`/`Invoke` destination is the caller-frame acquisition of a fresh
//! allocation — the event classifier births it, so the partition sites it);
//! every other fresh definition stays UNKNOWN, so a merge over it refuses
//! conservatively.
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
                ArcInstr::Let { dst, value, .. }
                    if matches!(value, ArcValue::PrimOp { .. })
                        || matches!(value, ArcValue::Literal(lit)
                            if matches!(lit, crate::ir::LitValue::String(_))) =>
                {
                    // A non-excluded STRING literal (the empty string is
                    // immortal-excluded) or a heap-producing PrimOp result
                    // (string concat) is a fresh allocation site, minted
                    // like a Construct. Non-string literals allocate
                    // nothing at runtime even under a heap-repr variable.
                    if state_map.is_excluded(*dst) {
                        continue;
                    }
                    let dst_node = whole_node(&mut partition, *dst);
                    partition.set(dst_node, BirthSiteId::new(next_site));
                    next_site += 1;
                }
                ArcInstr::Apply { dst, .. } | ArcInstr::ApplyIndirect { dst, .. } => {
                    mint_call_result_site(&mut partition, state_map, *dst, &mut next_site);
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
        match &block.terminator {
            ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } => {
                mint_call_result_site(&mut partition, state_map, *dst, &mut next_site);
            }
            _ => {}
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

/// Mint a birth site for a NON-aliased call result. An alias-carrying
/// destination (`ApplyAliasSource`) joins its source's class and inherits
/// that class's site; minting a second site there is skipped.
fn mint_call_result_site(
    partition: &mut BirthSitePartition,
    state_map: &AimsStateMap,
    dst: ArcVarId,
    next_site: &mut u32,
) {
    if state_map.is_excluded(dst) || state_map.apply_result_alias(dst).is_some() {
        return;
    }
    let dst_node = whole_node(partition, dst);
    partition.set(dst_node, BirthSiteId::new(*next_site));
    *next_site += 1;
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
    admit_fixpoint(partition, merges);
    if resolve_merge_families(partition, merges) {
        admit_fixpoint(partition, merges);
    }
}

/// The per-merge singleton-witness admission fixpoint.
fn admit_fixpoint(partition: &mut BirthSitePartition, merges: &[(ArcVarId, Vec<ArcVarId>)]) {
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
            } else {
                tracing::trace!(
                    target: "ori_arc::aims::intraprocedural",
                    merge_var = ?merge_var,
                    pred_sites = ?preds
                        .iter()
                        .map(|&node| partition.site(node))
                        .collect::<Vec<_>>(),
                    "phi admission refused: predecessors lack a singleton known birth site"
                );
            }
        }
    }
}

/// SCC flow witness (PV-1 P6, `scc_external_source_determines`): a family of
/// mutually-dependent unadmitted merges whose every EXTERNAL predecessor
/// resolves to ONE known birth site holds only that site's values — assign
/// the site to the family so the standard singleton witness admits each
/// edge on the next fixpoint round. Returns whether any site was assigned.
fn resolve_merge_families(
    partition: &mut BirthSitePartition,
    merges: &[(ArcVarId, Vec<ArcVarId>)],
) -> bool {
    // The unadmitted leftover: merges still cross-class with some pred.
    let mut leftover: Vec<usize> = Vec::new();
    for (index, (merge_var, args)) in merges.iter().enumerate() {
        let merge_node = whole_node(partition, *merge_var);
        let unadmitted = args.iter().any(|&arg| {
            let arg_node = whole_node(partition, arg);
            !partition.same_rep(arg_node, merge_node)
        });
        if unadmitted && partition.site(merge_node).is_none() {
            leftover.push(index);
        }
    }
    if leftover.is_empty() {
        return false;
    }
    // Group the leftover into families connected by pred-of edges (a pred
    // in another leftover merge's class links them).
    let mut assigned = false;
    let mut visited: Vec<bool> = vec![false; leftover.len()];
    for start in 0..leftover.len() {
        if visited[start] {
            continue;
        }
        let mut family: Vec<usize> = vec![start];
        visited[start] = true;
        let mut cursor = 0;
        while cursor < family.len() {
            let (merge_var, args) = &merges[leftover[family[cursor]]];
            let _ = merge_var;
            for &arg in args {
                let arg_node = whole_node(partition, arg);
                for (slot, &other) in leftover.iter().enumerate() {
                    if visited[slot] {
                        continue;
                    }
                    let other_node = whole_node(partition, merges[other].0);
                    if partition.same_rep(arg_node, other_node) {
                        visited[slot] = true;
                        family.push(slot);
                    }
                }
            }
            cursor += 1;
        }
        // External predecessors: args not in any family member's class.
        let member_nodes: Vec<NodeIdx> = family
            .iter()
            .map(|&slot| whole_node(partition, merges[leftover[slot]].0))
            .collect();
        let mut external_site: Option<
            crate::aims::intraprocedural::birth_site_partition::BirthSiteId,
        > = None;
        let mut sound = true;
        let mut has_external = false;
        for &slot in &family {
            let (_, args) = &merges[leftover[slot]];
            for &arg in args {
                let arg_node = whole_node(partition, arg);
                let internal = member_nodes
                    .iter()
                    .any(|&member| partition.same_rep(arg_node, member));
                if internal {
                    continue;
                }
                has_external = true;
                match (partition.site(arg_node), external_site) {
                    (Some(site), None) => external_site = Some(site),
                    (Some(site), Some(seen)) if site == seen => {}
                    _ => {
                        sound = false;
                        break;
                    }
                }
            }
            if !sound {
                break;
            }
        }
        let (true, true, Some(site)) = (sound, has_external, external_site) else {
            continue;
        };
        for &member in &member_nodes {
            partition.set(member, site);
        }
        assigned = true;
        tracing::trace!(
            target: "ori_arc::aims::intraprocedural",
            family_size = family.len(),
            ?site,
            "SCC flow witness: assigned the family's single external birth site"
        );
    }
    assigned
}

#[cfg(test)]
mod tests;
