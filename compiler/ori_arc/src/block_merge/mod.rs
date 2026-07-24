//! Post-lowering ARC block merge pass.
//!
//! Eliminates redundant basic blocks created by the ARC lowerer's use of
//! `Invoke` terminators and trivial if/else diamond patterns. After all ARC
//! optimization passes run (RC insertion, edge cleanup, `expand_reuse`, RC
//! elimination), many `Invoke`s become trivial — their unwind blocks are
//! empty `Resume` stubs and their normal blocks are single-predecessor
//! continuations connected by unconditional branches. Similarly, simple
//! if/else expressions with trivial arm bodies can be folded into `Select`
//! instructions.
//!
//! # Seven-Phase Transform
//!
//! 1. **Compact** — remove blocks unreachable from the entry (dead unwind
//!    blocks, orphaned blocks from earlier passes).
//! 2. **Downgrade** — convert trivial `Invoke`s to `Apply` + `Jump` when
//!    the unwind block is an empty `Resume` and the normal block has a
//!    single predecessor with no params.
//! 3. **Select-fold** — convert trivial if/else diamond patterns into
//!    `Select` instructions. A diamond is `Branch → then/else → merge+phi`
//!    where both arm blocks have empty or trivial bodies (only `Let`
//!    bindings of literals or pre-branch variables). After folding, a
//!    compaction sub-step (3b) removes the dead arm blocks.
//! 4. **Merge** — collapse `Jump`-chain blocks where the target has a single
//!    predecessor, merging the target's body into the source.
//! 5. **Single-pred phi elimination** — clear block params on blocks with
//!    exactly one predecessor, converting Jump args to Let bindings or
//!    dropping dead params from non-Jump predecessors. Safety net for
//!    patterns that Phase 4's fixed-point didn't reach.
//! 6. **Dead param elimination** — remove block params whose defined
//!    variables are never used anywhere in the function. Targets
//!    multi-predecessor blocks (e.g., loop exit blocks with unused
//!    mutable variable params).
//! 7. **Invariant param elimination** — remove block params where all
//!    incoming values agree (after filtering self-references). Targets
//!    loop-invariant mutable bindings carried through loop headers
//!    without modification.
//!
//! # Pipeline Placement
//!
//! **Must run AFTER RC elimination but BEFORE [`compute_drop_hints`].**
//! Drop hints store `(block_idx, instr_idx)` coordinates that would become
//! invalid if blocks are renumbered after hint computation.
//!
//! [`compute_drop_hints`]: crate::uniqueness::compute_drop_hints

mod compact;
mod dead_param;
mod downgrade;
mod invariant_param;
mod merge;
mod select;
mod single_pred_phi;

#[cfg(test)]
mod tests;

use crate::ir::{ArcBlockId, ArcFunction};
use crate::uniqueness::DropHints;

/// Run the full block merge pass on a function.
///
/// Calls the seven phases in order: compact → downgrade → select-fold →
/// merge → single-pred-phi → dead-param → invariant-param.
///
/// # Precondition
///
/// Drop hints must not have been computed yet — they use `(block_idx,
/// instr_idx)` coordinates that merge invalidates. This function
/// defensively clears `func.drop_hints` at entry.
pub(crate) fn merge_blocks(func: &mut ArcFunction) {
    // Why: Block rewrites invalidate coordinate-based drop hints.
    func.drop_hints = DropHints::default();

    compact::compact_blocks(func);

    downgrade::downgrade_trivial_invokes(func);

    select::fold_select_diamonds(func);

    compact::compact_blocks(func);

    merge::merge_jump_chains(func);

    // Why: Non-Jump predecessors can leave redundant single-predecessor phis.
    single_pred_phi::eliminate_single_pred_params(func);

    dead_param::eliminate_dead_params(func);

    invariant_param::eliminate_invariant_params(func);

    // Structural compaction can remove dead PrimOp definitions. Their frozen
    // facts are keyed to those exact SSA destinations and retire with them;
    // surviving facts are never reclassified or rewritten here.
    crate::aims::primitive::retire_removed_primitive_facts(func);
}

/// Convert a `usize` block index to an `ArcBlockId`.
///
/// # Panics
///
/// Panics if `idx` exceeds `u32::MAX`.
pub(crate) fn usize_to_block_id(idx: usize) -> ArcBlockId {
    let raw = u32::try_from(idx).unwrap_or_else(|_| panic!("block index {idx} exceeds u32::MAX"));
    ArcBlockId::new(raw)
}
