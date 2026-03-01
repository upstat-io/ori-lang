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

use rustc_hash::FxHashMap;

use crate::graph::{compute_postorder, compute_predecessors};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::liveness::BlockLiveness;
use crate::ArcClassification;

use super::{Uniqueness, UniquenessMap};

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
type LastUseMap = FxHashMap<ArcVarId, usize>;

/// Analyze uniqueness within a single function using forward dataflow.
///
/// Computes per-block entry/exit [`UniquenessMap`]s via fixed-point iteration
/// in reverse postorder. Uses liveness information for the dead-variable
/// optimization (alias → move when source is dead after the alias point).
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
    let entry_idx = func.entry.index();
    for param in &func.params {
        if needs_rc(param.var, func, classifier) {
            block_in[entry_idx].set(param.var, Uniqueness::MaybeShared);
        }
    }

    let mut iteration = 0u32;
    loop {
        iteration += 1;
        let mut changed = false;

        for &block_idx in &rpo {
            let new_in = if block_idx == entry_idx {
                block_in[entry_idx].clone()
            } else {
                join_predecessors(block_idx, func, &predecessors, &block_out)
            };

            let new_out = transfer_block(
                block_idx,
                func,
                classifier,
                &new_in,
                &last_use_maps[block_idx],
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
) -> UniquenessMap {
    let preds = &predecessors[block_idx];
    if preds.is_empty() {
        return UniquenessMap::new();
    }

    let mut result = map_incoming(block_idx, preds[0], func, block_out);
    for &pred_idx in &preds[1..] {
        let incoming = map_incoming(block_idx, pred_idx, func, block_out);
        result.join_from(&incoming);
    }
    result
}

/// Compute the incoming state from a single predecessor to a target block.
///
/// Takes the predecessor's exit state and maps jump arguments to the
/// target block's parameters. Invoke `dst` is set to `MaybeShared`
/// at the normal successor (function call result).
fn map_incoming(
    target_idx: usize,
    pred_idx: usize,
    func: &ArcFunction,
    block_out: &[UniquenessMap],
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
        ArcTerminator::Invoke { dst, normal, .. } if normal.index() == target_idx => {
            // Function call result: conservative (refined by §07.3).
            incoming.set(*dst, Uniqueness::MaybeShared);
        }
        _ => {}
    }

    incoming
}

// -- Transfer function --

/// Apply the transfer function to all instructions in a block.
fn transfer_block(
    block_idx: usize,
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    block_in: &UniquenessMap,
    last_use: &LastUseMap,
) -> UniquenessMap {
    let block = &func.blocks[block_idx];
    let mut state = block_in.clone();
    for (pos, instr) in block.body.iter().enumerate() {
        transfer_instr(&mut state, instr, pos, func, classifier, last_use);
    }
    state
}

/// Transfer function for a single instruction.
///
/// Updates the uniqueness map based on the instruction's semantics:
/// fresh allocations → `Unique`, aliases with dead source → move,
/// aliases with live source → both `Shared`, calls → `MaybeShared`.
fn transfer_instr(
    state: &mut UniquenessMap,
    instr: &ArcInstr,
    pos: usize,
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    last_use: &LastUseMap,
) {
    match instr {
        // Variable alias: move or sharing depending on source liveness.
        ArcInstr::Let {
            dst,
            value: ArcValue::Var(src),
            ..
        } => {
            if needs_rc(*dst, func, classifier) {
                if is_last_use(*src, pos, last_use) {
                    // Source is dead after this point → move semantics.
                    state.set(*dst, state.get(*src));
                } else {
                    // Source is still live → two references → both Shared.
                    state.mark_shared(*src);
                    state.mark_shared(*dst);
                }
            }
        }

        // Fresh allocations and fresh values: RC = 1 at birth.
        // Construct/PartialApply create new heap objects, literals and prim ops
        // produce fresh values, and Reuse recycles an allocation (still RC = 1).
        ArcInstr::Construct { dst, .. }
        | ArcInstr::PartialApply { dst, .. }
        | ArcInstr::Let {
            dst,
            value: ArcValue::Literal(_),
            ..
        }
        | ArcInstr::Let {
            dst,
            value: ArcValue::PrimOp { .. },
            ..
        }
        | ArcInstr::Reuse { dst, .. } => {
            if needs_rc(*dst, func, classifier) {
                state.mark_unique(*dst);
            }
        }

        // Function calls and projections: conservative MaybeShared.
        // Interprocedural analysis (§07.3) refines call results.
        // Projections borrow from the parent — may be shared.
        ArcInstr::Apply { dst, .. }
        | ArcInstr::ApplyIndirect { dst, .. }
        | ArcInstr::Project { dst, .. }
        | ArcInstr::IsShared { dst, .. } => {
            if needs_rc(*dst, func, classifier) {
                state.set(*dst, Uniqueness::MaybeShared);
            }
        }

        // Select: join of both branch values.
        ArcInstr::Select {
            dst,
            true_val,
            false_val,
            ..
        } => {
            if needs_rc(*dst, func, classifier) {
                let joined = state.get(*true_val).join(state.get(*false_val));
                state.set(*dst, joined);
            }
        }

        // RC operations and mutations (not expected pre-insertion; handle defensively).
        ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. }
        | ArcInstr::Reset { .. } => {}
    }
}

// -- Dead variable tracking --

/// Precompute the last-use position for each variable in a block.
///
/// `usize::MAX` means the variable is used in the terminator or `live_out`.
/// Otherwise, the value is the index of the last body instruction that uses it.
fn precompute_last_use(
    block: &crate::ir::ArcBlock,
    live_out: &crate::liveness::LiveSet,
) -> LastUseMap {
    let mut last_use = LastUseMap::default();

    // Variables in live_out: used after the block.
    for &var in live_out {
        last_use.insert(var, usize::MAX);
    }

    // Terminator uses: also "after" the block body.
    for var in block.terminator.used_vars() {
        last_use.insert(var, usize::MAX);
    }

    // Walk body backward: first insertion per variable is its last use.
    for (i, instr) in block.body.iter().enumerate().rev() {
        for var in instr.used_vars() {
            last_use.entry(var).or_insert(i);
        }
    }

    last_use
}

/// Check whether the current instruction at `pos` is the last use of `var`.
///
/// Returns `true` if `var` is dead after instruction `pos`, meaning an alias
/// at this point can be treated as a move (ownership transfer).
#[inline]
fn is_last_use(var: ArcVarId, pos: usize, last_use: &LastUseMap) -> bool {
    match last_use.get(&var) {
        None => true,
        Some(&last_pos) => last_pos <= pos,
    }
}

/// Check whether a variable's type requires reference counting.
#[inline]
fn needs_rc(var: ArcVarId, func: &ArcFunction, classifier: &dyn ArcClassification) -> bool {
    let idx = var.index();
    if idx < func.var_types.len() {
        classifier.needs_rc(func.var_types[idx])
    } else {
        true
    }
}

#[cfg(test)]
mod tests;
