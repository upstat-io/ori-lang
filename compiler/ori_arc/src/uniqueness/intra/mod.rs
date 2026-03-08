//! Intraprocedural uniqueness analysis (Section 07.2).
//!
//! Forward dataflow analysis that determines which variables are provably
//! uniquely owned (RC == 1) at each program point within a single function.
//!
//! # Algorithm
//!
//! 1. **Initialize**: function parameters → `MaybeShared` (callers may share)
//! 2. **Forward pass** over blocks in reverse postorder (RPO)
//! 3. **Block entry**: join predecessor exit states
//! 4. **Transfer** through each instruction:
//!    - `Construct` / `PartialApply` → `Unique` (fresh allocation, RC = 1)
//!    - `Let(Var(src))` where src is dead → move (inherit src's uniqueness)
//!    - `Let(Var(src))` where src is live → sharing (both `Shared`)
//!    - `Apply` / `ApplyIndirect` → `MaybeShared` (conservative; refined by §07.3)
//!    - `Project` → `MaybeShared` (borrows from parent)
//!    - `Select` → join of both value states
//! 5. **Iterate** until fixed point (for back edges in loops)
//!
//! # Dead Variable Optimization
//!
//! When `let b = a` and `a` is never used again, the alias is effectively a
//! **move**: `b` inherits `a`'s uniqueness rather than both becoming `Shared`.
//! This is the key optimization that keeps uniqueness through Ori's value
//! semantics rebinding pattern (`let a = a.push(x)`).
//!
//! # Key Insight
//!
//! **COW operation results are always `Unique`**. Whether the fast path
//! (in-place, RC was 1) or slow path (copy, new allocation) executes, the
//! output has exactly one reference. This is handled conservatively here
//! (`Apply` → `MaybeShared`) and refined by interprocedural analysis (§07.3)
//! with hardcoded collection method summaries.

mod transfer;

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::graph::{compute_postorder, compute_predecessors};
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};
use crate::liveness::BlockLiveness;
use crate::ArcClassification;

use super::{Uniqueness, UniquenessMap, UniquenessSummary};

use transfer::transfer_block;
pub(crate) use transfer::{needs_rc, precompute_last_use, transfer_instr};

/// Result of intraprocedural uniqueness analysis for a single function.
///
/// Contains per-block entry and exit uniqueness maps. To get the uniqueness
/// of a variable at a specific instruction within a block, start from
/// `block_in[block_idx]` and replay the transfer function up to that point.
pub struct UniquenessResult {
    /// Uniqueness state at the entry of each block (after joining predecessors).
    /// Indexed by `ArcBlockId::index()`.
    pub block_in: Vec<UniquenessMap>,
    /// Uniqueness state at the exit of each block (after all instructions).
    /// Indexed by `ArcBlockId::index()`.
    pub block_out: Vec<UniquenessMap>,
}

/// Per-block map of each variable's last use position.
///
/// - `usize::MAX`: variable is used in the terminator or `live_out`
/// - Other value: index of the last instruction in the block body that uses it
pub(crate) type LastUseMap = FxHashMap<ArcVarId, usize>;

/// Analyze uniqueness within a single function using forward dataflow.
///
/// Standalone version that treats all function call results as `MaybeShared`.
/// Use [`analyze_with_summaries`] for interprocedural refinement.
///
/// # Arguments
///
/// * `func` — the ARC IR function to analyze (pre-RC-insertion)
/// * `classifier` — type classifier for determining which variables are RC'd
/// * `liveness` — precomputed liveness (from [`compute_liveness`](crate::liveness::compute_liveness))
pub fn analyze_intraprocedural(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
) -> UniquenessResult {
    analyze_inner(func, classifier, liveness, &FxHashMap::default())
}

/// Analyze uniqueness with interprocedural callee summaries.
///
/// Like [`analyze_intraprocedural`], but uses `summaries` to refine `Apply`
/// and `Invoke` call results: if the callee has a known summary, its return
/// value uniqueness is used instead of the conservative `MaybeShared`.
///
/// This is the version used by [`super::inter::analyze_program`] during
/// SCC-based interprocedural analysis.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the concrete type used throughout ori_arc"
)]
pub fn analyze_with_summaries(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    summaries: &FxHashMap<Name, UniquenessSummary>,
) -> UniquenessResult {
    analyze_inner(func, classifier, liveness, summaries)
}

/// Inner implementation shared by standalone and summary-aware analysis.
fn analyze_inner(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    summaries: &FxHashMap<Name, UniquenessSummary>,
) -> UniquenessResult {
    let num_blocks = func.blocks.len();
    tracing::debug!(
        function = func.name.raw(),
        num_blocks,
        num_vars = func.var_types.len(),
        "starting intraprocedural uniqueness analysis"
    );

    let predecessors = compute_predecessors(func);
    let rpo = {
        let mut po = compute_postorder(func);
        po.reverse();
        po
    };
    let last_use_maps: Vec<LastUseMap> = func
        .blocks
        .iter()
        .enumerate()
        .map(|(i, block)| precompute_last_use(block, &liveness.live_out[i]))
        .collect();

    let capacity = func.var_types.len() / 4 + 1;
    let mut block_in: Vec<UniquenessMap> = (0..num_blocks)
        .map(|_| UniquenessMap::with_capacity(capacity))
        .collect();
    let mut block_out: Vec<UniquenessMap> = (0..num_blocks)
        .map(|_| UniquenessMap::with_capacity(capacity))
        .collect();

    // Initialize entry block: function parameters are MaybeShared.
    // Computed once — entry state is invariant across fixpoint iterations.
    let entry_idx = func.entry.index();
    let entry_in = {
        let mut state = UniquenessMap::with_capacity(capacity);
        for param in &func.params {
            if needs_rc(param.var, func, classifier) {
                state.set(param.var, Uniqueness::MaybeShared);
            }
        }
        state
    };
    block_in[entry_idx] = entry_in.clone();

    let mut iteration = 0u32;
    loop {
        iteration += 1;
        let mut changed = false;

        for &block_idx in &rpo {
            let new_in = if block_idx == entry_idx {
                entry_in.clone()
            } else {
                join_predecessors(block_idx, func, &predecessors, &block_out, summaries)
            };

            let new_out = transfer_block(
                block_idx,
                func,
                classifier,
                &new_in,
                &last_use_maps[block_idx],
                summaries,
            );

            if new_in != block_in[block_idx] || new_out != block_out[block_idx] {
                changed = true;
                block_in[block_idx] = new_in;
                block_out[block_idx] = new_out;
            }
        }

        if !changed {
            break;
        }
    }

    tracing::debug!(iterations = iteration, "uniqueness analysis converged");

    UniquenessResult {
        block_in,
        block_out,
    }
}

// -- Block entry computation --

/// Compute a block's entry state by joining all predecessors' exit states.
///
/// For each predecessor, maps jump arguments to block parameters and
/// joins the resulting states using the lattice join.
fn join_predecessors(
    block_idx: usize,
    func: &ArcFunction,
    predecessors: &[Vec<usize>],
    block_out: &[UniquenessMap],
    summaries: &FxHashMap<Name, UniquenessSummary>,
) -> UniquenessMap {
    let preds = &predecessors[block_idx];
    if preds.is_empty() {
        return UniquenessMap::new();
    }

    let mut result = map_incoming(block_idx, preds[0], func, block_out, summaries);
    for &pred_idx in &preds[1..] {
        let incoming = map_incoming(block_idx, pred_idx, func, block_out, summaries);
        result.join_from(&incoming);
    }
    result
}

/// Compute the incoming state from a single predecessor to a target block.
///
/// Takes the predecessor's exit state and maps jump arguments to the
/// target block's parameters. Invoke `dst` uniqueness is determined by
/// the callee's summary (if available) or defaults to `MaybeShared`.
fn map_incoming(
    target_idx: usize,
    pred_idx: usize,
    func: &ArcFunction,
    block_out: &[UniquenessMap],
    summaries: &FxHashMap<Name, UniquenessSummary>,
) -> UniquenessMap {
    let pred_block = &func.blocks[pred_idx];
    let target_block = &func.blocks[target_idx];
    let pred_out = &block_out[pred_idx];
    let mut incoming = pred_out.clone();

    match &pred_block.terminator {
        ArcTerminator::Jump { target, args } if target.index() == target_idx => {
            for (i, &(param_var, _)) in target_block.params.iter().enumerate() {
                if let Some(&arg) = args.get(i) {
                    incoming.set(param_var, pred_out.get(arg));
                }
            }
        }
        ArcTerminator::Invoke {
            dst,
            normal,
            func: callee,
            ..
        } if normal.index() == target_idx => {
            let return_uniq = summaries
                .get(callee)
                .map_or(Uniqueness::MaybeShared, |s| s.return_val);
            incoming.set(*dst, return_uniq);
        }
        _ => {}
    }

    incoming
}

#[cfg(test)]
mod tests;
