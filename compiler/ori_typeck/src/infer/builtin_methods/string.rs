//! String method handler.

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, StringInterner};
use ori_types::{InferenceContext, Type};

use super::{BuiltinMethodHandler, MethodTypeError, MethodTypeResult};

/// Type checking for string methods.
pub struct StringMethodHandler;

impl BuiltinMethodHandler for StringMethodHandler {
    fn handles(&self, receiver_ty: &Type) -> bool {
        matches!(receiver_ty, Type::Str)
    }

    fn check(
        &self,
        _ctx: &mut InferenceContext,
        _interner: &StringInterner,
        _receiver_ty: &Type,
        method: &str,
        _args: &[Type],
        _span: Span,
    ) -> MethodTypeResult {
        match method {
            "len" => MethodTypeResult::Ok(Type::Int),
            "is_empty" | "contains" | "starts_with" | "ends_with" => {
                MethodTypeResult::Ok(Type::Bool)
            }
            "to_uppercase" | "to_lowercase" | "trim" => MethodTypeResult::Ok(Type::Str),
            "split" => MethodTypeResult::Ok(Type::List(Box::new(Type::Str))),
            "chars" => MethodTypeResult::Ok(Type::List(Box::new(Type::Char))),
            "bytes" => MethodTypeResult::Ok(Type::List(Box::new(Type::Byte))),
            _ => MethodTypeResult::Err(MethodTypeError::new(
                format!("unknown method `{method}` for type `str`"),
                ErrorCode::E2002,
            )),
        }
    }
}
