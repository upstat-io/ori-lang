//! DP-2/DP-3 burden-op elimination consumer.
//!
//! Per §04A.2 of `plans/aims-burden-tracking/section-04A-minimal-lattice-adaptation.md`.
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
//! Per `aims-rules.md §4 Appendix C`:
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
//! Per ITEM-4 of `plans/aims-burden-tracking/section-04A-minimal-lattice-
//! adaptation.md §04A.5`: "When DP-2 elides a `BurdenDec`, the
//! corresponding `BurdenInc` (if elidable per DP-3) MUST also be elided in
//! the same pass. If DP-3 doesn't fire on the matching Inc, retain the
//! dec to preserve VF-1 balance."
//!
//! # References
//!
//! - `aims-rules.md §4 DP-2 + DP-3` — predicate truth tables.
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
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);
        eliminate_in_block(&mut block.body, state_map, block_id);
    }
}

/// Eliminate redundant burden ops within one block's body.
///
/// Two-pass paired elimination per `aims-rules.md §9 VF-1` intraprocedural
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
