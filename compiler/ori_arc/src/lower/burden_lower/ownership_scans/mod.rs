//! Ownership / transfer scans feeding `emit_burden_ops` — the dispatch hub.
//!
//! Per-scan-family submodules: `walk` (burden-walk collect / detect phases),
//! `borrowed` (borrowed-position scans), `dup_inc` (RL-1 duplication-inc
//! scans), `forwarder` (+ `union_find`, `forwarder_release` —
//! transfer-through-return forwarder scans), `dead_param` + `construct_fed`
//! (RL-5 dead-block-param releases), and the lineage scans
//! (`borrowed_invoke_lineage`, `live_extract`, `live_out`, `move_alias`).
//! Shared instr-level helpers live here.

mod borrowed;
mod borrowed_invoke_lineage;
mod branch_release;
mod call_arg_dup;
mod catch_recover_release;
mod closure_extract_borrow_view;
mod collection_literal_dead_source;
mod construct_fed;
mod cow_terminal_concat;
mod dead_param;
mod dup_inc;
mod forwarder;
mod forwarder_release;
mod fresh_call_result_borrowed_arg;
mod iter_consume_dead_thread;
mod lazy_iter_closure_borrow;
mod live_extract;
mod live_extract_site;
mod live_out;
mod loop_closure_dead_param;
mod loop_invariant_dead_local;
mod move_alias;
mod multi_exit_borrow_view;
mod nested_construct_return_passthrough;
mod owner_borrow_view;
mod reassign_release;
mod sharing_view_surplus;
mod store_dup;
mod union_find;
mod walk;

#[cfg(test)]
mod tests;

pub(super) use borrowed::{
    compute_borrowed_arg_let_aliases, compute_borrowed_projection_dsts,
    compute_borrowed_store_dup_args, compute_borrowed_terminator_invoke_args,
    compute_sole_carrier_borrowed_invoke_aliases, compute_yield_identity_push_dup_args,
};
pub(super) use borrowed_invoke_lineage::{
    borrowed_invoke_lineage_release_disabled, compute_borrowed_invoke_collection_lineage,
};
pub(super) use branch_release::compute_branch_exclusive_edge_releases;
pub(super) use call_arg_dup::compute_call_result_element_final_read_releases;
pub(super) use catch_recover_release::compute_catch_recover_release_lineage;
pub(super) use closure_extract_borrow_view::compute_closure_extract_borrow_view_lineage;
pub(super) use collection_literal_dead_source::compute_collection_literal_dead_source_suppression;
pub(super) use cow_terminal_concat::compute_cow_terminal_concat_inc_dsts;
pub(super) use fresh_call_result_borrowed_arg::compute_fresh_call_result_borrowed_arg_inc_dsts;
pub(super) use sharing_view_surplus::compute_sharing_view_surplus_inc_dsts;
// Test-only re-export: the RAW classification is consumed by the `tests`
// sibling; production consumers take the FUNDED set.
#[cfg(test)]
pub(super) use call_arg_dup::compute_genuine_dup_call_arg_aliases;
pub(crate) use call_arg_dup::{
    compute_funded_call_arg_dup_aliases, contract_consuming_arg_position,
};
pub(super) use construct_fed::{
    compute_construct_fed_dead_param_lineage, ConstructFedDeadParamLineage,
};
pub(super) use dead_param::{
    compute_dead_forwarder_block_param_releases, compute_rebuild_lineage_dead_param_releases,
};
pub(crate) use dup_inc::collect_move_edges_and_store_consumes;
pub(super) use dup_inc::{
    compute_genuine_dup_move_aliases, compute_sum_payload_iter_consume_dup_inc_suppression,
    compute_ttr_iter_consume_dup_aliases, compute_use_counts_and_dup_aliases,
};
pub(super) use forwarder::{
    compute_forwarder_identity_transparent_aliases, compute_transfer_through_return_param_vars,
    compute_transfer_through_return_results,
};
pub(super) use forwarder_release::compute_forwarder_result_under_release;
pub(crate) use forwarder_release::ForwarderReleasePos;
pub(super) use iter_consume_dead_thread::compute_iter_consume_dead_thread_orphan_inc;
pub(super) use lazy_iter_closure_borrow::compute_lazy_iter_closure_borrow_lineage;
pub(super) use live_extract::{
    compute_fresh_sum_live_extract_lineage, compute_retain_aliasing_lineage,
};
pub(super) use live_out::{compute_dead_owned_param_branch_releases, compute_live_out_owned};
pub(super) use loop_closure_dead_param::compute_loop_closure_dead_param_lineage;
pub(super) use loop_invariant_dead_local::compute_loop_invariant_dead_local_releases;
pub(super) use move_alias::{
    compute_multi_borrow_view_alias_surplus, compute_readonly_borrow_orphan_inc_suppression,
    compute_transfer_via_move_alias, SameAllocIdentity,
};
pub(super) use multi_exit_borrow_view::compute_multi_exit_borrow_view_lineage;
pub(super) use nested_construct_return_passthrough::compute_nested_construct_return_passthrough;
pub(super) use owner_borrow_view::extend_owner_last_use_for_borrow_views;
pub(super) use reassign_release::compute_reassign_rebind_releases;
pub(crate) use store_dup::{compute_funded_store_dup_aliases, store_family_funding_disabled};
pub(super) use walk::{
    collect_owned_burdens, compute_owned_vars_needing_rc, detect_last_uses, detect_transfer_points,
    group_last_uses_filtered,
};

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, PrimOp, ValueRepr};

/// Snapshot vars consumed at an owned position by `instr`, used to suppress
/// `BurdenDec` at transfer points per AIMS RL-2. `Set.value` is added
/// explicitly per AIMS TF-15 (`is_owned_position`'s `_ => false` catch-all
/// excludes it). A list-concat `PrimOp Binary(Add)` `RcPointer` operand is added
/// per the dual-consuming `ori_list_concat_cow` runtime contract (the helper
/// dec/frees BOTH input buffers;
/// `is_owned_position`'s `_ => false` excludes `Let { PrimOp }`). Shared by the
/// driver, `moved_fields`, and `emit` submodules.
pub(super) fn instr_transfer_vars(instr: &ArcInstr, func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut transfer_vars = instr_owned_position_transfer_vars(instr);
    transfer_vars.extend(list_concat_consumed_operands(instr, func));
    transfer_vars
}

/// The `func`-free subset of `instr_transfer_vars`: vars consumed at an
/// `is_owned_position` arg plus the `Set.value` TF-15 carve-out. Used at the
/// `&mut`-walk emission site where `func` cannot be re-borrowed for `var_repr`;
/// the list-concat consume set is consulted there via the precomputed
/// `BurdenAnalysisCtx::list_concat_transfer_vars` instead.
pub(super) fn instr_owned_position_transfer_vars(instr: &ArcInstr) -> FxHashSet<ArcVarId> {
    let mut transfer_vars: FxHashSet<ArcVarId> = instr
        .used_vars()
        .iter()
        .enumerate()
        .filter_map(|(pos, &arg)| instr.is_owned_position(pos).then_some(arg))
        .collect();
    if let ArcInstr::Set { value, .. } = instr {
        transfer_vars.insert(*value);
    }
    transfer_vars
}

/// Function-wide used-var set: a var appearing in ANY instr / terminator operand
/// position. A block-param NOT in this set is dead (`Cardinality = Absent`); its only
/// appearance is its own param-binding slot.
pub(in crate::lower::burden_lower) fn function_used_vars(
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let mut used: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            used.extend(instr.used_vars());
        }
        used.extend(block.terminator.used_vars());
    }
    used
}

/// Operands consumed by the dual-consuming `ori_list_concat_cow` runtime helper:
/// the `RcPointer` operands of a `Let { PrimOp Binary(Add) }`. SSOT for the
/// list-concat consume set — consulted by `instr_transfer_vars` (last-use dec
/// suppression) AND precomputed function-wide in `emit_burden_ops_for_blocks`
/// (the `&mut` walk cannot re-borrow `func` for `var_repr`).
///
/// `RcPointer` is the list discriminator: `str`/closure operands are `FatValue`
/// and BORROWED by `ori_str_concat`'s `*const OriStr` contract (not consumed);
/// scalar `Add` operands carry no RC (filtered by `owned_vars_needing_rc`).
/// User `Add` impls dispatch via trait `Apply`/`Invoke`, never `PrimOp Binary`.
/// List + list → COW concat: both operands are consumed by the runtime.
pub(crate) fn list_concat_consumed_operands(
    instr: &ArcInstr,
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let mut consumed = FxHashSet::default();
    if let ArcInstr::Let {
        value: ArcValue::PrimOp { op, args },
        ..
    } = instr
    {
        if matches!(op, PrimOp::Binary(ori_ir::BinaryOp::Add)) {
            for &arg in args {
                if matches!(func.var_repr(arg), Some(ValueRepr::RcPointer)) {
                    consumed.insert(arg);
                }
            }
        }
    }
    consumed
}

/// Per-var defining block: instruction dsts + block params. The
/// forward-reachability discriminator of the duplication scans CUTS at the
/// source's defining block — re-reaching the definition via a back-edge
/// re-DEFINES the binding, so the re-reached "use" belongs to the NEW binding,
/// not the consumed allocation. Shared by the call-arg and store duplication
/// scans (`call_arg_dup`, `store_dup`).
fn collect_def_blocks(func: &ArcFunction) -> rustc_hash::FxHashMap<ArcVarId, usize> {
    let mut def_block: rustc_hash::FxHashMap<ArcVarId, usize> = rustc_hash::FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for &(param, _) in &block.params {
            def_block.insert(param, block_idx);
        }
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                def_block.insert(dst, block_idx);
            }
        }
    }
    def_block
}

/// Forward-reachable block set from `start`'s SUCCESSORS, NOT expanding
/// through `cut` (the source's defining block): blocks beyond a re-definition
/// belong to the NEXT binding instance. `cut` itself may appear in the set
/// (callers exclude its use sites separately). Shared by the call-arg and
/// store duplication scans.
fn successor_reachable_blocks_with_cut(
    func: &ArcFunction,
    start: usize,
    cut: Option<usize>,
) -> FxHashSet<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = func
        .blocks
        .get(start)
        .map(|b| {
            crate::graph::successor_block_ids(&b.terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .collect()
        })
        .unwrap_or_default();
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        if cut == Some(b) {
            continue;
        }
        if let Some(block) = func.blocks.get(b) {
            for s in crate::graph::successor_block_ids(&block.terminator) {
                stack.push(s.index());
            }
        }
    }
    visited
}

/// Forward-reachable block set from `start`'s SUCCESSORS (`start` itself is
/// included only when a cycle re-reaches it).
fn successor_reachable_blocks(func: &ArcFunction, start: usize) -> FxHashSet<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = func
        .blocks
        .get(start)
        .map(|b| {
            crate::graph::successor_block_ids(&b.terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .collect()
        })
        .unwrap_or_default();
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        if let Some(block) = func.blocks.get(b) {
            for s in crate::graph::successor_block_ids(&block.terminator) {
                stack.push(s.index());
            }
        }
    }
    visited
}
