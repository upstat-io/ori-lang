//! Type-level binary operation checking for the type checker.
//!
//! Uses direct enum-based dispatch for the fixed set of built-in operators.
//! Pattern matching provides exhaustiveness checking and avoids trait object overhead.
//!
//! # Specification
//!
//! - Type rules: `docs/ori_lang/0.1-alpha/spec/operator-rules.md`
//! - Prose: `docs/ori_lang/0.1-alpha/spec/09-expressions.md`
//!
//! Implementation must match the inference rules in operator-rules.md.

use ori_diagnostic::ErrorCode;
use ori_ir::{BinaryOp, Span, StringInterner};
use ori_types::{InferenceContext, Type};

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
    #[cold]
    pub fn new(message: impl Into<String>, code: ErrorCode) -> Self {
        TypeOpError {
            message: message.into(),
            code,
        }
    }
}

/// Type check a binary operation using direct pattern matching.
pub fn check_binary_operation(
    ctx: &mut InferenceContext,
    interner: &StringInterner,
    op: BinaryOp,
    left: &Type,
    right: &Type,
    _span: Span,
) -> TypeOpResult {
    match op {
        // Arithmetic: +, -, *, /, %, div
        BinaryOp::Add | BinaryOp::Sub => {
            // Duration and Size support +/- between same types
            let left_resolved = ctx.resolve(left);
            let right_resolved = ctx.resolve(right);

            match (&left_resolved, &right_resolved) {
                (Type::Duration, Type::Duration) => return TypeOpResult::Ok(Type::Duration),
                (Type::Size, Type::Size) => return TypeOpResult::Ok(Type::Size),
                _ => {}
            }

            if let Err(e) = ctx.unify(left, right) {
                let msg = match e {
                    ori_types::TypeError::TypeMismatch { expected, found } => format!(
                        "type mismatch in arithmetic operation: expected `{}`, found `{}`",
                        expected.display(interner),
                        found.display(interner)
                    ),
                    _ => format!(
                        "type mismatch in arithmetic operation: operands have incompatible types `{}` and `{}`",
                        left.display(interner),
                        right.display(interner)
                    ),
                };
                return TypeOpResult::Err(TypeOpError::new(msg, ErrorCode::E2001));
            }

            let resolved = ctx.resolve(left);
            match resolved {
                Type::Str if op == BinaryOp::Add => TypeOpResult::Ok(Type::Str),
                Type::Int | Type::Float | Type::Var(_) => TypeOpResult::Ok(resolved),
                _ => {
                    let op_name = if op == BinaryOp::Add { "+" } else { "-" };
                    TypeOpResult::Err(TypeOpError::new(
                        format!(
                            "cannot apply `{}` to `{}`: operator requires numeric types (int or float), Duration, or Size",
                            op_name,
                            left.display(interner)
                        ),
                        ErrorCode::E2001,
                    ))
                }
            }
        }

        BinaryOp::Mul => {
            // Duration * int, int * Duration, Size * int, int * Size
            let left_resolved = ctx.resolve(left);
            let right_resolved = ctx.resolve(right);

            match (&left_resolved, &right_resolved) {
                (Type::Duration, Type::Int) | (Type::Int, Type::Duration) => {
                    return TypeOpResult::Ok(Type::Duration);
                }
                (Type::Size, Type::Int) | (Type::Int, Type::Size) => {
                    return TypeOpResult::Ok(Type::Size);
                }
                _ => {}
            }

            if let Err(e) = ctx.unify(left, right) {
                let msg = match e {
                    ori_types::TypeError::TypeMismatch { expected, found } => format!(
                        "type mismatch in arithmetic operation: expected `{}`, found `{}`",
                        expected.display(interner),
                        found.display(interner)
                    ),
                    _ => format!(
                        "type mismatch in arithmetic operation: operands have incompatible types `{}` and `{}`",
                        left.display(interner),
                        right.display(interner)
                    ),
                };
                return TypeOpResult::Err(TypeOpError::new(msg, ErrorCode::E2001));
            }

            let resolved = ctx.resolve(left);
            match resolved {
                Type::Int | Type::Float | Type::Var(_) => TypeOpResult::Ok(resolved),
                _ => TypeOpResult::Err(TypeOpError::new(
                    format!(
                        "cannot apply `*` to `{}`: multiplication requires numeric types (int or float), or Duration/Size with int",
                        left.display(interner)
                    ),
                    ErrorCode::E2001,
                )),
            }
        }

        BinaryOp::Div | BinaryOp::Mod => {
            // Duration / int, Duration % Duration, Size / int, Size % Size
            let left_resolved = ctx.resolve(left);
            let right_resolved = ctx.resolve(right);

            match (&left_resolved, &right_resolved) {
                (Type::Duration, Type::Int) if op == BinaryOp::Div => {
                    return TypeOpResult::Ok(Type::Duration);
                }
                (Type::Duration, Type::Duration) if op == BinaryOp::Mod => {
                    return TypeOpResult::Ok(Type::Duration);
                }
                (Type::Size, Type::Int) if op == BinaryOp::Div => {
                    return TypeOpResult::Ok(Type::Size);
                }
                (Type::Size, Type::Size) if op == BinaryOp::Mod => {
                    return TypeOpResult::Ok(Type::Size);
                }
                _ => {}
            }

            if let Err(e) = ctx.unify(left, right) {
                let msg = match e {
                    ori_types::TypeError::TypeMismatch { expected, found } => format!(
                        "type mismatch in arithmetic operation: expected `{}`, found `{}`",
                        expected.display(interner),
                        found.display(interner)
                    ),
                    _ => format!(
                        "type mismatch in arithmetic operation: operands have incompatible types `{}` and `{}`",
                        left.display(interner),
                        right.display(interner)
                    ),
                };
                return TypeOpResult::Err(TypeOpError::new(msg, ErrorCode::E2001));
            }

            let resolved = ctx.resolve(left);
            match resolved {
                Type::Int | Type::Float | Type::Var(_) => TypeOpResult::Ok(resolved),
                _ => {
                    let op_name = if op == BinaryOp::Div { "/" } else { "%" };
                    TypeOpResult::Err(TypeOpError::new(
                        format!(
                            "cannot apply `{}` to `{}`: operator requires numeric types (int or float)",
                            op_name,
                            left.display(interner)
                        ),
                        ErrorCode::E2001,
                    ))
                }
            }
        }

        BinaryOp::FloorDiv => {
            if let Err(e) = ctx.unify(left, right) {
                let msg = match e {
                    ori_types::TypeError::TypeMismatch { expected, found } => format!(
                        "type mismatch in arithmetic operation: expected `{}`, found `{}`",
                        expected.display(interner),
                        found.display(interner)
                    ),
                    _ => format!(
                        "type mismatch in arithmetic operation: operands have incompatible types `{}` and `{}`",
                        left.display(interner),
                        right.display(interner)
                    ),
                };
                return TypeOpResult::Err(TypeOpError::new(msg, ErrorCode::E2001));
            }

            let resolved = ctx.resolve(left);
            match resolved {
                Type::Int | Type::Float | Type::Var(_) => TypeOpResult::Ok(resolved),
                _ => TypeOpResult::Err(TypeOpError::new(
                    format!(
                        "cannot apply `div` to `{}`: floor division requires numeric types (int or float)",
                        left.display(interner)
                    ),
                    ErrorCode::E2001,
                )),
            }
        }

        // Comparison: ==, !=, <, <=, >, >=
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq => {
            // Duration and Size support all comparison operators
            let left_resolved = ctx.resolve(left);
            let right_resolved = ctx.resolve(right);

            match (&left_resolved, &right_resolved) {
                (Type::Duration, Type::Duration) | (Type::Size, Type::Size) => {
                    return TypeOpResult::Ok(Type::Bool);
                }
                _ => {}
            }

            if let Err(e) = ctx.unify(left, right) {
                let msg = match e {
                    ori_types::TypeError::TypeMismatch { expected, found } => format!(
                        "type mismatch in comparison: expected `{}`, found `{}`",
                        expected.display(interner),
                        found.display(interner)
                    ),
                    _ => format!(
                        "type mismatch in comparison: cannot compare `{}` with `{}`",
                        left.display(interner),
                        right.display(interner)
                    ),
                };
                return TypeOpResult::Err(TypeOpError::new(msg, ErrorCode::E2001));
            }
            TypeOpResult::Ok(Type::Bool)
        }

        // Logical: &&, ||
        BinaryOp::And | BinaryOp::Or => {
            if ctx.unify(left, &Type::Bool).is_err() {
                return TypeOpResult::Err(TypeOpError::new(
                    format!(
                        "left operand of logical operator must be `bool`, found `{}`",
                        left.display(interner)
                    ),
                    ErrorCode::E2001,
                ));
            }
            if ctx.unify(right, &Type::Bool).is_err() {
                return TypeOpResult::Err(TypeOpError::new(
                    format!(
                        "right operand of logical operator must be `bool`, found `{}`",
                        right.display(interner)
                    ),
                    ErrorCode::E2001,
                ));
            }
            TypeOpResult::Ok(Type::Bool)
        }

        // Bitwise: &, |, ^, <<, >>
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            if ctx.unify(left, &Type::Int).is_err() {
                return TypeOpResult::Err(TypeOpError::new(
                    format!(
                        "left operand of bitwise operator must be `int`, found `{}`",
                        left.display(interner)
                    ),
                    ErrorCode::E2001,
                ));
            }
            if ctx.unify(right, &Type::Int).is_err() {
                return TypeOpResult::Err(TypeOpError::new(
                    format!(
                        "right operand of bitwise operator must be `int`, found `{}`",
                        right.display(interner)
                    ),
                    ErrorCode::E2001,
                ));
            }
            TypeOpResult::Ok(Type::Int)
        }

        // Range: .., ..=
        BinaryOp::Range | BinaryOp::RangeInclusive => {
            if let Err(e) = ctx.unify(left, right) {
                let msg = match e {
                    ori_types::TypeError::TypeMismatch { expected, found } => format!(
                        "range bounds must have the same type: expected `{}`, found `{}`",
                        expected.display(interner),
                        found.display(interner)
                    ),
                    _ => format!(
                        "range bounds must have the same type: found `{}` and `{}`",
                        left.display(interner),
                        right.display(interner)
                    ),
                };
                return TypeOpResult::Err(TypeOpError::new(msg, ErrorCode::E2001));
            }
            TypeOpResult::Ok(Type::Range(Box::new(ctx.resolve(left))))
        }

        // Coalesce: ??
        // Supports two patterns:
        // - Option<T> ?? Option<T> -> Option<T> (chaining)
        // - Option<T> ?? T -> T (unwrapping with default)
        // Similarly for Result<T, E>
        BinaryOp::Coalesce => {
            let inner = ctx.fresh_var();
            let option_ty = Type::Option(Box::new(inner.clone()));

            // Left must be Option<T> (or Result<T, E>)
            let left_resolved = ctx.resolve(left);
            let is_result = matches!(left_resolved, Type::Result { .. });

            if is_result {
                // Handle Result<T, E> ?? ...
                if let Type::Result { ok: ok_ty, .. } = &left_resolved {
                    let ok_inner = ok_ty.as_ref().clone();
                    // Check if right is also Result<T, E> (chaining) or T (unwrap)
                    let right_resolved = ctx.resolve(right);

                    // Chaining: Result<T, E> ?? Result<T, E> -> Result<T, E>
                    if let Type::Result { ok: right_ok, .. } = &right_resolved {
                        if ctx.unify(&ok_inner, right_ok).is_ok() {
                            return TypeOpResult::Ok(left_resolved);
                        }
                    }

                    // Never type on the right: unwrap semantics (same reasoning as Option)
                    if matches!(right_resolved, Type::Never) {
                        return TypeOpResult::Ok(ctx.resolve(&ok_inner));
                    }

                    // Unwrapping: Result<T, E> ?? T -> T
                    if ctx.unify(&ok_inner, right).is_ok() {
                        return TypeOpResult::Ok(ctx.resolve(&ok_inner));
                    }
                    return TypeOpResult::Err(TypeOpError::new(
                        format!(
                            "right operand of `??` must be `{}` or `Result<{}, _>`, found `{}`",
                            ok_inner.display(interner),
                            ok_inner.display(interner),
                            right.display(interner)
                        ),
                        ErrorCode::E2001,
                    ));
                }
                unreachable!()
            }

            // Handle Option<T> ?? ...
            if ctx.unify(left, &option_ty).is_err() {
                return TypeOpResult::Err(TypeOpError::new(
                    format!(
                        "left operand of `??` must be `Option<T>` or `Result<T, E>`, found `{}`",
                        left.display(interner)
                    ),
                    ErrorCode::E2001,
                ));
            }

            // Check if right is also Option<T> (chaining) or T (unwrap)
            let right_resolved = ctx.resolve(right);

            // Chaining: Option<T> ?? Option<T> -> Option<T>
            if let Type::Option(right_inner) = &right_resolved {
                let inner_resolved = ctx.resolve(&inner);
                if ctx.unify(&inner_resolved, right_inner).is_ok() {
                    return TypeOpResult::Ok(Type::Option(Box::new(ctx.resolve(&inner))));
                }
            }

            // Never type on the right: unwrap semantics
            // With right-associative parsing, `Never` on the right only appears at
            // chain terminals (e.g., `opt ?? panic()`). Since the expression diverges
            // if we reach the right operand, return the unwrapped type T.
            // Chains like `a ?? panic() ?? 99` work because they parse as
            // `a ?? (panic() ?? 99)` where `panic()` is on the LEFT (unifies with Option<T>).
            if matches!(right_resolved, Type::Never) {
                return TypeOpResult::Ok(ctx.resolve(&inner));
            }

            // Unwrapping: Option<T> ?? T -> T
            if let Err(e) = ctx.unify(&inner, right) {
                let inner_resolved = ctx.resolve(&inner);
                let msg = match e {
                    ori_types::TypeError::TypeMismatch { expected, found } => format!(
                        "right operand of `??` must be `{}` or `Option<{}>`, found `{}`",
                        expected.display(interner),
                        expected.display(interner),
                        found.display(interner)
                    ),
                    _ => format!(
                        "right operand of `??` must be `{}` or `Option<{}>`, found `{}`",
                        inner_resolved.display(interner),
                        inner_resolved.display(interner),
                        right.display(interner)
                    ),
                };
                return TypeOpResult::Err(TypeOpError::new(msg, ErrorCode::E2001));
            }

            TypeOpResult::Ok(ctx.resolve(&inner))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::SharedInterner;

    #[test]
    fn test_arithmetic_op_int() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::Add,
            &Type::Int,
            &Type::Int,
            span,
        ) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Int),
            TypeOpResult::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_arithmetic_op_float() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::Mul,
            &Type::Float,
            &Type::Float,
            span,
        ) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Float),
            TypeOpResult::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_string_concat() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::Add,
            &Type::Str,
            &Type::Str,
            span,
        ) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Str),
            TypeOpResult::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_comparison_op() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::Eq,
            &Type::Int,
            &Type::Int,
            span,
        ) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Bool),
            TypeOpResult::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_logical_op() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::And,
            &Type::Bool,
            &Type::Bool,
            span,
        ) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Bool),
            TypeOpResult::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_bitwise_op() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::BitAnd,
            &Type::Int,
            &Type::Int,
            span,
        ) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Int),
            TypeOpResult::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_range_op() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::Range,
            &Type::Int,
            &Type::Int,
            span,
        ) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Range(Box::new(Type::Int))),
            TypeOpResult::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_coalesce_op() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        let option_int = Type::Option(Box::new(Type::Int));
        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::Coalesce,
            &option_int,
            &Type::Int,
            span,
        ) {
            TypeOpResult::Ok(ty) => assert_eq!(ty, Type::Int),
            TypeOpResult::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_type_mismatch() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::Add,
            &Type::Int,
            &Type::Str,
            span,
        ) {
            TypeOpResult::Ok(_) => panic!("expected error"),
            TypeOpResult::Err(_) => {}
        }
    }

    #[test]
    fn test_invalid_arithmetic_type() {
        let mut ctx = InferenceContext::new();
        let interner = SharedInterner::default();
        let span = Span::default();

        match check_binary_operation(
            &mut ctx,
            &interner,
            BinaryOp::Sub,
            &Type::Bool,
            &Type::Bool,
            span,
        ) {
            TypeOpResult::Ok(_) => panic!("expected error"),
            TypeOpResult::Err(_) => {}
        }
    }
}
