//! Numeric method handler for int, float, and bool types.

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, StringInterner};
use ori_types::{InferenceContext, Type};

use super::{BuiltinMethodHandler, MethodTypeError, MethodTypeResult};

/// Type checking for numeric type methods (int, float, bool).
pub struct NumericMethodHandler;

impl BuiltinMethodHandler for NumericMethodHandler {
    fn handles(&self, receiver_ty: &Type) -> bool {
        matches!(receiver_ty, Type::Int | Type::Float | Type::Bool)
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        _args: &[Type],
        _span: Span,
    ) -> MethodTypeResult {
        match receiver_ty {
            Type::Int => check_int_method(interner, method),
            Type::Float => check_float_method(interner, method),
            Type::Bool => check_bool_method(method),
            Type::Var(_) => MethodTypeResult::Ok(ctx.fresh_var()),
            _ => unreachable!("handles() verified type is numeric"),
        }
    }
}

/// Common methods shared by int and float: `to_string`, `compare`.
fn common_numeric_method(interner: &StringInterner, method: &str) -> Option<MethodTypeResult> {
    match method {
        "to_string" => Some(MethodTypeResult::Ok(Type::Str)),
        "compare" => Some(MethodTypeResult::Ok(Type::Named(
            interner.intern("Ordering"),
        ))),
        _ => None,
    }
}

fn check_int_method(interner: &StringInterner, method: &str) -> MethodTypeResult {
    if let Some(result) = common_numeric_method(interner, method) {
        return result;
    }
    match method {
        "abs" | "min" | "max" => MethodTypeResult::Ok(Type::Int),
        _ => MethodTypeResult::Err(MethodTypeError::new(
            format!("unknown method `{method}` for type `int`"),
            ErrorCode::E2002,
        )),
    }
}

fn check_float_method(interner: &StringInterner, method: &str) -> MethodTypeResult {
    if let Some(result) = common_numeric_method(interner, method) {
        return result;
    }
    match method {
        "abs" | "floor" | "ceil" | "round" | "sqrt" | "min" | "max" => {
            MethodTypeResult::Ok(Type::Float)
        }
        _ => MethodTypeResult::Err(MethodTypeError::new(
            format!("unknown method `{method}` for type `float`"),
            ErrorCode::E2002,
        )),
    }
}

fn check_bool_method(method: &str) -> MethodTypeResult {
    if method == "to_string" {
        MethodTypeResult::Ok(Type::Str)
    } else {
        MethodTypeResult::Err(MethodTypeError::new(
            format!("unknown method `{method}` for type `bool`"),
            ErrorCode::E2002,
        ))
    }
}
