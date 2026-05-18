//! Per-block burden-op net delta — shared by TRMC-scope
//! `verify_trmc_burden_balance` (in `aims/normalize/verify.rs`) and
//! full-function-scope `verify_burden_balance` (in
//! `aims/verify/burden_balance.rs`).
//!
//! Per `impl-hygiene.md §LEAK:algorithmic-duplication`, the same
//! `BurdenInc(v) - BurdenDec*(v)` walk over a block's body would otherwise
//! exist in two sites with identical control-flow shape; the helper here
//! is the canonical home.

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

/// Per-block burden delta for `var`: `Σ BurdenInc(var) - Σ BurdenDec*(var)`.
///
/// `BurdenDecPartial` and `BurdenDecVariant` mirror whole-var burden
/// tracking (like `BurdenDec`) and contribute `-1`. `BurdenDecField`
/// targets a field of the base — not the whole-var balance — so it is
/// excluded.
///
/// Returned vector is indexed by block index (`func.blocks[i]`), length
/// `func.blocks.len()`.
pub(crate) fn compute_var_block_deltas(func: &ArcFunction, var: ArcVarId) -> Vec<i64> {
    let mut delta: Vec<i64> = vec![0; func.blocks.len()];
    for (idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var: v } if *v == var => delta[idx] += 1,
                ArcInstr::BurdenDec { var: v } if *v == var => delta[idx] -= 1,
                ArcInstr::BurdenDecPartial { var: v, .. } if *v == var => delta[idx] -= 1,
                ArcInstr::BurdenDecVariant { var: v } if *v == var => delta[idx] -= 1,
                _ => {}
            }
        }
    }
    delta
}
