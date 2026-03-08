//! ARC IR structural verification.
//!
//! Validates invariants of [`ArcFunction`] that must hold after lowering
//! and throughout the pipeline. Inspired by Lean 4's `Compiler/IR/Checker.lean`
//! and Rust's `rustc_mir_transform/src/check_*.rs`.
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

use rustc_hash::FxHashSet;

use crate::graph::successor_block_ids;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId, ValueRepr};
use crate::Ownership;

/// A verification error found in the ARC IR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// Variable used before being defined (or not defined at all).
    UseBeforeDef { var: ArcVarId, block: ArcBlockId },

    /// Terminator references a block that doesn't exist.
    DanglingBlockRef {
        from_block: ArcBlockId,
        target: ArcBlockId,
    },

    /// `RcInc` or `RcDec` on a variable with `ValueRepr::Scalar`.
    RcOnScalar {
        var: ArcVarId,
        block: ArcBlockId,
        is_inc: bool,
    },

    /// `RcDec` on a borrowed parameter (borrowed values must not be freed).
    DecOnBorrowed { var: ArcVarId, block: ArcBlockId },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::UseBeforeDef { var, block } => {
                write!(f, "use-before-def: v{} in block {}", var.raw(), block.raw())
            }
            VerifyError::DanglingBlockRef { from_block, target } => {
                write!(
                    f,
                    "dangling block ref: block {} references non-existent block {}",
                    from_block.raw(),
                    target.raw()
                )
            }
            VerifyError::RcOnScalar { var, block, is_inc } => {
                let op = if *is_inc { "RcInc" } else { "RcDec" };
                write!(
                    f,
                    "{op} on scalar: v{} in block {} has ValueRepr::Scalar",
                    var.raw(),
                    block.raw()
                )
            }
            VerifyError::DecOnBorrowed { var, block } => {
                write!(
                    f,
                    "RcDec on borrowed param: v{} in block {}",
                    var.raw(),
                    block.raw()
                )
            }
        }
    }
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
// TODO(verify): Use DominatorTree for precise per-block scope checking.
fn check_variable_scope(func: &ArcFunction, errors: &mut Vec<VerifyError>) {
    // Collect all definitions globally (function params + block params +
    // instruction defs + invoke dsts).
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
        // Invoke dst is a definition visible in the normal successor.
        if let crate::ir::ArcTerminator::Invoke { dst, .. } = &block.terminator {
            defined.insert(*dst);
        }
    }

    // Check all uses against the global defined set.
    for block in &func.blocks {
        for instr in &block.body {
            for used in instr.used_vars() {
                if !defined.contains(&used) {
                    errors.push(VerifyError::UseBeforeDef {
                        var: used,
                        block: block.id,
                    });
                }
            }
        }
        for used in block.terminator.used_vars() {
            if !defined.contains(&used) {
                errors.push(VerifyError::UseBeforeDef {
                    var: used,
                    block: block.id,
                });
            }
        }
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

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { var, .. } => {
                    if is_scalar_var(func, *var) {
                        errors.push(VerifyError::RcOnScalar {
                            var: *var,
                            block: block.id,
                            is_inc: true,
                        });
                    }
                }
                ArcInstr::RcDec { var, .. } => {
                    if is_scalar_var(func, *var) {
                        errors.push(VerifyError::RcOnScalar {
                            var: *var,
                            block: block.id,
                            is_inc: false,
                        });
                    }
                }
                _ => {}
            }
        }
    }
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

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::RcDec { var, .. } = instr {
                if borrowed_params.contains(var) {
                    errors.push(VerifyError::DecOnBorrowed {
                        var: *var,
                        block: block.id,
                    });
                }
            }
        }
    }
}

fn is_scalar_var(func: &ArcFunction, var: ArcVarId) -> bool {
    func.var_reprs
        .get(var.index())
        .is_some_and(|repr| *repr == ValueRepr::Scalar)
}

#[cfg(test)]
mod tests;
