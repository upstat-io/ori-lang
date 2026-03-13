//! Post-emission query passes: RC-incremented variable tracking.
//!
//! Produces auxiliary data used by downstream pipeline steps (COW annotations,
//! drop hints).

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

/// Collect all variables whose underlying object had an `RcInc` emitted.
///
/// Returns a set containing:
/// - Every variable `v` that has an `RcInc { var: v }` instruction
/// - Every variable `d` defined as `Let { dst: d, value: Var(s) }` where
///   `s` is in the set (transitive through alias chains)
///
/// Used by both COW annotations and drop hints: after RC emission, any
/// variable in this set shares a heap object whose physical refcount was
/// bumped above 1. The pre-emission AIMS uniqueness state (`Unique`) no
/// longer reflects the physical reality for these variables.
pub(crate) fn collect_rc_incremented_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut incremented = FxHashSet::default();
    let mut alias_edges: Vec<Option<ArcVarId>> = vec![None; func.var_types.len()];

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { var, .. } => {
                    incremented.insert(*var);
                }
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    alias_edges[dst.index()] = Some(*src);
                }
                _ => {}
            }
        }
    }

    // Propagate: for each alias edge dst → src, if src is incremented,
    // dst is also incremented (shares the same object).
    // Fixed-point loop handles chains like %11 = %8 = %6 (where %6 has RcInc).
    loop {
        let mut changed = false;
        for (i, alias) in alias_edges.iter().enumerate() {
            if let Some(src) = alias {
                if incremented.contains(src) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "ARC IR var counts fit in u32"
                    )]
                    let dst = ArcVarId::new(i as u32);
                    if incremented.insert(dst) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    incremented
}
