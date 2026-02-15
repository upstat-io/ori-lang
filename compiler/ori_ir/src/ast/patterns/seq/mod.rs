//! Sequential Expression Constructs (`function_seq`)
//!
//! Contains run, try, and match pattern constructs where order matters.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use super::super::ranges::{ArmRange, CheckRange, SeqBindingRange};
use super::binding::MatchArm;
use crate::{BindingPatternId, ExprId, ParsedTypeId, Span, Spanned};

/// Element within a `function_seq` (run/try).
///
/// Can be either a let binding or a statement expression.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum SeqBinding {
    /// `let [mut] pattern [: Type] = expr`
    Let {
        pattern: BindingPatternId,
        /// Type annotation (`ParsedTypeId::INVALID` = no annotation).
        ty: ParsedTypeId,
        value: ExprId,
        mutable: bool,
        span: Span,
    },
    /// Statement expression (evaluated for side effects, e.g., assignment)
    Stmt { expr: ExprId, span: Span },
}

impl Spanned for SeqBinding {
    fn span(&self) -> Span {
        match self {
            SeqBinding::Let { span, .. } | SeqBinding::Stmt { span, .. } => *span,
        }
    }
}

/// Check condition with optional custom panic message.
///
/// Used in `run()` pre/post checks: `pre_check: condition | "message"`.
/// Post-checks use a lambda as the expression: `post_check: r -> r > 0 | "msg"`.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct CheckExpr {
    /// Condition expression (`pre_check`) or lambda (`post_check`).
    pub expr: ExprId,
    /// Optional custom panic message (string literal after `|`).
    pub message: Option<ExprId>,
    pub span: Span,
}

impl Spanned for CheckExpr {
    fn span(&self) -> Span {
        self.span
    }
}

/// Sequential expression construct (`function_seq`).
///
/// Contains a sequence of expressions where order matters.
/// NOT a function call - fundamentally different structure.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum FunctionSeq {
    /// `run(pre_check: cond, let x = a, let y = b, result, post_check: r -> r > 0)`
    Run {
        pre_checks: CheckRange,
        bindings: SeqBindingRange,
        result: ExprId,
        post_checks: CheckRange,
        span: Span,
    },

    /// try(let x = `fallible()`?, let y = `other()`?, Ok(x + y))
    Try {
        bindings: SeqBindingRange,
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
            FunctionSeq::Run { span, .. }
            | FunctionSeq::Try { span, .. }
            | FunctionSeq::Match { span, .. }
            | FunctionSeq::ForPattern { span, .. } => *span,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FunctionSeq::Run { .. } => "run",
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
