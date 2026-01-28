//! Type checking problem definitions.
//!
//! These problems occur during type checking when types don't match
//! or can't be inferred.

use super::impl_has_span;
use crate::ir::Span;

/// Problems that occur during type checking.
///
/// # Salsa Compatibility
/// Has Clone, Eq, `PartialEq`, Hash, Debug for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeProblem {
    /// Type mismatch between expected and found types.
    TypeMismatch {
        span: Span,
        expected: String,
        found: String,
    },

    /// Argument count mismatch in function call.
    ArgCountMismatch {
        span: Span,
        expected: usize,
        found: usize,
    },

    /// Tuple length mismatch in destructuring.
    TupleLengthMismatch {
        span: Span,
        expected: usize,
        found: usize,
    },

    /// List length mismatch in destructuring.
    ListLengthMismatch {
        span: Span,
        expected: usize,
        found: usize,
    },

    /// Infinite type detected (occurs check failure).
    InfiniteType { span: Span },

    /// Cannot infer type.
    CannotInfer { span: Span, context: String },

    /// Unknown type name.
    UnknownType { span: Span, name: String },

    /// Type is not callable.
    NotCallable { span: Span, found_type: String },

    /// Type is not indexable.
    NotIndexable { span: Span, found_type: String },

    /// Type doesn't have the accessed field.
    NoSuchField {
        span: Span,
        type_name: String,
        field_name: String,
        available_fields: Vec<String>,
    },

    /// Type doesn't have the called method.
    NoSuchMethod {
        span: Span,
        type_name: String,
        method_name: String,
        /// Available methods on this type (for "did you mean?" suggestions).
        available_methods: Vec<String>,
    },

    /// Binary operation not supported for types.
    InvalidBinaryOp {
        span: Span,
        op: String,
        left_type: String,
        right_type: String,
    },

    /// Unary operation not supported for type.
    InvalidUnaryOp {
        span: Span,
        op: String,
        operand_type: String,
    },

    /// Missing named argument in function call.
    MissingNamedArg { span: Span, arg_name: String },

    /// Unknown named argument in function call.
    UnknownNamedArg {
        span: Span,
        arg_name: String,
        valid_args: Vec<String>,
    },

    /// Duplicate named argument in function call.
    DuplicateNamedArg {
        span: Span,
        arg_name: String,
        first_span: Span,
    },

    /// Return type mismatch.
    ReturnTypeMismatch {
        span: Span,
        expected: String,
        found: String,
        func_name: String,
    },

    /// Try operator used on non-Result/Option type.
    InvalidTryOperand { span: Span, found_type: String },

    /// Await used on non-async value.
    InvalidAwait { span: Span, found_type: String },

    /// Condition must be bool.
    ConditionNotBool { span: Span, found_type: String },

    /// Iterator type mismatch in for loop.
    NotIterable { span: Span, found_type: String },

    /// Match arms have different types.
    MatchArmTypeMismatch {
        span: Span,
        first_type: String,
        this_type: String,
        first_span: Span,
    },

    /// Pattern type doesn't match scrutinee.
    PatternTypeMismatch {
        span: Span,
        expected: String,
        found: String,
    },

    /// Cyclic type definition.
    CyclicType { span: Span, type_name: String },

    /// Closure cannot capture itself.
    ClosureSelfReference { span: Span },
}

// Generate HasSpan implementation using macro.
// All variants use the standard `span` field.
impl_has_span! {
    TypeProblem {
        span: [
            TypeMismatch,
            ArgCountMismatch,
            TupleLengthMismatch,
            ListLengthMismatch,
            InfiniteType,
            CannotInfer,
            UnknownType,
            NotCallable,
            NotIndexable,
            NoSuchField,
            NoSuchMethod,
            InvalidBinaryOp,
            InvalidUnaryOp,
            MissingNamedArg,
            UnknownNamedArg,
            DuplicateNamedArg,
            ReturnTypeMismatch,
            InvalidTryOperand,
            InvalidAwait,
            ConditionNotBool,
            NotIterable,
            MatchArmTypeMismatch,
            PatternTypeMismatch,
            CyclicType,
            ClosureSelfReference,
        ],
    }
}

impl TypeProblem {
    /// Get the primary span of this problem.
    pub fn span(&self) -> Span {
        <Self as super::HasSpan>::span(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mismatch() {
        let problem = TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "int".into(),
            found: "str".into(),
        };

        assert_eq!(problem.span(), Span::new(10, 15));
    }

    #[test]
    fn test_arg_count_mismatch() {
        let problem = TypeProblem::ArgCountMismatch {
            span: Span::new(0, 20),
            expected: 2,
            found: 3,
        };

        assert_eq!(problem.span(), Span::new(0, 20));
    }

    #[test]
    fn test_no_such_field() {
        let problem = TypeProblem::NoSuchField {
            span: Span::new(5, 10),
            type_name: "Point".into(),
            field_name: "z".into(),
            available_fields: vec!["x".into(), "y".into()],
        };

        assert_eq!(problem.span(), Span::new(5, 10));
    }

    #[test]
    fn test_match_arm_type_mismatch() {
        let problem = TypeProblem::MatchArmTypeMismatch {
            span: Span::new(50, 60),
            first_type: "int".into(),
            this_type: "str".into(),
            first_span: Span::new(20, 25),
        };

        assert_eq!(problem.span(), Span::new(50, 60));
    }

    #[test]
    fn test_problem_equality() {
        let p1 = TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "int".into(),
            found: "str".into(),
        };

        let p2 = TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "int".into(),
            found: "str".into(),
        };

        let p3 = TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "float".into(),
            found: "str".into(),
        };

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_problem_hash() {
        use std::collections::HashSet;

        let p1 = TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "int".into(),
            found: "str".into(),
        };

        let p2 = p1.clone();
        let p3 = TypeProblem::CannotInfer {
            span: Span::new(10, 15),
            context: "return value".into(),
        };

        let mut set = HashSet::new();
        set.insert(p1);
        set.insert(p2); // duplicate
        set.insert(p3);

        assert_eq!(set.len(), 2);
    }
}
