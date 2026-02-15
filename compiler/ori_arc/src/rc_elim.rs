//! RC elimination pass for ARC IR (Section 08).
//!
//! Eliminates redundant `RcInc`/`RcDec` pairs using bidirectional intra-block
//! dataflow analysis. An `RcInc` immediately followed by an `RcDec` on the
//! same variable (with no intervening use) is a net-zero no-op that can be
//! safely removed.
//!
//! # Pipeline Position
//!
//! This pass runs AFTER constructor reuse expansion (Section 09). Input is
//! ARC IR with `RcInc`/`RcDec` from both RC insertion (07) and reuse
//! expansion (09). Execution order: 07 → 09 → 08 (this pass).
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

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

// Lattice states

/// Top-down RC state for a variable during forward scan.
///
/// Tracks whether we've seen an `RcInc` and are looking for a matching
/// `RcDec` without any intervening use of the variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TopDownState {
    /// Seen an `RcInc` at `inc_pos`. Looking forward for a matching `RcDec`.
    Incremented { inc_pos: usize },
    /// Variable used between the `RcInc` and a potential `RcDec`.
    /// Cannot eliminate — the value must stay alive during the use.
    MightBeUsed,
}

/// Bottom-up RC state for a variable during backward scan.
///
/// Tracks whether we've seen an `RcDec` and are looking backward for a
/// matching `RcInc` without any intervening use of the variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BottomUpState {
    /// Seen an `RcDec` at `dec_pos`. Looking backward for a matching `RcInc`.
    Decremented { dec_pos: usize },
    /// Variable used between the `RcDec` and a potential `RcInc`.
    /// Cannot eliminate.
    MightBeUsed,
}

// Elimination candidate

/// A matched `RcInc`/`RcDec` pair eligible for safe elimination.
///
/// Both positions are instruction indices within the same block's body.
/// The `inc_pos` is always less than `dec_pos` (Inc before Dec in program order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EliminationCandidate {
    /// The variable whose RC ops are being eliminated.
    var: ArcVarId,
    /// Block index within the function.
    block: usize,
    /// Instruction index of the `RcInc` within the block body.
    inc_pos: usize,
    /// Instruction index of the `RcDec` within the block body.
    dec_pos: usize,
}

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
    let mut total = 0;

    loop {
        let intra = eliminate_once(func);
        let cross = eliminate_cross_block_pairs(func);
        let eliminated = intra + cross;
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

// Single elimination pass

/// Run one round of elimination. Returns the number of pairs found and removed.
fn eliminate_once(func: &mut ArcFunction) -> usize {
    let mut candidates = Vec::new();

    for block_idx in 0..func.blocks.len() {
        let body = &func.blocks[block_idx].body;
        top_down_block_pass(block_idx, body, &mut candidates);
        bottom_up_block_pass(block_idx, body, &mut candidates);
    }

    if candidates.is_empty() {
        return 0;
    }

    // Deduplicate: both passes may find the same pair.
    candidates.sort_by_key(|c| (c.block, c.inc_pos, c.dec_pos));
    candidates
        .dedup_by(|a, b| a.block == b.block && a.inc_pos == b.inc_pos && a.dec_pos == b.dec_pos);

    apply_eliminations(func, &candidates)
}

// Top-down (forward) pass

/// Scan a block's instructions forward, looking for `RcInc(x); ...; RcDec(x)`
/// pairs where no instruction between them uses `x`.
fn top_down_block_pass(
    block_idx: usize,
    body: &[ArcInstr],
    candidates: &mut Vec<EliminationCandidate>,
) {
    let mut state: FxHashMap<ArcVarId, TopDownState> = FxHashMap::default();

    for (j, instr) in body.iter().enumerate() {
        match instr {
            ArcInstr::RcInc { var, count } => {
                if *count == 1 {
                    // Start (or restart) tracking this variable.
                    // Restarting is correct: if we were already tracking an
                    // Inc for this var, there was no Dec between them, so
                    // the old Inc is unmatchable. Start fresh with the new one.
                    state.insert(*var, TopDownState::Incremented { inc_pos: j });
                } else {
                    // Batched Inc (count > 1): treat conservatively as a use.
                    invalidate_td(&mut state, *var);
                }
            }
            ArcInstr::RcDec { var } => {
                if let Some(TopDownState::Incremented { inc_pos }) = state.get(var) {
                    // Match: Inc at inc_pos, Dec at j, no use of var between them.
                    candidates.push(EliminationCandidate {
                        var: *var,
                        block: block_idx,
                        inc_pos: *inc_pos,
                        dec_pos: j,
                    });
                }
                // Reset regardless — matched or not, this Dec is consumed.
                state.remove(var);
            }
            other => {
                // Non-RC instruction: invalidate tracking for any variables it uses.
                for used in other.used_vars() {
                    invalidate_td(&mut state, used);
                }
            }
        }
    }
}

/// Transition a top-down state from `Incremented` to `MightBeUsed`.
///
/// Called when a non-RC instruction uses a tracked variable.
fn invalidate_td(state: &mut FxHashMap<ArcVarId, TopDownState>, var: ArcVarId) {
    if let Some(s) = state.get_mut(&var) {
        if matches!(s, TopDownState::Incremented { .. }) {
            *s = TopDownState::MightBeUsed;
        }
    }
}

// Bottom-up (backward) pass

/// Scan a block's instructions backward, looking for `RcInc(x); ...; RcDec(x)`
/// pairs where no instruction between them uses `x`.
///
/// Complementary to the top-down pass. In practice, both passes find the
/// same pairs for intra-block analysis, but having both provides a safety net.
fn bottom_up_block_pass(
    block_idx: usize,
    body: &[ArcInstr],
    candidates: &mut Vec<EliminationCandidate>,
) {
    let mut state: FxHashMap<ArcVarId, BottomUpState> = FxHashMap::default();

    for (j, instr) in body.iter().enumerate().rev() {
        match instr {
            ArcInstr::RcDec { var } => {
                // Start (or restart) tracking. If we were already tracking
                // a Dec for this var, the old Dec had no matching Inc before
                // the new Dec. Replace with the tighter candidate (closer to
                // a potential Inc in program order).
                state.insert(*var, BottomUpState::Decremented { dec_pos: j });
            }
            ArcInstr::RcInc { var, count } => {
                if *count == 1 {
                    if let Some(BottomUpState::Decremented { dec_pos }) = state.get(var) {
                        // Match: Inc at j, Dec at dec_pos, no use of var between.
                        candidates.push(EliminationCandidate {
                            var: *var,
                            block: block_idx,
                            inc_pos: j,
                            dec_pos: *dec_pos,
                        });
                    }
                    // Reset regardless.
                    state.remove(var);
                } else {
                    // Batched Inc (count > 1): treat conservatively as a use.
                    invalidate_bu(&mut state, *var);
                }
            }
            other => {
                // Non-RC instruction: invalidate tracking for any variables it uses.
                for used in other.used_vars() {
                    invalidate_bu(&mut state, used);
                }
            }
        }
    }
}

/// Transition a bottom-up state from `Decremented` to `MightBeUsed`.
///
/// Called when a non-RC instruction uses a tracked variable.
fn invalidate_bu(state: &mut FxHashMap<ArcVarId, BottomUpState>, var: ArcVarId) {
    if let Some(s) = state.get_mut(&var) {
        if matches!(s, BottomUpState::Decremented { .. }) {
            *s = BottomUpState::MightBeUsed;
        }
    }
}

// Apply eliminations

/// Remove the instructions at the matched positions. Returns the number
/// of pairs eliminated.
fn apply_eliminations(func: &mut ArcFunction, candidates: &[EliminationCandidate]) -> usize {
    // Group removal positions by block for batch processing.
    let mut removals: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    for c in candidates {
        let set = removals.entry(c.block).or_default();
        set.insert(c.inc_pos);
        set.insert(c.dec_pos);
    }

    remove_instructions_by_index(func, &removals);

    candidates.len()
}

/// Remove instructions at specified indices from each block.
///
/// Takes a map from block index → set of instruction indices to remove.
/// Both body instructions and their corresponding spans are filtered out.
/// Spans may be shorter than the body (from prior passes); missing span
/// entries are treated as `None`.
fn remove_instructions_by_index(
    func: &mut ArcFunction,
    removals: &FxHashMap<usize, FxHashSet<usize>>,
) {
    for (&block_idx, remove_set) in removals {
        let block = &mut func.blocks[block_idx];
        let spans = &mut func.spans[block_idx];

        let old_body = std::mem::take(&mut block.body);
        let old_spans = std::mem::take(spans);

        let retained = old_body.len() - remove_set.len();
        let mut new_body = Vec::with_capacity(retained);
        let mut new_spans = Vec::with_capacity(retained);

        for (i, instr) in old_body.into_iter().enumerate() {
            if !remove_set.contains(&i) {
                new_body.push(instr);
                // Spans may be shorter than body (e.g., after prior passes).
                new_spans.push(old_spans.get(i).copied().flatten());
            }
        }

        block.body = new_body;
        *spans = new_spans;
    }
}

// Cross-block edge-pair elimination

/// Eliminate `RcInc(x)` at end of block P / `RcDec(x)` at start of block B
/// where B has exactly one predecessor P and `x` is not used in between
/// (i.e., P's terminator does not use `x` and no instruction between the
/// Inc position and end of P's body uses `x`).
///
/// This targets the most common cross-block redundancy created by RC
/// insertion's edge cleanup trampolines: P ends with `RcInc(x); Jump(B)`
/// and B starts with `RcDec(x)`.
///
/// Returns the number of pairs eliminated.
fn eliminate_cross_block_pairs(func: &mut ArcFunction) -> usize {
    let predecessors = compute_predecessors(func);
    let mut removals: Vec<(usize, usize)> = Vec::new();

    for (block_idx, preds) in predecessors.iter().enumerate() {
        // Only handle single-predecessor blocks (safe, no merging needed).
        if preds.len() != 1 {
            continue;
        }
        let pred_idx = preds[0];
        // Skip self-loops.
        if pred_idx == block_idx {
            continue;
        }

        // Collect leading RcDec instructions at the start of this block.
        let succ_body = &func.blocks[block_idx].body;
        let mut leading_decs: Vec<(usize, ArcVarId)> = Vec::new();
        for (j, instr) in succ_body.iter().enumerate() {
            if let ArcInstr::RcDec { var } = instr {
                leading_decs.push((j, *var));
            } else {
                // Stop at the first non-Dec instruction.
                break;
            }
        }

        if leading_decs.is_empty() {
            continue;
        }

        // Collect variables used by the predecessor's terminator.
        let term_uses: FxHashSet<ArcVarId> = func.blocks[pred_idx]
            .terminator
            .used_vars()
            .into_iter()
            .collect();

        let pred_body = &func.blocks[pred_idx].body;

        for &(dec_pos_in_succ, dec_var) in &leading_decs {
            // The terminator must not use this variable.
            if term_uses.contains(&dec_var) {
                continue;
            }

            // Scan predecessor body backwards for a matching RcInc.
            let mut found_inc_pos = None;
            for j in (0..pred_body.len()).rev() {
                match &pred_body[j] {
                    ArcInstr::RcInc { var, count } if *var == dec_var && *count == 1 => {
                        found_inc_pos = Some(j);
                        break;
                    }
                    other => {
                        // If this instruction uses the variable, the Inc (if any
                        // earlier) can't be eliminated with this Dec.
                        if other.uses_var(dec_var) {
                            break;
                        }
                    }
                }
            }

            if let Some(inc_pos) = found_inc_pos {
                // Record the pair for removal: (block, position).
                removals.push((pred_idx, inc_pos));
                removals.push((block_idx, dec_pos_in_succ));
            }
        }
    }

    if removals.is_empty() {
        return 0;
    }

    // Group by block and apply.
    let mut by_block: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    for (blk, pos) in &removals {
        by_block.entry(*blk).or_default().insert(*pos);
    }

    remove_instructions_by_index(func, &by_block);

    let pairs = removals.len() / 2;
    if pairs > 0 {
        tracing::debug!(
            function = func.name.raw(),
            pairs,
            "eliminated cross-block RC pairs",
        );
    }

    pairs
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
                ArcInstr::RcInc { var, count: 1 } | ArcInstr::RcDec { var } => *var,
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
                    .any(|i| matches!(i, ArcInstr::RcDec { var: v } if *v == source));

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
                ArcInstr::RcInc { var, count: 1 } if !term_uses.contains(var) => {
                    trailing.insert(*var);
                }
                ArcInstr::RcDec { var } => {
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
            if let ArcInstr::RcDec { var } = instr {
                if available.contains(var) {
                    // Remove the RcDec here and the trailing RcInc in each predecessor.
                    removals.push((block_idx, j));
                    for &pred_idx in block_preds {
                        // Find and mark the trailing RcInc for this var.
                        let pred_body = &func.blocks[pred_idx].body;
                        for (pi, pinstr) in pred_body.iter().enumerate().rev() {
                            if matches!(pinstr, ArcInstr::RcInc { var: v, count: 1 } if *v == *var)
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

// Tests

#[cfg(test)]
mod tests;
