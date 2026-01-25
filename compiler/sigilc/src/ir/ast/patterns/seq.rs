//! Sequential Expression Constructs (function_seq)
//!
//! Contains run, try, and match pattern constructs where order matters.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, PartialEq, Hash, Debug for Salsa requirements.

use crate::ir::{Span, TypeId, ExprId, Spanned};
use super::binding::{BindingPattern, MatchArm};
use super::super::ranges::{SeqBindingRange, ArmRange};

/// Element within a function_seq (run/try).
///
/// Can be either a let binding or a statement expression.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum SeqBinding {
    /// let [mut] pattern [: Type] = expr
    Let {
        pattern: BindingPattern,
        ty: Option<TypeId>,
        value: ExprId,
        mutable: bool,
        span: Span,
    },
    /// Statement expression (evaluated for side effects, e.g., assignment)
    Stmt {
        expr: ExprId,
        span: Span,
    },
}

impl Spanned for SeqBinding {
    fn span(&self) -> Span {
        match self {
            SeqBinding::Let { span, .. } => *span,
            SeqBinding::Stmt { span, .. } => *span,
        }
    }
}

/// Sequential expression construct (function_seq).
///
/// Contains a sequence of expressions where order matters.
/// NOT a function call - fundamentally different structure.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum FunctionSeq {
    /// run(let x = a, let y = b, result)
    Run {
        bindings: SeqBindingRange,
        result: ExprId,
        span: Span,
    },

    /// try(let x = fallible()?, let y = other()?, Ok(x + y))
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
            FunctionSeq::Run { span, .. } => *span,
            FunctionSeq::Try { span, .. } => *span,
            FunctionSeq::Match { span, .. } => *span,
            FunctionSeq::ForPattern { span, .. } => *span,
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
