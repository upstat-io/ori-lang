//! Verification error types and display.
//!
//! Defines [`VerifyError`] plus small construction helpers used by the
//! individual check functions in [`crate::verify`]. The helpers let each
//! `check_*` function stay shallow (≤ depth 4) by pushing the
//! error-construction detail through a named callee.

use ori_ir::Span;

use crate::ir::{ArcBlockId, ArcVarId};

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

    /// `arg_ownership` length doesn't match `args` length (when non-empty).
    /// Empty `arg_ownership` is valid (pre-annotation state).
    ArgOwnershipLenMismatch {
        block: ArcBlockId,
        expected: usize,
        actual: usize,
    },

    /// FIP structural violation detected during pipeline verification.
    ///
    /// Wraps FIP verification errors (`CertifiedButUnboundedStack`,
    /// `BoundedExceeded`) that represent genuine internal compiler bugs
    /// — the interprocedural analysis produced an inconsistent contract.
    FipStructural { message: String },

    /// A variable's type is `Tag::Var` or `Tag::Projection` after typeck
    /// exit — a PC-2 invariant violation. Wraps
    /// [`crate::ir::validate::UnresolvedTypeVar`] so existing verification
    /// error handling works unchanged.
    UnresolvedTypeVar(crate::ir::validate::UnresolvedTypeVar),

    /// A lambda parameter's type is `Tag::BoundVar` at codegen entry — a
    /// monomorphization-resolution invariant violation (sibling to PC-2;
    /// `types.md §SC-1` + `typeck.md §GN-2`). Wraps
    /// [`crate::ir::validate::UnresolvedBoundVar`].
    UnresolvedBoundVar(crate::ir::validate::UnresolvedBoundVar),

    /// Function-exit burden-balance violation: the algebraic sum
    /// `Σ BurdenInc(v) - Σ BurdenDec*(v)` along some reachable path from
    /// `func.entry` to a terminal block (`Return`/`Resume`/`Unreachable`)
    /// is non-zero for a variable in `func.burden_emitted`. AIMS Invariant 1
    /// violation (contract/realization disagreement) — VF-1 basic check.
    BurdenImbalance(BurdenBalanceError),
}

/// Per-violation payload for [`VerifyError::BurdenImbalance`].
///
/// `expected_net` is always `0` for the function-exit check (every
/// burden-emitted var MUST net to zero on every reachable exit per VF-1).
/// `observed_net` is the algebraic delta observed at `exit_block`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BurdenBalanceError {
    pub var: ArcVarId,
    pub expected_net: i64,
    pub observed_net: i64,
    pub exit_block: ArcBlockId,
}

impl From<crate::ir::validate::UnresolvedTypeVar> for VerifyError {
    fn from(violation: crate::ir::validate::UnresolvedTypeVar) -> Self {
        VerifyError::UnresolvedTypeVar(violation)
    }
}

impl From<crate::ir::validate::UnresolvedBoundVar> for VerifyError {
    fn from(violation: crate::ir::validate::UnresolvedBoundVar) -> Self {
        VerifyError::UnresolvedBoundVar(violation)
    }
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
            VerifyError::ArgOwnershipLenMismatch {
                block,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "arg_ownership length mismatch in block {}: \
                     expected {} (args.len()) but got {}",
                    block.raw(),
                    expected,
                    actual,
                )
            }
            VerifyError::FipStructural { message } => {
                write!(f, "FIP structural violation: {message}")
            }
            VerifyError::UnresolvedTypeVar(violation) => {
                write!(
                    f,
                    "unresolved type variable (PC-2 violation): function name_id={:?}, \
                     ArcVarId({}), idx={:?}, tag={:?}",
                    violation.function,
                    violation.var_id.raw(),
                    violation.idx,
                    violation.tag,
                )
            }
            VerifyError::UnresolvedBoundVar(violation) => {
                write!(
                    f,
                    "unresolved bound variable (monomorphization-resolution violation): \
                     lambda name_id={:?}, ArcVarId({}), idx={:?}",
                    violation.function,
                    violation.var_id.raw(),
                    violation.idx,
                )
            }
            VerifyError::BurdenImbalance(BurdenBalanceError {
                var,
                expected_net,
                observed_net,
                exit_block,
            }) => {
                write!(
                    f,
                    "burden imbalance (VF-1): v{} net={} expected={} at exit block {}",
                    var.raw(),
                    observed_net,
                    expected_net,
                    exit_block.raw(),
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

// Small constructor helpers keep the individual `check_*` functions
// shallow. Each helper is one line of logic; their job is to move
// the `VerifyError { ... }` literal out of a deeply-nested expression
// so the check function's nesting depth stays ≤ 4.

/// Record an `RcInc`/`RcDec` on a scalar variable.
pub(super) fn push_rc_on_scalar(
    errors: &mut Vec<VerifyError>,
    var: ArcVarId,
    block: ArcBlockId,
    is_inc: bool,
    span: Option<Span>,
) {
    errors.push(VerifyError::RcOnScalar {
        var,
        block,
        is_inc,
        span,
    });
}

/// Record an `RcDec` on a borrowed parameter.
pub(super) fn push_dec_on_borrowed(
    errors: &mut Vec<VerifyError>,
    var: ArcVarId,
    block: ArcBlockId,
    span: Option<Span>,
) {
    errors.push(VerifyError::DecOnBorrowed { var, block, span });
}

/// Record a use-before-def for a variable.
pub(super) fn push_use_before_def(
    errors: &mut Vec<VerifyError>,
    var: ArcVarId,
    block: ArcBlockId,
    span: Option<Span>,
) {
    errors.push(VerifyError::UseBeforeDef { var, block, span });
}
