//! Transfer functions and dead variable tracking for uniqueness analysis.
//!
//! Contains the per-instruction transfer function that updates the uniqueness
//! map, the block-level transfer that iterates over instructions, and helpers
//! for dead variable tracking (last-use precomputation, `is_last_use`, `needs_rc`).

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};
use crate::ArcClassification;

use super::{LastUseMap, Uniqueness, UniquenessMap, UniquenessSummary};

// -- Transfer function --

/// Apply the transfer function to all instructions in a block.
pub(super) fn transfer_block(
    block_idx: usize,
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    block_in: &UniquenessMap,
    last_use: &LastUseMap,
    summaries: &FxHashMap<Name, UniquenessSummary>,
) -> UniquenessMap {
    let block = &func.blocks[block_idx];
    let mut state = block_in.clone();
    for (pos, instr) in block.body.iter().enumerate() {
        transfer_instr(
            &mut state, instr, pos, func, classifier, last_use, summaries,
        );
    }
    state
}

/// Transfer function for a single instruction.
///
/// Updates the uniqueness map based on the instruction's semantics:
/// fresh allocations → `Unique`, aliases with dead source → move,
/// aliases with live source → both `Shared`, calls → callee summary
/// or `MaybeShared`.
pub(crate) fn transfer_instr(
    state: &mut UniquenessMap,
    instr: &ArcInstr,
    pos: usize,
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    last_use: &LastUseMap,
    summaries: &FxHashMap<Name, UniquenessSummary>,
) {
    match instr {
        // Variable alias: move or sharing depending on source liveness.
        ArcInstr::Let {
            dst,
            value: ArcValue::Var(src),
            ..
        } => {
            if needs_rc(*dst, func, classifier) {
                if is_last_use(*src, pos, last_use) {
                    // Source is dead after this point → move semantics.
                    state.set(*dst, state.get(*src));
                } else {
                    // Source is still live → two references → both Shared.
                    state.mark_shared(*src);
                    state.mark_shared(*dst);
                }
            }
        }

        // Fresh allocations and fresh values: RC = 1 at birth.
        // Construct/PartialApply create new heap objects, literals and prim ops
        // produce fresh values, and Reuse recycles an allocation (still RC = 1).
        ArcInstr::Construct { dst, .. }
        | ArcInstr::PartialApply { dst, .. }
        | ArcInstr::Let {
            dst,
            value: ArcValue::Literal(_),
            ..
        }
        | ArcInstr::Let {
            dst,
            value: ArcValue::PrimOp { .. },
            ..
        }
        | ArcInstr::Reuse { dst, .. }
        | ArcInstr::CollectionReuse { dst, .. } => {
            if needs_rc(*dst, func, classifier) {
                state.mark_unique(*dst);
            }
        }

        // Direct function call: use callee summary if available.
        // COW operations always return Unique; user functions may vary.
        ArcInstr::Apply {
            dst, func: callee, ..
        } => {
            if needs_rc(*dst, func, classifier) {
                let return_uniq = summaries
                    .get(callee)
                    .map_or(Uniqueness::MaybeShared, |s| s.return_val);
                state.set(*dst, return_uniq);
            }
        }

        // Indirect calls, projections, and IsShared: conservative MaybeShared.
        // Indirect calls have unknown callees. Projections borrow from parent.
        // IsShared produces a scalar (no-op via needs_rc check).
        ArcInstr::ApplyIndirect { dst, .. }
        | ArcInstr::Project { dst, .. }
        | ArcInstr::IsShared { dst, .. } => {
            if needs_rc(*dst, func, classifier) {
                state.set(*dst, Uniqueness::MaybeShared);
            }
        }

        // Select: join of both branch values.
        ArcInstr::Select {
            dst,
            true_val,
            false_val,
            ..
        } => {
            if needs_rc(*dst, func, classifier) {
                let joined = state.get(*true_val).join(state.get(*false_val));
                state.set(*dst, joined);
            }
        }

        // RC operations and mutations (not expected pre-insertion; handle defensively).
        ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. }
        | ArcInstr::Reset { .. } => {}
    }
}

// -- Dead variable tracking --

/// Precompute the last-use position for each variable in a block.
///
/// `usize::MAX` means the variable is used in the terminator or `live_out`.
/// Otherwise, the value is the index of the last body instruction that uses it.
pub(crate) fn precompute_last_use(
    block: &crate::ir::ArcBlock,
    live_out: &crate::liveness::LiveSet,
) -> LastUseMap {
    let mut last_use = LastUseMap::default();

    // Variables in live_out: used after the block.
    for &var in live_out {
        last_use.insert(var, usize::MAX);
    }

    // Terminator uses: also "after" the block body.
    for var in block.terminator.used_vars() {
        last_use.insert(var, usize::MAX);
    }

    // Walk body backward: first insertion per variable is its last use.
    for (i, instr) in block.body.iter().enumerate().rev() {
        for var in instr.used_vars() {
            last_use.entry(var).or_insert(i);
        }
    }

    last_use
}

/// Check whether the current instruction at `pos` is the last use of `var`.
///
/// Returns `true` if `var` is dead after instruction `pos`, meaning an alias
/// at this point can be treated as a move (ownership transfer).
#[inline]
fn is_last_use(var: ArcVarId, pos: usize, last_use: &LastUseMap) -> bool {
    match last_use.get(&var) {
        None => true,
        Some(&last_pos) => last_pos <= pos,
    }
}

/// Check whether a variable's type requires reference counting.
#[inline]
pub(crate) fn needs_rc(
    var: ArcVarId,
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
) -> bool {
    let idx = var.index();
    if idx < func.var_types.len() {
        classifier.needs_rc(func.var_types[idx])
    } else {
        true
    }
}
