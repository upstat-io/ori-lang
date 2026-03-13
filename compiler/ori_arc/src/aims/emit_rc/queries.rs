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
/// - Every block parameter that receives an incremented variable through
///   a `Jump { target, args }` terminator (phi-edge propagation)
///
/// Used by both COW annotations and drop hints: after RC emission, any
/// variable in this set shares a heap object whose physical refcount was
/// bumped above 1. The pre-emission AIMS uniqueness state (`Unique`) no
/// longer reflects the physical reality for these variables.
pub(crate) fn collect_rc_incremented_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    use crate::ir::ArcTerminator;

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

        // Phi-edge propagation: Jump args map to target block params.
        // A block param may receive values from multiple predecessors
        // (e.g., loop header from entry + back-edge). If ANY source is
        // incremented, the param inherits the flag. We use alias_edges
        // for single-source cases and direct insertion for multi-source.
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_idx = target.index();
            if target_idx < func.blocks.len() {
                for (i, &arg) in args.iter().enumerate() {
                    if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                        if let Some(existing) = alias_edges[param_var.index()] {
                            // Multi-predecessor: param already has an alias
                            // edge. If the new arg OR the existing source is
                            // incremented, mark the param directly. This
                            // handles loop headers where one predecessor
                            // passes an incremented var and the back-edge
                            // passes the param itself.
                            if incremented.contains(&arg) || incremented.contains(&existing) {
                                incremented.insert(param_var);
                            }
                        } else {
                            alias_edges[param_var.index()] = Some(arg);
                        }
                    }
                }
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
