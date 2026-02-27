//! Callee extraction from ARC IR functions.
//!
//! Provides [`extract_callees`] for building call graph edges from a
//! single function's instructions.

use rustc_hash::FxHashSet;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator};

/// Extract callee [`Name`]s from a function's instructions.
///
/// Scans `Apply`, `PartialApply`, and `Invoke` instructions for direct callees.
/// Indirect calls (`ApplyIndirect`) are excluded — their callees are unknown.
///
/// This duplicates `CallGraph::build`'s per-function logic as a standalone
/// helper, avoiding the need to reconstruct the full call graph in per-SCC
/// queries.
pub fn extract_callees(func: &ArcFunction) -> FxHashSet<Name> {
    let mut callees = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply { func: callee, .. }
                | ArcInstr::PartialApply { func: callee, .. } => {
                    callees.insert(*callee);
                }
                _ => {}
            }
        }
        if let ArcTerminator::Invoke { func: callee, .. } = &block.terminator {
            callees.insert(*callee);
        }
    }
    callees
}
