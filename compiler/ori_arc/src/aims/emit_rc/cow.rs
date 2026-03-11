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
use crate::aims::lattice::Uniqueness;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator};
use crate::uniqueness::CowAnnotations;
use crate::CowMode;

/// Compute COW annotations from the AIMS state map.
///
/// Walks the final IR (post-merge). For each `Apply`/`Invoke` calling a
/// COW method, looks up the receiver variable's uniqueness in the state map
/// and derives [`CowMode`].
///
/// Must be called AFTER block merge (pipeline step 11a in Section 06.2).
pub fn compute_aims_cow_annotations(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    interner: &StringInterner,
) -> CowAnnotations {
    let cow_names = crate::borrow::all_cow_method_names(interner);
    let mut annotations = CowAnnotations::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        annotate_block_body(
            func,
            block_idx,
            block,
            state_map,
            &cow_names,
            &mut annotations,
        );
        annotate_block_terminator(block_idx, block, state_map, &cow_names, &mut annotations);
    }

    annotations
}

/// Annotate COW operations in a block's body instructions.
fn annotate_block_body(
    _func: &ArcFunction,
    block_idx: usize,
    block: &crate::ir::ArcBlock,
    state_map: &AimsStateMap,
    cow_names: &FxHashSet<Name>,
    annotations: &mut CowAnnotations,
) {
    for (instr_idx, instr) in block.body.iter().enumerate() {
        if let ArcInstr::Apply {
            func: callee, args, ..
        } = instr
        {
            if cow_names.contains(callee) && !args.is_empty() {
                let receiver = args[0];
                let mode = uniqueness_to_cow_mode(state_map, block_idx, receiver);
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
    annotations: &mut CowAnnotations,
) {
    if let ArcTerminator::Invoke {
        func: callee, args, ..
    } = &block.terminator
    {
        if cow_names.contains(callee) && !args.is_empty() {
            let receiver = args[0];
            let mode = uniqueness_to_cow_mode(state_map, block_idx, receiver);
            // Invoke uses body.len() as instr_idx (one past last body instruction).
            annotations.set(block_idx, block.body.len(), mode);
        }
    }
}

/// Derive [`CowMode`] from a variable's uniqueness in the state map.
///
/// Uses the block entry state for the receiver. If the variable is scalar
/// or has no state entry, defaults to `Dynamic` (safe fallback).
fn uniqueness_to_cow_mode(
    state_map: &AimsStateMap,
    block_idx: usize,
    receiver: crate::ir::ArcVarId,
) -> CowMode {
    if state_map.is_scalar(receiver) {
        return CowMode::Dynamic;
    }
    let blk = super::block_id(block_idx);
    let state = state_map.var_state_at_block_entry(blk, receiver);
    match state.uniqueness {
        Uniqueness::Unique => CowMode::StaticUnique,
        Uniqueness::MaybeShared => CowMode::Dynamic,
        Uniqueness::Shared => CowMode::StaticShared,
    }
}
