//! Sequential Expression Constructs (`function_seq`)
//!
//! Contains try, match, and for-pattern constructs where order matters.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use super::super::ranges::ArmRange;
use super::binding::MatchArm;
use crate::{ExprId, Span, Spanned, StmtRange};

/// Sequential expression construct (`function_seq`).
///
/// Contains a sequence of expressions where order matters.
/// NOT a function call - fundamentally different structure.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum FunctionSeq {
    /// `try { let x = fallible()?; Ok(x) }`
    ///
    /// Uses the same `StmtRange` as block expressions. The try-specific
    /// semantics (auto-unwrap Result/Option in let bindings) are handled
    /// by the type checker, not by a separate AST type.
    Try {
        stmts: StmtRange,
        result: ExprId,
        span: Span,
    },

    /// match(scrutinee, Pattern -> expr, ...)
    Match {
        scrutinee: ExprId,
        arms: ArmRange,
        span: Span,
    },

    /// for(over: items, [map: transform,] match: Pattern -> expr, default: fallback)
    ForPattern {
        over: ExprId,
        map: Option<ExprId>,
        arm: MatchArm,
        default: ExprId,
        span: Span,
    },
}

impl FunctionSeq {
    pub fn span(&self) -> Span {
        match self {
            FunctionSeq::Try { span, .. }
            | FunctionSeq::Match { span, .. }
            | FunctionSeq::ForPattern { span, .. } => *span,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FunctionSeq::Try { .. } => "try",
            FunctionSeq::Match { .. } => "match",
            FunctionSeq::ForPattern { .. } => "for",
        }
    }
}

impl Spanned for FunctionSeq {
    fn span(&self) -> Span {
        self.span()
    }
}

#[cfg(test)]
mod tests;
