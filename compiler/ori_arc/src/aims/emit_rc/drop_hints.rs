//! AIMS-based drop hint computation.
//!
//! Derives [`DropHints`] from the converged [`AimsStateMap`] by walking
//! the final IR (post-merge) and identifying `RcDec` instructions where
//! the variable is provably unique. This replaces the separate construct/
//! increment scanning in the old pipeline.
//!
//! # Post-RC-emission correctness
//!
//! The AIMS uniqueness state is computed BEFORE RcInc/RcDec emission
//! (pipeline step 5), but drop hints are computed AFTER (step 12). An
//! `RcInc` on variable `%v` means the underlying object's refcount was
//! bumped — any alias of `%v` (via `Let { dst, Var(v) }`) shares that
//! object and is therefore NOT physically unique, even if the pre-emission
//! lattice says `Unique`. We track `RcInc`'d variables and their alias
//! chains to exclude them from the fast unique-drop path.
//!
//! # Keying
//!
//! Drop hints use the same `(block_idx, instr_idx)` keying as COW
//! annotations — positions in the FINAL instruction layout.

use rustc_hash::FxHashSet;

use ori_types::{Pool, Tag};

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::Uniqueness;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::uniqueness::DropHints;

/// Compute drop hints from the AIMS state map.
///
/// Walks the final IR (post-merge). For each `RcDec` of a collection
/// variable, checks the receiver's uniqueness in the state map. If the
/// variable is provably unique AND no `RcInc` was emitted for it or any
/// alias sharing the same underlying object, marks the drop for the
/// fast path.
///
/// Must be called AFTER RC emission (pipeline step 6) and block merge
/// (pipeline step 11).
pub fn compute_aims_drop_hints(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
) -> DropHints {
    let mut hints = DropHints::new();

    // Build the set of variables whose underlying object had RcInc.
    // A variable is "rc-incremented" if:
    //   1. It directly has an RcInc instruction, OR
    //   2. It is defined as `Let { dst, Var(src) }` where `src` is
    //      rc-incremented (transitive alias chain).
    let rc_incremented = super::collect_rc_incremented_vars(func);

    // Build the set of variables passed as Borrowed args to function calls.
    // Runtime functions like `ori_list_slice` may internally call `ori_rc_inc`
    // on the underlying buffer, making the physical refcount > 1. Since these
    // runtime-internal RC ops are invisible to AIMS analysis, we conservatively
    // exclude all borrowed call args from unique drops.
    let borrowed_call_args = collect_borrowed_call_args(func);

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::RcDec { var, .. } = instr {
                if state_map.is_excluded(*var) {
                    continue;
                }

                // Check the type is a collection with a buffer.
                if !is_collection_var(func, *var, pool) {
                    continue;
                }

                // If this variable shares an object with an RcInc'd variable,
                // the physical refcount is > 1 — not safe for unique drop.
                if rc_incremented.contains(var) {
                    continue;
                }

                // If this variable was passed as Borrowed to a function call,
                // the callee may have internally incremented the refcount
                // (e.g., ori_list_slice calls ori_rc_inc on the buffer).
                // Not safe for unique drop.
                if borrowed_call_args.contains(var) {
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
fn is_collection_var(func: &ArcFunction, var: ArcVarId, pool: &Pool) -> bool {
    let Some(&idx) = func.var_types.get(var.index()) else {
        return false;
    };
    let resolved = pool.resolve_fully(idx);
    let tag = pool.tag(resolved);
    matches!(tag, Tag::List | Tag::Set | Tag::Map)
}

/// Collect variables passed as `Borrowed` arguments to Apply/Invoke calls.
///
/// Runtime functions may internally call `ori_rc_inc` on borrowed arguments
/// (e.g., `ori_list_slice` increments the original buffer's refcount to keep
/// the slice alive). These runtime-internal RC ops are invisible to AIMS
/// analysis, so we conservatively exclude all borrowed call args from the
/// unique-drop fast path.
///
/// Also propagates through Let alias chains: if `%x` is borrowed in a call
/// and `%y = %x`, then `%y` is also excluded.
fn collect_borrowed_call_args(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    let mut alias_edges: Vec<Option<ArcVarId>> = vec![None; func.var_types.len()];

    for block in &func.blocks {
        // Scan body instructions for Apply with Borrowed args.
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    args,
                    arg_ownership,
                    ..
                } => {
                    for (i, &arg) in args.iter().enumerate() {
                        if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                            borrowed.insert(arg);
                        }
                    }
                }
                ArcInstr::Let {
                    dst,
                    value: crate::ir::ArcValue::Var(src),
                    ..
                } => {
                    alias_edges[dst.index()] = Some(*src);
                }
                _ => {}
            }
        }

        // Scan terminator for Invoke with Borrowed args.
        if let ArcTerminator::Invoke {
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            for (i, &arg) in args.iter().enumerate() {
                if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                    borrowed.insert(arg);
                }
            }
        }
    }

    // Propagate through alias chains: if src is borrowed, dst shares the
    // same object and should also be excluded from unique drops.
    loop {
        let mut changed = false;
        for (i, alias) in alias_edges.iter().enumerate() {
            if let Some(src) = alias {
                if borrowed.contains(src) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "ARC IR var counts fit in u32"
                    )]
                    let dst = ArcVarId::new(i as u32);
                    if borrowed.insert(dst) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    borrowed
}
