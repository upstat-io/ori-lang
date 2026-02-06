//! Sequential Expression Constructs (`function_seq`)
//!
//! Contains run, try, and match pattern constructs where order matters.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use super::super::ranges::{ArmRange, SeqBindingRange};
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

/// Sequential expression construct (`function_seq`).
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
mod tests {
    use super::*;
    use crate::{ExprId, MatchPattern};

    #[test]
    fn test_function_seq_name_all_variants() {
        // Verify all 4 FunctionSeq variants return correct names
        let run = FunctionSeq::Run {
            bindings: SeqBindingRange::EMPTY,
            result: ExprId::new(0),
            span: Span::new(0, 10),
        };
        assert_eq!(run.name(), "run");

        let try_seq = FunctionSeq::Try {
            bindings: SeqBindingRange::EMPTY,
            result: ExprId::new(0),
            span: Span::new(0, 10),
        };
        assert_eq!(try_seq.name(), "try");

        let match_seq = FunctionSeq::Match {
            scrutinee: ExprId::new(0),
            arms: ArmRange::EMPTY,
            span: Span::new(0, 10),
        };
        assert_eq!(match_seq.name(), "match");

        let for_pattern = FunctionSeq::ForPattern {
            over: ExprId::new(0),
            map: None,
            arm: MatchArm {
                pattern: MatchPattern::Wildcard,
                guard: None,
                body: ExprId::new(1),
                span: Span::new(5, 10),
            },
            default: ExprId::new(2),
            span: Span::new(0, 15),
        };
        assert_eq!(for_pattern.name(), "for");
    }

    #[test]
    fn test_function_seq_span_all_variants() {
        let run_span = Span::new(0, 10);
        let run = FunctionSeq::Run {
            bindings: SeqBindingRange::EMPTY,
            result: ExprId::new(0),
            span: run_span,
        };
        assert_eq!(run.span(), run_span);

        let try_span = Span::new(5, 20);
        let try_seq = FunctionSeq::Try {
            bindings: SeqBindingRange::EMPTY,
            result: ExprId::new(0),
            span: try_span,
        };
        assert_eq!(try_seq.span(), try_span);

        let match_span = Span::new(10, 30);
        let match_seq = FunctionSeq::Match {
            scrutinee: ExprId::new(0),
            arms: ArmRange::EMPTY,
            span: match_span,
        };
        assert_eq!(match_seq.span(), match_span);

        let for_span = Span::new(15, 40);
        let for_pattern = FunctionSeq::ForPattern {
            over: ExprId::new(0),
            map: Some(ExprId::new(1)),
            arm: MatchArm {
                pattern: MatchPattern::Wildcard,
                guard: None,
                body: ExprId::new(2),
                span: Span::new(20, 35),
            },
            default: ExprId::new(3),
            span: for_span,
        };
        assert_eq!(for_pattern.span(), for_span);
    }

    #[test]
    fn test_function_seq_spanned_trait() {
        use crate::Spanned;

        let run = FunctionSeq::Run {
            bindings: SeqBindingRange::EMPTY,
            result: ExprId::new(0),
            span: Span::new(100, 200),
        };

        // Test that Spanned trait works correctly
        let spanned: &dyn Spanned = &run;
        assert_eq!(spanned.span(), Span::new(100, 200));
    }
}
