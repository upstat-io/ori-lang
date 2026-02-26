//! Intra-block and cross-block RC pair elimination passes.
//!
//! Contains the bidirectional dataflow analysis (top-down forward and
//! bottom-up backward scans), the known-safe guarding elimination, and
//! single-predecessor cross-block edge-pair elimination.

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

// Single elimination pass

/// Run one round of elimination. Returns the number of pairs found and removed.
pub(super) fn eliminate_once(func: &mut ArcFunction) -> usize {
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
            ArcInstr::RcInc { var, count, .. } => {
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
            ArcInstr::RcDec { var, .. } => {
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
            ArcInstr::RcDec { var, .. } => {
                // Start (or restart) tracking. If we were already tracking
                // a Dec for this var, the old Dec had no matching Inc before
                // the new Dec. Replace with the tighter candidate (closer to
                // a potential Inc in program order).
                state.insert(*var, BottomUpState::Decremented { dec_pos: j });
            }
            ArcInstr::RcInc { var, count, .. } => {
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
/// Takes a map from block index -> set of instruction indices to remove.
/// Both body instructions and their corresponding spans are filtered out.
/// Spans may be shorter than the body (from prior passes); missing span
/// entries are treated as `None`.
pub(super) fn remove_instructions_by_index(
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

// Known-safe guarding pair elimination

/// Eliminate inner `RcInc`/`RcDec` pairs that are guarded by an outer pair.
///
/// Inspired by Swift's "Known Safe" optimization in `ARCSequenceOpts`. When
/// an outer `RcInc(x)` / `RcDec(x)` pair brackets a region, any inner
/// `RcInc(x)` / `RcDec(x)` pair on the same variable is provably redundant:
/// the outer pair guarantees `x`'s refcount never reaches 0 in the region.
///
/// This catches patterns that bidirectional elimination cannot:
///
/// ```text
///   RcInc(x)         <- outer guard
///   ...
///   RcInc(x)         <- inner (redundant)
///   ... use(x) ...   <- use prevents normal Inc/Dec elimination
///   RcDec(x)         <- inner (redundant)
///   ...
///   RcDec(x)         <- outer guard
/// ```
///
/// Returns the number of inner pairs eliminated.
pub(super) fn known_safe_guarding_elim(func: &mut ArcFunction) -> usize {
    let mut removals: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        // Stack of RcInc positions per variable. The bottom entry is the
        // "outer guard"; entries above it are inner Inc candidates that
        // can be paired with a Dec for removal.
        let mut inc_stacks: FxHashMap<ArcVarId, Vec<usize>> = FxHashMap::default();

        for (idx, instr) in block.body.iter().enumerate() {
            match instr {
                ArcInstr::RcInc { var, count: 1, .. } => {
                    inc_stacks.entry(*var).or_default().push(idx);
                }
                ArcInstr::RcDec { var, .. } => {
                    if let Some(stack) = inc_stacks.get_mut(var) {
                        if stack.len() > 1 {
                            // Inner pair: the most recent inner Inc is guarded
                            // by the stack entries below it. Eliminate both.
                            if let Some(inner_inc) = stack.pop() {
                                let remove_set = removals.entry(block_idx).or_default();
                                remove_set.insert(inner_inc);
                                remove_set.insert(idx);
                            }
                        } else if !stack.is_empty() {
                            // This Dec matches the outer guard Inc — pop it.
                            stack.pop();
                        }
                        if stack.is_empty() {
                            inc_stacks.remove(var);
                        }
                    }
                }
                _ => {
                    // Non-RC instructions don't affect guarding analysis.
                    // The outer Inc/Dec guarantees the refcount stays above
                    // 0 regardless of uses between them.
                }
            }
        }
    }

    if removals.is_empty() {
        return 0;
    }

    let pairs = removals.values().map(FxHashSet::len).sum::<usize>() / 2;
    remove_instructions_by_index(func, &removals);

    if pairs > 0 {
        tracing::debug!(pairs, "eliminated guarded inner RC pairs");
    }

    pairs
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
pub(super) fn eliminate_cross_block_pairs(func: &mut ArcFunction) -> usize {
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
            if let ArcInstr::RcDec { var, .. } = instr {
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
                    ArcInstr::RcInc { var, count, .. } if *var == dec_var && *count == 1 => {
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
