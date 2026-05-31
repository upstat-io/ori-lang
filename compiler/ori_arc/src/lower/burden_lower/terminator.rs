//! Terminator-position transfer + `BurdenInc` precomputation for the Phase 5
//! burden walk. Computed against the immutable `func.blocks` borrow so the
//! mutable emission pass can consume per-block results without aliasing
//! conflict (target-block lookup would otherwise collide with `iter_mut()`).

use crate::ir::ArcTerminator;
use crate::ir::{ArcBlock, ArcFunction, ArcVarId};
use crate::ownership::DerivedOwnership;
use rustc_hash::FxHashSet;

/// Terminator-transfer-var pre-computation. Computed against the IMMUTABLE
/// `func.blocks` borrow so subsequent mutable iteration can consume per-block
/// transfer sets without aliasing conflict (target-block lookup
/// `func.blocks[target.index()]` would otherwise collide with `iter_mut()`).
///
/// Per AIMS RL-2 ownership-transferring exception:
/// - `Return.value` transfers to caller.
/// - `Jump.args` at positions whose target-block params carry
///   `DerivedOwnership::Owned` transfer to the target block param.
/// - `Invoke`/`InvokeIndirect` arg-positions whose `arg_ownership[pos] ==
///   Owned` transfer ownership to the callee. The canonical helper
///   `ArcTerminator::is_owned_position(pos)` encodes empty-arg_ownership
///   defaults + closure-pos-0 Borrowed semantics in one place.
///
/// Empty `derived_ownership` or out-of-bounds index defaults to `Owned`. The
/// Jump-Borrowed case is structurally vacuous under that default.
pub(super) fn compute_terminator_transfer_per_block(
    func: &ArcFunction,
    derived_ownership: &[DerivedOwnership],
) -> Vec<FxHashSet<ArcVarId>> {
    func.blocks
        .iter()
        .map(|block| terminator_transfer_vars(block, &func.blocks, derived_ownership))
        .collect()
}

/// Build the transfer-var set for a single block's terminator. Extracted from
/// `compute_terminator_transfer_per_block` to keep cognitive complexity per
/// function under workspace limits.
fn terminator_transfer_vars(
    block: &ArcBlock,
    all_blocks: &[ArcBlock],
    derived_ownership: &[DerivedOwnership],
) -> FxHashSet<ArcVarId> {
    let mut transfers: FxHashSet<ArcVarId> = FxHashSet::default();
    match &block.terminator {
        ArcTerminator::Return { value } => {
            transfers.insert(*value);
        }
        ArcTerminator::Jump { target, args } => {
            let Some(target_block) = all_blocks.get(target.index()) else {
                return transfers;
            };
            for (i, &arg) in args.iter().enumerate() {
                let Some(&(block_param_var, _)) = target_block.params.get(i) else {
                    continue;
                };
                let ownership = derived_ownership
                    .get(block_param_var.index())
                    .copied()
                    .unwrap_or(DerivedOwnership::Owned);
                if matches!(ownership, DerivedOwnership::Owned) {
                    transfers.insert(arg);
                }
            }
        }
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
            for (pos, &var) in block.terminator.used_vars().iter().enumerate() {
                if block.terminator.is_owned_position(pos) {
                    transfers.insert(var);
                }
            }
        }
        _ => {}
    }
    transfers
}

/// Terminator-position `BurdenInc` pre-computation. Per RL-1 (RC inc emitted at
/// every ownership-transfer point on owned non-scalar SSA values), each
/// terminator-position Owned-arg gets a `BurdenInc` emitted before the
/// terminator. Mirrors `emit_instr_burdens` instruction-level behavior which
/// emits `BurdenInc` unconditionally at every `is_owned_position(pos)` position;
/// conservative Phase 5 emission — RC traffic is overcounted but balanced; the
/// lattice rewrite pass eliminates redundant Incs.
///
/// Ordered `Vec<Vec<ArcVarId>>` (NOT `FxHashSet` like the transfer-set), so
/// multi-position-same-var terminators emit one `BurdenInc` per occurrence
/// (e.g., Jump block1, args=[%0, %0] to 2 Owned params emits 2× `BurdenInc`).
///
/// Computed against the IMMUTABLE `func.blocks` borrow so subsequent mutable
/// iteration in `emit_burden_ops_for_blocks` can consume per-block Inc lists
/// without aliasing conflict. AIMS Invariant 5 preserved — `DerivedOwnership` is
/// existing analysis output, not a parallel ownership tracker.
pub(super) fn compute_terminator_inc_per_block(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    derived_ownership: &[DerivedOwnership],
) -> Vec<Vec<ArcVarId>> {
    func.blocks
        .iter()
        .map(|block| {
            terminator_inc_vars(
                block,
                &func.blocks,
                owned_vars_needing_rc,
                derived_ownership,
            )
        })
        .collect()
}

/// Build the ordered `BurdenInc` list for a single block's terminator. Extracted
/// from `compute_terminator_inc_per_block` to mirror `terminator_transfer_vars`
/// extraction and keep cognitive complexity per function under workspace
/// limits.
///
/// Jump-to-Owned-param: per-position Owned check against `target_block.params[i]`'s
/// `DerivedOwnership`. Empty `derived_ownership` or out-of-bounds defaults to
/// Owned, preserving `terminator_transfer_vars` semantics.
///
/// Invoke / `InvokeIndirect`: per-position check against canonical SSOT helper
/// `ArcTerminator::is_owned_position(pos)`, which encodes empty `arg_ownership`
/// defaults + `InvokeIndirect` closure-pos-0 Borrowed semantics in one place.
///
/// `owned_vars_needing_rc` filter rejects EMPTY-spec scalars per VF-1 `RcOnScalar`.
fn terminator_inc_vars(
    block: &ArcBlock,
    all_blocks: &[ArcBlock],
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    derived_ownership: &[DerivedOwnership],
) -> Vec<ArcVarId> {
    let mut incs: Vec<ArcVarId> = Vec::new();
    match &block.terminator {
        ArcTerminator::Jump { target, args } => {
            let Some(target_block) = all_blocks.get(target.index()) else {
                return incs;
            };
            for (i, &arg) in args.iter().enumerate() {
                if !owned_vars_needing_rc.contains(&arg) {
                    continue;
                }
                let Some(&(block_param_var, _)) = target_block.params.get(i) else {
                    continue;
                };
                let ownership = derived_ownership
                    .get(block_param_var.index())
                    .copied()
                    .unwrap_or(DerivedOwnership::Owned);
                if matches!(ownership, DerivedOwnership::Owned) {
                    incs.push(arg);
                }
            }
        }
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
            for (pos, &var) in block.terminator.used_vars().iter().enumerate() {
                if owned_vars_needing_rc.contains(&var) && block.terminator.is_owned_position(pos) {
                    incs.push(var);
                }
            }
        }
        _ => {}
    }
    incs
}
