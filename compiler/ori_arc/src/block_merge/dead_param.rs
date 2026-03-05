//! Phase 6: Dead block param elimination.
//!
//! After Phase 5 handles single-predecessor blocks, multi-predecessor
//! blocks may still have params whose defined variables are never used.
//! This arises from loop exit blocks that receive mutable variable values
//! via `break` jumps, where those variables are unused after the loop.
//!
//! For each block B with params:
//! 1. Check which param variables are used (read) anywhere in the function.
//! 2. Remove dead params (defined variable never appears as an operand).
//! 3. Remove the corresponding args from all predecessor `Jump`s.
//!
//! This is a global pass — it computes the used-var set once and applies
//! it to all blocks. No topology changes (no block merging or removal),
//! so predecessor relationships remain valid.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

/// Eliminate block params whose defined variables are never used.
///
/// A param `(var, ty)` on block B is dead if `var` never appears as a
/// read operand in any instruction body or terminator across the entire
/// function. Dead params produce dead LLVM phi nodes.
#[tracing::instrument(skip_all, name = "phase6_dead_param")]
pub(crate) fn eliminate_dead_params(func: &mut ArcFunction) {
    let used = collect_used_vars(func);

    for b_idx in 0..func.blocks.len() {
        if func.blocks[b_idx].params.is_empty() {
            continue;
        }

        // Identify dead param indices (defined var not in used set).
        let dead_indices: Vec<usize> = func.blocks[b_idx]
            .params
            .iter()
            .enumerate()
            .filter(|(_, (var, _))| !used.contains(var))
            .map(|(i, _)| i)
            .collect();

        if dead_indices.is_empty() {
            continue;
        }

        tracing::debug!(
            block = b_idx,
            dead_count = dead_indices.len(),
            total_params = func.blocks[b_idx].params.len(),
            "eliminating dead block params"
        );

        // Remove dead params (reverse order to preserve indices).
        for &i in dead_indices.iter().rev() {
            func.blocks[b_idx].params.remove(i);
        }

        // Remove corresponding args from all predecessor Jumps.
        let b_id = func.blocks[b_idx].id;
        for a_idx in 0..func.blocks.len() {
            if let ArcTerminator::Jump { target, args } = &mut func.blocks[a_idx].terminator {
                if *target == b_id && !args.is_empty() {
                    for &i in dead_indices.iter().rev() {
                        if i < args.len() {
                            args.remove(i);
                        }
                    }
                }
            }
        }
    }
}

/// Collect all variables that are read (used as operands) anywhere.
///
/// Includes operands from instruction bodies and terminators across
/// all blocks. Does NOT include variables that are only defined (dst
/// fields, block params) — only reads.
fn collect_used_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut used = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            for var in instr.used_vars() {
                used.insert(var);
            }
        }
        for var in block.terminator.used_vars() {
            used.insert(var);
        }
    }
    used
}
