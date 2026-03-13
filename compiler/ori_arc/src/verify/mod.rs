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

use ori_ir::Span;
use rustc_hash::FxHashSet;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Cardinality;
use crate::graph::successor_block_ids;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId, ValueRepr};
use crate::Ownership;

/// A verification error found in the ARC IR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// Variable used before being defined (or not defined at all).
    UseBeforeDef {
        var: ArcVarId,
        block: ArcBlockId,
        span: Option<Span>,
    },

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
        span: Option<Span>,
    },

    /// `RcDec` on a borrowed parameter (borrowed values must not be freed).
    DecOnBorrowed {
        var: ArcVarId,
        block: ArcBlockId,
        span: Option<Span>,
    },

    /// Parameter with `Cardinality::Absent` has uses in the function body.
    /// This indicates an inconsistency between the AIMS analysis result
    /// and the actual IR: Absent means "zero forward uses", so no
    /// instruction or terminator should reference this variable.
    AbsentParamHasUses { var: ArcVarId, param_index: usize },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::UseBeforeDef { var, block, span } => {
                write!(f, "use-before-def: v{} in block {}", var.raw(), block.raw())?;
                fmt_span(f, *span)
            }
            VerifyError::DanglingBlockRef { from_block, target } => {
                write!(
                    f,
                    "dangling block ref: block {} references non-existent block {}",
                    from_block.raw(),
                    target.raw()
                )
            }
            VerifyError::RcOnScalar {
                var,
                block,
                is_inc,
                span,
            } => {
                let op = if *is_inc { "RcInc" } else { "RcDec" };
                write!(
                    f,
                    "{op} on scalar: v{} in block {} has ValueRepr::Scalar",
                    var.raw(),
                    block.raw()
                )?;
                fmt_span(f, *span)
            }
            VerifyError::DecOnBorrowed { var, block, span } => {
                write!(
                    f,
                    "RcDec on borrowed param: v{} in block {}",
                    var.raw(),
                    block.raw()
                )?;
                fmt_span(f, *span)
            }
            VerifyError::AbsentParamHasUses { var, param_index } => {
                write!(
                    f,
                    "absent param has uses: v{} (param {param_index}) has Cardinality::Absent \
                     but is referenced in the function body",
                    var.raw(),
                )
            }
        }
    }
}

fn fmt_span(f: &mut std::fmt::Formatter<'_>, span: Option<Span>) -> std::fmt::Result {
    if let Some(s) = span {
        write!(f, " at {}..{}", s.start, s.end)
    } else {
        Ok(())
    }
}

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
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            for used in instr.used_vars() {
                if !defined.contains(&used) {
                    errors.push(VerifyError::UseBeforeDef {
                        var: used,
                        block: block.id,
                        span: get_span(func, block_idx, instr_idx),
                    });
                }
            }
        }
        // Terminators don't have instruction-level spans.
        for used in block.terminator.used_vars() {
            if !defined.contains(&used) {
                errors.push(VerifyError::UseBeforeDef {
                    var: used,
                    block: block.id,
                    span: None,
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

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            match instr {
                ArcInstr::RcInc { var, .. } => {
                    if is_scalar_var(func, *var) {
                        errors.push(VerifyError::RcOnScalar {
                            var: *var,
                            block: block.id,
                            is_inc: true,
                            span: get_span(func, block_idx, instr_idx),
                        });
                    }
                }
                ArcInstr::RcDec { var, .. } => {
                    if is_scalar_var(func, *var) {
                        errors.push(VerifyError::RcOnScalar {
                            var: *var,
                            block: block.id,
                            is_inc: false,
                            span: get_span(func, block_idx, instr_idx),
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

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::RcDec { var, .. } = instr {
                if borrowed_params.contains(var) {
                    errors.push(VerifyError::DecOnBorrowed {
                        var: *var,
                        block: block.id,
                        span: get_span(func, block_idx, instr_idx),
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

/// Parameters with `Cardinality::Absent` must have no uses in the function body.
///
/// Absent means the backward analysis found zero forward demand for this
/// parameter. If the IR actually references the parameter, the analysis
/// result is inconsistent — either the analysis is wrong or the IR was
/// mutated after analysis.
fn check_absent_param_no_uses(
    func: &ArcFunction,
    contract: &MemoryContract,
    errors: &mut Vec<VerifyError>,
) {
    // Collect parameter vars that the contract says are Absent.
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

    // Collect all used variables across the function body.
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

    for (param_index, var) in absent_params {
        if used.contains(&var) {
            errors.push(VerifyError::AbsentParamHasUses { var, param_index });
        }
    }
}

#[cfg(test)]
mod tests;
