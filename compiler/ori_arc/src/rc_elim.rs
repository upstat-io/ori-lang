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
pub fn eliminate_rc_ops(func: &mut ArcFunction) -> usize {
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

    for (&block_idx, remove_set) in &removals {
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

    candidates.len()
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

    for (&block_idx, remove_set) in &by_block {
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
                new_spans.push(old_spans.get(i).copied().flatten());
            }
        }

        block.body = new_body;
        *spans = new_spans;
    }

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

// Tests

#[cfg(test)]
mod tests {
    use ori_ir::Name;
    use ori_types::Idx;

    use crate::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
        CtorKind, LitValue,
    };
    use crate::ownership::Ownership;

    use super::eliminate_rc_ops;

    // Helpers

    fn make_func(
        params: Vec<ArcParam>,
        return_type: Idx,
        blocks: Vec<ArcBlock>,
        var_types: Vec<Idx>,
    ) -> ArcFunction {
        let span_vecs: Vec<Vec<Option<ori_ir::Span>>> =
            blocks.iter().map(|b| vec![None; b.body.len()]).collect();
        ArcFunction {
            name: Name::from_raw(1),
            params,
            return_type,
            blocks,
            entry: ArcBlockId::new(0),
            var_types,
            spans: span_vecs,
        }
    }

    fn owned_param(var: u32, ty: Idx) -> ArcParam {
        ArcParam {
            var: ArcVarId::new(var),
            ty,
            ownership: Ownership::Owned,
        }
    }

    fn v(n: u32) -> ArcVarId {
        ArcVarId::new(n)
    }

    fn b(n: u32) -> ArcBlockId {
        ArcBlockId::new(n)
    }

    /// Count `RcInc` for a specific var in a block.
    fn count_inc(func: &ArcFunction, block_idx: usize, var: ArcVarId) -> usize {
        func.blocks[block_idx]
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::RcInc { var: v, .. } if *v == var))
            .count()
    }

    /// Count `RcDec` for a specific var in a block.
    fn count_dec(func: &ArcFunction, block_idx: usize, var: ArcVarId) -> usize {
        func.blocks[block_idx]
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::RcDec { var: v } if *v == var))
            .count()
    }

    /// Count total RC ops (Inc + Dec) in a block.
    fn count_rc_ops(func: &ArcFunction, block_idx: usize) -> usize {
        func.blocks[block_idx]
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
            .count()
    }

    /// Total instruction count in a block (including RC ops).
    fn body_len(func: &ArcFunction, block_idx: usize) -> usize {
        func.blocks[block_idx].body.len()
    }

    // Basic elimination

    /// Adjacent `RcInc(x); RcDec(x)` → both eliminated.
    #[test]
    fn adjacent_inc_dec_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 1);
        assert_eq!(count_rc_ops(&func, 0), 0);
    }

    /// `RcInc(x); [unrelated instruction]; RcDec(x)` → eliminated
    /// (intervening instruction doesn't use x).
    #[test]
    fn non_adjacent_pair_no_use_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    // Unrelated instruction — doesn't use v(0).
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(42)),
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR, Idx::INT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 1);
        assert_eq!(count_rc_ops(&func, 0), 0);
        // The Let instruction remains.
        assert_eq!(body_len(&func, 0), 1);
    }

    /// `RcInc(x); use(x); RcDec(x)` → NOT eliminated (x is used between them).
    #[test]
    fn intervening_use_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    // Uses v(0) — prevents elimination.
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: Idx::UNIT,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
        assert_eq!(count_inc(&func, 0, v(0)), 1);
        assert_eq!(count_dec(&func, 0, v(0)), 1);
    }

    // Dec before Inc (unsafe)

    /// `RcDec(x); RcInc(x)` → NOT eliminated (Dec might free x).
    #[test]
    fn dec_before_inc_not_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
        assert_eq!(count_inc(&func, 0, v(0)), 1);
        assert_eq!(count_dec(&func, 0, v(0)), 1);
    }

    // Multiple independent pairs

    /// Two independent pairs: `RcInc(x); RcDec(x); RcInc(y); RcDec(y)`.
    #[test]
    fn multiple_independent_pairs() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::RcInc {
                        var: v(1),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(1) },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::UNIT,
                        value: ArcValue::Literal(LitValue::Unit),
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 2);
        assert_eq!(count_rc_ops(&func, 0), 0);
    }

    /// Interleaved vars: `RcInc(x); RcInc(y); RcDec(x); RcDec(y)`.
    /// Both pairs eliminated — different vars don't interfere.
    #[test]
    fn interleaved_vars_both_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcInc {
                        var: v(1),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::RcDec { var: v(1) },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::UNIT,
                        value: ArcValue::Literal(LitValue::Unit),
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 2);
        assert_eq!(count_rc_ops(&func, 0), 0);
    }

    // Cascading elimination

    /// Nested pairs: `RcInc(x); RcInc(x); RcDec(x); RcDec(x)`.
    /// First pass eliminates the inner pair, second pass eliminates the outer.
    #[test]
    fn nested_pairs_cascading() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 2);
        assert_eq!(count_rc_ops(&func, 0), 0);
    }

    // Edge cases

    /// No RC ops at all → no elimination.
    #[test]
    fn no_rc_ops_no_changes() {
        let mut func = make_func(
            vec![owned_param(0, Idx::INT)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::INT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    /// Only Inc, no Dec → no elimination.
    #[test]
    fn only_inc_no_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                }],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
        assert_eq!(count_inc(&func, 0, v(0)), 1);
    }

    /// Only Dec, no Inc → no elimination.
    #[test]
    fn only_dec_no_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::UNIT,
                        value: ArcValue::Literal(LitValue::Unit),
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
        assert_eq!(count_dec(&func, 0, v(0)), 1);
    }

    /// `RcInc(x, count: 2)` → NOT matched (batched Inc, conservative).
    #[test]
    fn batched_inc_not_matched() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 2,
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
        assert_eq!(count_inc(&func, 0, v(0)), 1);
        assert_eq!(count_dec(&func, 0, v(0)), 1);
    }

    // Multi-block

    /// Each block analyzed independently — pairs within a block are
    /// eliminated, cross-block pairs are not.
    #[test]
    fn multi_block_independent() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        // Eliminable pair in block 0.
                        ArcInstr::RcInc {
                            var: v(0),
                            count: 1,
                        },
                        ArcInstr::RcDec { var: v(0) },
                    ],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![],
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![
                        // Non-eliminable: use between Inc and Dec.
                        ArcInstr::RcInc {
                            var: v(0),
                            count: 1,
                        },
                        ArcInstr::Apply {
                            dst: v(1),
                            ty: Idx::UNIT,
                            func: Name::from_raw(99),
                            args: vec![v(0)],
                        },
                        ArcInstr::RcDec { var: v(0) },
                    ],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
            ],
            vec![Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        // Only block 0's pair is eliminated.
        assert_eq!(eliminated, 1);
        assert_eq!(count_rc_ops(&func, 0), 0);
        assert_eq!(count_inc(&func, 1, v(0)), 1);
        assert_eq!(count_dec(&func, 1, v(0)), 1);
    }

    // Non-RC instruction preservation

    /// Non-RC instructions are preserved after elimination.
    #[test]
    fn non_rc_instructions_preserved() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(1)),
                    },
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(2)),
                    },
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Let {
                        dst: v(3),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(3)),
                    },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR, Idx::INT, Idx::INT, Idx::INT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 1);
        // 5 original - 2 removed = 3 Let instructions.
        assert_eq!(body_len(&func, 0), 3);
        assert!(matches!(func.blocks[0].body[0], ArcInstr::Let { .. }));
        assert!(matches!(func.blocks[0].body[1], ArcInstr::Let { .. }));
        assert!(matches!(func.blocks[0].body[2], ArcInstr::Let { .. }));
    }

    // Construct / Project use

    /// `RcInc(x); Construct(..., x, ...); RcDec(x)` → NOT eliminated.
    /// x is used in the Construct.
    #[test]
    fn construct_use_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::ListLiteral,
                        args: vec![v(0)],
                    },
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::UNIT,
                        value: ArcValue::Literal(LitValue::Unit),
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    /// `RcInc(x); Project(y = x.0); RcDec(x)` → NOT eliminated.
    /// x is used in the Project.
    #[test]
    fn project_use_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::INT,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::INT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    // Partial elimination

    /// One pair eliminable, one not — only the eliminable one is removed.
    #[test]
    fn partial_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    // Eliminable: Inc(x), Dec(x) with no use between.
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                    // NOT eliminable: Inc(y), use(y), Dec(y).
                    ArcInstr::RcInc {
                        var: v(1),
                        count: 1,
                    },
                    ArcInstr::Apply {
                        dst: v(2),
                        ty: Idx::UNIT,
                        func: Name::from_raw(99),
                        args: vec![v(1)],
                    },
                    ArcInstr::RcDec { var: v(1) },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 1);
        assert_eq!(count_inc(&func, 0, v(0)), 0);
        assert_eq!(count_dec(&func, 0, v(0)), 0);
        assert_eq!(count_inc(&func, 0, v(1)), 1);
        assert_eq!(count_dec(&func, 0, v(1)), 1);
    }

    // Reuse-related patterns

    /// Pattern from reuse expansion: `IsShared` + `RcInc`/`RcDec` in slow path.
    /// The Inc/Dec pair around an `IsShared` that uses a DIFFERENT var is eliminable.
    #[test]
    fn reuse_pattern_different_var_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    // IsShared uses v(1), not v(0) — doesn't block elimination.
                    ArcInstr::IsShared {
                        dst: v(2),
                        var: v(1),
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR, Idx::STR, Idx::BOOL],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 1);
        assert_eq!(count_rc_ops(&func, 0), 0);
    }

    /// `IsShared` that uses the SAME var blocks elimination.
    #[test]
    fn is_shared_same_var_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::IsShared {
                        dst: v(1),
                        var: v(0),
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR, Idx::BOOL],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    // Sequential same-var pairs

    /// `Inc(x); Dec(x); Inc(x); Dec(x)` — two sequential pairs, both eliminated.
    #[test]
    fn sequential_same_var_pairs() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 2);
        assert_eq!(count_rc_ops(&func, 0), 0);
    }

    // Empty block

    /// Empty block body (only terminator) → no crash, no changes.
    #[test]
    fn empty_block_body() {
        let mut func = make_func(
            vec![owned_param(0, Idx::INT)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::INT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    // Span preservation

    /// Span vectors are correctly maintained after elimination.
    #[test]
    fn spans_preserved_after_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(42)),
                    },
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR, Idx::INT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 1);
        // 1 Let instruction remains.
        assert_eq!(body_len(&func, 0), 1);
        // Spans length matches body length.
        assert_eq!(func.spans[0].len(), func.blocks[0].body.len());
    }

    // Set / SetTag operations

    /// Set instruction using the tracked var prevents elimination.
    #[test]
    fn set_instruction_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::Set {
                        base: v(0),
                        field: 0,
                        value: v(1),
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    /// `SetTag` instruction using the tracked var prevents elimination.
    #[test]
    fn set_tag_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::SetTag { base: v(0), tag: 1 },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    // ApplyIndirect

    /// Indirect call using the tracked var as closure prevents elimination.
    #[test]
    fn apply_indirect_closure_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::ApplyIndirect {
                        dst: v(1),
                        ty: Idx::UNIT,
                        closure: v(0),
                        args: vec![],
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    /// Indirect call using the tracked var as an argument prevents elimination.
    #[test]
    fn apply_indirect_arg_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::ApplyIndirect {
                        dst: v(2),
                        ty: Idx::UNIT,
                        closure: v(1),
                        args: vec![v(0)],
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    // PartialApply

    /// `PartialApply` capturing the tracked var prevents elimination.
    #[test]
    fn partial_apply_prevents_elimination() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::PartialApply {
                        dst: v(1),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    // Return value

    /// `eliminate_rc_ops` returns 0 for functions with no RC ops.
    #[test]
    fn return_value_zero_when_nothing_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::INT)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::INT, Idx::INT],
        );

        assert_eq!(eliminate_rc_ops(&mut func), 0);
    }

    // Cross-block edge pair elimination

    /// `RcInc(x)` at end of B0, `RcDec(x)` at start of B1 (single
    /// predecessor) → eliminated.
    #[test]
    fn cross_block_edge_pair_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    }],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![],
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![ArcInstr::RcDec { var: v(0) }],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
            ],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 1);
        assert_eq!(count_rc_ops(&func, 0), 0);
        assert_eq!(count_rc_ops(&func, 1), 0);
    }

    /// Cross-block pair where `x` IS used by the terminator → NOT eliminated.
    #[test]
    fn cross_block_terminator_uses_var_not_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                }],
                // Return uses v(0) — blocks cross-block elimination.
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        // Only intra-block analysis can run here; no cross-block target.
        // The Inc has no matching Dec in the same block.
        assert_eq!(eliminated, 0);
    }

    /// Multi-predecessor block: `RcDec(x)` at start of merge block
    /// reached from two different predecessors → NOT eliminated
    /// (conservative, would need Inc in ALL predecessors).
    #[test]
    fn cross_block_diamond_not_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![
                // B0: branch to B1 or B2
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    }],
                    terminator: ArcTerminator::Branch {
                        cond: v(1),
                        then_block: b(1),
                        else_block: b(2),
                    },
                },
                // B1: Inc(x) then jump to merge
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    }],
                    terminator: ArcTerminator::Jump {
                        target: b(3),
                        args: vec![],
                    },
                },
                // B2: no Inc, also jumps to merge
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Jump {
                        target: b(3),
                        args: vec![],
                    },
                },
                // B3 (merge): Dec(x) at start — has TWO predecessors
                ArcBlock {
                    id: b(3),
                    params: vec![],
                    body: vec![ArcInstr::RcDec { var: v(0) }],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
            ],
            vec![Idx::STR, Idx::BOOL],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        // B3 has 2 predecessors → cross-block won't eliminate.
        // B1's Inc has no matching Dec in B1.
        assert_eq!(eliminated, 0);
        assert_eq!(count_inc(&func, 1, v(0)), 1);
        assert_eq!(count_dec(&func, 3, v(0)), 1);
    }

    /// Cross-block chain: Inc at end of B0, no use in B1, Dec at start
    /// of B1 → eliminated (B1 is single-predecessor).
    #[test]
    fn cross_block_with_intervening_unrelated_instr() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        // Unrelated instruction, then Inc(x)
                        ArcInstr::Let {
                            dst: v(1),
                            ty: Idx::INT,
                            value: ArcValue::Literal(LitValue::Int(42)),
                        },
                        ArcInstr::RcInc {
                            var: v(0),
                            count: 1,
                        },
                    ],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![],
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![ArcInstr::RcDec { var: v(0) }],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
            ],
            vec![Idx::STR, Idx::INT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 1);
        assert_eq!(count_rc_ops(&func, 0), 0);
        assert_eq!(count_rc_ops(&func, 1), 0);
        // The Let instruction in B0 remains.
        assert_eq!(body_len(&func, 0), 1);
    }

    /// Cross-block: Inc NOT at end of B0 (instruction uses x after Inc)
    /// → NOT eliminated.
    #[test]
    fn cross_block_use_after_inc_in_pred_not_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        ArcInstr::RcInc {
                            var: v(0),
                            count: 1,
                        },
                        // Uses v(0) AFTER the Inc — blocks cross-block elimination.
                        ArcInstr::Apply {
                            dst: v(1),
                            ty: Idx::UNIT,
                            func: Name::from_raw(99),
                            args: vec![v(0)],
                        },
                    ],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![],
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![ArcInstr::RcDec { var: v(0) }],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
            ],
            vec![Idx::STR, Idx::UNIT],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        assert_eq!(eliminated, 0);
    }

    /// Self-loop: block jumps to itself → NOT eliminated.
    #[test]
    fn cross_block_self_loop_not_eliminated() {
        let mut func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: b(0),
                    args: vec![],
                },
            }],
            vec![Idx::STR],
        );

        let eliminated = eliminate_rc_ops(&mut func);

        // Dec→Inc in same block is NOT safe (Dec might free).
        // Self-loop cross-block is skipped.
        assert_eq!(eliminated, 0);
    }
}
