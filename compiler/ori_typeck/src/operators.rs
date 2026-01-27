//! Type-level operator implementations for the type checker.
//!
//! This module extracts binary operation type checking logic,
//! following the Open/Closed Principle. New operators can be added
//! by implementing the `TypeOperator` trait.

use ori_diagnostic::ErrorCode;
use ori_ir::{BinaryOp, Span, StringInterner};
use ori_types::{Type, InferenceContext};

// Type Operator Result

/// Result of type checking a binary operation.
pub enum TypeOpResult {
    /// Successfully type checked, returning the result type.
    Ok(Type),
    /// Type error occurred.
    Err(TypeOpError),
}

/// Error from type checking a binary operation.
#[derive(Debug)]
pub struct TypeOpError {
    /// Error message.
    pub message: String,
    /// Error code for diagnostics.
    pub code: ErrorCode,
}

impl TypeOpError {
    pub fn new(message: impl Into<String>, code: ErrorCode) -> Self {
        TypeOpError {
            message: message.into(),
            code,
        }
    }
}

// Type Operator Trait

/// Trait for type checking binary operations.
///
/// Implementations handle specific operator categories.
pub trait TypeOperator: Send + Sync {
    /// Check if this operator handles the given operation.
    fn handles(&self, op: BinaryOp) -> bool;

    /// Type check the binary operation.
    ///
    /// The inference context is provided for unification and fresh variables.
    /// The interner is provided for type display in error messages.
    fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        op: BinaryOp,
        left: &Type,
        right: &Type,
        span: Span,
    ) -> TypeOpResult;
}

// Arithmetic Type Operator

/// Type checking for arithmetic operators: +, -, *, /, %, div
pub struct ArithmeticTypeOp;

impl TypeOperator for ArithmeticTypeOp {
    fn handles(&self, op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Mod
                | BinaryOp::FloorDiv
        )
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        op: BinaryOp,
        left: &Type,
        right: &Type,
        _span: Span,
    ) -> TypeOpResult {
        // Unify left and right types
        if let Err(e) = ctx.unify(left, right) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("type mismatch in arithmetic operation: {e:?}"),
                ErrorCode::E2001,
            ));
        }

        let resolved = ctx.resolve(left);
        match resolved {
            Type::Str if op == BinaryOp::Add => TypeOpResult::Ok(Type::Str), // String concat
            Type::Int | Type::Float | Type::Var(_) => TypeOpResult::Ok(resolved), // Defer Var checking
            _ => {
                let op_name = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Mod => "%",
                    BinaryOp::FloorDiv => "div",
                    _ => "arithmetic",
                };
                TypeOpResult::Err(TypeOpError::new(
                    format!(
                        "cannot apply `{}` to `{}`: arithmetic operators require numeric types (int or float)",
                        op_name,
                        left.display(interner)
                    ),
                    ErrorCode::E2001,
                ))
            }
        }
    }
}

// Comparison Type Operator

/// Type checking for comparison operators: ==, !=, <, <=, >, >=
pub struct ComparisonTypeOp;

impl TypeOperator for ComparisonTypeOp {
    fn handles(&self, op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
        )
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        _interner: &StringInterner,
        _op: BinaryOp,
        left: &Type,
        right: &Type,
        _span: Span,
    ) -> TypeOpResult {
        if let Err(e) = ctx.unify(left, right) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("type mismatch in comparison: {e:?}"),
                ErrorCode::E2001,
            ));
        }
        TypeOpResult::Ok(Type::Bool)
    }
}

// Logical Type Operator

/// Type checking for logical operators: &&, ||
pub struct LogicalTypeOp;

impl TypeOperator for LogicalTypeOp {
    fn handles(&self, op: BinaryOp) -> bool {
        matches!(op, BinaryOp::And | BinaryOp::Or)
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        _interner: &StringInterner,
        _op: BinaryOp,
        left: &Type,
        right: &Type,
        _span: Span,
    ) -> TypeOpResult {
        if let Err(e) = ctx.unify(left, &Type::Bool) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("left operand of logical operator must be bool: {e:?}"),
                ErrorCode::E2001,
            ));
        }
        if let Err(e) = ctx.unify(right, &Type::Bool) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("right operand of logical operator must be bool: {e:?}"),
                ErrorCode::E2001,
            ));
        }
        TypeOpResult::Ok(Type::Bool)
    }
}

// Bitwise Type Operator

/// Type checking for bitwise operators: &, |, ^, <<, >>
pub struct BitwiseTypeOp;

impl TypeOperator for BitwiseTypeOp {
    fn handles(&self, op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr
        )
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        _interner: &StringInterner,
        _op: BinaryOp,
        left: &Type,
        right: &Type,
        _span: Span,
    ) -> TypeOpResult {
        if let Err(e) = ctx.unify(left, &Type::Int) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("left operand of bitwise operator must be int: {e:?}"),
                ErrorCode::E2001,
            ));
        }
        if let Err(e) = ctx.unify(right, &Type::Int) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("right operand of bitwise operator must be int: {e:?}"),
                ErrorCode::E2001,
            ));
        }
        TypeOpResult::Ok(Type::Int)
    }
}

// Range Type Operator

/// Type checking for range operators: .., ..=
pub struct RangeTypeOp;

impl TypeOperator for RangeTypeOp {
    fn handles(&self, op: BinaryOp) -> bool {
        matches!(op, BinaryOp::Range | BinaryOp::RangeInclusive)
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        _interner: &StringInterner,
        _op: BinaryOp,
        left: &Type,
        right: &Type,
        _span: Span,
    ) -> TypeOpResult {
        if let Err(e) = ctx.unify(left, right) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("range bounds must have the same type: {e:?}"),
                ErrorCode::E2001,
            ));
        }
        TypeOpResult::Ok(Type::Range(Box::new(ctx.resolve(left))))
    }
}

// Coalesce Type Operator

/// Type checking for coalesce operator: ??
pub struct CoalesceTypeOp;

impl TypeOperator for CoalesceTypeOp {
    fn handles(&self, op: BinaryOp) -> bool {
        matches!(op, BinaryOp::Coalesce)
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        _interner: &StringInterner,
        _op: BinaryOp,
        left: &Type,
        right: &Type,
        _span: Span,
    ) -> TypeOpResult {
        let inner = ctx.fresh_var();
        let option_ty = Type::Option(Box::new(inner.clone()));

        if let Err(e) = ctx.unify(left, &option_ty) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("left operand of ?? must be Option: {e:?}"),
                ErrorCode::E2001,
            ));
        }
        if let Err(e) = ctx.unify(&inner, right) {
            return TypeOpResult::Err(TypeOpError::new(
                format!("right operand of ?? must match Option inner type: {e:?}"),
                ErrorCode::E2001,
            ));
        }

        TypeOpResult::Ok(ctx.resolve(&inner))
    }
}

// Type Operator Registry

/// Registry of type operators.
///
/// Provides a way to type check binary operations by delegating to registered operators.
pub struct TypeOperatorRegistry {
    operators: Vec<Box<dyn TypeOperator>>,
}

impl TypeOperatorRegistry {
    /// Create a new type operator registry with all built-in operators.
    pub fn new() -> Self {
        TypeOperatorRegistry {
            operators: vec![
                Box::new(ArithmeticTypeOp),
                Box::new(ComparisonTypeOp),
                Box::new(LogicalTypeOp),
                Box::new(BitwiseTypeOp),
                Box::new(RangeTypeOp),
                Box::new(CoalesceTypeOp),
            ],
        }
    }

    /// Type check a binary operation.
    ///
    /// Tries each registered operator in order until one handles the operation.
    pub fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        op: BinaryOp,
        left: &Type,
        right: &Type,
        span: Span,
    ) -> TypeOpResult {
        for handler in &self.operators {
            if handler.handles(op) {
                return handler.check(ctx, interner, op, left, right, span);
            }
        }

        TypeOpResult::Err(TypeOpError::new(
            format!("unsupported binary operator: {op:?}"),
            ErrorCode::E2001,
        ))
    }
}

impl Default for TypeOperatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::SharedInterner;

    #[test]
    fn test_arithmetic_op_int() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::Add, &Type::Int, &Type::Int, span) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Int),
            TypeOpResult::Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_arithmetic_op_float() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::Mul, &Type::Float, &Type::Float, span) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Float),
            TypeOpResult::Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_string_concat() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::Add, &Type::Str, &Type::Str, span) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Str),
            TypeOpResult::Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_comparison_op() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::Eq, &Type::Int, &Type::Int, span) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Bool),
            TypeOpResult::Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_logical_op() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::And, &Type::Bool, &Type::Bool, span) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Bool),
            TypeOpResult::Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_bitwise_op() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::BitAnd, &Type::Int, &Type::Int, span) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Int),
            TypeOpResult::Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_range_op() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::Range, &Type::Int, &Type::Int, span) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Range(Box::new(Type::Int))),
            TypeOpResult::Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_coalesce_op() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        let option_int = Type::Option(Box::new(Type::Int));
        match registry.check(&mut ctx, &interner, BinaryOp::Coalesce, &option_int, &Type::Int, span) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Int),
            TypeOpResult::Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_type_mismatch() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::Add, &Type::Int, &Type::Str, span) {
            TypeOpResult::Ok(_) => panic!("expected error"),
            TypeOpResult::Err(_) => {} // Expected
        }
    }

    #[test]
    fn test_invalid_arithmetic_type() {
        let registry = TypeOperatorRegistry::new();
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match registry.check(&mut ctx, &interner, BinaryOp::Sub, &Type::Bool, &Type::Bool, span) {
            TypeOpResult::Ok(_) => panic!("expected error"),
            TypeOpResult::Err(_) => {} // Expected
        }
    }
}
