//! RC emission helpers for the unified realization pipeline.
//!
//! Contains helper functions, submodules, and re-exports used by `realize/`
//! for RC operations. The legacy `emit_rc_ops()` entry point
//! has been removed — RC emission is now driven by `realize_rc_reuse()`.
//!
//! # Submodules
//!
//! - [`arg_ownership`] — Apply/Invoke ownership propagation
//! - [`cow`] — COW annotation helpers
//! - [`drop_hints`] — drop hint helpers
//! - [`coalesce`] — adjacent RC op merging
//! - [`dead_cleanup`] — dead-at-entry/invoke-dst cleanup
//! - [`edge_cleanup`] — inter-block edge RC decrements
//! - [`forward_walk`] — terminator RC emission
//! - [`helpers`] — block context, use precomputation, liveness queries
//! - [`queries`] — RC state queries (incremented vars)
//!
//! # References
//!
//! - Perceus (Reinking et al., PLDI 2021): dup/drop = contraction/weakening
//! - Lean 4 `RC.lean`: backward liveness-driven insertion with last-use opt

pub mod arg_ownership;
pub(crate) mod borrowed_defs;
mod coalesce;
pub mod cow;
pub(crate) mod dead_cleanup;
pub mod drop_hints;
mod edge_cleanup;
mod forward_walk;
mod helpers;
pub(crate) mod queries;
pub(crate) mod take_project;
mod trampoline;
pub(crate) mod unwind_cleanup;

use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId, RcAtomicity, RcStrategy};

/// Edge-specific RC decrement: variable + strategy.
pub(crate) type EdgeDec = (ArcVarId, RcStrategy);

/// Deferred dec routed to edge cleanup.
/// - `None` target: emit on ALL outgoing edges (Phase B deferred parents)
/// - `Some(succ)` target: emit only on edge to `succ` (merge-edge decs)
pub(crate) type DeferredDec = (Option<usize>, ArcVarId, RcStrategy);

// Re-export for cow/drop_hints that import via `super::collect_rc_incremented_vars`.
pub(crate) use queries::{collect_param_borrowed_vars, collect_rc_incremented_vars};

// Re-exports for `realize/` unified annotation walk.
pub(crate) use cow::{has_borrows_from_aggregate, is_borrow_disjoint_from_siblings};
pub(crate) use drop_hints::{collect_borrowed_call_args, is_collection_var};

// Re-exports for `realize/` unified forward walk.
pub(crate) use borrowed_defs::{
    collect_all_borrowed_defs, collect_borrowed_defs, collect_cow_borrowed_receivers,
    collect_inline_enum_projected_defs, collect_iter_element_defs, collect_project_borrowed_defs,
    is_take_project,
};
pub(crate) use coalesce::coalesce_block_rc;
pub(crate) use dead_cleanup::{emit_dead_at_entry_decs, emit_dead_invoke_dsts};
pub(crate) use edge_cleanup::{compute_same_alloc_reps, emit_edge_cleanup, same_alloc};
pub(crate) use forward_walk::emit_terminator_rc;
pub(crate) use helpers::{
    collect_defined_vars, compute_child_effective_last_use, compute_function_project_sources,
    is_consuming_primop, is_live_at_exit, is_owned_at_entry, is_ownership_transfer,
    precompute_block_uses, should_suppress_apply_aliased_dec, should_suppress_return_transfer_dec,
    BlockCtx, LastUse,
};

/// Compute `RcStrategy` for a variable, returning `None` for scalars.
///
/// Visible to all sibling submodules (`edge_cleanup`, `dead_cleanup`,
/// `forward_walk`, `helpers`) via `super::rc_strategy`, and to
/// `realize/` via `pub(crate)` re-export.
#[inline]
pub(crate) fn rc_strategy(
    func: &ArcFunction,
    var: ArcVarId,
    pool: &ori_types::Pool,
) -> Option<RcStrategy> {
    use crate::ir::ValueRepr;
    let repr = func.var_reprs[var.index()];
    if repr == ValueRepr::Scalar {
        return None;
    }
    Some(RcStrategy::from_var(
        repr,
        pool,
        func.var_types[var.index()],
    ))
}

/// Convert a `usize` block index to `ArcBlockId`.
#[inline]
pub(crate) fn block_id(idx: usize) -> ArcBlockId {
    ArcBlockId::new(
        u32::try_from(idx).unwrap_or_else(|_| panic!("block index {idx} exceeds u32::MAX")),
    )
}

/// Whether `var` carries burden ops (Step-4b walk set a `BurdenInc`/`BurdenDec`
/// for it). SSOT for the site-pairing gate used by every faithful release site.
#[inline]
pub(crate) fn carries_burden(func: &ArcFunction, var: ArcVarId) -> bool {
    func.burden_emitted
        .get(var.index())
        .copied()
        .unwrap_or(false)
}

/// Faithful release: a `BurdenDec` paired adjacent to a release `RcDec`, gated
/// on `carries_burden`. The `BurdenDec` mirrors the predicate-stack release per
/// AIMS RL-2 / RL-4 / RL-5 so the per-value burden ledger nets 0 along the path
/// the `RcDec` covers (`Spec: Annex E §AIMS` realization rules). When `var`
/// carries no burden ops the returned vec holds only the `RcDec`.
///
/// SSOT for the burden-pair-then-RcDec pattern shared by edge cleanup
/// (single-pred prepend + multi-pred trampoline), dead-at-entry cleanup, and
/// the project-escape succ-dec path.
#[inline]
pub(crate) fn release_with_burden(
    func: &ArcFunction,
    var: ArcVarId,
    strategy: RcStrategy,
) -> Vec<ArcInstr> {
    let mut ops = Vec::with_capacity(2);
    if carries_burden(func, var) {
        ops.push(ArcInstr::BurdenDec { var });
    }
    ops.push(ArcInstr::RcDec {
        var,
        strategy,
        atomicity: RcAtomicity::default_atomic(),
    });
    ops
}

/// True iff `var` is consumed at an OWNED `Invoke`/`InvokeIndirect` arg
/// position in `pred_block`'s terminator. The value's ownership transfers to
/// the callee on the normal path; the unwind/normal edge `RcDec` (RL-4) is the
/// predicate-stack release of that owned arg when the callee unwinds before
/// consuming it.
///
/// The burden walk already balanced such a var with its terminator-block
/// `BurdenInc`/`BurdenDec` pair at the transfer point (`emit_terminator_burden_*`),
/// so pairing a second `BurdenDec` onto the edge cleanup would net the per-value
/// burden ledger to -1 (VF-1 imbalance). This is the edge-cleanup inverse of the
/// terminator-position `invoke_terminator_borrowed_args` suppression: that one
/// suppresses the burden dec for BORROWED args (released at the successor); this
/// one suppresses the EDGE burden dec for OWNED args (already balanced at the
/// transfer point).
#[inline]
fn is_owned_transfer_arg_at_terminator(
    func: &ArcFunction,
    pred_block: usize,
    var: ArcVarId,
) -> bool {
    let Some(block) = func.blocks.get(pred_block) else {
        return false;
    };
    block
        .terminator
        .used_vars()
        .iter()
        .enumerate()
        .any(|(pos, &arg)| arg == var && block.terminator.is_owned_position(pos))
}

/// Edge-cleanup release: like [`release_with_burden`] but suppresses the paired
/// `BurdenDec` when `var` is an owned-transfer arg of `pred_block`'s terminator
/// (per [`is_owned_transfer_arg_at_terminator`] — the burden ledger is already
/// balanced at the transfer point). The `RcDec` is always emitted (RL-4 holds
/// regardless of the burden-ledger accounting).
#[inline]
pub(crate) fn release_with_burden_edge(
    func: &ArcFunction,
    pred_block: usize,
    var: ArcVarId,
    strategy: RcStrategy,
) -> Vec<ArcInstr> {
    let mut ops = Vec::with_capacity(2);
    if carries_burden(func, var) && !is_owned_transfer_arg_at_terminator(func, pred_block, var) {
        ops.push(ArcInstr::BurdenDec { var });
    }
    ops.push(ArcInstr::RcDec {
        var,
        strategy,
        atomicity: RcAtomicity::default_atomic(),
    });
    ops
}

/// True iff `pred_block`'s body already contains a whole-var `BurdenDec` for
/// `var`. The Phase-5 burden walk emits an in-body `BurdenDec` exactly for a var
/// that is dead-out of its block (a genuine last-use release per RL-4); it
/// DEFERS the dec for live-out vars to edge cleanup. A var already carrying an
/// in-body whole-var `BurdenDec` was therefore released by the walk and must NOT
/// receive a second edge `BurdenDec` (else a double-free on the lowered path).
#[inline]
fn has_whole_var_burden_dec_in_block(func: &ArcFunction, pred_block: usize, var: ArcVarId) -> bool {
    func.blocks.get(pred_block).is_some_and(|block| {
        block
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::BurdenDec { var: v } if *v == var))
    })
}

/// Burden-only edge-cleanup release: emits the dying-edge `BurdenDec` for `var`
/// WITHOUT the predicate-stack `RcDec`. Used by the probe path
/// (`ORI_DISABLE_PREDICATE_STACK_RC=1`) where the burden path is the sole RC
/// emitter — Phase 7 (`lower_burden_ops_to_rc`) lowers this whole-var
/// `BurdenDec` to a real `RcDec`. Emits ONLY for a var the Phase-5 burden walk
/// DEFERRED (live-out of `pred_block`, so no in-body `BurdenDec` exists):
/// suppresses when `var` carries no burden, is an owned-transfer arg of
/// `pred_block`'s terminator (already balanced at the transfer point), OR
/// already has an in-body whole-var `BurdenDec` (the walk's own dead-out
/// release — a second edge dec would double-free). Spec: Annex E §AIMS RL-4.
#[inline]
pub(crate) fn release_burden_only_edge(
    func: &ArcFunction,
    pred_block: usize,
    var: ArcVarId,
) -> Vec<ArcInstr> {
    if carries_burden(func, var)
        && !is_owned_transfer_arg_at_terminator(func, pred_block, var)
        && !has_whole_var_burden_dec_in_block(func, pred_block, var)
    {
        vec![ArcInstr::BurdenDec { var }]
    } else {
        Vec::new()
    }
}

/// True iff `succ_block` is a normal/unwind successor of some predecessor whose
/// `Invoke`/`InvokeIndirect` terminator consumes `var` at an OWNED arg position.
///
/// Dead-at-entry / block-entry releases (`dead_cleanup`) of an owned-transfer
/// Invoke arg are the same over-count case as the edge cleanup keyed on the
/// predecessor: the value's burden ledger was balanced at the transfer point
/// (`emit_terminator_burden_*`), so a block-entry `BurdenDec` on the Invoke's
/// successor double-counts. The `RcDec` itself stays (RL-5 dead-at-entry holds).
/// Keyed on the successor block since `dead_cleanup` emits at block entry, this
/// is the successor-side companion of [`is_owned_transfer_arg_at_terminator`].
fn is_owned_transfer_arg_into_block(func: &ArcFunction, succ_block: usize, var: ArcVarId) -> bool {
    use crate::ir::ArcTerminator;
    let succ_id = block_id(succ_block);
    func.blocks.iter().any(|pred| {
        let (ArcTerminator::Invoke { normal, unwind, .. }
        | ArcTerminator::InvokeIndirect { normal, unwind, .. }) = &pred.terminator
        else {
            return false;
        };
        if *normal != succ_id && *unwind != succ_id {
            return false;
        }
        pred.terminator
            .used_vars()
            .iter()
            .enumerate()
            .any(|(pos, &arg)| arg == var && pred.terminator.is_owned_position(pos))
    })
}

/// Block-entry release: like [`release_with_burden`] but suppresses the paired
/// `BurdenDec` when `var` is an owned-transfer Invoke/InvokeIndirect arg whose
/// successor is `succ_block` (per [`is_owned_transfer_arg_into_block`] — the
/// burden ledger is already balanced at the transfer point). The `RcDec` is
/// always emitted (RL-5 dead-at-entry holds regardless of the burden ledger).
#[inline]
pub(crate) fn release_with_burden_into_block(
    func: &ArcFunction,
    succ_block: usize,
    var: ArcVarId,
    strategy: RcStrategy,
) -> Vec<ArcInstr> {
    let mut ops = Vec::with_capacity(2);
    if carries_burden(func, var) && !is_owned_transfer_arg_into_block(func, succ_block, var) {
        ops.push(ArcInstr::BurdenDec { var });
    }
    ops.push(ArcInstr::RcDec {
        var,
        strategy,
        atomicity: RcAtomicity::default_atomic(),
    });
    ops
}
