//! Option method handler.

use sigil_diagnostic::ErrorCode;
use sigil_ir::{Span, StringInterner};
use sigil_types::{InferenceContext, Type};

use super::{BuiltinMethodHandler, MethodTypeError, MethodTypeResult};

/// Type checking for Option methods.
pub struct OptionMethodHandler;

impl BuiltinMethodHandler for OptionMethodHandler {
    fn handles(&self, receiver_ty: &Type) -> bool {
        matches!(receiver_ty, Type::Option(_))
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        _interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        _args: &[Type],
        _span: Span,
    ) -> MethodTypeResult {
        let Type::Option(inner_ty) = receiver_ty else {
            unreachable!("handles() verified type is Option");
        };

        match method {
            "is_some" | "is_none" => MethodTypeResult::Ok(Type::Bool),
            "unwrap" | "unwrap_or" => MethodTypeResult::Ok((**inner_ty).clone()),
            "map" | "and_then" => {
                let result_inner = ctx.fresh_var();
                MethodTypeResult::Ok(Type::Option(Box::new(result_inner)))
            }
            "filter" => MethodTypeResult::Ok(Type::Option(inner_ty.clone())),
            "ok_or" => {
                let err_ty = ctx.fresh_var();
                MethodTypeResult::Ok(Type::Result {
                    ok: inner_ty.clone(),
                    err: Box::new(err_ty),
                })
            }
            _ => MethodTypeResult::Err(MethodTypeError::new(
                format!("unknown method `{method}` for type `Option<T>`"),
                ErrorCode::E2002,
            )),
        }
    }
}
