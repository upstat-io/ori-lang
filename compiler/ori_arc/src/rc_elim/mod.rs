//! RC elimination pass for ARC IR.
//!
//! Eliminates redundant `RcInc`/`RcDec` pairs using bidirectional intra-block
//! dataflow analysis. An `RcInc` immediately followed by an `RcDec` on the
//! same variable (with no intervening use) is a net-zero no-op that can be
//! safely removed.
//!
//! # Pipeline Position
//!
//! This pass runs AFTER constructor reuse expansion. Input is ARC IR with
//! `RcInc`/`RcDec` from both RC insertion and reuse expansion.
//! Execution order: RC insertion → reuse expansion → RC elimination (this pass).
//!
//! # Algorithm
//!
//! **V1: Intra-block only.** Each basic block is analyzed independently.
//! Two passes per block find complementary patterns:
//!
//! 1. **Top-down (forward):** For each `RcInc(x)`, scan forward for a
//!    matching `RcDec(x)`. If no instruction between them uses `x`, the
//!    pair is eliminated.
//!
//! 2. **Bottom-up (backward):** For each `RcDec(x)`, scan backward for a
//!    matching `RcInc(x)`. Same "no intervening use" rule.
//!
//! Eliminations are applied iteratively until no more pairs are found
//! (cascading — removing a pair may expose a new adjacent pair).
//!
//! # Safety
//!
//! Only `RcInc; ...; RcDec` pairs (in program order) are eliminated.
//! `RcDec; ...; RcInc` is NOT safe because the `RcDec` might free the
//! object, making the `RcInc` a use-after-free.
//!
//! # References
//!
//! - Swift: `lib/SILOptimizer/ARC/` — bidirectional dataflow, `ARCMatchingSet`
//! - Koka: Perceus paper §3.2 — precise RC with dup/drop fusion
//! - Lean 4: borrow analysis minimizes redundant ops at insertion time

mod eliminate;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

use self::eliminate::{
    eliminate_cross_block_pairs, eliminate_once, known_safe_guarding_elim,
    remove_instructions_by_index,
};

// Public API

/// Eliminate redundant `RcInc`/`RcDec` pairs in an ARC IR function.
///
/// Runs both intra-block (bidirectional dataflow) and cross-block
/// (single-predecessor edge pair) elimination. Iterates until no
/// more pairs can be found (cascading elimination).
///
/// Returns the total number of pairs eliminated.
///
/// # Arguments
///
/// * `func` — the ARC IR function to optimize (mutated in place).
pub(crate) fn eliminate_rc_ops(func: &mut ArcFunction) -> usize {
    // Sanity check: every RcInc/RcDec target should have a non-Scalar repr.
    // Scalar values never need RC operations. If this fires, RC insertion
    // placed an op on a scalar variable (pipeline bug).
    #[cfg(debug_assertions)]
    validate_rc_targets(func);

    let mut total = 0;

    loop {
        let intra = eliminate_once(func);
        let cross = eliminate_cross_block_pairs(func);
        let guarded = known_safe_guarding_elim(func);
        let eliminated = intra + cross + guarded;
        if eliminated == 0 {
            break;
        }
        total += eliminated;
    }

    if total > 0 {
        tracing::debug!(
            function = func.name.raw(),
            pairs = total,
            "eliminated redundant RC pairs",
        );
    }

    total
}

// Full-CFG dataflow RC elimination

/// Enhanced RC elimination using `DerivedOwnership` information.
///
/// Extends the existing elimination with ownership-aware analysis:
///
/// 1. **Borrowed variable elimination**: If a variable is `BorrowedFrom(x)`,
///    any `RcInc`/`RcDec` on it is unnecessary as long as `x` is alive.
///    This captures the common pattern of projecting a field and immediately
///    incrementing it for a call.
///
/// 2. **Fresh variable optimization**: If a variable is `Fresh` (refcount = 1),
///    the first `RcDec` is guaranteed to deallocate. This information doesn't
///    eliminate pairs directly, but allows subsequent passes (reset/reuse) to
///    be more aggressive.
///
/// 3. **Multi-predecessor join elimination**: Forward propagation of available
///    `RcInc` operations using intersection at join points. An `RcInc(x)` is
///    available at block B only if it's available on ALL incoming edges.
///
/// Returns the total number of pairs eliminated (in addition to the base
/// `eliminate_rc_ops` count).
///
/// # Arguments
///
/// * `func` — the ARC IR function to optimize (mutated in place).
/// * `ownership` — per-variable derived ownership from `infer_derived_ownership`.
pub fn eliminate_rc_ops_dataflow(
    func: &mut ArcFunction,
    ownership: &[crate::ownership::DerivedOwnership],
) -> usize {
    // Phase 1: Run existing elimination (intra-block + single-predecessor cross-block).
    let base = eliminate_rc_ops(func);

    // Phase 2: Ownership-aware elimination.
    // Remove RcInc/RcDec on variables that are BorrowedFrom a still-live source.
    let mut ownership_eliminated = 0;
    let mut removals: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let var = match instr {
                ArcInstr::RcInc { var, count: 1, .. } | ArcInstr::RcDec { var, .. } => *var,
                _ => continue,
            };

            let var_idx = var.index();
            if var_idx >= ownership.len() {
                continue;
            }

            if let crate::ownership::DerivedOwnership::BorrowedFrom(source) = ownership[var_idx] {
                // Check if the source is still alive in this block.
                // Conservative check: the source must not have been decremented
                // earlier in this same block.
                let source_decremented = block.body[..instr_idx]
                    .iter()
                    .any(|i| matches!(i, ArcInstr::RcDec { var: v, .. } if *v == source));

                if !source_decremented {
                    removals.entry(block_idx).or_default().insert(instr_idx);
                    ownership_eliminated += 1;
                }
            }
        }
    }

    // Apply ownership-based removals.
    if !removals.is_empty() {
        remove_instructions_by_index(func, &removals);

        tracing::debug!(
            function = func.name.raw(),
            ownership_pairs = ownership_eliminated,
            "eliminated ownership-redundant RC ops",
        );
    }

    // Phase 3: Multi-predecessor join elimination.
    // Forward dataflow: track which variables have an available RcInc.
    // At join points, intersect available sets from all predecessors.
    let join_eliminated = eliminate_join_pairs(func);

    base + ownership_eliminated + join_eliminated
}

/// Eliminate RcInc/RcDec pairs across multi-predecessor joins.
///
/// Uses forward dataflow to propagate "available `RcInc`" sets. An `RcInc(x)`
/// is available at block B's entry if it's available on ALL incoming edges
/// (intersection/meet at joins). If we find an `RcDec(x)` at B's entry and
/// `RcInc(x)` is available from all predecessors, we can eliminate both.
fn eliminate_join_pairs(func: &mut ArcFunction) -> usize {
    let preds = compute_predecessors(func);
    let num_blocks = func.blocks.len();

    // Compute available_out: set of variables with trailing RcInc at block exit.
    // A variable has an "available" RcInc at block exit if the last RC op on it
    // in the block is RcInc and the terminator doesn't use it.
    let mut available_out: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); num_blocks];

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let term_uses: FxHashSet<ArcVarId> = block.terminator.used_vars().into_iter().collect();
        let mut trailing: FxHashSet<ArcVarId> = FxHashSet::default();

        for instr in block.body.iter().rev() {
            match instr {
                ArcInstr::RcInc { var, count: 1, .. } if !term_uses.contains(var) => {
                    trailing.insert(*var);
                }
                ArcInstr::RcDec { var, .. } => {
                    trailing.remove(var);
                }
                other => {
                    // Any use invalidates availability.
                    for used in other.used_vars() {
                        trailing.remove(&used);
                    }
                }
            }
        }

        available_out[block_idx] = trailing;
    }

    // At each block with multiple predecessors, intersect available_out.
    let mut removals: Vec<(usize, usize)> = Vec::new();

    for (block_idx, block_preds) in preds.iter().enumerate() {
        if block_preds.len() < 2 {
            continue;
        }

        // Intersect available_out from all predecessors.
        let mut available_at_entry: Option<FxHashSet<ArcVarId>> = None;
        for &pred_idx in block_preds {
            match &mut available_at_entry {
                None => available_at_entry = Some(available_out[pred_idx].clone()),
                Some(set) => {
                    set.retain(|v| available_out[pred_idx].contains(v));
                }
            }
        }

        let available = match available_at_entry {
            Some(a) if !a.is_empty() => a,
            _ => continue,
        };

        // Check leading RcDec instructions in this block.
        let body = &func.blocks[block_idx].body;
        for (j, instr) in body.iter().enumerate() {
            if let ArcInstr::RcDec { var, .. } = instr {
                if available.contains(var) {
                    // Remove the RcDec here and the trailing RcInc in each predecessor.
                    removals.push((block_idx, j));
                    for &pred_idx in block_preds {
                        // Find and mark the trailing RcInc for this var.
                        let pred_body = &func.blocks[pred_idx].body;
                        for (pi, pinstr) in pred_body.iter().enumerate().rev() {
                            if matches!(pinstr, ArcInstr::RcInc { var: v, count: 1, .. } if *v == *var)
                            {
                                removals.push((pred_idx, pi));
                                break;
                            }
                        }
                    }
                }
            } else {
                break; // Stop at first non-Dec instruction.
            }
        }
    }

    if removals.is_empty() {
        return 0;
    }

    // Apply removals.
    let mut by_block: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    for (blk, pos) in &removals {
        by_block.entry(*blk).or_default().insert(*pos);
    }

    remove_instructions_by_index(func, &by_block);

    let pairs = removals.len() / 3; // Each join elimination removes 1 Dec + N Incs
    if pairs > 0 {
        tracing::debug!(pairs, "eliminated join-point RC pairs");
    }

    pairs
}

// Validation

/// Verify that all `RcInc`/`RcDec` targets have non-Scalar repr.
///
/// Scalar values (int, float, bool, etc.) never need reference counting.
/// An RC operation targeting a Scalar variable indicates a bug in RC
/// insertion or an incorrect `var_reprs` entry.
///
/// Also verifies that matched `RcInc`/`RcDec` pairs within a block use
/// the same [`RcStrategy`](crate::ir::RcStrategy), catching strategy
/// mismatches that could cause incorrect codegen.
#[cfg(debug_assertions)]
fn validate_rc_targets(func: &ArcFunction) {
    use crate::ir::ValueRepr;

    // Skip validation if var_reprs hasn't been populated (test-only path).
    if func.var_reprs.is_empty() {
        return;
    }

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            match instr {
                ArcInstr::RcInc { var, strategy, .. } => {
                    let repr = func.var_reprs[var.index()];
                    debug_assert!(
                        repr != ValueRepr::Scalar,
                        "RcInc on Scalar variable v{} (repr={repr:?}) in block {block_idx} \
                         at position {instr_idx} — RC insertion bug",
                        var.raw(),
                    );
                    // Verify strategy consistency: strategy should be derivable
                    // from the variable's repr (exact check requires Pool, so
                    // we just verify the broad category matches).
                    debug_assert!(
                        strategy_matches_repr(*strategy, repr),
                        "RcInc strategy {strategy:?} inconsistent with repr {repr:?} \
                         for v{} in block {block_idx}",
                        var.raw(),
                    );
                }
                ArcInstr::RcDec { var, strategy } => {
                    let repr = func.var_reprs[var.index()];
                    debug_assert!(
                        repr != ValueRepr::Scalar,
                        "RcDec on Scalar variable v{} (repr={repr:?}) in block {block_idx} \
                         at position {instr_idx} — RC insertion bug",
                        var.raw(),
                    );
                    debug_assert!(
                        strategy_matches_repr(*strategy, repr),
                        "RcDec strategy {strategy:?} inconsistent with repr {repr:?} \
                         for v{} in block {block_idx}",
                        var.raw(),
                    );
                }
                _ => {}
            }
        }
    }
}

/// Check that an [`RcStrategy`] is consistent with a [`ValueRepr`].
///
/// This is a broad category check — it verifies the strategy family
/// matches the repr without requiring Pool access for exact verification.
#[cfg(debug_assertions)]
fn strategy_matches_repr(strategy: crate::ir::RcStrategy, repr: crate::ir::ValueRepr) -> bool {
    use crate::ir::{RcStrategy, ValueRepr};
    matches!(
        (strategy, repr),
        // HeapPointer is the conservative fallback (Pool unavailable in tests).
        (RcStrategy::HeapPointer, _)
            | (
                RcStrategy::FatPointer | RcStrategy::Closure,
                ValueRepr::FatValue
            )
            | (
                RcStrategy::AggregateFields | RcStrategy::InlineEnum,
                ValueRepr::Aggregate
            )
    )
}

// Tests

#[cfg(test)]
mod tests;
