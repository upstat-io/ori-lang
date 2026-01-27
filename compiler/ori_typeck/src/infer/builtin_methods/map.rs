//! Map method handler.

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, StringInterner};
use ori_types::{InferenceContext, Type};

use super::{BuiltinMethodHandler, MethodTypeError, MethodTypeResult};

/// Type checking for map methods.
pub struct MapMethodHandler;

impl BuiltinMethodHandler for MapMethodHandler {
    fn handles(&self, receiver_ty: &Type) -> bool {
        matches!(receiver_ty, Type::Map { .. })
    }

    fn check(
        &self,
        _ctx: &mut InferenceContext,
        _interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        _args: &[Type],
        _span: Span,
    ) -> MethodTypeResult {
        let Type::Map { key: key_ty, value: val_ty } = receiver_ty else {
            unreachable!("handles() verified type is Map");
        };

        match method {
            "len" => MethodTypeResult::Ok(Type::Int),
            "is_empty" | "contains_key" => MethodTypeResult::Ok(Type::Bool),
            "get" | "insert" | "remove" => MethodTypeResult::Ok(Type::Option(val_ty.clone())),
            "keys" => MethodTypeResult::Ok(Type::List(key_ty.clone())),
            "values" => MethodTypeResult::Ok(Type::List(val_ty.clone())),
            _ => MethodTypeResult::Err(MethodTypeError::new(
                format!("unknown method `{method}` for type `{{K: V}}`"),
                ErrorCode::E2002,
            )),
        }
    }
}
