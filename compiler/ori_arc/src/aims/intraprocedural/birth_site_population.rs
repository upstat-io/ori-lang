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

use rustc_hash::{FxHashMap, FxHashSet};

use ori_registry::PrimitiveResultOwnership;

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
    compute_birth_site_partition_with_admitted(func, state_map, &FxHashSet::default())
}

/// [`compute_birth_site_partition`] with an ADMISSION override: vars in
/// `admitted` participate although the state map excludes them — the
/// scalar user-`@drop` obligation carriers (RL-DROP: no refcount, one
/// balance-neutral drop obligation booked like a reference).
pub(crate) fn compute_birth_site_partition_with_admitted(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    admitted: &FxHashSet<ArcVarId>,
) -> BirthSitePartition {
    let mut partition = BirthSitePartition::new();
    let mut next_site: u32 = 0;

    for block in &func.blocks {
        for instr in &block.body {
            admit_instruction(
                &mut partition,
                func,
                state_map,
                admitted,
                instr,
                &mut next_site,
            );
        }
        match &block.terminator {
            ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } => {
                mint_call_result_site(&mut partition, state_map, admitted, *dst, &mut next_site);
            }
            _ => {}
        }
    }

    let mut merges = collect_contract_alias_edges(&mut partition, state_map, admitted);
    collect_block_param_edges(&mut partition, func, state_map, admitted, &mut merges);
    admit_witnessed_merges(&mut partition, &merges);
    admit_field_congruence(&mut partition);

    partition
}

fn admit_instruction(
    partition: &mut BirthSitePartition,
    func: &ArcFunction,
    state_map: &AimsStateMap,
    admitted: &FxHashSet<ArcVarId>,
    instr: &ArcInstr,
    next_site: &mut u32,
) {
    match instr {
        ArcInstr::Construct { dst, args, .. } => {
            admit_construct(partition, state_map, admitted, *dst, args, next_site);
        }
        ArcInstr::Let { dst, value, .. } => {
            admit_let(partition, func, state_map, admitted, *dst, value, next_site);
        }
        ArcInstr::Apply { dst, .. } | ArcInstr::ApplyIndirect { dst, .. } => {
            mint_call_result_site(partition, state_map, admitted, *dst, next_site);
        }
        ArcInstr::Project {
            dst, value, field, ..
        } if !is_excluded(state_map, admitted, *dst) => {
            // Single-hop composition: multi-hop chains compose transitively
            // through the union-find, never hand-built multi-hop paths.
            let dst_node = whole_node(partition, *dst);
            let field_node = partition.register_node(*value, FieldPath::single(*field));
            partition.union_tier1(dst_node, field_node);
        }
        ArcInstr::Set { base, field, .. } if !is_excluded(state_map, admitted, *base) => {
            let base_node = whole_node(partition, *base);
            partition.mark_cow_boundary(base_node);
            let field_node = partition.register_node(*base, FieldPath::single(*field));
            partition.mark_cow_boundary(field_node);
        }
        ArcInstr::SetTag { base, .. } if !is_excluded(state_map, admitted, *base) => {
            let base_node = whole_node(partition, *base);
            partition.mark_cow_boundary(base_node);
        }
        _ => {}
    }
}

fn admit_construct(
    partition: &mut BirthSitePartition,
    state_map: &AimsStateMap,
    admitted: &FxHashSet<ArcVarId>,
    dst: ArcVarId,
    args: &[ArcVarId],
    next_site: &mut u32,
) {
    if is_excluded(state_map, admitted, dst) {
        return;
    }
    mint_fresh_site(partition, dst, next_site);
    // Constructor positions are exactly the field indices Project reads.
    for (position, &arg) in args.iter().enumerate() {
        if is_excluded(state_map, admitted, arg) {
            continue;
        }
        let Ok(field) = u32::try_from(position) else {
            unreachable!("constructor arg position exceeds u32::MAX");
        };
        let field_node = partition.register_node(dst, FieldPath::single(field));
        let arg_node = whole_node(partition, arg);
        partition.union_tier1(field_node, arg_node);
    }
}

fn admit_let(
    partition: &mut BirthSitePartition,
    func: &ArcFunction,
    state_map: &AimsStateMap,
    admitted: &FxHashSet<ArcVarId>,
    dst: ArcVarId,
    value: &ArcValue,
    next_site: &mut u32,
) {
    if is_excluded(state_map, admitted, dst) {
        return;
    }
    match value {
        ArcValue::Var(src) if !is_excluded(state_map, admitted, *src) => {
            let dst_node = whole_node(partition, dst);
            let src_node = whole_node(partition, *src);
            partition.union_tier1(dst_node, src_node);
        }
        ArcValue::PrimOp { args, .. } => {
            admit_primitive_result(partition, func, state_map, admitted, dst, args, next_site);
        }
        // A non-excluded string literal is a fresh allocation site; the empty
        // string has already been excluded as immortal.
        ArcValue::Literal(crate::ir::LitValue::String(_)) => {
            mint_fresh_site(partition, dst, next_site);
        }
        _ => {}
    }
}

fn admit_primitive_result(
    partition: &mut BirthSitePartition,
    func: &ArcFunction,
    state_map: &AimsStateMap,
    admitted: &FxHashSet<ArcVarId>,
    dst: ArcVarId,
    args: &[ArcVarId],
    next_site: &mut u32,
) {
    let fact = func
        .primitive_facts
        .get(dst)
        .unwrap_or_else(|| panic!("validated PrimOp v{} is missing its frozen fact", dst.raw()));
    match fact.descriptor.result {
        PrimitiveResultOwnership::Scalar => {}
        PrimitiveResultOwnership::IndependentOwned
        | PrimitiveResultOwnership::OwnedFromConsumedOrIndependent { .. } => {
            mint_fresh_site(partition, dst, next_site);
        }
        PrimitiveResultOwnership::Alias { operand } => {
            let source = args.get(usize::from(operand)).copied().unwrap_or_else(|| {
                panic!("validated primitive alias operand {operand} is out of bounds")
            });
            if !is_excluded(state_map, admitted, source) {
                let dst_node = whole_node(partition, dst);
                let source_node = whole_node(partition, source);
                partition.union_tier1(dst_node, source_node);
            }
        }
    }
}

fn mint_fresh_site(partition: &mut BirthSitePartition, dst: ArcVarId, next_site: &mut u32) {
    let dst_node = whole_node(partition, dst);
    partition.set(dst_node, BirthSiteId::new(*next_site));
    *next_site += 1;
}

fn is_excluded(state_map: &AimsStateMap, admitted: &FxHashSet<ArcVarId>, var: ArcVarId) -> bool {
    state_map.is_excluded(var) && !admitted.contains(&var)
}

/// Field-congruence closure: two whole-var nodes in ONE class name the SAME
/// allocation (PV-1 invariant of every admitted union), so their same-index
/// field nodes name the same field allocation — union them. A witness-finding
/// pass over the existing admission rule (the `scc_flow_witness_admits_phi`
/// precedent), NEVER a new admission kind: each congruence edge carries the
/// same-birth-site witness `samerep_birthsite_sound` requires. Composes the
/// single-hop `Project` registration across `Let`-Var alias hops (the field
/// node registered on the Construct dst and the field node registered on the
/// alias a `Project` reads through join one class). Fixpoint: a field-node
/// union can merge classes holding further whole-var nodes (nested
/// aggregates), exposing new congruence pairs; unions strictly reduce class
/// count, so the loop terminates within the node count.
fn admit_field_congruence(partition: &mut BirthSitePartition) {
    let cap = partition.len() + 2;
    for _round in 0..cap {
        // Group registered FIELD nodes by (whole-var class rep, field path).
        let snapshot = partition.nodes_snapshot();
        let mut groups: FxHashMap<(NodeIdx, FieldPath), Vec<NodeIdx>> = FxHashMap::default();
        for (var, path, node) in &snapshot {
            if path.is_whole_var() {
                continue;
            }
            let base = partition.register_node(*var, FieldPath::whole_var());
            let base_rep = partition.rep_of(base);
            groups
                .entry((base_rep, path.clone()))
                .or_default()
                .push(*node);
        }
        let mut changed = false;
        let mut keys: Vec<_> = groups.keys().cloned().collect();
        keys.sort_unstable_by_key(|(rep, path)| (*rep, path.clone()));
        for key in keys {
            let nodes = &groups[&key];
            let Some((&first, rest)) = nodes.split_first() else {
                continue;
            };
            for &other in rest {
                if partition.same_rep(first, other) {
                    continue;
                }
                // union_tier1 REFUSES on distinct known birth sites
                // (release-active fail-closed); a refusal leaves the
                // classes apart — congruence never forces an unsound join.
                if partition.union_tier1(first, other) {
                    changed = true;
                }
            }
        }
        if !changed {
            return;
        }
    }
    unreachable!("field-congruence fixpoint exceeded its derived cap");
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
    admitted: &FxHashSet<ArcVarId>,
    dst: ArcVarId,
    next_site: &mut u32,
) {
    let excluded = |var: ArcVarId| state_map.is_excluded(var) && !admitted.contains(&var);
    if excluded(dst) || state_map.apply_result_alias(dst).is_some() {
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
    admitted: &FxHashSet<ArcVarId>,
) -> Vec<(ArcVarId, Vec<ArcVarId>)> {
    let excluded = |var: ArcVarId| state_map.is_excluded(var) && !admitted.contains(&var);
    let mut aliases: Vec<(ArcVarId, &ApplyAliasSource)> = state_map
        .apply_result_aliases()
        .iter()
        .map(|(&dst, source)| (dst, source))
        .collect();
    aliases.sort_by_key(|&(dst, _)| dst.raw());

    let mut merges: Vec<(ArcVarId, Vec<ArcVarId>)> = Vec::new();
    for (dst, source) in aliases {
        if excluded(dst) {
            continue;
        }
        match source {
            ApplyAliasSource::Direct(arg) => {
                if excluded(*arg) {
                    continue;
                }
                let dst_node = whole_node(partition, dst);
                let arg_node = whole_node(partition, *arg);
                partition.union_tier1(dst_node, arg_node);
            }
            ApplyAliasSource::Project { arg, field } => {
                if excluded(*arg) {
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
    admitted: &FxHashSet<ArcVarId>,
    merges: &mut Vec<(ArcVarId, Vec<ArcVarId>)>,
) {
    let excluded = |var: ArcVarId| state_map.is_excluded(var) && !admitted.contains(&var);
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
        if excluded(param) {
            continue;
        }
        let Some(args) = incoming.remove(&param) else {
            continue;
        };
        if let [only_arg] = args.as_slice() {
            if excluded(*only_arg) {
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
    while resolve_merge_families(partition, merges) {
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
