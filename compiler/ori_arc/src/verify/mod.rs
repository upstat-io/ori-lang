//! ARC IR structural verification.
//!
//! Validates invariants of [`ArcFunction`] that must hold after lowering
//! and throughout the pipeline. Historical influence: Lean 4's `Compiler/IR/Checker.lean`
//! and Rust's `rustc_mir_transform/src/check_*.rs` SHAPE.
//!
//! Checks are grouped into two categories:
//!
//! 1. **Structural** — variable scope, block connectivity, terminator presence.
//!    These must hold after lowering, before any optimization pass.
//!
//! 2. **RC properties** — no RC on scalars, no dec on borrowed params.
//!    These must hold after RC insertion and after RC elimination.
//!
//! The entry point [`check_function`] runs all applicable checks and returns
//! a list of [`VerifyError`]s (empty = all invariants hold).

mod error;

use ori_ir::Span;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Cardinality;
use crate::graph::successor_block_ids;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ValueRepr};
use crate::Ownership;

use error::{push_dec_on_borrowed, push_rc_on_scalar, push_use_before_def};
pub use error::{BurdenBalanceError, VerifyError};

/// Safely look up the source span for an instruction.
///
/// Returns `None` if spans haven't been populated yet (e.g., verification
/// runs before lowering completes) or if the indices are out of bounds
/// (e.g., synthetic instructions inserted by later passes).
fn get_span(func: &ArcFunction, block_idx: usize, instr_idx: usize) -> Option<Span> {
    func.spans
        .get(block_idx)
        .and_then(|block_spans| block_spans.get(instr_idx))
        .copied()
        .flatten()
}

/// Run all verification checks on an `ArcFunction`.
///
/// Returns an empty vec if all invariants hold. Checks are independent
/// and run unconditionally — a failure in one does not affect others.
///
/// Call after lowering for structural checks, and after RC insertion
/// for RC property checks.
pub fn check_function(func: &ArcFunction) -> Vec<VerifyError> {
    let mut errors = Vec::new();
    check_variable_scope(func, &mut errors);
    check_block_connectivity(func, &mut errors);
    check_no_rc_on_scalar(func, &mut errors);
    check_no_dec_on_borrowed(func, &mut errors);
    check_arg_ownership_len(func, &mut errors);
    errors
}

// Structural checks

/// Every `ArcVarId` used in an instruction or terminator must be defined
/// before use — by a preceding instruction, function parameter, or block
/// parameter.
///
/// This uses a flat global `defined` set, which is sound for SSA-form IR
/// where each variable is defined exactly once. It is an over-approximation:
/// it will not catch use-before-def where the definition exists but does not
/// dominate the use.
///
// NOTE: Flat global defined-set is intentionally over-approximate for SSA-form IR.
// Dominator-based scope checking would catch more bugs but the current check is
// sufficient for the invariants we need to maintain (use-before-def for SSA vars).
fn check_variable_scope(func: &ArcFunction, errors: &mut Vec<VerifyError>) {
    let defined = collect_defined_vars(func);
    for (block_idx, block) in func.blocks.iter().enumerate() {
        check_block_uses(func, block, block_idx, &defined, errors);
    }
}

/// Collect every variable defined in the function: function params, block
/// params, instruction defs, and invoke dsts.
fn collect_defined_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut defined: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    for block in &func.blocks {
        for (var, _ty) in &block.params {
            defined.insert(*var);
        }
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                defined.insert(dst);
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } => {
                defined.insert(*dst);
            }
            _ => {}
        }
    }
    defined
}

/// Check every use within `block` against `defined`, recording
/// `UseBeforeDef` for any miss.
fn check_block_uses(
    func: &ArcFunction,
    block: &crate::ir::ArcBlock,
    block_idx: usize,
    defined: &FxHashSet<ArcVarId>,
    errors: &mut Vec<VerifyError>,
) {
    for (instr_idx, instr) in block.body.iter().enumerate() {
        for used in instr.used_vars() {
            if defined.contains(&used) {
                continue;
            }
            push_use_before_def(errors, used, block.id, get_span(func, block_idx, instr_idx));
        }
    }
    // Terminators don't have instruction-level spans.
    for used in block.terminator.used_vars() {
        if defined.contains(&used) {
            continue;
        }
        push_use_before_def(errors, used, block.id, None);
    }
}

/// Every block referenced by a terminator must exist in the function.
fn check_block_connectivity(func: &ArcFunction, errors: &mut Vec<VerifyError>) {
    let valid_blocks: FxHashSet<ArcBlockId> = func.blocks.iter().map(|b| b.id).collect();

    for block in &func.blocks {
        for target in successor_block_ids(&block.terminator) {
            if !valid_blocks.contains(&target) {
                errors.push(VerifyError::DanglingBlockRef {
                    from_block: block.id,
                    target,
                });
            }
        }
    }
}

// RC property checks

/// `RcInc`/`RcDec` must never operate on variables with `ValueRepr::Scalar`.
///
/// This catches misclassification bugs where a scalar type was incorrectly
/// given RC operations. Only runs when `var_reprs` has been computed
/// (non-empty).
fn check_no_rc_on_scalar(func: &ArcFunction, errors: &mut Vec<VerifyError>) {
    if func.var_reprs.is_empty() {
        return;
    }

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            check_rc_instr_scalar(func, block.id, instr, block_idx, instr_idx, errors);
        }
    }
}

/// Inspect a single instruction for `RcInc`/`RcDec` on a scalar variable.
fn check_rc_instr_scalar(
    func: &ArcFunction,
    block_id: ArcBlockId,
    instr: &ArcInstr,
    block_idx: usize,
    instr_idx: usize,
    errors: &mut Vec<VerifyError>,
) {
    let (var, is_inc) = match instr {
        ArcInstr::RcInc { var, .. } => (*var, true),
        ArcInstr::RcDec { var, .. } => (*var, false),
        _ => return,
    };
    if !is_scalar_var(func, var) {
        return;
    }
    push_rc_on_scalar(
        errors,
        var,
        block_id,
        is_inc,
        get_span(func, block_idx, instr_idx),
    );
}

/// `RcDec` must never target a borrowed parameter.
///
/// Borrowed parameters are read-only references — the caller retains
/// ownership, so the callee must not decrement their reference count.
fn check_no_dec_on_borrowed(func: &ArcFunction, errors: &mut Vec<VerifyError>) {
    let borrowed_params: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();

    if borrowed_params.is_empty() {
        return;
    }

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::RcDec { var, .. } = instr else {
                continue;
            };
            if !borrowed_params.contains(var) {
                continue;
            }
            push_dec_on_borrowed(errors, *var, block.id, get_span(func, block_idx, instr_idx));
        }
    }
}

/// `arg_ownership` length must match `args` length when non-empty.
///
/// Empty `arg_ownership` is valid (pre-annotation state). A non-empty
/// `arg_ownership` that doesn't match `args.len()` indicates a pipeline
/// bug where annotation populated an incorrect number of entries.
fn check_arg_ownership_len(func: &ArcFunction, errors: &mut Vec<VerifyError>) {
    for block in &func.blocks {
        for instr in &block.body {
            check_instr_arg_ownership(block.id, instr, errors);
        }
        check_terminator_arg_ownership(block.id, &block.terminator, errors);
    }
}

/// Check `arg_ownership` length on `Apply` / `ApplyIndirect` instructions.
fn check_instr_arg_ownership(
    block_id: ArcBlockId,
    instr: &ArcInstr,
    errors: &mut Vec<VerifyError>,
) {
    let (args_len, ownership_len) = match instr {
        ArcInstr::Apply {
            args,
            arg_ownership,
            ..
        }
        | ArcInstr::ApplyIndirect {
            args,
            arg_ownership,
            ..
        } => (args.len(), arg_ownership.len()),
        _ => return,
    };
    record_arg_ownership_mismatch(block_id, args_len, ownership_len, errors);
}

/// Check `arg_ownership` length on `Invoke` / `InvokeIndirect` terminators.
fn check_terminator_arg_ownership(
    block_id: ArcBlockId,
    term: &ArcTerminator,
    errors: &mut Vec<VerifyError>,
) {
    let (args_len, ownership_len) = match term {
        ArcTerminator::Invoke {
            args,
            arg_ownership,
            ..
        }
        | ArcTerminator::InvokeIndirect {
            args,
            arg_ownership,
            ..
        } => (args.len(), arg_ownership.len()),
        _ => return,
    };
    record_arg_ownership_mismatch(block_id, args_len, ownership_len, errors);
}

/// Push an `ArgOwnershipLenMismatch` when `arg_ownership` is non-empty
/// but its length diverges from `args`.
fn record_arg_ownership_mismatch(
    block_id: ArcBlockId,
    args_len: usize,
    ownership_len: usize,
    errors: &mut Vec<VerifyError>,
) {
    if ownership_len == 0 || ownership_len == args_len {
        return;
    }
    errors.push(VerifyError::ArgOwnershipLenMismatch {
        block: block_id,
        expected: args_len,
        actual: ownership_len,
    });
}

fn is_scalar_var(func: &ArcFunction, var: ArcVarId) -> bool {
    func.var_reprs
        .get(var.index())
        .is_some_and(|repr| *repr == ValueRepr::Scalar)
}

// AIMS consistency checks

/// Run structural checks plus AIMS-specific consistency checks.
///
/// Extends [`check_function`] with checks that require the AIMS
/// [`MemoryContract`] — specifically, that parameters with
/// `Cardinality::Absent` have no uses in the function body.
pub fn check_function_with_contract(
    func: &ArcFunction,
    contract: &MemoryContract,
) -> Vec<VerifyError> {
    let mut errors = check_function(func);
    check_absent_param_no_uses(func, contract, &mut errors);
    errors
}

/// Parameters with `Cardinality::Absent` must have no uses on *live* paths.
///
/// A block is "live" if it is both forward-reachable from entry AND
/// backward-reachable to a Return terminator. A block on a path that
/// always ends in panic/unreachable is "dead" — uses there are expected
/// when the backward analysis correctly identifies zero forward demand
/// through the live path (e.g., `if true then panic(); x` where `x` is
/// only referenced in the dead else-branch return).
///
/// Uses in dead blocks are silently ignored. Uses in LIVE blocks are
/// genuine inconsistencies between the AIMS analysis and the IR.
fn check_absent_param_no_uses(
    func: &ArcFunction,
    contract: &MemoryContract,
    errors: &mut Vec<VerifyError>,
) {
    let absent_params: Vec<(usize, ArcVarId)> = func
        .params
        .iter()
        .zip(&contract.params)
        .enumerate()
        .filter(|(_, (_, pc))| pc.cardinality == Cardinality::Absent)
        .map(|(i, (param, _))| (i, param.var))
        .collect();

    if absent_params.is_empty() {
        return;
    }

    // Live blocks = forward-reachable from entry ∩ backward-reachable to Return.
    let live = live_blocks(func);
    let used = collect_used_vars_in_live_blocks(func, &live);

    for (param_index, var) in absent_params {
        if used.contains(&var) {
            errors.push(VerifyError::AbsentParamHasUses { var, param_index });
        }
    }
}

/// Collect every variable referenced by an instruction or terminator in a
/// block that is part of `live`. Blocks outside `live` are dead code and
/// are silently ignored.
fn collect_used_vars_in_live_blocks(
    func: &ArcFunction,
    live: &FxHashSet<ArcBlockId>,
) -> FxHashSet<ArcVarId> {
    let mut used = FxHashSet::default();
    for block in &func.blocks {
        if !live.contains(&block.id) {
            continue;
        }
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

/// Compute blocks that are both forward-reachable from `func.entry` AND
/// backward-reachable to a real exit (Return or Resume).
///
/// A block on a path that always ends in `Unreachable` is dead.
/// `Resume` IS a real exit (exceptional unwind path carries demand).
fn live_blocks(func: &ArcFunction) -> FxHashSet<ArcBlockId> {
    let forward = forward_reachable(func);
    let preds = build_predecessor_map(func);
    let backward = backward_reachable_from_exits(func, &preds);
    forward.intersection(&backward).copied().collect()
}

/// BFS from `func.entry` over successor edges. Entry may be non-zero
/// after TRMC prologue insertion.
fn forward_reachable(func: &ArcFunction) -> FxHashSet<ArcBlockId> {
    let mut reached = FxHashSet::default();
    let mut worklist = vec![func.entry];
    while let Some(id) = worklist.pop() {
        if !reached.insert(id) {
            continue;
        }
        let Some(b) = func.blocks.iter().find(|b| b.id == id) else {
            continue;
        };
        for succ in successor_block_ids(&b.terminator) {
            worklist.push(succ);
        }
    }
    reached
}

/// Build `successor → predecessors` map for backward BFS.
fn build_predecessor_map(func: &ArcFunction) -> FxHashMap<ArcBlockId, Vec<ArcBlockId>> {
    let mut preds: FxHashMap<ArcBlockId, Vec<ArcBlockId>> = FxHashMap::default();
    for block in &func.blocks {
        for succ in successor_block_ids(&block.terminator) {
            preds.entry(succ).or_default().push(block.id);
        }
    }
    preds
}

/// BFS backward from every `Return`/`Resume` block over predecessor edges.
/// `Unreachable` is NOT a real exit — blocks leading only to `Unreachable`
/// are dead code and are excluded from the live set.
fn backward_reachable_from_exits(
    func: &ArcFunction,
    preds: &FxHashMap<ArcBlockId, Vec<ArcBlockId>>,
) -> FxHashSet<ArcBlockId> {
    let mut reached = FxHashSet::default();
    let mut worklist: Vec<ArcBlockId> = func
        .blocks
        .iter()
        .filter(|b| {
            matches!(
                b.terminator,
                ArcTerminator::Return { .. } | ArcTerminator::Resume
            )
        })
        .map(|b| b.id)
        .collect();
    while let Some(id) = worklist.pop() {
        if !reached.insert(id) {
            continue;
        }
        let Some(pred_list) = preds.get(&id) else {
            continue;
        };
        for &pred in pred_list {
            worklist.push(pred);
        }
    }
    reached
}

#[cfg(test)]
mod tests;
