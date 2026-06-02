//! DP-2/DP-3 burden-op elimination consumer.
//!
//! Walks every block backward; for each `BurdenInc` / `BurdenDec` /
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` site, queries
//! the AIMS lattice via DP-2 (`is_rc_dec_unnecessary` at
//! `aims/transfer/mod.rs:403`) or DP-3 (`is_rc_inc_elidable` at
//! `aims/transfer/mod.rs:411`); on `true`, removes the instruction.
//!
//! # Pipeline position
//!
//! Runs inside `emit_rc_unified` between Phase 2.1 (project-escape Incs) and
//! Phase 3 (coalesce). Burden ops are TF-N/A in both forward and backward
//! transfer (verified at `aims/transfer/mod.rs:94-104, 287-297`), so the
//! per-block state queried via `var_state_at_block_exit(block, var)` is the
//! state at every burden-op site within that block: burden ops carry no
//! transfer effect, so `block_exit_state[var]` = state at every burden-op
//! position for `var` within the block (subject to non-burden instruction
//! effects within the block widening it — but only widening is conservative
//! for elimination, never unsound).
//!
//! # Soundness
//!
//! Per the DP-2 / DP-3 predicate truth tables:
//! - DP-2 `is_rc_dec_unnecessary(s)` ⟺ `s.cardinality = Absent ∨
//!   s.consumption = Dead` — both imply no future use, so a dec is
//!   redundant.
//! - DP-3 `is_rc_inc_elidable(s)` ⟺ `s.cardinality = Once ∧
//!   s.consumption = Linear` — single linear consumer, no inc needed.
//!
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` follow the
//! `BurdenDec` elimination rule on the WHOLE-VAR state. `BurdenDecField`
//! targets a sub-field of `base`, so DP-2 is queried against `base`'s
//! state — if the WHOLE value is dead/absent, every field-positional dec
//! against it is redundant too.
//!
//! # Paired elimination — VF-1 balance preservation
//!
//! `var_state_at_block_exit` returns `AimsState::BOTTOM` (Borrowed, Dead,
//! Absent, ...) for variables not present in the block's exit map — most
//! notably for variables defined and consumed in a terminal (Return /
//! Resume / Unreachable) block whose exit map is empty because there is
//! no successor demand to capture. Per DP-2 truth table, BOTTOM satisfies
//! `is_rc_dec_unnecessary` (Absent ∨ Dead), but BOTTOM fails DP-3
//! (`Once ∧ Linear`). A naïve per-op pass would elide the `BurdenDec`
//! but retain the `BurdenInc`, producing `Σ Inc - Σ Dec = +1` and
//! violating VF-1 per `aims/verify/burden_balance.rs`.
//!
//! The fix is paired elimination: group ops by target var per block,
//! check DP-2 / DP-3 against the var's exit state once, and elide ALL
//! Inc + Dec ops for that var ONLY when BOTH predicates fire — otherwise
//! retain every op for that var so the intraprocedural net stays zero.
//! When DP-2 elides a `BurdenDec`, the corresponding `BurdenInc` (if
//! elidable per DP-3) is also elided in the same pass; if DP-3 does not
//! fire on the matching Inc, the dec is retained to preserve VF-1 balance.
//!
//! # References
//!
//! - DP-2 + DP-3 — predicate truth tables.
//! - `aims/transfer/mod.rs:403,411` — predicate source.
//! - Koka Perceus paired dup/drop elimination (Reinking et al., PLDI 2021).

#[cfg(test)]
mod tests;

use rustc_hash::FxHashMap;

use crate::aims::intraprocedural::AimsStateMap;
use crate::aims::transfer::{is_rc_dec_unnecessary, is_rc_inc_elidable};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId};

/// Eliminate burden ops whose DP-2/DP-3 predicates fire.
///
/// Walks every block backward (matches `coalesce_block_rc` shape at
/// `aims/emit_rc/coalesce/mod.rs:44`); for each burden-op instruction,
/// queries the appropriate predicate against `var_state_at_block_exit`
/// for the op's target var; removes the instruction when the predicate
/// returns `true`.
///
/// Operates in place on `func.blocks[*].body`. Non-burden instructions
/// are preserved verbatim. Run BEFORE `coalesce_block_rc` (Phase 3) so
/// coalesce operates on the post-elimination IR with redundant ops
/// already removed.
pub(crate) fn eliminate_burden_ops(func: &mut ArcFunction, state_map: &AimsStateMap) {
    // INVARIANT: Phase 6 is an OPTIMIZER over the burden-emitted
    // baseline: it ELIMINATES burden ops, NEVER CONSTRUCTS them. Capture the
    // per-kind burden-op census before the pass; assert the post-pass census is
    // ≤ the pre-pass census for EVERY kind. Any future regression that appends a
    // `BurdenInc` / `BurdenDec*` inside Phase 6 trips this guard loudly rather
    // than silently lowering to a spurious `RcInc`/`RcDec` downstream.
    // Canonical RC-Emission Path: Phase 5 emits, Phase 6
    // eliminates, Phase 7 mechanically lowers.
    #[cfg(debug_assertions)]
    let before = burden_op_census(func);

    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);
        eliminate_in_block(&mut block.body, state_map, block_id);
    }

    #[cfg(debug_assertions)]
    debug_assert_burden_removal_only(&before, &burden_op_census(func));
}

/// Per-kind burden-op census across all blocks of a function.
///
/// Five counts, one per burden-op variant, in the order `[BurdenInc,
/// BurdenDec, BurdenDecPartial, BurdenDecField, BurdenDecVariant]`. Used by the
/// removal-only structural guard in [`eliminate_burden_ops`].
#[cfg(any(debug_assertions, test))]
fn burden_op_census(func: &ArcFunction) -> [usize; 5] {
    let mut census = [0usize; 5];
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { .. } => census[0] += 1,
                ArcInstr::BurdenDec { .. } => census[1] += 1,
                ArcInstr::BurdenDecPartial { .. } => census[2] += 1,
                ArcInstr::BurdenDecField { .. } => census[3] += 1,
                ArcInstr::BurdenDecVariant { .. } => census[4] += 1,
                _ => {}
            }
        }
    }
    census
}

/// Whether a Phase-6 burden census transition is removal-only.
///
/// `true` iff the post-pass census of EVERY burden-op kind is `≤` its
/// pre-pass census — i.e. Phase 6 removed or left-alone every kind and
/// constructed none. This is the SSOT predicate behind the removal-only
/// structural guard; both the debug-assert and the negative pin consult it.
#[cfg(any(debug_assertions, test))]
fn is_burden_removal_only(before: &[usize; 5], after: &[usize; 5]) -> bool {
    (0..5).all(|kind| after[kind] <= before[kind])
}

/// Removal-only structural guard.
///
/// Phase 6 (the lattice optimizer) MUST only remove or annotate burden ops —
/// it MUST NOT construct new ones: `eliminate_burden_ops` consumes DP-2/DP-3
/// at burden-op sites and NEVER constructs burden ops. The post-pass census
/// of EVERY burden-op kind is
/// therefore `≤` its pre-pass census. A violation is a Phase-6 construction
/// regression: a `BurdenInc`/`BurdenDec*` was appended where only elimination
/// is permitted, which would mechanically lower to a spurious `RcInc`/`RcDec`
/// in Phase 7 and corrupt RC balance.
#[cfg(debug_assertions)]
#[track_caller]
fn debug_assert_burden_removal_only(before: &[usize; 5], after: &[usize; 5]) {
    const KIND_NAMES: [&str; 5] = [
        "BurdenInc",
        "BurdenDec",
        "BurdenDecPartial",
        "BurdenDecField",
        "BurdenDecVariant",
    ];
    debug_assert!(
        is_burden_removal_only(before, after),
        "AIMS Phase-6 invariant: eliminate_burden_ops constructed burden ops \
         where only elimination is permitted — Phase 6 MUST eliminate burden \
         ops, never construct them. \
         per-kind census {KIND_NAMES:?}: before = {before:?}, after = {after:?}",
    );
}

/// Eliminate redundant burden ops within one block's body.
///
/// Two-pass paired elimination per VF-1 intraprocedural
/// balance:
/// 1. Scan once; group all whole-var burden ops by target var; record per-op
///    DP-2 / DP-3 verdicts against the var's exit-state lookup.
/// 2. For each var, elide its Inc + Dec ops ONLY when DP-3 fires on EVERY
///    Inc AND DP-2 fires on EVERY whole-var Dec — otherwise retain every op
///    for that var so `Σ Inc - Σ Dec` stays at its pre-elimination value.
/// 3. `BurdenDecField` queries DP-2 against `base`'s whole-var state but is
///    NOT included in the whole-var balance (per `aims/verify/burden_delta.rs`
///    it contributes to a separate field-grain accumulator); elision is
///    per-op, independent of the whole-var pairing.
// Per-var balance state. `inc_indices` / `dec_indices` index into `body`;
// the booleans accumulate AND over per-op predicate verdicts so a single
// non-elidable op pins all the var's whole-var ops to RETAIN.
#[derive(Default)]
struct WholeVarBalance {
    inc_indices: Vec<usize>,
    dec_indices: Vec<usize>,
    all_inc_elidable: bool,
    all_dec_unnecessary: bool,
}

fn eliminate_in_block(body: &mut Vec<ArcInstr>, state_map: &AimsStateMap, block_id: ArcBlockId) {
    if body.is_empty() {
        return;
    }

    let mut balances: FxHashMap<ArcVarId, WholeVarBalance> = FxHashMap::default();
    let mut remove: Vec<bool> = vec![false; body.len()];

    // Pass 1: classify each whole-var op into the per-var balance buckets;
    // settle field-grain Decs (BurdenDecField) inline since they do not
    // participate in whole-var pairing.
    for (idx, instr) in body.iter().enumerate() {
        match instr {
            ArcInstr::BurdenInc { var } => {
                let state = state_map.var_state_at_block_exit(block_id, *var);
                let entry = balances.entry(*var).or_insert(WholeVarBalance {
                    all_inc_elidable: true,
                    all_dec_unnecessary: true,
                    ..WholeVarBalance::default()
                });
                entry.inc_indices.push(idx);
                entry.all_inc_elidable &= is_rc_inc_elidable(&state);
            }
            ArcInstr::BurdenDec { var }
            | ArcInstr::BurdenDecPartial { var, .. }
            | ArcInstr::BurdenDecVariant { var } => {
                let state = state_map.var_state_at_block_exit(block_id, *var);
                let entry = balances.entry(*var).or_insert(WholeVarBalance {
                    all_inc_elidable: true,
                    all_dec_unnecessary: true,
                    ..WholeVarBalance::default()
                });
                entry.dec_indices.push(idx);
                entry.all_dec_unnecessary &= is_rc_dec_unnecessary(&state);
            }
            ArcInstr::BurdenDecField { base, .. } => {
                if should_elide_dec(state_map, block_id, *base) {
                    remove[idx] = true;
                }
            }
            _ => {}
        }
    }

    // Pass 2: per-var paired elimination. Elide all of a var's Inc + whole-
    // var Dec ops iff DP-3 fires on every Inc AND DP-2 fires on every Dec.
    // Else retain every op for that var to preserve VF-1 balance.
    for balance in balances.values() {
        if balance.all_inc_elidable && balance.all_dec_unnecessary {
            for &i in &balance.inc_indices {
                remove[i] = true;
            }
            for &i in &balance.dec_indices {
                remove[i] = true;
            }
        }
    }

    if !remove.iter().any(|r| *r) {
        return;
    }

    // Compact: retain only non-removed instructions.
    let mut idx = 0usize;
    body.retain(|_| {
        let keep = !remove[idx];
        idx += 1;
        keep
    });
}

/// Whether a `BurdenDecField` site can be elided per DP-2 at `(block, var)`.
///
/// Field-grain Decs query DP-2 against the base's whole-var state, but
/// they do not participate in the whole-var Inc/Dec pairing — they
/// contribute to a separate field-grain accumulator per
/// `aims/verify/burden_delta.rs`. Elision is per-op, independent of
/// whole-var pairing.
#[inline]
fn should_elide_dec(state_map: &AimsStateMap, block_id: ArcBlockId, var: ArcVarId) -> bool {
    let state = state_map.var_state_at_block_exit(block_id, var);
    is_rc_dec_unnecessary(&state)
}
