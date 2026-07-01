//! Final burden-removal bitset application for `eliminate_burden_ops`.

use crate::ir::{ArcFunction, ArcInstr};

/// Force-drop EVERY `BurdenDec` / `BurdenDecPartial` / `BurdenDecField` /
/// `BurdenDecVariant` release — the deliberately-over-eliminating shape gated by
/// `ORI_FORCE_OVERELIMINATE=1`. Removal-only (the census guard still holds); a
/// dropped release leaks its allocation on the burden-sole path, tripping the
/// negative pin. Never reached with the flag unset.
pub(super) fn force_overeliminate_releases(func: &ArcFunction, remove: &mut [Vec<bool>]) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if matches!(
                instr,
                ArcInstr::BurdenDec { .. }
                    | ArcInstr::BurdenDecPartial { .. }
                    | ArcInstr::BurdenDecField { .. }
                    | ArcInstr::BurdenDecVariant { .. }
            ) {
                remove[block_idx][instr_idx] = true;
            }
        }
    }
}

/// Compact each block: retain only non-removed instructions.
pub(super) fn compact_removed(func: &mut ArcFunction, remove: &[Vec<bool>]) {
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        if !remove[block_idx].iter().any(|r| *r) {
            continue;
        }
        let mut idx = 0usize;
        block.body.retain(|_| {
            let keep = !remove[block_idx][idx];
            idx += 1;
            keep
        });
    }
}
