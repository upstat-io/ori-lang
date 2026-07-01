//! Per-kind burden-op census and the removal-only structural guard
//! consulted by `eliminate_burden_ops`.

use crate::ir::{ArcFunction, ArcInstr};

/// Per-kind burden-op census across all blocks of a function.
///
/// Five counts, one per burden-op variant, in the order `[BurdenInc,
/// BurdenDec, BurdenDecPartial, BurdenDecField, BurdenDecVariant]`. Used by the
/// removal-only structural guard in `eliminate_burden_ops`.
pub(super) fn burden_op_census(func: &ArcFunction) -> [usize; 5] {
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
/// structural guard; both the always-on assert and the negative pin consult it.
pub(super) fn is_burden_removal_only(before: &[usize; 5], after: &[usize; 5]) -> bool {
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
/// in Phase 7 and corrupt RC balance. Uses `assert!` (never `debug_assert!`):
/// a silently-corrupted RC balance in a release binary is a double-free or
/// leak, not merely a debug-build diagnostic.
#[track_caller]
pub(super) fn assert_burden_removal_only(before: &[usize; 5], after: &[usize; 5]) {
    const KIND_NAMES: [&str; 5] = [
        "BurdenInc",
        "BurdenDec",
        "BurdenDecPartial",
        "BurdenDecField",
        "BurdenDecVariant",
    ];
    assert!(
        is_burden_removal_only(before, after),
        "AIMS Phase-6 invariant: eliminate_burden_ops constructed burden ops \
         where only elimination is permitted — Phase 6 MUST eliminate burden \
         ops, never construct them. \
         per-kind census {KIND_NAMES:?}: before = {before:?}, after = {after:?}",
    );
}
