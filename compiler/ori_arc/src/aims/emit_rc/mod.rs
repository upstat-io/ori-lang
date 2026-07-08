//! RC emission helpers for the unified realization pipeline.
//!
//! Contains helper functions, submodules, and re-exports used by `realize/`
//! for RC operations. RC emission is driven by `realize_rc_reuse()`.
//!
//! # Submodules
//!
//! - [`arg_ownership`] — Apply/Invoke argument ownership annotation
//! - [`borrowed_defs`] — borrowed-definition collection for RC emission
//! - [`coalesce`] — static RC coalescing peephole pass
//! - [`cow`] — COW annotation helpers
//! - [`dec_suppression`] — Apply-aliased RC-dec suppression predicates
//! - [`drop_hints`] — drop hint helpers
//! - [`edge_cleanup`] — inter-block edge RC cleanup
//! - [`queries`] — post-emission RC-incremented variable tracking
//! - [`take_project`] — take-project lineage + bypass-safe facts
//! - [`trampoline`] — merge-edge trampoline block insertion
//! - [`unwind_cleanup`] — Invoke-terminator unwind cleanup
//!
//! # References
//!
//! - Perceus (Reinking et al., PLDI 2021): dup/drop = contraction/weakening
//! - Backward liveness-driven RC insertion with last-use optimization
//!   (counting-immutable-beans technique)

pub mod arg_ownership;
pub(crate) mod borrowed_defs;
mod coalesce;
pub mod cow;
mod dec_suppression;
pub mod drop_hints;
mod edge_cleanup;
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
pub(crate) use borrowed_defs::{collect_all_borrowed_defs, collect_iter_element_defs};
pub(crate) use coalesce::coalesce_block_rc;
pub(crate) use dec_suppression::should_suppress_apply_aliased_dec;
pub(crate) use edge_cleanup::{
    compute_same_alloc_reps, emit_edge_cleanup, emit_invoke_unwind_pair_net_releases, same_alloc,
};

/// Compute `RcStrategy` for a variable, returning `None` for scalars.
///
/// Visible to sibling submodules via `super::rc_strategy`, and to
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
    Some(RcStrategy::from_repr(
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

/// True iff `var` is consumed at an OWNED `Invoke`/`InvokeIndirect` arg
/// position in `pred_block`'s terminator: ownership transfers to the callee
/// on the normal path, so the edge `RcDec` (RL-4) is the predicate-stack
/// release of that owned arg only when the callee unwinds before consuming it.
///
/// # Why
///
/// `emit_terminator_burden_*` already balanced such a var with its
/// terminator-block `BurdenInc`/`BurdenDec` pair at the transfer point, so a
/// second edge `BurdenDec` would net the per-value burden ledger to -1 (VF-1
/// imbalance). Inverse of `invoke_terminator_borrowed_args`: that suppresses
/// the burden dec for BORROWED args (released at the successor); this
/// suppresses the EDGE burden dec for OWNED args (already balanced).
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

/// Edge-cleanup release: suppresses the paired `BurdenDec` when `var` is an
/// owned-transfer arg of `pred_block`'s terminator (per
/// [`is_owned_transfer_arg_at_terminator`] — the burden ledger is already
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
/// `BurdenDec` to a real `RcDec`.
///
/// # Suppression
///
/// Emits ONLY for a var the Phase-5 burden walk DEFERRED (live-out of
/// `pred_block`, so no in-body `BurdenDec` exists). Suppresses when `var`
/// carries no burden, is an owned-transfer arg of `pred_block`'s terminator
/// (already balanced at the transfer point), the predecessor already has an
/// in-body whole-var `BurdenDec` (the walk's own dead-out release), OR the
/// successor block already carries a whole-var `BurdenDec` for `var` (the
/// Phase-5 RL-4/RL-5 dead-at-entry releases land in the successor's body —
/// a second dec on the same edge double-frees). Spec: Annex E §AIMS RL-4.
#[inline]
pub(crate) fn release_burden_only_edge(
    func: &ArcFunction,
    pred_block: usize,
    succ_block: usize,
    var: ArcVarId,
) -> Vec<ArcInstr> {
    if carries_burden(func, var)
        && !is_owned_transfer_arg_at_terminator(func, pred_block, var)
        && !has_whole_var_burden_dec_in_block(func, pred_block, var)
        && !has_whole_var_burden_dec_in_block(func, succ_block, var)
    {
        vec![ArcInstr::BurdenDec { var }]
    } else {
        Vec::new()
    }
}
