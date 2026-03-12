//! COW annotation computation from AIMS state map.
//!
//! Derives [`CowAnnotations`] by walking the final (post-merge) IR and
//! looking up each COW operation's receiver uniqueness from the converged
//! [`AimsStateMap`]. This replaces the separate uniqueness analysis pass.
//!
//! # Keying
//!
//! COW annotations are keyed by `(block_idx, instr_idx)` in the FINAL
//! instruction layout (after RC insertion, reuse, and block merge). For
//! `Invoke` terminators, `instr_idx == block.body.len()`.

use rustc_hash::FxHashSet;

use ori_ir::{Name, StringInterner};

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{
    AccessClass, AimsState, BorrowSource, Cardinality, Consumption, Uniqueness,
};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::uniqueness::CowAnnotations;
use crate::CowMode;

/// Compute COW annotations from the AIMS state map.
///
/// Walks the final IR (post-merge). For each `Apply`/`Invoke` calling a
/// COW method, looks up the receiver variable's uniqueness in the state map
/// and derives [`CowMode`].
///
/// # Post-RC-emission correctness
///
/// The AIMS uniqueness state is computed BEFORE RC emission (step 5), but
/// COW annotations are computed AFTER (step 11a). An `RcInc` on variable
/// `%v` means the underlying object's refcount was bumped — the physical
/// refcount may exceed 1 even though the AIMS lattice says `Unique`.
/// We track `RcInc`'d variables and their alias chains to force `Dynamic`
/// mode for these receivers (runtime uniqueness check required).
///
/// Must be called AFTER block merge (pipeline step 11a in Section 06.2).
pub fn compute_aims_cow_annotations(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    interner: &StringInterner,
) -> CowAnnotations {
    let cow_names = crate::borrow::all_cow_method_names(interner);
    let param_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let rc_incremented = super::collect_rc_incremented_vars(func);
    let mut annotations = CowAnnotations::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        annotate_block_body(
            func,
            block_idx,
            block,
            state_map,
            &cow_names,
            &param_vars,
            &rc_incremented,
            &mut annotations,
        );
        annotate_block_terminator(
            block_idx,
            block,
            state_map,
            &cow_names,
            &param_vars,
            &rc_incremented,
            &mut annotations,
        );
    }

    annotations
}

/// Annotate COW operations in a block's body instructions.
#[expect(
    clippy::too_many_arguments,
    reason = "block-level annotation needs full context"
)]
fn annotate_block_body(
    _func: &ArcFunction,
    block_idx: usize,
    block: &crate::ir::ArcBlock,
    state_map: &AimsStateMap,
    cow_names: &FxHashSet<Name>,
    param_vars: &FxHashSet<ArcVarId>,
    rc_incremented: &FxHashSet<ArcVarId>,
    annotations: &mut CowAnnotations,
) {
    for (instr_idx, instr) in block.body.iter().enumerate() {
        if let ArcInstr::Apply {
            func: callee, args, ..
        } = instr
        {
            if cow_names.contains(callee) && !args.is_empty() {
                let receiver = args[0];
                let mode = uniqueness_to_cow_mode(
                    state_map,
                    block_idx,
                    receiver,
                    param_vars,
                    rc_incremented,
                );
                tracing::debug!(
                    block_idx,
                    instr_idx,
                    receiver = receiver.raw(),
                    ?mode,
                    rc_inc = rc_incremented.contains(&receiver),
                    "COW annotation for Apply"
                );
                annotations.set(block_idx, instr_idx, mode);
            }
        }
    }
}

/// Annotate COW operations in a block's terminator (Invoke).
fn annotate_block_terminator(
    block_idx: usize,
    block: &crate::ir::ArcBlock,
    state_map: &AimsStateMap,
    cow_names: &FxHashSet<Name>,
    param_vars: &FxHashSet<ArcVarId>,
    rc_incremented: &FxHashSet<ArcVarId>,
    annotations: &mut CowAnnotations,
) {
    if let ArcTerminator::Invoke {
        func: callee, args, ..
    } = &block.terminator
    {
        if cow_names.contains(callee) && !args.is_empty() {
            let receiver = args[0];
            let mode =
                uniqueness_to_cow_mode(state_map, block_idx, receiver, param_vars, rc_incremented);
            // Invoke uses body.len() as instr_idx (one past last body instruction).
            annotations.set(block_idx, block.body.len(), mode);
        }
    }
}

/// Derive [`CowMode`] from a variable's state in the state map.
///
/// Uses the block entry state for the receiver. If the variable is scalar
/// or has no state entry, defaults to `Dynamic` (safe fallback).
///
/// # Post-RC-emission guard
///
/// If the receiver was `RcInc`'d (or is an alias of an `RcInc`'d variable),
/// the physical refcount exceeds 1 regardless of what the AIMS lattice says.
/// Forces `Dynamic` to ensure the runtime performs a uniqueness check.
///
/// # COW-aware borrowing (Section 07.3.1)
///
/// For function parameters, the combined state `(Owned, Linear, Once)` proves
/// the callee received ownership, never duplicated the reference, and uses it
/// exactly once. This allows `StaticUnique` even when the `uniqueness` dimension
/// alone is `MaybeShared` — the three non-uniqueness dimensions prove that the
/// callee's view of the value is uniquely owned at the COW mutation point.
fn uniqueness_to_cow_mode(
    state_map: &AimsStateMap,
    block_idx: usize,
    receiver: ArcVarId,
    param_vars: &FxHashSet<ArcVarId>,
    rc_incremented: &FxHashSet<ArcVarId>,
) -> CowMode {
    if state_map.is_excluded(receiver) {
        return CowMode::Dynamic;
    }

    // Post-RC-emission guard: if an RcInc was emitted for this variable
    // (or any alias sharing the same heap object), the physical refcount
    // is > 1 — StaticUnique would skip the runtime check and corrupt data.
    if rc_incremented.contains(&receiver) {
        return CowMode::Dynamic;
    }

    let blk = super::block_id(block_idx);
    let state = state_map.var_state_at_block_entry(blk, receiver);

    // COW-aware borrowing: parameter with (Owned, Linear, Once) combined
    // state is provably uniquely owned at this point. The callee received
    // ownership and hasn't duplicated the reference — safe for in-place
    // mutation regardless of what the uniqueness dimension reports.
    if param_vars.contains(&receiver) && is_cow_aware_unique(&state) {
        return CowMode::StaticUnique;
    }

    match state.uniqueness {
        Uniqueness::Unique => CowMode::StaticUnique,
        Uniqueness::MaybeShared => {
            // Uniqueness-preserving borrows (Section 07.3.2): if the receiver
            // is a field borrow from a uniquely-owned source, check whether all
            // sibling borrows target disjoint fields. If so, the mutation is
            // safe — the borrowed field isn't aliased by any other borrow.
            if is_borrow_disjoint_from_siblings(state_map, receiver) {
                return CowMode::StaticUnique;
            }
            CowMode::Dynamic
        }
        Uniqueness::Shared => CowMode::StaticShared,
    }
}

/// Check if a receiver's borrow is disjoint from all sibling borrows.
///
/// For the optimization to apply, ALL of:
/// 1. The receiver has `BorrowSource::Exact { source, field: Some(f) }`
/// 2. The source variable is `Unique` (or qualifies for COW-aware borrowing)
/// 3. ALL other borrows from the same source have field info (`Some(g)`)
///    where `g != f` — i.e., they borrow different fields
///
/// If any sibling borrow targets the same field or has `None` (whole-object
/// borrow), the optimization is unsound and we return `false`.
fn is_borrow_disjoint_from_siblings(state_map: &AimsStateMap, receiver: ArcVarId) -> bool {
    let Some(&BorrowSource::Exact {
        source,
        field: Some(receiver_field),
    }) = state_map.borrow_source(receiver)
    else {
        return false;
    };

    // Check that the source is uniquely owned.
    let source_state = state_map.var_state_at_block_entry(super::block_id(0), source);
    if source_state.uniqueness != Uniqueness::Unique && !is_cow_aware_unique(&source_state) {
        return false;
    }

    // Check all sibling borrows from the same source for field disjointness.
    for (borrow_var, borrow_field) in state_map.borrows_from_source(source) {
        if borrow_var == receiver {
            continue; // skip self
        }
        match borrow_field {
            // Same field → aliasing, not safe.
            Some(f) if f == receiver_field => return false,
            // Whole-object borrow (no field) → conservatively unsafe.
            None => return false,
            // Different field → disjoint, safe.
            Some(_) => {}
        }
    }

    true
}

/// Check if a variable's combined state qualifies for COW-aware borrowing.
///
/// The combined state `(Owned, Linear, Once)` proves unique ownership from
/// cross-dimensional reasoning:
/// - `Owned`: callee received ownership transfer (one reference)
/// - `Linear`: callee consumes the value exactly once (no duplication/RcInc)
/// - `Once`: single use point (no additional references created)
///
/// Together, these guarantee that at the COW mutation point, the callee has
/// the only reference it was given and has not created additional ones. This
/// allows `StaticUnique` even when `uniqueness` is `MaybeShared` (conservative
/// backward default for parameters).
///
/// Note: this optimization requires the caller to pass `ArgOwnership::Owned`
/// (not borrowed). The `access == Owned` check confirms the callee's contract
/// demands ownership.
fn is_cow_aware_unique(state: &AimsState) -> bool {
    state.access == AccessClass::Owned
        && state.consumption == Consumption::Linear
        && state.cardinality == Cardinality::Once
}
