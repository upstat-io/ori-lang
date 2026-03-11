//! AIMS-based drop hint computation.
//!
//! Derives [`DropHints`] from the converged [`AimsStateMap`] by walking
//! the final IR (post-merge) and identifying `RcDec` instructions where
//! the variable is provably unique. This replaces the separate construct/
//! increment scanning in the old pipeline.
//!
//! # Keying
//!
//! Drop hints use the same `(block_idx, instr_idx)` keying as COW
//! annotations — positions in the FINAL instruction layout.

use ori_types::{Pool, Tag};

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::Uniqueness;
use crate::ir::{ArcFunction, ArcInstr};
use crate::uniqueness::DropHints;

/// Compute drop hints from the AIMS state map.
///
/// Walks the final IR (post-merge). For each `RcDec` of a collection
/// variable, checks the receiver's uniqueness in the state map. If the
/// variable is provably unique, marks the drop for the fast path.
///
/// Must be called AFTER block merge (pipeline step 11a in Section 06.2).
pub fn compute_aims_drop_hints(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
) -> DropHints {
    let mut hints = DropHints::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::RcDec { var, .. } = instr {
                if state_map.is_scalar(*var) {
                    continue;
                }

                // Check the type is a collection with a buffer.
                if !is_collection_var(func, *var, pool) {
                    continue;
                }

                // Use AIMS uniqueness state to determine if unique.
                let blk = super::block_id(block_idx);
                let state = state_map.var_state_at_block_entry(blk, *var);
                if state.uniqueness == Uniqueness::Unique {
                    hints.mark_unique(block_idx, instr_idx);
                }
            }
        }
    }

    if !hints.is_empty() {
        tracing::debug!(
            function = func.name.raw(),
            unique_drops = hints.len(),
            "AIMS drop hint analysis complete"
        );
    }

    hints
}

/// Check if a variable's type is a collection (List, Set, Map).
fn is_collection_var(func: &ArcFunction, var: crate::ir::ArcVarId, pool: &Pool) -> bool {
    let Some(&idx) = func.var_types.get(var.index()) else {
        return false;
    };
    let resolved = pool.resolve_fully(idx);
    let tag = pool.tag(resolved);
    matches!(tag, Tag::List | Tag::Set | Tag::Map)
}
